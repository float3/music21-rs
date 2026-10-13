//! The crate's `extend_duration`, `merge_elements` and `clone_empty`
//! against music21's `extendDuration`, `mergeElements` and `cloneEmpty`.
//!
//! Each corpus score is read by music21 and by `from_musicxml` (which
//! `musicxml_read_parity` holds to music21's reader). Every measure of
//! every part is stretched by `GeneralNote` and by `Note`, merged into an
//! empty stream whole and filtered to notes, and emptied; each answer is
//! the class, offset and length of everything the measure then holds,
//! compared bit for bit.
//!
//! What `clone_empty` carries over is not compared, only that it carries
//! nothing: music21 copies an id only where one was set, an auto id being
//! the object's own address, and the crate has no auto ids to tell apart.
//!
//! Only the notes, rests and chords are compared, as `stream_span_parity`
//! compares them: music21 keeps a measure's barlines among its elements
//! and the crate keeps them as the measure's own. That also keeps the
//! comparison off the one difference the crate documents -- music21
//! stretches any element, every object there carrying a duration, where
//! the crate keeps a dynamic or a clef timeless and has nothing to write.
//!
//! `STREAM_FILLING_PARITY_SCORES`, a `;`-separated list of corpus names,
//! runs those instead of the writer test's scores, which is how the corpus
//! is swept.

use music21_rs::musicxml::from_musicxml;
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

def parts_of(text):
    from music21 import converter
    return list(converter.parse(text, format='musicxml', forceSource=True).parts)

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

def dump(s):
    """Every sounding element of a stream, sorted, as text."""
    items = []
    for e in s:
        if kind(e) is None:
            continue
        items.append(f'{kind(e)}@{bits(s.elementOffset(e))}/{bits(e.quarterLength)}')
    return ' '.join(sorted(items))

CLASSES = {'GeneralNote': note.GeneralNote, 'Note': note.Note}

def answers(part):
    """For each measure: stretched two ways, merged two ways, emptied."""
    out = []
    for m in part.getElementsByClass(stream.Measure):
        for name in ('GeneralNote', 'Note'):
            out.append(dump(m.extendDuration(CLASSES[name], inPlace=False)))
        whole = stream.Measure()
        whole.mergeElements(m)
        out.append(dump(whole))
        notes = stream.Measure()
        notes.mergeElements(m, classFilterList=('Note',))
        out.append(dump(notes))
        out.append(f'emptied {len(m.cloneEmpty())}')
    return out
"#;

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

/// Every sounding element of a stream, sorted, as the Python side spells it.
fn dump(stream: &Stream) -> String {
    let mut items: Vec<String> = stream
        .events()
        .iter()
        .filter_map(|event| {
            kind(event.element()).map(|kind| {
                format!(
                    "{kind}@{}/{}",
                    event.offset().to_bits(),
                    event.element().quarter_length().to_bits()
                )
            })
        })
        .collect();
    items.sort();
    items.join(" ")
}

#[test]
fn the_crate_fills_and_empties_a_stream_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"stream_filling_parity_music21.py",
            c"stream_filling_parity_music21",
        )?;

        let chosen: Vec<String> = match std::env::var("STREAM_FILLING_PARITY_SCORES") {
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

        let mut failures = Vec::new();
        let mut compared = 0;
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

            for (at, (our_part, their_part)) in our_parts.iter().zip(&their_parts).enumerate() {
                let theirs: Vec<String> = music21
                    .getattr("answers")?
                    .call1((their_part,))?
                    .extract()?;
                let mut ours: Vec<String> = Vec::new();
                for measure in our_part.measures() {
                    for classes in [&["GeneralNote"][..], &["Note"][..]] {
                        match measure.extend_duration(classes) {
                            Ok(stretched) => ours.push(dump(&stretched)),
                            Err(error) => ours.push(format!("refused: {error}")),
                        }
                    }
                    let mut whole = Stream::with_kind(StreamKind::Measure);
                    whole.merge_elements(measure, None);
                    ours.push(dump(&whole));
                    let mut notes = Stream::with_kind(StreamKind::Measure);
                    notes.merge_elements(measure, Some(&["Note"]));
                    ours.push(dump(&notes));
                    ours.push(format!("emptied {}", measure.clone_empty().len()));
                }
                if ours.len() != theirs.len() {
                    failures.push(format!(
                        "{name} part {at}: {} answers from music21, {} from the crate",
                        theirs.len(),
                        ours.len()
                    ));
                    continue;
                }
                compared += ours.len();
                for (place, (ours, theirs)) in ours.iter().zip(&theirs).enumerate() {
                    if ours != theirs {
                        failures.push(format!(
                            "{name} part {at} answer {place}:\n  music21: {theirs}\n  crate  : {ours}"
                        ));
                    }
                }
            }
        }
        Ok((failures, compared))
    })
    .unwrap_or_else(|error| panic!("asking music21 to fill a stream: {error}"));

    let shown: Vec<&str> = failures.iter().take(8).map(String::as_str).collect();
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
