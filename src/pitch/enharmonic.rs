//! Respelling a pitch: its enharmonics, the simplest of them, and the
//! spelling a set of pitches reads best in together.

use super::*;

impl Pitch {
    pub(super) fn get_all_common_enharmonics(
        &mut self,
        alter_limit: FloatType,
    ) -> Result<Vec<Pitch>> {
        let mut post = Vec::new();

        let simplified = self.clone().simplify_enharmonic(false)?;
        if simplified.name() != self.name() {
            post.push(simplified);
        }

        let mut higher = self.clone();
        while let Ok(next) = higher.get_higher_enharmonic() {
            if next.accidental.alter.abs() > alter_limit {
                break;
            }
            if post.contains(&next) {
                break;
            }
            post.push(next.clone());
            higher = next;
        }

        let mut lower = self.clone();
        while let Ok(next) = lower.get_lower_enharmonic() {
            if next.accidental.alter.abs() > alter_limit {
                break;
            }
            if post.contains(&next) {
                break;
            }
            post.push(next.clone());
            lower = next;
        }

        Ok(post)
    }

    /// Returns a simpler enharmonic spelling of this pitch.
    ///
    /// When `most_common` is true, common spellings such as `E-` are preferred
    /// over less common equivalents such as `D#`, following music21's
    /// `Pitch.simplifyEnharmonic` behavior.
    pub fn simplify_enharmonic(&self, most_common: bool) -> Result<Pitch> {
        let mut pitch = self.clone();
        pitch.simplify_enharmonic_in_place(most_common)?;
        Ok(pitch)
    }

