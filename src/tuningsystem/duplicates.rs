//! Finding the same tuning under two different names.
//!
//! Tuning data accretes duplicates. The Scala archive has thousands of files
//! contributed over decades, and plenty of them are the same scale written out
//! by different people; this crate's own tables have had the problem too —
//! `IndianFull` was a second name for the same twenty-two ratios as `Indian22`
//! and was removed, and `JustIntonation` turned out to be Wendy Carlos's
//! Harmonic scale under a name that claimed far more.
//!
//! Nothing here decides what to do about a duplicate. It answers the prior
//! question — *which of these are the same thing?* — and leaves the judgement
//! to whoever is looking, because two names for one scale are sometimes a
//! mistake and sometimes just history.
//!
//! The comparison is deliberately by *sound*, not by spelling. A scale written
//! in cents and the same scale written as ratios are the same scale, so
//! everything is reduced to cents within a tolerance before being compared.
//! `a_scale_written_two_ways_fingerprints_the_same` below is that in three
//! notes.
//!
//! None of this is public API, and the module is compiled for tests alone.
//! What it answers is a question about *this crate's own tables* and the
//! archive bundled beside them — that nothing in `ALL_TUNING_SYSTEMS` is a
//! second name for anything else in it, and that the archive still holds the
//! 44 groups it is known to hold. A caller of the library wants a scale it can
//! play, not a fingerprint it can only compare; if one ever asks for the
//! comparison, exporting it then is an addition rather than a break.

use crate::defaults::FloatType;
use crate::error::{Error, Result};

use std::collections::HashMap;
use std::hash::Hash;

/// How near two degrees must be, in cents, to count as the same degree.
///
/// Half a cent: far under anything a listener tells apart, and wide enough to
/// swallow the rounding in an archive file that writes its degrees to three
/// decimal places.
pub const DEFAULT_TOLERANCE: FloatType = 0.5;

/// A scale reduced to something two scales can be compared by.
///
/// Degrees are taken to cents, sorted, and rounded to the tolerance, so a
/// scale written as ratios and the same scale written in cents come out equal.
/// The fingerprint is what a duplicate hunt groups by; it is not a scale and
/// cannot be played.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct ScaleFingerprint {
    steps: Vec<i64>,
}

impl ScaleFingerprint {
    /// Reduces a scale given as cents above its root.
    ///
    /// Errors on a tolerance that is not a positive real number, and on a
    /// degree that is not a real number of cents.
    pub fn from_cents(degrees: &[FloatType], tolerance: FloatType) -> Result<Self> {
        if !tolerance.is_finite() || tolerance <= 0.0 {
            return Err(Error::TuningSystem(format!(
                "{tolerance} cents is not a tolerance two scales can be compared within"
            )));
        }
        if let Some(bad) = degrees.iter().find(|degree| !degree.is_finite()) {
            return Err(Error::TuningSystem(format!(
                "{bad} is not a real number of cents"
            )));
        }
        let mut steps: Vec<i64> = degrees
            .iter()
            .map(|degree| (degree / tolerance).round() as i64)
            .collect();
        // Sorted, because the order a scale was written in says nothing about
        // it; *not* deduplicated, because how many degrees it has does. An
        // earlier version deduplicated and made Young II and Neidhardt I look
        // like one temperament: they use the same handful of deviations, in
        // different places.
        steps.sort_unstable();
        Ok(Self { steps })
    }

    /// How many degrees the scale came down to.
    #[must_use]
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Whether the scale had no degrees at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

/// A rank-2 temperament reduced to something two of them can be compared by.
///
/// Two temperaments are the same when they map the same primes the same way.
/// The generator can be taken either way round — a fifth up and a fourth down
/// generate the same notes — so the mapping is canonicalized to the reading
/// whose first non-zero step is positive before being compared. Without that,
/// one page's `1; 1 4 10` and another's `1; -1 -4 -10` would look like two
/// temperaments.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct TemperamentFingerprint {
    subgroup: Vec<crate::defaults::IntegerType>,
    periods_per_equave: crate::defaults::UnsignedIntegerType,
    generator_rows: Vec<Vec<crate::defaults::IntegerType>>,
}

