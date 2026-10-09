//! Runs of notes climbing or falling through a scale: music21's
//! `alpha.analysis.search`.
//!
//! ```
//! use music21_rs::alpha::analysis::search::{ScaleRunOptions, find_consecutive_scale};
//! use music21_rs::scale::{Scale, ScaleType};
//! use music21_rs::tinynotation::from_tiny_notation;
//! use music21_rs::Pitch;
//!
//! let line = from_tiny_notation("4/4 a4 b c'# d' e' f'#")?.flatten();
//! let a_major = Scale::new(ScaleType::Major, Pitch::from_name("A4")?);
//! let runs = find_consecutive_scale(&line, &a_major, &ScaleRunOptions::default())?;
//! assert_eq!(runs.len(), 1);
//! assert_eq!(runs[0].elements.len(), 6);
//! # Ok::<(), music21_rs::Error>(())
//! ```

use std::collections::BTreeSet;

use crate::error::{Error, Result};
use crate::pitch::Pitch;
use crate::scale::{DegreeComparison, Scale};
use crate::stream::{Stream, StreamElement};

/// Which way a run goes through the scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RunDirection {
    /// Up the scale.
    Ascending,
    /// Down the scale.
    Descending,
}

/// What two pitches must share to count as the same: music21's
/// `comparisonAttribute`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PitchAttribute {
    /// The name, octave aside: `name`.
    #[default]
    Name,
    /// The name and the octave: `nameWithOctave`.
    NameWithOctave,
    /// The pitch class: `pitchClass`.
    PitchClass,
    /// The letter: `step`.
    Step,
}

impl PitchAttribute {
    fn key(self, pitch: &Pitch) -> String {
        match self {
            Self::Name => pitch.name(),
            Self::NameWithOctave => pitch.name_with_octave(),
            Self::PitchClass => pitch.pitch_class().number().to_string(),
            Self::Step => pitch.step().as_char().to_string(),
        }
    }

    /// How a scale degree is found under this comparison. music21 looks a
    /// pitch up among the scale's pitches realized about it, octave and
    /// all, so a name with its octave finds the degree its name does.
    fn degree_comparison(self) -> DegreeComparison {
        match self {
            Self::Name | Self::NameWithOctave => DegreeComparison::Name,
            Self::PitchClass => DegreeComparison::PitchClass,
            Self::Step => DegreeComparison::Step,
        }
    }
}

/// music21's keyword arguments to `findConsecutiveScale`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScaleRunOptions {
    /// How many different degrees a run must reach: `degreesRequired`.
    pub degrees_required: usize,
    /// How many degrees each step of a run moves: `stepSize`.
    pub step_size: usize,
    /// What a pitch and a scale pitch must share: `comparisonAttribute`.
    pub comparison: PitchAttribute,
    /// Whether a degree repeated keeps a run going: `repeatsAllowed`.
    pub repeats_allowed: bool,
}

impl Default for ScaleRunOptions {
    fn default() -> Self {
        Self {
            degrees_required: 5,
            step_size: 1,
            comparison: PitchAttribute::Name,
            repeats_allowed: true,
        }
    }
}

/// A run found: the notes in it, by their place among the stream's own
/// elements, and the way it goes, where it has gone one.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScaleRun {
    /// The places of the notes in the stream's own elements, in order.
    pub elements: Vec<usize>,
    /// Which way the run went.
    pub direction: Option<RunDirection>,
}

/// The degree a pitch stands on, read in a direction, or both where none
/// is said: music21's `getScaleDegreeFromPitch`.
fn degree(
    scale: &Scale,
    pitch: &Pitch,
    direction: Option<RunDirection>,
    comparison: PitchAttribute,
) -> Result<Option<usize>> {
    let comparison = comparison.degree_comparison();
    match direction {
        Some(RunDirection::Ascending) => scale.degree_of_by(pitch, comparison),
        Some(RunDirection::Descending) => scale.descending().degree_of_by(pitch, comparison),
        None => match scale.degree_of_by(pitch, comparison)? {
            Some(found) => Ok(Some(found)),
            None => scale.descending().degree_of_by(pitch, comparison),
        },
    }
}

