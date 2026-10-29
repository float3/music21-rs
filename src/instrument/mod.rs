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
//! What an instrument does to a stream it stands in is here too: moving the
//! instruments notes keep as their own into the stream and back
//! ([`unbundle_instruments`], [`bundle_instruments`]), folding together the
//! instruments a part says twice ([`deduplicate`]) and splitting a score into
//! a part for each instrument ([`partition_by_instrument`]).

mod streams;
mod tables;

pub(crate) use streams::settle_by_offset;
pub use streams::{bundle_instruments, deduplicate, partition_by_instrument, unbundle_instruments};

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
    string_pitches: Option<Vec<Pitch>>,
    modifier: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    headless: Option<bool>,
    #[cfg_attr(feature = "serde", serde(default))]
    pitched: Option<bool>,
    #[cfg_attr(feature = "serde", serde(default))]
    part_id: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    instrument_id: Option<String>,
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
        // A flag music21 sets in one class's `__init__` belongs to every
        // kind below it too, so the whole chain is asked.
        let flagged = |classes: &[&str]| {
            std::iter::once(kind.class)
                .chain(kind.parents.iter().copied())
                .any(|class| classes.contains(&class))
                .then_some(false)
        };
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
            string_pitches: tables::STRING_PITCHES
                .iter()
                .find(|(named, _)| *named == kind.class)
                .map(|(_, names)| names.iter().map(Pitch::from_name).collect())
                .transpose()?,
            modifier: tables::MODIFIERS
                .iter()
                .find(|(named, ..)| *named == kind.class)
                .map(|(_, modifier, ..)| modifier.to_string()),
            headless: flagged(&tables::HEADLESS),
            pitched: flagged(&tables::PITCHED),
            part_id: None,
            instrument_id: None,
        })
    }

    /// The id of the part this instrument plays, as a score writes it:
    /// music21's `partId`, `P1` in a score read from MusicXML.
    pub fn part_id(&self) -> Option<&str> {
        self.part_id.as_deref()
    }

    /// Sets the id of the part this instrument plays.
    pub fn set_part_id(&mut self, id: Option<String>) {
        self.part_id = id;
    }

    /// The instrument's own id within its part, `P1-I1`: music21's
    /// `instrumentId`.
    pub fn instrument_id(&self) -> Option<&str> {
        self.instrument_id.as_deref()
    }

    /// Sets the instrument's own id.
    pub fn set_instrument_id(&mut self, id: Option<String>) {
        self.instrument_id = id;
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

    /// music21's own name for this kind of instrument, where it is one
    /// music21 has a class for: the name as the table spells it, which is
    /// what `is_a` and the class lineage answer with.
    pub fn class_name(&self) -> Option<&'static str> {
        kind_named(&self.kind).map(|kind| kind.class)
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

    /// Sets the General MIDI drum the instrument plays.
    pub fn set_percussion_pitch(&mut self, pitch: Option<u8>) {
        self.percussion_pitch = pitch;
    }

    /// Whether the drum is struck muted: music21's `muted`, which is the
    /// *drum* it plays and not a flag beside it -- a muted conga drum is
    /// key 62 where an open one is 63.
    ///
    /// Three of music21's classes carry it, and `None` here is an
    /// instrument that is not one of them.
    #[must_use]
    pub fn muted(&self) -> Option<bool> {
        let (muted, _, _) = Self::muting(&self.kind)?;
        Some(self.percussion_pitch == Some(muted))
    }

    /// Strikes it muted or open, where it is an instrument that can be.
    ///
    /// A conga drum's muting is the high drum's alone, so asking it of the
    /// low one does nothing, exactly as music21's setter does nothing
    /// unless the modifier is `high`.
    pub fn set_muted(&mut self, muted: bool) {
        let Some((muted_key, open_key, only)) = Self::muting(&self.kind) else {
            return;
        };
        if only.is_some_and(|modifier| self.modifier() != Some(modifier)) {
            return;
        }
        self.percussion_pitch = Some(if muted { muted_key } else { open_key });
    }

    /// Whether the tambourine has no drum head -- the rock and pop kind, a
    /// ring of jingles -- where a classical one is headed: music21's
    /// `headless`, which only its `Tambourine` carries, so `None` is an
    /// instrument that is not one.
    ///
    /// No document says it: MIDI and MusicXML both write a tambourine the
    /// same either way, and music21's MIDI reader takes every one as
    /// headless because most such files are rock.
    #[must_use]
    pub fn headless(&self) -> Option<bool> {
        self.headless
    }

    /// Says whether the tambourine is headless, where it is a tambourine at
    /// all; asked of anything else it does nothing, as [`Instrument::muted`]
    /// does.
    pub fn set_headless(&mut self, headless: bool) {
        if self.headless.is_some() {
            self.headless = Some(headless);
        }
    }

    /// Whether the whistle sounds a definite pitch -- a tin whistle does, a
    /// referee's does not: music21's `pitched`, which only its `Whistle`
    /// carries, so `None` is an instrument that is not one.
    #[must_use]
    pub fn pitched(&self) -> Option<bool> {
        self.pitched
    }

    /// Says whether the whistle is pitched, where it is a whistle at all.
    pub fn set_pitched(&mut self, pitched: bool) {
        if self.pitched.is_some() {
            self.pitched = Some(pitched);
        }
    }

    /// The pair of drums a kind is struck muted and open, and the modifier
    /// its muting is of where it has one.
    fn muting(kind: &str) -> Option<(u8, u8, Option<&'static str>)> {
        tables::MUTED
            .iter()
            .find(|(class, _, _, _)| *class == kind)
            .map(|(_, muted, open, only)| (*muted, *open, *only))
    }

    /// Says whether the instrument is in the General MIDI percussion map.
    pub fn set_in_percussion_map(&mut self, in_map: bool) {
        self.percussion_map = in_map;
    }

    /// The open strings, lowest first unless the tuning is reentrant, for a
    /// string instrument music21 tunes: music21's `stringPitches`.
    ///
    /// ```
    /// use music21_rs::Instrument;
    ///
    /// let violin = Instrument::of_kind("Violin")?;
    /// let open: Vec<String> = violin
    ///     .string_pitches()
    ///     .unwrap()
    ///     .iter()
    ///     .map(|pitch| pitch.name_with_octave())
    ///     .collect();
    /// assert_eq!(open, ["G3", "D4", "A4", "E5"]);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    pub fn string_pitches(&self) -> Option<&[Pitch]> {
        self.string_pitches.as_deref()
    }

    /// Retunes the open strings, a scordatura.
    pub fn set_string_pitches(&mut self, pitches: Vec<Pitch>) {
        self.string_pitches = Some(pitches);
    }

    /// How an unpitched percussion instrument is played, such as `"low"` for
    /// a bongo or `"rimshot"` for a snare: music21's `modifier`.
    pub fn modifier(&self) -> Option<&str> {
        self.modifier.as_deref()
    }

    /// Sets how the instrument is played. A modifier its drum map knows
    /// changes the drum it plays, and is written back as that drum's own name
    /// for it, so a woodblock set to `"LO"` plays the low block and says
    /// `"low"`.
    ///
    /// ```
    /// use music21_rs::Instrument;
    ///
    /// let mut woodblock = Instrument::of_kind("Woodblock")?;
    /// woodblock.set_modifier("LO");
    /// assert_eq!(woodblock.modifier(), Some("low"));
    /// assert_eq!(woodblock.percussion_pitch(), Some(77));
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    pub fn set_modifier(&mut self, modifier: &str) {
        let mut modifier = modifier.trim().to_lowercase();
        let table = std::iter::once(self.kind.as_str())
            .chain(self.families().iter().copied())
            .find_map(|class| tables::MODIFIERS.iter().find(|(named, ..)| *named == class));
        if self.percussion_map
            && let Some((_, _, to_pitch, to_name)) = table
            && let Some((_, pitch)) = to_pitch.iter().find(|(named, _)| *named == modifier)
        {
            self.percussion_pitch = Some(*pitch);
            if let Some((_, named)) = to_name.iter().find(|(drum, _)| drum == pitch) {
                modifier = (*named).to_string();
            }
        }
        self.modifier = Some(modifier);
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
        // An instrument no table names by its kind is looked up by its own
        // name, as the class it would be.
        let named = std::iter::once(self.kind.as_str())
            .chain(self.families().iter().copied())
            .find(|class| tables::ALL.iter().any(|(_, named)| named == class))
            .or(self.name.as_deref());
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

/// The instrument a key of the General MIDI percussion map plays, played
/// the way that key says: music21's
/// `midi.percussion.PercussionMapper.midiPitchToInstrument`.
///
/// A key is 0 to 127 and the map runs from 27 to 87. Where the instrument
/// is played more than one way the modifier is the one that key plays, so
/// key 36 is a bass drum played `"midi-1"` and key 35 the same drum played
/// `"acoustic"`. Nine of the map's sounds are not instruments at all --
/// a hand clap, a metronome click, a scratch -- and each of those is a
/// bare unpitched percussion instrument under the sound's own name
/// (music21's `namedSoundMapping`). A key the map says nothing of is an
/// error, as it raises there; [`for_percussion_key_or_unmapped`] is
/// music21's `raiseIfUnmapped=False`.
///
/// **The key is the drum, not the class's own.** music21 writes the key
/// onto the instrument after the modifier, so a key that is the muted one
/// of a pair stays muted where its class starts on the open one: key 62 is
/// a conga drum with the *high* drum's modifier struck muted, where 63 is
/// the same modifier struck open.
///
/// ```
/// use music21_rs::instrument::for_percussion_key;
///
/// let drum = for_percussion_key(36)?;
/// assert_eq!(drum.kind(), "BassDrum");
/// assert_eq!(drum.modifier(), Some("midi-1"));
/// assert_eq!(drum.percussion_pitch(), Some(36));
///
/// let muted = for_percussion_key(62)?;
/// assert_eq!((muted.modifier(), muted.muted()), (Some("high"), Some(true)));
///
/// let clap = for_percussion_key(39)?;
/// assert_eq!(clap.name(), Some("Hand Clap"));
///
/// assert!(for_percussion_key(22).is_err());
/// # Ok::<(), music21_rs::Error>(())
/// ```
///
/// # Errors
///
/// A key the map names neither an instrument nor a sound for.
pub fn for_percussion_key(key: u8) -> Result<Instrument> {
    percussion_key_instrument(key)
        .ok_or_else(|| Error::Instrument(format!("{key} does not map to a valid instrument!")))?
}

/// The instrument a key of the General MIDI percussion map plays, and a
/// bare unpitched percussion instrument carrying the key where the map
/// names nothing at all: music21's `midiPitchToInstrument` asked with
/// `raiseIfUnmapped=False`, which is how its MIDI reader asks.
///
/// ```
/// use music21_rs::instrument::for_percussion_key_or_unmapped;
///
/// let bare = for_percussion_key_or_unmapped(22);
/// assert_eq!(bare.kind(), "UnpitchedPercussion");
/// assert_eq!(bare.name(), Some("Percussion"));
/// assert_eq!(bare.percussion_pitch(), Some(22));
/// ```
pub fn for_percussion_key_or_unmapped(key: u8) -> Instrument {
    percussion_key_instrument(key)
        .unwrap_or_else(|| {
            Instrument::of_kind("UnpitchedPercussion").map(|mut bare| {
                bare.set_percussion_pitch(Some(key));
                bare
            })
        })
        .unwrap_or_default()
}

/// Every key of the General MIDI percussion map that names an instrument
/// class, in key order: music21's `reverseInstrumentMapping` read as its
/// keys, which is how a caller listing the map asks for one class per key
/// without the named sounds among them.
pub fn percussion_keys() -> impl Iterator<Item = u8> {
    tables::PERCUSSION_KEYS.iter().map(|(key, _)| *key)
}

/// Every General MIDI percussion sound that is no instrument, each with
/// the name music21 gives it: music21's `namedSoundMapping`, a hand clap
/// and a metronome among them.
pub fn named_percussion_sounds() -> impl Iterator<Item = (u8, &'static str)> {
    tables::NAMED_SOUNDS.iter().copied()
}

/// The instrument or the named sound a key plays, and nothing for a key
/// the map says nothing of. The inner result is the class lookup, which
/// cannot fail for a class in the table.
fn percussion_key_instrument(key: u8) -> Option<Result<Instrument>> {
    if let Some((_, kind)) = tables::PERCUSSION_KEYS
        .iter()
        .find(|(named, _)| *named == key)
    {
        return Some(Instrument::of_kind(kind).map(|mut made| {
            // music21 writes the modifier that key plays onto the
            // instrument it built, which moves the drum it plays with it,
            // and then writes the key itself -- so a muted drum is the key
            // struck and not the one its modifier names.
            if let Some((_, _, _, by_key)) =
                tables::MODIFIERS.iter().find(|(named, ..)| named == kind)
                && let Some((_, modifier)) = by_key.iter().find(|(drum, _)| *drum == key)
            {
                made.set_modifier(modifier);
            }
            made.set_percussion_pitch(Some(key));
            made
        }));
    }
    let (_, sound) = tables::NAMED_SOUNDS
        .iter()
        .find(|(named, _)| *named == key)?;
    Some(Instrument::of_kind("UnpitchedPercussion").map(|mut made| {
        made.set_name(Some((*sound).to_string()));
        made.set_percussion_pitch(Some(key));
        made
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every expectation here was read off music21 11.0.0b9.

    #[test]
    fn an_instrument_of_no_named_kind_is_looked_up_by_its_name() {
        let mut violin = Instrument::new();
        violin.set_name(Some("Violin".to_string()));
        assert_eq!(
            violin.all_names(SearchLanguage::English),
            [(SearchLanguage::English, vec!["violin", "violins"])]
        );
    }

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
            "Eb4"
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

    #[test]
    fn a_percussion_key_is_the_drum_that_key_strikes_and_not_the_class_s_own() {
        // music21 writes the key onto the instrument *after* the modifier,
        // so a key that is the muted one of a pair stays the muted one
        // where its class starts on the open one -- and a muted conga is
        // key 62 with the *high* drum's modifier, 63 being the open high
        // drum. Read off music21 11.0.0b14.
        for (key, kind, modifier, muted) in [
            (62u8, "CongaDrum", Some("high"), Some(true)),
            (63, "CongaDrum", Some("high"), Some(false)),
            (64, "CongaDrum", Some("low"), Some(false)),
            (78, "Cuica", None, Some(true)),
            (79, "Cuica", None, Some(false)),
            (86, "Surdo", None, Some(true)),
            (87, "Surdo", None, Some(false)),
            (35, "BassDrum", Some("acoustic"), None),
            (36, "BassDrum", Some("midi-1"), None),
        ] {
            let drum = for_percussion_key(key).expect("the map names a drum for it");
            assert_eq!(
                (
                    drum.kind(),
                    drum.modifier(),
                    drum.percussion_pitch(),
                    drum.muted()
                ),
                (kind, modifier, Some(key), muted),
                "key {key}"
            );
        }
    }

    #[test]
    fn a_sound_the_map_names_no_class_for_is_named_percussion() {
        // music21's `namedSoundMapping`, new in v11: nine General MIDI
        // sounds that are not instruments, each a bare unpitched
        // percussion instrument under the sound's own name.
        for (key, name) in [(27u8, "High Q"), (34, "Metronome Bell"), (39, "Hand Clap")] {
            let sound = for_percussion_key(key).expect("the map names the sound");
            assert_eq!(sound.kind(), "UnpitchedPercussion");
            assert_eq!(sound.name(), Some(name));
            assert_eq!(sound.percussion_pitch(), Some(key));
            assert_eq!(sound.modifier(), None);
        }
        // A key the map says nothing of at all is refused, unless the
        // caller asks for one anyway -- music21's `raiseIfUnmapped=False`,
        // which keeps the key on an instrument saying only `Percussion`.
        assert!(for_percussion_key(22).is_err());
        let bare = for_percussion_key_or_unmapped(22);
        assert_eq!(bare.kind(), "UnpitchedPercussion");
        assert_eq!(bare.name(), Some("Percussion"));
        assert_eq!(bare.percussion_pitch(), Some(22));
    }

    #[test]
    fn two_instrument_classes_carry_a_flag_of_their_own() {
        // music21 gives a tambourine `headless` and a whistle `pitched`,
        // and nothing else either.
        let tambourine = Instrument::of_kind("Tambourine").unwrap();
        assert_eq!(tambourine.headless(), Some(false));
        assert_eq!(tambourine.pitched(), None);
        let whistle = Instrument::of_kind("Whistle").unwrap();
        assert_eq!(whistle.pitched(), Some(false));
        assert_eq!(whistle.headless(), None);
        let mut piano = Instrument::of_kind("Piano").unwrap();
        assert_eq!(piano.headless(), None);
        piano.set_headless(true);
        assert_eq!(piano.headless(), None);
    }
}
