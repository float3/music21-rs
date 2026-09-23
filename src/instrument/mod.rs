//! Instruments: music21's `instrument` module.
//!
//! An [`Instrument`] is one of music21's instrument classes -- a piano, a
//! clarinet in B-flat, a triangle -- with what an instrument of that class
//! starts out as: its name and abbreviation, its General MIDI program and
//! sound, the range it plays, the interval from what is written for it to
//! what sounds, and for unpitched percussion the General MIDI drum it plays.
//! The class survives as the instrument's *kind*, and a kind belongs to the
//! families above it, so a clarinet is a woodwind and a triangle unpitched
//! percussion ([`Instrument::is_a`]).
//!
//! An instrument is found by kind ([`Instrument::of_kind`]), by MIDI program
//! ([`Instrument::from_midi_program`]) or by what a score calls it, in six
//! languages and their abbreviations ([`Instrument::from_name`], music21's
//! `fromString`), which also reads the key a transposing instrument is in:
//! `"Clarinet in A"` is a clarinet sounding a minor third below what is
//! written.
//!
//! What stays music21's is what an instrument does to a stream -- part ids,
//! bundling, partitioning a score by instrument -- which is where a score
//! holds one rather than what one is.

mod tables;

use std::fmt;

use crate::{
    error::{Error, Result},
    interval::Interval,
    pitch::Pitch,
};

use tables::{KINDS, Kind};

/// Which of music21's name tables [`Instrument::from_name`] reads: music21's
/// `SearchLanguage`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SearchLanguage {
    /// Every language and the abbreviations, English winning where two
    /// disagree.
    All,
    /// English names.
    English,
    /// French names.
    French,
    /// German names.
    German,
    /// Italian names.
    Italian,
    /// Russian names, transliterated.
    Russian,
    /// Spanish names.
    Spanish,
    /// The standard abbreviations, an honorary language.
    Abbreviation,
}

impl SearchLanguage {
    /// Every language on its own, in music21's order, without
    /// [`SearchLanguage::All`].
    pub const EACH: [SearchLanguage; 7] = [
        SearchLanguage::English,
        SearchLanguage::French,
        SearchLanguage::German,
        SearchLanguage::Italian,
        SearchLanguage::Russian,
        SearchLanguage::Spanish,
        SearchLanguage::Abbreviation,
    ];

    /// music21's name for the language: `"all"`, `"english"` and so on.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::English => "english",
            Self::French => "french",
            Self::German => "german",
            Self::Italian => "italian",
            Self::Russian => "russian",
            Self::Spanish => "spanish",
            Self::Abbreviation => "abbreviation",
        }
    }

    /// The language music21 calls by that name, in any case.
    pub fn from_name(name: &str) -> Result<Self> {
        let lower = name.to_lowercase();
        std::iter::once(Self::All)
            .chain(Self::EACH)
            .find(|language| language.as_str() == lower)
            .ok_or_else(|| {
                Error::Instrument(format!("Chosen language {lower} not currently supported."))
            })
    }

    fn table(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::All => &tables::ALL,
            Self::English => &tables::ENGLISH,
            Self::French => &tables::FRENCH,
            Self::German => &tables::GERMAN,
            Self::Italian => &tables::ITALIAN,
            Self::Russian => &tables::RUSSIAN,
            Self::Spanish => &tables::SPANISH,
            Self::Abbreviation => &tables::ABBREVIATION,
        }
    }
}

/// An instrument of one of music21's kinds: music21's `Instrument`.
///
/// ```
/// use music21_rs::instrument::{Instrument, SearchLanguage};
///
/// let clarinet = Instrument::from_name("Clarinet in A", SearchLanguage::All)?;
/// assert_eq!(clarinet.kind(), "Clarinet");
/// assert!(clarinet.is_a("WoodwindInstrument"));
/// assert_eq!(clarinet.transposition().unwrap().directed_name(), "m-3");
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Instrument {
    kind: String,
    name: Option<String>,
    abbreviation: Option<String>,
    part_name: Option<String>,
    part_abbreviation: Option<String>,
    sound: Option<String>,
    midi_program: Option<u8>,
    midi_channel: Option<u8>,
    lowest: Option<Pitch>,
    highest: Option<Pitch>,
    transposition: Option<Interval>,
    percussion_map: bool,
    percussion_pitch: Option<u8>,
}

