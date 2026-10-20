//! Finding the notes in a recording: music21's `audioSearch`.
//!
//! A recording is cut into chunks of [`AUDIO_CHUNK_LENGTH`] frames; each
//! chunk's frequency is found by autocorrelation
//! ([`autocorrelation_frequency`]) and snapped to the nearest pitch of a
//! scale ([`detect_pitch_frequencies`]). The frequencies are smoothed
//! ([`smooth_frequencies`]), turned into pitches whose octaves are evened
//! out ([`pitch_frequencies_to_objects`]), and runs of one pitch long
//! enough to be a note become notes and rests
//! ([`join_consecutive_identical_pitches`]), whose lengths are quantized
//! against the most common length ([`notes_and_durations_to_stream`]).
//! [`monophonic_stream_from_wav`] does all of it, as music21's
//! `transcriber.monophonicStreamFromFile` does.
//!
//! As music21 does, every recording is taken to be sampled at
//! [`RECORD_SAMPLE_RATE`], whatever its file says, and its samples are read
//! as 16-bit numbers. music21's recording from a microphone and its score
//! follower are not here.

use crate::defaults::{FloatType, IntegerType};
use crate::duration::Duration;
use crate::error::{Error, Result};
use crate::metadata::Metadata;
use crate::note::Note;
use crate::pitch::Pitch;
use crate::rest::Rest;
use crate::scale::{Scale, ScaleType};
use crate::stream::{Stream, StreamElement, StreamKind};

/// How many frames of a recording each frequency is found from: music21's
/// `audioChunkLength`.
pub const AUDIO_CHUNK_LENGTH: usize = 1024;

/// The sample rate every recording is taken to have: music21's
/// `recordSampleRate`.
pub const RECORD_SAMPLE_RATE: FloatType = 44100.0;

/// The frequency standing for a rest.
pub const REST_FREQUENCY: FloatType = 10.0;

fn audio_error(message: impl Into<String>) -> Error {
    Error::Analysis(message.into())
}

fn default_scale(scale: Option<&Scale>) -> Result<Scale> {
    match scale {
        Some(scale) => Ok(scale.clone()),
        None => Ok(Scale::new(ScaleType::Major, Pitch::from_name("C4")?)),
    }
}

/// How many of `data` fall in each of `bins` equal bins from its least to
/// its greatest, and the bins' edges: music21's `histogram`. A value on an
/// edge counts in the bin below it.
///
/// ```
/// use music21_rs::audiosearch::histogram;
///
/// let (counts, edges) = histogram(&[1.0, 2.0, 2.0, 3.0, 5.0], 4)?;
/// assert_eq!(counts, [3, 1, 0, 1]);
/// assert_eq!(edges, [1.0, 2.0, 3.0, 4.0, 5.0]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
///
/// # Errors
///
/// No data, or no bins.
pub fn histogram(data: &[FloatType], bins: usize) -> Result<(Vec<usize>, Vec<FloatType>)> {
    let most = data
        .iter()
        .copied()
        .reduce(FloatType::max)
        .ok_or_else(|| audio_error("max() arg is an empty sequence"))?;
    let least = data.iter().copied().fold(most, FloatType::min);
    let width = (most - least) / bins as FloatType;
    let mut counts = vec![0; bins];
    for value in data {
        let mut count = 1;
        while *value > least + count as FloatType * width {
            count += 1;
        }
        *counts
            .get_mut(count - 1)
            .ok_or_else(|| audio_error("list index out of range"))? += 1;
    }
    let edges = (0..=bins)
        .map(|index| least + index as FloatType * width)
        .collect();
    Ok((counts, edges))
}

/// Where between three points about a peak the top of the parabola
/// through them lies: music21's `interpolation`. A peak at either end
/// is taken as it is.
pub fn interpolation(correlation: &[FloatType], peak: usize) -> FloatType {
    if peak == 0 || peak + 1 == correlation.len() {
        return peak as FloatType;
    }
    let (before, at, after) = (
        correlation[peak - 1],
        correlation[peak],
        correlation[peak + 1],
    );
    (before - after) / (before - 2.0 * at + after) * 0.5 + peak as FloatType
}

