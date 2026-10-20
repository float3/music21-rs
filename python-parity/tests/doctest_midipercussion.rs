//! music21's `midi/percussion.py` doctests against the crate. See
//! `src/doctest.rs`. The map is `instrument::for_percussion_key`, which
//! names the drum a key plays *and how it is played* -- key 36 is a bass
//! drum played `1` and key 35 the same drum played `acoustic` -- and
//! `Instrument::percussion_pitch` is the key a drum is struck on.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    midi_percussion_doctests_against_the_crate,
    "music21.midi.percussion",
    "midipercussion",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.instrument", doctest::INSTRUMENT_NAMES),
        ("music21.midi.percussion", doctest::MIDI_PERCUSSION_NAMES),
    ]
);
