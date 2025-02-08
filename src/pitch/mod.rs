pub(crate) mod accidental;
pub(crate) mod microtone;

use crate::{
    defaults::{self, IntegerType},
    exceptions::{Exception, ExceptionResult},
    interval::{interval_to_pythagorean_ratio, Interval, PitchOrNote},
    key::keysignature::KeySignature,
    note::Note,
    pitchclass::PitchClassString,
    prebase::{ProtoM21Object, ProtoM21ObjectTrait},
    stepname::StepName,
};
use std::sync::Arc;

use accidental::{Accidental, IntoAccidental};
use fraction::GenericFraction;
use itertools::Itertools;
use microtone::{IntoCentShift, Microtone};
use num::Num;
use ordered_float::OrderedFloat;

use self::defaults::{FloatType, Octave, PITCH_STEP};

#[derive(Clone, Debug)]
pub(crate) struct Pitch {
    proto: ProtoM21Object,
    _step: StepName,
    _octave: Octave,
    _overriden_freq440: Option<FloatType>,
    _accidental: Accidental,
    _microtone: Option<Microtone>,
    _client: Option<Arc<Note>>,
    spelling_is_infered: bool,
    fundamental: Option<Arc<Pitch>>,
}

impl PartialEq for Pitch {
    fn eq(&self, other: &Self) -> bool {
        self._step == other._step
            && self._octave == other._octave
            && self._accidental == other._accidental
            && self._microtone == other._microtone
    }
}

impl Pitch {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new<T, U, V>(
        name: Option<T>,
        step: Option<StepName>,
        octave: Octave,
        accidental: Option<U>,
        microtone: Option<V>,
        pitch_class: Option<PitchClassString>,
        midi: Option<IntegerType>,
        ps: Option<FloatType>,
        fundamental: Option<Pitch>,
    ) -> ExceptionResult<Self>
    where
        T: IntoPitchName,
        U: IntoAccidental,
        V: IntoCentShift,
    {
        let mut self_name = None;
        let mut self_step = PITCH_STEP;
        let mut self_accidental;
        let mut self_microtone: Option<Microtone> = None;
        let mut self_spelling_is_inferred = false;
        let mut self_octave = None;
        let self_pitch_class = pitch_class;
        let self_fundamental = fundamental;
        let self_midi = midi;
        let self_ps = ps;

        if let Some(name) = name {
            let x = name.into_name();
            self_name = x.name;
            if let Some(step) = x.step {
                self_step = step
            }

            if let Some(accidental) = x.accidental {
                self_accidental = accidental
            }

            if let Some(spelling) = x.spelling_is_inferred {
                self_spelling_is_inferred = spelling
            }
            self_octave = x.octave;
        } else if let Some(step) = step {
            self_step = step;
        };

        if let Some(octave) = octave {
            self_octave = Some(octave)
        }

        if let Some(accidental) = accidental {
            if accidental.is_accidental() {
                self_accidental = accidental.accidental();
            } else {
                self_accidental = Accidental::new(accidental)?;
            }
        } else {
            self_accidental = Accidental::new("natural")?;
        }

        if let Some(microtone) = microtone {
            if microtone.is_microtone() {
                self_microtone = Some(microtone.microtone());
            } else {
                self_microtone = Some(microtone.into_microtone()?);
            }
        }

        //we can't just assign here because the original library has a bunch of lgoic in the setters that we have to port

        let mut pitch = Pitch {
            proto: ProtoM21Object::new(),
            _step: self_step,
            _overriden_freq440: None,
            _accidental: self_accidental,
            _microtone: self_microtone,
            _octave: self_octave,
            _client: None,
            spelling_is_infered: self_spelling_is_inferred,
            fundamental: None,
        };

        //TODO implement all the setterse

        pitch.set_name(self_name);
        Ok(pitch)
    }

    pub(crate) fn name_with_octave(&self) -> String {
        todo!()
    }

    pub(crate) fn name(&self) -> String {
        match self.accidental() {
            Some(acc) => format!("{:?}{}", self._step, acc.modifier()),
            None => format!("{:?}", self._step),
        }
    }

    fn name_setter(&mut self, usr_str: &str) -> ExceptionResult<()> {
        let usr_str = usr_str.trim();

        let digit_index = usr_str
            .char_indices()
            .find(|&(_, c)| c.is_ascii_digit())
            .map(|(i, _)| i);

        let (pitch_part, octave_part) = if let Some(i) = digit_index {
            if i == 0 {
                return Err(Exception::Pitch(format!(
                    "Cannot have octave given before pitch name in {:?}",
                    usr_str
                )));
            }
            (&usr_str[..i], &usr_str[i..])
        } else {
            (usr_str, "")
        };

        // Process the pitch part.
        let mut pitch_chars = pitch_part.chars();
        let step = pitch_chars.next().ok_or(Exception::Pitch(format!(
            "Cannot make a name out of {:?}",
            pitch_part
        )))?;
        self.step_setter(step)?;

        let accidental_str: String = pitch_chars.collect();
        if accidental_str.is_empty() {
            self.accidental_setter(Accidental::natural())?;
        } else {
            self.accidental_setter(Accidental::new(accidental_str)?)?;
        }

        if !octave_part.is_empty() {
            let octave = octave_part.parse::<IntegerType>().map_err(|_| {
                Exception::Pitch(format!("Cannot parse {:?} to octave", octave_part))
            })?;
            self.octave_setter(Some(octave));
        }

        Ok(())
    }