/// A mapping row read the way round whose first non-zero step is positive.
fn canonical(row: &[crate::defaults::IntegerType]) -> Vec<crate::defaults::IntegerType> {
    let flip = row
        .iter()
        .find(|step| **step != 0)
        .is_some_and(|step| *step < 0);
    row.iter()
        .map(|step| if flip { -step } else { *step })
        .collect()
}

impl TemperamentFingerprint {
    /// Reduces a mapping to its canonical reading.
    pub fn new(
        subgroup: &[crate::defaults::IntegerType],
        periods_per_equave: crate::defaults::UnsignedIntegerType,
        generator_rows: &[&[crate::defaults::IntegerType]],
    ) -> Self {
        Self {
            subgroup: subgroup.to_vec(),
            periods_per_equave,
            generator_rows: generator_rows.iter().map(|row| canonical(row)).collect(),
        }
    }
}

/// Groups whatever is handed over by a key, keeping only the groups with more
/// than one member.
///
/// This is the whole of the duplicate hunt: fingerprint each thing, group by
/// the fingerprint, and report the groups that are not alone. Groups come back
/// in the order their first member was seen, and each group keeps the order it
/// was given in, so a report reads the same way twice running.
pub fn duplicate_groups<T, K, F>(items: impl IntoIterator<Item = T>, key: F) -> Vec<Vec<T>>
where
    K: Eq + Hash,
    F: Fn(&T) -> K,
{
    let mut order: Vec<K> = Vec::new();
    let mut groups: HashMap<K, Vec<T>> = HashMap::new();
    for item in items {
        let fingerprint = key(&item);
        match groups.get_mut(&fingerprint) {
            Some(group) => group.push(item),
            None => {
                order.push(key(&item));
                let _ = groups.insert(fingerprint, vec![item]);
            }
        }
    }
    order
        .into_iter()
        .filter_map(|fingerprint| groups.remove(&fingerprint))
        .filter(|group| group.len() > 1)
        .collect()
}

