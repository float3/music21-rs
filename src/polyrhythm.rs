use num::integer::lcm;
use std::collections::BTreeSet;

use crate::chord::Chord;
use crate::defaults::{FloatType, UnsignedIntegerType};
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

impl Polyrhythm {
    /// Constructs a new Polyrhythm given a time signature, tempo, and subdivisions.
    pub fn new_with_time_signature(
        base: UnsignedIntegerType,
        tempo: UnsignedIntegerType,
        subdivisions: &[UnsignedIntegerType],
    ) -> ExceptionResult<Self> {
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
            tempo: Some(tempo),
            cycle,
            current_tick: 0,
        })
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

    /// Returns beat timings (in seconds) for each subdivision voice over one full measure.
    pub fn beat_timings(&self) -> ExceptionResult<Vec<Vec<FloatType>>> {
        let tick_duration = self.tick_duration()?;
        Ok(self
            .components
            .iter()
            .map(|&sub| {
                let interval = self.cycle / sub;
                (0..sub).map(|i| i as FloatType * tick_duration).collect()
            })
            .collect())
    }

    // Existing functionality (e.g. as_chord, as_polypitch) remains unchanged.
    fn as_chord<T>(&self, base: T) -> ExceptionResult<Chord>
    where
        T: IntoPitch,
    {
        let mut offsets = BTreeSet::new();
        for &sub in &self.components {
            let interval = self.cycle / sub;
            for i in 0..sub {
                let tick = i * interval;
                let ratio = tick as FloatType / self.cycle as FloatType;
                let semitones = (ratio * 12.0).round() as UnsignedIntegerType;
                offsets.insert(semitones);
            }
        }

        let base_pitch = base.into_pitch()?;
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

    fn as_polypitch<T>(&self, base: T) -> ExceptionResult<Chord>
    where
        T: IntoPitch,
    {
        self.as_chord(base)
    }
}

impl Iterator for Polyrhythm {
    type Item = (UnsignedIntegerType, Vec<bool>);

    /// Advances the polyrhythm by one tick.
    /// Returns the current tick and a vector indicating which subdivision triggers a beat.
    fn next(&mut self) -> Option<Self::Item> {
        let tick = self.current_tick;
        let triggers = self
            .components
            .iter()
            .map(|&sub| tick % (self.cycle / sub) == 0)
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

    // Additional tests for beat_timings, next, etc.
}
