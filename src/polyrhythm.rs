use num::integer::{gcd, lcm};
use std::collections::BTreeMap;

use crate::chord::Chord;
use crate::defaults::{FloatType, IntegerType, UnsignedIntegerType};
use crate::duration::Duration;
use crate::error::{Error, Result};
use crate::interval::Interval;
use crate::meter::TimeSignature;
use crate::note::Note;
use crate::pitch::Pitch;
use crate::stream::{Stream, StreamKind};
use crate::tempo::MetronomeMark;

#[derive(Debug, Clone)]
/// A repeating polyrhythm defined by a base meter and subdivision voices.
#[must_use]
pub struct Polyrhythm {
    /// Beats per measure (e.g. 4 for 4/4 time)
    pub base: UnsignedIntegerType,
    /// Subdivisions (e.g. [3, 4] for a 3:4 polyrhythm)
    pub components: Vec<UnsignedIntegerType>,
    /// Tempo in BPM. `None` means no tempo has been assigned yet.
    pub tempo: Option<UnsignedIntegerType>,
    /// Total ticks per measure (lcm of subdivisions)
    pub cycle: UnsignedIntegerType,
    current_tick: UnsignedIntegerType,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A single tick in a polyrhythm cycle.
#[must_use]
pub struct PolyrhythmEvent {
    /// Tick index within the cycle.
    pub tick: UnsignedIntegerType,
    /// Time in seconds from the start of the cycle.
    pub time_seconds: FloatType,
    /// Per-component trigger flags for this tick.
    pub triggers: Vec<bool>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A chord tone inferred from a polyrhythm's subdivision ratios.
#[must_use]
pub struct PolyrhythmRatioTone {
    /// The reduced subdivision component that produced this tone.
    pub component: UnsignedIntegerType,
    /// The nearest whole number of semitones above the lowest reduced ratio.
    pub offset: IntegerType,
    /// Frequency ratio above the lowest reduced ratio.
    pub ratio: FloatType,
    /// The exact distance above the lowest reduced ratio, in cents.
    pub cents: FloatType,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Timing and ratio analysis for one polyrhythm cycle.
#[must_use]
pub struct PolyrhythmAnalysis {
    /// Beats per measure.
    pub base: UnsignedIntegerType,
    /// Subdivision voices.
    pub components: Vec<UnsignedIntegerType>,
    /// Tempo in beats per minute.
    pub tempo: UnsignedIntegerType,
    /// Total ticks per measure.
    pub cycle: UnsignedIntegerType,
    /// Duration of one tick in seconds.
    pub tick_duration: FloatType,
    /// Tick interval for each subdivision voice.
    pub component_intervals: Vec<UnsignedIntegerType>,
    /// Tick events where at least one voice triggers.
    pub hit_events: Vec<PolyrhythmEvent>,
    /// Ratio-derived chord tones.
    pub ratio_tones: Vec<PolyrhythmRatioTone>,
}

impl Polyrhythm {
    /// Creates a polyrhythm from a base meter and nonzero subdivisions.
    pub fn new(base: UnsignedIntegerType, subdivisions: &[UnsignedIntegerType]) -> Result<Self> {
        if base == 0 {
            return Err(Error::Polyrhythm("Base must be nonzero".into()));
        }
        if subdivisions.is_empty() {
            return Err(Error::Polyrhythm(
                "At least one subdivision is required".into(),
            ));
        }
        for &sub in subdivisions {
            if sub == 0 {
                return Err(Error::Polyrhythm("Subdivision must be nonzero".into()));
            }
        }
        let cycle = subdivisions.iter().fold(1, |acc, &x| lcm(acc, x));
        Ok(Self {
            base,
            components: subdivisions.to_vec(),
            tempo: None,
            cycle,
            current_tick: 0,
        })
    }

