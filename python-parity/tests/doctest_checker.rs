//! music21's `figuredBass/checker.py` doctests against the crate. See
//! `src/doctest.rs`. The checker stays music21's and checks voice leading
//! through the crate's pitches, notes, voice-leading quartets and
//! possibility rules.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    checker_doctests_against_the_crate,
    "music21.figuredBass.checker",
    "checker",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.note", doctest::NOTE_NAMES),
        ("music21.beam", doctest::BEAM_NAMES),
        ("music21.tie", doctest::TIE_NAMES),
        ("music21.duration", doctest::DURATION_NAMES),
        ("music21.chord", doctest::CHORD_NAMES),
        ("music21.key", doctest::KEY_NAMES),
        ("music21.voiceLeading", doctest::VOICE_LEADING_NAMES),
        (
            "music21.figuredBass.possibility",
            doctest::POSSIBILITY_NAMES
        ),
    ]
);
