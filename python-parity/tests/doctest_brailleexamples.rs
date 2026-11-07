//! music21's `braille/examples.py` doctests against the crate. See
//! `src/doctest.rs`.
//!
//! The examples write whole pieces, through music21's own `translate`,
//! which is left music21's here: what it is run over is the crate's
//! segments -- `findSegments`, `BrailleSegment` and the grand segment a
//! piano score is written with -- holding the crate's notes, which is the
//! test these examples make of the segments. `braille_parity` holds the
//! crate's own `translate` to the same pieces.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    braille_examples_doctests_against_the_crate,
    "music21.braille.examples",
    "brailleexamples",
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
        ("music21.braille.segment", doctest::BRAILLE_SEGMENT_NAMES),
        (
            "music21.braille.noteGrouping",
            doctest::BRAILLE_NOTE_GROUPING_NAMES
        ),
    ]
);
