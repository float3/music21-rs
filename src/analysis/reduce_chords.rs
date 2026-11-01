//! A score reduced to the chords it is made of: music21's
//! `analysis.reduceChords`.
//!
//! [`reduce_chords`] is music21's `ChordReducer.run`. It reads the score's
//! notes as spans of time -- a part, a stretch, and the notes sounding in
//! it -- drops the spans of no length and, if asked, the notes above the
//! bass of a dissonant moment. Each measure of the first part is then cut
//! into slices wherever a note starts or stops, and the slices sounding
//! the same pitch classes are weighed together: how long each lasts, times
//! how strong the beat it starts on is (the last sounding slice counting
//! its length alone), times a tenth if it is dissonant. The few heaviest
//! are kept, each lasting until the next kept slice of other classes.

use std::borrow::Cow;

use crate::chord::Chord;
use crate::defaults::FloatType;
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
    /// Whether the notes above the bass of a dissonant moment go before
    /// the measures are weighed: music21's `removeDissonances`, off by
    /// default.
    pub remove_dissonances: bool,
}

impl Default for ReductionOptions {
    fn default() -> Self {
        Self {
            maximum_number_of_chords: 3,
            closed_position: false,
            allowable_chords: Vec::new(),
            forbidden_chords: Vec::new(),
            remove_dissonances: false,
        }
    }
}

/// A score reduced to its chords: music21's `ChordReducer.run`, which hands
/// back a score of one part, the first part's measures holding the chords.
///
/// # Errors
///
/// A stream that is not a score or has no part, a meter that cannot weigh
/// a slice, or a length no duration can have.
pub fn reduce_chords(score: &Stream, options: &ReductionOptions) -> Result<Stream> {
    if score.kind() != StreamKind::Score {
        return Err(Error::Analysis(
            "a reduction is of a score, as music21's is".to_string(),
        ));
    }
    let template = score
        .parts()
        .into_iter()
        .next()
        .ok_or_else(|| Error::Analysis("a score with no part to reduce".to_string()))?;
    let mut reduction = Reduction::of(score);
    reduction.remove_zero_duration();
    if options.remove_dissonances {
        reduction.remove_vertical_dissonances(options);
    }
    let consonance = Consonance::of(options);

    let mut part = crate::stream::chordify::template_of(template);
    let points = reduction.time_points();
    let mut meter = TimeSignature::default();
    let mut index = 0;
    for event in template.events() {
        let Some(measure) = event.element().as_stream() else {
            continue;
        };
        if measure.kind() != StreamKind::Measure {
            continue;
        }
        if let Some(stated) = stated_meter(measure) {
            meter = stated.clone();
        }
        let start = event.offset();
        let length = match measure.end_offset() {
            0.0 => meter.bar_quarter_length(),
            length => length,
        };
        let slices = reduction.measure_slices(start, op_frac(start + length), &points);
        let weights = slice_weights(&slices, &meter, measure.padding_left(), &consonance)?;
        let kept = keep_heaviest_slices(&slices, &weights, options.maximum_number_of_chords);
        let mut made = Vec::new();
        if kept.is_empty() {
            made.push((
                0.0,
                StreamElement::Rest(crate::rest::Rest::new(Duration::new(length)?)),
            ));
        }
        for slice in &kept {
            made.push((slice.offset, reduction.element_at(slice)?));
        }
        if let Some(reduced) = nth_measure(&mut part, index) {
            for (offset, element) in made {
                reduced.insert(offset, element);
            }
        }
        index += 1;
    }
    if options.closed_position {
        close_positions(&mut part);
    }
    let mut answer = Stream::with_kind(StreamKind::Score);
    answer.insert(0.0, part);
    Ok(answer)
}

/// The meter a measure states at its start: music21's
/// `Measure.timeSignature`.
fn stated_meter(measure: &Stream) -> Option<&TimeSignature> {
    measure
        .events()
        .iter()
        .find_map(|event| match event.element() {
            StreamElement::TimeSignature(meter) => Some((event.offset(), meter)),
            _ => None,
        })
        .filter(|(offset, _)| *offset == 0.0)
        .map(|(_, meter)| meter)
}

