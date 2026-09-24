//! music21's `analysis/neoRiemannian.py` doctests against the crate. See
//! `src/doctest.rs`. Every function is the crate's, reading the crate's
//! chords and pitches.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    neoriemannian_doctests_against_the_crate,
    "music21.analysis.neoRiemannian",
    "neoriemannian",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.note", doctest::NOTE_NAMES),
        ("music21.beam", doctest::BEAM_NAMES),
        ("music21.tie", doctest::TIE_NAMES),
        ("music21.duration", doctest::DURATION_NAMES),
        ("music21.chord", doctest::CHORD_NAMES),
        (
            "music21.analysis.neoRiemannian",
            doctest::NEO_RIEMANNIAN_NAMES
        ),
    ]
);
