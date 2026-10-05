//! What music21's `makeNotation` does to a whole score, and the pieces of it
//! the other functions of this module do not already cover: brackets over
//! tuplets, tuplets cut to complete one another or joined back into one
//! value, and lengths no single value writes cut into ones that do.

use crate::defaults::FloatType;
use crate::duration::{Duration, Tuplet, TupletBracket, TupletType};
use crate::error::Result;
use crate::spanner::Spanner;
use crate::stream::{Stream, StreamElement, StreamEvent, StreamKind};

use super::{
    is_general_note, is_not_rest, is_voice, make_accidentals_by, make_beams, make_measures,
    make_ties, op_frac, split_element,
};

/// Works out what a score leaves unsaid, as music21's `makeNotation` does,
/// changing the stream in place.
///
/// A stream holding parts has each part done; any other stream is done as
/// one part. A part with no measures is first cut into them, its
/// overlapping notes put in voices. Then, in music21's order: accidentals are
/// decided where no accidental in the part has been decided yet, notes
/// running past a barline are cut and tied, tuplets left incomplete by a cut
/// are completed and ones a cut left whole are joined back, the notes are
/// beamed where no note in the part is beamed yet, and each measure's
/// tuplets are bracketed where none of them is yet.
///
/// Spanners stay on the elements they joined; where one of those is cut, on
/// its first piece.
///
/// # Errors
///
/// A part that cannot be cut into measures, as its meters say.
///
/// ```
/// use music21_rs::makenotation::make_notation;
/// use music21_rs::{Duration, Note, Stream, StreamKind, TimeSignature};
///
/// let mut part = Stream::with_kind(StreamKind::Part);
/// part.insert(0.0, TimeSignature::new(2, 4)?);
/// for (place, name) in ["C4", "D4", "E4", "F4"].iter().enumerate() {
///     let note = Note::from_name(name)?.with_duration(Duration::eighth());
///     part.insert(place as f64 * 0.5, note);
/// }
/// part.insert(2.0, Note::from_name("G4")?.with_duration(Duration::half()));
/// make_notation(&mut part)?;
/// let measures = part.measures();
/// assert_eq!(measures.len(), 2);
/// # Ok::<(), music21_rs::Error>(())
/// ```
pub fn make_notation(stream: &mut Stream) -> Result<()> {
    keeping_spanners(stream, Kept::Retained, &mut |stream| {
        if has_parts(stream) {
            for event in stream.events_mut() {
                if let StreamElement::Stream(inner) = event.element_mut() {
                    make_part_notation(inner)?;
                }
            }
            Ok(())
        } else {
            make_part_notation(stream).map(|_| ())
        }
    })
}

/// What music21's makeNotation leaves undone on one part, so that its
/// exporter may try again: whether beaming failed.
pub(crate) struct PartNotation {
    #[cfg_attr(not(feature = "musicxml"), allow(dead_code))]
    pub(crate) beams_failed: bool,
}

/// Whether a stream holds parts: music21's `hasPartLikeStreams`, as a score
/// asks it.
fn has_parts(stream: &Stream) -> bool {
    stream.has_part_like_streams()
}

/// music21's `Stream.makeNotation` on one part.
pub(crate) fn make_part_notation(part: &mut Stream) -> Result<PartNotation> {
    make_part_notation_by(part, true)
}

/// music21's `Stream.makeNotation` on one part, given its
/// `cautionaryNotImmediateRepeat`.
pub(crate) fn make_part_notation_by(
    part: &mut Stream,
    cautionary_not_immediate_repeat: bool,
) -> Result<PartNotation> {
    if part.measures().is_empty() {
        make_voices_filling_gaps(part)?;
        *part = make_measures(part)?;
        if part.measures().is_empty() {
            return Err(crate::error::Error::Meter(format!(
                "no measures found in stream with {} elements",
                part.len()
            )));
        }
    }
    if !accidentals_made(part) {
        make_accidentals_by(part, cautionary_not_immediate_repeat);
    }
    make_ties(part)?;
    // Which measures had tuplets joined in their own line, which music21
    // marks as wanting their brackets made again.
    let mut consolidated: Vec<bool> = Vec::new();
    for_each_measure(part, &mut |measure| {
        for_each_container(measure, &mut split_elements_to_complete_tuplets)?;
        let own = consolidate_completed_tuplets(measure);
        for event in measure.events_mut() {
            if let StreamElement::Stream(inner) = event.element_mut() {
                for_each_container(inner, &mut |container| {
                    consolidate_completed_tuplets(container);
                    Ok(())
                })?;
            }
        }
        consolidated.push(own);
        Ok(())
    })?;
    let mut beams_failed = false;
    if !beams_made(part) && make_beams(part).is_err() {
        beams_failed = true;
    }
    let mut index = 0;
    for_each_measure(part, &mut |measure| {
        let redo = consolidated.get(index).copied().unwrap_or(false);
        index += 1;
        if redo || tuplet_brackets_made(measure) != Some(true) {
            make_tuplet_brackets(measure);
        }
        Ok(())
    })?;
    Ok(PartNotation { beams_failed })
}

/// music21's `Measure.makeNotation`, on each measure of a part: accidentals
/// decided, a meter where the measure states none (`best_time_signature`,
/// or `4/4` where none fits), tuplets completed and the tied ones joined,
/// the notes beamed and each line's tuplets bracketed.
#[cfg(feature = "musicxml")]
pub(crate) fn make_measure_notation(part: &mut Stream) -> Result<()> {
    make_accidentals_by(part, true);
    for_each_measure(part, &mut |measure| {
        if super::meter_in(measure).is_none() {
            let meter = crate::meter::best_time_signature(measure)
                .or_else(|_| crate::meter::TimeSignature::new(4, 4))?;
            super::insert_sorted(measure, 0.0, StreamElement::TimeSignature(meter));
        }
        for_each_container(measure, &mut split_elements_to_complete_tuplets)?;
        for_each_container(measure, &mut |container| {
            consolidate_completed_tuplets(container);
            Ok(())
        })
    })?;
    make_beams(part)?;
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
    })
}

