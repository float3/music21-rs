use crate::defaults::{FloatType, IntegerType};
use crate::duration::Duration;
use crate::error::Result;
use crate::interval::Interval;
use crate::notation::{Beams, Lyric, Notehead, StemDirection, Syllabic, Tie};
use crate::pitch::Pitch;
use crate::volume::Volume;

use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A pitched note.
pub struct Note {
    pub(crate) pitch: Pitch,
    duration: Option<Duration>,
    #[cfg_attr(feature = "serde", serde(default))]
    notation: Notation,
}

/// The notation a note carries besides its pitch and duration: how it is
/// tied, drawn, stemmed, coloured, sung and sounded.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct Notation {
    tie: Option<Tie>,
    notehead: Notehead,
    notehead_fill: Option<bool>,
    notehead_parenthesis: bool,
    stem_direction: StemDirection,
    color: Option<String>,
    volume: Option<Volume>,
    lyrics: Vec<Lyric>,
    beams: Beams,
}

impl Note {
    /// Builds a note from a pitch name such as `"C#4"` or `"E-"`.
    pub fn from_name(name: impl Into<String>) -> Result<Self> {
        Pitch::from_name(name).map(Self::from_pitch)
    }

    /// Builds a note from a pitch-space number, where 60 is middle C.
    pub fn from_number(number: FloatType) -> Result<Self> {
        Pitch::from_number(number).map(Self::from_pitch)
    }

    /// Builds a note from an existing [`Pitch`].
    pub fn from_pitch(pitch: Pitch) -> Self {
        Self {
            pitch,
            duration: None,
            notation: Notation::default(),
        }
    }

    /// Returns the note's pitch.
    pub fn pitch(&self) -> &Pitch {
        &self.pitch
    }

    /// Sets the note's pitch, keeping its duration and notation: music21's
    /// `Note.pitch` setter.
    pub fn set_pitch(&mut self, pitch: Pitch) {
        self.pitch = pitch;
    }

    /// Returns the pitch name without an octave, such as `"C#"` or `"E-"`.
    pub fn pitch_name(&self) -> String {
        self.pitch.name()
    }

    /// Returns the pitch name with an octave when one is set.
    pub fn pitch_name_with_octave(&self) -> String {
        self.pitch.name_with_octave()
    }

    /// The pitch's step letter.
    pub fn step(&self) -> char {
        self.pitch.step().as_char()
    }

    /// The pitch's octave, if it has one.
    pub fn octave(&self) -> crate::defaults::Octave {
        self.pitch.octave()
    }

    /// The note's one pitch as a list, the shape a chord's `pitches` has.
    pub fn pitches(&self) -> Vec<Pitch> {
        vec![self.pitch.clone()]
    }

    /// music21's `fullName`: `E-flat in octave 4 Quarter Note`, with the
    /// duration's name left out when the note has none.
    pub fn full_name(&self) -> String {
        match self.duration.as_ref() {
            Some(duration) => format!("{} {} Note", self.pitch.full_name(), duration.full_name()),
            None => format!("{} Note", self.pitch.full_name()),
        }
    }

    /// Returns the note duration when one has been assigned.
    pub fn duration(&self) -> Option<&Duration> {
        self.duration.as_ref()
    }

    /// Assigns a duration to the note.
    pub fn set_duration(&mut self, duration: Duration) {
        self.duration = Some(duration);
    }

