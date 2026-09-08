//! Regular temperaments — a period, some generators, and what they mean.
//!
//! A regular temperament is a decision to stop telling two intervals apart.
//! Meantone decides that four fifths and a major third are the same note,
//! which is to say it *tempers out* the syntonic comma, and everything else
//! about meantone follows: the fifth has to shrink to about 697 cents, every
//! 5-limit interval is then some number of those fifths and octaves, and the
//! scales that come out are the pentatonic, the diatonic and the chromatic.
//!
//! What a temperament is, concretely, is a *mapping*: how many periods and how
//! many of each generator every prime is worth. The wiki writes meantone's as
//! `1; 1 4 10` — one period to the octave, and the fifth reached in one
//! generator, the third in four, the harmonic seventh in ten.
//! [`Temperament::from_mapping`] takes exactly that, and works the period row
//! out for itself.
//!
//! Two things here are more general than the usual account, because the data
//! is. A temperament need not have **one** generator: marvel has two, so its
//! mapping is two rows and [`Temperament::from_mapping_rows`] takes them.
//! And a temperament need not repeat at the **octave**: a subgroup written
//! `3.5.7` has no 2 in it at all, and its period divides a tritave instead.
//! The interval a temperament repeats at is its *equave*, and the octave is
//! only the usual one.
//!
//! ```
//! use music21_rs::tuningsystem::{Monzo, Temperament};
//!
//! // Meantone: one period to the octave, generator a fifth of 696.7 cents.
//! let meantone = Temperament::from_mapping(1, &[1, 4, 10], 696.7, &[2, 3, 5, 7])?;
//! assert!(meantone.tempers_out(&Monzo::from_ratio(81, 80)?));
//! assert_eq!(meantone.pattern(7)?.to_string(), "5L 2s");
//! # Ok::<(), music21_rs::Error>(())
//! ```

use crate::defaults::{FloatType, IntegerType, UnsignedIntegerType};
use crate::error::{Error, Result};
use crate::tuningsystem::monzo::{Monzo, PRIMES};
use crate::tuningsystem::mos::{Mos, MosScale, OCTAVE_CENTS};

use std::fmt::{Display, Formatter};

/// How far a derived period count may sit from a whole number.
///
/// The mappings the literature publishes land within a fortieth of a step, so
/// anything past a third of one is a generator that does not go with the
/// mapping it was handed rather than a rounding to be swallowed.
const MAPPING_TOLERANCE: FloatType = 0.35;

/// A regular temperament: a period, some generators, and a mapping onto primes.
///
/// The mapping says how many periods and how many of each generator every
/// prime of the subgroup is worth. Everything else — how wide a ratio comes
/// out, whether a comma vanishes, which scales the generator makes — is read
/// off that.
///
/// The number of rows is the temperament's [`Temperament::rank`]: one for the
/// period plus one for each generator. Rank 2 is the common case and the only
/// one where moments of symmetry mean anything, since those come of stacking a
/// single generator.
///
/// ```
/// use music21_rs::tuningsystem::{Monzo, Temperament};
///
/// // Porcupine, from its own infobox: three generators to a fourth.
/// let porcupine = Temperament::from_mapping(1, &[-3, -5], 163.6, &[2, 3, 5])?;
/// assert!(porcupine.tempers_out(&Monzo::from_ratio(250, 243)?));
/// assert_eq!(porcupine.pattern(7)?.to_string(), "1L 6s");
/// assert_eq!(porcupine.rank(), 2);
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Temperament {
    primes: Vec<IntegerType>,
    periods: Vec<IntegerType>,
    generators: Vec<Vec<IntegerType>>,
    equave_cents: FloatType,
    period_cents: FloatType,
    generator_cents: Vec<FloatType>,
}