/// music21's `makeVoices` with `fillGaps`, on a stream with no measures:
/// overlapping notes are put in voices, which are filled out with rests.
fn make_voices_filling_gaps(stream: &mut Stream) -> Result<()> {
    let has_voices = |stream: &Stream| stream.events().iter().any(|e| is_voice(e.element()));
    if has_voices(stream) {
        return Ok(());
    }
    super::make_voices(stream);
    if !has_voices(stream) {
        return Ok(());
    }
    // music21 fills the voices out, then takes every rest standing in the
    // stream itself away.
    let end = stream.end_offset();
    let mut events = stream.events().to_vec();
    for event in &mut events {
        if let StreamElement::Stream(voice) = event.element_mut()
            && voice.kind() == StreamKind::Voice
        {
            super::fill_line(voice, 0.0, end, false)?;
        }
    }
    events.retain(|event| !matches!(event.element(), StreamElement::Rest(_)));
    *stream = stream.with_events(super::sorted_events(events));
    Ok(())
}

/// Calls an edit on each measure a part holds.
pub(crate) fn for_each_measure(
    part: &mut Stream,
    edit: &mut dyn FnMut(&mut Stream) -> Result<()>,
) -> Result<()> {
    for event in part.events_mut() {
        if let StreamElement::Stream(measure) = event.element_mut()
            && measure.kind() == StreamKind::Measure
        {
            edit(measure)?;
        }
    }
    Ok(())
}

/// Calls an edit on a stream and on every stream inside it, the outer one
/// first: music21's `recurse(streamsOnly=True, includeSelf=True)`.
fn for_each_container(
    stream: &mut Stream,
    edit: &mut dyn FnMut(&mut Stream) -> Result<()>,
) -> Result<()> {
    edit(stream)?;
    for event in stream.events_mut() {
        if let StreamElement::Stream(inner) = event.element_mut() {
            for_each_container(inner, edit)?;
        }
    }
    Ok(())
}

/// Whether some accidental in a stream has had its showing decided:
/// music21's `haveAccidentalsBeenMade`.
pub(crate) fn accidentals_made(stream: &Stream) -> bool {
    stream.pitches().iter().any(|pitch| {
        pitch
            .written_accidental()
            .is_some_and(|accidental| accidental.display_status().is_some())
    })
}

/// Whether some note or chord in a stream is beamed: music21's
/// `haveBeamsBeenMade`.
pub(crate) fn beams_made(stream: &Stream) -> bool {
    stream.recurse().iter().any(|(_, element)| match element {
        StreamElement::Note(note) => !note.beams().is_empty(),
        StreamElement::Chord(chord) => !chord.beams().is_empty(),
        StreamElement::Unpitched(stroke) => !stroke.written().beams().is_empty(),
        StreamElement::PercussionChord(chord) => !chord.written().beams().is_empty(),
        _ => false,
    })
}

/// Whether a stream's tuplets are bracketed: music21's
/// `haveTupletBracketsBeenMade`. Nothing where it has no tuplets, and
/// otherwise whether any of them says where its bracket falls.
pub(crate) fn tuplet_brackets_made(stream: &Stream) -> Option<bool> {
    let mut found = false;
    for (_, element) in stream.recurse() {
        if !is_general_note(element) {
            continue;
        }
        let Some(duration) = element.duration() else {
            continue;
        };
        let tuplets = duration.tuplets();
        if let Some(first) = tuplets.first() {
            found = true;
            if first.tuplet_type().is_some() {
                return Some(true);
            }
        }
    }
    found.then_some(false)
}

/// The duration an element keeps, or a quarter where it keeps none.
fn duration_of(element: &StreamElement) -> Duration {
    element.duration().cloned().unwrap_or_default()
}

/// Gives an element a duration, keeping whatever else it says.
pub(crate) fn set_duration(element: &mut StreamElement, duration: Duration) {
    match element {
        StreamElement::Note(note) => note.set_duration(duration),
        StreamElement::Chord(chord) => chord.set_duration(duration),
        StreamElement::Rest(rest) => rest.set_duration(duration),
        StreamElement::Unpitched(stroke) => stroke.written_mut().set_duration(duration),
        StreamElement::PercussionChord(chord) => chord.written_mut().set_duration(duration),
        StreamElement::ChordSymbol(symbol) => symbol.set_duration(duration),
        _ => {}
    }
}

