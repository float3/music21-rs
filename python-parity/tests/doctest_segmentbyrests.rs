//! music21's `analysis/segmentByRests.py` doctests against the crate. See
//! `src/doctest.rs`. The runs and the intervals are the crate's, over
//! music21's own notes.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    segment_by_rests_doctests_against_the_crate,
    "music21.analysis.segmentByRests",
    "segmentbyrests",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.note", doctest::NOTE_NAMES),
        ("music21.duration", doctest::DURATION_NAMES),
        ("music21.chord", doctest::CHORD_NAMES),
        ("music21.tie", doctest::TIE_NAMES),
        ("music21.volume", doctest::VOLUME_NAMES),
        ("music21.beam", doctest::BEAM_NAMES),
        (
            "music21.analysis.segmentByRests",
            doctest::SEGMENT_BY_RESTS_NAMES
        ),
    ]
);
