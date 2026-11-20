//! The ticks and labels of a graph's axes: the numbers behind music21's
//! `graph.axis` classes.
//!
//! An axis of music21's reads its values from a stream and then says
//! where its ticks go and what each says. The reading is the wheel's,
//! over whatever stream it is given; what the ticks are, given what was
//! read, is here.

use crate::defaults::FloatType;
use crate::dynamics::SHORT_NAMES;
use crate::error::{Error, Result};
use crate::pitch::Pitch;

use super::py_range;
use super::utilities::accidental_label_to_unicode;

/// The barlines a measure tick is never left out at: music21's
/// `bar.strongBarlineTypes`.
pub const STRONG_BARLINES: [&str; 5] = ["heavy", "double", "final", "heavy-light", "heavy-heavy"];

/// Whether a barline type is a strong one.
///
/// ```
/// use music21::graph::axis::is_strong_barline;
///
/// assert!(is_strong_barline("final"));
/// assert!(!is_strong_barline("regular"));
/// ```
pub fn is_strong_barline(kind: &str) -> bool {
    STRONG_BARLINES.contains(&kind)
}

/// The lowest an axis's ticks start from when no minimum is set.
const DEFAULT_MIN: FloatType = 0.0;

/// The highest an axis's ticks reach when no maximum is set.
const DEFAULT_MAX: FloatType = 10.0;

/// Python's `int()` of a float: toward nought.
fn truncate(value: FloatType) -> i64 {
    value.trunc() as i64
}

/// About ten ticks at a power of ten apart, covering `min` to `max`:
/// music21's `Axis.ticks`. A missing bound is 0 or 10.
///
/// ```
/// use music21::graph::axis::numeric_ticks;
///
/// assert_eq!(numeric_ticks(Some(1.0), Some(9.0))?, (0..=10).collect::<Vec<_>>());
/// assert_eq!(numeric_ticks(Some(7.0), Some(80.0))?[..3], [0, 10, 20]);
/// assert_eq!(numeric_ticks(Some(712.0), Some(2213.0))?[0], 600);
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// A maximum below the minimum, or a span under a tenth, which have no
/// whole step to count with.
pub fn numeric_ticks(min: Option<FloatType>, max: Option<FloatType>) -> Result<Vec<i64>> {
    let min = min.unwrap_or(DEFAULT_MIN);
    let max = max.unwrap_or(DEFAULT_MAX);
    let difference = max - min;
    if difference < 0.0 {
        return Err(Error::Value("math domain error".to_string()));
    }

    // The power of ten just under the span, a tenth of it for spans of
    // two such powers or less, so there are at least three steps.
    let power = if difference == 0.0 {
        0
    } else {
        truncate(difference.log10())
    };
    if power < 0 {
        return Err(Error::Value(
            "'float' object cannot be interpreted as an integer".to_string(),
        ));
    }
    let mut step = 10_i64.pow(power as u32);
    if step > 1 && difference / step as FloatType <= 2.0 {
        step /= 10;
    }

    let mut start = (truncate(min / step as FloatType) - 1) * step;
    if start < 0 && min >= 0.0 {
        start = 0;
    }
    let stop = (truncate(max / step as FloatType) + 2) * step;
    Ok(py_range(start, stop, step))
}

/// How a pitch axis writes octaves: music21's `showOctaves`, which is
/// `False`, `'few'` or `True`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Octaves {
    /// No octave numbers.
    Hidden,
    /// Only on the first label of each octave.
    First,
    /// On every label.
    All,
}

/// How a pitch axis labels its ticks: music21's `PitchAxis` attributes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PitchLabels {
    /// music21's `showOctaves`.
    pub octaves: Octaves,
    /// Whether a tick names every spelling found of it, or only the most
    /// used: music21's `showEnharmonic`.
    pub show_enharmonic: bool,
    /// Whether a tick no pitch found is left unlabelled, rather than
    /// named: music21's `blankLabelUnused`.
    pub blank_label_unused: bool,
    /// Whether a tick no pitch found is left out: music21's `hideUnused`.
    pub hide_unused: bool,
}

/// A name with `A` sorted after `G`, as music21 sorts enharmonics.
fn sort_name(name: &str) -> String {
    match name.strip_prefix('A') {
        Some(rest) => format!("H{rest}"),
        None => name.to_string(),
    }
}

/// The digits taken out of a label.
fn without_digits(label: &str) -> String {
    label.chars().filter(|c| !c.is_ascii_digit()).collect()
}

