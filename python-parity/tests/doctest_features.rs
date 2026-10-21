//! music21's `features/base.py` doctests against the crate. See
//! `src/doctest.rs`. The reading is the crate's `features`, whose
//! extractors `features_parity` holds to music21 vector for vector; what
//! is swapped is the data set and the five ways an extractor is found by
//! its id or by its name.
//!
//! `FeatureExtractor`, `Feature`, `StreamForms` and `DataInstance` stay
//! music21's. The first is the base class every one of its extractors was
//! built from before any of this runs, so standing in for it would reach
//! neither them nor its own examples, which are all demonstrated on a
//! subclass; the other three are a bag of attributes, a cache of a stream
//! read a dozen ways, and that cache with a path to parse. Their examples
//! pass on music21's own code.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    features_doctests_against_the_crate,
    "music21.features.base",
    "features",
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
        ("music21.features.base", doctest::FEATURES_NAMES),
    ]
);
