//! music21's `languageExcerpts/naturalLanguageObjects.py` doctests against
//! the crate. See `src/doctest.rs`. The names are read by
//! `pitch::from_language_name`, whose table is written by the same
//! `german`, `french`, `italian` and `spanish` music21 writes its own
//! with.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    natural_language_doctests_against_the_crate,
    "music21.languageExcerpts.naturalLanguageObjects",
    "naturallanguage",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.note", doctest::NOTE_NAMES),
        ("music21.duration", doctest::DURATION_NAMES),
        ("music21.chord", doctest::CHORD_NAMES),
        (
            "music21.languageExcerpts.naturalLanguageObjects",
            doctest::NATURAL_LANGUAGE_NAMES
        ),
    ]
);
