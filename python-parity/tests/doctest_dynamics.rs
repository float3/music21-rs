//! music21's `dynamics.py` doctests against the crate. See `src/doctest.rs`.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    dynamics_doctests_against_the_crate,
    "music21.dynamics",
    "dynamics",
    [("music21.dynamics", doctest::DYNAMICS_NAMES)]
);
