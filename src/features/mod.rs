//! Features of a piece of music, extracted for machine learning and corpus
//! study: music21's `features` package.
//!
//! A [`DataInstance`] prepares a stream once -- ties stripped, every part
//! and voice prepared on its own -- and works out each representation an
//! extractor asks for the first time one does: its pitches, its pitch and
//! interval histograms, its melodic contour. An [`Extractor`] reads those
//! and answers a [`Feature`], a vector of numbers of a fixed length.
//! [`jsymbolic::JSYMBOLIC`] holds the extractors music21 ports from Cory
//! McKay's jSymbolic.
//!
//! ```
//! use music21_rs::features::{DataInstance, jsymbolic};
//! use music21_rs::tinynotation::from_tiny_notation;
//!
//! let line = from_tiny_notation("4/4 c4 d e f g a b c'")?;
//! let data = DataInstance::new(&line)?;
//! let range = jsymbolic::extractor("P10").expect("the range extractor");
//! assert_eq!(range.extract(&data)?.vector(), [12.0]);
//! # Ok::<(), music21_rs::Error>(())
//! ```

pub mod jsymbolic;

use std::cell::OnceCell;

use crate::{
    analysis::pitch_analysis,
    defaults::{FloatType, IntegerType},
    error::{Error, Result},
    meter::TimeSignature,
    pitch::Pitch,
    stream::{ConsecutiveOptions, Stream, StreamElement, StreamEvent, StreamKind},
    tempo::MetronomeMark,
};

/// What an [`Extractor`] answers: its name and the vector it found.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Feature {
    id: String,
    name: String,
    discrete: bool,
    vector: Vec<FloatType>,
}

impl Feature {
    /// The extractor's id: jSymbolic's code for it, such as `P10`.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The feature's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Whether the values are counts rather than measurements.
    pub fn discrete(&self) -> bool {
        self.discrete
    }

    /// The values, as many as the extractor has dimensions.
    pub fn vector(&self) -> &[FloatType] {
        &self.vector
    }

    /// The labels music21 gives the values in a data set: the name with
    /// underscores for spaces, numbered where there is more than one.
    pub fn attribute_labels(&self) -> Vec<String> {
        let name = self.name.replace(' ', "_");
        if self.vector.len() == 1 {
            vec![name]
        } else {
            (0..self.vector.len())
                .map(|index| format!("{name}_{index}"))
                .collect()
        }
    }
}

/// One way of reading a feature out of a piece: music21's
/// `FeatureExtractor`, with what its class says about the feature.
#[derive(Clone, Copy)]
pub struct Extractor {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    dimensions: usize,
    discrete: bool,
    normalize: bool,
    process: fn(&DataInstance, &mut [FloatType]) -> Result<()>,
}

impl std::fmt::Debug for Extractor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Extractor")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("dimensions", &self.dimensions)
            .finish()
    }
}

impl Extractor {
    /// The extractor's id.
    pub fn id(&self) -> &'static str {
        self.id
    }

    /// What the feature is called.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// What the feature measures.
    pub fn description(&self) -> &'static str {
        self.description
    }

    /// How many values the feature has.
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    /// Whether the values are counts rather than measurements.
    pub fn discrete(&self) -> bool {
        self.discrete
    }

    /// Whether the values are scaled to sum to one.
    pub fn normalize(&self) -> bool {
        self.normalize
    }

    /// The feature of a prepared piece: music21's `extract`.
    ///
    /// # Errors
    ///
    /// A piece the feature cannot be read from -- one with no notes, for
    /// most of them -- or a normalized feature whose values are all nought.
    pub fn extract(&self, data: &DataInstance) -> Result<Feature> {
        let mut vector = vec![0.0; self.dimensions];
        (self.process)(data, &mut vector)?;
        if self.normalize {
            let sum: FloatType = vector.iter().sum();
            if sum == 0.0 {
                return Err(Error::Feature("cannot normalize zero vector".to_string()));
            }
            // music21 multiplies by the reciprocal rather than dividing.
            let scalar = 1.0 / sum;
            for value in &mut vector {
                *value *= scalar;
            }
        }
        Ok(Feature {
            id: self.id.to_string(),
            name: self.name.to_string(),
            discrete: self.discrete,
            vector,
        })
    }
}

