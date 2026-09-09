/// Guitar tuning and fingering helpers.
pub mod guitar;
pub(crate) mod root;
pub mod tables;

use crate::common::numbertools::ORDINALS;
use crate::defaults::{FloatType, IntegerType, UnsignedIntegerType};
use crate::duration::Duration;
use crate::error::Error;
use crate::error::Result;
use crate::interval::{Interval, PitchOrNote};
use crate::key::Key;
use crate::key::keysignature::KeySignature;
use crate::notation::{Beams, Lyric, Notehead, StemDirection, Tie};
use crate::note::{IntoNote, Note};
use crate::pitch::{Pitch, PitchClass, PitchClassSpecifier};
use crate::volume::Volume;

pub use guitar::{GuitarFingering, GuitarStringFingering, GuitarTuning, GuitarTuningString};

use num::integer::{gcd, lcm};
use std::fmt::{Display, Formatter};
use std::ops::Index;
use std::str::FromStr;
use std::sync::LazyLock;

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A collection of notes analyzed as one vertical sonority.
///
/// `Chord` accepts several note-like inputs, including whitespace-separated
/// pitch names, slices of pitches or notes, MIDI pitch numbers, vectors, and
/// `None` for an empty chord.
#[must_use]
pub struct Chord {
    notes: Vec<Note>,
    duration: Option<Duration>,
    /// A volume for the chord as a whole, used when its notes carry none.
    #[cfg_attr(feature = "serde", serde(default))]
    volume: Option<Volume>,
    /// A colour for the chord as a whole, used when its notes carry none.
    #[cfg_attr(feature = "serde", serde(default))]
    color: Option<String>,
    /// The notation the chord carries in its own right, apart from its
    /// notes': music21 keeps these on `NotRest`, which a chord is, and a
    /// chord's are read independently of the notes inside it.
    #[cfg_attr(feature = "serde", serde(default))]
    notehead: Notehead,
    #[cfg_attr(feature = "serde", serde(default))]
    notehead_fill: Option<bool>,
    #[cfg_attr(feature = "serde", serde(default))]
    notehead_parenthesis: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    stem_direction: StemDirection,
    /// The beams joining the chord's flags to its neighbours'.
    #[cfg_attr(feature = "serde", serde(default))]
    beams: Beams,
    #[cfg_attr(feature = "serde", serde(skip))]
    from_integer_pitches: bool,
    /// A root the caller decided on, which wins over the one the pitches
    /// imply: music21's overridden root, for chords spelled oddly or with
    /// added notes.
    #[cfg_attr(feature = "serde", serde(default))]
    root_override: Option<Pitch>,
    /// A bass the caller decided on, which wins over the lowest pitch.
    #[cfg_attr(feature = "serde", serde(default))]
    bass_override: Option<Pitch>,
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
#[must_use]
pub struct ChordResolutionSuggestion {
    /// The suggested resolution chord.
    pub chord: Chord,
    /// Human-readable harmonic context for the suggestion.
    pub key_context: String,
}

/// Writes a list of pitch classes the way music21's
/// `Chord.formatVectorString` does, with ten and eleven as `A` and `B`:
/// `[0, 11]` is `<0B>`.
pub fn format_vector_string(values: &[u8]) -> String {
    let digits: String = values
        .iter()
        .map(|value| crate::pitch::convert_pitch_class_to_str(*value as IntegerType))
        .collect();
    format!("<{digits}>")
}

/// The perfect fifth the seventh-chord spelling check walks by, parsed once
/// rather than re-parsed per chord.
static PERFECT_FIFTH: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("P5").expect("P5 is a valid interval"));

const CANDIDATE_TONICS: [&str; 12] = [
    "C", "D-", "D", "E-", "E", "F", "F#", "G", "A-", "A", "B-", "B",
];

impl Index<usize> for Chord {
    type Output = Note;

    fn index(&self, index: usize) -> &Self::Output {
        &self.notes[index]
    }
}

impl IntoIterator for Chord {
    type Item = Note;
    type IntoIter = std::vec::IntoIter<Note>;

    fn into_iter(self) -> Self::IntoIter {
        self.notes.into_iter()
    }
}

impl<'a> IntoIterator for &'a Chord {
    type Item = &'a Note;
    type IntoIter = std::slice::Iter<'a, Note>;

    fn into_iter(self) -> Self::IntoIter {
        self.notes.iter()
    }
}

impl<'a> IntoIterator for &'a mut Chord {
    type Item = &'a mut Note;
    type IntoIter = std::slice::IterMut<'a, Note>;

