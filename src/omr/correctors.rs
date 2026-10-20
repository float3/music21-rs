//! Correcting the rhythm of a score read by optical music recognition:
//! music21's `omr.correctors`.
//!
//! A measure whose notes do not fill its bar was most likely misread. The
//! correctors look for the measure that most likely stands where it should
//! -- another measure of the same part a likely distance away whose rhythm
//! differs from it in likely ways (the horizontal model), or the measure
//! at the same place in another part that tends to move with it (the
//! vertical model) -- and put that measure's rhythm in its place, keeping
//! its pitches.
//!
//! Rhythms are compared as text: each note, chord or rest of a measure is
//! one character for its length, notes on even codes and rests on odd
//! ones ([`measure_hash`]).

use std::collections::HashMap;

use crate::defaults::FloatType;
use crate::error::{Error, Result};
use crate::meter::TimeSignature;
use crate::search::difflib;
use crate::stream::{Stream, StreamElement, StreamEvent, StreamKind};

pub use crate::search::difflib::{Opcode, OpcodeTag};

/// A flagged measure and the measure found to correct it, with how likely
/// the correction is: music21's `MeasureRelationship`.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MeasureRelationship {
    /// The part of the flagged measure.
    pub flagged_measure_part: usize,
    /// The flagged measure's index among its part's measures.
    pub flagged_measure_index: usize,
    /// The part of the measure correcting it.
    pub correct_measure_part: usize,
    /// The correcting measure's index among its part's measures.
    pub correct_measure_index: usize,
    /// How likely the correction is.
    pub correction_probability: FloatType,
}

/// How many flagged measures were corrected, and by which model: music21's
/// `PriorsIntegrationScore`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PriorsIntegrationScore {
    /// Flagged measures both models found a correction for.
    pub total: usize,
    /// Those the horizontal model's correction was used for.
    pub horizontal: usize,
    /// Those the vertical model's correction was used for.
    pub vertical: usize,
    /// Those left as they were, which music21 never counts.
    pub ignored: usize,
}

/// How likely a note is read right: music21's `getProbabilityOnEquality`.
pub const PROBABILITY_ON_EQUALITY: FloatType = 0.9675;
/// How likely a note is missed: music21's `getProbabilityOnOmission`.
pub const PROBABILITY_ON_OMISSION: FloatType = 0.009;
/// How likely a note is read that is not there: music21's
/// `getProbabilityOnAddition`.
pub const PROBABILITY_ON_ADDITION: FloatType = 0.004;

/// The rhythm of a measure as text, a character for each note, chord or
/// rest, as music21's `MeasureHash.getHashString` writes it. A measure
/// holding voices is chordified first.
///
/// The character is ten times the base-2 logarithm of the length in 256ths
/// of a quarter, from 1 to 127 -- made even for a note or chord and odd for
/// a rest -- and that of a 64th for anything of no length. A stroke with no
/// pitch is left out.
///
/// ```
/// use music21_rs::omr::correctors::measure_hash;
/// use music21_rs::tinynotation::from_tiny_notation;
///
/// let line = from_tiny_notation("4/4 c4 d8 r8 e2")?;
/// let measure = line.measures()[0];
/// assert_eq!(measure_hash(measure)?, "PFGZ");
/// # Ok::<(), music21_rs::Error>(())
/// ```
///
/// # Errors
///
/// A measure of voices that cannot be chordified.
pub fn measure_hash(measure: &Stream) -> Result<String> {
    let flat = !measure
        .events()
        .iter()
        .any(|event| matches!(event.element(), StreamElement::Stream(_)));
    let chordified;
    let source = if flat {
        measure
    } else {
        chordified = measure.chordify()?;
        &chordified
    };
    let mut hash = String::new();
    for event in source.events() {
        let element = event.element();
        if !is_general_note(element) {
            continue;
        }
        let length = element.quarter_length();
        let code = if length == 0.0 {
            Some(hash_grace())
        } else {
            match element {
                StreamElement::Note(_)
                | StreamElement::Chord(_)
                | StreamElement::ChordSymbol(_) => Some(hash_note(length)),
                StreamElement::Rest(_) => Some(hash_rest(length)),
                _ => None,
            }
        };
        if let Some(code) = code {
            hash.push(code);
        }
    }
    Ok(hash)
}

/// music21's `notesAndRests`: what has a duration of a note.
fn is_general_note(element: &StreamElement) -> bool {
    matches!(
        element,
        StreamElement::Note(_)
            | StreamElement::Chord(_)
            | StreamElement::Rest(_)
            | StreamElement::Unpitched(_)
            | StreamElement::PercussionChord(_)
            | StreamElement::ChordSymbol(_)
    )
}

/// music21's `hashQuarterLength`: ten times the base-2 logarithm of a
/// length in 256ths of a quarter, from 1 to 127, and 1 for no length.
pub fn hash_quarter_length(length: FloatType) -> u8 {
    if length == 0.0 {
        return 1;
    }
    let code = ((length * 256.0).log2() * 10.0) as i64;
    code.clamp(1, 127) as u8
}