/// A pitch axis's ticks from `min` to `max`, one a whole number:
/// music21's `PitchAxis._pitchTickHelper`.
///
/// `counts` is how often each name was found, in the order found;
/// `value_of` says where a name stands on the axis, and `unused_label`
/// names a tick no name stands at, where one is wanted.
///
/// ```
/// use music21::graph::axis::{Octaves, PitchLabels, pitch_ticks};
///
/// let labels = PitchLabels {
///     octaves: Octaves::Hidden,
///     show_enharmonic: true,
///     blank_label_unused: true,
///     hide_unused: false,
/// };
/// let counts = [("G#".to_string(), 1), ("A-".to_string(), 2)];
/// let ticks = pitch_ticks(labels, 7, 8, &counts, |_| Ok(8.0), |_| Ok(String::new()))?;
/// assert_eq!(ticks, vec![(7, String::new()), (8, "G\u{266f}/A\u{266d}".to_string())]);
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// What `value_of` or `unused_label` returns.
pub fn pitch_ticks(
    labels: PitchLabels,
    min: i64,
    max: i64,
    counts: &[(String, usize)],
    value_of: impl Fn(&str) -> Result<FloatType>,
    unused_label: impl Fn(i64) -> Result<String>,
) -> Result<Vec<(i64, String)>> {
    let mut values = Vec::with_capacity(counts.len());
    for (name, _) in counts {
        values.push(value_of(name)?);
    }

    let mut ticks = Vec::new();
    let mut octaves_seen: Vec<char> = Vec::new();
    for tick in min..=max {
        // The names found at this tick, ordered as music21 orders them.
        let mut weights: Vec<(usize, &str)> = counts
            .iter()
            .zip(&values)
            .filter(|(_, value)| **value == tick as FloatType)
            .map(|((name, count), _)| (*count, name.as_str()))
            .collect();
        if labels.show_enharmonic {
            weights.sort_by_key(|(count, name)| (*count as i64, sort_name(name)));
        } else {
            weights.sort_by_key(|(count, name)| (-(*count as i64), sort_name(name)));
        }

        let mut label = if weights.is_empty() {
            if labels.hide_unused {
                continue;
            }
            if labels.blank_label_unused {
                String::new()
            } else {
                unused_label(tick)?
            }
        } else if !labels.show_enharmonic {
            weights[0].1.to_string()
        } else {
            let names: Vec<String> = weights
                .iter()
                .map(|(_, name)| accidental_label_to_unicode(name))
                .collect();
            names.join("/")
        };

        // Octave numbers as asked.
        match labels.octaves {
            Octaves::Hidden => label = without_digits(&label),
            Octaves::First => {
                if let Some(digit) = label.chars().find(char::is_ascii_digit) {
                    if octaves_seen.contains(&digit) {
                        label = without_digits(&label);
                    } else {
                        octaves_seen.push(digit);
                    }
                }
            }
            Octaves::All => {}
        }
        ticks.push((tick, accidental_label_to_unicode(&label)));
    }
    Ok(ticks)
}

/// A tick at each octave of `start` from `min` to `max`: music21's
/// `PitchSpaceOctaveAxis.ticks`.
///
/// ```
/// use music21::graph::axis::octave_ticks;
///
/// let ticks = octave_ticks("A2", 36.0, 100.0)?;
/// assert_eq!(ticks[0], (45, "A2".to_string()));
/// assert_eq!(ticks.len(), 5);
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// A start that names no pitch.
pub fn octave_ticks(start: &str, min: FloatType, max: FloatType) -> Result<Vec<(i64, String)>> {
    let mut pitch = Pitch::from_name(start)?;
    let mut ticks = Vec::new();
    while pitch.ps() <= max {
        if pitch.ps() >= min {
            let label = accidental_label_to_unicode(&pitch.name_with_octave());
            ticks.push((truncate(pitch.ps()), label));
        }
        let Some(octave) = pitch.octave() else {
            return Err(Error::Value(format!("{start} names no octave")));
        };
        pitch.set_octave(Some(octave + 1));
    }
    Ok(ticks)
}

/// Ticks every `step` from `min` to `max`, rounded outward: music21's
/// `OffsetAxis.ticks` where there are no measures.
///
/// ```
/// use music21::graph::axis::offset_ticks;
///
/// assert_eq!(offset_ticks(0.0, 20.0, 10), vec![0, 10, 20]);
/// assert_eq!(offset_ticks(0.5, 4.5, 2), vec![0, 2, 4]);
/// ```
pub fn offset_ticks(min: FloatType, max: FloatType, step: i64) -> Vec<i64> {
    py_range(min.floor() as i64, max.ceil() as i64 + 1, step)
}

