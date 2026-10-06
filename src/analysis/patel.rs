//! Aniruddh D. Patel's measures of rhythm and melody: music21's
//! `analysis.patel`.

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
    let (deviation, mean) = crate::statistics::deviation_and_mean(&sizes);
    if mean == 0.0 {
        return Err(Error::Analysis(
            "the intervals of a line of one pitch have no variability".to_string(),
        ));
    }
    Ok(100.0 * (deviation / mean))
}
