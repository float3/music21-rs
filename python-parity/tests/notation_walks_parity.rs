//! Four small ports against music21, each asked the same thing on both
//! sides: `getGrace` (`grace_note`), `Beams.naiveBeams`
//! (`Beams::naive_beams`), `Volume.getDynamicContext`
//! (`volume::dynamic_context`) and `Trill.splitClient`
//! (`Ornament::split_client`, and the split `make_ties` makes with it).
//!
//! music21 is music21 here, with nothing of the crate installed over it.

use music21_rs::expressions::{Expression, Ornament, OrnamentKind};
use music21_rs::makenotation::{make_measures, make_ties};
use music21_rs::musicxml::from_musicxml;
use music21_rs::volume::dynamic_context;
use music21_rs::{
    Beams, Chord, Duration, Note, Rest, Spanner, SpannerKind, Stream, StreamElement, StreamKind,
};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use music21_rs_python_parity::music21_name;
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod built_streams;
use built_streams::{
    BUILD, Spec, build, build_stream, literal, marked, measure, note, outline, stream,
};

/// What music21 answers, as text the crate's answers are written as too.
const ASKED: &str = r#"
import os
import zipfile
from music21 import beam, chord, corpus, expressions, note, stream

def number(value):
    return f'{float(value):.5f}'

def grace(kind, length, appoggiatura):
    if kind == 'note':
        made = note.Note('G4', quarterLength=length)
    elif kind == 'chord':
        made = chord.Chord('C4 E4 G4', quarterLength=length)
    else:
        made = note.Rest(quarterLength=length)
    d = made.getGrace(appoggiatura=appoggiatura).duration
    written = ' '.join(f'{c.type}:{c.dots}' for c in d.components)
    return (f'{number(d.quarterLength)} grace={d.isGrace} linked={d.linked} slash={d.slash} '
            f'makeTime={d.makeTime} tuplets={len(d.tuplets)} [{written}]')

def naive(elements):
    return [None if made is None else len(made.beamsList)
            for made in beam.Beams.naiveBeams(elements)]

def said(e):
    if type(e).__name__ == 'Note':
        return e.nameWithOctave
    return ' '.join(p.nameWithOctave for p in e.pitches)

def contexts(held):
    out = []
    for e in held.recurse():
        if type(e).__name__ not in ('Note', 'Chord'):
            continue
        found = e.volume.getDynamicContext()
        out.append(f'{number(e.getOffsetInHierarchy(held))} {said(e)} -> '
                   f'{None if found is None else found.value}')
    return sorted(out)

def split_client(count, extended):
    pieces = [note.Note('C4') for _ in range(count)]
    held = None
    if extended:
        held = expressions.TrillExtension([pieces[0], note.Note('D4')])
    made = expressions.Trill().splitClient(pieces)
    marks = ['+'.join(type(mark).__name__ for mark in piece.expressions) for piece in pieces]
    spanned = [f'{type(each).__name__}:{len(each.getSpannedElements())}' for each in made]
    return f'marks={marks} spanners={spanned}'

def tied(held):
    made = held.makeMeasures()
    made.makeTies(inPlace=True)
    return made

def source_text(name):
    path = corpus.getWork(name)
    if isinstance(path, (list, tuple)):
        path = path[0]
    path = str(path)
    if path.lower().endswith('.mxl'):
        with zipfile.ZipFile(path) as archive:
            names = [n for n in archive.namelist()
                     if not n.startswith('META-INF') and n.lower().endswith(('.xml', '.musicxml'))]
            data = archive.read(names[0])
    else:
        with open(path, 'rb') as handle:
            data = handle.read()
    for encoding in ('utf-8-sig', 'utf-16'):
        try:
            return data.decode(encoding)
        except UnicodeDecodeError:
            pass
    return data.decode('latin-1')
"#;

fn number(value: f64) -> String {
    format!("{value:.5}")
}

fn python_bool(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}

