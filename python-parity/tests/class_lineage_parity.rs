//! What the crate says a stream element *is*, against what music21 says.
//!
//! [`StreamElement::music21_classes`] is the hierarchy music21 answers
//! through `classes`, and every filter that takes a class name -- the
//! crate's `elements_of_class`, the Python package's `getElementsByClass`
//! and the rest -- is read through it. It was transcribed by hand and
//! never checked against music21; this builds music21's own object for
//! each kind the crate can hold and compares the two chains.
//!
//! music21's chain carries `ProtoM21Object`, `object` and, for a stream,
//! `typing.Generic` after `Music21Object`; the crate's stops there, so
//! music21's is cut at `Music21Object` before comparing. Everything
//! before it must be the same, in the same order: a `Chord` is a
//! `ChordBase` before it is a `NotRest`.

use music21_rs::clef::{Clef, ClefKind};
use music21_rs::instrument::Instrument;
use music21_rs::layout::{Layout, StaffLayout};
use music21_rs::{
    Barline, Chord, Dynamic, Key, KeySignature, MetricModulation, MetronomeMark, Note,
    PercussionChord, Rest, Stream, StreamElement, StreamKind, TimeSignature, Unpitched,
};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

/// Reads music21's own chain for an expression, cut at `Music21Object`.
const MUSIC21: &str = r#"
from music21 import (bar, chord, clef, dynamics, expressions, harmony, instrument, key,
                     layout, meter, note, percussion, roman, stream, tempo, text, variant,
                     volpiano)


def lineage(made):
    """The classes music21 says it is, down to `Music21Object`.

    What follows that is the plumbing every music21 object is built on --
    `ProtoM21Object`, `object`, and `typing.Generic` for a stream -- which
    the crate does not name.
    """
    names = list(made.classes)
    return names[: names.index('Music21Object') + 1]
"#;

/// A clef of a kind, for the kinds whose chain is more than `Clef`.
fn clef(kind: ClefKind) -> StreamElement {
    Clef::of_kind(kind).into()
}

/// An instrument of a kind.
fn instrument(kind: &str) -> StreamElement {
    Instrument::of_kind(kind)
        .unwrap_or_else(|_| panic!("music21 has an instrument class named {kind}"))
        .into()
}

/// A stream of a kind, which is all its chain turns on.
fn stream(kind: StreamKind) -> StreamElement {
    StreamElement::Stream(Box::new(Stream::with_kind(kind)))
}

/// Every kind the crate can hold, with the music21 expression that makes
/// the same thing.
fn subjects() -> Vec<(&'static str, StreamElement)> {
    let note = |name: &str| Note::from_name(name).expect("a note name");
    vec![
        ("note.Note('C4')", note("C4").into()),
        (
            "note.Rest()",
            Rest::new(music21_rs::Duration::quarter()).into(),
        ),
        ("note.Unpitched()", Unpitched::new().into()),
        (
            "chord.Chord(['C4', 'E4'])",
            Chord::new("C4 E4").expect("a chord").into(),
        ),
        (
            "percussion.PercussionChord([note.Unpitched()])",
            PercussionChord::new(vec![Unpitched::new().into()])
                .expect("a chord of one stroke")
                .into(),
        ),
        ("clef.Clef()", clef(ClefKind::Clef)),
        ("clef.TrebleClef()", clef(ClefKind::TrebleClef)),
        ("clef.BassClef()", clef(ClefKind::BassClef)),
        ("clef.AltoClef()", clef(ClefKind::AltoClef)),
        ("clef.PercussionClef()", clef(ClefKind::PercussionClef)),
        ("clef.TabClef()", clef(ClefKind::TabClef)),
        ("clef.Treble8vbClef()", clef(ClefKind::Treble8vbClef)),
        ("instrument.Instrument()", Instrument::new().into()),
        ("instrument.Flute()", instrument("Flute")),
        ("instrument.Ocarina()", instrument("Ocarina")),
        ("instrument.Violin()", instrument("Violin")),
        ("instrument.Piano()", instrument("Piano")),
        ("instrument.BassDrum()", instrument("BassDrum")),
        ("instrument.Soprano()", instrument("Soprano")),
        ("key.KeySignature(2)", KeySignature::new(2).into()),
        ("key.Key('G')", Key::from_tonic("G").expect("a key").into()),
        (
            "meter.TimeSignature('4/4')",
            TimeSignature::from_ratio_string("4/4")
                .expect("a meter")
                .into(),
        ),
        (
            "tempo.MetronomeMark(number=96)",
            MetronomeMark::new(96.0).into(),
        ),
        (
            "tempo.MetricModulation()",
            StreamElement::MetricModulation(Box::new(MetricModulation::new())),
        ),
        ("dynamics.Dynamic('mf')", Dynamic::new("mf").into()),
        (
            "bar.Barline()",
            Barline::new(music21_rs::bar::BarlineType::Regular).into(),
        ),
        (
            "layout.StaffLayout()",
            StreamElement::Layout(Layout::Staff(StaffLayout::default())),
        ),
        ("stream.Stream()", stream(StreamKind::Stream)),
        ("stream.Measure()", stream(StreamKind::Measure)),
        ("stream.Part()", stream(StreamKind::Part)),
        ("stream.PartStaff()", stream(StreamKind::PartStaff)),
        ("stream.Voice()", stream(StreamKind::Voice)),
        ("stream.Score()", stream(StreamKind::Score)),
        ("stream.Opus()", stream(StreamKind::Opus)),
    ]
}

#[test]
fn the_crate_says_what_a_thing_is_as_music21_says_it() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"class_lineage_parity_music21.py",
            c"class_lineage_parity_music21",
        )?;
        let mut failures = Vec::new();
        let mut compared = 0;
        for (expression, element) in subjects() {
            let made = py.eval(
                &std::ffi::CString::new(expression).expect("no nul in the expression"),
                Some(&music21.dict()),
                None,
            )?;
            let theirs: Vec<String> = music21.getattr("lineage")?.call1((made,))?.extract()?;
            let ours: Vec<String> = element
                .music21_classes()
                .iter()
                .map(|name| (*name).to_string())
                .collect();
            if ours != theirs {
                failures.push(format!(
                    "{expression}: music21 {theirs:?}, the crate {ours:?}"
                ));
            }
            compared += 1;
        }
        Ok((failures, compared))
    })
    .expect("ask music21 what each thing is");

    assert!(
        failures.is_empty(),
        "{} of {compared} kinds are not what music21 says they are:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
    assert!(compared >= 30, "only {compared} kinds were compared");
}
