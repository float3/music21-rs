//! music21's `figuredBass/harmony.py` doctests against the crate. See
//! `src/doctest.rs`. Its `FiguredBass` stays music21's and reads its figures
//! through the crate's `Notation`.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    figured_bass_harmony_doctests_against_the_crate,
    "music21.figuredBass.harmony",
    "fbharmony",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.figuredBass.notation", doctest::FIGURED_BASS_NAMES),
    ]
);
