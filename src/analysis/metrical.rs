//! Metrical and melodic accent: music21's `analysis.metrical`.

use crate::{
    defaults::FloatType,
    error::{Error, Result},
    meter::TimeSignature,
    notation::TieType,
    stream::{Stream, StreamElement, StreamKind},
};

/// How many levels of the metrical hierarchy start at each note and rest of
/// a part's measures, as positions in [`Stream::leaves`] beside the count:
/// the depths music21's `labelBeatDepth` marks.
///
/// Each measure is read in the meter it states, else the one in force where
/// it starts, else four-four, nested three levels deep, and every note,
/// chord and rest it holds itself -- not those in its voices -- is counted
/// where it starts. A note ending a tie is passed over.
///
/// # Errors
///
/// A note standing outside its measure's bar.
pub fn beat_depths(part: &Stream) -> Result<Vec<(usize, u8)>> {
    let mut depths = Vec::new();
    let mut position = 0;
    for event in part.events() {
        let StreamElement::Stream(measure) = event.element() else {
            position += 1;
            continue;
        };
        let leaves = measure.leaves().len();
        if measure.kind() != StreamKind::Measure {
            position += leaves;
            continue;
        }
        let mut meter = measure_meter(part, measure, event.offset())?;
        meter
            .beat_sequence_mut()
            .subdivide_nested_hierarchy(3, None, true)?;
        let mut inside = position;
        for own in measure.events() {
            match own.element() {
                StreamElement::Stream(nested) => inside += nested.leaves().len(),
                element => {
                    if is_note_or_rest(element) && !ends_tie(element) {
                        depths.push((inside, meter.beat_depth(own.offset())?));
                    }
                    inside += 1;
                }
            }
        }
        position += leaves;
    }
    Ok(depths)
}

/// Marks each note and rest of a part's measures with a lyric of a star for
/// every level of the metrical hierarchy starting where it does: music21's
/// `labelBeatDepth`. The depths are [`beat_depths`]'.
///
/// ```
/// use music21_rs::analysis::metrical::label_beat_depth;
/// use music21_rs::stream::StreamElement;
/// use music21_rs::tinynotation::from_tiny_notation;
///
/// let mut part = from_tiny_notation("4/4 c8 c c c c c c c")?;
/// label_beat_depth(&mut part)?;
/// let stars: Vec<String> = part
///     .leaves()
///     .into_iter()
///     .filter_map(|(_, element)| match element {
///         StreamElement::Note(note) => {
///             Some(note.lyrics().iter().map(|lyric| lyric.text()).collect())
///         }
///         _ => None,
///     })
///     .collect();
/// assert_eq!(stars, ["****", "*", "**", "*", "***", "*", "**", "*"]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
///
/// # Errors
///
/// As [`beat_depths`], or a chord symbol to mark, which carries no lyric
/// here.
pub fn label_beat_depth(part: &mut Stream) -> Result<()> {
    let depths = beat_depths(part)?;
    let mut position = 0;
    let mut next = depths.iter().peekable();
    let mut refused = None;
    part.for_each_mut(&mut |_, element| {
        if let Some(&&(at, depth)) = next.peek()
            && at == position
        {
            next.next();
            for _ in 0..depth {
                let added = match element {
                    StreamElement::Note(note) => note.add_lyric("*", None, false),
                    StreamElement::Chord(chord) => chord.add_lyric("*", None, false),
                    StreamElement::Rest(rest) => {
                        rest.add_lyric("*", None, false);
                        Ok(())
                    }
                    StreamElement::Unpitched(stroke) => {
                        stroke.written_mut().add_lyric("*", None, false)
                    }
                    StreamElement::PercussionChord(chord) => {
                        chord.written_mut().add_lyric("*", None, false)
                    }
                    _ => Err(Error::Analysis(
                        "a chord symbol carries no lyric to mark its beat depth with".to_string(),
                    )),
                };
                if let Err(error) = added {
                    refused.get_or_insert(error);
                }
            }
        }
        position += 1;
    });
    refused.map_or(Ok(()), Err)
}

