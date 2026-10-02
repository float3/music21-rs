//! The formats the editor reads through the crate alone -- MIDI, Humdrum,
//! MEI, RomanText and TinyNotation -- each read by music21's reader, ported,
//! and turned into what the editor writes ABC from by [`super::stream`].
//! None of them carries anything the editor wants beside the score.

use super::{Imported, stream::Extras};
use music21_rs::{
    Stream, humdrum::from_humdrum, makenotation::make_ties, mei::from_mei, midi::from_midi,
    romantext::from_roman_text, tinynotation::from_tiny_notation,
};

fn imported(
    read: music21_rs::Result<Stream>,
    format: &str,
    extras: &Extras,
) -> Result<Imported, String> {
    let score = read.map_err(|err| format!("the {format} does not read: {err}"))?;
    super::stream::read(&score, extras)
}

/// A standard MIDI file: a part for every track with notes, quantized to
/// sixteenths and triplet eighths, the drum channel left out.
pub(super) fn midi(bytes: &[u8]) -> Result<Imported, String> {
    imported(from_midi(bytes), "MIDI file", &Extras::default())
}

/// A Humdrum file, a part for each `**kern` spine.
pub(super) fn humdrum(text: &str) -> Result<Imported, String> {
    imported(from_humdrum(text), "Humdrum file", &Extras::default())
}

/// An MEI document, a part for each staff.
pub(super) fn mei(text: &str) -> Result<Imported, String> {
    imported(from_mei(text), "MEI document", &Extras::default())
}

/// A RomanText analysis: the chords its numerals stand for, each numeral
/// written under its chord.
pub(super) fn roman_text(text: &str) -> Result<Imported, String> {
    let extras = Extras {
        figures: true,
        ..Extras::default()
    };
    imported(from_roman_text(text), "RomanText", &extras)
}

/// A line of TinyNotation. The reader leaves a note running past its
/// barline whole, as music21 does; the editor writes whole bars, so it is
/// cut and tied there.
pub(super) fn tiny_notation(text: &str) -> Result<Imported, String> {
    let tied = from_tiny_notation(text).and_then(|mut part| {
        make_ties(&mut part)?;
        Ok(part)
    });
    imported(tied, "TinyNotation", &Extras::default())
}
