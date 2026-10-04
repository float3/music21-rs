//! Working out what a score leaves unsaid: music21's `stream.makeNotation`.
//!
//! What a reader of another format needs is here: cutting a line into
//! measures, tying what runs past a barline, filling measures out with
//! rests, putting overlapping notes in voices, moving notes onto a grid,
//! beaming the notes of each measure by its meter and pointing the stems of
//! each beamed group one way.
//!
//! [`make_notation`] is the whole of it in music21's order, for a score
//! that says only what sounds: measures, accidentals, ties, tuplets
//! completed and bracketed, beams. [`split_at_durations`] cuts a length no
//! single note value writes into values that do.

use crate::articulations::Articulation;
use crate::clef::Clef;
use crate::defaults::FloatType;
use crate::duration::Duration;
use crate::error::{Error, Result};
use crate::expressions::split_expressions;
use crate::meter::{BeamedNote, TimeSignature};
use crate::notation::{BeamType, StemDirection, Tie, TieType};
use crate::note::Note;
use crate::pitch::{AccidentalDisplayOptions, Pitch};
use crate::rest::Rest;
use crate::stream::{Stream, StreamElement, StreamEvent, StreamKind};

mod score;

#[cfg(feature = "musicxml")]
pub(crate) use score::{
    Kept, accidentals_made, for_each_measure, keeping_spanners, make_measure_notation,
    make_part_notation, tuplet_brackets_made,
};
pub use score::{make_notation, make_tuplet_brackets, split_at_durations};

/// Beams the notes of every measure of a part by the meter in force, and
/// points the stems of each beamed group one way: music21's `makeBeams`.
///
/// A stream that is a measure is beamed itself. The meter in force is the
/// last one stated by a measure or standing in the part at or before the
/// measure. A measure with none, or holding more than its bar's length, is
/// left as it is; grace
/// notes are passed over. A measure is beamed voice by voice where it has
/// voices.
///
/// An error stops the beaming where it stands, leaving the measures before
/// it beamed and no stems set, as an exception in music21's does.
pub fn make_beams(stream: &mut Stream) -> Result<()> {
    if stream.kind() == StreamKind::Measure {
        let meter = meter_in(stream);
        if let Some(meter) = meter {
            beam_measure(stream, &meter)?;
        }
    } else {
        // The meters standing in the part itself, outside any measure.
        let loose: Vec<(FloatType, TimeSignature)> = stream
            .events()
            .iter()
            .filter_map(|event| match event.element() {
                StreamElement::TimeSignature(meter) => Some((event.offset(), meter.clone())),
                _ => None,
            })
            .collect();
        // The last meter a measure stated, and where that measure began.
        let mut stated: Option<(FloatType, TimeSignature)> = None;
        for event in stream.events_mut() {
            let offset = event.offset();
            let StreamElement::Stream(measure) = event.element_mut() else {
                continue;
            };
            if measure.kind() != StreamKind::Measure {
                continue;
            }
            if let Some(own) = meter_in(measure) {
                stated = Some((offset, own));
            }
            let outside = loose.iter().rev().find(|(at, _)| *at <= offset + 1e-9);
            let meter = match (&stated, outside) {
                (Some((at, _)), Some((loose_at, meter))) if loose_at > at => Some(meter),
                (Some((_, meter)), _) => Some(meter),
                (None, outside) => outside.map(|(_, meter)| meter),
            };
            if let Some(meter) = meter {
                beam_measure(measure, meter)?;
            }
        }
    }
    set_stem_direction_for_beam_groups(stream);
    Ok(())
}

/// The meter a measure opens with: music21's `timeSignature`, which is the
/// one standing at the very start and no other.
fn meter_in(measure: &Stream) -> Option<TimeSignature> {
    measure
        .events()
        .iter()
        .find_map(|event| match event.element() {
            StreamElement::TimeSignature(meter) if event.offset() == 0.0 => Some(meter.clone()),
            _ => None,
        })
}

fn beam_measure(measure: &mut Stream, meter: &TimeSignature) -> Result<()> {
    let padding = (measure.padding_left(), measure.padding_right());
    let has_voices = measure.events().iter().any(|event| {
        event
            .element()
            .as_stream()
            .is_some_and(|inner| inner.kind() == StreamKind::Voice)
    });
    if has_voices {
        for event in measure.events_mut() {
            if let StreamElement::Stream(voice) = event.element_mut()
                && voice.kind() == StreamKind::Voice
            {
                beam_line(voice, meter, padding)?;
            }
        }
        Ok(())
    } else {
        beam_line(measure, meter, padding)
    }
}

/// Beams one line of notes: a measure with no voices, or one voice. The
/// padding is the measure's, left and right.
fn beam_line(
    line: &mut Stream,
    meter: &TimeSignature,
    (padding_left, padding_right): (FloatType, FloatType),
) -> Result<()> {
    // music21's `notesAndRests`, which takes a chord symbol for a note.
    let is_general_note = |element: &StreamElement| {
        matches!(
            element,
            StreamElement::Note(_)
                | StreamElement::Chord(_)
                | StreamElement::Rest(_)
                | StreamElement::Unpitched(_)
                | StreamElement::PercussionChord(_)
                | StreamElement::ChordSymbol(_)
        )
    };
    let count = line
        .events()
        .iter()
        .filter(|event| is_general_note(event.element()))
        .count();
    if count <= 1 {
        return Ok(());
    }
    let mut places: Vec<usize> = Vec::new();
    let mut notes: Vec<BeamedNote> = Vec::new();
    for (place, event) in line.events().iter().enumerate() {
        let element = event.element();
        if !is_general_note(element) {
            continue;
        }
        let duration = element.duration();
        if duration.is_some_and(|duration| duration.is_grace()) {
            continue;
        }
        let sounds = !matches!(
            element,
            StreamElement::Rest(_) | StreamElement::ChordSymbol(_)
        );
        places.push(place);
        notes.push(BeamedNote {
            offset: event.offset(),
            quarter_length: element.quarter_length(),
            // The written value, which a tuplet leaves standing.
            duration_type: duration
                .and_then(|duration| match duration.components().as_slice() {
                    [(duration_type, _)] => Some(*duration_type),
                    _ => None,
                })
                .unwrap_or(crate::duration::DurationType::Quarter),
            sounds,
        });
    }
    let summed: FloatType = notes.iter().map(|note| note.quarter_length).sum();
    let bar = meter.bar_quarter_length();
    // A hair of latitude for lengths no float spells exactly.
    if summed > bar + 1e-9 {
        return Ok(());
    }
    let highest = notes
        .iter()
        .map(|note| note.offset + note.quarter_length)
        .fold(0.0, FloatType::max);
    // A measure padded on the left starts that far into its bar, and one
    // padded on the right at its start; a short measure saying neither is
    // taken as ending with the bar, a pickup.
    let start = if padding_left != 0.0 {
        op_frac(padding_left)
    } else if padding_right != 0.0 {
        0.0
    } else if highest < bar - 1e-9 {
        bar - highest
    } else {
        0.0
    };
    // music21 reads each note's written value to beam it, and a copy of a
    // duration whose one value has been read keeps that value as said. Every
    // step that asks whether a length may be written another way works on
    // such a copy, so the reading is recorded here.
    for place in &places {
        let element = line.events_mut()[*place].element_mut();
        let Some(mut duration) = element.duration().cloned() else {
            continue;
        };
        if duration.expression_is_inferred()
            && duration.linked()
            && duration.tuplets().is_empty()
            && duration.components().len() <= 1
        {
            duration.set_expression_is_inferred(false);
            score::set_duration(element, duration);
        }
    }
    let beams = meter.beams_for(&notes, start, None)?;
    for (place, beams) in places.into_iter().zip(beams) {
        let beams = beams.unwrap_or_default();
        match line.events_mut()[place].element_mut() {
            StreamElement::Note(note) => note.set_beams(beams),
            StreamElement::Chord(chord) => chord.set_beams(beams),
            StreamElement::Unpitched(stroke) => stroke.written_mut().set_beams(beams),
            StreamElement::PercussionChord(chord) => chord.written_mut().set_beams(beams),
            _ => {}
        }
    }
    Ok(())
}