/// The frequency a signal sounds, found by autocorrelation: music21's
/// `autocorrelationFunction`. The signal's correlation with itself at
/// each lag is taken; past the first lag where it starts to rise, its
/// highest point, placed between samples by [`interpolation`], is the
/// period. A signal whose correlation never rises is a rest, frequency
/// ten.
///
/// music21 correlates with scipy's FFT convolution, which rounds; the
/// crate sums the products exactly, so the two can differ in the last
/// bits.
///
/// ```
/// use music21_rs::audiosearch::autocorrelation_frequency;
///
/// let rate = 44100.0;
/// let signal: Vec<f64> = (0..1024)
///     .map(|i| (2.0 * std::f64::consts::PI * 441.0 * i as f64 / rate).sin() * 10000.0)
///     .collect();
/// let frequency = autocorrelation_frequency(&signal, rate)?;
/// // music21 finds 442.2033...: the interpolated peak sits a little early.
/// assert!((frequency - 442.2033).abs() < 1e-3);
/// # Ok::<(), music21_rs::Error>(())
/// ```
///
/// # Errors
///
/// An empty signal.
pub fn autocorrelation_frequency(
    signal: &[FloatType],
    sample_rate: FloatType,
) -> Result<FloatType> {
    if signal.is_empty() {
        return Err(audio_error("v cannot be empty"));
    }
    let length = signal.len();
    let correlation: Vec<FloatType> = (0..length)
        .map(|lag| {
            signal[..length - lag]
                .iter()
                .zip(&signal[lag..])
                .map(|(a, b)| a * b)
                .sum()
        })
        .collect();
    let Some(beginning) = correlation
        .windows(2)
        .position(|pair| pair[1] - pair[0] > 0.0)
    else {
        return Ok(REST_FREQUENCY);
    };
    // numpy's argmax: the first of equal highs.
    let peak = correlation[beginning..]
        .iter()
        .enumerate()
        .fold((0, FloatType::NEG_INFINITY), |best, (index, value)| {
            if *value > best.1 {
                (index, *value)
            } else {
                best
            }
        })
        .0
        + beginning;
    Ok(sample_rate / interpolation(&correlation, peak))
}

/// The thresholds between a scale's pitches, as fractions of an octave
/// above the octave's C, and the pitches: music21's `prepareThresholds`.
/// The scale is the chromatic scale on C4 where none is given.
///
/// # Errors
///
/// A scale whose pitches cannot be worked out.
pub fn prepare_thresholds(scale: Option<&Scale>) -> Result<(Vec<FloatType>, Vec<Pitch>)> {
    let scale = match scale {
        Some(scale) => scale.clone(),
        None => Scale::new(ScaleType::Chromatic, Pitch::from_name("C4")?),
    };
    let pitches = scale.pitches()?;
    let mut remainders: Vec<FloatType> = pitches
        .iter()
        .map(|pitch| pitch.frequency_hz().log2().fract())
        .collect();
    if let Some(last) = remainders.last_mut() {
        *last += 1.0;
    }
    let thresholds = remainders
        .windows(2)
        .map(|pair| (pair[0] + pair[1]) / 2.0)
        .collect();
    Ok((thresholds, pitches))
}

/// The scale pitch a frequency falls nearest, put in the octave it sounds
/// in, and the frequency of that pitch as the scale has it: music21's
/// `normalizeInputFrequency`, with thresholds and pitches from
/// [`prepare_thresholds`].
///
/// # Errors
///
/// A frequency that is not a finite number above nought, or no pitches.
pub fn normalize_input_frequency(
    frequency: FloatType,
    thresholds: &[FloatType],
    pitches: &[Pitch],
) -> Result<(FloatType, Pitch)> {
    let log = frequency.log2();
    if !log.is_finite() {
        return Err(audio_error(format!(
            "cannot place a frequency of {frequency}"
        )));
    }
    let (remainder, octave) = (log.fract(), log.trunc() as IntegerType);
    let (index, shift) = match thresholds
        .iter()
        .position(|threshold| remainder < *threshold)
    {
        Some(index) => (index, 4),
        None => (pitches.len().wrapping_sub(1), 3),
    };
    let scale_pitch = pitches
        .get(index)
        .ok_or_else(|| audio_error("list index out of range"))?;
    let mut placed = scale_pitch.clone();
    placed.set_octave(Some(octave - shift));
    Ok((scale_pitch.frequency_hz(), placed))
}

