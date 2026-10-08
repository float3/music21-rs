//! The crate's hashes and alignments against music21's:
//! `alpha.analysis.hasher` and `alpha.analysis.aligner`.
//!
//! The text of each corpus score is read by music21 and by `from_musicxml`
//! (which `musicxml_read_parity` holds to music21's reader). Every note, rest
//! and chord is hashed by both as music21's `Hasher` does -- by default, by
//! pitch name with and without octave, with octaves, unrounded, chords as
//! chords with their normal order and prime form, and with the interval from
//! the note before and accidentals -- each hash with what it was made from.
//! The interval hash is compared on scores of up to 600 notes and rests with
//! no part split into staves: music21 takes time growing with the square of a
//! score's length to find each note's note before, and for a staff split from
//! a part looks in the whole part, kept among the note's sites, unless the
//! score was read from the corpus cache, which drops it. The first two parts,
//! and each part with itself, are aligned as music21's `StreamAligner` aligns
//! them, by their first 60 notes and, where they are small enough for
//! music21's loops, whole. The hashes, the changes and the similarity must be
//! music21's. music21 rounds a stream's own lengths and offsets as it hashes
//! them, so the unrounded hash is made from a reading of its own.
//! `ALPHA_PARITY_SCORES`, a `;`-separated list of corpus names, runs those
//! instead of the writer test's scores, which is how the corpus is swept.

use music21_rs::alpha::analysis::aligner::{ChangeOp, StreamAligner};
use music21_rs::alpha::analysis::hasher::{HashReference, HashValue, Hasher, NoteHash};
use music21_rs::musicxml::from_musicxml;
use music21_rs::stream::{Stream, StreamElement};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::SCORES;

const MUSIC21: &str = r#"
import zipfile
from fractions import Fraction
from music21 import converter, corpus, chord, note, stream
from music21.alpha.analysis import hasher, aligner

def source_text(name):
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

LIMIT = 60
INTERVAL_LIMIT = 600
SKIPPED = 'skipped: too long for music21 to hash its intervals'

VARIANTS = [
    {},
    {'hashMIDI': False},
    {'hashMIDI': False, 'hashNoteNameOctave': True},
    {'hashOctave': True},
    {'roundDurationAndOffset': False},
    {'hashChordsAsNotes': False, 'hashChordsAsChords': True, 'hashNormalOrderString': True,
     'hashPrimeFormString': True, 'hashOctave': True},
    {'hashIntervalFromLastNote': True, 'hashIsAccidental': True},
]

def value(v):
    if v is None:
        return ('n', '')
    if isinstance(v, bool):
        return ('i', str(int(v)))
    if isinstance(v, int):
        return ('i', str(v))
    if isinstance(v, (float, Fraction)):
        return ('f', repr(float(v)))
    return ('s', str(v))

def references(stream):
    # Each hashed element by its place among the stream's notes, rests and
    # chords, which both readers hold alike.
    places = {}
    i = 0
    for e in stream.recurse():
        if not isinstance(e, (note.Note, note.Rest, chord.Chord)):
            continue
        places[id(e)] = (i, -1)
        if isinstance(e, chord.Chord):
            for k, n in enumerate(e.notes):
                places[id(n)] = (i, k)
        i += 1
    return places

def hashes(stream, h):
    places = references(stream)
    out = []
    for nh in h.hashStream(stream):
        ref = getattr(nh, 'reference', None)
        out.append(([value(v) for v in nh], places.get(id(ref), (-2, -2)) if ref is not None else (-3, -3)))
    return out

def report(text):
    out = []
    # Rounding is all music21 changes as it hashes, and rounding again
    # changes nothing, so one reading serves every rounded hash and the
    # alignments; the unrounded hash has its own.
    rounded = converter.parse(text, format='musicxml', forceSource=True)
    for keywords in VARIANTS:
        if keywords.get('roundDurationAndOffset', True):
            score = rounded
        else:
            score = converter.parse(text, format='musicxml', forceSource=True)
        # music21 finds each note's note before by searching back through
        # the score, which grows with the square of its length: the interval
        # hash is compared on scores small enough for that.
        if (keywords.get('hashIntervalFromLastNote')
                and len(score.recurse().notesAndRests) > INTERVAL_LIMIT):
            out.append(([], SKIPPED))
            continue
        # A note of a staff music21 split from a part keeps the whole part
        # among its sites, and music21 finds the note before it there, in
        # the other staff -- unless the score came out of the corpus cache,
        # which drops that site: its answer depends on how it was read.
        if (keywords.get('hashIntervalFromLastNote')
                and score.getElementsByClass(stream.PartStaff)):
            out.append(([], SKIPPED))
            continue
        h = hasher.Hasher()
        h.includeReference = True
        for k, v in keywords.items():
            setattr(h, k, v)
        try:
            out.append((hashes(score, h), ''))
        except Exception as e:
            out.append(([], type(e).__name__))
    alignments = []
    parts = list(rounded.parts)
    pairs = []
    if len(parts) >= 2:
        pairs.append((0, 1))
    pairs.extend((i, i) for i in range(len(parts)))
    for a, b in pairs:
        target, source = parts[a], parts[b]
        tp, sp = references(target), references(source)
        target_hashes = aligner.StreamAligner().getDefaultHasher().hashStream(target)
        source_hashes = aligner.StreamAligner().getDefaultHasher().hashStream(source)
        # music21 aligns in Python loops, so whole parts are aligned only
        # where they are small, and every pair by its first 60 notes.
        lengths = [LIMIT]
        if len(target_hashes) * len(source_hashes) <= 40000:
            lengths.append(None)
        for length in lengths:
            try:
                sa = aligner.StreamAligner(target_hashes[:length], source_hashes[:length],
                                           preHashed=True)
                sa.align()
                changes = [(tp.get(id(t), (-2, -2)), sp.get(id(s), (-2, -2)), int(op))
                           for t, s, op in sa.changes]
                alignments.append((a, b, length or 0, changes, float(sa.similarityScore), ''))
            except Exception as e:
                alignments.append((a, b, length or 0, [], 0.0, type(e).__name__))
    return (out, alignments)