/// The character a note or chord of a length is written as: music21's
/// `hashNote`, which is [`hash_quarter_length`] made even.
pub fn hash_note(length: FloatType) -> char {
    let code = hash_quarter_length(length);
    char::from(if code.is_multiple_of(2) {
        code
    } else {
        code + 1
    })
}

/// The character a rest of a length is written as: music21's `hashRest`,
/// which is [`hash_quarter_length`] made odd.
pub fn hash_rest(length: FloatType) -> char {
    let code = hash_quarter_length(length);
    char::from(if code.is_multiple_of(2) {
        code + 1
    } else {
        code
    })
}

/// The character anything of no length is written as: music21's
/// `hashGrace`, which writes a grace note as a 128th.
pub fn hash_grace() -> char {
    char::from(hash_quarter_length(0.015625))
}

/// How unlike two rhythms are, as music21's `getMeasureDifference` has it:
/// one less their `difflib` likeness, except that the same rhythm is as
/// unlike as can be, one.
pub fn measure_difference(own: &str, other: &str) -> FloatType {
    difference(&chars(own), &chars(other))
}

fn chars(hash: &str) -> Vec<char> {
    hash.chars().collect()
}

fn difference(own: &[char], other: &[char]) -> FloatType {
    let ratio = difflib::ratio(own, other);
    1.0 - if ratio == 1.0 { 0.0 } else { ratio }
}

/// The steps that turn one rhythm into another: Python's `difflib` opcodes,
/// music21's `getOpCodes`.
pub fn opcodes(own: &str, other: &str) -> Vec<Opcode> {
    difflib::opcodes(&chars(own), &chars(other))
}

/// How likely `other` was misread as `own`, from the steps that turn the
/// one into the other: music21's `getProbabilityBasedOnChanges`. Each equal
/// note is [`PROBABILITY_ON_EQUALITY`], each one missed
/// [`PROBABILITY_ON_OMISSION`], each one added [`PROBABILITY_ON_ADDITION`],
/// and a stretch replaced is [`probability_on_substitute`].
pub fn probability_based_on_changes(own: &str, other: &str) -> FloatType {
    changes(&chars(own), &chars(other))
}

fn changes(own: &[char], other: &[char]) -> FloatType {
    changes_between(own, other, own)
}

/// How likely the measure `source` was misread as `destination`, with the
/// steps read between `matching` and `source`: music21's
/// `getProbabilityBasedOnChanges` with its two strings told apart.
///
/// music21 reads its steps off whatever its `SequenceMatcher` was set
/// from and reads a replacement off its own hash, which are the same
/// string in every call it makes -- [`probability_based_on_changes`] is
/// that call -- but a caller may set the one and not the other.
pub fn probability_based_on_changes_between(
    matching: &str,
    source: &str,
    destination: &str,
) -> FloatType {
    changes_between(&chars(matching), &chars(source), &chars(destination))
}

fn changes_between(matching: &[char], source: &[char], destination: &[char]) -> FloatType {
    let mut probability = 0.0;
    for (index, step) in difflib::opcodes(matching, source).iter().enumerate() {
        let one = one_opcode(step, source, destination);
        if index == 0 {
            probability = one;
        } else {
            probability *= one;
        }
    }
    probability
}

/// How likely one of the steps that turn a rhythm into another is:
/// music21's `differenceProbabilityForOneOpCode`.
///
/// `source` is the measure that would do the correcting and `destination`
/// the measure flagged, which is how music21 names them. A stretch read
/// alike is [`PROBABILITY_ON_EQUALITY`] for each character, a stretch
/// replaced is [`probability_on_substitute`] of the two, a stretch the
/// flagged measure has not got is [`PROBABILITY_ON_OMISSION`] for each and
/// one it has over is [`PROBABILITY_ON_ADDITION`].
pub fn probability_for_one_opcode(step: &Opcode, source: &str, destination: &str) -> FloatType {
    one_opcode(step, &chars(source), &chars(destination))
}

fn one_opcode(step: &Opcode, source: &[char], destination: &[char]) -> FloatType {
    match step.tag {
        OpcodeTag::Equal => PROBABILITY_ON_EQUALITY.powf((step.j2 - step.j1) as FloatType),
        OpcodeTag::Replace => substitute(&source[step.j1..step.j2], &destination[step.i1..step.i2]),
        OpcodeTag::Insert => PROBABILITY_ON_OMISSION.powf((step.j2 - step.j1) as FloatType),
        OpcodeTag::Delete => PROBABILITY_ON_ADDITION.powf((step.i2 - step.i1) as FloatType),
    }
}

/// How likely the rhythm `source` was misread as `destination`: music21's
/// `getProbabilityOnSubstitute`. The longer is cut to the shorter, each
/// character cut an addition or an omission, and the rest compared
/// character by character ([`probability_from_one_char_sub`]).
pub fn probability_on_substitute(source: &str, destination: &str) -> FloatType {
    substitute(&chars(source), &chars(destination))
}