/// Marks the first and last of each run of tuplets in a line of notes as
/// where its bracket starts and stops: music21's `makeTupletBrackets`.
///
/// A line is a measure's own notes, or one voice's; grace notes are passed
/// over. A run ends when the notes of a tuplet have filled its length, or
/// where the next note is in no tuplet. A tuplet with none either side of it
/// is bracketed alone, and drawn with no bracket. A note written in more
/// than one tuplet at once is taken as in none.
pub fn make_tuplet_brackets(line: &mut Stream) {
    let places: Vec<usize> = line
        .events()
        .iter()
        .enumerate()
        .filter(|(_, event)| is_general_note(event.element()))
        .filter(|(_, event)| !event.element().duration().is_some_and(Duration::is_grace))
        .map(|(place, _)| place)
        .collect();
    let tuplets: Vec<Option<Tuplet>> = places
        .iter()
        .map(|place| {
            let tuplets = duration_of(line.events()[*place].element()).tuplets();
            match tuplets.as_slice() {
                [only] => Some(*only),
                _ => None,
            }
        })
        .collect();

    let mut count: FloatType = 0.0;
    let mut target: Option<FloatType> = None;
    let mut previous_was_tuplet = false;
    for (index, place) in places.iter().enumerate() {
        let Some(tuplet) = tuplets[index] else {
            previous_was_tuplet = false;
            continue;
        };
        let next = tuplets.get(index + 1).copied().flatten();
        let element = line.events_mut()[*place].element_mut();
        let length = element.quarter_length();
        count = op_frac(count + length);
        let mut marked = tuplet;
        if !previous_was_tuplet || target.is_none() {
            if next.is_none() {
                marked.set_tuplet_type(Some(TupletType::StartStop));
                marked.set_bracket(TupletBracket::None);
                count = 0.0;
            } else {
                marked.set_tuplet_type(Some(TupletType::Start));
                target = Some(tuplet.total_tuplet_length());
            }
        } else if next.is_none() || target.is_some_and(|target| count >= target) {
            marked.set_tuplet_type(Some(TupletType::Stop));
            target = None;
            count = 0.0;
        } else {
            // Inside a run: whatever an earlier pass said here is unsaid.
            marked.set_tuplet_type(None);
        }
        let mut duration = duration_of(element);
        duration.set_tuplets(vec![marked]);
        set_duration(element, duration);
        previous_was_tuplet = true;
    }
}

/// Whether a duration was worked out from a length rather than written as
/// values: music21's `expressionIsInferred`. An element given no duration
/// has the quarter every note starts with, which nothing worked out.
fn expression_is_inferred(element: &StreamElement) -> bool {
    element
        .duration()
        .is_some_and(Duration::expression_is_inferred)
}

/// music21's `opFrac`-exact comparison of two offsets.
fn same_offset(left: FloatType, right: FloatType) -> bool {
    (left - right).abs() < 1e-9
}

/// Cuts the note or rest after an unfinished tuplet where that finishes it:
/// music21's `splitElementsToCompleteTuplets`, on one line.
///
/// Both the notes of the tuplet and the one cut must have had their lengths
/// worked out rather than written, and the one cut must follow without a
/// gap. The pieces are tied.
fn split_elements_to_complete_tuplets(line: &mut Stream) -> Result<()> {
    // Each event, and whether it was there before any cut: only those are
    // walked, as music21 walks a list taken before it starts.
    let mut events: Vec<(StreamEvent, bool)> = line
        .events()
        .iter()
        .map(|event| (event.clone(), true))
        .collect();
    let mut last: Option<Tuplet> = None;
    let mut sum: FloatType = 0.0;
    let mut index = 0;
    while index < events.len() {
        let (event, original) = &events[index];
        if !original || !is_general_note(event.element()) {
            index += 1;
            continue;
        }
        let element = event.element();
        let tuplets = duration_of(element).tuplets();
        match tuplets.first() {
            Some(first)
                if expression_is_inferred(element) && last.is_none_or(|last| last == *first) =>
            {
                last = Some(*first);
                sum = op_frac(element.quarter_length() + sum);
            }
            _ => {
                last = None;
                sum = 0.0;
                index += 1;
                continue;
            }
        }
        let first = tuplets[0];
        let to_complete = op_frac(first.total_tuplet_length() - sum);
        if to_complete == 0.0 {
            last = None;
            sum = 0.0;
            index += 1;
            continue;
        }
        let offset = event.offset();
        let end = op_frac(offset + element.quarter_length());
        // The next note after this one: music21's `next`, which looks on
        // through the line and down into the voices it holds. A note in a
        // voice comes after this one only if it starts later, and before a
        // note of the line itself starting where it does.
        let own = (index + 1..events.len()).find(|at| is_general_note(events[*at].0.element()));
        let mut nested: Option<(usize, usize, FloatType)> = None;
        for (at, (held, _)) in events.iter().enumerate() {
            let StreamElement::Stream(inner) = held.element() else {
                continue;
            };
            let found = inner.events().iter().enumerate().find(|(_, inner_event)| {
                is_general_note(inner_event.element())
                    && held.offset() + inner_event.offset() > offset + 1e-9
            });
            if let Some((place, inner_event)) = found {
                let starts = held.offset() + inner_event.offset();
                if nested.is_none_or(|(_, _, best)| starts < best - 1e-9) {
                    nested = Some((at, place, starts));
                }
            }
        }
        let nested = nested.filter(|(_, _, starts)| {
            own.is_none_or(|own| *starts <= events[own].0.offset() + 1e-9)
        });
        if let Some((voice_at, place, _)) = nested {
            let StreamElement::Stream(inner) = events[voice_at].0.element() else {
                index += 1;
                continue;
            };
            let next_event = inner.events()[place].clone();
            // music21 compares the note's offset in its own voice.
            if same_offset(next_event.offset(), end)
                && expression_is_inferred(next_event.element())
                && 0.0 < to_complete
                && to_complete < next_event.element().quarter_length()
            {
                let at = next_event.offset();
                let (left, right) = split_element(next_event.element(), to_complete)?;
                if let StreamElement::Stream(inner) = events[voice_at].0.element_mut() {
                    inner.events_mut()[place] = StreamEvent::new(at, left);
                }
                // The piece cut off goes into the line the walk is in, not
                // into the voice the note was in.
                let piece = StreamEvent::new(op_frac(at + to_complete), right);
                let place = events
                    .iter()
                    .position(|(held, _)| super::event_order(held, &piece).is_gt())
                    .unwrap_or(events.len());
                events.insert(place, (piece, false));
            }
            index += 1;
            continue;
        }
        let Some(next) = own else {
            index += 1;
            continue;
        };
        let (next_event, _) = &events[next];
        if !same_offset(next_event.offset(), end) {
            index += 1;
            continue;
        }
        if expression_is_inferred(next_event.element())
            && 0.0 < to_complete
            && to_complete < next_event.element().quarter_length()
        {
            let at = next_event.offset();
            let (left, right) = split_element(next_event.element(), to_complete)?;
            events[next].0 = StreamEvent::new(at, left);
            // The cut piece goes where music21 inserts it: after everything
            // that sorts no later, which is always after this note.
            let piece = StreamEvent::new(op_frac(at + to_complete), right);
            let place = events
                .iter()
                .position(|(held, _)| super::event_order(held, &piece).is_gt())
                .unwrap_or(events.len());
            events.insert(place, (piece, false));
        }
        index += 1;
    }
    let spanners = line.spanners().to_vec();
    *line = line.with_events(events.into_iter().map(|(event, _)| event).collect());
    for spanner in spanners {
        line.add_spanner(spanner);
    }
    Ok(())
}

