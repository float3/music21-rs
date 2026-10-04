//! Tied notes folded into one: music21's `stripTies`.

use super::{Stream, StreamElement, StreamKind};
use crate::defaults::FloatType;
use crate::error::{Error, Result};
use crate::notation::{Tie, TieType};
use crate::pitch::Pitch;

impl Stream {
    /// Folds every run of tied notes into its first note, lengthened by the
    /// rest of the run, which are taken out: music21's `stripTies`.
    ///
    /// A stream holding parts has each of them done, and one holding voices
    /// each voice. Otherwise every note, chord and rest is taken in one line
    /// in the order the stream flattens to, measures and voices alike, and a
    /// run of them is joined by its ties. With `match_by_pitch` a note joins
    /// the run before it only by sounding the pitch the last of it does --
    /// a chord, every pitch on the same letter and enharmonic -- whatever its
    /// tie says; without, a chord joins one whose notes are all tied on and
    /// as many. The note kept is given the run's length as a plain value and
    /// loses its tie; a spanner joined to a note taken out is joined to the
    /// note kept instead. Notes of no length, grace notes among them, are
    /// left as they are.
    ///
    /// ```
    /// use music21_rs::notation::{Tie, TieType};
    /// use music21_rs::{Duration, Note, Stream, StreamElement};
    ///
    /// let mut stream = Stream::new();
    /// let mut first = Note::from_name("C4")?.with_duration(Duration::half());
    /// first.set_tie(Some(Tie::new(TieType::Start)));
    /// let mut second = Note::from_name("C4")?;
    /// second.set_tie(Some(Tie::new(TieType::Stop)));
    /// stream.push(first);
    /// stream.push(second);
    /// stream.strip_ties(true)?;
    ///
    /// let leaves = stream.leaves();
    /// assert_eq!(leaves.len(), 1);
    /// assert_eq!(leaves[0].1.quarter_length(), 3.0);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// A run whose notes after the first last no time at all, as music21
    /// raises for one.
    pub fn strip_ties(&mut self, match_by_pitch: bool) -> Result<()> {
        if self.has_part_like_streams() {
            for event in &mut self.events {
                if let StreamElement::Stream(inner) = &mut event.element {
                    inner.strip_ties(match_by_pitch)?;
                }
            }
            return Ok(());
        }
        let has_voices = self.events.iter().any(|event| {
            matches!(&event.element, StreamElement::Stream(inner) if inner.kind == StreamKind::Voice)
        });
        if has_voices {
            for event in &mut self.events {
                if let StreamElement::Stream(inner) = &mut event.element
                    && inner.kind == StreamKind::Voice
                {
                    inner.strip_ties(match_by_pitch)?;
                }
            }
            return Ok(());
        }

        let leaves = self.leaves();
        // The leaves in the order music21's `flatten` gives them.
        let mut order: Vec<usize> = (0..leaves.len()).collect();
        order.sort_by(|left, right| {
            let (left_offset, left_element) = leaves[*left];
            let (right_offset, right_element) = leaves[*right];
            let grace = |element: &StreamElement| {
                element
                    .duration()
                    .is_some_and(crate::duration::Duration::is_grace)
            };
            left_offset
                .total_cmp(&right_offset)
                .then(
                    left_element
                        .class_sort_order()
                        .cmp(&right_element.class_sort_order()),
                )
                .then(grace(right_element).cmp(&grace(left_element)))
        });
        let line: Vec<(usize, Tied)> = order
            .into_iter()
            .filter_map(|position| {
                let element = leaves[position].1;
                Tied::of(element)
                    .filter(|_| element.quarter_length() > 0.0)
                    .map(|tied| (position, tied))
            })
            .collect();

        let mut merged: Vec<(usize, FloatType)> = Vec::new();
        let mut taken: Vec<(usize, usize)> = Vec::new();
        let mut connected: Vec<usize> = Vec::new();
        for index in 0..line.len() {
            let tied = &line[index].1;
            let last = index.checked_sub(1);
            let ends = |connected: &[usize]| {
                ends_run(
                    tied,
                    last.map(|last| &line[last].1),
                    last,
                    connected,
                    match_by_pitch,
                )
            };
            let mut end_match = None;
            match tied.tie {
                Some(TieType::Start) => {
                    if last.is_none_or(|last| !connected.contains(&last)) {
                        connected = vec![index];
                    } else {
                        connected.push(index);
                    }
                    end_match = Some(false);
                }
                Some(TieType::Continue) => {
                    if connected.is_empty() {
                        connected.push(index);
                    } else if match_by_pitch {
                        if ends(&connected) {
                            connected.push(index);
                        } else {
                            connected = vec![index];
                        }
                    } else if tied.all_continue {
                        let before = last.map(|last| &line[last].1);
                        if before.is_some_and(|before| {
                            before.sounds && before.pitches.len() != tied.pitches.len()
                        }) {
                            connected = vec![index];
                        } else {
                            connected.push(index);
                        }
                    } else {
                        connected.clear();
                    }
                    end_match = Some(false);
                }
                _ => {}
            }
            let end_match = end_match.unwrap_or_else(|| ends(&connected));
            if !end_match {
                continue;
            }
            connected.push(index);
            if connected.len() < 2 {
                connected.clear();
                continue;
            }
            let first = connected[0];
            let mut sum = 0.0;
            for &later in &connected[1..] {
                sum += line[later].1.quarter_length;
                taken.push((line[later].0, line[first].0));
            }
            if sum == 0.0 {
                return Err(Error::Stream(
                    "aggregated ties have a zero duration sum".to_string(),
                ));
            }
            merged.push((
                line[first].0,
                crate::makenotation::op_frac(line[first].1.quarter_length + sum),
            ));
            connected.clear();
        }
        if merged.is_empty() {
            return Ok(());
        }

        // A spanner holding a note taken out holds the note kept.
        let mut moved: Vec<Option<usize>> = (0..leaves.len()).map(Some).collect();
        for &(gone, kept) in &taken {
            moved[gone] = Some(kept);
        }
        self.move_spanners_everywhere(&moved);
        let gone: Vec<usize> = taken.iter().map(|(gone, _)| *gone).collect();
        let mut failure = None;
        self.retain_leaves(&mut |position, element| {
            if gone.contains(&position) {
                return false;
            }
            if let Some((_, length)) = merged.iter().find(|(kept, _)| *kept == position)
                && let Err(error) = lengthen(element, *length)
            {
                failure = Some(error);
            }
            true
        });
        failure.map_or(Ok(()), Err)
    }