fn substitute(source: &[char], destination: &[char]) -> FloatType {
    let (mut source, mut destination) = (source, destination);
    let mut probability = 1.0;
    if source.len() > destination.len() {
        let additions = source.len() - destination.len();
        probability = PROBABILITY_ON_ADDITION.powf(additions as FloatType);
        source = &source[..source.len() - additions];
    } else if source.len() < destination.len() {
        let omissions = destination.len() - source.len();
        probability = PROBABILITY_ON_OMISSION.powf(omissions as FloatType);
        destination = &destination[..destination.len() - omissions];
    }
    for (from, to) in source.iter().zip(destination) {
        probability *= probability_from_one_char_sub(*from, *to);
    }
    probability
}

/// How likely one length was misread as another: music21's
/// `getProbabilityFromOneCharSub`. A length doubled or halved any number of
/// times is 0.0165 for each; a rest read for a note of the same length an
/// addition, the other way an omission; a note read as a rest 0.003; and
/// anything else an omission and an addition at once.
pub fn probability_from_one_char_sub(source: char, destination: char) -> FloatType {
    let difference = source as i64 - destination as i64;
    let distance = difference.abs() as FloatType;
    if difference == 0 {
        1.0
    } else if distance % 10.0 == 0.0 {
        0.0165_f64.powf(distance / 10.0)
    } else if difference == 6 {
        PROBABILITY_ON_ADDITION
    } else if difference == -6 {
        PROBABILITY_ON_OMISSION
    } else if distance % 2.0 != 0.0 {
        0.003
    } else {
        PROBABILITY_ON_OMISSION * PROBABILITY_ON_ADDITION
    }
}

/// The best of some probabilities, and the first that is as good: where
/// music21 corrects a measure from.
fn best(probabilities: &[FloatType]) -> (usize, FloatType) {
    let most = probabilities
        .iter()
        .copied()
        .fold(FloatType::NEG_INFINITY, FloatType::max);
    let at = probabilities.iter().position(|&p| p == most).unwrap_or(0);
    (at, most)
}

/// The rhythm of every measure of a part: music21's
/// `SinglePart.getSequenceHashesFromMeasureStream`.
///
/// # Errors
///
/// A measure of voices that cannot be chordified.
pub fn part_hashes(part: &Stream) -> Result<Vec<String>> {
    ScoreCorrector::measure_events(part)
        .into_iter()
        .map(|index| match part.events()[index].element() {
            StreamElement::Stream(measure) => measure_hash(measure),
            _ => unreachable!("a measure's event holds the measure"),
        })
        .collect()
}

/// Which of a part's measures do not fill their bar, and so would be
/// flagged by an optical reader: music21's
/// `SinglePart.getIncorrectMeasureIndices`.
///
/// With `run_fast`, every bar is that of the first measure's time
/// signature, or of 4/4 where none is in force; without, each measure's
/// own. The places are counted from nought, as music21 counts them, and
/// are not measure numbers.
///
/// # Errors
///
/// Without `run_fast`, a measure with no time signature in force, which
/// music21 fails on.
pub fn incorrect_measures(part: &Stream, run_fast: bool) -> Result<Vec<usize>> {
    let measures = ScoreCorrector::measure_events(part);
    let meter_of = |index: usize| -> Option<TimeSignature> {
        let event = &part.events()[index];
        let StreamElement::Stream(measure) = event.element() else {
            return None;
        };
        measure
            .events()
            .iter()
            .take_while(|inner| inner.offset() == 0.0)
            .find_map(|inner| match inner.element() {
                StreamElement::TimeSignature(meter) => Some(meter.clone()),
                _ => None,
            })
            .or_else(|| part.time_signature_at(event.offset()))
    };
    let common_time = || TimeSignature::new(4, 4);
    let mut meter = if run_fast {
        match measures.first().and_then(|&index| meter_of(index)) {
            Some(meter) => meter,
            None => common_time()?,
        }
    } else {
        common_time()?
    };
    let mut incorrect = Vec::new();
    for (i, &index) in measures.iter().enumerate() {
        if !run_fast {
            meter = meter_of(index).ok_or_else(|| {
                Error::Omr("'NoneType' object has no attribute 'barDuration'".to_string())
            })?;
        }
        if part.events()[index].element().quarter_length() != meter.bar_quarter_length() {
            incorrect.push(i);
        }
    }
    Ok(incorrect)
}

/// Which measure of one place across the parts most likely corrects the
/// flagged one there: music21's `MeasureSlice.runSliceSearch`.
///
/// `hashes` is the rhythm of that measure in every part and `flagged`
/// says whose is itself flagged, since a flagged measure corrects
/// nothing; `distribution` is how often each part's measures read like
/// the flagged part's, which is one row of
/// [`ScoreCorrector::vertical_probability_dist`]. The likeliest is the
/// first of those as likely, and a part that cannot correct counts as
/// nothing at all.
pub fn slice_search(
    hashes: &[String],
    flagged: &[bool],
    distribution: &[FloatType],
    part: usize,
    measure: usize,
) -> MeasureRelationship {
    let mut probabilities = Vec::with_capacity(hashes.len());
    for (k, hash) in hashes.iter().enumerate() {
        let cannot = k == part || flagged.get(k).copied().unwrap_or(false);
        if cannot {
            probabilities.push(0.0);
            continue;
        }
        let likely = probability_based_on_changes(&hashes[part], hash);
        probabilities.push(likely * distribution.get(k).copied().unwrap_or(0.0));
    }
    let (at, most) = best(&probabilities);
    MeasureRelationship {
        flagged_measure_part: part,
        flagged_measure_index: measure,
        correct_measure_part: at,
        correct_measure_index: measure,
        correction_probability: most,
    }
}

