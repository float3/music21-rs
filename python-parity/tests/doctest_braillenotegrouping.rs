//! music21's `braille/noteGrouping.py` doctests against the crate. See
//! `src/doctest.rs`. The transcribing is
//! `braille::segment::NoteGroupingTranscriber`'s, which every score
//! written in braille goes through.
//!
//! `braille.segment`'s `BrailleElementGrouping` is swapped with it: the
//! docstrings build one and ask the transcriber for it. The segments
//! themselves are still music21's.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    braille_note_grouping_doctests_against_the_crate,
    "music21.braille.noteGrouping",
    "braillenotegrouping",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.note", doctest::NOTE_NAMES),
        ("music21.duration", doctest::DURATION_NAMES),
        ("music21.chord", doctest::CHORD_NAMES),
        ("music21.tie", doctest::TIE_NAMES),
        ("music21.volume", doctest::VOLUME_NAMES),
        ("music21.beam", doctest::BEAM_NAMES),
        ("music21.clef", doctest::CLEF_NAMES),
        ("music21.key", doctest::KEY_NAMES),
        ("music21.meter.base", doctest::METER_NAMES),
        ("music21.meter.core", doctest::METER_CORE_NAMES),
        ("music21.dynamics", doctest::DYNAMICS_NAMES),
        ("music21.expressions", doctest::EXPRESSION_NAMES),
        ("music21.articulations", doctest::ARTICULATION_NAMES),
        ("music21.braille.basic", doctest::BRAILLE_BASIC_NAMES),
        ("music21.braille.segment", doctest::BRAILLE_SEGMENT_NAMES),
        (
            "music21.braille.noteGrouping",
            doctest::BRAILLE_NOTE_GROUPING_NAMES
        ),
    ]
);
