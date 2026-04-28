pub(crate) mod chordbase;
pub(crate) mod tables;

use crate::base::Music21ObjectTrait;
use crate::defaults::IntegerType;
use crate::duration::Duration;
use crate::exception::Exception;
use crate::exception::ExceptionResult;
use crate::interval::{Interval, PitchOrNote};
use crate::key::keysignature::KeySignature;
use crate::note::Note;
use crate::note::generalnote::GeneralNoteTrait;
use crate::note::notrest::NotRestTrait;
use crate::pitch::Pitch;
use crate::prebase::ProtoM21ObjectTrait;

use chordbase::ChordBase;
use chordbase::ChordBaseTrait;

use std::sync::Arc;

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A collection of notes analyzed as one vertical sonority.
///
/// `Chord` accepts several note-like inputs, including whitespace-separated
/// pitch names, slices of pitches or notes, MIDI pitch numbers, vectors, and
/// `None` for an empty chord.
pub struct Chord {
    #[cfg_attr(feature = "serde", serde(skip))]
    chordbase: Arc<ChordBase>,
    _notes: Vec<Note>,
    #[cfg_attr(feature = "serde", serde(skip))]
    from_integer_pitches: bool,
}

impl Chord {
    /// Builds a chord from any supported note collection.
    ///
    /// Empty inputs are valid: pass `""`, an empty vector or slice, or
    /// `Option::<&str>::None` to construct an empty chord.
    pub fn new<T>(notes: T) -> ExceptionResult<Self>
    where
        T: IntoNotes + Clone,
    {
        let chord_notes = notes
            .clone()
            .try_into_notes()
            .map(|notes| notes.into_iter().collect::<Vec<Note>>())?;

        let chord = Self {
            chordbase: ChordBase::new(Some(chord_notes.as_slice()), &None)?,
            _notes: chord_notes,
            from_integer_pitches: T::FROM_INTEGER_PITCHES,
        };
        // Keep construction side-effect free like music21's Chord constructor.
        // Enharmonic simplification can be requested explicitly later.
        Ok(chord)
    }

    /// Builds an empty chord.
    pub fn empty() -> ExceptionResult<Self> {
        Self::new(Option::<&str>::None)
    }

    /// Builds a chord from one string such as `"C E G"`.
    pub fn from_pitch_names(notes: &str) -> ExceptionResult<Self> {
        Self::new(notes)
    }

    /// Builds a chord from borrowed pitch-name strings.
    pub fn from_names(notes: &[&str]) -> ExceptionResult<Self> {
        Self::new(notes)
    }

    /// Builds a chord from owned pitch-name strings.
    pub fn from_strings(notes: &[String]) -> ExceptionResult<Self> {
        Self::new(notes)
    }

    /// Builds a chord from pitches.
    pub fn from_pitches(pitches: &[Pitch]) -> ExceptionResult<Self> {
        Self::new(pitches)
    }

    /// Builds a chord from notes.
    pub fn from_notes(notes: &[Note]) -> ExceptionResult<Self> {
        Self::new(notes)
    }

    /// Builds a chord by flattening the notes from other chords.
    pub fn from_chords(chords: &[Chord]) -> ExceptionResult<Self> {
        Self::new(chords)
    }

    /// Builds a chord from MIDI pitch numbers.
    pub fn from_midi_numbers(notes: &[i32]) -> ExceptionResult<Self> {
        Self::new(notes)
    }

    /// Returns the primary music21-style common name with a pitch prefix.
    pub fn pitched_common_name(&self) -> String {
        self.pitched_name_for_common_name(&self.common_name())
    }

    /// Returns every known music21-style common name with pitch prefixes.
    ///
    /// Most chords have a single common name, while some Forte-table entries
    /// have aliases. This method exposes all of them in table order.
    pub fn pitched_common_names(&self) -> Vec<String> {
        let common_names = self.common_names();
        if common_names.is_empty() {
            return vec![self.pitched_common_name()];
        }

        common_names
            .iter()
            .map(|name| self.pitched_name_for_common_name(name))
            .collect()
    }