    /// Returns a copy of this note with the supplied duration.
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.set_duration(duration);
        self
    }

    /// Returns this note transposed by the interval, keeping everything but
    /// its pitch.
    pub fn transpose(&self, interval: &Interval) -> Result<Self> {
        Ok(Self {
            pitch: interval.transpose_pitch(&self.pitch)?,
            duration: self.duration.clone(),
            notation: self.notation.clone(),
        })
    }

    // ---- notation --------------------------------------------------------

    /// The beams joining this note's flags to its neighbours': music21's
    /// `beams`.
    pub fn beams(&self) -> &Beams {
        &self.notation.beams
    }

    /// The same, to be changed.
    pub fn beams_mut(&mut self) -> &mut Beams {
        &mut self.notation.beams
    }

    /// Replaces the beams.
    pub fn set_beams(&mut self, beams: Beams) {
        self.notation.beams = beams;
    }

    /// The tie joining this note to its neighbours, if any.
    pub fn tie(&self) -> Option<&Tie> {
        self.notation.tie.as_ref()
    }

    /// Ties this note, or unties it with `None`.
    pub fn set_tie(&mut self, tie: Option<Tie>) {
        self.notation.tie = tie;
    }

    /// The shape the note head is drawn with.
    pub fn notehead(&self) -> Notehead {
        self.notation.notehead
    }

    /// Sets the shape the note head is drawn with.
    pub fn set_notehead(&mut self, notehead: Notehead) {
        self.notation.notehead = notehead;
    }

    /// Whether the note head is filled in: `Some(true)` filled,
    /// `Some(false)` hollow, `None` to let the duration decide, which is
    /// music21's default.
    pub fn notehead_fill(&self) -> Option<bool> {
        self.notation.notehead_fill
    }

    /// Sets whether the note head is filled in.
    pub fn set_notehead_fill(&mut self, fill: Option<bool>) {
        self.notation.notehead_fill = fill;
    }

    /// Whether the note head is written in parentheses.
    pub fn notehead_parenthesis(&self) -> bool {
        self.notation.notehead_parenthesis
    }

    /// Sets whether the note head is written in parentheses.
    pub fn set_notehead_parenthesis(&mut self, parenthesis: bool) {
        self.notation.notehead_parenthesis = parenthesis;
    }

    /// Which way the stem points.
    pub fn stem_direction(&self) -> StemDirection {
        self.notation.stem_direction
    }

    /// Sets which way the stem points.
    pub fn set_stem_direction(&mut self, direction: StemDirection) {
        self.notation.stem_direction = direction;
    }

    /// The colour the note is written in, if one was said: music21 keeps
    /// this on the note's style.
    pub fn color(&self) -> Option<&str> {
        self.notation.color.as_deref()
    }

    /// Sets the colour the note is written in.
    pub fn set_color(&mut self, color: Option<String>) {
        self.notation.color = color;
    }

    /// How loud the note is. music21 makes a volume on first access, so this
    /// answers a default one for a note nobody has marked; see
    /// [`Self::has_volume_information`] to tell the two apart.
    pub fn volume(&self) -> Volume {
        self.notation.volume.clone().unwrap_or_default()
    }

    /// Sets how loud the note is, or clears it.
    pub fn set_volume(&mut self, volume: Option<Volume>) {
        self.notation.volume = volume;
    }

    /// Whether a volume was ever set on this note: music21's
    /// `hasVolumeInformation`, which asks only whether the object is there
    /// and not whether a velocity was written on it, so a bare
    /// `Volume::new()` set on a note counts.
    pub fn has_volume_information(&self) -> bool {
        self.notation.volume.is_some()
    }

    /// The syllables sung on this note, one per verse.
    pub fn lyrics(&self) -> &[Lyric] {
        &self.notation.lyrics
    }

    /// The syllables sung on this note, for editing in place.
    pub fn lyrics_mut(&mut self) -> &mut Vec<Lyric> {
        &mut self.notation.lyrics
    }

    /// The text of the first verse, with the hyphens of every further verse
    /// joined by newlines: music21's `lyric`.
    pub fn lyric(&self) -> Option<String> {
        if self.notation.lyrics.is_empty() {
            return None;
        }
        Some(
            self.notation
                .lyrics
                .iter()
                .map(Lyric::text)
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }

    /// Replaces every lyric with one verse of text, or clears them with
    /// `None`. Newlines start further verses, and hyphens say where each
    /// syllable falls in its word: music21's `lyric` setter.
    pub fn set_lyric(&mut self, lyric: Option<&str>) -> Result<()> {
        self.notation.lyrics.clear();
        let Some(lyric) = lyric else {
            return Ok(());
        };
        for (index, line) in lyric.split('\n').enumerate() {
            let mut parsed = Lyric::from_raw_text(line);
            parsed.set_number(index as IntegerType + 1)?;
            self.notation.lyrics.push(parsed);
        }
        Ok(())
    }

    /// Adds a syllable as the next verse: music21's `addLyric`. Hyphens in
    /// the text say where the syllable falls in its word unless `apply_raw`
    /// is set, which takes the text as written.
    /// Adds a syllable as a verse: music21's `addLyric`.
    ///
    /// With no `number` it becomes the next verse. With one, it *replaces*
    /// the text of the verse already carrying that number — leaving where
    /// that syllable falls in its word alone, as music21's plain text
    /// assignment does — and only becomes a new verse when no verse has it.
    pub fn add_lyric(
        &mut self,
        text: &str,
        number: Option<IntegerType>,
        apply_raw: bool,
    ) -> Result<()> {
        let Some(number) = number else {
            let mut lyric = Self::build_lyric(text, apply_raw);
            lyric.set_number(self.notation.lyrics.len() as IntegerType + 1)?;
            self.notation.lyrics.push(lyric);
            return Ok(());
        };
        if let Some(existing) = self
            .notation
            .lyrics
            .iter_mut()
            .find(|lyric| lyric.number() == number)
        {
            existing.set_text(text);
            return Ok(());
        }
        let mut lyric = Self::build_lyric(text, apply_raw);
        lyric.set_number(number)?;
        self.notation.lyrics.push(lyric);
        Ok(())
    }

    /// Puts a syllable in front of the verse at `index`, moving the verses
    /// from there on down a line: music21's `insertLyric`.
    ///
    /// An index past the end appends, as inserting into a list does.
    pub fn insert_lyric(&mut self, text: &str, index: usize, apply_raw: bool) -> Result<()> {
        let index = index.min(self.notation.lyrics.len());
        for (offset, lyric) in self.notation.lyrics[index..].iter_mut().enumerate() {
            lyric.set_number(index as IntegerType + offset as IntegerType + 2)?;
        }
        let mut lyric = Self::build_lyric(text, apply_raw);
        lyric.set_number(index as IntegerType + 1)?;
        self.notation.lyrics.insert(index, lyric);
        Ok(())
    }

    /// A syllable read from text, whose hyphens say where it falls in its
    /// word unless `apply_raw` takes the text as written.
    fn build_lyric(text: &str, apply_raw: bool) -> Lyric {
        if apply_raw {
            let mut lyric = Lyric::new(text);
            lyric.set_syllabic(Syllabic::Single);
            lyric
        } else {
            Lyric::from_raw_text(text)
        }
    }
}

