//! How a bar divides: music21's `MeterTerminal` and `MeterSequence`.
//!
//! A terminal is a span of a bar written as a ratio, with a weight. A
//! sequence is a terminal that is made of other terminals, so a bar of `4/4`
//! partitioned in two is `{1/2+1/2}`, and either half may be partitioned
//! again. music21 hangs four of these off every time signature — the beats,
//! the beams, the accents and what is displayed — and this is the part of it
//! that is arithmetic rather than notation.
//!
//! What a sequence may be partitioned into is not free: music21 keeps a list
//! of the ways each meter is conventionally divided, in priority order, and
//! partitioning by a count takes the first of those with that many parts.
//! That is why `5/8` in two is `{2/8+3/8}` and not `{2.5/8+2.5/8}`.

use crate::defaults::{FloatType, UnsignedIntegerType};
use crate::error::{Error, Result};

/// How close two quarter lengths must be to count as the same boundary.
const OFFSET_TOLERANCE: FloatType = 1e-9;

/// The denominators music21 will write a meter in.
const VALID_DENOMINATORS: [UnsignedIntegerType; 8] = [1, 2, 4, 8, 16, 32, 64, 128];

/// How an offset is matched against the boundaries of a level: music21's
/// `align` argument to `offsetToDepth`.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum OffsetAlign {
    /// Move the offset back to the start of the finest part holding it, then
    /// count the levels beginning there. music21's default.
    #[default]
    Quantize,
    /// Count only the levels whose part begins exactly at the offset.
    Start,
    /// Count the levels whose part ends exactly at the offset.
    End,
}

/// One span of a bar: a ratio, and how strongly it is felt.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MeterTerminal {
    numerator: UnsignedIntegerType,
    denominator: UnsignedIntegerType,
    weight: FloatType,
    /// What this span is itself divided into, if anything. A terminal with
    /// parts is music21's `MeterSequence`; one without is a leaf.
    parts: Vec<MeterTerminal>,
}

impl MeterTerminal {
    /// A span of `numerator/denominator`, undivided, weighing one.
    pub fn new(numerator: UnsignedIntegerType, denominator: UnsignedIntegerType) -> Result<Self> {
        if numerator == 0 {
            return Err(Error::Meter(
                "a meter terminal numerator must be non-zero".to_string(),
            ));
        }
        if denominator == 0 {
            return Err(Error::Meter(
                "a meter terminal denominator must be non-zero".to_string(),
            ));
        }
        Ok(Self {
            numerator,
            denominator,
            weight: 1.0,
            parts: Vec::new(),
        })
    }

    /// The same, read from `"3/8"`.
    pub fn from_ratio_string(ratio: &str) -> Result<Self> {
        let (numerator, denominator) = split_ratio(ratio)?;
        Self::new(numerator, denominator)
    }

    /// The numerator this span is written with.
    #[must_use]
    pub fn numerator(&self) -> UnsignedIntegerType {
        self.numerator
    }

    /// The denominator this span is written with.
    #[must_use]
    pub fn denominator(&self) -> UnsignedIntegerType {
        self.denominator
    }

    /// How long this span is in quarter notes.
    #[must_use]
    pub fn quarter_length(&self) -> FloatType {
        FloatType::from(self.numerator) * (4.0 / FloatType::from(self.denominator))
    }

    /// How strongly this span is felt: music21's `weight`.
    #[must_use]
    pub fn weight(&self) -> FloatType {
        self.weight
    }

    /// Sets how strongly this span is felt.
    pub fn set_weight(&mut self, weight: FloatType) {
        self.weight = weight;
    }

    /// What this span is divided into. Empty where it is a leaf.
    #[must_use]
    pub fn parts(&self) -> &[MeterTerminal] {
        &self.parts
    }

    /// The same, to be changed.
    pub fn parts_mut(&mut self) -> &mut Vec<MeterTerminal> {
        &mut self.parts
    }