    /// Creates a polyrhythm from a time-signature numerator, tempo, and
    /// subdivision voices.
    pub fn from_time_signature(
        beats_per_measure: UnsignedIntegerType,
        tempo: UnsignedIntegerType,
        subdivisions: &[UnsignedIntegerType],
    ) -> Result<Self> {
        Self::new(beats_per_measure, subdivisions)?.with_tempo(tempo)
    }

    /// Returns this polyrhythm with a nonzero tempo in beats per minute.
    pub fn with_tempo(mut self, tempo: UnsignedIntegerType) -> Result<Self> {
        self.set_tempo(tempo)?;
        Ok(self)
    }

    /// Sets the tempo in beats per minute.
    pub fn set_tempo(&mut self, tempo: UnsignedIntegerType) -> Result<()> {
        if tempo == 0 {
            return Err(Error::Polyrhythm("Tempo must be nonzero".into()));
        }
        self.tempo = Some(tempo);
        Ok(())
    }

    /// Returns the tempo in beats per minute.
    ///
    /// Returns `None` when the polyrhythm was constructed without a tempo and
    /// [`Self::set_tempo`] has not been called.
    pub fn tempo(&self) -> Option<UnsignedIntegerType> {
        self.tempo
    }

    /// Returns the subdivision voices.
    pub fn components(&self) -> &[UnsignedIntegerType] {
        &self.components
    }

    /// Returns the current iterator tick.
    pub fn current_tick(&self) -> UnsignedIntegerType {
        self.current_tick
    }

    /// Resets iteration to the first tick in the cycle.
    pub fn reset(&mut self) {
        self.current_tick = 0;
    }

    /// Returns the tick interval for each subdivision voice.
    pub fn component_intervals(&self) -> Vec<UnsignedIntegerType> {
        self.components
            .iter()
            .map(|sub| self.cycle / *sub)
            .collect()
    }

    /// Returns the duration of one measure (in seconds)
    pub fn measure_duration(&self) -> Result<FloatType> {
        match self.tempo {
            Some(tempo) => Ok(self.base as FloatType * 60.0 / (tempo as FloatType)),
            None => Err(Error::Polyrhythm("Tempo not set".into())),
        }
    }

    /// Returns the duration of one tick (smallest subdivision unit) in seconds.
    pub fn tick_duration(&self) -> Result<FloatType> {
        Ok(self.measure_duration()? / self.cycle as FloatType)
    }

    /// Returns the number of ticks in one full cycle.
    pub fn cycle_len(&self) -> UnsignedIntegerType {
        self.cycle
    }

    /// Returns beat timings (in seconds) for each subdivision voice over one
    /// full measure.
    pub fn beat_timings(&self) -> Result<Vec<Vec<FloatType>>> {
        let tick_duration = self.tick_duration()?;
        Ok(self
            .components
            .iter()
            .map(|&sub| {
                let interval = self.cycle / sub;
                (0..sub)
                    .map(|i| (i * interval) as FloatType * tick_duration)
                    .collect()
            })
            .collect())
    }

    /// Returns all tick events in one full cycle.
    pub fn events(&self) -> Result<Vec<PolyrhythmEvent>> {
        let tick_duration = self.tick_duration()?;
        Ok((0..self.cycle)
            .map(|tick| {
                let triggers = self
                    .components
                    .iter()
                    .map(|&sub| {
                        let divisor = self.cycle / sub;
                        divisor != 0 && tick % divisor == 0
                    })
                    .collect::<Vec<_>>();
                PolyrhythmEvent {
                    tick,
                    time_seconds: tick as FloatType * tick_duration,
                    triggers,
                }
            })
            .collect())
    }

    /// Returns only events where at least one component triggers.
    pub fn hit_events(&self) -> Result<Vec<PolyrhythmEvent>> {
        Ok(self
            .events()?
            .into_iter()
            .filter(|event| event.triggers.iter().any(|trigger| *trigger))
            .collect())
    }

