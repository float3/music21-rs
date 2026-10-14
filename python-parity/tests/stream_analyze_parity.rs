//! The crate's `analyze`, `is_twelve_tone` and `notes_and_rests` against
//! music21's `Stream.analyze`, `isTwelveTone` and `notesAndRests`.
//!
//! Two questions, asked separately because they cost differently. Which
//! analysis a method name asks for is a pure reading of the name, so every
//! name music21 answers to is asked of one small stream, together with
//! names that are near misses and nonsense. What each analysis then
//! *answers* needs the analysis run, so a handful of names are asked of
//! every corpus score.
//!
//! Each corpus score is read by music21 and by `from_musicxml` (which
//! `musicxml_read_parity` holds to music21's reader). The score and each
//! part are asked whether they are twelve-tone, and each part flattened for
//! its notes and rests, compared by class, offset and length, bit for bit.
//!
//! A key's flats are spelled `b` on both sides before comparing: the crate
//! writes a flat `b` where music21 writes `-`, by its own documented
//! choice, and each reads the other.
//!
//! `STREAM_ANALYZE_PARITY_SCORES`, a `;`-separated list of corpus names,
//! runs those instead of the writer test's scores, which is how the corpus
//! is swept.

use music21_rs::musicxml::from_musicxml;
use music21_rs::stream::Analysis;
use music21_rs::{Stream, StreamElement, StreamKind};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::SCORES;

const MUSIC21: &str = r#"
import struct
import zipfile
from music21 import chord, harmony, note, percussion, stream
from music21.analysis import discrete

def bits(value):
    return str(struct.unpack('<Q', struct.pack('<d', float(value)))[0])

def source_text(name):
    from music21 import corpus
    path = corpus.getWork(name)
    if isinstance(path, (list, tuple)):
        path = path[0]
    path = str(path)
    lower = path.lower()
    if lower.endswith('.mxl'):
        with zipfile.ZipFile(path) as archive:
            names = [n for n in archive.namelist()
                     if not n.startswith('META-INF') and n.lower().endswith(('.xml', '.musicxml'))]
            data = archive.read(names[0])
    elif lower.endswith(('.xml', '.musicxml')):
        with open(path, 'rb') as handle:
            data = handle.read()
    else:
        return None
    for encoding in ('utf-8-sig', 'utf-16'):
        try:
            return data.decode(encoding)
        except UnicodeDecodeError:
            pass
    return data.decode('latin-1')

def every_method_name():
    """Every name music21 answers to, plus near misses and nonsense."""
    names = {'range', 'key', 'ambitus', 'span', 'keyscape'}
    for cls in (discrete.Ambitus, discrete.KrumhanslSchmuckler, discrete.AardenEssen,
                discrete.SimpleWeights, discrete.BellmanBudge,
                discrete.TemperleyKostkaPayne):
        names.add(cls.__name__)
        names.add(cls.__name__.lower())
        names.update(cls.identifiers)
    names.update({'', 'KEY', 'Key', 'AMBITUS', 'ambit', 'amb', 'temp', 'krum',
                  'unknown-format', 'nonsense', 'key.', '.key', 'keys',
                  'Simple Weight Key Analysis', 'Analysis', 'analysis'})
    return sorted(names)

def dispatch(name):
    """The class a name asks for, or `none`."""
    found = discrete.analysisClassFromMethodName('span' if name == 'range' else name)
    return 'none' if found is None else found.__name__

def flats_as_b(name):
    """music21 writes a flat `-` and the crate writes it `b`, by its own
    choice, each reading the other; spell them the same way to compare."""
    return name.replace('-', 'b')

def answer(s, name):
    """What the analysis answers, or `error` where music21 refuses."""
    try:
        found = s.analyze(name)
    except Exception:
        return 'error'
    if found is None:
        return 'none'
    if hasattr(found, 'name') and hasattr(found, 'semitones'):
        return f'interval {found.name}'
    return f'key {flats_as_b(found.tonicPitchNameWithCase)} {found.mode}'

def parts_of(text):
    from music21 import converter
    return list(converter.parse(text, format='musicxml', forceSource=True).parts)

def twelve_tone(text):
    """The score's answer, then each part's."""
    from music21 import converter
    score = converter.parse(text, format='musicxml', forceSource=True)
    return [score.isTwelveTone()] + [p.isTwelveTone() for p in score.parts]

def kind(e):
    if isinstance(e, (harmony.ChordSymbol, harmony.NoChord)):
        return 'symbol'
    if isinstance(e, percussion.PercussionChord):
        return 'pchord'
    if isinstance(e, chord.Chord):
        return 'chord'
    if isinstance(e, note.Unpitched):
        return 'unpitched'
    if isinstance(e, note.Note):
        return 'note'
    if isinstance(e, note.Rest):
        return 'rest'
    return None

def notes_and_rests(part):
    flat = part.flatten()
    out = []
    for e in flat.notesAndRests:
        out.append(f'{kind(e)}@{bits(flat.elementOffset(e))}/{bits(e.quarterLength)}')
    return out

def small_stream():
    s = stream.Stream()
    for n in ('C4', 'E4', 'G4', 'C5'):
        s.append(note.Note(n))
    return s
"#;

/// The names asked of every corpus score, one per analysis.
const PER_SCORE: [&str; 6] = [
    "ambitus",
    "key",
    "krumhansl",
    "simple",
    "bellman",
    "temperley",
];

