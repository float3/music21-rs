//! Realizing a figured bass line: music21's `figuredBass.realizer`.
//!
//! A [`FiguredBassLine`] is a run of bass notes, each with its figures, in a
//! key. Realizing it makes a [`Segment`] of each note, finds every pair of
//! voicings each segment may move to the next by, and drops the voicings
//! that lead nowhere, leaving a [`Realization`]: every way of voicing the
//! whole line that keeps the rules, which it can count, list, or pick from.
//!
//! music21 builds its realizations into scores; this answers the voicings
//! themselves, a pitch per part for each bass note, highest part first.

use crate::defaults::FloatType;
use crate::error::{Error, Result};
use crate::figuredbass::Notation;
use crate::figuredbass::rules::Rules;
use crate::figuredbass::scale::{FiguredBassMode, FiguredBassScale};
use crate::figuredbass::segment::{Possibility, Segment};
use crate::key::Key;
use crate::pitch::Pitch;

/// One thing a bass line is realized over.
#[derive(Clone, Debug, PartialEq)]
enum Element {
    /// A bass note and the figures written under it.
    Figured {
        bass: Pitch,
        quarter_length: FloatType,
        notation: Option<String>,
    },
    /// A bass note and the names of the notes above it, as music21 reads a
    /// chord symbol or a roman numeral.
    Named {
        bass: Pitch,
        quarter_length: FloatType,
        pitch_names: Vec<String>,
    },
}

/// A bass line and its figures, in a key.
///
/// ```
/// use music21_rs::Pitch;
/// use music21_rs::figuredbass::realizer::FiguredBassLine;
/// use music21_rs::figuredbass::rules::Rules;
///
/// let mut line = FiguredBassLine::default();
/// line.add_element(Pitch::from_name("C3")?, 1.0, None);
/// line.add_element(Pitch::from_name("D3")?, 1.0, Some("4,3"));
/// line.add_element(Pitch::from_name("C3")?, 2.0, None);
/// let realization = line.realize(&Rules::default(), 4, &Pitch::from_name("B5")?)?;
/// assert_eq!(realization.num_solutions(), 30);
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
#[must_use]
pub struct FiguredBassLine {
    scale: FiguredBassScale,
    elements: Vec<Element>,
}

impl Default for FiguredBassLine {
    /// A line in C major, with nothing in it yet.
    fn default() -> Self {
        Self::new(FiguredBassScale::default())
    }
}

impl FiguredBassLine {
    /// An empty line whose figures are read in `scale`.
    pub fn new(scale: FiguredBassScale) -> Self {
        Self {
            scale,
            elements: Vec::new(),
        }
    }

    /// An empty line in `key`, which must be in one of the five modes a
    /// figured bass is read in.
    ///
    /// # Errors
    ///
    /// A key in another mode.
    pub fn in_key(key: &Key) -> Result<Self> {
        let tonic = key.pitch_from_degree(1)?;
        let mode = FiguredBassMode::from_name(key.mode())?;
        Ok(Self::new(FiguredBassScale::new(tonic, mode)?))
    }

    /// The scale its figures are read in.
    pub fn scale(&self) -> &FiguredBassScale {
        &self.scale
    }

    /// Adds a bass note lasting `quarter_length`, with the figures written
    /// under it, if any: music21's `addElement` of a note.
    pub fn add_element(&mut self, bass: Pitch, quarter_length: FloatType, notation: Option<&str>) {
        self.elements.push(Element::Figured {
            bass,
            quarter_length,
            notation: notation.map(str::to_string),
        });
    }

    /// Adds a bass note with the names of the notes above it rather than
    /// figures, as music21 reads a chord symbol or a roman numeral into a
    /// line. A chord lasting nothing is realized as lasting a quarter, as
    /// music21's is.
    pub fn add_chord(&mut self, bass: Pitch, quarter_length: FloatType, pitch_names: Vec<String>) {
        self.elements.push(Element::Named {
            bass,
            quarter_length: if quarter_length == 0.0 {
                1.0
            } else {
                quarter_length
            },
            pitch_names,
        });
    }

