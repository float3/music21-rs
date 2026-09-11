//! Hears the notes in microphone audio and follows them over time, so the
//! chord listener page can name a chord whether its notes sound together or
//! one after another.
//!
//! Each frame is windowed and transformed, and its spectral peaks found. A
//! fundamental is estimated by summing the harmonic amplitudes above every
//! plausible guess: the guess whose partials weigh the most is taken, its
//! partials are cancelled up to a smooth spectral envelope, so that a note
//! sharing a partial keeps what it adds on top, and the search repeats until
//! what is left is too weak to be a note. A guess only stands when its own
//! fundamental is still there, which keeps the upper partials of one note
//! from being heard as notes of their own.

use crate::display_pitch_name;
use music21_rs::{Chord, pitch_class_name};
use serde::Serialize;
use std::{collections::BTreeMap, f64::consts::PI};
use wasm_bindgen::prelude::*;

/// The lowest note listened for, C2.
pub(crate) const LOWEST_MIDI: i32 = 36;
/// The highest note listened for, C7.
pub(crate) const HIGHEST_MIDI: i32 = 96;
const HIGHEST_PARTIAL_HZ: f64 = 7000.0;
const MAX_HARMONIC: usize = 16;
const GUESS_PEAKS: usize = 32;
const GUESS_HARMONICS: usize = 6;
const MAX_NOTES: usize = 8;
const FLOOR_PERCENTILE: f64 = 0.2;
const PEAK_OVER_FLOOR: f64 = 4.0;
const PEAK_RANGE: f64 = 1e-3;
const SIDELOBE_RATIO: f64 = 25.0;
const HARMONIC_ROLLOFF: f64 = 0.2;
const FUNDAMENTAL_SHARE: f64 = 0.1;
const FUNDAMENTAL_FLOOR: f64 = 0.1;
const ONSET_RATIO: f64 = 4.0;

fn midi_frequency(midi: f64) -> f64 {
    440.0 * 2f64.powf((midi - 69.0) / 12.0)
}

fn frequency_midi(frequency: f64) -> f64 {
    69.0 + 12.0 * (frequency / 440.0).log2()
}

fn cents_between(from: f64, to: f64) -> f64 {
    1200.0 * (to / from).log2()
}

/// A note found in one frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct HeardNote {
    pub midi: i32,
    pub frequency_hz: f64,
    pub cents: f64,
    pub strength: f64,
}

/// What one frame of audio held: its level, the notes found in it, and
/// whether an attack is still passing through it, which smears the spectrum
/// of the notes just struck.
#[derive(Debug)]
pub(crate) struct Heard {
    pub level_db: f64,
    pub notes: Vec<HeardNote>,
    pub settling: bool,
}

#[derive(Clone, Copy, Debug)]
struct Peak {
    frequency: f64,
    amplitude: f64,
}

struct Guess {
    frequency: f64,
    midi: i32,
    partials: Vec<(usize, usize)>,
}

/// Finds notes in frames of audio, keeping its buffers between frames.
pub(crate) struct Analyzer {
    sample_rate: f64,
    window: Vec<f64>,
    real: Vec<f64>,
    imaginary: Vec<f64>,
    magnitudes: Vec<f64>,
    scratch: Vec<f64>,
    cosines: Vec<f64>,
    sines: Vec<f64>,
}

impl Analyzer {
    pub(crate) fn new(sample_rate: f64) -> Self {
        Self {
            sample_rate,
            window: Vec::new(),
            real: Vec::new(),
            imaginary: Vec::new(),
            magnitudes: Vec::new(),
            scratch: Vec::new(),
            cosines: Vec::new(),
            sines: Vec::new(),
        }
    }

    fn resize(&mut self, size: usize) {
        if self.window.len() == size {
            return;
        }
        self.window = (0..size)
            .map(|index| 0.5 - 0.5 * (2.0 * PI * index as f64 / size as f64).cos())
            .collect();
        self.real = vec![0.0; size];
        self.imaginary = vec![0.0; size];
        self.magnitudes = vec![0.0; size / 2];
        (self.cosines, self.sines) = (0..size / 2)
            .map(|index| (-2.0 * PI * index as f64 / size as f64).sin_cos())
            .map(|(sin, cos)| (cos, sin))
            .unzip();
    }

