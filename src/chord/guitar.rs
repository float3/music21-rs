use super::Chord;
use crate::{
    defaults::{FloatType, IntegerType},
    pitch::Pitch,
};
use std::collections::BTreeSet;

const STANDARD_TUNING: [&str; 6] = ["E2", "A2", "D3", "G3", "B3", "E4"];
const MAX_FRET: u8 = 12;
const MAX_FRET_SPAN: u8 = 4;

/// Open-string pitch data for a guitar tuning.
///
/// Tunings are ordered from low string to high string and use concrete pitches,
/// not just pitch classes, so fingering generation can respect octaves.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GuitarTuningString {
    /// Open-string pitch name, including octave.
    pub name: String,
    /// Open-string pitch space, where C4 is 60.
    pub pitch_space: IntegerType,
    /// Open-string pitch class.
    pub pitch_class: u8,
}

/// Guitar tuning used for fingering generation.
///
/// Strings are ordered from low to high, for example standard six-string guitar
/// tuning is `["E2", "A2", "D3", "G3", "B3", "E4"]`.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GuitarTuning {
    strings: Vec<GuitarTuningString>,
}

impl GuitarTuning {
    /// Builds a tuning from low-to-high open-string pitch names.
    pub fn new<I, S>(strings: I) -> crate::Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let strings = strings
            .into_iter()
            .map(|name| {
                let pitch = Pitch::from_name(name.as_ref())?;
                let pitch_space = pitch_space(&pitch)?;
                Ok(GuitarTuningString {
                    name: pitch.name_with_octave(),
                    pitch_space,
                    pitch_class: pitch_class(&pitch),
                })
            })
            .collect::<crate::Result<Vec<_>>>()?;

        if strings.is_empty() {
            return Err(crate::Error::Chord(
                "guitar tuning must contain at least one string".to_string(),
            ));
        }
        if strings.len() > u8::MAX as usize {
            return Err(crate::Error::Chord(
                "guitar tuning cannot contain more than 255 strings".to_string(),
            ));
        }

        Ok(Self { strings })
    }

    /// Returns standard six-string guitar tuning.
    pub fn standard() -> Self {
        Self::new(STANDARD_TUNING).expect("standard guitar tuning should be valid")
    }

    /// Returns the tuning strings from low to high.
    pub fn strings(&self) -> &[GuitarTuningString] {
        &self.strings
    }

    fn len(&self) -> usize {
        self.strings.len()
    }
}

impl Default for GuitarTuning {
    fn default() -> Self {
        Self::standard()
    }
}

/// One string in a suggested guitar fingering.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GuitarStringFingering {
    /// Guitar string number, where the lowest string has the highest number.
    pub string_number: u8,
    /// Open-string pitch name, including octave.
    pub string_name: String,
    /// Open-string pitch space, where C4 is 60.
    pub open_pitch_space: IntegerType,
    /// Open-string pitch class.
    pub open_pitch_class: u8,
    /// Fret to play, or `None` for a muted string.
    pub fret: Option<u8>,
    /// Suggested fretting finger, where `1` is index and `4` is pinky.
    ///
    /// Open and muted strings do not use a finger.
    pub finger: Option<u8>,
    /// Sounding pitch space, or `None` for a muted string.
    pub pitch_space: Option<IntegerType>,
    /// Sounding pitch class, or `None` for a muted string.
    pub pitch_class: Option<u8>,
}

/// A suggested guitar fingering for a chord.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GuitarFingering {
    /// String fingerings from low string to high string.
    pub strings: Vec<GuitarStringFingering>,
    /// Lowest fretted position in the shape. `0` means the shape uses open strings.
    pub base_fret: u8,
    /// Distance between the lowest and highest fretted notes.
    pub fret_span: u8,
    /// Pitch spaces sounded by the fingering.
    pub covered_pitch_spaces: Vec<IntegerType>,
    /// Chord pitch spaces not present in the fingering.
    pub omitted_pitch_spaces: Vec<IntegerType>,
    /// Pitch classes sounded by the fingering.
    pub covered_pitch_classes: Vec<u8>,
    /// Chord pitch classes not present in the fingering.
    pub omitted_pitch_classes: Vec<u8>,
}