    pub(crate) fn alter(&self) -> FloatType {
        let mut post = 0.0;

        post += self._accidental._alter;

        if let Some(microtone) = &self._microtone {
            post += microtone.alter();
        }

        post
    }

    pub(crate) fn octave_setter(&mut self, octave: Octave) {
        self._octave = octave;
        self.inform_client()
    }

    fn get_all_common_enharmonics(&self, alter_limit: IntegerType) -> Vec<Pitch> {
        todo!()
    }

    fn inform_client(&self) {
        if let Some(ref client) = self._client {
            client.pitch_changed();
        }
    }

    pub(crate) fn transpose(&self, clone: Interval) -> Pitch {
        todo!()
    }

    pub(crate) fn ps(&self) -> FloatType {
        todo!()
    }

    fn set_name(&mut self, self_name: Option<String>) {
        todo!()
    }

    fn accidental(&self) -> Option<Accidental> {
        todo!()
    }

    fn step_setter(&self, step: char) -> ExceptionResult<()> {
        todo!()
    }

    fn accidental_setter(&self, natural: Accidental) -> ExceptionResult<()> {
        todo!()
    }
}

impl ProtoM21ObjectTrait for Pitch {}

pub(crate) struct PitchParameteres {
    pub(crate) name: Option<String>,
    pub(crate) step: Option<StepName>,
    pub(crate) accidental: Option<Accidental>,
    pub(crate) spelling_is_inferred: Option<bool>,
    pub(crate) octave: Octave,
}

pub(crate) trait IntoPitchName {
    fn into_name(self) -> PitchParameteres;
}

impl IntoPitchName for Pitch {
    fn into_name(self) -> PitchParameteres {
        self.name_with_octave().into_name()
    }
}

impl IntoPitchName for IntegerType {
    fn into_name(self) -> PitchParameteres {
        let (step_name, accidental, _, _) = convert_ps_to_step(self);

        let octave = if self >= 12 {
            Some(self / 12 - 1)
        } else {
            None
        };

        PitchParameteres {
            name: None,
            step: Some(step_name),
            accidental: Some(accidental),
            spelling_is_inferred: Some(true),
            octave,
        }
    }
}

impl IntoPitchName for String {
    fn into_name(self) -> PitchParameteres {
        PitchParameteres {
            name: Some(self),
            step: None,
            accidental: None,
            spelling_is_inferred: None,
            octave: None,
        }
    }
}

impl IntoPitchName for &str {
    fn into_name(self) -> PitchParameteres {
        PitchParameteres {
            name: Some(self.to_string()),
            step: None,
            accidental: None,
            spelling_is_inferred: None,
            octave: None,
        }
    }
}

fn convert_ps_to_step<T: Num>(ps: T) -> (StepName, Accidental, Microtone, IntegerType) {
    todo!()
}

type CriterionFunction = fn(&[Pitch]) -> ExceptionResult<FloatType>;

pub(crate) fn simplify_multiple_enharmonics(
    pitches: Vec<Pitch>,
    criterion: Option<CriterionFunction>,
    key_context: Option<KeySignature>,
) -> ExceptionResult<Vec<Pitch>> {
    let mut old_pitches = pitches;

    let criterion: CriterionFunction =
        criterion.unwrap_or(|x: &[Pitch]| dissonance_score(x, true, true, true));

    match key_context {
        Some(key) => {
            old_pitches.insert(0, key.as_key("major").tonic());
        }
        None => todo!(),
    }

    if old_pitches.len() < 5 {
        brute_force_enharmonics_search(old_pitches, criterion)
    } else {
        greedy_enharmonics_search(old_pitches, criterion)
    }
}

fn brute_force_enharmonics_search(
    old_pitches: Vec<Pitch>,
    score_func: CriterionFunction,
) -> ExceptionResult<Vec<Pitch>> {
    let all_possible_pitches: Vec<Vec<Pitch>> = old_pitches[1..]
        .iter()
        .map(|p| {
            let mut enharmonics = p.get_all_common_enharmonics(2);
            enharmonics.insert(0, (*p).clone());
            enharmonics
        })
        .collect();

    let all_pitch_combinations = all_possible_pitches.into_iter().multi_cartesian_product();

    let mut min_score = FloatType::MAX;
    let mut best_combination: Vec<Pitch> = Vec::new();

    for combination in all_pitch_combinations {
        let mut pitches: Vec<Pitch> = old_pitches[..1].to_vec();
        pitches.extend(combination);
        let score = score_func(&pitches)?;
        if score < min_score {
            min_score = score;
            best_combination = pitches;
        }
    }

    Ok(best_combination)
}

