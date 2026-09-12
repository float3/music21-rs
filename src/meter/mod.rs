//! Time signatures, ported from music21's `meter` package.
//!
//! [`TimeSignature`] covers the part of music21's meter handling that is a pure
//! function of the numerator and denominator: how long a bar is, how many beats
//! it carries, how long each beat is, and how that beat subdivides.
//!
//! music21 derives all of this from a `MeterSequence` partition tree that also
//! drives beaming, display sequences and accent weighting. Only the partition
//! *result* is ported here — the tree's other consumers have no counterpart in
//! this crate yet, and building the tree to read one number back off it would be
//! the transliterated machinery the repository guidance warns against. The
//! partition rule itself is music21's `_setDefaultBeatPartitions`, verified
//! against upstream by the `meter_parity` fixture.

pub mod sequence;

pub use sequence::{MeterTerminal, OffsetAlign};

use crate::defaults::{FloatType, UnsignedIntegerType};
use crate::duration::Duration;
use crate::error::{Error, Result};

/// Names music21 gives a partition count, indexed by the count itself.
///
/// Index 0 is music21's `Empty`, which a valid time signature never reaches.
const BEAT_COUNT_NAMES: [&str; 9] = [
    "Empty",
    "Single",
    "Duple",
    "Triple",
    "Quadruple",
    "Quintuple",
    "Sextuple",
    "Septuple",
    "Octuple",
];

/// How the beat of a meter subdivides.
///
/// Mirrors music21's `beatDivisionCountName`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BeatDivision {
    /// A single-beat meter, which music21 reports as `Other` rather than as a
    /// division — there is no lower level to divide.
    Other,
    /// Beats divide in two.
    Simple,
    /// Beats divide in three.
    Compound,
}

impl BeatDivision {
    /// Returns music21's name for this division.
    pub fn music21_name(&self) -> &'static str {
        match self {
            Self::Other => "Other",
            Self::Simple => "Simple",
            Self::Compound => "Compound",
        }
    }

    /// Returns the number of divisions in one beat.
    ///
    /// Matches music21's `beatDivisionCount`, which reports `1` rather than
    /// raising for a single-beat meter.
    pub fn count(&self) -> UnsignedIntegerType {
        match self {
            Self::Other => 1,
            Self::Simple => 2,
            Self::Compound => 3,
        }
    }
}

/// A time signature, such as `4/4` or `6/8`.
///
/// ```
/// use music21_rs::TimeSignature;
///
/// let six_eight = TimeSignature::from_ratio_string("6/8")?;
/// assert_eq!(six_eight.beat_count(), 2);
/// assert_eq!(six_eight.beat_quarter_length()?, 1.5);
/// assert_eq!(six_eight.classification(), "Compound Duple");
/// # Ok::<(), music21_rs::Error>(())
/// ```
/// Not `Copy`, `Eq` or `Hash`: a meter is about to carry the partitions it
/// is felt in, and those weigh their parts in floats.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct TimeSignature {
    numerator: UnsignedIntegerType,
    denominator: UnsignedIntegerType,
    favor_compound: bool,
    display_sequence: MeterTerminal,
    beat_sequence: MeterTerminal,
    beam_sequence: MeterTerminal,
    accent_sequence: MeterTerminal,
}

impl Default for TimeSignature {
    /// Returns `4/4`, matching music21's default `TimeSignature()`.
    fn default() -> Self {
        Self::common()
    }
}

impl TimeSignature {
    /// Creates a time signature from a numerator and denominator.
    ///
    /// Both must be non-zero. The denominator need not be a power of two —
    /// music21 accepts irrational meters such as `4/3`, and so does this.
    pub fn new(numerator: UnsignedIntegerType, denominator: UnsignedIntegerType) -> Result<Self> {
        if numerator == 0 {
            return Err(Error::Meter(
                "time signature numerator must be non-zero".to_string(),
            ));
        }
        if denominator == 0 {
            return Err(Error::Meter(
                "time signature denominator must be non-zero".to_string(),
            ));
        }
        let mut signature = Self {
            numerator,
            denominator,
            favor_compound: favor_compound(numerator, denominator, None),
            display_sequence: whole_bar(numerator, denominator)?,
            beat_sequence: whole_bar(numerator, denominator)?,
            beam_sequence: whole_bar(numerator, denominator)?,
            accent_sequence: whole_bar(numerator, denominator)?,
        };
        signature.set_default_partitions()?;
        Ok(signature)
    }

    /// How the bar is written, before anything divides it: music21's
    /// `displaySequence`. A meter written additively as `"3/8+2/8"` is two
    /// parts here, and that is what makes the beats fall where they are
    /// written rather than where the numerator alone would put them.
    #[must_use]
    pub fn display_sequence(&self) -> &MeterTerminal {
        &self.display_sequence
    }

    /// How the bar is counted: music21's `beatSequence`, one part per beat,
    /// each divided again into what the beat is felt in.
    #[must_use]
    pub fn beat_sequence(&self) -> &MeterTerminal {
        &self.beat_sequence
    }

    /// How the bar is beamed: music21's `beamSequence`, the groups a run of
    /// short notes is written in.
    #[must_use]
    pub fn beam_sequence(&self) -> &MeterTerminal {
        &self.beam_sequence
    }

    /// How the bar is weighted: music21's `accentSequence`, one part per
    /// accent partition, each carrying the weight [`Self::accent_weights`]
    /// gives it.
    #[must_use]
    pub fn accent_sequence(&self) -> &MeterTerminal {
        &self.accent_sequence
    }

    /// Whether the bar is felt in compound beats: music21's `favorCompound`,
    /// which is what counts a `6/8` in two and a `slow 6/8` in six.
    #[must_use]
    pub fn favors_compound(&self) -> bool {
        self.favor_compound
    }

    /// Counts the bar in a different number of beats: music21's settable
    /// `beatCount`.
    ///
    /// The beats are repartitioned and each divided again, so a `6/8`
    /// counted in six is six eighths rather than two dotted quarters. A
    /// count the bar cannot be divided into is an error.
    pub fn set_beat_count(&mut self, count: UnsignedIntegerType) -> Result<()> {
        if count == 0 {
            return Err(Error::Meter(
                "a bar cannot be counted in no beats".to_string(),
            ));
        }
        let mut beats = MeterTerminal::new(self.numerator, self.denominator)?;
        beats.partition_by_count(count as usize, false)?;
        if beats.len() > 1 {
            let _ = beats.subdivide_partitions_equal(None);
        }
        self.beat_sequence = beats;
        Ok(())
    }

    /// Builds the beam, beat and accent partitions the way music21 does when
    /// nobody has said how: `_setDefaultBeamPartitions`,
    /// `_setDefaultBeatPartitions` and `_setDefaultAccentWeights`.
    fn set_default_partitions(&mut self) -> Result<()> {
        self.set_default_beam_partitions()?;
        self.set_default_beat_partitions()?;
        self.set_default_accent_weights();
        Ok(())
    }

