//! Scales built from a caller-supplied cycle of intervals.
//!
//! [`ScaleType`](super::ScaleType) covers music21's *named* `ConcreteScale`
//! subclasses, which are fixed tables and so can be an enum. `CyclicalScale`
//! and `OctaveRepeatingScale` are not: they take an arbitrary interval list at
//! construction, so they are a runtime type rather than a variant.
//!
//! The two differ only in how they close:
//!
//! - [`StepScale::cyclical`] walks the intervals once and stops, so the scale
//!   need not span or repeat at an octave — `["P5"]` from C is just `C G`.
//! - [`StepScale::octave_repeating`] appends whatever interval is needed to
//!   reach the octave above the tonic, so `["m3", "M3"]` from C becomes
//!   `C E- G C`.

use crate::error::Result;
use crate::interval::Interval;
use crate::pitch::Pitch;
use crate::sieve::Sieve;

use std::sync::LazyLock;

/// music21 defaults an absent interval list to a single minor second.
static DEFAULT_STEP: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("m2").expect("m2 is a valid interval name"));

/// A scale built by walking a cycle of intervals from a tonic.
///
/// ```
/// use music21_rs::{Pitch, StepScale};
///
/// let tonic = Pitch::from_name("C4")?;
/// let cyclical = StepScale::cyclical(tonic.clone(), &["m3", "M3"])?;
/// let names: Vec<String> = cyclical.pitches()?.iter().map(|p| p.name()).collect();
/// assert_eq!(names, ["C", "E-", "G"]);
///
/// let repeating = StepScale::octave_repeating(tonic, &["m3", "M3"])?;
/// let names: Vec<String> = repeating.pitches()?.iter().map(|p| p.name()).collect();
/// assert_eq!(names, ["C", "E-", "G", "C"]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
/// `Interval` implements neither `PartialEq` nor `Hash`, so neither is derived
/// here; compare realized [`pitches`](StepScale::pitches) instead.
#[derive(Clone, Debug)]
pub struct StepScale {
    tonic: Pitch,
    steps: Vec<Interval>,
}

impl StepScale {
    /// Builds music21's `CyclicalScale`: the intervals walked once, no closing.
    ///
    /// An empty list defaults to a single `m2`, as music21 does.
    pub fn cyclical(tonic: Pitch, steps: &[&str]) -> Result<Self> {
        Ok(Self {
            tonic,
            steps: parse_steps(steps)?,
        })
    }

    /// Builds music21's `OctaveRepeatingScale`: the intervals plus a closing
    /// interval that completes the octave.
    ///
    /// An empty list defaults to a single `m2`, as music21 does.
    ///
    /// The closing interval is the *complement of the interval sum*, which is
    /// how music21 derives it — not the interval between the last realized
    /// pitch and the octave. The two differ in spelling whenever realization
    /// respells a degree: three `m2` steps from C sum to `dd4`, whose
    /// complement is `AA5`, so the scale closes on `B#` rather than `C`. Taking
    /// it from the realized pitches would read the answer off already-simplified
    /// output and lose that.
    ///
    /// music21's own behaviour for a cycle wider than an octave is erratic —
    /// `["P5", "P5"]` returns pitches an octave above the tonic it was given,
    /// and it mutates the caller's interval list in place. Neither is
    /// reproduced here: the cycle is closed at the octave above the last pitch
    /// and the input is left alone.
    pub fn octave_repeating(tonic: Pitch, steps: &[&str]) -> Result<Self> {
        let mut steps = parse_steps(steps)?;
        steps.push(interval_sum(&tonic, &steps)?.inversion()?);
        Ok(Self { tonic, steps })
    }

    /// Builds music21's `SieveScale` from a Xenakis sieve expression.
    ///
    /// The sieve's interval widths become the cycle, which music21 then treats
    /// as a `CyclicalScale` — so `"3@0"` from C is `C E-`, and the major-scale
    /// sieve gives a major scale.
    ///
    /// music21's `SieveScale` also takes an `eld` (elementary displacement) to
    /// scale the widths for non-semitone steps. Only the default of one
    /// semitone is supported here, since the crate has no microtonal step type
    /// to widen to.
    pub fn sieve(tonic: Pitch, expression: &str) -> Result<Self> {
        let widths = Sieve::parse(expression)?.interval_widths()?;
        let steps = widths
            .into_iter()
            .map(Interval::from_semitones)
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { tonic, steps })
    }

    /// Returns the tonic pitch.
    pub fn tonic(&self) -> &Pitch {
        &self.tonic
    }

    /// Returns the step intervals, including any closing interval.
    pub fn steps(&self) -> &[Interval] {
        &self.steps
    }

    /// Returns the number of steps, which is one fewer than the pitch count.
    pub fn degree_count(&self) -> usize {
        self.steps.len()
    }

    /// Returns the pitches of one pass through the cycle, starting at the tonic.
    pub fn pitches(&self) -> Result<Vec<Pitch>> {
        let mut pitches = Vec::with_capacity(self.steps.len() + 1);
        pitches.push(self.tonic.clone());

        let mut current = self.tonic.clone();
        for step in &self.steps {
            // music21's IntervalNetwork defaults to pitchSimplification
            // 'maxAccidental' with a cap of one, which is what respells the
            // third step of an m2 cycle from E-double-flat to D.
            current = step.transpose_pitch_with_options(&current, false, Some(1))?;
            pitches.push(current.clone());
        }
        Ok(pitches)
    }
}

