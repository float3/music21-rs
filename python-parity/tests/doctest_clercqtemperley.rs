//! music21's `romanText/clercqTemperley.py` doctests against the crate.
//! See `src/doctest.rs`. The reading is the crate's
//! `romantext::clercq_temperley`, which `clercq_temperley_parity` holds
//! to music21 on 207 songs by outline and then byte for byte as
//! MusicXML.
//!
//! The part a song plays through to is music21's own, built by
//! `stream::music21_from_crate`: music21's own example indexes it by
//! class. The four steps inside the expansion are refused rather than
//! answered differently, the crate driving the expansion from the song
//! rather than from the rule that keeps the state.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    clercq_temperley_doctests_against_the_crate,
    "music21.romanText.clercqTemperley",
    "clercqtemperley",
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
        ("music21.metadata", doctest::METADATA_NAMES),
        (
            "music21.romanText.clercqTemperley",
            doctest::CLERCQ_TEMPERLEY_NAMES
        ),
    ]
);