/// What a grace note's duration says, as `ASKED`'s `grace` writes it.
fn grace_said(duration: &Duration) -> String {
    let grace = duration.grace();
    let written: Vec<String> = duration
        .components()
        .iter()
        .map(|(value, dots)| format!("{}:{dots}", value.music21_name()))
        .collect();
    format!(
        "{} grace={} linked={} slash={} makeTime={} tuplets={} [{}]",
        number(duration.quarter_length()),
        python_bool(duration.is_grace()),
        python_bool(duration.linked()),
        python_bool(grace.is_some_and(|grace| grace.slash())),
        python_bool(grace.is_some_and(|grace| grace.make_time())),
        duration.tuplets().len(),
        written.join(" ")
    )
}

fn our_grace(kind: &str, length: f64, appoggiatura: bool) -> String {
    let duration = Duration::new(length).expect("a length");
    match kind {
        "note" => grace_said(
            Note::from_name("G4")
                .unwrap()
                .with_duration(duration)
                .grace_note(appoggiatura)
                .duration()
                .unwrap(),
        ),
        "chord" => grace_said(
            Chord::new("C4 E4 G4")
                .unwrap()
                .with_duration(duration)
                .grace_note(appoggiatura)
                .duration()
                .unwrap(),
        ),
        _ => grace_said(Rest::new(duration).grace_note(appoggiatura).duration()),
    }
}

/// Every note and chord of a stream with the dynamic it is read against,
/// as `ASKED`'s `contexts` writes them.
fn our_contexts(held: &Stream) -> Vec<String> {
    let leaves = held.leaves();
    let mut out: Vec<String> = leaves
        .iter()
        .enumerate()
        .filter_map(|(position, (offset, element))| {
            let said = match element {
                StreamElement::Note(note) => music21_name(&note.pitch().name_with_octave()),
                StreamElement::Chord(chord) => chord
                    .pitches()
                    .iter()
                    .map(|pitch| music21_name(&pitch.name_with_octave()))
                    .collect::<Vec<_>>()
                    .join(" "),
                _ => return None,
            };
            let found = dynamic_context(held, position).map(|place| match leaves[place].1 {
                StreamElement::Dynamic(dynamic) => dynamic.value().to_string(),
                other => panic!("not a dynamic: {other:?}"),
            });
            Some(format!(
                "{} {said} -> {}",
                number(*offset),
                found.unwrap_or_else(|| "None".to_string())
            ))
        })
        .collect();
    out.sort();
    out
}