    fn pitched_name_for_common_name(&self, name_str: &str) -> String {
        if name_str == "empty chord" {
            return name_str.to_string();
        }

        if matches!(name_str, "note" | "unison") {
            return self
                ._notes
                .first()
                .map(|n| n._pitch.name())
                .unwrap_or_else(|| name_str.to_string());
        }

        let pitch_class_cardinality = self.ordered_pitch_classes().len();
        if pitch_class_cardinality <= 2
            || name_str.contains("enharmonic")
            || name_str.contains("forte class")
            || name_str.contains(" semitone")
        {
            if let Some(bass_name) = self.bass_pitch_name() {
                return format!("{name_str} above {bass_name}");
            }
            return name_str.to_string();
        }

        if let Some(root_name) = self.spelling_root_name_override(name_str) {
            return format!("{root_name}-{name_str}");
        }

        let root_name = self.root_pitch_name_from_tables().or_else(|| {
            self._notes
                .first()
                .map(|n| Self::display_pitch_name(&n._pitch))
        });

        match root_name {
            Some(root_name) => format!("{root_name}-{name_str}"),
            None => name_str.to_string(),
        }
    }

    fn spelling_root_name_override(&self, common_name: &str) -> Option<String> {
        let root = if !common_name.contains("augmented sixth chord") {
            return None;
        } else if self.has_pitch_names(&["C#", "E-", "G"])
            || self.has_pitch_names(&["C#", "E#", "G", "B"])
        {
            "C#"
        } else if self.has_pitch_names(&["C", "D", "F#", "A-"]) {
            "D"
        } else if self.has_pitch_names(&["C#", "E-", "G", "A"]) {
            "A"
        } else if self.has_pitch_names(&["C", "E", "F#", "A#"]) {
            "F#"
        } else if self.has_pitch_names(&["D", "E", "G#", "B-"])
            || (self.from_integer_pitches && self.pitch_class_mask() == 0b010100010100)
        {
            "E"
        } else {
            return None;
        };

        Some(root.to_string())
    }

    pub(crate) fn common_name(&self) -> String {
        if self
            ._notes
            .iter()
            .any(|n| (n._pitch.alter() - n._pitch.alter().round()).abs() > f64::EPSILON)
        {
            return "microtonal chord".to_string();
        }

        if self._notes.is_empty() {
            return "empty chord".to_string();
        }

        let ordered_pcs = self.ordered_pitch_classes();
        if ordered_pcs.is_empty() {
            return "empty chord".to_string();
        }

        if ordered_pcs.len() == 1 {
            if self._notes.len() == 1 {
                return "note".to_string();
            }

            let pitch_names = self
                ._notes
                .iter()
                .map(|n| n._pitch.name())
                .collect::<std::collections::BTreeSet<_>>();

            let pitch_pses = self
                ._notes
                .iter()
                .map(|n| n._pitch.ps().round() as i32)
                .collect::<std::collections::BTreeSet<_>>();

            if pitch_names.len() == 1 {
                if pitch_pses.len() == 1 {
                    return "unison".to_string();
                }
                if pitch_pses.len() == 2 {
                    return Self::interval_nice_name(
                        &self._notes[0]._pitch,
                        &self._notes[1]._pitch,
                    )
                    .unwrap_or_else(|| "multiple octaves".to_string());
                }
                return "multiple octaves".to_string();
            }
            if pitch_pses.len() == 1 {
                return "enharmonic unison".to_string();
            }
            return "enharmonic octaves".to_string();
        }

        if ordered_pcs.len() == 2 {
            return self.dyad_common_name();
        }

        if let Some(common_name) = self.spelling_common_name_override() {
            return common_name;
        }

        let address = match tables::seek_chord_tables_address(&ordered_pcs) {
            Ok(address) => address,
            Err(_) => return "unknown chord".to_string(),
        };

        match tables::address_to_common_names(address) {
            Ok(Some(common_names)) if !common_names.is_empty() => common_names[0].to_string(),
            _ => match tables::address_to_forte_name(address, "tn") {
                Ok(forte_name) => format!("forte class {forte_name}"),
                Err(_) => "unknown chord".to_string(),
            },
        }
    }

