//! An analysis run over windows of a stream, from a quarter note long to
//! the whole of it: music21's `analysis.windowed`.

use crate::{
    error::{Error, Result},
    makenotation::{make_measures_by, make_ties},
    meter::TimeSignature,
    stream::{Stream, StreamEvent, StreamKind},
};

/// How a window of several quarters moves along the stream:
/// [`WindowedAnalysis::analyze`]'s `windowType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum WindowType {
    /// A window starting at every quarter: quarters one to three, then two
    /// to four, and on.
    #[default]
    Overlap,
    /// Windows side by side: quarters one to three, then four to six. Where
    /// the quarters divide evenly into windows a last, empty window is
    /// analysed too, as music21 does.
    NoOverlap,
    /// For each quarter, every overlapping window holding it, put end to
    /// end: one answer per quarter.
    AdjacentAverage,
}

/// How [`WindowedAnalysis::process`] goes from one window size to the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum WindowStep {
    /// Each size this many quarters larger than the last: music21's
    /// `windowStepSize` given as a number.
    Add(usize),
    /// Each size this many times the last, until past three quarters of the
    /// largest: music21's `windowStepSize` given as text, `'2x'`.
    Multiply(usize),
}

/// A stream cut into windows of a quarter note each, ready to have an
/// analysis run over windows of any number of them: music21's
/// `WindowedAnalysis`.
///
/// music21 hands each window to an analysis object's `process`; here the
/// analysis is a function of the window, and a window is a stream of the
/// quarter-note measures it spans, put end to end. A window holding nothing
/// the analysis can use is the analysis's to answer for, as an `Option` or
/// a `Result`.
///
/// ```
/// use music21_rs::analysis::{KeyProfile, estimate_key_of_stream};
/// use music21_rs::analysis::windowed::{WindowType, WindowedAnalysis};
/// use music21_rs::tinynotation::from_tiny_notation;
///
/// let line = from_tiny_notation("4/4 c4 e g c' f a c' f'")?;
/// let windowed = WindowedAnalysis::new(&line)?;
/// assert_eq!(windowed.window_count(), 8);
/// let keys = windowed.analyze(4, WindowType::Overlap, |window| {
///     estimate_key_of_stream(KeyProfile::KrumhanslSchmuckler, window)
///         .map(|estimates| estimates[0].key().tonic_pitch_name_with_case())
/// });
/// assert_eq!(keys.first(), Some(&Some("C".to_string())));
/// assert_eq!(keys.last(), Some(&Some("F".to_string())));
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct WindowedAnalysis {
    windows: Vec<Stream>,
}

impl WindowedAnalysis {
    /// Cuts a stream into measures of one quarter note, a note running past
    /// a barline tied into the next: music21's `getMinimumWindowStream`. A
    /// stream holding parts is flattened first, its parts sounding as one.
    ///
    /// # Errors
    ///
    /// A stream that cannot be put into measures.
    pub fn new(stream: &Stream) -> Result<Self> {
        let source = if stream.has_part_like_streams() {
            stream.flatten()
        } else {
            stream.clone()
        };
        let mut measured = make_measures_by(&source, &[(0.0, TimeSignature::new(1, 4)?)])?;
        let measures: Vec<StreamEvent> = measured
            .events()
            .iter()
            .filter(|event| {
                event
                    .element()
                    .as_stream()
                    .is_some_and(|inner| inner.kind() == StreamKind::Measure)
            })
            .cloned()
            .collect();
        if measures.is_empty() {
            return Err(Error::Analysis("making measures failed".to_string()));
        }
        measured = measured.with_events(measures);
        make_ties(&mut measured)?;
        let windows = measured
            .events()
            .iter()
            .filter_map(|event| event.element().as_stream().cloned())
            .collect();
        Ok(Self { windows })
    }

    /// The quarter-note measures every window is made of.
    pub fn minimum_windows(&self) -> &[Stream] {
        &self.windows
    }

    /// How many quarter-note measures there are.
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// The analysis run over every window of `window_size` quarters, in
    /// order: music21's `analyze`, its answers without the colours.
    pub fn analyze<T>(
        &self,
        window_size: usize,
        window_type: WindowType,
        mut analysis: impl FnMut(&Stream) -> T,
    ) -> Vec<T> {
        let count = self.windows.len();
        match window_type {
            WindowType::Overlap => (0..(count + 1).saturating_sub(window_size))
                .map(|start| analysis(&self.window(start..start + window_size)))
                .collect(),
            WindowType::NoOverlap => {
                let windows = count.checked_div(window_size).map_or(0, |whole| whole + 1);
                (0..windows)
                    .map(|index| {
                        let start = (index * window_size).min(count);
                        let end = (start + window_size).min(count);
                        analysis(&self.window(start..end))
                    })
                    .collect()
            }
            WindowType::AdjacentAverage => {
                let overlapped: Vec<std::ops::Range<usize>> = (0..(count + 1)
                    .saturating_sub(window_size))
                    .map(|start| start..start + window_size)
                    .collect();
                (0..count)
                    .map(|quarter| {
                        let held: Vec<usize> = overlapped
                            .iter()
                            .filter(|range| range.contains(&quarter))
                            .flat_map(Clone::clone)
                            .collect();
                        analysis(&self.end_to_end(held))
                    })
                    .collect()
            }
        }
    }

    /// The analysis run over windows of each size from `min_window` to
    /// `max_window` quarters, stepping by `step`, each size with its
    /// answers: music21's `process`. A size left out is the whole stream,
    /// and with `include_total_window` the whole stream is always among
    /// the sizes.
    ///
    /// # Errors
    ///
    /// Multiplying by less than two, which never grows.
    pub fn process<T>(
        &self,
        min_window: Option<usize>,
        max_window: Option<usize>,
        step: WindowStep,
        window_type: WindowType,
        include_total_window: bool,
        mut analysis: impl FnMut(&Stream) -> T,
    ) -> Result<Vec<(usize, Vec<T>)>> {
        let total = self.windows.len();
        let largest = max_window.unwrap_or(total);
        let smallest = min_window.unwrap_or(total);
        let mut sizes: Vec<usize> = match step {
            WindowStep::Add(by) => {
                if by == 0 {
                    return Err(Error::Analysis(
                        "a window step cannot be nought".to_string(),
                    ));
                }
                (smallest..=largest).step_by(by).collect()
            }
            WindowStep::Multiply(by) => {
                if by < 2 {
                    return Err(Error::Analysis(
                        "a window size multiplied by less than two never grows".to_string(),
                    ));
                }
                let mut sizes = vec![smallest];
                let mut size = smallest;
                loop {
                    size *= by;
                    if size as f64 > largest as f64 * 0.75 || smallest == 0 {
                        break;
                    }
                    sizes.push(size);
                }
                sizes
            }
        };
        if include_total_window && !sizes.contains(&total) {
            sizes.push(total);
        }
        Ok(sizes
            .into_iter()
            .map(|size| (size, self.analyze(size, window_type, &mut analysis)))
            .collect())
    }

    fn window(&self, range: std::ops::Range<usize>) -> Stream {
        self.end_to_end(range.collect())
    }

    /// The measures at these indices, each starting where the last ended.
    fn end_to_end(&self, indices: Vec<usize>) -> Stream {
        let mut window = Stream::new();
        for index in indices {
            window.push(self.windows[index].clone());
        }
        window
    }
}
