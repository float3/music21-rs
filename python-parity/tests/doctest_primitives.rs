//! music21's `metadata/primitives.py` doctests against the crate. See
//! `src/doctest.rs`. These are the crate's `metadata::primitives`, which
//! `metadata_parity` holds to music21 name for name, relevance for
//! relevance and date for date.
//!
//! One crate type carries all four of music21's date classes, each of
//! which is a facade class over it refusing a relevance that is not its
//! own, exactly as music21's own setters refuse one.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    metadata_primitives_doctests_against_the_crate,
    "music21.metadata.primitives",
    "primitives",
    [("music21.metadata.primitives", doctest::PRIMITIVES_NAMES)]
);
