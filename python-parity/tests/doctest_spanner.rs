//! music21's `spanner.py` doctests against the crate. See
//! `src/doctest.rs`. A spanner here holds the objects it joins, as
//! music21's does, and what it says of itself besides them -- the kind,
//! how it is drawn, an octave line's shift -- is the crate's value.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    spanner_doctests_against_the_crate,
    "music21.spanner",
    "spanner",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.note", doctest::NOTE_NAMES),
        ("music21.duration", doctest::DURATION_NAMES),
        ("music21.chord", doctest::CHORD_NAMES),
        ("music21.spanner", doctest::SPANNER_NAMES),
    ]
);