    /// Finds the notes in the latest power-of-two stretch of `samples`.
    ///
    /// `threshold` is how strong a further note must be against the
    /// strongest, and nothing is heard in a frame quieter than `gate_db`.
    pub(crate) fn hear(&mut self, samples: &[f32], threshold: f64, gate_db: f64) -> Heard {
        let level_db = level_db(samples);
        if samples.len() < 1024 || level_db < gate_db {
            return Heard {
                level_db,
                notes: Vec::new(),
                settling: false,
            };
        }
        let size = 1usize << samples.len().ilog2();
        self.resize(size);
        let frame = &samples[samples.len() - size..];
        let (older, newer) = frame.split_at(size / 2);
        let settling = energy(newer) > ONSET_RATIO * energy(older);
        for ((real, imaginary), (&sample, &weight)) in self
            .real
            .iter_mut()
            .zip(self.imaginary.iter_mut())
            .zip(frame.iter().zip(&self.window))
        {
            *real = f64::from(sample) * weight;
            *imaginary = 0.0;
        }
        fft(
            &mut self.real,
            &mut self.imaginary,
            &self.cosines,
            &self.sines,
        );
        let scale = 4.0 / size as f64;
        for (magnitude, (real, imaginary)) in self
            .magnitudes
            .iter_mut()
            .zip(self.real.iter().zip(&self.imaginary))
        {
            *magnitude = real.hypot(*imaginary) * scale;
        }
        let bin_hz = self.sample_rate / size as f64;
        let peaks = self.peaks(bin_hz);
        Heard {
            level_db,
            notes: estimate(&peaks, bin_hz, threshold),
            settling,
        }
    }

    fn peaks(&mut self, bin_hz: f64) -> Vec<Peak> {
        let lowest_hz = midi_frequency(f64::from(LOWEST_MIDI) - 0.5);
        let first = ((lowest_hz * 0.9 / bin_hz).floor() as usize).max(4);
        let last = ((HIGHEST_PARTIAL_HZ / bin_hz).ceil() as usize)
            .min(self.magnitudes.len().saturating_sub(5));
        if last <= first + 8 {
            return Vec::new();
        }

        let mut bands = Vec::new();
        let mut start = first;
        while start < last {
            let end = (start + (start * 41 / 100).max(48)).min(last);
            self.scratch.clear();
            self.scratch.extend_from_slice(&self.magnitudes[start..end]);
            let rank = ((self.scratch.len() as f64 * FLOOR_PERCENTILE) as usize)
                .min(self.scratch.len() - 1);
            let (_, floor, _) = self.scratch.select_nth_unstable_by(rank, f64::total_cmp);
            bands.push((end, *floor));
            start = end;
        }

        let magnitudes = &self.magnitudes;
        let loudest = magnitudes[first..last].iter().copied().fold(0.0, f64::max);
        let mut band = 0;
        let mut peaks = Vec::new();
        for bin in first..last {
            while bands[band].0 <= bin {
                band += 1;
            }
            let magnitude = magnitudes[bin];
            if magnitude <= magnitudes[bin - 1]
                || magnitude < magnitudes[bin + 1]
                || magnitude <= bands[band].1 * PEAK_OVER_FLOOR
                || magnitude <= loudest * PEAK_RANGE
            {
                continue;
            }
            let neighbourhood = magnitudes[bin - 4..=bin + 4]
                .iter()
                .copied()
                .fold(0.0, f64::max);
            if neighbourhood > magnitude * SIDELOBE_RATIO {
                continue;
            }
            let (left, centre, right) = (
                magnitudes[bin - 1].max(f64::MIN_POSITIVE).ln(),
                magnitude.ln(),
                magnitudes[bin + 1].max(f64::MIN_POSITIVE).ln(),
            );
            let curvature = left - 2.0 * centre + right;
            let offset = if curvature < 0.0 {
                (0.5 * (left - right) / curvature).clamp(-0.5, 0.5)
            } else {
                0.0
            };
            peaks.push(Peak {
                frequency: (bin as f64 + offset) * bin_hz,
                amplitude: (centre - 0.25 * (left - right) * offset).exp(),
            });
        }
        peaks
    }
}

fn energy(samples: &[f32]) -> f64 {
    samples
        .iter()
        .map(|&sample| f64::from(sample) * f64::from(sample))
        .sum()
}

/// The level of `samples` in decibels relative to full scale.
pub(crate) fn level_db(samples: &[f32]) -> f64 {
    if samples.is_empty() {
        return f64::NEG_INFINITY;
    }
    let power = samples
        .iter()
        .map(|&sample| f64::from(sample) * f64::from(sample))
        .sum::<f64>()
        / samples.len() as f64;
    10.0 * power.max(1e-20).log10()
}

/// How strong a further note must be against the strongest, from a
/// sensitivity between zero and one.
pub(crate) fn threshold(sensitivity: f64) -> f64 {
    0.42 - 0.4 * sensitivity.clamp(0.0, 1.0)
}

fn fft(real: &mut [f64], imaginary: &mut [f64], cosines: &[f64], sines: &[f64]) {
    let size = real.len();
    let mut swap = 0;
    for index in 1..size {
        let mut bit = size >> 1;
        while swap & bit != 0 {
            swap ^= bit;
            bit >>= 1;
        }
        swap |= bit;
        if index < swap {
            real.swap(index, swap);
            imaginary.swap(index, swap);
        }
    }
    let mut length = 2;
    while length <= size {
        let half = length / 2;
        let stride = size / length;
        for start in (0..size).step_by(length) {
            for offset in 0..half {
                let (cos, sin) = (cosines[offset * stride], sines[offset * stride]);
                let (near, far) = (start + offset, start + offset + half);
                let turned_real = real[far] * cos - imaginary[far] * sin;
                let turned_imaginary = real[far] * sin + imaginary[far] * cos;
                real[far] = real[near] - turned_real;
                imaginary[far] = imaginary[near] - turned_imaginary;
                real[near] += turned_real;
                imaginary[near] += turned_imaginary;
            }
        }
        length <<= 1;
    }
}