/// Joins a run of tied notes, or of rests, that fills one tuplet exactly
/// into one note of the tuplet's whole length: music21's
/// `consolidateCompletedTuplets` with `onlyIfTied`, on one line. Whether
/// anything was joined.
fn consolidate_completed_tuplets(line: &mut Stream) -> bool {
    let mut events = line.events().to_vec();
    let mut removed: Vec<usize> = Vec::new();
    let mut consolidated = false;
    // music21's `TupletSearchState`.
    let mut group: Vec<Option<usize>> = Vec::new();
    let mut partial: FloatType = 0.0;
    let mut last: Option<Tuplet> = None;
    let mut target: Option<FloatType> = None;
    let notes: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, event)| is_general_note(event.element()))
        .map(|(index, _)| index)
        .collect();
    let reexpressible = |event: &StreamEvent| {
        let element = event.element();
        let tied = match element {
            StreamElement::Note(note) => note.tie().is_some(),
            StreamElement::Chord(chord) => chord.notes().iter().any(|note| note.tie().is_some()),
            StreamElement::Unpitched(stroke) => stroke.written().tie().is_some(),
            _ => false,
        };
        expression_is_inferred(element)
            && duration_of(element).tuplets().len() < 2
            && (matches!(element, StreamElement::Rest(_)) || tied)
    };
    for (walked, &index) in notes.iter().enumerate() {
        let event = &events[index];
        let element = event.element();
        partial = op_frac(partial + element.quarter_length());
        let tested = if group.is_empty() {
            true
        } else {
            let previous = &events[notes[walked - 1]];
            let rests = matches!(element, StreamElement::Rest(_))
                && matches!(previous.element(), StreamElement::Rest(_));
            let same_notes = is_not_rest(element)
                && is_not_rest(previous.element())
                && same_pitches(element, previous.element());
            let tuplets = duration_of(element).tuplets();
            (rests || same_notes)
                && same_offset(
                    op_frac(previous.offset() + previous.element().quarter_length()),
                    event.offset(),
                )
                && tuplets.len() == 1
                && Some(tuplets[0]) == last
        };
        if tested {
            if group.is_empty() {
                partial = element.quarter_length();
                if let Some(first) = duration_of(element).tuplets().first() {
                    last = Some(*first);
                    target = Some(first.total_tuplet_length());
                    group.push(Some(index));
                }
            } else {
                group.push(Some(index));
            }
        } else if !group.is_empty() {
            group.push(None);
        }
        if target.is_some_and(|target| target == partial) {
            let all = group
                .iter()
                .all(|held| held.is_some_and(|held| reexpressible(&events[held])));
            if all && let Some(Some(first)) = group.first() {
                consolidated = true;
                removed.extend(group.iter().skip(1).flatten());
                if let Ok(duration) = Duration::new(partial) {
                    set_duration(events[*first].element_mut(), duration);
                }
            }
            group.clear();
            partial = 0.0;
            last = None;
            target = None;
        }
    }
    if !consolidated {
        return false;
    }
    let kept: Vec<StreamEvent> = events
        .drain(..)
        .enumerate()
        .filter(|(index, _)| !removed.contains(index))
        .map(|(_, event)| event)
        .collect();
    let spanners = line.spanners().to_vec();
    *line = line.with_events(kept);
    for spanner in spanners {
        line.add_spanner(spanner);
    }
    true
}

/// Whether two notes or chords sound the same pitches, spelled alike:
/// music21's `pitches ==`.
fn same_pitches(left: &StreamElement, right: &StreamElement) -> bool {
    let left = left.pitches();
    let right = right.pitches();
    left.len() == right.len() && left.iter().zip(&right).all(|(one, other)| one == other)
}

/// Cuts every note, chord and rest whose length no single written value
/// writes into pieces that each are one, tied: music21's `splitAtDurations`
/// with `recurse`.
///
/// A rest filling its measure's whole bar is left as it is, as a whole-bar
/// rest is written as one whatever its length. Spanners joining a note that
/// is cut start on its first piece and end on its last.
///
/// ```
/// use music21_rs::makenotation::split_at_durations;
/// use music21_rs::{Duration, Note, Stream};
///
/// let mut line = Stream::new();
/// line.insert(0.0, Note::from_name("C4")?.with_duration(Duration::new(5.0)?));
/// split_at_durations(&mut line)?;
/// assert_eq!(line.len(), 2);
/// assert_eq!(line.events()[1].offset(), 4.0);
/// # Ok::<(), music21_rs::Error>(())
/// ```
///
/// # Errors
///
/// A length no tie of written values reaches.
pub fn split_at_durations(stream: &mut Stream) -> Result<()> {
    keeping_spanners(stream, Kept::Split, &mut |stream| {
        split_container(stream, None)
    })
}

