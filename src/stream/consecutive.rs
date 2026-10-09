//! The notes that follow one another in a stream, and the intervals between
//! them: music21's `Stream.findConsecutiveNotes` and `melodicIntervals`.

use super::{Stream, StreamElement};
use crate::defaults::FloatType;
use crate::error::Result;
use crate::interval::Interval;
use crate::makenotation::op_frac;
use crate::pitch::Pitch;

/// Which notes [`Stream::find_consecutive_notes`] passes over, and where it
/// marks a break: music21's keyword arguments to `findConsecutiveNotes`.
/// Every option is off by default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConsecutiveOptions {
    /// Whether a rest is passed over rather than breaking the line:
    /// `skipRests`.
    pub skip_rests: bool,
    /// Whether a chord breaks the line rather than joining it:
    /// `skipChords`.
    pub skip_chords: bool,
    /// Whether a note sounding the pitch of the note before, or a chord
    /// sounding the pitches of the chord before, is passed over:
    /// `skipUnisons`. Pitches are compared as sounded, so `F#` and `G-` are
    /// one. What is passed over still sounds until it ends, so no break
    /// follows it.
    pub skip_unisons: bool,
    /// Whether a note an octave or more from the note before, in the same
    /// pitch class, is passed over too: `skipOctaves`, which implies
    /// `skip_unisons`.
    pub skip_octaves: bool,
    /// Whether time with nothing in it is passed over rather than breaking
    /// the line: `skipGaps`.
    pub skip_gaps: bool,
    /// Whether a note starting before the one before it ends is kept:
    /// `getOverlaps`.
    pub get_overlaps: bool,
    /// Whether the list leaves out its breaks: `noNone`.
    pub no_none: bool,
}

/// The interval from one note of a line to the next, as
/// [`Stream::melodic_intervals`] finds it.
#[derive(Clone, Debug, PartialEq)]
pub struct MelodicInterval {
    interval: Interval,
    start: usize,
    end: usize,
    offset: FloatType,
    quarter_length: FloatType,
}

impl MelodicInterval {
    /// The interval, from the first note to the second; to a chord's first
    /// note where either is a chord.
    pub fn interval(&self) -> &Interval {
        &self.interval
    }

    /// The position in [`Stream::leaves`] of the note the interval leaves.
    pub fn start(&self) -> usize {
        self.start
    }

    /// The position in [`Stream::leaves`] of the note the interval reaches.
    pub fn end(&self) -> usize {
        self.end
    }

    /// Where the interval begins, counted from the stream's start: where
    /// the note it leaves ends.
    pub fn offset(&self) -> FloatType {
        self.offset
    }

    /// How long the interval lasts: from where the first note ends to where
    /// the second starts, nothing for notes that touch.
    pub fn quarter_length(&self) -> FloatType {
        self.quarter_length
    }
}

/// One stream in the walk, as music21's `recurse(streamsOnly=True,
/// includeSelf=True)` hands it over.
struct Container {
    /// Where it stands in the stream holding it.
    offset: FloatType,
    /// How far its own contents reach.
    highest_time: FloatType,
    /// The elements it holds itself that are notes, rests and the like, each
    /// with its offset in it and its position among the leaves.
    notes: Vec<(FloatType, usize)>,
}

fn containers(stream: &Stream, offset: FloatType, position: &mut usize, out: &mut Vec<Container>) {
    let index = out.len();
    out.push(Container {
        offset,
        highest_time: stream.end_offset(),
        notes: Vec::new(),
    });
    for event in &stream.events {
        match &event.element {
            StreamElement::Stream(inner) => containers(inner, event.offset, position, out),
            element => {
                if is_general_note(element) {
                    out[index].notes.push((event.offset, *position));
                }
                *position += 1;
            }
        }
    }
}

/// music21's `GeneralNote`: anything written that sounds or rests.
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

/// The pitches a chord sounds, for a chord or a chord symbol.
fn chord_pitches(element: &StreamElement) -> Result<Option<Vec<Pitch>>> {
    Ok(match element {
        StreamElement::Chord(chord) => Some(chord.pitches()),
        StreamElement::ChordSymbol(symbol) => Some(symbol.pitches()?),
        _ => None,
    })
}

