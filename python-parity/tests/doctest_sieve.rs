//! music21's `sieve.py` doctests against the crate. See `src/doctest.rs`.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    sieve_doctests_against_the_crate,
    "music21.sieve",
    "sieve",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.sieve", doctest::SIEVE_NAMES),
    ]
);
