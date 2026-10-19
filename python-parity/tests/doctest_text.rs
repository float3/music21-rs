//! music21's `text.py` doctests against the crate. See `src/doctest.rs`.
//! The lyrics a stream is read for, and the articles moved to the front of
//! a title and back, are the crate's; a text box is drawn with music21's
//! own `TextStyle`, as everything styled in this package is.
//!
//! `LanguageDetector` and `Trigram` stay music21's: guessing which
//! language a line is in is a table of letter triplets rather than
//! anything musical, and the crate has none, so their docstrings run
//! music21's own code.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    text_doctests_against_the_crate,
    "music21.text",
    "text",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.note", doctest::NOTE_NAMES),
        ("music21.duration", doctest::DURATION_NAMES),
        ("music21.chord", doctest::CHORD_NAMES),
        ("music21.tie", doctest::TIE_NAMES),
        ("music21.volume", doctest::VOLUME_NAMES),
        ("music21.beam", doctest::BEAM_NAMES),
        ("music21.text", doctest::TEXT_NAMES),
    ]
);