    /// Moves the places of every spanner here and in every nested stream,
    /// each by the leaves of the stream holding it.
    fn move_spanners_everywhere(&mut self, moved: &[Option<usize>]) {
        let mut first = 0;
        self.move_spanners_from(&mut first, moved);
    }

    fn move_spanners_from(&mut self, first: &mut usize, moved: &[Option<usize>]) {
        let start = *first;
        let count = self.leaves().len();
        let mine: Vec<Option<usize>> = (0..count)
            .map(|place| {
                moved
                    .get(start + place)
                    .copied()
                    .flatten()
                    .and_then(|to| to.checked_sub(start))
                    .filter(|to| *to < count)
            })
            .collect();
        for spanner in &mut self.labels.spanners {
            spanner.move_places(&mine);
            spanner.drop_repeated_places();
        }
        for event in &mut self.events {
            match &mut event.element {
                StreamElement::Stream(inner) => inner.move_spanners_from(first, moved),
                _ => *first += 1,
            }
        }
    }
}

/// What `stripTies` asks of a note, chord or rest.
struct Tied {
    tie: Option<TieType>,
    /// Whether it sounds: music21's `NotRest`.
    sounds: bool,
    /// A note's one pitch, for a note alone.
    pitch: Option<Pitch>,
    /// The pitches of a chord, or of anything else that sounds.
    pitches: Vec<Pitch>,
    is_chord: bool,
    /// Whether it is tied on, and for a chord every note of it.
    all_continue: bool,
    /// Whether every note of a chord ends a tie.
    all_stop: bool,
    quarter_length: FloatType,
}