fn nearest_peak(peaks: &[Peak], frequency: f64, tolerance: f64) -> Option<usize> {
    let start = peaks.partition_point(|peak| peak.frequency < frequency - tolerance);
    peaks[start..]
        .iter()
        .take_while(|peak| peak.frequency <= frequency + tolerance)
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            (left.frequency - frequency)
                .abs()
                .total_cmp(&(right.frequency - frequency).abs())
        })
        .map(|(offset, _)| start + offset)
}

fn match_partials(peaks: &[Peak], guess: f64, bin_hz: f64) -> Option<Guess> {
    let mut partials = Vec::new();
    let (mut weighted, mut total) = (0.0, 0.0);
    for harmonic in 1..=MAX_HARMONIC {
        let fundamental = if total > 0.0 { weighted / total } else { guess };
        let expected = fundamental * harmonic as f64;
        if expected > HIGHEST_PARTIAL_HZ {
            break;
        }
        let spread = if harmonic == 1 { 0.03 } else { 0.02 };
        match nearest_peak(peaks, expected, (expected * spread).max(bin_hz * 0.75)) {
            Some(index) => {
                partials.push((index, harmonic));
                let weight = harmonic as f64;
                weighted += peaks[index].frequency / harmonic as f64 * weight;
                total += weight;
            }
            None if harmonic == 1 => return None,
            None => {}
        }
    }
    let frequency = weighted / total;
    let midi = frequency_midi(frequency).round() as i32;
    (LOWEST_MIDI..=HIGHEST_MIDI)
        .contains(&midi)
        .then_some(Guess {
            frequency,
            midi,
            partials,
        })
}

fn harmonic_weight(harmonic: usize) -> f64 {
    (harmonic as f64).powf(-HARMONIC_ROLLOFF)
}

fn estimate(peaks: &[Peak], bin_hz: f64, threshold: f64) -> Vec<HeardNote> {
    let weights = peaks
        .iter()
        .map(|peak| peak.amplitude.sqrt())
        .collect::<Vec<_>>();
    let mut loudest = (0..peaks.len()).collect::<Vec<_>>();
    loudest.sort_by(|&left, &right| weights[right].total_cmp(&weights[left]));

    let lowest_hz = midi_frequency(f64::from(LOWEST_MIDI) - 0.5);
    let mut guesses: Vec<Guess> = Vec::new();
    for &index in loudest.iter().take(GUESS_PEAKS) {
        for harmonic in 1..=GUESS_HARMONICS {
            let frequency = peaks[index].frequency / harmonic as f64;
            if frequency < lowest_hz {
                break;
            }
            if let Some(guess) = match_partials(peaks, frequency, bin_hz)
                && guesses
                    .iter()
                    .all(|other| cents_between(other.frequency, guess.frequency).abs() > 25.0)
            {
                guesses.push(guess);
            }
        }
    }

    let mut residual = weights.clone();
    let mut notes: Vec<HeardNote> = Vec::new();
    let mut strongest = None;
    while notes.len() < MAX_NOTES {
        let best = guesses
            .iter()
            .filter(|guess| notes.iter().all(|note| note.midi != guess.midi))
            .filter(|guess| {
                let (fundamental, _) = guess.partials[0];
                let loudest_partial = guess
                    .partials
                    .iter()
                    .map(|&(index, _)| weights[index])
                    .fold(0.0, f64::max);
                residual[fundamental] >= FUNDAMENTAL_SHARE * weights[fundamental]
                    && weights[fundamental] >= FUNDAMENTAL_FLOOR * loudest_partial
            })
            .map(|guess| {
                let score = guess
                    .partials
                    .iter()
                    .map(|&(index, harmonic)| residual[index] * harmonic_weight(harmonic))
                    .sum::<f64>();
                (guess, score)
            })
            .max_by(|(_, left), (_, right)| left.total_cmp(right));
        let Some((guess, score)) = best else {
            break;
        };
        let strongest = *strongest.get_or_insert(score);
        if score <= 0.0 || score < strongest * threshold {
            break;
        }
        for &(index, harmonic) in &guess.partials {
            let envelope = |step: usize| {
                let (sum, count) = guess
                    .partials
                    .iter()
                    .filter(|(_, other)| other.abs_diff(harmonic) == step)
                    .fold((0.0, 0.0), |(sum, count), &(other, _)| {
                        (sum + weights[other].ln(), count + 1.0)
                    });
                (count > 0.0).then(|| (sum / count).exp())
            };
            let limit = match (envelope(1), envelope(2)) {
                (None, None) => weights[index],
                (near, far) => near.unwrap_or(0.0).max(far.unwrap_or(0.0)),
            };
            residual[index] = (residual[index] - limit).max(0.0);
        }
        notes.push(HeardNote {
            midi: guess.midi,
            frequency_hz: guess.frequency,
            cents: cents_between(midi_frequency(f64::from(guess.midi)), guess.frequency),
            strength: score / strongest,
        });
    }
    notes.sort_by_key(|note| note.midi);
    notes
}

