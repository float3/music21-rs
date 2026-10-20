//! music21's `braille/segment.py` doctests against the crate. See
//! `src/doctest.rs`.
//!
//! `BrailleElementGrouping` is the crate's -- a run of elements braille
//! writes together -- and so is the transcriber music21's segments ask for
//! each grouping (`braille.noteGrouping`). The segments themselves,
//! `BrailleSegment` and `BrailleGrandSegment` and the functions that cut a
//! part into them, are still music21's here, running over the crate's
//! groupings and the crate's transcriptions, which is the harder test of
//! the two.
//!
//! `braille.text` is deliberately *not* swapped here, though it has a
//! facade of its own: music21's `BrailleSegment` derives from its
//! `BrailleText` and calls `text.BrailleText.__str__(self)` by name, so
//! with the base class swapped under it that call is handed an object of
//! the class it no longer names. A class already made keeps the base it
//! was made from, which `rebind` cannot change; its own suite covers the
//! laying out.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    braille_segment_doctests_against_the_crate,
    "music21.braille.segment",
    "braillesegment",
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