/// The scales in a Scala archive that are the same scale under another name.
///
/// Each group is the file names sharing one sound, in the order the archive
/// gave them. An empty scale has no degrees to compare and is left out, since
/// every empty scale would otherwise match every other one.
#[cfg(feature = "scala-archive")]
pub fn archive_duplicates(
    archive: &crate::tuningsystem::scala::ScalaArchive,
    tolerance: FloatType,
) -> Result<Vec<Vec<String>>> {
    let mut fingerprinted: Vec<(String, ScaleFingerprint)> = Vec::new();
    for (name, scale) in archive.iter() {
        if scale.is_empty() {
            continue;
        }
        let degrees: Vec<FloatType> = scale
            .degrees()
            .iter()
            .map(|degree| degree.cents())
            .collect();
        fingerprinted.push((
            name.to_string(),
            ScaleFingerprint::from_cents(&degrees, tolerance)?,
        ));
    }
    fingerprinted.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(
        duplicate_groups(fingerprinted, |(_, fingerprint)| fingerprint.clone())
            .into_iter()
            .map(|group| group.into_iter().map(|(name, _)| name).collect())
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuningsystem::{ALL_TUNING_SYSTEMS, TuningSystem, WIKI_TEMPERAMENTS};

    /// One octave of a tuning system's degrees, as cents above the root.
    ///
    /// Not `TuningSystem::cents`, which is the degree's *deviation* from equal
    /// temperament: every equal division has a deviation of nought all the way
    /// up, so fingerprinting that made 12-tone, whole-tone, quarter-tone,
    /// Javanese and Thai equal temperament look like one tuning.
    fn system_cents(system: TuningSystem) -> Vec<FloatType> {
        (0..system.octave_size() as usize)
            .map(|degree| 1200.0 * system.ratio(degree).log2())
            .collect()
    }

    #[test]
    fn a_scale_written_two_ways_fingerprints_the_same() {
        let ratios = ScaleFingerprint::from_cents(&[0.0, 386.3137, 701.955], DEFAULT_TOLERANCE)
            .expect("a scale");
        let cents = ScaleFingerprint::from_cents(&[0.0, 386.31, 701.96], DEFAULT_TOLERANCE)
            .expect("a scale");
        assert_eq!(ratios, cents);
        assert_eq!(ratios.len(), 3);
        assert!(!ratios.is_empty());

        // The order it was written in says nothing about the scale.
        let shuffled = ScaleFingerprint::from_cents(&[701.955, 0.0, 386.3137], DEFAULT_TOLERANCE)
            .expect("a scale");
        assert_eq!(ratios, shuffled);

        // A different scale is a different fingerprint.
        let other =
            ScaleFingerprint::from_cents(&[0.0, 400.0, 700.0], DEFAULT_TOLERANCE).expect("a scale");
        assert_ne!(ratios, other);
    }

    #[test]
    fn a_tolerance_that_is_not_one_is_refused() {
        assert!(ScaleFingerprint::from_cents(&[0.0], 0.0).is_err());
        assert!(ScaleFingerprint::from_cents(&[0.0], -1.0).is_err());
        assert!(ScaleFingerprint::from_cents(&[FloatType::NAN], 0.5).is_err());
        assert!(
            ScaleFingerprint::from_cents(&[], 0.5)
                .expect("no degrees")
                .is_empty()
        );
    }

    #[test]
    fn grouping_keeps_only_what_is_shared_and_the_order_it_came_in() {
        let groups = duplicate_groups(["a", "bb", "c", "dd", "e"], |word| word.len());
        assert_eq!(groups, vec![vec!["a", "c", "e"], vec!["bb", "dd"]]);
        assert!(duplicate_groups(["a", "bb", "ccc"], |word| word.len()).is_empty());
        assert!(duplicate_groups(Vec::<&str>::new(), |word| word.len()).is_empty());
    }

    /// The guard for the defect this module exists because of: `IndianFull`
    /// was a second name for `Indian22`'s twenty-two ratios and shipped that
    /// way. Nothing in the table may be a second name for anything else in it.
    #[test]
    fn no_two_tuning_systems_are_the_same_tuning() {
        let groups = duplicate_groups(ALL_TUNING_SYSTEMS, |system| {
            ScaleFingerprint::from_cents(&system_cents(*system), DEFAULT_TOLERANCE)
                .expect("a tuning system")
        });
        assert!(
            groups.is_empty(),
            "these tuning systems are the same tuning under different names: {groups:?}"
        );
    }

    /// Two generators one way round each are the same temperament, and the
    /// fingerprint has to say so.
    #[test]
    fn a_mapping_read_either_way_round_is_one_temperament() {
        let up = TemperamentFingerprint::new(&[2, 3, 5, 7], 1, &[&[1, 4, 10]]);
        let down = TemperamentFingerprint::new(&[2, 3, 5, 7], 1, &[&[-1, -4, -10]]);
        assert_eq!(
            up, down,
            "a fifth up and a fourth down generate the same notes"
        );

        // A different subgroup is a different temperament, mapping and all.
        assert_ne!(
            up,
            TemperamentFingerprint::new(&[2, 3, 5], 1, &[&[1, 4, 10]])
        );
        // So is a different number of periods.
        assert_ne!(
            up,
            TemperamentFingerprint::new(&[2, 3, 5, 7], 2, &[&[1, 4, 10]])
        );
        // A leading zero must not decide the direction on its own.
        assert_eq!(
            TemperamentFingerprint::new(&[2, 3, 5], 1, &[&[0, -1]]),
            TemperamentFingerprint::new(&[2, 3, 5], 1, &[&[0, 1]])
        );
    }

    /// What the bundled Scala archive holds twice.
    ///
    /// Forty-four groups covering fifty-four redundant files out of 3,994 —
    /// and they are the right ones: four names for quarter-comma meantone
    /// (`meanquar`, `meanquarr`, `smith_mq`, `12-MT-1_4-SC`), five files that
    /// are 12-tone equal temperament to within half a cent (`12edo`,
    /// `atomschis`, `ellis_r`, `neidhardt4`, `schulter_12`), and the duodene
    /// beside the 12-tone 5-limit lattice it is.
    ///
    /// The archive is bundled verbatim and these are not defects to fix — a
    /// scale contributed twice under two names is the archive's history. The
    /// count is pinned so that a submodule bump which quietly changes what the
    /// archive holds is noticed.
    #[test]
    #[cfg(feature = "scala-archive")]
    fn the_bundled_archive_holds_the_duplicates_it_is_known_to_hold() {
        use crate::tuningsystem::scala::ScalaArchive;

        let archive = ScalaArchive::bundled();
        let groups =
            super::archive_duplicates(&archive, DEFAULT_TOLERANCE).expect("a bundled archive");
        let redundant: usize = groups.iter().map(|group| group.len() - 1).sum();
        assert_eq!(groups.len(), 44, "groups of scales sharing one sound");
        assert_eq!(
            redundant, 54,
            "files that repeat a scale already in the archive"
        );

        // Spot-check the two that say the comparison is by sound and not by
        // spelling, since that is the whole point of the fingerprint.
        assert!(
            groups
                .iter()
                .any(|group| group.iter().any(|name| name == "meanquar.scl")
                    && group.iter().any(|name| name == "smith_mq.scl")),
            "the quarter-comma meantone files should group together"
        );
        assert!(
            groups
                .iter()
                .any(|group| group.iter().any(|name| name == "12edo.scl")
                    && group.iter().any(|name| name == "atomschis.scl")),
            "the atomic schisma temperament is 12edo to within half a cent"
        );

        // A tolerance of nothing much finds fewer, which is the tolerance
        // doing its job rather than the archive changing.
        let strict = super::archive_duplicates(&archive, 0.000_1).expect("a bundled archive");
        assert!(
            strict.len() < groups.len(),
            "a tighter tolerance should find fewer duplicates, not more"
        );
    }

    /// What the crate's own collections hold twice *between* them.
    ///
    /// Each list is checked against itself elsewhere here; this is the check
    /// across them, and it says something the separate checks cannot: the
    /// ratio tables and the Scala archive are largely the same scales stored
    /// two ways. 23 of the 28 tables are in the archive too, by sound — the
    /// five that are not are the equal-step approximations, which the archive
    /// has no exact counterpart for.
    ///
    /// That overlap is deliberate, not a defect to remove. A `TuningSystem` is
    /// an exact `Fraction` table available with no features turned on; an
    /// archive entry is parsed cents behind `scala-archive` and a megabyte of
    /// data. They answer different questions about the same scale. What is
    /// worth pinning is that the overlap is *known*, so nobody adds a table
    /// believing it is new when the archive already had it.
    #[test]
    #[cfg(feature = "scala-archive")]
    fn the_tables_and_the_archive_are_largely_the_same_scales_stored_twice() {
        use crate::tuningsystem::{HISTORICAL_TEMPERAMENTS, scala::ScalaArchive};

        // The historical temperaments are a strict subset of the tables, so
        // counting them as a separate collection counts fourteen of them twice.
        assert!(
            HISTORICAL_TEMPERAMENTS
                .iter()
                .all(|entry| ALL_TUNING_SYSTEMS.contains(entry)),
            "the historical temperaments are a subset of the tuning systems"
        );

        // Both sides list the root and keep the period out of the degrees, so
        // the fingerprints compare as they stand.
        let archive = ScalaArchive::bundled();
        let mut index: std::collections::HashSet<ScaleFingerprint> =
            std::collections::HashSet::new();
        for (_, scale) in archive.iter() {
            if scale.is_empty() {
                continue;
            }
            let degrees: Vec<FloatType> = scale
                .degrees()
                .iter()
                .map(|degree| degree.cents())
                .collect();
            if let Ok(fingerprint) = ScaleFingerprint::from_cents(&degrees, DEFAULT_TOLERANCE) {
                let _ = index.insert(fingerprint);
            }
        }
        let shared = ALL_TUNING_SYSTEMS
            .into_iter()
            .filter(|system| {
                ScaleFingerprint::from_cents(&system_cents(*system), DEFAULT_TOLERANCE)
                    .is_ok_and(|fingerprint| index.contains(&fingerprint))
            })
            .count();
        assert_eq!(
            shared, 23,
            "tuning tables the bundled Scala archive also holds"
        );
    }

    /// The wiki names some temperaments twice, on separate pages. Which those
    /// are is pinned, so a collection that introduces a new one is noticed.
    #[test]
    fn the_wiki_temperaments_that_share_a_mapping_are_the_ones_pinned_here() {
        let groups = duplicate_groups(&WIKI_TEMPERAMENTS, |entry| {
            TemperamentFingerprint::new(
                entry.subgroup,
                entry.periods_per_equave,
                entry.generator_rows,
            )
        });
        let names: Vec<Vec<&str>> = groups
            .iter()
            .map(|group| group.iter().map(|entry| entry.name).collect())
            .collect();
        assert!(
            names.is_empty(),
            "these collected temperaments share a mapping: {names:?}"
        );
    }
}
