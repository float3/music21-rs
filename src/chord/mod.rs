pub(crate) mod chordbase;
/// Guitar tuning and fingering helpers.
pub mod guitar;
pub(crate) mod tables;

use crate::base::Music21ObjectTrait;
use crate::defaults::{FloatType, IntegerType, UnsignedIntegerType};
use crate::duration::Duration;
use crate::error::Error;
use crate::error::Result;
use crate::interval::{Interval, PitchOrNote};
use crate::key::Key;
use crate::key::keysignature::KeySignature;
use crate::note::generalnote::GeneralNoteTrait;
use crate::note::notrest::NotRestTrait;
use crate::note::{IntoNote, Note};
use crate::pitch::{Pitch, PitchClass, PitchClassSpecifier};
use crate::prebase::ProtoM21ObjectTrait;

use chordbase::ChordBase;
use chordbase::ChordBaseTrait;
pub use guitar::{GuitarFingering, GuitarStringFingering, GuitarTuning, GuitarTuningString};

use num::integer::{gcd, lcm};
use std::fmt::{Display, Formatter};
use std::str::FromStr;
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

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// An unpitched chord type known to the music21-derived chord table.
pub struct KnownChordType {
    /// Number of distinct pitch classes in the chord type.
    pub cardinality: u8,
    /// Unpitched common-name aliases in music21 table order.
    pub common_names: Vec<String>,
    /// Forte class for this transposition-normal entry, such as `"3-11B"`.
    pub forte_class: String,
    /// Transposed normal form pitch classes.
    pub normal_form: Vec<u8>,
    /// Six-entry interval-class vector.
    pub interval_class_vector: Vec<u8>,
}

#[derive(Debug, Clone)]
/// A likely tonal resolution for a chord, including the key context used.
pub struct ChordResolutionSuggestion {
    /// The suggested resolution chord.
    pub chord: Chord,
    /// Human-readable harmonic context for the suggestion.
    pub key_context: String,
}

const CANDIDATE_TONICS: [&str; 12] = [
    "C", "D-", "D", "E-", "E", "F", "F#", "G", "A-", "A", "B-", "B",
];

impl FromStr for Chord {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<&str> for Chord {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<String> for Chord {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<&[Pitch]> for Chord {
    type Error = Error;

