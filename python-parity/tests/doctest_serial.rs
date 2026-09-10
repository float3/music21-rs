//! music21's `serial.py` doctests against the crate. See `src/doctest.rs`.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    serial_doctests_against_the_crate,
    "music21.serial",
    "serial",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.serial", doctest::SERIAL_NAMES),
    ]
);
