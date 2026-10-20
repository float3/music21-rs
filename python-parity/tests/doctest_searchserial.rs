//! music21's `search/serial.py` doctests against the crate. See
//! `src/doctest.rs`. The searching is `search::ContiguousSegmentSearcher`
//! and `search::SegmentMatcher`, which `search_serial_parity` holds to
//! music21; the runs answered are the stream's own notes, found by the
//! searcher's places.
//!
//! `serial` is swapped as well as `search.serial`: a run says what it was
//! read as with a `ToneRow`, and the transformations between two rows are
//! the crate's.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    search_serial_doctests_against_the_crate,
    "music21.search.serial",
    "searchserial",
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
        ("music21.serial", doctest::SERIAL_NAMES),
        ("music21.search.serial", doctest::SEARCH_SERIAL_NAMES),
    ]
);
