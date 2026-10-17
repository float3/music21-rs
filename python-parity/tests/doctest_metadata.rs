//! music21's `metadata/__init__.py` doctests against the crate. See
//! `src/doctest.rs`. The values are the crate's -- a name and a role --
//! and the property vocabulary is `metadata::STANDARD_PROPERTIES`, which
//! `metadata_parity` holds to music21's live table.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    metadata_doctests_against_the_crate,
    "music21.metadata",
    "metadata",
    [("music21.metadata", doctest::METADATA_NAMES)]
);
