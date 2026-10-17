//! music21's `bar.py` doctests against the crate. See `src/doctest.rs`.
//! A barline here is the crate's value -- what it is drawn as, which way a
//! repeat plays and how many times -- and `musicxml_parity` holds what is
//! written of one to music21 byte for byte.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    bar_doctests_against_the_crate,
    "music21.bar",
    "bar",
    [("music21.bar", doctest::BAR_NAMES)]
);