    /// How many parts this span has at its top level, music21's `len`.
    #[must_use]
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    /// Whether this span is divided at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// How many levels of division this span has: music21's `depth`. A leaf
    /// is nought.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.parts
            .iter()
            .map(|part| 1 + part.depth())
            .max()
            .unwrap_or(0)
    }

    /// Every leaf of this span, in order: music21's `flatten`.
    #[must_use]
    pub fn flattened(&self) -> Vec<MeterTerminal> {
        if self.parts.is_empty() {
            return vec![self.clone()];
        }
        self.parts
            .iter()
            .flat_map(MeterTerminal::flattened)
            .collect()
    }

    /// Divides this span into the parts named, as `["2/8", "3/8"]`.
    ///
    /// The parts must come to what this span already is; a partition that
    /// lengthened or shortened the bar would not be one.
    pub fn partition_by_parts(&mut self, parts: &[&str]) -> Result<()> {
        let mut built = Vec::with_capacity(parts.len());
        for part in parts {
            built.push(MeterTerminal::from_ratio_string(part)?);
        }
        let total: FloatType = built.iter().map(MeterTerminal::quarter_length).sum();
        if (total - self.quarter_length()).abs() > OFFSET_TOLERANCE {
            return Err(Error::Meter(format!(
                "cannot set partition by {parts:?}: it comes to {total} where the meter is {}",
                self.quarter_length()
            )));
        }
        self.parts = built;
        Ok(())
    }

    /// Divides this span into `count` parts, the way music21 divides it.
    ///
    /// The first of [`Self::division_options`] with that many parts wins. A
    /// count nothing divides into leaves the first option standing, which is
    /// music21's `loadDefault`.
    pub fn partition_by_count(&mut self, count: usize, load_default: bool) -> Result<()> {
        let options = self.division_options();
        let chosen = options.iter().find(|option| option.len() == count);
        let chosen = match chosen {
            Some(option) => option.clone(),
            None => {
                if !load_default || options.is_empty() {
                    return Err(Error::Meter(format!(
                        "cannot set partition by {count} ({}/{})",
                        self.numerator, self.denominator
                    )));
                }
                options[0].clone()
            }
        };
        let borrowed: Vec<&str> = chosen.iter().map(String::as_str).collect();
        self.partition_by_parts(&borrowed)
    }

    /// Divides this span by a list of numerators, as `[3, 1]` for `{3/4+1/4}`.
    ///
    /// music21 reads the list against this span's own denominator where the
    /// numerators come to its numerator, and against a finer one where they
    /// come to a multiple of it — `[1, 1, 1, 1]` of a `2/4` bar is four
    /// eighths, not four quarters.
    pub fn partition_by_list(&mut self, numerators: &[UnsignedIntegerType]) -> Result<()> {
        if numerators.is_empty() {
            return Err(Error::Meter(
                "cannot partition a meter into nothing".to_string(),
            ));
        }
        let total: UnsignedIntegerType = numerators.iter().sum();
        for multiple in 1..=8 {
            if total != self.numerator * multiple {
                continue;
            }
            let denominator = self.denominator * multiple;
            let parts: Vec<String> = numerators
                .iter()
                .map(|numerator| format!("{numerator}/{denominator}"))
                .collect();
            let borrowed: Vec<&str> = parts.iter().map(String::as_str).collect();
            return self.partition_by_parts(&borrowed);
        }
        Err(Error::Meter(format!(
            "cannot set partition by {numerators:?} ({}/{})",
            self.numerator, self.denominator
        )))
    }

    /// Divides this span into `count` parts and leaves those parts divided
    /// as they were: music21's `MeterTerminal.subdivide`.
    pub fn subdivide(&mut self, count: usize) -> Result<()> {
        self.partition_by_count(count, false)
    }

    /// Divides every part of this span again, the way music21 divides it
    /// when nobody has said how: music21's `subdividePartitionsEqual`.
    ///
    /// `divisions` says how many parts each part becomes; `None` asks for
    /// the division each part conventionally takes — two for a part written
    /// in a binary numerator, three for one written in three, and threes for
    /// the compound numerators. A part nothing divides into is an error,
    /// which is what music21 raises and what a meter written in notes
    /// shorter than a 128th is forgiven.
    pub fn subdivide_partitions_equal(&mut self, divisions: Option<usize>) -> Result<()> {
        for part in &mut self.parts {
            let count = match divisions {
                Some(count) => count,
                None => match part.numerator {
                    1 | 2 | 4 | 8 | 16 | 32 | 64 => 2,
                    3 => 3,
                    6 | 9 | 12 | 15 | 18 | 21 | 24 | 27 => (part.numerator / 3) as usize,
                    other => other as usize,
                },
            };
            part.partition_by_count(count, false)?;
        }
        Ok(())
    }

    /// The ways music21 conventionally divides this meter, in the order it
    /// prefers them: music21's `getPartitionOptions`.
    #[must_use]
    pub fn division_options(&self) -> Vec<Vec<String>> {
        let mut options = division_options_algorithmic(self.numerator, self.denominator);
        options.extend(division_options_preset(self.numerator, self.denominator));
        let mut seen = Vec::new();
        options.retain(|option| {
            if option.is_empty() || seen.contains(option) {
                return false;
            }
            seen.push(option.clone());
            true
        });
        options
    }

    /// The terminals at one level of this sequence: music21's `getLevelList`.
    ///
    /// A part that is not divided is taken as it stands. A part that is gets
    /// recursed into while there are levels left to descend, and at the level
    /// asked for is either kept whole or, when `flat`, flattened into a
    /// single terminal carrying that part's own weight.
    #[must_use]
    pub fn level_list(&self, level: usize, flat: bool) -> Vec<MeterTerminal> {
        let mut out = Vec::new();
        for part in &self.parts {
            if part.parts.is_empty() {
                out.push(part.clone());
            } else if level > 0 {
                out.extend(part.level_list(level - 1, flat));
            } else if flat {
                let mut flattened = part.clone();
                flattened.parts.clear();
                out.push(flattened);
            } else {
                out.push(part.clone());
            }
        }
        out
    }

    /// One level of this sequence as a sequence of its own: music21's
    /// `getLevel`.
    pub fn level(&self, level: usize, flat: bool) -> Result<Self> {
        let mut out = Self::new(self.numerator, self.denominator)?;
        out.weight = self.weight;
        out.parts = self.level_list(level, flat);
        Ok(out)
    }

    /// Where each terminal of a level starts and ends, in quarter lengths
    /// from the start of this span: music21's `getLevelSpan`.
    #[must_use]
    pub fn level_span(&self, level: usize) -> Vec<(FloatType, FloatType)> {
        let mut spans = Vec::new();
        let mut position = 0.0;
        for part in self.level_list(level, true) {
            let end = position + part.quarter_length();
            spans.push((position, end));
            position = end;
        }
        spans
    }

    /// How many levels of this sequence start at an offset: music21's
    /// `offsetToDepth`.
    ///
    /// A level counts when one of its parts begins where the offset does.
    /// Under [`OffsetAlign::Quantize`] the offset is first moved back to the
    /// start of the finest part holding it, so an offset inside a part still
    /// counts the levels that part begins.
    pub fn offset_to_depth(&self, offset: FloatType, align: OffsetAlign) -> Result<usize> {
        let length = self.quarter_length();
        if offset.is_nan() || offset < 0.0 || offset >= length {
            return Err(Error::Meter(format!(
                "cannot access from qLenPos {offset} where total duration is {length}"
            )));
        }
        let depth = self.depth();
        if depth == 0 {
            return Ok(0);
        }
        let finest = self.level(depth - 1, true)?;
        let index = finest.offset_to_index(offset)?;
        let spans = self.level_span(depth - 1);
        let position = match align {
            OffsetAlign::Quantize => spans[index].0,
            OffsetAlign::Start | OffsetAlign::End => offset,
        };
        let mut score = 0;
        for level in 0..depth {
            for (start, end) in self.level_span(level) {
                let boundary = match align {
                    OffsetAlign::Start | OffsetAlign::Quantize => start,
                    OffsetAlign::End => end,
                };
                if (boundary - position).abs() < OFFSET_TOLERANCE {
                    score += 1;
                }
            }
        }
        Ok(score)
    }

    /// Weighs the terminals of a level, looping the weights given over them:
    /// the write half of music21's `setAccentWeight`.
    ///
    /// A level with no terminals, or no weights to give it, is an error
    /// rather than a silent nothing.
    pub fn set_weights_at_level(&mut self, level: usize, weights: &[FloatType]) -> Result<()> {
        if weights.is_empty() {
            return Err(Error::Meter(
                "a weight has to be given to weigh a level with".to_string(),
            ));
        }
        let mut index = 0;
        Self::weigh(&mut self.parts, level, weights, &mut index);
        if index == 0 {
            return Err(Error::Meter(format!(
                "this meter has no level {level} to weigh"
            )));
        }
        Ok(())
    }

    /// Walks the terminals of a level in order, weighing each in turn.
    fn weigh(parts: &mut [MeterTerminal], level: usize, weights: &[FloatType], index: &mut usize) {
        for part in parts.iter_mut() {
            if part.parts.is_empty() || level == 0 {
                part.weight = weights[*index % weights.len()];
                *index += 1;
            } else {
                Self::weigh(&mut part.parts, level - 1, weights, index);
            }
        }
    }

    /// Whether every part of a level is the same ratio: music21's
    /// `isUniformPartition`.
    #[must_use]
    pub fn is_uniform_partition(&self, depth: usize) -> bool {
        let mut numerator = None;
        let mut denominator = None;
        for part in self.level_list(depth, false) {
            if *numerator.get_or_insert(part.numerator) != part.numerator
                || *denominator.get_or_insert(part.denominator) != part.denominator
            {
                return false;
            }
        }
        true
    }

    /// The parts written out without the braces around them: music21's
    /// `partitionDisplay`, so a bar of `2/4+6/8` reads as it was written.
    #[must_use]
    pub fn partition_display(&self) -> String {
        self.parts
            .iter()
            .map(MeterTerminal::to_string)
            .collect::<Vec<String>>()
            .join("+")
    }

    /// This span divided into `count` parts, as a new sequence: music21's
    /// `subdivideByCount`. The weight of this span goes with it.
    pub fn subdivide_by_count(&self, count: usize) -> Result<Self> {
        let mut out = Self::new(self.numerator, self.denominator)?;
        out.weight = self.weight;
        out.partition_by_count(count, true)?;
        Ok(out)
    }

    /// This span divided by a list of numerators, as a new sequence:
    /// music21's `subdivideByList`.
    pub fn subdivide_by_list(&self, numerators: &[UnsignedIntegerType]) -> Result<Self> {
        let mut out = Self::new(self.numerator, self.denominator)?;
        out.weight = self.weight;
        out.partition_by_list(numerators)?;
        Ok(out)
    }

    /// Which part an offset in quarter notes falls in: music21's
    /// `offsetToIndex`. An offset outside the span is an error.
    pub fn offset_to_index(&self, offset: FloatType) -> Result<usize> {
        let length = self.quarter_length();
        if offset.is_nan() || offset < 0.0 || offset >= length {
            return Err(Error::Meter(format!(
                "cannot access from qLenPos {offset} where total duration is {length}"
            )));
        }
        let mut start = 0.0;
        for (index, part) in self.parts.iter().enumerate() {
            let end = start + part.quarter_length();
            if offset >= start - OFFSET_TOLERANCE && offset < end - OFFSET_TOLERANCE {
                return Ok(index);
            }
            start = end;
        }
        Ok(self.parts.len().saturating_sub(1))
    }

    /// Where the part an offset falls in begins and ends: music21's
    /// `offsetToSpan`. With `permit_meter_modulus` an offset past the end of
    /// the span is read within it.
    pub fn offset_to_span(
        &self,
        offset: FloatType,
        permit_meter_modulus: bool,
    ) -> Result<(FloatType, FloatType)> {
        let length = self.quarter_length();
        let offset = if permit_meter_modulus && offset >= length {
            offset.rem_euclid(length)
        } else {
            offset
        };
        let index = self.offset_to_index(offset)?;
        let mut start = 0.0;
        for (position, part) in self.parts.iter().enumerate() {
            let end = start + part.quarter_length();
            if position == index {
                return Ok((start, end));
            }
            start = end;
        }
        Ok((0.0, length))
    }

    /// The weight of the part an offset falls in: music21's `offsetToWeight`.
    pub fn offset_to_weight(&self, offset: FloatType) -> Result<FloatType> {
        let index = self.offset_to_index(offset)?;
        Ok(self
            .parts
            .get(index)
            .map_or(self.weight, MeterTerminal::weight))
    }

    /// How this span is written: `3/4` undivided, `{1/4+1/4+1/4}` divided.
    #[must_use]
    pub fn partition_string(&self) -> String {
        if self.parts.is_empty() {
            return format!("{}/{}", self.numerator, self.denominator);
        }
        let inner: Vec<String> = self
            .parts
            .iter()
            .map(MeterTerminal::partition_string)
            .collect();
        format!("{{{}}}", inner.join("+"))
    }
}