impl Temperament {
    /// Builds a rank-2 temperament from the mapping line the literature publishes.
    ///
    /// `periods_per_equave` and `generator_steps` are the two halves of the
    /// wiki's `1; 1 4 10`: one period to the octave, then how many generators
    /// each prime *after the first* is worth. `primes` is the subgroup, whose
    /// first prime is the equave — usually 2, but `3.5.7` repeats at a tritave.
    /// A subgroup need not be every prime up to its largest: mavila is written
    /// over `2.3.5.11`, with no 7 in it.
    pub fn from_mapping(
        periods_per_equave: UnsignedIntegerType,
        generator_steps: &[IntegerType],
        generator_cents: FloatType,
        primes: &[IntegerType],
    ) -> Result<Self> {
        Self::from_mapping_rows(
            periods_per_equave,
            &[generator_steps],
            &[generator_cents],
            primes,
        )
    }

    /// Builds a temperament of any rank from one mapping row per generator.
    ///
    /// Each row says how many of *that* generator each prime after the equave
    /// is worth, so every row is one shorter than the subgroup. Marvel's two
    /// rows over `2.3.5.7.11` are `1 0 2 -1` and `0 1 2 -3`, tuned to a fifth
    /// and a major third.
    ///
    /// The period row is not asked for, because the generators already decide
    /// it: whatever they leave over has to be made up in whole periods. Errors
    /// when it cannot be — when some prime needs a fraction of a period, which
    /// means the generators and the mapping do not go together.
    pub fn from_mapping_rows(
        periods_per_equave: UnsignedIntegerType,
        generator_rows: &[&[IntegerType]],
        generator_cents: &[FloatType],
        primes: &[IntegerType],
    ) -> Result<Self> {
        if periods_per_equave == 0 {
            return Err(Error::TuningSystem(
                "a temperament needs at least one period to the equave".to_owned(),
            ));
        }
        let Some(&equave) = primes.first() else {
            return Err(Error::TuningSystem(
                "a subgroup needs at least the prime it repeats at".to_owned(),
            ));
        };
        if let Some(&unknown) = primes.iter().find(|prime| !PRIMES.contains(prime)) {
            return Err(Error::TuningSystem(format!(
                "{unknown} is not a prime this module carries"
            )));
        }
        if primes.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(Error::TuningSystem(
                "a subgroup is written lowest prime first, with no prime twice".to_owned(),
            ));
        }
        if generator_rows.len() != generator_cents.len() {
            return Err(Error::TuningSystem(format!(
                "{} mapping rows do not go with {} generator tunings",
                generator_rows.len(),
                generator_cents.len()
            )));
        }
        for row in generator_rows {
            if row.len() + 1 != primes.len() {
                return Err(Error::TuningSystem(format!(
                    "a mapping row of {} steps does not map a subgroup of {} primes",
                    row.len(),
                    primes.len()
                )));
            }
        }
        if let Some(bad) = generator_cents
            .iter()
            .find(|cents| !cents.is_finite() || **cents == 0.0)
        {
            return Err(Error::TuningSystem(format!(
                "{bad} is not a width a generator can have"
            )));
        }

        let equave_cents = OCTAVE_CENTS * FloatType::from(equave).log2();
        let period_cents = equave_cents / FloatType::from(periods_per_equave);
        let mut periods = vec![IntegerType::try_from(periods_per_equave).map_err(|_| {
            Error::TuningSystem(format!(
                "{periods_per_equave} periods is more than a mapping holds"
            ))
        })?];
        for (place, &prime) in primes.iter().enumerate().skip(1) {
            let just = OCTAVE_CENTS * FloatType::from(prime).log2();
            let left_over = generator_rows
                .iter()
                .zip(generator_cents)
                .map(|(row, cents)| FloatType::from(row[place - 1]) * cents)
                .fold(just, |left, taken| left - taken);
            let count = left_over / period_cents;
            if (count - count.round()).abs() > MAPPING_TOLERANCE {
                return Err(Error::TuningSystem(format!(
                    "the generators leave prime {prime} {count:.3} periods away, \
                     which is no whole mapping"
                )));
            }
            periods.push(count.round() as IntegerType);
        }