#[derive(Clone, Copy, Debug)]
struct Tracked {
    heard_since: f64,
    last_heard: f64,
    sounding: bool,
    confirmed: bool,
    frequency_hz: f64,
    cents: f64,
    strength: f64,
    onset: f64,
    held: bool,
}

/// A note the tracker holds: sounding now, or heard within the hold time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct HeldNote {
    pub midi: i32,
    pub frequency_hz: f64,
    pub cents: f64,
    pub strength: f64,
    pub sounding: bool,
}

/// Follows heard notes from frame to frame. A note sounds once it has been
/// heard for `ON_MS`, stops once it has gone unheard for `OFF_MS`, and is
/// held for `hold_ms` after that, which is what gathers an arpeggio's notes
/// into one chord.
///
/// A new chord lets go of the notes before it, which are no longer held once
/// they stop sounding: a note that starts after `PHRASE_GAP_MS` with nothing
/// sounding, or notes that start within `TOGETHER_MS` of each other. While
/// an attack is settling, only the notes already sounding are heard, so the
/// smeared first moments of a chord start no notes of their own.
#[derive(Default)]
pub(crate) struct Tracker {
    notes: BTreeMap<i32, Tracked>,
    last_sounding: Option<f64>,
}

impl Tracker {
    pub(crate) const ON_MS: f64 = 45.0;
    pub(crate) const OFF_MS: f64 = 120.0;
    pub(crate) const PHRASE_GAP_MS: f64 = 250.0;
    pub(crate) const TOGETHER_MS: f64 = 60.0;

    pub(crate) fn update(
        &mut self,
        now_ms: f64,
        heard: &[HeardNote],
        hold_ms: f64,
        settling: bool,
    ) {
        let heard = heard
            .iter()
            .filter(|note| {
                !settling
                    || self
                        .notes
                        .get(&note.midi)
                        .is_some_and(|tracked| tracked.sounding)
            })
            .copied()
            .collect::<Vec<_>>();
        let quiet = self
            .last_sounding
            .is_none_or(|last| now_ms - last > Self::PHRASE_GAP_MS);
        let mut started = false;
        for note in &heard {
            let tracked = self.notes.entry(note.midi).or_insert(Tracked {
                heard_since: now_ms,
                last_heard: now_ms,
                sounding: false,
                confirmed: false,
                frequency_hz: note.frequency_hz,
                cents: note.cents,
                strength: note.strength,
                onset: now_ms,
                held: true,
            });
            if !tracked.sounding && now_ms - tracked.last_heard > Self::OFF_MS {
                tracked.heard_since = now_ms;
            }
            tracked.last_heard = now_ms;
            tracked.frequency_hz = note.frequency_hz;
            tracked.cents = note.cents;
            tracked.strength = 0.6 * tracked.strength + 0.4 * note.strength;
            if !tracked.sounding && now_ms - tracked.heard_since >= Self::ON_MS {
                tracked.sounding = true;
                tracked.confirmed = true;
                tracked.onset = now_ms;
                tracked.held = true;
                started = true;
            }
        }
        self.notes.retain(|midi, tracked| {
            let silent_for = now_ms - tracked.last_heard;
            if heard.iter().any(|note| note.midi == *midi) {
                return true;
            }
            if silent_for >= Self::OFF_MS {
                tracked.sounding = false;
            }
            if tracked.sounding {
                true
            } else if tracked.confirmed {
                tracked.held && silent_for <= Self::OFF_MS + hold_ms.max(0.0)
            } else {
                silent_for < Self::OFF_MS
            }
        });
        if started {
            let fresh =
                |tracked: &Tracked| tracked.sounding && now_ms - tracked.onset <= Self::TOGETHER_MS;
            if quiet || self.notes.values().filter(|tracked| fresh(tracked)).count() >= 2 {
                self.notes.retain(|_, tracked| {
                    tracked.held = fresh(tracked);
                    tracked.sounding || !tracked.confirmed
                });
            }
        }
        if self.notes.values().any(|tracked| tracked.sounding) {
            self.last_sounding = Some(now_ms);
        }
    }

    pub(crate) fn clear(&mut self) {
        self.notes.clear();
        self.last_sounding = None;
    }

    /// Every note that has sounded and is still held, lowest first.
    pub(crate) fn held(&self) -> Vec<HeldNote> {
        self.notes
            .iter()
            .filter(|(_, tracked)| tracked.confirmed)
            .map(|(&midi, tracked)| HeldNote {
                midi,
                frequency_hz: tracked.frequency_hz,
                cents: tracked.cents,
                strength: tracked.strength,
                sounding: tracked.sounding,
            })
            .collect()
    }
}