/// Returns the sum of `steps`, spelled exactly.
///
/// Computed by transposing a reference pitch with simplification switched off
/// and measuring the result, so the sum keeps the accidentals the arithmetic
/// actually produces rather than the ones a realized scale would show.
fn interval_sum(reference: &Pitch, steps: &[Interval]) -> Result<Interval> {
    let mut current = reference.clone();
    for step in steps {
        current = step.transpose_pitch_with_options(&current, false, None)?;
    }
    Interval::between_pitches(reference, &current)
}

fn parse_steps(steps: &[&str]) -> Result<Vec<Interval>> {
    if steps.is_empty() {
        return Ok(vec![DEFAULT_STEP.clone()]);
    }
    steps
        .iter()
        .map(|name| Interval::from_name(*name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(scale: &StepScale) -> Vec<String> {
        scale
            .pitches()
            .expect("scale realizes")
            .iter()
            .map(|pitch| pitch.name())
            .collect()
    }

    fn tonic(name: &str) -> Pitch {
        Pitch::from_name(name).expect("valid tonic")
    }

    #[test]
    fn cyclical_walks_the_cycle_once() {
        let scale = StepScale::cyclical(tonic("C4"), &["P5"]).unwrap();
        assert_eq!(names(&scale), ["C", "G"]);

        let scale = StepScale::cyclical(tonic("C4"), &["m3", "M3"]).unwrap();
        assert_eq!(names(&scale), ["C", "E-", "G"]);
    }

    #[test]
    fn octave_repeating_closes_on_the_octave() {
        let scale = StepScale::octave_repeating(tonic("C4"), &["m3", "M3"]).unwrap();
        assert_eq!(names(&scale), ["C", "E-", "G", "C"]);
        let pitches = scale.pitches().unwrap();
        assert_eq!(pitches.first().unwrap().octave(), Some(4));
        assert_eq!(pitches.last().unwrap().octave(), Some(5));
    }

    #[test]
    fn an_empty_interval_list_defaults_to_a_minor_second() {
        // music21: CyclicalScale() is [C4, D-4]; OctaveRepeatingScale() is
        // [C4, D-4, C5].
        assert_eq!(
            names(&StepScale::cyclical(tonic("C4"), &[]).unwrap()),
            ["C", "D-"]
        );
        assert_eq!(
            names(&StepScale::octave_repeating(tonic("C4"), &[]).unwrap()),
            ["C", "D-", "C"]
        );
    }

    #[test]
    fn the_closing_interval_comes_from_the_interval_sum_not_the_pitches() {
        // Three m2 steps sum to dd4, whose complement is AA5. Realization
        // respells the third degree from E-double-flat to D, so reading the
        // closing interval off the pitches would give a major sixth to C
        // instead. music21 closes on B#, and so does this.
        let scale = StepScale::octave_repeating(tonic("C4"), &["m2", "m2", "m2"]).unwrap();
        assert_eq!(names(&scale), ["C", "D-", "D", "E-", "B#"]);

        // Where realization changes nothing, the two agree: M2+M2+m2 is P4 and
        // its complement P5 closes on the octave.
        let scale = StepScale::octave_repeating(tonic("C4"), &["M2", "M2", "m2"]).unwrap();
        assert_eq!(names(&scale), ["C", "D", "E", "F", "C"]);
        let scale = StepScale::octave_repeating(tonic("F#4"), &["M2", "M2", "m2"]).unwrap();
        assert_eq!(names(&scale), ["F#", "G#", "A#", "B", "F#"]);
    }

    #[test]
    fn a_cycle_wider_than_an_octave_closes_above_it() {
        // Two fifths sum to M9, whose complement is m7, so the cycle closes an
        // octave higher rather than folding back. music21 agrees on the
        // intervals here but reports the pitches an octave off its own tonic.
        let scale = StepScale::octave_repeating(tonic("C4"), &["P5", "P5"]).unwrap();
        assert_eq!(names(&scale), ["C", "G", "D", "C"]);
        let pitches = scale.pitches().unwrap();
        assert_eq!(pitches.first().unwrap().octave(), Some(4));
        assert_eq!(pitches.last().unwrap().octave(), Some(6));
    }

    #[test]
    fn sieve_scales_match_music21() {
        let cases: [(&str, &str, &[&str]); 6] = [
            ("C4", "3@0", &["C", "E-"]),
            ("D4", "3@0", &["D", "F"]),
            ("E-4", "2@0", &["E-", "F"]),
            (
                "C2",
                "(-3@2 & 4) | (-3@1 & 4@1) | (3@2 & 4@2) | (-3 & 4@3)",
                &["C", "D", "E", "F", "G", "A", "B", "C"],
            ),
            (
                "C4",
                "3@0|7@0",
                &["C", "E-", "F#", "G", "A", "C", "D", "E-", "F#", "A"],
            ),
            ("C4", "{3@0|4@0}", &["C", "E-", "E", "F#", "G#", "A", "C"]),
        ];

        for (tonic_name, expression, expected) in cases {
            let scale = StepScale::sieve(tonic(tonic_name), expression).expect("sieve realizes");
            assert_eq!(names(&scale), expected, "{tonic_name} {expression}");
        }
    }

    #[test]
    fn a_sieve_with_no_intervals_errors() {
        assert!(StepScale::sieve(tonic("C4"), "3@1").is_err());
        assert!(StepScale::sieve(tonic("C4"), "not a sieve").is_err());
    }

    #[test]
    fn malformed_interval_names_error_instead_of_panicking() {
        assert!(StepScale::cyclical(tonic("C4"), &["nonsense"]).is_err());
    }
}
