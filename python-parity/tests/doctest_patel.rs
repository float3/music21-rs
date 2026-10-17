//! music21's `analysis/patel.py` doctests against the crate. See
//! `src/doctest.rs`. The streams are music21's own: the crate's
//! `analysis::patel` reads what the wheel hands it, so music21's examples
//! build their lines as they always did.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    patel_doctests_against_the_crate,
    "music21.analysis.patel",
    "patel",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.note", doctest::NOTE_NAMES),
        ("music21.duration", doctest::DURATION_NAMES),
        ("music21.chord", doctest::CHORD_NAMES),
        ("music21.tie", doctest::TIE_NAMES),
        ("music21.volume", doctest::VOLUME_NAMES),
        ("music21.beam", doctest::BEAM_NAMES),
        ("music21.analysis.patel", doctest::PATEL_NAMES),
    ]
);