/// Whether `other` is the pitch `steps` degrees from `origin` in a
/// direction: music21's `isNext`, which is false where there is no other
/// pitch and refuses where no direction is said.
fn is_next(
    scale: &Scale,
    other: Option<&Pitch>,
    origin: &Pitch,
    direction: Option<RunDirection>,
    steps: usize,
    comparison: PitchAttribute,
) -> Result<bool> {
    let Some(other) = other else {
        return Ok(false);
    };
    let next = match direction {
        Some(RunDirection::Ascending) => scale.next_pitch_beside(origin, steps as _, true)?,
        Some(RunDirection::Descending) => scale.descending().next_pitch_beside(
            origin,
            -(steps as crate::defaults::IntegerType),
            false,
        )?,
        None => {
            return Err(Error::Scale(
                "cannot match direction specification: None".to_string(),
            ));
        }
    };
    Ok(comparison.key(&next) == comparison.key(other))
}

/// Every run of notes in a stream going one way through a scale, a degree
/// at a time, that reaches at least as many different degrees as asked:
/// music21's `findConsecutiveScale`.
///
/// The stream's own notes are read in order, its chords, rests and
/// streams passed over -- so a rest never breaks a run, whatever
/// music21's `restsAllowed` says, since it never reads one. A run is
/// greedy: it goes on as long as the next note continues it. A note off
/// the scale ends a run; a note on it that does not continue the run ends
/// it and starts the next.
///
/// # Errors
///
/// A scale its pitches cannot be read in, or a run asked for of fewer than
/// two degrees, which music21 cannot tell the way of.
pub fn find_consecutive_scale(
    source: &Stream,
    scale: &Scale,
    options: &ScaleRunOptions,
) -> Result<Vec<ScaleRun>> {
    let notes: Vec<(usize, Pitch)> = source
        .events()
        .iter()
        .enumerate()
        .filter_map(|(place, event)| match event.element() {
            StreamElement::Note(note) => Some((place, note.pitch().clone())),
            _ => None,
        })
        .collect();
    let mut order: Vec<usize> = (0..notes.len()).collect();
    order.sort_by(|&left, &right| {
        source.events()[notes[left].0]
            .offset()
            .total_cmp(&source.events()[notes[right].0].offset())
    });
    let notes: Vec<(usize, Pitch)> = order
        .into_iter()
        .map(|index| notes[index].clone())
        .collect();

    let comparison = options.comparison;
    let step = options.step_size;
    let mut degree_last: Option<usize> = None;
    let mut pitch_last: Option<Pitch> = None;
    let mut direction_last: Option<RunDirection> = None;
    let mut degrees: BTreeSet<usize> = BTreeSet::new();
    let mut elements: Vec<usize> = Vec::new();
    let mut clear = false;
    let mut clear_keeping_last = false;
    let mut runs = Vec::new();

    for (index, (place, pitch)) in notes.iter().enumerate() {
        let next = notes.get(index + 1).map(|(_, next)| next);
        let found_degree = degree(scale, pitch, direction_last, comparison)?;
        match found_degree {
            None => clear = true,
            Some(found) => {
                let collect = match (degree_last, &pitch_last) {
                    (None, _) | (_, None) => true,
                    (Some(last), _) if options.repeats_allowed && last == found => true,
                    (Some(_), Some(previous)) => {
                        if matches!(direction_last, None | Some(RunDirection::Ascending))
                            && is_next(
                                scale,
                                Some(pitch),
                                previous,
                                Some(RunDirection::Ascending),
                                step,
                                comparison,
                            )?
                        {
                            direction_last = Some(RunDirection::Ascending);
                            true
                        } else if matches!(direction_last, None | Some(RunDirection::Descending))
                            && is_next(
                                scale,
                                Some(pitch),
                                previous,
                                Some(RunDirection::Descending),
                                step,
                                comparison,
                            )?
                        {
                            direction_last = Some(RunDirection::Descending);
                            true
                        } else {
                            clear_keeping_last = true;
                            false
                        }
                    }
                };
                if collect {
                    degrees.insert(found);
                    elements.push(*place);
                }
            }
        }
        degree_last = found_degree;
        pitch_last = Some(pitch.clone());

        if degrees.len() >= options.degrees_required
            && !is_next(scale, next, pitch, direction_last, 1, comparison)?
        {
            runs.push(ScaleRun {
                elements: elements.clone(),
                direction: direction_last,
            });
            if !clear && !clear_keeping_last {
                let continues = is_next(
                    scale,
                    next,
                    pitch,
                    Some(RunDirection::Descending),
                    step,
                    comparison,
                )? || is_next(
                    scale,
                    next,
                    pitch,
                    Some(RunDirection::Ascending),
                    step,
                    comparison,
                )?;
                if continues {
                    clear_keeping_last = true;
                } else {
                    clear = true;
                }
            }
        }
        if clear {
            degree_last = None;
            direction_last = None;
            degrees.clear();
            elements.clear();
            clear = false;
        }
        if clear_keeping_last {
            direction_last = None;
            degrees.clear();
            // music21 keeps the degree the note was last found on, which
            // is nothing where it is off the scale.
            if let Some(found) = found_degree {
                degrees.insert(found);
            }
            elements = vec![*place];
            clear_keeping_last = false;
        }
    }
    Ok(runs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note::Note;
    use crate::scale::ScaleType;

    fn line(names: &[&str]) -> Stream {
        let mut stream = Stream::new();
        for name in names {
            stream.push(Note::from_name(name).unwrap());
        }
        stream
    }

    fn runs(names: &[&str], required: usize, comparison: PitchAttribute) -> Vec<(usize, String)> {
        let source = line(names);
        let a_major = Scale::new(ScaleType::Major, Pitch::from_name("A4").unwrap());
        let options = ScaleRunOptions {
            degrees_required: required,
            comparison,
            ..ScaleRunOptions::default()
        };
        find_consecutive_scale(&source, &a_major, &options)
            .unwrap()
            .into_iter()
            .map(|run| {
                let StreamElement::Note(first) = source.events()[run.elements[0]].element() else {
                    unreachable!("a run holds notes")
                };
                (run.elements.len(), first.pitch().name_with_octave())
            })
            .collect()
    }

    /// music21's testFindConsecutiveScaleA.
    #[test]
    fn runs_are_found_as_music21_finds_them() {
        use PitchAttribute::{Name, NameWithOctave};
        let scale = ["a4", "b4", "c#4", "d4", "e4", "f#4", "g#4", "a4"];
        assert_eq!(runs(&scale, 4, Name).len(), 1);
        assert_eq!(runs(&scale, 4, Name)[0].0, 8);
        assert_eq!(runs(&scale, 4, NameWithOctave)[0].0, 6);
        let twice = ["a4", "b4", "c#5", "d5", "e5", "a4", "b4", "c#5", "d5", "e5"];
        assert_eq!(
            runs(&twice, 4, NameWithOctave)
                .iter()
                .map(|r| r.0)
                .collect::<Vec<_>>(),
            [5, 5]
        );
        let shifted = [
            "a4", "b8", "c#3", "d3", "e4", "a4", "b9", "c#2", "d4", "e12",
        ];
        assert_eq!(
            runs(&shifted, 4, Name)
                .iter()
                .map(|r| r.0)
                .collect::<Vec<_>>(),
            [5, 5]
        );
        let thrice = [
            "a4", "b4", "c#5", "d-3", "a4", "b4", "c#5", "d-3", "a4", "b4", "c#5", "d-3",
        ];
        assert!(runs(&thrice, 4, NameWithOctave).is_empty());
        assert_eq!(
            runs(&thrice, 3, NameWithOctave)
                .iter()
                .map(|r| r.0)
                .collect::<Vec<_>>(),
            [3, 3, 3]
        );

        let up = ["c#5", "d3", "e4", "f#4", "g#4"];
        let down = ["g#4", "f#4", "e4", "d3", "c#5"];
        let turning: Vec<&str> = [up, down, up, down, down].concat();
        assert!(runs(&turning, 5, NameWithOctave).is_empty());
        let found = runs(&turning, 5, Name);
        assert_eq!(
            found.iter().map(|r| r.1.as_str()).collect::<Vec<_>>(),
            ["C#5", "G#4", "C#5", "G#4", "G#4"]
        );
        assert!(found.iter().all(|run| run.0 == 5));

        let mingled: Vec<&str> = [
            &up[..],
            &down,
            &["g2", "e#7"],
            &up,
            &["a-2"],
            &down,
            &["a", "b"],
            &down,
        ]
        .concat();
        assert!(runs(&mingled, 5, NameWithOctave).is_empty());
        let found = runs(&mingled, 5, Name);
        assert_eq!(
            found.iter().map(|r| r.1.as_str()).collect::<Vec<_>>(),
            ["C#5", "G#4", "C#5", "G#4", "G#4"]
        );
    }
}
