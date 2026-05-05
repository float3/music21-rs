//! Rust helpers inspired by selected parts of Python's `music21`.
//!
//! The crate currently focuses on pitch construction, chord naming and
//! lightweight theory utilities such as polyrhythm and tuning-system helpers.
// #![feature(inline_const_pat)]
// #![feature(negative_impls)]
// #![feature(specialization)]
// #![feature(lazy_get)]
/// ABC notation export helpers.
pub mod abc;
/// Key-finding and compact analysis helpers.
pub mod analysis;
/// Chord construction, common-name analysis and chord input conversion traits.
pub mod chord;
/// Lead-sheet chord-symbol parsing.
pub mod chordsymbol;
pub(crate) mod common;
pub(crate) mod defaults;
pub(crate) mod display;
/// Rhythmic duration primitives.
pub mod duration;
/// Error and result types used by the crate.
pub mod error;

pub(crate) mod fraction_pow;
/// Public interval parsing, naming and transposition helpers.
pub mod interval;
/// Public key and key-signature helpers.
pub mod key;
/// Minimal MIDI import/export helpers.
pub mod midi;
/// Note construction and pitch access helpers.
pub mod note;
/// Pitch construction, spelling and pitch-space helpers.
pub mod pitch;
/// Polyrhythm timing and pitch-set helpers.
pub mod polyrhythm;
/// Silent duration-bearing musical event.
pub mod rest;
/// Roman numeral parsing and compact harmonic analysis.
pub mod roman;
/// Public scale helpers.
pub mod scale;
pub(crate) mod stepname;
/// Small ordered timeline container.
pub mod stream;
/// Tuning-system ratios, labels and frequency helpers.
pub mod tuningsystem;
// #[macro_use]
// pub(crate) mod macros;

pub use abc::{
    abc_chord, abc_duration, abc_note, abc_rest, pitch_name_from_abc_note,
    pitch_names_from_abc_chord,
};
pub use analysis::{KeyEstimate, estimate_key_from_chords, estimate_key_from_pitches};
pub use chord::{
    Chord, ChordResolutionSuggestion, GuitarFingering, GuitarStringFingering, GuitarTuning,
    GuitarTuningString, IntoNotes, KnownChordType,
};
pub use chordsymbol::{ChordAlteration, ChordQuality, ChordSymbol};
pub use defaults::{FloatType, FractionType, IntegerType, Octave, UnsignedIntegerType};
pub use duration::Duration;
pub use error::{Error, Result};
pub use interval::{Interval, IntervalDirection};
pub use key::{Key, KeySignature};
pub use midi::{
    DEFAULT_TICKS_PER_QUARTER, MidiNote, midi_notes_from_stream, read_midi_bytes,
    read_midi_bytes_with_tempo, stream_from_midi_notes, write_midi_bytes,
};
pub use note::{IntoNote, Note};
pub use pitch::{
    Accidental, AccidentalSpecifier, CHROMATIC_PITCH_CLASS_NAMES, Microtone, MicrotoneSpecifier,
    Pitch, PitchClass, PitchClassSpecifier, PitchName, PitchOptions, pitch_class_name,
};
pub use polyrhythm::{Polyrhythm, PolyrhythmAnalysis, PolyrhythmEvent, PolyrhythmRatioTone};
pub use rest::Rest;
pub use roman::{RomanNumeral, analyze_chord, analyze_chord_with_root};
pub use scale::DiatonicScale;
pub use stream::{Stream, StreamElement, StreamEvent};
pub use tuningsystem::{
    ALL_TUNING_SYSTEMS, COMMON_TWELVE_TONE_TUNING_SYSTEMS, Fraction, TuningSystem,
};