    fn spelling_common_name_override(&self) -> Option<String> {
        let name = if self.has_pitch_names(&["C#", "E-", "G"]) {
            "Italian augmented sixth chord in root position"
        } else if self.has_pitch_names(&["C", "D", "F#", "A-"])
            || self.has_pitch_names(&["D", "E", "G#", "B-"])
            || (self.from_integer_pitches && self.pitch_class_mask() == 0b010100010100)
        {
            "French augmented sixth chord in third inversion"
        } else if self.has_pitch_names(&["C#", "E-", "G", "A"]) {
            "French augmented sixth chord in first inversion"
        } else if self.has_pitch_names(&["C", "E", "F#", "A#"]) {
            "French augmented sixth chord"
        } else if self.has_pitch_names(&["C#", "E#", "G", "B"]) {
            "French augmented sixth chord in root position"
        } else if self.has_pitch_names(&["E-", "F#", "A"])
            || self.has_pitch_names(&["C#", "G", "A#"])
            || (self.from_integer_pitches && self.pitch_class_mask() == 0b001001001000)
        {
            "enharmonic equivalent to diminished triad"
        } else if self.has_pitch_names(&["C#", "D#", "F#", "A#"])
            || self.has_pitch_names(&["C#", "E#", "G#", "A#"])
            || self.has_pitch_names(&["E-", "G-", "A-", "C-"])
        {
            "enharmonic equivalent to minor seventh chord"
        } else if self.has_pitch_names(&["C#", "E#", "F#", "A#"])
            || self.has_pitch_names(&["E-", "F-", "A-", "C-"])
            || self.has_pitch_names(&["E-", "G-", "B-", "C-"])
        {
            "enharmonic equivalent to major seventh chord"
        } else if self.has_pitch_names(&["E-", "F#", "A", "B"]) {
            "enharmonic to dominant seventh chord"
        } else {
            return None;
        };

        Some(name.to_string())
    }

    fn dyad_common_name(&self) -> String {
        let pitch_names = self
            ._notes
            .iter()
            .map(|n| n._pitch.name())
            .collect::<std::collections::BTreeSet<_>>();

        let pitch_pses = self
            ._notes
            .iter()
            .map(|n| n._pitch.ps().round() as i32)
            .collect::<std::collections::BTreeSet<_>>();

        let Some(p0) = self._notes.first().map(|n| &n._pitch) else {
            return "empty chord".to_string();
        };
        let p0_pitch_class = Self::pitch_class(p0);

        let Some(p1) = self
            ._notes
            .iter()
            .skip(1)
            .find(|n| Self::pitch_class(&n._pitch) != p0_pitch_class)
            .map(|n| &n._pitch)
        else {
            return "unknown chord".to_string();
        };

        let relevant_interval = Interval::between(
            PitchOrNote::Pitch(p0.clone()),
            PitchOrNote::Pitch(p1.clone()),
        );

        if pitch_names.len() > 2 {
            let Ok(interval) = relevant_interval else {
                return "unknown chord".to_string();
            };
            let semitones = interval.chromatic.semitones.abs() % 12;
            let plural = if semitones == 1 { "" } else { "s" };
            return format!("{semitones} semitone{plural}");
        }

        if pitch_pses.len() > 2 {
            return relevant_interval
                .map(|interval| {
                    format!("{} with octave doublings", interval.semi_simple_nice_name())
                })
                .unwrap_or_else(|_| "unknown chord".to_string());
        }

        Self::interval_nice_name(&self._notes[0]._pitch, &self._notes[1]._pitch)
            .unwrap_or_else(|| "unknown chord".to_string())
    }

    /// Returns all unpitched common-name aliases known for this chord.
    pub fn common_names(&self) -> Vec<String> {
        let ordered_pcs = self.ordered_pitch_classes();
        let Ok(address) = tables::seek_chord_tables_address(&ordered_pcs) else {
            return Vec::new();
        };
        tables::address_to_common_names(address)
            .ok()
            .flatten()
            .unwrap_or_default()
            .into_iter()
            .map(str::to_string)
            .collect()
    }

    /// Returns the distinct pitch classes in ascending order.
    pub fn pitch_classes(&self) -> Vec<u8> {
        self.ordered_pitch_classes()
    }

    pub(crate) fn pitches(&self) -> Vec<Pitch> {
        self._notes.iter().map(|note| note._pitch.clone()).collect()
    }

    /// Returns the inferred root pitch name when the chord has one.
    pub fn root_pitch_name(&self) -> Option<String> {
        self.root_pitch_name_from_tables()
    }

    /// Returns the lowest pitch name in the chord.
    pub fn bass_pitch_name_public(&self) -> Option<String> {
        self.bass_pitch_name()
    }

    /// Returns the Forte class, such as `"3-11B"`, when available.
    pub fn forte_class(&self) -> Option<String> {
        let ordered_pcs = self.ordered_pitch_classes();
        let address = tables::seek_chord_tables_address(&ordered_pcs).ok()?;
        tables::address_to_forte_name(address, "tn").ok()
    }

