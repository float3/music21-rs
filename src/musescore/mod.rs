//! MuseScore's own files, read natively, and MuseScore run as a converter.
//!
//! [`from_mscx`](crate::musescore::from_mscx) reads the uncompressed `.mscx` format MuseScore 4 saves --
//! and MuseScore 3's, where the two are the same -- into a score, with no
//! program installed. It opens no file and unpacks nothing: a `.mscz` is a
//! zip archive holding the `.mscx`, and it is the caller's to unpack and
//! hand over as text.
//!
//! With the `musescore` feature, `MuseScore` finds an installed MuseScore
//! and runs it, which reads and writes every format the program does --
//! its own, Guitar Pro, Capella, MIDI, PDF and the rest -- by handing the
//! score across as MusicXML. That half runs a program and touches files,
//! and is left out of a wasm build.

mod beams;
mod read;

pub use read::from_mscx;

#[cfg(all(feature = "musescore", not(target_arch = "wasm32")))]
mod program;

#[cfg(all(feature = "musescore", not(target_arch = "wasm32")))]
pub use program::{MUSESCORE_PATH, MuseScore};
