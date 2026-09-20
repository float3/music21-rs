//! music21's `scale/scala/__init__.py` doctests against the crate. See
//! `src/doctest.rs`.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    scala_doctests_against_the_crate,
    "music21.scale.scala",
    "scala",
    [("music21.scale.scala", doctest::SCALA_NAMES)]
);