/// One note or chord as the stem walk sees it.
struct Stemmed {
    /// The indices down through the nested streams to the element.
    path: Vec<usize>,
    offset: FloatType,
    first_beam: Option<BeamType>,
    direction: StemDirection,
    pitches: Vec<Pitch>,
}

fn gather(
    stream: &Stream,
    base: FloatType,
    path: &mut Vec<usize>,
    out: &mut Vec<Stemmed>,
    clefs: &mut Vec<(FloatType, Clef)>,
) {
    for (index, event) in stream.events().iter().enumerate() {
        path.push(index);
        let offset = base + event.offset();
        match event.element() {
            StreamElement::Stream(inner) => gather(inner, offset, path, out, clefs),
            StreamElement::Clef(clef) => clefs.push((offset, clef.clone())),
            StreamElement::Note(note) => out.push(Stemmed {
                path: path.clone(),
                offset,
                first_beam: note.beams().by_number(1).and_then(|beam| beam.beam_type()),
                direction: note.stem_direction(),
                pitches: vec![note.pitch().clone()],
            }),
            StreamElement::Chord(chord) => out.push(Stemmed {
                path: path.clone(),
                offset,
                first_beam: chord.beams().by_number(1).and_then(|beam| beam.beam_type()),
                direction: chord.stem_direction(),
                pitches: chord.pitches(),
            }),
            _ => {}
        }
        path.pop();
    }
}

fn set_direction(stream: &mut Stream, path: &[usize], direction: StemDirection) {
    let Some((first, rest)) = path.split_first() else {
        return;
    };
    let Some(event) = stream.events_mut().get_mut(*first) else {
        return;
    };
    match event.element_mut() {
        StreamElement::Stream(inner) => set_direction(inner, rest, direction),
        StreamElement::Note(note) => note.set_stem_direction(direction),
        StreamElement::Chord(chord) => chord.set_stem_direction(direction),
        _ => {}
    }
}

/// Points the stems of each beamed group the way its first and last pitches
/// lie about the middle line of the clef in force: music21's
/// `setStemDirectionForBeamGroups`.
///
/// A group runs from a note whose first beam starts to the one whose first
/// beam stops. Stems a score already points all one way are left alone, and
/// a group with no clef before it is not touched.
pub fn set_stem_direction_for_beam_groups(stream: &mut Stream) {
    let mut notes = Vec::new();
    let mut clefs = Vec::new();
    gather(stream, 0.0, &mut Vec::new(), &mut notes, &mut clefs);

    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut inside = false;
    for (index, note) in notes.iter().enumerate() {
        if note.first_beam == Some(BeamType::Start) {
            inside = true;
        }
        if inside {
            current.push(index);
        }
        if note.first_beam == Some(BeamType::Stop) {
            groups.push(std::mem::take(&mut current));
            inside = false;
        }
    }
    if !current.is_empty() {
        groups.push(current);
    }

    let settable = |direction: StemDirection| {
        matches!(
            direction,
            StemDirection::Up | StemDirection::Down | StemDirection::Unspecified
        )
    };
    for group in groups {
        let Some(first) = group.first().map(|index| &notes[*index]) else {
            continue;
        };
        let mut said: Vec<StemDirection> = Vec::new();
        for index in &group {
            let direction = notes[*index].direction;
            if settable(direction) && !said.contains(&direction) {
                said.push(direction);
            }
        }
        let consistent = !said.contains(&StemDirection::Unspecified) && said.len() < 2;
        let Some((_, clef)) = clefs
            .iter()
            .rev()
            .find(|(offset, _)| *offset <= first.offset)
        else {
            continue;
        };
        let pitches: Vec<Pitch> = group
            .iter()
            .flat_map(|index| notes[*index].pitches.iter().cloned())
            .collect();
        let Ok(direction) = clef.stem_direction_for_pitches(&pitches, true, false) else {
            continue;
        };
        for index in &group {
            let note = &notes[*index];
            if !settable(note.direction) {
                continue;
            }
            if note.direction != StemDirection::Unspecified && consistent {
                continue;
            }
            set_direction(stream, &note.path, direction);
        }
    }
}

/// One element waiting for its measure.
struct Placed {
    offset: FloatType,
    end: FloatType,
    voice: Option<usize>,
    element: StreamElement,
}

/// A measure while it is being filled.
struct Bar {
    start: FloatType,
    end: FloatType,
    own: Vec<StreamEvent>,
    voices: Vec<Vec<StreamEvent>>,
}

/// Snaps an offset as music21's `opFrac` does, to the nearest fraction with
/// a denominator up to 65535, so that three triplets end on the beat.
pub(crate) fn op_frac(value: FloatType) -> FloatType {
    match crate::duration::limited_fraction(value, 65535) {
        Some((numerator, denominator)) if denominator != 0 => {
            let snapped = numerator as FloatType / denominator as FloatType;
            if (snapped - value).abs() < 1e-9 {
                snapped
            } else {
                value
            }
        }
        _ => value,
    }
}

/// Sorts events as music21 sorts a stream: by offset, then by class, grace
/// notes first, then in the order given.
pub(crate) fn sorted_events(mut events: Vec<StreamEvent>) -> Vec<StreamEvent> {
    events.sort_by(event_order);
    events
}

/// Puts an element into a stream where music21's sort puts it: after
/// everything that sorts no later.
#[cfg(feature = "musicxml")]
pub(crate) fn insert_sorted(stream: &mut Stream, offset: FloatType, element: StreamElement) {
    let event = StreamEvent::new(offset, element);
    let mut events = stream.events().to_vec();
    let place = events
        .iter()
        .position(|held| event_order(held, &event).is_gt())
        .unwrap_or(events.len());
    events.insert(place, event);
    let spanners = stream.spanners().to_vec();
    *stream = stream.with_events(events);
    for spanner in spanners {
        stream.add_spanner(spanner);
    }
}

/// The order music21 sorts two things standing in one stream in, leaving
/// equals in the order they were put there.
pub(crate) fn event_order(left: &StreamEvent, right: &StreamEvent) -> std::cmp::Ordering {
    let grace = |event: &StreamEvent| {
        event
            .element()
            .duration()
            .is_some_and(crate::duration::Duration::is_grace)
    };
    left.offset()
        .total_cmp(&right.offset())
        .then(
            left.element()
                .class_sort_order()
                .cmp(&right.element().class_sort_order()),
        )
        .then(grace(right).cmp(&grace(left)))
}

fn placed(offset: FloatType, voice: Option<usize>, element: &StreamElement) -> Placed {
    Placed {
        offset,
        end: op_frac(offset + element.quarter_length()),
        voice,
        element: element.clone(),
    }
}

/// Cuts a stream into measures by the meters it holds: music21's
/// `makeMeasures`.
///
/// A stream holding parts has each of them cut. Otherwise the stream is
/// flattened, unless it holds voices, which are kept as the voices of each
/// measure. A stream with no meter at its start is in `4/4` until it says
/// otherwise. The first measure takes the clef standing at the start, or the
/// one that fits the notes best; each measure where the meter changes states
/// it; the last ends with a final barline. A note is put in the measure it
/// starts in and is not cut where it runs past the barline.
///
/// Spanners are not carried across, as the places they name move.
///
/// ```
/// use music21_rs::makenotation::make_measures;
/// use music21_rs::{Duration, Note, Stream, TimeSignature};
///
/// let mut line = Stream::new();
/// line.insert(0.0, TimeSignature::new(3, 4)?);
/// for (place, name) in ["C4", "D4", "E4", "F4"].iter().enumerate() {
///     let note = Note::from_name(name)?.with_duration(Duration::quarter());
///     line.insert(place as f64, note);
/// }
/// let measured = make_measures(&line)?;
/// assert_eq!(measured.measures().len(), 2);
/// assert_eq!(measured.measures()[1].number(), 2);
/// # Ok::<(), music21_rs::Error>(())
/// ```
pub fn make_measures(stream: &Stream) -> Result<Stream> {
    make_measures_by(stream, &[])
}