impl Tied {
    fn of(element: &StreamElement) -> Option<Self> {
        let tie_type = |tie: Option<&Tie>| tie.map(Tie::tie_type);
        let quarter_length = element.quarter_length();
        Some(match element {
            StreamElement::Note(note) => Self {
                tie: tie_type(note.tie()),
                sounds: true,
                pitch: Some(note.pitch().clone()),
                pitches: vec![note.pitch().clone()],
                is_chord: false,
                all_continue: false,
                all_stop: false,
                quarter_length,
            },
            StreamElement::Chord(chord) => {
                let ties: Vec<Option<TieType>> = chord
                    .notes()
                    .iter()
                    .map(|note| tie_type(note.tie()))
                    .collect();
                Self {
                    tie: tie_type(chord.tie()),
                    sounds: true,
                    pitch: None,
                    pitches: chord.pitches(),
                    is_chord: true,
                    all_continue: ties.iter().all(|tie| *tie == Some(TieType::Continue)),
                    all_stop: ties.iter().all(|tie| *tie == Some(TieType::Stop)),
                    quarter_length,
                }
            }
            StreamElement::ChordSymbol(symbol) => Self {
                tie: None,
                sounds: true,
                pitch: None,
                pitches: symbol.pitches().unwrap_or_default(),
                is_chord: true,
                all_continue: false,
                all_stop: false,
                quarter_length,
            },
            StreamElement::Rest(rest) => Self {
                tie: tie_type(rest.tie()),
                sounds: false,
                pitch: None,
                pitches: Vec::new(),
                is_chord: false,
                all_continue: false,
                all_stop: false,
                quarter_length,
            },
            // A stroke has no pitch music21 compares; it has its written
            // note's tie.
            StreamElement::Unpitched(stroke) => Self {
                tie: tie_type(stroke.written().tie()),
                sounds: true,
                pitch: None,
                pitches: Vec::new(),
                is_chord: false,
                all_continue: false,
                all_stop: false,
                quarter_length,
            },
            StreamElement::PercussionChord(chord) => Self {
                tie: tie_type(chord.written().tie()),
                sounds: true,
                pitch: None,
                pitches: chord.pitches(),
                is_chord: false,
                all_continue: false,
                all_stop: false,
                quarter_length,
            },
            _ => return None,
        })
        .map(|mut tied| {
            // music21's `allTiesAreContinue`: the tie continues, and for a
            // chord every note's does.
            tied.all_continue =
                tied.tie == Some(TieType::Continue) && (!tied.is_chord || tied.all_continue);
            tied
        })
    }
}

/// music21's `updateEndMatch`: whether `tied` ends the run the notes in
/// `connected` make, `last` being the note before it.
fn ends_run(
    tied: &Tied,
    before: Option<&Tied>,
    last: Option<usize>,
    connected: &[usize],
    match_by_pitch: bool,
) -> bool {
    if !tied.is_chord && tied.tie == Some(TieType::Stop) {
        return true;
    }
    if !match_by_pitch {
        return tied.is_chord
            && before.is_some_and(|before| {
                before.is_chord && tied.all_stop && before.pitches.len() == tied.pitches.len()
            });
    }
    let Some(before) = before else {
        return false;
    };
    let joined = last.is_some_and(|last| connected.contains(&last));
    if !joined {
        return false;
    }
    if let (Some(previous), Some(pitch)) = (&before.pitch, &tied.pitch) {
        return previous == pitch;
    }
    // Anything but a note, against whatever came before it: every pitch on
    // the same letter and enharmonic, a rest having none.
    if tied.pitch.is_none() {
        return before.pitches.len() == tied.pitches.len()
            && before
                .pitches
                .iter()
                .zip(&tied.pitches)
                .all(|(previous, pitch)| {
                    previous.step() == pitch.step() && previous.is_enharmonic(pitch)
                });
    }
    false
}

/// Gives the note kept by a run its run's length, as a plain value, and
/// takes its tie off.
fn lengthen(element: &mut StreamElement, length: FloatType) -> Result<()> {
    let duration = crate::duration::Duration::new(length)?;
    match element {
        StreamElement::Note(note) => {
            note.set_duration(duration);
            note.set_tie(None);
        }
        StreamElement::Chord(chord) => {
            chord.set_duration(duration);
            chord.set_tie(None);
        }
        StreamElement::ChordSymbol(symbol) => symbol.set_duration(duration),
        StreamElement::Rest(rest) => {
            rest.set_duration(duration);
            rest.set_tie(None);
        }
        StreamElement::Unpitched(stroke) => {
            stroke.written_mut().set_duration(duration);
            stroke.written_mut().set_tie(None);
        }
        StreamElement::PercussionChord(chord) => {
            chord.written_mut().set_duration(duration);
            chord.written_mut().set_tie(None);
        }
        _ => {}
    }
    Ok(())
}
