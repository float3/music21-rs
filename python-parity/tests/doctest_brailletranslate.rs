//! music21's `braille/translate.py` doctests against the crate. See
//! `src/doctest.rs`. The writing is `braille::translate`'s, which
//! `braille_parity` holds to music21 on built parts and corpus scores.
//!
//! The docstrings read parts out of TinyNotation and scores out of the
//! corpus, so every class those hold is swapped: the reading refuses a
//! music21 object it has no value for.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    braille_translate_doctests_against_the_crate,
    "music21.braille.translate",
    "brailletranslate",
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
        ("music21.braille.basic", doctest::BRAILLE_BASIC_NAMES),
        ("music21.braille.text", doctest::BRAILLE_TEXT_NAMES),
        (
            "music21.braille.translate",
            doctest::BRAILLE_TRANSLATE_NAMES
        ),
    ]
);