fn note_name(midi: i32) -> String {
    format!(
        "{}{}",
        display_pitch_name(pitch_class_name(midi.rem_euclid(12) as u8)),
        midi.div_euclid(12) - 1
    )
}

#[derive(Serialize)]
struct ListenedNote {
    midi: i32,
    name: String,
    frequency_hz: f64,
    cents: f64,
    strength: f64,
    sounding: bool,
}

#[derive(Clone, Serialize)]
struct ListenedChord {
    midi: Vec<i32>,
    pitch_names: Vec<String>,
    pitch_classes: Vec<u8>,
    common_name: String,
    pitched_common_name: String,
    chord_symbol: Option<String>,
    root: Option<String>,
    bass: Option<String>,
    inversion_name: Option<String>,
}

#[derive(Serialize)]
struct ListenFrame<'a> {
    level_db: f64,
    notes: Vec<ListenedNote>,
    chord: Option<&'a ListenedChord>,
}

fn name_chord(midi: &[i32]) -> Option<ListenedChord> {
    let chord = Chord::new(midi).ok()?;
    Some(ListenedChord {
        midi: midi.to_vec(),
        pitch_names: midi.iter().map(|&note| note_name(note)).collect(),
        pitch_classes: chord.pitch_classes(),
        common_name: chord.common_name(),
        pitched_common_name: chord.pitched_common_name(),
        chord_symbol: chord.chord_symbols().into_iter().next(),
        root: chord
            .root_pitch_name()
            .map(|name| display_pitch_name(&name)),
        bass: chord
            .bass_pitch_name()
            .map(|name| display_pitch_name(&name)),
        inversion_name: chord
            .inversion()
            .map(|_| chord.inversion_text().to_lowercase()),
    })
}

#[wasm_bindgen]
/// Names the chord a stream of audio frames holds, for the chord listener page.
pub struct ChordListener {
    analyzer: Analyzer,
    tracker: Tracker,
    sensitivity: f64,
    gate_db: f64,
    hold_ms: f64,
    chord: Option<ListenedChord>,
}

#[wasm_bindgen]
impl ChordListener {
    #[wasm_bindgen(constructor)]
    /// Starts listening to audio at `sample_rate` hertz.
    pub fn new(sample_rate: f64) -> Result<ChordListener, JsValue> {
        if !(sample_rate.is_finite() && sample_rate >= 8000.0) {
            return Err(JsValue::from_str(&format!(
                "sample rate must be at least 8000 Hz, got {sample_rate}"
            )));
        }
        Ok(Self {
            analyzer: Analyzer::new(sample_rate),
            tracker: Tracker::default(),
            sensitivity: 0.6,
            gate_db: -60.0,
            hold_ms: 1200.0,
            chord: None,
        })
    }

    /// Sets how readily quieter notes are heard beside the loudest, from zero
    /// to one.
    pub fn set_sensitivity(&mut self, sensitivity: f64) {
        self.sensitivity = sensitivity;
    }

    /// Sets the level in decibels below which a frame is taken as silence.
    pub fn set_gate_db(&mut self, gate_db: f64) {
        self.gate_db = gate_db;
    }

    /// Sets how long a note that stopped sounding still counts toward the chord.
    pub fn set_hold_ms(&mut self, hold_ms: f64) {
        self.hold_ms = hold_ms;
    }

    /// Forgets every held note.
    pub fn clear(&mut self) {
        self.tracker.clear();
        self.chord = None;
    }