impl std::fmt::Display for MeterTerminal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.partition_string())
    }
}

/// `"3/8"` as the two numbers it is written with.
fn split_ratio(ratio: &str) -> Result<(UnsignedIntegerType, UnsignedIntegerType)> {
    let trimmed = ratio.trim();
    let (numerator, denominator) = trimmed.split_once('/').ok_or_else(|| {
        Error::Meter(format!(
            "a meter terminal is written numerator/denominator, not {ratio:?}"
        ))
    })?;
    let read = |part: &str, label: &str| {
        part.trim()
            .parse::<UnsignedIntegerType>()
            .map_err(|_| Error::Meter(format!("cannot read a {label} from {part:?} in {ratio:?}")))
    };
    Ok((
        read(numerator, "numerator")?,
        read(denominator, "denominator")?,
    ))
}

/// The same ratio in shorter notes, while the denominator allows: music21's
/// `divisionOptionsFractionsUpward`.
fn fractions_upward(
    numerator: UnsignedIntegerType,
    denominator: UnsignedIntegerType,
) -> Vec<String> {
    let largest = VALID_DENOMINATORS[VALID_DENOMINATORS.len() - 1];
    let mut out = Vec::new();
    if denominator >= largest {
        return out;
    }
    let (mut numerator, mut denominator) = (numerator * 2, denominator * 2);
    while denominator <= largest {
        out.push(format!("{numerator}/{denominator}"));
        numerator *= 2;
        denominator *= 2;
    }
    out
}

