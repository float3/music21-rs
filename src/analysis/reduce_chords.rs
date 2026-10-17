//! A score reduced to the chords it is made of: music21's
//! `analysis.reduceChords`.
//!
//! [`reduce_chords`] is music21's `ChordReducer.run`. It reads the score's
//! notes as spans of time -- a part, a stretch, and the notes sounding in
//! it -- and then thins them out: a span of no length goes, the spans of
//! each part are fused where they sound the same notes in one measure, they
//! are cut at every offset the bass moves, the notes above the bass of a
//! dissonant moment go, the gaps the thinning leaves are filled, and spans
//! shorter than half a note and then shorter than a note go unless they
//! fill their measure. What is left is chordified against the score, and
//! each measure is reduced to the few chords weighing most in it.
//!
//! The weight of a chord is music21's `qlbsmpConsonance`: how long it
//! sounds, times how strong its beat is, times a tenth if it is dissonant,
//! with the last chord of a measure counted as if it stood on the
//! downbeat.

use std::borrow::Cow;

use crate::chord::Chord;
use crate::defaults::{FloatType, IntegerType};
use crate::duration::Duration;
use crate::error::{Error, Result};
use crate::makenotation::op_frac;
use crate::meter::TimeSignature;
use crate::pitch::Pitch;
use crate::stream::{Stream, StreamElement, StreamKind};
use crate::tree::{Flatten, as_timespans};

/// What a reduction is asked for: music21's arguments to `ChordReducer.run`.
#[derive(Clone, Debug)]
pub struct ReductionOptions {
    /// How many chords a measure may keep: music21's
    /// `maximumNumberOfChords`, three by default.
    pub maximum_number_of_chords: usize,
    /// Whether every chord is written in closed position in the fourth
    /// octave afterwards: music21's `closedPosition`.
    pub closed_position: bool,
    /// Chords whose interval classes count as consonant however they
    /// sound: music21's `allowableChords`.
    pub allowable_chords: Vec<Chord>,
    /// Chords whose interval classes count as dissonant however they
    /// sound: music21's `forbiddenChords`.
    pub forbidden_chords: Vec<Chord>,
}

impl Default for ReductionOptions {
    fn default() -> Self {
        Self {
            maximum_number_of_chords: 3,
            closed_position: false,
            allowable_chords: Vec::new(),
            forbidden_chords: Vec::new(),
        }
    }
}

/// A score reduced to its chords: music21's `ChordReducer.run`, which hands
/// back a score of one part.
///
/// # Errors
///
/// A stream that is not a score, a part with no instrument to order it by,
/// or a length no duration can have.
pub fn reduce_chords(score: &Stream, options: &ReductionOptions) -> Result<Stream> {
    if score.kind() != StreamKind::Score {
        return Err(Error::Analysis(
            "a reduction is of a score, as music21's is".to_string(),
        ));
    }
    let mut reduction = Reduction::of(score)?;
    reduction.remove_zero_duration();
    reduction.split_by_bass();
    reduction.remove_vertical_dissonances(options);
    reduction.fill_bass_gaps();
    reduction.remove_short_spans(0.5);
    reduction.fill_bass_gaps();
    reduction.fill_measure_gaps();
    reduction.remove_short_spans(1.0);
    reduction.fill_bass_gaps();
    reduction.fill_measure_gaps();

    let chordified = reduction.chordified(score)?;
    let mut part = Stream::with_kind(StreamKind::Part);
    let mut at = 0.0;
    for event in chordified.events() {
        let Some(measure) = event.element().as_stream() else {
            continue;
        };
        if measure.kind() != StreamKind::Measure {
            continue;
        }
        let reduced = reduce_measure_to_n_chords(
            measure,
            options.maximum_number_of_chords,
            0.25,
            reduction.meter_at(event.offset()),
        )?;
        let length = reduced.end_offset();
        part.insert(at, reduced);
        at = op_frac(at + length);
    }
    if options.closed_position {
        close_positions(&mut part);
    }
    let mut answer = Stream::with_kind(StreamKind::Score);
    answer.insert(0.0, part);
    Ok(answer)
}

/// One stretch of one part, and the notes sounding in it: music21's
/// `PitchedTimespan` as the reducer keeps it.
#[derive(Clone, Debug)]
struct Span<'a> {
    /// Which of the score's parts it came from, in the order the reduction
    /// sorted them.
    part: usize,
    offset: FloatType,
    end: FloatType,
    /// Where the measure holding it stands, and where it ends: music21's
    /// `parentOffset` and `parentEndTime`.
    parent_offset: FloatType,
    parent_end: FloatType,
    measure: Option<IntegerType>,
    element: Cow<'a, StreamElement>,
    /// The strength of the beat it starts on, which the gap filling writes
    /// onto a span it moves: music21's `beatStrength` on a timespan.
    beat_strength: Option<FloatType>,
    /// When it was made. music21's steps take a span out and put a new one
    /// back, which lands after the spans already standing where it does,
    /// and the number says so without relying on how a sort breaks ties.
    seq: u64,
}

