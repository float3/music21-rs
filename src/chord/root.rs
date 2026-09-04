use crate::defaults::{FloatType, IntegerType};
use crate::pitch::Pitch;
use std::collections::{BTreeMap, BTreeSet};

/// Finds the root of a set of pitches the way music21's `Chord.root` does:
/// the pitch from which every other letter is stacked in thirds, or failing
/// that the pitch with the best-weighted set of chord steps above it.
pub(crate) fn find_root_pitch<'a>(
    pitches: impl IntoIterator<Item = &'a Pitch>,
) -> Option<&'a Pitch> {
    let pitches = pitches.into_iter().collect::<Vec<_>>();
    let mut seen_steps = BTreeSet::new();
    let unique_steps = pitches
        .iter()
        .copied()
        .filter(|pitch| seen_steps.insert(step_num(pitch)))
        .collect::<Vec<_>>();

    match unique_steps.len() {
        0 => return None,
        1 => return pitches.first().copied(),
        7 => return bass_pitch(pitches),
        _ => {}
    }

    let step_nums_to_pitches = unique_steps
        .iter()
        .map(|pitch| (step_num(pitch), *pitch))
        .collect::<BTreeMap<_, _>>();
    let step_nums = step_nums_to_pitches.keys().copied().collect::<Vec<_>>();

    for start_index in 0..step_nums.len() {
        let mut last_step_num = step_nums[start_index];
        let all_are_thirds = (start_index + 1..start_index + step_nums.len()).all(|end_index| {
            let end_step_num = step_nums[end_index % step_nums.len()];
            let is_third = matches!(end_step_num - last_step_num, 2 | -5);
            last_step_num = end_step_num;
            is_third
        });
        if all_are_thirds {
            return step_nums_to_pitches.get(&step_nums[start_index]).copied();
        }
    }

    let ordered_chord_steps = [3, 5, 7, 2, 4, 6];
    let score = |pitch: &Pitch| {
        let this_step_num = step_num(pitch);
        ordered_chord_steps
            .iter()
            .enumerate()
            .filter(|(_, chord_step)| {
                step_nums_to_pitches.contains_key(&(this_step_num + *chord_step - 1).rem_euclid(7))
            })
            .map(|(root_index, _)| 1.0 / (root_index as FloatType + 6.0))
            .sum::<FloatType>()
    };

    let mut best = unique_steps[0];
    let mut best_score = FloatType::NEG_INFINITY;
    for pitch in unique_steps {
        let pitch_score = score(pitch);
        if pitch_score > best_score {
            best_score = pitch_score;
            best = pitch;
        }
    }
    Some(best)
}

pub(crate) fn bass_pitch<'a>(pitches: impl IntoIterator<Item = &'a Pitch>) -> Option<&'a Pitch> {
    pitches.into_iter().min_by(|left, right| {
        left.ps()
            .partial_cmp(&right.ps())
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

pub(crate) fn step_num(pitch: &Pitch) -> IntegerType {
    pitch.step().step_to_dnn_offset() - 1
}

pub(crate) fn pitch_class(pitch: &Pitch) -> u8 {
    (pitch.ps().round() as IntegerType).rem_euclid(12) as u8
}
