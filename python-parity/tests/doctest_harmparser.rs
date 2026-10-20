//! music21's `humdrum/harmParser.py` doctests against the crate. See
//! `src/doctest.rs`. The reading of a `**harm` token and the numeral it
//! writes are `humdrum::harm`'s, which `humdrum_parity` holds to music21
//! token for token; `HarmDefs` stays music21's, being the regular
//! expressions themselves.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    harm_parser_doctests_against_the_crate,
    "music21.humdrum.harmParser",
    "harmparser",
    [("music21.humdrum.harmParser", doctest::HARM_PARSER_NAMES)]
);
