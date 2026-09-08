//! Rank-2 regular temperaments — a period, a generator, and what they mean.
//!
//! A regular temperament is a decision to stop telling two intervals apart.
//! Meantone decides that four fifths and a major third are the same note,
//! which is to say it *tempers out* the syntonic comma, and everything else
//! about meantone follows: the fifth has to shrink to about 697 cents, every
//! 5-limit interval is then some number of those fifths and octaves, and the
//! scales that come out are the pentatonic, the diatonic and the chromatic.
//!
//! What a temperament is, concretely, is a *mapping*: how many periods and how
//! many generators each prime is worth. The wiki writes meantone's as
//! `1; 1 4 10` — one period to the octave, and the fifth reached in one
//! generator, the third in four, the harmonic seventh in ten.
//! [`Temperament::from_mapping`] takes exactly that, and works the period row
//! out for itself.
//!
//! ```
//! use music21_rs::tuningsystem::{Monzo, Temperament};
//!
//! // Meantone: one period to the octave, generator a fifth of 696.7 cents.
//! let meantone = Temperament::from_mapping(1, &[1, 4, 10], 696.7, &[2, 3, 5, 7])?;
//! assert!(meantone.tempers_out(&Monzo::from_ratio(81, 80)?));
//! assert_eq!(meantone.mos(7)?.pattern()?.to_string(), "5L 2s");
//! # Ok::<(), music21_rs::Error>(())
//! ```

use crate::defaults::{FloatType, IntegerType, UnsignedIntegerType};
use crate::error::{Error, Result};
use crate::tuningsystem::monzo::{Monzo, PRIMES};
use crate::tuningsystem::mos::{MosScale, OCTAVE_CENTS};

use std::fmt::{Display, Formatter};

/// How far a derived period count may sit from a whole number.
///
/// The mappings the literature publishes land within a fortieth of a step, so
/// anything past a third of one is a generator that does not go with the
/// mapping it was handed rather than a rounding to be swallowed.
const MAPPING_TOLERANCE: FloatType = 0.35;

/// A rank-2 temperament: a period, a generator, and a mapping onto primes.
///
/// The mapping says how many periods and how many generators each prime of the
/// subgroup is worth. Everything else — how wide a ratio comes out, whether a
/// comma vanishes, which scales the generator makes — is read off that.
///
/// ```
/// use music21_rs::tuningsystem::{Monzo, Temperament};
///
/// // Porcupine, from its own infobox: three generators to a fourth.
/// let porcupine = Temperament::from_mapping(1, &[-3, -5], 163.6, &[2, 3, 5])?;
/// assert!(porcupine.tempers_out(&Monzo::from_ratio(250, 243)?));
/// assert_eq!(porcupine.mos(7)?.pattern()?.to_string(), "1L 6s");
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Temperament {
    primes: Vec<IntegerType>,
    periods: Vec<IntegerType>,
    generators: Vec<IntegerType>,
    period_cents: FloatType,
    generator_cents: FloatType,
}