    /// How many bass notes it has.
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Whether it has none.
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// A segment for each bass note, voiced in `num_parts` parts under
    /// `rules`, no part above `max_pitch`.
    ///
    /// # Errors
    ///
    /// Figures the line's scale cannot read.
    pub fn segments(
        &self,
        rules: &Rules,
        num_parts: usize,
        max_pitch: &Pitch,
    ) -> Result<Vec<Segment>> {
        self.elements
            .iter()
            .map(|element| match element {
                Element::Figured {
                    bass,
                    quarter_length,
                    notation,
                } => Ok(Segment::new(
                    bass.clone(),
                    *quarter_length,
                    &Notation::parse(notation.as_deref().unwrap_or(""))?,
                    &self.scale,
                    rules.clone(),
                    num_parts,
                    max_pitch.clone(),
                )?
                .overlaid()),
                Element::Named {
                    bass,
                    quarter_length,
                    pitch_names,
                } => Segment::from_pitch_names(
                    bass.clone(),
                    *quarter_length,
                    pitch_names.clone(),
                    rules.clone(),
                    num_parts,
                    max_pitch.clone(),
                ),
            })
            .collect()
    }

    /// Every way of voicing the line in `num_parts` parts that keeps
    /// `rules`, no part above `max_pitch`: music21's `realize`.
    ///
    /// # Errors
    ///
    /// A line with nothing in it, figures its scale cannot read, or a
    /// resolution that fails.
    pub fn realize(
        &self,
        rules: &Rules,
        num_parts: usize,
        max_pitch: &Pitch,
    ) -> Result<Realization> {
        if self.elements.is_empty() {
            return Err(Error::FiguredBass(
                "No (bassNote, notationString) pairs to realize.".to_string(),
            ));
        }
        Realization::of(self.segments(rules, num_parts, max_pitch)?)
    }
}

/// Where each voicing of one segment may go in the next, in the order the
/// voicings were found.
type Movements = Vec<(Possibility, Vec<Possibility>)>;

/// Every way of voicing a figured bass line: music21's `Realization`.
#[derive(Clone, Debug)]
#[must_use]
pub struct Realization {
    segments: Vec<Segment>,
    /// For each segment but the last, where its voicings may go.
    movements: Vec<Movements>,
    /// The voicings of a line of one segment.
    single: Vec<Possibility>,
    /// What each chord that fell back to an ordinary resolution warned.
    fallbacks: Vec<String>,
}

impl Realization {
    /// The realization of `segments`, a line's segments in order.
    ///
    /// # Errors
    ///
    /// No segments, or a resolution that fails.
    pub fn of(mut segments: Vec<Segment>) -> Result<Self> {
        if segments.is_empty() {
            return Err(Error::FiguredBass(
                "No (bassNote, notationString) pairs to realize.".to_string(),
            ));
        }
        let mut movements: Vec<Movements> = Vec::new();
        let mut fallbacks = Vec::new();
        let mut single = Vec::new();
        if segments.len() == 1 {
            single = segments[0].all_correct_single_possibilities()?;
        }
        for index in 0..segments.len().saturating_sub(1) {
            let (before, after) = segments.split_at_mut(index + 1);
            let found = before[index].all_correct_consecutive_possibilities(&mut after[0])?;
            fallbacks.extend(found.fallback);
            let mut moves: Movements = Vec::new();
            for (from, to) in found.pairs {
                match moves.iter_mut().find(|(known, _)| *known == from) {
                    Some((_, targets)) => targets.push(to),
                    None => moves.push((from, vec![to])),
                }
            }
            movements.push(moves);
        }
        trim(&mut movements);
        Ok(Self {
            segments,
            movements,
            single,
            fallbacks,
        })
    }

    /// The segments realized, one for each bass note.
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// What each chord that found no resolution of its own warned, in
    /// order.
    pub fn fallbacks(&self) -> &[String] {
        &self.fallbacks
    }