    /// music21's `_setDefaultBeamPartitions`: a short bar of short notes is
    /// beamed all together, and anything else in the groups its numerator is
    /// felt in.
    ///
    /// A meter written additively is beamed by these rules too — music21
    /// beams `"3/8+2/8"` as `{2/8+3/8}`, by the rule for a five, and not in
    /// the parts it was written in.
    fn set_default_beam_partitions(&mut self) -> Result<()> {
        let numerator = self.numerator;
        let denominator = self.denominator;
        if (denominator == 8 && matches!(numerator, 1..=3))
            || (denominator == 16 && matches!(numerator, 1..=5))
            || (denominator == 32 && matches!(numerator, 1..=11))
        {
            return Ok(());
        }
        match numerator {
            2..=4 => {
                self.beam_sequence
                    .partition_by_count(numerator as usize, true)?;
                if denominator == 4 {
                    for part in self.beam_sequence.parts_mut() {
                        part.subdivide(2)?;
                    }
                }
            }
            5 => {
                self.beam_sequence.partition_by_list(&[2, 3])?;
                if denominator == 4 {
                    for (part, count) in self.beam_sequence.parts_mut().iter_mut().zip([2, 3]) {
                        part.subdivide(count)?;
                    }
                }
            }
            7 => self.beam_sequence.partition_by_count(3, true)?,
            6 | 9 | 12 | 15 | 18 | 21 => {
                let threes = vec![3; (numerator / 3) as usize];
                self.beam_sequence.partition_by_list(&threes)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// music21's `_setDefaultBeatPartitions`: the top-level count of the bar,
    /// then each beat divided into what it is felt in.
    ///
    /// A meter written additively is counted in the parts it was written in,
    /// each keeping the note it was written in — `"2/4+3/8"` is counted
    /// `{{1/4+1/4}+{1/8+1/8+1/8}}`.
    fn set_default_beat_partitions(&mut self) -> Result<()> {
        let numerator = self.numerator;
        let compound = self.favor_compound;
        if self.display_sequence.len() == 1 {
            match numerator {
                2 => self.beat_sequence.partition_by_count(2, true)?,
                6 if compound => self.beat_sequence.partition_by_count(2, true)?,
                3 if compound => self.beat_sequence.partition_by_count(1, true)?,
                3 => self.beat_sequence.partition_by_list(&[1, 1, 1])?,
                9 if compound => self.beat_sequence.partition_by_list(&[3, 3, 3])?,
                4 => self.beat_sequence.partition_by_count(4, true)?,
                12 if compound => self.beat_sequence.partition_by_count(4, true)?,
                other if other >= 15 && other.is_multiple_of(3) && compound => {
                    let threes = vec![3; (other / 3) as usize];
                    self.beat_sequence.partition_by_list(&threes)?;
                }
                other => self
                    .beat_sequence
                    .partition_by_count(other as usize, true)?,
            }
        } else {
            let written: Vec<String> = self
                .display_sequence
                .parts()
                .iter()
                .map(|part| format!("{}/{}", part.numerator(), part.denominator()))
                .collect();
            let borrowed: Vec<&str> = written.iter().map(String::as_str).collect();
            self.beat_sequence.partition_by_parts(&borrowed)?;
        }
        if self.beat_sequence.len() > 1 {
            // music21 forgives a bar written in notes shorter than a 128th
            // for being too short to divide again, and nothing else.
            match self.beat_sequence.subdivide_partitions_equal(None) {
                Ok(()) => {}
                Err(error) if self.denominator >= 128 => {
                    let _ = error;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    /// music21's `_setDefaultAccentWeights`, read off the weights this meter
    /// already gives its accent partitions rather than derived a second time
    /// from a nested hierarchy: one part per weight, carrying that weight.
    fn set_default_accent_weights(&mut self) {
        let weights = self.default_accent_weights();
        let ones = vec![1; weights.len()];
        if self.accent_sequence.partition_by_list(&ones).is_err() {
            return;
        }
        for (part, weight) in self.accent_sequence.parts_mut().iter_mut().zip(weights) {
            part.set_weight(weight);
        }
    }

    /// Parses a `"numerator/denominator"` string such as `"6/8"`.
    ///
    /// music21 writes more than a bare ratio here. A word before the ratio
    /// says how the beat is felt — `"slow 6/8"` is counted in six and
    /// `"fast 6/8"` in two — and a meter can be written additively, as
    /// `"3/8+2/8"` or `"3+2/8"`, where a numerator with no denominator of
    /// its own takes the next one written. What the parts add up to is the
    /// meter this returns; how they are grouped is [`Self::parts`].
    pub fn from_ratio_string(ratio: &str) -> Result<Self> {
        let parts = Self::parts(ratio)?;
        let word = division_word(ratio);
        let denominator = parts[0].1;
        if parts.iter().all(|(_, part)| *part == denominator) {
            let numerator = parts.iter().map(|(count, _)| count).sum();
            let mut signature = Self::new(numerator, denominator)?;
            signature.write_as(&parts, word)?;
            return Ok(signature);
        }
        // Parts measured in different notes: what they come to together is
        // the meter, over the shortest note any of them is written in.
        let denominator = parts
            .iter()
            .map(|(_, part)| *part)
            .max()
            .unwrap_or(denominator);
        let numerator = parts
            .iter()
            .map(|(count, part)| count * (denominator / part))
            .sum();
        let mut signature = Self::new(numerator, denominator)?;
        signature.write_as(&parts, word)?;
        Ok(signature)
    }

    /// Records how the meter was written — the parts it was written in, and
    /// the word saying how it is counted — and rebuilds what depends on
    /// either.
    fn write_as(
        &mut self,
        parts: &[(UnsignedIntegerType, UnsignedIntegerType)],
        word: Option<&str>,
    ) -> Result<()> {
        self.favor_compound = favor_compound(self.numerator, self.denominator, word);
        if parts.len() > 1 {
            let written: Vec<String> = parts
                .iter()
                .map(|(count, part)| format!("{count}/{part}"))
                .collect();
            let borrowed: Vec<&str> = written.iter().map(String::as_str).collect();
            self.display_sequence.partition_by_parts(&borrowed)?;
        }
        self.beat_sequence = whole_bar(self.numerator, self.denominator)?;
        self.beam_sequence = whole_bar(self.numerator, self.denominator)?;
        self.accent_sequence = whole_bar(self.numerator, self.denominator)?;
        self.set_default_partitions()
    }

    /// The `(numerator, denominator)` pairs a meter string is written in,
    /// in the order written: music21's `slashMixedToFraction`.
    ///
    /// `"3/8+2/8"` is two parts and `"6/8"` is one. A part written as a bare
    /// numerator takes the denominator of the next part that has one, which
    /// is how `"3+2/8"` is two eighth-note parts; a string whose last part
    /// says no denominator at all says nothing about how it is measured, and
    /// is refused.
    pub fn parts(ratio: &str) -> Result<Vec<(UnsignedIntegerType, UnsignedIntegerType)>> {
        let written = ratio.trim();
        if written.is_empty() {
            return Err(Error::Meter("a time signature says nothing".to_string()));
        }
        let mut pre: Vec<(UnsignedIntegerType, Option<UnsignedIntegerType>)> = Vec::new();
        for part in written.split('+') {
            let part = part.trim();
            match part.split_once('/') {
                Some((numerator, denominator)) => pre.push((
                    Self::count(numerator, "numerator", written)?,
                    Some(Self::count(denominator, "denominator", written)?),
                )),
                None => pre.push((Self::count(part, "numerator", written)?, None)),
            }
        }
        let mut out = Vec::with_capacity(pre.len());
        for (index, (numerator, denominator)) in pre.iter().enumerate() {
            let denominator = match denominator {
                Some(denominator) => *denominator,
                None => pre[index + 1..]
                    .iter()
                    .find_map(|(_, later)| *later)
                    .ok_or_else(|| {
                        Error::Meter(format!(
                            "cannot match a denominator to every numerator in {written:?}"
                        ))
                    })?,
            };
            out.push((*numerator, denominator));
        }
        Ok(out)
    }

    /// One number out of a meter string, with the word music21 allows before
    /// it — `"slow 6"` is six — taken off first.
    fn count(part: &str, label: &str, ratio: &str) -> Result<UnsignedIntegerType> {
        let digits = part
            .trim()
            .rsplit(|character: char| character.is_whitespace())
            .next()
            .unwrap_or("")
            .trim();
        digits
            .parse::<UnsignedIntegerType>()
            .map_err(|_| Error::Meter(format!("cannot read a {label} from {part:?} in {ratio:?}")))
    }

    /// Returns common time, `4/4`.
    pub fn common() -> Self {
        Self::new(4, 4).expect("4/4 is a meter")
    }

    /// Returns cut time, `2/2`.
    pub fn cut() -> Self {
        Self::new(2, 2).expect("2/2 is a meter")
    }

    /// Returns the numerator.
    pub fn numerator(&self) -> UnsignedIntegerType {
        self.numerator
    }

    /// Returns the denominator.
    pub fn denominator(&self) -> UnsignedIntegerType {
        self.denominator
    }

    /// Returns the `"numerator/denominator"` spelling.
    pub fn ratio_string(&self) -> String {
        format!("{}/{}", self.numerator, self.denominator)
    }

    /// Returns the length of one bar in quarter lengths.
    pub fn bar_quarter_length(&self) -> FloatType {
        FloatType::from(self.numerator) * 4.0 / FloatType::from(self.denominator)
    }

    /// Returns the length of one bar as a [`Duration`].
    pub fn bar_duration(&self) -> Duration {
        Duration::new(self.bar_quarter_length())
            .expect("a non-zero numerator and denominator give a positive finite bar length")
    }

    /// Returns how many beats one bar carries.
    ///
    /// This is music21's `beatCount`, which follows the numerator rather than
    /// the denominator — except at `3`, where `3/4` is three beats but `3/8` is
    /// one.
    pub fn beat_count(&self) -> UnsignedIntegerType {
        self.beat_sequence.len() as UnsignedIntegerType
    }

    /// Returns music21's name for the beat count, such as `"Duple"`.
    ///
    /// Counts above eight are spelled as `"<n>-uple"`, as music21 does.
    pub fn beat_count_name(&self) -> String {
        let count = self.beat_count();
        BEAT_COUNT_NAMES
            .get(count as usize)
            .map_or_else(|| format!("{count}-uple"), |name| (*name).to_string())
    }

    /// Returns the length of one beat in quarter lengths.
    ///
    /// Every meter this type can express has a uniform beat, so unlike
    /// music21's `beatDuration` this never fails. music21 only reports a
    /// non-uniform beat for a hand-partitioned `MeterSequence`, which has no
    /// counterpart here.
    pub fn beat_quarter_length(&self) -> Result<FloatType> {
        let spans = self.beat_spans();
        let first = spans[0].1 - spans[0].0;
        if spans
            .iter()
            .any(|(start, end)| ((end - start) - first).abs() > OFFSET_TOLERANCE)
        {
            let lengths: Vec<FloatType> = spans.iter().map(|(s, e)| e - s).collect();
            return Err(Error::Meter(format!(
                "non uniform beat division: {lengths:?}"
            )));
        }
        Ok(first)
    }

    /// Returns the length of one beat as a [`Duration`].
    pub fn beat_duration(&self) -> Result<Duration> {
        Duration::new(self.beat_quarter_length()?)
    }

    /// Returns how the beat subdivides.
    pub fn beat_division(&self) -> BeatDivision {
        let parts = self.beat_sequence.parts();
        if parts.len() <= 1 {
            return BeatDivision::Other;
        }
        let mut counts = parts.iter().map(MeterTerminal::len);
        let Some(first) = counts.next() else {
            return BeatDivision::Other;
        };
        if first == 0 || !counts.all(|count| count == first) {
            // The beats of a bar written additively need not divide alike;
            // music21 reports such a meter as Other and counts one.
            return BeatDivision::Other;
        }
        match first {
            2 => BeatDivision::Simple,
            3 => BeatDivision::Compound,
            _ => BeatDivision::Other,
        }
    }

    /// Returns the number of divisions in one beat.
    pub fn beat_division_count(&self) -> UnsignedIntegerType {
        self.beat_division().count()
    }

    /// Returns `true` when beats divide in three.
    pub fn is_compound(&self) -> bool {
        self.beat_division() == BeatDivision::Compound
    }

    /// Returns music21's `classification`, such as `"Compound Duple"`.
    pub fn classification(&self) -> String {
        format!(
            "{} {}",
            self.beat_division().music21_name(),
            self.beat_count_name()
        )
    }

    /// Returns music21's `beatDivisionCountName`: `Simple`, `Compound` or
    /// `Other`.
    pub fn beat_division_count_name(&self) -> &'static str {
        self.beat_division().music21_name()
    }

    /// Returns whether two time signatures have the same numerator and
    /// denominator: music21's `ratioEqual`, so `4/4` and `2/2` differ.
    pub fn ratio_equal(&self, other: &TimeSignature) -> bool {
        self.numerator == other.numerator && self.denominator == other.denominator
    }

    /// Returns how many quarter lengths one unit of the denominator lasts:
    /// `0.5` in `6/8`, `2.0` in `2/2`.
    pub fn beat_length_to_quarter_length_ratio(&self) -> FloatType {
        4.0 / FloatType::from(self.denominator)
    }

    /// Returns how many denominator units make one quarter length, the
    /// inverse of [`Self::beat_length_to_quarter_length_ratio`].
    pub fn quarter_length_to_beat_length_ratio(&self) -> FloatType {
        FloatType::from(self.denominator) / 4.0
    }

    /// Returns the quarter length of each division of one beat, in order:
    /// two eighths in `4/4`, three in `6/8`, the whole dotted-quarter beat
    /// in `3/8` where the beat does not divide.
    pub fn beat_division_quarter_lengths(&self) -> Result<Vec<FloatType>> {
        let count = self.beat_division_count().max(1);
        let beat = self.beat_quarter_length()?;
        Ok(vec![beat / FloatType::from(count); count as usize])
    }

    /// Returns [`Self::beat_division_quarter_lengths`] as durations: music21's
    /// `beatDivisionDurations`.
    pub fn beat_division_durations(&self) -> Result<Vec<Duration>> {
        self.beat_division_quarter_lengths()?
            .into_iter()
            .map(Duration::new)
            .collect()
    }

    /// Returns each division of the beat halved: music21's
    /// `beatSubDivisionDurations`, four sixteenths in `4/4`.
    pub fn beat_sub_division_durations(&self) -> Result<Vec<Duration>> {
        let mut out = Vec::new();
        for quarter_length in self.beat_division_quarter_lengths()? {
            let half = Duration::new(quarter_length / 2.0)?;
            out.push(half.clone());
            out.push(half);
        }
        Ok(out)
    }

    /// Returns the quarter-length offset of a one-based, possibly fractional
    /// beat: music21's `getOffsetFromBeat`, so beat `2.5` of `4/4` is `1.5`
    /// and beat `1.5` of `6/8` is `0.75`. A beat past the bar is an error.
    pub fn offset_from_beat(&self, beat: FloatType) -> Result<FloatType> {
        let whole = beat.floor();
        if !beat.is_finite() || whole < 1.0 || whole > FloatType::from(self.beat_count()) {
            return Err(Error::Meter(format!(
                "requested beat value ({beat}) not found in the {} beats of {}",
                self.beat_count(),
                self.ratio_string()
            )));
        }
        let (start, end) = self.beat_spans()[whole as usize - 1];
        Ok(start + (beat - whole) * (end - start))
    }

    /// Returns the one-based beat containing `offset` and how far into that
    /// beat it lies, in quarter lengths: music21's `getBeatProgress`.
    pub fn beat_progress(&self, offset: FloatType) -> Result<(UnsignedIntegerType, FloatType)> {
        let index = self.beat_index(offset)?;
        let (start, _) = self.beat_spans()[index];
        Ok((index as UnsignedIntegerType + 1, offset - start))
    }

    /// Returns the position within the bar as a fractional beat: music21's
    /// `getBeatProportion`, `2.5` for the second eighth of beat two in `4/4`
    /// and `1.333…` for the second eighth of `6/8`.
    pub fn beat_proportion(&self, offset: FloatType) -> Result<FloatType> {
        let (beat, progress) = self.beat_progress(offset)?;
        let (start, end) = self.beat_spans()[beat as usize - 1];
        Ok(FloatType::from(beat) + progress / (end - start))
    }

    /// Returns [`Self::beat_proportion`] the way music21's
    /// `getBeatProportionStr` writes it: the beat alone on the beat, otherwise
    /// the beat and the fraction of it elapsed, `2 1/2`, with the fraction's
    /// denominator limited to 16.
    pub fn beat_proportion_string(&self, offset: FloatType) -> Result<String> {
        let (beat, progress) = self.beat_progress(offset)?;
        let (start, end) = self.beat_spans()[beat as usize - 1];
        let proportion = progress / (end - start);
        if proportion == 0.0 {
            return Ok(beat.to_string());
        }
        let (numerator, denominator) = closest_fraction(proportion, 16);
        Ok(format!("{beat} {numerator}/{denominator}"))
    }

    /// Where each beat starts and ends, in quarter lengths from the start of
    /// the bar. The beats of a meter written additively are not all the same
    /// length, so this is read off the beat sequence rather than divided out.
    fn beat_spans(&self) -> Vec<(FloatType, FloatType)> {
        let mut spans = Vec::new();
        let mut position = 0.0;
        for part in self.beat_sequence.parts() {
            let end = position + part.quarter_length();
            spans.push((position, end));
            position = end;
        }
        if spans.is_empty() {
            spans.push((0.0, self.bar_quarter_length()));
        }
        spans
    }

    /// How long the beat holding an offset is: music21's `getBeatDuration`.
    ///
    /// A meter written additively answers differently along the bar — the
    /// first beat of `2/4+3/8` is two quarters long and the second is three
    /// eighths.
    pub fn beat_duration_at(&self, offset: FloatType) -> Result<Duration> {
        let index = self.beat_index(offset)?;
        let (start, end) = self.beat_spans()[index];
        Duration::new(end - start)
    }

    /// Which beat holds an offset, counted from nought.
    fn beat_index(&self, offset: FloatType) -> Result<usize> {
        if !offset.is_finite() || offset < 0.0 || offset >= self.bar_quarter_length() {
            return Err(Error::Meter(format!(
                "offset {offset} is outside a {} bar of {} quarter lengths",
                self.ratio_string(),
                self.bar_quarter_length()
            )));
        }
        let spans = self.beat_spans();
        for (index, (start, end)) in spans.iter().enumerate() {
            let _ = start;
            if offset < end - OFFSET_TOLERANCE {
                return Ok(index);
            }
        }
        Ok(spans.len() - 1)
    }

    /// Returns the quarter-length offset of each beat within one bar.
    pub fn beat_offsets(&self) -> Vec<FloatType> {
        self.beat_spans()
            .into_iter()
            .map(|(start, _)| start)
            .collect()
    }

    /// Returns the one-based beat containing `offset` quarter lengths into a bar.
    ///
    /// Matches music21's `getBeat`. Offsets at or beyond the end of the bar are
    /// rejected rather than wrapping.
    pub fn beat_at_offset(&self, offset: FloatType) -> Result<UnsignedIntegerType> {
        if !offset.is_finite() || offset < 0.0 || offset >= self.bar_quarter_length() {
            return Err(Error::Meter(format!(
                "offset {offset} is outside a {} bar of {} quarter lengths",
                self.ratio_string(),
                self.bar_quarter_length()
            )));
        }
        Ok(self.beat_index(offset)? as UnsignedIntegerType + 1)
    }
}

/// The fraction closest to `value` with a denominator no larger than
/// `max_denominator`, as Python's `Fraction.limit_denominator` finds it for
/// the proportions music21 prints. Ties go to the smaller denominator.
fn closest_fraction(value: FloatType, max_denominator: u32) -> (u32, u32) {
    let mut best = (value.round() as u32, 1);
    let mut best_error = (value - value.round()).abs();
    for denominator in 2..=max_denominator {
        let numerator = (value * FloatType::from(denominator)).round();
        let error = (value - numerator / FloatType::from(denominator)).abs();
        if error < best_error {
            best = (numerator as u32, denominator);
            best_error = error;
        }
    }
    best
}

impl std::fmt::Display for TimeSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.ratio_string())
    }
}

/// How far an offset may sit from a partition boundary and still be read as
/// on it.
const OFFSET_TOLERANCE: FloatType = 1e-9;

/// The shortest accent partition music21 can write, a 128th note.
const SHORTEST_PARTITION: FloatType = 4.0 / 128.0;

/// The longest, a whole note.
const LONGEST_PARTITION: FloatType = 4.0;

impl TimeSignature {
    /// How music21's default accent hierarchy divides the bar, three levels
    /// deep: how many parts the bar divides into, how many each of those
    /// divides into, and how many each of those divides into.
    ///
    /// music21 builds the hierarchy by subdividing a `MeterSequence`. The top
    /// level takes the beat count, except that one, two, four, eight, sixteen
    /// and thirty-two beats divide in two and three beats divide in three; a
    /// single beat takes the numerator's place in that rule. Below that, each
    /// span divides the way music21 divides a span by default: two or more
    /// triples into its triples, an even count of anything in two, a single
    /// unit in two, and an odd count into its units.
    fn accent_hierarchy(
        &self,
    ) -> (
        UnsignedIntegerType,
        UnsignedIntegerType,
        UnsignedIntegerType,
    ) {
        let beats = self.beat_count();
        let first = if beats > 1 { beats } else { self.numerator };
        let top = match first {
            1 | 2 | 4 | 8 | 16 | 32 => 2,
            3 => 3,
            other => other,
        };
        let (span, unit) = split_span(self.numerator, self.denominator, top);
        let second = default_division(span, unit);
        let (span, unit) = split_span(span, unit, second);
        let third = default_division(span, unit);
        // music21 partitions the accent sequence into that many equal parts
        // written as whole numbers of a note value, and gives up — leaving
        // the bar as one partition — when a part would be longer than a
        // whole note or shorter than a 128th.
        let partition = self.bar_quarter_length() / FloatType::from(top * second * third);
        if !(SHORTEST_PARTITION..=LONGEST_PARTITION).contains(&partition) {
            return (1, 1, 1);
        }
        (top, second, third)
    }

    /// The length of one partition of music21's default accent hierarchy, the
    /// finest level the accent weights are given at.
    pub fn accent_partition_quarter_length(&self) -> FloatType {
        let parts = self.accent_sequence.parts();
        if parts.len() > 1 {
            return parts[0].quarter_length();
        }
        let (top, second, third) = self.accent_hierarchy();
        self.bar_quarter_length() / FloatType::from(top * second * third)
    }

    /// The accent weight of every partition of the bar, music21's default
    /// `accentSequence`: `1.0` on the downbeat, halving with every level of
    /// the hierarchy a partition's start is not a boundary of, so `4/4` reads
    /// `1.0, 0.125, 0.25, 0.125, 0.5, 0.125, 0.25, 0.125`.
    pub fn accent_weights(&self) -> Vec<FloatType> {
        let carried: Vec<FloatType> = self
            .accent_sequence
            .parts()
            .iter()
            .map(MeterTerminal::weight)
            .collect();
        if carried.len() > 1 {
            return carried;
        }
        self.default_accent_weights()
    }

    /// The weights music21's default hierarchy gives the bar, which is what
    /// the accent sequence is built carrying. Kept apart from
    /// [`Self::accent_weights`], which reads the sequence, so that weights a
    /// caller has set are the ones read back.
    fn default_accent_weights(&self) -> Vec<FloatType> {
        let (top, second, third) = self.accent_hierarchy();
        let count = top * second * third;
        (0..count)
            .map(|index| {
                let depth = 1
                    + UnsignedIntegerType::from(index.is_multiple_of(third))
                    + UnsignedIntegerType::from(index.is_multiple_of(second * third))
                    + UnsignedIntegerType::from(index == 0);
                FloatType::from(2u32.pow(depth - 1)) / 8.0
            })
            .collect()
    }

    /// Whether an offset in quarter lengths starts an accent partition:
    /// music21's `getAccent`, which is false for any offset off the grid,
    /// beyond the bar included.
    pub fn accent(&self, offset: FloatType) -> bool {
        let partition = self.accent_partition_quarter_length();
        let index = (offset / partition).round();
        index >= 0.0
            && index < FloatType::from(self.accent_weights().len() as u32)
            && (offset - index * partition).abs() < OFFSET_TOLERANCE
    }

    /// The accent weight at an offset in quarter lengths: music21's
    /// `getAccentWeight`, the weight of the partition the offset falls in.
    /// An offset outside the bar is an error.
    pub fn accent_weight(&self, offset: FloatType) -> Result<FloatType> {
        self.accent_weight_with(offset, false, false)
    }

    /// The accent weight at an offset, read at a level of the accent
    /// sequence: music21's `getAccentWeight` with its `level`.
    pub fn accent_weight_at_level(
        &self,
        offset: FloatType,
        level: usize,
        force_position_match: bool,
        permit_meter_modulus: bool,
    ) -> Result<FloatType> {
        let bar = self.bar_quarter_length();
        let offset = if permit_meter_modulus {
            offset.rem_euclid(bar)
        } else {
            offset
        };
        if offset.is_nan() || offset < 0.0 || offset >= bar {
            return Err(Error::Meter(format!(
                "cannot access from qLenPos {} where total duration is {}",
                offset_repr(offset),
                offset_repr(bar)
            )));
        }
        let terminals = self.accent_sequence.level_list(level, true);
        if terminals.len() <= 1 {
            return self.accent_weight_with(offset, force_position_match, permit_meter_modulus);
        }
        let spans = self.accent_sequence.level_span(level);
        let smallest = terminals
            .iter()
            .map(MeterTerminal::weight)
            .fold(FloatType::INFINITY, FloatType::min);
        for (index, (start, end)) in spans.iter().enumerate() {
            if offset < end - OFFSET_TOLERANCE {
                if force_position_match && (offset - start).abs() >= OFFSET_TOLERANCE {
                    return Ok(smallest * 0.5);
                }
                return Ok(terminals[index].weight());
            }
        }
        Ok(terminals[terminals.len() - 1].weight())
    }

    /// Weighs the accent partitions of a level, looping the weights given
    /// over them: music21's `setAccentWeight`.
    pub fn set_accent_weight(&mut self, weights: &[FloatType], level: usize) -> Result<()> {
        self.accent_sequence.set_weights_at_level(level, weights)
    }

    /// [`Self::accent_weight`] with music21's two options. With
    /// `force_position_match` an offset that does not start a partition
    /// answers half the smallest weight rather than its partition's; with
    /// `permit_meter_modulus` an offset beyond the bar is read within it.
    pub fn accent_weight_with(
        &self,
        offset: FloatType,
        force_position_match: bool,
        permit_meter_modulus: bool,
    ) -> Result<FloatType> {
        let bar = self.bar_quarter_length();
        let offset = if permit_meter_modulus {
            offset.rem_euclid(bar)
        } else {
            offset
        };
        if offset.is_nan() || offset < 0.0 || offset >= bar {
            return Err(Error::Meter(format!(
                "cannot access from qLenPos {} where total duration is {}",
                offset_repr(offset),
                offset_repr(bar)
            )));
        }
        let weights = self.accent_weights();
        let partition = self.accent_partition_quarter_length();
        let index = ((offset + OFFSET_TOLERANCE) / partition).floor() as usize;
        let index = index.min(weights.len() - 1);
        if force_position_match
            && (offset - index as FloatType * partition).abs() >= OFFSET_TOLERANCE
        {
            let smallest = weights
                .iter()
                .copied()
                .fold(FloatType::INFINITY, FloatType::min);
            return Ok(smallest * 0.5);
        }
        Ok(weights[index])
    }

    /// The mean accent weight of what a stream holds: music21's
    /// `averageBeatStrength`, each element weighed where it falls in the
    /// bar, with an element off the accent grid counting half the smallest
    /// weight. `notes_only` weighs the notes, chords and rests alone; an empty
    /// stream weighs nothing.
    pub fn average_beat_strength(&self, stream: &crate::Stream, notes_only: bool) -> FloatType {
        let bar = self.bar_quarter_length();
        let offsets: Vec<FloatType> = if notes_only {
            stream
                .notes()
                .into_iter()
                .map(|(offset, _)| offset)
                .collect()
        } else {
            stream
                .recurse()
                .into_iter()
                .filter(|(_, element)| element.as_stream().is_none())
                .map(|(offset, _)| offset)
                .collect()
        };
        if offsets.is_empty() {
            return 0.0;
        }
        let total: FloatType = offsets
            .iter()
            .map(|offset| {
                self.accent_weight_with(offset.rem_euclid(bar), true, false)
                    .unwrap_or(0.0)
            })
            .sum();
        total / offsets.len() as FloatType
    }

    /// How many levels of the beat hierarchy start at an offset: music21's
    /// `getBeatDepth`, which quantizes the offset to the beat's division and
    /// then counts the beat level and the division level. A meter of one beat
    /// has one level and answers one everywhere in the bar; an offset outside
    /// the bar is an error.
    pub fn beat_depth(&self, offset: FloatType) -> Result<u8> {
        let bar = self.bar_quarter_length();
        if offset.is_nan() || offset < 0.0 || offset >= bar {
            return Err(Error::Meter(format!(
                "cannot access from qLenPos {}",
                offset_repr(offset)
            )));
        }
        let depth = self
            .beat_sequence
            .offset_to_depth(offset, crate::meter::OffsetAlign::Quantize)?;
        Ok(depth as u8)
    }
}

/// A whole bar as a sequence of one part, which is what music21 starts each
/// of a meter's four sequences as: `6/8` undivided is `{6/8}`, not a bare
/// span, so a sequence nobody has divided still has one part.
fn whole_bar(
    numerator: UnsignedIntegerType,
    denominator: UnsignedIntegerType,
) -> Result<MeterTerminal> {
    let mut bar = MeterTerminal::new(numerator, denominator)?;
    let whole = format!("{numerator}/{denominator}");
    bar.partition_by_parts(&[whole.as_str()])?;
    Ok(bar)
}

/// The word music21 allows before a ratio, saying how the bar is counted:
/// `"slow 6/8"` is counted in six and `"fast 6/8"` in two.
fn division_word(ratio: &str) -> Option<&str> {
    let first = ratio.trim().split('+').next()?.trim();
    let word = first.split_whitespace().next()?;
    matches!(word, "slow" | "fast").then_some(word)
}

/// Whether a bar is felt in compound beats: music21's `favorCompound`.
///
/// The word written before the ratio decides it where there is one. Where
/// there is not, a bare three written in quarters or longer is felt slow,
/// which is what counts `3/4` in three while `3/8` is counted in one.
fn favor_compound(
    numerator: UnsignedIntegerType,
    denominator: UnsignedIntegerType,
    word: Option<&str>,
) -> bool {
    match word {
        Some("slow") => false,
        Some("fast") => true,
        _ => !(numerator == 3 && denominator < 8),
    }
}

/// The time signature that fits what a measure holds: music21's
/// `bestTimeSignature`.
///
/// The shortest note value in the measure that is not a tuplet is the
/// denominator, halved until it divides the measure's length evenly, and the
/// count of it in the measure is the numerator, reduced to lowest terms with
/// the whole and half taken up to quarters. A measure that reads as `3/4` is
/// `6/8` instead when that weighs its notes at least as strongly, and one
/// that reads as `6/4` is whichever of `6/4`, `12/8` and `3/2` weighs them
/// most. A measure whose length is no binary fraction — tuplets — takes the
/// fraction itself.
pub fn best_time_signature(measure: &crate::Stream) -> Result<TimeSignature> {
    let elements = measure.recurse();
    let sum = elements
        .iter()
        .filter(|(_, element)| element.as_stream().is_none())
        .map(|(offset, element)| offset + element.quarter_length())
        .fold(0.0, FloatType::max);

    // The shortest sounding value that is not a tuplet, with its dots.
    let mut min_dur = 4.0;
    let mut min_dots = 0;
    for (_, element) in &elements {
        let sounds =
            element.is_note_or_chord() || matches!(element, crate::stream::StreamElement::Rest(_));
        let quarter_length = element.quarter_length();
        if sounds && quarter_length != 0.0 && quarter_length < min_dur && is_binary(quarter_length)
        {
            min_dur = quarter_length;
            min_dots = element.duration().map_or(0, Duration::dots);
        }
    }

    let (numerator, denominator) = if is_binary(sum) {
        binary_signature(sum, min_dur, min_dots)?
    } else {
        let (numerator, denominator) = crate::duration::limited_fraction(sum, 65535)
            .ok_or_else(|| Error::Meter("Cannot find a good match for this measure".to_string()))?;
        (
            numerator as UnsignedIntegerType,
            denominator as UnsignedIntegerType,
        )
    };
    let (numerator, denominator) = simplified_signature(numerator, denominator);

    let strength =
        |ratio: (UnsignedIntegerType, UnsignedIntegerType)| -> Result<(TimeSignature, FloatType)> {
            let signature = TimeSignature::new(ratio.0, ratio.1)?;
            let strength = signature.average_beat_strength(measure, true);
            Ok((signature, strength))
        };
    match (numerator, denominator) {
        // Three-four or six-eight, whichever weighs the notes more strongly.
        (3, 4) => {
            let (three_four, simple) = strength((3, 4))?;
            let (six_eight, compound) = strength((6, 8))?;
            Ok(if simple <= compound {
                six_eight
            } else {
                three_four
            })
        }
        // Six-four, twelve-eight or three-two, the same way.
        (6, 4) => {
            let (six_four, first) = strength((6, 4))?;
            let (twelve_eight, second) = strength((12, 8))?;
            let (three_two, third) = strength((3, 2))?;
            let most = first.max(second).max(third);
            Ok(if most == first {
                six_four
            } else if most == third {
                three_two
            } else {
                twelve_eight
            })
        }
        _ => TimeSignature::new(numerator, denominator),
    }
}

/// The numerator and denominator of a measure whose length is a binary
/// fraction: the shortest value halved, dot and all, until it divides the
/// length evenly, named as a note value for the denominator and counted for
/// the numerator, in lowest terms.
fn binary_signature(
    sum: FloatType,
    min_dur: FloatType,
    min_dots: u32,
) -> Result<(UnsignedIntegerType, UnsignedIntegerType)> {
    let no_match = || Error::Meter("Cannot find a good match for this measure".to_string());
    let smallest_type = crate::duration::DurationType::from_music21_name("128th")
        .expect("music21 names the 128th note");
    let limit = smallest_type.quarter_length();
    let dot_multiplier =
        FloatType::from(2u32.pow(min_dots + 1) - 1) / FloatType::from(2u32.pow(min_dots));

    let mut min_test = min_dur;
    let mut remaining = 10;
    while remaining > 0 {
        let parts = sum / min_test;
        if parts.floor() == parts || min_test <= limit {
            break;
        }
        min_test /= 2.0 * dot_multiplier;
        remaining -= 1;
    }
    let mut remaining = 10;
    while remaining > 0 {
        if min_test < limit {
            min_test = limit;
            break;
        }
        let (duration_type, matched) =
            crate::duration::quarter_length_to_closest_type(min_test).map_err(|_| no_match())?;
        if matched || duration_type == smallest_type {
            break;
        }
        min_test /= 2.0 * dot_multiplier;
        remaining -= 1;
    }
    let (duration_type, matched) =
        crate::duration::quarter_length_to_closest_type(min_test).map_err(|_| no_match())?;
    if !matched {
        return Err(Error::Meter(format!(
            "cannot find a type for denominator {min_test}"
        )));
    }
    let mut float_denominator = duration_type.type_number().unwrap_or(1.0);
    let mut multiplier = 1.0;
    let mut numerator_float = 0.0;
    while remaining > 0 {
        numerator_float = multiplier * sum / min_test;
        if numerator_float == numerator_float.floor() {
            break;
        }
        multiplier *= 2.0;
        remaining -= 1;
    }
    float_denominator *= multiplier;
    let numerator = numerator_float as UnsignedIntegerType;
    let denominator = float_denominator as UnsignedIntegerType;
    let divisor = num::integer::gcd(numerator, denominator).max(1);
    Ok((numerator / divisor, denominator / divisor))
}

/// The rare signatures written the usual way: sixteen-sixteen and one-one
/// are four-four, and a whole or half denominator is written in quarters.
fn simplified_signature(
    numerator: UnsignedIntegerType,
    denominator: UnsignedIntegerType,
) -> (UnsignedIntegerType, UnsignedIntegerType) {
    if numerator == denominator && !matches!(numerator, 2 | 4) {
        (4, 4)
    } else if numerator != denominator && denominator == 1 {
        (numerator * 4, 4)
    } else if numerator != denominator && denominator == 2 {
        (numerator * 2, 4)
    } else {
        (numerator, denominator)
    }
}

/// Whether a quarter length is a binary fraction music21 keeps as a float
/// rather than turning into a `Fraction`: one whose exact denominator is a
/// power of two no larger than its `DENOM_LIMIT`.
fn is_binary(quarter_length: FloatType) -> bool {
    (quarter_length * 32768.0).fract() == 0.0
}

/// A span of `count` units of `1/unit`, divided into `parts` equal spans, as
/// a count of a unit: `6/8` in two is `3/8`, and `1/4` in two is `1/8`.
fn split_span(
    count: UnsignedIntegerType,
    unit: UnsignedIntegerType,
    parts: UnsignedIntegerType,
) -> (UnsignedIntegerType, UnsignedIntegerType) {
    if count.is_multiple_of(parts) {
        (count / parts, unit)
    } else {
        (count, unit * parts)
    }
}

/// How music21 divides a span by default when nothing says otherwise: its
/// `MeterSequence.subdivide` with no count given, which takes the first of
/// the span's division options.
fn default_division(count: UnsignedIntegerType, unit: UnsignedIntegerType) -> UnsignedIntegerType {
    let _ = unit;
    if count > 3 && count.is_multiple_of(3) {
        count / 3
    } else if count == 1 || count.is_multiple_of(2) {
        2
    } else {
        count
    }
}

/// An offset written the way music21 writes one in a message: a whole number
/// as `3.0`, anything else as it is.
fn offset_repr(offset: FloatType) -> String {
    if offset.fract() == 0.0 {
        format!("{offset:.1}")
    } else {
        offset.to_string()
    }
}

#[cfg(test)]
mod tests {
    /// music21 writes a word before the ratio to say how the beat is felt,
    /// and writes an additive meter as parts that add up. Both were read as
    /// nonsense here, which is ten of music21's own meter examples.
    #[test]
    fn a_meter_is_read_as_music21_writes_it() {
        use crate::meter::TimeSignature;

        // The word before the ratio is not decoration: music21 counts a
        // `slow 6/8` in six and a plain one in two, so the two are different
        // meters that happen to be written over the same numbers.
        let plain = TimeSignature::from_ratio_string("6/8").unwrap();
        let slow = TimeSignature::from_ratio_string("slow 6/8").unwrap();
        assert_eq!(TimeSignature::from_ratio_string("fast 6/8").unwrap(), plain);
        assert_ne!(slow, plain);
        assert_eq!(
            (slow.numerator(), slow.denominator()),
            (plain.numerator(), plain.denominator())
        );
        assert_eq!(plain.beat_sequence().len(), 2);
        assert_eq!(slow.beat_sequence().len(), 6);

        let additive = TimeSignature::from_ratio_string("3/8+2/8").unwrap();
        assert_eq!((additive.numerator(), additive.denominator()), (5, 8));
        assert_eq!(
            TimeSignature::parts("3/8+2/8").unwrap(),
            vec![(3, 8), (2, 8)]
        );
        // A numerator with no denominator of its own takes the next one.
        assert_eq!(TimeSignature::parts("3+2/8").unwrap(), vec![(3, 8), (2, 8)]);
        assert_eq!(
            TimeSignature::parts("3+2+5/8+3/4").unwrap(),
            vec![(3, 8), (2, 8), (5, 8), (3, 4)]
        );
        // Parts in different notes come to what they add up to.
        let mixed = TimeSignature::from_ratio_string("3/8+1/4").unwrap();
        assert_eq!((mixed.numerator(), mixed.denominator()), (5, 8));

        assert!(TimeSignature::parts("3+2+5").is_err());
        assert!(TimeSignature::from_ratio_string("").is_err());
        assert!(TimeSignature::from_ratio_string("3.0/4.0").is_err());
    }

    #[test]
    fn the_sequences_are_partitioned_as_music21_partitions_them() {
        use crate::meter::TimeSignature;

        // Read off music21 11.0.0b9 directly: `str(TimeSignature(r).beatSequence)`
        // and `.beamSequence` for each meter below.
        let expected = [
            ("1/4", "{1/4}", "{1/4}"),
            ("2/4", "{{1/8+1/8}+{1/8+1/8}}", "{{1/8+1/8}+{1/8+1/8}}"),
            (
                "3/4",
                "{{1/8+1/8}+{1/8+1/8}+{1/8+1/8}}",
                "{{1/8+1/8}+{1/8+1/8}+{1/8+1/8}}",
            ),
            (
                "4/4",
                "{{1/8+1/8}+{1/8+1/8}+{1/8+1/8}+{1/8+1/8}}",
                "{{1/8+1/8}+{1/8+1/8}+{1/8+1/8}+{1/8+1/8}}",
            ),
            (
                "5/4",
                "{{1/8+1/8}+{1/8+1/8}+{1/8+1/8}+{1/8+1/8}+{1/8+1/8}}",
                "{{1/4+1/4}+{1/4+1/4+1/4}}",
            ),
            ("6/4", "{{1/4+1/4+1/4}+{1/4+1/4+1/4}}", "{3/4+3/4}"),
            ("2/2", "{{1/4+1/4}+{1/4+1/4}}", "{1/2+1/2}"),
            ("3/2", "{{1/4+1/4}+{1/4+1/4}+{1/4+1/4}}", "{1/2+1/2+1/2}"),
            ("3/8", "{3/8}", "{3/8}"),
            (
                "5/8",
                "{{1/16+1/16}+{1/16+1/16}+{1/16+1/16}+{1/16+1/16}+{1/16+1/16}}",
                "{2/8+3/8}",
            ),
            ("6/8", "{{1/8+1/8+1/8}+{1/8+1/8+1/8}}", "{3/8+3/8}"),
            (
                "9/8",
                "{{1/8+1/8+1/8}+{1/8+1/8+1/8}+{1/8+1/8+1/8}}",
                "{3/8+3/8+3/8}",
            ),
            (
                "12/8",
                "{{1/8+1/8+1/8}+{1/8+1/8+1/8}+{1/8+1/8+1/8}+{1/8+1/8+1/8}}",
                "{3/8+3/8+3/8+3/8}",
            ),
            (
                "5/16",
                "{{1/32+1/32}+{1/32+1/32}+{1/32+1/32}+{1/32+1/32}+{1/32+1/32}}",
                "{5/16}",
            ),
            (
                "slow 6/8",
                "{{1/16+1/16}+{1/16+1/16}+{1/16+1/16}+{1/16+1/16}+{1/16+1/16}+{1/16+1/16}}",
                "{3/8+3/8}",
            ),
            ("fast 6/8", "{{1/8+1/8+1/8}+{1/8+1/8+1/8}}", "{3/8+3/8}"),
            ("3/8+2/8", "{{1/8+1/8+1/8}+{1/8+1/8}}", "{2/8+3/8}"),
            ("2/4+3/8", "{{1/4+1/4}+{1/8+1/8+1/8}}", "{2/8+2/8+3/8}"),
        ];
        for (ratio, beats, beams) in expected {
            let signature = TimeSignature::from_ratio_string(ratio).unwrap();
            assert_eq!(
                signature.beat_sequence().to_string(),
                beats,
                "beats of {ratio}"
            );
            assert_eq!(
                signature.beam_sequence().to_string(),
                beams,
                "beams of {ratio}"
            );
        }
    }

    /// Read off music21's default `accentSequence` and `getBeatDepth`.
    #[test]
    fn accent_weights_and_beat_depths_follow_the_default_hierarchy() {
        let weights = |ratio: &str| {
            TimeSignature::from_ratio_string(ratio)
                .unwrap()
                .accent_weights()
        };
        assert_eq!(
            weights("4/4"),
            [1.0, 0.125, 0.25, 0.125, 0.5, 0.125, 0.25, 0.125]
        );
        assert_eq!(
            weights("6/8"),
            [
                1.0, 0.125, 0.25, 0.125, 0.25, 0.125, 0.5, 0.125, 0.25, 0.125, 0.25, 0.125
            ]
        );
        assert_eq!(
            weights("12/8"),
            [
                1.0, 0.125, 0.125, 0.25, 0.125, 0.125, 0.5, 0.125, 0.125, 0.25, 0.125, 0.125
            ]
        );
        assert_eq!(weights("3/4").len(), 12);
        assert_eq!(weights("24/8").len(), 24);
        assert_eq!(
            TimeSignature::new(1, 4)
                .unwrap()
                .accent_partition_quarter_length(),
            0.125
        );

        let three_four = TimeSignature::new(3, 4).unwrap();
        let read: Vec<FloatType> = (0..3)
            .map(|beat| three_four.accent_weight(FloatType::from(beat)).unwrap())
            .collect();
        assert_eq!(read, [1.0, 0.5, 0.5]);
        let beyond = three_four.accent_weight(3.0).unwrap_err().to_string();
        assert!(beyond.ends_with("cannot access from qLenPos 3.0 where total duration is 3.0"));
        // A bar whose parts would be longer than a whole note, or shorter
        // than a 128th, is one partition.
        assert_eq!(weights("16/1"), [1.0]);
        assert_eq!(weights("1/32"), [1.0]);
        assert_eq!(
            TimeSignature::new(24, 4)
                .unwrap()
                .accent_partition_quarter_length(),
            1.0
        );
        assert_eq!(
            three_four.accent_weight_with(4.0, false, true).unwrap(),
            0.5
        );
        assert_eq!(
            three_four.accent_weight_with(0.1, true, false).unwrap(),
            0.0625
        );
        assert_eq!(
            three_four.accent_weight_with(0.1, false, false).unwrap(),
            1.0
        );
        assert!(three_four.accent(2.0));
        assert!(!three_four.accent(0.1));
        assert!(!three_four.accent(3.0));

        assert_eq!(three_four.beat_depth(0.0).unwrap(), 2);
        assert_eq!(three_four.beat_depth(0.25).unwrap(), 2);
        assert_eq!(three_four.beat_depth(0.5).unwrap(), 1);
        assert_eq!(three_four.beat_depth(1.0).unwrap(), 2);
        assert!(three_four.beat_depth(3.0).is_err());
        assert_eq!(
            TimeSignature::new(3, 8).unwrap().beat_depth(0.5).unwrap(),
            1
        );
        assert_eq!(
            TimeSignature::new(6, 8).unwrap().beat_depth(1.0).unwrap(),
            1
        );
        assert_eq!(
            TimeSignature::new(6, 8).unwrap().beat_depth(1.5).unwrap(),
            2
        );
    }

    /// music21's own `bestTimeSignature` examples.
    #[test]
    fn the_best_time_signature_fits_what_the_measure_holds() {
        use crate::{Note, Pitch, Stream};

        let measure = |lengths: &[FloatType]| {
            let mut stream = Stream::new();
            for length in lengths {
                let mut note = Note::from_pitch(Pitch::from_name("C4").unwrap());
                note.set_duration(Duration::new(*length).unwrap());
                stream.push(note);
            }
            stream
        };
        let best = |lengths: &[FloatType]| {
            best_time_signature(&measure(lengths))
                .unwrap()
                .ratio_string()
        };
        assert_eq!(best(&[1.0, 1.0, 0.5, 0.5]), "3/4");
        assert_eq!(best(&[0.75, 0.25, 0.5, 0.75, 0.25, 0.5]), "6/8");
        assert_eq!(best(&[2.0, 2.0, 2.0]), "3/2");
        assert_eq!(best(&[0.75, 0.25, 0.5, 0.75, 0.25, 0.5, 1.5, 1.5]), "12/8");
        assert_eq!(best(&[1.0, 2.0, 1.0, 2.0]), "6/4");
        assert_eq!(best(&[1.0, 0.375]), "11/32");
        assert_eq!(best(&[3.5, 5.5]), "9/4");
        assert_eq!(best(&[1.0, 1.0, 1.0, 1.0]), "4/4");
        assert_eq!(best(&[4.0]), "4/4");
        // Tuplets are passed over when the shortest value is looked for.
        assert_eq!(best(&[1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0, 1.0]), "2/4");
    }

    /// music21's own `averageBeatStrength` example: `C4 D4 E8 F8` under
    /// six-eight and three-four.
    #[test]
    fn beat_strength_is_averaged_over_a_stream() {
        use crate::{Note, Pitch, Stream};

        let mut stream = Stream::new();
        for (name, length) in [("C4", 1.0), ("D4", 1.0), ("E4", 0.5), ("F4", 0.5)] {
            let mut note = Note::from_pitch(Pitch::from_name(name).unwrap());
            note.set_duration(Duration::new(length).unwrap());
            stream.push(note);
        }
        assert_eq!(
            TimeSignature::new(6, 8)
                .unwrap()
                .average_beat_strength(&stream, true),
            0.4375
        );
        assert_eq!(
            TimeSignature::new(3, 4)
                .unwrap()
                .average_beat_strength(&stream, true),
            0.5625
        );
        stream.insert(0.0, TimeSignature::new(3, 4).unwrap());
        stream.insert(0.0, TimeSignature::new(6, 8).unwrap());
        assert_eq!(
            TimeSignature::new(6, 8)
                .unwrap()
                .average_beat_strength(&stream, false),
            0.625
        );
        assert_eq!(
            TimeSignature::new(4, 4)
                .unwrap()
                .average_beat_strength(&Stream::new(), true),
            0.0
        );
    }

    #[test]
    fn a_signature_reports_its_two_numbers_and_its_bar() {
        let six_eight = TimeSignature::new(6, 8).unwrap();
        assert_eq!((six_eight.numerator(), six_eight.denominator()), (6, 8));
        assert_eq!(six_eight.bar_duration().quarter_length(), 3.0);
    }

    #[test]
    fn division_helpers_match_music21() {
        let quarter_lengths = |durations: Vec<Duration>| -> Vec<FloatType> {
            durations.into_iter().map(|d| d.quarter_length()).collect()
        };
        let cases: [(&str, FloatType, &str, &[FloatType], usize); 7] = [
            ("4/4", 1.0, "Simple", &[0.5, 0.5], 4),
            ("6/8", 0.5, "Compound", &[0.5, 0.5, 0.5], 6),
            ("2/2", 2.0, "Simple", &[1.0, 1.0], 4),
            ("3/8", 0.5, "Other", &[1.5], 2),
            ("7/8", 0.5, "Simple", &[0.25, 0.25], 4),
            ("12/16", 0.25, "Compound", &[0.25, 0.25, 0.25], 6),
            ("1/4", 1.0, "Other", &[1.0], 2),
        ];
        for (ratio, beat_to_quarter, division_name, divisions, sub_divisions) in cases {
            let ts = ts(ratio);
            assert_eq!(
                ts.beat_length_to_quarter_length_ratio(),
                beat_to_quarter,
                "{ratio}"
            );
            assert_eq!(
                ts.quarter_length_to_beat_length_ratio(),
                1.0 / beat_to_quarter,
                "{ratio}"
            );
            assert_eq!(ts.beat_division_count_name(), division_name, "{ratio}");
            assert_eq!(
                quarter_lengths(ts.beat_division_durations().unwrap()),
                divisions,
                "{ratio}"
            );
            let subs = quarter_lengths(ts.beat_sub_division_durations().unwrap());
            assert_eq!(subs.len(), sub_divisions, "{ratio}");
            assert!(subs.iter().all(|ql| *ql == divisions[0] / 2.0), "{ratio}");
        }
        assert!(ts("4/4").ratio_equal(&ts("4/4")));
        assert!(!ts("4/4").ratio_equal(&ts("2/2")));
    }

    #[test]
    fn beat_positions_match_music21() {
        let common = ts("4/4");
        let cases: [(FloatType, UnsignedIntegerType, FloatType, FloatType, &str); 7] = [
            (0.0, 1, 0.0, 1.0, "1"),
            (0.5, 1, 0.5, 1.5, "1 1/2"),
            (1.25, 2, 0.25, 2.25, "2 1/4"),
            (2.75, 3, 0.75, 3.75, "3 3/4"),
            (3.0, 4, 0.0, 4.0, "4"),
            (3.75, 4, 0.75, 4.75, "4 3/4"),
            (3.9, 4, 0.9, 4.9, "4 9/10"),
        ];
        for (offset, beat, progress, proportion, text) in cases {
            let (actual_beat, actual_progress) = common.beat_progress(offset).unwrap();
            assert_eq!(actual_beat, beat, "{offset}");
            assert!((actual_progress - progress).abs() < 1e-9, "{offset}");
            assert!(
                (common.beat_proportion(offset).unwrap() - proportion).abs() < 1e-9,
                "{offset}"
            );
            assert_eq!(
                common.beat_proportion_string(offset).unwrap(),
                text,
                "{offset}"
            );
        }

        let compound = ts("6/8");
        assert_eq!(compound.beat_proportion_string(0.5).unwrap(), "1 1/3");
        assert_eq!(compound.beat_proportion_string(1.0).unwrap(), "1 2/3");
        assert_eq!(compound.beat_proportion_string(2.5).unwrap(), "2 2/3");
        assert!((compound.beat_proportion(2.0).unwrap() - 7.0 / 3.0).abs() < 1e-9);
        assert_eq!(ts("3/8").beat_proportion_string(1.0).unwrap(), "1 2/3");
        assert!(common.beat_progress(4.0).is_err());
    }

    #[test]
    fn offset_from_beat_matches_music21() {
        let common = ts("4/4");
        for (beat, offset) in [
            (1.0, 0.0),
            (1.5, 0.5),
            (2.5, 1.5),
            (3.25, 2.25),
            (4.75, 3.75),
        ] {
            assert_eq!(common.offset_from_beat(beat).unwrap(), offset, "{beat}");
        }
        assert!(common.offset_from_beat(5.0).is_err());
        assert!(common.offset_from_beat(0.5).is_err());
        let compound = ts("6/8");
        assert_eq!(compound.offset_from_beat(1.5).unwrap(), 0.75);
        assert_eq!(compound.offset_from_beat(2.5).unwrap(), 2.25);
        assert!((compound.offset_from_beat(2.999).unwrap() - 2.9985).abs() < 1e-9);
        assert_eq!(ts("3/8").offset_from_beat(1.5).unwrap(), 0.75);
    }

    #[test]
    fn closest_fraction_limits_the_denominator_like_python() {
        assert_eq!(closest_fraction(0.5, 16), (1, 2));
        assert_eq!(closest_fraction(1.0 / 3.0, 16), (1, 3));
        assert_eq!(closest_fraction(0.9, 16), (9, 10));
        assert_eq!(closest_fraction(0.75, 16), (3, 4));
        assert_eq!(closest_fraction(0.1234, 16), (1, 8));
    }
    use super::*;

    fn ts(ratio: &str) -> TimeSignature {
        TimeSignature::from_ratio_string(ratio).expect("valid time signature")
    }

    #[test]
    fn common_and_cut_time_match_their_ratios() {
        assert_eq!(TimeSignature::common().ratio_string(), "4/4");
        assert_eq!(TimeSignature::cut().ratio_string(), "2/2");
        assert_eq!(TimeSignature::default(), TimeSignature::common());
    }

    #[test]
    fn bar_and_beat_lengths_follow_the_ratio() {
        assert_eq!(ts("4/4").bar_quarter_length(), 4.0);
        assert_eq!(ts("5/16").bar_quarter_length(), 1.25);
        assert_eq!(ts("3/8").bar_quarter_length(), 1.5);
        assert_eq!(ts("6/8").beat_quarter_length().unwrap(), 1.5);
        assert_eq!(ts("4/4").beat_quarter_length().unwrap(), 1.0);
        assert_eq!(ts("2/2").beat_duration().unwrap().quarter_length(), 2.0);
    }

    #[test]
    fn compound_meters_beat_in_threes() {
        for (ratio, beats, division) in [
            ("6/8", 2, BeatDivision::Compound),
            ("9/8", 3, BeatDivision::Compound),
            ("12/8", 4, BeatDivision::Compound),
            ("15/8", 5, BeatDivision::Compound),
            ("18/8", 6, BeatDivision::Compound),
            ("24/8", 8, BeatDivision::Compound),
        ] {
            assert_eq!(ts(ratio).beat_count(), beats, "{ratio}");
            assert_eq!(ts(ratio).beat_division(), division, "{ratio}");
            assert!(ts(ratio).is_compound(), "{ratio}");
        }
    }

    #[test]
    fn three_is_the_one_denominator_sensitive_numerator() {
        // 3/2 and 3/4 read as three beats; 3/8 and shorter read as one.
        assert_eq!(ts("3/2").beat_count(), 3);
        assert_eq!(ts("3/4").beat_count(), 3);
        assert_eq!(ts("3/8").beat_count(), 1);
        assert_eq!(ts("3/16").beat_count(), 1);
        assert_eq!(ts("3/32").beat_count(), 1);
        // The cut is at an eighth rather than at a power of two: music21
        // counts 3/3 and 3/6 in three, and 3/12 in one.
        assert_eq!(ts("3/3").beat_count(), 3);
        assert_eq!(ts("3/6").beat_count(), 3);
        assert_eq!(ts("3/12").beat_count(), 1);
        // Every other numerator ignores the denominator entirely.
        for denominator in [2, 4, 8, 16] {
            assert_eq!(TimeSignature::new(6, denominator).unwrap().beat_count(), 2);
            assert_eq!(TimeSignature::new(5, denominator).unwrap().beat_count(), 5);
        }
    }

    #[test]
    fn a_bar_written_in_unequal_parts_is_counted_along_its_own_beats() {
        use crate::meter::TimeSignature;

        // Read off music21 11.0.0b9. `2/4+3/8` is two beats, of two quarters
        // and of three eighths, so nothing here is the bar divided evenly.
        let mixed = TimeSignature::from_ratio_string("2/4+3/8").unwrap();
        assert_eq!(mixed.bar_quarter_length(), 3.5);
        assert_eq!(mixed.beat_count(), 2);
        assert_eq!(mixed.beat_offsets(), vec![0.0, 2.0]);
        assert_eq!(mixed.beat_at_offset(2.5).unwrap(), 2);
        assert_eq!(mixed.beat_duration_at(0.0).unwrap().quarter_length(), 2.0);
        assert_eq!(mixed.beat_duration_at(2.5).unwrap().quarter_length(), 1.5);
        assert_eq!(mixed.offset_from_beat(2.0).unwrap(), 2.0);
        // Half way through the first beat is half of *that* beat, not half of
        // an averaged one.
        assert_eq!(mixed.offset_from_beat(1.5).unwrap(), 1.0);
        assert_eq!(mixed.beat_proportion_string(2.5).unwrap(), "2 1/3");
        assert_eq!(mixed.beat_depth(0.0).unwrap(), 2);

        let other = TimeSignature::from_ratio_string("3/8+2/8").unwrap();
        assert_eq!(other.beat_offsets(), vec![0.0, 1.5]);
        assert_eq!(other.offset_from_beat(2.0).unwrap(), 1.5);
        assert_eq!(other.offset_from_beat(1.5).unwrap(), 0.75);
        // The bar is 2.5 long, so 2.5 is past the end of it.
        assert!(other.beat_proportion_string(2.5).is_err());

        // An evenly divided bar answers exactly as it did.
        let common = TimeSignature::common();
        assert_eq!(common.beat_offsets(), vec![0.0, 1.0, 2.0, 3.0]);
        assert_eq!(common.offset_from_beat(2.0).unwrap(), 1.0);
        assert_eq!(common.offset_from_beat(1.5).unwrap(), 0.5);
        assert_eq!(common.beat_proportion_string(2.5).unwrap(), "3 1/2");
    }

    #[test]
    fn a_bar_whose_beats_differ_has_no_one_beat_length() {
        use crate::meter::TimeSignature;

        // music21 11.0.0b9 raises on beatDuration here and reports the
        // division as Other, counting one.
        let mixed = TimeSignature::from_ratio_string("2/4+3/8").unwrap();
        assert!(mixed.beat_quarter_length().is_err());
        assert!(mixed.beat_duration().is_err());
        assert!(mixed.beat_division_durations().is_err());
        assert_eq!(mixed.beat_division_count(), 1);
        assert_eq!(mixed.beat_division_count_name(), "Other");
        assert_eq!(mixed.classification(), "Other Duple");

        let other = TimeSignature::from_ratio_string("3/8+2/8").unwrap();
        assert!(other.beat_quarter_length().is_err());
        assert_eq!(other.beat_division_count(), 1);
        assert_eq!(other.classification(), "Other Duple");

        // An evenly divided bar still answers, and answers as it did.
        assert_eq!(TimeSignature::common().beat_quarter_length().unwrap(), 1.0);
        assert_eq!(
            TimeSignature::from_ratio_string("6/8")
                .unwrap()
                .beat_quarter_length()
                .unwrap(),
            1.5
        );
    }

    #[test]
    fn weights_a_caller_sets_are_the_weights_read_back() {
        use crate::meter::TimeSignature;

        let mut common = TimeSignature::common();
        let default = common.accent_weights();
        assert_eq!(default[0], 1.0);

        // music21 loops a shorter list over the partitions.
        common.set_accent_weight(&[0.8, 0.2], 0).unwrap();
        let set = common.accent_weights();
        assert_eq!(set.len(), default.len());
        assert!((set[0] - 0.8).abs() < 1e-9);
        assert!((set[1] - 0.2).abs() < 1e-9);
        assert!((set[2] - 0.8).abs() < 1e-9);

        // And the reading at an offset comes off the same sequence.
        assert!((common.accent_weight(0.0).unwrap() - 0.8).abs() < 1e-9);

        // A fresh meter is untouched by that.
        assert_eq!(TimeSignature::common().accent_weights()[0], 1.0);
    }

    #[test]
    fn classification_joins_division_and_count() {
        assert_eq!(ts("4/4").classification(), "Simple Quadruple");
        assert_eq!(ts("6/8").classification(), "Compound Duple");
        assert_eq!(ts("3/8").classification(), "Other Single");
        assert_eq!(ts("5/4").classification(), "Simple Quintuple");
        assert_eq!(ts("13/8").classification(), "Simple 13-uple");
        assert_eq!(ts("21/16").classification(), "Compound Septuple");
    }

    #[test]
    fn beat_offsets_partition_the_bar() {
        assert_eq!(ts("4/4").beat_offsets(), [0.0, 1.0, 2.0, 3.0]);
        assert_eq!(ts("6/8").beat_offsets(), [0.0, 1.5]);
        assert_eq!(ts("5/8").beat_offsets(), [0.0, 0.5, 1.0, 1.5, 2.0]);
    }

    #[test]
    fn beat_at_offset_is_one_based_and_bounded() {
        assert_eq!(ts("4/4").beat_at_offset(1.5).unwrap(), 2);
        assert_eq!(ts("6/8").beat_at_offset(1.5).unwrap(), 2);
        assert_eq!(ts("5/8").beat_at_offset(1.5).unwrap(), 4);
        assert_eq!(ts("4/4").beat_at_offset(0.0).unwrap(), 1);
        assert!(ts("4/4").beat_at_offset(4.0).is_err());
        assert!(ts("4/4").beat_at_offset(-0.5).is_err());
        assert!(ts("4/4").beat_at_offset(FloatType::NAN).is_err());
    }

    #[test]
    fn irrational_denominators_are_accepted_as_music21_accepts_them() {
        let four_three = ts("4/3");
        assert!((four_three.bar_quarter_length() - 16.0 / 3.0).abs() < 1e-12);
        assert_eq!(four_three.beat_count(), 4);
    }

    #[test]
    fn malformed_ratios_error_instead_of_panicking() {
        for ratio in [
            "", "4", "4/", "/4", "4/4/4", "x/4", "4/x", "0/4", "4/0", "-1/4",
        ] {
            assert!(
                TimeSignature::from_ratio_string(ratio).is_err(),
                "{ratio:?} should not parse"
            );
        }
    }

    #[test]
    fn display_is_the_ratio_string() {
        assert_eq!(ts("7/8").to_string(), "7/8");
    }
}
