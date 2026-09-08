//! music21's `voiceLeading.py` doctests against the crate. See
//! `src/doctest.rs`.

use music21_rs_python_parity::{doctest, music21_rs_facade};

#[test]
fn voice_leading_doctests_against_the_crate() {
    pyo3::append_to_inittab!(music21_rs_facade);
    doctest::run(
        "music21.voiceLeading",
        "voiceleading",
        &[
            ("music21.pitch", doctest::PITCH_NAMES),
            ("music21.interval", doctest::INTERVAL_NAMES),
            ("music21.note", doctest::NOTE_NAMES),
            ("music21.beam", doctest::BEAM_NAMES),
            ("music21.tie", doctest::TIE_NAMES),
            ("music21.duration", doctest::DURATION_NAMES),
            ("music21.chord", doctest::CHORD_NAMES),
            ("music21.key", doctest::KEY_NAMES),
            ("music21.voiceLeading", doctest::VOICE_LEADING_NAMES),
        ],
    );
}
