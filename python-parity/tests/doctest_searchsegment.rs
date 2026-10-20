//! music21's `search/segment.py` doctests against the crate. See
//! `src/doctest.rs`. The translating, the cutting and the comparing are
//! the crate's; music21's own file indexers reach them through
//! `indexScoreParts`.
//!
//! The docstrings read whole scores out of the corpus, so every class
//! those scores hold is swapped: the reading refuses a music21 object it
//! has no value for.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    search_segment_doctests_against_the_crate,
    "music21.search.segment",
    "searchsegment",
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
        ("music21.search.segment", doctest::SEARCH_SEGMENT_NAMES),
    ]
);
