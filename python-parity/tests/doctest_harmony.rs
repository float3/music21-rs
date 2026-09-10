//! music21's `harmony.py` doctests against the crate. See `src/doctest.rs`.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    harmony_doctests_against_the_crate,
    "music21.harmony",
    "harmony",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.duration", doctest::DURATION_NAMES),
        ("music21.note", doctest::NOTE_NAMES),
        ("music21.beam", doctest::BEAM_NAMES),
        ("music21.tie", doctest::TIE_NAMES),
        ("music21.volume", doctest::VOLUME_NAMES),
        ("music21.chord", doctest::CHORD_NAMES),
        ("music21.key", doctest::KEY_NAMES),
        ("music21.harmony", doctest::HARMONY_NAMES),
    ]
);
