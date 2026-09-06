//! music21's `interval.py` doctests against the crate. See `src/doctest.rs`.

use music21_rs_python_parity::{doctest, music21_rs_facade};

#[test]
fn interval_doctests_against_the_crate() {
    pyo3::append_to_inittab!(music21_rs_facade);
    doctest::run(
        "music21.interval",
        "interval",
        &[
            ("music21.pitch", doctest::PITCH_NAMES),
            ("music21.interval", doctest::INTERVAL_NAMES),
        ],
    );
}
