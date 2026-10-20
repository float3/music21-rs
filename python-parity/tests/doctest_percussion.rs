//! music21's `percussion.py` doctests against the crate. See
//! `src/doctest.rs`. A percussion chord keeps what is struck as the
//! objects they are -- a stroke or a note, music21's own or this
//! package's -- since a caller holding one is holding the chord's.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    percussion_doctests_against_the_crate,
    "music21.percussion",
    "percussion",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.duration", doctest::DURATION_NAMES),
        ("music21.note", doctest::NOTE_NAMES),
        ("music21.tie", doctest::TIE_NAMES),
        ("music21.volume", doctest::VOLUME_NAMES),
        ("music21.beam", doctest::BEAM_NAMES),
        ("music21.instrument", doctest::INSTRUMENT_NAMES),
        ("music21.percussion", doctest::PERCUSSION_NAMES),
    ]
);
