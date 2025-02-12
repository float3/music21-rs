use std::collections::BTreeSet;

use num::integer::lcm;

use crate::chord::Chord;
use crate::defaults::FloatType;
use crate::defaults::UnsignedIntegerType;
use crate::exception::Exception;
use crate::exception::ExceptionResult;
use crate::interval::Interval;
use crate::interval::IntervalArgument;
use crate::note::IntoPitch;
use crate::pitch::Pitch;

#[derive(Debug, Clone)]
pub struct Polyrhythm {
    components: Vec<UnsignedIntegerType>,
    tempo: UnsignedIntegerType,
    cycle: UnsignedIntegerType,
    current_tick: UnsignedIntegerType,
}

impl Polyrhythm {
    /// Constructs a new `Polyrhythm` from a colon-separated string.
    ///
    /// The input string should be in the format
    /// `"tempo:component1:component2:..."` where:
    ///
    /// - **tempo**: The first token, representing the tempo in BPM (beats per
    ///   minute).   The BPM value determines the duration of each tick,
    ///   computed as `60.0 / tempo` seconds.
    /// - **componentX**: Each subsequent token is a positive, nonzero unsigned
    ///   integer representing the number of beats (or subdivisions) in one
    ///   polyrhythmic voice. The overall cycle duration (in ticks) is computed
    ///   as the least common multiple (LCM) of these components.
    ///
    /// # Examples
    ///
    /// ```
    /// // For the string "120:3:4":
    /// // - The tempo is 120 BPM.
    /// // - The components are 3 and 4, so the cycle duration is lcm(3, 4) = 12 ticks.
    /// use music21_rs::polyrhythm::Polyrhythm;
    ///
    /// let poly = Polyrhythm::from_string("120:3:4").unwrap();
    /// assert_eq!(poly.cycle_duration(), 12);
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The string does not contain at least one tempo and one component.
    /// - The tempo or any component cannot be parsed into a valid unsigned
    ///   integer.
    /// - Any component is zero.
    pub fn from_string(s: &str) -> ExceptionResult<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() < 2 {
            return Err(Exception::Polyrhythm(
                "Invalid format: expected tempo and at least one component".into(),
            ));
        }
        let tempo = parts[0]
            .parse::<UnsignedIntegerType>()
            .map_err(|_| Exception::Polyrhythm("Invalid tempo".into()))?;
        let mut components = Vec::new();
        for &part in &parts[1..] {
            let comp = part
                .parse::<UnsignedIntegerType>()
                .map_err(|_| Exception::Polyrhythm("Invalid component".into()))?;
            if comp == 0 {
                return Err(Exception::Polyrhythm("Component must be nonzero".into()));
            }
            components.push(comp);
        }
        let cycle = components.iter().fold(1, |acc, &x| lcm(acc, x));
        Ok(Self {
            components,
            tempo,
            cycle,
            current_tick: 0,
        })
    }

    /// Constructs a new `Polyrhythm` from the given tempo and components.
    ///
    /// The `tempo` is specified in BPM (beats per minute) and determines the
    /// duration of each tick, calculated as `60.0 / tempo` seconds per
    /// tick. The `components` vector contains positive, nonzero
    /// unsigned integers that represent the number of beats (or subdivisions)
    /// for each polyrhythmic voice. The overall cycle duration (in ticks)
    /// is computed as the least common multiple (LCM) of these components.
    ///
    /// # Arguments
    ///
    /// * `tempo` - An unsigned integer representing the beats per minute.
    /// * `components` - A vector of positive, nonzero unsigned integers
    ///   representing the subdivisions.
    ///
    /// # Examples
    ///
    /// ```
    /// // Create a Polyrhythm with a tempo of 120 BPM and components [3, 4]:
    /// // The cycle duration is lcm(3, 4) = 12 ticks.
    /// use music21_rs::polyrhythm::Polyrhythm;
    ///
    /// let poly = Polyrhythm::new(120, vec![3, 4]).unwrap();
    /// assert_eq!(poly.cycle_duration(), 12);
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The `components` vector is empty.
    /// - Any component is zero.
    pub fn new(
        tempo: UnsignedIntegerType,
        components: Vec<UnsignedIntegerType>,
    ) -> ExceptionResult<Self> {
        if components.is_empty() {
            return Err(Exception::Polyrhythm(
                "At least one component is required".into(),
            ));
        }
        for &comp in &components {
            if comp == 0 {
                return Err(Exception::Polyrhythm("Component must be nonzero".into()));
            }
        }
        let cycle = components.iter().fold(1, |acc, &x| lcm(acc, x));
        Ok(Self {
            components,
            tempo,
            cycle,
            current_tick: 0,
        })
    }

    /// Returns the cycle duration in subdivisions (ticks). This is the least
    /// common multiple (LCM) of all the components.
    pub fn cycle_duration(&self) -> UnsignedIntegerType {
        self.cycle
    }

    // Returns beat timings (in seconds) for each component over one full cycle.
    pub fn beat_timings(&self) -> Vec<Vec<f64>> {
        let tick_duration = 60.0 / (self.tempo as f64);
        self.components
            .iter()
            .map(|&comp| {
                let interval = self.cycle / comp;
                (0..comp)
                    .map(|i| (i * interval) as f64 * tick_duration)
                    .collect()
            })
            .collect()
    }

    fn as_chord<T>(&self, base: T) -> ExceptionResult<Chord>
    where
        T: IntoPitch,
    {
        let mut offsets = BTreeSet::new();
        for &comp in &self.components {
            let interval = self.cycle / comp;
            for i in 0..comp {
                let tick = i * interval;
                let ratio = tick as FloatType / self.cycle as FloatType;
                let semitones = (ratio * 12.0).round() as UnsignedIntegerType;
                offsets.insert(semitones);
            }
        }

        let base_pitch = base.into_pitch()?;
        let notes: Result<Vec<Pitch>, Exception> = offsets
            .into_iter()
            .map(|offset| -> Result<Pitch, Exception> {
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

    // Advances the state machine by one tick.
    // Returns (current tick, vector indicating which component has a beat).
    fn next(&mut self) -> Option<Self::Item> {
        let tick = self.current_tick;
        let triggers = self
            .components
            .iter()
            .map(|&comp| tick % (self.cycle / comp) == 0)
            .collect();
        self.current_tick = (self.current_tick + 1) % self.cycle;
        Some((tick, triggers))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_string_valid() {
        let poly = Polyrhythm::from_string("120:3:4").unwrap();
        assert_eq!(poly.tempo, 120);
        assert_eq!(poly.components, vec![3, 4]);
        assert_eq!(poly.cycle_duration(), 12);
    }

    #[test]
    fn test_from_string_invalid_format() {
        let err = Polyrhythm::from_string("120").unwrap_err();
        match err {
            Exception::Polyrhythm(ref msg) => {
                assert!(msg.contains("Invalid format"));
            }
            _ => panic!("Expected Exception::Polyrhythm"),
        }
    }

    #[test]
    fn test_from_string_invalid_tempo() {
        let err = Polyrhythm::from_string("abc:3:4").unwrap_err();
        match err {
            Exception::Polyrhythm(ref msg) => {
                assert!(msg.contains("Invalid tempo"));
            }
            _ => panic!("Expected Exception::Polyrhythm"),
        }
    }

    #[test]
    fn test_from_string_invalid_component() {
        let err = Polyrhythm::from_string("120:3:0").unwrap_err();
        match err {
            Exception::Polyrhythm(ref msg) => {
                assert!(msg.contains("Component must be nonzero"));
            }
            _ => panic!("Expected Exception::Polyrhythm"),
        }
    }

    #[test]
    fn test_new_valid() {
        let poly = Polyrhythm::new(120, vec![3, 4]).unwrap();
        assert_eq!(poly.tempo, 120);
        assert_eq!(poly.components, vec![3, 4]);
        assert_eq!(poly.cycle_duration(), 12);
    }

    #[test]
    fn test_new_empty_components() {
        let err = Polyrhythm::new(120, vec![]).unwrap_err();
        match err {
            Exception::Polyrhythm(ref msg) => {
                assert!(msg.contains("At least one component is required"));
            }
            _ => panic!("Expected Exception::Polyrhythm"),
        }
    }

    #[test]
    fn test_new_invalid_component() {
        let err = Polyrhythm::new(120, vec![3, 0, 4]).unwrap_err();
        match err {
            Exception::Polyrhythm(ref msg) => {
                assert!(msg.contains("Component must be nonzero"));
            }
            _ => panic!("Expected Exception::Polyrhythm"),
        }
    }

    #[test]
    fn test_beat_timings() {
        let poly = Polyrhythm::new(120, vec![3, 4]).unwrap();
        let timings = poly.beat_timings();
        // For tempo 120 BPM, tick_duration = 60 / 120 = 0.5 seconds.
        // For component 3: period = 12 / 3 = 4 ticks -> beats at ticks 0, 4, 8 (times:
        // 0, 2.0, 4.0). For component 4: period = 12 / 4 = 3 ticks -> beats at
        // ticks 0, 3, 6, 9 (times: 0, 1.5, 3.0, 4.5).
        assert_eq!(timings.len(), 2);
        assert_eq!(timings[0], vec![0.0, 2.0, 4.0]);
        assert_eq!(timings[1], vec![0.0, 1.5, 3.0, 4.5]);
    }

    #[test]
    fn test_next() {
        let mut poly = Polyrhythm::new(120, vec![3, 4]).unwrap();
        // Expected triggers per tick:
        // Component 1 (3 beats): fires every 12/3 = 4 ticks -> ticks 0, 4, 8.
        // Component 2 (4 beats): fires every 12/4 = 3 ticks -> ticks 0, 3, 6, 9.
        let expected = vec![
            (0, vec![true, true]),
            (1, vec![false, false]),
            (2, vec![false, false]),
            (3, vec![false, true]),
            (4, vec![true, false]),
            (5, vec![false, false]),
            (6, vec![false, true]),
            (7, vec![false, false]),
            (8, vec![true, false]),
            (9, vec![false, true]),
            (10, vec![false, false]),
            (11, vec![false, false]),
        ];

        for (expected_tick, expected_triggers) in expected {
            if let Some((tick, triggers)) = poly.next() {
                assert_eq!(tick, expected_tick);
                assert_eq!(triggers, expected_triggers);
            }
        }
    }
}
