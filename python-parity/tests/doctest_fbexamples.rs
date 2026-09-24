//! music21's `figuredBass/examples.py` doctests against the crate. See
//! `src/doctest.rs`. music21's realizer drives the crate's segments here.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    examples_doctests_against_the_crate,
    "music21.figuredBass.examples",
    "examples",
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