        Ok(Self {
            primes: primes.to_vec(),
            periods,
            // The equave takes none of any generator: it is what the periods
            // divide, so its column is the period count and nothing else.
            generators: generator_rows
                .iter()
                .map(|row| std::iter::once(0).chain(row.iter().copied()).collect())
                .collect(),
            equave_cents,
            period_cents,
            generator_cents: generator_cents.to_vec(),
        })
    }

    /// How many rows the mapping has: one for the period, one per generator.
    ///
    /// Two is the usual case, and the only one where a moment of symmetry
    /// means anything.
    #[must_use]
    pub fn rank(&self) -> usize {
        1 + self.generators.len()
    }

    /// The primes the temperament is written over, its subgroup.
    #[must_use]
    pub fn primes(&self) -> &[IntegerType] {
        &self.primes
    }

    /// The prime the temperament repeats at — 2 for an octave, 3 for a tritave.
    #[must_use]
    pub fn equave(&self) -> IntegerType {
        self.primes.first().copied().unwrap_or(2)
    }

    /// How wide the equave is, in cents.
    #[must_use]
    pub fn equave_cents(&self) -> FloatType {
        self.equave_cents
    }

    /// Whether the temperament repeats at the octave, as most do.
    #[must_use]
    pub fn repeats_at_the_octave(&self) -> bool {
        self.equave() == 2
    }

    /// How many periods each prime is worth, in the subgroup's own order.
    #[must_use]
    pub fn period_map(&self) -> &[IntegerType] {
        &self.periods
    }

    /// How many of each generator every prime is worth, one row per generator.
    #[must_use]
    pub fn generator_map(&self) -> &[Vec<IntegerType>] {
        &self.generators
    }

    /// How many periods there are to an equave.
    #[must_use]
    pub fn periods_per_equave(&self) -> IntegerType {
        self.periods.first().copied().unwrap_or(1)
    }

    /// The period, in cents.
    #[must_use]
    pub fn period_cents(&self) -> FloatType {
        self.period_cents
    }

    /// Each generator's width, in cents.
    #[must_use]
    pub fn generator_cents(&self) -> &[FloatType] {
        &self.generator_cents
    }

    /// How many periods and how many of each generator `interval` comes to.
    ///
    /// The answer is one number per mapping row, the period's first. Errors on
    /// an interval using a prime outside the subgroup, since the temperament
    /// has said nothing about it — reading that as nought steps would be an
    /// answer it has not got.
    pub fn map(&self, interval: &Monzo) -> Result<Vec<IntegerType>> {
        let mut steps = vec![0; self.rank()];
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
            steps[0] += exponent * self.periods[place];
            for (row, generator) in self.generators.iter().enumerate() {
                steps[row + 1] += exponent * generator[place];
            }
        }
        Ok(steps)
    }

    /// How wide `interval` comes out once tempered, in cents.
    pub fn cents(&self, interval: &Monzo) -> Result<FloatType> {
        let steps = self.map(interval)?;
        Ok(FloatType::from(steps[0]) * self.period_cents
            + steps[1..]
                .iter()
                .zip(&self.generator_cents)
                .map(|(count, cents)| FloatType::from(*count) * cents)
                .sum::<FloatType>())
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
        self.map(comma)
            .is_ok_and(|steps| steps.iter().all(|count| *count == 0))
    }

    /// The single generator, for a rank-2 temperament.
    ///
    /// Errors for any other rank: a moment of symmetry comes of stacking *one*
    /// interval, so there is nothing to stack when there are two.
    fn sole_generator(&self) -> Result<FloatType> {
        match self.generator_cents.as_slice() {
            [only] => Ok(*only),
            other => Err(Error::TuningSystem(format!(
                "a temperament with {} generators makes no moment of symmetry; \
                 that needs exactly one",
                other.len()
            ))),
        }
    }

    /// The scale of `notes` notes to an equave that this generator makes.
    ///
    /// The count is notes per **equave**, the way the literature counts them.
    ///
    /// A temperament with more than one period to the equave repeats its
    /// pattern in each, so the scale this returns covers one period and the
    /// count has to divide by [`Temperament::periods_per_equave`] — augmented
    /// has three periods, and its `3L 3s` is one large and one small step in
    /// each of them. Errors on a count that does not divide, on a rank other
    /// than 2, and where [`MosScale::new`] does.
    pub fn mos(&self, notes: UnsignedIntegerType) -> Result<MosScale> {
        let generator = self.sole_generator()?;
        let periods = self.periods_per_equave().unsigned_abs();
        if periods == 0 || !notes.is_multiple_of(periods) {
            return Err(Error::TuningSystem(format!(
                "{notes} notes do not divide into {periods} periods to the equave"
            )));
        }
        MosScale::new(generator, self.period_cents, notes / periods)
    }

    /// The step pattern of `notes` notes to the equave.
    ///
    /// This is what a temperament's published MOS list names. It is the
    /// pattern of one period repeated in each, so augmented's two-note period
    /// over three periods is `3L 3s` and not `1L 1s`.
    pub fn pattern(&self, notes: UnsignedIntegerType) -> Result<Mos> {
        let period = self.mos(notes)?.pattern()?;
        let periods = self.periods_per_equave().unsigned_abs();
        Mos::new(period.large() * periods, period.small() * periods)
    }

    /// Every note count to the equave, up to `most`, that makes a moment of symmetry.
    ///
    /// This is the temperament's list of MOS scales, and it starts lower than a
    /// published one does: a two- or three-note moment is real but nobody
    /// bothers writing it down.
    pub fn moments(&self, most: UnsignedIntegerType) -> Result<Vec<UnsignedIntegerType>> {
        let generator = self.sole_generator()?;
        let periods = self.periods_per_equave().unsigned_abs();
        Ok(crate::tuningsystem::mos::moment_of_symmetry_sizes(
            generator,
            self.period_cents,
            most / periods.max(1),
        )?
        .into_iter()
        .map(|notes| notes * periods)
        .collect())
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
    /// Writes the subgroup, the mapping rows and the generators, as an infobox does.
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.subgroup_name(), self.periods_per_equave())?;
        for row in &self.generators {
            write!(f, ";")?;
            for steps in row.iter().skip(1) {
                write!(f, " {steps}")?;
            }
        }
        write!(f, " (generator")?;
        if self.generator_cents.len() > 1 {
            write!(f, "s")?;
        }
        for (position, cents) in self.generator_cents.iter().enumerate() {
            write!(f, "{} {cents:.1}¢", if position == 0 { "" } else { "," })?;
        }
        write!(f, ")")
    }
}

