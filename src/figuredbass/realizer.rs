//! Realizing a figured bass line: music21's `figuredBass.realizer`.
//!
//! A [`FiguredBassLine`] is a run of bass notes, each with its figures, in a
//! key and a meter. Realizing it makes a [`Segment`] of each note as the
//! line is written -- a note cut at a barline is two -- finds every pair of
//! voicings each segment may move to the next by, and drops the voicings
//! that lead nowhere, leaving a [`Realization`]: every way of voicing the
//! whole line that keeps the rules, which it can count, list, pick from,
//! and write out as a score.

use crate::chord::Chord;
use crate::clef::Clef;
use crate::defaults::FloatType;
use crate::duration::Duration;
use crate::error::{Error, Result};
use crate::figuredbass::Notation;
use crate::figuredbass::rules::Rules;
use crate::figuredbass::scale::{FiguredBassMode, FiguredBassScale};
use crate::figuredbass::segment::{Possibility, Segment};
use crate::key::{Key, KeySignature};
use crate::makenotation::{make_accidentals_by, make_beams, make_measures, make_ties, op_frac};
use crate::meter::TimeSignature;
use crate::note::Note;
use crate::pitch::Pitch;
use crate::stream::{Stream, StreamElement, StreamKind};

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

/// One note or rest of a part laid over the line: where it starts, how long
/// it lasts, and its pitch where it is a note.
#[derive(Clone, Debug, PartialEq)]
struct Sounding {
    offset: FloatType,
    quarter_length: FloatType,
    pitch: Option<Pitch>,
}

impl Sounding {
    fn end(&self) -> FloatType {
        op_frac(self.offset + self.quarter_length)
    }
}

/// One written note of the bass line: a bass note, or one of the pieces a
/// barline cut it into.
#[derive(Clone, Debug)]
struct Piece {
    offset: FloatType,
    note: Note,
    notation: Option<String>,
}

impl Piece {
    fn quarter_length(&self) -> FloatType {
        self.note.duration().map_or(1.0, Duration::quarter_length)
    }
}

/// What sounds in one part during a moment of the line.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Voice {
    /// A note of the part laid over the line of that number, by its place
    /// there.
    Upper(usize, usize),
    /// A piece of the bass line, by its place there.
    Bass(usize),
}

/// A stretch of the line through which every part holds one note: its
/// start, its end, and what each part sounds, highest first.
type Moment = ((FloatType, FloatType), Vec<Voice>);

/// A bass line and its figures, in a key and a meter.
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
    time: TimeSignature,
    elements: Vec<Element>,
    overlaid: Vec<Vec<Sounding>>,
}

impl Default for FiguredBassLine {
    /// A line in C major and common time, with nothing in it yet.
    fn default() -> Self {
        Self::new(FiguredBassScale::default())
    }
}

/// music21's meter for a line that names none.
fn common_time() -> TimeSignature {
    TimeSignature::new(4, 4).expect("four quarters make a bar")
}

impl FiguredBassLine {
    /// An empty line in common time whose figures are read in `scale`.
    pub fn new(scale: FiguredBassScale) -> Self {
        Self {
            scale,
            time: common_time(),
            elements: Vec::new(),
            overlaid: Vec::new(),
        }
    }

    /// An empty line in `key` and common time. The key must be in one of
    /// the five modes a figured bass is read in.
    ///
    /// # Errors
    ///
    /// A key in another mode.
    pub fn in_key(key: &Key) -> Result<Self> {
        let tonic = key.pitch_from_degree(1)?;
        let mode = FiguredBassMode::from_name(key.mode())?;
        Ok(Self::new(FiguredBassScale::new(tonic, mode)?))
    }

    /// An empty line in `key` and `time`: music21's
    /// `FiguredBassLine(inKey, inTime)`.
    ///
    /// # Errors
    ///
    /// A key in a mode no figured bass is read in.
    pub fn in_key_and_time(key: &Key, time: TimeSignature) -> Result<Self> {
        let mut line = Self::in_key(key)?;
        line.time = time;
        Ok(line)
    }

