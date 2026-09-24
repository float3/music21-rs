//! music21's `expressions.py` doctests against the crate. See
//! `src/doctest.rs`.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    expressions_doctests_against_the_crate,
    "music21.expressions",
    "expressions",
    [("music21.expressions", doctest::EXPRESSION_NAMES)]
);