/// Streams with dynamics here and there, and what each asks of the search.
fn dynamic_cases() -> Vec<(&'static str, Spec)> {
    let part = StreamKind::Part;
    let score = StreamKind::Score;
    let voice = StreamKind::Voice;
    let plain = StreamKind::Stream;
    let dynamic = Spec::Dynamic;
    vec![
        (
            "a dynamic in an earlier measure, and never one in another part",
            stream(
                score,
                vec![
                    (
                        0.0,
                        stream(
                            part,
                            vec![
                                (
                                    0.0,
                                    measure(1, vec![(0.0, dynamic("ff")), (0.0, note("C4", 4.0))]),
                                ),
                                (4.0, measure(2, vec![(0.0, note("D4", 4.0))])),
                            ],
                        ),
                    ),
                    (
                        0.0,
                        stream(
                            part,
                            vec![
                                (
                                    0.0,
                                    measure(1, vec![(2.0, dynamic("pp")), (0.0, note("E4", 4.0))]),
                                ),
                                (4.0, measure(2, vec![(0.0, note("F4", 4.0))])),
                            ],
                        ),
                    ),
                ],
            ),
        ),
        (
            "a dynamic in the other voice of the measure",
            stream(
                part,
                vec![(
                    0.0,
                    measure(
                        1,
                        vec![
                            (
                                0.0,
                                stream(voice, vec![(0.0, note("B4", 1.0)), (1.0, note("A4", 1.0))]),
                            ),
                            (
                                0.0,
                                stream(voice, vec![(0.0, dynamic("mf")), (0.0, note("G3", 4.0))]),
                            ),
                        ],
                    ),
                )],
            ),
        ),
        (
            "a dynamic put in after the note it stands with",
            stream(plain, vec![(0.0, note("C4", 1.0)), (0.0, dynamic("p"))]),
        ),
        (
            "the dynamic where the note stands wins over an earlier one",
            stream(
                plain,
                vec![
                    (1.0, note("C4", 1.0)),
                    (1.0, dynamic("p")),
                    (0.0, dynamic("f")),
                ],
            ),
        ),
        (
            "a dynamic in the measure, the note in a voice of it",
            stream(
                part,
                vec![(
                    0.0,
                    measure(
                        1,
                        vec![
                            (0.0, dynamic("sf")),
                            (0.0, stream(voice, vec![(1.0, note("C4", 1.0))])),
                        ],
                    ),
                )],
            ),
        ),
        (
            "a dynamic after the note is not its context",
            stream(plain, vec![(0.0, note("C4", 1.0)), (2.0, dynamic("p"))]),
        ),
        (
            "a dynamic standing in the part outside its measures",
            stream(
                part,
                vec![
                    (
                        0.0,
                        measure(1, vec![(0.0, note("C4", 1.0)), (1.0, note("D4", 1.0))]),
                    ),
                    (0.5, dynamic("ppp")),
                ],
            ),
        ),
        (
            "of two dynamics standing together the later is the one",
            stream(
                plain,
                vec![
                    (1.0, note("C4", 1.0)),
                    (0.0, dynamic("p")),
                    (0.0, dynamic("f")),
                ],
            ),
        ),
        (
            "a chord has a context as a note has",
            stream(
                plain,
                vec![
                    (0.0, dynamic("fff")),
                    (
                        1.0,
                        Spec::Chord {
                            names: "C4 E4 G4",
                            length: 1.0,
                        },
                    ),
                ],
            ),
        ),
        (
            "the measure's own dynamic wins over a later one loose in the part",
            stream(
                part,
                vec![
                    (0.0, measure(1, vec![(0.0, note("C4", 4.0))])),
                    (
                        4.0,
                        measure(
                            2,
                            vec![
                                (0.0, dynamic("mf")),
                                (0.0, note("D4", 2.0)),
                                (2.0, note("E4", 1.0)),
                            ],
                        ),
                    ),
                    (5.0, dynamic("p")),
                ],
            ),
        ),
        (
            "a dynamic standing in the score itself",
            stream(
                score,
                vec![
                    (
                        0.0,
                        stream(part, vec![(0.0, measure(1, vec![(2.0, note("C4", 1.0))]))]),
                    ),
                    (0.0, dynamic("ff")),
                ],
            ),
        ),
        (
            "the note's own voice is searched before the other",
            stream(
                part,
                vec![(
                    0.0,
                    measure(
                        1,
                        vec![
                            (
                                0.0,
                                stream(voice, vec![(1.5, dynamic("pp")), (0.0, note("G3", 1.0))]),
                            ),
                            (
                                0.0,
                                stream(voice, vec![(1.0, dynamic("ff")), (2.0, note("C4", 1.0))]),
                            ),
                        ],
                    ),
                )],
            ),
        ),
        (
            "a plain stream of parts is searched through, where a score is not",
            stream(
                plain,
                vec![
                    (
                        0.0,
                        stream(part, vec![(0.0, dynamic("ff")), (0.0, note("C4", 2.0))]),
                    ),
                    (0.0, stream(part, vec![(1.0, note("E4", 1.0))])),
                ],
            ),
        ),
    ]
}

/// Corpus scores with dynamics in them.
const SCORES: &[&str] = &[
    "beethoven/opus18no1/movement1.mxl",
    "schoenberg/opus19/movement2",
    "beach/prayer_of_a_tired_child",
];