/// The measure of a part counted from nought.
fn nth_measure(part: &mut Stream, index: usize) -> Option<&mut Stream> {
    part.events_mut()
        .iter_mut()
        .filter_map(|event| match event.element_mut() {
            StreamElement::Stream(inner) if inner.kind() == StreamKind::Measure => {
                Some(inner.as_mut())
            }
            _ => None,
        })
        .nth(index)
}

/// One stretch of one part, and the notes sounding in it: music21's
/// `PitchedTimespan` as the reducer keeps it.
#[derive(Clone, Debug)]
struct Span<'a> {
    /// Which of the score's parts it came from, in the score's order.
    part: usize,
    offset: FloatType,
    end: FloatType,
    element: Cow<'a, StreamElement>,
}

impl Span<'_> {
    fn quarter_length(&self) -> FloatType {
        op_frac(self.end - self.offset)
    }

    /// The notes it sounds, which are its element's: music21's `pitches`.
    fn pitches(&self) -> Vec<Pitch> {
        self.element.pitches()
    }

    fn lowest(&self) -> Option<Pitch> {
        lowest_of(&self.pitches())
    }
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

/// What counts as consonant: the interval classes of music21's
/// `allowableChords` and `forbiddenChords`, as `run` makes them.
struct Consonance {
    allowable: Vec<Vec<u8>>,
    forbidden: Vec<Vec<u8>>,
}

impl Consonance {
    fn of(options: &ReductionOptions) -> Self {
        let classes = |chords: &[Chord]| {
            chords
                .iter()
                .map(|chord| interval_classes(&chord.pitches()))
                .collect()
        };
        Self {
            allowable: classes(&options.allowable_chords),
            forbidden: classes(&options.forbidden_chords),
        }
    }

    /// Whether notes sound consonant: music21's `isConsonant`. A forbidden
    /// chord's interval classes are dissonant and an allowable chord's
    /// consonant; anything else is as consonant as its chord.
    fn holds(&self, pitches: &[Pitch]) -> bool {
        let classes = interval_classes(pitches);
        if self.forbidden.contains(&classes) {
            return false;
        }
        if self.allowable.contains(&classes) {
            return true;
        }
        Chord::new(pitches).is_ok_and(|chord| chord.is_consonant())
    }
}

/// One stretch of a measure between two places where a note starts or
/// stops: music21's `MeasureSlice`.
#[derive(Clone, Debug)]
struct Slice {
    /// Where it stands in the measure.
    offset: FloatType,
    quarter_length: FloatType,
    /// Where its verticality stands in the score, which is where it
    /// started before a kept slice was stretched or moved.
    at: FloatType,
    /// The notes sounding at its start: the verticality's `pitchSet`.
    pitches: Vec<Pitch>,
}

impl Slice {
    /// Its sorted pitch classes, or none where nothing sounds.
    fn key(&self) -> Option<Vec<u8>> {
        if self.pitches.is_empty() {
            return None;
        }
        Some(sorted_classes(&self.pitches))
    }
}

/// The spans of a score, and the steps that read them.
struct Reduction<'a> {
    spans: Vec<Span<'a>>,
}

