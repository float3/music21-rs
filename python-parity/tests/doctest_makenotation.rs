//! music21's `stream/makeNotation.py` doctests against the crate. See
//! `src/doctest.rs`. The steps are the crate's `makenotation`, which
//! `musicxml_make_notation_parity` holds to music21 byte for byte on 257
//! subjects, and what each decides is carried back by position onto
//! music21's own notes.
//!
//! Only the steps that *write onto what they are given* are swapped so
//! far -- `makeBeams`, `makeTupletBrackets`, `setStemDirectionForBeamGroups`,
//! and the two that only read. `makeMeasures`, `makeRests`, `makeTies`,
//! `realizeOrnaments`, `splitElementsToCompleteTuplets` and
//! `consolidateCompletedTuplets` change which elements there are, so a
//! position no longer names the same thing on both sides; each wants a
//! by-position twin in the crate saying what comes to be where, and until
//! then their docstrings are music21's own.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    make_notation_doctests_against_the_crate,
    "music21.stream.makeNotation",
    "makenotation",
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
        ("music21.percussion", doctest::PERCUSSION_NAMES),
        ("music21.stream.makeNotation", doctest::MAKE_NOTATION_NAMES),
    ]
);