impl crate::tuningsystem::NamedTemperament {
    /// Builds the temperament this entry describes.
    ///
    /// Errors where [`Temperament::from_mapping_rows`] does, which for a
    /// collected entry means the wiki's generators and its mapping disagree —
    /// a mistranscription, or a page that has changed under us.
    pub fn temperament(&self) -> Result<Temperament> {
        Temperament::from_mapping_rows(
            self.periods_per_equave,
            self.generator_rows,
            self.generator_cents,
            self.subgroup,
        )
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
            periods_per_equave: UnsignedIntegerType,
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
                periods_per_equave: 1,
                generator_steps: &[1, 4, 10],
                generator_cents: 696.7,
                primes: &[2, 3, 5, 7],
                comma: (81, 80),
                period_map: &[1, 1, 0, -3],
                moments: &[(5, "2L 3s"), (7, "5L 2s"), (12, "7L 5s"), (19, "12L 7s")],
            },
            Published {
                name: "mavila",
                periods_per_equave: 1,
                generator_steps: &[1, -3, -1],
                generator_cents: 679.0,
                primes: &[2, 3, 5, 11],
                comma: (135, 128),
                period_map: &[1, 1, 4, 4],
                moments: &[(5, "2L 3s"), (7, "2L 5s"), (9, "7L 2s")],
            },
            Published {
                name: "porcupine",
                periods_per_equave: 1,
                generator_steps: &[-3, -5, 6, -4],
                generator_cents: 163.6,
                primes: &[2, 3, 5, 7, 11],
                comma: (250, 243),
                period_map: &[1, 2, 3, 2, 4],
                moments: &[(7, "1L 6s"), (8, "7L 1s"), (15, "7L 8s")],
            },
        ] {
            let temperament = Temperament::from_mapping(
                published.periods_per_equave,
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
                    temperament.pattern(notes).expect("a moment").to_string(),
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

    /// Every temperament collected from the wiki, checked against what the
    /// wiki says about it.
    ///
    /// The collected table is not trusted: each entry has to build, each comma
    /// the wiki lists has to actually vanish under the mapping, and each MOS
    /// pattern it lists has to actually come out of the generator. A number
    /// mistyped on the way in fails here rather than shipping.
    #[test]
    fn wiki_temperaments_build_and_agree_with_the_wiki() {
        use crate::tuningsystem::{Mos, WIKI_TEMPERAMENTS};

        // A temperament's published MOS list describes the whole range of
        // tunings it covers, while its generator is one chosen optimum. Which
        // step is the large one flips as the generator crosses an equal
        // temperament, so at a boundary the two halves of one infobox can
        // disagree about which is which — the notes and the count are the
        // same either way. Two of the 277 published patterns are on the far
        // side of such a boundary from their own generator: catakleismic's
        // 34-note scale flips at 34edo (317.647 cents) and its generator is
        // 316.7, and tritikleismic's 12-note scale likewise. Pinned by name so
        // a third one fails rather than being waved through.
        const INVERTED: [(&str, &str); 2] =
            [("CATAKLEISMIC", "15L 19s"), ("TRITIKLEISMIC", "9L 3s")];

        assert!(!WIKI_TEMPERAMENTS.is_empty(), "the table is empty");
        let mut commas = 0;
        let mut moments = 0;
        for entry in &WIKI_TEMPERAMENTS {
            let temperament = entry
                .temperament()
                .unwrap_or_else(|error| panic!("{} did not build: {error}", entry.name));

            for comma in entry.commas {
                let (numerator, denominator) = comma
                    .split_once('/')
                    .unwrap_or_else(|| panic!("{}: {comma} is not a ratio", entry.name));
                let Ok(monzo) = Monzo::from_ratio(
                    numerator.parse().expect("a whole numerator"),
                    denominator.parse().expect("a whole denominator"),
                ) else {
                    // A comma past the 97-limit is not one this crate factors;
                    // that is a limit of `PRIMES`, not a bad entry.
                    continue;
                };
                assert!(
                    temperament.tempers_out(&monzo),
                    "{} should temper out {comma}, which the wiki lists for it",
                    entry.name
                );
                commas += 1;
            }

            for pattern in entry.moments {
                let published: Mos = pattern
                    .parse()
                    .unwrap_or_else(|_| panic!("{}: {pattern} is not a pattern", entry.name));
                let made = temperament
                    .pattern(published.notes())
                    .unwrap_or_else(|error| panic!("{} at {pattern}: {error}", entry.name));
                if INVERTED.contains(&(entry.name, pattern)) {
                    assert_eq!(
                        made,
                        published.inverted(),
                        "{} at {pattern} is pinned as inverted and no longer is;                          drop it from INVERTED",
                        entry.name
                    );
                } else {
                    assert_eq!(
                        made, published,
                        "{} should make {pattern}, which the wiki lists for it",
                        entry.name
                    );
                }
                moments += 1;
            }
        }
        // Guard against the checks quietly becoming vacuous.
        assert!(commas > 100, "only {commas} commas were checked");
        assert!(moments > 200, "only {moments} moments were checked");

        // The two shapes that took generalizing to carry at all have to still
        // be in the table, or a regression could drop them unnoticed.
        let rank_three = WIKI_TEMPERAMENTS.iter().filter(|e| e.rank() == 3).count();
        let non_octave = WIKI_TEMPERAMENTS
            .iter()
            .filter(|entry| entry.equave() != 2)
            .count();
        assert_eq!(rank_three, 13, "rank-3 temperaments carried");
        assert_eq!(non_octave, 5, "temperaments repeating at something else");
        assert_eq!(WIKI_TEMPERAMENTS.len(), 94);
    }

    /// Marvel has two generators, which is what rank 3 means and what the
    /// old rank-2-only `Temperament` could not hold.
    #[test]
    fn a_rank_three_temperament_maps_through_both_its_generators() {
        // From marvel's own infobox: 1; 1 0 2 -1; 0 1 2 -3 over 2.3.5.7.11,
        // tuned to a fifth of 700.6 cents and a major third of 383.5.
        let marvel = Temperament::from_mapping_rows(
            1,
            &[&[1, 0, 2, -1], &[0, 1, 2, -3]],
            &[700.6, 383.5],
            &[2, 3, 5, 7, 11],
        )
        .expect("marvel");
        assert_eq!(marvel.rank(), 3);
        assert_eq!(marvel.period_map(), [1, 1, 2, 1, 5]);
        // A fifth is one of the first generator and none of the second.
        assert_eq!(
            marvel
                .map(&Monzo::from_ratio(3, 2).expect("the fifth"))
                .expect("in the subgroup"),
            [0, 1, 0]
        );
        // Marvel is the temperament that tempers out 225/224, and does.
        assert!(marvel.tempers_out(&Monzo::from_ratio(225, 224).expect("the comma")));

        // Two generators make no moment of symmetry: there is no single
        // interval to stack, and saying so beats answering with one of them.
        assert!(marvel.mos(7).is_err());
        assert!(marvel.pattern(7).is_err());
        assert!(marvel.moments(12).is_err());
    }

    /// A subgroup with no 2 in it repeats at something else, and everything
    /// that used to say "octave" has to mean "equave" for it to work.
    #[test]
    fn a_temperament_can_repeat_at_a_tritave() {
        // Canopus, from its own infobox: 1; -5 -4 over 3.5.7, generator 7/5.
        let canopus =
            Temperament::from_mapping(1, &[-5, -4], 583.986, &[3, 5, 7]).expect("canopus");
        assert_eq!(canopus.equave(), 3);
        assert!(!canopus.repeats_at_the_octave());
        assert!((canopus.equave_cents() - 1901.955).abs() < 1e-3);
        assert!((canopus.period_cents() - 1901.955).abs() < 1e-3);
        assert_eq!(canopus.period_map(), [1, 3, 3]);
        assert_eq!(canopus.subgroup_name(), "3.5.7");

        // It tempers out the comma its page names, and its MOS scales are
        // counted to the tritave rather than to an octave — the wiki writes
        // them `3L 1s <3/1>`.
        assert!(canopus.tempers_out(&Monzo::from_ratio(16875, 16807).expect("the comma")));
        assert_eq!(canopus.pattern(4).expect("a moment").to_string(), "3L 1s");

        // The octave is not in its subgroup at all, so it has nothing to say
        // about one.
        assert!(
            canopus
                .map(&Monzo::from_ratio(2, 1).expect("the octave"))
                .is_err()
        );
    }

    #[test]
    fn meantone_maps_the_intervals_it_is_named_for() {
        let meantone =
            Temperament::from_mapping(1, &[1, 4, 10], 696.7, &[2, 3, 5, 7]).expect("meantone");
        let fifth = Monzo::from_ratio(3, 2).expect("the fifth");
        let third = Monzo::from_ratio(5, 4).expect("the third");

        // One generator up, one period down: the fifth is the generator.
        assert_eq!(meantone.map(&fifth).expect("the fifth"), [0, 1]);
        // Four fifths less two octaves is the major third.
        assert_eq!(meantone.map(&third).expect("the third"), [-2, 4]);
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
        // A subgroup has to hold primes this module carries, written up.
        assert!(Temperament::from_mapping(1, &[1, 4], 696.7, &[2, 3, 9]).is_err());
        assert!(Temperament::from_mapping(1, &[1, 4], 696.7, &[2, 5, 3]).is_err());
        assert!(Temperament::from_mapping(1, &[1, 4], 696.7, &[2, 3, 3]).is_err());
        assert!(Temperament::from_mapping(1, &[1, 4], 696.7, &[]).is_err());
        // It need *not* start at 2, though: the first prime is the equave, and
        // a subgroup with no 2 in it repeats at something else.
        let tritave = Temperament::from_mapping(1, &[4, 10], 696.7, &[3, 5, 7])
            .expect("a subgroup repeating at a tritave");
        assert_eq!(tritave.equave(), 3);
        assert!(!tritave.repeats_at_the_octave());
        assert!((tritave.equave_cents() - 1901.955).abs() < 1e-3);
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
        assert_eq!(meantone.periods_per_equave(), 1);
        assert_eq!(meantone.primes(), [2, 3, 5, 7]);
        assert_eq!(meantone.generator_map(), [vec![0, 1, 4, 10]]);
        assert!((meantone.period_cents() - OCTAVE_CENTS).abs() < 1e-9);
    }
}