/// A measure an offset axis can tick: where it starts, its number, and
/// whether it ends in a strong barline.
#[derive(Clone, Debug, PartialEq)]
pub struct MeasureMark {
    /// Where the measure starts.
    pub offset: FloatType,
    /// Its number as it is written.
    pub number: String,
    /// Whether its right barline is heavy, double or final.
    pub strong: bool,
}

/// Which of the measures an offset axis ticks: music21's
/// `minMaxMeasureOnly`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeasureSpan {
    /// The first and the last only.
    Ends,
    /// The first, the last, every strong barline, and others evenly, to
    /// about this many in all: music21's `mostMeasureTicksToShow`.
    Spaced(i64),
}

/// The measure ticks of an offset axis, of the measures starting from
/// `min` to `max`: music21's `OffsetAxis._measureTicks`.
///
/// ```
/// use music21::graph::axis::{MeasureMark, MeasureSpan, measure_ticks};
///
/// let marks: Vec<MeasureMark> = (0..9)
///     .map(|n| MeasureMark { offset: f64::from(n) * 4.0, number: n.to_string(), strong: false })
///     .collect();
/// let ticks = measure_ticks(&marks, 0.0, 32.0, MeasureSpan::Ends)?;
/// assert_eq!(ticks, vec![(0.0, "0".to_string()), (32.0, "8".to_string())]);
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// No measure starting in the range, or a span that leaves no room for
/// any more ticks.
pub fn measure_ticks(
    marks: &[MeasureMark],
    min: FloatType,
    max: FloatType,
    span: MeasureSpan,
) -> Result<Vec<(FloatType, String)>> {
    let used: Vec<&MeasureMark> = marks
        .iter()
        .filter(|mark| min <= mark.offset && mark.offset <= max)
        .collect();
    if used.is_empty() {
        return Err(Error::Value("list index out of range".to_string()));
    }
    let tick_of = |index: usize| (used[index].offset, used[index].number.clone());

    let most = match span {
        MeasureSpan::Ends => {
            let mut ticks = vec![tick_of(0)];
            let last = tick_of(used.len() - 1);
            if !ticks.contains(&last) {
                ticks.push(last);
            }
            return Ok(ticks);
        }
        MeasureSpan::Spaced(most) => most,
    };

    // The first, the last, and every strong barline between.
    let mut chosen: Vec<usize> = Vec::new();
    let choose = |index: usize, chosen: &mut Vec<usize>| {
        if !chosen.contains(&index) {
            chosen.push(index);
        }
    };
    choose(0, &mut chosen);
    choose(used.len() - 1, &mut chosen);
    for (index, mark) in used.iter().enumerate().take(used.len() - 1).skip(1) {
        if mark.strong {
            choose(index, &mut chosen);
        }
    }

    // Then evenly, up to about `most`.
    let more = (most - chosen.len() as i64 + 1).min(used.len() as i64);
    if more == 0 {
        return Err(Error::Value(
            "integer division or modulo by zero".to_string(),
        ));
    }
    let step = ((used.len() as FloatType / more as FloatType).floor() as i64).max(1) as usize;
    let mut index = step;
    while index < used.len() - 1 {
        choose(index, &mut chosen);
        index += step;
    }

    let mut ticks: Vec<(FloatType, String)> = chosen.into_iter().map(tick_of).collect();
    ticks.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    Ok(ticks)
}

/// A tick for each dynamic from `min` to `max`, by its index in
/// `dynamics.shortNames`, written in TeX italics: music21's
/// `DynamicsAxis.ticks`.
///
/// ```
/// use music21::graph::axis::dynamics_ticks;
///
/// let ticks = dynamics_ticks(3, 4)?;
/// assert_eq!(ticks, vec![(3, "$ppp$".to_string()), (4, "$pp$".to_string())]);
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// An index past the last dynamic.
pub fn dynamics_ticks(min: i64, max: i64) -> Result<Vec<(i64, String)>> {
    let mut ticks = Vec::new();
    for index in min..=max {
        let at = if index < 0 {
            index + SHORT_NAMES.len() as i64
        } else {
            index
        };
        let Some(name) = usize::try_from(at).ok().and_then(|at| SHORT_NAMES.get(at)) else {
            return Err(Error::Value("list index out of range".to_string()));
        };
        ticks.push((index, format!("${name}$")));
    }
    Ok(ticks)
}

/// The index of the last dynamic: the most a dynamics axis reaches.
pub fn dynamics_max() -> i64 {
    SHORT_NAMES.len() as i64 - 1
}

/// Whether a quarter-length axis is scaled: music21's `useLogScale`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scale {
    /// As the lengths are.
    Linear,
    /// By their logarithm, base two.
    Log2,
}

