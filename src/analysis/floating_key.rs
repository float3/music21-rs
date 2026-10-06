//! A key for every measure, each measure's own reading smoothed by its
//! neighbours': music21's `analysis.floatingKey`.

use crate::{
    analysis::{KeyEstimate, KeyProfile, estimate_key_of_stream},
    defaults::{FloatType, IntegerType},
    error::{Error, Result},
    key::Key,
    stream::{Stream, StreamKind},
};

/// How much a neighbouring measure's reading counts at a distance: its
/// coefficient over the distance plus one. music21's `divide`.
pub fn divide(coefficient: FloatType, distance: IntegerType) -> FloatType {
    coefficient / (distance.abs() + 1) as FloatType
}

/// The keys of a stream's measures, read one measure at a time and then
/// smoothed by the measures around each: music21's `KeyAnalyzer`.
///
/// A measure is read as every part's measure at that place, by the
/// Aarden-Essen profile music21's `analyze('key')` uses, and a measure with
/// no notes has no reading. Each reading is then given the readings of the
/// `window_size` measures either side, each weighed by `weight_algorithm`
/// at its distance, and the key scoring best is the measure's.
///
/// ```
/// use music21_rs::analysis::floating_key::KeyAnalyzer;
/// use music21_rs::tinynotation::from_tiny_notation;
///
/// let line = from_tiny_notation("4/4 c4 e g c' c e g c' g b d' g d f# a d'")?;
/// let analyzer = KeyAnalyzer::new(&line)?;
/// let keys: Vec<String> = analyzer
///     .run()?
///     .iter()
///     .map(|key| key.tonic_pitch_name_with_case())
///     .collect();
/// assert_eq!(keys, ["C", "C", "G", "G"]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct KeyAnalyzer<'a> {
    stream: &'a Stream,
    measure_count: usize,
    /// How many measures either side of each are taken into account. Four
    /// by default; fewer for music that changes key often.
    pub window_size: usize,
    /// How a neighbour's reading is weighed at its distance in measures.
    pub weight_algorithm: fn(FloatType, IntegerType) -> FloatType,
}

/// A measure's readings, each key's name with case beside its coefficient,
/// best first.
type Interpretations = Vec<(String, FloatType)>;

impl<'a> KeyAnalyzer<'a> {
    /// An analyzer of a stream of measures, or of a score whose first part
    /// is in measures, which says how many there are.
    ///
    /// # Errors
    ///
    /// A stream with no measures in it.
    pub fn new(stream: &'a Stream) -> Result<Self> {
        let first = if stream.has_part_like_streams() {
            stream.parts().into_iter().next().unwrap_or(stream)
        } else {
            stream
        };
        let measure_count = measures_of(first).len();
        if measure_count == 0 {
            return Err(Error::Analysis(
                "Stream must have Measures inside it".to_string(),
            ));
        }
        Ok(Self {
            stream,
            measure_count,
            window_size: 4,
            weight_algorithm: divide,
        })
    }

    /// How many measures are read.
    pub fn measure_count(&self) -> usize {
        self.measure_count
    }

    /// Every measure's reading on its own, nothing for a measure with no
    /// notes: music21's `getRawKeyByMeasure`.
    pub fn raw_key_by_measure(&self) -> Vec<Option<Vec<KeyEstimate>>> {
        (0..self.measure_count)
            .map(|index| estimate_key_of_stream(KeyProfile::AardenEssen, &self.slice(index)))
            .collect()
    }

    /// A measure's reading as each key's name with case and its coefficient,
    /// best first: music21's `getInterpretationByMeasure`.
    pub fn interpretation_by_measure(&self, index: usize) -> Option<Interpretations> {
        estimate_key_of_stream(KeyProfile::AardenEssen, &self.slice(index))
            .map(|estimates| interpretations(&estimates))
    }

    /// The key of every measure that has a reading, smoothed by its
    /// neighbours': music21's `smoothInterpretationByMeasure`. A measure
    /// with no reading has no key, and is left out.
    ///
    /// music21 keeps each measure's reading once it has been asked for and
    /// hands that very reading back after, so a measure smoothed by the time
    /// a later one asks for it is read smoothed; the first measure is
    /// smoothed on a copy and stays as it was read. This is kept.
    ///
    /// # Errors
    ///
    /// A best reading that names no key.
    pub fn smooth_interpretation_by_measure(&self) -> Result<Vec<Key>> {
        let raw: Vec<Option<Interpretations>> = self
            .raw_key_by_measure()
            .iter()
            .map(|estimates| estimates.as_deref().map(interpretations))
            .collect();
        let mut kept: Vec<Option<Option<Interpretations>>> = vec![None; self.measure_count];
        let mut smoothed = Vec::new();
        for index in 0..self.measure_count {
            // The first time a reading is asked for it is kept and a copy
            // handed back; after that the kept reading itself.
            let first_time = kept[index].is_none();
            let mut base = match &kept[index] {
                Some(reading) => reading.clone(),
                None => {
                    kept[index] = Some(raw[index].clone());
                    raw[index].clone()
                }
            };
            let Some(readings) = base.as_mut() else {
                continue;
            };
            let window = self.window_size as IntegerType;
            for distance in -window..=window {
                let neighbour = index as IntegerType + distance;
                if neighbour < 0 || neighbour >= self.measure_count as IntegerType || distance == 0
                {
                    continue;
                }
                let neighbour = neighbour as usize;
                if kept[neighbour].is_none() {
                    kept[neighbour] = Some(raw[neighbour].clone());
                }
                let Some(Some(theirs)) = &kept[neighbour] else {
                    continue;
                };
                for (name, value) in readings.iter_mut() {
                    if let Some((_, coefficient)) = theirs.iter().find(|(key, _)| key == name) {
                        *value += (self.weight_algorithm)(*coefficient, distance);
                    }
                }
            }
            let mut best: Option<&(String, FloatType)> = None;
            for reading in readings.iter() {
                if best.is_none_or(|(_, value)| reading.1 > *value) {
                    best = Some(reading);
                }
            }
            if let Some((name, _)) = best {
                smoothed.push(Key::from_tonic(name)?);
            }
            if !first_time {
                kept[index] = Some(base);
            }
        }
        Ok(smoothed)
    }

    /// Every measure's key, smoothed: music21's `run`.
    ///
    /// # Errors
    ///
    /// As [`KeyAnalyzer::smooth_interpretation_by_measure`].
    pub fn run(&self) -> Result<Vec<Key>> {
        self.smooth_interpretation_by_measure()
    }

    /// The measure at an index of every part, as one stream; for a stream
    /// that holds no parts, its own measure there.
    fn slice(&self, index: usize) -> Stream {
        let mut slice = Stream::new();
        if self.stream.has_part_like_streams() {
            for part in self.stream.parts() {
                if let Some(measure) = measures_of(part).get(index) {
                    slice.insert(0.0, (*measure).clone());
                }
            }
        } else if let Some(measure) = measures_of(self.stream).get(index) {
            slice.insert(0.0, (*measure).clone());
        }
        slice
    }
}

fn measures_of(stream: &Stream) -> Vec<&Stream> {
    stream
        .events()
        .iter()
        .filter_map(|event| event.element().as_stream())
        .filter(|inner| inner.kind() == StreamKind::Measure)
        .collect()
}

fn interpretations(estimates: &[KeyEstimate]) -> Interpretations {
    estimates
        .iter()
        .map(|estimate| {
            (
                estimate.key().tonic_pitch_name_with_case(),
                estimate.score(),
            )
        })
        .collect()
}
