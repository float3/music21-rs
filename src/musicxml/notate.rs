//! What music21's exporter works out before it writes a score with
//! `makeNotation=True`: `GeneralObjectExporter.fromGeneralObject`, the
//! score's `makeNotation`, and each `PartExporter`'s `splitAtDurations` and
//! `fixupNotationMeasured`.

use crate::clef::Clef;
use crate::error::{Error, Result};
use crate::makenotation::{
    Kept, fill_rests, for_each_measure, insert_sorted, keeping_spanners, make_beams, make_measures,
    make_part_notation, make_tuplet_brackets, split_at_durations, tuplet_brackets_made,
};
use crate::pitch::Pitch;
use crate::stream::{Stream, StreamElement, StreamEvent, StreamKind};

/// A copy of the stream with the notation it leaves unsaid worked out, as
/// music21's exporter makes it before writing.
pub(super) fn notated(stream: &Stream) -> Result<Stream> {
    let mut stream = stream.clone();
    if stream.is_empty() {
        // Written as music21 writes an empty score, with nothing to make.
        return Ok(stream);
    }
    // `fromGeneralObject`: every gap filled with rests that are not
    // printed, out to where the stream ends.
    let end = stream.end_offset();
    keeping_spanners(&mut stream, Kept::InMeasure, &mut |stream| {
        fill_rests(stream, Some((0.0, end)), true)
    })?;

    let mut score = match stream.kind() {
        StreamKind::Score => stream,
        StreamKind::Part | StreamKind::PartStaff => from_part(stream)?,
        StreamKind::Stream => from_stream(stream)?,
        kind => {
            return Err(Error::MusicXml(format!(
                "making the notation of a lone {kind} is not supported"
            )));
        }
    };

    // `fromScore`: the score's `makeNotation`.
    let beams_failed = keeping_spanners(&mut score, Kept::Retained, &mut |score| {
        let mut failed = Vec::new();
        if score.has_part_like_streams() {
            for event in score.events_mut() {
                if let StreamElement::Stream(part) = event.element_mut() {
                    failed.push(make_part_notation(part)?.beams_failed);
                }
            }
        } else {
            failed.push(make_part_notation(score)?.beams_failed);
        }
        Ok(failed)
    })?;

    // Each `PartExporter`: lengths no one value writes cut into values...
    split_at_durations(&mut score)?;
    // ... and then `fixupNotationMeasured`.
    keeping_spanners(&mut score, Kept::Retained, &mut |score| {
        if score.has_part_like_streams() {
            let mut index = 0;
            for event in score.events_mut() {
                if let StreamElement::Stream(part) = event.element_mut()
                    && matches!(part.kind(), StreamKind::Part | StreamKind::PartStaff)
                {
                    let failed = beams_failed.get(index).copied().unwrap_or(false);
                    index += 1;
                    fix_up_measured(part, failed)?;
                }
            }
            Ok(())
        } else {
            let failed = beams_failed.first().copied().unwrap_or(false);
            fix_up_measured(score, failed)
        }
    })?;
    Ok(score)
}

/// music21's `fromPart`: a part with nothing nested in it cut into
/// measures, and the part put in a score of its own.
fn from_part(mut part: Stream) -> Result<Stream> {
    let is_flat = !part
        .events()
        .iter()
        .any(|event| matches!(event.element(), StreamElement::Stream(_)));
    if is_flat {
        part = make_measures(&part)?;
    }
    let mut score = Stream::with_kind(StreamKind::Score);
    score.set_metadata(part.metadata().cloned());
    score.insert(0.0, part);
    Ok(score)
}

/// music21's `fromStream`: a stream of no particular kind taken as the part
/// or the score it holds the music of.
fn from_stream(mut stream: Stream) -> Result<Stream> {
    if stream.has_part_like_streams() {
        stream.set_kind(StreamKind::Score);
        return Ok(stream);
    }
    let is_flat = !stream
        .events()
        .iter()
        .any(|event| matches!(event.element(), StreamElement::Stream(_)));
    stream.set_kind(StreamKind::Part);
    let has_clef = stream
        .events()
        .iter()
        .any(|event| event.offset() == 0.0 && matches!(event.element(), StreamElement::Clef(_)));
    if is_flat && !has_clef {
        let pitches: Vec<Pitch> = stream
            .events()
            .iter()
            .flat_map(|event| event.element().pitches())
            .collect();
        insert_sorted(&mut stream, 0.0, Clef::best_for(&pitches, false).into());
    }
    keeping_spanners(&mut stream, Kept::Retained, &mut |stream| {
        make_part_notation(stream).map(|_| ())
    })?;
    from_part(stream)
}

/// music21's `fixupNotationMeasured`: the first measure given the clef, key
/// and meter a part states outside its measures where it states none
/// itself; the notes beamed if beaming failed before; and every measure's
/// and voice's tuplets bracketed if none in the part are yet.
fn fix_up_measured(part: &mut Stream, beams_failed: bool) -> Result<()> {
    let loose: Vec<StreamElement> = part
        .events()
        .iter()
        .map(StreamEvent::element)
        .filter(|element| !matches!(element, StreamElement::Stream(_)))
        .cloned()
        .collect();
    let Some(first) = part
        .events_mut()
        .iter_mut()
        .find_map(|event| match event.element_mut() {
            StreamElement::Stream(measure) if measure.kind() == StreamKind::Measure => {
                Some(measure)
            }
            _ => None,
        })
    else {
        return Ok(());
    };
    let opens_with = |measure: &Stream, wanted: &dyn Fn(&StreamElement) -> bool| {
        measure
            .events()
            .iter()
            .any(|event| event.offset() == 0.0 && wanted(event.element()))
    };
    let kinds: [&dyn Fn(&StreamElement) -> bool; 3] = [
        &|element| matches!(element, StreamElement::Clef(_)),
        &|element| {
            matches!(
                element,
                StreamElement::KeySignature(_) | StreamElement::Key(_)
            )
        },
        &|element| matches!(element, StreamElement::TimeSignature(_)),
    ];
    for wanted in kinds {
        if opens_with(first, wanted) {
            continue;
        }
        if let Some(element) = loose.iter().find(|element| wanted(element)) {
            insert_sorted(first, 0.0, element.clone());
        }
    }

    if beams_failed {
        // music21 warns and writes the notes unbeamed.
        let _ = make_beams(part);
    }
    if tuplet_brackets_made(part) != Some(true) {
        for_each_measure(part, &mut |measure| {
            make_tuplet_brackets(measure);
            for event in measure.events_mut() {
                if let StreamElement::Stream(voice) = event.element_mut()
                    && voice.kind() == StreamKind::Voice
                {
                    make_tuplet_brackets(voice);
                }
            }
            Ok(())
        })?;
    }
    Ok(())
}