    /// Simplifies this pitch's enharmonic spelling in place.
    pub fn simplify_enharmonic_in_place(&mut self, most_common: bool) -> Result<()> {
        const EXCLUDED_NAMES: [&str; 4] = ["E#", "B#", "C-", "F-"];
        if self.accidental.alter.abs().partial_cmp(&2.0) != Some(Ordering::Less)
            || EXCLUDED_NAMES.contains(&self.name().as_str())
        {
            // by resetting the pitch space value, we get a simpler enharmonic spelling
            let save_octave = self.octave;
            self.ps_setter(self.ps());
            if save_octave.is_none() {
                self.octave_setter(None);
            }
        }

        if most_common {
            match self.name().as_str() {
                "D#" => {
                    self.step_setter(StepName::E);
                    self.accidental_setter(Accidental::new("flat")?);
                }
                "A#" => {
                    self.step_setter(StepName::B);
                    self.accidental_setter(Accidental::new("flat")?);
                }
                "G-" => {
                    self.step_setter(StepName::F);
                    self.accidental_setter(Accidental::new("sharp")?);
                }
                "D-" => {
                    self.step_setter(StepName::C);
                    self.accidental_setter(Accidental::new("sharp")?);
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Returns the next higher enharmonic spelling.
    pub fn get_higher_enharmonic(&self) -> Result<Pitch> {
        self.enharmonic_neighbour(true)
    }

    /// Replaces this pitch with its next higher enharmonic spelling.
    pub fn get_higher_enharmonic_in_place(&mut self) -> Result<()> {
        self.enharmonic_neighbour_in_place(true)
    }

    /// Returns the next lower enharmonic spelling.
    pub fn get_lower_enharmonic(&self) -> Result<Pitch> {
        self.enharmonic_neighbour(false)
    }

    /// Replaces this pitch with its next lower enharmonic spelling.
    pub fn get_lower_enharmonic_in_place(&mut self) -> Result<()> {
        self.enharmonic_neighbour_in_place(false)
    }

    pub(super) fn enharmonic_neighbour(&self, up: bool) -> Result<Pitch> {
        let interval: &Interval = if up {
            &DIMINISHED_SECOND_UP
        } else {
            &DIMINISHED_SECOND_DOWN
        };

        let octave_stored = self.octave;

        let mut p = interval.transpose_pitch_with_options(self, false, None)?;
        if octave_stored.is_none() {
            p.octave_setter(None);
        }
        Ok(p)
    }

    pub(super) fn enharmonic_neighbour_in_place(&mut self, up: bool) -> Result<()> {
        *self = self.enharmonic_neighbour(up)?;
        Ok(())
    }

    /// Returns the enharmonic music21's `getEnharmonic` picks: sharps respell
    /// upward and flats downward, and a natural goes down for C, D and G and
    /// up for the rest, so C is B-sharp and E is F-flat.
    /// This pitch respelled to agree with a key signature, when the two
    /// disagree about the same sounding note.
    ///
    /// This is the rule music21 applies after transposing by a number of
    /// semitones: a `G-` in a key that writes an `F#` is written `F#`, and
    /// the other way round, because a chromatic step says how far to move
    /// and not how to spell what it lands on. A pitch with no accidental,
    /// or one the signature does not alter, is left as it is.
    pub fn respelled_for(&self, signature: &crate::key::KeySignature) -> Result<Pitch> {
        if !self.has_accidental() {
            return Ok(self.clone());
        }
        let alter = self.accidental().alter();
        for altered in signature.altered_pitches()? {
            if altered.pitch_class() == self.pitch_class() && altered.accidental().alter() != alter
            {
                return self.get_enharmonic();
            }
        }
        Ok(self.clone())
    }

    /// The next enharmonic spelling of this pitch: music21's `getEnharmonic`.
    ///
    /// A sharpened pitch respells on the letter above and a flattened one on
    /// the letter below; a natural takes whichever direction its letter has
    /// room for, so `C` answers `B#`.
    pub fn get_enharmonic(&self) -> Result<Pitch> {
        let alter = self.accidental.alter();
        let downward = if alter > 0.0 {
            false
        } else if alter < 0.0 {
            true
        } else {
            matches!(self.step.as_char(), 'C' | 'D' | 'G')
        };
        if downward {
            self.get_lower_enharmonic()
        } else {
            self.get_higher_enharmonic()
        }
    }

    /// Returns whether the two pitches sound the same. Without an octave on
    /// either side only the pitch class is compared.
    pub fn is_enharmonic(&self, other: &Pitch) -> bool {
        if self.octave.is_none() || other.octave.is_none() {
            (other.ps() - self.ps()).rem_euclid(12.0) == 0.0
        } else {
            other.ps() == self.ps()
        }
    }

    /// Returns the other spellings of this pitch with at most `alter_limit`
    /// sharps or flats, simplest first, as music21's `getAllCommonEnharmonics`
    /// lists them.
    pub fn all_common_enharmonics(&self, alter_limit: IntegerType) -> Vec<Pitch> {
        let mut found = Vec::new();
        if let Ok(simplified) = self.simplify_enharmonic(false)
            && simplified.name() != self.name()
        {
            found.push(simplified);
        }
        for upward in [true, false] {
            let mut current = self.clone();
            while let Ok(next) = current.enharmonic_neighbour(upward) {
                if next.accidental().alter().abs() > alter_limit as FloatType
                    || found.contains(&next)
                {
                    break;
                }
                found.push(next.clone());
                current = next;
            }
        }
        found
    }

    /// Returns this pitch moved down by octaves until it is at or below
    /// `target`. With `minimize` it is then raised back to within an octave.
    pub fn transpose_below_target(&self, target: &Pitch, minimize: bool) -> Result<Pitch> {
        let mut pitch = self.octave_bearing_copy("transposeBelowTarget")?;
        while pitch.ps() > target.ps() {
            pitch.shift_octave(-1);
        }
        if minimize {
            while target.ps() - pitch.ps() >= 12.0 {
                pitch.shift_octave(1);
            }
        }
        Ok(pitch)
    }

    /// Returns this pitch moved up by octaves until it is at or above
    /// `target`. With `minimize` it is then lowered back to within an octave.
    pub fn transpose_above_target(&self, target: &Pitch, minimize: bool) -> Result<Pitch> {
        let mut pitch = self.octave_bearing_copy("transposeAboveTarget")?;
        while pitch.ps() < target.ps() {
            pitch.shift_octave(1);
        }
        if minimize {
            while pitch.ps() - target.ps() >= 12.0 {
                pitch.shift_octave(-1);
            }
        }
        Ok(pitch)
    }
}

/// The two intervals enharmonic respelling can ever need: a diminished second
/// up and the same interval down.
pub(super) static DIMINISHED_SECOND_UP: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("d2").expect("d2 is a valid interval"));
pub(super) static DIMINISHED_SECOND_DOWN: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("-d2").expect("-d2 is a valid interval"));

/// A scoring function for [`simplify_multiple_enharmonics`]: lower is a
/// simpler spelling.
pub type CriterionFunction = fn(&[Pitch]) -> Result<FloatType>;

/// How far apart two dissonance scores must be to count as different: a
/// spelling within this of the best so far is a tie, and the first wins.
const SCORE_TOLERANCE: FloatType = 1e-9;

/// Respells a set of pitches so that they read as simply as possible
/// together: music21's `simplifyMultipleEnharmonics`. The first pitch is kept
/// as written and each of the others may be swapped for a common enharmonic;
/// the spelling chosen is the one the criterion scores lowest, by default
/// [`dissonance_score`]. Up to four pitches are searched exhaustively, more
/// are settled greedily one at a time, as upstream does. With a key
/// signature the tonic of its major key is placed first as an anchor and
/// removed again afterwards.
pub fn simplify_multiple_enharmonics(
    pitches: &[Pitch],
    criterion: Option<CriterionFunction>,
    key_context: Option<KeySignature>,
) -> Result<Vec<Pitch>> {
    let mut old_pitches: Vec<Pitch> = pitches.to_vec();
    if old_pitches.is_empty() {
        return Ok(Vec::new());
    }

    let criterion: CriterionFunction = criterion.unwrap_or(dissonance_score);

    let remove_first: bool = match key_context {
        Some(key) => {
            old_pitches.insert(0, key.as_key("major").tonic());
            true
        }
        None => false,
    };

    let mut simplified_pitches = match old_pitches.len() < 5 {
        true => brute_force_enharmonics_search(&mut old_pitches, criterion)?,
        false => greedy_enharmonics_search(&mut old_pitches, criterion)?,
    };

    for (new_p, old_p) in simplified_pitches.iter_mut().zip(old_pitches) {
        new_p.spelling_is_inferred = old_p.spelling_is_inferred;
    }

    if remove_first {
        let _ = simplified_pitches.remove(0);
    }

    Ok(simplified_pitches)
}

pub(super) fn brute_force_enharmonics_search(
    old_pitches: &mut [Pitch],
    score_func: CriterionFunction,
) -> Result<Vec<Pitch>> {
    let all_possible_pitches: Result<Vec<Vec<Pitch>>> = old_pitches[1..]
        .iter_mut()
        .map(|p| -> Result<Vec<Pitch>> {
            let mut enharmonics = p.get_all_common_enharmonics(2 as FloatType)?;
            enharmonics.insert(0, p.clone());
            Ok(enharmonics)
        })
        .collect();

    let all_pitch_combinations = all_possible_pitches?.into_iter().multi_cartesian_product();

    let mut min_score = FloatType::MAX;
    let mut best_combination: Vec<Pitch> = Vec::new();

    for combination in all_pitch_combinations {
        let mut pitches: Vec<Pitch> = old_pitches[..1].to_vec();
        pitches.extend(combination);
        let score = score_func(&pitches)?;
        // Two spellings that score the same keep the first, as music21's
        // `min` does; a difference in the last bits of a logarithm is not a
        // difference in dissonance.
        if score < min_score - SCORE_TOLERANCE {
            min_score = score;
            best_combination = pitches;
        }
    }

    Ok(best_combination)
}

pub(super) fn greedy_enharmonics_search(
    old_pitches: &mut [Pitch],
    score_func: CriterionFunction,
) -> Result<Vec<Pitch>> {
    let mut new_pitches = vec![];

    if let Some(first) = old_pitches.first() {
        new_pitches.push(first.clone());
    } else {
        return Err(Error::Pitch(
            "can't perform greedy enharmonics search on empty pitches".into(),
        ));
    }

    for old_pitch in old_pitches.iter_mut().skip(1) {
        let mut candidates = vec![old_pitch.clone()];
        candidates.extend(old_pitch.get_all_common_enharmonics(2 as FloatType)?);

        let mut best_candidate = None;
        let mut best_score: Option<OrderedFloat<FloatType>> = None;
        for candidate in candidates.iter() {
            let mut candidate_list = new_pitches.clone();
            candidate_list.push(candidate.clone());
            let score = score_func(&candidate_list)?;
            let score = OrderedFloat(score);
            if best_score.is_none_or(|best| score < best - SCORE_TOLERANCE) {
                best_score = Some(score);
                best_candidate = Some(candidate);
            }
        }
        let best_candidate = best_candidate
            .ok_or_else(|| Error::Pitch("candidates list is unexpectedly empty".to_string()))?;
        new_pitches.push(best_candidate.clone());
    }
    Ok(new_pitches)
}

/// How awkward a set of pitches reads together: music21's
/// `_dissonanceScore` with all three of its terms on. It averages a penalty
/// for accidentals beyond one sharp or flat, a penalty growing with the
/// denominator of each pair's Pythagorean ratio, and a reward for every
/// third and sixth, so that `C E G` scores below `C F- G`.
pub fn dissonance_score(pitches: &[Pitch]) -> Result<FloatType> {
    weighted_dissonance_score(pitches, true, true, true)
}

pub(super) fn weighted_dissonance_score(
    pitches: &[Pitch],
    small_pythagorean_ratio: bool,
    accidental_penalty: bool,
    triad_award: bool,
) -> Result<FloatType> {
    let mut score_accidentals: FloatType = 0.0;
    let mut score_ratio: FloatType = 0.0;
    let mut score_triad: FloatType = 0.0;

    if pitches.is_empty() {
        return Ok(0.0);
    }

    if accidental_penalty {
        let accidentals = pitches
            .iter()
            .map(|p| p.alter().abs())
            .collect::<Vec<FloatType>>();
        score_accidentals = accidentals
            .iter()
            .map(|a| if *a > 1.0 { *a } else { 0.0 })
            .sum::<FloatType>()
            / pitches.len() as FloatType;
    }

    let mut intervals: Vec<Interval> = vec![];

    if small_pythagorean_ratio | triad_award {
        for (index, p1) in pitches.iter().enumerate() {
            for p2 in pitches.iter().skip(index + 1) {
                let mut p2 = (*p2).clone();
                p2.octave_setter(None);
                let Ok(interval) = Interval::between(
                    PitchOrNote::Pitch(p1.clone()),
                    PitchOrNote::Pitch(p2.clone()),
                ) else {
                    return Ok(FloatType::INFINITY);
                };
                intervals.push(interval);
            }
        }

        if small_pythagorean_ratio {
            for interval in intervals.iter() {
                score_ratio += pythagorean_denominator_log(interval)? * 0.075_853_268_88
            }
            score_ratio /= pitches.len() as FloatType;
        }

        if triad_award {
            intervals.into_iter().for_each(|interval| {
                let simple_directed = interval.generic().simple_directed();
                let interval_semitones = interval.chromatic.whole_semitones() % 12;
                if (simple_directed == 3 && (interval_semitones == 3 || interval_semitones == 4))
                    || (simple_directed == 6
                        && (interval_semitones == 8 || interval_semitones == 9))
                {
                    score_triad -= 1.0;
                }
            });
            score_triad /= pitches.len() as FloatType;
        }
    }

    Ok((score_accidentals + score_ratio + score_triad)
        / (small_pythagorean_ratio as IntegerType
            + accidental_penalty as IntegerType
            + triad_award as IntegerType) as FloatType)
}

pub(super) fn pythagorean_denominator_log(interval: &Interval) -> Result<FloatType> {
    let start_pitch = Pitch::from_name("C1")?;
    let end_pitch = interval.transpose_pitch_with_options(&start_pitch, false, Some(4))?;

    let natural_fifths = match end_pitch.step() {
        StepName::C => 0,
        StepName::D => 2,
        StepName::E => 4,
        StepName::F => -1,
        StepName::G => 1,
        StepName::A => 3,
        StepName::B => 5,
    };
    let fifth_count = natural_fifths + (end_pitch.alter().round() as IntegerType * 7);
    let found_pitch_space = start_pitch.ps() + (7 * fifth_count) as FloatType;
    let octave_adjust = ((end_pitch.ps() - found_pitch_space) / 12.0).round() as IntegerType;

    let mut denominator_twos = if fifth_count > 0 { fifth_count } else { 0 };
    let denominator_threes = if fifth_count < 0 { -fifth_count } else { 0 };
    denominator_twos = (denominator_twos - octave_adjust).max(0);

    Ok(denominator_twos as FloatType * (2.0 as FloatType).ln()
        + denominator_threes as FloatType * (3.0 as FloatType).ln())
}