impl Default for Instrument {
    /// music21's bare `Instrument`, which says nothing about itself.
    fn default() -> Self {
        Self::of_kind("Instrument").expect("music21's base class is in the table")
    }
}

fn kind_named(class: &str) -> Option<&'static Kind> {
    KINDS.iter().find(|kind| kind.class == class)
}

impl Instrument {
    /// music21's bare `Instrument`, which says nothing about itself.
    pub fn new() -> Self {
        Self::default()
    }

    /// An instrument of the kind music21 names by that class, as a new one of
    /// that class starts out: `"Piano"`, `"BassClarinet"`, `"Triangle"`.
    pub fn of_kind(class: &str) -> Result<Self> {
        let kind = kind_named(class)
            .ok_or_else(|| Error::Instrument(format!("no instrument class {class}")))?;
        let pitch = |name: Option<&str>| name.map(Pitch::from_name).transpose();
        Ok(Self {
            kind: kind.class.to_string(),
            name: kind.name.map(String::from),
            abbreviation: kind.abbreviation.map(String::from),
            part_name: None,
            part_abbreviation: None,
            sound: kind.sound.map(String::from),
            midi_program: kind.midi_program,
            midi_channel: kind.midi_channel,
            lowest: pitch(kind.lowest)?,
            highest: pitch(kind.highest)?,
            transposition: kind.transposition.map(Interval::from_name).transpose()?,
            percussion_map: kind.percussion_map,
            percussion_pitch: kind.percussion_pitch,
        })
    }

