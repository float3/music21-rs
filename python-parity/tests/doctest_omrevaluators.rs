//! music21's `omr/evaluators.py` doctests against the crate. See
//! `src/doctest.rs`. There is no facade for this module and it needs
//! none: `OmrGroundTruthPair` reads two files into music21 scores and
//! drives music21's own `ScoreCorrector` over them, and every rhythm that
//! driver compares is hashed by this package's `MeasureHash` -- so
//! `getDifferences` answering 32, and the whole correction
//! `evaluateCorrectingModel` reports, are measured against the crate.
//!
//! `minEditDist` stays music21's for the same reason its driver does:
//! `omr::evaluators::min_edit_distance` is the crate's reading of it, and
//! running music21's over our hashes is the harder test.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    omr_evaluators_doctests_against_the_crate,
    "music21.omr.evaluators",
    "omrevaluators",
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