impl Stream {
    /// The notes and chords that follow one another in this stream, as
    /// positions in [`Stream::leaves`], with `None` wherever the line breaks:
    /// music21's `findConsecutiveNotes`.
    ///
    /// Each stream is read on its own, its contents in order, and the line
    /// breaks at a rest, at time with nothing in it, at a chord when chords
    /// are skipped, and where one stream starts before the one read before
    /// it ended -- the second voice of a measure, the second part of a
    /// score. A note starting before the one before it ends is left out.
    /// `options` changes each of those. A chord of one note is passed over,
    /// as are unpitched strokes; a chord symbol counts as a chord. The list
    /// never ends on a break.
    ///
    /// # Errors
    ///
    /// A chord symbol whose notes cannot be worked out.
    pub fn find_consecutive_notes(
        &self,
        options: &ConsecutiveOptions,
    ) -> Result<Vec<Option<usize>>> {
        let leaves = self.leaves();
        let mut walk = Vec::new();
        containers(self, 0.0, &mut 0, &mut walk);

        let skip_unisons = options.skip_unisons || options.skip_octaves;
        let mut found: Vec<Option<usize>> = Vec::new();
        let mut last_container_end: FloatType = 0.0;
        let mut last_was_none = false;
        let mut last_pitches: Vec<Pitch> = Vec::new();

        for container in &walk {
            let holds_notes = !container.notes.is_empty();
            if container.offset < last_container_end && holds_notes && !options.no_none {
                found.push(None);
                last_was_none = true;
                last_pitches.clear();
            }
            let mut last_end: FloatType = 0.0;
            if holds_notes {
                last_container_end = container.highest_time;
            }
            for &(offset, position) in &container.notes {
                let element = leaves[position].1;
                if !last_was_none && !options.skip_gaps && offset > last_end && !options.no_none {
                    found.push(None);
                    last_was_none = true;
                }
                let length = element.quarter_length();
                if let StreamElement::Note(note) = element {
                    let pitch = note.pitch();
                    // A unison is judged against a single note only. One
                    // passed over still sounds until it ends.
                    if skip_unisons
                        && last_pitches.len() == 1
                        && pitch.pitch_class() == last_pitches[0].pitch_class()
                        && (options.skip_octaves || pitch.ps() == last_pitches[0].ps())
                    {
                        last_end = last_end.max(op_frac(offset + length));
                        continue;
                    }
                    if !options.get_overlaps && offset < last_end {
                        continue;
                    }
                    found.push(Some(position));
                    if offset < last_end {
                        continue;
                    }
                    last_end = op_frac(offset + length);
                    last_was_none = false;
                    last_pitches = vec![pitch.clone()];
                } else if let Some(pitches) = chord_pitches(element)? {
                    if pitches.len() <= 1 {
                        continue;
                    }
                    if options.skip_chords {
                        if !last_was_none && !options.no_none {
                            found.push(None);
                            last_was_none = true;
                            last_pitches.clear();
                        }
                    } else if skip_unisons && same_sounds(&pitches, &last_pitches) {
                        // A repeated chord is passed over, but still sounds
                        // until it ends.
                        last_end = last_end.max(op_frac(offset + length));
                    } else if options.get_overlaps || offset >= last_end {
                        found.push(Some(position));
                        if offset < last_end {
                            continue;
                        }
                        last_end = op_frac(offset + length);
                        last_pitches = pitches;
                        last_was_none = false;
                    }
                } else if let StreamElement::Rest(_) = element {
                    if options.skip_rests {
                        last_end = op_frac(offset + length);
                    } else if !last_was_none && !options.no_none {
                        found.push(None);
                        last_was_none = true;
                        last_pitches.clear();
                    }
                }
            }
        }
        if last_was_none {
            found.pop();
        }
        Ok(found)
    }

    /// The interval between each two notes [`Stream::find_consecutive_notes`]
    /// finds next to each other, with no break between them: music21's
    /// `melodicIntervals`. An interval to or from a chord is taken from
    /// the chord's first note.
    ///
    /// ```
    /// use music21_rs::stream::ConsecutiveOptions;
    /// use music21_rs::tinynotation::from_tiny_notation;
    ///
    /// let line = from_tiny_notation("3/4 c4 d' r b b'")?;
    /// let names: Vec<String> = line
    ///     .melodic_intervals(&ConsecutiveOptions::default())?
    ///     .iter()
    ///     .map(|step| step.interval().short_name())
    ///     .collect();
    /// assert_eq!(names, ["M9", "P8"]);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// As [`Stream::find_consecutive_notes`], or two notes no interval can
    /// be spelled between.
    pub fn melodic_intervals(&self, options: &ConsecutiveOptions) -> Result<Vec<MelodicInterval>> {
        let found = self.find_consecutive_notes(options)?;
        let leaves = self.leaves();
        let mut intervals = Vec::new();
        for pair in found.windows(2) {
            let (Some(start), Some(end)) = (pair[0], pair[1]) else {
                continue;
            };
            let (Some(from), Some(to)) =
                (first_pitch(leaves[start].1)?, first_pitch(leaves[end].1)?)
            else {
                continue;
            };
            let offset = op_frac(leaves[start].0 + leaves[start].1.quarter_length());
            intervals.push(MelodicInterval {
                interval: Interval::between_pitches(&from, &to)?,
                start,
                end,
                offset,
                quarter_length: op_frac(leaves[end].0 - offset),
            });
        }
        Ok(intervals)
    }
}

/// Whether two chords sound the same pitches, in any order.
fn same_sounds(pitches: &[Pitch], other: &[Pitch]) -> bool {
    let sorted = |pitches: &[Pitch]| {
        let mut sounds: Vec<FloatType> = pitches.iter().map(Pitch::ps).collect();
        sounds.sort_by(FloatType::total_cmp);
        sounds
    };
    pitches.len() == other.len() && sorted(pitches) == sorted(other)
}

/// A note's pitch, or a chord's first note's.
fn first_pitch(element: &StreamElement) -> Result<Option<Pitch>> {
    Ok(match element {
        StreamElement::Note(note) => Some(note.pitch().clone()),
        element => chord_pitches(element)?.and_then(|pitches| pitches.into_iter().next()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tinynotation::from_tiny_notation;

    #[test]
    fn a_skipped_unison_does_not_break_the_line() {
        let line = from_tiny_notation("4/4 c4 c4 d4 e4").unwrap();
        let options = ConsecutiveOptions {
            skip_unisons: true,
            ..ConsecutiveOptions::default()
        };
        let found = line.find_consecutive_notes(&options).unwrap();
        assert_eq!(found.len(), 3);
        assert!(found.iter().all(Option::is_some));
    }
}