/// One part of a score being corrected, its measures' rhythms and those
/// flagged: music21's `SinglePart`.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SinglePart {
    part_number: usize,
    hashed_notes: Vec<String>,
    incorrect_measures: Vec<usize>,
    probability_distribution: Option<Vec<FloatType>>,
    index_array: Option<Vec<i64>>,
}

impl SinglePart {
    /// One part read on its own, its measures hashed and those not
    /// filling their bar flagged: music21's `SinglePart(part, pn)`, whose
    /// own constructor does both.
    ///
    /// # Errors
    ///
    /// A measure of voices that cannot be chordified.
    pub fn from_part(part: &Stream, number: usize) -> Result<Self> {
        Ok(Self {
            part_number: number,
            hashed_notes: part_hashes(part)?,
            incorrect_measures: incorrect_measures(part, true)?,
            probability_distribution: None,
            index_array: None,
        })
    }

    /// The part's place among the score's parts.
    pub fn part_number(&self) -> usize {
        self.part_number
    }

    /// The rhythm of each measure, as [`measure_hash`] writes it, as they
    /// stood when the corrector was made or the part last corrected:
    /// music21's `hashedNotes`.
    pub fn hashed_notes(&self) -> &[String] {
        &self.hashed_notes
    }

    /// The indices of the measures whose notes do not fill their bars:
    /// music21's `incorrectMeasures`.
    pub fn incorrect_measures(&self) -> &[usize] {
        &self.incorrect_measures
    }

    /// For each distance from one measure to another, from one less than
    /// the number of measures back to as many forward, the share of
    /// measures whose rhythm is like the one that far away -- as music21
    /// counts likeness, where the same rhythm and a wholly different one
    /// both count, and the distance nought counts every measure: music21's
    /// `horizontalProbabilityDist`. Worked out once and kept.
    pub fn horizontal_probability_dist(&mut self) -> &[FloatType] {
        if self.probability_distribution.is_none() {
            let count = self.hashed_notes.len();
            let hashes: Vec<Vec<char>> = self.hashed_notes.iter().map(|hash| chars(hash)).collect();
            let mut all = vec![0.0; count * 2];
            let mut indices = vec![0_i64; count * 2];
            for i in 0..count {
                for k in 0..count {
                    let at = count + k - i;
                    indices[at] = k as i64 - i as i64;
                    if i == k {
                        all[at] = count as FloatType;
                    } else if difference(&hashes[i], &hashes[k]) == 1.0 {
                        all[at] += 1.0;
                    }
                }
            }
            if !indices.is_empty() {
                indices.remove(0);
                all.remove(0);
            }
            self.index_array = Some(indices);
            self.probability_distribution =
                Some(all.iter().map(|value| value / count as FloatType).collect());
        }
        self.probability_distribution.as_deref().unwrap_or_default()
    }

    /// The distance from one measure to another each entry of
    /// [`Self::horizontal_probability_dist`] is for, once it has been worked
    /// out: music21's `indexArray`.
    pub fn index_array(&self) -> Option<&[i64]> {
        self.index_array.as_deref()
    }

    /// How likely a measure as far from another as `source` is from
    /// `destination` has its rhythm: music21's `getProbabilityDistribution`.
    pub fn probability_distribution(&mut self, source: usize, destination: usize) -> FloatType {
        let count = self.hashed_notes.len();
        let index = source + count - 1 - destination;
        self.horizontal_probability_dist()[index]
    }

    /// The measure of this part most likely to correct the `i`th flagged
    /// one: of those not flagged, the one most likely misread as it by its
    /// changes, times how likely its distance is, the first of those as
    /// likely: music21's `runHorizontalSearch`.
    pub fn run_horizontal_search(&mut self, i: usize) -> MeasureRelationship {
        self.horizontal_probability_dist();
        let incorrect = self.incorrect_measures[i];
        let own = chars(&self.hashed_notes[incorrect]);
        let mut probabilities = Vec::with_capacity(self.hashed_notes.len());
        for k in 0..self.hashed_notes.len() {
            if self.incorrect_measures.contains(&k) {
                probabilities.push(0.0);
            } else {
                let likely = changes(&own, &chars(&self.hashed_notes[k]));
                probabilities.push(likely * self.probability_distribution(k, incorrect));
            }
        }
        let (at, most) = best(&probabilities);
        MeasureRelationship {
            flagged_measure_part: self.part_number,
            flagged_measure_index: incorrect,
            correct_measure_part: self.part_number,
            correct_measure_index: at,
            correction_probability: most,
        }
    }