/// [`split_at_durations`] with no spanners to keep: the length of the bar
/// in force where the stream is a measure or a voice in one.
fn split_container(stream: &mut Stream, bar: Option<FloatType>) -> Result<()> {
    let own_bar = match stream.kind() {
        StreamKind::Measure | StreamKind::Voice => bar,
        _ => None,
    };
    let mut events: Vec<StreamEvent> = Vec::new();
    // The pieces of each element cut, in the order the elements stood.
    let mut pieces: Vec<StreamEvent> = Vec::new();
    for event in stream.events() {
        let element = event.element();
        let Some(duration) = element.duration() else {
            events.push(event.clone());
            continue;
        };
        if matches!(element, StreamElement::Stream(_)) || !duration.is_complex() {
            events.push(event.clone());
            continue;
        }
        if matches!(element, StreamElement::Rest(_))
            && own_bar.is_some_and(|bar| {
                duration.tuplets().is_empty() && duration.quarter_length() == bar
            })
        {
            events.push(event.clone());
            continue;
        }
        let multiplier = duration.aggregate_tuplet_multiplier();
        let multiplier = *multiplier.numer().unwrap_or(&1) as FloatType
            / *multiplier.denom().unwrap_or(&1) as FloatType;
        let lengths: Vec<FloatType> = duration
            .written_values()
            .iter()
            .map(|value| op_frac(value.quarter_length() * multiplier))
            .collect();
        let mut offset = event.offset();
        let mut remain = element.clone();
        for length in &lengths[..lengths.len() - 1] {
            let (piece, rest) = split_element(&remain, *length)?;
            pieces.push(StreamEvent::new(offset, piece));
            offset = op_frac(offset + length);
            remain = rest;
        }
        pieces.push(StreamEvent::new(offset, remain));
    }
    // music21 puts each piece in as a new element, the first as much as the
    // others: after everything already standing where it starts.
    for piece in pieces {
        let place = events
            .iter()
            .position(|held| super::event_order(held, &piece).is_gt())
            .unwrap_or(events.len());
        events.insert(place, piece);
    }
    let spanners = stream.spanners().to_vec();
    *stream = stream.with_events(events);
    for spanner in spanners {
        stream.add_spanner(spanner);
    }
    // The meter in force, measure by measure, for the streams inside.
    let mut meter_bar = bar;
    for event in stream.events_mut() {
        if let StreamElement::Stream(inner) = event.element_mut() {
            if inner.kind() == StreamKind::Measure
                && let Some(meter) = super::meter_in(inner)
            {
                meter_bar = Some(meter.bar_quarter_length());
            }
            let inner_bar = match inner.kind() {
                StreamKind::Measure => Some(meter_bar.unwrap_or(4.0)),
                StreamKind::Voice => own_bar,
                _ => None,
            };
            split_container(inner, inner_bar)?;
        }
    }
    Ok(())
}

// --------------------------------------------------------------- spanners

/// Which piece of a cut element a spanner is left holding.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kept {
    /// The first, which is the element itself cut short, whatever end of
    /// the spanner it stands at: music21's `splitAtQuarterLength`.
    Retained,
    /// As `Retained`, for an edit that may move measures but leaves every
    /// element where it stood in its own: the element is looked for by its
    /// measure and its place in it.
    InMeasure,
    /// The first at the spanner's start and the last at its end, and none
    /// in its middle: music21's `splitAtDurations`, which replaces the
    /// element with copies.
    Split,
}

/// One element as it stood before an edit, to be found again after it.
struct Leaf {
    /// The event of the holding stream it is inside.
    top: usize,
    /// The voice it is in, if any.
    voice: Option<String>,
    /// Which of the holder's measures it is in, counted from the first, and
    /// where in it.
    measure: Option<usize>,
    local: FloatType,
    offset: FloatType,
    /// Where its last piece starts, if it is cut into the values it is
    /// written as.
    last_offset: FloatType,
    what: String,
}

fn describe(element: &StreamElement) -> String {
    let class = match element {
        StreamElement::Note(_) => "Note",
        StreamElement::Chord(_) => "Chord",
        StreamElement::Rest(_) => "Rest",
        StreamElement::Unpitched(_) => "Unpitched",
        StreamElement::PercussionChord(_) => "PercussionChord",
        StreamElement::ChordSymbol(_) => "ChordSymbol",
        StreamElement::Dynamic(_) => "Dynamic",
        StreamElement::Clef(_) => "Clef",
        StreamElement::TextExpression(_) => "TextExpression",
        StreamElement::RepeatExpression(_) => "RepeatExpression",
        StreamElement::MetronomeMark(_) => "MetronomeMark",
        StreamElement::TempoText(_) => "TempoText",
        _ => "Other",
    };
    let pitches: Vec<String> = element
        .pitches()
        .iter()
        .map(crate::pitch::Pitch::name_with_octave)
        .collect();
    let grace = element.duration().is_some_and(Duration::is_grace);
    format!("{class} {} {grace}", pitches.join(" "))
}