    /// How many ways there are of voicing the whole line: music21's
    /// `getNumSolutions`.
    pub fn num_solutions(&self) -> usize {
        if self.segments.len() == 1 {
            return self.single.len();
        }
        let mut paths: Vec<(&Possibility, usize)> = Vec::new();
        for (depth, moves) in self.movements.iter().rev().enumerate() {
            paths = moves
                .iter()
                .map(|(from, targets)| {
                    let count = if depth == 0 {
                        targets.len()
                    } else {
                        targets
                            .iter()
                            .map(|target| {
                                paths
                                    .iter()
                                    .find(|(known, _)| *known == target)
                                    .map_or(0, |(_, count)| *count)
                            })
                            .sum()
                    };
                    (from, count)
                })
                .collect();
        }
        paths.iter().map(|(_, count)| count).sum()
    }

    /// Every way of voicing the whole line, a voicing for each bass note:
    /// music21's `getAllPossibilityProgressions`, in its order.
    pub fn all_possibility_progressions(&self) -> Vec<Vec<Possibility>> {
        if self.segments.len() == 1 {
            return self
                .single
                .iter()
                .map(|voicing| vec![voicing.clone()])
                .collect();
        }
        let Some(first) = self.movements.first() else {
            return Vec::new();
        };
        let mut progressions: Vec<Vec<Possibility>> = first
            .iter()
            .flat_map(|(from, targets)| {
                targets
                    .iter()
                    .map(move |target| vec![from.clone(), target.clone()])
            })
            .collect();
        for moves in &self.movements[1..] {
            progressions = progressions
                .into_iter()
                .flat_map(|progression| {
                    let last = progression.last().cloned().unwrap_or_default();
                    moves
                        .iter()
                        .find(|(from, _)| *from == last)
                        .map(|(_, targets)| targets.clone())
                        .unwrap_or_default()
                        .into_iter()
                        .map(move |target| {
                            let mut longer = progression.clone();
                            longer.push(target);
                            longer
                        })
                })
                .collect();
        }
        progressions
    }

    /// One way of voicing the whole line, each voicing chosen by `choose`,
    /// which is given how many there are to choose from and answers the
    /// index of one: music21's `getRandomPossibilityProgression`, with the
    /// choosing left to the caller.
    ///
    /// # Errors
    ///
    /// A line with no way of voicing it, or a choice out of range.
    pub fn possibility_progression_by(
        &self,
        mut choose: impl FnMut(usize) -> usize,
    ) -> Result<Vec<Possibility>> {
        let mut pick = |options: &[Possibility]| -> Result<Possibility> {
            let index = choose(options.len());
            options.get(index).cloned().ok_or_else(|| {
                Error::FiguredBass(format!("no voicing {index} of {}", options.len()))
            })
        };
        if self.segments.len() == 1 {
            return Ok(vec![pick(&self.single)?]);
        }
        if self.num_solutions() == 0 {
            return Err(Error::FiguredBass("Zero solutions".to_string()));
        }
        let firsts: Vec<Possibility> = self.movements[0]
            .iter()
            .map(|(from, _)| from.clone())
            .collect();
        let mut previous = pick(&firsts)?;
        let mut progression = vec![previous.clone()];
        for moves in &self.movements {
            let targets = moves
                .iter()
                .find(|(from, _)| *from == previous)
                .map(|(_, targets)| targets.as_slice())
                .unwrap_or_default();
            previous = pick(targets)?;
            progression.push(previous.clone());
        }
        Ok(progression)
    }
}