/// The meter a measure is read in: the first it holds, else the one in
/// force in the part where it starts, else four-four.
fn measure_meter(part: &Stream, measure: &Stream, offset: FloatType) -> Result<TimeSignature> {
    if let Some(meter) = measure
        .recurse()
        .into_iter()
        .find_map(|(_, element)| match element {
            StreamElement::TimeSignature(meter) => Some(meter.clone()),
            _ => None,
        })
    {
        return Ok(meter);
    }
    match part.time_signature_at(offset) {
        Some(meter) => Ok(meter),
        None => TimeSignature::new(4, 4),
    }
}

/// music21's `notesAndRests`: anything that sounds or rests.
fn is_note_or_rest(element: &StreamElement) -> bool {
    matches!(
        element,
        StreamElement::Note(_)
            | StreamElement::Chord(_)
            | StreamElement::Rest(_)
            | StreamElement::Unpitched(_)
            | StreamElement::PercussionChord(_)
            | StreamElement::ChordSymbol(_)
    )
}

fn ends_tie(element: &StreamElement) -> bool {
    let tie = match element {
        StreamElement::Note(note) => note.tie(),
        StreamElement::Chord(chord) => chord.tie(),
        StreamElement::Rest(rest) => rest.tie(),
        StreamElement::Unpitched(stroke) => stroke.written().tie(),
        StreamElement::PercussionChord(chord) => chord.written().tie(),
        _ => None,
    };
    tie.is_some_and(|tie| tie.tie_type() == TieType::Stop)
}

/// The melodic accent of each note of a line, after Thomassen (1982) as
/// Huron and Royal (1996) apply it: music21's `thomassenMelodicAccent`.
///
/// The first note takes an accent of one. Each note after it is accented by
/// the contour around it -- a peak or a valley, a turn, a run on or a note
/// repeated -- times what the contour before it left over for it, and the
/// last note takes what is left over. Only pitch heights are compared, so
/// enharmonic notes are one. The line is the notes the stream holds itself,
/// in order.
///
/// ```
/// use music21_rs::Stream;
/// use music21_rs::analysis::metrical::thomassen_melodic_accent;
/// use music21_rs::stream::StreamElement;
/// use music21_rs::tinynotation::from_tiny_notation;
///
/// let line = from_tiny_notation("7/4 c4 c c d e d d")?.flatten();
/// let notes = Stream::from_events(
///     line.events()
///         .iter()
///         .filter(|event| matches!(event.element(), StreamElement::Note(_)))
///         .cloned(),
/// );
/// let accents = thomassen_melodic_accent(&notes)?;
/// assert_eq!(accents, [1.0, 0.0, 0.0, 0.33, 0.67 * 0.83, 0.17, 0.0]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
///
/// # Errors
///
/// Anything in the stream but a note, once there are three elements to
/// compare.
pub fn thomassen_melodic_accent(line: &Stream) -> Result<Vec<FloatType>> {
    let events = line.events();
    let height = |index: usize| -> Result<FloatType> {
        match events[index].element() {
            StreamElement::Note(note) => Ok(note.pitch().ps()),
            _ => Err(Error::Analysis(
                "melodic accent is read off notes alone".to_string(),
            )),
        }
    };
    let last = events.len().saturating_sub(1);
    let mut accents = Vec::with_capacity(events.len());
    let mut carried = 1.0;
    for index in 0..events.len() {
        if index == 0 {
            accents.push(1.0);
            continue;
        }
        if index == last {
            accents.push(carried);
            continue;
        }
        let (before, here, after) = (height(index - 1)?, height(index)?, height(index + 1)?);
        let (this, next) = if before == here && here == after {
            (0.0, 0.0)
        } else if before != here && here == after {
            (1.0, 0.0)
        } else if before == here {
            (0.0, 1.0)
        } else if before < here && here > after {
            (0.83, 0.17)
        } else if before > here && here < after {
            (0.71, 0.29)
        } else if before < here && here < after {
            (0.33, 0.67)
        } else if before > here && here > after {
            (0.5, 0.5)
        } else {
            (0.0, 0.0)
        };
        accents.push(this * carried);
        carried = next;
    }
    Ok(accents)
}