impl FromStr for Note {
    type Err = crate::error::Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::from_name(value)
    }
}

impl TryFrom<&str> for Note {
    type Error = crate::error::Error;

    fn try_from(value: &str) -> Result<Self> {
        Self::from_name(value)
    }
}

impl TryFrom<String> for Note {
    type Error = crate::error::Error;

    fn try_from(value: String) -> Result<Self> {
        Self::from_name(value)
    }
}

impl From<Pitch> for Note {
    fn from(value: Pitch) -> Self {
        Self::from_pitch(value)
    }
}

impl From<&Pitch> for Note {
    fn from(value: &Pitch) -> Self {
        Self::from_pitch(value.clone())
    }
}

impl TryFrom<IntegerType> for Note {
    type Error = crate::error::Error;

    fn try_from(value: IntegerType) -> Result<Self> {
        Self::from_number(value as FloatType)
    }
}

impl Display for Note {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.pitch_name_with_octave())
    }
}

/// Converts a single note-like value into a [`Note`].
///
/// This is useful when constructing vectors or other collections that are
/// later passed to APIs such as `Chord::new`.
pub trait IntoNote {
    /// Whether this value came from an integer pitch class or MIDI-like number.
    const FROM_INTEGER_PITCH: bool = false;

    /// Converts the value into a note.
    fn try_into_note(self) -> Result<Note>;
}

