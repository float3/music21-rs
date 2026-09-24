//! music21's `figuredBass/realizer.py` doctests against the crate. See
//! `src/doctest.rs`.
//!
//! The realizer's own classes stay music21's: they build a bass line and a
//! score out of streams, and it is music21's orchestration that drives the
//! crate's segments, rules and resolutions here.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    realizer_doctests_against_the_crate,
    "music21.figuredBass.realizer",
    "realizer",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        (
            "music21.figuredBass.possibility",
            doctest::POSSIBILITY_NAMES
        ),
        (
            "music21.figuredBass.realizerScale",
            doctest::REALIZER_SCALE_NAMES
        ),
        ("music21.figuredBass.rules", doctest::RULES_NAMES),
        ("music21.figuredBass.resolution", doctest::RESOLUTION_NAMES),
        ("music21.figuredBass.segment", doctest::SEGMENT_NAMES)
    ]
);
