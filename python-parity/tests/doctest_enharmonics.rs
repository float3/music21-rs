//! music21's `analysis/enharmonics.py` doctests against the crate. See
//! `src/doctest.rs`. The simplifier and its rule classes are the crate's,
//! spelling the crate's pitches.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    enharmonics_doctests_against_the_crate,
    "music21.analysis.enharmonics",
    "enharmonics",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.analysis.enharmonics", doctest::ENHARMONICS_NAMES),
    ]
);
