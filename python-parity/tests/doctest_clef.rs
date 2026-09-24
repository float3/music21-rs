//! music21's `clef.py` doctests against the crate. See `src/doctest.rs`.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    clef_doctests_against_the_crate,
    "music21.clef",
    "clef",
    [("music21.clef", doctest::CLEF_NAMES)]
);