impl<'a> Reduction<'a> {
    /// The score read as spans, its parts in the score's own order, which
    /// is the order music21's `allParts` answers.
    fn of(score: &'a Stream) -> Self {
        let parts: Vec<&Stream> = score.parts().into_iter().collect();
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
                element: Cow::Borrowed(span.element()),
            });
        }
        Self { spans }
    }

    /// Spans of no length go: music21's `removeZeroDurationTimespans`.
    fn remove_zero_duration(&mut self) {
        self.spans.retain(|span| span.quarter_length() != 0.0);
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
    /// `startAndOverlapTimespans`.
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

    /// The notes above the bass of a dissonant moment go: music21's
    /// `removeVerticalDissonances`.
    ///
    /// music21 walks the verticalities live, from the first offset onwards
    /// by `nextVerticality`, and takes spans out as it goes -- so what
    /// sounds at a later offset is what the earlier removals left, and the
    /// walk steps to the next offset a span still starts at.
    fn remove_vertical_dissonances(&mut self, options: &ReductionOptions) {
        let consonance = Consonance::of(options);
        let mut offset = self.lowest_offset();
        while let Some(at) = offset {
            let pitches = self.pitch_set(at);
            if !consonance.holds(&pitches)
                && let Some(lowest) = lowest_of(&pitches)
            {
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

    /// A measure from `start` to `end` cut wherever a span starts or
    /// stops: music21's `measureSlices`, given the score's time points.
    fn measure_slices(&self, start: FloatType, end: FloatType, points: &[FloatType]) -> Vec<Slice> {
        let mut cuts = vec![start];
        cuts.extend(points.iter().copied().filter(|at| start < *at && *at < end));
        cuts.push(end);
        cuts.windows(2)
            .map(|pair| Slice {
                offset: op_frac(pair[0] - start),
                quarter_length: op_frac(pair[1] - pair[0]),
                at: pair[0],
                pitches: self.pitch_set(pair[0]),
            })
            .collect()
    }

    /// The chord a kept slice is written as: music21's
    /// `Verticality.makeElement` with no ties added.
    fn element_at(&self, slice: &Slice) -> Result<StreamElement> {
        let spans: Vec<crate::stream::chordify::Span<'_>> = self
            .sounding_at(slice.at)
            .into_iter()
            .map(|place| crate::stream::chordify::Span {
                start: self.spans[place].offset,
                end: self.spans[place].end,
                element: &self.spans[place].element,
                part: self.spans[place].part,
            })
            .collect();
        let options = crate::stream::ChordifyOptions {
            add_ties: false,
            ..crate::stream::ChordifyOptions::default()
        };
        crate::stream::chordify::make_element(&spans, slice.at, slice.quarter_length, &options)
    }
}

/// What each set of sorted pitch classes sounding in a measure weighs, in
/// the order they were met: music21's `sliceWeights`.
///
/// A slice weighs its length times the meter's accent where it starts,
/// read past the measure's `padding` so that a pickup is accented as the
/// end of a bar; the last sounding slice weighs its length alone, and a
/// dissonant one a tenth of it.
fn slice_weights(
    slices: &[Slice],
    meter: &TimeSignature,
    padding: FloatType,
    consonance: &Consonance,
) -> Result<Vec<(Vec<u8>, FloatType)>> {
    let sounding: Vec<usize> = (0..slices.len())
        .filter(|at| !slices[*at].pitches.is_empty())
        .collect();
    let mut weights: Vec<(Vec<u8>, FloatType)> = Vec::new();
    for at in &sounding {
        let slice = &slices[*at];
        let mut weight = if Some(at) == sounding.last() {
            slice.quarter_length
        } else {
            let accent = meter.accent_weight_with(op_frac(slice.offset + padding), true, true)?;
            slice.quarter_length * accent
        };
        if !consonance.holds(&slice.pitches) {
            weight *= 0.1;
        }
        let classes = sorted_classes(&slice.pitches);
        match weights.iter_mut().find(|(known, _)| *known == classes) {
            Some((_, total)) => *total += weight,
            None => weights.push((classes, weight)),
        }
    }
    Ok(weights)
}

/// The slices a measure keeps, lengthened: music21's
/// `keepHeaviestSlices` with its `trimBelow` of a quarter.
///
/// The `maximum` heaviest sets of classes are kept that weigh at least a
/// quarter of the heaviest. A kept slice lasts until the next kept slice
/// of other classes, the first starting the measure; one starting a
/// sixteenth, thirty-second, sixty-fourth or triplet eighth past a beat
/// is moved onto the beat.
fn keep_heaviest_slices(
    slices: &[Slice],
    weights: &[(Vec<u8>, FloatType)],
    maximum: usize,
) -> Vec<Slice> {
    let mut sorted: Vec<&(Vec<u8>, FloatType)> = weights.iter().collect();
    sorted.sort_by(|left, right| right.1.total_cmp(&left.1));
    sorted.truncate(maximum);
    let Some(heaviest) = sorted.first().map(|(_, weight)| *weight) else {
        return Vec::new();
    };
    let kept_keys: Vec<&Vec<u8>> = sorted
        .iter()
        .filter(|(_, weight)| *weight >= heaviest * 0.25)
        .map(|(classes, _)| classes)
        .collect();

    let mut kept: Vec<Slice> = Vec::new();
    let mut current: Option<Vec<u8>> = None;
    for slice in slices {
        let key = slice.key();
        let end = op_frac(slice.offset + slice.quarter_length);
        if let Some(classes) = &key
            && kept_keys.contains(&classes)
            && key != current
        {
            let offset = if kept.is_empty() { 0.0 } else { slice.offset };
            kept.push(Slice {
                offset,
                quarter_length: op_frac(end - offset),
                ..slice.clone()
            });
            current = key;
        } else if let Some(last) = kept.last_mut() {
            last.quarter_length = op_frac(end - last.offset);
        }
    }
    for index in 1..kept.len() {
        let past = op_frac(kept[index].offset - kept[index].offset.trunc());
        let rounded = (past * 1000.0).round() / 1000.0;
        if ![0.25, 0.125, 0.333, 0.063, 0.062].contains(&rounded) {
            continue;
        }
        kept[index - 1].quarter_length = op_frac(kept[index - 1].quarter_length - past);
        kept[index].offset = op_frac(kept[index].offset - past);
        kept[index].quarter_length = op_frac(kept[index].quarter_length + past);
    }
    kept.retain(|slice| slice.quarter_length != 0.0);
    kept
}

/// The distinct pitch classes of some notes, in order.
fn sorted_classes(pitches: &[Pitch]) -> Vec<u8> {
    let mut classes: Vec<u8> = pitches.iter().map(pitch_class).collect();
    classes.sort_unstable();
    classes.dedup();
    classes
}

/// A note's pitch class: music21's `pitchClass`.
fn pitch_class(pitch: &Pitch) -> u8 {
    (pitch.ps().round_ties_even() as i64).rem_euclid(12) as u8
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

/// How much a chord weighs in its measure: music21's four weight
/// algorithms, one of which its `ChordReducer` hands to
/// `computeMeasureChordWeights`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ChordWeight {
    /// How long the chord sounds, and nothing else: music21's
    /// `quarterLengthOnly`, which is what `computeMeasureChordWeights`
    /// weighs by when it is handed none.
    #[default]
    Length,
    /// That times how strong its beat is: `quarterLengthBeatStrength`.
    LengthByBeat,
    /// The same, except that the last chord of the measure counts as a
    /// full beat: `quarterLengthBeatStrengthMeasurePosition`.
    LengthByBeatInMeasure,
    /// The same again, a tenth of it where the chord is dissonant:
    /// `qlbsmpConsonance`, which is the one a reduction weighs by.
    Consonance,
}

impl ChordWeight {
    /// The name music21 gives the method.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Length => "quarterLengthOnly",
            Self::LengthByBeat => "quarterLengthBeatStrength",
            Self::LengthByBeatInMeasure => "quarterLengthBeatStrengthMeasurePosition",
            Self::Consonance => "qlbsmpConsonance",
        }
    }

    /// What a name stands for, which is how a caller handing music21's own
    /// method over is read.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        [
            Self::Length,
            Self::LengthByBeat,
            Self::LengthByBeatInMeasure,
            Self::Consonance,
        ]
        .into_iter()
        .find(|found| found.name() == name)
    }

    /// How much one chord weighs under this algorithm.
    #[must_use]
    pub fn of(
        self,
        chord: &Chord,
        length: FloatType,
        beat_strength: FloatType,
        last_in_measure: bool,
    ) -> FloatType {
        let by_beat = if last_in_measure {
            length
        } else {
            length * beat_strength
        };
        match self {
            Self::Length => length,
            Self::LengthByBeat => length * beat_strength,
            Self::LengthByBeatInMeasure => by_beat,
            Self::Consonance => {
                let consonance = if chord.is_consonant() { 1.0 } else { 0.1 };
                by_beat * consonance
            }
        }
    }
}

