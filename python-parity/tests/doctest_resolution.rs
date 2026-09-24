//! music21's `figuredBass/resolution.py` doctests against the crate. See
//! `src/doctest.rs`.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    resolution_doctests_against_the_crate,
    "music21.figuredBass.resolution",
    "resolution",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.figuredBass.resolution", doctest::RESOLUTION_NAMES)
    ]
);