    /// Returns ratio-derived chord tones for the subdivision components.
    ///
    /// Components are first reduced by their greatest common divisor, and the
    /// smallest is the root: a component sounds `component / root` above it,
    /// which is `cents` exactly and `offset` to the nearest semitone. Two
    /// components that round to the same semitone give one tone, the smaller.
    pub fn ratio_tones(&self) -> Vec<PolyrhythmRatioTone> {
        let divisor = self
            .components
            .iter()
            .copied()
            .reduce(gcd)
            .unwrap_or(1)
            .max(1);
        let reduced_components = self
            .components
            .iter()
            .map(|component| component / divisor)
            .collect::<Vec<_>>();
        let root_ratio = reduced_components.iter().copied().min().unwrap_or(1).max(1);
        let mut tones_by_offset = BTreeMap::new();

        for component in reduced_components {
            let ratio = component as FloatType / root_ratio as FloatType;
            let offset = (12.0 * ratio.log2()).round() as IntegerType;
            tones_by_offset.entry(offset).or_insert(component);
        }

        tones_by_offset
            .into_iter()
            .map(|(offset, component)| {
                let ratio = component as FloatType / root_ratio as FloatType;
                PolyrhythmRatioTone {
                    component,
                    offset,
                    ratio,
                    cents: 1200.0 * ratio.log2(),
                }
            })
            .collect()
    }

    /// Returns the pitches the subdivision ratios sound above `base`, in tune.
    ///
    /// Each tone is spelled at its nearest semitone and carries the rest as a
    /// microtone, so a 4:5:6 rhythm on `C4` is `C4 E4(-14c) G4(+2c)`: the just
    /// major triad, not the equal-tempered one.
    pub fn ratio_pitches<T>(&self, base: T) -> Result<Vec<Pitch>>
    where
        T: TryInto<Pitch>,
        T::Error: Into<Error>,
    {
        let base_pitch = base.try_into().map_err(Into::into)?;
        self.ratio_tones()
            .into_iter()
            .map(|tone| pitch_above(&base_pitch, tone.cents))
            .collect()
    }

    /// One measure of the rhythm as a score: a part for each subdivision, in
    /// the order given, each playing its ratio's pitch above `base` as
    /// [`Polyrhythm::ratio_pitches`] tunes it, so the score is heard as the
    /// rhythm and its chord at once.
    ///
    /// The measure is `base` quarter notes in `base/4`, and a subdivision of
    /// `n` divides it into `n` equal notes, which are tuplets wherever `n`
    /// does not divide it evenly. The first part carries the tempo, when one
    /// is set.
    ///
    /// ```
    /// use music21_rs::Polyrhythm;
    ///
    /// let score = Polyrhythm::new(4, &[3, 2])?.with_tempo(90)?.to_score("C4")?;
    /// let parts = score.parts();
    /// assert_eq!(parts.len(), 2);
    /// let triplets = parts[0].measures()[0].notes();
    /// assert_eq!(triplets.len(), 3);
    /// assert!((triplets[1].0 - 4.0 / 3.0).abs() < 1e-9);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    pub fn to_score<T>(&self, base: T) -> Result<Stream>
    where
        T: TryInto<Pitch>,
        T::Error: Into<Error>,
    {
        let base_pitch = base.try_into().map_err(Into::into)?;
        let divisor = self
            .components
            .iter()
            .copied()
            .reduce(gcd)
            .unwrap_or(1)
            .max(1);
        let root = self.components.iter().copied().min().unwrap_or(1).max(1) / divisor;
        let bar = FloatType::from(self.base);
        let mut score = Stream::with_kind(StreamKind::Score);
        for (index, &component) in self.components.iter().enumerate() {
            let ratio = FloatType::from(component / divisor) / FloatType::from(root);
            let pitch = pitch_above(&base_pitch, 1200.0 * ratio.log2())?;
            let length = bar / FloatType::from(component);
            let mut measure = Stream::with_kind(StreamKind::Measure);
            measure.set_number(1);
            measure.insert(0.0, TimeSignature::new(self.base, 4)?);
            if let (0, Some(tempo)) = (index, self.tempo) {
                measure.insert(0.0, MetronomeMark::new(FloatType::from(tempo)));
            }
            for beat in 0..component {
                let note = Note::from_pitch(pitch.clone()).with_duration(Duration::new(length)?);
                measure.insert(FloatType::from(beat) * length, note);
            }
            let mut part = Stream::with_kind(StreamKind::Part);
            part.set_name(Some(component.to_string()));
            part.push(measure);
            score.push(part);
        }
        Ok(score)
    }

