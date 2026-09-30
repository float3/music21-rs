//! What a chord is called: the common names of the Forte tables spelled
//! on the chord's own root, and the lead-sheet symbol it is written as.

use super::*;

impl Chord {
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

    pub(super) fn pitched_name_for_common_name(&self, name_str: &str) -> String {
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
            // music21's `bass()`, which a bass set by hand answers.
            if let Some(bass) = self.bass() {
                return format!("{name_str} above {}", bass.name());
            }
            return name_str.to_string();
        }

        // music21's `root()`, which a root set by hand answers, or the first
        // note where no root is found.
        let root_name = self
            .root()
            .or_else(|| self.notes.first().map(|n| &n.pitch))
            .map(Pitch::name);

        match root_name {
            Some(root_name) => format!("{root_name}-{name_str}"),
            None => name_str.to_string(),
        }
    }

    pub(super) fn chord_symbol_root_pitch_class(root: PitchClassSpecifier) -> Result<u8> {
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

    pub(super) fn integer_pitch_class_from_value(pitch_class: PitchClass) -> Result<u8> {
        let Some(root) = pitch_class.integer() else {
            return Err(Error::Chord(
                "chord symbols require an integer pitch-class root".to_string(),
            ));
        };
        Ok(root as u8)
    }

    pub(super) fn integer_pitch_class_for_chord_symbol_root(ps: FloatType) -> Result<u8> {
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

            let (pitch_names, pitch_pses) = self.distinct_names_and_spaces();

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

        let address = match tables::seek_chord_tables_address(&ordered_pcs) {
            Ok(address) => address,
            Err(_) => return "unknown chord".to_string(),
        };

        // The names stay borrowed from the table until one of them is the
        // answer: a chord is named constantly, and writing every name a set
        // class carries out to hand back one of them is three or four
        // allocations for nothing.
        let common_names: Vec<&'static str> = match tables::address_to_common_names(address) {
            Ok(Some(names)) => names,
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
                        (*first).to_string()
                    } else {
                        format!("enharmonic equivalent to {first}")
                    };
                }
                if let Some(spelled) = self.spelled_as_named(forte_name) {
                    return if spelled {
                        (*first).to_string()
                    } else {
                        format!("enharmonic equivalent to {first}")
                    };
                }
            }
        }

        match common_names.first() {
            Some(name) => (*name).to_string(),
            None => match forte_name {
                Some(forte_name) => format!("forte class {forte_name}"),
                None => "unknown chord".to_string(),
            },
        }
    }

    /// music21 names the set classes that carry an augmented sixth by which
    /// augmented sixth they are actually spelled as, and falls back to
    /// `enharmonic to` the plain name when the spelling does not match.
    pub(super) fn augmented_sixth_common_name(
        &self,
        forte_name: &str,
        common_names: &[&str],
    ) -> Option<String> {
        let named = |index: usize| common_names.get(index).map(|name| (*name).to_string());
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
    pub(super) fn spelled_as_named(&self, forte_name: &str) -> Option<bool> {
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
    pub(super) fn is_seventh_with_perfect_fifths_above_root_and_third(&self) -> bool {
        if !self.is_seventh() {
            return false;
        }
        let names = self.pitch_names();
        // Through the pitch's own transpose, as music21 does: a pitch whose
        // spelling was inferred is respelled on the way, so the fifth above
        // an integer-built D# is B-, which the chord does not hold.
        let has_fifth_above = |pitch: &Pitch| {
            pitch
                .transpose(&PERFECT_FIFTH)
                .is_ok_and(|above| names.contains(&above.name()))
        };
        let (Some(root), Some(third)) = (self.root(), self.third()) else {
            return false;
        };
        has_fifth_above(root) && has_fifth_above(third)
    }

    /// The distinct pitch names and the distinct rounded pitch spaces among
    /// the notes: `C4 C5` is one name and two spaces.
    fn distinct_names_and_spaces(
        &self,
    ) -> (
        std::collections::BTreeSet<String>,
        std::collections::BTreeSet<IntegerType>,
    ) {
        let names = self.notes.iter().map(|n| n.pitch.name()).collect();
        let spaces = self
            .notes
            .iter()
            .map(|n| n.pitch.ps().round() as IntegerType)
            .collect();
        (names, spaces)
    }

    pub(super) fn dyad_common_name(&self) -> String {
        let (pitch_names, pitch_pses) = self.distinct_names_and_spaces();

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

        let relevant_interval = Interval::between(p0.clone(), p1.clone());

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
        self.bass_pitch().map(Pitch::name)
    }

    pub(super) fn root_pitch_name_from_tables(&self) -> Option<String> {
        self.find_root_pitch().map(Pitch::name)
    }

    pub(super) fn common_names_with_primary(&self) -> Vec<String> {
        let mut names = vec![self.common_name()];
        names.extend(self.common_names());
        names.sort();
        names.dedup();
        names
    }

    pub(super) fn interval_nice_name(start: &Pitch, end: &Pitch) -> Option<String> {
        Interval::between(start.clone(), end.clone())
            .ok()
            .map(|interval| interval.nice_name())
    }

    pub(super) fn display_key_name(key: &Key) -> String {
        format!("{} {}", key.tonic().name(), key.mode())
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

#[cfg(test)]
mod augmented_sixth_voicings {
    use crate::{Chord, defaults::IntegerType};

    /// music21's `commonName` for every voicing of the augmented-sixth
    /// spellings and their enharmonic neighbours, bass by bass, and for the
    /// same sets built from pitch-class numbers. Generated from music21.
    const EXPECTED: &str = "\
A#3 C#4 D#4 F#4	minor seventh chord
A#3 C#4 E#4 F#4	major seventh chord
A#3 C#4 E#4 G#4	minor seventh chord
A#3 C#4 G4	enharmonic equivalent to diminished triad
A#3 C4 E4 F#4	French augmented sixth chord in first inversion
A- C D F#	French augmented sixth chord in third inversion
A- C E- F#	German augmented sixth chord in second inversion
A- C F#	Italian augmented sixth chord in second inversion
A-3 C-4 E-4 F-4	major seventh chord
A-3 C-4 E-4 G-4	minor seventh chord
A-3 C4 D4 F#4	French augmented sixth chord
A-3 C4 E-4 F#4	German augmented sixth chord
A-3 C4 F#4	Italian augmented sixth chord
A3 B3 E-4 F#4	enharmonic to dominant seventh chord
A3 C#4 E-4 G4	French augmented sixth chord in root position
A3 E-4 F#4	enharmonic equivalent to diminished triad
B-3 C-4 E-4 G-4	major seventh chord
B-3 D4 E4 G#4	French augmented sixth chord
B3 C#4 E#4 G4	French augmented sixth chord in third inversion
B3 E-4 F#4 A4	enharmonic to dominant seventh chord
C D F# A-	French augmented sixth chord in third inversion
C E F# A#	French augmented sixth chord
C# D# F# A#	minor seventh chord
C# E# F# A#	major seventh chord
C# E# G B	French augmented sixth chord in root position
C# E# G# A#	minor seventh chord
C# E- G	Italian augmented sixth chord in root position
C# E- G A	French augmented sixth chord in first inversion
C# G A#	enharmonic equivalent to diminished triad
C#3 D#3 F#3 A#3	minor seventh chord
C#3 E#3 F#3 A#3	major seventh chord
C#3 E#3 G#3 A#3	minor seventh chord
C#3 E#3 G3 B3	French augmented sixth chord in root position
C#3 E-3 G3	Italian augmented sixth chord in root position
C#3 E-3 G3 A3	French augmented sixth chord in first inversion
C#3 G3 A#3	enharmonic equivalent to diminished triad
C-3 E-3 F-3 A-3	major seventh chord
C-3 E-3 G-3 A-3	minor seventh chord
C-3 E-3 G-3 B-3	major seventh chord
C3 D3 F#3 A-3	French augmented sixth chord in third inversion
C3 E-3 F#3 A-3	German augmented sixth chord in second inversion
C3 E3 F#3 A#3	French augmented sixth chord
C3 F#3 A-3	Italian augmented sixth chord in second inversion
D E G# B-	French augmented sixth chord in third inversion
D#3 F#3 A#3 C#4	minor seventh chord
D3 E3 G#3 B-3	French augmented sixth chord in third inversion
D3 F#3 A-3 C4	French augmented sixth chord in root position
E#3 F#3 A#3 C#4	major seventh chord
E#3 G#3 A#3 C#4	minor seventh chord
E#3 G3 B3 C#4	French augmented sixth chord in first inversion
E- F# A	enharmonic equivalent to diminished triad
E- F# A B	enharmonic to dominant seventh chord
E- F- A- C-	major seventh chord
E- G- A- C-	minor seventh chord
E- G- B- C-	major seventh chord
E-3 F#3 A-3 C4	German augmented sixth chord in third inversion
E-3 F#3 A3	enharmonic equivalent to diminished triad
E-3 F#3 A3 B3	enharmonic to dominant seventh chord
E-3 F-3 A-3 C-4	major seventh chord
E-3 G-3 A-3 C-4	minor seventh chord
E-3 G-3 B-3 C-4	major seventh chord
E-3 G3 A3 C#4	French augmented sixth chord
E-3 G3 C#4	Italian augmented sixth chord
E3 F#3 A#3 C4	French augmented sixth chord in third inversion
E3 G#3 B-3 D4	French augmented sixth chord in root position
F#3 A#3 C#4 D#4	minor seventh chord
F#3 A#3 C#4 E#4	major seventh chord
F#3 A#3 C4 E4	French augmented sixth chord in root position
F#3 A-3 C4	Italian augmented sixth chord in root position
F#3 A-3 C4 D4	French augmented sixth chord in first inversion
F#3 A-3 C4 E-4	German augmented sixth chord in root position
F#3 A3 B3 E-4	enharmonic to dominant seventh chord
F#3 A3 E-4	enharmonic equivalent to diminished triad
F-3 A-3 C-4 E-4	major seventh chord
G#3 A#3 C#4 E#4	minor seventh chord
G#3 B-3 D4 E4	French augmented sixth chord in first inversion
G-3 A-3 C-4 E-4	minor seventh chord
G-3 B-3 C-4 E-4	major seventh chord
G3 A#3 C#4	enharmonic equivalent to diminished triad
G3 A3 C#4 E-4	French augmented sixth chord in third inversion
G3 B3 C#4 E#4	French augmented sixth chord
G3 C#4 E-4	Italian augmented sixth chord in second inversion
[0, 2, 6, 8]	French augmented sixth chord in third inversion
[1, 3, 6, 10]	enharmonic equivalent to minor seventh chord
[1, 3, 7]	Italian augmented sixth chord in root position
[1, 5, 6, 10]	enharmonic equivalent to major seventh chord
[1, 5, 8, 10]	enharmonic equivalent to minor seventh chord
[1, 7, 10]	enharmonic equivalent to diminished triad
[3, 4, 8, 11]	enharmonic equivalent to major seventh chord
[3, 6, 10, 11]	enharmonic equivalent to major seventh chord
[3, 6, 8, 11]	enharmonic equivalent to minor seventh chord
[3, 6, 9]	enharmonic equivalent to diminished triad
";

    /// music21 puts the root it asks `root()` for in front of the name, and
    /// the bass `bass()` gives after `above`: overrides included, and for
    /// every voicing.
    #[test]
    fn pitched_names_read_the_root_music21_reads() {
        let cases = [
            (
                "C# E- G",
                "C#-Italian augmented sixth chord in root position",
            ),
            (
                "G4 C#5 E-5",
                "C#-Italian augmented sixth chord in second inversion",
            ),
            ("E-4 G4 C#5", "C#-Italian augmented sixth chord"),
            (
                "C D F# A-",
                "D-French augmented sixth chord in third inversion",
            ),
            ("A-4 C5 D5 F#5", "D-French augmented sixth chord"),
            (
                "F#4 A-4 C5 D5",
                "D-French augmented sixth chord in first inversion",
            ),
            (
                "D4 F#4 A-4 C5",
                "D-French augmented sixth chord in root position",
            ),
            (
                "C# E- G A",
                "A-French augmented sixth chord in first inversion",
            ),
            (
                "A4 C#5 E-5 G5",
                "A-French augmented sixth chord in root position",
            ),
            ("C E F# A#", "F#-French augmented sixth chord"),
            (
                "A#3 C4 E4 F#4",
                "F#-French augmented sixth chord in first inversion",
            ),
            (
                "C# E# G B",
                "C#-French augmented sixth chord in root position",
            ),
            (
                "B4 C#5 E#5 G5",
                "C#-French augmented sixth chord in third inversion",
            ),
            (
                "D E G# B-",
                "E-French augmented sixth chord in third inversion",
            ),
        ];
        for (notes, expected) in cases {
            assert_eq!(
                Chord::new(notes).unwrap().pitched_common_name(),
                expected,
                "{notes}"
            );
        }

        let mut rooted = Chord::new("F# A C# E#").unwrap();
        rooted.set_root(Some("A".parse().unwrap()));
        assert_eq!(rooted.pitched_common_name(), "A-minor-augmented tetrachord");

        let mut added = Chord::new("C E G B#").unwrap();
        added.set_bass(Some("E".parse().unwrap()));
        assert_eq!(
            added.pitched_common_name(),
            "enharmonic equivalent to major triad above E"
        );
    }

    #[test]
    fn every_voicing_is_named_as_music21_names_it() {
        let mut wrong = Vec::new();
        for line in EXPECTED.lines() {
            let (notes, expected) = line.split_once('\t').unwrap();
            let chord = if let Some(numbers) = notes.strip_prefix('[') {
                let classes: Vec<IntegerType> = numbers
                    .trim_end_matches(']')
                    .split(", ")
                    .map(|number| number.parse().unwrap())
                    .collect();
                Chord::new(classes.as_slice()).unwrap()
            } else {
                Chord::new(notes).unwrap()
            };
            let named = chord.common_name();
            if named != expected {
                wrong.push(format!("{notes}: {named}, music21 {expected}"));
            }
        }
        assert!(
            wrong.is_empty(),
            "{} of {}:\n{}",
            wrong.len(),
            EXPECTED.lines().count(),
            wrong.join("\n")
        );
    }
}
