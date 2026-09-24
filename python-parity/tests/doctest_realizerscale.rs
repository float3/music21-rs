//! music21's `figuredBass/realizerScale.py` doctests against the crate. See
//! `src/doctest.rs`.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    realizer_scale_doctests_against_the_crate,
    "music21.figuredBass.realizerScale",
    "realizerscale",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        (
            "music21.figuredBass.realizerScale",
            doctest::REALIZER_SCALE_NAMES
        )
    ]
);