    /// Returns the frequencies the subdivision ratios sound at, the lowest at
    /// `base_hertz`.
    ///
    /// This is the same ratio heard as pitch rather than as rhythm: a 3:2
    /// polyrhythm at 110 and 165 Hz is a fifth.
    pub fn ratio_frequencies(&self, base_hertz: FloatType) -> Vec<FloatType> {
        self.ratio_tones()
            .into_iter()
            .map(|tone| base_hertz * tone.ratio)
            .collect()
    }

    /// Converts subdivision ratios into a chord above `base`.
    pub fn ratio_chord<T>(&self, base: T) -> Result<Chord>
    where
        T: TryInto<Pitch>,
        T::Error: Into<Error>,
    {
        let pitches = self.ratio_pitches(base)?;
        Chord::new(pitches.as_slice())
    }

    /// Returns timing and ratio analysis for one cycle.
    pub fn analysis(&self) -> Result<PolyrhythmAnalysis> {
        let tempo = self
            .tempo
            .ok_or_else(|| Error::Polyrhythm("Tempo not set".into()))?;
        Ok(PolyrhythmAnalysis {
            base: self.base,
            components: self.components.clone(),
            tempo,
            cycle: self.cycle,
            tick_duration: self.tick_duration()?,
            component_intervals: self.component_intervals(),
            hit_events: self.hit_events()?,
            ratio_tones: self.ratio_tones(),
        })
    }

    /// Returns ticks where at least `min_simultaneous` components trigger.
    pub fn coincidence_ticks(&self, min_simultaneous: usize) -> Vec<UnsignedIntegerType> {
        if min_simultaneous == 0 {
            return (0..self.cycle).collect();
        }

        (0..self.cycle)
            .filter(|tick| {
                self.components
                    .iter()
                    .filter(|sub| {
                        let divisor = self.cycle / **sub;
                        divisor != 0 && *tick % divisor == 0
                    })
                    .count()
                    >= min_simultaneous
            })
            .collect()
    }
}

/// `cents` above `base`, spelled at the nearest semitone with the rest as a
/// microtone on top of any `base` already carries.
fn pitch_above(base: &Pitch, cents: FloatType) -> Result<Pitch> {
    let semitones = (cents / 100.0).round();
    let mut pitch = base.transpose(&Interval::from_semitones(semitones as IntegerType)?)?;
    let carried = base.microtone().map_or(0.0, |microtone| microtone.cents());
    pitch.set_microtone_cents(carried + cents - 100.0 * semitones)?;
    Ok(pitch)
}

impl Iterator for Polyrhythm {
    type Item = (UnsignedIntegerType, Vec<bool>);