fn greedy_enharmonics_search(
    old_pitches: Vec<Pitch>,
    score_func: CriterionFunction,
) -> ExceptionResult<Vec<Pitch>> {
    let mut new_pitches = vec![];

    if let Some(first) = old_pitches.first() {
        new_pitches.push(first.clone());
    } else {
        return Err(Exception::Pitch(
            "can't perform greedy enharmonics search on empty pitches".into(),
        ));
    }

    for old_pitch in old_pitches.iter().skip(1) {
        let mut candidates = vec![old_pitch.clone()];
        candidates.extend(old_pitch.get_all_common_enharmonics(2).into_iter());

        let mut best_candidate = None;
        let mut best_score: Option<OrderedFloat<FloatType>> = None;
        for candidate in candidates.iter() {
            let mut candidate_list = new_pitches.clone();
            candidate_list.push(candidate.clone());
            let score = score_func(&candidate_list)?;
            let score = OrderedFloat(score);
            if best_score.is_none() || score < best_score.unwrap() {
                best_score = Some(score);
                best_candidate = Some(candidate);
            }
        }
        let best_candidate = best_candidate
            .ok_or_else(|| Exception::Pitch("candidates list is unexpectedly empty".to_string()))?;
        new_pitches.push(best_candidate.clone());
    }
    Ok(new_pitches)
}

fn dissonance_score(
    pitches: &[Pitch],
    small_pythagorean_ratio: bool,
    accidental_penalty: bool,
    triad_award: bool,
) -> ExceptionResult<FloatType> {
    let mut score_accidentals = 0.0;
    let mut score_ratio = 0.0;
    let mut score_triad = 0.0;

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
            .sum::<f64>()
            / pitches.len() as FloatType;
    }

    let mut intervals: Vec<Interval> = vec![];

    if small_pythagorean_ratio | triad_award {
        for (index, p1) in pitches.iter().enumerate() {
            for p2 in pitches.iter().skip(index + 1) {
                let mut p1 = (*p1).clone();
                let mut p2 = (*p2).clone();
                p1.octave_setter(None);
                p2.octave_setter(None);
                intervals.push(Interval::between(
                    Some(PitchOrNote::Pitch(p1.clone())),
                    Some(PitchOrNote::Pitch(p2.clone())),
                )?);
            }
        }

        if small_pythagorean_ratio {
            for interval in intervals.iter() {
                score_ratio += (match interval_to_pythagorean_ratio(interval.clone())? {
                    GenericFraction::Rational(sign, ratio) => *ratio.denom(),
                    GenericFraction::Infinity(sign) => {
                        return Err(Exception::Pitch(format!(
                            "the ratio computed from {:?} is Infinity",
                            interval
                        )));
                    }
                    GenericFraction::NaN => {
                        return Err(Exception::Pitch(format!(
                            "the ratio comptued from {:?} is NaN",
                            interval
                        )));
                    }
                } as FloatType)
                    .ln()
                    * 0.03792663444
            }
            score_ratio /= pitches.len() as FloatType;
        }

        if triad_award {
            intervals.into_iter().for_each(|interval| {
                let simple_directed = interval.generic().simple_directed();
                let interval_semitones = interval.chromatic.semitones % 12;
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

fn convert_harmonic_to_cents(_harmonic_shift: IntegerType) -> IntegerType {
    todo!()
}

#[cfg(test)]
mod tests {
    use crate::defaults::IntegerType;

    use super::{simplify_multiple_enharmonics, Pitch};

    #[test]
    #[ignore]
    fn simplify_multiple_enharmonics_test() {
        let more_than_five = vec![
            Pitch::new(
                Some(0),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
            Pitch::new(
                Some(1),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
            Pitch::new(
                Some(2),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
            Pitch::new(
                Some(3),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
            Pitch::new(
                Some(4),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
            Pitch::new(
                Some(5),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
            Pitch::new(
                Some(12),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
            Pitch::new(
                Some(13),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
        ];

        let x = simplify_multiple_enharmonics(more_than_five, None, None);
        let less_than_five = vec![
            Pitch::new(
                Some(0),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            ),
            Pitch::new(
                Some(1),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            ),
            Pitch::new(
                Some(2),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            ),
            Pitch::new(
                Some(12),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            ),
            Pitch::new(
                Some(13),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            ),
        ];
    }
}
