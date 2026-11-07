//! music21's `braille/segment.py` doctests against the crate. See
//! `src/doctest.rs`.
//!
//! The groupings, the segments, the grand segment and the functions that
//! cut a part into them are the crate's facades, holding the crate's
//! notes; so is the transcriber they ask for each grouping
//! (`braille.noteGrouping`). `splitMeasure` is still music21's.
//!
//! `braille.text` is not swapped: the segments here are built on the
//! package's own `BrailleText` whatever music21's module holds, and its
//! own suite covers the laying out.

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