/// Where a quarter length stands on a quarter-length axis: music21's
/// `dataFromQL`. A grace note, of no length, stands at `grace`.
///
/// ```
/// use music21::graph::axis::{Scale, quarter_length_value};
///
/// assert_eq!(quarter_length_value(0.5, Scale::Log2, 0.0625)?, -1.0);
/// assert_eq!(quarter_length_value(0.0, Scale::Log2, 0.0625)?, -4.0);
/// assert_eq!(quarter_length_value(0.0, Scale::Linear, 0.0625)?, 0.0625);
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// A negative length on a logarithmic axis.
pub fn quarter_length_value(ql: FloatType, scale: Scale, grace: FloatType) -> Result<FloatType> {
    match scale {
        Scale::Log2 => {
            let length = if ql == 0.0 { grace } else { ql };
            if length < 0.0 {
                return Err(Error::Graph(format!(
                    "cannot take log of x value: {length}"
                )));
            }
            Ok(length.log2())
        }
        Scale::Linear if ql > 0.0 => Ok(ql),
        Scale::Linear => Ok(grace),
    }
}

/// What a quarter-length axis's label gains for its scale: music21's
/// `labelLogTag`, given the base written, or none for a linear axis.
///
/// ```
/// use music21::graph::axis::log_label_tag;
///
/// assert_eq!(log_label_tag(Some(2)), " ($log_2$)");
/// assert_eq!(log_label_tag(None), "");
/// ```
pub fn log_label_tag(base: Option<i64>) -> String {
    match base {
        Some(base) => format!(" ($log_{base}$)"),
        None => String::new(),
    }
}

/// Whether a note carries a tie on to the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tied {
    /// It does.
    Onward,
    /// It does not.
    Not,
}

/// Where a note's bar starts and how long it is drawn, a little short of
/// its length so untied notes stand apart: music21's
/// `OffsetEndAxis.extractOneElement`.
///
/// ```
/// use music21::graph::axis::{Tied, offset_end};
///
/// assert_eq!(offset_end(1.0, 0.5, 0.0625, Tied::Not), (1.0, 0.4375));
/// assert_eq!(offset_end(1.0, 0.5, 0.0625, Tied::Onward), (1.0, 0.5));
/// ```
pub fn offset_end(
    offset: FloatType,
    length: FloatType,
    spacing: FloatType,
    tied: Tied,
) -> (FloatType, FloatType) {
    if length < spacing {
        return (offset, spacing);
    }
    if length > spacing * 2.0 && tied == Tied::Not {
        return (offset, length - spacing);
    }
    (offset, length)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measure_ticks_space_out_and_keep_strong_bars() {
        let offsets = [0.0, 1.0, 5.0, 9.0, 13.0, 17.0, 21.0, 25.0, 29.0];
        let mut marks: Vec<MeasureMark> = offsets
            .iter()
            .enumerate()
            .map(|(number, offset)| MeasureMark {
                offset: *offset,
                number: number.to_string(),
                strong: false,
            })
            .collect();
        let numbers = |ticks: Vec<(FloatType, String)>| -> Vec<String> {
            ticks.into_iter().map(|(_, label)| label).collect()
        };
        let ticks = measure_ticks(&marks, 0.0, 33.0, MeasureSpan::Spaced(4)).unwrap();
        assert_eq!(numbers(ticks), ["0", "3", "6", "8"]);
        marks[5].strong = true;
        let ticks = measure_ticks(&marks, 0.0, 33.0, MeasureSpan::Spaced(4)).unwrap();
        assert_eq!(numbers(ticks), ["0", "4", "5", "8"]);
        let ticks = measure_ticks(&marks, 8.0, 12.0, MeasureSpan::Spaced(20)).unwrap();
        assert_eq!(numbers(ticks), ["3"]);
    }

    #[test]
    fn pitch_ticks_write_octaves_once() {
        let labels = PitchLabels {
            octaves: Octaves::First,
            show_enharmonic: true,
            blank_label_unused: false,
            hide_unused: false,
        };
        let names = ["G#0", "A0", "B-0", "B0", "C1"];
        let ticks = pitch_ticks(
            labels,
            20,
            24,
            &[],
            |_| Ok(0.0),
            |tick| Ok(names[(tick - 20) as usize].to_string()),
        )
        .unwrap();
        let written: Vec<&str> = ticks.iter().map(|(_, label)| label.as_str()).collect();
        assert_eq!(written, ["G\u{266f}0", "A", "B\u{266d}", "B", "C1"]);
    }
}
