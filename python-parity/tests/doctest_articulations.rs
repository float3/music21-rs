//! music21's `articulations.py` doctests against the crate. See
//! `src/doctest.rs`.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    articulations_doctests_against_the_crate,
    "music21.articulations",
    "articulations",
    [("music21.articulations", doctest::ARTICULATION_NAMES)]
);