/// Each frequency's nearest pitch of `scale` (C major on C4 where none is
/// given), and the frequency of that pitch in the octave it sounds:
/// music21's `detectPitchFrequencies`.
///
/// # Errors
///
/// As [`normalize_input_frequency`].
pub fn detect_pitch_frequencies(
    frequencies: &[FloatType],
    scale: Option<&Scale>,
) -> Result<Vec<FloatType>> {
    let (thresholds, pitches) = prepare_thresholds(Some(&default_scale(scale)?))?;
    frequencies
        .iter()
        .map(|frequency| {
            Ok(
                normalize_input_frequency(*frequency, &thresholds, &pitches)?
                    .1
                    .frequency_hz(),
            )
        })
        .collect()
}

/// Each frequency averaged with its neighbours, `smooth_levels` at a time,
/// and rounded to a whole number: music21's `smoothFrequencies`. As in
/// music21, each average takes in those already made before it, and the
/// first and last few frequencies are the truncated average of the first
/// and last `smooth_levels`.
///
/// ```
/// use music21_rs::audiosearch::smooth_frequencies;
///
/// let smoothed = smooth_frequencies(&[100.0, 100.0, 400.0, 100.0, 100.0], 3)?;
/// // Each average takes in the ones made before it.
/// assert_eq!(smoothed, [200, 233, 244, 200, 200]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
///
/// # Errors
///
/// No levels, or more levels than frequencies.
pub fn smooth_frequencies(frequencies: &[FloatType], smooth_levels: usize) -> Result<Vec<i64>> {
    // music21 raises a `ValueError` for both of these rather than its own
    // class: they are arguments that were never values, not music that
    // cannot be read.
    if smooth_levels < 1 {
        return Err(Error::Value("smoothLevels must be >= 1".to_string()));
    }
    let count = frequencies.len();
    if smooth_levels > count {
        return Err(Error::Value(format!(
            "There cannot be more smoothLevels ({smooth_levels}) than inputPitches ({count})"
        )));
    }
    let mut smoothed = frequencies.to_vec();
    let levels = smooth_levels as FloatType;
    let beginning = smoothed[..smooth_levels].iter().sum::<FloatType>() / levels;
    let ends = (0..smooth_levels)
        .map(|i| smoothed[count - 1 - i])
        .sum::<FloatType>()
        / levels;
    let half_down = smooth_levels / 2;
    let half_up = smooth_levels.div_ceil(2);
    for i in 0..count {
        smoothed[i] = if i < half_down {
            beginning.trunc()
        } else if (i as i64) > count as i64 - half_up as i64 - 1 {
            ends.trunc()
        } else {
            (0..smooth_levels)
                .map(|j| smoothed[i + j - half_down])
                .sum::<FloatType>()
                / levels
        };
    }
    Ok(smoothed
        .into_iter()
        .map(|value| value.round_ties_even() as i64)
        .collect())
}

