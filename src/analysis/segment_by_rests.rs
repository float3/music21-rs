//! A part cut into the runs of notes between its rests: music21's
//! `analysis.segmentByRests`.

use crate::{
    error::Result,
    interval::Interval,
    stream::{Stream, StreamElement},
};

/// The runs of notes between rests and clefs, through every stream nested
/// in this one, each note as its position in [`Stream::leaves`]: music21's
/// `Segmenter.getSegmentsList`.
///
/// Only single notes count: a chord or an unpitched stroke neither joins a
/// run nor ends one. Parts are read one after another with nothing between
/// them, so a part that opens without a clef or a rest carries on the last
/// run of the part before it. With `remove_empty` a rest or clef following
/// another gives no empty run.
///
/// ```
/// use music21_rs::analysis::segment_by_rests::segments_list;
/// use music21_rs::stream::StreamElement;
/// use music21_rs::tinynotation::from_tiny_notation;
///
/// let part = from_tiny_notation("C4 r D E r r F r G r A B r c")?;
/// let leaves = part.leaves();
/// let names: Vec<Vec<String>> = segments_list(&part, true)
///     .into_iter()
///     .map(|run| {
///         run.into_iter()
///             .map(|position| match leaves[position].1 {
///                 StreamElement::Note(note) => note.pitch().name(),
///                 _ => unreachable!(),
///             })
///             .collect()
///     })
///     .collect();
/// assert_eq!(names, [vec!["C"], vec!["D", "E"], vec!["F"], vec!["G"], vec!["A", "B"], vec!["C"]]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
pub fn segments_list(stream: &Stream, remove_empty: bool) -> Vec<Vec<usize>> {
    let marks = marks(stream);
    let mut segments: Vec<Vec<usize>> = Vec::new();
    let mut current = Vec::new();
    for (index, &(position, is_note)) in marks.iter().enumerate() {
        if is_note {
            current.push(position);
            if index == marks.len() - 1 {
                segments.push(std::mem::take(&mut current));
            }
        } else {
            segments.push(std::mem::take(&mut current));
        }
    }
    if remove_empty {
        segments.retain(|segment| !segment.is_empty());
    }
    segments
}

/// The interval between each two notes standing next to each other with no
/// rest or clef between them, through every stream nested in this one:
/// music21's `Segmenter.getIntervalList`.
///
/// # Errors
///
/// Two notes no interval can be spelled between.
pub fn interval_list(stream: &Stream) -> Result<Vec<Interval>> {
    let leaves = stream.leaves();
    let marks = marks(stream);
    let mut intervals = Vec::new();
    for pair in marks.windows(2) {
        let ((first, true), (second, true)) = (pair[0], pair[1]) else {
            continue;
        };
        if let (StreamElement::Note(start), StreamElement::Note(end)) =
            (leaves[first].1, leaves[second].1)
        {
            intervals.push(Interval::between_notes(start, end)?);
        }
    }
    Ok(intervals)
}

/// Every note, rest and clef in order, as its position among the leaves and
/// whether it is a note.
fn marks(stream: &Stream) -> Vec<(usize, bool)> {
    stream
        .leaves()
        .into_iter()
        .enumerate()
        .filter_map(|(position, (_, element))| match element {
            StreamElement::Note(_) => Some((position, true)),
            StreamElement::Rest(_) | StreamElement::Clef(_) => Some((position, false)),
            _ => None,
        })
        .collect()
}
