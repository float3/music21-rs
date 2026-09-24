//! music21's `analysis/discrete.py` doctests against the crate. See
//! `src/doctest.rs`. The analysis classes stay music21's -- colour tables,
//! legends and a log of solutions around the arithmetic, which the crate
//! carries as `analysis` and `key_analysis_parity` checks -- and they read
//! the crate's pitches, notes, chords, intervals and keys here.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    discrete_doctests_against_the_crate,
    "music21.analysis.discrete",
    "discrete",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.note", doctest::NOTE_NAMES),
        ("music21.beam", doctest::BEAM_NAMES),
        ("music21.tie", doctest::TIE_NAMES),
        ("music21.duration", doctest::DURATION_NAMES),
        ("music21.chord", doctest::CHORD_NAMES),
        ("music21.key", doctest::KEY_NAMES),
    ]
);