fn leaves_of(stream: &Stream) -> Vec<Leaf> {
    /// Where a walk has got to: the measure it is in and where that starts.
    #[derive(Clone)]
    struct Within {
        top: usize,
        voice: Option<String>,
        measure: Option<(usize, FloatType)>,
    }
    fn leaf(element: &StreamElement, offset: FloatType, within: &Within) -> Leaf {
        let last_offset = match element.duration() {
            Some(duration) if duration.is_complex() => {
                let lengths: Vec<FloatType> = duration
                    .written_values()
                    .iter()
                    .map(|value| value.quarter_length())
                    .collect();
                let written: FloatType = lengths.iter().sum();
                let scale = if written == 0.0 {
                    1.0
                } else {
                    duration.quarter_length() / written
                };
                offset + (written - lengths.last().copied().unwrap_or(0.0)) * scale
            }
            _ => offset,
        };
        Leaf {
            top: within.top,
            voice: within.voice.clone(),
            measure: within.measure.map(|(index, _)| index),
            local: within.measure.map_or(offset, |(_, start)| offset - start),
            offset,
            last_offset,
            what: describe(element),
        }
    }
    fn enter(inner: &Stream, offset: FloatType, within: &Within, measures: &mut usize) -> Within {
        let mut inside = within.clone();
        match inner.kind() {
            StreamKind::Voice => {
                inside.voice = Some(inner.id().unwrap_or_default().to_string());
            }
            StreamKind::Measure => {
                inside.measure = Some((*measures, offset));
                *measures += 1;
            }
            _ => {}
        }
        inside
    }
    fn walk(
        stream: &Stream,
        base: FloatType,
        within: &Within,
        measures: &mut usize,
        out: &mut Vec<Leaf>,
    ) {
        for event in stream.events() {
            let offset = base + event.offset();
            match event.element() {
                StreamElement::Stream(inner) => {
                    let inside = enter(inner, offset, within, measures);
                    walk(inner, offset, &inside, measures, out);
                }
                element => out.push(leaf(element, offset, within)),
            }
        }
    }
    let mut out = Vec::new();
    let mut measures = 0;
    for (top, event) in stream.events().iter().enumerate() {
        let within = Within {
            top,
            voice: None,
            measure: None,
        };
        match event.element() {
            StreamElement::Stream(inner) => {
                let inside = enter(inner, event.offset(), &within, &mut measures);
                walk(inner, event.offset(), &inside, &mut measures, &mut out);
            }
            // An element the holder holds itself is in no part, and where
            // it stands among the holder's events moves as others are cut.
            element => out.push(leaf(
                element,
                event.offset(),
                &Within {
                    top: usize::MAX,
                    ..within
                },
            )),
        }
    }
    out
}

/// Where each element standing before an edit stands after it: its first
/// piece, and its last, by position; nothing for one that is gone.
fn match_leaves(
    before: &[Leaf],
    after: &[Leaf],
    by_measure: bool,
) -> (Vec<Option<usize>>, Vec<Option<usize>>) {
    let near = |one: FloatType, other: FloatType| (one - other).abs() < 1e-6;
    let mut used = vec![false; after.len()];
    let find = |leaf: &Leaf, offset: FloatType, used: &[bool], exact_voice: bool| {
        after.iter().enumerate().position(|(index, candidate)| {
            let same_place = if by_measure && offset == leaf.offset {
                candidate.measure == leaf.measure && near(candidate.local, leaf.local)
            } else {
                near(candidate.offset, offset)
            };
            !used[index]
                && candidate.what == leaf.what
                && same_place
                && (!exact_voice || candidate.voice == leaf.voice)
                && (candidate.top == leaf.top || !exact_voice)
        })
    };
    let mut first = Vec::with_capacity(before.len());
    for leaf in before {
        let found =
            find(leaf, leaf.offset, &used, true).or_else(|| find(leaf, leaf.offset, &used, false));
        if let Some(found) = found {
            used[found] = true;
        }
        first.push(found);
    }
    let none = vec![false; after.len()];
    let last = before
        .iter()
        .zip(&first)
        .map(|(leaf, first)| {
            if near(leaf.last_offset, leaf.offset) {
                *first
            } else {
                find(leaf, leaf.last_offset, &none, true).or(*first)
            }
        })
        .collect();
    (first, last)
}

/// The spanners a stream and its parts hold, taken off them, with the
/// elements as they stood.
struct HeldSpanners {
    /// For the stream itself and for each stream it holds directly, by
    /// event index: its spanners and its elements.
    holders: Vec<(Option<usize>, Vec<Spanner>, Vec<Leaf>)>,
}

fn take_spanners(stream: &mut Stream) -> HeldSpanners {
    let mut holders = Vec::new();
    let own = stream.spanners().to_vec();
    if !own.is_empty() {
        holders.push((None, own, leaves_of(stream)));
        stream.clear_spanners();
    }
    for (index, event) in stream.events_mut().iter_mut().enumerate() {
        if let StreamElement::Stream(inner) = event.element_mut() {
            let spanners = inner.spanners().to_vec();
            if !spanners.is_empty() {
                holders.push((Some(index), spanners, leaves_of(inner)));
                inner.clear_spanners();
            }
        }
    }
    HeldSpanners { holders }
}

