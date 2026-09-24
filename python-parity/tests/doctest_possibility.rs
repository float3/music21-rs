//! music21's `figuredBass/possibility.py` doctests against the crate. See
//! `src/doctest.rs`.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    possibility_doctests_against_the_crate,
    "music21.figuredBass.possibility",
    "possibility",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        (
            "music21.figuredBass.possibility",
            doctest::POSSIBILITY_NAMES
        )
    ]
);
