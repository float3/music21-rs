//! music21's `analysis/metrical.py` doctests against the crate. See
//! `src/doctest.rs`. The beat depths and the melodic accents are the
//! crate's, written onto music21's own notes.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    metrical_doctests_against_the_crate,
    "music21.analysis.metrical",
    "metrical",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.note", doctest::NOTE_NAMES),
        ("music21.duration", doctest::DURATION_NAMES),
        ("music21.chord", doctest::CHORD_NAMES),
        ("music21.tie", doctest::TIE_NAMES),
        ("music21.volume", doctest::VOLUME_NAMES),
        ("music21.beam", doctest::BEAM_NAMES),
        ("music21.analysis.metrical", doctest::METRICAL_NAMES),
    ]
);
