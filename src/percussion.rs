//! Notes with no pitch: music21's `note.Unpitched` and
//! `percussion.PercussionChord`.
//!
//! A drum stroke is written on a staff line without sounding that line's
//! pitch. What it keeps is where it is *displayed* -- a step and an octave,
//! read as if the staff were in treble clef -- and everything a written note
//! has besides a pitch: a length, a stem, a notehead, beams.

use crate::chord::Chord;
use crate::defaults::IntegerType;
use crate::duration::Duration;
use crate::error::Result;
use crate::instrument::Instrument;
use crate::note::Note;
use crate::pitch::Pitch;

/// A note with no pitch, as a drum stroke: music21's `note.Unpitched`.
///
/// ```
/// use music21_rs::percussion::Unpitched;
///
/// let stroke = Unpitched::new();
/// assert_eq!((stroke.display_step(), stroke.display_octave()), ('B', 4));
///
/// let low = Unpitched::from_display_name("D5")?;
/// assert_eq!(low.display_name(), "D5");
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Unpitched {
    /// A note standing where this one is displayed, carrying everything
    /// written about it.
    written: Note,
}

impl Default for Unpitched {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for Unpitched {
    /// Where they are displayed, what plays them and how long they last,
    /// as music21 compares them.
    fn eq(&self, other: &Self) -> bool {
        self.display_name() == other.display_name()
            && self.stored_instrument() == other.stored_instrument()
            && self.duration() == other.duration()
    }
}

impl Unpitched {
    /// A stroke displayed on the middle line, `B4`, which is where music21
    /// puts one that says nothing.
    pub fn new() -> Self {
        Self::from_display_name("B4").expect("B4 is a pitch name")
    }

    /// A stroke displayed where this pitch would stand in treble clef. An
    /// accidental in the name is dropped, since a display position has
    /// none, and a name with no octave is displayed in the fourth.
    pub fn from_display_name(name: &str) -> Result<Self> {
        let named = Pitch::from_name(name)?;
        Self::displayed_at(named.step().as_char(), named.implicit_octave())
    }

    /// A stroke displayed at this step and octave.
    pub fn displayed_at(step: char, octave: IntegerType) -> Result<Self> {
        Ok(Self {
            written: Note::from_name(format!("{step}{octave}"))?,
        })
    }

    /// The letter of the line or space it is displayed on: music21's
    /// `displayStep`.
    pub fn display_step(&self) -> char {
        self.written.pitch().step().as_char()
    }

    /// The octave it is displayed in: music21's `displayOctave`.
    pub fn display_octave(&self) -> IntegerType {
        self.written.pitch().implicit_octave()
    }

    /// Where it is displayed, as a pitch, which never has an accidental:
    /// music21's `displayPitch`.
    pub fn display_pitch(&self) -> &Pitch {
        self.written.pitch()
    }

    /// Where it is displayed, as a name: music21's `displayName`.
    pub fn display_name(&self) -> String {
        self.written.pitch().name_with_octave()
    }

    /// Moves it to another line or space.
    pub fn set_display(&mut self, step: char, octave: IntegerType) -> Result<()> {
        let pitch = Pitch::from_name(format!("{step}{octave}"))?;
        self.written.set_pitch(pitch);
        Ok(())
    }

    /// How long it lasts, where that has been said.
    pub fn duration(&self) -> Option<&Duration> {
        self.written.duration()
    }

    /// The same stroke lasting this long.
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.written.set_duration(duration);
        self
    }

    /// The instrument that plays it, where one is kept on the stroke itself:
    /// music21's `storedInstrument`.
    pub fn stored_instrument(&self) -> Option<&Instrument> {
        self.written.stored_instrument()
    }

    /// Everything written about the stroke -- its length, stem, notehead,
    /// beams, tie, lyrics, expressions -- as a note standing where it is
    /// displayed. The note's pitch is that position and nothing sounds it.
    pub fn written(&self) -> &Note {
        &self.written
    }

    /// What is written about the stroke, for editing. Changing the pitch
    /// moves where it is displayed.
    pub fn written_mut(&mut self) -> &mut Note {
        &mut self.written
    }
}

/// One member of a percussion chord: a stroke, or a note with a pitch.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PercussionNote {
    /// A stroke with no pitch.
    Unpitched(Unpitched),
    /// A note with a pitch, as a timpani's or a glockenspiel's.
    Note(Note),
}

impl From<Unpitched> for PercussionNote {
    fn from(value: Unpitched) -> Self {
        Self::Unpitched(value)
    }
}