/// The same ratio in longer notes, while it stays whole: music21's
/// `divisionOptionsFractionsDownward`.
fn fractions_downward(
    numerator: UnsignedIntegerType,
    denominator: UnsignedIntegerType,
) -> Vec<String> {
    let smallest = VALID_DENOMINATORS[0];
    let mut out = Vec::new();
    if denominator <= smallest || !numerator.is_multiple_of(2) {
        return out;
    }
    let (mut numerator, mut denominator) = (numerator / 2, denominator / 2);
    loop {
        out.push(format!("{numerator}/{denominator}"));
        if !numerator.is_multiple_of(2) || denominator <= smallest {
            break;
        }
        numerator /= 2;
        denominator /= 2;
    }
    out
}

/// One unit at a time, then twice as many half as long: music21's
/// `divisionOptionsAdditiveMultiplesUpward`, which stops at sixteen parts or
/// at the numerator where that is larger.
fn additive_multiples_upward(
    numerator: UnsignedIntegerType,
    denominator: UnsignedIntegerType,
) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    if numerator <= 1 {
        return out;
    }
    let largest = VALID_DENOMINATORS[VALID_DENOMINATORS.len() - 1];
    let limit = if numerator > 16 { numerator } else { 16 };
    let (mut denominator, mut count) = (denominator, numerator);
    while denominator <= largest && count <= limit {
        out.push(vec![format!("1/{denominator}"); count as usize]);
        denominator *= 2;
        count *= 2;
    }
    out
}