#[derive(Clone, Debug)]
struct StringChoice {
    fret: Option<u8>,
    pitch_space: Option<IntegerType>,
    pitch_class: Option<u8>,
}

#[derive(Clone, Debug)]
struct Candidate {
    fingering: GuitarFingering,
    score: usize,
}

#[derive(Clone, Debug)]
struct Barre {
    fret: u8,
    start: usize,
    end: usize,
    covers: Vec<usize>,
}

#[derive(Clone, Debug)]
struct FingerGroup {
    fret: u8,
    start: usize,
    covers: Vec<usize>,
}

pub(crate) fn suggested_guitar_fingering(chord: &Chord) -> Option<GuitarFingering> {
    suggested_guitar_fingering_with_tuning(chord, &GuitarTuning::standard())
}

pub(crate) fn suggested_guitar_fingering_with_tuning(
    chord: &Chord,
    tuning: &GuitarTuning,
) -> Option<GuitarFingering> {
    if chord.pitches().iter().any(|pitch| {
        let ps = pitch.ps();
        (ps - ps.round()).abs() > FloatType::EPSILON
    }) {
        return None;
    }

    let target_pitch_spaces = chord
        .pitches()
        .iter()
        .map(pitch_space)
        .collect::<crate::Result<BTreeSet<_>>>()
        .ok()?;
    if target_pitch_spaces.is_empty() {
        return None;
    }
    let target_pitch_classes = chord.pitch_classes().into_iter().collect::<BTreeSet<_>>();

    let root_pitch_class = chord
        .root_pitch_name()
        .and_then(|name| Pitch::from_name(name).ok())
        .map(|pitch| pitch_class(&pitch));

    let mut candidates = Vec::new();
    for base_fret in 0..=(MAX_FRET - MAX_FRET_SPAN) {
        let options = tuning
            .strings()
            .iter()
            .map(|string| string_choices(string.pitch_space, &target_pitch_spaces, base_fret))
            .collect::<Vec<_>>();
        collect_candidates(
            &options,
            0,
            Vec::with_capacity(tuning.len()),
            &target_pitch_spaces,
            &target_pitch_classes,
            root_pitch_class,
            tuning,
            &mut candidates,
        );
    }

    candidates.sort_by(|left, right| {
        left.score
            .cmp(&right.score)
            .then_with(|| {
                left.fingering
                    .omitted_pitch_classes
                    .len()
                    .cmp(&right.fingering.omitted_pitch_classes.len())
            })
            .then_with(|| left.fingering.base_fret.cmp(&right.fingering.base_fret))
            .then_with(|| left.fingering.fret_span.cmp(&right.fingering.fret_span))
    });
    candidates
        .into_iter()
        .next()
        .map(|candidate| candidate.fingering)
}

fn string_choices(
    open_pitch_space: IntegerType,
    target: &BTreeSet<IntegerType>,
    base_fret: u8,
) -> Vec<StringChoice> {
    let mut choices = vec![StringChoice {
        fret: None,
        pitch_space: None,
        pitch_class: None,
    }];
    let first_fret = if base_fret == 0 { 0 } else { base_fret };
    let last_fret = (base_fret + MAX_FRET_SPAN).min(MAX_FRET);

    for fret in first_fret..=last_fret {
        let pitch_space = open_pitch_space + IntegerType::from(fret);
        if target.contains(&pitch_space) {
            choices.push(StringChoice {
                fret: Some(fret),
                pitch_space: Some(pitch_space),
                pitch_class: Some(pitch_space.rem_euclid(12) as u8),
            });
        }
    }

    choices
}

fn collect_candidates(
    options: &[Vec<StringChoice>],
    string_index: usize,
    current: Vec<StringChoice>,
    target_pitch_spaces: &BTreeSet<IntegerType>,
    target_pitch_classes: &BTreeSet<u8>,
    root_pitch_class: Option<u8>,
    tuning: &GuitarTuning,
    candidates: &mut Vec<Candidate>,
) {
    if string_index == options.len() {
        if let Some(candidate) = score_candidate(
            current,
            target_pitch_spaces,
            target_pitch_classes,
            root_pitch_class,
            tuning,
        ) {
            candidates.push(candidate);
        }
        return;
    }

    for choice in &options[string_index] {
        let mut next = current.clone();
        next.push(choice.clone());
        collect_candidates(
            options,
            string_index + 1,
            next,
            target_pitch_spaces,
            target_pitch_classes,
            root_pitch_class,
            tuning,
            candidates,
        );
    }
}

