//! The crate's serial search against music21's: `search.serial`.
//!
//! The text of each corpus score is read by music21 and by `from_musicxml`
//! (which `musicxml_read_parity` holds to music21's reader). Its segments of
//! three and four pitch classes are found in each way of reading
//! repetitions, chords in and out; the score's own first segments and two
//! common ones are searched for by each kind of matcher; and the score is
//! labelled by exact, transposed and transformed segments. Every segment,
//! match, line and lyric must be music21's. music21's matchers take minutes
//! on a large score, so this runs a handful of small ones -- chorales, a
//! piano piece of Schoenberg's and Webern's, voices, chords and early
//! music. `SEARCH_PARITY_SCORES`, a `;`-separated list of corpus names, runs
//! those instead, which is how the corpus is swept.

use music21_rs::musicxml::from_musicxml;
use music21_rs::search::{
    ContiguousSegment, ContiguousSegmentSearcher, Matching, Repetitions, SegmentMatcher,
};
use music21_rs::serial::{ToneRow, TransformationConvention};
use music21_rs::spanner::{Spanner, SpannerKind};
use music21_rs::stream::{Stream, StreamElement};
use music21_rs::{FloatType, IntegerType};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

const SCORES: [&str; 10] = [
    "bach/bwv66.6.mxl",
    "bach/bwv324.mxl",
    "schoenberg/opus19/movement2.mxl",
    "webern/webern_dormi_jesu_op_16_no_2.mxl",
    "demos/two-voices.xml",
    "demos/voices_with_chords.xml",
    "demos/chorale_with_parallels.mxl",
    "ciconia/quod_jactatur.xml",
    "trecento/PMFC_06_8-In Verde Prato.xml",
    "luca/gloria.xml",
];

const MUSIC21: &str = r#"
import zipfile
from music21 import converter, corpus, spanner, serial as m21serial
from music21.search import serial

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

REPS = ['skipConsecutive', 'rowsOnly', 'includeAll', 'ignoreAll']
MATCHERS = [serial.SegmentMatcher, serial.TransposedSegmentMatcher,
            serial.TransformedSegmentMatcher, serial.MultisetSegmentMatcher,
            serial.TransposedMultisetMatcher, serial.TransposedInvertedMultisetMatcher]

def notes(segment):
    return [(n.measureNumber, float(n.offset), [float(p.ps) for p in n.pitches])
            for n in segment.segment]

def active(value):
    if isinstance(value, m21serial.ToneRow):
        return ('', list(value.pitchClasses()))
    if isinstance(value, str):
        return (value, [])
    return ('', list(value))

def attempt(function):
    try:
        return (function(), '')
    except Exception as error:
        return (None, type(error).__name__)

def report(text):
    score = converter.parse(text, format='musicxml', forceSource=True)
    segments = []
    for chords in (True, False):
        for reps in REPS:
            for length in (3, 4):
                found, error = attempt(lambda: serial.ContiguousSegmentSearcher(
                    score, reps, chords).byLength(length))
                segments.append(([(s.partNumber, notes(s)) for s in found] if found is not None
                                 else [], error))
    own = serial.ContiguousSegmentSearcher(score, 'skipConsecutive', True)
    search = [[0, 4, 7], [0, 2, 4, 5]]
    for length in (3, 4):
        found = own.byLength(length)
        if found:
            search.append(found[0].readPitchClassesFromBottom()[:length])
    matches = []
    for matcher in MATCHERS:
        for reps in ('skipConsecutive', 'ignoreAll'):
            found, error = attempt(lambda: matcher(score, reps, True).find(search))
            matches.append(([(s.partNumber, notes(s), active(s.activeSegment), list(s.matchedSegment))
                             for s in found] if found is not None else [], error))
    named = {chr(65 + i): segment for i, segment in enumerate(search)}
    labels = []
    for label in (lambda: serial.labelSegments(score, named),
                  lambda: serial.labelTransposedSegments(score, named),
                  lambda: serial.labelTransformedSegments(score, named, convention='original'),
                  lambda: serial.labelTransformedSegments(score, named, convention='zero')):
        labelled, error = attempt(label)
        if labelled is None:
            labels.append((0, [], error))
            continue
        lines = len(list(labelled.recurse().getElementsByClass(spanner.Line)))
        lyrics = [[l.text for l in n.lyrics] for n in labelled.recurse().notes
                  if 'Chord' in n.classes or 'Note' in n.classes]
        labels.append((lines, lyrics, error))
    return search, segments, matches, labels
