//! Where a chord goes next: the resolutions of dominants, leading-tone
//! sonorities and augmented sixths, in a key and out of one.

use super::*;

impl Chord {
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

    pub(super) fn resolve_by_root_motion(&self, key: &Key, semitones: u8) -> Result<Option<Self>> {
        let Some(root_pitch) = self.find_root_pitch() else {
            return Ok(None);
        };
        let target_pc = (root::pitch_class(root_pitch) + semitones) % 12;
        Self::triad_for_key_pitch_class(key, target_pc)?
            .map(|chord| self.place_resolution_near_source(chord))
            .transpose()
    }

    pub(super) fn triad_for_key_pitch_class(key: &Key, target_pc: u8) -> Result<Option<Self>> {
        for degree in 1..=7 {
            let degree_pitch = key.pitch_from_degree(degree)?;
            if root::pitch_class(&degree_pitch) == target_pc {
                return Ok(Some(key.triad_from_degree(degree)?));
            }
        }
        Ok(None)
    }

    pub(super) fn place_resolution_near_source(&self, resolution: Self) -> Result<Self> {
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

    pub(super) fn pitch_center(pitches: &[Pitch]) -> Option<FloatType> {
        if pitches.is_empty() {
            return None;
        }

        Some(pitches.iter().map(Pitch::ps).sum::<FloatType>() / pitches.len() as FloatType)
    }

    pub(super) fn deduplicate_resolution_chords(chords: Vec<Self>) -> Vec<Self> {
        let mut seen = std::collections::BTreeSet::new();
        let mut deduped = Vec::new();

        for chord in chords {
            if seen.insert(chord.pitch_classes()) {
                deduped.push(chord);
            }
        }

        deduped
    }

    pub(super) fn augmented_sixth_contexts(&self) -> Result<Vec<(&'static str, &'static str)>> {
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

    pub(super) fn push_resolution_suggestion(
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

    pub(super) fn has_augmented_sixth_spelling(&self) -> bool {
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

    pub(super) fn is_directed_augmented_sixth(lower: &Pitch, upper: &Pitch) -> bool {
        let generic_interval = (root::step_num(upper) - root::step_num(lower)).rem_euclid(7) + 1;
        let semitones = ((upper.ps().round() as IntegerType) - (lower.ps().round() as IntegerType))
            .rem_euclid(12);
        generic_interval == 6 && semitones == 10
    }

    pub(super) fn add_resolution_suggestions_for_key(
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

    pub(super) fn is_dominant_function_sonority(&self) -> bool {
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

    pub(super) fn is_leading_tone_function_sonority(&self) -> bool {
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

    pub(super) fn is_contextual_augmented_sixth(&self, key: &Key) -> Result<bool> {
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

pub(super) const CANDIDATE_TONICS: [&str; 12] = [
    "C", "D-", "D", "E-", "E", "F", "F#", "G", "A-", "A", "B-", "B",
];