impl IntoNote for Note {
    fn try_into_note(self) -> Result<Note> {
        Ok(self)
    }
}

impl IntoNote for &Note {
    fn try_into_note(self) -> Result<Note> {
        Ok(self.clone())
    }
}

impl IntoNote for Pitch {
    fn try_into_note(self) -> Result<Note> {
        Ok(Note::from_pitch(self))
    }
}

impl IntoNote for &Pitch {
    fn try_into_note(self) -> Result<Note> {
        Ok(Note::from_pitch(self.clone()))
    }
}

impl IntoNote for String {
    fn try_into_note(self) -> Result<Note> {
        Note::from_name(self)
    }
}

impl IntoNote for &String {
    fn try_into_note(self) -> Result<Note> {
        Note::from_name(self.as_str())
    }
}

impl IntoNote for &str {
    fn try_into_note(self) -> Result<Note> {
        Note::from_name(self)
    }
}

impl IntoNote for IntegerType {
    const FROM_INTEGER_PITCH: bool = true;

    fn try_into_note(self) -> Result<Note> {
        Note::from_number(self as FloatType)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn full_name_step_and_octave_match_music21() {
        let flat = Note::from_name("E-4").unwrap();
        assert_eq!(flat.full_name(), "E-flat in octave 4 Note");
        assert_eq!(flat.step(), 'E');
        assert_eq!(flat.octave(), Some(4));
        assert_eq!(flat.pitches()[0].name_with_octave(), "E-4");
        let dotted = Note::from_name("C#5")
            .unwrap()
            .with_duration(crate::Duration::new(1.5).unwrap());
        assert_eq!(
            dotted.full_name(),
            "C-sharp in octave 5 Dotted Quarter Note"
        );
        let bare = Note::from_name("G").unwrap();
        assert_eq!(bare.octave(), None);
        assert_eq!(bare.full_name(), "G Note");
    }
    use super::{IntoNote, Note};
    use crate::defaults::IntegerType;
    use crate::pitch::Pitch;

    #[test]
    fn into_note_accepts_note_like_inputs() {
        fn from_integer_pitch<T: IntoNote>() -> bool {
            T::FROM_INTEGER_PITCH
        }

        assert!(!from_integer_pitch::<&str>());
        assert!(from_integer_pitch::<IntegerType>());

        let note = Note::from_name("C4").unwrap();
        assert_eq!(
            note.clone()
                .try_into_note()
                .unwrap()
                .pitch_name_with_octave(),
            "C4"
        );

        let borrowed_note = Note::from_name("D4").unwrap();
        assert_eq!(
            (&borrowed_note)
                .try_into_note()
                .unwrap()
                .pitch_name_with_octave(),
            "D4"
        );

        let pitch = Pitch::from_name("E4").unwrap();
        assert_eq!(
            pitch.try_into_note().unwrap().pitch_name_with_octave(),
            "E4"
        );

        let borrowed_pitch = Pitch::from_name("F4").unwrap();
        assert_eq!(
            (&borrowed_pitch)
                .try_into_note()
                .unwrap()
                .pitch_name_with_octave(),
            "F4"
        );

        assert_eq!(
            "G4".to_string()
                .try_into_note()
                .unwrap()
                .pitch_name_with_octave(),
            "G4"
        );

        let owned_name = "A4".to_string();
        assert_eq!(
            (&owned_name)
                .try_into_note()
                .unwrap()
                .pitch_name_with_octave(),
            "A4"
        );

        assert_eq!("B4".try_into_note().unwrap().pitch_name_with_octave(), "B4");

        assert_eq!(
            (60 as IntegerType)
                .try_into_note()
                .unwrap()
                .pitch_name_with_octave(),
            "C4"
        );
    }

    #[test]
    fn transposing_a_note_keeps_its_duration() {
        let note = Note::from_name("C4")
            .unwrap()
            .with_duration(crate::Duration::half());
        let moved = note
            .transpose(&crate::Interval::from_name("M3").unwrap())
            .unwrap();
        assert_eq!(moved.pitch_name_with_octave(), "E4");
        assert_eq!(moved.duration().unwrap().quarter_length(), 2.0);
    }

    #[test]
    fn setting_a_pitch_keeps_the_duration_and_the_notation() {
        let mut note = Note::from_name("C4")
            .unwrap()
            .with_duration(crate::Duration::half());
        note.set_notehead(crate::Notehead::Diamond);
        note.set_pitch(crate::Pitch::from_name("E-5").unwrap());
        assert_eq!(note.pitch_name_with_octave(), "E-5");
        assert_eq!(note.duration().unwrap().quarter_length(), 2.0);
        assert_eq!(note.notehead(), crate::Notehead::Diamond);
    }

    #[test]
    fn a_numbered_lyric_replaces_the_verse_that_has_that_number() {
        let mut note = Note::from_name("C4").unwrap();
        note.add_lyric("hello", None, false).unwrap();
        note.add_lyric("bye", Some(3), false).unwrap();
        assert_eq!(
            note.lyrics()
                .iter()
                .map(|lyric| (lyric.number(), lyric.text()))
                .collect::<Vec<_>>(),
            [(1, "hello".to_string()), (3, "bye".to_string())]
        );

        // the same number again replaces the text, and leaves where the
        // syllable falls in its word alone
        note.add_lyric("ciao", Some(3), false).unwrap();
        assert_eq!(note.lyrics().len(), 2);
        assert_eq!(note.lyrics()[1].text(), "ciao");
        assert_eq!(note.lyrics()[1].number(), 3);

        // and `lyric` reads the syllables, not their hyphenated spellings
        let mut hyphenated = Note::from_name("C4").unwrap();
        hyphenated.set_lyric(Some("hel-")).unwrap();
        assert_eq!(hyphenated.lyrics()[0].raw_text(), "hel-");
        assert_eq!(hyphenated.lyric().as_deref(), Some("hel"));
    }

    #[test]
    fn inserting_a_lyric_moves_the_verses_after_it_down() {
        let mut note = Note::from_name("C4").unwrap();
        note.add_lyric("second", None, false).unwrap();
        note.insert_lyric("first", 0, false).unwrap();
        note.insert_lyric("newSecond", 1, false).unwrap();
        assert_eq!(
            note.lyrics()
                .iter()
                .map(|lyric| (lyric.number(), lyric.text()))
                .collect::<Vec<_>>(),
            [
                (1, "first".to_string()),
                (2, "newSecond".to_string()),
                (3, "second".to_string())
            ]
        );
        // an index past the end appends, as inserting into a list does
        note.insert_lyric("last", 99, false).unwrap();
        assert_eq!(note.lyrics()[3].number(), 4);
        assert_eq!(note.lyrics()[3].text(), "last");
    }

    #[test]
    fn note_supports_rust_conversion_traits() {
        let parsed: Note = "C#4".parse().unwrap();
        assert_eq!(parsed.to_string(), "C#4");

        let from_pitch = Note::from(Pitch::from_name("D4").unwrap());
        assert_eq!(from_pitch.pitch_name_with_octave(), "D4");

        let from_integer = Note::try_from(60 as IntegerType).unwrap();
        assert_eq!(from_integer.pitch_name_with_octave(), "C4");
    }
}