    /// Hears one frame of samples taken at `time_ms` and returns its level,
    /// the notes held, and the chord they make.
    pub fn listen(&mut self, samples: &[f32], time_ms: f64) -> Result<JsValue, JsValue> {
        let heard = self
            .analyzer
            .hear(samples, threshold(self.sensitivity), self.gate_db);
        self.tracker
            .update(time_ms, &heard.notes, self.hold_ms, heard.settling);
        let held = self.tracker.held();
        let midi = held.iter().map(|note| note.midi).collect::<Vec<_>>();
        if self.chord.as_ref().map(|chord| chord.midi.as_slice()) != Some(midi.as_slice()) {
            self.chord = if midi.is_empty() {
                None
            } else {
                name_chord(&midi)
            };
        }
        let frame = ListenFrame {
            level_db: heard.level_db,
            notes: held
                .into_iter()
                .map(|note| ListenedNote {
                    midi: note.midi,
                    name: note_name(note.midi),
                    frequency_hz: note.frequency_hz,
                    cents: note.cents,
                    strength: note.strength,
                    sounding: note.sounding,
                })
                .collect(),
            chord: self.chord.as_ref(),
        };
        serde_wasm_bindgen::to_value(&frame).map_err(|err| JsValue::from_str(&err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::{Analyzer, HeardNote, Tracker, midi_frequency, name_chord, note_name, threshold};
    use std::f64::consts::PI;

    const SAMPLE_RATE: f64 = 48_000.0;
    const SIZE: usize = 8192;
    const HOP: usize = 1600;

    struct Random(u64);

    impl Random {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    #[derive(Clone, Copy, Debug)]
    enum Timbre {
        Sawtooth,
        Hollow,
        Piano,
        Bright,
        Soft,
    }

    impl Timbre {
        const ALL: [Timbre; 5] = [
            Timbre::Sawtooth,
            Timbre::Hollow,
            Timbre::Piano,
            Timbre::Bright,
            Timbre::Soft,
        ];

        fn partial(self, harmonic: usize) -> f64 {
            let h = harmonic as f64;
            match self {
                Timbre::Sawtooth => 1.0 / h,
                Timbre::Hollow => {
                    if harmonic % 2 == 1 {
                        1.0 / h
                    } else {
                        0.15 / h
                    }
                }
                Timbre::Piano => (-0.35 * (h - 1.0)).exp(),
                Timbre::Bright => [0.6, 1.0, 0.8, 0.5, 0.45, 0.3, 0.2, 0.15]
                    .get(harmonic - 1)
                    .copied()
                    .unwrap_or(0.1 / h),
                Timbre::Soft => 0.5f64.powi(harmonic as i32 - 1),
            }
        }

        fn inharmonicity(self) -> f64 {
            match self {
                Timbre::Piano => 4e-4,
                _ => 0.0,
            }
        }
    }

    fn add_partial(
        samples: &mut [f64],
        frequency: f64,
        amplitude: f64,
        phase: f64,
        decay: f64,
        attack_seconds: f64,
    ) {
        let step = 2.0 * PI * frequency / SAMPLE_RATE;
        let turn = 2.0 * step.cos();
        let fall = (-decay / SAMPLE_RATE).exp();
        let (mut previous, mut current) = ((phase - step).sin(), phase.sin());
        let mut level = amplitude;
        for (index, sample) in samples.iter_mut().enumerate() {
            let attack = if attack_seconds > 0.0 {
                (index as f64 / (attack_seconds * SAMPLE_RATE)).min(1.0)
            } else {
                1.0
            };
            *sample += level * attack * current;
            (previous, current) = (current, turn * current - previous);
            level *= fall;
        }
    }

    fn add_note(
        samples: &mut [f64],
        midi: i32,
        gain: f64,
        timbre: Timbre,
        decay: Option<f64>,
        random: &mut Random,
    ) {
        let fundamental = midi_frequency(f64::from(midi) + (random.next() - 0.5) * 0.1);
        let decay = decay.map(|rate| rate + 0.004 * fundamental.min(1000.0) * random.next());
        for harmonic in 1..=24usize {
            let h = harmonic as f64;
            let frequency = fundamental * h * (1.0 + timbre.inharmonicity() * h * h).sqrt();
            if frequency > SAMPLE_RATE / 2.2 {
                break;
            }
            let amplitude = gain * timbre.partial(harmonic) * (0.8 + 0.4 * random.next());
            let phase = random.next() * 2.0 * PI;
            match decay {
                Some(rate) => {
                    let rate = rate * (1.0 + 0.15 * (h - 1.0));
                    add_partial(samples, frequency, amplitude, phase, rate, 0.012);
                }
                None => add_partial(samples, frequency, amplitude, phase, 0.0, 0.0),
            }
        }
    }

    fn render(samples: &[f64], noise: f64, random: &mut Random) -> Vec<f32> {
        let peak = samples
            .iter()
            .fold(0.0f64, |peak, sample| peak.max(sample.abs()));
        samples
            .iter()
            .map(|sample| (0.3 * sample / peak.max(1e-9) + noise * (random.next() - 0.5)) as f32)
            .collect()
    }

    fn hear(notes: &[(i32, f64)], timbre: Timbre, noise: f64, seed: u64) -> Vec<i32> {
        let mut random = Random(seed);
        let mut samples = vec![0.0; SIZE];
        for &(midi, gain) in notes {
            add_note(&mut samples, midi, gain, timbre, None, &mut random);
        }
        Analyzer::new(SAMPLE_RATE)
            .hear(&render(&samples, noise, &mut random), threshold(0.6), -60.0)
            .notes
            .into_iter()
            .map(|note| note.midi)
            .collect()
    }

    fn listen_through(events: &[(i32, f64)], timbre: Timbre, seed: u64) -> (Vec<i32>, Vec<i32>) {
        let mut random = Random(seed);
        let seconds = events.iter().map(|&(_, start)| start).fold(0.0, f64::max) + 0.9;
        let mut samples = vec![0.0; (seconds * SAMPLE_RATE) as usize];
        for &(midi, start) in events {
            let first = (start * SAMPLE_RATE) as usize;
            add_note(
                &mut samples[first..],
                midi,
                1.0,
                timbre,
                Some(0.3),
                &mut random,
            );
        }
        let samples = render(&samples, 0.001, &mut random);
        let mut analyzer = Analyzer::new(SAMPLE_RATE);
        let mut tracker = Tracker::default();
        let mut ever = Vec::new();
        let mut end = SIZE;
        while end <= samples.len() {
            let heard = analyzer.hear(&samples[end - SIZE..end], threshold(0.6), -60.0);
            let now = end as f64 / SAMPLE_RATE * 1000.0;
            tracker.update(now, &heard.notes, 1200.0, heard.settling);
            ever.extend(held(&tracker));
            end += HOP;
        }
        ever.sort_unstable();
        ever.dedup();
        (ever, held(&tracker))
    }

    fn pitch_classes(midi: &[i32]) -> Vec<i32> {
        let mut classes = midi
            .iter()
            .map(|note| note.rem_euclid(12))
            .collect::<Vec<_>>();
        classes.sort_unstable();
        classes.dedup();
        classes
    }

    fn bass_class(midi: &[i32]) -> Option<i32> {
        midi.first().map(|note| note.rem_euclid(12))
    }

    fn names(midi: &[i32]) -> Vec<String> {
        midi.iter().map(|&note| note_name(note)).collect()
    }

    #[test]
    fn single_notes_are_heard_alone() {
        let mut failures = Vec::new();
        for timbre in Timbre::ALL {
            for midi in (super::LOWEST_MIDI..=super::HIGHEST_MIDI).step_by(2) {
                let heard = hear(&[(midi, 1.0)], timbre, 0.001, midi as u64);
                if heard != [midi] {
                    failures.push(format!("{timbre:?} {} heard {heard:?}", note_name(midi)));
                }
            }
        }
        assert!(
            failures.is_empty(),
            "{} failures:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn chords_are_heard_by_their_pitch_classes_and_bass() {
        let voicings: &[&[i32]] = &[
            &[60, 64, 67],
            &[48, 55, 64],
            &[48, 64, 67],
            &[40, 47, 52, 56, 59, 64],
            &[45, 52, 57, 60, 64],
            &[43, 47, 50, 55, 59, 67],
            &[50, 57, 62, 66],
            &[48, 52, 55, 58],
            &[53, 57, 60, 64],
            &[55, 59, 62, 65],
            &[57, 60, 64],
            &[62, 65, 69, 72],
            &[52, 55, 59, 62],
            &[64, 67, 72],
            &[59, 62, 65, 69],
            &[46, 53, 58, 62, 65],
        ];
        let mut failures = Vec::new();
        let mut total = 0;
        for timbre in Timbre::ALL {
            for (seed, voicing) in voicings.iter().enumerate() {
                let notes = voicing
                    .iter()
                    .enumerate()
                    .map(|(index, &midi)| (midi, 1.0 - 0.1 * ((index * 7 + seed) % 4) as f64))
                    .collect::<Vec<_>>();
                let heard = hear(&notes, timbre, 0.002, seed as u64 + 100);
                total += 1;
                if pitch_classes(&heard) != pitch_classes(voicing)
                    || bass_class(&heard) != bass_class(voicing)
                {
                    failures.push(format!(
                        "{timbre:?} {:?} heard {:?}",
                        names(voicing),
                        names(&heard)
                    ));
                }
            }
        }
        assert!(
            failures.len() * 10 <= total,
            "{} of {total} failures:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn struck_and_arpeggiated_chords_start_no_stray_notes() {
        let voicings: &[&[i32]] = &[
            &[41, 48, 57, 64],
            &[48, 55, 60, 64],
            &[56, 59, 64, 68],
            &[43, 59, 62, 65],
            &[45, 52, 57, 60, 64],
            &[50, 53, 56, 60],
        ];
        let mut failures = Vec::new();
        let mut total = 0;
        for timbre in [Timbre::Piano] {
            for (seed, voicing) in voicings.iter().enumerate() {
                for arpeggio in [false, true] {
                    let events = voicing
                        .iter()
                        .enumerate()
                        .map(|(index, &midi)| {
                            let delay = if arpeggio { index as f64 * 0.32 } else { 0.0 };
                            (midi, 0.3 + delay)
                        })
                        .collect::<Vec<_>>();
                    let (ever, last) = listen_through(&events, timbre, seed as u64);
                    let expected = pitch_classes(voicing);
                    let stray = pitch_classes(&ever)
                        .into_iter()
                        .any(|class| !expected.contains(&class));
                    total += 1;
                    if stray
                        || pitch_classes(&last) != expected
                        || bass_class(&last) != bass_class(voicing)
                    {
                        failures.push(format!(
                            "{timbre:?} {} {:?} ever {:?} last {:?}",
                            if arpeggio { "arpeggiated" } else { "struck" },
                            names(voicing),
                            names(&ever),
                            names(&last)
                        ));
                    }
                }
            }
        }
        assert!(
            failures.len() * 6 <= total,
            "{} of {total} failures:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn silence_and_noise_hold_no_notes() {
        let mut analyzer = Analyzer::new(SAMPLE_RATE);
        assert!(
            analyzer
                .hear(&[0.0; SIZE], threshold(0.6), -60.0)
                .notes
                .is_empty()
        );
        let mut random = Random(7);
        let noise = (0..SIZE)
            .map(|_| (0.0005 * (random.next() - 0.5)) as f32)
            .collect::<Vec<_>>();
        assert!(
            analyzer
                .hear(&noise, threshold(0.6), -60.0)
                .notes
                .is_empty()
        );
    }

    fn note(midi: i32) -> HeardNote {
        HeardNote {
            midi,
            frequency_hz: midi_frequency(f64::from(midi)),
            cents: 0.0,
            strength: 1.0,
        }
    }

    fn held(tracker: &Tracker) -> Vec<i32> {
        tracker.held().iter().map(|note| note.midi).collect()
    }

    #[test]
    fn tracker_gathers_an_arpeggio_and_lets_it_go() {
        let mut tracker = Tracker::default();
        let mut now = 0.0;
        for midi in [57, 60, 64] {
            for _ in 0..12 {
                tracker.update(now, &[note(midi)], 1000.0, false);
                now += 16.0;
            }
        }
        assert_eq!(held(&tracker), [57, 60, 64]);
        assert_eq!(
            tracker.held().iter().filter(|note| note.sounding).count(),
            1
        );
        now += 1200.0;
        tracker.update(now, &[], 1000.0, false);
        assert!(held(&tracker).is_empty());
    }

    #[test]
    fn tracker_ignores_a_one_frame_flicker() {
        let mut tracker = Tracker::default();
        tracker.update(0.0, &[note(60), note(79)], 1000.0, false);
        for frame in 1..10 {
            tracker.update(f64::from(frame) * 16.0, &[note(60)], 1000.0, false);
        }
        assert_eq!(held(&tracker), [60]);
    }

    #[test]
    fn tracker_without_hold_keeps_only_what_sounds() {
        let mut tracker = Tracker::default();
        for frame in 0..10 {
            tracker.update(f64::from(frame) * 16.0, &[note(60)], 0.0, false);
        }
        for frame in 10..30 {
            tracker.update(f64::from(frame) * 16.0, &[note(64)], 0.0, false);
        }
        assert_eq!(held(&tracker), [64]);
    }

    #[test]
    fn tracker_starts_a_new_chord_after_a_silence() {
        let mut tracker = Tracker::default();
        let mut now = 0.0;
        for _ in 0..20 {
            tracker.update(now, &[note(57), note(60), note(64)], 2000.0, false);
            now += 16.0;
        }
        for _ in 0..25 {
            tracker.update(now, &[], 2000.0, false);
            now += 16.0;
        }
        assert_eq!(held(&tracker), [57, 60, 64]);
        for _ in 0..10 {
            tracker.update(now, &[note(62)], 2000.0, false);
            now += 16.0;
        }
        assert_eq!(held(&tracker), [62]);
    }

    #[test]
    fn tracker_starts_a_new_chord_on_notes_struck_together() {
        let mut tracker = Tracker::default();
        let mut now = 0.0;
        for midi in [57, 60, 64] {
            for _ in 0..12 {
                tracker.update(now, &[note(midi)], 2000.0, false);
                now += 16.0;
            }
        }
        for _ in 0..4 {
            tracker.update(now, &[note(62), note(65)], 2000.0, false);
            now += 16.0;
        }
        assert_eq!(held(&tracker), [62, 64, 65]);
        for _ in 0..10 {
            tracker.update(now, &[note(62), note(65)], 2000.0, false);
            now += 16.0;
        }
        assert_eq!(held(&tracker), [62, 65]);
    }

    #[test]
    fn tracker_starts_no_note_while_an_attack_settles() {
        let mut tracker = Tracker::default();
        let mut now = 0.0;
        for _ in 0..10 {
            tracker.update(now, &[note(48)], 1000.0, false);
            now += 16.0;
        }
        for _ in 0..10 {
            tracker.update(now, &[note(48), note(76)], 1000.0, true);
            now += 16.0;
        }
        assert_eq!(held(&tracker), [48]);
    }

    #[test]
    fn heard_chords_are_named_by_the_crate() {
        let chord = name_chord(&[45, 60, 64, 67]).unwrap();
        assert_eq!(chord.pitch_names, ["A2", "C4", "E4", "G4"]);
        assert_eq!(chord.common_name, "minor seventh chord");
        assert_eq!(chord.chord_symbol.as_deref(), Some("Am7"));
        assert_eq!(chord.root.as_deref(), Some("A"));
        assert_eq!(chord.bass.as_deref(), Some("A"));
        let flat = name_chord(&[51, 55, 58]).unwrap();
        assert_eq!(flat.pitch_names, ["Eb3", "G3", "Bb3"]);
        assert_eq!(flat.common_name, "major triad");
        assert_eq!(flat.root.as_deref(), Some("Eb"));
    }
}