    /// [`Self::run_horizontal_search`] for every flagged measure: music21's
    /// `runHorizontalCorrectionModel`.
    pub fn run_horizontal_correction_model(&mut self) -> Vec<MeasureRelationship> {
        (0..self.incorrect_measures.len())
            .map(|i| self.run_horizontal_search(i))
            .collect()
    }
}

/// A score being corrected, part by part: music21's `ScoreCorrector`.
///
/// ```
/// use music21_rs::omr::correctors::ScoreCorrector;
/// use music21_rs::tinynotation::from_tiny_notation;
/// use music21_rs::{Stream, StreamKind};
///
/// // The third bar is a quarter short.
/// let part = from_tiny_notation("4/4 c4 d e f g a b c' d'4 e' f'")?;
/// let mut score = Stream::with_kind(StreamKind::Score);
/// score.insert(0.0, part);
///
/// let mut corrector = ScoreCorrector::new(score)?;
/// assert_eq!(corrector.single_parts()[0].incorrect_measures(), [2]);
/// corrector.run()?;
/// assert_eq!(corrector.single_parts()[0].hashed_notes()[2], "PPPP");
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct ScoreCorrector {
    score: Stream,
    single_parts: Vec<SinglePart>,
    /// Where each part stands among the score's events.
    part_events: Vec<usize>,
    distribution: Option<Vec<Vec<FloatType>>>,
    /// The rhythms of the measures at each place across the parts, as they
    /// stood when that place was first searched.
    slices: HashMap<usize, Vec<String>>,
}

impl ScoreCorrector {
    /// A corrector for a score: each part's measures hashed and those not
    /// filling the bar of the score's first time signature flagged.
    ///
    /// # Errors
    ///
    /// A measure of voices that cannot be chordified.
    pub fn new(score: Stream) -> Result<Self> {
        let part_events: Vec<usize> = score
            .events()
            .iter()
            .enumerate()
            .filter_map(|(index, event)| match event.element() {
                StreamElement::Stream(inner)
                    if matches!(inner.kind(), StreamKind::Part | StreamKind::PartStaff) =>
                {
                    Some(index)
                }
                _ => None,
            })
            .collect();
        let mut corrector = Self {
            score,
            single_parts: Vec::new(),
            part_events,
            distribution: None,
            slices: HashMap::new(),
        };
        for part_number in 0..corrector.part_events.len() {
            let single = SinglePart::from_part(corrector.part(part_number), part_number)?;
            corrector.single_parts.push(single);
        }
        Ok(corrector)
    }

    /// The score, as corrected so far.
    pub fn score(&self) -> &Stream {
        &self.score
    }

    /// The score, as corrected so far, given back.
    pub fn into_score(self) -> Stream {
        self.score
    }

    /// The parts being corrected: music21's `singleParts`.
    pub fn single_parts(&self) -> &[SinglePart] {
        &self.single_parts
    }

    /// One part, to run its horizontal model on its own.
    pub fn single_part_mut(&mut self, part: usize) -> &mut SinglePart {
        &mut self.single_parts[part]
    }

    /// Every part's measure rhythms: music21's `getAllHashes`.
    pub fn all_hashes(&self) -> Vec<&[String]> {
        self.single_parts
            .iter()
            .map(SinglePart::hashed_notes)
            .collect()
    }

    /// Every part's flagged measures: music21's `getAllIncorrectMeasures`.
    pub fn all_incorrect_measures(&self) -> Vec<&[usize]> {
        self.single_parts
            .iter()
            .map(SinglePart::incorrect_measures)
            .collect()
    }

    fn part(&self, part: usize) -> &Stream {
        match self.score.events()[self.part_events[part]].element() {
            StreamElement::Stream(inner) => inner,
            _ => unreachable!("a part's event holds the part"),
        }
    }

    /// The events of a part that are its measures.
    fn measure_events(part: &Stream) -> Vec<usize> {
        part.events()
            .iter()
            .enumerate()
            .filter_map(|(index, event)| match event.element() {
                StreamElement::Stream(inner) if inner.kind() == StreamKind::Measure => Some(index),
                _ => None,
            })
            .collect()
    }

    fn hash_part(&self, part: usize) -> Result<Vec<String>> {
        part_hashes(self.part(part))
    }

    /// Flags a part's measures whose notes do not fill their bar, and
    /// gives them back: music21's `getIncorrectMeasureIndices`. With
    /// `run_fast`, every bar is that of the first measure's time signature,
    /// or of 4/4 where none is in force; without, each measure's own.
    ///
    /// # Errors
    ///
    /// Without `run_fast`, a measure with no time signature in force,
    /// which music21 fails on.
    pub fn find_incorrect_measures(&mut self, part: usize, run_fast: bool) -> Result<&[usize]> {
        self.single_parts[part].incorrect_measures = incorrect_measures(self.part(part), run_fast)?;
        Ok(&self.single_parts[part].incorrect_measures)
    }