/// The pitches of a list of frequencies, each the nearest pitch of `scale`
/// (C major on C4 where none is given), with each run of one name put in
/// the run's average octave, and the frequencies so placed: music21's
/// `pitchFrequenciesToObjects`.
///
/// As in music21, a run's octave goes to the pitches from the one before
/// it to the one before its last, the very first run's to the last pitch,
/// and the last pitch is never counted in a run.
///
/// # Errors
///
/// As [`normalize_input_frequency`].
pub fn pitch_frequencies_to_objects(
    frequencies: &[FloatType],
    scale: Option<&Scale>,
) -> Result<(Vec<Pitch>, Vec<FloatType>)> {
    let (thresholds, pitches) = prepare_thresholds(Some(&default_scale(scale)?))?;
    let mut placed: Vec<Pitch> = frequencies
        .iter()
        .map(|frequency| Ok(normalize_input_frequency(*frequency, &thresholds, &pitches)?.1))
        .collect::<Result<_>>()?;
    let count = placed.len();
    let mut plotted = Vec::new();
    let mut i = 0;
    while i + 1 < count {
        let name = placed[i].name();
        let hold = i;
        let mut total: IntegerType = 0;
        while i + 1 < count && placed[i].name() == name {
            total += placed[i].octave().unwrap_or(4);
            i += 1;
        }
        let octave =
            (total as FloatType / (i - hold) as FloatType).round_ties_even() as IntegerType;
        for j in 0..i - hold {
            // Python's index one before, the last for the first.
            let index = (hold + j + count - 1) % count;
            placed[index].set_octave(Some(octave));
            plotted.push(placed[index].frequency_hz());
        }
    }
    Ok((placed, plotted))
}

/// The pitch music21 makes by setting a pitch's frequency.
fn pitch_at_frequency(hertz: FloatType) -> Result<Pitch> {
    let space = 12.0 * ((hertz / 440.0).ln() / (2.0 as FloatType).ln()) + 69.0;
    Pitch::from_pitch_space((space * 1e6).round_ties_even() / 1e6)
}

/// One note or rest a run of pitches makes: the by-position twin of
/// [`join_consecutive_identical_pitches`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct JoinedNote {
    /// Which of the pitches it sounds, or none where it is a rest.
    pub pitch: Option<usize>,
    /// How many pitches it stands for.
    pub length: usize,
}

/// What each run of one pitch makes, as the places of the pitches rather
/// than as notes: the by-position twin of
/// [`join_consecutive_identical_pitches`].
///
/// music21 hands the note it makes the very `Pitch` object of the run's
/// last pitch, so a caller holding those objects wants to know which.
///
/// # Errors
///
/// No pitches.
pub fn joined_pitches(pitches: &[Pitch]) -> Result<Vec<JoinedNote>> {
    let mut read = pitches.to_vec();
    let first = read
        .first_mut()
        .ok_or_else(|| audio_error("list index out of range"))?;
    *first = pitch_at_frequency(REST_FREQUENCY)?;
    let mut joined = Vec::new();
    let (mut good, mut bad) = (0, 0);
    let mut j = 0;
    while j < read.len() {
        let frequency = read[j].frequency_hz();
        let mut valid = false;
        while j < read.len() && read[j].frequency_hz() == frequency {
            good += 1;
            if good >= 6 {
                valid = true;
                if bad >= 15 {
                    joined.push(JoinedNote {
                        pitch: None,
                        length: bad,
                    });
                }
                bad = 0;
            }
            j += 1;
        }
        if valid {
            joined.push(JoinedNote {
                pitch: Some(j - 1),
                length: good,
            });
        } else {
            bad += good;
        }
        good = 0;
        j += 1;
    }
    Ok(joined)
}

/// Notes and rests from runs of one pitch, and how many pitches each
/// lasted: music21's `joinConsecutiveIdenticalPitches`. A run of six or
/// more is a note of its last pitch; fifteen or more pitches in shorter
/// runs before a note make a rest. The first pitch is taken as a rest's,
/// and as in music21 the first pitch after each run is passed over.
///
/// # Errors
///
/// No pitches.
pub fn join_consecutive_identical_pitches(
    mut pitches: Vec<Pitch>,
) -> Result<(Vec<StreamElement>, Vec<usize>)> {
    let first = pitches
        .first_mut()
        .ok_or_else(|| audio_error("list index out of range"))?;
    *first = pitch_at_frequency(REST_FREQUENCY)?;
    let mut notes: Vec<StreamElement> = Vec::new();
    let mut lengths = Vec::new();
    for one in joined_pitches(&pitches)? {
        lengths.push(one.length);
        notes.push(match one.pitch {
            Some(place) => Note::from_pitch(pitches[place].clone()).into(),
            None => Rest::new(Duration::quarter()).into(),
        });
    }
    Ok((notes, lengths))
}