"#;

type Value = (String, String);
type Hashed = (Vec<Value>, (i64, i64));
/// A change: what of the target and of the source it is made from, and
/// which change it is.
type Change = ((i64, i64), (i64, i64), u8);
type Alignment = (usize, usize, usize, Vec<Change>, f64, String);
type Report = (Vec<(Vec<Hashed>, String)>, Vec<Alignment>);

fn hasher_for(variant: usize) -> Hasher {
    let mut hasher = Hasher {
        include_reference: true,
        ..Hasher::default()
    };
    match variant {
        1 => hasher.hash_midi = false,
        2 => {
            hasher.hash_midi = false;
            hasher.hash_note_name_octave = true;
        }
        3 => hasher.hash_octave = true,
        4 => hasher.round_duration_and_offset = false,
        5 => {
            hasher.hash_chords_as_notes = false;
            hasher.hash_chords_as_chords = true;
            hasher.hash_normal_order_string = true;
            hasher.hash_prime_form_string = true;
            hasher.hash_octave = true;
        }
        6 => {
            hasher.hash_interval_from_last_note = true;
            hasher.hash_is_accidental = true;
        }
        _ => {}
    }
    hasher
}

fn value(value: &HashValue) -> Value {
    match value {
        HashValue::Integer(number) => ("i".to_string(), number.to_string()),
        HashValue::Float(number) => ("f".to_string(), number.to_string()),
        HashValue::Text(text) => ("s".to_string(), text.clone()),
        HashValue::None => ("n".to_string(), String::new()),
    }
}

/// A reference as the place of its element among the stream's notes, rests
/// and chords, as the helper counts them.
/// Each element of a stream's walk by its place among the stream's notes,
/// rests and chords, as the helper counts them.
fn places(stream: &Stream) -> Vec<usize> {
    let mut count = 0;
    stream
        .recurse()
        .iter()
        .map(|(_, element)| {
            let place = count;
            if matches!(
                element,
                StreamElement::Note(_)
                    | StreamElement::Rest(_)
                    | StreamElement::Chord(_)
                    | StreamElement::ChordSymbol(_)
            ) {
                count += 1;
            }
            place
        })
        .collect()
}

fn reference(places: &[usize], reference: Option<HashReference>) -> (i64, i64) {
    match reference {
        Some(HashReference { element, component }) => (
            places[element] as i64,
            component.map_or(-1, |component| component as i64),
        ),
        None => (-3, -3),
    }
}

fn hashed(stream: &Stream, hashes: &[NoteHash]) -> Vec<Hashed> {
    let places = places(stream);
    hashes
        .iter()
        .map(|hash| {
            (
                hash.values.iter().map(value).collect(),
                reference(&places, hash.reference),
            )
        })
        .collect()
}

/// Whether two values agree: floats to a billionth, as the readers' lengths
/// differ in their last bits.
fn values_agree(ours: &Value, theirs: &Value) -> bool {
    if ours.0 != theirs.0 {
        return false;
    }
    if ours.0 == "f" {
        let (a, b): (f64, f64) = (
            ours.1.parse().unwrap_or(f64::NAN),
            theirs.1.parse().unwrap_or(f64::NAN),
        );
        return (a - b).abs() < 1e-9;
    }
    ours.1 == theirs.1
}

fn hashes_agree(ours: &[Hashed], theirs: &[Hashed]) -> bool {
    ours.len() == theirs.len()
        && ours.iter().zip(theirs).all(|(a, b)| {
            a.1 == b.1
                && a.0.len() == b.0.len()
                && a.0.iter().zip(&b.0).all(|(x, y)| values_agree(x, y))
        })
}