fn score_candidate(
    choices: Vec<StringChoice>,
    target_pitch_spaces: &BTreeSet<IntegerType>,
    target_pitch_classes: &BTreeSet<u8>,
    root_pitch_class: Option<u8>,
    tuning: &GuitarTuning,
) -> Option<Candidate> {
    let sounding_indices = choices
        .iter()
        .enumerate()
        .filter_map(|(index, choice)| choice.fret.map(|_| index))
        .collect::<Vec<_>>();
    if sounding_indices.is_empty() {
        return None;
    }

    let covered_pitch_spaces = choices
        .iter()
        .filter_map(|choice| choice.pitch_space)
        .collect::<BTreeSet<_>>();
    let omitted_pitch_spaces = target_pitch_spaces
        .difference(&covered_pitch_spaces)
        .copied()
        .collect::<Vec<_>>();
    let covered_pitch_classes = choices
        .iter()
        .filter_map(|choice| choice.pitch_class)
        .collect::<BTreeSet<_>>();
    let omitted_pitch_classes = target_pitch_classes
        .difference(&covered_pitch_classes)
        .copied()
        .collect::<Vec<_>>();
    let fretted = choices
        .iter()
        .filter_map(|choice| choice.fret.filter(|fret| *fret > 0))
        .collect::<Vec<_>>();
    let finger_assignment = finger_assignment(&choices)?;
    let base_fret = fretted.iter().copied().min().unwrap_or(0);
    let fret_span = fretted
        .iter()
        .copied()
        .max()
        .zip(fretted.iter().copied().min())
        .map(|(max, min)| max - min)
        .unwrap_or(0);
    let muted_count = choices
        .iter()
        .filter(|choice| choice.fret.is_none())
        .count();
    let internal_mutes = internal_muted_string_count(&choices, &sounding_indices);
    let bass_pitch_class = sounding_indices
        .first()
        .and_then(|index| choices[*index].pitch_class);
    let root_is_missing =
        root_pitch_class.is_some_and(|root| !covered_pitch_classes.contains(&root));
    let bass_is_not_root =
        root_pitch_class.is_some_and(|root| bass_pitch_class.is_some_and(|bass| bass != root));
    let duplicate_count = sounding_indices
        .len()
        .saturating_sub(covered_pitch_spaces.len());
    let fret_sum = fretted.iter().map(|fret| *fret as usize).sum::<usize>();

    let mut score = omitted_pitch_spaces.len() * 1000
        + usize::from(root_is_missing) * 250
        + usize::from(bass_is_not_root) * 45
        + internal_mutes * 40
        + muted_count * 6
        + fret_span as usize * 8
        + base_fret as usize * 3
        + fretted.len() * 2
        + duplicate_count
        + fret_sum;

    if covered_pitch_spaces.len() == target_pitch_spaces.len() {
        score = score.saturating_sub(50);
    }

    let strings = choices
        .into_iter()
        .enumerate()
        .map(|(index, choice)| {
            let tuning_string = &tuning.strings()[index];
            GuitarStringFingering {
                string_number: (tuning.len() - index) as u8,
                string_name: tuning_string.name.clone(),
                open_pitch_space: tuning_string.pitch_space,
                open_pitch_class: tuning_string.pitch_class,
                fret: choice.fret,
                finger: finger_assignment[index],
                pitch_space: choice.pitch_space,
                pitch_class: choice.pitch_class,
            }
        })
        .collect::<Vec<_>>();

    Some(Candidate {
        fingering: GuitarFingering {
            strings,
            base_fret,
            fret_span,
            covered_pitch_spaces: covered_pitch_spaces.into_iter().collect(),
            omitted_pitch_spaces,
            covered_pitch_classes: covered_pitch_classes.into_iter().collect(),
            omitted_pitch_classes,
        },
        score,
    })
}