/// [`make_measures`] by meters given rather than found: music21's
/// `makeMeasures` with a `meterStream`. Each meter is given with the offset
/// it takes over at; with none given the stream's own are used.
pub fn make_measures_by(stream: &Stream, given: &[(FloatType, TimeSignature)]) -> Result<Stream> {
    let is_part_like = |element: &StreamElement| {
        element
            .as_stream()
            .is_some_and(|inner| !matches!(inner.kind(), StreamKind::Measure | StreamKind::Voice))
    };
    if stream
        .events()
        .iter()
        .any(|event| is_part_like(event.element()))
    {
        let mut made = stream.clone();
        for event in made.events_mut() {
            if let StreamElement::Stream(inner) = event.element_mut() {
                **inner = make_measures_by(inner, given)?;
            }
        }
        return Ok(made);
    }

    // Everything the stream holds, with where it starts and ends and which
    // voice it is in.
    let voices = stream.voices();
    let voice_count = voices.len();
    let mut waiting: Vec<Placed> = Vec::new();
    if voice_count > 0 {
        for event in stream.events() {
            let is_voice = event
                .element()
                .as_stream()
                .is_some_and(|inner| inner.kind() == StreamKind::Voice);
            if !is_voice {
                waiting.push(placed(event.offset(), None, event.element()));
            }
        }
        for (index, voice) in voices.iter().enumerate() {
            for event in sorted_events(voice.flatten().events().to_vec()) {
                waiting.push(placed(event.offset(), Some(index), event.element()));
            }
        }
    } else {
        for event in sorted_events(stream.flatten().events().to_vec()) {
            waiting.push(placed(event.offset(), None, event.element()));
        }
    }

    // The meters, with 4/4 at the start where the stream says nothing there.
    let mut meters: Vec<(FloatType, TimeSignature)> = waiting
        .iter()
        .filter_map(|item| match &item.element {
            StreamElement::TimeSignature(meter) => Some((item.offset, meter.clone())),
            _ => None,
        })
        .collect();
    meters.sort_by(|left, right| left.0.total_cmp(&right.0));
    if meters.first().is_none_or(|(offset, _)| *offset > 0.0) {
        meters.insert(0, (0.0, TimeSignature::new(4, 4)?));
    }
    if !given.is_empty() {
        meters = given.to_vec();
        meters.sort_by(|left, right| left.0.total_cmp(&right.0));
    }

    // The clef at the start, which the first measure takes and which is then
    // not placed a second time.
    let own_clef = waiting.iter().position(|item| {
        item.voice.is_none() && item.offset == 0.0 && matches!(item.element, StreamElement::Clef(_))
    });
    let clef = match own_clef.map(|index| &waiting[index].element) {
        Some(StreamElement::Clef(clef)) => clef.clone(),
        _ => {
            let mut pitches: Vec<Pitch> = Vec::new();
            for item in &waiting {
                match &item.element {
                    StreamElement::Note(note) => pitches.push(note.pitch().clone()),
                    StreamElement::Chord(chord) => pitches.extend(chord.pitches()),
                    StreamElement::ChordSymbol(symbol) => {
                        pitches.extend(symbol.pitches().unwrap_or_default());
                    }
                    _ => {}
                }
            }
            Clef::best_for(&pitches, false)
        }
    };
    let key = waiting.iter().find_map(|item| match &item.element {
        element @ (StreamElement::KeySignature(_) | StreamElement::Key(_))
            if item.voice.is_none() && item.offset == 0.0 =>
        {
            Some(element.clone())
        }
        _ => None,
    });

    let end = waiting
        .iter()
        .map(|item| item.end)
        .fold(0.0, FloatType::max);

    // The measures, empty: where each starts and ends, and what it opens
    // with.
    let mut bars: Vec<Bar> = Vec::new();
    let mut offset = 0.0;
    let mut last_meter: Option<usize> = None;
    loop {
        let here = meters
            .iter()
            .rposition(|(at, _)| *at <= offset)
            .unwrap_or(0);
        let mut own = Vec::new();
        if last_meter != Some(here) {
            last_meter = Some(here);
            own.push(StreamEvent::new(0.0, meters[here].1.clone()));
        }
        if bars.is_empty() {
            own.push(StreamEvent::new(0.0, clef.clone()));
            // music21 copies the key in beside the one placed below.
            if voice_count > 0
                && let Some(key) = &key
            {
                own.push(StreamEvent::new(0.0, key.clone()));
            }
        }
        let length = meters[here].1.bar_quarter_length();
        if length == 0.0 {
            return Err(crate::error::Error::Meter(format!(
                "time signature {} has no duration",
                meters[here].1.ratio_string()
            )));
        }
        let next = op_frac(offset + length);
        bars.push(Bar {
            start: offset,
            end: next,
            own,
            voices: vec![Vec::new(); voice_count],
        });
        offset = next;
        if offset >= end {
            break;
        }
    }

    let mut loose: Vec<StreamEvent> = Vec::new();
    for (index, item) in waiting.into_iter().enumerate() {
        let Some(bar) = bars
            .iter_mut()
            .find(|bar| bar.start <= item.offset && item.offset < bar.end)
        else {
            // Something with no length standing at the very end is kept
            // after the measures.
            if item.offset == item.end && item.offset == end {
                loose.push(StreamEvent::new(item.offset, item.element));
                continue;
            }
            return Err(crate::error::Error::Meter(format!(
                "cannot place an element with start/end {}/{} within any measures",
                item.offset, item.end
            )));
        };
        let within = op_frac(item.offset - bar.start);
        if own_clef == Some(index) {
            continue;
        }
        if within == 0.0 && matches!(item.element, StreamElement::TimeSignature(_)) {
            continue;
        }
        match item.voice {
            None => bar.own.push(StreamEvent::new(within, item.element)),
            Some(voice) => bar.voices[voice].push(StreamEvent::new(within, item.element)),
        }
    }

    let count = bars.len();
    let mut events: Vec<StreamEvent> = Vec::new();
    for (index, bar) in bars.into_iter().enumerate() {
        let mut own = bar.own;
        for (id, voice) in bar.voices.into_iter().enumerate() {
            let mut voice = Stream::new().with_events(sorted_events(voice));
            voice.set_kind(StreamKind::Voice);
            voice.set_id(Some(id.to_string()));
            own.push(StreamEvent::new(0.0, voice));
        }
        let mut measure = Stream::new().with_events(sorted_events(own));
        measure.set_kind(StreamKind::Measure);
        measure.set_number(index as crate::defaults::IntegerType + 1);
        if index + 1 == count {
            measure.set_right_barline(Some(crate::bar::Barline::new(
                crate::bar::BarlineType::Final,
            )));
        }
        events.push(StreamEvent::new(bar.start, measure));
    }
    events.extend(loose);
    Ok(stream.with_events(events))
}