    /// Advances the polyrhythm by one tick.
    /// Returns the current tick and a vector indicating which subdivision
    /// triggers a beat.
    fn next(&mut self) -> Option<Self::Item> {
        let tick = self.current_tick;
        let triggers = self
            .components
            .iter()
            .map(|&sub| {
                let divisor = self.cycle / sub;
                tick.checked_rem(divisor) == Some(0)
            })
            .collect();
        self.current_tick = (self.current_tick + 1) % self.cycle;
        Some((tick, triggers))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::StreamElement;

    #[test]
    fn test_from_time_signature() {
        let poly = Polyrhythm::from_time_signature(4, 120, &[2, 3]).unwrap();
        // For subdivisions 2 and 3, lcm is 6 ticks per measure.
        assert_eq!(poly.cycle_len(), 6);
        // tick_duration = (4 * 60 / 120) / 6 = (4 * 0.5) / 6 = 2 / 6 ≈ 0.3333 sec.
        let tick_dur = poly.tick_duration().unwrap();
        assert!((tick_dur - 0.3333).abs() < 0.01);
    }

    #[test]
    fn test_new_rejects_zero_base() {
        let err = Polyrhythm::new(0, &[2, 3]).unwrap_err();
        assert!(err.to_string().contains("Base must be nonzero"));
    }

    #[test]
    fn test_new_rejects_empty_and_zero_subdivisions() {
        let empty = Polyrhythm::new(4, &[]).unwrap_err();
        assert!(empty.to_string().contains("At least one subdivision"));

        let zero_subdivision = Polyrhythm::new(4, &[2, 0, 3]).unwrap_err();
        assert!(
            zero_subdivision
                .to_string()
                .contains("Subdivision must be nonzero")
        );
    }

    #[test]
    fn test_set_tempo_rejects_zero() {
        let mut poly = Polyrhythm::new(4, &[2, 3]).unwrap();
        let err = poly.set_tempo(0).unwrap_err();
        assert!(err.to_string().contains("Tempo must be nonzero"));
    }

    #[test]
    fn test_with_tempo_sets_tempo() {
        let poly = Polyrhythm::new(4, &[3, 4]).unwrap().with_tempo(90).unwrap();
        assert_eq!(poly.tempo(), Some(90));
    }

    #[test]
    fn test_without_tempo_rejects_time_queries() {
        let poly = Polyrhythm::new(4, &[2, 3]).unwrap();
        assert!(poly.measure_duration().is_err());
        assert!(poly.tick_duration().is_err());
        assert!(poly.beat_timings().is_err());
        assert!(poly.events().is_err());
    }

    #[test]
    fn test_beat_timings_are_spaced_by_component_interval() {
        let poly = Polyrhythm::from_time_signature(4, 120, &[2, 3]).unwrap();
        let timings = poly.beat_timings().unwrap();
        assert_eq!(timings.len(), 2);
        assert_eq!(timings[0].len(), 2);
        assert_eq!(timings[1].len(), 3);
        assert!((timings[0][1] - 1.0).abs() < 0.001);
        assert!((timings[1][1] - 0.6666).abs() < 0.01);
    }

    #[test]
    fn test_events() {
        let poly = Polyrhythm::from_time_signature(4, 120, &[2, 3]).unwrap();
        let events = poly.events().unwrap();
        assert_eq!(events.len(), 6);
        assert_eq!(events[0].triggers, vec![true, true]);
        assert_eq!(events[1].triggers, vec![false, false]);
        assert_eq!(events[2].triggers, vec![false, true]);
        assert_eq!(events[3].triggers, vec![true, false]);

        let hits = poly.hit_events().unwrap();
        assert_eq!(hits.len(), 4);
        assert_eq!(
            hits.iter().map(|event| event.tick).collect::<Vec<_>>(),
            vec![0, 2, 3, 4]
        );
    }

    #[test]
    fn ratio_tones_reduce_components_and_project_to_pitches() {
        let poly = Polyrhythm::from_time_signature(4, 120, &[3, 4, 6]).unwrap();
        let tones = poly.ratio_tones();
        assert_eq!(
            tones
                .iter()
                .map(|tone| (tone.component, tone.offset))
                .collect::<Vec<_>>(),
            vec![(3, 0), (4, 5), (6, 12)]
        );

        let pitches = poly.ratio_pitches("C4").unwrap();
        assert_eq!(
            pitches
                .iter()
                .map(Pitch::name_with_octave)
                .collect::<Vec<_>>(),
            vec!["C4", "F4", "C5"]
        );
        // 4/3 is a just fourth, two cents flat of the tempered one.
        assert!((tones[1].cents - 498.044_999_134_798).abs() < 1e-9);
        assert!((pitches[1].microtone().unwrap().cents() + 1.955).abs() < 1e-3);
        assert!(pitches[2].microtone().is_none());

        let analysis = poly.analysis().unwrap();
        assert_eq!(analysis.component_intervals, vec![4, 3, 2]);
        assert_eq!(analysis.ratio_tones, tones);
    }

    #[test]
    fn test_coincidence_ticks() {
        let poly = Polyrhythm::from_time_signature(4, 120, &[2, 3]).unwrap();
        assert_eq!(poly.coincidence_ticks(0), vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(poly.coincidence_ticks(2), vec![0]);
        assert_eq!(poly.coincidence_ticks(1), vec![0, 2, 3, 4]);
    }

    #[test]
    fn a_rhythm_of_four_five_and_six_is_a_just_major_triad() {
        let poly = Polyrhythm::new(4, &[4, 5, 6]).unwrap();
        let pitches = poly.ratio_pitches("C4").unwrap();
        let written = pitches
            .iter()
            .map(|pitch| {
                let microtone = pitch.microtone().map(ToString::to_string);
                format!(
                    "{}{}",
                    pitch.name_with_octave(),
                    microtone.unwrap_or_default()
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(written, ["C4", "E4(-14c)", "G4(+2c)"]);

        let hertz = poly.ratio_frequencies(200.0);
        assert_eq!(hertz, [200.0, 250.0, 300.0]);
        // The pitches sound where the ratios say.
        let just = pitches[1].frequency_hz() / pitches[0].frequency_hz();
        assert!((just - 1.25).abs() < 1e-3);
    }

    #[test]
    fn a_score_holds_a_part_for_each_subdivision_in_tuplets() {
        let poly = Polyrhythm::new(4, &[4, 5, 6])
            .unwrap()
            .with_tempo(72)
            .unwrap();
        let score = poly.to_score("C4").unwrap();
        let parts = score.parts();
        assert_eq!(parts.len(), 3);
        for (part, (count, name)) in parts.iter().zip([(4, "C4"), (5, "E4"), (6, "G4")]) {
            let notes = part.measures()[0].notes();
            assert_eq!(notes.len(), count);
            let StreamElement::Note(note) = &notes[0].1 else {
                panic!("a part plays notes");
            };
            assert_eq!(note.pitch().name_with_octave(), name);
            let total: FloatType = notes
                .iter()
                .map(|(_, element)| element.duration().map_or(0.0, |d| d.quarter_length()))
                .sum();
            assert!((total - 4.0).abs() < 1e-9);
        }
        let StreamElement::Note(quintuplet) = &parts[1].measures()[0].notes()[0].1 else {
            panic!("a part plays notes");
        };
        assert!(!quintuplet.duration().unwrap().tuplets().is_empty());
        assert_eq!(parts[1].name(), Some("5"));
    }

    #[cfg(feature = "musicxml")]
    #[test]
    fn a_score_writes_as_musicxml_with_its_tuplets() {
        use crate::musicxml::{ExportOptions, to_musicxml};
        for components in [[3, 2], [5, 4], [7, 3]] {
            let score = Polyrhythm::new(4, &components)
                .unwrap()
                .with_tempo(60)
                .unwrap()
                .to_score("A3")
                .unwrap();
            let xml = to_musicxml(&score, &ExportOptions::default()).unwrap();
            assert!(xml.contains("<time-modification>"), "{components:?}");
            assert!(
                xml.contains("<per-minute>60</per-minute>"),
                "{components:?}"
            );
        }
    }

    #[test]
    fn test_iterator_state_and_reset() {
        let mut poly = Polyrhythm::new(4, &[2, 4]).unwrap();
        assert_eq!(poly.components(), &[2, 4]);
        assert_eq!(poly.component_intervals(), vec![2, 1]);
        assert_eq!(poly.current_tick(), 0);

        assert_eq!(poly.next(), Some((0, vec![true, true])));
        assert_eq!(poly.current_tick(), 1);
        assert_eq!(poly.next(), Some((1, vec![false, true])));
        poly.reset();
        assert_eq!(poly.current_tick(), 0);
    }
}