impl Span<'_> {
    fn quarter_length(&self) -> FloatType {
        op_frac(self.end - self.offset)
    }

    /// The notes it sounds, which are its element's: music21's `pitches`.
    fn pitches(&self) -> Vec<Pitch> {
        self.element.pitches()
    }

    /// Whether two spans sound the same notes, written the same way, which
    /// is what music21's `==` on a tuple of pitches asks.
    fn sounds_as(&self, other: &Span<'_>) -> bool {
        same_pitches(&self.pitches(), &other.pitches())
    }

    fn lowest(&self) -> Option<Pitch> {
        lowest_of(&self.pitches())
    }
}

/// Whether two lists of notes are the same list, written the same way,
/// which is what music21's `==` on a tuple of pitches asks: the crate's own
/// `==` reads a written natural as the note without one, where music21
/// does not.
fn same_pitches(one: &[Pitch], other: &[Pitch]) -> bool {
    one.len() == other.len()
        && one
            .iter()
            .zip(other)
            .all(|(left, right)| written_alike(left, right))
}

/// Whether two notes are the one note as it is written.
fn written_alike(one: &Pitch, other: &Pitch) -> bool {
    one.name_with_octave() == other.name_with_octave()
        && one.written_accidental().is_some() == other.written_accidental().is_some()
}

/// The lowest note of a list, by where it sounds.
fn lowest_of(pitches: &[Pitch]) -> Option<Pitch> {
    pitches
        .iter()
        .min_by(|left, right| left.ps().total_cmp(&right.ps()))
        .cloned()
}

/// What `remove_short_spans` groups a part's spans by: the measure, whether
/// the span is short, and the bass note under it.
type ShortKey = (Option<IntegerType>, bool, Option<usize>);

/// The spans of a score, and the steps that thin them out.
struct Reduction<'a> {
    spans: Vec<Span<'a>>,
    /// The meter each measure of the score states or is in, by the
    /// measure's offset.
    meters: Vec<(FloatType, TimeSignature)>,
    /// How many parts the score has, which says which is the bass.
    parts: usize,
    /// The number the next span made is given.
    next_seq: u64,
}

impl<'a> Reduction<'a> {
    /// The score read as spans, its parts in the order music21 sorts them:
    /// by the id of each part's instrument, as `allParts` does.
    fn of(score: &'a Stream) -> Result<Self> {
        let mut parts: Vec<&Stream> = score.parts().into_iter().collect();
        parts.sort_by_key(|part| part_id(part));
        let timespans = as_timespans(
            score,
            Flatten::Flat,
            Some(|element: &StreamElement| {
                matches!(element, StreamElement::Note(_) | StreamElement::Chord(_))
            }),
        );
        let mut spans = Vec::new();
        for span in timespans.timespans() {
            let part = span
                .part()
                .and_then(|held| {
                    parts.iter().position(|known| {
                        std::ptr::eq(std::ptr::from_ref(*known), std::ptr::from_ref(held))
                    })
                })
                .unwrap_or(0);
            spans.push(Span {
                part,
                offset: span.offset(),
                end: span.end_time(),
                parent_offset: span.parent_offset(),
                parent_end: span.parent_end_time(),
                measure: span.measure_number(),
                element: Cow::Borrowed(span.element()),
                beat_strength: None,
                seq: spans.len() as u64,
            });
        }
        let next_seq = spans.len() as u64;
        Ok(Self {
            spans,
            next_seq,
            meters: meters_of(score),
            parts: parts.len(),
        })
    }

    /// The meter in force where a measure stands, which is what says how
    /// strong each beat of it is: the last meter the score states at or
    /// before it, as a chord's `beatStrength` reads it off its measure.
    fn meter_at(&self, offset: FloatType) -> Option<TimeSignature> {
        self.meters
            .iter()
            .rfind(|(at, _)| *at <= offset)
            .map(|(_, meter)| meter.clone())
    }

    /// Spans of no length go: music21's `removeZeroDurationTimespans`.
    fn remove_zero_duration(&mut self) {
        self.spans.retain(|span| span.quarter_length() != 0.0);
    }

    /// Every part's spans fused where they sound alike, and every span cut
    /// where the bass moves: music21's `splitByBass`.
    fn split_by_bass(&mut self) {
        for part in 0..self.parts {
            self.fuse_by_part(part);
        }
        // The bass is the part sorting last, as music21 takes `parts[-1]`.
        let bass = self.parts.saturating_sub(1);
        let mut offsets: Vec<FloatType> = self
            .spans
            .iter()
            .filter(|span| span.part == bass)
            .map(|span| span.offset)
            .collect();
        offsets.sort_by(FloatType::total_cmp);
        offsets.dedup();
        self.split_at(&offsets);
    }