/// One chord of a measure as the weighing reads it: where it stands, what
/// it is, and how strong the beat it stands on is.
///
/// music21 reads the beat strength off the chord's own place in the measure
/// holding it; a chord here knows neither, so the caller says.
#[derive(Clone, Copy, Debug)]
pub struct WeighedChord<'a> {
    /// Where the chord stands in the measure.
    pub offset: FloatType,
    /// The chord itself.
    pub chord: &'a Chord,
    /// How strong the beat it stands on is.
    pub beat_strength: FloatType,
}

/// How much each chord of a measure weighs, keyed by the pitch classes it
/// sounds and in the order the chords were met: music21's
/// `computeMeasureChordWeights`.
///
/// Two chords of the same classes weigh as one, which is what makes a chord
/// struck again over a moving bass count towards the chord already sounding
/// rather than as a chord of its own. The classes are sorted, so two
/// voicings of them are the one chord.
#[must_use]
pub fn measure_chord_weights(
    chords: &[WeighedChord<'_>],
    algorithm: ChordWeight,
) -> Vec<(Vec<u8>, FloatType)> {
    let mut weights: Vec<(Vec<u8>, FloatType)> = Vec::new();
    for (index, held) in chords.iter().enumerate() {
        let classes = pitch_classes(held.chord);
        let length = held
            .chord
            .duration()
            .map_or(0.0, crate::duration::Duration::quarter_length);
        let weight = algorithm.of(
            held.chord,
            length,
            held.beat_strength,
            index + 1 == chords.len(),
        );
        match weights.iter_mut().find(|(known, _)| *known == classes) {
            Some((_, total)) => *total += weight,
            None => weights.push((classes, weight)),
        }
    }
    weights
}

/// One chord a reduction keeps: which of the measure's chords it is, where
/// it goes and how long it sounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeptChord {
    /// Which of the chords given it is, counted from nought.
    pub at: usize,
    /// Where it stands in the measure afterwards.
    pub offset: FloatType,
    /// How long it sounds afterwards.
    pub length: FloatType,
}

