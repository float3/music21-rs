#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_assignments)]
#![allow(unused_results)]
#![allow(unreachable_code)]
#![allow(unused_macros)]
#![allow(private_bounds)]
//! Rust helpers inspired by selected parts of Python's `music21`.
//!
//! The crate currently focuses on pitch construction, chord naming and
//! lightweight theory utilities such as polyrhythm and tuning-system helpers.
// #![feature(inline_const_pat)]
// #![feature(negative_impls)]
// #![feature(specialization)]
// #![feature(lazy_get)]
pub(crate) mod base;
/// Chord construction, common-name analysis and chord input conversion traits.
pub mod chord;
pub(crate) mod common;
pub(crate) mod defaults;
pub(crate) mod display;
pub(crate) mod duration;
/// Error and result types used by the crate.
pub mod exception;
pub(crate) mod fraction_pow;
pub(crate) mod interval;
pub(crate) mod key;
/// Note construction and pitch access helpers.
pub mod note;
/// Pitch construction, spelling and pitch-space helpers.
pub mod pitch;
/// Polyrhythm timing and pitch-set helpers.
pub mod polyrhythm;
pub(crate) mod prebase;
pub(crate) mod scale;
pub(crate) mod stepname;
/// Tuning-system ratios, labels and frequency helpers.
pub mod tuningsystem;
// #[macro_use]
// pub(crate) mod macros;

pub use chord::{Chord, IntoNote, IntoNotes, KnownChordType};
pub use exception::{Exception, ExceptionResult};
pub use note::Note;
pub use pitch::{
    Accidental, AccidentalSpecifier, Microtone, MicrotoneSpecifier, Pitch, PitchClass,
    PitchClassSpecifier, PitchName, PitchOptions,
};
pub use polyrhythm::{Polyrhythm, PolyrhythmEvent};
pub use tuningsystem::{COMMON_TWELVE_TONE_TUNING_SYSTEMS, Fraction, TuningSystem};
