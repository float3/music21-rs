//! music21's `metadata/__init__.py` doctests against the crate. See
//! `src/doctest.rs`. The values are the crate's `metadata::primitives` and
//! the property vocabulary is `metadata::STANDARD_PROPERTIES`, both of
//! which `metadata_parity` holds to music21's live tables.
//!
//! The primitives are swapped beside the metadata itself: what every
//! reading of a name answers is one of them, and a `repr` of one is what
//! most of this module's examples print.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    metadata_doctests_against_the_crate,
    "music21.metadata",
    "metadata",
    [
        ("music21.metadata.primitives", doctest::PRIMITIVES_NAMES),
        ("music21.metadata", doctest::METADATA_NAMES),
    ]
);