fn finger_assignment(choices: &[StringChoice]) -> Option<Vec<Option<u8>>> {
    let fretted_positions = choices
        .iter()
        .enumerate()
        .filter_map(|(string_index, choice)| {
            choice
                .fret
                .filter(|fret| *fret > 0)
                .map(|fret| (string_index, fret))
        })
        .collect::<Vec<_>>();

    if fretted_positions.is_empty() {
        return Some(vec![None; choices.len()]);
    }

    if !fret_span_is_reachable(&fretted_positions) {
        return None;
    }

    if fretted_positions.len() <= 4 {
        let groups = fretted_positions
            .iter()
            .enumerate()
            .map(|(position_index, (string_index, fret))| FingerGroup {
                fret: *fret,
                start: *string_index,
                covers: vec![position_index],
            })
            .collect::<Vec<_>>();
        return assignment_from_groups(groups, choices.len(), &fretted_positions);
    }

    let barres = possible_barres(choices, &fretted_positions);
    let mut best: Option<(usize, usize, Vec<FingerGroup>)> = None;
    choose_barres(&barres, 0, Vec::new(), &fretted_positions, &mut best);

    let (_, _, groups) = best?;
    assignment_from_groups(groups, choices.len(), &fretted_positions)
}

fn fret_span_is_reachable(fretted_positions: &[(usize, u8)]) -> bool {
    let Some(lowest) = fretted_positions.iter().map(|(_, fret)| *fret).min() else {
        return true;
    };
    let highest = fretted_positions
        .iter()
        .map(|(_, fret)| *fret)
        .max()
        .unwrap_or(lowest);
    let span = highest - lowest;

    span <= if lowest >= 5 { 4 } else { 3 }
}

fn assignment_from_groups(
    mut groups: Vec<FingerGroup>,
    string_count: usize,
    fretted_positions: &[(usize, u8)],
) -> Option<Vec<Option<u8>>> {
    if groups.len() > 4 {
        return None;
    }

    groups.sort_by(|left, right| {
        left.fret
            .cmp(&right.fret)
            .then_with(|| left.start.cmp(&right.start))
    });

    let mut assignment = vec![None; string_count];
    for (finger_index, group) in groups.into_iter().enumerate() {
        let finger = (finger_index + 1) as u8;
        for position_index in group.covers {
            let string_index = fretted_positions[position_index].0;
            assignment[string_index] = Some(finger);
        }
    }
    Some(assignment)
}

fn possible_barres(choices: &[StringChoice], fretted_positions: &[(usize, u8)]) -> Vec<Barre> {
    let frets = fretted_positions
        .iter()
        .map(|(_, fret)| *fret)
        .collect::<BTreeSet<_>>();
    let mut barres = Vec::new();

    for fret in frets {
        for start in 0..choices.len() {
            for end in (start + 1)..choices.len() {
                if !barre_range_is_clear(choices, fret, start, end) {
                    continue;
                }

                let covers = fretted_positions
                    .iter()
                    .enumerate()
                    .filter_map(|(position_index, (string_index, position_fret))| {
                        (*position_fret == fret && (start..=end).contains(string_index))
                            .then_some(position_index)
                    })
                    .collect::<Vec<_>>();

                if covers.len() >= 2 {
                    barres.push(Barre {
                        fret,
                        start,
                        end,
                        covers,
                    });
                }
            }
        }
    }

    barres
}

fn barre_range_is_clear(choices: &[StringChoice], fret: u8, start: usize, end: usize) -> bool {
    choices[start..=end]
        .iter()
        .all(|choice| choice.fret.is_some_and(|string_fret| string_fret >= fret))
}