fn restore_spanners(stream: &mut Stream, held: HeldSpanners, kept: Kept) {
    for (holder, spanners, before) in held.holders {
        let target: &mut Stream = match holder {
            None => stream,
            Some(index) => match stream
                .events_mut()
                .get_mut(index)
                .map(StreamEvent::element_mut)
            {
                Some(StreamElement::Stream(inner)) => inner,
                _ => continue,
            },
        };
        let after = leaves_of(target);
        let (first, last) = match_leaves(&before, &after, kept == Kept::InMeasure);
        for mut spanner in spanners {
            let count = spanner.spanned().len();
            for (at, position) in spanner.spanned_mut().iter_mut().enumerate() {
                let Some(old) = *position else {
                    continue;
                };
                let was_cut = before
                    .get(old)
                    .is_some_and(|leaf| (leaf.last_offset - leaf.offset).abs() > 1e-6);
                *position = match kept {
                    Kept::Retained | Kept::InMeasure => first.get(old).copied().flatten(),
                    Kept::Split if at == 0 => first.get(old).copied().flatten(),
                    Kept::Split if at + 1 == count => last.get(old).copied().flatten(),
                    Kept::Split if was_cut => None,
                    Kept::Split => first.get(old).copied().flatten(),
                };
            }
            target.add_spanner(spanner);
        }
    }
}

