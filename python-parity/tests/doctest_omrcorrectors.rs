//! music21's `omr/correctors.py` doctests against the crate. See
//! `src/doctest.rs`. Every number of the correction is the crate's
//! `omr::correctors`, which `omr_parity` holds to music21 on corpus
//! scores and on music21's own pair of K. 525.
//!
//! `ScoreCorrector` stays music21's, and that is the harder test: it is
//! the driver, building a `SinglePart` for each part, a `MeasureSlice`
//! for each place and a `MeasureHash` for each measure, all three of them
//! ours -- and it is the one member that edits the score, which is
//! surgery on music21's own measure objects.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    omr_correctors_doctests_against_the_crate,
    "music21.omr.correctors",
    "omrcorrectors",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.note", doctest::NOTE_NAMES),
        ("music21.duration", doctest::DURATION_NAMES),
        ("music21.chord", doctest::CHORD_NAMES),
        ("music21.tie", doctest::TIE_NAMES),
        ("music21.volume", doctest::VOLUME_NAMES),
        ("music21.beam", doctest::BEAM_NAMES),
        ("music21.tempo", doctest::TEMPO_NAMES),
        ("music21.clef", doctest::CLEF_NAMES),
        ("music21.key", doctest::KEY_NAMES),
        ("music21.meter.base", doctest::METER_NAMES),
        ("music21.meter.core", doctest::METER_CORE_NAMES),
        ("music21.instrument", doctest::INSTRUMENT_NAMES),
        ("music21.dynamics", doctest::DYNAMICS_NAMES),
        ("music21.expressions", doctest::EXPRESSION_NAMES),
        ("music21.articulations", doctest::ARTICULATION_NAMES),
        ("music21.bar", doctest::BAR_NAMES),
        ("music21.omr.correctors", doctest::OMR_CORRECTORS_NAMES),
    ]
);
