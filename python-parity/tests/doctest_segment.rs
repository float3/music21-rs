//! music21's `figuredBass/segment.py` doctests against the crate. See
//! `src/doctest.rs`.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    segment_doctests_against_the_crate,
    "music21.figuredBass.segment",
    "segment",
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