/// Drops every move that leads to a voicing with nowhere to go, from the
/// end of the line back, as music21's `_trimAllMovements` does.
fn trim(movements: &mut [Movements]) {
    if movements.len() < 2 {
        return;
    }
    for index in (1..movements.len()).rev() {
        movements[index].retain(|(_, targets)| !targets.is_empty());
        let (before, after) = movements.split_at_mut(index);
        let reachable = &after[0];
        for (_, targets) in &mut before[index - 1] {
            targets.retain(|target| reachable.iter().any(|(from, _)| from == target));
        }
    }
    movements[0].retain(|(_, targets)| !targets.is_empty());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(key: &str, notes: &[(&str, FloatType, Option<&str>)]) -> FiguredBassLine {
        let key = Key::from_tonic(key).unwrap();
        let mut line = FiguredBassLine::in_key(&key).unwrap();
        for (bass, length, notation) in notes {
            line.add_element(Pitch::from_name(*bass).unwrap(), *length, *notation);
        }
        line
    }

    /// A line, the key it is in, how many ways it voices, and the first
    /// voicing of the first way and the last voicing of the last.
    type Case = (
        &'static str,
        &'static str,
        Vec<(&'static str, FloatType, Option<&'static str>)>,
        usize,
        [&'static str; 4],
        [&'static str; 4],
    );

    fn names(voicing: &[Pitch]) -> Vec<String> {
        voicing.iter().map(Pitch::name_with_octave).collect()
    }

    /// Every count and voicing here is music21's own.
    #[test]
    fn a_line_realizes_as_music21_s_does() {
        let top = Pitch::from_name("B5").unwrap();
        let cases: [Case; 6] = [
            (
                "intro",
                "C",
                vec![
                    ("C3", 1.0, None),
                    ("D3", 1.0, Some("4,3")),
                    ("C3", 2.0, None),
                ],
                30,
                ["G3", "G3", "E3", "C3"],
                ["G5", "E5", "C5", "C3"],
            ),
            (
                "minor",
                "a",
                vec![
                    ("A2", 1.0, None),
                    ("B2", 1.0, Some("6")),
                    ("C3", 1.0, Some("6")),
                    ("D3", 1.0, Some("6")),
                    ("E3", 1.0, None),
                    ("A2", 2.0, None),
                ],
                2829,
                ["A3", "E3", "C3", "A2"],
                ["A5", "E5", "C5", "A2"],
            ),
            (
                "dominant seventh",
                "C",
                vec![("G2", 1.0, Some("7")), ("C3", 1.0, None)],
                7,
                ["F3", "D3", "B2", "G2"],
                ["E5", "C5", "C5", "C3"],
            ),
            (
                "diminished seventh",
                "c",
                vec![("B2", 1.0, Some("-7")), ("C3", 1.0, None)],
                35,
                ["Abb3", "F3", "D3", "B2"],
                ["G5", "G5", "Eb5", "C3"],
            ),
            (
                "German sixth",
                "C",
                vec![("Ab2", 1.0, Some("#6,b5,3")), ("G2", 1.0, None)],
                7,
                ["F#3", "Eb3", "C3", "Ab2"],
                ["G5", "D5", "B4", "G2"],
            ),
            (
                "one chord",
                "C",
                vec![("C3", 1.0, None)],
                21,
                ["G3", "E3", "C3", "C3"],
                ["G5", "G5", "E5", "C3"],
            ),
        ];
        for (label, key, notes, count, first, last) in cases {
            let realization = line(key, &notes)
                .realize(&Rules::default(), 4, &top)
                .unwrap();
            assert_eq!(realization.num_solutions(), count, "{label}");
            let progressions = realization.all_possibility_progressions();
            assert_eq!(progressions.len(), count, "{label}");
            assert_eq!(names(&progressions[0][0]), first, "{label}: first");
            let final_voicing = progressions.last().unwrap().last().unwrap();
            assert_eq!(names(final_voicing), last, "{label}: last");
        }

        let limited = Rules {
            part_movement_limits: vec![(1, 2), (2, 12), (3, 12)],
            ..Rules::default()
        };
        let realization = line(
            "C",
            &[
                ("C3", 1.0, None),
                ("D3", 1.0, Some("4,3")),
                ("C3", 2.0, None),
            ],
        )
        .realize(&limited, 4, &top)
        .unwrap();
        assert_eq!(realization.num_solutions(), 20);

        let picked = line("C", &[("G2", 1.0, Some("7")), ("C3", 1.0, None)])
            .realize(&Rules::default(), 4, &top)
            .unwrap()
            .possibility_progression_by(|_| 0)
            .unwrap();
        assert_eq!(names(&picked[0]), ["F3", "D3", "B2", "G2"]);
        assert_eq!(names(&picked[1]), ["E3", "C3", "C3", "C3"]);
    }
}
