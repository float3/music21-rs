//! music21's `analysis/reduction.py` doctests against the crate. See
//! `src/doctest.rs`. The reduction is the crate's
//! `analysis::reduction::ScoreReduction`, and the score it answers is
//! music21's own, built by `stream::music21_from_crate`.
//!
//! `ReductiveNote` and `PartReduction` stay music21's: the first is a
//! specification read off a lyric, which the crate answers through
//! `ReductiveParameters`, and the second draws a graph of how busy each
//! part is, which is presentation. Neither has an example of its own.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    reduction_doctests_against_the_crate,
    "music21.analysis.reduction",
    "reduction",
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
        ("music21.analysis.reduction", doctest::REDUCTION_NAMES),
    ]
);