/// The bar halved, and halved again while it stays even: music21's
/// `divisionOptionsAdditiveMultiplesEvenDivision`. A `4/4` bar reads
/// `1/2+1/2`, written in the note value a half of it actually is.
fn additive_multiples_even_division(
    numerator: UnsignedIntegerType,
    denominator: UnsignedIntegerType,
) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    if !numerator.is_multiple_of(2) || denominator < 2 {
        return out;
    }
    let (mut count, mut denominator) = (numerator / 2, denominator / 2);
    while denominator >= 1 && count > 1 {
        out.push(vec![format!("1/{denominator}"); count as usize]);
        if !count.is_multiple_of(2) || denominator == 1 {
            break;
        }
        denominator /= 2;
        count /= 2;
    }
    out
}

/// Even groupings in the bar's own note value: music21's
/// `divisionOptionsAdditiveMultiples`, which reads `6/4` as `3/4+3/4`.
fn additive_multiples(
    numerator: UnsignedIntegerType,
    denominator: UnsignedIntegerType,
) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    if numerator <= 3 || !numerator.is_multiple_of(2) {
        return out;
    }
    let mut divisor = 2;
    while numerator.is_multiple_of(divisor) {
        let count = numerator / divisor;
        if count <= 1 {
            break;
        }
        out.push(vec![format!("{count}/{denominator}"); divisor as usize]);
        divisor *= 2;
    }
    out
}