    fn into_iter(self) -> Self::IntoIter {
        self.notes.iter_mut()
    }
}

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
            volume: None,
            color: None,
            notehead: Notehead::default(),
            notehead_fill: None,
            notehead_parenthesis: false,
            stem_direction: StemDirection::default(),
            beams: Beams::default(),
            root_override: None,
            bass_override: None,
        })
    }

    /// Builds the chord of a Forte set class from its name, `3-11` or
    /// `4-27B`: music21's `fromForteClass`. The pitches are the transposed
    /// normal form from C, without octaves, so `3-11` is `C E- G` and `3-11B`
    /// is `C E G`.
    pub fn from_forte_class(notation: &str) -> Result<Self> {
        let Some((cardinality, rest)) = notation.split_once('-') else {
            return Err(Error::Chord(format!(
                "cannot extract set-class representation from string: {notation}"
            )));
        };
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        let letters = &rest[digits.len()..];
        let inversion = match letters.to_ascii_lowercase().as_str() {
            "a" => Some(1),
            "b" => Some(-1),
            _ => None,
        };
        let cardinality = cardinality
            .parse::<u8>()
            .map_err(|_| Error::Chord(format!("cannot read a cardinality out of {notation}")))?;
        let index = digits
            .parse::<u8>()
            .map_err(|_| Error::Chord(format!("cannot read a Forte number out of {notation}")))?;
        Self::from_forte_address(cardinality, index, inversion)
    }

    /// Builds the chord of a Forte set class from its table address: the
    /// cardinality, the number within it, and `1`, `-1` or `None` for the
    /// inversion, where `None` takes the form the table lists first.
    pub fn from_forte_address(cardinality: u8, index: u8, inversion: Option<i8>) -> Result<Self> {
        let pitch_classes = tables::transposed_normal_form(cardinality, index, inversion)?;
        Self::from_pitch_class_list(&pitch_classes)
    }

    /// Builds the chord whose interval-class vector this is: music21's
    /// `fromIntervalVector`. Z-related pairs share a vector; the first of
    /// the pair is returned unless `z_relation` asks for the second. `None`
    /// when no set class has the vector.
    pub fn from_interval_vector(vector: &[u8; 6], z_relation: bool) -> Option<Self> {
        let mut seen: Vec<(u8, String)> = Vec::new();
        for entry in tables::known_chord_table_entries() {
            if entry.interval_class_vector != vector {
                continue;
            }
            let number = entry.forte_class.trim_end_matches(['A', 'B']).to_string();
            if seen
                .iter()
                .any(|(card, seen_number)| *card == entry.cardinality && *seen_number == number)
            {
                continue;
            }
            seen.push((entry.cardinality, number));
        }
        let (cardinality, number) = match (seen.len(), z_relation) {
            (1, _) | (2, false) => seen.first()?.clone(),
            (2, true) => seen.get(1)?.clone(),
            _ => return None,
        };
        let index = number.split_once('-')?.1.parse().ok()?;
        Self::from_forte_address(cardinality, index, None).ok()
    }

    /// A chord from pitch-class integers, spelled the way music21 spells a
    /// chord built from integers: each class on its own and then
    /// `simplifyEnharmonics` over the whole, so `[0, 3, 6, 8]` is `C E- G- A-`.
    fn from_pitch_class_list(pitch_classes: &[u8]) -> Result<Self> {
        let pitches = pitch_classes
            .iter()
            .map(|&pc| Pitch::from_pitch_class(IntegerType::from(pc)))
            .collect::<Result<Vec<_>>>()?;
        Self::new(pitches.as_slice())?.simplify_enharmonics(None)
    }

    /// Builds an empty chord.
    pub fn empty() -> Self {
        Self {
            notes: Vec::new(),
            duration: None,
            from_integer_pitches: false,
            volume: None,
            color: None,
            notehead: Notehead::default(),
            notehead_fill: None,
            notehead_parenthesis: false,
            stem_direction: StemDirection::default(),
            beams: Beams::default(),
            root_override: None,
            bass_override: None,
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
        if !common_name.contains("augmented sixth chord") {
            return None;
        }
        let names = self.unique_pitch_names();
        let root = if self.names_are(&names, &["C#", "E-", "G"])
            || self.names_are(&names, &["C#", "E#", "G", "B"])
        {
            "C#"
        } else if self.names_are(&names, &["C", "D", "F#", "A-"]) {
            "D"
        } else if self.names_are(&names, &["C#", "E-", "G", "A"]) {
            "A"
        } else if self.names_are(&names, &["C", "E", "F#", "A#"]) {
            "F#"
        } else if self.names_are(&names, &["D", "E", "G#", "B-"])
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

        let common_names: Vec<String> = match tables::address_to_common_names(address) {
            Ok(Some(names)) => names.iter().map(|name| name.to_string()).collect(),
            _ => Vec::new(),
        };
        let forte_name = tables::address_to_forte_name(address, "tn").ok();

        if let Some(forte_name) = forte_name.as_deref() {
            if let Some(name) = self.augmented_sixth_common_name(forte_name, &common_names) {
                return name;
            }
            if let Some(first) = common_names.first() {
                if matches!(forte_name, "4-20" | "4-26") {
                    return if self.is_seventh_with_perfect_fifths_above_root_and_third() {
                        first.clone()
                    } else {
                        format!("enharmonic equivalent to {first}")
                    };
                }
                if let Some(spelled) = self.spelled_as_named(forte_name) {
                    return if spelled {
                        first.clone()
                    } else {
                        format!("enharmonic equivalent to {first}")
                    };
                }
            }
        }

        match common_names.first() {
            Some(name) => name.clone(),
            None => match forte_name {
                Some(forte_name) => format!("forte class {forte_name}"),
                None => "unknown chord".to_string(),
            },
        }
    }

    /// music21 names the set classes that carry an augmented sixth by which
    /// augmented sixth they are actually spelled as, and falls back to
    /// `enharmonic to` the plain name when the spelling does not match.
    fn augmented_sixth_common_name(
        &self,
        forte_name: &str,
        common_names: &[String],
    ) -> Option<String> {
        let named = |index: usize| common_names.get(index).cloned();
        let in_inversion = |index: usize| {
            named(index).map(|name| format!("{name} in {}", self.inversion_text().to_lowercase()))
        };
        match forte_name {
            "4-27B" => {
                if self.is_dominant_seventh() {
                    named(0)
                } else if self.is_german_augmented_sixth(false) {
                    named(2)
                } else if self.is_german_augmented_sixth(true) {
                    in_inversion(2)
                } else if self.is_swiss_augmented_sixth(false) {
                    named(3)
                } else if self.is_swiss_augmented_sixth(true) {
                    in_inversion(3)
                } else {
                    named(0).map(|name| format!("enharmonic to {name}"))
                }
            }
            "4-25" => {
                if self.is_french_augmented_sixth(false) {
                    named(1)
                } else if self.is_french_augmented_sixth(true) {
                    in_inversion(1)
                } else {
                    named(0)
                }
            }
            "3-8A" => {
                if self.is_italian_augmented_sixth(false, false) {
                    named(1)
                } else if self.is_italian_augmented_sixth(true, false) {
                    in_inversion(1)
                } else {
                    named(0)
                }
            }
            _ => None,
        }
    }

    /// Whether the chord is spelled as the set class's common name says, for
    /// the classes where music21 checks: a `3-11B` that is not a major triad
    /// is only enharmonically one.
    fn spelled_as_named(&self, forte_name: &str) -> Option<bool> {
        match forte_name {
            "3-11A" => Some(self.is_minor_triad()),
            "3-11B" => Some(self.is_major_triad()),
            "3-10" => Some(self.is_diminished_triad()),
            "3-12" => Some(self.is_augmented_triad()),
            "4-27A" => Some(self.is_half_diminished_seventh()),
            "4-28" => Some(self.is_diminished_seventh()),
            "5-27A" | "5-27B" | "5-34" => Some(self.is_ninth()),
            _ => None,
        }
    }

    /// music21's check for the two seventh-chord set classes that can be
    /// spelled several ways: a real major or minor seventh has a perfect
    /// fifth above both its root and its third.
    fn is_seventh_with_perfect_fifths_above_root_and_third(&self) -> bool {
        if !self.is_seventh() {
            return false;
        }
        let names = self.pitch_names();
        let has_fifth_above = |pitch: &Pitch| {
            PERFECT_FIFTH
                .transpose_pitch(pitch)
                .is_ok_and(|above| names.contains(&above.name()))
        };
        let (Some(root), Some(third)) = (self.root(), self.third()) else {
            return false;
        };
        has_fifth_above(root) && has_fifth_above(third)
    }

    fn spelling_common_name_override(&self) -> Option<String> {
        let names = self.unique_pitch_names();
        let name = if self.names_are(&names, &["C#", "E-", "G"]) {
            "Italian augmented sixth chord in root position"
        } else if self.names_are(&names, &["C", "D", "F#", "A-"])
            || self.names_are(&names, &["D", "E", "G#", "B-"])
            || (self.from_integer_pitches && self.pitch_class_mask() == 0b010100010100)
        {
            "French augmented sixth chord in third inversion"
        } else if self.names_are(&names, &["C#", "E-", "G", "A"]) {
            "French augmented sixth chord in first inversion"
        } else if self.names_are(&names, &["C", "E", "F#", "A#"]) {
            "French augmented sixth chord"
        } else if self.names_are(&names, &["C#", "E#", "G", "B"]) {
            "French augmented sixth chord in root position"
        } else if self.names_are(&names, &["E-", "F#", "A"])
            || self.names_are(&names, &["C#", "G", "A#"])
            || (self.from_integer_pitches && self.pitch_class_mask() == 0b001001001000)
        {
            "enharmonic equivalent to diminished triad"
        } else if self.names_are(&names, &["C#", "D#", "F#", "A#"])
            || self.names_are(&names, &["C#", "E#", "G#", "A#"])
            || self.names_are(&names, &["E-", "G-", "A-", "C-"])
        {
            "enharmonic equivalent to minor seventh chord"
        } else if self.names_are(&names, &["C#", "E#", "F#", "A#"])
            || self.names_are(&names, &["E-", "F-", "A-", "C-"])
            || self.names_are(&names, &["E-", "G-", "B-", "C-"])
        {
            "enharmonic equivalent to major seventh chord"
        } else if self.names_are(&names, &["E-", "F#", "A", "B"]) {
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
            let semitones = interval.chromatic.simple_undirected();
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

    /// Adds pitches or notes to the end of the chord, as music21's
    /// `Chord.add` does. The chord is not re-sorted: the new notes sit after
    /// the ones already there.
    pub fn add<T>(&mut self, notes: T) -> Result<()>
    where
        T: IntoNotes,
    {
        self.notes.extend(notes.try_into_notes()?);
        Ok(())
    }

    /// Removes the first note whose pitch equals this one, as music21's
    /// `Chord.remove` does, and errors when the chord has no such pitch.
    pub fn remove(&mut self, pitch: &Pitch) -> Result<()> {
        let found = self.notes.iter().position(|note| &note.pitch == pitch);
        match found {
            Some(index) => {
                let _ = self.notes.remove(index);
                Ok(())
            }
            None => Err(Error::Chord("Chord.remove(x), x not in chord".to_string())),
        }
    }

    /// Removes the first note whose written pitch name matches, as music21's
    /// `Chord.remove` does with a string.
    pub fn remove_named(&mut self, name_with_octave: &str) -> Result<()> {
        let found = self
            .notes
            .iter()
            .position(|note| note.pitch.name_with_octave() == name_with_octave);
        match found {
            Some(index) => {
                let _ = self.notes.remove(index);
                Ok(())
            }
            None => Err(Error::Chord("Chord.remove(x), x not in chord".to_string())),
        }
    }

    /// Returns cloned pitches for every note in the chord, in input order.
    pub fn pitches(&self) -> Vec<Pitch> {
        self.notes.iter().map(|note| note.pitch.clone()).collect()
    }

    /// Borrows the pitches in input order.
    ///
    /// [`Chord::pitches`] is music21's accessor and clones each pitch; an
    /// `Accidental` owns two `String`s, so that is two allocations a pitch a
    /// caller that only reads them does not need.
    pub fn iter_pitches(&self) -> impl Iterator<Item = &Pitch> + '_ {
        self.notes.iter().map(|note| &note.pitch)
    }

    /// Returns the notes in input order.
    pub fn notes(&self) -> &[Note] {
        &self.notes
    }

    /// The notes in input order, for editing in place. This is how a note's
    /// notation is changed through the chord: `chord.notes_mut()[1]
    /// .set_notehead(Notehead::Diamond)`.
    pub fn notes_mut(&mut self) -> &mut [Note] {
        &mut self.notes
    }

    /// How many notes the chord holds: what `len(chord)` answers upstream.
    pub fn len(&self) -> usize {
        self.notes.len()
    }

    /// Whether the chord holds no notes at all.
    pub fn is_empty(&self) -> bool {
        self.notes.is_empty()
    }

    /// The first note whose pitch equals this one: music21's per-note
    /// accessors take a pitch this way.
    pub fn note_for_pitch(&self, pitch: &Pitch) -> Option<&Note> {
        self.notes.iter().find(|note| &note.pitch == pitch)
    }

    /// The first note whose pitch equals this one, for editing in place.
    pub fn note_for_pitch_mut(&mut self, pitch: &Pitch) -> Option<&mut Note> {
        self.notes.iter_mut().find(|note| &note.pitch == pitch)
    }

    /// Whether *every* note carries a volume of its own: music21's
    /// `hasComponentVolumes`, which counts the notes that have one and
    /// compares the count against the whole chord. A chord where only some
    /// notes have been given a volume reads as having none.
    pub fn has_component_volumes(&self) -> bool {
        self.notes.iter().all(Note::has_volume_information)
    }

    /// How loud the chord is. With volumes on its notes this is their
    /// average velocity, as music21 reads it; otherwise it is the chord's
    /// own volume.
    pub fn volume(&self) -> Volume {
        // music21 asks in this order: a volume of the chord's own wins, then
        // the notes' average, and a chord with neither — an empty one
        // included — reads as a default volume.
        if let Some(volume) = &self.volume {
            return volume.clone();
        }
        if !self.has_component_volumes() {
            return Volume::default();
        }
        let velocities: Vec<IntegerType> = self
            .notes
            .iter()
            .filter_map(|note| note.volume().velocity())
            .collect();
        if velocities.is_empty() {
            return Volume::default();
        }
        let total: IntegerType = velocities.iter().sum();
        let mean = FloatType::from(total) / velocities.len() as FloatType;
        Volume::from_velocity(mean.round_ties_even() as IntegerType)
    }

    /// Sets the chord's own volume, which drops any the notes carried.
    pub fn set_volume(&mut self, volume: Option<Volume>) {
        for note in &mut self.notes {
            note.set_volume(None);
        }
        self.volume = volume;
    }

    /// Gives each note a volume from the list, cycling through it when the
    /// chord has more notes than the list has volumes: music21's
    /// `setVolumes`. The chord's own volume is dropped.
    pub fn set_volumes(&mut self, volumes: &[Volume]) -> Result<()> {
        if volumes.is_empty() {
            return Err(Error::Chord(
                "setVolumes needs at least one volume".to_string(),
            ));
        }
        self.volume = None;
        for (index, note) in self.notes.iter_mut().enumerate() {
            note.set_volume(Some(volumes[index % volumes.len()].clone()));
        }
        Ok(())
    }

    /// The colour the chord is written in, when the chord itself carries one
    /// rather than its notes.
    pub fn color(&self) -> Option<&str> {
        self.color.as_deref()
    }

    /// Sets the colour the chord as a whole is written in.
    pub fn set_color(&mut self, color: Option<String>) {
        self.color = color;
    }

    /// The shape the chord as a whole is drawn with: music21's `notehead`,
    /// which a chord has in its own right and not only through its notes.
    pub fn notehead(&self) -> Notehead {
        self.notehead
    }

    /// Sets that shape.
    pub fn set_notehead(&mut self, notehead: Notehead) {
        self.notehead = notehead;
    }

    /// Whether the chord's own note heads are filled, when it says.
    pub fn notehead_fill(&self) -> Option<bool> {
        self.notehead_fill
    }

    /// Says whether they are filled.
    pub fn set_notehead_fill(&mut self, fill: Option<bool>) {
        self.notehead_fill = fill;
    }

    /// Whether the chord's own note heads are bracketed.
    pub fn notehead_parenthesis(&self) -> bool {
        self.notehead_parenthesis
    }

    /// Says whether they are bracketed.
    pub fn set_notehead_parenthesis(&mut self, parenthesis: bool) {
        self.notehead_parenthesis = parenthesis;
    }

    /// Which way the chord's own stem points.
    pub fn stem_direction(&self) -> StemDirection {
        self.stem_direction
    }

    /// Sets which way it points.
    pub fn set_stem_direction(&mut self, direction: StemDirection) {
        self.stem_direction = direction;
    }

    /// The beams joining the chord's flags to its neighbours'.
    pub fn beams(&self) -> &Beams {
        &self.beams
    }

    /// Replaces those beams.
    pub fn set_beams(&mut self, beams: Beams) {
        self.beams = beams;
    }

    /// The colour a pitch is written in: the note's own colour when it has
    /// one, and the chord's otherwise, as music21's `getColor` reads it.
    pub fn color_of_pitch(&self, pitch: &Pitch) -> Option<&str> {
        self.note_for_pitch(pitch)
            .and_then(Note::color)
            .or(self.color())
    }

    /// The tie of the first note that carries one: music21's chord-level
    /// `tie`.
    pub fn tie(&self) -> Option<&Tie> {
        self.notes.iter().find_map(Note::tie)
    }

    /// Ties every note in the chord, or unties them all with `None`.
    pub fn set_tie(&mut self, tie: Option<Tie>) {
        for note in &mut self.notes {
            note.set_tie(tie.clone());
        }
    }

    /// The syllables sung on the chord, kept on its first note as music21
    /// keeps them on the chord itself.
    pub fn lyrics(&self) -> &[Lyric] {
        self.notes.first().map_or(&[], Note::lyrics)
    }

    /// Adds a syllable as the next verse: music21's `addLyric`.
    pub fn add_lyric(
        &mut self,
        text: &str,
        number: Option<IntegerType>,
        apply_raw: bool,
    ) -> Result<()> {
        match self.notes.first_mut() {
            Some(note) => note.add_lyric(text, number, apply_raw),
            None => Err(Error::Chord(
                "an empty chord has nothing to sing".to_string(),
            )),
        }
    }

    /// Names the interval from the chord's lowest pitch up to each of the
    /// others, highest first: music21's `annotateIntervals`.
    ///
    /// With `strip_specifiers` the names are bare numbers (`8`, `5`, `3`)
    /// and sorted downward; without it they are full interval names (`P8`,
    /// `P5`, `M3`). Repeated pitches are dropped first, and `sort_pitches`
    /// measures from the lowest pitch rather than the written first one.
    pub fn annotate_intervals(
        &self,
        strip_specifiers: bool,
        sort_pitches: bool,
    ) -> Result<Vec<String>> {
        let mut reduced = self.remove_redundant_pitches();
        if sort_pitches {
            reduced = reduced.sort_ascending();
        }
        let pitches = reduced.pitches();
        let Some(lowest) = pitches.first() else {
            return Ok(Vec::new());
        };
        let mut names = Vec::with_capacity(pitches.len().saturating_sub(1));
        for pitch in pitches.iter().skip(1).rev() {
            let interval = Interval::between_pitches(lowest, pitch)?;
            names.push(if strip_specifiers {
                interval.generic().semi_simple_undirected().to_string()
            } else {
                interval.semi_simple_name()
            });
        }
        if strip_specifiers && sort_pitches {
            names.sort_by(|left, right| right.cmp(left));
        }
        Ok(names)
    }

    /// Writes the interval names of [`Self::annotate_intervals`] onto the
    /// chord as lyrics, one verse each.
    pub fn annotated_with_intervals(
        &self,
        strip_specifiers: bool,
        sort_pitches: bool,
    ) -> Result<Self> {
        let names = self.annotate_intervals(strip_specifiers, sort_pitches)?;
        let mut annotated = self.clone();
        for name in names {
            annotated.add_lyric(&name, None, false)?;
        }
        Ok(annotated)
    }

    /// Reads a string harmonic: given a chord whose second note is written
    /// with a diamond head, the sounding pitch is the harmonic of the first
    /// note that their distance picks out, and the chord comes back with it
    /// added on top. A chord not written as a harmonic answers `None`.
    ///
    /// This is music21's `Pitch.getStringHarmonic`, which reads the notehead
    /// off the chord rather than off the pitch it is called on.
    pub fn string_harmonic(&self) -> Result<Option<Self>> {
        let [stopped, touched] = match self.notes.get(..2) {
            Some([first, second]) => [first, second],
            _ => return Ok(None),
        };
        if touched.notehead() != Notehead::Diamond {
            return Ok(None);
        }
        let distance = crate::interval::notes_to_chromatic(&stopped.pitch, &touched.pitch);
        let harmonic = match distance.interval_class() {
            0 => 2,
            7 => 3,
            5 => 4,
            4 => 5,
            3 => 6,
            6 => 7,
            _ => 1,
        };
        let sounding = if harmonic == 1 {
            stopped.pitch.clone()
        } else {
            stopped.pitch.harmonic(harmonic)?
        };
        let mut sounding_note = Note::from_pitch(sounding);
        sounding_note.set_notehead_parenthesis(true);
        sounding_note.set_notehead_fill(Some(true));
        sounding_note.set_stem_direction(crate::notation::StemDirection::NoStem);
        let mut touched_note = Note::from_pitch(touched.pitch.clone());
        touched_note.set_notehead(touched.notehead());
        let notes = vec![
            Note::from_pitch(stopped.pitch.clone()),
            touched_note,
            sounding_note,
        ];
        Ok(Some(Self::new(notes.as_slice())?))
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

    /// Returns the normal form transposed to start on zero, `[0, 3, 6, 8]`
    /// for `C E G B-`. This is the Forte-table form music21 reads off the
    /// chord's table address; [`Self::normal_order`] is music21's
    /// `normalOrder`, on the chord's own pitch classes.
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

    /// Returns the inversion number the way music21 finds it: root position
    /// is `0`, and the number climbs with the chord step the bass sits on,
    /// so a bass on the seventh is `3`. `None` only for an empty chord.
    pub fn inversion(&self) -> Option<u8> {
        let bass_to_root = diatonic_steps_above(self.bass()?, self.root()?);
        Some([0, 3, 6, 2, 5, 1, 4][usize::from(bass_to_root - 1)])
    }

    /// The inversion the way [`Self::inversion`] counts it, but measured from a
    /// root the caller has decided on rather than the one the chord infers.
    pub(crate) fn inversion_with_root(&self, root: &Pitch) -> Option<u8> {
        let bass_to_root = diatonic_steps_above(self.bass()?, root);
        Some([0, 3, 6, 2, 5, 1, 4][usize::from(bass_to_root - 1)])
    }

    /// Returns a human-readable inversion label.
    ///
    /// Returns `None` whenever [`Self::inversion`] returns `None`.
    /// The figured-bass number music21's `inversionName` answers: `53`, `6`
    /// and `64` for a triad, `7`, `65`, `43` and `42` for a seventh. `None`
    /// when the chord has no inversion, and an error when it is neither a
    /// triad nor a seventh, as music21 raises there. For the words, see
    /// [`Self::inversion_text`].
    pub fn inversion_name(&self) -> Result<Option<IntegerType>> {
        let Some(inversion) = self.inversion() else {
            return Ok(None);
        };
        let inversion = usize::from(inversion);
        if self.is_seventh() || self.seventh().is_some() {
            return [7, 65, 43, 42]
                .get(inversion)
                .copied()
                .map(Some)
                .ok_or_else(|| {
                    Error::Chord(format!("Not a normal inversion for a seventh: {inversion}"))
                });
        }
        if self.is_triad() {
            return [53, 6, 64]
                .get(inversion)
                .copied()
                .map(Some)
                .ok_or_else(|| {
                    Error::Chord(format!("Not a normal inversion for a triad: {inversion}"))
                });
        }
        Err(Error::Chord(
            "Not a triad or Seventh, cannot determine inversion.".to_string(),
        ))
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
        self.root_override
            .as_ref()
            .or_else(|| self.find_root_pitch())
    }

    /// The root the pitches imply, ignoring any override.
    pub fn found_root(&self) -> Option<&Pitch> {
        self.find_root_pitch()
    }

    /// Fixes the root the chord reports, or clears the override with `None`:
    /// music21's `root(newroot)`. The pitch need not be in the chord, which
    /// is the point of it for oddly spelled or added-note chords.
    pub fn set_root(&mut self, root: Option<Pitch>) {
        self.root_override = root;
    }

    /// Returns the lowest pitch, or the bass a caller fixed with
    /// [`Self::set_bass`].
    pub fn bass(&self) -> Option<&Pitch> {
        self.bass_override.as_ref().or_else(|| self.bass_pitch())
    }

    /// The lowest pitch, ignoring any override.
    pub fn found_bass(&self) -> Option<&Pitch> {
        self.bass_pitch()
    }

    /// The bass a caller fixed with [`Self::set_bass`], and nothing when
    /// none was.
    pub fn overridden_bass(&self) -> Option<&Pitch> {
        self.bass_override.as_ref()
    }

    /// The root a caller fixed with [`Self::set_root`], and nothing when
    /// none was.
    pub fn overridden_root(&self) -> Option<&Pitch> {
        self.root_override.as_ref()
    }

    /// Fixes the bass the chord reports, or clears the override with `None`:
    /// music21's `bass(newbass)`. A pitch the chord does not already carry is
    /// added below the others, as music21 adds it.
    pub fn set_bass(&mut self, bass: Option<Pitch>) {
        let Some(bass) = bass else {
            self.bass_override = None;
            return;
        };
        let known = self
            .notes
            .iter()
            .any(|note| note.pitch.name_with_octave() == bass.name_with_octave());
        if !known {
            self.notes.insert(0, Note::from_pitch(bass.clone()));
        }
        self.bass_override = Some(bass);
    }

    /// Rearranges the chord so it stands in the given inversion, raising the
    /// bass by octaves until it does: music21's `inversion(newInversion)`.
    /// An inversion the chord cannot reach is an error.
    pub fn set_inversion(&mut self, inversion: u8) -> Result<()> {
        self.bass_override = None;
        let mut runs = self.notes.len() + 2;
        while self.inversion() != Some(inversion) {
            if runs == 0 {
                return Err(Error::Chord(
                    "Could not invert chord: inversion may not exist".to_string(),
                ));
            }
            runs -= 1;
            let highest_ps = self
                .notes
                .iter()
                .map(|note| note.pitch.ps())
                .fold(FloatType::NEG_INFINITY, FloatType::max);
            let Some(bass_name) = self.bass().map(Pitch::name_with_octave) else {
                return Err(Error::Chord(
                    "Could not invert chord: inversion may not exist".to_string(),
                ));
            };
            let Some(index) = self
                .notes
                .iter()
                .position(|note| note.pitch.name_with_octave() == bass_name)
            else {
                return Err(Error::Chord(
                    "Could not invert chord: inversion may not exist".to_string(),
                ));
            };
            while self.notes[index].pitch.ps() < highest_ps {
                let octave = self.notes[index].pitch.implicit_octave();
                self.notes[index].pitch.octave_setter(Some(octave + 1));
            }
        }
        *self = self.sort_ascending();
        Ok(())
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
                let closed = self.closed_position(None, false).remove_redundant_pitches();
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
    pub fn closed_position(
        &self,
        force_octave: Option<IntegerType>,
        leave_redundant_pitches: bool,
    ) -> Self {
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
        if !leave_redundant_pitches {
            chord.retain_first_by(Pitch::name_with_octave);
        }
        chord.sort_ascending_in_place();
        chord
    }

    /// The chord with duplicate pitches removed, and the pitches that went:
    /// music21's `removeRedundantPitches`, which hands back what it dropped.
    pub fn remove_redundant_pitches_reporting(&self) -> (Self, Vec<Pitch>) {
        self.reduced_reporting(Pitch::name_with_octave)
    }

    /// The chord with pitches of the same name removed, and the ones that
    /// went: music21's `removeRedundantPitchNames`.
    pub fn remove_redundant_pitch_names_reporting(&self) -> (Self, Vec<Pitch>) {
        self.reduced_reporting(Pitch::name)
    }

    /// The chord with pitches of the same pitch class removed, and the ones
    /// that went: music21's `removeRedundantPitchClasses`.
    pub fn remove_redundant_pitch_classes_reporting(&self) -> (Self, Vec<Pitch>) {
        self.reduced_reporting(root::pitch_class)
    }

    fn reduced_reporting<K: PartialEq>(&self, key: impl Fn(&Pitch) -> K) -> (Self, Vec<Pitch>) {
        let mut kept = self.clone();
        let mut seen: Vec<K> = Vec::with_capacity(self.notes.len());
        let mut removed = Vec::new();
        kept.notes.retain(|note| {
            let candidate = key(&note.pitch);
            if seen.contains(&candidate) {
                removed.push(note.pitch.clone());
                false
            } else {
                seen.push(candidate);
                true
            }
        });
        (kept, removed)
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

    /// The first pitch at the given chord step above a root the caller
    /// decided on, rather than the one the chord infers: music21's
    /// `getChordStep(step, testRoot)`.
    pub fn chord_step_with_root(&self, step: u8, root: &Pitch) -> Option<&Pitch> {
        self.chord_step_from(step, root)
    }

    /// The semitone distance from a caller-supplied root to the pitch at the
    /// given chord step: music21's `semitonesFromChordStep(step, testRoot)`.
    pub fn semitones_from_chord_step_with_root(&self, step: u8, root: &Pitch) -> Option<u8> {
        let pitch = self.chord_step_from(step, root)?;
        Some(semitones_above(root, pitch))
    }

    /// The inversion measured from a root the caller decided on: music21's
    /// `inversion(testRoot=...)`.
    pub fn inversion_from_root(&self, root: &Pitch) -> Option<u8> {
        self.inversion_with_root(root)
    }

    pub(crate) fn chord_step_from(&self, step: u8, root: &Pitch) -> Option<&Pitch> {
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

    /// Returns a copy with every note transposed by the interval.
    pub fn transpose(&self, interval: &Interval) -> Result<Self> {
        let mut chord = self.clone();
        for note in &mut chord.notes {
            note.pitch = interval.transpose_pitch(&note.pitch)?;
        }
        // A root or bass a caller fixed moves with the chord, as music21
        // moves it: a chord symbol carries its root as an override, and one
        // transposed up a semitone is a chord on the note above.
        if let Some(root) = &chord.root_override {
            chord.root_override = Some(interval.transpose_pitch(root)?);
        }
        if let Some(bass) = &chord.bass_override {
            chord.bass_override = Some(interval.transpose_pitch(bass)?);
        }
        Ok(chord)
    }

    /// Returns whether the chord is an Italian, French, German or Swiss
    /// augmented sixth. Each must be in its conventional inversion unless
    /// `permit_any_inversion` is set.
    pub fn is_augmented_sixth(&self, permit_any_inversion: bool) -> bool {
        match self.pitch_class_cardinality() {
            3 => self.is_italian_augmented_sixth(permit_any_inversion, false),
            4 => {
                self.is_french_augmented_sixth(permit_any_inversion)
                    || self.is_german_augmented_sixth(permit_any_inversion)
                    || self.is_swiss_augmented_sixth(permit_any_inversion)
            }
            _ => false,
        }
    }

    /// Returns whether the chord is an Italian augmented sixth, such as
    /// `A- C F#`.
    pub fn is_italian_augmented_sixth(
        &self,
        permit_any_inversion: bool,
        restrict_doublings: bool,
    ) -> bool {
        if !self.is_augmented_sixth_of_type(
            (3, 8, 1),
            1,
            permit_any_inversion,
            &AUGMENTED_SIXTHS.italian,
        ) {
            return false;
        }
        if !restrict_doublings {
            return true;
        }
        let (Some(root), Some(third), Some(fifth)) = (self.root(), self.third(), self.fifth())
        else {
            return false;
        };
        self.pitch_refs().all(|pitch| {
            pitch.name() == fifth.name() || std::ptr::eq(pitch, third) || std::ptr::eq(pitch, root)
        })
    }

    /// Returns whether the chord is a French augmented sixth, such as
    /// `A- C D F#`.
    pub fn is_french_augmented_sixth(&self, permit_any_inversion: bool) -> bool {
        self.is_augmented_sixth_of_type(
            (4, 25, 0),
            2,
            permit_any_inversion,
            &AUGMENTED_SIXTHS.french,
        )
    }

    /// Returns whether the chord is a German augmented sixth, such as
    /// `A- C E- F#`.
    pub fn is_german_augmented_sixth(&self, permit_any_inversion: bool) -> bool {
        self.is_augmented_sixth_of_type(
            (4, 27, -1),
            1,
            permit_any_inversion,
            &AUGMENTED_SIXTHS.german,
        )
    }

    /// Returns whether the chord is a Swiss augmented sixth, such as
    /// `A- C D# F#`.
    pub fn is_swiss_augmented_sixth(&self, permit_any_inversion: bool) -> bool {
        self.is_augmented_sixth_of_type(
            (4, 27, -1),
            2,
            permit_any_inversion,
            &AUGMENTED_SIXTHS.swiss,
        )
    }

    /// Returns whether the chord is five distinct pitch names with a third,
    /// fifth, seventh and ninth above the root.
    pub fn is_ninth(&self) -> bool {
        self.unique_pitch_names().len() == 5
            && self.third().is_some()
            && self.fifth().is_some()
            && self.seventh().is_some()
            && self.chord_step(2).is_some()
    }

    /// Returns whether some transposition other than the octave maps the
    /// pitch-class set onto itself. With `require_intervallic_evenness` only
    /// the evenly spaced sets count, the way Straus defines the property.
    pub fn is_transpositionally_symmetrical(&self, require_intervallic_evenness: bool) -> bool {
        let Some((card, index, _)) = self.forte_address() else {
            return self.notes.is_empty();
        };
        if card == 1 {
            return require_intervallic_evenness;
        }
        const EVEN: [(u8, u8); 5] = [(2, 6), (3, 12), (4, 28), (6, 35), (12, 1)];
        const UNEVEN: [(u8, u8); 10] = [
            (4, 9),
            (4, 25),
            (6, 7),
            (6, 20),
            (6, 30),
            (8, 9),
            (8, 25),
            (8, 28),
            (9, 12),
            (10, 6),
        ];
        EVEN.contains(&(card, index))
            || (!require_intervallic_evenness && UNEVEN.contains(&(card, index)))
    }

    /// Returns whether the chord could serve as a dominant: a major triad or
    /// a dominant seventh.
    pub fn can_be_dominant_v(&self) -> bool {
        self.is_major_triad() || self.is_dominant_seventh()
    }

    /// Returns whether the chord could serve as a tonic: a major or minor
    /// triad.
    pub fn can_be_tonic(&self) -> bool {
        self.is_major_triad() || self.is_minor_triad()
    }

    /// Returns whether two pitches share a letter under different
    /// accidentals, such as `E` and `E-`.
    pub fn has_any_repeated_diatonic_note(&self) -> bool {
        let steps = self
            .pitch_refs()
            .map(Pitch::step)
            .collect::<std::collections::BTreeSet<_>>();
        steps.len() != self.unique_pitch_names().len()
    }

    /// Returns the Forte prime form of the pitch-class set, such as
    /// `[0, 3, 7]` for any major or minor triad. Empty when the chord has no
    /// table entry.
    pub fn prime_form(&self) -> Vec<u8> {
        tables::seek_chord_tables_address(&self.ordered_pitch_classes())
            .and_then(tables::prime_form_from_address)
            .unwrap_or_default()
    }

    /// Returns the prime form in music21's angle-bracket notation, such as
    /// `"<037>"`, with `A` and `B` standing for ten and eleven.
    pub fn prime_form_string(&self) -> String {
        format_pitch_classes(&self.prime_form())
    }

    /// Returns the Forte class without the `A`/`B` inversion suffix, such as
    /// `"3-11"` for both major and minor triads.
    pub fn forte_class_tni(&self) -> Option<String> {
        let address = tables::seek_chord_tables_address(&self.ordered_pitch_classes()).ok()?;
        tables::address_to_forte_name(address, "tni").ok()
    }

    /// Returns the number of distinct pitch classes.
    pub fn pitch_class_cardinality(&self) -> usize {
        self.pitch_class_set().len()
    }

    /// Returns the distinct pitch classes in ascending order in music21's
    /// angle-bracket notation, such as `"<047>"`.
    pub fn ordered_pitch_classes_string(&self) -> String {
        format_pitch_classes(&self.ordered_pitch_classes())
    }

    fn is_augmented_sixth_of_type(
        &self,
        address: (u8, u8, i8),
        required_inversion: u8,
        permit_any_inversion: bool,
        intervals: &[[Interval; 2]],
    ) -> bool {
        if self.forte_address() != Some(address) || self.has_any_enharmonic_spelled_pitches() {
            return false;
        }
        if !permit_any_inversion && self.inversion() != Some(required_inversion) {
            return false;
        }
        let Some(root) = self.root() else {
            return false;
        };
        let steps = [self.third(), self.fifth(), self.seventh()];
        intervals.iter().zip(steps).all(|(accepted, step)| {
            step.and_then(|pitch| Interval::between_pitches(root, pitch).ok())
                .is_some_and(|interval| {
                    accepted.iter().any(|candidate| {
                        candidate.directed_simple_key() == interval.directed_simple_key()
                    })
                })
        })
    }

    /// Returns each pitch's degree in `scale` with the accidental that
    /// separates it from the scale's spelling, as music21's `scaleDegrees`
    /// does: `C E- G` in C major is `(1, None), (3, flat), (5, None)`. A pitch
    /// whose letter the scale lacks reports `(None, None)`.
    pub fn scale_degrees(
        &self,
        scale: &crate::scale::Scale,
    ) -> Result<Vec<(Option<usize>, Option<crate::pitch::Accidental>)>> {
        self.pitch_refs()
            .map(|pitch| match scale.degree_and_accidental_of(pitch) {
                Ok((degree, accidental)) => Ok((Some(degree), accidental)),
                Err(Error::Scale(_)) => Ok((None, None)),
                Err(error) => Err(error),
            })
            .collect()
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

    /// Whether the chord is spelled with exactly these pitch names.
    ///
    /// The set of names it compares against is passed in rather than built
    /// here: the two spelling cascades ask this question up to fifteen times
    /// in a row, and building the set per question was most of what
    /// `common_name` cost.
    fn names_are(&self, names: &std::collections::BTreeSet<String>, expected: &[&str]) -> bool {
        self.notes.len() == expected.len() && expected.iter().all(|name| names.contains(*name))
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

struct AugmentedSixthIntervals {
    italian: [[Interval; 2]; 2],
    french: [[Interval; 2]; 3],
    german: [[Interval; 2]; 3],
    swiss: [[Interval; 2]; 3],
}

/// The intervals each augmented-sixth type stacks above its root, as music21
/// spells them: the third and fifth (and seventh) may each be written either
/// way up.
static AUGMENTED_SIXTHS: LazyLock<AugmentedSixthIntervals> = LazyLock::new(|| {
    let pair = |up: &str, down: &str| {
        [
            Interval::from_name(up).expect("augmented sixth intervals parse"),
            Interval::from_name(down).expect("augmented sixth intervals parse"),
        ]
    };
    AugmentedSixthIntervals {
        italian: [pair("d3", "A-6"), pair("d5", "A-4")],
        french: [pair("M3", "m-6"), pair("d5", "A-4"), pair("m7", "M-2")],
        german: [pair("d3", "A-6"), pair("d5", "A-4"), pair("d7", "A-2")],
        swiss: [pair("m3", "M-6"), pair("dd5", "AA-4"), pair("d7", "A-2")],
    }
});

fn format_pitch_classes(pitch_classes: &[u8]) -> String {
    let mut out = String::with_capacity(pitch_classes.len() + 2);
    out.push('<');
    for pitch_class in pitch_classes {
        out.push_str(&crate::pitch::pitchclass::convert_pitch_class_to_str(
            IntegerType::from(*pitch_class),
        ));
    }
    out.push('>');
    out
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

/// Where a chord's set class sits in the Forte tables: music21's
/// `ChordTableAddress`.
///
/// The cardinality and the class number index the table; the inversion says
/// which of an inversionally related pair this is, `0` when the class is its
/// own inversion; and the original pitch class is the one the prime form was
/// transposed away from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct ChordTableAddress {
    /// How many distinct pitch classes the chord has.
    pub cardinality: u8,
    /// The Forte class number within that cardinality.
    pub forte_class: u8,
    /// `1`, `-1`, or `0` for a class that is its own inversion.
    pub inversion: i8,
    /// The pitch class the prime form was transposed away from.
    pub pitch_class_original: u8,
}

impl Chord {
    /// Where this chord's set class sits in the Forte tables.
    ///
    /// An empty chord answers all zeros rather than failing, which is the
    /// one place music21's `Chord.chordTablesAddress` differs from the
    /// `seekChordTablesAddress` underneath it.
    pub fn chord_tables_address_entry(&self) -> ChordTableAddress {
        match self.chord_tables_address() {
            Some((cardinality, forte_class, inversion, original)) => ChordTableAddress {
                cardinality,
                forte_class,
                inversion,
                pitch_class_original: original.unwrap_or(0),
            },
            None => ChordTableAddress {
                cardinality: 0,
                forte_class: 0,
                inversion: 0,
                pitch_class_original: 0,
            },
        }
    }

    fn chord_tables_address(&self) -> Option<tables::RawAddress> {
        tables::seek_chord_tables_address(&self.ordered_pitch_classes()).ok()
    }

    /// Returns music21's `geometricNormalForm`: the distinct pitch classes
    /// rotated so the intervals between neighbours read smallest first, then
    /// written from zero, so both `C E G` and `E G C` are `[0, 3, 8]`. Empty
    /// for an empty chord.
    pub fn geometric_normal_form(&self) -> Vec<u8> {
        let pitch_classes = self.ordered_pitch_classes();
        if pitch_classes.is_empty() {
            return Vec::new();
        }
        let intervals: Vec<u8> = pitch_classes
            .iter()
            .zip(pitch_classes.iter().cycle().skip(1))
            .map(|(&low, &high)| (high + 12 - low) % 12)
            .collect();
        let best = (0..intervals.len())
            .map(|rotation| {
                let mut rotated = intervals[rotation + 1..].to_vec();
                rotated.extend_from_slice(&intervals[..=rotation]);
                rotated
            })
            .min()
            .unwrap_or_default();
        let mut sum = 0;
        best.iter()
            .map(|interval| {
                let pitch_class = sum;
                sum += interval;
                pitch_class
            })
            .collect()
    }

    /// Returns the interval-class vector in music21's angle-bracket
    /// notation, `<001110>`; an empty chord reads `<000000>`.
    pub fn interval_vector_string(&self) -> String {
        format_pitch_classes(&self.interval_class_vector().unwrap_or_else(|| vec![0; 6]))
    }

    /// Returns music21's `normalOrder`: the most compact rotation of the
    /// pitch classes, on the chord's own pitch classes rather than
    /// transposed to zero, so `C E G B-` is `[4, 7, 10, 0]` where
    /// [`Self::normal_form`] is `[0, 3, 6, 8]`. Empty for an empty chord.
    pub fn normal_order(&self) -> Vec<u8> {
        let Some(transposed) = self.normal_form() else {
            return Vec::new();
        };
        let ordered = self.ordered_pitch_classes();
        ordered
            .iter()
            .map(|&transposition| {
                transposed
                    .iter()
                    .map(|&pc| (pc + transposition) % 12)
                    .collect::<Vec<u8>>()
            })
            .find(|candidate| {
                let mut sorted = candidate.clone();
                sorted.sort_unstable();
                sorted == ordered
            })
            .unwrap_or_default()
    }

    /// Returns [`Self::normal_order`] in music21's angle-bracket notation,
    /// `<47A0>`.
    pub fn normal_order_string(&self) -> String {
        format_pitch_classes(&self.normal_order())
    }

    /// Returns the Forte class number within the cardinality, `11` for a
    /// major or minor triad. `None` for an empty chord.
    pub fn forte_class_number(&self) -> Option<u8> {
        self.chord_tables_address().map(|address| address.1)
    }

    /// Returns the Forte class under transposition equivalence, with the
    /// `A`/`B` inversion suffix: music21's `forteClassTn`, the same as
    /// [`Self::forte_class`].
    pub fn forte_class_tn(&self) -> Option<String> {
        self.forte_class()
    }

    /// Returns the number of notes, counting repeated pitch classes: music21's
    /// `multisetCardinality`.
    pub fn multiset_cardinality(&self) -> usize {
        self.notes.len()
    }

    /// Returns whether the pitch-class set is the inversion of its prime
    /// form, so its Forte class carries a `B` suffix.
    pub fn is_prime_form_inversion(&self) -> bool {
        self.chord_tables_address()
            .is_some_and(|address| address.2 == -1)
    }

    /// Returns whether music21 records a Z-related set class for this chord.
    pub fn has_z_relation(&self) -> bool {
        self.z_relation().is_some()
    }

    /// Returns whether `other` belongs to the set class Z-related to this
    /// chord's, so the two share an interval vector without being related by
    /// transposition or inversion.
    pub fn are_z_relations(&self, other: &Chord) -> bool {
        let Some(z_relation) = self.z_relation() else {
            return false;
        };
        other
            .chord_tables_address()
            .is_some_and(|address| format!("{}-{}", address.0, address.1) == z_relation)
    }

    /// Returns whether the pitch classes form a fully diminished seventh
    /// however it is spelled: music21's `isFalseDiminishedSeventh`, true for
    /// `C E- G- A` where [`Self::is_diminished_seventh`] is not.
    pub fn is_false_diminished_seventh(&self) -> bool {
        self.chord_tables_address()
            .is_some_and(|address| (address.0, address.1, address.2) == (4, 28, 0))
    }

    /// Returns music21's `inversionText`: `Root Position`, `First Inversion`
    /// and so on, or `Unknown Position` for an empty chord.
    pub fn inversion_text(&self) -> String {
        match self.inversion() {
            Some(0) => "Root Position".to_string(),
            Some(inversion) => format!("{} Inversion", ORDINALS[usize::from(inversion)]),
            None => "Unknown Position".to_string(),
        }
    }

    /// Returns the interval from the root to the first pitch lying on the
    /// given chord step, compound intervals included, so the third of
    /// `C3 G3 E4 C5` is a major tenth. `None` when the chord has no root or
    /// no pitch on that step.
    pub fn interval_from_chord_step(&self, step: u8) -> Option<Interval> {
        let root = self.root()?;
        self.pitch_refs()
            .filter_map(|pitch| Interval::between_pitches(root, pitch).ok())
            .find(|interval| interval.mod7() == IntegerType::from(step))
    }

    /// Returns the chord in closed position with every repeated step raised
    /// an octave, so an eight-note cluster spreads into a scale: music21's
    /// `semiClosedPosition`.
    pub fn semi_closed_position(
        &self,
        force_octave: Option<IntegerType>,
        leave_redundant_pitches: bool,
    ) -> Self {
        let mut chord = self.closed_position(force_octave, leave_redundant_pitches);
        let implicit_octave = crate::defaults::PITCH_OCTAVE as IntegerType;
        let mut remaining: Vec<usize> = (0..chord.notes.len()).collect();
        while !remaining.is_empty() {
            let mut used_steps = Vec::new();
            let mut still_clashing = Vec::new();
            for index in remaining {
                let pitch = &mut chord.notes[index].pitch;
                let step = root::diatonic_note_number(pitch).rem_euclid(7);
                if used_steps.contains(&step) {
                    let octave = pitch.octave().unwrap_or(implicit_octave) + 1;
                    pitch.octave_setter(Some(octave));
                    still_clashing.push(index);
                } else {
                    used_steps.push(step);
                }
            }
            remaining = still_clashing;
        }
        chord.sort_ascending_in_place();
        chord
    }

    /// Returns a copy sorted by pitch space alone, so enharmonic pairs keep
    /// their input order: music21's `sortChromaticAscending`.
    pub fn sort_chromatic_ascending(&self) -> Self {
        let mut chord = self.clone();
        chord.notes.sort_by(|left, right| {
            left.pitch
                .ps()
                .partial_cmp(&right.pitch.ps())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        chord
    }

    /// Returns a copy sorted by staff position and then pitch space, so
    /// `B#3` sorts below `C4`: music21's `sortDiatonicAscending`, which is
    /// also what [`Self::sort_ascending`] does.
    pub fn sort_diatonic_ascending(&self) -> Self {
        self.sort_ascending()
    }

    /// Returns a copy sorted by frequency: music21's `sortFrequencyAscending`.
    pub fn sort_frequency_ascending(&self) -> Self {
        let mut chord = self.clone();
        chord.notes.sort_by(|left, right| {
            left.pitch
                .frequency_hz()
                .partial_cmp(&right.pitch.frequency_hz())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        chord
    }

    /// Returns the pitch names in input order, without octaves.
    pub fn pitch_names(&self) -> Vec<String> {
        self.notes.iter().map(|note| note.pitch.name()).collect()
    }

    /// The pitch class of every note, in the order the chord holds them and
    /// with repeats kept: music21's `pitchClasses`. For the sorted, distinct
    /// list see [`Self::pitch_classes`].
    pub fn note_pitch_classes(&self) -> Vec<u8> {
        self.notes
            .iter()
            .map(|note| root::pitch_class(&note.pitch))
            .collect()
    }

    /// Returns music21's `fullName`: the pitches' full names between braces,
    /// followed by the duration's full name when the chord has a duration
    /// that music21 can name.
    pub fn full_name(&self) -> String {
        let pitches = self
            .notes
            .iter()
            .map(|note| note.pitch.full_name())
            .collect::<Vec<_>>()
            .join(" | ");
        let duration = self
            .duration
            .clone()
            .unwrap_or_else(Duration::quarter)
            .full_name();
        format!("Chord {{{pitches}}} {duration}")
    }
}

#[cfg(test)]
mod notation_tests {
    use super::*;
    use crate::notation::TieType;

    #[test]
    fn annotate_intervals_matches_music21() {
        let triad = Chord::new("C4 E4 G4").unwrap();
        assert_eq!(triad.annotate_intervals(true, true).unwrap(), ["5", "3"]);
        assert_eq!(triad.annotate_intervals(false, true).unwrap(), ["P5", "M3"]);
        let with_octave = Chord::new("C4 E4 G4 C5").unwrap();
        assert_eq!(
            with_octave.annotate_intervals(true, true).unwrap(),
            ["8", "5", "3"]
        );
        assert_eq!(
            with_octave.annotate_intervals(false, true).unwrap(),
            ["P8", "P5", "M3"]
        );
        // A pitch repeated at the same octave drops out before the
        // intervals are read; one an octave up is a tenth and stays, which
        // reads as another third.
        let doubled = Chord::new("C4 E4 G4 E4").unwrap();
        assert_eq!(doubled.annotate_intervals(true, true).unwrap(), ["5", "3"]);
        let spread = Chord::new("C4 E4 G4 E5").unwrap();
        assert_eq!(
            spread.annotate_intervals(true, true).unwrap(),
            ["5", "3", "3"]
        );
        assert!(
            Chord::empty()
                .annotate_intervals(true, true)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn annotated_intervals_become_lyrics() {
        let annotated = Chord::new("C4 E4 G4")
            .unwrap()
            .annotated_with_intervals(true, true)
            .unwrap();
        let texts: Vec<String> = annotated.lyrics().iter().map(Lyric::text).collect();
        assert_eq!(texts, ["5", "3"]);
        assert_eq!(annotated.lyrics()[1].number(), 2);
    }

    #[test]
    fn component_volumes_average_into_the_chord() {
        let mut chord = Chord::new("C4 E4 G4").unwrap();
        assert!(!chord.has_component_volumes());
        assert_eq!(chord.volume().velocity(), None);

        chord
            .set_volumes(&[
                Volume::from_velocity(60),
                Volume::from_velocity(20),
                Volume::from_velocity(120),
            ])
            .unwrap();
        assert!(chord.has_component_volumes());
        let velocities: Vec<Option<IntegerType>> = chord
            .notes()
            .iter()
            .map(|note| note.volume().velocity())
            .collect();
        assert_eq!(velocities, [Some(60), Some(20), Some(120)]);
        assert_eq!(chord.volume().velocity(), Some(67));

        // A volume set on the chord replaces the ones on its notes.
        chord.set_volume(Some(Volume::from_velocity(90)));
        assert!(!chord.has_component_volumes());
        assert_eq!(chord.volume().velocity(), Some(90));
        assert!(chord.set_volumes(&[]).is_err());
    }

    #[test]
    fn a_shorter_volume_list_cycles() {
        let mut chord = Chord::new("C4 E4 G4 B-4").unwrap();
        chord
            .set_volumes(&[Volume::from_velocity(40), Volume::from_velocity(80)])
            .unwrap();
        let velocities: Vec<Option<IntegerType>> = chord
            .notes()
            .iter()
            .map(|note| note.volume().velocity())
            .collect();
        assert_eq!(velocities, [Some(40), Some(80), Some(40), Some(80)]);
    }

    #[test]
    fn ties_and_colours_reach_the_notes() {
        let mut chord = Chord::new("C4 E4 G4").unwrap();
        assert!(chord.tie().is_none());
        chord.set_tie(Some(Tie::new(TieType::Start)));
        assert_eq!(chord.tie().map(Tie::tie_type), Some(TieType::Start));
        assert!(chord.notes().iter().all(|note| note.tie().is_some()));
        chord.set_tie(None);
        assert!(chord.tie().is_none());

        let e4 = Pitch::from_name("E4").unwrap();
        chord.set_color(Some("blue".to_string()));
        assert_eq!(chord.color_of_pitch(&e4), Some("blue"));
        chord
            .note_for_pitch_mut(&e4)
            .unwrap()
            .set_color(Some("red".to_string()));
        assert_eq!(chord.color_of_pitch(&e4), Some("red"));
        let c4 = Pitch::from_name("C4").unwrap();
        assert_eq!(chord.color_of_pitch(&c4), Some("blue"));
    }

    #[test]
    fn a_diamond_notehead_reads_as_a_string_harmonic() {
        let mut chord = Chord::new("D3 G3").unwrap();
        assert!(chord.string_harmonic().unwrap().is_none());
        chord.notes_mut()[1].set_notehead(Notehead::Diamond);
        let sounded = chord.string_harmonic().unwrap().unwrap();
        assert_eq!(
            sounded
                .pitches()
                .iter()
                .map(Pitch::name_with_octave)
                .collect::<Vec<_>>(),
            ["D3", "G3", "D5"]
        );
        let sounding = &sounded.notes()[2];
        assert!(sounding.notehead_parenthesis());
        assert_eq!(sounding.stem_direction(), StemDirection::NoStem);
        assert_eq!(sounded.notes()[1].notehead(), Notehead::Diamond);
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_chord_indexes_and_iterates_over_its_notes() {
        let chord = Chord::new("C4 E4 G4").unwrap();

        assert_eq!(chord.len(), 3);
        assert!(!chord.is_empty());
        assert_eq!(chord[1].pitch.name_with_octave(), "E4");

        let borrowed: Vec<String> = (&chord)
            .into_iter()
            .map(|note| note.pitch.name_with_octave())
            .collect();
        assert_eq!(borrowed, vec!["C4", "E4", "G4"]);
        assert_eq!(chord.into_iter().count(), 3);
    }

    #[test]
    fn forte_and_interval_vector_constructors_match_music21() {
        let names = |chord: &Chord| chord.pitch_names();
        assert_eq!(
            names(&Chord::from_forte_class("3-11").unwrap()),
            ["C", "E-", "G"]
        );
        assert_eq!(
            names(&Chord::from_forte_class("3-11B").unwrap()),
            ["C", "E", "G"]
        );
        assert_eq!(
            names(&Chord::from_forte_class("3-11a").unwrap()),
            ["C", "E-", "G"]
        );
        assert_eq!(
            names(&Chord::from_forte_address(4, 27, Some(-1)).unwrap()),
            ["C", "E-", "G-", "A-"]
        );
        assert_eq!(
            Chord::from_forte_class("3-11").unwrap().prime_form_string(),
            "<037>"
        );
        assert!(Chord::from_forte_class("311").is_err());
        assert!(Chord::from_forte_class("3-99").is_err());

        assert_eq!(
            names(&Chord::from_interval_vector(&[0, 0, 1, 1, 1, 0], false).unwrap()),
            ["C", "E-", "G"]
        );
        assert_eq!(
            names(&Chord::from_interval_vector(&[1, 1, 1, 1, 1, 1], false).unwrap()),
            ["C", "C#", "E", "F#"]
        );
        assert_eq!(
            names(&Chord::from_interval_vector(&[1, 1, 1, 1, 1, 1], true).unwrap()),
            ["C", "D-", "E-", "G"]
        );
        assert!(Chord::from_interval_vector(&[9, 9, 9, 9, 9, 9], false).is_none());
    }

    #[test]
    fn geometric_normal_form_matches_music21() {
        let cases: [(&str, &[u8]); 9] = [
            ("C4 E4 G4", &[0, 3, 8]),
            ("E4 G4 C5", &[0, 3, 8]),
            ("C4 D-4 E4 G-4", &[0, 1, 4, 6]),
            ("C4 E-4 G-4 A4", &[0, 3, 6, 9]),
            ("C4", &[0]),
            ("C4 C5", &[0]),
            ("B3 C4 E4", &[0, 1, 5]),
            ("F#4 A4 C5 E-5", &[0, 3, 6, 9]),
            ("C4 D4 E4 F4 G4 A4 B4", &[0, 1, 3, 5, 6, 8, 10]),
        ];
        for (notes, expected) in cases {
            assert_eq!(
                Chord::new(notes).unwrap().geometric_normal_form(),
                expected,
                "{notes}"
            );
        }
        assert!(Chord::empty().geometric_normal_form().is_empty());
    }

    #[test]
    #[allow(clippy::type_complexity)]
    fn set_class_strings_and_flags_match_music21() {
        let cases: [(&str, &str, &str, u8, &str, usize, bool, bool, &str, bool); 14] = [
            (
                "C4 E4 G4",
                "<001110>",
                "<047>",
                11,
                "3-11B",
                3,
                true,
                false,
                "Root Position",
                false,
            ),
            (
                "C4 E-4 G-4 B--4",
                "<004002>",
                "<0369>",
                28,
                "4-28",
                4,
                false,
                false,
                "Root Position",
                true,
            ),
            (
                "C4 E4 G4 B-4",
                "<012111>",
                "<47A0>",
                27,
                "4-27B",
                4,
                true,
                false,
                "Root Position",
                false,
            ),
            (
                "C3 G3 E4 C5",
                "<001110>",
                "<047>",
                11,
                "3-11B",
                4,
                true,
                false,
                "Root Position",
                false,
            ),
            (
                "E-4 G4 C5",
                "<001110>",
                "<037>",
                11,
                "3-11A",
                3,
                false,
                false,
                "First Inversion",
                false,
            ),
            (
                "C4 D-4 E4 G-4",
                "<111111>",
                "<0146>",
                15,
                "4-15A",
                4,
                false,
                true,
                "Root Position",
                false,
            ),
            (
                "C4 D-4 E-4 G4",
                "<111111>",
                "<0137>",
                29,
                "4-29A",
                4,
                false,
                true,
                "Root Position",
                false,
            ),
            (
                "C4 C#4 D4 E4 F#4 G4 A4 B4",
                "<465472>",
                "<B0124679>",
                23,
                "8-23",
                8,
                false,
                false,
                "Root Position",
                false,
            ),
            (
                "G3 C4 E4",
                "<001110>",
                "<047>",
                11,
                "3-11B",
                3,
                true,
                false,
                "Second Inversion",
                false,
            ),
            (
                "B#3 C4 E4 G-4 F#4",
                "<010101>",
                "<046>",
                8,
                "3-8B",
                5,
                true,
                false,
                "Third Inversion",
                false,
            ),
            (
                "C4 E-4 G-4 A4",
                "<004002>",
                "<0369>",
                28,
                "4-28",
                4,
                false,
                false,
                "First Inversion",
                true,
            ),
            (
                "C4",
                "<000000>",
                "<0>",
                1,
                "1-1",
                1,
                false,
                false,
                "Root Position",
                false,
            ),
            (
                "C4 F4 G4",
                "<010020>",
                "<570>",
                9,
                "3-9",
                3,
                false,
                false,
                "Second Inversion",
                false,
            ),
            (
                "D4 F4 A-4 C-5",
                "<004002>",
                "<258B>",
                28,
                "4-28",
                4,
                false,
                false,
                "Root Position",
                true,
            ),
        ];
        for (
            notes,
            vector,
            normal_order,
            forte_number,
            forte_tn,
            cardinality,
            prime_inversion,
            z_relation,
            inversion_text,
            false_diminished,
        ) in cases
        {
            let chord = Chord::new(notes).unwrap();
            assert_eq!(chord.interval_vector_string(), vector, "{notes}");
            assert_eq!(chord.normal_order_string(), normal_order, "{notes}");
            assert_eq!(chord.forte_class_number(), Some(forte_number), "{notes}");
            assert_eq!(chord.forte_class_tn().as_deref(), Some(forte_tn), "{notes}");
            assert_eq!(chord.multiset_cardinality(), cardinality, "{notes}");
            assert_eq!(chord.is_prime_form_inversion(), prime_inversion, "{notes}");
            assert_eq!(chord.has_z_relation(), z_relation, "{notes}");
            assert_eq!(chord.inversion_text(), inversion_text, "{notes}");
            assert_eq!(
                chord.is_false_diminished_seventh(),
                false_diminished,
                "{notes}"
            );
        }

        let empty = Chord::empty();
        assert_eq!(empty.interval_vector_string(), "<000000>");
        assert_eq!(empty.normal_order_string(), "<>");
        assert_eq!(empty.forte_class_number(), None);
        assert_eq!(empty.multiset_cardinality(), 0);
        assert!(!empty.has_z_relation());
        assert_eq!(empty.inversion_text(), "Unknown Position");
    }

    #[test]
    fn z_relations_pair_up_like_music21() {
        let z15 = Chord::new("C4 D-4 E4 G-4").unwrap();
        let z29 = Chord::new("C4 D-4 E-4 G4").unwrap();
        let triad = Chord::new("C E G").unwrap();
        assert!(z15.are_z_relations(&z29));
        assert!(z29.are_z_relations(&z15));
        assert!(!z15.are_z_relations(&triad));
        assert!(!triad.are_z_relations(&z15));
    }

    #[test]
    fn interval_from_chord_step_matches_music21() {
        let cases: [(&str, Option<&str>, Option<&str>); 9] = [
            ("C4 E4 G4", Some("M3"), Some("P5")),
            ("C4 E-4 G-4 B--4", Some("m3"), Some("d5")),
            ("C3 G3 E4 C5", Some("M10"), Some("P5")),
            ("E-4 G4 C5", Some("M6"), Some("P4")),
            ("C4 D-4 E4 G-4", Some("M3"), Some("d5")),
            ("C4 E-4 G-4 A4", Some("M6"), Some("A4")),
            ("A2 C#4 E4 G4", Some("M10"), Some("P12")),
            ("C4", None, None),
            ("C4 F4 G4", None, Some("P4")),
        ];
        for (notes, third, fifth) in cases {
            let chord = Chord::new(notes).unwrap();
            let name = |interval: Option<Interval>| interval.map(|interval| interval.short_name());
            assert_eq!(
                name(chord.interval_from_chord_step(3)).as_deref(),
                third,
                "{notes}"
            );
            assert_eq!(
                name(chord.interval_from_chord_step(5)).as_deref(),
                fifth,
                "{notes}"
            );
        }
        assert!(Chord::empty().interval_from_chord_step(3).is_none());
    }

    fn octave_names(chord: &Chord) -> Vec<String> {
        chord
            .pitches()
            .iter()
            .map(Pitch::name_with_octave)
            .collect()
    }

    #[test]
    fn semi_closed_position_matches_music21() {
        let cases: [(&str, &[&str], &[&str]); 7] = [
            ("C4 E4 G4", &["C4", "E4", "G4"], &["C3", "E3", "G3"]),
            ("C3 G3 E4 C5", &["C3", "E3", "G3"], &["C3", "E3", "G3"]),
            ("E-4 G4 C5", &["E-4", "G4", "C5"], &["E-3", "G3", "C4"]),
            (
                "C4 C#4 D4 E4 F#4 G4 A4 B4",
                &["C4", "D4", "E4", "F#4", "G4", "A4", "B4", "C#5"],
                &["C3", "D3", "E3", "F#3", "G3", "A3", "B3", "C#4"],
            ),
            ("C4 E4 G4 C5 E5", &["C4", "E4", "G4"], &["C3", "E3", "G3"]),
            (
                "B#3 C4 E4 G-4 F#4",
                &["B#3", "C4", "E4", "F#4", "G-4"],
                &["B#3", "C4", "E4", "F#4", "G-4"],
            ),
            (
                "E4 G#4 B4 D5 F5",
                &["E4", "F4", "G#4", "B4", "D5"],
                &["E3", "F3", "G#3", "B3", "D4"],
            ),
        ];
        for (notes, expected, expected_forced) in cases {
            let chord = Chord::new(notes).unwrap();
            assert_eq!(
                octave_names(&chord.semi_closed_position(None, false)),
                expected,
                "{notes}"
            );
            assert_eq!(
                octave_names(&chord.semi_closed_position(Some(3), false)),
                expected_forced,
                "{notes} forced to octave 3"
            );
        }
    }

    #[test]
    fn sort_variants_match_music21() {
        let chord = Chord::new("B#3 C4 E4 G-4 F#4").unwrap();
        assert_eq!(
            octave_names(&chord.sort_chromatic_ascending()),
            ["B#3", "C4", "E4", "G-4", "F#4"]
        );
        assert_eq!(
            octave_names(&chord.sort_diatonic_ascending()),
            ["B#3", "C4", "E4", "F#4", "G-4"]
        );
        assert_eq!(
            octave_names(&chord.sort_frequency_ascending()),
            ["B#3", "C4", "E4", "G-4", "F#4"]
        );
        let spread = Chord::new("C5 G3 E4 C3").unwrap();
        assert_eq!(
            octave_names(&spread.sort_chromatic_ascending()),
            ["C3", "G3", "E4", "C5"]
        );
        assert_eq!(
            octave_names(&spread.sort_frequency_ascending()),
            ["C3", "G3", "E4", "C5"]
        );
        assert_eq!(chord.pitch_names(), ["B#", "C", "E", "G-", "F#"]);
    }

    #[test]
    fn full_name_matches_music21() {
        // A chord with no duration of its own reads as a quarter, which is
        // the duration music21 gives every chord by default.
        let chord = Chord::new("C4 E-4 G-4 B--4").unwrap();
        assert_eq!(
            chord.full_name(),
            "Chord {C in octave 4 | E-flat in octave 4 | G-flat in octave 4 | B-double-flat in octave 4} Quarter"
        );
        let quarter = chord.clone().with_duration(Duration::quarter());
        assert_eq!(
            quarter.full_name(),
            "Chord {C in octave 4 | E-flat in octave 4 | G-flat in octave 4 | B-double-flat in octave 4} Quarter"
        );
        let dotted = Chord::new("C4 E4 G4")
            .unwrap()
            .with_duration(Duration::new(1.5).unwrap());
        assert_eq!(
            dotted.full_name(),
            "Chord {C in octave 4 | E in octave 4 | G in octave 4} Dotted Quarter"
        );
    }
    use crate::{Duration, GuitarTuning, Interval, Key, Pitch, chord::Chord, chord::TriadQuality};

    #[test]
    fn a_chord_carries_notation_for_all_its_notes() {
        use crate::notation::{Beams, Notehead, StemDirection};

        let mut chord = Chord::new("C4 E4 G4").unwrap();
        assert_eq!(chord.notehead(), Notehead::Normal);
        chord.set_notehead(Notehead::Diamond);
        assert_eq!(chord.notehead(), Notehead::Diamond);
        assert_eq!(chord.notehead_fill(), None);
        chord.set_notehead_fill(Some(false));
        assert_eq!(chord.notehead_fill(), Some(false));
        assert!(!chord.notehead_parenthesis());
        chord.set_notehead_parenthesis(true);
        assert!(chord.notehead_parenthesis());
        chord.set_stem_direction(StemDirection::Down);
        assert_eq!(chord.stem_direction(), StemDirection::Down);
        assert!(chord.beams().is_empty());
        let mut beams = Beams::default();
        beams
            .fill(crate::duration::DurationType::Eighth, None)
            .unwrap();
        chord.set_beams(beams.clone());
        assert_eq!(chord.beams(), &beams);
    }

    #[test]
    fn notes_are_added_and_removed_as_music21_adds_and_removes_them() {
        let mut chord = Chord::new("C4 E4 G4").unwrap();
        chord.add("B-4").unwrap();
        assert_eq!(chord.pitch_names(), ["C", "E", "G", "B-"]);
        chord.remove(&Pitch::from_name("E4").unwrap()).unwrap();
        assert!(chord.remove(&Pitch::from_name("E4").unwrap()).is_err());
        chord.remove_named("G4").unwrap();
        assert!(chord.remove_named("G4").is_err());
        assert_eq!(chord.pitch_names(), ["C", "B-"]);
        assert_eq!(chord.note_pitch_classes(), [0, 10]);
        assert_eq!(
            Chord::new("E4 C4 G4 C5").unwrap().note_pitch_classes(),
            [4, 0, 7, 0]
        );
    }

    #[test]
    fn an_overridden_bass_or_root_is_kept_apart_from_the_inferred_one() {
        let mut chord = Chord::new("C4 E4 G4").unwrap();
        let e = Pitch::from_name("E4").unwrap();
        assert_eq!(chord.overridden_bass(), None);
        assert_eq!(chord.found_bass().map(Pitch::name), Some("C".to_string()));
        chord.set_bass(Some(e.clone()));
        assert_eq!(chord.overridden_bass(), Some(&e));
        assert_eq!(chord.bass().map(Pitch::name), Some("E".to_string()));
        assert_eq!(chord.found_bass().map(Pitch::name), Some("C".to_string()));
        chord.set_bass(None);
        assert_eq!(chord.overridden_bass(), None);
        // A bass the chord does not carry is added below the others.
        chord.set_bass(Some(Pitch::from_name("A3").unwrap()));
        assert_eq!(chord.pitch_names(), ["A", "C", "E", "G"]);

        let mut chord = Chord::new("C4 E4 G4").unwrap();
        assert_eq!(chord.overridden_root(), None);
        chord.set_root(Some(e.clone()));
        assert_eq!(chord.overridden_root(), Some(&e));
        assert_eq!(chord.root().map(Pitch::name), Some("E".to_string()));
        assert_eq!(chord.found_root().map(Pitch::name), Some("C".to_string()));
    }

    #[test]
    fn set_inversion_raises_the_bass_until_the_chord_stands_in_it() {
        let mut chord = Chord::new("C4 E4 G4").unwrap();
        chord.set_inversion(1).unwrap();
        assert_eq!(chord.inversion(), Some(1));
        assert_eq!(
            chord
                .pitches()
                .iter()
                .map(Pitch::name_with_octave)
                .collect::<Vec<_>>(),
            ["E4", "G4", "C5"]
        );
        chord.set_inversion(0).unwrap();
        assert_eq!(chord.bass().map(Pitch::name), Some("C".to_string()));
        assert!(chord.set_inversion(5).is_err());
    }

    #[test]
    fn chord_steps_can_be_measured_from_a_root_the_caller_names() {
        let chord = Chord::new("E4 G4 C5").unwrap();
        let c = Pitch::from_name("C4").unwrap();
        let g = Pitch::from_name("G4").unwrap();
        assert_eq!(chord.inversion_from_root(&c), Some(1));
        // A bass a sixth above the root is music21's sixth inversion.
        assert_eq!(chord.inversion_from_root(&g), Some(6));
        assert_eq!(
            chord.chord_step_with_root(3, &c).map(Pitch::name),
            Some("E".to_string())
        );
        assert_eq!(chord.chord_step_with_root(3, &g), None);
        assert_eq!(chord.semitones_from_chord_step_with_root(5, &c), Some(7));
        assert_eq!(chord.semitones_from_chord_step_with_root(3, &g), None);
    }

    #[test]
    fn the_reductions_report_what_they_dropped() {
        let names = |pitches: Vec<Pitch>| -> Vec<String> {
            pitches.iter().map(Pitch::name_with_octave).collect()
        };
        let (kept, dropped) = Chord::new("C4 E4 G4 C4")
            .unwrap()
            .remove_redundant_pitches_reporting();
        assert_eq!(kept.pitch_names(), ["C", "E", "G"]);
        assert_eq!(names(dropped), ["C4"]);
        let (kept, dropped) = Chord::new("C4 E4 G4 C5")
            .unwrap()
            .remove_redundant_pitch_names_reporting();
        assert_eq!(kept.pitch_names(), ["C", "E", "G"]);
        assert_eq!(names(dropped), ["C5"]);
        let (kept, dropped) = Chord::new("C4 E4 G4 B#4")
            .unwrap()
            .remove_redundant_pitch_classes_reporting();
        assert_eq!(kept.pitch_names(), ["C", "E", "G"]);
        assert_eq!(names(dropped), ["B#4"]);
    }

    #[test]
    fn the_chord_table_address_is_a_record_and_an_empty_chord_has_one() {
        let address = Chord::new("C E G").unwrap().chord_tables_address_entry();
        assert_eq!(address.cardinality, 3);
        assert_eq!(address.forte_class, 11);
        assert_eq!(address.inversion, -1);
        assert_eq!(address.pitch_class_original, 0);
        let empty = Chord::new("").unwrap().chord_tables_address_entry();
        assert_eq!(
            (
                empty.cardinality,
                empty.forte_class,
                empty.inversion,
                empty.pitch_class_original
            ),
            (0, 0, 0, 0)
        );
        assert_eq!(super::format_vector_string(&[0, 0, 1, 1, 1, 0]), "<001110>");
    }

    #[test]
    fn a_chord_is_built_from_any_of_the_inputs_try_from_accepts() {
        use crate::note::Note;

        let names = |chord: Chord| chord.pitch_names();
        assert_eq!(names(Chord::try_from("C E G").unwrap()), ["C", "E", "G"]);
        assert_eq!(
            names(Chord::try_from("C E G".to_string()).unwrap()),
            ["C", "E", "G"]
        );
        let pitches = [
            Pitch::from_name("C4").unwrap(),
            Pitch::from_name("E4").unwrap(),
        ];
        assert_eq!(names(Chord::try_from(&pitches[..]).unwrap()), ["C", "E"]);
        let notes = [Note::from_pitch(pitches[0].clone())];
        assert_eq!(names(Chord::try_from(&notes[..]).unwrap()), ["C"]);
        assert_eq!(names(Chord::try_from(&[60, 64][..]).unwrap()), ["C", "E"]);
        assert_eq!(names(Chord::try_from(&["C", "G"][..]).unwrap()), ["C", "G"]);
        let owned = ["D".to_string(), "A".to_string()];
        assert_eq!(names(Chord::try_from(&owned[..]).unwrap()), ["D", "A"]);
    }

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
                names(chord.closed_position(force_octave, false)),
                expected,
                "{notes}"
            );
        }
        assert!(
            Chord::empty()
                .closed_position(None, false)
                .notes()
                .is_empty()
        );
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
    fn inversions_match_music21() {
        let cases = [
            ("C E G", 0, "C", "C"),
            ("E G C", 0, "C", "C"),
            ("G C E", 0, "C", "C"),
            ("C4 E4 G4 B-4", 0, "C4", "C4"),
            ("E4 G4 B-4 C5", 1, "C5", "E4"),
            ("B-3 C4 E4 G4", 3, "C4", "B-3"),
            ("G3 C4 E4 B-4", 2, "C4", "G3"),
            ("A-4 C5 F#5", 1, "F#5", "A-4"),
            ("C5 F#5 A-5", 2, "F#5", "C5"),
            ("F#4 A-4 C5", 0, "F#4", "F#4"),
            ("C F G", 2, "F", "C"),
            ("C4 G4 E5", 0, "C4", "C4"),
            ("C", 0, "C", "C"),
            ("G C", 0, "C", "C"),
            ("E C", 0, "C", "C"),
            ("F A C E", 2, "F", "C"),
            ("E4 C5 G5", 1, "C5", "E4"),
            ("C E G A", 1, "A", "C"),
            ("C F# G", 2, "F#", "C"),
            ("D4 F#4 A4 C5 E5", 0, "D4", "D4"),
            ("A-3 C4 E-4 F#4", 1, "F#4", "A-3"),
            ("C4 A-4 E-5 F#5", 2, "F#5", "C4"),
            ("E3 G3 B-3 D-4", 0, "E3", "E3"),
        ];
        for (notes, inversion, root, bass) in cases {
            let chord = Chord::new(notes).unwrap();
            assert_eq!(chord.inversion(), Some(inversion), "{notes} inversion");
            assert_eq!(
                chord.root().map(Pitch::name_with_octave).as_deref(),
                Some(root),
                "{notes} root"
            );
            assert_eq!(
                chord.bass().map(Pitch::name_with_octave).as_deref(),
                Some(bass),
                "{notes} bass"
            );
        }
        assert_eq!(Chord::empty().inversion(), None);
        assert_eq!(
            Chord::new("B#3 C4 E4")
                .unwrap()
                .bass()
                .unwrap()
                .name_with_octave(),
            "B#3"
        );
    }

    #[test]
    fn augmented_sixths_and_set_class_helpers_match_music21() {
        struct Case {
            notes: &'static str,
            flags: [bool; 11],
            prime_form: &'static str,
            forte_tni: &'static str,
            cardinality: usize,
            ordered: &'static str,
        }
        let t = true;
        let f = false;
        let cases = [
            Case {
                notes: "A-4 C5 F#5",
                flags: [t, t, f, f, f, t, t, f, f, f, f],
                prime_form: "<026>",
                forte_tni: "3-8",
                cardinality: 3,
                ordered: "<068>",
            },
            Case {
                notes: "A-4 C5 D5 F#5",
                flags: [t, f, t, f, f, t, f, f, t, f, f],
                prime_form: "<0268>",
                forte_tni: "4-25",
                cardinality: 4,
                ordered: "<0268>",
            },
            Case {
                notes: "A-4 C5 E-5 F#5",
                flags: [t, f, f, t, f, t, f, f, f, f, f],
                prime_form: "<0258>",
                forte_tni: "4-27",
                cardinality: 4,
                ordered: "<0368>",
            },
            Case {
                notes: "A-4 C5 D#5 F#5",
                flags: [t, f, f, f, t, t, f, f, f, f, f],
                prime_form: "<0258>",
                forte_tni: "4-27",
                cardinality: 4,
                ordered: "<0368>",
            },
            Case {
                notes: "C4 E4 G4",
                flags: [f, f, f, f, f, f, f, f, f, t, t],
                prime_form: "<037>",
                forte_tni: "3-11",
                cardinality: 3,
                ordered: "<047>",
            },
            Case {
                notes: "F#4 A-4 C5",
                flags: [f, f, f, f, f, t, t, f, f, f, f],
                prime_form: "<026>",
                forte_tni: "3-8",
                cardinality: 3,
                ordered: "<068>",
            },
            Case {
                notes: "C5 F#5 A-5",
                flags: [f, f, f, f, f, t, t, f, f, f, f],
                prime_form: "<026>",
                forte_tni: "3-8",
                cardinality: 3,
                ordered: "<068>",
            },
            Case {
                notes: "A-4 F#5 C6",
                flags: [t, t, f, f, f, t, t, f, f, f, f],
                prime_form: "<026>",
                forte_tni: "3-8",
                cardinality: 3,
                ordered: "<068>",
            },
            Case {
                notes: "A-4 C5 E-5 F#5 A-5",
                flags: [t, f, f, t, f, t, f, f, f, f, f],
                prime_form: "<0258>",
                forte_tni: "4-27",
                cardinality: 4,
                ordered: "<0368>",
            },
            Case {
                notes: "C E G B- D",
                flags: [f, f, f, f, f, f, f, t, f, f, f],
                prime_form: "<02469>",
                forte_tni: "5-34",
                cardinality: 5,
                ordered: "<0247A>",
            },
            Case {
                notes: "C E G B D F",
                flags: [f, f, f, f, f, f, f, f, f, f, f],
                prime_form: "<013568>",
                forte_tni: "6-25",
                cardinality: 6,
                ordered: "<02457B>",
            },
            Case {
                notes: "C E G#",
                flags: [f, f, f, f, f, f, f, f, t, f, f],
                prime_form: "<048>",
                forte_tni: "3-12",
                cardinality: 3,
                ordered: "<048>",
            },
            Case {
                notes: "C E- G- B--",
                flags: [f, f, f, f, f, f, f, f, t, f, f],
                prime_form: "<0369>",
                forte_tni: "4-28",
                cardinality: 4,
                ordered: "<0369>",
            },
            Case {
                notes: "C D E F# G# A#",
                flags: [f, f, f, f, f, f, f, f, t, f, f],
                prime_form: "<02468A>",
                forte_tni: "6-35",
                cardinality: 6,
                ordered: "<02468A>",
            },
            Case {
                notes: "C C# D D# E F F# G G# A A# B",
                flags: [f, f, f, f, f, f, f, f, t, f, f],
                prime_form: "<0123456789AB>",
                forte_tni: "12-1",
                cardinality: 12,
                ordered: "<0123456789AB>",
            },
            Case {
                notes: "C",
                flags: [f, f, f, f, f, f, f, f, f, f, f],
                prime_form: "<0>",
                forte_tni: "1-1",
                cardinality: 1,
                ordered: "<0>",
            },
            Case {
                notes: "B- D F A-",
                flags: [f, f, f, f, f, f, f, f, f, t, f],
                prime_form: "<0258>",
                forte_tni: "4-27",
                cardinality: 4,
                ordered: "<258A>",
            },
            Case {
                notes: "C F#",
                flags: [f, f, f, f, f, f, f, f, t, f, f],
                prime_form: "<06>",
                forte_tni: "2-6",
                cardinality: 2,
                ordered: "<06>",
            },
        ];
        for case in cases {
            let chord = Chord::new(case.notes).unwrap();
            let notes = case.notes;
            let actual = [
                chord.is_augmented_sixth(false),
                chord.is_italian_augmented_sixth(false, false),
                chord.is_french_augmented_sixth(false),
                chord.is_german_augmented_sixth(false),
                chord.is_swiss_augmented_sixth(false),
                chord.is_augmented_sixth(true),
                chord.is_italian_augmented_sixth(true, false),
                chord.is_ninth(),
                chord.is_transpositionally_symmetrical(false),
                chord.can_be_dominant_v(),
                chord.can_be_tonic(),
            ];
            assert_eq!(actual, case.flags, "{notes}");
            assert_eq!(
                chord.prime_form_string(),
                case.prime_form,
                "{notes} prime form"
            );
            assert_eq!(
                chord.forte_class_tni().as_deref(),
                Some(case.forte_tni),
                "{notes} forte tni"
            );
            assert_eq!(chord.pitch_class_cardinality(), case.cardinality, "{notes}");
            assert_eq!(
                chord.ordered_pitch_classes_string(),
                case.ordered,
                "{notes}"
            );
        }

        assert!(
            Chord::new("C")
                .unwrap()
                .is_transpositionally_symmetrical(true)
        );
        assert!(
            !Chord::new("C D-")
                .unwrap()
                .is_transpositionally_symmetrical(false)
        );
        assert!(
            Chord::new("A-4 C5 D5 F#5")
                .unwrap()
                .is_transpositionally_symmetrical(false)
        );
        assert!(
            !Chord::new("A-4 C5 D5 F#5")
                .unwrap()
                .is_transpositionally_symmetrical(true)
        );
        assert!(
            Chord::new("C C# D D# E F F# G G# A A# B")
                .unwrap()
                .has_any_repeated_diatonic_note()
        );
        assert!(
            !Chord::new("C E G")
                .unwrap()
                .has_any_repeated_diatonic_note()
        );

        let empty = Chord::empty();
        assert!(empty.is_transpositionally_symmetrical(false));
        assert!(empty.prime_form().is_empty());
        assert_eq!(empty.prime_form_string(), "<>");
        assert_eq!(empty.forte_class_tni(), None);
        assert_eq!(empty.pitch_class_cardinality(), 0);
        assert_eq!(empty.ordered_pitch_classes_string(), "<>");
    }

    #[test]
    fn transpose_moves_every_note() {
        let names = |chord: Chord| {
            chord
                .pitches()
                .iter()
                .map(Pitch::name_with_octave)
                .collect::<Vec<_>>()
        };
        let chord = Chord::new("C4 E4 G4").unwrap();
        let up = |name: &str| {
            chord
                .transpose(&Interval::from_name(name).unwrap())
                .unwrap()
        };
        assert_eq!(names(up("M2")), vec!["D4", "F#4", "A4"]);
        assert_eq!(names(up("-m3")), vec!["A3", "C#4", "E4"]);
        assert_eq!(
            names(
                chord
                    .transpose(&Interval::from_semitones(3).unwrap())
                    .unwrap()
            ),
            vec!["E-4", "G4", "B-4"]
        );
        let bare = Chord::new("C E G").unwrap();
        assert_eq!(
            names(bare.transpose(&Interval::from_name("P5").unwrap()).unwrap()),
            vec!["G", "B", "D"]
        );
    }

    #[test]
    fn scale_degrees_match_music21() {
        let cases = [
            (
                "C",
                "C E G",
                vec![(Some(1), None), (Some(3), None), (Some(5), None)],
            ),
            (
                "C",
                "C E- G",
                vec![(Some(1), None), (Some(3), Some("flat")), (Some(5), None)],
            ),
            (
                "F",
                "F A C E",
                vec![
                    (Some(1), None),
                    (Some(3), None),
                    (Some(5), None),
                    (Some(7), None),
                ],
            ),
            (
                "a",
                "G# B D",
                vec![(Some(7), Some("sharp")), (Some(2), None), (Some(4), None)],
            ),
            (
                "B-",
                "E- G B-",
                vec![(Some(4), None), (Some(6), None), (Some(1), None)],
            ),
            (
                "D",
                "C# E G B-",
                vec![
                    (Some(7), None),
                    (Some(2), None),
                    (Some(4), None),
                    (Some(6), Some("flat")),
                ],
            ),
            (
                "C",
                "C E G B- D-",
                vec![
                    (Some(1), None),
                    (Some(3), None),
                    (Some(5), None),
                    (Some(7), Some("flat")),
                    (Some(2), Some("flat")),
                ],
            ),
        ];
        for (key, notes, expected) in cases {
            let scale = Key::from_tonic(key).unwrap().as_scale().unwrap();
            let degrees = Chord::new(notes)
                .unwrap()
                .scale_degrees(&scale)
                .unwrap()
                .into_iter()
                .map(|(degree, accidental)| (degree, accidental.map(|a| a.name().to_string())))
                .collect::<Vec<_>>();
            let expected = expected
                .into_iter()
                .map(|(degree, accidental): (Option<usize>, Option<&str>)| {
                    (degree, accidental.map(str::to_string))
                })
                .collect::<Vec<_>>();
            assert_eq!(degrees, expected, "{key} {notes}");
        }
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
        assert_eq!(chord.inversion_name().unwrap(), Some(53));
        assert_eq!(chord.inversion_text(), "Root Position");
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
        assert_eq!(chord.inversion_name().unwrap(), Some(6));
        assert_eq!(chord.inversion_text(), "First Inversion");
        assert_eq!(
            Chord::new("C E G B-").unwrap().inversion_name().unwrap(),
            Some(7)
        );
        // A chord that is neither a triad nor carries a seventh has no
        // figured-bass number, which music21 reports by raising.
        assert!(Chord::new("C D E").unwrap().inversion_name().is_err());
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