    /// Every kind of instrument music21 has a class for, families first.
    pub fn kinds() -> impl Iterator<Item = &'static str> {
        KINDS.iter().map(|kind| kind.class)
    }

    /// The instrument music21 makes of a General MIDI program, 0 to 127,
    /// with its program set to that number: music21's
    /// `instrumentFromMidiProgram`.
    pub fn from_midi_program(program: u8) -> Result<Self> {
        let class = tables::MIDI_PROGRAMS
            .iter()
            .find(|(number, _)| *number == program)
            .map(|(_, class)| *class)
            .ok_or_else(|| {
                Error::Instrument(format!("No instrument found for MIDI program {program}"))
            })?;
        let mut made = Self::of_kind(class)?;
        made.midi_program = Some(program);
        Ok(made)
    }

    /// The instrument a score's name for one means, read the way music21's
    /// `fromString` reads it.
    ///
    /// The name is taken apart into every run of its words, and the longest
    /// run naming an instrument wins, a later one breaking a tie unless the
    /// one found is already of its kind -- so `"Bb Piccolo Trumpet"` is a
    /// trumpet, not a piccolo. The instrument is named with the text as
    /// given. For an instrument that comes in several keys, a run
    /// naming a pitch sets its transposition: `"Horn in F"`, `"Klarinette in
    /// B."`.
    pub fn from_name(text: &str, language: SearchLanguage) -> Result<Self> {
        let table = language.table();
        let cleaned: String = text
            .replace('.', " ")
            .to_lowercase()
            .chars()
            .filter(|c| !c.is_ascii_punctuation())
            .collect();
        let runs = word_runs(&cleaned);

        let mut best: Option<(Self, String)> = None;
        for run in &runs {
            let Some((_, class)) = table.iter().find(|(name, _)| name == run) else {
                continue;
            };
            let candidate = Self::of_kind(class)?;
            let Some(candidate_name) = candidate.best_name().map(str::to_lowercase) else {
                continue;
            };
            let replaces = match &best {
                None => true,
                Some((found, found_name)) => {
                    candidate_name.split_whitespace().count()
                        >= found_name.split_whitespace().count()
                        && !found.is_a(class)
                }
            };
            if replaces {
                let mut named = candidate;
                named.name = Some(text.to_string());
                best = Some((named, candidate_name));
            }
        }
        let Some((mut found, found_name)) = best else {
            return Err(Error::Instrument(format!(
                "Could not match string with instrument: {text}"
            )));
        };
        let Some((_, keys)) = tables::TRANSPOSITIONS
            .iter()
            .find(|(name, _)| *name == found_name)
        else {
            return Ok(found);
        };
        for run in &runs {
            let Some((_, pitch)) = tables::PITCH_NAMES.iter().find(|(name, _)| name == run) else {
                continue;
            };
            if let Some((_, interval)) = keys.iter().find(|(key, _)| key == pitch) {
                found.transposition = Some(Interval::from_name(interval)?);
                break;
            }
        }
        Ok(found)
    }

    /// The kind of instrument this is: music21's class name for it.
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// The families this kind belongs to, nearest first and ending with
    /// `Instrument`: a clarinet is a `WoodwindInstrument`.
    pub fn families(&self) -> &'static [&'static str] {
        kind_named(&self.kind).map_or(&[], |kind| kind.parents)
    }

    /// Whether this is an instrument of that kind or of a kind in that
    /// family, as music21's `isinstance` answers.
    pub fn is_a(&self, class: &str) -> bool {
        self.kind == class || self.families().contains(&class)
    }

    /// The instrument's name: music21's `instrumentName`.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Sets the instrument's name, or takes it away.
    pub fn set_name(&mut self, name: Option<String>) {
        self.name = name;
    }

    /// The instrument's abbreviation: music21's `instrumentAbbreviation`.
    pub fn abbreviation(&self) -> Option<&str> {
        self.abbreviation.as_deref()
    }

    /// Sets the abbreviation, or takes it away.
    pub fn set_abbreviation(&mut self, abbreviation: Option<String>) {
        self.abbreviation = abbreviation;
    }

    /// What the part this instrument plays is called, where that is not
    /// the instrument's name: music21's `partName`.
    pub fn part_name(&self) -> Option<&str> {
        self.part_name.as_deref()
    }

    /// Sets what the part is called.
    pub fn set_part_name(&mut self, name: Option<String>) {
        self.part_name = name;
    }

    /// The part's abbreviation: music21's `partAbbreviation`.
    pub fn part_abbreviation(&self) -> Option<&str> {
        self.part_abbreviation.as_deref()
    }

    /// Sets the part's abbreviation.
    pub fn set_part_abbreviation(&mut self, abbreviation: Option<String>) {
        self.part_abbreviation = abbreviation;
    }

    /// The best name there is for this instrument: the part's name, the
    /// part's abbreviation, the instrument's name or its abbreviation, the
    /// first of those given. music21's `bestName`.
    pub fn best_name(&self) -> Option<&str> {
        self.part_name
            .as_deref()
            .or(self.part_abbreviation.as_deref())
            .or(self.name.as_deref())
            .or(self.abbreviation.as_deref())
    }

    /// The sound's MusicXML id, `"wind.reed.clarinet.bflat"` and the like:
    /// music21's `instrumentSound`.
    pub fn sound(&self) -> Option<&str> {
        self.sound.as_deref()
    }

    /// Sets the sound's id.
    pub fn set_sound(&mut self, sound: Option<String>) {
        self.sound = sound;
    }

    /// The General MIDI program, counted from nought: music21's `midiProgram`.
    pub fn midi_program(&self) -> Option<u8> {
        self.midi_program
    }

    /// Sets the General MIDI program.
    pub fn set_midi_program(&mut self, program: Option<u8>) {
        self.midi_program = program;
    }

    /// The MIDI channel, counted from nought, 9 being the drums: music21's
    /// `midiChannel`.
    pub fn midi_channel(&self) -> Option<u8> {
        self.midi_channel
    }

    /// Sets the MIDI channel.
    pub fn set_midi_channel(&mut self, channel: Option<u8>) {
        self.midi_channel = channel;
    }

    /// The lowest note the instrument plays, as it sounds: music21's
    /// `lowestNote`.
    pub fn lowest(&self) -> Option<&Pitch> {
        self.lowest.as_ref()
    }

    /// Sets the lowest note.
    pub fn set_lowest(&mut self, pitch: Option<Pitch>) {
        self.lowest = pitch;
    }

    /// The highest note the instrument plays: music21's `highestNote`.
    pub fn highest(&self) -> Option<&Pitch> {
        self.highest.as_ref()
    }

    /// Sets the highest note.
    pub fn set_highest(&mut self, pitch: Option<Pitch>) {
        self.highest = pitch;
    }

    /// The interval from what is written for the instrument to what sounds:
    /// a B-flat clarinet sounds a major second below, `M-2`. music21's
    /// `transposition`.
    pub fn transposition(&self) -> Option<&Interval> {
        self.transposition.as_ref()
    }

    /// Sets the transposition, or takes it away.
    pub fn set_transposition(&mut self, interval: Option<Interval>) {
        self.transposition = interval;
    }

    /// What sounds when `written` is played: the pitch moved by the
    /// transposition, or itself for an instrument that does not transpose.
    pub fn sounding(&self, written: &Pitch) -> Result<Pitch> {
        match &self.transposition {
            Some(interval) => interval.transpose_pitch(written),
            None => Ok(written.clone()),
        }
    }

    /// What is written for `sounding` to be heard.
    pub fn written(&self, sounding: &Pitch) -> Result<Pitch> {
        match &self.transposition {
            Some(interval) => interval.reversed()?.transpose_pitch(sounding),
            None => Ok(sounding.clone()),
        }
    }

    /// Whether the instrument is one of General MIDI's percussion map:
    /// music21's `inGMPercMap`.
    pub fn in_percussion_map(&self) -> bool {
        self.percussion_map
    }

    /// The General MIDI drum the instrument plays, for unpitched percussion:
    /// music21's `percMapPitch`.
    pub fn percussion_pitch(&self) -> Option<u8> {
        self.percussion_pitch
    }

    /// Takes a MIDI channel no instrument in `used` has, and returns it:
    /// music21's `autoAssignMidiChannel`.
    ///
    /// Unpitched percussion goes to channel 9 when that is free; otherwise
    /// the lowest free channel below `max_channels` that is not a drum
    /// channel is taken. It is an error when every channel is in use; where
    /// none is free the answer is 0 and no channel is set, as music21 answers.
    pub fn auto_assign_midi_channel(&mut self, used: &[u8], max_channels: u8) -> Result<u8> {
        let used: std::collections::BTreeSet<u8> = used.iter().copied().collect();
        if self.is_a("UnpitchedPercussion") && !used.contains(&9) {
            self.midi_channel = Some(9);
            return Ok(9);
        }
        if used.is_empty() {
            self.midi_channel = Some(0);
            return Ok(0);
        }
        if used.len() >= usize::from(max_channels).saturating_sub(1) {
            return Err(Error::Instrument(
                "we are out of midi channels! help!".to_string(),
            ));
        }
        for channel in 0..max_channels {
            if used.contains(&channel) || channel % 16 == 9 {
                continue;
            }
            self.midi_channel = Some(channel);
            return Ok(channel);
        }
        Ok(0)
    }

    /// Every name a score may call this kind of instrument in one language,
    /// or in each of them: music21's `getAllNamesForInstrument`.
    ///
    /// The names are those of the nearest kind in the instrument's family
    /// that the tables name at all -- its own, or for a guitar, which they do
    /// not name, a string instrument's -- whatever the instrument itself is
    /// called.
    pub fn all_names(&self, language: SearchLanguage) -> Vec<(SearchLanguage, Vec<&'static str>)> {
        let named = std::iter::once(self.kind.as_str())
            .chain(self.families().iter().copied())
            .find(|class| tables::ALL.iter().any(|(_, named)| named == class));
        let languages: &[SearchLanguage] = match language {
            SearchLanguage::All => &SearchLanguage::EACH,
            _ => std::slice::from_ref(&language),
        };
        languages
            .iter()
            .map(|language| {
                let names = language
                    .table()
                    .iter()
                    .filter(|(_, class)| Some(*class) == named)
                    .map(|(name, _)| *name)
                    .collect();
                (*language, names)
            })
            .collect()
    }
}

