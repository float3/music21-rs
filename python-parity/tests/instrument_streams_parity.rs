//! The crate's `unbundle_instruments`, `bundle_instruments`, `deduplicate`
//! and `partition_by_instrument` against music21's `unbundleInstruments`,
//! `bundleInstruments`, `deduplicate` and `partitionByInstrument`.
//!
//! Each stream is built twice from one description, as music21's objects and
//! as the crate's, handed to the same function on each side, and what comes
//! out is written as the same text. music21 is music21 here, with nothing of
//! the crate installed over it.

use music21_rs::instrument::{
    bundle_instruments, deduplicate, partition_by_instrument, unbundle_instruments,
};
use music21_rs::{Stream, StreamKind};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod built_streams;
use built_streams::{
    BUILD, Named, Spec, build_stream, instrument, literal, measure, note, outline, played_on,
    stream,
};

/// Which function a case is handed to, and its name in music21.
#[derive(Clone, Copy, Debug)]
enum Walk {
    Unbundle,
    Bundle,
    /// Unbundled and then bundled again, as music21's own example does.
    UnbundleThenBundle,
    Deduplicate,
    Partition,
}

/// What each side runs for a walk, as Python.
const WALKS: &str = r#"
from music21 import instrument

def walk(which, held):
    if which == 'Unbundle':
        return instrument.unbundleInstruments(held)
    if which == 'Bundle':
        return instrument.bundleInstruments(held)
    if which == 'UnbundleThenBundle':
        return instrument.bundleInstruments(instrument.unbundleInstruments(held))
    if which == 'Deduplicate':
        return instrument.deduplicate(held)
    if which == 'Partition':
        return instrument.partitionByInstrument(held)
    raise ValueError(which)
"#;

fn ours(which: Walk, mut held: Stream) -> Stream {
    match which {
        Walk::Unbundle => unbundle_instruments(&mut held),
        Walk::Bundle => bundle_instruments(&mut held),
        Walk::UnbundleThenBundle => {
            unbundle_instruments(&mut held);
            bundle_instruments(&mut held);
        }
        Walk::Deduplicate => deduplicate(&mut held),
        Walk::Partition => return partition_by_instrument(&held),
    }
    held
}

fn named(kind: &'static str, part_name: Option<&'static str>, name: Named) -> Spec {
    Spec::Instrument {
        kind,
        part_name,
        name,
    }
}

