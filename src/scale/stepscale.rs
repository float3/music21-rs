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

use crate::defaults::{FloatType, IntegerType};
use crate::error::Result;
use crate::interval::{ChromaticInterval, Interval};
use crate::pitch::Pitch;
use crate::scale::Scale;
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
/// assert_eq!(names, ["C", "Eb", "G"]);
///
/// let repeating = StepScale::octave_repeating(tonic, &["m3", "M3"])?;
/// let names: Vec<String> = repeating.pitches()?.iter().map(|p| p.name()).collect();
/// assert_eq!(names, ["C", "Eb", "G", "C"]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
/// `Interval` implements neither `PartialEq` nor `Hash`, so neither is derived
/// here; compare realized [`pitches`](StepScale::pitches) instead.
#[derive(Clone, Debug)]
#[must_use]
pub struct StepScale {
    tonic: Pitch,
    steps: Vec<Interval>,
    /// Whether the cycle was closed at the octave, which makes it a scale
    /// that repeats there rather than a cycle walked from its tonic.
    closed: bool,
}

impl StepScale {
    /// Builds music21's `CyclicalScale`: the intervals walked once, no closing.
    ///
    /// An empty list defaults to a single `m2`, as music21 does.
    pub fn cyclical(tonic: Pitch, steps: &[&str]) -> Result<Self> {
        Ok(Self::cyclical_of(tonic, parse_steps(steps)?))
    }

    /// The same from intervals already in hand, which is the only way to
    /// give a step that has no name: one measured in cents.
    pub fn cyclical_of(tonic: Pitch, steps: Vec<Interval>) -> Self {
        Self {
            tonic,
            steps: or_default(steps),
            closed: false,
        }
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
        Self::octave_repeating_of(tonic, parse_steps(steps)?)
    }

    /// The same from intervals already in hand.
    pub fn octave_repeating_of(tonic: Pitch, steps: Vec<Interval>) -> Result<Self> {
        let mut steps = or_default(steps);
        let closing = match interval_sum(&tonic, &steps).and_then(|sum| sum.inversion()) {
            Ok(closing) => closing,
            // A sum no accidental can spell -- eleven minor seconds come to a
            // twelfth diminished ten times over -- still has a size, and the
            // cycle closes by what is left of the octave above it.
            Err(_) => {
                let risen: FloatType = steps.iter().map(Interval::semitones).sum();
                let octaves = (risen / 12.0).floor() + 1.0;
                Interval::from_semitones((octaves * 12.0 - risen).round() as IntegerType)?
            }
        };
        steps.push(closing);
        Ok(Self {
            tonic,
            steps,
            closed: true,
        })
    }

    /// Builds music21's `SieveScale` from a Xenakis sieve expression.
    ///
    /// The sieve's interval widths become the cycle, which music21 then treats
    /// as a `CyclicalScale` — so `"3@0"` from C is `C E-`, and the major-scale
    /// sieve gives a major scale.
    ///
    /// The widths are counted in semitones; [`Self::sieve_by`] counts them in
    /// anything else.
    pub fn sieve(tonic: Pitch, expression: &str) -> Result<Self> {
        Self::sieve_by(tonic, expression, 1.0)
    }

