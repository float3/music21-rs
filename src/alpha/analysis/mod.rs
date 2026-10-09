//! music21's experimental analyses: `alpha.analysis`.
//!
//! [`hasher`] hashes notes into tuples, and [`aligner`] aligns two streams
//! by them. [`ornament_recognizer`] recognizes the trill or turn a run of
//! notes plays, and [`search`] finds runs of notes through a scale.
//! [`fixer`] corrects a score read by optical music recognition by a
//! performance of it aligned to it.

pub mod aligner;
pub mod fixer;
pub mod hasher;
pub mod ornament_recognizer;
pub mod search;
