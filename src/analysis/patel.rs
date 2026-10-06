//! Aniruddh D. Patel's measures of rhythm and melody: music21's
//! `analysis.patel`.

use num::{BigUint, ToPrimitive};

use crate::{
    defaults::FloatType,
    error::{Error, Result},
    stream::{ConsecutiveOptions, Stream},
};

/// The normalized pairwise variability index of a stream's rhythm (Low,
/// Grabe and Nolan, 2000): music21's `nPVI`.
///
/// Each element the stream holds itself is compared with the one before by
/// how long it lasts -- the difference of the two lengths over their mean
/// -- and the comparisons are averaged and scaled by a hundred. A pair in
/// which either lasts no time adds nothing, but still counts. Every element
/// counts, clefs and meters included, so a stream of the notes and rests
/// alone is usually what is meant.
///
/// ```
/// use music21_rs::Stream;
/// use music21_rs::analysis::patel::n_pvi;
/// use music21_rs::stream::{StreamElement, StreamEvent};
/// use music21_rs::tinynotation::from_tiny_notation;
///
/// let line = from_tiny_notation("4/4 C4 D8 C4 D8 C4")?.flatten();
/// let notes = Stream::from_events(line.events().iter().cloned().filter(|event| {
///     matches!(event.element(), StreamElement::Note(_) | StreamElement::Rest(_))
/// }));
/// assert!((n_pvi(&notes)? - 66.666_666_666_666_67).abs() < 1e-9);
/// # Ok::<(), music21_rs::Error>(())
/// ```
///
/// # Errors
///
/// A stream holding fewer than two elements.
pub fn n_pvi(stream: &Stream) -> Result<FloatType> {
    let lengths: Vec<FloatType> = stream
        .events()
        .iter()
        .map(|event| event.element().quarter_length())
        .collect();
    if lengths.len() < 2 {
        return Err(Error::Analysis(
            "the pairwise variability of a rhythm needs two elements".to_string(),
        ));
    }
    let summation: FloatType = lengths
        .windows(2)
        .filter(|pair| pair[0] > 0.0 && pair[1] > 0.0)
        .map(|pair| (pair[1] - pair[0]).abs() / ((pair[1] + pair[0]) / 2.0))
        .sum();
    Ok(summation * 100.0 / (lengths.len() - 1) as FloatType)
}

/// The melodic interval variability of a stream (Patel, *Music, Language,
/// and the Brain*, p. 223): music21's `melodicIntervalVariability`.
///
/// It is a hundred times the coefficient of variation -- the sample
/// standard deviation over the mean -- of the sizes in semitones of the
/// intervals [`Stream::melodic_intervals`] finds with `options`, worked out
/// exactly and rounded once, as Python's `statistics` module does.
///
/// ```
/// use music21_rs::analysis::patel::melodic_interval_variability;
/// use music21_rs::stream::ConsecutiveOptions;
/// use music21_rs::tinynotation::from_tiny_notation;
///
/// let line = from_tiny_notation("4/4 C4 D E F G C")?;
/// let miv = melodic_interval_variability(&line, &ConsecutiveOptions::default())?;
/// assert!((miv - 85.266_688).abs() < 1e-6);
/// # Ok::<(), music21_rs::Error>(())
/// ```
///
/// # Errors
///
/// Fewer than two intervals, intervals averaging nothing, or as
/// [`Stream::melodic_intervals`].
pub fn melodic_interval_variability(
    stream: &Stream,
    options: &ConsecutiveOptions,
) -> Result<FloatType> {
    let sizes: Vec<FloatType> = stream
        .melodic_intervals(options)?
        .iter()
        .map(|step| step.interval().chromatic().undirected())
        .collect();
    if sizes.len() < 2 {
        return Err(Error::Analysis(
            "need at least three notes to have a std-deviation of intervals (and thus a MIV)"
                .to_string(),
        ));
    }
    let (deviation, mean) = deviation_and_mean(&sizes);
    if mean == 0.0 {
        return Err(Error::Analysis(
            "the intervals of a line of one pitch have no variability".to_string(),
        ));
    }
    Ok(100.0 * (deviation / mean))
}