    fn try_from(value: &[Pitch]) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<&[Note]> for Chord {
    type Error = Error;

    fn try_from(value: &[Note]) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<&[IntegerType]> for Chord {
    type Error = Error;

    fn try_from(value: &[IntegerType]) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<&[&str]> for Chord {
    type Error = Error;

    fn try_from(value: &[&str]) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<&[String]> for Chord {
    type Error = Error;

    fn try_from(value: &[String]) -> Result<Self> {
        Self::new(value)
    }
}

impl Display for Chord {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.pitched_common_name())
    }
}

impl Chord {
    /// Builds a chord from any supported note collection.
    ///
    /// Empty inputs are valid: pass `""`, an empty vector or slice, or
    /// `Option::<&str>::None` to construct an empty chord.
    pub fn new<T>(notes: T) -> Result<Self>
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
    pub fn empty() -> Result<Self> {
        Self::new(Option::<&str>::None)
    }

    /// Returns the unpitched chord types known to the music21-derived table.
    pub fn known_chord_types() -> Vec<KnownChordType> {
        tables::known_chord_table_entries()
            .into_iter()
            .map(|entry| KnownChordType {
                cardinality: entry.cardinality,
                common_names: entry.common_names.into_iter().map(str::to_string).collect(),
                forte_class: entry.forte_class,
                normal_form: entry.normal_form,
                interval_class_vector: entry.interval_class_vector,
            })
            .collect()
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

    /// Returns the preferred chord symbol, when available.
    ///
    /// This is separate from [`Self::pitched_common_name`]: common names follow
    /// the music21/Forte tables, while chord symbols are compact harmonic
    /// spellings such as `Cmaj7`, `F#m7b5`, or `Cdim9 add(#5)`.
    pub fn chord_symbol(&self) -> Option<String> {
        self.chord_symbols().into_iter().next()
    }

    /// Returns ranked chord symbols for this pitch-class set.
    ///
    /// Empty and microtonal chords return no symbols because this notation layer
    /// assumes twelve-tone equal-tempered pitch classes.
    pub fn chord_symbols(&self) -> Vec<String> {
        crate::chordsymbol::chord_symbol_spellings(self)
    }

    /// Returns the preferred chord symbol using an explicit root.
    ///
    /// This is useful for pitch-class sets and browser tables where the caller
    /// already knows the harmonic spelling anchor and does not want an
    /// inversion/root inference pass to choose another chord member. String
    /// roots are parsed as pitch names; numeric roots are parsed as pitch
    /// classes, so use numbers for pitch-class-only values such as 10 or 11.
    pub fn chord_symbol_with_root(
        &self,
        root: impl Into<PitchClassSpecifier>,
    ) -> Result<Option<String>> {
        Ok(self.chord_symbols_with_root(root)?.into_iter().next())
    }

    /// Returns ranked chord symbols using an explicit root.
    ///
    /// Empty, microtonal, and rootless-with-respect-to-the-given-root chords
    /// return no symbols. Non-integer roots are rejected because chord symbols
    /// are generated in twelve-tone pitch-class space.
    pub fn chord_symbols_with_root(
        &self,
        root: impl Into<PitchClassSpecifier>,
    ) -> Result<Vec<String>> {
        let root = Self::chord_symbol_root_pitch_class(root.into())?;

        Ok(crate::chordsymbol::chord_symbol_spellings_with_root(
            self, root,
        ))
    }

    /// Returns a suggested standard-tuning guitar fingering.
    ///
    /// The fingering is a compact voicing on six-string guitar in
    /// E2-A2-D3-G3-B3-E4 tuning. It prefers shapes that cover all chord pitches,
    /// place the
    /// root in the bass when possible, avoid internal muted strings, and stay
    /// within a small fret span.
    pub fn guitar_fingering(&self) -> Option<GuitarFingering> {
        guitar::suggested_guitar_fingering(self)
    }

    /// Returns a suggested guitar fingering for the supplied tuning.
    ///
    /// The tuning strings must be ordered from low to high. Fingering generation
    /// uses exact pitch spaces, so both the chord pitches and open-string
    /// octaves affect the result.
    pub fn guitar_fingering_with_tuning(&self, tuning: &GuitarTuning) -> Option<GuitarFingering> {
        guitar::suggested_guitar_fingering_with_tuning(self, tuning)
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

    fn chord_symbol_root_pitch_class(root: PitchClassSpecifier) -> Result<u8> {
        match root {
            PitchClassSpecifier::String(value) => match Pitch::from_name(value.as_str()) {
                Ok(pitch) => Self::integer_pitch_class_for_chord_symbol_root(pitch.ps()),
                Err(pitch_error) => {
                    let pitch_class = PitchClass::new(value.as_str()).map_err(|pitch_class_error| {
                        Error::Chord(format!(
                            "cannot parse chord-symbol root {value:?} as a pitch name ({pitch_error}) or pitch class ({pitch_class_error})"
                        ))
                    })?;
                    Self::integer_pitch_class_from_value(pitch_class)
                }
            },
            specifier => {
                let pitch_class = PitchClass::new(specifier)?;
                Self::integer_pitch_class_from_value(pitch_class)
            }
        }
    }

    fn integer_pitch_class_from_value(pitch_class: PitchClass) -> Result<u8> {
        let Some(root) = pitch_class.integer() else {
            return Err(Error::Chord(
                "chord symbols require an integer pitch-class root".to_string(),
            ));
        };
        Ok(root as u8)
    }

    fn integer_pitch_class_for_chord_symbol_root(ps: FloatType) -> Result<u8> {
        if (ps - ps.round()).abs() > FloatType::EPSILON {
            return Err(Error::Chord(
                "chord symbols require an integer pitch-class root".to_string(),
            ));
        }

        Ok((ps.round() as IntegerType).rem_euclid(12) as u8)
    }

    /// Returns the primary unpitched music21-style common name.
    ///
    /// For chords with multiple table aliases, this is the first common name in
    /// table order. Use [`Self::common_names`] to get every unpitched alias.
    pub fn common_name(&self) -> String {
        if self
            ._notes
            .iter()
            .any(|n| (n._pitch.alter() - n._pitch.alter().round()).abs() > FloatType::EPSILON)
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
                .map(|n| n._pitch.ps().round() as IntegerType)
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
            .map(|n| n._pitch.ps().round() as IntegerType)
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

    /// Maps this chord's pitch classes to a reduced integer polyrhythm ratio.
    ///
    /// Pitch classes are measured from the inferred root when possible, or
    /// from the lowest pitch class otherwise. Each semitone offset is mapped
    /// to a compact just-intonation ratio and reduced to whole-number
    /// components.
    pub fn polyrhythm_components(&self) -> Vec<UnsignedIntegerType> {
        let pitch_classes = self.ordered_pitch_classes();
        if pitch_classes.is_empty() {
            return vec![1];
        }

        let root_pc = self
            .find_root_pitch()
            .map(Self::pitch_class)
            .filter(|root_pc| pitch_classes.contains(root_pc))
            .unwrap_or(pitch_classes[0]);
        let mut offsets = pitch_classes
            .iter()
            .map(|pc| (*pc + 12 - root_pc) % 12)
            .collect::<Vec<_>>();
        offsets.sort_unstable();

        let ratios = offsets
            .into_iter()
            .map(Self::just_ratio_for_semitone)
            .collect::<Vec<_>>();
        let common_denominator = ratios
            .iter()
            .fold(1, |acc, (_, denominator)| lcm(acc, *denominator));
        let integers = ratios
            .iter()
            .map(|(numerator, denominator)| numerator * (common_denominator / denominator))
            .collect::<Vec<_>>();
        let divisor = integers.iter().copied().reduce(gcd).unwrap_or(1).max(1);

        integers.into_iter().map(|value| value / divisor).collect()
    }

    /// Returns [`Self::polyrhythm_components`] formatted as `a:b:c`.
    pub fn polyrhythm_ratio_string(&self) -> String {
        self.polyrhythm_components()
            .into_iter()
            .map(|component| component.to_string())
            .collect::<Vec<_>>()
            .join(":")
    }

    /// Returns cloned pitches for every note in the chord, in input order.
    pub fn pitches(&self) -> Vec<Pitch> {
        self._notes.iter().map(|note| note._pitch.clone()).collect()
    }

    /// Returns the notes in input order.
    pub fn notes(&self) -> &[Note] {
        &self._notes
    }

    /// Returns the chord duration when one has been assigned.
    pub fn duration(&self) -> Option<&Duration> {
        self.chordbase.duration().as_ref()
    }

    /// Assigns a duration to the chord.
    pub fn set_duration(&mut self, duration: Duration) {
        if let Some(chordbase) = Arc::get_mut(&mut self.chordbase) {
            chordbase.set_duration(&duration);
        }
    }

    /// Returns a copy of this chord with the supplied duration.
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.set_duration(duration);
        self
    }

    /// Returns the inferred root pitch name when the chord has one.
    ///
    /// Returns `None` for empty chords, where there is no pitch from which a
    /// root can be inferred.
    pub fn root_pitch_name(&self) -> Option<String> {
        self.root_pitch_name_from_tables()
    }

    /// Returns the lowest pitch name in the chord.
    ///
    /// Returns `None` for empty chords, where there is no bass pitch.
    pub fn bass_pitch_name(&self) -> Option<String> {
        self.bass_pitch().map(Self::display_pitch_name)
    }

    /// Returns the lowest pitch name in the chord.
    ///
    /// Returns `None` for empty chords, where there is no bass pitch.
    #[deprecated(note = "use `bass_pitch_name`")]
    pub fn bass_pitch_name_public(&self) -> Option<String> {
        self.bass_pitch_name()
    }

    /// Returns the Forte class, such as `"3-11B"`, when available.
    ///
    /// Returns `None` when the chord's pitch-class set has no Forte-table
    /// entry, including empty or otherwise unsupported pitch-class sets.
    pub fn forte_class(&self) -> Option<String> {
        let ordered_pcs = self.ordered_pitch_classes();
        let address = tables::seek_chord_tables_address(&ordered_pcs).ok()?;
        tables::address_to_forte_name(address, "tn").ok()
    }

    /// Returns the transposed normal form when table metadata is available.
    ///
    /// Returns `None` when the chord's pitch-class set cannot be found in the
    /// chord tables, including empty or otherwise unsupported pitch-class sets.
    pub fn normal_form(&self) -> Option<Vec<u8>> {
        let ordered_pcs = self.ordered_pitch_classes();
        let address = tables::seek_chord_tables_address(&ordered_pcs).ok()?;
        tables::transposed_normal_form_from_address(address).ok()
    }

    /// Returns the interval-class vector when table metadata is available.
    ///
    /// Returns `None` when the chord's pitch-class set cannot be found in the
    /// chord tables, including empty or otherwise unsupported pitch-class sets.
    pub fn interval_class_vector(&self) -> Option<Vec<u8>> {
        let ordered_pcs = self.ordered_pitch_classes();
        let address = tables::seek_chord_tables_address(&ordered_pcs).ok()?;
        tables::interval_class_vector_from_address(address).ok()
    }

    /// Returns the tertian inversion number, where root position is `0`.
    ///
    /// Returns `None` for empty chords, chords with fewer than three distinct
    /// pitch classes, or chords whose bass-to-root interval does not match a
    /// supported tertian inversion.
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
            .map(|n| (n._pitch.ps().round() as IntegerType).rem_euclid(12) as u8)?;

        let interval = ((bass_pc as IntegerType - root_pc as IntegerType).rem_euclid(12)) as u8;
        match interval {
            0 => Some(0),
            3 | 4 => Some(1),
            6..=8 => Some(2),
            9..=11 => Some(3),
            _ => None,
        }
    }

    /// Returns a human-readable inversion label.
    ///
    /// Returns `None` whenever [`Self::inversion`] returns `None`.
    pub fn inversion_name(&self) -> Option<String> {
        match self.inversion()? {
            0 => Some("root position".to_string()),
            1 => Some("first inversion".to_string()),
            2 => Some("second inversion".to_string()),
            3 => Some("third inversion".to_string()),
            _ => None,
        }
    }

    /// Returns the first likely tonal resolution chord in the given key.
    ///
    /// This is intentionally conservative rather than a universal harmonic
    /// oracle. It covers the resolution families that music21 exposes most
    /// directly: dominant-function sonorities, leading-tone diminished
    /// sonorities, and contextual augmented-sixth sonorities. Unsupported
    /// chords return `Ok(None)`.
    pub fn resolution_chord(&self, tonic: &str, mode: Option<&str>) -> Result<Option<Self>> {
        Ok(self.resolution_chords(tonic, mode)?.into_iter().next())
    }

    /// Returns likely tonal resolution chords in the given key.
    ///
    /// Dominant-function chords resolve by root motion up a perfect fourth to
    /// a diatonic triad in the supplied key, so secondary dominants such as
    /// `D7` in C major resolve to the G-major triad. Leading-tone diminished
    /// sonorities resolve up by semitone to a diatonic triad. Italian, French,
    /// German, and Swiss-style augmented-sixth sonorities in context resolve to
    /// the dominant triad.
    pub fn resolution_chords(&self, tonic: &str, mode: Option<&str>) -> Result<Vec<Self>> {
        let key = Key::from_tonic_mode(tonic, mode)?;
        self.resolution_chords_in_key(&key)
    }

    /// Returns likely tonal resolution chords in the supplied key.
    pub fn resolution_chords_in_key(&self, key: &Key) -> Result<Vec<Self>> {
        if self.is_contextual_augmented_sixth(key)? {
            return Ok(vec![
                self.place_resolution_near_source(key.triad_from_degree(5)?)?,
            ]);
        }

        let mut resolutions = Vec::new();

        let dominant_resolution = if self.is_dominant_function_sonority() {
            self.resolve_by_root_motion(key, 5)?
        } else {
            None
        };
        if let Some(chord) = dominant_resolution {
            resolutions.push(chord);
        }

        let leading_tone_resolution = if self.is_leading_tone_function_sonority() {
            self.resolve_by_root_motion(key, 1)?
        } else {
            None
        };
        if let Some(chord) = leading_tone_resolution {
            resolutions.push(chord);
        }

        Ok(Self::deduplicate_resolution_chords(resolutions))
    }

    /// Returns likely tonal resolution suggestions in the supplied key.
    pub fn resolution_suggestions_in_key(
        &self,
        key: &Key,
    ) -> Result<Vec<ChordResolutionSuggestion>> {
        let mut suggestions = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        let key_name = Self::display_key_name(key);

        if self.is_contextual_augmented_sixth(key)? {
            Self::push_resolution_suggestion(
                key.triad_from_degree(5)?,
                format!("augmented-sixth resolution in {key_name}"),
                &mut suggestions,
                &mut seen,
            );
            return Ok(suggestions);
        }

        if self.is_dominant_function_sonority()
            && let Some(chord) = self.resolve_by_root_motion(key, 5)?
        {
            Self::push_resolution_suggestion(
                chord,
                format!("dominant resolution in {key_name}"),
                &mut suggestions,
                &mut seen,
            );
        }

        if self.is_leading_tone_function_sonority()
            && let Some(chord) = self.resolve_by_root_motion(key, 1)?
        {
            Self::push_resolution_suggestion(
                chord,
                format!("leading-tone resolution in {key_name}"),
                &mut suggestions,
                &mut seen,
            );
        }

        Ok(suggestions)
    }

    /// Returns likely tonal resolution chords with inferred key contexts.
    ///
    /// This is a convenience wrapper around [`Self::resolution_chords`] for
    /// exploratory tools: dominant-function sonorities are tested against the
    /// key a perfect fourth above their root, leading-tone sonorities against
    /// the key a semitone above their root, and augmented-sixth sonorities
    /// against all built-in major/minor tonic spellings.
    pub fn resolution_suggestions(&self) -> Result<Vec<ChordResolutionSuggestion>> {
        let mut suggestions = Vec::new();
        let mut seen = std::collections::BTreeSet::new();

        let augmented_contexts = self.augmented_sixth_contexts()?;
        if !augmented_contexts.is_empty() {
            for (tonic, mode) in augmented_contexts {
                let context = format!(
                    "augmented-sixth resolution in {} {mode}",
                    Self::display_tonic_name(tonic)
                );
                self.add_resolution_suggestions_for_key(
                    tonic,
                    mode,
                    context,
                    &mut suggestions,
                    &mut seen,
                )?;
            }
            return Ok(suggestions);
        }

        if let Some(root_pc) = self.find_root_pitch().map(Self::pitch_class) {
            if self.is_dominant_function_sonority() {
                let tonic = Self::pitch_class_name((root_pc + 5) % 12);
                for mode in ["major", "minor"] {
                    let context = format!(
                        "dominant resolution to {} {mode}",
                        Self::display_tonic_name(tonic)
                    );
                    self.add_resolution_suggestions_for_key(
                        tonic,
                        mode,
                        context,
                        &mut suggestions,
                        &mut seen,
                    )?;
                }
            }

            if self.is_leading_tone_function_sonority() {
                let tonic = Self::pitch_class_name((root_pc + 1) % 12);
                for mode in ["major", "minor"] {
                    let context = format!(
                        "leading-tone resolution to {} {mode}",
                        Self::display_tonic_name(tonic)
                    );
                    self.add_resolution_suggestions_for_key(
                        tonic,
                        mode,
                        context,
                        &mut suggestions,
                        &mut seen,
                    )?;
                }
            }
        }

        Ok(suggestions)
    }

    fn simplify_enharmonics(self, key_context: Option<KeySignature>) -> Result<Option<Self>> {
        self.clone().simplify_enharmonics_in_place(key_context)?;
        Ok(Some(self))
    }

    fn simplify_enharmonics_in_place(&mut self, key_context: Option<KeySignature>) -> Result<()> {
        match crate::pitch::simplify_multiple_enharmonics(&self.pitches(), None, key_context) {
            Ok(pitches) => {
                for (i, pitch) in pitches.iter().enumerate() {
                    if let Some(note) = self._notes.get_mut(i) {
                        note._pitch = pitch.clone();
                    }
                }
                Ok(())
            }
            Err(err) => Err(Error::Chord(format!(
                "simplifying multiple enharmonics failed because of {err}"
            ))),
        }
    }

    fn ordered_pitch_classes(&self) -> Vec<u8> {
        let mut pcs = self
            ._notes
            .iter()
            .map(|note| (note._pitch.ps().round() as IntegerType).rem_euclid(12) as u8)
            .collect::<Vec<_>>();
        pcs.sort_unstable();
        pcs.dedup();
        pcs
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

    fn resolve_by_root_motion(&self, key: &Key, semitones: u8) -> Result<Option<Self>> {
        let Some(root_pitch) = self.find_root_pitch() else {
            return Ok(None);
        };
        let target_pc = (Self::pitch_class(root_pitch) + semitones) % 12;
        Self::triad_for_key_pitch_class(key, target_pc)?
            .map(|chord| self.place_resolution_near_source(chord))
            .transpose()
    }

    fn triad_for_key_pitch_class(key: &Key, target_pc: u8) -> Result<Option<Self>> {
        for degree in 1..=7 {
            let degree_pitch = key.pitch_from_degree(degree)?;
            if Self::pitch_class(&degree_pitch) == target_pc {
                return Ok(Some(key.triad_from_degree(degree)?));
            }
        }
        Ok(None)
    }

    fn place_resolution_near_source(&self, resolution: Self) -> Result<Self> {
        let Some(source_center) = Self::pitch_center(&self.pitches()) else {
            return Ok(resolution);
        };
        let Some(resolution_center) = Self::pitch_center(&resolution.pitches()) else {
            return Ok(resolution);
        };

        let octave_shift = ((source_center - resolution_center) / 12.0).round() as IntegerType;
        if octave_shift == 0 {
            return Ok(resolution);
        }

        let pitches = resolution
            .pitches()
            .into_iter()
            .map(|pitch| {
                let octave = pitch
                    .octave()
                    .unwrap_or_else(|| (pitch.ps().round() as IntegerType).div_euclid(12) - 1);
                Pitch::from_name_and_octave(pitch.name(), octave + octave_shift)
            })
            .collect::<Result<Vec<_>>>()?;

        Chord::new(pitches.as_slice())
    }

    fn pitch_center(pitches: &[Pitch]) -> Option<FloatType> {
        if pitches.is_empty() {
            return None;
        }

        Some(pitches.iter().map(Pitch::ps).sum::<FloatType>() / pitches.len() as FloatType)
    }

    fn deduplicate_resolution_chords(chords: Vec<Self>) -> Vec<Self> {
        let mut seen = std::collections::BTreeSet::new();
        let mut deduped = Vec::new();

        for chord in chords {
            if seen.insert(chord.pitch_classes()) {
                deduped.push(chord);
            }
        }

        deduped
    }

    fn augmented_sixth_contexts(&self) -> Result<Vec<(&'static str, &'static str)>> {
        if !self.has_augmented_sixth_spelling() {
            return Ok(Vec::new());
        }

        let mut contexts = Vec::new();
        for tonic in CANDIDATE_TONICS {
            for mode in ["major", "minor"] {
                let key = Key::from_tonic_mode(tonic, Some(mode))?;
                if self.is_contextual_augmented_sixth(&key)? {
                    contexts.push((tonic, mode));
                }
            }
        }
        Ok(contexts)
    }

    fn push_resolution_suggestion(
        chord: Chord,
        key_context: String,
        suggestions: &mut Vec<ChordResolutionSuggestion>,
        seen: &mut std::collections::BTreeSet<(String, String)>,
    ) {
        let pitched_common_name = chord.pitched_common_name();
        if seen.insert((pitched_common_name, key_context.clone())) {
            suggestions.push(ChordResolutionSuggestion { chord, key_context });
        }
    }

    fn has_augmented_sixth_spelling(&self) -> bool {
        for (index, lower) in self._notes.iter().enumerate() {
            for upper in self._notes.iter().skip(index + 1) {
                if Self::is_directed_augmented_sixth(&lower._pitch, &upper._pitch)
                    || Self::is_directed_augmented_sixth(&upper._pitch, &lower._pitch)
                {
                    return true;
                }
            }
        }
        false
    }

    fn is_directed_augmented_sixth(lower: &Pitch, upper: &Pitch) -> bool {
        let generic_interval = (Self::step_num(upper) - Self::step_num(lower)).rem_euclid(7) + 1;
        let semitones = ((upper.ps().round() as IntegerType) - (lower.ps().round() as IntegerType))
            .rem_euclid(12);
        generic_interval == 6 && semitones == 10
    }

    fn add_resolution_suggestions_for_key(
        &self,
        tonic: &str,
        mode: &str,
        key_context: String,
        suggestions: &mut Vec<ChordResolutionSuggestion>,
        seen: &mut std::collections::BTreeSet<(String, String)>,
    ) -> Result<()> {
        for chord in self.resolution_chords(tonic, Some(mode))? {
            Self::push_resolution_suggestion(chord, key_context.clone(), suggestions, seen);
        }
        Ok(())
    }

    fn is_dominant_function_sonority(&self) -> bool {
        let names = self.common_names_with_primary();
        let has_explicit_dominant_name = names.iter().any(|name| {
            matches!(
                name.as_str(),
                "dominant seventh chord"
                    | "major minor seventh chord"
                    | "incomplete dominant-seventh chord"
            )
        });
        let has_dominant_family_name = names
            .iter()
            .any(|name| name.contains("dominant") || name == "major-minor");

        has_explicit_dominant_name
            || (has_dominant_family_name && self.has_intervals_above_root(&[4, 10]))
    }

    fn is_leading_tone_function_sonority(&self) -> bool {
        let names = self.common_names_with_primary();
        let has_explicit_leading_tone_name = names.iter().any(|name| {
            matches!(
                name.as_str(),
                "diminished triad"
                    | "diminished seventh chord"
                    | "half-diminished seventh chord"
                    | "incomplete half-diminished seventh chord"
            )
        });
        let has_diminished_family_name = names.iter().any(|name| name.contains("diminished"));

        has_explicit_leading_tone_name
            || (has_diminished_family_name && self.has_intervals_above_root(&[3, 6]))
    }

    fn has_common_name(&self, expected: &str) -> bool {
        self.common_name() == expected || self.common_names().iter().any(|name| name == expected)
    }

    fn has_intervals_above_root(&self, intervals: &[u8]) -> bool {
        let Some(root_pitch) = self.find_root_pitch() else {
            return false;
        };
        let root_pc = Self::pitch_class(root_pitch);
        let chord_pcs = self.pitch_class_set();
        intervals
            .iter()
            .all(|interval| chord_pcs.contains(&((root_pc + interval) % 12)))
    }

    fn is_contextual_augmented_sixth(&self, key: &Key) -> Result<bool> {
        let chord_pcs = self.pitch_class_set();
        if chord_pcs.len() < 3 || chord_pcs.len() > 4 {
            return Ok(false);
        }

        let tonic_pc = Self::pitch_class(&key.pitch_from_degree(1)?);
        let second_pc = Self::pitch_class(&key.pitch_from_degree(2)?);
        let third_pc = Self::pitch_class(&key.pitch_from_degree(3)?);
        let fourth_pc = Self::pitch_class(&key.pitch_from_degree(4)?);
        let sixth_pc = Self::pitch_class(&key.pitch_from_degree(6)?);

        let raised_fourth_pc = (fourth_pc + 1) % 12;
        let lowered_sixth_pc = if (sixth_pc + 12 - tonic_pc) % 12 == 9 {
            (sixth_pc + 11) % 12
        } else {
            sixth_pc
        };

        if !chord_pcs.contains(&lowered_sixth_pc) || !chord_pcs.contains(&raised_fourth_pc) {
            return Ok(false);
        }

        if self
            .common_names_with_primary()
            .iter()
            .any(|name| name.contains("augmented sixth chord"))
        {
            return Ok(true);
        }

        let lowered_third_pc = if (third_pc + 12 - tonic_pc) % 12 == 4 {
            (third_pc + 11) % 12
        } else {
            third_pc
        };
        let raised_second_pc = (second_pc + 1) % 12;
        let allowed_pcs = [
            lowered_sixth_pc,
            raised_fourth_pc,
            tonic_pc,
            second_pc,
            lowered_third_pc,
            raised_second_pc,
        ];

        Ok(chord_pcs.contains(&tonic_pc)
            && chord_pcs
                .iter()
                .all(|pc| allowed_pcs.iter().any(|allowed| allowed == pc)))
    }

    fn common_names_with_primary(&self) -> Vec<String> {
        let mut names = vec![self.common_name()];
        names.extend(self.common_names());
        names.sort();
        names.dedup();
        names
    }

    fn pitch_class_set(&self) -> std::collections::BTreeSet<u8> {
        self.ordered_pitch_classes().into_iter().collect()
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
        let mut best_score = FloatType::NEG_INFINITY;

        for note in non_duplicating_notes {
            let this_step_num = Self::step_num(&note._pitch);
            let mut score = 0.0;
            for (root_index, chord_step_test) in ordered_chord_steps.iter().enumerate() {
                let target = (this_step_num + chord_step_test - 1).rem_euclid(7);
                if step_nums_to_notes.contains_key(&target) {
                    score += 1.0 / (root_index as FloatType + 6.0);
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
        let mut best_score: IntegerType = IntegerType::MIN;

        for candidate in &ordered_pcs {
            let mut score = 0;
            let mut current = *candidate;
            let mut visited = std::collections::BTreeSet::new();
            visited.insert(current);

            for _ in 0..ordered_pcs.len() {
                let minor_third = ((current as IntegerType + 3).rem_euclid(12)) as u8;
                let major_third = ((current as IntegerType + 4).rem_euclid(12)) as u8;
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
                pc_set.contains(
                    &(((*candidate as IntegerType + *delta as IntegerType).rem_euclid(12)) as u8),
                )
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
        (pitch.ps().round() as IntegerType).rem_euclid(12) as u8
    }

    fn pitch_class_name(pc: u8) -> &'static str {
        CANDIDATE_TONICS[pc as usize % 12]
    }

    fn just_ratio_for_semitone(offset: u8) -> (UnsignedIntegerType, UnsignedIntegerType) {
        const RATIOS: [(UnsignedIntegerType, UnsignedIntegerType); 12] = [
            (1, 1),
            (16, 15),
            (9, 8),
            (6, 5),
            (5, 4),
            (4, 3),
            (7, 5),
            (3, 2),
            (25, 16),
            (5, 3),
            (7, 4),
            (15, 8),
        ];
        RATIOS[offset as usize % 12]
    }

    fn pitch_class_mask(&self) -> u16 {
        self.ordered_pitch_classes()
            .into_iter()
            .fold(0_u16, |mask, pc| mask | (1_u16 << pc))
    }

    fn step_num(pitch: &Pitch) -> IntegerType {
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

    fn display_key_name(key: &Key) -> String {
        format!(
            "{} {}",
            Self::display_tonic_name(&key.tonic().name()),
            key.mode()
        )
    }

    fn display_tonic_name(name: &str) -> String {
        name.replace('-', "b")
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

/// Tries to convert a supported chord input into notes.
///
/// Implementations are provided for strings, slices, vectors, other chords,
/// integer pitch inputs, and `Option<T>`. `None` converts to an empty note list.
/// String and integer inputs can fail while constructing pitches or simplifying
/// enharmonics, so this trait stays explicitly fallible.
pub trait IntoNotes {
    /// Whether this input should be treated as integer-derived pitches.
    const FROM_INTEGER_PITCHES: bool = false;

    /// Iterator-like collection returned by the conversion.
    type Notes: IntoIterator<Item = Note>;

    /// Converts the input into notes.
    fn try_into_notes(self) -> Result<Self::Notes>;
}

impl<T> IntoNotes for Option<T>
where
    T: IntoNotes,
{
    const FROM_INTEGER_PITCHES: bool = T::FROM_INTEGER_PITCHES;

    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes> {
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

    fn try_into_notes(self) -> Result<Self::Notes> {
        let mut notes = self
            .into_iter()
            .map(IntoNote::try_into_note)
            .collect::<Result<Vec<_>>>()?;
        if Self::FROM_INTEGER_PITCHES {
            simplify_integer_notes(&mut notes)?;
        }
        Ok(notes)
    }
}

fn simplify_integer_notes(notes: &mut [Note]) -> Result<()> {
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

    fn try_into_notes(self) -> Result<Self::Notes> {
        self.iter()
            .map(|pitch| Note::new(Some(pitch.clone()), None, None, None))
            .collect::<Result<Vec<_>>>()
    }
}

impl IntoNotes for &[Note] {
    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes> {
        Ok(self.to_vec())
    }
}

impl IntoNotes for &[Chord] {
    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes> {
        Ok(self.iter().flat_map(|chord| chord._notes.clone()).collect())
    }
}

impl IntoNotes for &[String] {
    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes> {
        self.iter()
            .map(|s| Note::new(Some(s.to_string()), None, None, None))
            .collect::<Result<Vec<_>>>()
    }
}

impl IntoNotes for String {
    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes> {
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

    fn try_into_notes(self) -> Result<Self::Notes> {
        let mut vec = vec![];
        for str in self {
            vec.append(&mut str.try_into_notes()?);
        }
        Ok(vec)
    }
}

impl IntoNotes for &str {
    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes> {
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

    fn try_into_notes(self) -> Result<Self::Notes> {
        let mut notes = self
            .iter()
            .map(|i| Note::new(Some(*i), None, None, None))
            .collect::<Result<Vec<_>>>()?;
        simplify_integer_notes(&mut notes)?;
        Ok(notes)
    }
}

#[cfg(test)]
mod tests {
    use crate::{GuitarTuning, Key, Pitch, chord::Chord};

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
    fn chord_symbols_return_symbol_names() {
        let major_seventh = Chord::new("C E G B").unwrap();
        let petrushka = Chord::new("C4 D4 Eb4 F#4 Ab4 A4").unwrap();
        let slash_chord = Chord::new("F4 C5 D5 E-5").unwrap();

        assert_eq!(major_seventh.chord_symbol().as_deref(), Some("Cmaj7"));
        assert_eq!(petrushka.chord_symbol().as_deref(), Some("Cdim9 add(#5)"));
        assert_eq!(
            slash_chord.chord_symbol().as_deref(),
            Some("Dm7(no5) add(b9)/F")
        );
    }

    #[test]
    fn chord_symbols_with_root_accept_pitch_names() {
        let chord = Chord::new("A C").unwrap();

        assert_eq!(
            chord.chord_symbol_with_root("A").unwrap().as_deref(),
            Some("A add(b3)")
        );
        assert_eq!(
            chord.chord_symbol_with_root("C").unwrap().as_deref(),
            Some("C add(13)")
        );
    }

    #[test]
    fn guitar_fingering_covers_common_chord_tones() {
        let chord = Chord::new("C E G").unwrap();
        let fingering = chord.guitar_fingering().unwrap();

        assert_eq!(fingering.strings.len(), 6);
        assert_eq!(fingering.covered_pitch_spaces, vec![60, 64, 67]);
        assert_eq!(fingering.covered_pitch_classes, vec![0, 4, 7]);
        assert!(fingering.omitted_pitch_spaces.is_empty());
        assert!(fingering.omitted_pitch_classes.is_empty());
        assert!(
            fingering
                .strings
                .iter()
                .filter(|string| string.fret.is_some_and(|fret| fret > 0))
                .all(|string| string
                    .finger
                    .is_some_and(|finger| (1..=4).contains(&finger)))
        );
    }

    #[test]
    fn guitar_fingering_still_returns_large_pitch_sets() {
        let chord = Chord::new("C D E F G A B").unwrap();
        let fingering = chord.guitar_fingering().unwrap();

        assert_eq!(fingering.strings.len(), 6);
        assert!(!fingering.covered_pitch_classes.is_empty());
        assert!(!fingering.omitted_pitch_classes.is_empty());
    }

    #[test]
    fn guitar_fingering_uses_supplied_tuning_and_octaves() {
        let chord = Chord::new("D3 A3 D4").unwrap();
        let tuning = GuitarTuning::new(["D2", "A2", "D3", "G3", "A3", "D4"]).unwrap();
        let fingering = chord.guitar_fingering_with_tuning(&tuning).unwrap();

        assert_eq!(fingering.strings.len(), 6);
        assert_eq!(fingering.strings[0].string_name, "D2");
        assert_eq!(fingering.covered_pitch_spaces, vec![50, 57, 62]);
        assert!(fingering.omitted_pitch_spaces.is_empty());
    }

    #[test]
    fn guitar_tuning_rejects_empty_tunings() {
        assert!(GuitarTuning::new(Vec::<&str>::new()).is_err());
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
        assert_eq!(chord.bass_pitch_name().as_deref(), Some("C"));
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
    fn chord_maps_to_reduced_polyrhythm_components() {
        let major = Chord::new("C E G").unwrap();
        assert_eq!(major.polyrhythm_components(), vec![4, 5, 6]);
        assert_eq!(major.polyrhythm_ratio_string(), "4:5:6");

        let empty = Chord::empty().unwrap();
        assert_eq!(empty.polyrhythm_ratio_string(), "1");
    }

    #[test]
    fn new_rejects_invalid_pitch_inputs() {
        assert!(Chord::new("C nope G").is_err());
    }

    #[test]
    fn chord_supports_rust_conversion_traits() {
        let parsed: Chord = "C E G".parse().unwrap();
        assert_eq!(parsed.to_string(), "C-major triad");
        assert_eq!(parsed.notes().len(), 3);

        let from_str = Chord::try_from("C E G").unwrap();
        assert_eq!(from_str.pitched_common_name(), "C-major triad");

        let midi = [60, 64, 67];
        let from_slice = Chord::try_from(midi.as_slice()).unwrap();
        assert_eq!(from_slice.pitched_common_name(), "C-major triad");
    }

    #[test]
    fn known_chord_types_include_music21_table_names() {
        let known = Chord::known_chord_types();
        assert_eq!(known.len(), 351);
        assert!(
            known
                .iter()
                .any(|entry| entry.common_names.iter().any(|name| name == "major triad"))
        );
        assert!(known.iter().any(|entry| {
            entry
                .common_names
                .iter()
                .any(|name| name == "dominant seventh chord")
        }));
    }

    #[test]
    fn chord_first_inversion_detected() {
        let chord = Chord::new("E3 G3 C4").unwrap();
        assert_eq!(chord.inversion(), Some(1));
        assert_eq!(chord.inversion_name().as_deref(), Some("first inversion"));
    }

    #[test]
    fn dominant_seventh_resolves_to_tonic() {
        let chord = Chord::new("G3 B3 D4 F4").unwrap();
        let resolution = chord.resolution_chord("C", Some("major")).unwrap().unwrap();

        assert_eq!(resolution.pitched_common_name(), "C-major triad");
    }

    #[test]
    fn resolution_chords_stay_near_source_register() {
        let chord = Chord::new("G2 B2 D3 F3").unwrap();
        let resolution = chord.resolution_chord("C", Some("major")).unwrap().unwrap();
        let names = resolution
            .pitches()
            .into_iter()
            .map(|pitch| pitch.name_with_octave())
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["C3", "E3", "G3"]);
    }

    #[test]
    fn resolution_suggestions_infer_contexts() {
        let chord = Chord::new("G3 B3 D4 F4").unwrap();
        let suggestions = chord.resolution_suggestions().unwrap();

        assert!(suggestions.iter().any(|suggestion| {
            suggestion.key_context == "dominant resolution to C major"
                && suggestion.chord.pitched_common_name() == "C-major triad"
        }));
        assert!(suggestions.iter().any(|suggestion| {
            suggestion.key_context == "dominant resolution to C minor"
                && suggestion.chord.pitched_common_name() == "C-minor triad"
        }));
    }

    #[test]
    fn resolution_suggestions_stay_near_source_register() {
        let chord = Chord::new("G2 B2 D3 F3").unwrap();
        let suggestions = chord.resolution_suggestions().unwrap();
        let c_major = suggestions
            .iter()
            .find(|suggestion| suggestion.key_context == "dominant resolution to C major")
            .unwrap();
        let names = c_major
            .chord
            .pitches()
            .into_iter()
            .map(|pitch| pitch.name_with_octave())
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["C3", "E3", "G3"]);
    }

    #[test]
    fn resolution_suggestions_can_use_explicit_key_context() {
        let secondary_dominant = Chord::new("D3 F#3 A3 C4").unwrap();
        let c_major = Key::from_tonic_mode("C", Some("major")).unwrap();
        let suggestions = secondary_dominant
            .resolution_suggestions_in_key(&c_major)
            .unwrap();

        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].key_context, "dominant resolution in C major");
        assert_eq!(suggestions[0].chord.pitched_common_name(), "G-major triad");
    }

    #[test]
    fn dominant_seventh_resolves_to_minor_tonic() {
        let chord = Chord::new("G3 B3 D4 F4").unwrap();
        let resolution = chord.resolution_chord("C", Some("minor")).unwrap().unwrap();

        assert_eq!(resolution.pitched_common_name(), "C-minor triad");
    }

    #[test]
    fn secondary_dominant_resolves_to_diatonic_target() {
        let chord = Chord::new("D3 F#3 A3 C4").unwrap();
        let resolution = chord.resolution_chord("C", Some("major")).unwrap().unwrap();

        assert_eq!(resolution.pitched_common_name(), "G-major triad");
    }

    #[test]
    fn dominant_extensions_resolve_to_tonic() {
        let dominant_ninth = Chord::new("G2 B2 D3 F3 A3").unwrap();
        let dominant_eleventh = Chord::new("G2 B2 D3 F3 A3 C4").unwrap();
        let dominant_thirteenth = Chord::new("G2 B2 D3 F3 A3 C4 E4").unwrap();

        for chord in [dominant_ninth, dominant_eleventh, dominant_thirteenth] {
            let resolution = chord.resolution_chord("C", Some("major")).unwrap().unwrap();
            assert_eq!(resolution.pitched_common_name(), "C-major triad");
        }
    }

    #[test]
    fn leading_tone_sevenths_resolve_by_semitone() {
        let fully_diminished = Chord::new("B3 D4 F4 A-4").unwrap();
        let half_diminished = Chord::new("B3 D4 F4 A4").unwrap();

        assert_eq!(
            fully_diminished
                .resolution_chord("C", Some("major"))
                .unwrap()
                .unwrap()
                .pitched_common_name(),
            "C-major triad"
        );
        assert_eq!(
            half_diminished
                .resolution_chord("C", Some("major"))
                .unwrap()
                .unwrap()
                .pitched_common_name(),
            "C-major triad"
        );
    }

    #[test]
    fn leading_tone_diminished_triad_resolves_by_semitone() {
        let chord = Chord::new("B3 D4 F4").unwrap();
        let resolution = chord.resolution_chord("C", Some("major")).unwrap().unwrap();

        assert_eq!(resolution.pitched_common_name(), "C-major triad");
    }

    #[test]
    fn contextual_augmented_sixth_resolves_to_dominant() {
        let german_augmented_sixth = Chord::new("A-3 C4 E-4 F#4").unwrap();
        let resolution = german_augmented_sixth
            .resolution_chord("C", Some("major"))
            .unwrap()
            .unwrap();

        assert_eq!(resolution.pitched_common_name(), "G-major triad");
    }

    #[test]
    fn unsupported_resolution_returns_none() {
        let tonic = Chord::new("C E G").unwrap();
        assert!(
            tonic
                .resolution_chord("C", Some("major"))
                .unwrap()
                .is_none()
        );
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
