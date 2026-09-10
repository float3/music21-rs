//! music21's `figuredBass/notation.py` doctests against the crate. See
//! `src/doctest.rs`.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    figured_bass_doctests_against_the_crate,
    "music21.figuredBass.notation",
    "figuredbass",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.figuredBass.notation", doctest::FIGURED_BASS_NAMES),
    ]
);