/// A single unit split into smaller ones: music21's
/// `divisionOptionsAdditiveMultiplesDownward`, which only ever applies where
/// the numerator is one.
fn additive_multiples_downward(
    numerator: UnsignedIntegerType,
    denominator: UnsignedIntegerType,
) -> Vec<Vec<String>> {
    let largest = VALID_DENOMINATORS[VALID_DENOMINATORS.len() - 1];
    let mut out = Vec::new();
    if denominator >= largest || numerator != 1 {
        return out;
    }
    let (mut count, mut denominator) = (2usize, denominator * 2);
    while denominator <= largest {
        out.push(vec![format!("{numerator}/{denominator}"); count]);
        denominator *= 2;
        count *= 2;
    }
    out
}

/// The ways music21 divides a meter, in the order it prefers them:
/// music21's `divisionOptionsAlgo`.
///
/// The order is the behaviour, not decoration:
/// [`MeterTerminal::partition_by_count`] takes the first option of the right
/// length, so these pieces are composed in the same sequence music21
/// composes them in.
fn division_options_algorithmic(
    numerator: UnsignedIntegerType,
    denominator: UnsignedIntegerType,
) -> Vec<Vec<String>> {
    let mut options: Vec<Vec<String>> = Vec::new();

    // Compound meters divide into threes first: 6/8 is two dotted beats.
    if numerator > 3 && numerator.is_multiple_of(3) {
        options.push(vec![format!("3/{denominator}"); (numerator / 3) as usize]);
    }
    // The odd meters music21 keeps a grouping for.
    let groupings: &[&[UnsignedIntegerType]] = match numerator {
        5 => &[&[2, 3], &[3, 2]],
        7 => &[&[2, 2, 3], &[3, 2, 2], &[2, 3, 2]],
        10 => &[&[2, 2, 3, 3]],
        _ => &[],
    };
    for grouping in groupings {
        options.push(
            grouping
                .iter()
                .map(|count| format!("{count}/{denominator}"))
                .collect(),
        );
    }
    options.extend(additive_multiples_upward(numerator, denominator));
    options.extend(additive_multiples_even_division(numerator, denominator));
    options.push(vec![format!("{numerator}/{denominator}")]);
    options.extend(additive_multiples(numerator, denominator));
    options.extend(additive_multiples_downward(numerator, denominator));
    for written in fractions_downward(numerator, denominator) {
        options.push(vec![written]);
    }
    for written in fractions_upward(numerator, denominator) {
        options.push(vec![written]);
    }
    options
}

/// The divisions music21 keeps by hand because no rule produces them: only
/// the two extra readings of a five.
fn division_options_preset(
    numerator: UnsignedIntegerType,
    denominator: UnsignedIntegerType,
) -> Vec<Vec<String>> {
    if numerator != 5 {
        return Vec::new();
    }
    vec![
        vec![
            format!("2/{denominator}"),
            format!("2/{denominator}"),
            format!("1/{denominator}"),
        ],
        vec![
            format!("2/{denominator}"),
            format!("1/{denominator}"),
            format!("2/{denominator}"),
        ],
    ]
}

#[cfg(test)]
mod tests {
    use super::MeterTerminal;
    use super::OffsetAlign;

