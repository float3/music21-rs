/// Guitar tuning and fingering helpers.
pub mod guitar;
pub(crate) mod root;
pub(crate) mod tables;

use crate::defaults::{FloatType, IntegerType, UnsignedIntegerType};
use crate::duration::Duration;
use crate::error::Error;
use crate::error::Result;
use crate::interval::{Interval, PitchOrNote};
use crate::key::Key;
use crate::key::keysignature::KeySignature;
use crate::note::{IntoNote, Note};
use crate::pitch::{Pitch, PitchClass, PitchClassSpecifier};

pub use guitar::{GuitarFingering, GuitarStringFingering, GuitarTuning, GuitarTuningString};

use num::integer::{gcd, lcm};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A collection of notes analyzed as one vertical sonority.
///
/// `Chord` accepts several note-like inputs, including whitespace-separated
/// pitch names, slices of pitches or notes, MIDI pitch numbers, vectors, and
/// `None` for an empty chord.
pub struct Chord {
    notes: Vec<Note>,
    duration: Option<Duration>,
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
        T: IntoNotes,
    {
        Ok(Self {
            notes: notes.try_into_notes()?.into_iter().collect(),
            duration: None,
            from_integer_pitches: T::FROM_INTEGER_PITCHES,
        })
    }

    /// Builds an empty chord.
    pub fn empty() -> Self {
        Self {
            notes: Vec::new(),
            duration: None,
            from_integer_pitches: false,
        }
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
    /// the music21/Forte tables, while chord symbols use music21-style
    /// figures such as `Cmaj7`, `F#m7b5`, or `Ddom7dim5/CaddA,E-`.
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
                .notes
                .first()
                .map(|n| n.pitch.name())
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
            self.notes
                .first()
                .map(|n| Self::display_pitch_name(&n.pitch))
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
            .notes
            .iter()
            .any(|n| (n.pitch.alter() - n.pitch.alter().round()).abs() > FloatType::EPSILON)
        {
            return "microtonal chord".to_string();
        }

        if self.notes.is_empty() {
            return "empty chord".to_string();
        }

        let ordered_pcs = self.ordered_pitch_classes();
        if ordered_pcs.is_empty() {
            return "empty chord".to_string();
        }

        if ordered_pcs.len() == 1 {
            if self.notes.len() == 1 {
                return "note".to_string();
            }

            let pitch_names = self
                .notes
                .iter()
                .map(|n| n.pitch.name())
                .collect::<std::collections::BTreeSet<_>>();

            let pitch_pses = self
                .notes
                .iter()
                .map(|n| n.pitch.ps().round() as IntegerType)
                .collect::<std::collections::BTreeSet<_>>();

            if pitch_names.len() == 1 {
                if pitch_pses.len() == 1 {
                    return "unison".to_string();
                }
                if pitch_pses.len() == 2 {
                    return Self::interval_nice_name(&self.notes[0].pitch, &self.notes[1].pitch)
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
            .notes
            .iter()
            .map(|n| n.pitch.name())
            .collect::<std::collections::BTreeSet<_>>();

        let pitch_pses = self
            .notes
            .iter()
            .map(|n| n.pitch.ps().round() as IntegerType)
            .collect::<std::collections::BTreeSet<_>>();

        let Some(p0) = self.notes.first().map(|n| &n.pitch) else {
            return "empty chord".to_string();
        };
        let p0_pitch_class = root::pitch_class(p0);

        let Some(p1) = self
            .notes
            .iter()
            .skip(1)
            .find(|n| root::pitch_class(&n.pitch) != p0_pitch_class)
            .map(|n| &n.pitch)
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

        Self::interval_nice_name(&self.notes[0].pitch, &self.notes[1].pitch)
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
            .map(root::pitch_class)
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
        self.notes.iter().map(|note| note.pitch.clone()).collect()
    }

    /// Returns the notes in input order.
    pub fn notes(&self) -> &[Note] {
        &self.notes
    }

    /// Returns the chord duration when one has been assigned.
    pub fn duration(&self) -> Option<&Duration> {
        self.duration.as_ref()
    }

    /// Assigns a duration to the chord.
    pub fn set_duration(&mut self, duration: Duration) {
        self.duration = Some(duration);
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

    /// Returns Robert Morris's eight-entry invariance vector, when available.
    ///
    /// The values are taken from the same music21 Forte table as
    /// [`Self::forte_class`] and [`Self::interval_class_vector`].
    pub fn invariance_vector(&self) -> Option<Vec<u8>> {
        let ordered_pcs = self.ordered_pitch_classes();
        let address = tables::seek_chord_tables_address(&ordered_pcs).ok()?;
        tables::invariance_vector_from_address(address).ok()
    }

    /// Returns this chord's Z-related Forte class, when music21 records one.
    pub fn z_relation(&self) -> Option<String> {
        let ordered_pcs = self.ordered_pitch_classes();
        let address = tables::seek_chord_tables_address(&ordered_pcs).ok()?;
        tables::z_relation_from_address(address).ok().flatten()
    }

    /// Returns the tertian inversion number, where root position is `0`.
    ///
    /// Returns `None` for empty chords, chords with fewer than three distinct
    /// pitch classes, or chords whose bass-to-root interval does not match a
    /// supported tertian inversion.
    pub fn inversion(&self) -> Option<u8> {
        let root_pc = self.root_pitch_class_tertian()?;
        let bass_pc = self
            .notes
            .iter()
            .min_by(|a, b| {
                a.pitch
                    .ps()
                    .partial_cmp(&b.pitch.ps())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|n| (n.pitch.ps().round() as IntegerType).rem_euclid(12) as u8)?;

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

        if let Some(root_pc) = self.find_root_pitch().map(root::pitch_class) {
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

    /// Returns a copy with simplified enharmonic spellings.
    ///
    /// This mirrors music21's explicit enharmonic simplification workflow:
    /// construction stays side-effect free, and callers can request simpler
    /// spellings with an optional key-signature context.
    pub fn simplify_enharmonics(&self, key_context: Option<KeySignature>) -> Result<Self> {
        let mut chord = self.clone();
        chord.simplify_enharmonics_in_place(key_context)?;
        Ok(chord)
    }

    /// Simplifies this chord's pitch spellings in place.
    pub fn simplify_enharmonics_in_place(
        &mut self,
        key_context: Option<KeySignature>,
    ) -> Result<()> {
        match crate::pitch::simplify_multiple_enharmonics(&self.pitches(), None, key_context) {
            Ok(pitches) => {
                for (i, pitch) in pitches.iter().enumerate() {
                    if let Some(note) = self.notes.get_mut(i) {
                        note.pitch = pitch.clone();
                    }
                }
                Ok(())
            }
            Err(err) => Err(Error::Chord(format!(
                "simplifying multiple enharmonics failed because of {err}"
            ))),
        }
    }

    /// Returns the root, found the way music21's `Chord.root` finds it.
    pub fn root(&self) -> Option<&Pitch> {
        self.find_root_pitch()
    }

    /// Returns the lowest pitch.
    pub fn bass(&self) -> Option<&Pitch> {
        self.bass_pitch()
    }

    /// Returns the first pitch lying at the given chord step above the root,
    /// so `3` is the third and `7` the seventh. Steps of eight and above are
    /// folded down by an octave, so `9` finds a second.
    pub fn chord_step(&self, step: u8) -> Option<&Pitch> {
        self.chord_step_from(step, self.root()?)
    }

    /// Returns the third above the root, if the chord has one.
    pub fn third(&self) -> Option<&Pitch> {
        self.chord_step(3)
    }

    /// Returns the fifth above the root, if the chord has one.
    pub fn fifth(&self) -> Option<&Pitch> {
        self.chord_step(5)
    }

    /// Returns the seventh above the root, if the chord has one.
    pub fn seventh(&self) -> Option<&Pitch> {
        self.chord_step(7)
    }

    /// Returns the semitones from the root to the given chord step, within an
    /// octave, if the chord has that step.
    pub fn semitones_from_chord_step(&self, step: u8) -> Option<u8> {
        let root = self.root()?;
        let pitch = self.chord_step_from(step, root)?;
        Some(semitones_above(root, pitch))
    }

    /// Returns whether the chord has the given step spelled two different
    /// ways, such as both `E` and `E-` above `C`.
    pub fn has_repeated_chord_step(&self, step: u8) -> bool {
        let Some(root) = self.root() else {
            return false;
        };
        let step = fold_chord_step(step);
        let Some(first) = self
            .chord_step_from(step, root)
            .map(|pitch| semitones_above(root, pitch))
        else {
            return false;
        };
        self.pitch_refs().any(|pitch| {
            diatonic_steps_above(root, pitch) == step && semitones_above(root, pitch) != first
        })
    }

    /// Returns whether two pitches share a pitch class under different names,
    /// such as `C#` and `D-`.
    pub fn has_any_enharmonic_spelled_pitches(&self) -> bool {
        self.pitch_class_set().len() != self.unique_pitch_names().len()
    }

    /// Returns whether the chord is exactly three distinct pitch names with a
    /// third and a fifth above the root, of any quality.
    pub fn is_triad(&self) -> bool {
        self.unique_pitch_names().len() == 3 && self.third().is_some() && self.fifth().is_some()
    }

    /// Returns whether the chord is exactly four distinct pitch names with a
    /// third, fifth and seventh above the root, of any quality.
    pub fn is_seventh(&self) -> bool {
        self.unique_pitch_names().len() == 4
            && self.third().is_some()
            && self.fifth().is_some()
            && self.seventh().is_some()
    }

    /// Returns whether the chord is a correctly spelled major triad.
    pub fn is_major_triad(&self) -> bool {
        self.is_triad_of_type((3, 11, -1), 4, 7)
    }

    /// Returns whether the chord is a correctly spelled minor triad.
    pub fn is_minor_triad(&self) -> bool {
        self.is_triad_of_type((3, 11, 1), 3, 7)
    }

    /// Returns whether the chord is a correctly spelled diminished triad.
    pub fn is_diminished_triad(&self) -> bool {
        self.is_triad_of_type((3, 10, 0), 3, 6)
    }

    /// Returns whether the chord is a correctly spelled augmented triad.
    pub fn is_augmented_triad(&self) -> bool {
        self.is_triad_of_type((3, 12, 0), 4, 8)
    }

    /// Returns whether the chord is a seventh chord whose pitches all lie at
    /// the given semitone offsets above the root.
    pub fn is_seventh_of_type(&self, semitones: &[u8]) -> bool {
        if !self.is_seventh() {
            return false;
        }
        let Some(root) = self.root() else {
            return false;
        };
        self.pitch_refs()
            .all(|pitch| semitones.contains(&semitones_above(root, pitch)))
    }

    /// Returns whether the chord is a dominant seventh: a major triad with a
    /// minor seventh.
    pub fn is_dominant_seventh(&self) -> bool {
        self.is_seventh_of_type(&[0, 4, 7, 10])
    }

    /// Returns whether the chord is a half-diminished seventh.
    pub fn is_half_diminished_seventh(&self) -> bool {
        self.is_seventh_of_type(&[0, 3, 6, 10])
    }

    /// Returns whether the chord is a fully diminished seventh.
    pub fn is_diminished_seventh(&self) -> bool {
        self.is_seventh_of_type(&[0, 3, 6, 9])
    }

    /// Returns whether the chord is only a root and a major third above it.
    pub fn is_incomplete_major_triad(&self) -> bool {
        self.is_incomplete_triad_of_type((2, 4), 4)
    }

    /// Returns whether the chord is only a root and a minor third above it.
    pub fn is_incomplete_minor_triad(&self) -> bool {
        self.is_incomplete_triad_of_type((2, 3), 3)
    }

    /// Returns whether the chord has a third and a fifth above its root. A
    /// dominant seventh is not a triad but contains one.
    pub fn contains_triad(&self) -> bool {
        self.third().is_some() && self.fifth().is_some()
    }

    /// Returns whether the chord contains a triad and a seventh above its root.
    pub fn contains_seventh(&self) -> bool {
        self.contains_triad() && self.seventh().is_some()
    }

    /// Returns the quality of the triad above the root, following music21's
    /// `Chord.quality`: incomplete triads still count, and a chord with a
    /// repeated or missing chord step is [`TriadQuality::Other`].
    pub fn quality(&self) -> TriadQuality {
        let Some(third) = self.semitones_from_chord_step(3) else {
            return TriadQuality::Other;
        };
        if self.has_repeated_chord_step(1) || self.has_repeated_chord_step(3) {
            return TriadQuality::Other;
        }
        let Some(fifth) = self.semitones_from_chord_step(5) else {
            return match third {
                4 => TriadQuality::Major,
                3 => TriadQuality::Minor,
                _ => TriadQuality::Other,
            };
        };
        if self.has_repeated_chord_step(5) {
            return TriadQuality::Other;
        }
        match (third, fifth) {
            (4, 7) => TriadQuality::Major,
            (3, 7) => TriadQuality::Minor,
            (4, 8) => TriadQuality::Augmented,
            (3, 6) => TriadQuality::Diminished,
            _ => TriadQuality::Other,
        }
    }

    /// Returns whether the chord is consonant in the common-practice sense:
    /// one pitch name, two whose closed-position interval is consonant, or a
    /// major or minor triad not in second inversion.
    pub fn is_consonant(&self) -> bool {
        let distinct = self.remove_redundant_pitch_names();
        match distinct.notes.len() {
            1 => true,
            2 => {
                let closed = self.closed_position(None).remove_redundant_pitches();
                Interval::between_pitches(&closed.notes[0].pitch, &closed.notes[1].pitch)
                    .is_ok_and(|interval| interval.is_consonant())
            }
            3 => (self.is_major_triad() || self.is_minor_triad()) && self.inversion() != Some(2),
            _ => false,
        }
    }

    /// Returns a copy with every pitch brought within an octave above the
    /// bass, duplicates removed and the notes sorted, as music21's
    /// `closedPosition` does. `force_octave` moves the bass to that octave
    /// first, carrying the rest of the chord with it.
    pub fn closed_position(&self, force_octave: Option<IntegerType>) -> Self {
        let mut chord = self.clone();
        let Some(bass_index) = chord.bass_index() else {
            return chord;
        };
        let implicit_octave = crate::defaults::PITCH_OCTAVE as IntegerType;
        if let Some(force_octave) = force_octave {
            let bass_octave = chord.notes[bass_index]
                .pitch
                .octave()
                .unwrap_or(implicit_octave);
            let shift = force_octave - bass_octave;
            for note in &mut chord.notes {
                let octave = note.pitch.octave().unwrap_or(implicit_octave);
                note.pitch.octave_setter(Some(octave + shift));
            }
        }
        let bass_ps = chord.notes[bass_index].pitch.ps();
        let bass_number = root::diatonic_note_number(&chord.notes[bass_index].pitch);
        for note in &mut chord.notes {
            let mut octave = note.pitch.octave().unwrap_or(implicit_octave);
            note.pitch.octave_setter(Some(octave));
            while note.pitch.ps() >= bass_ps + 12.0 {
                octave -= 1;
                note.pitch.octave_setter(Some(octave));
            }
            if root::diatonic_note_number(&note.pitch) < bass_number {
                note.pitch.octave_setter(Some(octave + 1));
            }
        }
        chord.retain_first_by(Pitch::name_with_octave);
        chord.sort_ascending_in_place();
        chord
    }

    /// Returns a copy keeping the first of every pitch that appears more than
    /// once with the same name and octave.
    pub fn remove_redundant_pitches(&self) -> Self {
        let mut chord = self.clone();
        chord.retain_first_by(Pitch::name_with_octave);
        chord
    }

    /// Returns a copy keeping the first of every pitch name, regardless of
    /// octave.
    pub fn remove_redundant_pitch_names(&self) -> Self {
        let mut chord = self.clone();
        chord.retain_first_by(Pitch::name);
        chord
    }

    /// Returns a copy keeping the first of every pitch class, so `C#` and
    /// `D-` count as one.
    pub fn remove_redundant_pitch_classes(&self) -> Self {
        let mut chord = self.clone();
        chord.retain_first_by(root::pitch_class);
        chord
    }

    /// Returns a copy sorted by staff position and then pitch space, so
    /// `F##` sorts below `G-`.
    pub fn sort_ascending(&self) -> Self {
        let mut chord = self.clone();
        chord.sort_ascending_in_place();
        chord
    }

    fn chord_step_from(&self, step: u8, root: &Pitch) -> Option<&Pitch> {
        let step = fold_chord_step(step);
        self.pitch_refs()
            .find(|pitch| diatonic_steps_above(root, pitch) == step)
    }

    fn is_triad_of_type(
        &self,
        address: (u8, u8, i8),
        third_semitones: u8,
        fifth_semitones: u8,
    ) -> bool {
        if self.forte_address() != Some(address) {
            return false;
        }
        if !self.is_triad() || self.has_any_enharmonic_spelled_pitches() {
            return false;
        }
        let (Some(root), Some(third), Some(fifth)) = (self.root(), self.third(), self.fifth())
        else {
            return false;
        };
        semitones_above(root, third) == third_semitones
            && semitones_above(root, fifth) == fifth_semitones
    }

    fn is_incomplete_triad_of_type(&self, address: (u8, u8), third_semitones: u8) -> bool {
        if self
            .forte_address()
            .is_none_or(|(card, index, _)| (card, index) != address)
        {
            return false;
        }
        let (Some(root), Some(_)) = (self.root(), self.third()) else {
            return false;
        };
        self.pitch_refs()
            .all(|pitch| [0, third_semitones].contains(&semitones_above(root, pitch)))
    }

    fn forte_address(&self) -> Option<(u8, u8, i8)> {
        tables::seek_chord_tables_address(&self.ordered_pitch_classes())
            .ok()
            .map(|(card, index, inversion, _)| (card, index, inversion))
    }

    fn unique_pitch_names(&self) -> std::collections::BTreeSet<String> {
        self.pitch_refs().map(Pitch::name).collect()
    }

    fn bass_index(&self) -> Option<usize> {
        self.notes
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| {
                left.pitch
                    .ps()
                    .partial_cmp(&right.pitch.ps())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(index, _)| index)
    }

    fn retain_first_by<K: PartialEq>(&mut self, key: impl Fn(&Pitch) -> K) {
        let mut seen: Vec<K> = Vec::with_capacity(self.notes.len());
        self.notes.retain(|note| {
            let candidate = key(&note.pitch);
            if seen.contains(&candidate) {
                false
            } else {
                seen.push(candidate);
                true
            }
        });
    }

    fn sort_ascending_in_place(&mut self) {
        self.notes.sort_by(|left, right| {
            root::diatonic_note_number(&left.pitch)
                .cmp(&root::diatonic_note_number(&right.pitch))
                .then_with(|| {
                    left.pitch
                        .ps()
                        .partial_cmp(&right.pitch.ps())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });
    }

    fn ordered_pitch_classes(&self) -> Vec<u8> {
        let mut pcs = self
            .notes
            .iter()
            .map(|note| root::pitch_class(&note.pitch))
            .collect::<Vec<_>>();
        pcs.sort_unstable();
        pcs.dedup();
        pcs
    }

    fn bass_pitch(&self) -> Option<&Pitch> {
        root::bass_pitch(self.pitch_refs())
    }

    fn find_root_pitch(&self) -> Option<&Pitch> {
        root::find_root_pitch(self.pitch_refs())
    }

    fn pitch_refs(&self) -> impl Iterator<Item = &Pitch> {
        self.notes.iter().map(|note| &note.pitch)
    }

    fn root_pitch_name_from_tables(&self) -> Option<String> {
        self.find_root_pitch().map(Self::display_pitch_name)
    }

    fn resolve_by_root_motion(&self, key: &Key, semitones: u8) -> Result<Option<Self>> {
        let Some(root_pitch) = self.find_root_pitch() else {
            return Ok(None);
        };
        let target_pc = (root::pitch_class(root_pitch) + semitones) % 12;
        Self::triad_for_key_pitch_class(key, target_pc)?
            .map(|chord| self.place_resolution_near_source(chord))
            .transpose()
    }

    fn triad_for_key_pitch_class(key: &Key, target_pc: u8) -> Result<Option<Self>> {
        for degree in 1..=7 {
            let degree_pitch = key.pitch_from_degree(degree)?;
            if root::pitch_class(&degree_pitch) == target_pc {
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
        for (index, lower) in self.notes.iter().enumerate() {
            for upper in self.notes.iter().skip(index + 1) {
                if Self::is_directed_augmented_sixth(&lower.pitch, &upper.pitch)
                    || Self::is_directed_augmented_sixth(&upper.pitch, &lower.pitch)
                {
                    return true;
                }
            }
        }
        false
    }

    fn is_directed_augmented_sixth(lower: &Pitch, upper: &Pitch) -> bool {
        let generic_interval = (root::step_num(upper) - root::step_num(lower)).rem_euclid(7) + 1;
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

    fn has_intervals_above_root(&self, intervals: &[u8]) -> bool {
        let Some(root_pitch) = self.find_root_pitch() else {
            return false;
        };
        let root_pc = root::pitch_class(root_pitch);
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

        let tonic_pc = root::pitch_class(&key.pitch_from_degree(1)?);
        let second_pc = root::pitch_class(&key.pitch_from_degree(2)?);
        let third_pc = root::pitch_class(&key.pitch_from_degree(3)?);
        let fourth_pc = root::pitch_class(&key.pitch_from_degree(4)?);
        let sixth_pc = root::pitch_class(&key.pitch_from_degree(6)?);

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

    fn has_pitch_names(&self, expected: &[&str]) -> bool {
        if self.notes.len() != expected.len() {
            return false;
        }

        let actual = self
            .notes
            .iter()
            .map(|note| note.pitch.name())
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

/// The quality of the triad above a chord's root, as music21's
/// `Chord.quality` reports it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TriadQuality {
    /// A major third with a perfect fifth, or a major third alone.
    Major,
    /// A minor third with a perfect fifth, or a minor third alone.
    Minor,
    /// A major third with an augmented fifth.
    Augmented,
    /// A minor third with a diminished fifth.
    Diminished,
    /// Anything else, including a missing third or a repeated chord step.
    Other,
}

impl TriadQuality {
    /// Returns music21's lowercase name for the quality.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Major => "major",
            Self::Minor => "minor",
            Self::Augmented => "augmented",
            Self::Diminished => "diminished",
            Self::Other => "other",
        }
    }
}

impl Display for TriadQuality {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

fn fold_chord_step(step: u8) -> u8 {
    if step >= 8 { step - 7 } else { step }
}

fn diatonic_steps_above(root: &Pitch, pitch: &Pitch) -> u8 {
    ((root::step_num(pitch) - root::step_num(root)).rem_euclid(7) + 1) as u8
}

fn semitones_above(root: &Pitch, pitch: &Pitch) -> u8 {
    (root::pitch_class(pitch) + 12 - root::pitch_class(root)) % 12
}

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
        .map(|note| note.pitch.clone())
        .collect::<Vec<_>>();
    for (note, pitch) in notes
        .iter_mut()
        .zip(crate::pitch::simplify_multiple_enharmonics(
            &pitches, None, None,
        )?)
    {
        note.pitch = pitch;
    }

    Ok(())
}

impl IntoNotes for &[Pitch] {
    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes> {
        Ok(self.iter().cloned().map(Note::from_pitch).collect())
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
        Ok(self.iter().flat_map(|chord| chord.notes.clone()).collect())
    }
}

impl IntoNotes for &[String] {
    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes> {
        self.iter()
            .map(|name| Note::from_name(name.as_str()))
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
            Ok(vec![Note::from_name(self)?])
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
            Ok(vec![Note::from_name(self)?])
        }
    }
}

impl IntoNotes for &[IntegerType] {
    const FROM_INTEGER_PITCHES: bool = true;

    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes> {
        let mut notes = self
            .iter()
            .map(|number| Note::from_number(*number as FloatType))
            .collect::<Result<Vec<_>>>()?;
        simplify_integer_notes(&mut notes)?;
        Ok(notes)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Duration, GuitarTuning, Key, Pitch, chord::Chord, chord::TriadQuality};

    #[test]
    fn set_duration_applies_to_non_empty_chords() {
        // Regression: the duration used to live behind an `Arc<ChordBase>` that
        // every note in the chord also held a reference to, so `Arc::get_mut`
        // returned `None` and the setter silently did nothing for any chord
        // that actually had notes in it.
        for input in ["", "C", "C E G", "C E G B-"] {
            let mut chord = Chord::new(input).unwrap();
            chord.set_duration(Duration::whole());
            assert_eq!(
                chord.duration().map(Duration::quarter_length),
                Some(4.0),
                "set_duration on {input:?}"
            );
        }
    }

    struct PredicateCase {
        notes: &'static str,
        quality: TriadQuality,
        flags: [bool; 14],
        third: Option<&'static str>,
        fifth: Option<&'static str>,
        seventh: Option<&'static str>,
        enharmonic: bool,
        repeated_third: bool,
        third_semitones: Option<u8>,
    }

    #[test]
    fn triad_and_seventh_predicates_match_music21() {
        use TriadQuality::*;
        let t = true;
        let f = false;
        let cases = [
            PredicateCase {
                notes: "C E G",
                quality: Major,
                flags: [t, t, f, f, f, f, f, f, f, t, f, f, t, f],
                third: Some("E"),
                fifth: Some("G"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(4),
            },
            PredicateCase {
                notes: "C E- G",
                quality: Minor,
                flags: [t, f, t, f, f, f, f, f, f, t, f, f, t, f],
                third: Some("E-"),
                fifth: Some("G"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(3),
            },
            PredicateCase {
                notes: "C E- G-",
                quality: Diminished,
                flags: [t, f, f, t, f, f, f, f, f, f, f, f, t, f],
                third: Some("E-"),
                fifth: Some("G-"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(3),
            },
            PredicateCase {
                notes: "C E G#",
                quality: Augmented,
                flags: [t, f, f, f, t, f, f, f, f, f, f, f, t, f],
                third: Some("E"),
                fifth: Some("G#"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(4),
            },
            PredicateCase {
                notes: "C4 E4 G4 B-4",
                quality: Major,
                flags: [f, f, f, f, f, t, t, f, f, f, f, f, t, t],
                third: Some("E4"),
                fifth: Some("G4"),
                seventh: Some("B-4"),
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(4),
            },
            PredicateCase {
                notes: "C E- G- B--",
                quality: Diminished,
                flags: [f, f, f, f, f, t, f, f, t, f, f, f, t, t],
                third: Some("E-"),
                fifth: Some("G-"),
                seventh: Some("B--"),
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(3),
            },
            PredicateCase {
                notes: "C E- G- B-",
                quality: Diminished,
                flags: [f, f, f, f, f, t, f, t, f, f, f, f, t, t],
                third: Some("E-"),
                fifth: Some("G-"),
                seventh: Some("B-"),
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(3),
            },
            PredicateCase {
                notes: "C E G B",
                quality: Major,
                flags: [f, f, f, f, f, t, f, f, f, f, f, f, t, t],
                third: Some("E"),
                fifth: Some("G"),
                seventh: Some("B"),
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(4),
            },
            PredicateCase {
                notes: "E G C",
                quality: Major,
                flags: [t, t, f, f, f, f, f, f, f, t, f, f, t, f],
                third: Some("E"),
                fifth: Some("G"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(4),
            },
            PredicateCase {
                notes: "G C E",
                quality: Major,
                flags: [t, t, f, f, f, f, f, f, f, t, f, f, t, f],
                third: Some("E"),
                fifth: Some("G"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(4),
            },
            PredicateCase {
                notes: "C E",
                quality: Major,
                flags: [f, f, f, f, f, f, f, f, f, t, t, f, f, f],
                third: Some("E"),
                fifth: None,
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(4),
            },
            PredicateCase {
                notes: "C E-",
                quality: Minor,
                flags: [f, f, f, f, f, f, f, f, f, t, f, t, f, f],
                third: Some("E-"),
                fifth: None,
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(3),
            },
            PredicateCase {
                notes: "C G",
                quality: Other,
                flags: [f, f, f, f, f, f, f, f, f, t, f, f, f, f],
                third: None,
                fifth: Some("G"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: None,
            },
            PredicateCase {
                notes: "C F",
                quality: Other,
                flags: [f, f, f, f, f, f, f, f, f, f, f, f, f, f],
                third: None,
                fifth: Some("C"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: None,
            },
            PredicateCase {
                notes: "C4 F4",
                quality: Other,
                flags: [f, f, f, f, f, f, f, f, f, f, f, f, f, f],
                third: None,
                fifth: Some("C4"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: None,
            },
            PredicateCase {
                notes: "C E G C5",
                quality: Major,
                flags: [t, t, f, f, f, f, f, f, f, t, f, f, t, f],
                third: Some("E"),
                fifth: Some("G"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(4),
            },
            PredicateCase {
                notes: "C E E- G",
                quality: Other,
                flags: [f, f, f, f, f, f, f, f, f, f, f, f, t, f],
                third: Some("E"),
                fifth: Some("G"),
                seventh: None,
                enharmonic: f,
                repeated_third: t,
                third_semitones: Some(4),
            },
            PredicateCase {
                notes: "B# E G",
                quality: Other,
                flags: [t, f, f, f, f, f, f, f, f, f, f, f, t, f],
                third: Some("G"),
                fifth: Some("B#"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(3),
            },
            PredicateCase {
                notes: "C F# G",
                quality: Other,
                flags: [f, f, f, f, f, f, f, f, f, f, f, f, f, f],
                third: None,
                fifth: Some("C"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: None,
            },
            PredicateCase {
                notes: "C E G B- D",
                quality: Major,
                flags: [f, f, f, f, f, f, f, f, f, f, f, f, t, t],
                third: Some("E"),
                fifth: Some("G"),
                seventh: Some("B-"),
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(4),
            },
            PredicateCase {
                notes: "C",
                quality: Other,
                flags: [f, f, f, f, f, f, f, f, f, t, f, f, f, f],
                third: None,
                fifth: None,
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: None,
            },
            PredicateCase {
                notes: "E-4 G4 B-4",
                quality: Major,
                flags: [t, t, f, f, f, f, f, f, f, t, f, f, t, f],
                third: Some("G4"),
                fifth: Some("B-4"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(4),
            },
            PredicateCase {
                notes: "C#4 E4 G4",
                quality: Diminished,
                flags: [t, f, f, t, f, f, f, f, f, f, f, f, t, f],
                third: Some("E4"),
                fifth: Some("G4"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(3),
            },
            PredicateCase {
                notes: "C4 E4 G4 E5",
                quality: Major,
                flags: [t, t, f, f, f, f, f, f, f, t, f, f, t, f],
                third: Some("E4"),
                fifth: Some("G4"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(4),
            },
            PredicateCase {
                notes: "C#4 D-4 E4",
                quality: Minor,
                flags: [f, f, f, f, f, f, f, f, f, f, f, t, f, f],
                third: Some("E4"),
                fifth: None,
                seventh: None,
                enharmonic: t,
                repeated_third: f,
                third_semitones: Some(3),
            },
        ];
        for case in cases {
            let chord = Chord::new(case.notes).unwrap();
            let notes = case.notes;
            let name = |pitch: Option<&Pitch>| pitch.map(Pitch::name_with_octave);
            assert_eq!(chord.quality(), case.quality, "{notes} quality");
            let actual = [
                chord.is_triad(),
                chord.is_major_triad(),
                chord.is_minor_triad(),
                chord.is_diminished_triad(),
                chord.is_augmented_triad(),
                chord.is_seventh(),
                chord.is_dominant_seventh(),
                chord.is_half_diminished_seventh(),
                chord.is_diminished_seventh(),
                chord.is_consonant(),
                chord.is_incomplete_major_triad(),
                chord.is_incomplete_minor_triad(),
                chord.contains_triad(),
                chord.contains_seventh(),
            ];
            assert_eq!(actual, case.flags, "{notes} predicates");
            assert_eq!(name(chord.third()).as_deref(), case.third, "{notes} third");
            assert_eq!(name(chord.fifth()).as_deref(), case.fifth, "{notes} fifth");
            assert_eq!(
                name(chord.seventh()).as_deref(),
                case.seventh,
                "{notes} seventh"
            );
            assert_eq!(
                chord.has_any_enharmonic_spelled_pitches(),
                case.enharmonic,
                "{notes} enharmonic"
            );
            assert_eq!(
                chord.has_repeated_chord_step(3),
                case.repeated_third,
                "{notes} repeated third"
            );
            assert_eq!(
                chord.semitones_from_chord_step(3),
                case.third_semitones,
                "{notes} third semitones"
            );
        }

        let empty = Chord::empty();
        assert_eq!(empty.quality(), TriadQuality::Other);
        assert!(!empty.is_triad());
        assert!(!empty.is_consonant());
        assert!(empty.third().is_none());
        assert!(!empty.contains_triad());
        assert_eq!(TriadQuality::Diminished.to_string(), "diminished");
    }

    #[test]
    fn consonance_of_dyads_follows_closed_position() {
        assert!(Chord::new("C4 C5 E5").unwrap().is_consonant());
        assert!(!Chord::new("C4 F4 C5").unwrap().is_consonant());
        assert!(Chord::new("F4 C5").unwrap().is_consonant());
        assert!(!Chord::new("C4 G3").unwrap().is_consonant());
    }

    #[test]
    fn closed_position_matches_music21() {
        let names = |chord: Chord| {
            chord
                .pitches()
                .iter()
                .map(Pitch::name_with_octave)
                .collect::<Vec<_>>()
        };
        let cases = [
            ("C#4 G5 E6", None, vec!["C#4", "E4", "G4"]),
            ("C#4 G5 E6", Some(2), vec!["C#2", "E2", "G2"]),
            ("C#4 G5 E6", Some(6), vec!["C#6", "E6", "G6"]),
            ("C#4 F4 C5 F5", None, vec!["C#4", "F4", "C5"]),
            ("A B", None, vec!["A4", "B4"]),
            ("C4 B#7", None, vec!["C4", "B#4"]),
            ("E4 C5 G5", None, vec!["E4", "G4", "C5"]),
            (
                "C3 C#3 E-3 E3 E#3 G3",
                None,
                vec!["C3", "C#3", "E-3", "E3", "E#3", "G3"],
            ),
            ("G4 C4 E4", Some(5), vec!["C5", "E5", "G5"]),
            ("C4 E4 G4 C5 E5", None, vec!["C4", "E4", "G4"]),
            ("C#4 D-4 E4", None, vec!["C#4", "D-4", "E4"]),
        ];
        for (notes, force_octave, expected) in cases {
            let chord = Chord::new(notes).unwrap();
            assert_eq!(
                names(chord.closed_position(force_octave)),
                expected,
                "{notes}"
            );
        }
        assert!(Chord::empty().closed_position(None).notes().is_empty());
        assert_eq!(
            names(
                Chord::new("C4 E4 C4 E5")
                    .unwrap()
                    .remove_redundant_pitches()
            ),
            vec!["C4", "E4", "E5"]
        );
        assert_eq!(
            names(
                Chord::new("C4 E4 C5 E5")
                    .unwrap()
                    .remove_redundant_pitch_names()
            ),
            vec!["C4", "E4"]
        );
        assert_eq!(
            names(
                Chord::new("C#4 D-4 E4")
                    .unwrap()
                    .remove_redundant_pitch_classes()
            ),
            vec!["C#4", "E4"]
        );
        assert_eq!(
            names(Chord::new("G-4 F##4 E4").unwrap().sort_ascending()),
            vec!["E4", "F##4", "G-4"]
        );
    }

    #[test]
    fn pitched_common_names_match_the_music21_reference() {
        let cases = [
            ("C E G", "C-major triad"),
            ("C E- G", "C-minor triad"),
            ("C E G B-", "C-dominant seventh chord"),
            ("C E G B", "C-major seventh chord"),
            ("C E- G B-", "C-minor seventh chord"),
            ("C E- G- B-", "C-half-diminished seventh chord"),
            ("C E- G- B--", "C-diminished seventh chord"),
            ("C E G B- D", "C-dominant-ninth"),
            ("C E G B D", "C-major-ninth chord"),
            ("C E- G B- D", "C-minor-ninth chord"),
            ("G2 B2 D3 F3", "G-dominant seventh chord"),
            ("B2 D3 F3 A3", "B-half-diminished seventh chord"),
        ];
        for (notes, expected) in cases {
            let chord = Chord::new(notes).unwrap();
            assert_eq!(chord.pitched_common_name(), expected, "{notes}");
        }

        let integers: &[crate::IntegerType] = &[1, 2, 3, 4, 5, 10];
        let chord = Chord::new(integers).unwrap();
        assert_eq!(chord.pitched_common_name(), "forte class 6-36B above C#");
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
        assert_eq!(
            petrushka.chord_symbol().as_deref(),
            Some("Ddom7dim5/CaddA,E-")
        );
        assert_eq!(slash_chord.chord_symbol().as_deref(), None);
    }

    #[test]
    fn chord_symbols_with_root_accept_pitch_names() {
        let chord = Chord::new("G3 C4 E4").unwrap();

        assert_eq!(
            chord.chord_symbol_with_root("C").unwrap().as_deref(),
            Some("C/G")
        );
        assert_eq!(
            chord.chord_symbol_with_root(0).unwrap().as_deref(),
            Some("C/G")
        );
    }

    #[test]
    fn guitar_fingering_covers_common_chord_tones() {
        let chord = Chord::new("C E G").unwrap();
        let fingering = chord.guitar_fingering().unwrap();

        assert_eq!(fingering.strings.len(), 6);
        // A voicing sounds chord *tones*, in whatever octave falls under the
        // hand — it is not required to reproduce the written octaves, which is
        // what used to confine every shape to the top three strings.
        assert_eq!(fingering.covered_pitch_classes, vec![0, 4, 7]);
        assert!(fingering.omitted_pitch_classes.is_empty());
        assert!(
            fingering.covered_pitch_spaces.len() >= 3,
            "expected a full voicing, got {:?}",
            fingering.covered_pitch_spaces
        );
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
        assert_eq!(fingering.covered_pitch_classes, vec![2, 9]);
        assert!(fingering.omitted_pitch_classes.is_empty());
    }

    /// Renders a fingering as the `x 3 2 0 1 0` notation guitarists read.
    fn shape(notes: &str) -> String {
        Chord::new(notes)
            .unwrap()
            .guitar_fingering()
            .unwrap()
            .strings
            .iter()
            .map(|string| match string.fret {
                None => "x".to_string(),
                Some(fret) => fret.to_string(),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn guitar_fingering_finds_the_standard_open_chords() {
        // The shapes any player would name for these chords. Before voicings
        // were matched by pitch class these all came back as `x x x n n n`.
        assert_eq!(shape("C E G"), "x 3 2 0 1 0");
        assert_eq!(shape("A C# E"), "x 0 2 2 2 0");
        assert_eq!(shape("E G# B"), "0 2 2 1 0 0");
        assert_eq!(shape("D F# A"), "x x 0 2 3 2");
        assert_eq!(shape("A C E"), "x 0 2 2 1 0");
        assert_eq!(shape("E G B"), "0 2 2 0 0 0");
        assert_eq!(shape("D F A"), "x x 0 2 3 1");
        assert_eq!(shape("G B D F"), "3 2 0 0 0 1");
        assert_eq!(shape("C E G B"), "x 3 2 0 0 0");
        assert_eq!(shape("A C E G"), "x 0 2 0 1 0");
    }

    #[test]
    fn guitar_fingering_keeps_every_chord_tone() {
        // A seventh chord that silently dropped its seventh was the other half
        // of the old scoring: omitting a written octave was punished a thousand
        // times harder than omitting an actual chord tone.
        for notes in ["G B D F", "C E G B-", "A C E G", "C E G B", "B D F"] {
            let fingering = Chord::new(notes).unwrap().guitar_fingering().unwrap();
            assert!(
                fingering.omitted_pitch_classes.is_empty(),
                "{notes} dropped {:?}",
                fingering.omitted_pitch_classes
            );
        }
    }

    #[test]
    fn guitar_fingering_puts_the_root_in_the_bass_for_open_chords() {
        for (notes, root) in [("C E G", 0), ("G B D", 7), ("E G# B", 4), ("A C E", 9)] {
            let fingering = Chord::new(notes).unwrap().guitar_fingering().unwrap();
            let bass = fingering
                .strings
                .iter()
                .find_map(|string| string.fret.and(string.pitch_class))
                .expect("a sounding string");
            assert_eq!(bass, root, "{notes} should sound its root lowest");
        }
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
        assert_eq!(chord.interval_class_vector(), Some(vec![0, 0, 1, 1, 1, 0]));
        assert!(chord.invariance_vector().is_some());
        assert_eq!(chord.z_relation(), None);
        assert!(
            chord
                .common_names()
                .iter()
                .any(|name| name == "major triad")
        );
    }

    #[test]
    fn chord_simplifies_enharmonics_explicitly() {
        let chord = Chord::new("D# F## A#").unwrap();
        let simplified = chord.simplify_enharmonics(None).unwrap();
        assert_eq!(chord.pitches()[0].name(), "D#");
        assert_eq!(simplified.pitches().len(), chord.pitches().len());

        let mut in_place = chord.clone();
        in_place.simplify_enharmonics_in_place(None).unwrap();
        assert_eq!(
            simplified
                .pitches()
                .into_iter()
                .map(|pitch| pitch.name_with_octave())
                .collect::<Vec<_>>(),
            in_place
                .pitches()
                .into_iter()
                .map(|pitch| pitch.name_with_octave())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn chord_maps_to_reduced_polyrhythm_components() {
        let major = Chord::new("C E G").unwrap();
        assert_eq!(major.polyrhythm_components(), vec![4, 5, 6]);
        assert_eq!(major.polyrhythm_ratio_string(), "4:5:6");

        let empty = Chord::empty();
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
}