impl fmt::Display for Instrument {
    /// music21's `str` of an instrument, less the part id a score gives it:
    /// the part's name where it differs from the instrument's, then the
    /// instrument's name.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(part) = &self.part_name
            && self.name.as_ref() != Some(part)
        {
            write!(f, "{part}: ")?;
        }
        if let Some(name) = &self.name {
            f.write_str(name)?;
        }
        Ok(())
    }
}

/// Every run of consecutive words in `text`, shortest first and left to
/// right within a length: music21's `_combinations`.
fn word_runs(text: &str) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut runs = Vec::new();
    for size in 1..=words.len() {
        for start in 0..=words.len() - size {
            runs.push(words[start..start + size].join(" "));
        }
    }
    runs
}

/// What an ensemble of so many players is called, from `"no performers"` to
/// `"centet"`: music21's `ensembleNameBySize`, which is `None` past a hundred.
pub fn ensemble_name_by_size(players: usize) -> Option<&'static str> {
    tables::ENSEMBLE_NAMES.get(players).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every expectation here was read off music21 11.0.0b9.

    #[test]
    fn an_instrument_starts_out_as_its_class_does() {
        let piano = Instrument::of_kind("Piano").unwrap();
        assert_eq!(piano.name(), Some("Piano"));
        assert_eq!(piano.midi_program(), Some(0));
        assert_eq!(piano.lowest().unwrap().name_with_octave(), "A0");
        assert!(piano.is_a("KeyboardInstrument"));
        let saxophone = Instrument::of_kind("AltoSaxophone").unwrap();
        assert_eq!(saxophone.transposition().unwrap().directed_name(), "M-6");
        assert_eq!(
            saxophone
                .sounding(&Pitch::from_name("C5").unwrap())
                .unwrap()
                .name_with_octave(),
            "E-4"
        );
        assert!(Instrument::of_kind("Kazoo").is_err());
    }

    #[test]
    fn a_score_s_name_for_an_instrument_is_read_as_music21_reads_it() {
        let found = |text: &str| Instrument::from_name(text, SearchLanguage::All).unwrap();
        assert_eq!(found("Contrabassoon").kind(), "Contrabassoon");
        assert_eq!(found("Bb Piccolo Trumpet").kind(), "Trumpet");
        let clarinet = found("Klarinette in B.");
        assert_eq!(clarinet.kind(), "Clarinet");
        assert_eq!(clarinet.name(), Some("Klarinette in B."));
        assert_eq!(clarinet.transposition().unwrap().directed_name(), "M-2");
        assert_eq!(found("Cl.").kind(), "Clarinet");
        assert!(Instrument::from_name("kazoo concerto", SearchLanguage::All).is_err());
        assert!(Instrument::from_name("Klarinette", SearchLanguage::French).is_err());
    }

    #[test]
    fn a_channel_is_found_as_music21_finds_one() {
        let mut violin = Instrument::of_kind("Violin").unwrap();
        assert_eq!(
            violin
                .auto_assign_midi_channel(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11], 16)
                .unwrap(),
            12
        );
        let mut drum = Instrument::of_kind("Triangle").unwrap();
        assert_eq!(drum.auto_assign_midi_channel(&[0, 1], 16).unwrap(), 9);
        let full: Vec<u8> = (0..16).collect();
        assert!(violin.auto_assign_midi_channel(&full, 16).is_err());
        assert_eq!(violin.auto_assign_midi_channel(&full, 32).unwrap(), 16);
        assert_eq!(ensemble_name_by_size(4), Some("quartet"));
    }
}
