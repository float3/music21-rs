//! music21's `duration.py` doctests against the crate. See `src/doctest.rs`.

use music21_rs_python_parity::{doctest, music21_rs_facade};

#[test]
fn duration_doctests_against_the_crate() {
    pyo3::append_to_inittab!(music21_rs_facade);
    doctest::run(
        "music21.duration",
        "duration",
        &[
            ("music21.duration", doctest::DURATION_NAMES),
            ("music21.note", doctest::NOTE_NAMES),
            ("music21.pitch", doctest::PITCH_NAMES),
        ],
    );
}
