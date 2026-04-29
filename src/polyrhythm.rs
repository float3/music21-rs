use num::integer::{gcd, lcm};
use std::collections::{BTreeMap, BTreeSet};

use crate::chord::Chord;
use crate::defaults::{FloatType, IntegerType, UnsignedIntegerType};
use crate::error::{Error, Result};
use crate::interval::{Interval, IntervalArgument};
use crate::pitch::Pitch;

#[derive(Debug, Clone)]
/// A repeating polyrhythm defined by a base meter and subdivision voices.
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
pub struct PolyrhythmRatioTone {
    /// The reduced subdivision component that produced this tone.
    pub component: UnsignedIntegerType,
    /// Semitone offset above the lowest reduced ratio.
    pub offset: IntegerType,
    /// Frequency ratio above the lowest reduced ratio.
    pub ratio: FloatType,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Timing and ratio analysis for one polyrhythm cycle.
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

    /// Creates a polyrhythm and assigns a nonzero tempo in beats per minute.
    #[deprecated(note = "use `Polyrhythm::new(...).and_then(|p| p.with_tempo(...))`")]
    pub fn new_with_tempo(
        base: UnsignedIntegerType,
        tempo: UnsignedIntegerType,
        subdivisions: &[UnsignedIntegerType],
    ) -> Result<Self> {
        Self::new(base, subdivisions)?.with_tempo(tempo)
    }

    /// Constructs a new Polyrhythm given a time signature, tempo, and
    /// subdivisions.
    #[deprecated(note = "use `Polyrhythm::from_time_signature`")]
    pub fn new_with_time_signature(
        base: UnsignedIntegerType,
        tempo: UnsignedIntegerType,
        subdivisions: &[UnsignedIntegerType],
    ) -> Result<Self> {
        Self::from_time_signature(base, tempo, subdivisions)
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

    /// Returns the number of ticks in one full cycle (measure).
    #[deprecated(note = "use `cycle_len`")]
    pub fn cycle_duration(&self) -> UnsignedIntegerType {
        self.cycle_len()
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
    #[deprecated(note = "use `events`")]
    pub fn events_one_cycle(&self) -> Result<Vec<PolyrhythmEvent>> {
        self.events()
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
    /// Components are first reduced by their greatest common divisor. The
    /// smallest reduced component is treated as the root ratio, and each
    /// remaining component is mapped to the nearest twelve-tone semitone
    /// offset using `12 * log2(component / root)`.
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
            .map(|(offset, component)| PolyrhythmRatioTone {
                component,
                offset,
                ratio: component as FloatType / root_ratio as FloatType,
            })
            .collect()
    }

    /// Returns ratio-derived pitches above `base`.
    pub fn ratio_pitches<T>(&self, base: T) -> Result<Vec<Pitch>>
    where
        T: TryInto<Pitch>,
        T::Error: Into<Error>,
    {
        let base_pitch = base.try_into().map_err(Into::into)?;
        self.ratio_tones()
            .into_iter()
            .map(|tone| {
                let interval = Interval::new(IntervalArgument::Int(tone.offset))?;
                Ok(base_pitch.transpose(interval))
            })
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
    #[deprecated(note = "use `coincidence_ticks`")]
    pub fn coincidence_ticks_one_cycle(&self, min_simultaneous: usize) -> Vec<UnsignedIntegerType> {
        self.coincidence_ticks(min_simultaneous)
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

    fn chord_from_base_pitch(&self, base_pitch: Pitch) -> Result<Chord> {
        let mut offsets = BTreeSet::new();
        for &sub in &self.components {
            let interval = self.cycle / sub;
            for i in 0..sub {
                let tick = i * interval;
                let ratio = tick as FloatType / self.cycle as FloatType;
                let semitones = (ratio * 12.0).round() as IntegerType;
                offsets.insert(semitones);
            }
        }

        let notes: Result<Vec<Pitch>, Error> = offsets
            .into_iter()
            .map(|offset| {
                let interval = Interval::new(IntervalArgument::Int(offset))?;
                Ok(base_pitch.transpose(interval))
            })
            .collect();

        let notes = notes?;
        Chord::new(notes.as_slice())
    }

    /// Converts one polyrhythm cycle into a chord above `base`.
    #[deprecated(note = "use `to_chord`")]
    pub fn as_chord<T>(&self, base: T) -> Result<Chord>
    where
        T: TryInto<Pitch>,
        T::Error: Into<Error>,
    {
        self.to_chord(base)
    }

    /// Converts one polyrhythm cycle into a pitch collection above `base`.
    #[deprecated(note = "use `to_polypitch`")]
    pub fn as_polypitch<T>(&self, base: T) -> Result<Chord>
    where
        T: TryInto<Pitch>,
        T::Error: Into<Error>,
    {
        self.to_polypitch(base)
    }

    /// Converts one polyrhythm cycle into a chord above `base`.
    pub fn to_chord<T>(&self, base: T) -> Result<Chord>
    where
        T: TryInto<Pitch>,
        T::Error: Into<Error>,
    {
        self.chord_from_base_pitch(base.try_into().map_err(Into::into)?)
    }

    /// Converts one polyrhythm cycle into a pitch collection above `base`.
    pub fn to_polypitch<T>(&self, base: T) -> Result<Chord>
    where
        T: TryInto<Pitch>,
        T::Error: Into<Error>,
    {
        self.to_chord(base)
    }
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
    fn test_to_chord_is_public_and_works() {
        let poly = Polyrhythm::from_time_signature(4, 120, &[2, 3, 4]).unwrap();
        let chord = poly.to_chord("C4").unwrap();
        assert!(!chord.pitched_common_name().is_empty());
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

    #[allow(deprecated)]
    #[test]
    fn deprecated_polyrhythm_aliases_delegate_to_new_names() {
        let poly = Polyrhythm::new_with_tempo(4, 120, &[2, 3]).unwrap();
        assert_eq!(poly.cycle_duration(), poly.cycle_len());
        assert_eq!(poly.events_one_cycle().unwrap(), poly.events().unwrap());
        assert_eq!(
            poly.coincidence_ticks_one_cycle(1),
            poly.coincidence_ticks(1)
        );

        let timed = Polyrhythm::new_with_time_signature(4, 120, &[2, 3, 4]).unwrap();
        assert_eq!(
            timed.as_chord("C4").unwrap().pitch_classes(),
            timed.to_chord("C4").unwrap().pitch_classes()
        );
        assert_eq!(
            timed.as_polypitch("C4").unwrap().pitch_classes(),
            timed.to_polypitch("C4").unwrap().pitch_classes()
        );
    }
}