    /// Every part's horizontal model: music21's
    /// `runHorizontalCorrectionModel`.
    pub fn run_horizontal_correction_model(&mut self) -> Vec<Vec<MeasureRelationship>> {
        self.single_parts
            .iter_mut()
            .map(SinglePart::run_horizontal_correction_model)
            .collect()
    }

    /// For each part, the share of its measures whose rhythm is like that
    /// of the measure at the same place in each part, as music21 counts
    /// likeness: music21's `verticalProbabilityDist`. Worked out once and
    /// kept.
    ///
    /// # Errors
    ///
    /// Parts of different lengths, or one with no measures, which music21
    /// fails on.
    pub fn vertical_probability_dist(&mut self) -> Result<&[Vec<FloatType>]> {
        if self.distribution.is_none() {
            let mut distribution = Vec::with_capacity(self.single_parts.len());
            for part in 0..self.single_parts.len() {
                distribution.push(self.vertical_probability_dist_of_part(part)?);
            }
            self.distribution = Some(distribution);
        }
        Ok(self.distribution.as_deref().unwrap_or_default())
    }

    /// How often each part's measures read like one part's, that part's own
    /// counting for one: music21's
    /// `getVerticalProbabilityDistributionSinglePart`.
    ///
    /// # Errors
    ///
    /// A part with no measures, or one shorter than this part.
    pub fn vertical_probability_dist_of_part(&self, part: usize) -> Result<Vec<FloatType>> {
        let measures = self
            .single_parts
            .get(part)
            .ok_or_else(|| Error::Omr("list index out of range".to_string()))?
            .hashed_notes
            .len();
        if measures == 0 {
            return Err(Error::Omr("division by zero".to_string()));
        }
        let mut sums = vec![0.0; self.single_parts.len()];
        for measure in 0..measures {
            for (sum, alike) in sums
                .iter_mut()
                .zip(self.vertical_probability_dist_of_measure(part, measure)?)
            {
                *sum += alike;
            }
        }
        Ok(sums.iter().map(|sum| sum / measures as FloatType).collect())
    }

    /// Which parts read one measure of one part exactly alike, that part's
    /// own counting for one: music21's
    /// `getVerticalProbabilityDistributionSinglePartSingleMeasure`.
    ///
    /// # Errors
    ///
    /// A part or a measure there is none of.
    pub fn vertical_probability_dist_of_measure(
        &self,
        part: usize,
        measure: usize,
    ) -> Result<Vec<FloatType>> {
        let hash = self
            .single_parts
            .get(part)
            .and_then(|single| single.hashed_notes.get(measure))
            .ok_or_else(|| Error::Omr("list index out of range".to_string()))?;
        let mut alike = Vec::with_capacity(self.single_parts.len());
        for (other, single) in self.single_parts.iter().enumerate() {
            if other == part {
                alike.push(1.0);
                continue;
            }
            let their = single
                .hashed_notes
                .get(measure)
                .ok_or_else(|| Error::Omr("list index out of range".to_string()))?;
            alike.push(if measure_difference(hash, their) == 1.0 {
                1.0
            } else {
                0.0
            });
        }
        Ok(alike)
    }

    /// The rhythm of one measure in every part: music21's
    /// `MeasureSlice.getSliceHashes`, the slice its `getMeasureSlice`
    /// makes.
    ///
    /// # Errors
    ///
    /// A measure there is none of in some part, or one of voices that
    /// cannot be chordified.
    pub fn slice_hashes(&mut self, measure: usize) -> Result<&[String]> {
        if !self.slices.contains_key(&measure) {
            let mut hashes = Vec::with_capacity(self.single_parts.len());
            for number in 0..self.single_parts.len() {
                let stream = self.part(number);
                let index = *Self::measure_events(stream)
                    .get(measure)
                    .ok_or_else(|| Error::Omr("list index out of range".to_string()))?;
                let StreamElement::Stream(inner) = stream.events()[index].element() else {
                    unreachable!("a measure's event holds the measure");
                };
                hashes.push(measure_hash(inner)?);
            }
            self.slices.insert(measure, hashes);
        }
        Ok(&self.slices[&measure])
    }

    /// The measure at the same place in another part most likely to
    /// correct a flagged one: of the parts whose measure there is not
    /// flagged, the one most likely misread as it by its changes, times how
    /// often the two parts' rhythms are alike, the first of those as
    /// likely: music21's `runVerticalSearch`.
    ///
    /// # Errors
    ///
    /// As [`Self::vertical_probability_dist`], or a measure of voices that
    /// cannot be chordified.
    pub fn run_vertical_search(
        &mut self,
        measure: usize,
        part: usize,
    ) -> Result<MeasureRelationship> {
        self.vertical_probability_dist()?;
        self.slice_hashes(measure)?;
        let flagged: Vec<bool> = self
            .single_parts
            .iter()
            .map(|single| single.incorrect_measures.contains(&measure))
            .collect();
        let hashes = &self.slices[&measure];
        let distribution = self.distribution.as_deref().unwrap_or_default();
        Ok(slice_search(
            hashes,
            &flagged,
            distribution.get(part).map_or(&[][..], Vec::as_slice),
            part,
            measure,
        ))
    }

