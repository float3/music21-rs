//! music21's `figuredBass/notation.py` doctests against the crate. See
//! `src/doctest.rs`.

use music21_rs_python_parity::{doctest, music21_rs_facade};

#[test]
fn figured_bass_doctests_against_the_crate() {
    pyo3::append_to_inittab!(music21_rs_facade);
    doctest::run(
        "music21.figuredBass.notation",
        "figuredbass",
        &[
            ("music21.pitch", doctest::PITCH_NAMES),
            ("music21.figuredBass.notation", doctest::FIGURED_BASS_NAMES),
        ],
    );
}