/// A length, in quarters, snapped to a sixteenth, eighth, quarter, dotted
/// quarter, half or whole: music21's `quantizeDuration`.
pub fn quantize_duration(length: FloatType) -> FloatType {
    let length = length * 100.0;
    let typical = [25.0, 50.0, 100.0, 150.0, 200.0, 400.0];
    let mut chosen = typical[0];
    for (index, pair) in typical.windows(2).enumerate() {
        if length > (pair[0] + pair[1]) / 2.0 {
            chosen = typical[index + 1];
        }
    }
    chosen / 100.0
}

/// How many pitches a quarter note lasts, from how many each note lasted:
/// the middle of the last of the fullest bins of an eight-bin
/// [`histogram`] of the lengths and nought, taken as
/// `most_repeated_quarter_length` (a quarter where nought): music21's
/// `quarterLengthEstimation`.
///
/// # Errors
///
/// As [`histogram`].
pub fn quarter_length_estimation(
    lengths: &[usize],
    most_repeated_quarter_length: FloatType,
) -> Result<FloatType> {
    let mut data: Vec<FloatType> = lengths.iter().map(|length| *length as FloatType).collect();
    data.push(0.0);
    let (counts, edges) = histogram(&data, 8)?;
    let fullest = counts.iter().copied().max().unwrap_or(0);
    let index = counts
        .iter()
        .rposition(|count| *count == fullest)
        .unwrap_or(0);
    let estimate = (edges[index] + edges[index + 1]) / 2.0;
    let most_repeated = if most_repeated_quarter_length == 0.0 {
        1.0
    } else {
        most_repeated_quarter_length
    };
    Ok(estimate * (2.0 as FloatType).powf(0.0 - most_repeated.log2()))
}

/// What one note of a transcription is given: the by-position twin of
/// [`notes_and_durations_to_stream`].
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TranscribedNote {
    /// Which of the notes given it is.
    pub note: usize,
    /// How long it is to last, in quarters.
    pub quarter_length: FloatType,
    /// Whether it goes into the part: a rest before the first note is
    /// left out where the caller asked for that.
    pub kept: bool,
}

/// How long each note of a transcription is to last and which of them go
/// into the part, and the quarter length estimated: the by-position twin
/// of [`notes_and_durations_to_stream`].
///
/// music21 writes the length onto *every* note it is given, the rests it
/// then leaves out included, which is why each note is answered for
/// whether it is kept or not.
///
/// # Errors
///
/// Fewer notes than lengths, or no lengths.
pub fn transcribed_notes(
    notes: &[StreamElement],
    lengths: &[usize],
    score_notes: Option<&Stream>,
    remove_rests_at_beginning: bool,
) -> Result<(Vec<TranscribedNote>, FloatType)> {
    let most_common = match score_notes {
        Some(score) => crate::features::vector_by_id(score, "QL2")?
            .and_then(|vector| vector.first().map(|value| value.as_float()))
            .unwrap_or(1.0),
        None => 1.0,
    };
    let estimate = quarter_length_estimation(lengths, most_common)?;
    let mut out = Vec::with_capacity(lengths.len());
    let mut skipping = remove_rests_at_beginning;
    for (index, length) in lengths.iter().enumerate() {
        let element = notes
            .get(index)
            .ok_or_else(|| audio_error("list index out of range"))?;
        let quarter_length = quantize_duration(*length as FloatType / estimate);
        let kept = !(skipping && matches!(element, StreamElement::Rest(_)));
        if kept {
            skipping = false;
        }
        out.push(TranscribedNote {
            note: index,
            quarter_length,
            kept,
        });
    }
    Ok((out, estimate))
}