    /// [`Self::run_vertical_search`] for every flagged measure of every
    /// part: music21's `runVerticalCorrectionModel`.
    ///
    /// # Errors
    ///
    /// As [`Self::run_vertical_search`].
    pub fn run_vertical_correction_model(&mut self) -> Result<Vec<Vec<MeasureRelationship>>> {
        self.vertical_probability_dist()?;
        let mut all = Vec::with_capacity(self.single_parts.len());
        for part in 0..self.single_parts.len() {
            let flagged = self.single_parts[part].incorrect_measures.clone();
            all.push(
                flagged
                    .into_iter()
                    .map(|measure| self.run_vertical_search(measure, part))
                    .collect::<Result<Vec<_>>>()?,
            );
        }
        Ok(all)
    }

    /// Puts the contents of one measure in another's place, the notes
    /// keeping the pitches the replaced measure's notes had, in order, as
    /// far as they go: music21's `substituteOneMeasureContentsForAnother`.
    ///
    /// As music21 does, everything is appended one after another, a
    /// measure's right barline and its voices included: a barline ends up
    /// inside the measure, and voices one after another rather than
    /// together. Spanners naming the replaced measure's notes name nothing
    /// afterwards. A measure put in its own place is emptied, as music21
    /// empties it before copying from it.
    ///
    /// # Errors
    ///
    /// A pitch whose accidental cannot be written again from its modifier.
    ///
    /// # Panics
    ///
    /// A part or measure past the last.
    pub fn substitute_measure(
        &mut self,
        source_measure: usize,
        source_part: usize,
        destination_measure: usize,
        destination_part: usize,
    ) -> Result<()> {
        let source = if (source_part, source_measure) == (destination_part, destination_measure) {
            Stream::with_kind(StreamKind::Measure)
        } else {
            let stream = self.part(source_part);
            let index = Self::measure_events(stream)[source_measure];
            match stream.events()[index].element() {
                StreamElement::Stream(inner) => (**inner).clone(),
                _ => unreachable!("a measure's event holds the measure"),
            }
        };
        let destination = self.part(destination_part);
        let index = Self::measure_events(destination)[destination_measure];
        let StreamElement::Stream(measure) = destination.events()[index].element() else {
            unreachable!("a measure's event holds the measure");
        };
        let old_pitches: Vec<crate::pitch::Pitch> = measure
            .events()
            .iter()
            .filter_map(|event| match event.element() {
                StreamElement::Note(note) => Some(note.pitch().clone()),
                _ => None,
            })
            .collect();
        // What music21 iterates: the elements in order, the left barline
        // among them at the start, and the right barline after them all.
        let mut elements: Vec<StreamEvent> = source.events().to_vec();
        if let Some(barline) = source.left_barline() {
            let at = elements
                .iter()
                .position(|event| event.offset() > 0.0 || event.element().class_sort_order() > -5)
                .unwrap_or(elements.len());
            elements.insert(
                at,
                StreamEvent::new(0.0, StreamElement::Barline(barline.clone())),
            );
        }
        if let Some(barline) = source.right_barline() {
            elements.push(StreamEvent::new(
                0.0,
                StreamElement::Barline(barline.clone()),
            ));
        }
        let mut contents = Stream::with_kind(StreamKind::Measure);
        let (mut left, mut pitch_index) = (None, 0);
        for event in elements {
            let mut element = event.element().clone();
            if let StreamElement::Note(note) = &mut element
                && let Some(old) = old_pitches.get(pitch_index)
            {
                let mut pitch = note.pitch().clone();
                pitch.set_octave(old.octave());
                pitch.take_name_of(old)?;
                note.set_pitch(pitch);
                pitch_index += 1;
            }
            let offset = contents.end_offset();
            match element {
                // A barline at the start is the measure's left barline.
                StreamElement::Barline(barline) if offset == 0.0 && left.is_none() => {
                    left = Some(barline);
                }
                element => contents.push(element),
            }
        }
        let mut replaced = measure.with_events(contents.events().to_vec());
        replaced.set_left_barline(left);
        replaced.set_right_barline(None);
        let path = [self.part_events[destination_part], index];
        self.score.replace_nested(&path, replaced);
        Ok(())
    }

    /// Corrects each flagged measure both models found a correction for,
    /// from the horizontal model's measure where it is more likely, and
    /// from the vertical model's otherwise, then hashes each part's
    /// measures again: music21's `generateCorrectedScore`. Corrections are
    /// made in turn, so a measure corrected may correct another after it.
    ///
    /// # Errors
    ///
    /// A measure of voices that cannot be chordified.
    pub fn generate_corrected_score(
        &mut self,
        horizontal: &[Vec<MeasureRelationship>],
        vertical: &[Vec<MeasureRelationship>],
    ) -> Result<PriorsIntegrationScore> {
        let mut score = PriorsIntegrationScore::default();
        for part in 0..self.single_parts.len() {
            let out_of_range = || Error::Omr("list index out of range".to_string());
            let across = horizontal.get(part).ok_or_else(out_of_range)?;
            for h in across {
                for v in vertical.get(part).ok_or_else(out_of_range)? {
                    if h.flagged_measure_part != v.flagged_measure_part
                        || h.flagged_measure_index != v.flagged_measure_index
                    {
                        continue;
                    }
                    score.total += 1;
                    let source = if h.correction_probability > v.correction_probability {
                        score.horizontal += 1;
                        h
                    } else {
                        score.vertical += 1;
                        v
                    };
                    self.substitute_measure(
                        source.correct_measure_index,
                        source.correct_measure_part,
                        h.flagged_measure_index,
                        h.flagged_measure_part,
                    )?;
                }
            }
            self.single_parts[part].hashed_notes = self.hash_part(part)?;
        }
        Ok(score)
    }