/// What music21 calls an element the crate models as sounding.
fn kind(element: &StreamElement) -> Option<&'static str> {
    match element {
        StreamElement::ChordSymbol(_) => Some("symbol"),
        StreamElement::PercussionChord(_) => Some("pchord"),
        StreamElement::Chord(_) => Some("chord"),
        StreamElement::Unpitched(_) => Some("unpitched"),
        StreamElement::Note(_) => Some("note"),
        StreamElement::Rest(_) => Some("rest"),
        _ => None,
    }
}

/// How the crate spells an analysis, as the Python side spells it.
fn answer(stream: &Stream, method: &str) -> String {
    match stream.analyze(method) {
        Err(_) => "error".to_string(),
        Ok(None) => "none".to_string(),
        Ok(Some(Analysis::Ambitus(span))) => format!("interval {}", span.short_name()),
        Ok(Some(Analysis::Key(key))) => {
            format!("key {} {}", key.tonic_pitch_name_with_case(), key.mode())
        }
    }
}

#[test]
fn the_crate_analyzes_a_stream_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"stream_analyze_parity_music21.py",
            c"stream_analyze_parity_music21",
        )?;

        let mut failures = Vec::new();
        let mut compared = 0;

        // Every method name, asked of one small stream.
        let names: Vec<String> = music21
            .getattr("every_method_name")?
            .call0()?
            .extract()?;
        let their_small = music21.getattr("small_stream")?.call0()?;
        let mut our_small = Stream::new();
        for name in ["C4", "E4", "G4", "C5"] {
            our_small.push(music21_rs::Note::from_name(name).expect("a note name"));
        }
        for name in &names {
            let theirs: String = music21
                .getattr("answer")?
                .call1((&their_small, name))?
                .extract()?;
            let ours = answer(&our_small, name);
            compared += 1;
            if ours != theirs {
                let asked: String = music21.getattr("dispatch")?.call1((name,))?.extract()?;
                failures.push(format!(
                    "{name:?} asks for {asked}: music21 {theirs:?}, the crate {ours:?}"
                ));
            }
        }

        let chosen: Vec<String> = match std::env::var("STREAM_ANALYZE_PARITY_SCORES") {
            Ok(list) => list
                .split(';')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect(),
            Err(_) => SCORES
                .iter()
                .map(|(name, _)| name.to_string())
                .filter(|name| !name.starts_with("built:"))
                .collect(),
        };

        for name in &chosen {
            let Some(text): Option<String> =
                music21.getattr("source_text")?.call1((name,))?.extract()?
            else {
                continue;
            };
            let ours = match from_musicxml(&text) {
                Ok(score) => score,
                Err(error) => {
                    failures.push(format!("{name}: the crate could not read it: {error}"));
                    continue;
                }
            };
            if ours.kind() != StreamKind::Score {
                continue;
            }
            let their_parts: Vec<Bound<'_, PyAny>> =
                music21.getattr("parts_of")?.call1((&text,))?.extract()?;
            let our_parts = ours.parts();
            if our_parts.len() != their_parts.len() {
                failures.push(format!(
                    "{name}: {} parts to music21, {} to the crate",
                    their_parts.len(),
                    our_parts.len()
                ));
                continue;
            }

            let their_twelve: Vec<bool> =
                music21.getattr("twelve_tone")?.call1((&text,))?.extract()?;
            let mut our_twelve = vec![ours.is_twelve_tone()];
            our_twelve.extend(our_parts.iter().map(|part| part.is_twelve_tone()));
            compared += 1;
            if our_twelve != their_twelve {
                failures.push(format!(
                    "{name}: twelve-tone is {their_twelve:?} to music21, {our_twelve:?} to the crate"
                ));
            }

            for (at, (our_part, their_part)) in our_parts.iter().zip(&their_parts).enumerate() {
                for method in PER_SCORE {
                    let theirs: String = music21
                        .getattr("answer")?
                        .call1((their_part, method))?
                        .extract()?;
                    let ours = answer(our_part, method);
                    compared += 1;
                    if ours != theirs {
                        failures.push(format!(
                            "{name} part {at} {method:?}: music21 {theirs:?}, the crate {ours:?}"
                        ));
                    }
                }

                let their_notes: Vec<String> = music21
                    .getattr("notes_and_rests")?
                    .call1((their_part,))?
                    .extract()?;
                let our_notes: Vec<String> = our_part
                    .notes_and_rests()
                    .into_iter()
                    .filter_map(|(offset, element)| {
                        kind(&element).map(|kind| {
                            format!(
                                "{kind}@{}/{}",
                                offset.to_bits(),
                                element.quarter_length().to_bits()
                            )
                        })
                    })
                    .collect();
                compared += 1;
                if our_notes != their_notes {
                    failures.push(format!(
                        "{name} part {at}: {} notes and rests to music21, {} to the crate",
                        their_notes.len(),
                        our_notes.len()
                    ));
                }
            }
        }
        Ok((failures, compared))
    })
    .unwrap_or_else(|error| panic!("asking music21 to analyze a stream: {error}"));

    let shown: Vec<&str> = failures.iter().take(10).map(String::as_str).collect();
    assert!(
        failures.is_empty(),
        "{} of {compared} answers differ, first {}:\n{}",
        failures.len(),
        shown.len(),
        shown.join("\n")
    );
    assert!(compared > 0, "nothing was compared");
    println!("{compared} answers compared");
}
