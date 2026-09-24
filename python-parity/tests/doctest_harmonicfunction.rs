//! music21's `analysis/harmonicFunction.py` doctests against the crate. See
//! `src/doctest.rs`. The enum is built from the crate's labels, and both
//! lookups are the crate's.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    harmonicfunction_doctests_against_the_crate,
    "music21.analysis.harmonicFunction",
    "harmonicfunction",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.note", doctest::NOTE_NAMES),
        ("music21.duration", doctest::DURATION_NAMES),
        ("music21.chord", doctest::CHORD_NAMES),
        ("music21.key", doctest::KEY_NAMES),
        ("music21.scale", doctest::SCALE_NAMES),
        ("music21.roman", doctest::ROMAN_NAMES),
        (
            "music21.analysis.harmonicFunction",
            doctest::HARMONIC_FUNCTION_NAMES
        ),
    ]
);