    /// Corrects the score by both models: music21's `run` and
    /// `runPriorModel`. The score corrected is [`Self::score`].
    ///
    /// # Errors
    ///
    /// As [`Self::run_vertical_correction_model`].
    pub fn run(&mut self) -> Result<PriorsIntegrationScore> {
        let horizontal = self.run_horizontal_correction_model();
        let vertical = self.run_vertical_correction_model()?;
        self.generate_corrected_score(&horizontal, &vertical)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_are_hashed_as_music21_hashes_them() {
        assert_eq!(hash_quarter_length(1.0), 80);
        assert_eq!(hash_quarter_length(0.015625), 20);
        assert_eq!(hash_quarter_length(0.0), 1);
        assert_eq!(hash_quarter_length(1000.0), 127);
    }

    /// music21's `hashNote`, `hashRest` and `hashQuarterLength`
    /// docstrings, and the measure its `getHashString` writes `VFUF` for.
    #[test]
    fn a_note_is_written_as_music21_writes_it() -> Result<()> {
        assert_eq!(hash_quarter_length(1.0), 80);
        assert_eq!(hash_quarter_length(2.0), 90);
        assert_eq!(hash_note(1.0), 'P');
        assert_eq!(hash_note(2.0), 'Z');
        assert_eq!(hash_note(1.5), 'V');
        assert_eq!(hash_rest(1.0), 'Q');
        // A grace note is written as a 128th.
        assert_eq!(hash_grace(), char::from(hash_quarter_length(0.015625)));
        let mut measure = Stream::with_kind(StreamKind::Measure);
        for (name, length) in [("C4", 1.5), ("C4", 0.5)] {
            measure.push(
                crate::note::Note::from_name(name)?
                    .with_duration(crate::duration::Duration::new(length)?),
            );
        }
        measure.push(crate::Rest::new(crate::duration::Duration::new(1.5)?));
        measure.push(
            crate::note::Note::from_name("B4")?.with_duration(crate::duration::Duration::new(0.5)?),
        );
        assert_eq!(measure_hash(&measure)?, "VFUF");
        assert_eq!(measure_difference("VFUF", "VGUF"), 0.25);
        assert_eq!(measure_difference("VFUF", "VFUF"), 1.0);
        Ok(())
    }

    /// music21's `differenceProbabilityForOneOpCode` docstring: one step
    /// of each kind, between the rhythms it names.
    #[test]
    fn one_step_of_a_correction_is_as_likely_as_music21_has_it() {
        let step = |tag, i1, i2, j1, j2| Opcode {
            tag,
            i1,
            i2,
            j1,
            j2,
        };
        assert_eq!(
            probability_for_one_opcode(&step(OpcodeTag::Equal, 0, 4, 0, 4), "PFPFFF", "PFPFGF"),
            0.876_201_303_164_062_6
        );
        assert_eq!(
            probability_for_one_opcode(&step(OpcodeTag::Insert, 4, 4, 4, 5), "PFPFFF", "PFPFGF"),
            0.009
        );
        assert_eq!(
            probability_for_one_opcode(&step(OpcodeTag::Replace, 2, 4, 2, 4), "PPPPP", "PPVZP"),
            0.000_148_5
        );
        assert_eq!(
            probability_for_one_opcode(&step(OpcodeTag::Delete, 0, 5, 0, 0), "e", "GFPGF"),
            1.024e-12
        );
        // The whole correction is every step multiplied, which is
        // music21's `getProbabilityBasedOnChanges`.
        assert_eq!(
            probability_based_on_changes("GFPGF", "e"),
            2.947_283_212_5e-14
        );
    }

    #[test]
    fn a_rhythm_misread_is_as_likely_as_music21_has_it() {
        // Read off music21's MeasureHash methods.
        assert_eq!(probability_from_one_char_sub('P', 'P'), 1.0);
        assert_eq!(probability_from_one_char_sub('Z', 'P'), 0.0165);
        assert_eq!(
            probability_from_one_char_sub('V', 'P'),
            PROBABILITY_ON_ADDITION
        );
        assert_eq!(probability_from_one_char_sub('Q', 'P'), 0.003);
        assert_eq!(measure_difference("PPPP", "PPPP"), 1.0);
        assert_eq!(measure_difference("", ""), 1.0);
        assert_eq!(measure_difference("PP", "ZZ"), 1.0);
    }
}
