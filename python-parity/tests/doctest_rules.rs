//! music21's `figuredBass/rules.py` doctests against the crate. See
//! `src/doctest.rs`.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    rules_doctests_against_the_crate,
    "music21.figuredBass.rules",
    "rules",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.figuredBass.rules", doctest::RULES_NAMES)
    ]
);
