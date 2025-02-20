pub(crate) mod accidental;
pub(crate) mod microtone;
pub(crate) mod pitchclass;
pub(crate) mod pitchclassstring;

use crate::defaults::FloatType;
use crate::defaults::IntegerType;
use crate::defaults::Octave;
use crate::defaults::PITCH_STEP;
use crate::exception::Exception;
use crate::exception::ExceptionResult;
use crate::interval::Interval;
use crate::interval::IntervalArgument;
use crate::interval::PitchOrNote;
use crate::interval::interval_to_pythagorean_ratio;
use crate::interval::intervalstring::IntervalString;
use crate::key::keysignature::KeySignature;
use crate::note::Note;
use crate::prebase::ProtoM21Object;
use crate::prebase::ProtoM21ObjectTrait;
use crate::stepname::StepName;

use accidental::Accidental;
use accidental::IntoAccidental;
use microtone::IntoCentShift;
use microtone::Microtone;
use pitchclassstring::PitchClassString;

use fraction::GenericFraction;
use itertools::Itertools;
use num::Num;
use ordered_float::OrderedFloat;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;

// TODO: rework this, don't use a HashMap for two possible inputs, either figure
// out what the -d2 and d2 intervals are beforehand or caculate them and store
// them each in a static
static TRANSPOSITIONAL_INTERVALS: LazyLock<Mutex<HashMap<IntervalString, Interval>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct Pitch {
    proto: ProtoM21Object,
    _step: StepName,
    _octave: Octave,
    _overriden_freq440: Option<FloatType>,
    _accidental: Accidental,
    _microtone: Option<Microtone>,
    #[cfg_attr(feature = "serde", serde(skip))]
    _client: Option<Arc<Note>>,
    spelling_is_infered: bool,
    #[cfg_attr(feature = "serde", serde(skip))]
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
        // --- Step 1: Parse parameters ---
        let mut self_name = None;
        let mut self_step = PITCH_STEP;
        let mut self_accidental: Option<Accidental> = None;
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
                self_step = step;
            }
            if let Some(accidental) = x.accidental {
                self_accidental = Some(accidental);
            }
            if let Some(inferred) = x.spelling_is_inferred {
                self_spelling_is_inferred = inferred;
            }
            self_octave = x.octave;
        } else if let Some(s) = step {
            self_step = s;
        }

        if let Some(oct) = octave {
            self_octave = Some(oct);
        }

        if let Some(acc) = accidental {
            self_accidental = Some(if acc.is_accidental() {
                acc.accidental()
            } else {
                Accidental::new(acc)?
            });
        } else if self_accidental.is_none() {
            self_accidental = Some(Accidental::new("natural")?);
        }

        if let Some(mt) = microtone {
            self_microtone = Some(if mt.is_microtone() {
                mt.microtone()
            } else {
                mt.into_microtone()?
            });
        }

        // --- Step 2: Construct Pitch with initial values ---
        let mut pitch = Pitch {
            proto: ProtoM21Object::new(),
            _step: self_step,
            _overriden_freq440: None,
            _accidental: self_accidental.clone().unwrap(),
            _microtone: self_microtone,
            _octave: self_octave,
            _client: None,
            spelling_is_infered: self_spelling_is_inferred,
            fundamental: None,
        };

        // --- Step 3: Call setters in proper order ---
        if let Some(ref n) = self_name {
            pitch.name_setter(n)?;
        }

        pitch.step_setter(self_step);

        pitch.octave_setter(self_octave);

        pitch.accidental_setter(pitch._accidental.clone());
        if let Some(ref mt) = pitch._microtone {
            pitch.microtone_setter(mt.clone());
        }
        if let Some(pc) = self_pitch_class {
            pitch.pitch_class_setter(pc);
        }
        if let Some(f) = self_fundamental {
            pitch.fundamental_setter(f);
        }
        if let Some(m) = self_midi {
            pitch.midi_setter(m);
        }
        if let Some(p) = self_ps {
            pitch.ps_setter(p);
        }

        Ok(pitch)
    }

    pub(crate) fn name_with_octave(&self) -> String {
        todo!()
    }

    pub(crate) fn name(&self) -> String {
        format!("{:?}{}", self._step, self._accidental.modifier())
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
        self.step_setter(StepName::try_from(step)?);

        let accidental_str: String = pitch_chars.collect();
        if accidental_str.is_empty() {
            self.accidental_setter(Accidental::natural());
        } else {
            self.accidental_setter(Accidental::new(accidental_str)?);
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

    fn get_all_common_enharmonics(
        &mut self,
        alter_limit: FloatType,
    ) -> ExceptionResult<Vec<Pitch>> {
        let mut post: Vec<Pitch> = vec![];

        // Initial simplified enharmonic
        let c = self.simplify_enharmonic(false)?;
        if c.name() != self.name() {
            post.push(c.clone());
        }

        // Iterative scan upward
        let mut c_up = self.clone();
        loop {
            let next = match c_up.get_higher_enharmonic() {
                Ok(p) => p,
                Err(e) => {
                    if {
                        let this = &e;
                        matches!(this, Exception::Accidental(_))
                    } {
                        break;
                    } else {
                        return Err(e);
                    }
                }
            };

            c_up = next;

            if c_up._accidental._alter.abs() > alter_limit {
                break;
            }

            if !post.contains(&c_up) {
                post.push(c_up.clone());
            } else {
                break;
            }
        }

        // Iterative scan downward
        let mut c_down = self.clone();
        loop {
            let next = match c_down.get_higher_enharmonic() {
                Ok(p) => p,
                Err(e) => {
                    if {
                        let this = &e;
                        matches!(this, Exception::Accidental(_))
                    } {
                        break;
                    } else {
                        return Err(e);
                    }
                }
            };

            c_down = next;

            if c_down._accidental._alter.abs() > alter_limit {
                break;
            }

            if !post.contains(&c_down) {
                post.push(c_down.clone());
            } else {
                break;
            }
        }

        Ok(post)
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

    fn step_setter(&mut self, step_name: StepName) {
        self._step = step_name;
        self.spelling_is_infered = true;
        self.inform_client();
    }

    fn accidental_setter(&mut self, value: Accidental) {
        self._accidental = value;
        self.inform_client();
    }

    fn microtone_setter(&self, mt: Microtone) {
        todo!()
    }

    fn pitch_class_setter(&self, pc: PitchClassString) {
        todo!()
    }

    fn fundamental_setter(&self, f: Pitch) {
        todo!()
    }

    fn midi_setter(&self, m: IntegerType) {
        todo!()
    }

    fn ps_setter(&self, p: FloatType) {
        todo!()
    }

    fn simplify_enharmonic(&mut self, most_common: bool) -> ExceptionResult<Pitch> {
        const EXCLUDED_NAMES: [&str; 4] = ["E#", "B#", "C-", "F-"];
        if self._accidental._alter.abs().partial_cmp(&2.0) != Some(Ordering::Less)
            || EXCLUDED_NAMES.contains(&self.name().as_str())
        {
            // by resetting the pitch space value, we get a simpler enharmonic spelling
            let save_octave = self._octave;
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

        Ok(self.clone())
    }

    fn simplify_enharmonic_in_place(&mut self, most_common: bool) -> ExceptionResult<()> {
        todo!()
    }

    fn get_higher_enharmonic(&self) -> ExceptionResult<Pitch> {
        self._get_enharmonic_helper(true)
    }

    fn get_higher_enharmonic_in_place(&mut self) -> ExceptionResult<()> {
        self._get_enharmonic_helper_in_place(true)
    }

    fn _get_enharmonic_helper(&self, up: bool) -> ExceptionResult<Pitch> {
        let interval_string = match up {
            true => IntervalString::Up,
            false => IntervalString::Down,
        };

        let mut dict = match TRANSPOSITIONAL_INTERVALS.lock() {
            Ok(dict) => dict,
            Err(err) => err.into_inner(),
        };

        let interval: Interval = match dict.get(&interval_string) {
            None => {
                let interval =
                    Interval::new(IntervalArgument::Str(interval_string.clone().string()))?;
                dict.insert(interval_string.clone(), interval.clone());
                interval
            }
            Some(interval) => interval.to_owned(),
        };

        let octave_stored = self._octave;

        let p = interval.transpose_pitch(self, false, None);

        todo!()
    }

    fn _get_enharmonic_helper_in_place(&mut self, up: bool) -> ExceptionResult<()> {
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
    pitches: &[Pitch],
    criterion: Option<CriterionFunction>,
    key_context: Option<KeySignature>,
) -> ExceptionResult<Vec<Pitch>> {
    let mut old_pitches: Vec<Pitch> = pitches.to_vec();

    let criterion: CriterionFunction =
        criterion.unwrap_or(|x: &[Pitch]| dissonance_score(x, true, true, true));

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
        new_p.spelling_is_infered = old_p.spelling_is_infered;
    }

    if remove_first {
        simplified_pitches.remove(0);
    }

    Ok(simplified_pitches)
}

fn brute_force_enharmonics_search(
    old_pitches: &mut [Pitch],
    score_func: CriterionFunction,
) -> ExceptionResult<Vec<Pitch>> {
    let all_possible_pitches: ExceptionResult<Vec<Vec<Pitch>>> = old_pitches[1..]
        .iter_mut()
        .map(|p| -> ExceptionResult<Vec<Pitch>> {
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
        if score < min_score {
            min_score = score;
            best_combination = pitches;
        }
    }

    Ok(best_combination)
}

fn greedy_enharmonics_search(
    old_pitches: &mut [Pitch],
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

    for old_pitch in old_pitches.iter_mut().skip(1) {
        let mut candidates = vec![old_pitch.clone()];
        candidates.extend(
            old_pitch
                .get_all_common_enharmonics(2 as FloatType)?
                .into_iter(),
        );

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
                let mut p1 = (*p1).clone();
                let mut p2 = (*p2).clone();
                p1.octave_setter(None);
                p2.octave_setter(None);
                intervals.push(Interval::between(
                    PitchOrNote::Pitch(p1.clone()),
                    PitchOrNote::Pitch(p2.clone()),
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
                    * 0.037_926_633
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

    use super::{Pitch, simplify_multiple_enharmonics};

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

        let x = simplify_multiple_enharmonics(&more_than_five, None, None);
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
