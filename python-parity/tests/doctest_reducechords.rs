//! music21's `analysis/reduceChords.py` doctests against the crate. See
//! `src/doctest.rs`. The reduction is the crate's
//! `analysis::reduce_chords`, which `reduce_chords_parity` holds to music21
//! chord for chord on twelve corpus scores.
//!
//! The steps that take music21's own *mutable* timespan tree and walk it
//! while editing it -- `collapseArpeggios`, `splitByBass`, `fillBassGaps`
//! and the rest -- are refused with that reason rather than answered
//! differently; the crate reads a score as a list of spans of its own and
//! runs every step over that list in music21's order, which is what `run`
//! answers with. `collapseArpeggios` is the only one of the three
//! documented members among them, and it is 11 of this module's 28
//! examples.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    reduce_chords_doctests_against_the_crate,
    "music21.analysis.reduceChords",
    "reducechords",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.note", doctest::NOTE_NAMES),
        ("music21.duration", doctest::DURATION_NAMES),
        ("music21.chord", doctest::CHORD_NAMES),
        ("music21.tie", doctest::TIE_NAMES),
        ("music21.volume", doctest::VOLUME_NAMES),
        ("music21.beam", doctest::BEAM_NAMES),
        ("music21.tempo", doctest::TEMPO_NAMES),
        ("music21.clef", doctest::CLEF_NAMES),
        ("music21.key", doctest::KEY_NAMES),
        ("music21.meter.base", doctest::METER_NAMES),
        ("music21.meter.core", doctest::METER_CORE_NAMES),
        ("music21.instrument", doctest::INSTRUMENT_NAMES),
        ("music21.dynamics", doctest::DYNAMICS_NAMES),
        ("music21.expressions", doctest::EXPRESSION_NAMES),
        ("music21.articulations", doctest::ARTICULATION_NAMES),
        ("music21.bar", doctest::BAR_NAMES),
        (
            "music21.analysis.reduceChords",
            doctest::REDUCE_CHORDS_NAMES
        ),
    ]
);