    /// music21's own `divisionOptionsAlgo(4, 4)`, in its order.
    ///
    /// The order is what decides a partition: `partition_by_count` takes the
    /// first option of the length asked for, so `4/4` in two is `1/2+1/2`
    /// and not `2/4+2/4`, which comes later in the same list.
    #[test]
    fn levels_are_read_as_music21_reads_them() {
        // Read off music21 11.0.0b9: TimeSignature("4/4").beatSequence.
        let mut bar = MeterTerminal::new(4, 4).unwrap();
        bar.partition_by_parts(&["4/4"]).unwrap();
        bar.partition_by_count(4, true).unwrap();
        bar.subdivide_partitions_equal(None).unwrap();
        assert_eq!(bar.to_string(), "{{1/8+1/8}+{1/8+1/8}+{1/8+1/8}+{1/8+1/8}}");

        // getLevelList(0, True) is four quarters; (1, True) is eight eighths.
        let first: Vec<String> = bar
            .level_list(0, true)
            .iter()
            .map(MeterTerminal::to_string)
            .collect();
        assert_eq!(first, ["1/4", "1/4", "1/4", "1/4"]);
        let second: Vec<String> = bar
            .level_list(1, true)
            .iter()
            .map(MeterTerminal::to_string)
            .collect();
        assert_eq!(second, ["1/8"; 8]);

        // music21 reads this bar as two levels deep.
        assert_eq!(bar.depth(), 2);
        assert_eq!(bar.level_span(0).len(), 4);
        assert_eq!(bar.level_span(0)[1], (1.0, 2.0));

        // Every part of either level is the same ratio.
        assert!(bar.is_uniform_partition(0));
        assert!(bar.is_uniform_partition(1));
        // music21 reads the depth at an offset as 2, 1, 2 across the first beat.
        assert_eq!(bar.offset_to_depth(0.0, OffsetAlign::Quantize).unwrap(), 2);
        assert_eq!(bar.offset_to_depth(0.5, OffsetAlign::Quantize).unwrap(), 1);
        assert_eq!(bar.offset_to_depth(1.0, OffsetAlign::Quantize).unwrap(), 2);
    }

    #[test]
    fn a_bar_written_in_unequal_parts_is_not_uniform() {
        let mut bar = MeterTerminal::new(5, 8).unwrap();
        bar.partition_by_parts(&["2/8", "3/8"]).unwrap();
        assert!(!bar.is_uniform_partition(0));
        assert_eq!(bar.partition_display(), "2/8+3/8");
        assert_eq!(bar.depth(), 1);
    }

    #[test]
    fn subdividing_leaves_the_span_alone_and_returns_a_new_one() {
        let mut beat = MeterTerminal::new(1, 4).unwrap();
        beat.set_weight(0.5);
        let divided = beat.subdivide_by_count(2).unwrap();
        assert_eq!(divided.to_string(), "{1/8+1/8}");
        // music21's subdivide does not happen in place, and carries the
        // weight of the span it divided.
        assert!(beat.is_empty());
        assert!((divided.weight() - 0.5).abs() < 1e-9);

        let listed = MeterTerminal::new(5, 8)
            .unwrap()
            .subdivide_by_list(&[2, 3])
            .unwrap();
        assert_eq!(listed.to_string(), "{2/8+3/8}");
    }

    #[test]
    fn the_options_come_in_the_order_music21_offers_them() {
        use super::division_options_algorithmic;

        let offered: Vec<Vec<String>> = division_options_algorithmic(4, 4)
            .into_iter()
            .take(6)
            .collect();
        assert_eq!(
            offered,
            vec![
                vec!["1/4"; 4],
                vec!["1/8"; 8],
                vec!["1/16"; 16],
                vec!["1/2"; 2],
                vec!["4/4"],
                vec!["2/4"; 2],
            ]
        );

        // A compound meter is offered its threes first, and a five the two
        // groupings music21 keeps for it.
        assert_eq!(division_options_algorithmic(6, 8)[0], vec!["3/8"; 2]);
        assert_eq!(
            division_options_algorithmic(5, 8)[0],
            vec!["2/8".to_string(), "3/8".to_string()]
        );
    }