"#;

type Notes = Vec<(Option<IntegerType>, FloatType, Vec<FloatType>)>;
type Found = Vec<(Option<usize>, Notes)>;
type Active = (String, Vec<u8>);
type Matched = Vec<(Option<usize>, Notes, Active, Vec<IntegerType>)>;
type Labelled = (usize, Vec<Vec<String>>, String);
type Report = (
    Vec<Vec<IntegerType>>,
    Vec<(Found, String)>,
    Vec<(Matched, String)>,
    Vec<Labelled>,
);

const REPS: [Repetitions; 4] = [
    Repetitions::SkipConsecutive,
    Repetitions::RowsOnly,
    Repetitions::IncludeAll,
    Repetitions::IgnoreAll,
];

const MATCHERS: [Matching; 6] = [
    Matching::Exact,
    Matching::Transposed,
    Matching::Transformed,
    Matching::Multiset,
    Matching::TransposedMultiset,
    Matching::TransposedInvertedMultiset,
];

fn notes(segment: &ContiguousSegment<'_>) -> Notes {
    segment
        .notes
        .iter()
        .map(|note| {
            let pitches = match note.element {
                StreamElement::ChordSymbol(symbol) => symbol.pitches().unwrap_or_default(),
                element => element.pitches(),
            };
            (
                note.measure,
                note.offset,
                pitches.iter().map(|pitch| pitch.ps()).collect(),
            )
        })
        .collect()
}

fn found(result: music21_rs::Result<Vec<ContiguousSegment<'_>>>) -> (Found, String) {
    match result {
        Ok(found) => (
            found
                .iter()
                .map(|segment| (segment.part, notes(segment)))
                .collect(),
            String::new(),
        ),
        Err(error) => (Vec::new(), error.to_string()),
    }
}

fn matched(
    matching: Matching,
    result: music21_rs::Result<Vec<ContiguousSegment<'_>>>,
) -> (Matched, String) {
    match result {
        Ok(found) => (
            found
                .iter()
                .map(|segment| {
                    let active = if matching == Matching::Transposed {
                        let row = ToneRow::new(
                            segment.active.iter().map(|class| IntegerType::from(*class)),
                        );
                        (row.intervals_as_string(), Vec::new())
                    } else {
                        (String::new(), segment.active.clone())
                    };
                    (
                        segment.part,
                        notes(segment),
                        active,
                        segment.matched.clone().unwrap_or_default(),
                    )
                })
                .collect(),
            String::new(),
        ),
        Err(error) => (Vec::new(), error.to_string()),
    }
}

fn labelled(result: music21_rs::Result<Stream>) -> Labelled {
    match result {
        Ok(stream) => {
            let is_line = |spanner: &&Spanner| spanner.kind() == SpannerKind::Line;
            let mut lines = stream.spanners().iter().filter(is_line).count();
            for (_, element) in stream.recurse() {
                if let StreamElement::Stream(inner) = element {
                    lines += inner.spanners().iter().filter(is_line).count();
                }
            }
            let lyrics = stream
                .leaves()
                .into_iter()
                .filter_map(|(_, element)| match element {
                    StreamElement::Note(note) => {
                        Some(note.lyrics().iter().map(|lyric| lyric.text()).collect())
                    }
                    StreamElement::Chord(chord) => {
                        Some(chord.lyrics().iter().map(|lyric| lyric.text()).collect())
                    }
                    // A chord symbol is a chord to music21.
                    StreamElement::ChordSymbol(symbol) => {
                        Some(symbol.lyrics().iter().map(|lyric| lyric.text()).collect())
                    }
                    _ => None,
                })
                .collect();
            (lines, lyrics, String::new())
        }
        Err(error) => (0, Vec::new(), error.to_string()),
    }
}

/// Whether two answers agree: both refused, or both the same.
fn agree<T: PartialEq>(ours: &(T, String), theirs: &(T, String)) -> bool {
    if theirs.1.is_empty() {
        ours.1.is_empty() && ours.0 == theirs.0
    } else {
        !ours.1.is_empty()
    }
}

