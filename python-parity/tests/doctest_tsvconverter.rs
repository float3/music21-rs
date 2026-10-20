//! music21's `romanText/tsvConverter.py` doctests against the crate. See
//! `src/doctest.rs`. Every reading is the crate's `romantext::tsv`, which
//! `tsv_parity` holds to music21 on the four tables music21 ships and on
//! all 68 RomanText analyses of the corpus, written out in both of the
//! lab's standards.
//!
//! `TsvHandler` and `M21toTSV` stay music21's, and that is the harder
//! test: one reads a file and builds a music21 score out of the rows, the
//! other reads a score and writes the rows back, and both reach the crate
//! through the classes and functions they name.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    tsv_converter_doctests_against_the_crate,
    "music21.romanText.tsvConverter",
    "tsvconverter",
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
        ("music21.scale", doctest::SCALE_NAMES),
        ("music21.roman", doctest::ROMAN_NAMES),
        ("music21.harmony", doctest::HARMONY_NAMES),
        ("music21.meter.base", doctest::METER_NAMES),
        ("music21.meter.core", doctest::METER_CORE_NAMES),
        ("music21.instrument", doctest::INSTRUMENT_NAMES),
        ("music21.dynamics", doctest::DYNAMICS_NAMES),
        ("music21.expressions", doctest::EXPRESSION_NAMES),
        ("music21.articulations", doctest::ARTICULATION_NAMES),
        ("music21.bar", doctest::BAR_NAMES),
        (
            "music21.romanText.tsvConverter",
            doctest::TSV_CONVERTER_NAMES
        ),
    ]
);
