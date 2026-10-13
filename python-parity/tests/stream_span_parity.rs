//! The crate's `Stream::elements_in_span` against music21's
//! `getElementsByOffset`, which music21's own comment calls a beast.
//!
//! Each corpus score is read by music21 and by `from_musicxml` (which
//! `musicxml_read_parity` holds to music21's reader). Every measure of every
//! part is then asked the same thirty-eight questions: six offsets with
//! music21's own defaults, and both a stretch and a span of no length under
//! each of the sixteen settings of the four flags. What each question finds
//! -- the class of every element, where it stands and how long it lasts --
//! must be the same list on both sides. Offsets and lengths are compared bit
//! for bit.
//!
//! Only the notes, rests and chords of a measure are compared. music21 keeps
//! a measure's barlines among its elements and the crate keeps them as the
//! measure's own, so the two element lists are not the same list to begin
//! with; what the flags discriminate on is an offset and a length, which
//! every sounding element has.
//!
//! `STREAM_SPAN_PARITY_SCORES`, a `;`-separated list of corpus names, runs
//! those instead of the writer test's scores, which is how the corpus is
//! swept.

use music21_rs::musicxml::from_musicxml;
use music21_rs::stream::OffsetSpan;
use music21_rs::{StreamElement, StreamKind};
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

def describe(m, e):
    return f'{kind(e)}@{bits(m.elementOffset(e))}/{bits(e.quarterLength)}'

def answers(part, cases):
    """One line per measure per case: what the span finds, sorted."""
    out = []
    for m in part.getElementsByClass(stream.Measure):
        for start, end, endBoundary, finishIn, beginIn, endAtStart in cases:
            found = m.getElementsByOffset(
                start,
                end,
                includeEndBoundary=endBoundary,
                mustFinishInSpan=finishIn,
                mustBeginInSpan=beginIn,
                includeElementsThatEndAtStart=endAtStart,
            )
            items = sorted(describe(m, e) for e in found if kind(e) is not None)
            out.append(' '.join(items))
    return out

def parts_of(text):
    from music21 import converter
    score = converter.parse(text, format='musicxml', forceSource=True)
    return list(score.parts)
"#;

/// One question: where the span starts, where it ends, and the four flags
/// in music21's keyword order.
type Case = (f64, Option<f64>, bool, bool, bool, bool);

/// The offsets asked with music21's own defaults, and the two spans asked
/// under every setting of the four flags.
fn cases() -> Vec<Case> {
    let mut cases: Vec<Case> = Vec::new();
    for start in [0.0, 1.0 / 3.0, 0.5, 1.0, 2.0, 4.0] {
        cases.push((start, None, true, false, true, true));
    }
    for end in [Some(3.0), None] {
        for flags in 0..16u8 {
            cases.push((
                1.0,
                end,
                flags & 1 != 0,
                flags & 2 != 0,
                flags & 4 != 0,
                flags & 8 != 0,
            ));
        }
    }
    cases
}

/// A case as a span the crate reads.
fn span_of(case: &Case) -> OffsetSpan {
    let &(start, end, include_end_boundary, must_finish_in_span, must_begin_in_span, end_at_start) =
        case;
    OffsetSpan {
        start,
        end,
        include_end_boundary,
        must_finish_in_span,
        must_begin_in_span,
        include_elements_that_end_at_start: end_at_start,
    }
}

/// What music21 calls an element the crate models as sounding, or nothing
/// for one it does not compare.
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

/// A number's bits, as the Python side spells them, so offsets and lengths
/// are compared exactly.
fn bits(value: f64) -> String {
    value.to_bits().to_string()
}

#[test]
fn the_crate_finds_elements_in_a_span_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let cases = cases();
    let (failures, compared, scores) = Python::attach(|py| -> PyResult<(Vec<String>, usize, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"stream_span_parity_music21.py",
            c"stream_span_parity_music21",
        )?;

        let chosen: Vec<String> = match std::env::var("STREAM_SPAN_PARITY_SCORES") {
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
        let mut scores = 0;
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
            scores += 1;
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
                let their_answers: Vec<String> = music21
                    .getattr("answers")?
                    .call1((their_part, cases.clone()))?
                    .extract()?;
                let our_measures = our_part.measures();
                let mut our_answers: Vec<String> = Vec::new();
                for measure in &our_measures {
                    for case in &cases {
                        let mut items: Vec<String> = measure
                            .elements_in_span(&span_of(case))
                            .into_iter()
                            .filter_map(|event| {
                                kind(event.element()).map(|kind| {
                                    format!(
                                        "{kind}@{}/{}",
                                        bits(event.offset()),
                                        bits(event.element().quarter_length())
                                    )
                                })
                            })
                            .collect();
                        items.sort();
                        our_answers.push(items.join(" "));
                    }
                }
                if our_answers.len() != their_answers.len() {
                    failures.push(format!(
                        "{name} part {at}: {} answers from music21, {} from the crate",
                        their_answers.len(),
                        our_answers.len()
                    ));
                    continue;
                }
                compared += our_answers.len();
                for (place, (ours, theirs)) in our_answers.iter().zip(&their_answers).enumerate() {
                    if ours == theirs {
                        continue;
                    }
                    let case = &cases[place % cases.len()];
                    failures.push(format!(
                        "{name} part {at} measure {} case {case:?}:\n  music21: {theirs}\n  crate  : {ours}",
                        place / cases.len()
                    ));
                }
            }
        }
        Ok((failures, compared, scores))
    })
    .unwrap_or_else(|error| panic!("asking music21 for the elements in a span: {error}"));

    let shown: Vec<&String> = failures.iter().take(12).collect();
    assert!(
        failures.is_empty(),
        "{} of {compared} answers differ, first {}:\n{}",
        failures.len(),
        shown.len(),
        shown
            .iter()
            .map(|line| line.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(compared > 0, "no span was compared");
    println!("{compared} spans compared over {} scores", scores);
}