/// music21's own example for `partitionByInstrument`: two parts, each
/// changing instrument in the middle of a measure.
fn two_parts_changing_instruments() -> Spec {
    let part = |clef: &'static str,
                names: [&'static str; 9],
                changes: [(usize, f64, &'static str); 3]|
     -> Spec {
        let mut measures: Vec<Vec<(f64, Spec)>> = vec![
            vec![(0.0, Spec::Clef(clef)), (0.0, Spec::Meter("4/4"))],
            Vec::new(),
            Vec::new(),
        ];
        for (index, name) in names.iter().enumerate().take(8) {
            measures[index / 4].push(((index % 4) as f64, note(name, 1.0)));
        }
        measures[2].push((0.0, note(names[8], 4.0)));
        for (which, at, kind) in changes {
            measures[which].push((at, instrument(kind)));
        }
        stream(
            StreamKind::Part,
            measures
                .into_iter()
                .enumerate()
                .map(|(index, held)| (index as f64 * 4.0, measure(index as i32 + 1, held)))
                .collect(),
        )
    };
    stream(
        StreamKind::Score,
        vec![
            (
                0.0,
                part(
                    "treble",
                    ["C4", "D4", "E4", "F4", "G4", "A4", "B4", "C5", "C4"],
                    [
                        (0, 0.0, "Piccolo"),
                        (0, 2.0, "AltoSaxophone"),
                        (1, 3.0, "Piccolo"),
                    ],
                ),
            ),
            (
                0.0,
                part(
                    "bass",
                    [
                        "C#3", "D#3", "E#3", "F#3", "G#3", "A#3", "B#3", "C#4", "C#3",
                    ],
                    [
                        (0, 0.0, "Trombone"),
                        (0, 3.0, "Piccolo"),
                        (1, 1.0, "Trombone"),
                    ],
                ),
            ),
        ],
    )
}

fn cases() -> Vec<(&'static str, Walk, Spec)> {
    let plain = StreamKind::Stream;
    let part = StreamKind::Part;
    let score = StreamKind::Score;
    vec![
        (
            "music21's example: an instrument beside each stroke that keeps one",
            Walk::Unbundle,
            stream(
                plain,
                vec![
                    (0.0, played_on("C4", "BassDrum")),
                    (1.0, played_on("D4", "Cowbell")),
                ],
            ),
        ),
        (
            "music21's example: the stroke between takes the bass drum",
            Walk::UnbundleThenBundle,
            stream(
                plain,
                vec![
                    (0.0, played_on("C4", "BassDrum")),
                    (1.0, note("D4", 1.0)),
                    (2.0, played_on("E4", "Cowbell")),
                ],
            ),
        ),
        (
            "unbundling looks at the stream's own notes and not a measure's",
            Walk::Unbundle,
            stream(
                part,
                vec![
                    (0.0, played_on("C4", "Violin")),
                    (1.0, measure(1, vec![(0.0, played_on("D4", "Flute"))])),
                    (2.0, Spec::Rest(1.0)),
                    (
                        3.0,
                        Spec::Chord {
                            names: "C4 E4",
                            length: 1.0,
                        },
                    ),
                ],
            ),
        ),
        (
            "of two instruments standing together the later is the one kept",
            Walk::Bundle,
            stream(
                plain,
                vec![
                    (0.0, instrument("Violin")),
                    (0.0, instrument("Flute")),
                    (0.0, note("C4", 1.0)),
                    (1.0, instrument("Viola")),
                    (1.0, note("D4", 1.0)),
                    (2.0, note("E4", 1.0)),
                ],
            ),
        ),
        (
            "a note before any instrument is left with none of its own",
            Walk::Bundle,
            stream(
                plain,
                vec![
                    (0.0, played_on("C4", "Oboe")),
                    (1.0, instrument("Violin")),
                    (1.0, note("D4", 1.0)),
                    (2.0, Spec::Rest(1.0)),
                    (
                        3.0,
                        measure(1, vec![(0.0, instrument("Tuba")), (0.0, note("F4", 1.0))]),
                    ),
                ],
            ),
        ),
        (
            "music21's example: two bare instruments naming different things",
            Walk::Deduplicate,
            stream(
                plain,
                vec![
                    (
                        4.0,
                        named("Instrument", None, Named::As("Semi-Hollow Body")),
                    ),
                    (
                        4.0,
                        named("Instrument", Some("Electric Guitar"), Named::AsMade),
                    ),
                ],
            ),
        ),
        (
            "music21's example: a bare instrument beside a piccolo, and two flutes",
            Walk::Deduplicate,
            stream(
                score,
                vec![
                    (
                        0.0,
                        stream(
                            part,
                            vec![
                                (0.0, named("Instrument", Some("Piccolo"), Named::AsMade)),
                                (0.0, instrument("Piccolo")),
                            ],
                        ),
                    ),
                    (
                        0.0,
                        stream(
                            part,
                            vec![(0.0, instrument("Flute")), (0.0, instrument("Flute"))],
                        ),
                    ),
                ],
            ),
        ),
        (
            "instruments that disagree on the part's name are left alone",
            Walk::Deduplicate,
            stream(
                part,
                vec![
                    (0.0, named("Violin", Some("First"), Named::AsMade)),
                    (0.0, named("Violin", Some("Second"), Named::AsMade)),
                    (0.0, note("C4", 1.0)),
                ],
            ),
        ),
        (
            "instruments far apart in a part's measures are still one",
            Walk::Deduplicate,
            stream(
                score,
                vec![(
                    0.0,
                    stream(
                        part,
                        vec![
                            (
                                0.0,
                                measure(
                                    1,
                                    vec![(0.0, instrument("Piano")), (0.0, note("C4", 4.0))],
                                ),
                            ),
                            (
                                4.0,
                                measure(
                                    2,
                                    vec![(0.0, note("D4", 2.0)), (2.0, instrument("Piano"))],
                                ),
                            ),
                        ],
                    ),
                )],
            ),
        ),
        (
            "of several kinds the bare ones go and the rest take the names",
            Walk::Deduplicate,
            stream(
                score,
                vec![
                    (0.0, instrument("Flute")),
                    (
                        0.0,
                        stream(
                            part,
                            vec![
                                (
                                    0.0,
                                    measure(
                                        1,
                                        vec![
                                            (
                                                0.0,
                                                named("Instrument", Some("Keys"), Named::Nothing),
                                            ),
                                            (0.0, note("C4", 4.0)),
                                        ],
                                    ),
                                ),
                                (
                                    4.0,
                                    measure(
                                        2,
                                        vec![
                                            (0.0, instrument("Piano")),
                                            (1.0, named("Harpsichord", None, Named::Nothing)),
                                            (0.0, note("D4", 4.0)),
                                        ],
                                    ),
                                ),
                            ],
                        ),
                    ),
                ],
            ),
        ),
        (
            "music21's example: three instruments over two parts",
            Walk::Partition,
            two_parts_changing_instruments(),
        ),
        (
            "an instrument overtaken where it stands takes only what lasts no time",
            Walk::Partition,
            stream(
                part,
                vec![
                    (0.0, Spec::Clef("treble")),
                    (0.0, instrument("Violin")),
                    (0.0, instrument("Flute")),
                    (0.0, note("C4", 1.0)),
                    (1.0, note("D4", 1.0)),
                    (1.0, instrument("Viola")),
                ],
            ),
        ),
        (
            "instruments with no name share the part of the last of them",
            Walk::Partition,
            stream(
                part,
                vec![
                    (0.0, named("Violin", Some("One"), Named::Nothing)),
                    (0.0, note("C4", 1.0)),
                    (1.0, named("Flute", Some("Two"), Named::Nothing)),
                    (1.0, note("D4", 1.0)),
                ],
            ),
        ),
        (
            "a stream with no instrument comes back as its parts flattened",
            Walk::Partition,
            stream(
                score,
                vec![(
                    0.0,
                    stream(
                        part,
                        vec![(
                            0.0,
                            measure(1, vec![(0.0, note("C4", 1.0)), (1.0, note("D4", 1.0))]),
                        )],
                    ),
                )],
            ),
        ),
        (
            "a stream of loose notes is one part to split",
            Walk::Partition,
            stream(
                plain,
                vec![
                    (0.0, instrument("Piccolo")),
                    (0.0, note("C4", 1.0)),
                    (1.0, instrument("Trombone")),
                    (1.0, note("D4", 1.0)),
                    (2.0, note("E4", 2.0)),
                    (3.0, instrument("Piccolo")),
                    (4.0, note("F4", 1.0)),
                ],
            ),
        ),
    ]
}

#[test]
fn instruments_rearrange_a_stream_as_music21_s_do() {
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
        let walks = module(WALKS, "instrument_walks")?;
        let read = py.import("ast")?.getattr("literal_eval")?;

        let mut failures = Vec::new();
        for (label, which, spec) in cases() {
            let held = built
                .getattr("build")?
                .call1((read.call1((literal(&spec),))?,))?;
            let walked = walks.getattr("walk")?.call1((format!("{which:?}"), held))?;
            let theirs: String = built.getattr("outline")?.call1((walked,))?.extract()?;
            let ours = outline(&ours(which, build_stream(&spec)));
            if std::env::var_os("PARITY_SHOW").is_some() {
                println!(
                    "{label} ({which:?})
{theirs}
"
                );
            }
            if ours != theirs {
                failures.push(format!(
                    "{label} ({which:?})\n--- music21\n{theirs}\n--- music21-rs\n{ours}"
                ));
            }
        }
        Ok(failures)
    })
    .unwrap_or_else(|error| panic!("running music21: {error}"));

    assert!(
        failures.is_empty(),
        "{} of {} streams come out differently:\n\n{}",
        failures.len(),
        cases().len(),
        failures.join("\n\n")
    );
}
