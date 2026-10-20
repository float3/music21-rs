//! music21's `alpha/analysis/aligner.py` doctests against the crate. See
//! `src/doctest.rs`. Every step of the alignment is the crate's -- the
//! four costs over a pair of hashes, the matrix before and after it is
//! filled, the three moves onto a cell and the one taken, and the edit
//! read back out of the matrix -- and `alpha_parity` holds the whole to
//! music21.
//!
//! `ChangeOps` stays music21's: it is an `IntEnum` holding nothing of the
//! crate, and its own four examples pass on music21's own class. The
//! matrix stays numpy's, which is what music21's examples write into and
//! print.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    aligner_doctests_against_the_crate,
    "music21.alpha.analysis.aligner",
    "aligner",
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
        ("music21.alpha.analysis.hasher", doctest::HASHER_NAMES),
        ("music21.alpha.analysis.aligner", doctest::ALIGNER_NAMES),
    ]
);
