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

    pub(super) fn spelling_root_name_override(&self, common_name: &str) -> Option<String> {
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
    pub(super) fn augmented_sixth_common_name(
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

    pub(super) fn spelling_common_name_override(&self) -> Option<String> {
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

    pub(super) fn dyad_common_name(&self) -> String {
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

    pub(super) fn root_pitch_name_from_tables(&self) -> Option<String> {
        self.find_root_pitch().map(Self::display_pitch_name)
    }

    pub(super) fn common_names_with_primary(&self) -> Vec<String> {
        let mut names = vec![self.common_name()];
        names.extend(self.common_names());
        names.sort();
        names.dedup();
        names
    }

    pub(super) fn pitch_class_name(pc: u8) -> &'static str {
        CANDIDATE_TONICS[pc as usize % 12]
    }

    /// Whether the chord is spelled with exactly these pitch names.
    ///
    /// The set of names it compares against is passed in rather than built
    /// here: the two spelling cascades ask this question up to fifteen times
    /// in a row, and building the set per question was most of what
    /// `common_name` cost.
    pub(super) fn names_are(
        &self,
        names: &std::collections::BTreeSet<String>,
        expected: &[&str],
    ) -> bool {
        self.notes.len() == expected.len() && expected.iter().all(|name| names.contains(*name))
    }

    pub(super) fn interval_nice_name(start: &Pitch, end: &Pitch) -> Option<String> {
        Interval::between(
            PitchOrNote::Pitch(start.clone()),
            PitchOrNote::Pitch(end.clone()),
        )
        .ok()
        .map(|interval| interval.nice_name())
    }

    pub(super) fn display_pitch_name(pitch: &Pitch) -> String {
        pitch.name().replace('-', "b")
    }

    pub(super) fn display_key_name(key: &Key) -> String {
        format!(
            "{} {}",
            Self::display_tonic_name(&key.tonic().name()),
            key.mode()
        )
    }

    pub(super) fn display_tonic_name(name: &str) -> String {
        name.replace('-', "b")
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
