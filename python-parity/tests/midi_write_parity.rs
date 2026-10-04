//! The crate's MIDI writer against music21's, byte for byte.
//!
//! Each subject is read twice, by music21's reader and by the crate's, and
//! each side writes what it read as a MIDI file: music21 with
//! `midi.translate.streamToMidiFile(...).writestr()`, the crate with
//! `music21_rs::midi::to_midi`. The readers are held to music21's by their
//! own tests, so two files differ only where the writers do. A difference
//! is reported as the first event that differs, both files read back by
//! music21's own MIDI reader.
//!
//! music21 is music21 here, with nothing of the crate installed over it.
//!
//! A subject is named by its kind and a name: `xml:` and a corpus score kept
//! as MusicXML, `tiny:` and a TinyNotation line, and `built:` and a stream
//! each side builds itself, for what no corpus score holds. `MIDI_WRITE_SUBJECTS`,
//! a `;`-separated list, runs those instead of the ones below.

use music21_rs::midi::{ExportOptions, to_midi};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

/// Subjects both sides write alike, and what each exercises.
const SUBJECTS: &[(&str, &str)] = &[
    ("tiny:4/4 c4 d e f g1", "one line of notes"),
    ("tiny:3/4 c4~ c8 d e4 f#4 g a", "a tie folded into one note"),
    ("tiny:2/4 trip{c8 d e} f4", "a triplet"),
    ("tiny:4/4 c4 r4 e4 r4", "rests between notes"),
    ("xml:bach/bwv66.6.mxl", "a chorale in four parts"),
    (
        "xml:demos/ComprehensiveChordSymbolsTestFile.mxl",
        "chord symbols sounding, one of a kind music21 lacks",
    ),
    ("xml:demos/drum_sample.xml", "strokes on the drum channel"),
    (
        "xml:webern/webern_dormi_jesu_op_16_no_2.mxl",
        "lyrics, and a dynamic under triplets",
    ),
    (
        "xml:mozart/k155/movement3.mxl",
        "a dynamic's span ending a hair past the next one's start",
    ),
    (
        "xml:beethoven/opus59no1/movement3.mxl",
        "a string quartet with many dynamics",
    ),
    (
        "built:microtones",
        "overlapping notes between the keys, each bent on a channel of its own",
    ),
];

const HELPERS: &str = r#"
import zipfile
from music21 import converter, corpus, midi

def source_text(name):
    path = str(corpus.getWork(name))
    if path.lower().endswith('.mxl'):
        with zipfile.ZipFile(path) as archive:
            container = archive.read('META-INF/container.xml').decode('utf-8')
            import re
            root = re.search(r'full-path="([^"]+)"', container).group(1)
            data = archive.read(root)
    else:
        with open(path, 'rb') as handle:
            data = handle.read()
    for encoding in ('utf-8', 'utf-16'):
        try:
            return data.decode(encoding)
        except UnicodeDecodeError:
            pass
    return data.decode('latin-1')

def read(kind, name):
    if kind == 'xml':
        return corpus.parse(name, forceSource=True)
    if kind == 'built':
        return built(name)
    return converter.parse('tinyNotation: ' + name)

def built(name):
    from music21 import note, stream
    part = stream.Part()
    if name == 'microtones':
        for offset, written, cents, length in [
            (0, 'C4', 50, 2), (1, 'E4', -30, 2), (1, 'G4', 0, 1), (3, 'A4', 25, 1)
        ]:
            n = note.Note(written, quarterLength=length)
            if cents:
                n.pitch.microtone = cents
            part.insert(offset, n)
    return part

def written(score):
    return midi.translate.streamToMidiFile(score).writestr()

def events(data):
    mf = midi.MidiFile()
    mf.readstr(data)
    lines = []
    for number, track in enumerate(mf.tracks):
        lines.append(f'track {number}')
        for event in track.events:
            lines.append('  ' + repr(event))
    return lines
"#;