/// A score of one part holding `notes`, each as long as its length in
/// [`quantize_duration`]d quarters of the estimated quarter length, and that
/// estimate: music21's `notesAndDurationsToStream`. The estimate is
/// [`quarter_length_estimation`] against the most common note length of
/// `score_notes` where given, or against a quarter. With
/// `remove_rests_at_beginning`, rests before the first note are left out.
/// The score is titled `Automatic Music21 Transcription`.
///
/// # Errors
///
/// Fewer notes than lengths, or no lengths.
pub fn notes_and_durations_to_stream(
    notes: &[StreamElement],
    lengths: &[usize],
    score_notes: Option<&Stream>,
    remove_rests_at_beginning: bool,
) -> Result<(Stream, FloatType)> {
    let (given, estimate) =
        transcribed_notes(notes, lengths, score_notes, remove_rests_at_beginning)?;
    let mut part = Stream::with_kind(StreamKind::Part);
    for one in &given {
        let mut element = notes
            .get(one.note)
            .cloned()
            .ok_or_else(|| audio_error("list index out of range"))?;
        let duration = Duration::new(one.quarter_length)?;
        match &mut element {
            StreamElement::Note(note) => note.set_duration(duration),
            StreamElement::Rest(rest) => rest.set_duration(duration),
            _ => {}
        }
        if one.kept {
            part.push(element);
        }
    }
    let mut score = Stream::with_kind(StreamKind::Score);
    let mut metadata = Metadata::default();
    metadata.add_text("title", "Automatic Music21 Transcription");
    score.set_metadata(Some(metadata));
    score.insert(0.0, part);
    Ok((score, estimate))
}

/// Which of the parts a score follower is weighing to take as where the
/// player is, and the count of doubtful guesses so far: music21's
/// `decisionProcess`. Each part is its index into `beginning_data`, where
/// it begins, and how well it matched; parts are in order of how well they
/// matched. Of the parts matching within nine tenths of the best and
/// beginning after `last_note_position`, the one beginning nearest
/// `note_prediction` is taken; the countdown grows when the guess is
/// doubtful and is otherwise reset.
///
/// # Errors
///
/// No parts, a part beginning where `beginning_data` has nothing, or every
/// part beginning before the prediction with none taken instead.
pub fn decision_process(
    parts: &[(usize, FloatType)],
    note_prediction: FloatType,
    beginning_data: &[FloatType],
    last_note_position: FloatType,
    countdown: usize,
    page: Option<(FloatType, FloatType)>,
) -> Result<(usize, usize)> {
    let begins = |part: usize| -> Result<FloatType> {
        let id = parts
            .get(part)
            .ok_or_else(|| audio_error("list index out of range"))?
            .0;
        beginning_data
            .get(id)
            .copied()
            .ok_or_else(|| audio_error("list index out of range"))
    };
    let mut position = 0;
    let mut i = 0;
    while i < parts.len() && begins(i)? < note_prediction {
        i += 1;
        position = i;
    }
    if parts.len() == 1 {
        position = 0;
    }
    let first = beginning_data
        .first()
        .copied()
        .ok_or_else(|| audio_error("list index out of range"))?;
    let mut distance = (first - note_prediction).abs();
    let best = parts
        .first()
        .ok_or_else(|| audio_error("list index out of range"))?
        .1;
    for (index, (_, probability)) in parts.iter().enumerate() {
        let start = begins(index)?;
        if *probability >= 0.9 * best
            && start > last_note_position
            && (start - note_prediction).abs() < distance
        {
            distance = (start - note_prediction).abs();
            position = index;
        }
    }
    let start = begins(position)?;
    let probability = parts[position].1;
    let doubtful = probability < 0.6
        || parts.len() == 1
        || (distance > 20.0 && countdown == 0)
        || (distance > 30.0 && countdown == 1)
        || page.is_some_and(|(first, last)| (start < first || start > last) && countdown < 2);
    Ok((position, if doubtful { countdown + 1 } else { 0 }))
}