fn mark_names(note: &Note) -> String {
    note.expressions()
        .iter()
        .map(|expression| match expression {
            Expression::Ornament(ornament) => ornament.kind().class_name(),
            Expression::Fermata(_) => "Fermata",
            Expression::Arpeggio(_) => "ArpeggioMark",
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// What `split_client` does to so many pieces, as `ASKED`'s writes it.
fn our_split_client(count: usize, extended: bool) -> String {
    let mut held = Stream::new();
    for index in 0..count {
        held.insert(index as f64, Note::from_name("C4").unwrap());
    }
    held.insert(count as f64, Note::from_name("D4").unwrap());
    if extended {
        held.add_spanner(Spanner::new(SpannerKind::TrillExtension, vec![0, count]));
    }
    let places: Vec<usize> = (0..count).collect();
    let made = Ornament::of_kind(OrnamentKind::Trill)
        .split_client(&mut held, &places)
        .expect("a trill is split with its note");
    let marks: Vec<String> = held
        .events()
        .iter()
        .take(count)
        .map(|event| match event.element() {
            StreamElement::Note(note) => format!("'{}'", mark_names(note)),
            other => panic!("not a note: {other:?}"),
        })
        .collect();
    let spanned: Vec<String> = made
        .iter()
        .map(|spanner| format!("'{}:{}'", spanner.kind().class_name(), spanner.len()))
        .collect();
    format!(
        "marks=[{}] spanners=[{}]",
        marks.join(", "),
        spanned.join(", ")
    )
}

/// Parts whose notes run past their barlines carrying expressions.
fn tie_cases() -> Vec<(&'static str, Spec)> {
    vec![
        (
            "a trill stays on the first piece, a fermata goes to the last",
            stream(
                StreamKind::Part,
                vec![
                    (0.0, Spec::Meter("3/4")),
                    (0.0, marked("C4", 4.0, &["Trill"])),
                    (4.0, marked("D4", 4.0, &["Fermata"])),
                    (8.0, marked("E4", 4.0, &["Mordent", "Trill", "Turn"])),
                    (
                        12.0,
                        marked("F4", 4.0, &["Shake", "Fermata", "InvertedMordent"]),
                    ),
                    (16.0, marked("G4", 2.0, &["Trill", "Fermata"])),
                ],
            ),
        ),
        (
            "a note cut twice",
            stream(
                StreamKind::Part,
                vec![
                    (0.0, Spec::Meter("2/4")),
                    (0.0, note("C4", 1.0)),
                    (
                        1.0,
                        marked("D4", 4.0, &["WholeStepTrill", "Fermata", "Tremolo"]),
                    ),
                    (5.0, marked("E4", 1.0, &["Turn"])),
                ],
            ),
        ),
    ]
}

#[test]
fn the_small_ports_answer_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let failures = Python::attach(|py| -> PyResult<Vec<String>> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let _ = py.import("music21")?;
        let module = |code: &str, name: &str| -> PyResult<Bound<'_, PyModule>> {
            PyModule::from_code(
                py,
                &std::ffi::CString::new(code).expect("no nul in the helper"),
                &std::ffi::CString::new(format!("{name}.py")).expect("no nul in the name"),
                &std::ffi::CString::new(name).expect("no nul in the name"),
            )
        };
        let built = module(BUILD, "built_streams")?;
        let asked = module(ASKED, "notation_walks")?;
        let read = py.import("ast")?.getattr("literal_eval")?;
        let theirs_of = |spec: &Spec| -> PyResult<Bound<'_, PyAny>> {
            built
                .getattr("build")?
                .call1((read.call1((literal(spec),))?,))
        };
        let mut failures = Vec::new();

        // getGrace.
        for kind in ["note", "chord", "rest"] {
            for length in [1.0, 2.0, 1.5, 0.5, 1.25, 1.0 / 3.0, 4.0, 0.125, 0.75] {
                for appoggiatura in [false, true] {
                    let theirs: String = asked
                        .getattr("grace")?
                        .call1((kind, length, appoggiatura))?
                        .extract()?;
                    let ours = our_grace(kind, length, appoggiatura);
                    if ours != theirs {
                        failures.push(format!(
                            "getGrace of a {kind} of {length}, appoggiatura {appoggiatura}:\n  music21    {theirs}\n  music21-rs {ours}"
                        ));
                    }
                }
            }
        }

        // naiveBeams.
        let run: Vec<Spec> = vec![
            note("C4", 1.0),
            note("C4", 0.5),
            note("C4", 0.25),
            note("C4", 0.125),
            note("C4", 0.75),
            note("C4", 1.0 / 3.0),
            note("C4", 1.0 / 6.0),
            note("C4", 1.25),
            note("C4", 0.0625),
            note("C4", 2.0),
            Spec::Rest(0.125),
            Spec::Rest(0.5),
            Spec::Chord {
                names: "C4 E4",
                length: 0.5,
            },
            Spec::Note {
                name: "C4",
                length: 0.5,
                stored: None,
                marks: &[],
                grace: true,
            },
            Spec::Clef("treble"),
            Spec::Dynamic("f"),
        ];
        let elements = pyo3::types::PyList::empty(py);
        for spec in &run {
            elements.append(theirs_of(spec)?)?;
        }
        let theirs: Vec<Option<usize>> = asked.getattr("naive")?.call1((elements,))?.extract()?;
        let ours: Vec<Option<usize>> = Beams::naive_beams(&run.iter().map(build).collect::<Vec<_>>())
            .iter()
            .map(|beams| beams.as_ref().map(Beams::len))
            .collect();
        if ours != theirs {
            failures.push(format!(
                "naiveBeams:\n  music21    {theirs:?}\n  music21-rs {ours:?}"
            ));
        }

        // getDynamicContext, on built streams and on the corpus.
        for (label, spec) in dynamic_cases() {
            let theirs: Vec<String> = asked
                .getattr("contexts")?
                .call1((theirs_of(&spec)?,))?
                .extract()?;
            let ours = our_contexts(&build_stream(&spec));
            if ours != theirs {
                failures.push(format!(
                    "getDynamicContext, {label}:\n  music21    {theirs:?}\n  music21-rs {ours:?}"
                ));
            }
        }
        let corpus = py.import("music21.corpus")?;
        for name in SCORES {
            let text: String = asked.getattr("source_text")?.call1((name,))?.extract()?;
            let score = corpus.call_method1("parse", (name,))?;
            let ours = match from_musicxml(&text) {
                Ok(score) => score,
                Err(error) => {
                    failures.push(format!("{name}: the crate could not read it: {error}"));
                    continue;
                }
            };
            let parts = score.getattr("parts")?;
            let ours_parts = ours.parts();
            if parts.len()? != ours_parts.len() {
                failures.push(format!("{name}: the two readings have different parts"));
                continue;
            }
            let mut asked_of = 0;
            for (index, part) in ours_parts.iter().enumerate() {
                let theirs: Vec<String> = asked
                    .getattr("contexts")?
                    .call1((parts.get_item(index)?,))?
                    .extract()?;
                let ours = our_contexts(part);
                asked_of += theirs.len();
                if let Some(place) = (0..theirs.len().max(ours.len()))
                    .find(|&place| theirs.get(place) != ours.get(place))
                {
                    failures.push(format!(
                        "getDynamicContext, {name}, part {index}: of {} notes, the first to differ:\n  music21    {:?}\n  music21-rs {:?}",
                        theirs.len(),
                        theirs.get(place),
                        ours.get(place)
                    ));
                }
            }
            println!("{name}: {asked_of} notes asked for their dynamic");
        }

        // splitClient, alone and as makeTies meets it.
        for (count, extended) in [(0, false), (1, false), (2, false), (3, false), (2, true)] {
            let theirs: String = asked
                .getattr("split_client")?
                .call1((count, extended))?
                .extract()?;
            let ours = our_split_client(count, extended);
            if ours != theirs {
                failures.push(format!(
                    "splitClient of {count} pieces, extended {extended}:\n  music21    {theirs}\n  music21-rs {ours}"
                ));
            }
        }
        for (label, spec) in tie_cases() {
            let tied = asked.getattr("tied")?.call1((theirs_of(&spec)?,))?;
            let theirs: String = built.getattr("outline")?.call1((tied,))?.extract()?;
            let ours = make_measures(&build_stream(&spec)).and_then(|mut made| {
                make_ties(&mut made)?;
                Ok(outline(&made))
            });
            match ours {
                Ok(ours) if ours == theirs => {}
                Ok(ours) => failures.push(format!(
                    "makeTies, {label}:\n--- music21\n{theirs}\n--- music21-rs\n{ours}"
                )),
                Err(error) => failures.push(format!("makeTies, {label}: {error}")),
            }
        }
        Ok(failures)
    })
    .unwrap_or_else(|error| panic!("running music21: {error}"));

    assert!(
        failures.is_empty(),
        "{} answers differ:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}