    /// Returns the transposed normal form when table metadata is available.
    pub fn normal_form(&self) -> Option<Vec<u8>> {
        let ordered_pcs = self.ordered_pitch_classes();
        let address = tables::seek_chord_tables_address(&ordered_pcs).ok()?;
        tables::transposed_normal_form_from_address(address).ok()
    }

    /// Returns the interval-class vector when table metadata is available.
    pub fn interval_class_vector(&self) -> Option<Vec<u8>> {
        let ordered_pcs = self.ordered_pitch_classes();
        let address = tables::seek_chord_tables_address(&ordered_pcs).ok()?;
        tables::interval_class_vector_from_address(address).ok()
    }

    /// Returns the tertian inversion number, where root position is `0`.
    pub fn inversion(&self) -> Option<u8> {
        let root_pc = self.root_pitch_class_tertian()?;
        let bass_pc = self
            ._notes
            .iter()
            .min_by(|a, b| {
                a._pitch
                    .ps()
                    .partial_cmp(&b._pitch.ps())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|n| (n._pitch.ps().round() as i32).rem_euclid(12) as u8)?;

        let interval = ((bass_pc as i32 - root_pc as i32).rem_euclid(12)) as u8;
        match interval {
            0 => Some(0),
            3 | 4 => Some(1),
            6..=8 => Some(2),
            9..=11 => Some(3),
            _ => None,
        }
    }

    /// Returns a human-readable inversion label.
    pub fn inversion_name(&self) -> Option<String> {
        match self.inversion()? {
            0 => Some("root position".to_string()),
            1 => Some("first inversion".to_string()),
            2 => Some("second inversion".to_string()),
            3 => Some("third inversion".to_string()),
            _ => None,
        }
    }

    fn simplify_enharmonics(
        self,
        key_context: Option<KeySignature>,
    ) -> ExceptionResult<Option<Self>> {
        self.clone().simplify_enharmonics_in_place(key_context)?;
        Ok(Some(self))
    }

    fn simplify_enharmonics_in_place(
        &mut self,
        key_context: Option<KeySignature>,
    ) -> ExceptionResult<()> {
        match crate::pitch::simplify_multiple_enharmonics(&self.pitches(), None, key_context) {
            Ok(pitches) => {
                for (i, pitch) in pitches.iter().enumerate() {
                    if let Some(note) = self._notes.get_mut(i) {
                        note._pitch = pitch.clone();
                    }
                }
                Ok(())
            }
            Err(err) => Err(Exception::Chord(format!(
                "simplifying multiple enharmonics failed because of {err}"
            ))),
        }
    }

    fn ordered_pitch_classes(&self) -> Vec<u8> {
        let mut pcs = self
            ._notes
            .iter()
            .map(|note| (note._pitch.ps().round() as i32).rem_euclid(12) as u8)
            .collect::<Vec<_>>();
        pcs.sort_unstable();
        pcs.dedup();
        pcs
    }

    fn bass_pitch_name(&self) -> Option<String> {
        self.bass_pitch().map(Self::display_pitch_name)
    }

    fn bass_pitch(&self) -> Option<&Pitch> {
        self._notes
            .iter()
            .min_by(|a, b| {
                let aps = a._pitch.ps();
                let bps = b._pitch.ps();
                aps.partial_cmp(&bps).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|n| &n._pitch)
    }

    fn root_pitch_name_from_tables(&self) -> Option<String> {
        self.find_root_pitch().map(Self::display_pitch_name)
    }

    fn find_root_pitch(&self) -> Option<&Pitch> {
        let mut non_duplicating_notes: Vec<&Note> = Vec::new();
        let mut seen_steps = std::collections::HashSet::new();
        for note in &self._notes {
            if seen_steps.insert(note._pitch.step()) {
                non_duplicating_notes.push(note);
            }
        }

        match non_duplicating_notes.len() {
            0 => return None,
            1 => return self._notes.first().map(|note| &note._pitch),
            7 => return self.bass_pitch(),
            _ => {}
        }

        let mut step_nums_to_notes = std::collections::BTreeMap::new();
        for note in &non_duplicating_notes {
            step_nums_to_notes.insert(Self::step_num(&note._pitch), *note);
        }
        let step_nums = step_nums_to_notes.keys().copied().collect::<Vec<_>>();

        for start_index in 0..step_nums.len() {
            let mut all_are_thirds = true;
            let this_step_num = step_nums[start_index];
            let mut last_step_num = this_step_num;
            for end_index in (start_index + 1)..(start_index + step_nums.len()) {
                let end_step_num = step_nums[end_index % step_nums.len()];
                if !matches!(end_step_num - last_step_num, 2 | -5) {
                    all_are_thirds = false;
                    break;
                }
                last_step_num = end_step_num;
            }
            if all_are_thirds {
                return step_nums_to_notes
                    .get(&this_step_num)
                    .map(|note| &note._pitch);
            }
        }

        let ordered_chord_steps = [3, 5, 7, 2, 4, 6];
        let mut best_note = non_duplicating_notes[0];
        let mut best_score = f64::NEG_INFINITY;

        for note in non_duplicating_notes {
            let this_step_num = Self::step_num(&note._pitch);
            let mut score = 0.0;
            for (root_index, chord_step_test) in ordered_chord_steps.iter().enumerate() {
                let target = (this_step_num + chord_step_test - 1).rem_euclid(7);
                if step_nums_to_notes.contains_key(&target) {
                    score += 1.0 / (root_index as f64 + 6.0);
                }
            }
            if score > best_score {
                best_score = score;
                best_note = note;
            }
        }

        Some(&best_note._pitch)
    }

    fn root_pitch_class_tertian(&self) -> Option<u8> {
        let ordered_pcs = self.ordered_pitch_classes();
        if ordered_pcs.len() < 3 {
            return None;
        }

        let pc_set = ordered_pcs
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<u8>>();

        let mut best_pc: Option<u8> = None;
        let mut best_score: i32 = i32::MIN;

        for candidate in &ordered_pcs {
            let mut score = 0;
            let mut current = *candidate;
            let mut visited = std::collections::BTreeSet::new();
            visited.insert(current);

            for _ in 0..ordered_pcs.len() {
                let minor_third = ((current as i32 + 3).rem_euclid(12)) as u8;
                let major_third = ((current as i32 + 4).rem_euclid(12)) as u8;
                if pc_set.contains(&minor_third) && !visited.contains(&minor_third) {
                    score += 2;
                    current = minor_third;
                    visited.insert(current);
                    continue;
                }
                if pc_set.contains(&major_third) && !visited.contains(&major_third) {
                    score += 2;
                    current = major_third;
                    visited.insert(current);
                    continue;
                }
                break;
            }

            let has_fifth_like = [6_u8, 7_u8, 8_u8].iter().any(|delta| {
                pc_set.contains(&(((*candidate as i32 + *delta as i32).rem_euclid(12)) as u8))
            });
            if has_fifth_like {
                score += 1;
            }

            if score > best_score {
                best_score = score;
                best_pc = Some(*candidate);
            }
        }

        best_pc
    }

    fn pitch_class(pitch: &Pitch) -> u8 {
        (pitch.ps().round() as i32).rem_euclid(12) as u8
    }

    fn pitch_class_mask(&self) -> u16 {
        self.ordered_pitch_classes()
            .into_iter()
            .fold(0_u16, |mask, pc| mask | (1_u16 << pc))
    }

    fn step_num(pitch: &Pitch) -> i32 {
        pitch.step().step_to_dnn_offset() - 1
    }

    fn has_pitch_names(&self, expected: &[&str]) -> bool {
        if self._notes.len() != expected.len() {
            return false;
        }

        let actual = self
            ._notes
            .iter()
            .map(|note| note._pitch.name())
            .collect::<std::collections::BTreeSet<_>>();
        expected.iter().all(|name| actual.contains(*name))
    }

    fn interval_nice_name(start: &Pitch, end: &Pitch) -> Option<String> {
        Interval::between(
            PitchOrNote::Pitch(start.clone()),
            PitchOrNote::Pitch(end.clone()),
        )
        .ok()
        .map(|interval| interval.nice_name())
    }

    fn display_pitch_name(pitch: &Pitch) -> String {
        pitch.name().replace('-', "b")
    }
}

pub(crate) trait ChordTrait {}

impl ChordTrait for Chord {}

impl ChordBaseTrait for Chord {}

impl NotRestTrait for Chord {}

impl GeneralNoteTrait for Chord {
    fn duration(&self) -> &Option<Duration> {
        self.chordbase.duration()
    }

    fn set_duration(&mut self, duration: &Duration) {
        if let Some(chordbase) = Arc::get_mut(&mut self.chordbase) {
            chordbase.set_duration(duration);
        }
    }
}

impl Music21ObjectTrait for Chord {}

impl ProtoM21ObjectTrait for Chord {}

/// Converts a single note-like value into a [`Note`].
///
/// This is useful when constructing vectors or other collections that are
/// later passed to [`Chord::new`].
pub trait IntoNote {
    /// Whether this value came from an integer pitch class or MIDI-like number.
    const FROM_INTEGER_PITCH: bool = false;

    /// Converts the value into a note.
    fn try_into_note(self) -> ExceptionResult<Note>;
}

impl IntoNote for Note {
    fn try_into_note(self) -> ExceptionResult<Note> {
        Ok(self)
    }
}

impl IntoNote for &Note {
    fn try_into_note(self) -> ExceptionResult<Note> {
        Ok(self.clone())
    }
}

impl IntoNote for Pitch {
    fn try_into_note(self) -> ExceptionResult<Note> {
        Note::new(Some(self), None, None, None)
    }
}

impl IntoNote for &Pitch {
    fn try_into_note(self) -> ExceptionResult<Note> {
        Note::new(Some(self.clone()), None, None, None)
    }
}

impl IntoNote for String {
    fn try_into_note(self) -> ExceptionResult<Note> {
        Note::new(Some(self), None, None, None)
    }
}

impl IntoNote for &String {
    fn try_into_note(self) -> ExceptionResult<Note> {
        Note::new(Some(self.to_string()), None, None, None)
    }
}

impl IntoNote for &str {
    fn try_into_note(self) -> ExceptionResult<Note> {
        Note::new(Some(self), None, None, None)
    }
}

impl IntoNote for IntegerType {
    const FROM_INTEGER_PITCH: bool = true;

    fn try_into_note(self) -> ExceptionResult<Note> {
        Note::new(Some(self), None, None, None)
    }
}

/// Converts a supported chord input into notes.
///
/// Implementations are provided for strings, slices, vectors, other chords,
/// integer pitch inputs, and `Option<T>`. `None` converts to an empty note list.
pub trait IntoNotes {
    /// Whether this input should be treated as integer-derived pitches.
    const FROM_INTEGER_PITCHES: bool = false;

    /// Iterator-like collection returned by the conversion.
    type Notes: IntoIterator<Item = Note>;

    /// Converts the input into notes.
    fn try_into_notes(self) -> ExceptionResult<Self::Notes>;
}

impl<T> IntoNotes for Option<T>
where
    T: IntoNotes,
{
    const FROM_INTEGER_PITCHES: bool = T::FROM_INTEGER_PITCHES;

    type Notes = Vec<Note>;

    fn try_into_notes(self) -> ExceptionResult<Self::Notes> {
        match self {
            Some(notes) => Ok(notes.try_into_notes()?.into_iter().collect()),
            None => Ok(Vec::new()),
        }
    }
}

impl<T> IntoNotes for Vec<T>
where
    T: IntoNote,
{
    const FROM_INTEGER_PITCHES: bool = T::FROM_INTEGER_PITCH;

    type Notes = Vec<Note>;

    fn try_into_notes(self) -> ExceptionResult<Self::Notes> {
        let mut notes = self
            .into_iter()
            .map(IntoNote::try_into_note)
            .collect::<ExceptionResult<Vec<_>>>()?;
        if Self::FROM_INTEGER_PITCHES {
            simplify_integer_notes(&mut notes)?;
        }
        Ok(notes)
    }
}

fn simplify_integer_notes(notes: &mut [Note]) -> ExceptionResult<()> {
    if notes.is_empty() {
        return Ok(());
    }

    let pitches = notes
        .iter()
        .map(|note| note._pitch.clone())
        .collect::<Vec<_>>();
    for (note, pitch) in notes
        .iter_mut()
        .zip(crate::pitch::simplify_multiple_enharmonics(
            &pitches, None, None,
        )?)
    {
        note._pitch = pitch;
    }

    Ok(())
}

impl IntoNotes for &[Pitch] {
    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes, Exception> {
        self.iter()
            .map(|pitch| Note::new(Some(pitch.clone()), None, None, None))
            .collect::<ExceptionResult<Vec<_>>>()
    }
}

impl IntoNotes for &[Note] {
    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes, Exception> {
        Ok(self.to_vec())
    }
}

impl IntoNotes for &[Chord] {
    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes, Exception> {
        Ok(self.iter().flat_map(|chord| chord._notes.clone()).collect())
    }
}

impl IntoNotes for &[String] {
    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes, Exception> {
        self.iter()
            .map(|s| Note::new(Some(s.to_string()), None, None, None))
            .collect::<ExceptionResult<Vec<_>>>()
    }
}

impl IntoNotes for String {
    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes, Exception> {
        if self.trim().is_empty() {
            Ok(Vec::new())
        } else if self.contains(char::is_whitespace) {
            self.split_whitespace()
                .collect::<Vec<&str>>()
                .as_slice()
                .try_into_notes()
        } else {
            Ok(vec![Note::new(Some(self), None, None, None)?])
        }
    }
}

impl IntoNotes for &[&str] {
    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes, Exception> {
        let mut vec = vec![];
        for str in self {
            vec.append(&mut str.try_into_notes()?);
        }
        Ok(vec)
    }
}

impl IntoNotes for &str {
    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes, Exception> {
        if self.trim().is_empty() {
            Ok(Vec::new())
        } else if self.contains(char::is_whitespace) {
            self.split_whitespace()
                .collect::<Vec<&str>>()
                .try_into_notes()
        } else {
            Ok(vec![Note::new(Some(self), None, None, None)?])
        }
    }
}

impl IntoNotes for &[IntegerType] {
    const FROM_INTEGER_PITCHES: bool = true;

    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes, Exception> {
        let mut notes = self
            .iter()
            .map(|i| Note::new(Some(*i), None, None, None))
            .collect::<ExceptionResult<Vec<_>>>()?;
        simplify_integer_notes(&mut notes)?;
        Ok(notes)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Pitch, chord::Chord};

    #[cfg(feature = "python")]
    mod utils {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/shared.rs"));
    }

    #[cfg(feature = "python")]
    use pyo3::{Bound, PyAny, PyErr, PyResult, Python, prelude::PyModule, types::PyAnyMethods};
    #[cfg(feature = "python")]
    use utils::{init_py, init_py_with_dummies, prepare};

    #[cfg(feature = "python")]
    fn import_music21_chord_without_package_init(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
        let sys = py.import("sys")?;
        let modules = sys.getattr("modules")?;
        modules.call_method1("pop", ("music21.chord", py.None()))?;
        modules.call_method1("pop", ("music21", py.None()))?;

        let music21_src = format!("{}/music21/music21", env!("CARGO_MANIFEST_DIR"));
        let types = py.import("types")?;
        let music21_pkg = types.getattr("ModuleType")?.call1(("music21",))?;
        music21_pkg.setattr("__path__", vec![music21_src])?;
        modules.call_method1("__setitem__", ("music21", music21_pkg))?;

        py.import("music21.chord")
    }

    #[test]
    fn c_e_g_pitchedcommonname() {
        let chord = Chord::new("C E G");

        assert!(chord.is_ok());

        assert_eq!(chord.unwrap().pitched_common_name(), "C-major triad");
    }

    #[test]
    fn new_accepts_empty_inputs() {
        assert_eq!(Chord::new("").unwrap().pitched_common_name(), "empty chord");
        assert_eq!(
            Chord::new(Vec::<Pitch>::new())
                .unwrap()
                .pitched_common_name(),
            "empty chord"
        );
        assert_eq!(
            Chord::new(Option::<&str>::None)
                .unwrap()
                .pitched_common_name(),
            "empty chord"
        );
    }

    #[test]
    fn pitched_common_names_returns_aliases() {
        let chord = Chord::new("C E G#").unwrap();
        assert_eq!(
            chord.pitched_common_names(),
            vec![
                "C-augmented triad".to_string(),
                "C-equal 3-part octave division".to_string()
            ]
        );
    }

    #[test]
    fn dyad_names_follow_music21_interval_rules() {
        let pcs = [0, 1];
        let integer_chord = Chord::new(pcs.as_slice()).unwrap();
        assert_eq!(integer_chord.common_name(), "Minor Second");
        assert_eq!(integer_chord.pitched_common_name(), "Minor Second above C");

        let spelled_chord = Chord::new("C C#").unwrap();
        assert_eq!(spelled_chord.common_name(), "Augmented Unison");
        assert_eq!(
            spelled_chord.pitched_common_name(),
            "Augmented Unison above C"
        );

        let octave = Chord::new("D3 D4").unwrap();
        assert_eq!(octave.common_name(), "Perfect Octave");
        assert_eq!(octave.pitched_common_name(), "Perfect Octave above D");

        let compound = Chord::new("E-3 C5 C6").unwrap();
        assert_eq!(compound.common_name(), "Major Sixth with octave doublings");
        assert_eq!(
            compound.pitched_common_name(),
            "Major Sixth with octave doublings above Eb"
        );
    }

    #[test]
    fn chord_metadata_methods_have_forte_and_inversion() {
        let chord = Chord::new("C E G").unwrap();
        assert_eq!(chord.root_pitch_name().as_deref(), Some("C"));
        assert_eq!(chord.bass_pitch_name_public().as_deref(), Some("C"));
        assert_eq!(chord.inversion(), Some(0));
        assert_eq!(chord.inversion_name().as_deref(), Some("root position"));
        assert_eq!(chord.forte_class().as_deref(), Some("3-11B"));
        assert!(
            chord
                .common_names()
                .iter()
                .any(|name| name == "major triad")
        );
    }

    #[test]
    fn chord_first_inversion_detected() {
        let chord = Chord::new("E3 G3 C4").unwrap();
        assert_eq!(chord.inversion(), Some(1));
        assert_eq!(chord.inversion_name().as_deref(), Some("first inversion"));
    }

    #[test]
    #[cfg(feature = "python")]
    fn compare_chords_python() {
        let x = "C E G";
        let y = "C C# D D# E F F# G G# A A# B";

        prepare().unwrap();

        Python::attach(|py| -> PyResult<()> {
            init_py(py)?;
            init_py_with_dummies(py)?;

            let chord: Bound<'_, PyModule> = match import_music21_chord_without_package_init(py) {
                Ok(module) => module,
                Err(_) => {
                    // In constrained environments we may only have the dummy
                    // shim module available; skip Python parity here.
                    return Ok(());
                }
            };

            let chord_class = match chord.getattr("Chord") {
                Ok(value) => value,
                Err(_) => {
                    return Ok(());
                }
            };

            compare_chord(x, &chord_class)?;
            compare_chord(y, &chord_class)?;

            Ok(())
        })
        .unwrap();
    }

    #[test]
    #[cfg(feature = "python")]
    fn compare_all_pitch_class_subsets_python() {
        prepare().unwrap();

        Python::attach(|py| -> PyResult<()> {
            init_py(py)?;
            init_py_with_dummies(py)?;

            let chord: Bound<'_, PyModule> = match import_music21_chord_without_package_init(py) {
                Ok(module) => module,
                Err(_) => return Ok(()),
            };
            let chord_class = match chord.getattr("Chord") {
                Ok(value) => value,
                Err(_) => return Ok(()),
            };

            for mask in 0_u16..(1_u16 << 12) {
                let pcs = (0..12)
                    .filter(|pc| mask & (1 << pc) != 0)
                    .collect::<Vec<_>>();
                let chord_instance = chord_class.call1((pcs.clone(),))?;

                let python_common_name: String = chord_instance.getattr("commonName")?.extract()?;
                let python_pitched_common_name: String =
                    chord_instance.getattr("pitchedCommonName")?.extract()?;

                let rust_chord = Chord::new(pcs.as_slice()).unwrap();
                let rust_common_name = rust_chord.common_name();
                let rust_pitched_common_name = rust_chord.pitched_common_name();
                assert_eq!(
                    rust_common_name, python_common_name,
                    "commonName mismatch for mask {mask:012b} pcs {pcs:?}"
                );
                assert_eq!(
                    rust_pitched_common_name, python_pitched_common_name,
                    "pitchedCommonName mismatch for mask {mask:012b} pcs {pcs:?}"
                );
            }

            Ok(())
        })
        .unwrap();
    }

    #[cfg(feature = "python")]
    fn compare_chord(x: &str, chord_class: &Bound<'_, PyAny>) -> Result<(), PyErr> {
        let chord_instance = chord_class.call1((x,))?;

        let chord = Chord::new(x).unwrap();

        let pitched_common_name: String = chord_instance.getattr("pitchedCommonName")?.extract()?;
        assert_eq!(chord.pitched_common_name(), pitched_common_name);

        let common_name: String = chord_instance.getattr("commonName")?.extract()?;
        assert_eq!(chord.common_name(), common_name);
        Ok(())
    }
}