/// The 16-bit samples of a WAV file and how many frames it holds, read as
/// Python's `wave` module reads an uncompressed one.
fn wav_samples(bytes: &[u8]) -> Result<(Vec<u8>, usize, usize)> {
    let word = |at: usize| -> Result<u32> {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| audio_error("file does not start with RIFF id"))
    };
    if bytes.get(..4) != Some(b"RIFF") || bytes.get(8..12) != Some(b"WAVE") {
        return Err(audio_error("file does not start with RIFF id"));
    }
    let mut at = 12;
    let mut frame_size = None;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = word(at + 4)? as usize;
        let body = at + 8;
        let end = (body + size).min(bytes.len());
        match id {
            b"fmt " => {
                let field = |offset: usize| -> Result<u16> {
                    bytes
                        .get(body + offset..body + offset + 2)
                        .map(|b| u16::from_le_bytes([b[0], b[1]]))
                        .ok_or_else(|| audio_error("fmt chunk too short"))
                };
                let format = field(0)?;
                if format != 1 && format != 0xFFFE {
                    return Err(audio_error(format!("unknown format: {format}")));
                }
                let channels = usize::from(field(2)?);
                let width = usize::from(field(14)?).div_ceil(8);
                if channels == 0 || width == 0 {
                    return Err(audio_error("bad # of channels or sample width"));
                }
                frame_size = Some(channels * width);
            }
            b"data" => {
                let frame_size =
                    frame_size.ok_or_else(|| audio_error("data chunk before fmt chunk"))?;
                let data = bytes[body..end].to_vec();
                let frames = size / frame_size;
                return Ok((data, frames, frame_size));
            }
            _ => {}
        }
        at = body + size + (size & 1);
    }
    Err(audio_error("fmt chunk and/or data chunk missing"))
}

/// The frequency of each whole chunk of [`AUDIO_CHUNK_LENGTH`] frames of a
/// WAV file, found by [`autocorrelation_frequency`]: music21's
/// `getFrequenciesFromAudioFile`. Each chunk's bytes are read as 16-bit
/// samples, every channel's in turn, and taken to be sampled at
/// [`RECORD_SAMPLE_RATE`]; frames after the last whole chunk are left out.
///
/// # Errors
///
/// A file that is not an uncompressed WAV file.
pub fn frequencies_from_wav(bytes: &[u8]) -> Result<Vec<FloatType>> {
    let (data, frames, frame_size) = wav_samples(bytes)?;
    let chunk_bytes = AUDIO_CHUNK_LENGTH * frame_size;
    (0..frames / AUDIO_CHUNK_LENGTH)
        .map(|chunk| {
            let samples: Vec<FloatType> = data[chunk * chunk_bytes..(chunk + 1) * chunk_bytes]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| FloatType::from(i16::from_le_bytes(*pair)))
                .collect();
            autocorrelation_frequency(&samples, RECORD_SAMPLE_RATE)
        })
        .collect()
}

/// The notes of a recording of one line, as a part: music21's
/// `transcriber.monophonicStreamFromFile`, which runs
/// [`frequencies_from_wav`], [`detect_pitch_frequencies`],
/// [`smooth_frequencies`] by seven, [`pitch_frequencies_to_objects`],
/// [`join_consecutive_identical_pitches`] and
/// [`notes_and_durations_to_stream`], rests at the beginning left out.
/// Pitches are taken from `scale`, C major on C4 where none is given.
///
/// # Errors
///
/// A file that is not an uncompressed WAV file, or one too short to
/// smooth or to hold a note.
pub fn monophonic_stream_from_wav(bytes: &[u8], scale: Option<&Scale>) -> Result<Stream> {
    let frequencies = frequencies_from_wav(bytes)?;
    let detected = detect_pitch_frequencies(&frequencies, scale)?;
    let smoothed: Vec<FloatType> = smooth_frequencies(&detected, 7)?
        .into_iter()
        .map(|value| value as FloatType)
        .collect();
    let (pitches, _) = pitch_frequencies_to_objects(&smoothed, scale)?;
    let (notes, lengths) = join_consecutive_identical_pitches(pitches)?;
    let (score, _) = notes_and_durations_to_stream(&notes, &lengths, None, true)?;
    score
        .parts()
        .first()
        .map(|part| (*part).clone())
        .ok_or_else(|| audio_error("no part"))
}