fn choose_barres(
    barres: &[Barre],
    index: usize,
    selected: Vec<Barre>,
    fretted_positions: &[(usize, u8)],
    best: &mut Option<(usize, usize, Vec<FingerGroup>)>,
) {
    if index == barres.len() {
        let Some(groups) = finger_groups_from_barres(selected, fretted_positions) else {
            return;
        };
        if groups.len() > 4 {
            return;
        }

        let barre_span = groups
            .iter()
            .filter(|group| group.covers.len() > 1)
            .map(|group| group.covers.len())
            .sum::<usize>();
        let key = (groups.len(), usize::MAX - barre_span);
        if best
            .as_ref()
            .is_none_or(|(best_count, best_barre_score, _)| key < (*best_count, *best_barre_score))
        {
            *best = Some((key.0, key.1, groups));
        }
        return;
    }

    choose_barres(barres, index + 1, selected.clone(), fretted_positions, best);

    let mut next = selected;
    next.push(barres[index].clone());
    choose_barres(barres, index + 1, next, fretted_positions, best);
}

fn finger_groups_from_barres(
    barres: Vec<Barre>,
    fretted_positions: &[(usize, u8)],
) -> Option<Vec<FingerGroup>> {
    let mut covered = vec![false; fretted_positions.len()];
    let mut groups = Vec::new();

    for barre in barres {
        if barre.covers.iter().any(|position| covered[*position]) {
            return None;
        }
        for position in &barre.covers {
            covered[*position] = true;
        }
        groups.push(FingerGroup {
            fret: barre.fret,
            start: barre.start.min(barre.end),
            covers: barre.covers,
        });
    }

    for (position_index, (string_index, fret)) in fretted_positions.iter().enumerate() {
        if !covered[position_index] {
            groups.push(FingerGroup {
                fret: *fret,
                start: *string_index,
                covers: vec![position_index],
            });
        }
    }

    Some(groups)
}

fn internal_muted_string_count(choices: &[StringChoice], sounding_indices: &[usize]) -> usize {
    let Some(first) = sounding_indices.first() else {
        return 0;
    };
    let Some(last) = sounding_indices.last() else {
        return 0;
    };

    choices[*first..=*last]
        .iter()
        .filter(|choice| choice.fret.is_none())
        .count()
}

fn pitch_class(pitch: &Pitch) -> u8 {
    (pitch.ps().round() as IntegerType).rem_euclid(12) as u8
}

fn pitch_space(pitch: &Pitch) -> crate::Result<IntegerType> {
    let pitch_space = pitch.ps();
    let rounded = pitch_space.round();
    if (pitch_space - rounded).abs() > FloatType::EPSILON {
        return Err(crate::Error::Chord(
            "guitar fingering requires chromatic pitches".to_string(),
        ));
    }
    Ok(rounded as IntegerType)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choices(frets: &[Option<u8>]) -> Vec<StringChoice> {
        frets
            .iter()
            .copied()
            .map(|fret| StringChoice {
                fret,
                pitch_space: None,
                pitch_class: None,
            })
            .collect()
    }

    #[test]
    fn finger_assignment_rejects_more_than_four_independent_fingers() {
        assert!(
            finger_assignment(&choices(&[
                Some(1),
                Some(2),
                Some(3),
                Some(4),
                Some(5),
                None
            ]))
            .is_none()
        );
    }

    #[test]
    fn finger_assignment_accepts_barre_shapes() {
        let assignment = finger_assignment(&choices(&[
            Some(1),
            Some(3),
            Some(3),
            Some(2),
            Some(1),
            Some(1),
        ]))
        .unwrap();

        assert_eq!(
            assignment.iter().flatten().collect::<BTreeSet<_>>().len(),
            3
        );
    }

    #[test]
    fn finger_assignment_rejects_low_position_five_fret_stretches() {
        assert!(
            finger_assignment(&choices(&[Some(1), Some(2), Some(3), Some(5), None, None]))
                .is_none()
        );
    }

    #[test]
    fn finger_assignment_allows_higher_position_extended_reaches() {
        let assignment =
            finger_assignment(&choices(&[Some(5), Some(7), Some(8), Some(9), None, None])).unwrap();

        assert_eq!(assignment.iter().flatten().count(), 4);
    }

    #[test]
    fn finger_assignment_does_not_barre_over_open_string() {
        let assignment = finger_assignment(&choices(&[
            Some(1),
            Some(2),
            Some(0),
            Some(3),
            Some(1),
            Some(1),
        ]))
        .unwrap();

        assert!(assignment.iter().flatten().collect::<BTreeSet<_>>().len() <= 4);
        assert_ne!(assignment[0], assignment[4]);
    }
}