impl Temperament {
    /// Builds a temperament from the mapping line the literature publishes.
    ///
    /// `periods_per_octave` and `generator_steps` are the two halves of the
    /// wiki's `1; 1 4 10`: one period to the octave, then how many generators
    /// each prime *after the first* is worth. `primes` is the subgroup, which
    /// begins at 2 and need not be every prime up to its largest — mavila is
    /// written over `2.3.5.11`, with no 7 in it.
    ///
    /// The period row is not asked for, because the generator already decides
    /// it: whatever the generators leave over has to be made up in whole
    /// periods. Errors when it cannot be — when some prime needs a fraction of
    /// a period, which means the generator and the mapping do not go together.
    pub fn from_mapping(
        periods_per_octave: UnsignedIntegerType,
        generator_steps: &[IntegerType],
        generator_cents: FloatType,
        primes: &[IntegerType],
    ) -> Result<Self> {
        if periods_per_octave == 0 {
            return Err(Error::TuningSystem(
                "a temperament needs at least one period to the octave".to_owned(),
            ));
        }
        if primes.first() != Some(&2) {
            return Err(Error::TuningSystem(
                "a subgroup is written from 2 upwards and has to begin with it".to_owned(),
            ));
        }
        if let Some(&unknown) = primes.iter().find(|prime| !PRIMES.contains(prime)) {
            return Err(Error::TuningSystem(format!(
                "{unknown} is not a prime this module carries"
            )));
        }
        if generator_steps.len() + 1 != primes.len() {
            return Err(Error::TuningSystem(format!(
                "{} generator steps do not map a subgroup of {} primes",
                generator_steps.len(),
                primes.len()
            )));
        }
        if !generator_cents.is_finite() || generator_cents == 0.0 {
            return Err(Error::TuningSystem(
                "a generator has to be a real number of cents, and not nought".to_owned(),
            ));
        }

        let period_cents = OCTAVE_CENTS / FloatType::from(periods_per_octave);
        let mut periods = vec![IntegerType::try_from(periods_per_octave).map_err(|_| {
            Error::TuningSystem(format!(
                "{periods_per_octave} periods is more than a mapping holds"
            ))
        })?];
        for (&prime, &steps) in primes[1..].iter().zip(generator_steps) {
            let just = OCTAVE_CENTS * FloatType::from(prime).log2();
            let left_over = just - FloatType::from(steps) * generator_cents;
            let count = left_over / period_cents;
            if (count - count.round()).abs() > MAPPING_TOLERANCE {
                return Err(Error::TuningSystem(format!(
                    "a generator of {generator_cents} cents leaves prime {prime} \
                     {count:.3} periods away, which is no whole mapping"
                )));
            }
            periods.push(count.round() as IntegerType);
        }

        Ok(Self {
            primes: primes.to_vec(),
            periods,
            generators: std::iter::once(0)
                .chain(generator_steps.iter().copied())
                .collect(),
            period_cents,
            generator_cents,
        })
    }

    /// The primes the temperament is written over, its subgroup.
    #[must_use]
    pub fn primes(&self) -> &[IntegerType] {
        &self.primes
    }

    /// How many periods each prime is worth, in the subgroup's own order.
    #[must_use]
    pub fn period_map(&self) -> &[IntegerType] {
        &self.periods
    }

    /// How many generators each prime is worth, in the subgroup's own order.
    #[must_use]
    pub fn generator_map(&self) -> &[IntegerType] {
        &self.generators
    }

    /// How many periods there are to an octave.
    #[must_use]
    pub fn periods_per_octave(&self) -> IntegerType {
        self.periods.first().copied().unwrap_or(1)
    }

    /// The period, in cents.
    #[must_use]
    pub fn period_cents(&self) -> FloatType {
        self.period_cents
    }

    /// The generator, in cents.
    #[must_use]
    pub fn generator_cents(&self) -> FloatType {
        self.generator_cents
    }

    /// How many periods and how many generators `interval` comes to.
    ///
    /// Errors on an interval using a prime outside the subgroup, since the
    /// temperament has said nothing about it — reading that as nought steps
    /// would be an answer it has not got.
    pub fn map(&self, interval: &Monzo) -> Result<(IntegerType, IntegerType)> {
        let mut periods = 0;
        let mut generators = 0;
        for (index, &exponent) in interval.exponents().iter().enumerate() {
            if exponent == 0 {
                continue;
            }
            let prime = PRIMES[index];
            let place = self
                .primes
                .iter()
                .position(|&known| known == prime)
                .ok_or_else(|| {
                    Error::TuningSystem(format!(
                        "{prime} is outside the subgroup {}",
                        self.subgroup_name()
                    ))
                })?;
            periods += exponent * self.periods[place];
            generators += exponent * self.generators[place];
        }
        Ok((periods, generators))
    }

    /// How wide `interval` comes out once tempered, in cents.
    pub fn cents(&self, interval: &Monzo) -> Result<FloatType> {
        let (periods, generators) = self.map(interval)?;
        Ok(FloatType::from(periods) * self.period_cents
            + FloatType::from(generators) * self.generator_cents)
    }

    /// How far off just `interval` sounds here, in cents; positive is sharp.
    pub fn error_cents(&self, interval: &Monzo) -> Result<FloatType> {
        Ok(self.cents(interval)? - interval.cents())
    }

    /// Whether `comma` vanishes — no periods and no generators at all.
    ///
    /// A comma using a prime the subgroup has not got is not tempered out; the
    /// temperament has said nothing about it either way.
    #[must_use]
    pub fn tempers_out(&self, comma: &Monzo) -> bool {
        self.map(comma).is_ok_and(|steps| steps == (0, 0))
    }

