//! music21's `scale/scala/__init__.py` doctests against the crate. See
//! `src/doctest.rs`.
//!
//! The module's doctest builds a `scale.ScalaScale` from the archive, which
//! hands the crate's intervals to the scale; the scale and interval modules
//! are the crate's as well, so the two meet.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    scala_doctests_against_the_crate,
    "music21.scale.scala",
    "scala",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.scale", doctest::SCALE_NAMES),
        ("music21.scale.scala", doctest::SCALA_NAMES),
    ]
);
