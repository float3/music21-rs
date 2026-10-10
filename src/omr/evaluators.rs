//! Measuring how well the correctors do: music21's `omr.evaluators`. A
//! score read by optical music recognition is held to the same score
//! entered by hand, its ground truth, by how many measures' rhythms differ.

use crate::error::{Error, Result};
use crate::stream::Stream;

use super::correctors::ScoreCorrector;

/// How many measures it takes to turn one list of measure rhythms into
/// another, a measure put in or left out costing one and one replaced two:
/// music21's `minEditDist`.
pub fn min_edit_distance(target: &[String], source: &[String]) -> usize {
    let (n, m) = (target.len(), source.len());
    let mut distance = vec![vec![0; m + 1]; n + 1];
    for i in 1..=n {
        distance[i][0] = distance[i - 1][0] + 1;
    }
    for j in 1..=m {
        distance[0][j] = distance[0][j - 1] + 1;
    }
    for i in 1..=n {
        for j in 1..=m {
            let substitution = if source[j - 1] == target[i - 1] { 0 } else { 2 };
            distance[i][j] = (distance[i - 1][j] + 1)
                .min(distance[i][j - 1] + 1)
                .min(distance[i - 1][j - 1] + substitution);
        }
    }
    distance[n][m]
}

/// A score read by optical music recognition and its ground truth, each
/// ready to be corrected: music21's `OmrGroundTruthPair`.
#[derive(Clone, Debug)]
pub struct OmrGroundTruthPair {
    omr: ScoreCorrector,
    ground: ScoreCorrector,
}

impl OmrGroundTruthPair {
    /// The pair, each score in a corrector.
    ///
    /// # Errors
    ///
    /// As [`ScoreCorrector::new`].
    pub fn new(omr: Stream, ground: Stream) -> Result<Self> {
        Ok(Self {
            omr: ScoreCorrector::new(omr)?,
            ground: ScoreCorrector::new(ground)?,
        })
    }

    /// The score read by optical music recognition.
    pub fn omr(&self) -> &ScoreCorrector {
        &self.omr
    }

    /// The same, to be corrected.
    pub fn omr_mut(&mut self) -> &mut ScoreCorrector {
        &mut self.omr
    }

    /// The ground truth.
    pub fn ground(&self) -> &ScoreCorrector {
        &self.ground
    }

    /// The edit distances between the measure rhythms of each part of the
    /// recognised score and those of the same part of the ground truth,
    /// summed: music21's `getDifferences`.
    ///
    /// # Errors
    ///
    /// A ground truth with fewer parts, which music21 fails on.
    pub fn differences(&self) -> Result<usize> {
        let theirs = self.ground.all_hashes();
        let mut total = 0;
        for (part, ours) in self.omr.all_hashes().into_iter().enumerate() {
            let truth = theirs
                .get(part)
                .ok_or_else(|| Error::Omr("list index out of range".to_string()))?;
            total += min_edit_distance(ours, truth);
        }
        Ok(total)
    }
}

/// What [`evaluate_correcting_model`] finds: music21's dictionary of the
/// same.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Evaluation {
    /// The edit distance from the ground truth before correcting.
    pub original_edit_distance: usize,
    /// The edit distance after.
    pub new_edit_distance: usize,
    /// How many measures were flagged.
    pub number_of_flagged_measures: usize,
    /// How many measures the recognised score has.
    pub total_number_of_measures: usize,
}

/// Corrects a recognised score and measures it against its ground truth
/// before and after: music21's `evaluateCorrectingModel`. Measures are
/// flagged against each one's own time signature; `original_differences`
/// stands for the distance before, where it is known.
///
/// # Errors
///
/// As [`OmrGroundTruthPair::differences`],
/// [`ScoreCorrector::find_incorrect_measures`] and [`ScoreCorrector::run`].
pub fn evaluate_correcting_model(
    omr: Stream,
    ground: Stream,
    original_differences: Option<usize>,
) -> Result<Evaluation> {
    let mut pair = OmrGroundTruthPair::new(omr, ground)?;
    let original_edit_distance = match original_differences {
        Some(differences) => differences,
        None => pair.differences()?,
    };
    let corrector = pair.omr_mut();
    let mut horizontal = Vec::new();
    let (mut flagged, mut total) = (0, 0);
    for part in 0..corrector.single_parts().len() {
        flagged += corrector.find_incorrect_measures(part, false)?.len();
        horizontal.push(
            corrector
                .single_part_mut(part)
                .run_horizontal_correction_model(),
        );
        total += corrector.single_parts()[part].hashed_notes().len();
    }
    let vertical = corrector.run_vertical_correction_model()?;
    corrector.generate_corrected_score(&horizontal, &vertical)?;
    Ok(Evaluation {
        original_edit_distance,
        new_edit_distance: pair.differences()?,
        number_of_flagged_measures: flagged,
        total_number_of_measures: total,
    })
}

/// How many measures not flagged have their rhythm somewhere else -- in
/// another measure of their part, or at the same place in another part --
/// out of how many there are: music21's `autoCorrelationBestMeasure`, as
/// `(measures, matches)`.
///
/// # Errors
///
/// As [`ScoreCorrector::new`] and
/// [`ScoreCorrector::find_incorrect_measures`], or parts of different
/// lengths where music21 runs out of measures.
pub fn auto_correlation_best_measure(score: Stream) -> Result<(usize, usize)> {
    let mut corrector = ScoreCorrector::new(score)?;
    let hashes: Vec<Vec<String>> = corrector
        .all_hashes()
        .into_iter()
        .map(<[String]>::to_vec)
        .collect();
    let (mut measures, mut matches) = (0, 0);
    for (part, own) in hashes.iter().enumerate() {
        let incorrect = corrector.find_incorrect_measures(part, false)?.to_vec();
        for (i, hash) in own.iter().enumerate() {
            if incorrect.contains(&i) {
                continue;
            }
            measures += 1;
            let mut found = own
                .iter()
                .enumerate()
                .any(|(j, other)| i != j && other == hash);
            if !found {
                for (other_part, theirs) in hashes.iter().enumerate() {
                    if other_part == part {
                        continue;
                    }
                    let other = theirs
                        .get(i)
                        .ok_or_else(|| Error::Omr("list index out of range".to_string()))?;
                    if other == hash {
                        found = true;
                        break;
                    }
                }
            }
            if found {
                matches += 1;
            }
        }
    }
    Ok((measures, matches))
}
