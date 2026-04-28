use num::integer::lcm;
use std::collections::BTreeSet;

use crate::chord::Chord;
use crate::defaults::{FloatType, IntegerType, UnsignedIntegerType};
use crate::exception::{Exception, ExceptionResult};
use crate::interval::{Interval, IntervalArgument};
use crate::note::IntoPitch;
use crate::pitch::Pitch;

#[derive(Debug, Clone)]
pub struct Polyrhythm {
    /// Beats per measure (e.g. 4 for 4/4 time)
    pub base: UnsignedIntegerType,
    /// Subdivisions (e.g. [3, 4] for a 3:4 polyrhythm)
    pub components: Vec<UnsignedIntegerType>,
    /// Tempo in BPM
    pub tempo: Option<UnsignedIntegerType>,
    /// Total ticks per measure (lcm of subdivisions)
    pub cycle: UnsignedIntegerType,
    current_tick: UnsignedIntegerType,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PolyrhythmEvent {
    pub tick: UnsignedIntegerType,
    pub time_seconds: FloatType,
    pub triggers: Vec<bool>,
}

impl Polyrhythm {
    pub fn new(
        base: UnsignedIntegerType,
        subdivisions: &[UnsignedIntegerType],
    ) -> ExceptionResult<Self> {
        if base == 0 {
            return Err(Exception::Polyrhythm("Base must be nonzero".into()));
        }
        if subdivisions.is_empty() {
            return Err(Exception::Polyrhythm(
                "At least one subdivision is required".into(),
            ));
        }
        for &sub in subdivisions {
            if sub == 0 {
                return Err(Exception::Polyrhythm("Subdivision must be nonzero".into()));
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

    pub fn new_with_tempo(
        base: UnsignedIntegerType,
        tempo: UnsignedIntegerType,
        subdivisions: &[UnsignedIntegerType],
    ) -> ExceptionResult<Self> {
        let mut poly = Self::new(base, subdivisions)?;
        poly.set_tempo(tempo)?;
        Ok(poly)
    }

    /// Constructs a new Polyrhythm given a time signature, tempo, and
    /// subdivisions.
    pub fn new_with_time_signature(
        base: UnsignedIntegerType,
        tempo: UnsignedIntegerType,
        subdivisions: &[UnsignedIntegerType],
    ) -> ExceptionResult<Self> {
        Self::new_with_tempo(base, tempo, subdivisions)
    }

    pub fn set_tempo(&mut self, tempo: UnsignedIntegerType) -> ExceptionResult<()> {
        if tempo == 0 {
            return Err(Exception::Polyrhythm("Tempo must be nonzero".into()));
        }
        self.tempo = Some(tempo);
        Ok(())
    }

    pub fn tempo(&self) -> Option<UnsignedIntegerType> {
        self.tempo
    }

    pub fn components(&self) -> &[UnsignedIntegerType] {
        &self.components
    }

    pub fn current_tick(&self) -> UnsignedIntegerType {
        self.current_tick
    }

    pub fn reset(&mut self) {
        self.current_tick = 0;
    }

    pub fn component_intervals(&self) -> Vec<UnsignedIntegerType> {
        self.components
            .iter()
            .map(|sub| self.cycle / *sub)
            .collect()
    }

    /// Returns the duration of one measure (in seconds)
    pub fn measure_duration(&self) -> ExceptionResult<FloatType> {
        match self.tempo {
            Some(tempo) => Ok(self.base as FloatType * 60.0 / (tempo as FloatType)),
            None => Err(Exception::Polyrhythm("Tempo not set".into())),
        }
    }

    /// Returns the duration of one tick (smallest subdivision unit) in seconds.
    pub fn tick_duration(&self) -> ExceptionResult<FloatType> {
        Ok(self.measure_duration()? / self.cycle as FloatType)
    }

    /// Returns the number of ticks in one full cycle (measure).
    pub fn cycle_duration(&self) -> UnsignedIntegerType {
        self.cycle
    }

    /// Returns beat timings (in seconds) for each subdivision voice over one
    /// full measure.
    pub fn beat_timings(&self) -> ExceptionResult<Vec<Vec<FloatType>>> {
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

    pub fn events_one_cycle(&self) -> ExceptionResult<Vec<PolyrhythmEvent>> {
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

    pub fn coincidence_ticks_one_cycle(&self, min_simultaneous: usize) -> Vec<UnsignedIntegerType> {
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

    fn chord_from_base_pitch(&self, base_pitch: Pitch) -> ExceptionResult<Chord> {
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

        let notes: Result<Vec<Pitch>, Exception> = offsets
            .into_iter()
            .map(|offset| {
                let interval = Interval::new(IntervalArgument::Int(offset))?;
                Ok(base_pitch.transpose(interval))
            })
            .collect();

        let notes = notes?;
        Chord::new(Some(notes.as_slice()))
    }

    pub fn as_chord<T>(&self, base: T) -> ExceptionResult<Chord>
    where
        T: IntoPitch,
    {
        self.chord_from_base_pitch(base.into_pitch()?)
    }

    pub fn as_polypitch<T>(&self, base: T) -> ExceptionResult<Chord>
    where
        T: IntoPitch,
    {
        self.as_chord(base)
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
    fn test_new_with_time_signature() {
        let poly = Polyrhythm::new_with_time_signature(4, 120, &[2, 3]).unwrap();
        // For subdivisions 2 and 3, lcm is 6 ticks per measure.
        assert_eq!(poly.cycle_duration(), 6);
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
    fn test_set_tempo_rejects_zero() {
        let mut poly = Polyrhythm::new(4, &[2, 3]).unwrap();
        let err = poly.set_tempo(0).unwrap_err();
        assert!(err.to_string().contains("Tempo must be nonzero"));
    }

    #[test]
    fn test_new_with_tempo_sets_tempo() {
        let poly = Polyrhythm::new_with_tempo(4, 90, &[3, 4]).unwrap();
        assert_eq!(poly.tempo(), Some(90));
    }

    #[test]
    fn test_beat_timings_are_spaced_by_component_interval() {
        let poly = Polyrhythm::new_with_time_signature(4, 120, &[2, 3]).unwrap();
        let timings = poly.beat_timings().unwrap();
        assert_eq!(timings.len(), 2);
        assert_eq!(timings[0].len(), 2);
        assert_eq!(timings[1].len(), 3);
        assert!((timings[0][1] - 1.0).abs() < 0.001);
        assert!((timings[1][1] - 0.6666).abs() < 0.01);
    }

    #[test]
    fn test_events_one_cycle() {
        let poly = Polyrhythm::new_with_time_signature(4, 120, &[2, 3]).unwrap();
        let events = poly.events_one_cycle().unwrap();
        assert_eq!(events.len(), 6);
        assert_eq!(events[0].triggers, vec![true, true]);
        assert_eq!(events[1].triggers, vec![false, false]);
        assert_eq!(events[2].triggers, vec![false, true]);
        assert_eq!(events[3].triggers, vec![true, false]);
    }

    #[test]
    fn test_coincidence_ticks_one_cycle() {
        let poly = Polyrhythm::new_with_time_signature(4, 120, &[2, 3]).unwrap();
        assert_eq!(poly.coincidence_ticks_one_cycle(2), vec![0]);
        assert_eq!(poly.coincidence_ticks_one_cycle(1), vec![0, 2, 3, 4]);
    }

    #[test]
    fn test_as_chord_is_public_and_works() {
        let poly = Polyrhythm::new_with_time_signature(4, 120, &[2, 3, 4]).unwrap();
        let chord = poly.as_chord("C4").unwrap();
        assert!(!chord.pitched_common_name().is_empty());
    }
}