fn op_code(op: ChangeOp) -> u8 {
    match op {
        ChangeOp::Insertion => 0,
        ChangeOp::Deletion => 1,
        ChangeOp::Substitution => 2,
        ChangeOp::NoChange => 3,
    }
}

#[test]
fn the_crate_hashes_and_aligns_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"alpha_parity_music21.py",
            c"alpha_parity_music21",
        )?;
        let mut failures = Vec::new();
        let chosen: Vec<String> = match std::env::var("ALPHA_PARITY_SCORES") {
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
        let mut compared = 0;
        for name in &chosen {
            let Some(text): Option<String> =
                music21.getattr("source_text")?.call1((name,))?.extract()?
            else {
                continue;
            };
            let score = match from_musicxml(&text) {
                Ok(score) => score,
                Err(error) => {
                    failures.push(format!("{name}: the crate could not read it: {error}"));
                    continue;
                }
            };
            let theirs: Report = match music21.getattr("report")?.call1((&text,)) {
                Ok(report) => report.extract()?,
                Err(error) => {
                    failures.push(format!("{name}: music21 could not hash it: {error}"));
                    continue;
                }
            };
            compared += 1;
            for (variant, (their_hashes, their_error)) in theirs.0.iter().enumerate() {
                if their_error.starts_with("skipped") {
                    continue;
                }
                let ours = hasher_for(variant).hash_stream(&score);
                match ours {
                    Ok(ours) => {
                        let ours = hashed(&score, &ours);
                        if !their_error.is_empty() {
                            failures.push(format!("{name}: hasher {variant}: music21 refused ({their_error})"));
                        } else if !hashes_agree(&ours, their_hashes) {
                            let at = ours
                                .iter()
                                .zip(their_hashes)
                                .position(|(a, b)| !hashes_agree(std::slice::from_ref(a), std::slice::from_ref(b)))
                                .unwrap_or(ours.len().min(their_hashes.len()));
                            failures.push(format!(
                                "{name}: hasher {variant}: {} hashes and {} in music21; first difference at {at}:\n  music21    {:?}\n  music21-rs {:?}",
                                ours.len(),
                                their_hashes.len(),
                                their_hashes.get(at),
                                ours.get(at)
                            ));
                        }
                    }
                    Err(error) => {
                        if their_error.is_empty() {
                            failures.push(format!("{name}: hasher {variant}: the crate refused ({error})"));
                        }
                    }
                }
            }
            let parts: Vec<&Stream> = score.parts();
            for (a, b, length, their_changes, their_score, their_error) in &theirs.1 {
                let (Some(target), Some(source)) = (parts.get(*a), parts.get(*b)) else {
                    failures.push(format!("{name}: no parts {a} and {b}"));
                    continue;
                };
                let (target_places, source_places) = (places(target), places(source));
                let mut aligner = StreamAligner::new();
                let hashed = aligner
                    .hasher
                    .hash_stream(target)
                    .and_then(|target| Ok((target, aligner.hasher.hash_stream(source)?)));
                let aligned = hashed.and_then(|(mut target, mut source)| {
                    if *length > 0 {
                        target.truncate(*length);
                        source.truncate(*length);
                    }
                    aligner.align_hashed(target, source)
                });
                match aligned {
                    Ok(()) => {
                        if !their_error.is_empty() {
                            failures.push(format!("{name}: align {a} {b}: music21 refused ({their_error})"));
                            continue;
                        }
                        let ours: Vec<Change> = aligner
                            .changes
                            .iter()
                            .map(|change| (
                                    reference(&target_places, change.target),
                                    reference(&source_places, change.source),
                                    op_code(change.op),
                                ))
                            .collect();
                        if &ours != their_changes || (aligner.similarity_score - their_score).abs() > 1e-12 {
                            let at = ours
                                .iter()
                                .zip(their_changes)
                                .position(|(x, y)| x != y)
                                .unwrap_or(ours.len().min(their_changes.len()));
                            failures.push(format!(
                                "{name}: align {a} {b}: {} changes and {} in music21, similarity {} and {}; first difference at {at}: music21 {:?}, music21-rs {:?}",
                                ours.len(),
                                their_changes.len(),
                                aligner.similarity_score,
                                their_score,
                                their_changes.get(at),
                                ours.get(at)
                            ));
                        }
                    }
                    Err(error) => {
                        if their_error.is_empty() {
                            failures.push(format!("{name}: align {a} {b}: the crate refused ({error})"));
                        }
                    }
                }
            }
        }
        Ok((failures, compared))
    })
    .expect("the Python side runs");

    println!("{compared} scores compared");
    assert!(
        failures.is_empty(),
        "{} differences:\n\n{}",
        failures.len(),
        failures.join("\n")
    );
}
