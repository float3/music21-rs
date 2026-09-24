//! music21's `analysis/enharmonics.py` doctests against the crate. See
//! `src/doctest.rs`. The simplifier stays music21's, holding its rules and
//! spellings as state, and spells the crate's pitches here; the crate's own
//! `best_spelling` answers beneath the neo-Riemannian doctests.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    enharmonics_doctests_against_the_crate,
    "music21.analysis.enharmonics",
    "enharmonics",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
    ]
);