impl From<Note> for PercussionNote {
    fn from(value: Note) -> Self {
        Self::Note(value)
    }
}

/// Several strokes at once, as a hi-hat over a bass drum: music21's
/// `percussion.PercussionChord`.
///
/// It is not a harmony and answers none of a chord's harmonic questions;
/// its members stay in the order given.
///
/// ```
/// use music21_rs::percussion::{PercussionChord, PercussionNote, Unpitched};
/// use music21_rs::Note;
///
/// let chord = PercussionChord::new(vec![
///     Unpitched::from_display_name("E3")?.into(),
///     Note::from_name("G4")?.into(),
/// ])?;
/// assert_eq!(chord.len(), 2);
/// assert!(chord.is_unpitched(0) && !chord.is_unpitched(1));
/// assert_eq!(chord.pitches().len(), 1);
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct PercussionChord {
    /// The members as written notes, a stroke standing where it is
    /// displayed, with what is written of the chord as a whole.
    written: Chord,
    /// Which members have no pitch.
    unpitched: Vec<bool>,
}

impl PercussionChord {
    /// A chord of these members, in this order.
    pub fn new(members: Vec<PercussionNote>) -> Result<Self> {
        let unpitched = members
            .iter()
            .map(|member| matches!(member, PercussionNote::Unpitched(_)))
            .collect();
        let notes: Vec<Note> = members
            .into_iter()
            .map(|member| match member {
                PercussionNote::Unpitched(stroke) => stroke.written,
                PercussionNote::Note(note) => note,
            })
            .collect();
        Ok(Self {
            written: Chord::new(notes)?,
            unpitched,
        })
    }

    /// How many members it has.
    pub fn len(&self) -> usize {
        self.unpitched.len()
    }

    /// Whether it has no members.
    pub fn is_empty(&self) -> bool {
        self.unpitched.is_empty()
    }

    /// Whether the member at this place has no pitch.
    pub fn is_unpitched(&self, index: usize) -> bool {
        self.unpitched.get(index).copied().unwrap_or(false)
    }

    /// The members, in order.
    pub fn members(&self) -> Vec<PercussionNote> {
        self.written
            .notes()
            .iter()
            .zip(&self.unpitched)
            .map(|(note, unpitched)| {
                if *unpitched {
                    PercussionNote::Unpitched(Unpitched {
                        written: note.clone(),
                    })
                } else {
                    PercussionNote::Note(note.clone())
                }
            })
            .collect()
    }

    /// The pitches of the members that have one: music21's `pitches`.
    pub fn pitches(&self) -> Vec<Pitch> {
        self.written
            .notes()
            .iter()
            .zip(&self.unpitched)
            .filter(|(_, unpitched)| !**unpitched)
            .map(|(note, _)| note.pitch().clone())
            .collect()
    }

    /// How long it lasts, where that has been said.
    pub fn duration(&self) -> Option<&Duration> {
        self.written.duration()
    }

    /// The same chord lasting this long.
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.written.set_duration(duration);
        self
    }

    /// Everything written about the chord as a whole -- its length, stem,
    /// beams, expressions -- and about each member, as a chord of notes
    /// standing where the members are displayed. It is a carrier of
    /// notation: its harmony means nothing.
    pub fn written(&self) -> &Chord {
        &self.written
    }

    /// What is written about the chord, for editing. Adding or removing
    /// notes here is not supported: build a new chord instead.
    pub fn written_mut(&mut self) -> &mut Chord {
        &mut self.written
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stroke_is_displayed_without_an_accidental() {
        // music21: Unpitched(displayName='D#5') keeps the step and octave.
        let stroke = Unpitched::from_display_name("D#5").unwrap();
        assert_eq!(stroke.display_name(), "D5");
        assert_eq!(Unpitched::new().display_name(), "B4");
    }

    #[test]
    fn strokes_compare_by_where_they_are_displayed() {
        let one = Unpitched::from_display_name("B4").unwrap();
        assert_eq!(one, Unpitched::new());
        assert_ne!(one, Unpitched::from_display_name("A4").unwrap());
    }

    #[test]
    fn a_percussion_chord_keeps_its_order() {
        let chord = PercussionChord::new(vec![
            Unpitched::from_display_name("B3").unwrap().into(),
            Unpitched::from_display_name("E3").unwrap().into(),
        ])
        .unwrap();
        let names: Vec<String> = chord
            .written()
            .notes()
            .iter()
            .map(|note| note.pitch().name_with_octave())
            .collect();
        assert_eq!(names, ["B3", "E3"]);
        assert!(chord.pitches().is_empty());
    }
}
