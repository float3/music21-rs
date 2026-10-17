//! music21's `analysis/elements.py` doctests against the crate. See
//! `src/doctest.rs`.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    elements_doctests_against_the_crate,
    "music21.analysis.elements",
    "elements",
    [("music21.analysis.elements", doctest::ELEMENTS_NAMES)]
);