/// A piece prepared for feature extraction, with each representation of it
/// worked out once: music21's `DataInstance` and the `StreamForms` it keeps.
#[derive(Debug)]
pub struct DataInstance {
    prepared: Stream,
    /// The parts of a score and then every voice anywhere in the piece,
    /// each prepared on its own, as music21 lists them.
    parts: Vec<DataInstance>,
    parts_count: usize,
    pitches: OnceCell<Vec<Pitch>>,
    midi_pitch_histogram: OnceCell<Vec<(IntegerType, usize)>>,
    pitch_class_histogram: OnceCell<[usize; 12]>,
    midi_interval_histogram: OnceCell<Vec<usize>>,
    contour: OnceCell<Vec<IntegerType>>,
    seconds: OnceCell<std::result::Result<Vec<Seconds>, Error>>,
    parts_sounding: OnceCell<std::result::Result<Vec<usize>, Error>>,
}

/// Where a note sounds in time, in seconds at the tempi of the piece:
/// music21's `secondsMap` entry.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Seconds {
    /// When the note starts.
    pub offset: FloatType,
    /// How long it sounds.
    pub duration: FloatType,
    /// When it stops.
    pub end: FloatType,
}

impl DataInstance {
    /// Prepares a piece: its ties stripped, and each part of a score and
    /// each voice anywhere in it prepared on its own.
    ///
    /// # Errors
    ///
    /// A tie the piece cannot be stripped of.
    pub fn new(stream: &Stream) -> Result<Self> {
        let mut instance = Self::forms(stream)?;
        if stream.kind() == StreamKind::Score {
            let parts = stream.parts();
            instance.parts_count = parts.len();
            for part in parts {
                instance.parts.push(Self::forms(part)?);
            }
        }
        for (_, element) in stream.recurse() {
            if let StreamElement::Stream(voice) = element
                && voice.kind() == StreamKind::Voice
            {
                instance.parts.push(Self::forms(voice)?);
            }
        }
        Ok(instance)
    }

    /// music21's `StreamForms`: the stream with its ties stripped.
    fn forms(stream: &Stream) -> Result<Self> {
        let mut prepared = stream.clone();
        prepared.strip_ties(true)?;
        Ok(Self {
            prepared,
            parts: Vec::new(),
            parts_count: 0,
            pitches: OnceCell::new(),
            midi_pitch_histogram: OnceCell::new(),
            pitch_class_histogram: OnceCell::new(),
            midi_interval_histogram: OnceCell::new(),
            contour: OnceCell::new(),
            seconds: OnceCell::new(),
            parts_sounding: OnceCell::new(),
        })
    }

    /// The piece with its ties stripped.
    pub fn prepared(&self) -> &Stream {
        &self.prepared
    }

    /// Each part of a score prepared on its own, then each voice: music21's
    /// `data['parts']`.
    pub fn parts(&self) -> &[DataInstance] {
        &self.parts
    }

    /// How many parts a score has: music21's `partsCount`, nought for a
    /// stream that is not a score.
    pub fn parts_count(&self) -> usize {
        self.parts_count
    }

    /// Every pitch of the piece's notes, chords and chord symbols: music21's
    /// `Stream.pitches`.
    pub fn pitches(&self) -> &[Pitch] {
        self.pitches
            .get_or_init(|| pitch_analysis::music21_pitches(&self.prepared).unwrap_or_default())
    }

    /// How often each MIDI note number sounds, in the order each is first
    /// met: music21's `midiPitchHistogram`, a `Counter`.
    pub fn midi_pitch_histogram(&self) -> &[(IntegerType, usize)] {
        self.midi_pitch_histogram.get_or_init(|| {
            let mut counts: Vec<(IntegerType, usize)> = Vec::new();
            for pitch in self.pitches() {
                let midi = pitch.midi();
                match counts.iter_mut().find(|(known, _)| *known == midi) {
                    Some((_, count)) => *count += 1,
                    None => counts.push((midi, 1)),
                }
            }
            counts
        })
    }

    /// How often each pitch class sounds: music21's `pitchClassHistogram`.
    pub fn pitch_class_histogram(&self) -> &[usize; 12] {
        self.pitch_class_histogram.get_or_init(|| {
            let mut counts = [0; 12];
            for pitch in self.pitches() {
                // music21's `pitchClass`: the pitch space rounded half to
                // even, folded into the octave.
                counts[(pitch.ps().round_ties_even() as i64).rem_euclid(12) as usize] += 1;
            }
            counts
        })
    }

