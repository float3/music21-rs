pub(crate) mod accidental;
pub(crate) mod microtone;

use crate::{
    defaults::{self, IntegerType},
    interval::{interval_to_pythagorean_ratio, Interval, PitchOrNote},
    note::Note,
    prebase::{ProtoM21Object, ProtoM21ObjectTrait},
    stepname::StepName,
};
use std::sync::Arc;

use accidental::{Accidental, IntoAccidental};
use itertools::Itertools;
use microtone::{IntoMicrotone, Microtone};
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

impl Pitch {
    pub(crate) fn new<T, U, V>(
        name: Option<T>,
        step: Option<StepName>,
        octave: Octave,
        accidental: Option<U>,
        microtone: Option<V>,
    ) -> Self
    where
        T: IntoPitchName,
        U: IntoAccidental,
        V: IntoMicrotone,
    {
        let mut self_name = None;
        let mut self_step = PITCH_STEP;
        let mut self_accidental;
        let mut self_microtone: Option<Microtone> = None;
        let mut self_spelling_is_inferred = false;
        let mut self_octave = None;

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
            self_accidental = accidental.into_accidental();
        } else {
            self_accidental = Accidental::new("natural");
        }

        if let Some(microtone) = microtone {
            self_microtone = Some(microtone.into_microtone());
        }

        //TODO(more stuff here)

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

        pitch.set_name(self_name);
        pitch
    }

    pub(crate) fn name_with_octave(&self) -> String {
        todo!()
    }

    pub(crate) fn name(&self) -> String {
        todo!()
    }

    pub(crate) fn alter(&self) -> FloatType {
        let mut post = 0.0;

        post += self._accidental._alter;

        if let Some(microtone) = &self._microtone {
            post += microtone._alter;
        }

        post
    }

    pub(crate) fn set_octave(&mut self, octave: Octave) {
        self._octave = octave;
        self.informclient()
    }

    fn get_all_common_enharmonics(&self, alter_limit: IntegerType) -> Vec<Pitch> {
        todo!()
    }

    fn informclient(&self) {
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

pub(crate) fn simplify_multiple_enharmonics(pitches: Vec<Pitch>) -> Option<Vec<Pitch>> {
    if pitches.len() < 5 {
        brute_force_enharmonics_search(pitches, |x| dissonance_score(x, true, true, true))
    } else {
        greedy_enharmonics_search(pitches, |x| dissonance_score(x, true, true, true))
    }
}

fn brute_force_enharmonics_search(
    old_pitches: Vec<Pitch>,
    score_func: fn(&[Pitch]) -> f64,
) -> Option<Vec<Pitch>> {
    let all_possible_pitches: Vec<Vec<Pitch>> = old_pitches[1..]
        .iter()
        .map(|p| {
            let mut enharmonics = p.get_all_common_enharmonics(2);
            enharmonics.insert(0, (*p).clone());
            enharmonics
        })
        .collect();

    let all_pitch_combinations = all_possible_pitches.into_iter().multi_cartesian_product();

    let mut min_score = f64::MAX;
    let mut best_combination: Vec<Pitch> = Vec::new();

    for combination in all_pitch_combinations {
        let mut pitches: Vec<Pitch> = old_pitches[..1].to_vec();
        pitches.extend(combination);
        let score = score_func(&pitches);
        if score < min_score {
            min_score = score;
            best_combination = pitches;
        }
    }

    Some(best_combination)
}

fn greedy_enharmonics_search(
    old_pitches: Vec<Pitch>,
    score_func: fn(&[Pitch]) -> f64,
) -> Option<Vec<Pitch>> {
    let mut new_pitches = vec![old_pitches.first()?.clone()];

    for old_pitch in old_pitches.iter().skip(1) {
        let mut candidates = vec![old_pitch.clone()];
        candidates.extend(old_pitch.get_all_common_enharmonics(2).into_iter());

        let best_candidate = candidates.iter().min_by_key(|candidate| {
            let mut candidate_list = new_pitches.clone();
            candidate_list.push((*candidate).clone());
            OrderedFloat(score_func(&candidate_list))
        })?;

        new_pitches.push(best_candidate.clone());
    }
    Some(new_pitches)
}

fn dissonance_score(
    pitches: &[Pitch],
    small_pythagorean_ratio: bool,
    accidental_penalty: bool,
    triad_award: bool,
) -> f64 {
    let mut score_accidentals = 0.0;
    let mut score_ratio = 0.0;
    let mut score_triad = 0.0;

    if pitches.is_empty() {
        return 0.0;
    }

    if accidental_penalty {
        let accidentals = pitches
            .iter()
            .map(|p| p.alter().abs())
            .collect::<Vec<f64>>();
        score_accidentals = accidentals
            .iter()
            .map(|a| if *a > 1.0 { *a } else { 0.0 })
            .sum::<f64>()
            / pitches.len() as f64;
    }

    let mut intervals: Vec<Interval> = vec![];

    if small_pythagorean_ratio | triad_award {
        for (index, p1) in pitches.iter().enumerate() {
            for p2 in pitches.iter().skip(index + 1) {
                let mut p1 = (*p1).clone();
                let mut p2 = (*p2).clone();
                p1.set_octave(None);
                p2.set_octave(None);
                match Interval::between(
                    Some(PitchOrNote::Pitch(p1.clone())),
                    Some(PitchOrNote::Pitch(p2.clone())),
                ) {
                    Some(interval) => intervals.push(interval),
                    None => return f64::INFINITY,
                }
            }
        }

        if small_pythagorean_ratio {
            for interval in intervals.iter() {
                match interval_to_pythagorean_ratio(interval.clone()) {
                    Result::Ok(ratio) => {
                        score_ratio += (match ratio {
                            fraction::GenericFraction::Rational(sign, ratio) => *ratio.denom(),
                            fraction::GenericFraction::Infinity(sign) => panic!(),
                            fraction::GenericFraction::NaN => panic!(),
                        } as f64)
                            .ln()
                            * 0.03792663444
                    }
                    Result::Err(_) => return f64::INFINITY, //TODO: investigate this
                };
            }
            score_ratio /= pitches.len() as f64;
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
            score_triad /= pitches.len() as f64;
        }
    }

    (score_accidentals + score_ratio + score_triad)
        / (small_pythagorean_ratio as IntegerType
            + accidental_penalty as IntegerType
            + triad_award as IntegerType) as f64
}