    /// The scale its figures are read in.
    pub fn scale(&self) -> &FiguredBassScale {
        &self.scale
    }

    /// The meter it is written in: music21's `inTime`.
    pub fn time_signature(&self) -> &TimeSignature {
        &self.time
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

    /// Lays a written part over the line, to be kept as it is by every
    /// realization: music21's `overlayPart`.
    ///
    /// Each note of the part holds one part of the voicing to its pitch
    /// while it sounds, the first part laid over being the highest. Where
    /// it moves and the bass does not, the line gains a segment of no
    /// length, into which only the parts laid over may move. As in music21,
    /// the first segment of the line is not held to the parts laid over it.
    ///
    /// Only the part's notes and rests are read, at their offsets from its
    /// start.
    pub fn overlay_part(&mut self, part: &Stream) {
        let flat = part.flatten();
        let mut sounding: Vec<Sounding> = flat
            .events()
            .iter()
            .filter_map(|event| {
                let pitch = match event.element() {
                    StreamElement::Note(note) => Some(note.pitch().clone()),
                    StreamElement::Chord(_)
                    | StreamElement::Rest(_)
                    | StreamElement::Unpitched(_)
                    | StreamElement::PercussionChord(_)
                    | StreamElement::ChordSymbol(_) => None,
                    _ => return None,
                };
                Some(Sounding {
                    offset: event.offset(),
                    quarter_length: event.element().quarter_length(),
                    pitch,
                })
            })
            .collect();
        sounding.sort_by(|left, right| left.offset.total_cmp(&right.offset));
        self.overlaid.push(sounding);
    }

    /// The bass line written out as a part: a bass clef, the key signature,
    /// the meter and the bass notes one after another, each with its
    /// figures under it as lyrics, cut into measures with the notes that
    /// run past a barline tied. music21's `generateBassLine`.
    ///
    /// Tuplets are not bracketed.
    ///
    /// ```
    /// use music21_rs::figuredbass::realizer::FiguredBassLine;
    /// use music21_rs::{Key, Pitch, TimeSignature};
    ///
    /// let key = Key::from_tonic("B")?;
    /// let mut line = FiguredBassLine::in_key_and_time(&key, TimeSignature::new(3, 4)?)?;
    /// line.add_element(Pitch::from_name("B2")?, 1.0, None);
    /// line.add_element(Pitch::from_name("C#3")?, 1.0, Some("6"));
    /// line.add_element(Pitch::from_name("D#3")?, 2.5, Some("6"));
    /// let written = line.generate_bass_line()?;
    /// assert_eq!(written.measures().len(), 2);
    /// assert_eq!(written.notes().len(), 4);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// A line holding chords rather than bass notes and figures, figures
    /// that cannot be read, or a length no duration has.
    pub fn generate_bass_line(&self) -> Result<Stream> {
        let mut line = Stream::with_kind(StreamKind::Part);
        line.push(Clef::bass());
        line.push(self.scale.key_signature().clone());
        line.push(self.time.clone());
        for element in &self.elements {
            let Element::Figured {
                bass,
                quarter_length,
                notation,
            } = element
            else {
                return Err(Error::FiguredBass(
                    "a bass line is written from bass notes and their figures, not from chords"
                        .to_string(),
                ));
            };
            line.push(bass_note(bass, *quarter_length, notation.as_deref())?);
        }
        notated(&line)
    }

    /// The bass line as its written notes, each with the figures of the
    /// bass note it is, or is a piece of.
    fn pieces(&self) -> Result<Vec<Piece>> {
        // Where each bass note starts and stops.
        let mut spans: Vec<(FloatType, FloatType, Option<&str>)> = Vec::new();
        let mut start = 0.0;
        for element in &self.elements {
            if let Element::Figured {
                quarter_length,
                notation,
                ..
            } = element
            {
                let stop = op_frac(start + quarter_length);
                spans.push((start, stop, notation.as_deref()));
                start = stop;
            }
        }
        let written = self.generate_bass_line()?.flatten();
        Ok(written
            .events()
            .iter()
            .filter_map(|event| match event.element() {
                StreamElement::Note(note) => {
                    let offset = event.offset();
                    let notation = spans
                        .iter()
                        .find(|(start, stop, _)| *start <= offset + 1e-9 && offset < stop - 1e-9)
                        .or(spans.last())
                        .and_then(|(_, _, notation)| *notation);
                    Some(Piece {
                        offset,
                        note: note.clone(),
                        notation: notation.map(str::to_string),
                    })
                }
                _ => None,
            })
            .collect())
    }

    /// The moments of the line and the parts laid over it, in order:
    /// music21's `checker.extractHarmonies` over the parts and the bass,
    /// or its `createOffsetMapping` of the bass where no part is laid over.
    fn moments(&self, pieces: &[Piece]) -> Vec<Moment> {
        let bass: Vec<(Sounding, Voice)> = pieces
            .iter()
            .enumerate()
            .map(|(index, piece)| {
                let sounding = Sounding {
                    offset: piece.offset,
                    quarter_length: piece.quarter_length(),
                    pitch: None,
                };
                (sounding, Voice::Bass(index))
            })
            .collect();
        let mut parts: Vec<Vec<(Sounding, Voice)>> = self
            .overlaid
            .iter()
            .enumerate()
            .map(|(part, sounding)| {
                sounding
                    .iter()
                    .enumerate()
                    .map(|(index, each)| (each.clone(), Voice::Upper(part, index)))
                    .collect()
            })
            .collect();
        parts.push(bass);

        // The first part's notes, those starting and ending together sharing
        // a moment.
        let mut moments: Vec<Moment> = Vec::new();
        for (sounding, voice) in &parts[0] {
            let key = (sounding.offset, sounding.end());
            match moments.iter_mut().find(|(held, _)| *held == key) {
                Some((_, voices)) => voices.push(*voice),
                None => moments.push((key, vec![*voice])),
            }
        }
        // Each part after it cuts the moments where its own notes change.
        for part in &parts[1..] {
            moments.sort_by(|left, right| compare_spans(left.0, right.0));
            let mut cut: Vec<Moment> = Vec::new();
            for ((start, end), voices) in &moments {
                for (sounding, voice) in part {
                    if !overlaps(sounding, *start, *end) {
                        continue;
                    }
                    let begins = if sounding.offset < *start {
                        *start
                    } else {
                        sounding.offset
                    };
                    let ends = if sounding.offset + sounding.quarter_length > *end {
                        *end
                    } else {
                        sounding.end()
                    };
                    let mut together = voices.clone();
                    together.push(*voice);
                    match cut.iter_mut().find(|(held, _)| *held == (begins, ends)) {
                        Some((_, held)) => *held = together,
                        None => cut.push(((begins, ends), together)),
                    }
                }
            }
            moments = cut;
        }
        moments.sort_by(|left, right| compare_spans(left.0, right.0));
        moments
    }

    /// A segment for each bass note as the line is written, voiced in
    /// `num_parts` parts under `rules`, no part above `max_pitch`: music21's
    /// `retrieveSegments`.
    ///
    /// A bass note cut by a barline is a segment for each piece, and a part
    /// laid over the line holds its part of each segment to its pitch. A
    /// line of chords is a segment for each chord.
    ///
    /// # Errors
    ///
    /// Figures the line's scale cannot read, or a part laid over the line
    /// that sounds something other than a note where a segment needs its
    /// pitch.
    pub fn segments(
        &self,
        rules: &Rules,
        num_parts: usize,
        max_pitch: &Pitch,
    ) -> Result<Vec<Segment>> {
        Ok(self
            .written_segments(rules, num_parts, max_pitch)?
            .into_iter()
            .map(|(segment, _)| segment)
            .collect())
    }

    /// [`FiguredBassLine::segments`], each with the bass note written under
    /// it.
    fn written_segments(
        &self,
        rules: &Rules,
        num_parts: usize,
        max_pitch: &Pitch,
    ) -> Result<Vec<(Segment, Note)>> {
        if self.elements.is_empty() {
            return Ok(Vec::new());
        }
        // music21 reads a line holding any chord as a line of chords, one
        // segment each, with no bass line written.
        if self
            .elements
            .iter()
            .any(|element| matches!(element, Element::Named { .. }))
        {
            return self
                .elements
                .iter()
                .map(|element| match element {
                    Element::Figured {
                        bass,
                        quarter_length,
                        notation,
                    } => Ok((
                        Segment::new(
                            bass.clone(),
                            *quarter_length,
                            &Notation::parse(notation.as_deref().unwrap_or(""))?,
                            &self.scale,
                            rules.clone(),
                            num_parts,
                            max_pitch.clone(),
                        )?
                        .overlaid(),
                        bass_note(bass, *quarter_length, notation.as_deref())?,
                    )),
                    Element::Named {
                        bass,
                        quarter_length,
                        pitch_names,
                    } => Ok((
                        Segment::from_pitch_names(
                            bass.clone(),
                            *quarter_length,
                            pitch_names.clone(),
                            rules.clone(),
                            num_parts,
                            max_pitch.clone(),
                        )?,
                        Note::from_pitch(Pitch::from_name(bass.name_with_octave())?)
                            .with_duration(Duration::new(*quarter_length)?),
                    )),
                })
                .collect();
        }

        let pieces = self.pieces()?;
        let moments = self.moments(&pieces);
        let segment_of = |piece: &Piece, quarter_length: FloatType| -> Result<Segment> {
            Ok(Segment::new(
                piece.note.pitch().clone(),
                quarter_length,
                &Notation::parse(piece.notation.as_deref().unwrap_or(""))?,
                &self.scale,
                rules.clone(),
                num_parts,
                max_pitch.clone(),
            )?
            .overlaid())
        };
        let bass_of = |voices: &[Voice]| -> Result<usize> {
            match voices.last() {
                Some(Voice::Bass(index)) => Ok(*index),
                _ => Err(Error::FiguredBass(
                    "a moment of the line has no bass note".to_string(),
                )),
            }
        };

        let mut made: Vec<(Segment, Note)> = Vec::new();
        let Some((_, first)) = moments.first() else {
            return Ok(made);
        };
        let first = &pieces[bass_of(first)?];
        made.push((
            segment_of(first, pieces[0].quarter_length())?,
            first.note.clone(),
        ));
        // The last piece of the bass a segment has taken its length from.
        let mut reached = 0;
        for ((start, _), voices) in &moments[1..] {
            let piece = &pieces[bass_of(voices)?];
            let before = &pieces[reached];
            let quarter_length = if *start == op_frac(before.offset + before.quarter_length()) {
                reached += 1;
                let Some(next) = pieces.get(reached) else {
                    return Err(Error::FiguredBass(
                        "the parts laid over the line run past its last bass note".to_string(),
                    ));
                };
                next.quarter_length()
            } else {
                // The bass holds while a part laid over it moves: the parts
                // not laid over stay where they were.
                if let Some((previous, _)) = made.last_mut() {
                    previous
                        .rules
                        .parts_to_check
                        .extend(voices.len()..=num_parts);
                }
                0.0
            };
            let mut segment = segment_of(piece, quarter_length)?;
            for (number, voice) in voices[..voices.len() - 1].iter().enumerate() {
                let Voice::Upper(part, index) = voice else {
                    continue;
                };
                let Some(pitch) = self.overlaid[*part][*index].pitch.clone() else {
                    return Err(Error::FiguredBass(
                        "only a note of a part laid over the line can hold a part to its pitch"
                            .to_string(),
                    ));
                };
                segment.rules.part_pitch_limits.push((number + 1, pitch));
            }
            made.push((segment, piece.note.clone()));
        }
        Ok(made)
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
        let (segments, bass_notes) = self
            .written_segments(rules, num_parts, max_pitch)?
            .into_iter()
            .unzip();
        let mut realization = Realization::of(segments)?;
        realization.key_signature = self.scale.key_signature().clone();
        realization.time = self.time.clone();
        realization.bass_notes = bass_notes;
        Ok(realization)
    }
}

/// Which of two spans comes first, by its start and then by its end.
fn compare_spans(
    left: (FloatType, FloatType),
    right: (FloatType, FloatType),
) -> std::cmp::Ordering {
    left.0.total_cmp(&right.0).then(left.1.total_cmp(&right.1))
}

/// Whether a note sounds during a span: music21's `getElementsByOffset`
/// taking in what begins before the span and leaving out what only touches
/// either end of it.
fn overlaps(sounding: &Sounding, start: FloatType, end: FloatType) -> bool {
    let stop = sounding.end();
    if sounding.offset > end || stop < start {
        return false;
    }
    if start == end && sounding.quarter_length == 0.0 {
        return true;
    }
    sounding.offset != end && stop != start
}

/// A bass note with its figures written under it as lyrics, one a line,
/// padded to one width: music21's `addLyricsToBassNote`.
fn bass_note(bass: &Pitch, quarter_length: FloatType, notation: Option<&str>) -> Result<Note> {
    let mut note = Note::from_pitch(bass.clone()).with_duration(Duration::new(quarter_length)?);
    let notation = Notation::parse(notation.unwrap_or(""))?;
    let figures = notation.figure_strings();
    let width = figures
        .iter()
        .map(|figure| figure.chars().count())
        .max()
        .unwrap_or(0);
    for figure in figures {
        note.add_lyric(&format!("{figure:>width$}"), None, true)?;
    }
    Ok(note)
}

/// A part cut into measures and written out: music21's `makeNotation` with
/// `cautionaryNotImmediateRepeat=False`, which is how the realizer writes
/// every part it makes.
fn notated(part: &Stream) -> Result<Stream> {
    let mut made = make_measures(part)?;
    make_accidentals_by(&mut made, false);
    make_ties(&mut made)?;
    // music21 warns and carries on where a meter cannot beam its bar.
    let _ = make_beams(&mut made);
    Ok(made)
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
    /// The key signature and the meter a realization is written in.
    key_signature: KeySignature,
    time: TimeSignature,
    /// The bass note written under each segment.
    bass_notes: Vec<Note>,
    keyboard_style: bool,
}

impl Realization {
    /// The realization of `segments`, a line's segments in order. Written
    /// out, it is in C major and common time, each segment's bass lasting
    /// as long as the segment; a realization made by
    /// [`FiguredBassLine::realize`] is written as its line is.
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
        let bass_notes = segments
            .iter()
            .map(|segment| {
                Ok(Note::from_pitch(segment.bass().clone())
                    .with_duration(Duration::new(segment.quarter_length())?))
            })
            .collect::<Result<Vec<Note>>>()?;
        Ok(Self {
            segments,
            movements,
            single,
            fallbacks,
            key_signature: KeySignature::new(0),
            time: common_time(),
            bass_notes,
            keyboard_style: true,
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

    /// Whether realizations are written in keyboard style, the upper parts
    /// as chords on one staff over the bass, as they are to begin with, or
    /// in chorale style with a staff for each part: music21's
    /// `keyboardStyleOutput`.
    pub fn keyboard_style_output(&self) -> bool {
        self.keyboard_style
    }

    /// Says whether realizations are written in keyboard style or in
    /// chorale style.
    pub fn set_keyboard_style_output(&mut self, keyboard_style: bool) {
        self.keyboard_style = keyboard_style;
    }

    /// One way of voicing the line written out as a score, a voicing for
    /// each segment: music21's `generateRealizationFromPossibilityProgression`.
    ///
    /// In keyboard style the score is two parts, the upper parts of each
    /// voicing as a chord under a treble clef and the bass under a bass
    /// clef; in chorale style each part has a staff of its own, under the
    /// clef that fits it best. Every part opens with the key signature and
    /// the meter of the line, is cut into measures, and has the notes that
    /// run past a barline tied. Tuplets are not bracketed.
    ///
    /// ```
    /// use music21_rs::Pitch;
    /// use music21_rs::figuredbass::realizer::FiguredBassLine;
    /// use music21_rs::figuredbass::rules::Rules;
    ///
    /// let mut line = FiguredBassLine::default();
    /// line.add_element(Pitch::from_name("C3")?, 2.0, None);
    /// line.add_element(Pitch::from_name("G2")?, 2.0, Some("7"));
    /// line.add_element(Pitch::from_name("C3")?, 4.0, None);
    /// let realization = line.realize(&Rules::default(), 4, &Pitch::from_name("B5")?)?;
    /// let voicings = realization.all_possibility_progressions();
    /// let score = realization.generate_realization_from_possibility_progression(&voicings[0])?;
    /// assert_eq!(score.parts().len(), 2);
    /// assert_eq!(score.parts()[0].measures().len(), 2);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// A progression with fewer voicings than the line has segments, or a
    /// voicing that is no chord.
    pub fn generate_realization_from_possibility_progression(
        &self,
        progression: &[Possibility],
    ) -> Result<Stream> {
        if progression.len() < self.segments.len() {
            return Err(Error::FiguredBass(format!(
                "a progression of {} voicings does not voice a line of {} segments",
                progression.len(),
                self.segments.len()
            )));
        }
        let opening = |part: &mut Stream| {
            part.push(self.key_signature.clone());
            part.push(self.time.clone());
        };
        let mut score = Stream::with_kind(StreamKind::Score);

        let mut bass = Stream::with_kind(StreamKind::Part);
        opening(&mut bass);
        for note in &self.bass_notes {
            bass.push(note.clone());
        }
        bass.insert(0.0, Clef::bass());

        if self.keyboard_style {
            let mut right = Stream::with_kind(StreamKind::Part);
            opening(&mut right);
            for (segment, voicing) in self.segments.iter().zip(progression) {
                let upper = &voicing[..voicing.len().saturating_sub(1)];
                let chord =
                    Chord::new(upper)?.with_duration(Duration::new(segment.quarter_length())?);
                right.push(chord);
            }
            right.insert(0.0, Clef::treble());
            score.insert(0.0, notated(&right)?);
        } else {
            let count = progression
                .first()
                .map_or(0, |voicing| voicing.len().saturating_sub(1));
            for number in 0..count {
                let mut part = Stream::with_kind(StreamKind::Part);
                opening(&mut part);
                for (segment, voicing) in self.segments.iter().zip(progression) {
                    let Some(pitch) = voicing.get(number) else {
                        return Err(Error::FiguredBass(format!(
                            "a voicing of {} parts has no part {}",
                            voicing.len(),
                            number + 1
                        )));
                    };
                    part.push(
                        Note::from_pitch(pitch.clone())
                            .with_duration(Duration::new(segment.quarter_length())?),
                    );
                }
                let clef = Clef::best_for(&part.pitches(), true);
                part.insert(0.0, clef);
                score.insert(0.0, notated(&part)?);
            }
        }

        score.insert(0.0, notated(&bass)?);
        Ok(score)
    }

    /// Every way of voicing the line written out one after another in one
    /// score: music21's `generateAllRealizations`. Each realization keeps
    /// its own measure numbers and its closing barline.
    ///
    /// # Errors
    ///
    /// A line with no way of voicing it, or a realization that cannot be
    /// written.
    pub fn generate_all_realizations(&self) -> Result<Stream> {
        let progressions = self.all_possibility_progressions();
        if progressions.is_empty() {
            return Err(Error::FiguredBass("Zero solutions".to_string()));
        }
        let mut written = progressions
            .iter()
            .map(|progression| self.generate_realization_from_possibility_progression(progression));
        let first = written
            .next()
            .ok_or_else(|| Error::FiguredBass("Zero solutions".to_string()))??;
        gathered(first, written)
    }

    /// One way of voicing the line written out as a score, each voicing
    /// chosen by `choose`, which is given how many there are to choose from
    /// and answers the index of one: music21's `generateRandomRealization`,
    /// with the choosing left to the caller.
    ///
    /// # Errors
    ///
    /// A line with no way of voicing it, or a choice out of range.
    pub fn generate_random_realization(
        &self,
        choose: impl FnMut(usize) -> usize,
    ) -> Result<Stream> {
        let progression = self.possibility_progression_by(choose)?;
        self.generate_realization_from_possibility_progression(&progression)
    }

    /// So many ways of voicing the line, each chosen by `choose`, written
    /// out one after another in one score: music21's
    /// `generateRandomRealizations`. Asked for more than there are, it
    /// answers every one of them in order, as [`generate_all_realizations`]
    /// does. One way may be chosen more than once.
    ///
    /// [`generate_all_realizations`]: Realization::generate_all_realizations
    ///
    /// # Errors
    ///
    /// A line with no way of voicing it, or a choice out of range.
    pub fn generate_random_realizations(
        &self,
        amount: usize,
        mut choose: impl FnMut(usize) -> usize,
    ) -> Result<Stream> {
        if amount > self.num_solutions() {
            return self.generate_all_realizations();
        }
        let first = self.generate_random_realization(&mut choose)?;
        let mut rest = Vec::new();
        for _ in 1..amount {
            rest.push(self.generate_random_realization(&mut choose));
        }
        gathered(first, rest.into_iter())
    }
}

/// Several realizations as one score: the parts of the first, each followed
/// by what the same part of every later realization holds. The parts are
/// put one after another as music21 puts them, each starting where the one
/// before it ended.
fn gathered(first: Stream, rest: impl Iterator<Item = Result<Stream>>) -> Result<Stream> {
    let mut parts: Vec<Stream> = first
        .events()
        .iter()
        .filter_map(|event| event.element().as_stream().cloned())
        .collect();
    // music21 appends each part to the score before anything is added to
    // it, so each starts where the one before ended as it then stood.
    let starts: Vec<FloatType> = parts
        .iter()
        .scan(0.0, |reached, part| {
            let start = *reached;
            *reached = op_frac(start + part.end_offset());
            Some(start)
        })
        .collect();
    for realization in rest {
        let realization = realization?;
        let held = realization
            .events()
            .iter()
            .filter_map(|event| event.element().as_stream());
        for (part, more) in parts.iter_mut().zip(held) {
            for event in more.events() {
                part.push(event.element().clone());
            }
        }
    }
    let mut score = Stream::with_kind(StreamKind::Score);
    for (start, part) in starts.into_iter().zip(parts) {
        score.insert(start, part);
    }
    Ok(score)
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

    /// Every count here is music21's own.
    #[test]
    fn a_line_is_realized_as_it_is_written() {
        let top = Pitch::from_name("B5").unwrap();
        let rules = Rules::default();

        // A bass note cut by a barline is a segment for each piece.
        let key = Key::from_tonic("B").unwrap();
        let meter = TimeSignature::new(3, 4).unwrap();
        let mut tied = FiguredBassLine::in_key_and_time(&key, meter).unwrap();
        tied.add_element(Pitch::from_name("B2").unwrap(), 1.0, None);
        tied.add_element(Pitch::from_name("C#3").unwrap(), 1.0, Some("6"));
        tied.add_element(Pitch::from_name("D#3").unwrap(), 2.5, Some("6,#4"));
        tied.add_element(Pitch::from_name("E3").unwrap(), 0.5, None);
        let realization = tied.realize(&rules, 4, &top).unwrap();
        let lengths: Vec<FloatType> = realization
            .segments()
            .iter()
            .map(Segment::quarter_length)
            .collect();
        assert_eq!(lengths, [1.0, 1.0, 1.0, 1.5, 0.5]);
        assert_eq!(realization.num_solutions(), 4748);

        // The figures are written under the bass, padded to one width, and
        // not again under the piece tied on.
        let written = tied.generate_bass_line().unwrap();
        let sung: Vec<Vec<String>> = written
            .notes()
            .iter()
            .map(|(_, element)| match element {
                StreamElement::Note(note) => {
                    note.lyrics().iter().map(|lyric| lyric.text()).collect()
                }
                _ => Vec::new(),
            })
            .collect();
        assert_eq!(
            sung,
            [
                vec![String::new()],
                vec!["6".to_string()],
                vec![" 6".to_string(), "#4".to_string()],
                Vec::new(),
                vec![String::new()]
            ]
        );
    }

    /// Every count here is music21's own.
    #[test]
    fn a_part_laid_over_the_line_is_kept() {
        let top = Pitch::from_name("B5").unwrap();
        let mut held = line(
            "C",
            &[
                ("C3", 1.0, None),
                ("D3", 1.0, Some("6")),
                ("E3", 2.0, Some("6")),
            ],
        );
        let mut melody = Stream::with_kind(StreamKind::Part);
        for name in ["E5", "F5", "G5", "C5"] {
            melody.push(
                Note::from_name(name)
                    .unwrap()
                    .with_duration(Duration::quarter()),
            );
        }
        held.overlay_part(&melody);
        let segments = held.segments(&Rules::default(), 4, &top).unwrap();
        let lengths: Vec<FloatType> = segments.iter().map(Segment::quarter_length).collect();
        // The melody moves once while the bass holds.
        assert_eq!(lengths, [1.0, 1.0, 2.0, 0.0]);
        assert!(segments[0].rules.part_pitch_limits.is_empty());
        assert_eq!(segments[2].rules.parts_to_check, [2, 3, 4]);
        let realization = held.realize(&Rules::default(), 4, &top).unwrap();
        assert_eq!(realization.num_solutions(), 9);
        for progression in realization.all_possibility_progressions() {
            let melody: Vec<String> = progression[1..]
                .iter()
                .map(|voicing| voicing[0].name_with_octave())
                .collect();
            assert_eq!(melody, ["F5", "G5", "C5"]);
        }
    }

    #[test]
    fn realizations_are_written_one_after_another() {
        let top = Pitch::from_name("B5").unwrap();
        let mut realization = line("C", &[("G2", 2.0, Some("7")), ("C3", 2.0, None)])
            .realize(&Rules::default(), 4, &top)
            .unwrap();
        let count = realization.num_solutions();
        let all = realization.generate_all_realizations().unwrap();
        assert_eq!(all.parts().len(), 2);
        assert_eq!(all.parts()[0].measures().len(), count);
        // Chosen by the caller, and as many as asked for.
        let chosen = realization.generate_random_realizations(3, |_| 0).unwrap();
        assert_eq!(chosen.parts()[1].measures().len(), 3);
        assert_eq!(
            realization
                .generate_random_realizations(count + 1, |_| 0)
                .unwrap()
                .parts()[0]
                .measures()
                .len(),
            count
        );
        // A staff for each part in chorale style.
        realization.set_keyboard_style_output(false);
        let chorale = realization.generate_random_realization(|_| 0).unwrap();
        assert_eq!(chorale.parts().len(), 4);
        // A line of chords has no bass line to write.
        let mut chords = FiguredBassLine::default();
        chords.add_chord(
            Pitch::from_name("C3").unwrap(),
            1.0,
            vec!["C".to_string(), "E".to_string(), "G".to_string()],
        );
        assert!(chords.generate_bass_line().is_err());
    }
}