    /// How often each melodic interval, in semitones either way, comes
    /// between neighbouring notes of each part, chords and rests passed
    /// over: music21's `midiIntervalHistogram`, 128 bins.
    pub fn midi_interval_histogram(&self) -> &[usize] {
        self.midi_interval_histogram.get_or_init(|| {
            let mut counts = vec![0; 128];
            let options = ConsecutiveOptions {
                skip_rests: true,
                skip_chords: true,
                skip_gaps: true,
                no_none: true,
                ..ConsecutiveOptions::default()
            };
            for part in self.melodic_parts(self.prepared.kind() == StreamKind::Score) {
                let midis = consecutive_midis(part, &options, false);
                for pair in midis.windows(2) {
                    let size = (pair[0] - pair[1]).unsigned_abs() as usize;
                    if let Some(count) = counts.get_mut(size) {
                        *count += 1;
                    }
                }
            }
            counts
        })
    }

    /// Every melodic step of each part in semitones, rising steps positive,
    /// a chord read as its highest note: music21's `contourList`.
    pub fn contour_list(&self) -> &[IntegerType] {
        self.contour.get_or_init(|| {
            let options = ConsecutiveOptions {
                skip_rests: true,
                skip_gaps: true,
                no_none: true,
                ..ConsecutiveOptions::default()
            };
            let mut contour = Vec::new();
            for part in self.melodic_parts(self.prepared.has_part_like_streams()) {
                let midis = consecutive_midis(part, &options, true);
                contour.extend(midis.windows(2).map(|pair| pair[1] - pair[0]));
            }
            contour
        })
    }

    /// The stretches of the piece each tempo holds, start, end and the mark
    /// sounding there: music21's `metronomeMarkBoundaries`. Where the piece
    /// states no tempo, or none from its start, a quarter at 120 holds.
    ///
    /// # Errors
    ///
    /// A metric modulation whose new tempo cannot be worked out.
    pub fn metronome_mark_boundaries(&self) -> Result<Vec<(FloatType, FloatType, MetronomeMark)>> {
        let flat = self.prepared.flatten();
        let highest = flat.end_offset();
        let lowest = flat.events().first().map_or(0.0, StreamEvent::offset);
        let mut marks: Vec<(FloatType, MetronomeMark)> = Vec::new();
        for event in flat.events() {
            let sounding = match event.element() {
                StreamElement::MetronomeMark(mark) => mark.clone(),
                StreamElement::TempoText(text) => text.metronome_mark(),
                StreamElement::MetricModulation(modulation) => {
                    let mut modulation = (**modulation).clone();
                    if modulation
                        .new_metronome()
                        .is_some_and(|mark| mark.number().is_none())
                    {
                        let previous = marks.last().map(|(_, mark)| mark.clone());
                        modulation.update_from(previous.as_ref());
                    }
                    modulation.new_metronome().cloned().ok_or_else(|| {
                        Error::Tempo("a metric modulation with no new tempo".to_string())
                    })?
                }
                _ => continue,
            };
            marks.push((event.offset(), sounding));
        }
        let default = || MetronomeMark::new(120.0);
        let mut boundaries = Vec::new();
        match marks.as_slice() {
            [] => boundaries.push((lowest, highest, default())),
            [(offset, mark)] => {
                if *offset > lowest {
                    boundaries.push((lowest, *offset, default()));
                    boundaries.push((*offset, highest, mark.clone()));
                } else {
                    boundaries.push((lowest, highest, mark.clone()));
                }
            }
            _ => {
                if marks[0].0 > lowest {
                    boundaries.push((lowest, marks[0].0, default()));
                }
                boundaries.push((marks[0].0, marks[1].0, marks[0].1.clone()));
                for index in 1..marks.len() {
                    let end = marks.get(index + 1).map_or(highest, |(offset, _)| *offset);
                    boundaries.push((marks[index].0, end, marks[index].1.clone()));
                }
            }
        }
        Ok(boundaries)
    }

    /// When each note, chord and chord symbol of the piece sounds, in
    /// seconds: music21's `secondsMap`, notes alone, in the order the piece
    /// holds them.
    ///
    /// # Errors
    ///
    /// A tempo that says no number, or as
    /// [`DataInstance::metronome_mark_boundaries`].
    pub fn seconds_map(&self) -> Result<&[Seconds]> {
        self.seconds
            .get_or_init(|| self.work_out_seconds())
            .as_deref()
            .map_err(Clone::clone)
    }

