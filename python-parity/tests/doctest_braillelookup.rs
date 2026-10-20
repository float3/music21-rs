//! music21's `braille/lookup.py` doctests against the crate. See
//! `src/doctest.rs`. The two functions that make a sign out of its dots
//! are `braille::lookup`'s; the tables stay music21's, the crate
//! answering what they say as functions rather than as dictionaries.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    braille_lookup_doctests_against_the_crate,
    "music21.braille.lookup",
    "braillelookup",
    [("music21.braille.lookup", doctest::BRAILLE_LOOKUP_NAMES)]
);