/// Runs an edit over a stream, and puts the spanners it and the streams it
/// holds directly carried back on the elements they joined.
pub(crate) fn keeping_spanners<T>(
    stream: &mut Stream,
    kept: Kept,
    edit: &mut dyn FnMut(&mut Stream) -> Result<T>,
) -> Result<T> {
    let held = take_spanners(stream);
    let result = edit(stream);
    restore_spanners(stream, held, kept);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notation::{Tie, TieType};
    use crate::spanner::Spanner;
    use crate::{Note, Rest, TimeSignature};

    fn note(name: &str, quarter_length: FloatType) -> Note {
        Note::from_name(name)
            .unwrap()
            .with_duration(Duration::new(quarter_length).unwrap())
    }

    fn lengths(stream: &Stream) -> Vec<FloatType> {
        stream
            .leaves()
            .iter()
            .filter(|(_, element)| is_general_note(element))
            .map(|(_, element)| element.quarter_length())
            .collect()
    }

    fn ties(stream: &Stream) -> Vec<Option<TieType>> {
        stream
            .leaves()
            .iter()
            .filter_map(|(_, element)| match element {
                StreamElement::Note(note) => Some(note.tie().map(Tie::tie_type)),
                _ => None,
            })
            .collect()
    }

    fn close(left: &[FloatType], right: &[FloatType]) -> bool {
        left.len() == right.len()
            && left
                .iter()
                .zip(right)
                .all(|(one, other)| (one - other).abs() < 1e-9)
    }

    #[test]
    fn tuplets_are_bracketed_a_group_at_a_time() {
        // music21: six triplet eighths in 2/4 are
        // ['start', None, 'stop', 'start', None, 'stop'].
        let mut line = Stream::new();
        line.insert(0.0, TimeSignature::new(2, 4).unwrap());
        for index in 0..6 {
            line.insert(FloatType::from(index) / 3.0, note("C4", 1.0 / 3.0));
        }
        make_tuplet_brackets(&mut line);
        let types: Vec<Option<TupletType>> = line
            .leaves()
            .iter()
            .filter(|(_, element)| is_general_note(element))
            .map(|(_, element)| duration_of(element).tuplets()[0].tuplet_type())
            .collect();
        assert_eq!(
            types,
            [
                Some(TupletType::Start),
                None,
                Some(TupletType::Stop),
                Some(TupletType::Start),
                None,
                Some(TupletType::Stop),
            ]
        );
    }

    #[test]
    fn a_tuplet_alone_is_bracketed_on_its_own_with_no_bracket_drawn() {
        let mut line = Stream::new();
        line.insert(0.0, note("C4", 1.0));
        line.insert(1.0, note("D4", 1.0 / 3.0));
        line.insert(4.0 / 3.0, note("E4", 1.0));
        make_tuplet_brackets(&mut line);
        let tuplet = duration_of(line.events()[1].element()).tuplets()[0];
        assert_eq!(tuplet.tuplet_type(), Some(TupletType::StartStop));
        assert_eq!(tuplet.bracket(), TupletBracket::None);
    }

    #[test]
    fn a_note_is_cut_to_finish_the_tuplet_before_it() {
        // music21's splitElementsToCompleteTuplets on lengths 1/3, 1, 2/3:
        // [1/3, 2/3, 1/3, 2/3], tied [None, start, stop, None].
        let mut line = Stream::new();
        line.insert(0.0, note("C4", 1.0 / 3.0));
        line.insert(1.0 / 3.0, note("C4", 1.0));
        line.insert(4.0 / 3.0, note("C4", 2.0 / 3.0));
        split_elements_to_complete_tuplets(&mut line).unwrap();
        assert!(close(
            &lengths(&line),
            &[1.0 / 3.0, 2.0 / 3.0, 1.0 / 3.0, 2.0 / 3.0]
        ));
        assert_eq!(
            ties(&line),
            [None, Some(TieType::Start), Some(TieType::Stop), None]
        );
    }

    #[test]
    fn a_length_that_was_said_is_not_cut() {
        let mut line = Stream::new();
        line.insert(0.0, note("C4", 1.0 / 3.0));
        line.insert(
            1.0 / 3.0,
            Note::from_name("C4")
                .unwrap()
                .with_duration(Duration::quarter()),
        );
        split_elements_to_complete_tuplets(&mut line).unwrap();
        assert!(close(&lengths(&line), &[1.0 / 3.0, 1.0]));
    }

    #[test]
    fn a_rest_between_two_sixths_is_cut_to_finish_the_first() {
        // music21: a measure of a sixth, a rest and a sixth at 5/6 is
        // [1/6, 1/3, 1/3, 1/6] once the rest is cut.
        let mut measure = Stream::with_kind(StreamKind::Measure);
        measure.insert(0.0, note("C4", 1.0 / 6.0));
        measure.insert(5.0 / 6.0, note("C4", 1.0 / 6.0));
        super::super::fill_line(&mut measure, 0.0, 1.0, false).unwrap();
        let mut part = Stream::with_kind(StreamKind::Part);
        part.insert(0.0, measure);
        for_each_container(&mut part, &mut split_elements_to_complete_tuplets).unwrap();
        assert!(close(
            &lengths(&part),
            &[1.0 / 6.0, 1.0 / 3.0, 1.0 / 3.0, 1.0 / 6.0]
        ));
    }

    #[test]
    fn rests_filling_a_tuplet_are_joined_into_one() {
        // music21's consolidateCompletedTuplets: five sixth rests and a
        // sixth note are [0.5, 1/6, 1/6, 1/6].
        let mut line = Stream::new();
        for index in 0..5 {
            line.insert(
                FloatType::from(index) / 6.0,
                Rest::new(Duration::new(1.0 / 6.0).unwrap()),
            );
        }
        line.insert(5.0 / 6.0, note("C4", 1.0 / 6.0));
        assert!(consolidate_completed_tuplets(&mut line));
        assert!(close(
            &lengths(&line),
            &[0.5, 1.0 / 6.0, 1.0 / 6.0, 1.0 / 6.0]
        ));
    }

    #[test]
    fn untied_notes_filling_a_tuplet_are_left_apart() {
        let mut line = Stream::new();
        for index in 0..3 {
            line.insert(FloatType::from(index) / 3.0, note("C4", 1.0 / 3.0));
        }
        assert!(!consolidate_completed_tuplets(&mut line));
        assert_eq!(lengths(&line).len(), 3);
    }

    #[test]
    fn a_length_no_one_value_writes_is_cut_into_values() {
        // music21: a note of five quarters is a whole tied to a quarter.
        let mut line = Stream::new();
        line.insert(0.0, note("C4", 5.0));
        split_at_durations(&mut line).unwrap();
        assert!(close(&lengths(&line), &[4.0, 1.0]));
        assert_eq!(ties(&line), [Some(TieType::Start), Some(TieType::Stop)]);
    }

    #[test]
    fn a_rest_filling_its_bar_is_not_cut() {
        // music21 leaves a rest of five quarters whole in a 5/4 measure,
        // and cuts one that does not fill its measure.
        let mut measure = Stream::with_kind(StreamKind::Measure);
        measure.insert(0.0, TimeSignature::new(5, 4).unwrap());
        measure.insert(0.0, Rest::new(Duration::new(5.0).unwrap()));
        let mut part = Stream::with_kind(StreamKind::Part);
        part.insert(0.0, measure);
        split_at_durations(&mut part).unwrap();
        assert!(close(&lengths(&part), &[5.0]));

        let mut measure = Stream::with_kind(StreamKind::Measure);
        measure.insert(0.0, TimeSignature::new(6, 4).unwrap());
        measure.insert(0.0, Rest::new(Duration::new(5.0).unwrap()));
        let mut part = Stream::with_kind(StreamKind::Part);
        part.insert(0.0, measure);
        split_at_durations(&mut part).unwrap();
        assert!(close(&lengths(&part), &[4.0, 1.0]));
    }

    #[test]
    fn a_slur_stays_on_its_notes_and_ends_on_the_last_piece() {
        let mut line = Stream::new();
        line.insert(0.0, note("C4", 1.0));
        line.insert(1.0, note("D4", 5.0));
        line.add_spanner(Spanner::slur(vec![0, 1]));
        split_at_durations(&mut line).unwrap();
        // D4 is now a whole at 1 and a quarter at 5; the slur ends on the
        // quarter.
        assert_eq!(line.spanners()[0].spanned(), [Some(0), Some(2)]);
    }

    #[test]
    fn loose_notes_are_measured_tied_and_beamed() {
        let mut part = Stream::with_kind(StreamKind::Part);
        part.insert(0.0, TimeSignature::new(2, 4).unwrap());
        for (index, name) in ["C4", "D4", "E4", "F4"].iter().enumerate() {
            part.insert(index as FloatType * 0.5, note(name, 0.5));
        }
        part.insert(2.0, note("G4", 1.0));
        part.insert(3.0, note("A4", 2.0));
        part.add_spanner(Spanner::slur(vec![1, 6]));
        make_notation(&mut part).unwrap();
        assert_eq!(part.measures().len(), 3);
        // The half note is cut at the second barline.
        assert!(close(&lengths(&part), &[0.5, 0.5, 0.5, 0.5, 1.0, 1.0, 1.0]));
        assert_eq!(
            ties(&part)[5..],
            [Some(TieType::Start), Some(TieType::Stop)]
        );
        assert!(beams_made(&part));
        // The slur still starts on the first note and ends on the first
        // piece of the last.
        let leaves = part.leaves();
        let ends: Vec<String> = part.spanners()[0]
            .spanned()
            .iter()
            .map(|position| describe(leaves[position.unwrap()].1))
            .collect();
        assert_eq!(ends, ["Note C4 false", "Note A4 false"]);
        let last = part.spanners()[0].spanned()[1].unwrap();
        assert!((leaves[last].0 - 3.0).abs() < 1e-9);
    }

    #[test]
    fn a_duration_says_whether_its_writing_was_worked_out() {
        // music21: Duration(0.5).expressionIsInferred is True, and
        // Duration('eighth').expressionIsInferred False.
        assert!(Duration::new(0.5).unwrap().expression_is_inferred());
        assert!(!Duration::eighth().expression_is_inferred());
        let mut duration = Duration::eighth();
        duration.set_quarter_length(0.25).unwrap();
        assert!(duration.expression_is_inferred());
    }
}