    /// The same with the sieve's unit said: music21's `eld`, the elementary
    /// displacement, in semitones. A sieve of every integer counted in twos
    /// is a whole-tone scale, and counted in halves is a scale of quarter
    /// tones.
    pub fn sieve_by(tonic: Pitch, expression: &str, eld: FloatType) -> Result<Self> {
        let widths = Sieve::parse(expression)?.interval_widths()?;
        let steps = widths
            .into_iter()
            .map(|width| {
                Interval::from_chromatic(ChromaticInterval::new(FloatType::from(width) * eld)?)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            tonic,
            steps,
            closed: false,
        })
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

    /// The same cycle as a [`Scale`], which is what answers everything a
    /// scale is asked past its own notes: a range of them, the degree a note
    /// stands on, the note beside another.
    pub fn scale(&self) -> Result<Scale> {
        if self.closed {
            Scale::from_steps(self.tonic.clone(), self.steps.clone())
        } else {
            Scale::from_cycle(self.tonic.clone(), self.steps.clone())
        }
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

/// music21 reads no steps at all as a single minor second.
fn or_default(steps: Vec<Interval>) -> Vec<Interval> {
    if steps.is_empty() {
        vec![DEFAULT_STEP.clone()]
    } else {
        steps
    }
}

fn parse_steps(steps: &[&str]) -> Result<Vec<Interval>> {
    steps
        .iter()
        .map(|name| Interval::from_name(*name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_scale_reports_its_tonic_and_steps() {
        let tonic = Pitch::from_name("C4").unwrap();
        let scale = StepScale::cyclical(tonic.clone(), &["M2", "M2"]).unwrap();
        assert_eq!(scale.tonic(), &tonic);
        assert_eq!(scale.steps().len(), 2);
        assert_eq!(scale.degree_count(), 2);
    }

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
    fn a_step_scale_answers_as_a_scale() {
        // Read off music21 11.0.0b9: `OctaveRepeatingScale('c4', ['m3', 'M3'])`
        // asked for `getPitches('g2', 'g4')`, the degree of three notes, and
        // `nextPitch` from `c4`, down from `e-5`, and two steps up from `g3`.
        let scale = StepScale::octave_repeating(tonic("C4"), &["m3", "M3"])
            .unwrap()
            .scale()
            .unwrap();
        let spelled = |pitches: Vec<Pitch>| -> Vec<String> {
            pitches.iter().map(Pitch::name_with_octave).collect()
        };
        assert_eq!(
            spelled(scale.pitches_between(&tonic("G2"), &tonic("G4")).unwrap()),
            ["G2", "C3", "Eb3", "G3", "C4", "Eb4", "G4"]
        );
        // The closing octave is the tonic again, not a fourth degree.
        assert_eq!(scale.degree_count(), 3);
        assert_eq!(scale.degree_of(&tonic("C4")).unwrap(), Some(1));
        assert_eq!(scale.degree_of(&tonic("E-")).unwrap(), Some(2));
        assert_eq!(scale.degree_of(&tonic("G5")).unwrap(), Some(3));
        assert_eq!(scale.degree_of(&tonic("D4")).unwrap(), None);
        let next = |origin: &str, steps: usize| {
            scale
                .next_pitch_above(&tonic(origin), steps)
                .unwrap()
                .name_with_octave()
        };
        assert_eq!(next("C4", 1), "Eb4");
        assert_eq!(next("G3", 2), "Eb4");
        assert_eq!(
            scale
                .next_pitch_below(&tonic("Eb5"), 1)
                .unwrap()
                .name_with_octave(),
            "C5"
        );
    }

    #[test]
    fn a_respelling_scale_comes_down_by_its_own_walk() {
        // music21: `OctaveRepeatingScale('d', ['m2', 'm2', 'm2'])`, going up
        // and coming down. Going up the second step is an `F-`; coming down
        // from `F` the same sound is an `E`, and the one below it a `D#`.
        let scale = StepScale::octave_repeating(tonic("D"), &["m2", "m2", "m2"])
            .unwrap()
            .scale()
            .unwrap();
        let spelled = |pitches: Vec<Pitch>| -> Vec<String> {
            pitches.iter().map(Pitch::name_with_octave).collect()
        };
        assert_eq!(
            spelled(scale.pitches_between(&tonic("D4"), &tonic("D5")).unwrap()),
            ["D4", "Eb4", "Fb4", "F4", "D5"]
        );
        assert_eq!(
            spelled(
                scale
                    .pitches_between_descending(&tonic("D4"), &tonic("D5"))
                    .unwrap()
            ),
            ["D5", "F4", "E4", "D#4", "D4"]
        );
    }

    #[test]
    fn a_cycle_that_does_not_close_at_the_octave_is_walked_both_ways() {
        // music21's own `testCyclicalScales` and `testSieveScaleA`.
        let spelled = |pitches: Vec<Pitch>| -> Vec<String> {
            pitches.iter().map(ToString::to_string).collect()
        };
        let fifths = StepScale::cyclical(tonic("C4"), &["P5"])
            .unwrap()
            .scale()
            .unwrap();
        assert_eq!(spelled(fifths.pitches().unwrap()), ["C4", "G4"]);
        assert_eq!(
            spelled(fifths.pitches_between(&tonic("G2"), &tonic("G6")).unwrap()),
            ["Bb2", "F3", "C4", "G4", "D5", "A5", "E6"]
        );
        // A cycle of one step has one degree, and every note of it is that.
        assert_eq!(fifths.degree_of(&tonic("G4")).unwrap(), Some(1));
        assert_eq!(fifths.degree_of(&tonic("Bb2")).unwrap(), Some(1));
        // A degree is looked for an octave either side of the note, which is
        // where music21 looks: the cycle's F# is six fifths up, far past F#3.
        assert_eq!(fifths.degree_of(&tonic("F#3")).unwrap(), None);
        assert_eq!(fifths.degree_of(&tonic("F3")).unwrap(), Some(1));

        // A cycle of two steps may stand one name on both of them: G3 is its
        // first degree and G4 its second, and both lie within an octave of G4.
        let mixed = StepScale::cyclical(tonic("C4"), &["M2", "m3"])
            .unwrap()
            .scale()
            .unwrap();
        assert_eq!(
            mixed
                .degrees_of_by(&tonic("G4"), crate::scale::DegreeComparison::Name)
                .unwrap(),
            [1, 2]
        );
        assert_eq!(
            fifths
                .next_pitch_above(&tonic("G4"), 2)
                .unwrap()
                .to_string(),
            "A5"
        );

        let seconds = StepScale::cyclical(tonic("C4"), &["m2", "m2"])
            .unwrap()
            .scale()
            .unwrap();
        assert_eq!(
            spelled(seconds.pitches_between(&tonic("G4"), &tonic("G5")).unwrap()),
            [
                "G4", "Ab4", "A4", "Bb4", "Cb5", "C5", "Db5", "D5", "Eb5", "Fb5", "F5", "Gb5", "G5"
            ]
        );

        // A sieve of every integer, counted in whole tones and then in
        // quarter tones, each walked down from D4.
        let whole = StepScale::sieve_by(tonic("D4"), "1@0", 2.0)
            .unwrap()
            .scale()
            .unwrap();
        assert_eq!(
            spelled(whole.pitches_between(&tonic("C2"), &tonic("C3")).unwrap()),
            ["C2", "D2", "Fb2", "Gb2", "Ab2", "Bb2", "C3"]
        );
        let quarter = StepScale::sieve_by(tonic("D4"), "1@0", 0.5)
            .unwrap()
            .scale()
            .unwrap();
        assert_eq!(
            spelled(quarter.pitches_between(&tonic("C2"), &tonic("D2")).unwrap()),
            ["C2", "C~2", "Db2", "D`2", "D2"]
        );
    }

    #[test]
    fn a_cycle_whose_sum_cannot_be_spelled_still_closes() {
        let steps = ["m2"; 11];
        let scale = StepScale::octave_repeating(tonic("C4"), &steps).unwrap();
        let pitches = scale.pitches().unwrap();
        assert_eq!(pitches.len(), 13);
        assert_eq!(pitches.last().unwrap().ps(), 72.0);
    }

    #[test]
    fn cyclical_walks_the_cycle_once() {
        let scale = StepScale::cyclical(tonic("C4"), &["P5"]).unwrap();
        assert_eq!(names(&scale), ["C", "G"]);

        let scale = StepScale::cyclical(tonic("C4"), &["m3", "M3"]).unwrap();
        assert_eq!(names(&scale), ["C", "Eb", "G"]);
    }

    #[test]
    fn octave_repeating_closes_on_the_octave() {
        let scale = StepScale::octave_repeating(tonic("C4"), &["m3", "M3"]).unwrap();
        assert_eq!(names(&scale), ["C", "Eb", "G", "C"]);
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
            ["C", "Db"]
        );
        assert_eq!(
            names(&StepScale::octave_repeating(tonic("C4"), &[]).unwrap()),
            ["C", "Db", "C"]
        );
    }

    #[test]
    fn the_closing_interval_comes_from_the_interval_sum_not_the_pitches() {
        // Three m2 steps sum to dd4, whose complement is AA5. Realization
        // respells the third degree from E-double-flat to D, so reading the
        // closing interval off the pitches would give a major sixth to C
        // instead. music21 closes on B#, and so does this.
        let scale = StepScale::octave_repeating(tonic("C4"), &["m2", "m2", "m2"]).unwrap();
        assert_eq!(names(&scale), ["C", "Db", "D", "Eb", "B#"]);

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
            ("C4", "3@0", &["C", "Eb"]),
            ("D4", "3@0", &["D", "F"]),
            ("E-4", "2@0", &["Eb", "F"]),
            (
                "C2",
                "(-3@2 & 4) | (-3@1 & 4@1) | (3@2 & 4@2) | (-3 & 4@3)",
                &["C", "D", "E", "F", "G", "A", "B", "C"],
            ),
            (
                "C4",
                "3@0|7@0",
                &["C", "Eb", "F#", "G", "A", "C", "D", "Eb", "F#", "A"],
            ),
            ("C4", "{3@0|4@0}", &["C", "Eb", "E", "F#", "G#", "A", "C"]),
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
