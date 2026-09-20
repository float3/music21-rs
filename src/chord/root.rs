use crate::defaults::{FloatType, IntegerType};
use crate::pitch::Pitch;

/// How many letters there are, which is how many steps a chord can stand on.
const STEPS: usize = 7;

/// Finds the root of a set of pitches the way music21's `Chord.root` does:
/// the pitch from which every other letter is stacked in thirds, or failing
/// that the pitch with the best-weighted set of chord steps above it.
///
/// There are seven letters, so everything this needs to know fits in a pair
/// of arrays of seven filled in one pass: which pitch stands on each letter,
/// and the letters in the order the chord gave them. A chord comes through
/// here for its root, its inversion, every chord step it is asked for and
/// every predicate written on one of those, so the sets and maps this used to
/// build were allocated on nearly every question a chord answers.
pub(crate) fn find_root_pitch<'a>(
    pitches: impl IntoIterator<Item = &'a Pitch>,
) -> Option<&'a Pitch> {
    // The pitch standing on each letter, the first one where a letter is
    // doubled, which is the pitch music21's own set keeps.
    let mut by_step: [Option<&'a Pitch>; STEPS] = [None; STEPS];
    // The same pitches in the order the chord gave them, which is the order
    // ties are broken in below.
    let mut unique: [Option<&'a Pitch>; STEPS] = [None; STEPS];
    let mut unique_count = 0;
    let mut first = None;
    let mut bass: Option<&'a Pitch> = None;
    for pitch in pitches {
        if first.is_none() {
            first = Some(pitch);
        }
        // The written-lowest pitch, worked out as it goes: only a pitch that
        // is strictly lower wins, so a tie keeps the first, as `bass_pitch`
        // does.
        if bass.is_none_or(|lowest| is_written_below(pitch, lowest)) {
            bass = Some(pitch);
        }
        let step = step_index(pitch);
        if by_step[step].is_none() {
            by_step[step] = Some(pitch);
            unique[unique_count] = Some(pitch);
            unique_count += 1;
        }
    }

    match unique_count {
        0 => return None,
        1 => return first,
        STEPS => return bass,
        _ => {}
    }

    // The letters the chord stands on, lowest first, which is the order
    // music21 walks them in.
    let mut steps = [0; STEPS];
    let mut count = 0;
    for (step, pitch) in by_step.iter().enumerate() {
        if pitch.is_some() {
            steps[count] = step as IntegerType;
            count += 1;
        }
    }
    let steps = &steps[..count];

    for start_index in 0..count {
        let mut last_step = steps[start_index];
        let all_are_thirds = (start_index + 1..start_index + count).all(|end_index| {
            let end_step = steps[end_index % count];
            let is_third = matches!(end_step - last_step, 2 | -5);
            last_step = end_step;
            is_third
        });
        if all_are_thirds {
            return by_step[steps[start_index] as usize];
        }
    }

    let ordered_chord_steps = [3, 5, 7, 2, 4, 6];
    let score = |pitch: &Pitch| {
        let this_step = step_num(pitch);
        ordered_chord_steps
            .iter()
            .enumerate()
            .filter(|(_, chord_step)| {
                by_step[(this_step + *chord_step - 1).rem_euclid(7) as usize].is_some()
            })
            .map(|(root_index, _)| 1.0 / (root_index as FloatType + 6.0))
            .sum::<FloatType>()
    };

    let mut best = unique[0];
    let mut best_score = FloatType::NEG_INFINITY;
    for pitch in unique[..unique_count].iter().flatten() {
        let pitch_score = score(pitch);
        if pitch_score > best_score {
            best_score = pitch_score;
            best = Some(pitch);
        }
    }
    best
}

/// Whether one pitch is written below another: the lower staff position, and
/// the lower pitch space where they share one.
fn is_written_below(pitch: &Pitch, other: &Pitch) -> bool {
    match diatonic_note_number(pitch).cmp(&diatonic_note_number(other)) {
        std::cmp::Ordering::Less => true,
        std::cmp::Ordering::Greater => false,
        std::cmp::Ordering::Equal => pitch.ps() < other.ps(),
    }
}

/// The letter a pitch stands on, as an index into the arrays above.
fn step_index(pitch: &Pitch) -> usize {
    step_num(pitch).rem_euclid(STEPS as IntegerType) as usize
}

/// Finds the written-lowest pitch, as music21's `Chord.bass` does: the lowest
/// staff position, then the lowest pitch space, keeping the first on a tie.
pub(crate) fn bass_pitch<'a>(pitches: impl IntoIterator<Item = &'a Pitch>) -> Option<&'a Pitch> {
    pitches.into_iter().min_by(|left, right| {
        diatonic_note_number(left)
            .cmp(&diatonic_note_number(right))
            .then_with(|| {
                left.ps()
                    .partial_cmp(&right.ps())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    })
}

pub(crate) fn step_num(pitch: &Pitch) -> IntegerType {
    pitch.step().step_to_dnn_offset() - 1
}

pub(crate) fn pitch_class(pitch: &Pitch) -> u8 {
    (pitch.ps().round_ties_even() as IntegerType).rem_euclid(12) as u8
}

/// music21's `diatonicNoteNum`: the staff position counting C0 as 1, with
/// the implicit octave standing in when none is set.
pub(crate) fn diatonic_note_number(pitch: &Pitch) -> IntegerType {
    let octave = pitch
        .octave()
        .unwrap_or(crate::defaults::PITCH_OCTAVE as IntegerType);
    pitch.step().step_to_dnn_offset() + 7 * octave
}
