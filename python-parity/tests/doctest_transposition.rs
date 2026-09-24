//! music21's `analysis/transposition.py` doctests against the crate. See
//! `src/doctest.rs`. The checker is the crate's, over the crate's pitches
//! and chords.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    transposition_doctests_against_the_crate,
    "music21.analysis.transposition",
    "transposition",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.note", doctest::NOTE_NAMES),
        ("music21.duration", doctest::DURATION_NAMES),
        ("music21.chord", doctest::CHORD_NAMES),
        (
            "music21.analysis.transposition",
            doctest::TRANSPOSITION_NAMES
        ),
    ]
);
