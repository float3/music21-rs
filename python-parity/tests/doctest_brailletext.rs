//! music21's `braille/text.py` doctests against the crate. See
//! `src/doctest.rs`. The laying out is `braille::text`'s; the facade keeps
//! each line as the object music21 keeps it as, since its own examples
//! write through one and read the text back.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    braille_text_doctests_against_the_crate,
    "music21.braille.text",
    "brailletext",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.note", doctest::NOTE_NAMES),
        ("music21.duration", doctest::DURATION_NAMES),
        ("music21.chord", doctest::CHORD_NAMES),
        ("music21.tie", doctest::TIE_NAMES),
        ("music21.volume", doctest::VOLUME_NAMES),
        ("music21.beam", doctest::BEAM_NAMES),
        ("music21.meter.base", doctest::METER_NAMES),
        ("music21.meter.core", doctest::METER_CORE_NAMES),
        ("music21.key", doctest::KEY_NAMES),
        ("music21.braille.basic", doctest::BRAILLE_BASIC_NAMES),
        ("music21.braille.text", doctest::BRAILLE_TEXT_NAMES),
    ]
);
