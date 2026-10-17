//! music21's `repeat.py` doctests against the crate. See `src/doctest.rs`.
//! The marks are the crate's `repeat::RepeatExpression`, one value for
//! every one of music21's eleven classes; `Expander` stays music21's,
//! since playing a score's repeats out is one function here
//! (`Stream::expand_repeats`, held to music21 by `expand_repeats_parity`
//! on every corpus file with repeats) and the object music21 keeps that
//! walk's state on has no counterpart.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    repeat_doctests_against_the_crate,
    "music21.repeat",
    "repeat",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.note", doctest::NOTE_NAMES),
        ("music21.duration", doctest::DURATION_NAMES),
        ("music21.chord", doctest::CHORD_NAMES),
        ("music21.tie", doctest::TIE_NAMES),
        ("music21.volume", doctest::VOLUME_NAMES),
        ("music21.beam", doctest::BEAM_NAMES),
        ("music21.bar", doctest::BAR_NAMES),
        ("music21.repeat", doctest::REPEAT_NAMES),
    ]
);
