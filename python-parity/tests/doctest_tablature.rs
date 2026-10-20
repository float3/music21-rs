//! music21's `tablature.py` doctests against the crate. See
//! `src/doctest.rs`. A fret note and a fret board are the crate's values,
//! and what a board sounds is its own `pitches`: each string's open pitch
//! raised by the fret stopping it.
//!
//! `ChordWithFretBoard` stays music21's, which builds it out of its own
//! `ChordSymbol` and a board; the symbol's notes are the harmony facade's
//! either way.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    tablature_doctests_against_the_crate,
    "music21.tablature",
    "tablature",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.note", doctest::NOTE_NAMES),
        ("music21.duration", doctest::DURATION_NAMES),
        ("music21.chord", doctest::CHORD_NAMES),
        ("music21.tablature", doctest::TABLATURE_NAMES),
    ]
);