#[test]
fn the_crate_searches_rows_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"search_serial_parity_music21.py",
            c"search_serial_parity_music21",
        )?;
        let mut failures = Vec::new();
        let chosen: Vec<String> = match std::env::var("SEARCH_PARITY_SCORES") {
            Ok(list) => list
                .split(';')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect(),
            Err(_) => SCORES.iter().map(|name| name.to_string()).collect(),
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
            let report = music21.getattr("report")?.call1((&text,));
            let (search, their_segments, their_matches, their_labels): Report = match report {
                Ok(report) => report.extract()?,
                Err(error) => {
                    failures.push(format!("{name}: music21 could not read it: {error}"));
                    continue;
                }
            };
            compared += 1;

            let mut theirs = their_segments.iter();
            for chords in [true, false] {
                for repetitions in REPS {
                    for length in [3, 4] {
                        let searcher = ContiguousSegmentSearcher::new(repetitions, chords);
                        let ours = found(searcher.by_length(&score, length));
                        let theirs = theirs.next().expect("one answer per search");
                        if !agree(&ours, theirs) {
                            let at = ours
                                .0
                                .iter()
                                .zip(&theirs.0)
                                .position(|(ours, theirs)| ours != theirs);
                            failures.push(format!(
                                "{name}: {repetitions:?}, chords {chords}, length {length}: \
                                 music21 {} {} segments, music21-rs {} {}; first difference \
                                 at {at:?}:\n  music21    {:?}\n  music21-rs {:?}",
                                theirs.1,
                                theirs.0.len(),
                                ours.1,
                                ours.0.len(),
                                at.and_then(|at| theirs.0.get(at)),
                                at.and_then(|at| ours.0.get(at))
                            ));
                        }
                    }
                }
            }

            let mut theirs = their_matches.iter();
            for matching in MATCHERS {
                for repetitions in [Repetitions::SkipConsecutive, Repetitions::IgnoreAll] {
                    let matcher = SegmentMatcher::new(matching, repetitions, true);
                    let ours = matched(matching, matcher.find(&score, &search));
                    let theirs = theirs.next().expect("one answer per matcher");
                    if !agree(&ours, theirs) {
                        let at = ours
                            .0
                            .iter()
                            .zip(&theirs.0)
                            .position(|(ours, theirs)| ours != theirs);
                        failures.push(format!(
                            "{name}: {matching:?}, {repetitions:?}: music21 {} {} matches, \
                             music21-rs {} {}; first difference at {at:?}:\n  \
                             music21    {:?}\n  music21-rs {:?}",
                            theirs.1,
                            theirs.0.len(),
                            ours.1,
                            ours.0.len(),
                            at.and_then(|at| theirs.0.get(at)),
                            at.and_then(|at| ours.0.get(at))
                        ));
                    }
                }
            }

            let named: Vec<(String, Vec<IntegerType>)> = search
                .iter()
                .enumerate()
                .map(|(index, segment)| {
                    (char::from(b'A' + index as u8).to_string(), segment.clone())
                })
                .collect();
            let exact = SegmentMatcher::new(Matching::Exact, Repetitions::SkipConsecutive, true);
            let transposed =
                SegmentMatcher::new(Matching::Transposed, Repetitions::SkipConsecutive, true);
            let transformed =
                SegmentMatcher::new(Matching::Transformed, Repetitions::SkipConsecutive, true);
            let ours = [
                labelled(exact.label(&score, &named, None)),
                labelled(transposed.label(&score, &named, None)),
                labelled(transformed.label(
                    &score,
                    &named,
                    Some(TransformationConvention::OriginalCentered),
                )),
                labelled(transformed.label(
                    &score,
                    &named,
                    Some(TransformationConvention::ZeroCentered),
                )),
            ];
            for (label, (ours, theirs)) in ["exact", "transposed", "original", "zero"]
                .iter()
                .zip(ours.iter().zip(&their_labels))
            {
                let agree = if theirs.2.is_empty() {
                    ours.2.is_empty() && ours.0 == theirs.0 && ours.1 == theirs.1
                } else {
                    !ours.2.is_empty()
                };
                if !agree {
                    let at = ours
                        .1
                        .iter()
                        .zip(&theirs.1)
                        .position(|(ours, theirs)| ours != theirs);
                    failures.push(format!(
                        "{name}: label {label}: music21 {} {} lines, music21-rs {} {} lines; \
                         first lyric difference at {at:?}: music21 {:?}, music21-rs {:?}",
                        theirs.2,
                        theirs.0,
                        ours.2,
                        ours.0,
                        at.and_then(|at| theirs.1.get(at)),
                        at.and_then(|at| ours.1.get(at))
                    ));
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