/// Which chords of a measure a reduction keeps, and where: music21's
/// `reduceMeasureToNChords` read as places rather than as a stream.
///
/// Every chord not named is one the reduction drops, and a kept chord takes
/// the time of the chords it stands for -- music21's greedy walk. `others`
/// is where the measure's other elements stand, which the evening out of
/// syncopations reads: a chord may take its time from one of those, which
/// changes nothing a score says since none of them sounds.
#[must_use]
pub fn reduced_measure_places(
    chords: &[WeighedChord<'_>],
    others: &[FloatType],
    maximum: usize,
    trim_below: FloatType,
    algorithm: ChordWeight,
) -> Vec<KeptChord> {
    let weights = measure_chord_weights(chords, algorithm);
    if weights.is_empty() {
        return Vec::new();
    }
    let mut sorted = weights;
    sorted.sort_by(|(_, left), (_, right)| right.total_cmp(left));
    let keep = maximum.min(sorted.len());
    let highest = sorted[0].1;
    let kept: Vec<Vec<u8>> = sorted
        .into_iter()
        .take(keep)
        .take_while(|(_, weight)| *weight >= highest * trim_below)
        .map(|(classes, _)| classes)
        .collect();

    let mut greedy: Option<usize> = None;
    let mut written: Vec<KeptChord> = Vec::new();
    let mut running = 0.0;
    let mut last_classes: Option<Vec<u8>> = None;
    for (at, held) in chords.iter().enumerate() {
        let classes = pitch_classes(held.chord);
        let length = held
            .chord
            .duration()
            .map_or(0.0, crate::duration::Duration::quarter_length);
        if kept.contains(&classes) && Some(&classes) != last_classes.as_ref() {
            match greedy {
                None if held.offset != 0.0 => running = held.offset,
                None => {}
                Some(place) => {
                    written[place].length = running;
                    running = 0.0;
                }
            }
            let offset = if greedy.is_none() && held.offset != 0.0 {
                0.0
            } else {
                held.offset
            };
            written.push(KeptChord {
                at,
                offset,
                length: 0.0,
            });
            greedy = Some(written.len() - 1);
            last_classes = Some(classes);
            running = op_frac(running + length);
        } else {
            running = op_frac(running + length);
        }
    }
    if let Some(place) = greedy {
        written[place].length = running;
    }
    even_out_syncopations(&mut written, others);
    written
}

/// A measure reduced to the few chords weighing most in it: music21's
/// `reduceMeasureToNChords`, built on [`reduced_measure_places`].
///
/// `meter` is the meter in force where the measure stands, which the beat
/// strengths are read in; without one every beat is as strong as every
/// other, as music21 reads a measure standing in no meter.
///
/// # Errors
///
/// A length no duration can have.
pub fn reduce_measure_to_n_chords(
    measure: &Stream,
    maximum: usize,
    trim_below: FloatType,
    meter: Option<TimeSignature>,
    algorithm: ChordWeight,
) -> Result<Stream> {
    let held: Vec<(FloatType, Chord)> = measure
        .events()
        .iter()
        .filter_map(|event| match event.element() {
            StreamElement::Chord(chord) => Some((event.offset(), chord.clone())),
            _ => None,
        })
        .collect();
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
    let others: Vec<FloatType> = answer
        .events()
        .iter()
        .map(crate::stream::StreamEvent::offset)
        .collect();
    let chords: Vec<WeighedChord<'_>> = held
        .iter()
        .map(|(offset, chord)| WeighedChord {
            offset: *offset,
            chord,
            beat_strength: match &meter {
                Some(meter) => meter.accent_weight(*offset).unwrap_or(1.0),
                None => 1.0,
            },
        })
        .collect();
    let kept = reduced_measure_places(&chords, &others, maximum, trim_below, algorithm);
    if kept.is_empty() {
        let length = measure.end_offset();
        answer.insert(0.0, crate::rest::Rest::new(Duration::new(length)?));
        return Ok(answer);
    }
    for place in kept {
        let mut chord = held[place.at].1.clone();
        undecide_accidentals(&mut chord);
        chord.set_duration(Duration::new(place.length.max(0.0))?);
        answer.insert(place.offset, chord);
    }
    Ok(answer)
}

/// The pitch classes a chord holds, sorted, as music21 keys its weights
/// by them: two chords of the same classes in any order weigh as one.
fn pitch_classes(chord: &Chord) -> Vec<u8> {
    sorted_classes(&chord.pitches())
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
fn even_out_syncopations(written: &mut [KeptChord], others: &[FloatType]) {
    for index in 0..written.len() {
        let offset = written[index].offset;
        let whole = offset.trunc();
        let past = offset - whole;
        let rounded = (past * 1000.0).round() / 1000.0;
        if ![0.250, 0.125, 0.333, 0.063, 0.062].contains(&rounded) {
            continue;
        }
        // The element standing before it, which is a chord only where no
        // other element of the measure stands between the two.
        let before = written[..index].last().map(|kept| kept.offset);
        let Some(before) = before else {
            // music21 leaves the first element of a measure alone.
            if index == 0 && !others.iter().any(|at| *at < offset) {
                continue;
            }
            written[index].offset = whole;
            written[index].length = op_frac(written[index].length + past);
            continue;
        };
        let interrupted = others.iter().any(|at| *at > before && *at <= offset);
        if !interrupted {
            written[index - 1].length = op_frac(written[index - 1].length - past);
        }
        written[index].offset = whole;
        written[index].length = op_frac(written[index].length + past);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tinynotation::from_tiny_notation;

    fn score_of(lines: &[&str]) -> Stream {
        let mut score = Stream::with_kind(StreamKind::Score);
        for line in lines {
            score.insert(0.0, from_tiny_notation(line).unwrap());
        }
        score
    }

    /// Each measure of the reduction as its chords: offset, length and
    /// the notes.
    fn chords_of(reduction: &Stream) -> Vec<Vec<(FloatType, FloatType, Vec<String>)>> {
        let part = reduction.parts().into_iter().next().unwrap();
        part.measures()
            .into_iter()
            .map(|measure| {
                measure
                    .events()
                    .iter()
                    .filter_map(|event| match event.element() {
                        StreamElement::Chord(chord) => Some((
                            event.offset(),
                            chord.duration().map_or(0.0, Duration::quarter_length),
                            chord
                                .pitches()
                                .iter()
                                .map(Pitch::name_with_octave)
                                .collect(),
                        )),
                        _ => None,
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn a_passing_dissonance_is_dropped() {
        let score = score_of(&["4/4 e2 f4 e4 g2 r2", "4/4 C1 C2 r2"]);
        let reduction = reduce_chords(&score, &ReductionOptions::default()).unwrap();
        let names = |names: &[&str]| names.iter().map(ToString::to_string).collect();
        assert_eq!(
            chords_of(&reduction),
            [
                vec![(0.0, 4.0, names(&["C3", "E4"]))],
                vec![(0.0, 4.0, names(&["C3", "G4"]))],
            ]
        );
    }

    #[test]
    fn a_bass_gap_is_filled_up_to_the_next_span() {
        let score = score_of(&[
            "3/4 r8 r16 d'16 e'16 r16 r8 r4",
            "3/4 g8 G4 r8 r4",
            "3/4 G8 r8 r4 r16 G16 r8",
        ]);
        let reduction = reduce_chords(&score, &ReductionOptions::default()).unwrap();
        let names = |names: &[&str]| names.iter().map(ToString::to_string).collect();
        assert_eq!(
            chords_of(&reduction),
            [vec![(0.0, 3.0, names(&["G3", "G4"]))]]
        );
    }

    #[test]
    fn chords_are_chosen_from_slices_of_the_score() {
        let score = score_of(&["4/4 c4 d4 e4 f4 g1", "4/4 C2 G,2 C1"]);
        let reduction = reduce_chords(&score, &ReductionOptions::default()).unwrap();
        let names = |names: &[&str]| names.iter().map(ToString::to_string).collect();
        assert_eq!(
            chords_of(&reduction),
            [
                vec![
                    (0.0, 2.0, names(&["C3", "C4"])),
                    (2.0, 2.0, names(&["C3", "E4"])),
                ],
                vec![(0.0, 2.0, names(&["C3", "G4"])), (2.0, 2.0, names(&["G4"]))],
            ]
        );
        let score = score_of(&["4/4 g4 a8 b8 c'2", "4/4 e4 f4 e2", "4/4 C1"]);
        let reduction = reduce_chords(&score, &ReductionOptions::default()).unwrap();
        assert_eq!(
            chords_of(&reduction),
            [vec![
                (0.0, 2.0, names(&["C3", "E4", "G4"])),
                (2.0, 2.0, names(&["C3", "E4", "C5"])),
            ]]
        );
    }

    #[test]
    fn the_first_parts_measures_are_reduced() {
        let score = score_of(&["4/4 e1 f1", "2/4 C2 D2 C2 D2"]);
        let reduction = reduce_chords(&score, &ReductionOptions::default()).unwrap();
        let part = reduction.parts().into_iter().next().unwrap();
        let offsets: Vec<FloatType> = part
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
        assert_eq!(offsets, [0.0, 4.0]);
    }

    #[test]
    fn pitch_classes_in_any_order_weigh_as_one() {
        let half = || Duration::half();
        let first = Chord::new("B3 D#4").unwrap().with_duration(half());
        let second = Chord::new("D#3 B3").unwrap().with_duration(half());
        let chords = [
            WeighedChord {
                offset: 0.0,
                chord: &first,
                beat_strength: 1.0,
            },
            WeighedChord {
                offset: 2.0,
                chord: &second,
                beat_strength: 0.5,
            },
        ];
        assert_eq!(
            measure_chord_weights(&chords, ChordWeight::Length),
            [(vec![3, 11], 4.0)]
        );
    }
}
