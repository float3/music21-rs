//! music21's `alpha/analysis/hasher.py` doctests against the crate. See
//! `src/doctest.rs`. The hashing is
//! `alpha::analysis::hasher::Hasher::hash_stream`, which `alpha_parity`
//! holds to music21 note for note.
//!
//! `NoteHash` and `NoteHashWithReference` stay music21's: one is a tuple
//! and the other a wrapper round one, holding nothing of the crate, and
//! its own examples build the tuples by hand and hand them over.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    hasher_doctests_against_the_crate,
    "music21.alpha.analysis.hasher",
    "hasher",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.note", doctest::NOTE_NAMES),
        ("music21.duration", doctest::DURATION_NAMES),
        ("music21.chord", doctest::CHORD_NAMES),
        ("music21.tie", doctest::TIE_NAMES),
        ("music21.volume", doctest::VOLUME_NAMES),
        ("music21.beam", doctest::BEAM_NAMES),
        ("music21.key", doctest::KEY_NAMES),
        ("music21.meter.base", doctest::METER_NAMES),
        ("music21.meter.core", doctest::METER_CORE_NAMES),
        ("music21.alpha.analysis.hasher", doctest::HASHER_NAMES),
    ]
);
