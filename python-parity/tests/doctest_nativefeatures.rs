//! music21's `features/native.py` doctests against the crate. See
//! `src/doctest.rs`. Every one of these extractors is a row of
//! `features::native::NATIVE`, which `features_parity` holds to music21
//! vector for vector.
//!
//! `LanguageFeature` is not swapped: it reads which language a text is in,
//! which the crate answers only behind its `language-detection` feature and
//! which the wheel does not turn on, so there is no row to stand behind it.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    native_features_doctests_against_the_crate,
    "music21.features.native",
    "nativefeatures",
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
        ("music21.features.native", doctest::FEATURES_NATIVE_NAMES),
    ]
);