    /// One part's spans fused where a run of them sounds the same notes in
    /// one measure: music21's `fuseTimespansByPart`, which takes the run
    /// out and puts one span back in its place.
    fn fuse_by_part(&mut self, part: usize) {
        loop {
            let places: Vec<usize> = self
                .spans
                .iter()
                .enumerate()
                .filter(|(_, span)| span.part == part)
                .map(|(at, _)| at)
                .collect();
            // The first run of spans alike, if there is one.
            let mut run: Vec<usize> = Vec::new();
            for place in places {
                match run.last() {
                    None => run.push(place),
                    Some(last) => {
                        let (one, two) = (&self.spans[*last], &self.spans[place]);
                        if one.measure == two.measure && one.sounds_as(two) {
                            run.push(place);
                        } else if run.len() > 1 {
                            break;
                        } else {
                            run.clear();
                            run.push(place);
                        }
                    }
                }
            }
            if run.len() < 2 {
                return;
            }
            let mut fused = self.spans[run[0]].clone();
            fused.end = self.spans[run[run.len() - 1]].end;
            let fused = self.made(fused);
            let dropped: Vec<usize> = run;
            self.drop_places(&dropped);
            self.spans.push(fused);
            self.sort();
        }
    }

    /// Every span cut at each of these offsets: music21's `splitAt`, which
    /// takes each span straddling an offset out and puts its two pieces
    /// back -- so the pieces stand *after* the spans already there, among
    /// those starting and ending alike.
    fn split_at(&mut self, offsets: &[FloatType]) {
        for offset in offsets {
            let mut kept: Vec<Span<'a>> = Vec::new();
            let mut pieces: Vec<Span<'a>> = Vec::new();
            for span in self.spans.drain(..) {
                if span.offset < *offset && *offset < span.end {
                    let mut first = span.clone();
                    let mut second = span;
                    first.end = *offset;
                    second.offset = *offset;
                    first.seq = self.next_seq;
                    second.seq = self.next_seq + 1;
                    self.next_seq += 2;
                    pieces.push(first);
                    pieces.push(second);
                } else {
                    kept.push(span);
                }
            }
            kept.extend(pieces);
            self.spans = kept;
            self.sort();
        }
    }