/// The sample standard deviation and the mean of some values, each the
/// exact answer rounded once, as Python's `statistics.stdev` and `mean`
/// give them. Values that are whole numbers once scaled by a power of two
/// -- every interval a semitone or a quarter tone wide -- are summed as
/// integers; scaling by a power of two leaves the rounding where it was.
fn deviation_and_mean(values: &[FloatType]) -> (FloatType, FloatType) {
    let count = values.len() as i128;
    let scaled = (0..=40).find_map(|power| {
        let scale = (2.0 as FloatType).powi(power);
        values
            .iter()
            .map(|value| {
                let whole = value * scale;
                (whole.fract() == 0.0 && whole.abs() < 2e15).then_some(whole as i128)
            })
            .collect::<Option<Vec<i128>>>()
            .map(|scaled| (scale, scaled))
    });
    let Some((scale, scaled)) = scaled else {
        let mean = values.iter().sum::<FloatType>() / count as FloatType;
        let squares: FloatType = values.iter().map(|value| (value - mean).powi(2)).sum();
        return ((squares / (count - 1) as FloatType).sqrt(), mean);
    };
    let sum: i128 = scaled.iter().sum();
    let squares: i128 = scaled.iter().map(|value| value * value).sum();
    // The variance is (n * sum of squares - sum^2) / (n * (n - 1)).
    let numerator = (count * squares - sum * sum) as u128;
    let denominator = (count * (count - 1)) as u128;
    let deviation = square_root_of_fraction(numerator, denominator);
    let mean = sum as FloatType / count as FloatType;
    (deviation / scale, mean / scale)
}

/// The square root of a fraction, correctly rounded: Python's
/// `statistics._float_sqrt_of_frac`, which takes an integer square root
/// rounded to odd at twice a double's precision and lets the conversion to
/// a double round it once.
fn square_root_of_fraction(numerator: u128, denominator: u128) -> FloatType {
    if numerator == 0 {
        return 0.0;
    }
    const WIDTH: i64 = 2 * 53 + 3;
    let n = BigUint::from(numerator);
    let m = BigUint::from(denominator);
    let shift = (n.bits() as i64 - m.bits() as i64 - WIDTH).div_euclid(2);
    let rounded_to_odd = |n: &BigUint, m: &BigUint| -> BigUint {
        let root = (n / m).sqrt();
        let exact = &root * &root * m == *n;
        if exact {
            root
        } else {
            root | BigUint::from(1u8)
        }
    };
    if shift >= 0 {
        let root = rounded_to_odd(&n, &(m << (2 * shift) as usize));
        let root = root
            .to_u64()
            .map_or(FloatType::INFINITY, |root| root as FloatType);
        root * (2.0 as FloatType).powi(shift as i32)
    } else {
        let root = rounded_to_odd(&(n << (-2 * shift) as usize), &m);
        let root = root
            .to_u64()
            .map_or(FloatType::INFINITY, |root| root as FloatType);
        root / (2.0 as FloatType).powi((-shift) as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_square_root_is_rounded_once() {
        assert_eq!(square_root_of_fraction(2, 1), (2.0 as FloatType).sqrt());
        assert_eq!(square_root_of_fraction(9, 4), 1.5);
        assert_eq!(square_root_of_fraction(0, 7), 0.0);
        // Python: statistics.stdev([1, 2, 4]) == 1.5275252316519468
        let (deviation, mean) = deviation_and_mean(&[1.0, 2.0, 4.0]);
        assert_eq!(deviation, 1.527_525_231_651_946_8);
        assert_eq!(mean, 7.0 / 3.0);
    }

    #[test]
    fn half_steps_scale_without_moving_the_rounding() {
        let (whole, _) = deviation_and_mean(&[1.0, 2.0, 4.0]);
        let (half, mean) = deviation_and_mean(&[0.5, 1.0, 2.0]);
        assert_eq!(half * 2.0, whole);
        assert_eq!(mean, 3.5 / 3.0);
    }
}
