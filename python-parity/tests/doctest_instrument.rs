//! music21's `instrument.py` doctests against the crate. See `src/doctest.rs`.
//!
//! music21's instrument docstrings build pitches and transpose by intervals,
//! so those modules are swapped too.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    instrument_doctests_against_the_crate,
    "music21.instrument",
    "instrument",
    [
        ("music21.instrument", doctest::INSTRUMENT_NAMES),
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
    ]
);