    /// A span made by a step, numbered after every span before it, as
    /// music21's insertion puts a new timespan after the ones already
    /// standing where it does.
    fn made(&mut self, mut span: Span<'a>) -> Span<'a> {
        span.seq = self.next_seq;
        self.next_seq += 1;
        span
    }

    /// Where a span stands now, by its number.
    fn place_of(&self, seq: u64) -> Option<usize> {
        self.spans.iter().position(|span| span.seq == seq)
    }

    /// The spans kept in the order a tree walks them: by where each starts,
    /// then by where it ends.
    fn sort(&mut self) {
        self.spans.sort_by(|left, right| {
            left.offset
                .total_cmp(&right.offset)
                .then_with(|| left.end.total_cmp(&right.end))
                .then_with(|| left.seq.cmp(&right.seq))
        });
    }

    /// Every offset a span starts or stops at, in order: music21's
    /// `allTimePoints`.
    fn time_points(&self) -> Vec<FloatType> {
        let mut points: Vec<FloatType> = self
            .spans
            .iter()
            .flat_map(|span| [span.offset, span.end])
            .collect();
        points.sort_by(FloatType::total_cmp);
        points.dedup();
        points
    }

    /// The spans starting at an offset.
    fn starting_at(&self, offset: FloatType) -> Vec<usize> {
        self.spans
            .iter()
            .enumerate()
            .filter(|(_, span)| span.offset == offset)
            .map(|(at, _)| at)
            .collect()
    }

    /// The spans sounding at an offset, those starting there first and
    /// then those that began earlier: music21's
    /// `startAndOverlapTimespans`, whose order decides which span is read
    /// as the bass where two sound the same lowest note.
    fn sounding_at(&self, offset: FloatType) -> Vec<usize> {
        let mut places = self.starting_at(offset);
        places.extend(
            self.spans
                .iter()
                .enumerate()
                .filter(|(_, span)| span.offset < offset && offset < span.end)
                .map(|(at, _)| at),
        );
        places
    }

    /// The span holding the lowest note sounding at an offset: music21's
    /// `Verticality.bassTimespan`.
    fn bass_at(&self, offset: FloatType) -> Option<usize> {
        let mut lowest: Option<(Pitch, usize)> = None;
        for place in self.sounding_at(offset) {
            let Some(pitch) = self.spans[place].lowest() else {
                continue;
            };
            match &lowest {
                None => lowest = Some((pitch, place)),
                Some((overall, _)) if pitch.ps() <= overall.ps() => lowest = Some((pitch, place)),
                Some(_) => {}
            }
        }
        lowest.map(|(_, place)| place)
    }

    /// The notes above the bass of a dissonant moment go: music21's
    /// `removeVerticalDissonances`.
    ///
    /// music21 walks the verticalities live, from the first offset onwards
    /// by `nextVerticality`, and takes spans out as it goes -- so what
    /// sounds at a later offset is what the earlier removals left, and the
    /// walk steps to the next offset a span still starts at.
    fn remove_vertical_dissonances(&mut self, options: &ReductionOptions) {
        let allowable: Vec<Vec<u8>> = options
            .allowable_chords
            .iter()
            .map(|chord| interval_classes(&chord.pitches()))
            .collect();
        let forbidden: Vec<Vec<u8>> = options
            .forbidden_chords
            .iter()
            .map(|chord| interval_classes(&chord.pitches()))
            .collect();
        let mut offset = self.lowest_offset();
        while let Some(at) = offset {
            let pitches = self.pitch_set(at);
            let classes = interval_classes(&pitches);
            let mut consonant = allowable.contains(&classes);
            if Chord::new(&pitches[..]).is_ok_and(|chord| chord.is_consonant()) {
                consonant = true;
            }
            if forbidden.contains(&classes) {
                consonant = false;
            }
            if !consonant && let Some(lowest) = lowest_of(&pitches) {
                let dropped: Vec<usize> = self
                    .starting_at(at)
                    .into_iter()
                    .filter(|place| {
                        self.spans[*place]
                            .lowest()
                            .is_some_and(|pitch| !written_alike(&pitch, &lowest))
                    })
                    .collect();
                self.drop_places(&dropped);
            }
            offset = self.offset_after(at);
        }
    }

    /// The first offset a span starts at.
    fn lowest_offset(&self) -> Option<FloatType> {
        self.spans
            .iter()
            .map(|span| span.offset)
            .min_by(FloatType::total_cmp)
    }

    /// The next offset a span starts at: music21's `nextStartOffset`.
    fn offset_after(&self, offset: FloatType) -> Option<FloatType> {
        self.spans
            .iter()
            .map(|span| span.offset)
            .filter(|at| *at > offset)
            .min_by(FloatType::total_cmp)
    }

    /// The notes sounding at an offset, each written name once: music21's
    /// `Verticality.pitchSet`.
    fn pitch_set(&self, offset: FloatType) -> Vec<Pitch> {
        let mut pitches: Vec<Pitch> = Vec::new();
        for place in self.sounding_at(offset) {
            for pitch in self.spans[place].pitches() {
                if !pitches.iter().any(|known| written_alike(known, &pitch)) {
                    pitches.push(pitch);
                }
            }
        }
        pitches.sort_by(|left, right| left.ps().total_cmp(&right.ps()));
        pitches
    }

    /// The gaps the thinning left under each bass note filled: music21's
    /// `fillBassGaps`.
    ///
    /// Part by part, over a snapshot of that part's spans grouped by the
    /// bass note sounding under each: the first span of a group is moved
    /// back to where the bass note starts, the last is stretched to where
    /// it ends, and neighbours that sound alike or leave a gap are one
    /// span. Each change takes a span out and puts a new one back, as
    /// music21 does, so it stands after the spans it starts and ends with.
    fn fill_bass_gaps(&mut self) {
        for part in 0..self.parts {
            // A snapshot, as music21 takes `list(subtree)` before it
            // changes anything, with the bass note under each span.
            let mut snapshot: Vec<(u64, Option<u64>)> = Vec::new();
            for place in 0..self.spans.len() {
                if self.spans[place].part != part {
                    continue;
                }
                let seq = self.spans[place].seq;
                let bass = self
                    .bass_at(self.spans[place].offset)
                    .map(|under| self.spans[under].seq);
                snapshot.push((seq, bass));
            }
            let mut runs: Vec<(Option<u64>, Vec<u64>)> = Vec::new();
            for (seq, bass) in snapshot {
                match runs.last_mut() {
                    Some((known, run)) if *known == bass => run.push(seq),
                    _ => runs.push((bass, vec![seq])),
                }
            }
            for (bass, mut group) in runs {
                let Some(bass) = bass.and_then(|seq| self.place_of(seq)) else {
                    continue;
                };
                let bass_offset = self.spans[bass].offset;
                let bass_end = self.spans[bass].end;
                let bass_strength = self.spans[bass].beat_strength;
                if let Some(place) = self.place_of(group[0])
                    && bass_offset < self.spans[place].offset
                {
                    let mut offset = bass_offset;
                    if let Some(previous) = self.previous_in_part(place) {
                        offset = offset.max(self.spans[previous].end);
                    }
                    let mut moved = self.spans[place].clone();
                    moved.offset = offset;
                    moved.beat_strength = bass_strength;
                    let moved = self.made(moved);
                    group[0] = moved.seq;
                    self.spans.remove(place);
                    self.spans.push(moved);
                    self.sort();
                }
                let last = group.len() - 1;
                if let Some(place) = self.place_of(group[last])
                    && self.spans[place].end < bass_end
                {
                    let mut stretched = self.spans[place].clone();
                    stretched.end = bass_end;
                    let stretched = self.made(stretched);
                    group[last] = stretched.seq;
                    self.spans.remove(place);
                    self.spans.push(stretched);
                    self.sort();
                }
                self.fuse_neighbours(&mut group);
            }
        }
    }

    /// Neighbours of a group that sound alike, or that leave a gap between
    /// them, made one span: the last pass of music21's `fillBassGaps` and
    /// of its `fillMeasureGaps`, which writes the new span into both slots
    /// so that the next pair starts from it.
    fn fuse_neighbours(&mut self, group: &mut [u64]) {
        if group.len() < 2 {
            return;
        }
        for index in 0..group.len() - 1 {
            let (Some(one), Some(two)) =
                (self.place_of(group[index]), self.place_of(group[index + 1]))
            else {
                continue;
            };
            if one == two {
                continue;
            }
            let alike = self.spans[one].sounds_as(&self.spans[two]);
            let gap = self.spans[one].end != self.spans[two].offset;
            if !(alike || gap) {
                continue;
            }
            let mut fused = self.spans[one].clone();
            fused.end = self.spans[two].end;
            let fused = self.made(fused);
            group[index] = fused.seq;
            group[index + 1] = fused.seq;
            let (first, second) = (one.min(two), one.max(two));
            self.spans.remove(second);
            self.spans.remove(first);
            self.spans.push(fused);
            self.sort();
        }
    }

    /// The span of the same part standing before this one: music21's
    /// `findPreviousPitchedTimespanInSameStreamByClass`, which steps back
    /// verticality by verticality and takes the first span of the part
    /// *starting* there -- so a span that started earlier and is still
    /// sounding is the answer, however far past this one it reaches.
    fn previous_in_part(&self, place: usize) -> Option<usize> {
        let span = &self.spans[place];
        let offset = self
            .spans
            .iter()
            .filter(|other| other.part == span.part && other.offset < span.offset)
            .map(|other| other.offset)
            .max_by(FloatType::total_cmp)?;
        self.spans
            .iter()
            .position(|other| other.part == span.part && other.offset == offset)
    }

    /// Spans shorter than a length go, unless a run of them fills its
    /// measure or its bass note, where the notes sounding longest stay:
    /// music21's `removeShortTimespans`.
    fn remove_short_spans(&mut self, duration: FloatType) {
        let mut dropped: Vec<usize> = Vec::new();
        for part in 0..self.parts {
            let places: Vec<usize> = self
                .spans
                .iter()
                .enumerate()
                .filter(|(_, span)| span.part == part)
                .map(|(at, _)| at)
                .collect();
            let key = |place: usize| -> ShortKey {
                let span = &self.spans[place];
                let short = span.quarter_length() < duration;
                let bass = self
                    .bass_at(span.offset)
                    .filter(|bass| self.spans[*bass].quarter_length() >= duration);
                (span.measure, short, bass)
            };
            let mut runs: Vec<(ShortKey, Vec<usize>)> = Vec::new();
            for place in places {
                let said = key(place);
                match runs.last_mut() {
                    Some((known, run)) if *known == said => run.push(place),
                    _ => runs.push((said, vec![place])),
                }
            }
            for ((_, short, bass), run) in runs {
                if !short {
                    continue;
                }
                let first = &self.spans[run[0]];
                let last = &self.spans[run[run.len() - 1]];
                let mut whole_measure =
                    first.offset == first.parent_offset && last.end == first.parent_end;
                if let Some(bass) = bass
                    && first.offset == self.spans[bass].offset
                    && last.end == self.spans[bass].end
                {
                    whole_measure = true;
                }
                if !whole_measure {
                    dropped.extend(run);
                    continue;
                }
                // The notes the run sounds longest stay, as music21's
                // counter picks the most common.
                let mut counted: Vec<(Vec<Pitch>, FloatType)> = Vec::new();
                for place in &run {
                    let pitches = self.spans[*place].pitches();
                    let length = self.spans[*place].quarter_length();
                    match counted
                        .iter_mut()
                        .find(|(known, _)| same_pitches(known, &pitches))
                    {
                        Some((_, total)) => *total += length,
                        None => counted.push((pitches, length)),
                    }
                }
                // The notes sounding longest, and on a tie the ones the
                // run sounded first, as Python's `Counter.most_common`
                // keeps the order they were counted in.
                let best =
                    counted
                        .iter()
                        .fold(
                            None,
                            |best: Option<&(Vec<Pitch>, FloatType)>, next| match best {
                                Some(known) if known.1 >= next.1 => Some(known),
                                _ => Some(next),
                            },
                        );
                let Some((best, _)) = best else {
                    continue;
                };
                for place in &run {
                    if !same_pitches(&self.spans[*place].pitches(), best) {
                        dropped.push(*place);
                    }
                }
            }
        }
        self.drop_places(&dropped);
    }

    /// The gaps left inside each measure filled: music21's
    /// `fillMeasureGaps`.
    ///
    /// Part by part, over the spans grouped by the measure holding them:
    /// neighbours that sound alike or leave a gap are one span, the first
    /// span of a measure is moved back to the measure's start and given the
    /// downbeat's strength, and the last is stretched to its end.
    ///
    /// music21 keeps what to put in and what to take out in two sets and
    /// applies them only when the part is done, so the moving and the
    /// stretching both read the *fused* span rather than each other's
    /// answer -- a measure whose spans fuse into one comes out of this step
    /// as two overlapping spans, one reaching back to the measure's start
    /// and one on to its end, with the fused span itself taken out. That is
    /// what the steps after it are given, so it is what this does.
    fn fill_measure_gaps(&mut self) {
        for part in 0..self.parts {
            let mut runs: Vec<(Option<IntegerType>, Vec<Span<'a>>)> = Vec::new();
            for span in &self.spans {
                if span.part != part {
                    continue;
                }
                match runs.last_mut() {
                    Some((known, run)) if *known == span.measure => run.push(span.clone()),
                    _ => runs.push((span.measure, vec![span.clone()])),
                }
            }
            let mut to_remove: Vec<u64> = Vec::new();
            let mut to_insert: Vec<Span<'a>> = Vec::new();
            for (_, mut group) in runs {
                for index in 0..group.len().saturating_sub(1) {
                    let (one, two) = (group[index].clone(), group[index + 1].clone());
                    if !(one.sounds_as(&two) || one.end != two.offset) {
                        continue;
                    }
                    let mut fused = one.clone();
                    fused.end = two.end;
                    let fused = self.made(fused);
                    group[index] = fused.clone();
                    group[index + 1] = fused.clone();
                    to_insert.push(fused);
                    to_remove.push(one.seq);
                    to_remove.push(two.seq);
                }
                if group[0].offset != group[0].parent_offset {
                    let mut moved = group[0].clone();
                    moved.offset = moved.parent_offset;
                    moved.beat_strength = Some(1.0);
                    let moved = self.made(moved);
                    to_remove.push(group[0].seq);
                    to_insert.push(moved.clone());
                    group[0] = moved;
                }
                let last = group.len() - 1;
                if group[last].end != group[last].parent_end {
                    let mut stretched = group[last].clone();
                    stretched.end = stretched.parent_end;
                    let stretched = self.made(stretched);
                    to_remove.push(group[last].seq);
                    to_insert.push(stretched.clone());
                    group[last] = stretched;
                }
            }
            // What was put in and taken out again is simply gone, as
            // music21's `difference_update` leaves it.
            to_insert.retain(|span| !to_remove.contains(&span.seq));
            self.spans.extend(to_insert);
            self.spans.retain(|span| !to_remove.contains(&span.seq));
            self.sort();
        }
    }

    /// Takes spans out by where they stand.
    fn drop_places(&mut self, dropped: &[usize]) {
        if dropped.is_empty() {
            return;
        }
        let dropped: std::collections::HashSet<usize> = dropped.iter().copied().collect();
        let mut at = 0;
        self.spans.retain(|_| {
            let kept = !dropped.contains(&at);
            at += 1;
            kept
        });
    }

    /// The spans as a part of chords, measure by measure, against the
    /// score they came from: music21's `tree.toStream.chordified`.
    ///
    /// The part is the score's first part as a template -- everything but
    /// its notes, chords, rests and voices -- the spans are cut at every
    /// measure's start, and a chord is made for each stretch between two
    /// time points and *appended* to the measure it falls in, as music21
    /// appends it.
    fn chordified(&mut self, score: &Stream) -> Result<Stream> {
        let template = score
            .parts()
            .into_iter()
            .next()
            .ok_or_else(|| Error::Analysis("a score with no part to reduce".to_string()))?;
        let mut measure_offsets: Vec<FloatType> = template
            .events()
            .iter()
            .filter(|event| {
                event
                    .element()
                    .as_stream()
                    .is_some_and(|inner| inner.kind() == StreamKind::Measure)
            })
            .map(crate::stream::StreamEvent::offset)
            .collect();
        measure_offsets.push(score.end_offset());
        self.split_at(&measure_offsets);

        // music21's `chordified` templates the part with no
        // `removeClasses` of its own, so the dynamics and expressions go
        // with the notes.
        let mut out = crate::stream::chordify::template_removing(template, true);
        let mut filled: Vec<FloatType> = vec![0.0; measure_offsets.len()];
        let mut points = self.time_points();
        points.extend(measure_offsets.iter().copied());
        points.sort_by(FloatType::total_cmp);
        points.dedup();
        let mut index = 0;
        for pair in points.windows(2) {
            let (offset, end) = (pair[0], pair[1]);
            while index + 1 < measure_offsets.len() && measure_offsets[index + 1] <= offset {
                index += 1;
            }
            let sounding = self.sounding_at(offset);
            if sounding.is_empty() {
                continue;
            }
            let spans: Vec<crate::stream::chordify::Span<'_>> = sounding
                .iter()
                .map(|place| crate::stream::chordify::Span {
                    start: self.spans[*place].offset,
                    end: self.spans[*place].end,
                    element: &self.spans[*place].element,
                    part: self.spans[*place].part,
                })
                .collect();
            let element = crate::stream::chordify::make_element(
                &spans,
                offset,
                op_frac(end - offset),
                &crate::stream::ChordifyOptions::default(),
            )?;
            let mut measure = 0;
            for event in out.events_mut() {
                if let StreamElement::Stream(inner) = event.element_mut()
                    && inner.kind() == StreamKind::Measure
                {
                    if measure == index {
                        // Each chord goes after the one before it, from the
                        // measure's start, which is where music21's
                        // `append` puts them: what else the template holds
                        // -- a barline, a layout, a mark -- takes no time
                        // and does not move them along.
                        let at = filled[index];
                        filled[index] = op_frac(at + element.quarter_length());
                        inner.insert(at, element);
                        break;
                    }
                    measure += 1;
                }
            }
        }
        Ok(out)
    }
}

/// The id of the part's first instrument, which is what music21 sorts the
/// parts of a reduction by (`getInstrument().partId`). A part with no
/// instrument has none, and sorts last here where music21's own sort
/// refuses to compare it at all.
fn part_id(part: &Stream) -> Option<String> {
    part.leaves().iter().find_map(|(_, element)| match element {
        StreamElement::Instrument(instrument) => instrument.part_id().map(str::to_string),
        _ => None,
    })
}

/// Every meter the score states, by where it stands.
fn meters_of(score: &Stream) -> Vec<(FloatType, TimeSignature)> {
    let mut meters = Vec::new();
    for (offset, element) in score.leaves() {
        if let StreamElement::TimeSignature(meter) = element {
            meters.push((offset, meter.clone()));
        }
    }
    meters.sort_by(|left, right| left.0.total_cmp(&right.0));
    meters
}

/// Every interval class a list of notes holds, 1 to 6: music21's
/// `_getIntervalClassSet`.
fn interval_classes(pitches: &[Pitch]) -> Vec<u8> {
    let mut classes: Vec<u8> = Vec::new();
    for (index, one) in pitches.iter().enumerate() {
        for other in &pitches[index + 1..] {
            let mut semitones = (one.ps() - other.ps()).abs() as i64 % 12;
            if semitones >= 6 {
                semitones = 12 - semitones;
            }
            let class = semitones as u8;
            if class != 0 && !classes.contains(&class) {
                classes.push(class);
            }
        }
    }
    classes.sort_unstable();
    classes
}

/// How much a chord weighs in its measure: music21's `qlbsmpConsonance`,
/// which is how long it sounds times how strong its beat is, a tenth of
/// that where it is dissonant, and the length alone for the last chord of
/// the measure.
fn weight_of(
    chord: &Chord,
    length: FloatType,
    beat_strength: FloatType,
    last_in_measure: bool,
) -> FloatType {
    let consonance = if chord.is_consonant() { 1.0 } else { 0.1 };
    let weight = if last_in_measure {
        length
    } else {
        length * beat_strength
    };
    weight * consonance
}

/// A measure reduced to the few chords weighing most in it: music21's
/// `reduceMeasureToNChords`.
///
/// # Errors
///
/// A length no duration can have.
fn reduce_measure_to_n_chords(
    measure: &Stream,
    maximum: usize,
    trim_below: FloatType,
    meter: Option<TimeSignature>,
) -> Result<Stream> {
    let chords: Vec<(FloatType, Chord)> = measure
        .events()
        .iter()
        .filter_map(|event| match event.element() {
            StreamElement::Chord(chord) => Some((event.offset(), chord.clone())),
            _ => None,
        })
        .collect();
    let mut weights: Vec<(Vec<u8>, FloatType)> = Vec::new();
    for (index, (offset, chord)) in chords.iter().enumerate() {
        let classes = pitch_classes(chord);
        let length = chord
            .duration()
            .map_or(0.0, crate::duration::Duration::quarter_length);
        let strength = match &meter {
            Some(meter) => meter.accent_weight(*offset).unwrap_or(1.0),
            None => 1.0,
        };
        let weight = weight_of(chord, length, strength, index + 1 == chords.len());
        match weights.iter_mut().find(|(known, _)| *known == classes) {
            Some((_, total)) => *total += weight,
            None => weights.push((classes, weight)),
        }
    }
    // The measure as it stands without its notes, which is what music21
    // leaves when it removes the chords it does not keep: the clef, the
    // key, the meter, a layout, a barline and any rest the chordifying
    // made stay where they stood.
    let mut answer = measure.with_events(
        measure
            .events()
            .iter()
            .filter(|event| !matches!(event.element(), StreamElement::Chord(_)))
            .cloned()
            .collect(),
    );
    if weights.is_empty() {
        let length = measure.end_offset();
        answer.insert(0.0, crate::rest::Rest::new(Duration::new(length)?));
        return Ok(answer);
    }
    let mut sorted = weights.clone();
    sorted.sort_by(|(_, left), (_, right)| right.total_cmp(left));
    let keep = maximum.min(sorted.len());
    let highest = sorted[0].1;
    let kept: Vec<Vec<u8>> = sorted
        .into_iter()
        .take(keep)
        .take_while(|(_, weight)| *weight >= highest * trim_below)
        .map(|(classes, _)| classes)
        .collect();

    // Each kept chord takes the length of the chords it stands for, as
    // music21's greedy walk gives it.
    let mut greedy: Option<(usize, FloatType)> = None;
    let mut written: Vec<(FloatType, Chord, FloatType)> = Vec::new();
    let mut running = 0.0;
    let mut last_classes: Option<Vec<u8>> = None;
    for (offset, chord) in &chords {
        let classes = pitch_classes(chord);
        let length = chord
            .duration()
            .map_or(0.0, crate::duration::Duration::quarter_length);
        if kept.contains(&classes) && Some(&classes) != last_classes.as_ref() {
            match greedy {
                None if *offset != 0.0 => running = *offset,
                None => {}
                Some((place, _)) => {
                    written[place].2 = running;
                    running = 0.0;
                }
            }
            let mut kept_chord = chord.clone();
            undecide_accidentals(&mut kept_chord);
            let at = if greedy.is_none() && *offset != 0.0 {
                0.0
            } else {
                *offset
            };
            written.push((at, kept_chord, 0.0));
            greedy = Some((written.len() - 1, running));
            last_classes = Some(classes);
            running = op_frac(running + length);
        } else {
            running = op_frac(running + length);
        }
    }
    if let Some((place, _)) = greedy {
        written[place].2 = running;
    }
    // music21 evens out the syncopations over the measure as it stands,
    // the elements it keeps beside the chords included, so the element a
    // syncopated chord takes its time from may be one of those -- which
    // changes nothing a score says, since none of them sounds.
    let others: Vec<FloatType> = answer
        .events()
        .iter()
        .map(crate::stream::StreamEvent::offset)
        .collect();
    even_out_syncopations(&mut written, &others);
    for (offset, mut chord, length) in written {
        chord.set_duration(Duration::new(length.max(0.0))?);
        answer.insert(offset, chord);
    }
    Ok(answer)
}

/// The pitch classes a chord holds, as music21 keys its weights by them:
/// a set, so two chords of the same classes in another order weigh as one
/// -- which is what makes a chord struck again over a moving bass count
/// towards the chord already sounding rather than as a chord of its own.
fn pitch_classes(chord: &Chord) -> Vec<u8> {
    let classes: Vec<u8> = chord
        .pitches()
        .iter()
        .map(|pitch| (pitch.ps().round_ties_even() as i64).rem_euclid(12) as u8)
        .collect();
    python_set_order(&classes)
}

/// Pitch classes in the order music21 reads them out of a set, which is
/// CPython's own order for a set of small integers.
///
/// music21 keys a measure's weights by `tuple({p.pitchClass for p in ...})`,
/// and that tuple's order is the order of the hash table the set was built
/// in -- so two voicings of the same classes weigh as *one* chord where
/// their tables came out alike and as *two* where they did not, which
/// decides what a measure keeps. A set of up to four classes lives in a
/// table of eight slots, each class in slot `class & 7` or, where that is
/// taken, in the slot CPython's perturbation walks to next; the fifth class
/// grows the table to thirty-two slots, where every class has a slot of its
/// own and the order is ascending.
fn python_set_order(classes: &[u8]) -> Vec<u8> {
    let mut distinct: Vec<u8> = Vec::new();
    for class in classes {
        if !distinct.contains(class) {
            distinct.push(*class);
        }
    }
    if distinct.len() >= 5 {
        distinct.sort_unstable();
        return distinct;
    }
    let mut table: [Option<u8>; 8] = [None; 8];
    for class in &distinct {
        let mut index = usize::from(*class) & 7;
        while table[index].is_some() {
            // A class is its own hash and is smaller than 32, so CPython's
            // perturbation adds nothing to the walk.
            index = (index * 5 + 1) & 7;
        }
        table[index] = Some(*class);
    }
    table.into_iter().flatten().collect()
}

/// A chord's accidentals left undecided, as music21 leaves a kept chord's:
/// it clears the display status of every accidental actually written.
fn undecide_accidentals(chord: &mut Chord) {
    for note in chord.notes_mut() {
        let mut pitch = note.pitch().clone();
        if let Some(accidental) = pitch.written_accidental_mut() {
            accidental.set_display_status(None);
        }
        note.set_pitch(pitch);
    }
}

/// A chord standing a quarter, an eighth or a third of the way through a
/// beat is moved onto it, the element before it giving up the difference:
/// music21's "even chord lengths" pass, which walks the measure in order
/// and reads each offset as it has been left by the one before.
fn even_out_syncopations(written: &mut [(FloatType, Chord, FloatType)], others: &[FloatType]) {
    for index in 0..written.len() {
        let offset = written[index].0;
        let whole = offset.trunc();
        let past = offset - whole;
        let rounded = (past * 1000.0).round() / 1000.0;
        if ![0.250, 0.125, 0.333, 0.063, 0.062].contains(&rounded) {
            continue;
        }
        // The element standing before it, which is a chord only where no
        // other element of the measure stands between the two.
        let before = written[..index].last().map(|(at, _, _)| *at);
        let Some(before) = before else {
            // music21 leaves the first element of a measure alone.
            if index == 0 && !others.iter().any(|at| *at < offset) {
                continue;
            }
            written[index].0 = whole;
            written[index].2 = op_frac(written[index].2 + past);
            continue;
        };
        let interrupted = others.iter().any(|at| *at > before && *at <= offset);
        if !interrupted {
            written[index - 1].2 = op_frac(written[index - 1].2 - past);
        }
        written[index].0 = whole;
        written[index].2 = op_frac(written[index].2 + past);
    }
}

/// Every chord of the part written in closed position in the fourth
/// octave: music21's `closedPosition(forceOctave=4, inPlace=True)` over
/// every chord of the reduction.
fn close_positions(stream: &mut Stream) {
    for event in stream.events_mut() {
        match event.element_mut() {
            StreamElement::Stream(inner) => close_positions(inner),
            StreamElement::Chord(chord) => *chord = chord.closed_position(Some(4), false),
            _ => {}
        }
    }
}
