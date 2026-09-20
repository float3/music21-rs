//! music21's `meter/core.py` doctests against the crate. See `src/doctest.rs`.
//!
//! `MeterTerminal` and `MeterSequence` are one type here, `meter::sequence`'s
//! `MeterTerminal`: a terminal carrying parts is a sequence. The facade splits
//! it back into music21's two classes, and these are music21's own docstrings
//! for both of them.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    meter_core_doctests_against_the_crate,
    "music21.meter.core",
    "metercore",
    [
        ("music21.duration", doctest::DURATION_NAMES),
        ("music21.meter.core", doctest::METER_CORE_NAMES),
    ]
);
