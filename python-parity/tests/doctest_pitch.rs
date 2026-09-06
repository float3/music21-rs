//! music21's `pitch.py` doctests against the crate. See `src/doctest.rs`.

use music21_rs_python_parity::{doctest, music21_rs_facade};

#[test]
fn pitch_doctests_against_the_crate() {
    pyo3::append_to_inittab!(music21_rs_facade);
    doctest::run(
        "music21.pitch",
        "pitch",
        &[("music21.pitch", doctest::PITCH_NAMES)],
    );
}