#[test]
fn the_crate_writes_midi_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, count) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let _ = py.import("music21")?;
        let helpers = PyModule::from_code(
            py,
            &std::ffi::CString::new(HELPERS).expect("no nul in the helper"),
            c"midi_write_parity_helpers.py",
            c"midi_write_parity_helpers",
        )?;

        let chosen: Vec<String> = match std::env::var("MIDI_WRITE_SUBJECTS") {
            Ok(list) => list
                .split(';')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect(),
            Err(_) => SUBJECTS.iter().map(|(name, _)| name.to_string()).collect(),
        };
        let mut failures = Vec::new();
        for subject in &chosen {
            let Some((kind, name)) = subject.split_once(':') else {
                failures.push(format!("{subject}: no kind given"));
                continue;
            };
            let theirs: Vec<u8> = match helpers
                .getattr("read")?
                .call1((kind, name))
                .and_then(|score| helpers.getattr("written")?.call1((score,)))
                .and_then(|bytes| bytes.extract())
            {
                Ok(bytes) => bytes,
                Err(error) => {
                    failures.push(format!("{subject}: music21 could not write it: {error}"));
                    continue;
                }
            };
            let read = match kind {
                "xml" => {
                    let text: String = helpers.getattr("source_text")?.call1((name,))?.extract()?;
                    music21_rs::musicxml::from_musicxml(&text)
                }
                "tiny" => music21_rs::tinynotation::from_tiny_notation(name),
                "built" => built(name),
                other => {
                    failures.push(format!("{subject}: no subject of kind {other}"));
                    continue;
                }
            };
            let ours = match read.and_then(|score| to_midi(&score, &ExportOptions::default())) {
                Ok(bytes) => bytes,
                Err(error) => {
                    failures.push(format!("{subject}: the crate could not write it: {error}"));
                    continue;
                }
            };
            if ours == theirs {
                continue;
            }
            let decode = |bytes: &[u8]| -> PyResult<Vec<String>> {
                helpers
                    .getattr("events")?
                    .call1((pyo3::types::PyBytes::new(py, bytes),))?
                    .extract()
            };
            let (left, right) = match (decode(&theirs), decode(&ours)) {
                (Ok(left), Ok(right)) => (left, right),
                (_, Err(error)) => {
                    failures.push(format!(
                        "{subject}: music21 cannot read the crate's file: {error}"
                    ));
                    continue;
                }
                (Err(error), _) => {
                    failures.push(format!(
                        "{subject}: music21 cannot read its own file: {error}"
                    ));
                    continue;
                }
            };
            let at = left
                .iter()
                .zip(&right)
                .position(|(a, b)| a != b)
                .unwrap_or(left.len().min(right.len()));
            let from = at.saturating_sub(3);
            let show = |lines: &[String]| {
                lines[from.min(lines.len())..(at + 3).min(lines.len())].join("\n")
            };
            failures.push(format!(
                "{subject}: event {at} differs ({} events from music21, {} from the crate)\n\
                 music21:\n{}\ncrate:\n{}",
                left.len(),
                right.len(),
                show(&left),
                show(&right)
            ));
        }
        Ok((failures, chosen.len()))
    })
    .expect("the Python side of the comparison");

    assert!(
        failures.is_empty(),
        "{} of {count} subjects are written differently:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

/// The streams the helper's `built` makes, made the same way here.
fn built(name: &str) -> music21_rs::Result<music21_rs::Stream> {
    use music21_rs::{Duration, Note, Stream, StreamKind};
    let mut part = Stream::with_kind(StreamKind::Part);
    if name == "microtones" {
        for (offset, written, cents, length) in [
            (0.0, "C4", 50.0, 2.0),
            (1.0, "E4", -30.0, 2.0),
            (1.0, "G4", 0.0, 1.0),
            (3.0, "A4", 25.0, 1.0),
        ] {
            let mut note = Note::from_name(written)?.with_duration(Duration::new(length)?);
            if cents != 0.0 {
                let mut pitch = note.pitch().clone();
                pitch.set_microtone_cents(cents)?;
                note.set_pitch(pitch);
            }
            part.insert(offset, note);
        }
    }
    Ok(part)
}