/// Whether an element is a note, a chord, a rest or a stroke: music21's
/// `GeneralNote`, which a chord symbol is one of.
fn is_general_note(element: &StreamElement) -> bool {
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

/// Whether an element sounds: music21's `NotRest`.
pub(crate) fn is_not_rest(element: &StreamElement) -> bool {
    is_general_note(element) && !matches!(element, StreamElement::Rest(_))
}

fn is_voice(element: &StreamElement) -> bool {
    element
        .as_stream()
        .is_some_and(|inner| inner.kind() == StreamKind::Voice)
}

/// Gives an element a new length, keeping whatever else it says.
fn set_quarter_length(element: &mut StreamElement, quarter_length: FloatType) -> Result<()> {
    let duration = Duration::new(quarter_length)?;
    match element {
        StreamElement::Note(note) => note.set_duration(duration),
        StreamElement::Chord(chord) => chord.set_duration(duration),
        StreamElement::Rest(rest) => rest.set_duration(duration),
        StreamElement::Unpitched(stroke) => stroke.written_mut().set_duration(duration),
        StreamElement::PercussionChord(chord) => chord.written_mut().set_duration(duration),
        _ => {}
    }
    Ok(())
}

/// The tie a split leaves on the first piece, and the type of the one the
/// second piece takes.
fn split_tie(note: &mut Note) -> TieType {
    match note.tie().map(Tie::tie_type) {
        Some(TieType::Start) => TieType::Continue,
        Some(TieType::Stop) => {
            let mut tie = note
                .tie()
                .cloned()
                .unwrap_or_else(|| Tie::new(TieType::Stop));
            tie.set_tie_type(TieType::Continue);
            note.set_tie(Some(tie));
            TieType::Stop
        }
        Some(TieType::Continue) => TieType::Continue,
        _ => {
            note.set_tie(Some(Tie::new(TieType::Start)));
            TieType::Stop
        }
    }
}

/// The second piece of a split note does not show its accidental again.
fn hide_tied_accidental(first: &Note, second: &mut Note) {
    let Some(accidental) = first.pitch().written_accidental() else {
        return;
    };
    if accidental.display_type() == "even-tied" {
        return;
    }
    let mut pitch = second.pitch().clone();
    if let Some(accidental) = pitch.written_accidental_mut() {
        accidental.set_display_status(Some(false));
        second.set_pitch(pitch);
    }
}

/// An element cut in two at a length into it, the pieces tied: music21's
/// `splitAtQuarterLength`.
///
/// The second piece carries no lyrics, and each mark goes to the piece it is
/// attached to. A rest and a chord of strokes are cut without ties.
pub(crate) fn split_element(
    element: &StreamElement,
    at: FloatType,
) -> Result<(StreamElement, StreamElement)> {
    let whole = element.quarter_length();
    let first_length = Duration::new(at)?;
    let second_length = Duration::new(op_frac(whole - at))?;
    // Marks stay with the piece they are attached to.
    let share = |marks: &[Articulation]| -> (Vec<Articulation>, Vec<Articulation>) {
        let mut first = Vec::new();
        let mut second = Vec::new();
        for mark in marks {
            match mark.tie_attach() {
                "first" => first.push(mark.clone()),
                "last" => second.push(mark.clone()),
                _ => {
                    first.push(mark.clone());
                    second.push(mark.clone());
                }
            }
        }
        (first, second)
    };
    let split_note = |note: &Note| -> (Note, Note) {
        let mut first = note.clone();
        let mut second = note.clone();
        second.lyrics_mut().clear();
        let (kept, moved) = share(note.articulations());
        *first.articulations_mut() = kept;
        *second.articulations_mut() = moved;
        let (kept, moved) = split_expressions(note.expressions());
        *first.expressions_mut() = kept;
        *second.expressions_mut() = moved;
        first.set_duration(first_length.clone());
        second.set_duration(second_length.clone());
        let closing = split_tie(&mut first);
        second.set_tie(Some(Tie::new(closing)));
        hide_tied_accidental(&first, &mut second);
        (first, second)
    };
    match element {
        StreamElement::Note(note) => {
            let (first, second) = split_note(note);
            Ok((first.into(), second.into()))
        }
        StreamElement::Unpitched(stroke) => {
            let (first_note, second_note) = split_note(stroke.written());
            let mut first = stroke.clone();
            let mut second = stroke.clone();
            *first.written_mut() = first_note;
            *second.written_mut() = second_note;
            Ok((first.into(), second.into()))
        }
        StreamElement::Chord(chord) => {
            let mut first = chord.clone();
            let mut second = chord.clone();
            let (kept, moved) = share(chord.articulations());
            *first.articulations_mut() = kept;
            *second.articulations_mut() = moved;
            let (kept, moved) = split_expressions(chord.expressions());
            *first.expressions_mut() = kept;
            *second.expressions_mut() = moved;
            first.set_duration(first_length);
            second.set_duration(second_length);
            // A chord's lyrics are kept on its first note.
            if let Some(singer) = second.notes_mut().first_mut() {
                singer.lyrics_mut().clear();
            }
            for (one, other) in first.notes_mut().iter_mut().zip(second.notes_mut()) {
                let closing = split_tie(one);
                other.set_tie(Some(Tie::new(closing)));
                hide_tied_accidental(one, other);
            }
            Ok((first.into(), second.into()))
        }
        StreamElement::PercussionChord(chord) => {
            let mut first = chord.clone();
            let mut second = chord.clone();
            let (kept, moved) = split_expressions(chord.written().expressions());
            *first.written_mut().expressions_mut() = kept;
            *second.written_mut().expressions_mut() = moved;
            first.written_mut().set_duration(first_length);
            second.written_mut().set_duration(second_length);
            Ok((first.into(), second.into()))
        }
        StreamElement::Rest(rest) => {
            let mut first = rest.clone();
            let mut second = rest.clone();
            second.lyrics_mut().clear();
            let (kept, moved) = split_expressions(rest.expressions());
            *first.expressions_mut() = kept;
            *second.expressions_mut() = moved;
            first.set_duration(first_length);
            second.set_duration(second_length);
            Ok((first.into(), second.into()))
        }
        _ => Err(Error::Duration(
            "only a note, a chord or a rest is cut at a bar".to_string(),
        )),
    }
}

/// music21's `nearestMultiple`: the multiple of a unit nearest a number, how
/// far off it is, and which way.
fn nearest_multiple(number: FloatType, unit: FloatType) -> (FloatType, FloatType, FloatType) {
    let round7 = |value: FloatType| (value * 1e7).round_ties_even() / 1e7;
    let multiple = (number / unit).floor();
    let low = unit * multiple;
    let high = unit * (multiple + 1.0);
    if low <= number && number <= low + unit / 2.0 {
        (low, round7(number - low), round7(number - low))
    } else {
        (high, round7(high - number), round7(number - high))
    }
}

/// The best place on the grids for a length or an offset: music21's
/// `bestMatch`. The answer is what is left of the gap, the error, the unit
/// and the match, in the order they are compared.
fn best_match(
    target: FloatType,
    divisors: &[u32],
    zero_allowed: bool,
    gap: FloatType,
) -> (FloatType, FloatType, FloatType, FloatType) {
    let mut best: Option<(FloatType, FloatType, FloatType, FloatType)> = None;
    for divisor in divisors {
        let unit = 1.0 / FloatType::from(*divisor);
        let (mut found, mut error, _) = nearest_multiple(target, unit);
        if !zero_allowed && found == 0.0 {
            found = unit;
            error = ((target - found) * 1e7).round_ties_even().abs() / 1e7;
        }
        let remaining = if gap % unit == 0.0 {
            0.0
        } else {
            (gap - found).max(0.0)
        };
        let candidate = (remaining, error, unit, found);
        let better = best.is_none_or(|held| {
            candidate
                .0
                .total_cmp(&held.0)
                .then(candidate.1.total_cmp(&held.1))
                .then(candidate.2.total_cmp(&held.2))
                .then(candidate.3.total_cmp(&held.3))
                .is_lt()
        });
        if better {
            best = Some(candidate);
        }
    }
    best.unwrap_or((0.0, 0.0, 1.0, target))
}

/// The divisions of a quarter music21 quantizes to unless told otherwise:
/// sixteenths and triplet eighths.
pub const QUANTIZATION_DIVISORS: [u32; 2] = [4, 3];

/// Moves every offset and length in a stream onto the nearest place a
/// quarter divides into: music21's `quantize`, for the stream's own elements.
///
/// `divisors` are the ways a quarter may be divided, `[4, 3]` for sixteenths
/// and triplets. Where two grids are as near, the one that leaves no gap
/// before the next element wins, then the nearer, then the finer. A note is
/// never quantized to no length at all, though a grace note stays one.
pub fn quantize(stream: &mut Stream, divisors: &[u32]) -> Result<()> {
    let divisors: &[u32] = if divisors.is_empty() {
        &QUANTIZATION_DIVISORS
    } else {
        divisors
    };
    let mut events = stream.events().to_vec();
    for index in 0..events.len() {
        let original = events[index].offset();
        let sign = if original < 0.0 { -1.0 } else { 1.0 };
        let offset = op_frac(best_match(original.abs(), divisors, true, 0.0).3 * sign);
        let mut element = events[index].element().clone();

        let is_grace = element.duration().is_some_and(Duration::is_grace);
        let length = element.quarter_length().max(0.0);
        let zero_allowed = !is_not_rest(&element) || is_grace;
        // The next element that lands later than this one says how much
        // room there is to fill.
        let ahead = events[index + 1..]
            .iter()
            .map(|event| best_match(event.offset(), divisors, true, 0.0).3)
            .find(|landing| *landing > offset);
        let gap = ahead.map_or(0.0, |landing| op_frac(landing - offset));
        let matched = best_match(length, divisors, zero_allowed, gap).3;
        let is_stream = matches!(element, StreamElement::Stream(_));
        if !is_grace && !is_stream && !(matched == 0.0 && matches!(element, StreamElement::Rest(_)))
        {
            set_quarter_length(&mut element, op_frac(matched))?;
        }
        let dropped = matched == 0.0 && matches!(element, StreamElement::Rest(_));
        events[index] = StreamEvent::new(offset, element);
        if dropped {
            // Marked for removal below by an offset no element can have.
            events[index] = StreamEvent::new(FloatType::NAN, events[index].element().clone());
        }
    }
    events.retain(|event| !event.offset().is_nan());
    *stream = stream.with_events(sorted_events(events));
    Ok(())
}

/// music21's `_findLayering` and `_consolidateLayering`, for how many of a
/// line's notes sound at once at the most.
fn most_at_once(spans: &[(FloatType, FloatType)]) -> usize {
    let overlaps = |left: (FloatType, FloatType), right: (FloatType, FloatType)| {
        let (first, second) = if right.0 < left.0 || (right.0 == left.0 && right.1 < left.1) {
            (right, left)
        } else {
            (left, right)
        };
        second.0 < first.1
    };
    let mut layering: Vec<Vec<usize>> = vec![Vec::new(); spans.len()];
    for source in 0..spans.len() {
        for target in source + 1..spans.len() {
            if overlaps(spans[source], spans[target]) {
                layering[source].push(target);
                layering[target].push(source);
            } else {
                break;
            }
        }
    }
    for list in &mut layering {
        list.sort_unstable();
    }
    // Groups by the offset they were first gathered under.
    let mut groups: Vec<(FloatType, Vec<usize>)> = Vec::new();
    for (source, others) in layering.iter().enumerate() {
        if others.is_empty() {
            continue;
        }
        let mut destination: Option<FloatType> = None;
        for other in others {
            let mut store = true;
            if let Some((key, _)) = groups.iter().find(|(_, held)| held.contains(other)) {
                store = false;
                destination = Some(*key);
            }
            let key = *destination.get_or_insert(spans[source].0);
            if store {
                match groups.iter_mut().find(|(held, _)| *held == key) {
                    Some((_, held)) => held.push(*other),
                    None => groups.push((key, vec![*other])),
                }
            }
        }
        if !groups.iter().any(|(_, held)| held.contains(&source)) {
            let key = destination.unwrap_or(spans[source].0);
            match groups.iter_mut().find(|(held, _)| *held == key) {
                Some((_, held)) => held.push(source),
                None => groups.push((key, vec![source])),
            }
        }
    }
    groups
        .iter()
        .map(|(_, held)| held.len())
        .max()
        .unwrap_or(1)
        .max(1)
}

/// Puts a measure's notes into voices where they overlap: music21's
/// `makeVoices` with `fillGaps=False`.
///
/// There are as many voices as notes sounding at once at the most, and each
/// note goes to the first voice that has finished by the time it starts. A
/// note no voice is free for is lost, as it is there. A measure whose notes
/// never overlap is left alone.
pub fn make_voices(measure: &mut Stream) {
    let events = measure.events().to_vec();
    let spans: Vec<(FloatType, FloatType)> = events
        .iter()
        .filter(|event| is_not_rest(event.element()))
        .map(|event| (event.offset(), op_frac(event.end_offset())))
        .collect();
    let count = most_at_once(&spans);
    if count == 1 {
        return;
    }
    let mut voices: Vec<(FloatType, Vec<StreamEvent>)> = vec![(0.0, Vec::new()); count];
    let mut kept: Vec<StreamEvent> = Vec::new();
    for event in events {
        if !is_not_rest(event.element()) {
            // Rests go, as `removeByClass('Rest')` takes them.
            if !matches!(event.element(), StreamElement::Rest(_)) {
                kept.push(event);
            }
            continue;
        }
        let offset = event.offset();
        let end = op_frac(event.end_offset());
        if let Some((highest, held)) = voices.iter_mut().find(|(highest, _)| *highest <= offset) {
            *highest = highest.max(end);
            held.push(event);
        }
    }
    for (_, held) in voices {
        if held.is_empty() {
            continue;
        }
        let mut voice = Stream::new().with_events(held);
        voice.set_kind(StreamKind::Voice);
        kept.push(StreamEvent::new(0.0, voice));
    }
    *measure = measure.with_events(sorted_events(kept));
}

/// Every meter a part states, at its offset from the part's start, with
/// `4/4` at the start where the part says nothing there.
fn meters_of(part: &Stream) -> Result<Vec<(FloatType, TimeSignature)>> {
    let mut meters: Vec<(FloatType, TimeSignature)> = part
        .recurse()
        .into_iter()
        .filter_map(|(offset, element)| match element {
            StreamElement::TimeSignature(meter) => Some((offset, meter.clone())),
            _ => None,
        })
        .collect();
    meters.sort_by(|left, right| left.0.total_cmp(&right.0));
    // A meter standing again is the object the earlier measure holds, and
    // music21's list of meters keeps one place for one object: the last it
    // stands at. Each such meter and the one it restates are moved there.
    let mut group: Vec<usize> = (0..meters.len()).collect();
    for index in 0..meters.len() {
        if !meters[index].1.is_restated() {
            continue;
        }
        let ratio = meters[index].1.ratio_string();
        if let Some(earlier) = (0..index)
            .rev()
            .find(|earlier| meters[*earlier].1.ratio_string() == ratio)
        {
            group[index] = group[earlier];
        }
    }
    for index in (0..meters.len()).rev() {
        let last = (0..meters.len())
            .rev()
            .find(|other| group[*other] == group[index])
            .unwrap_or(index);
        meters[index].0 = meters[last].0;
    }
    meters.sort_by(|left, right| left.0.total_cmp(&right.0));
    if meters.first().is_none_or(|(offset, _)| *offset > 0.0) {
        meters.insert(0, (0.0, TimeSignature::new(4, 4)?));
    }
    Ok(meters)
}

/// Takes the voices with nothing in them out of a measure, and where one
/// voice is left puts its notes in the measure itself: music21's
/// `flattenUnnecessaryVoices`.
fn flatten_unnecessary_voices(measure: &mut Stream) {
    let events = measure.events().to_vec();
    if !events.iter().any(|event| is_voice(event.element())) {
        return;
    }
    let full = events
        .iter()
        .filter(|event| {
            event
                .element()
                .as_stream()
                .is_some_and(|inner| inner.kind() == StreamKind::Voice && !inner.is_empty())
        })
        .count();
    let mut kept: Vec<StreamEvent> = Vec::new();
    let mut freed: Vec<StreamEvent> = Vec::new();
    for event in events {
        match event.element() {
            StreamElement::Stream(inner) if inner.kind() == StreamKind::Voice => {
                if inner.is_empty() {
                    continue;
                }
                if full == 1 {
                    let shift = event.offset();
                    freed.extend(inner.events().iter().map(|held| {
                        StreamEvent::new(shift + held.offset(), held.element().clone())
                    }));
                } else {
                    kept.push(event);
                }
            }
            _ => kept.push(event),
        }
    }
    kept.extend(freed);
    *measure = measure.with_events(sorted_events(kept));
}

/// Cuts every note that runs past its barline and ties the pieces, the
/// second going to the start of the next measure: music21's `makeTies`.
///
/// A stream holding parts has each of them done. The stream must already be
/// in measures. A piece cut from a voice goes to the voice of the same name
/// in the next measure, or to the measure itself where there is none; one
/// cut from a measure with no voices goes to the next measure's first voice
/// where it has any.
pub fn make_ties(stream: &mut Stream) -> Result<()> {
    make_ties_of(stream, is_general_note)
}

fn make_ties_of(stream: &mut Stream, wanted: fn(&StreamElement) -> bool) -> Result<()> {
    let part_like = |element: &StreamElement| {
        element
            .as_stream()
            .is_some_and(|inner| !matches!(inner.kind(), StreamKind::Measure | StreamKind::Voice))
    };
    if stream
        .events()
        .iter()
        .any(|event| part_like(event.element()))
    {
        let mut events = stream.events().to_vec();
        for event in &mut events {
            if let StreamElement::Stream(inner) = event.element_mut() {
                make_ties_of(inner, wanted)?;
            }
        }
        let spanners = stream.spanners().to_vec();
        *stream = stream.with_events(events);
        for spanner in spanners {
            stream.add_spanner(spanner);
        }
        return Ok(());
    }
    let meters = meters_of(stream)?;
    let mut others: Vec<StreamEvent> = Vec::new();
    let mut measures: Vec<(FloatType, Stream)> = Vec::new();
    for event in stream.events() {
        match event.element() {
            StreamElement::Stream(inner) if inner.kind() == StreamKind::Measure => {
                measures.push((event.offset(), (**inner).clone()));
            }
            _ => others.push(event.clone()),
        }
    }
    if measures.is_empty() {
        return Err(Error::Meter(
            "cannot process a stream without measures".to_string(),
        ));
    }

    let mut index = 0;
    while index < measures.len() {
        let start = measures[index].0;
        let meter = meters
            .iter()
            .rfind(|(at, _)| *at <= start)
            .map_or(&meters[0].1, |(_, meter)| meter);
        let bar = meter.bar_quarter_length();
        let has_voices = measures[index]
            .1
            .events()
            .iter()
            .any(|event| is_voice(event.element()));

        // What is cut off, with the name of the voice it was cut from.
        let mut remains: Vec<(Option<String>, StreamElement)> = Vec::new();
        let mut cut = |line: &mut Stream, id: Option<String>| -> Result<()> {
            let mut events = line.events().to_vec();
            for event in &mut events {
                if !wanted(event.element()) {
                    continue;
                }
                let offset = event.offset();
                let end = op_frac(event.end_offset());
                if end - bar <= 1e-9 || offset >= bar - 1e-9 {
                    continue;
                }
                let (first, remain) = split_element(event.element(), op_frac(bar - offset))?;
                *event = StreamEvent::new(offset, first);
                remains.push((id.clone(), remain));
            }
            *line = line.with_events(events);
            Ok(())
        };
        {
            let measure = &mut measures[index].1;
            if has_voices {
                let mut events = measure.events().to_vec();
                for event in &mut events {
                    if let StreamElement::Stream(voice) = event.element_mut()
                        && voice.kind() == StreamKind::Voice
                    {
                        let id = voice.id().map(str::to_string);
                        cut(voice, id)?;
                    }
                }
                *measure = measure.with_events(events);
            } else {
                cut(measure, None)?;
            }
        }
        if remains.is_empty() {
            index += 1;
            continue;
        }
        if index + 1 == measures.len() {
            let mut next = Stream::with_kind(StreamKind::Measure);
            next.set_number(measures[index].1.number() + 1);
            measures.push((op_frac(start + bar), next));
        }
        let next = &mut measures[index + 1].1;
        let next_had_voices = next.events().iter().any(|event| is_voice(event.element()));
        for (id, remain) in remains {
            let mut events = next.events().to_vec();
            let voice_at = |events: &[StreamEvent], wanted: Option<&str>| -> Option<usize> {
                events.iter().position(|event| {
                    event.element().as_stream().is_some_and(|inner| {
                        inner.kind() == StreamKind::Voice
                            && wanted.is_none_or(|wanted| inner.id() == Some(wanted))
                    })
                })
            };
            let destination = if next_had_voices {
                if has_voices {
                    id.as_deref().and_then(|id| voice_at(&events, Some(id)))
                } else {
                    voice_at(&events, None)
                }
            } else if has_voices {
                // The notes of the next measure are gathered into a voice of
                // their own, and the piece joins its first voice.
                let (moved, kept): (Vec<StreamEvent>, Vec<StreamEvent>) = events
                    .into_iter()
                    .partition(|event| is_general_note(event.element()));
                events = kept;
                let mut voice = Stream::new().with_events(moved);
                voice.set_kind(StreamKind::Voice);
                events.push(StreamEvent::new(0.0, voice));
                events = sorted_events(events);
                voice_at(&events, None)
            } else {
                None
            };
            match destination {
                Some(place) => {
                    if let StreamElement::Stream(voice) = events[place].element_mut() {
                        let mut held = voice.events().to_vec();
                        held.push(StreamEvent::new(0.0, remain));
                        **voice = voice.with_events(sorted_events(held));
                    }
                }
                None => {
                    events.push(StreamEvent::new(0.0, remain));
                    events = sorted_events(events);
                }
            }
            *next = next.with_events(events);
        }
        index += 1;
    }

    for (offset, mut measure) in measures {
        flatten_unnecessary_voices(&mut measure);
        others.push(StreamEvent::new(offset, measure));
    }
    *stream = stream.with_events(sorted_events(others));
    Ok(())
}

/// Fills one line out with rests from `low` to `high`: before its first
/// element, after its last, and in every gap between. `hidden` rests are
/// left unprinted.
pub(crate) fn fill_line(
    line: &mut Stream,
    low: FloatType,
    high: FloatType,
    hidden: bool,
) -> Result<()> {
    let mut events = line.events().to_vec();
    let lowest = events
        .iter()
        .map(StreamEvent::offset)
        .fold(FloatType::INFINITY, FloatType::min);
    let lowest = if lowest.is_finite() { lowest } else { 0.0 };
    let highest = op_frac(line.end_offset());
    let rest = |at: FloatType, length: FloatType| -> Result<StreamEvent> {
        let mut rest = Rest::new(Duration::new(op_frac(length))?);
        rest.set_hidden(hidden);
        Ok(StreamEvent::new(at, rest))
    };
    if lowest - low > 1e-9 {
        events.push(rest(low, lowest - low)?);
    }
    if high - highest > 1e-9 {
        events.push(rest(highest, high - highest)?);
    }
    events = sorted_events(events);
    let mut gaps: Vec<StreamEvent> = Vec::new();
    let mut reached = 0.0;
    for event in &events {
        if event.offset() - reached > 1e-9 {
            gaps.push(rest(reached, event.offset() - reached)?);
        }
        reached = op_frac(FloatType::max(reached, event.end_offset()));
    }
    events.extend(gaps);
    *line = line.with_events(sorted_events(events));
    Ok(())
}

/// Fills every measure of a part with rests up to its bar's length, its
/// voices too: music21's `makeRests` with `fillGaps=True` and
/// `timeRangeFromBarDuration=True`.
///
/// A stream holding parts has each of them filled. A measure padded as a
/// pickup, or cut short, is filled only as far as its padding leaves. The
/// measures are then put one after another by what each holds.
pub fn make_rests(stream: &mut Stream) -> Result<()> {
    fill_rests(stream, None, false)
}

/// music21's `makeRests` with `fillGaps=True` and
/// `timeRangeFromBarDuration=True`, rests `hidden` or not. A stream with no
/// measures is filled over `range`, or from its start to its end where none
/// is given.
pub(crate) fn fill_rests(
    stream: &mut Stream,
    range: Option<(FloatType, FloatType)>,
    hidden: bool,
) -> Result<()> {
    let is_part = |element: &StreamElement| {
        element
            .as_stream()
            .is_some_and(|inner| matches!(inner.kind(), StreamKind::Part | StreamKind::PartStaff))
    };
    if stream.events().iter().any(|event| is_part(event.element())) {
        let mut events = stream.events().to_vec();
        for event in &mut events {
            if let StreamElement::Stream(inner) = event.element_mut()
                && matches!(inner.kind(), StreamKind::Part | StreamKind::PartStaff)
            {
                let spanners = inner.spanners().to_vec();
                fill_rests(inner, range, hidden)?;
                inner.clear_spanners();
                for spanner in spanners {
                    inner.add_spanner(spanner);
                }
            }
        }
        let spanners = stream.spanners().to_vec();
        *stream = stream.with_events(events);
        for spanner in spanners {
            stream.add_spanner(spanner);
        }
        return Ok(());
    }
    let has_measures = stream.events().iter().any(|event| {
        event
            .element()
            .as_stream()
            .is_some_and(|inner| inner.kind() == StreamKind::Measure)
    });
    if !has_measures {
        let (low, high) = range.unwrap_or((0.0, stream.end_offset()));
        let has_voices = stream
            .events()
            .iter()
            .any(|event| is_voice(event.element()));
        if has_voices {
            let mut events = stream.events().to_vec();
            for event in &mut events {
                if let StreamElement::Stream(voice) = event.element_mut()
                    && voice.kind() == StreamKind::Voice
                {
                    fill_line(voice, low, high, hidden)?;
                }
            }
            let spanners = stream.spanners().to_vec();
            *stream = stream.with_events(events);
            for spanner in spanners {
                stream.add_spanner(spanner);
            }
        } else {
            let spanners = stream.spanners().to_vec();
            fill_line(stream, low, high, hidden)?;
            for spanner in spanners {
                stream.add_spanner(spanner);
            }
        }
        return Ok(());
    }
    let mut events = stream.events().to_vec();
    // The meters standing in the part itself, outside any measure.
    let loose: Vec<(FloatType, TimeSignature)> = events
        .iter()
        .filter_map(|event| match event.element() {
            StreamElement::TimeSignature(meter) => Some((event.offset(), meter.clone())),
            _ => None,
        })
        .collect();
    let mut meter: Option<TimeSignature> = None;
    for event in &mut events {
        let offset = event.offset();
        let StreamElement::Stream(measure) = event.element_mut() else {
            continue;
        };
        if measure.kind() != StreamKind::Measure {
            continue;
        }
        if let Some(own) = meter_in(measure) {
            meter = Some(own);
        }
        // A measure no meter has been stated for takes the one standing in
        // the part before it, and failing that is as long as the meter that
        // fits what it holds: music21's `barDuration`.
        let bar = match &meter {
            Some(meter) => meter.bar_quarter_length(),
            None => match loose.iter().rev().find(|(at, _)| *at < offset) {
                Some((_, meter)) => meter.bar_quarter_length(),
                None => crate::meter::best_time_signature(measure)
                    .map_or_else(|_| measure.end_offset(), |meter| meter.bar_quarter_length()),
            },
        };
        let target = FloatType::max(
            op_frac(bar - measure.padding_left() - measure.padding_right()),
            0.0,
        );
        let mut held = measure.events().to_vec();
        for inner in &mut held {
            if let StreamElement::Stream(voice) = inner.element_mut()
                && voice.kind() == StreamKind::Voice
            {
                fill_line(voice, 0.0, target, hidden)?;
            }
        }
        **measure = measure.with_events(held);
        fill_line(measure, 0.0, target.min(bar), hidden)?;
    }
    *stream = stream.with_events(events);
    make_ties_of(stream, |element| matches!(element, StreamElement::Rest(_)))?;
    let mut events = stream.events().to_vec();
    let mut reached = 0.0;
    for event in &mut events {
        if let StreamElement::Stream(measure) = event.element()
            && measure.kind() == StreamKind::Measure
        {
            let length = measure.end_offset();
            *event = StreamEvent::new(reached, event.element().clone());
            reached = op_frac(reached + length);
        }
    }
    *stream = stream.with_events(sorted_events(events));
    Ok(())
}

/// The key signature a measure opens with: a key or a signature standing at
/// its very start.
fn key_signature_in(measure: &Stream) -> Option<crate::key::KeySignature> {
    measure
        .events()
        .iter()
        .find_map(|event| match event.element() {
            StreamElement::KeySignature(signature) if event.offset() == 0.0 => {
                Some(signature.clone())
            }
            StreamElement::Key(key) if event.offset() == 0.0 => Some(key.key_signature()),
            _ => None,
        })
}

/// The names of the notes a key signature leaves unaltered or alters: the
/// major scale it is the signature of.
fn diatonic_names(signature: &crate::key::KeySignature) -> Vec<String> {
    signature
        .scale("major")
        .and_then(|scale| scale.pitches())
        .map(|pitches| pitches.iter().map(Pitch::name).collect())
        .unwrap_or_default()
}

/// Every pitch a stream holds, element by element and each nested stream
/// where it stands, the notes a chord symbol stands for among them:
/// music21's `Stream.pitches`.
fn pitches_in_order(stream: &Stream) -> Vec<Pitch> {
    let mut out = Vec::new();
    for event in stream.events() {
        match event.element() {
            StreamElement::Stream(inner) => out.extend(pitches_in_order(inner)),
            StreamElement::ChordSymbol(symbol) => {
                out.extend(symbol.pitches().unwrap_or_default());
            }
            element => out.extend(element.pitches()),
        }
    }
    out
}

/// Decides which accidentals are written, measure by measure: music21's
/// `makeAccidentals` on a part.
///
/// A stream holding parts has each of them done. An accidental is written
/// where the key signature in force does not already say it, where the same
/// note has just been written otherwise in this measure or the one before,
/// and not where the note is tied from the one before. A key signature is in
/// force from the measure that opens with it.
///
/// The accidentals an ornament implies are not considered.
pub fn make_accidentals(stream: &mut Stream) {
    make_accidentals_by(stream, true);
}

/// [`make_accidentals`] with music21's `cautionaryNotImmediateRepeat` said:
/// whether an altered note written again later in its measure, not straight
/// after, shows its accidental again.
pub(crate) fn make_accidentals_by(stream: &mut Stream, cautionary_not_immediate_repeat: bool) {
    let part_like = |element: &StreamElement| {
        element
            .as_stream()
            .is_some_and(|inner| !matches!(inner.kind(), StreamKind::Measure | StreamKind::Voice))
    };
    let mut events = stream.events().to_vec();
    if events.iter().any(|event| part_like(event.element())) {
        for event in &mut events {
            if let StreamElement::Stream(inner) = event.element_mut() {
                make_accidentals_by(inner, cautionary_not_immediate_repeat);
            }
        }
        let spanners = stream.spanners().to_vec();
        *stream = stream.with_events(events);
        for spanner in spanners {
            stream.add_spanner(spanner);
        }
        return;
    }

    let mut signature: Option<crate::key::KeySignature> = None;
    let mut diatonic: Vec<String> = Vec::new();
    let mut past_measure: Vec<Pitch> = Vec::new();
    let mut tied: Option<Vec<String>> = None;
    // What the measure before held: its pitches, and the ties its last
    // sounding element left open.
    let mut previous: Option<(Vec<Pitch>, Option<Vec<String>>)> = None;
    for event in &mut events {
        let StreamElement::Stream(measure) = event.element_mut() else {
            continue;
        };
        if measure.kind() != StreamKind::Measure {
            continue;
        }
        let own = key_signature_in(measure);
        if let Some((pitches, open)) = &previous {
            if own.is_none() {
                past_measure = pitches.clone();
            } else if signature.is_some() {
                past_measure = pitches
                    .iter()
                    .filter(|pitch| !diatonic.contains(&pitch.name()))
                    .cloned()
                    .collect();
            }
            if let Some(open) = open {
                // A tie does not reach across a new key signature: music21
                // asks whether the tied note, octave and all, is a name of
                // the new key, which it never is.
                tied = Some(if own.is_some() {
                    Vec::new()
                } else {
                    open.clone()
                });
            }
        }
        if let Some(own) = own {
            diatonic = diatonic_names(&own);
            signature = Some(own);
        }
        let altered = signature
            .as_ref()
            .and_then(|signature| signature.altered_pitches().ok())
            .unwrap_or_default();

        let mut past: Vec<Pitch> = Vec::new();
        let mut open: Vec<String> = tied.clone().unwrap_or_default();
        let mut last_sounding: Option<Vec<String>> = None;
        measure.for_each_mut(&mut |_, element| match element {
            StreamElement::Note(note) => {
                let mut pitch = note.pitch().clone();
                pitch.update_accidental_display(&AccidentalDisplayOptions {
                    pitch_past: &past,
                    pitch_past_measure: &past_measure,
                    altered_pitches: &altered,
                    last_note_was_tied: open.contains(&pitch.name_with_octave()),
                    cautionary_not_immediate_repeat,
                    ..AccidentalDisplayOptions::default()
                });
                note.set_pitch(pitch.clone());
                open.clear();
                if note
                    .tie()
                    .is_some_and(|tie| tie.tie_type() != TieType::Stop)
                {
                    open.push(pitch.name_with_octave());
                }
                last_sounding = Some(open.clone());
                past.push(pitch);
            }
            StreamElement::Chord(chord) => {
                let mut seen: Vec<String> = Vec::new();
                for index in 0..chord.notes().len() {
                    let others: Vec<Pitch> = chord
                        .notes()
                        .iter()
                        .enumerate()
                        .filter(|(other, _)| *other != index)
                        .map(|(_, note)| note.pitch().clone())
                        .collect();
                    let note = &mut chord.notes_mut()[index];
                    let mut pitch = note.pitch().clone();
                    pitch.update_accidental_display(&AccidentalDisplayOptions {
                        pitch_past: &past,
                        pitch_past_measure: &past_measure,
                        other_simultaneous_pitches: &others,
                        altered_pitches: &altered,
                        last_note_was_tied: open.contains(&pitch.name_with_octave()),
                        cautionary_not_immediate_repeat,
                        ..AccidentalDisplayOptions::default()
                    });
                    note.set_pitch(pitch.clone());
                    if note
                        .tie()
                        .is_some_and(|tie| tie.tie_type() != TieType::Stop)
                    {
                        seen.push(pitch.name_with_octave());
                    }
                }
                open = seen;
                last_sounding = Some(open.clone());
                past.extend(chord.notes().iter().map(|note| note.pitch().clone()));
            }
            StreamElement::ChordSymbol(symbol) => {
                // A chord symbol is a chord to music21: each note it stands
                // for is decided like a chord's, and counts among those
                // already heard.
                let mut sounded = symbol.pitches().unwrap_or_default();
                for index in 0..sounded.len() {
                    let others: Vec<Pitch> = sounded
                        .iter()
                        .enumerate()
                        .filter(|(other, _)| *other != index)
                        .map(|(_, pitch)| pitch.clone())
                        .collect();
                    let tied = open.contains(&sounded[index].name_with_octave());
                    sounded[index].update_accidental_display(&AccidentalDisplayOptions {
                        pitch_past: &past,
                        pitch_past_measure: &past_measure,
                        other_simultaneous_pitches: &others,
                        altered_pitches: &altered,
                        last_note_was_tied: tied,
                        ..AccidentalDisplayOptions::default()
                    });
                }
                // The root and the bass are notes of that chord, so a
                // natural one of them is given is the root's or the bass's.
                let given = |named: &Pitch| {
                    sounded
                        .iter()
                        .find(|pitch| {
                            pitch.step() == named.step() && pitch.alter() == named.alter()
                        })
                        .and_then(Pitch::written_accidental)
                        .cloned()
                };
                if symbol.root().written_accidental().is_none()
                    && let Some(accidental) = given(symbol.root())
                {
                    let mut root = symbol.root().clone();
                    root.set_written_accidental(Some(accidental));
                    symbol.set_root(root);
                }
                if let Some(bass) = symbol.bass().cloned()
                    && bass.written_accidental().is_none()
                    && let Some(accidental) = given(&bass)
                {
                    let mut bass = bass;
                    bass.set_written_accidental(Some(accidental));
                    symbol.set_bass(Some(bass));
                }
                open.clear();
                last_sounding = Some(Vec::new());
                past.extend(sounded);
            }
            StreamElement::Rest(_) => open.clear(),
            StreamElement::Unpitched(_) | StreamElement::PercussionChord(_) => {
                open.clear();
                last_sounding = Some(Vec::new());
            }
            _ => {}
        });
        tied = Some(open);
        previous = Some((pitches_in_order(measure), last_sounding));
    }
    *stream = stream.with_events(events);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Duration, Note};

    fn eighths(names: &[&str]) -> Stream {
        let mut measure = Stream::with_kind(StreamKind::Measure);
        measure.insert(0.0, Clef::treble());
        measure.insert(0.0, TimeSignature::new(2, 4).unwrap());
        for (index, name) in names.iter().enumerate() {
            let note = Note::from_name(name)
                .unwrap()
                .with_duration(Duration::eighth());
            measure.insert(index as FloatType * 0.5, note);
        }
        measure
    }

    fn beams_and_stems(measure: &Stream) -> Vec<(Option<BeamType>, StemDirection)> {
        measure
            .events()
            .iter()
            .filter_map(|event| match event.element() {
                StreamElement::Note(note) => Some((
                    note.beams().by_number(1).and_then(|beam| beam.beam_type()),
                    note.stem_direction(),
                )),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn eighths_are_beamed_by_the_beat_and_stemmed_together() {
        // music21: 2/4 with C5 D5 E4 F4 beams in pairs, the high pair down
        // and the low pair up.
        let mut part = Stream::with_kind(StreamKind::Part);
        part.insert(0.0, eighths(&["C5", "D5", "E4", "F4"]));
        make_beams(&mut part).unwrap();
        let measure = part.measures()[0];
        assert_eq!(
            beams_and_stems(measure),
            [
                (Some(BeamType::Start), StemDirection::Down),
                (Some(BeamType::Stop), StemDirection::Down),
                (Some(BeamType::Start), StemDirection::Up),
                (Some(BeamType::Stop), StemDirection::Up),
            ]
        );
    }

    #[test]
    fn a_measure_with_no_meter_is_left_alone() {
        let mut measure = Stream::with_kind(StreamKind::Measure);
        for (index, name) in ["C4", "D4"].iter().enumerate() {
            let note = Note::from_name(name)
                .unwrap()
                .with_duration(Duration::eighth());
            measure.insert(index as FloatType * 0.5, note);
        }
        let mut part = Stream::with_kind(StreamKind::Part);
        part.insert(0.0, measure);
        make_beams(&mut part).unwrap();
        assert!(
            beams_and_stems(part.measures()[0])
                .iter()
                .all(|(beam, stem)| beam.is_none() && *stem == StemDirection::Unspecified)
        );
    }
}
