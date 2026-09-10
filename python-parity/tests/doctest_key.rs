//! music21's `key.py` doctests against the crate. See `src/doctest.rs`.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    key_doctests_against_the_crate,
    "music21.key",
    "key",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.key", doctest::KEY_NAMES),
    ]
);
