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

use accidental::Accidental;
use itertools::Itertools;
use microtone::Microtone;
use num::Num;
use ordered_float::OrderedFloat;

use self::defaults::FloatType;

#[derive(Clone, Debug)]
pub struct Pitch {
    proto: ProtoM21Object,
    _step: StepName,
    _octave: Option<IntegerType>,
    _overriden_freq440: Option<FloatType>,
    _accidental: Option<Accidental>,
    _microtone: Option<Microtone>,
    _client: Option<Arc<Note>>,
}

impl Pitch {
    pub fn new<T>(pitch: Option<T>) -> Self
    where
        T: IntoPitchName,
    {
        Pitch {
            proto: ProtoM21Object::new(),
            _step: defaults::PITCH_STEP,
            _overriden_freq440: None,
            _accidental: todo!(),
            _microtone: todo!(),
            _octave: todo!(),
            _client: todo!(),
        }
    }

    pub(crate) fn name_with_octave(&self) -> String {
        todo!()
    }

    pub(crate) fn name(&self) -> String {
        todo!()
    }

    pub(crate) fn alter(&self) -> FloatType {
        let mut post = 0.0;

        if let Some(accidental) = &self._accidental {
            post += accidental._alter;
        }

        if let Some(microtone) = &self._microtone {
            post += microtone._alter;
        }

        post
    }

    pub(crate) fn set_octave(&mut self, octave: Option<IntegerType>) {
        self._octave = octave;
        self.informclient()
    }

    fn get_all_common_enharmonics(&self, alter_limit: IntegerType) -> Vec<Pitch> {
        todo!()
    }

    fn informclient(&self) {
        if let Some(ref client) = self._client {
            client.pitchChanged();
        }
    }

    pub(crate) fn transpose(&self, clone: Interval) -> Pitch {
        todo!()
    }

    pub(crate) fn ps(&self) -> FloatType {
        todo!()
    }
}

impl ProtoM21ObjectTrait for Pitch {}

pub trait IntoPitchName {
    fn into_name(&self) -> String;
}

impl IntoPitchName for Pitch {
    fn into_name(&self) -> String {
        self.name_with_octave()
    }
}

impl IntoPitchName for IntegerType {
    fn into_name(&self) -> String {
        todo!()
    }
}

impl IntoPitchName for String {
    fn into_name(&self) -> String {
        todo!()
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

    best_combination
}

fn greedy_enharmonics_search(
    old_pitches: Vec<Pitch>,
    score_func: fn(&[Pitch]) -> f64,
) -> Option<Vec<Pitch>> {
    let mut new_pitches = vec![old_pitches.get(0)?.clone()];

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
                    None => return std::f64::INFINITY,
                }
            }
        }

        if small_pythagorean_ratio {
            for interval in intervals.iter() {
                match interval_to_pythagorean_ratio(interval.clone()) {
                    Some(ratio) => {
                        score_ratio += (*(ratio.denom().unwrap()) as f64).ln() * 0.03792663444
                    }
                    None => return std::f64::INFINITY,
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