    /// music21's own examples, which is what this has to reproduce.
    #[test]
    fn a_bar_divides_the_way_music21_divides_it() {
        let mut four_four = MeterTerminal::from_ratio_string("4/4").unwrap();
        four_four.partition_by_count(2, true).unwrap();
        assert_eq!(four_four.partition_string(), "{1/2+1/2}");
        four_four.partition_by_count(4, true).unwrap();
        assert_eq!(four_four.partition_string(), "{1/4+1/4+1/4+1/4}");

        // Irregular meters take the grouping music21 keeps for them.
        let mut five_eight = MeterTerminal::from_ratio_string("5/8").unwrap();
        five_eight.partition_by_count(2, true).unwrap();
        assert_eq!(five_eight.partition_string(), "{2/8+3/8}");
        five_eight.partition_by_count(3, true).unwrap();
        assert_eq!(five_eight.partition_string(), "{2/8+2/8+1/8}");

        // A count nothing divides into falls back to the first option.
        let mut also_five = MeterTerminal::from_ratio_string("5/8").unwrap();
        also_five.partition_by_count(11, true).unwrap();
        assert_eq!(also_five.partition_string(), "{2/8+3/8}");
        assert!(
            MeterTerminal::from_ratio_string("5/8")
                .unwrap()
                .partition_by_count(11, false)
                .is_err()
        );

        // Compound meters divide into threes first.
        let mut six_eight = MeterTerminal::from_ratio_string("6/8").unwrap();
        six_eight.partition_by_count(2, true).unwrap();
        assert_eq!(six_eight.partition_string(), "{3/8+3/8}");
    }

    #[test]
    fn a_partition_must_come_to_what_the_bar_is() {
        let mut bar = MeterTerminal::from_ratio_string("4/4").unwrap();
        bar.partition_by_parts(&["3/4", "1/8", "1/8"]).unwrap();
        assert_eq!(bar.partition_string(), "{3/4+1/8+1/8}");
        assert!(bar.partition_by_parts(&["3/4", "1/8", "5/8"]).is_err());

        // A list of numerators is read against a finer note value where it
        // comes to a multiple of the meter's own.
        let mut halves = MeterTerminal::from_ratio_string("2/4").unwrap();
        halves.partition_by_list(&[1, 1]).unwrap();
        assert_eq!(halves.partition_string(), "{1/4+1/4}");
        halves.partition_by_list(&[1, 1, 1, 1]).unwrap();
        assert_eq!(halves.partition_string(), "{1/8+1/8+1/8+1/8}");
        assert!(halves.partition_by_list(&[1, 1, 1]).is_err());
    }

    #[test]
    fn an_offset_finds_the_part_it_falls_in() {
        let mut bar = MeterTerminal::from_ratio_string("4/4").unwrap();
        bar.partition_by_count(4, true).unwrap();
        assert_eq!(bar.offset_to_index(0.0).unwrap(), 0);
        assert_eq!(bar.offset_to_index(1.5).unwrap(), 1);
        assert_eq!(bar.offset_to_index(3.99).unwrap(), 3);
        assert_eq!(bar.offset_to_span(1.5, false).unwrap(), (1.0, 2.0));
        // Past the bar is an error, or the same offset read within it.
        assert!(bar.offset_to_index(4.0).is_err());
        assert!(bar.offset_to_index(-0.5).is_err());
        assert_eq!(bar.offset_to_span(5.5, true).unwrap(), (1.0, 2.0));
    }

    #[test]
    fn a_span_says_how_deep_and_how_flat_it_is() {
        let mut bar = MeterTerminal::from_ratio_string("4/4").unwrap();
        assert_eq!(bar.depth(), 0);
        bar.partition_by_count(2, true).unwrap();
        assert_eq!(bar.depth(), 1);
        bar.parts_mut()[0].partition_by_count(2, true).unwrap();
        assert_eq!(bar.depth(), 2);
        assert_eq!(bar.partition_string(), "{{1/4+1/4}+1/2}");
        assert_eq!(bar.flattened().len(), 3);
        assert!((bar.flattened()[0].quarter_length() - 1.0).abs() < 1e-9);
    }
}
