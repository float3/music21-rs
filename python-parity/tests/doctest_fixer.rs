//! music21's `alpha/analysis/fixer.py` doctests against the crate. See
//! `src/doctest.rs`. All 91 examples are in one docstring,
//! `EnharmonicFixer`'s, and the respelling they drive is the crate's
//! `alpha::analysis::fixer::enharmonic_fix` -- music21's five cases for
//! one change.
//!
//! `DeleteFixer`, `OrnamentFixer`, `TrillFixer` and `TurnFixer` stay
//! music21's: each reads its changes as elements of two streams, where
//! the crate takes positions in two of its own, and none of them has an
//! example. `ChangeOps` stays music21's too, as it does in the aligner
//! suite.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    fixer_doctests_against_the_crate,
    "music21.alpha.analysis.fixer",
    "fixer",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.note", doctest::NOTE_NAMES),
        ("music21.duration", doctest::DURATION_NAMES),
        ("music21.chord", doctest::CHORD_NAMES),
        ("music21.tie", doctest::TIE_NAMES),
        ("music21.volume", doctest::VOLUME_NAMES),
        ("music21.beam", doctest::BEAM_NAMES),
        ("music21.expressions", doctest::EXPRESSION_NAMES),
        ("music21.alpha.analysis.hasher", doctest::HASHER_NAMES),
        ("music21.alpha.analysis.aligner", doctest::ALIGNER_NAMES),
        ("music21.alpha.analysis.fixer", doctest::FIXER_NAMES),
    ]
);