    /// The scale of `notes` notes to a period that this generator makes.
    ///
    /// The count is notes per *period*, which is notes per octave only for a
    /// temperament with one period to the octave. Errors where
    /// [`MosScale::new`] does.
    pub fn mos(&self, notes: UnsignedIntegerType) -> Result<MosScale> {
        MosScale::new(self.generator_cents, self.period_cents, notes)
    }

    /// Every note count up to `most` at which this generator makes a moment of symmetry.
    ///
    /// This is the temperament's list of MOS scales, and it starts lower than a
    /// published one does: a two- or three-note moment is real but nobody
    /// bothers writing it down.
    pub fn moments(&self, most: UnsignedIntegerType) -> Result<Vec<UnsignedIntegerType>> {
        crate::tuningsystem::mos::moment_of_symmetry_sizes(
            self.generator_cents,
            self.period_cents,
            most,
        )
    }

    /// The subgroup written the way the literature writes it, `2.3.5.11`.
    #[must_use]
    pub fn subgroup_name(&self) -> String {
        self.primes
            .iter()
            .map(IntegerType::to_string)
            .collect::<Vec<_>>()
            .join(".")
    }
}

impl Display for Temperament {
    /// Writes the subgroup, the mapping line and the generator, as an infobox does.
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.subgroup_name(), self.periods_per_octave())?;
        for (position, steps) in self.generators.iter().skip(1).enumerate() {
            write!(f, "{} {steps}", if position == 0 { ";" } else { "" })?;
        }
        write!(f, " (generator {:.1}¢)", self.generator_cents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mapping, generator and MOS list of each temperament's own infobox on
    /// the Xenharmonic Wiki, read back off the crate.
    ///
    /// The MOS lists are the ones the wiki publishes. Ours starts lower — a
    /// two-note moment is real and nobody writes it down — so the published
    /// list is checked to be the tail of what we find.
    #[test]
    fn published_temperaments_read_back_the_way_they_are_published() {
        struct Published {
            name: &'static str,
            periods_per_octave: UnsignedIntegerType,
            generator_steps: &'static [IntegerType],
            generator_cents: FloatType,
            primes: &'static [IntegerType],
            comma: (IntegerType, IntegerType),
            period_map: &'static [IntegerType],
            moments: &'static [(UnsignedIntegerType, &'static str)],
        }

        for published in [
            Published {
                name: "meantone",
                periods_per_octave: 1,
                generator_steps: &[1, 4, 10],
                generator_cents: 696.7,
                primes: &[2, 3, 5, 7],
                comma: (81, 80),
                period_map: &[1, 1, 0, -3],
                moments: &[(5, "2L 3s"), (7, "5L 2s"), (12, "7L 5s"), (19, "12L 7s")],
            },
            Published {
                name: "mavila",
                periods_per_octave: 1,
                generator_steps: &[1, -3, -1],
                generator_cents: 679.0,
                primes: &[2, 3, 5, 11],
                comma: (135, 128),
                period_map: &[1, 1, 4, 4],
                moments: &[(5, "2L 3s"), (7, "2L 5s"), (9, "7L 2s")],
            },
            Published {
                name: "porcupine",
                periods_per_octave: 1,
                generator_steps: &[-3, -5, 6, -4],
                generator_cents: 163.6,
                primes: &[2, 3, 5, 7, 11],
                comma: (250, 243),
                period_map: &[1, 2, 3, 2, 4],
                moments: &[(7, "1L 6s"), (8, "7L 1s"), (15, "7L 8s")],
            },
        ] {
            let temperament = Temperament::from_mapping(
                published.periods_per_octave,
                published.generator_steps,
                published.generator_cents,
                published.primes,
            )
            .unwrap_or_else(|error| panic!("{} did not build: {error}", published.name));

            // The period row is derived, not given, so it is worth checking.
            assert_eq!(
                temperament.period_map(),
                published.period_map,
                "{}'s period map",
                published.name
            );

            let comma = Monzo::from_ratio(published.comma.0, published.comma.1)
                .expect("a factorable comma");
            assert!(
                temperament.tempers_out(&comma),
                "{} should temper out {}/{}",
                published.name,
                published.comma.0,
                published.comma.1
            );

            for &(notes, pattern) in published.moments {
                assert_eq!(
                    temperament
                        .mos(notes)
                        .expect("a generator")
                        .pattern()
                        .expect("a moment")
                        .to_string(),
                    pattern,
                    "{} at {notes} notes",
                    published.name
                );
            }

            // Every published moment is one we find, in the same order.
            let largest = published.moments.last().expect("a published list").0;
            let found = temperament.moments(largest).expect("a generator");
            let published_counts: Vec<_> =
                published.moments.iter().map(|&(notes, _)| notes).collect();
            for notes in &published_counts {
                assert!(
                    found.contains(notes),
                    "{} should find its published {notes}-note moment among {found:?}",
                    published.name
                );
            }
        }
    }

    #[test]
    fn meantone_maps_the_intervals_it_is_named_for() {
        let meantone =
            Temperament::from_mapping(1, &[1, 4, 10], 696.7, &[2, 3, 5, 7]).expect("meantone");
        let fifth = Monzo::from_ratio(3, 2).expect("the fifth");
        let third = Monzo::from_ratio(5, 4).expect("the third");

        // One generator up, one period down: the fifth is the generator.
        assert_eq!(meantone.map(&fifth).expect("the fifth"), (0, 1));
        // Four fifths less two octaves is the major third.
        assert_eq!(meantone.map(&third).expect("the third"), (-2, 4));
        assert!((meantone.cents(&fifth).expect("the fifth") - 696.7).abs() < 1e-9);

        // Four fifths of 696.7 less two octaves is 386.8 cents, half a cent
        // sharp of a just third — which is what the comma costs when it is
        // paid off over four fifths instead of landing on one interval.
        let error = meantone.error_cents(&third).expect("the third");
        assert!((error - 0.486).abs() < 0.01, "{error}");

        // The Pythagorean comma does not vanish in meantone; the syntonic does.
        assert!(!meantone.tempers_out(&Monzo::from_ratio(531_441, 524_288).expect("a comma")));
        assert!(meantone.tempers_out(&Monzo::from_ratio(81, 80).expect("a comma")));
    }

    #[test]
    fn a_prime_outside_the_subgroup_has_no_answer_rather_than_a_wrong_one() {
        // Mavila is written over 2.3.5.11, with no 7 in it.
        let mavila =
            Temperament::from_mapping(1, &[1, -3, -1], 679.0, &[2, 3, 5, 11]).expect("mavila");
        assert_eq!(mavila.subgroup_name(), "2.3.5.11");
        let septimal = Monzo::from_ratio(7, 4).expect("the harmonic seventh");
        assert!(mavila.map(&septimal).is_err());
        assert!(mavila.cents(&septimal).is_err());
        // A comma it cannot see is not one it tempers out.
        assert!(!mavila.tempers_out(&Monzo::from_ratio(64, 63).expect("a comma")));
        // One it can see, it does.
        assert!(mavila.tempers_out(&Monzo::from_ratio(135, 128).expect("a comma")));
    }

    #[test]
    fn a_generator_that_does_not_go_with_its_mapping_is_refused() {
        // Meantone's mapping with porcupine's generator maps nothing whole.
        assert!(Temperament::from_mapping(1, &[1, 4, 10], 163.6, &[2, 3, 5, 7]).is_err());
        // A subgroup has to start at 2, and hold primes this module carries.
        assert!(Temperament::from_mapping(1, &[4, 10], 696.7, &[3, 5, 7]).is_err());
        assert!(Temperament::from_mapping(1, &[1, 4], 696.7, &[2, 3, 9]).is_err());
        // The mapping has to be as long as the subgroup, less its first prime.
        assert!(Temperament::from_mapping(1, &[1, 4], 696.7, &[2, 3, 5, 7]).is_err());
        // A period and a generator both have to be something.
        assert!(Temperament::from_mapping(0, &[1, 4, 10], 696.7, &[2, 3, 5, 7]).is_err());
        assert!(Temperament::from_mapping(1, &[1, 4, 10], 0.0, &[2, 3, 5, 7]).is_err());
    }

    #[test]
    fn a_temperament_writes_its_infobox_line_back() {
        let meantone =
            Temperament::from_mapping(1, &[1, 4, 10], 696.7, &[2, 3, 5, 7]).expect("meantone");
        assert_eq!(meantone.to_string(), "2.3.5.7 1; 1 4 10 (generator 696.7¢)");
        assert_eq!(meantone.periods_per_octave(), 1);
        assert_eq!(meantone.primes(), [2, 3, 5, 7]);
        assert_eq!(meantone.generator_map(), [0, 1, 4, 10]);
        assert!((meantone.period_cents() - OCTAVE_CENTS).abs() < 1e-9);
    }
}