    fn work_out_seconds(&self) -> Result<Vec<Seconds>> {
        let boundaries = self.metronome_mark_boundaries()?;
        let flat = self.prepared.flatten();
        let lowest = flat.events().first().map_or(0.0, StreamEvent::offset);
        let mut seconds = Vec::new();
        for event in flat.events() {
            let element = event.element();
            if !matches!(
                element,
                StreamElement::Note(_)
                    | StreamElement::Chord(_)
                    | StreamElement::Unpitched(_)
                    | StreamElement::PercussionChord(_)
                    | StreamElement::ChordSymbol(_)
            ) {
                continue;
            }
            // music21 rounds the offset to eight places first.
            let offset = crate::statistics::python_round(event.offset(), 8);
            let start = accumulated_seconds(&boundaries, lowest, offset)?;
            let duration =
                accumulated_seconds(&boundaries, offset, offset + element.quarter_length())?;
            seconds.push(Seconds {
                offset: start,
                duration,
                end: start + duration,
            });
        }
        Ok(seconds)
    }

    /// How many parts sound in each chord of the piece chordified, in order:
    /// music21's `chordify(addPartIdAsGroup=True)`, read as the number of
    /// part ids on each chord's pitches. A piece that is not a score is not
    /// chordified, and its own chords carry no part ids, so each counts
    /// nought.
    ///
    /// # Errors
    ///
    /// A score that cannot be chordified.
    pub fn parts_sounding(&self) -> Result<&[usize]> {
        self.parts_sounding
            .get_or_init(|| {
                if self.prepared.kind() == StreamKind::Score {
                    self.prepared.chordify_parts_sounding()
                } else {
                    Ok(self
                        .prepared
                        .flatten()
                        .events()
                        .iter()
                        .filter(|event| {
                            matches!(
                                event.element(),
                                StreamElement::Chord(_) | StreamElement::ChordSymbol(_)
                            )
                        })
                        .map(|_| 0)
                        .collect())
                }
            })
            .as_deref()
            .map_err(Clone::clone)
    }

    /// The meters the piece states, in order.
    fn time_signatures(&self) -> Vec<TimeSignature> {
        self.prepared
            .flatten()
            .events()
            .iter()
            .filter_map(|event| match event.element() {
                StreamElement::TimeSignature(meter) => Some(meter.clone()),
                _ => None,
            })
            .collect()
    }

    /// The modes of the keys the piece states, in order.
    fn key_modes(&self) -> Vec<String> {
        self.prepared
            .flatten()
            .events()
            .iter()
            .filter_map(|event| match event.element() {
                StreamElement::Key(key) => Some(key.mode().to_string()),
                _ => None,
            })
            .collect()
    }

    fn melodic_parts(&self, by_part: bool) -> Vec<&Stream> {
        if by_part {
            self.prepared.parts()
        } else {
            vec![&self.prepared]
        }
    }
}

/// The MIDI numbers of the notes [`Stream::find_consecutive_notes`] finds,
/// a chord's highest on the staff where chords are kept.
fn consecutive_midis(
    part: &Stream,
    options: &ConsecutiveOptions,
    chords: bool,
) -> Vec<IntegerType> {
    let leaves = part.leaves();
    let found = part.find_consecutive_notes(options).unwrap_or_default();
    found
        .into_iter()
        .flatten()
        .filter_map(|position| match leaves[position].1 {
            StreamElement::Note(note) => Some(note.pitch().midi()),
            StreamElement::Chord(chord) if chords => chord
                .sort_diatonic_ascending()
                .pitches()
                .last()
                .map(Pitch::midi),
            StreamElement::ChordSymbol(symbol) if chords => {
                let mut pitches = symbol.pitches().ok()?;
                pitches.sort_by(|left, right| {
                    (left.diatonic_note_number(), left.ps())
                        .partial_cmp(&(right.diatonic_note_number(), right.ps()))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                pitches.last().map(Pitch::midi)
            }
            _ => None,
        })
        .collect()
}

/// How long the stretch from one offset to another lasts in seconds, at the
/// tempo holding each part of it: music21's `_accumulatedSeconds`.
fn accumulated_seconds(
    boundaries: &[(FloatType, FloatType, MetronomeMark)],
    start: FloatType,
    end: FloatType,
) -> Result<FloatType> {
    let mut total = 0.0;
    let mut active_start = start;
    for (from, to, mark) in boundaries {
        if !(*from <= active_start && active_start < *to) {
            continue;
        }
        let active_end = if end < *to { end } else { *to };
        let per_quarter = mark
            .sounding_quarter_bpm()
            .map(|bpm| 60.0 / bpm)
            .ok_or_else(|| {
                Error::Tempo("cannot derive seconds as getQuarterBPM() returns None".to_string())
            })?;
        total += per_quarter * (active_end - active_start);
        if active_end == end {
            break;
        }
        active_start = active_end;
    }
    Ok(total)
}
