//! What an instrument does to a stream: music21's `unbundleInstruments`,
//! `bundleInstruments`, `deduplicate` and `partitionByInstrument`.
//!
//! A note may keep the instrument that plays it as its own (music21's
//! `storedInstrument`), or a stream may hold the instrument as an element
//! of its own, in force from where it stands. These move instruments
//! between the two, fold together the instruments a part says twice, and
//! split a score into a part for each instrument playing in it.

use super::Instrument;
use crate::defaults::FloatType;
use crate::makenotation::sorted_events;
use crate::stream::{Stream, StreamElement, StreamEvent, StreamKind};

/// The instrument a note, chord or unpitched stroke keeps as its own.
fn stored(element: &StreamElement) -> Option<&Instrument> {
    match element {
        StreamElement::Note(note) => note.stored_instrument(),
        StreamElement::Chord(chord) => chord.stored_instrument(),
        StreamElement::Unpitched(stroke) => stroke.written().stored_instrument(),
        StreamElement::PercussionChord(chord) => chord.written().stored_instrument(),
        _ => None,
    }
}

/// Gives a note, chord or unpitched stroke an instrument of its own, or
/// takes it away.
fn set_stored(element: &mut StreamElement, instrument: Option<Instrument>) {
    match element {
        StreamElement::Note(note) => note.set_stored_instrument(instrument),
        StreamElement::Chord(chord) => chord.set_stored_instrument(instrument),
        StreamElement::Unpitched(stroke) => stroke.written_mut().set_stored_instrument(instrument),
        StreamElement::PercussionChord(chord) => {
            chord.written_mut().set_stored_instrument(instrument);
        }
        _ => {}
    }
}

/// For each of a stream's leaves, whether the stream holds it itself rather
/// than through a stream nested in it.
fn held_directly(stream: &Stream) -> Vec<bool> {
    let mut direct = Vec::new();
    for event in stream.events() {
        match event.element() {
            StreamElement::Stream(inner) => {
                direct.extend(std::iter::repeat_n(false, inner.leaves().len()));
            }
            _ => direct.push(true),
        }
    }
    direct
}

/// Puts the instrument each note, chord or stroke of a stream keeps as its
/// own into the stream beside it, at its offset: music21's
/// `unbundleInstruments`.
///
/// Only what the stream holds itself is looked at, not what a measure or a
/// voice in it holds. The notes keep their instruments too.
///
/// ```
/// use music21_rs::{instrument::unbundle_instruments, Instrument, Stream, StreamElement, Unpitched};
///
/// let mut stream = Stream::new();
/// for (offset, kind) in [(0.0, "BassDrum"), (1.0, "Cowbell")] {
///     let mut stroke = Unpitched::new();
///     stroke.written_mut().set_stored_instrument(Some(Instrument::of_kind(kind)?));
///     stream.insert(offset, stroke);
/// }
/// unbundle_instruments(&mut stream);
/// let said: Vec<String> = stream
///     .events()
///     .iter()
///     .map(|event| match event.element() {
///         StreamElement::Instrument(instrument) => format!("{} {}", event.offset(), instrument.kind()),
///         _ => format!("{} stroke", event.offset()),
///     })
///     .collect();
/// assert_eq!(said, ["0 BassDrum", "0 stroke", "1 Cowbell", "1 stroke"]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
pub fn unbundle_instruments(stream: &mut Stream) {
    let added: Vec<StreamEvent> = stream
        .events()
        .iter()
        .filter_map(|event| {
            stored(event.element())
                .map(|instrument| StreamEvent::new(event.offset(), instrument.clone()))
        })
        .collect();
    stream.insert_sorted(added);
}

/// Gives each note, chord or stroke of a stream the instrument standing
/// last before it as its own, and takes the instruments out of the stream:
/// music21's `bundleInstruments`, which undoes
/// [`unbundle_instruments`].
///
/// Only what the stream holds itself is looked at. A note with no
/// instrument before it is left with none of its own, whatever it had.
pub fn bundle_instruments(stream: &mut Stream) {
    stream.insert_sorted(Vec::new());
    let direct = held_directly(stream);
    let mut last: Option<Instrument> = None;
    stream.retain_leaves(&mut |position, element| {
        if !direct[position] {
            return true;
        }
        if let StreamElement::Instrument(instrument) = element {
            last = Some(instrument.as_ref().clone());
            return false;
        }
        if matches!(
            element,
            StreamElement::Note(_)
                | StreamElement::Chord(_)
                | StreamElement::Unpitched(_)
                | StreamElement::PercussionChord(_)
        ) {
            set_stored(element, last.clone());
        }
        true
    });
}

/// What [`deduplicate`] makes of the instruments one part holds, each
/// beside its offset in the part, in order: for each, nothing where it is
/// left as it is, else what it is to stand as, or nothing where it goes.
/// Instruments are settled with the others at their offset only.
pub(crate) fn settle_by_offset(
    held: &[(FloatType, &Instrument)],
) -> Vec<Option<Option<Instrument>>> {
    let mut plan: Vec<Option<Option<Instrument>>> = vec![None; held.len()];
    let mut offsets: Vec<FloatType> = Vec::new();
    for (offset, _) in held {
        if !offsets.contains(offset) {
            offsets.push(*offset);
        }
    }
    for offset in offsets {
        let places: Vec<usize> = (0..held.len())
            .filter(|&index| held[index].0 == offset)
            .collect();
        let together: Vec<&Instrument> = places.iter().map(|&index| held[index].1).collect();
        if let Some(settled) = settle(&together) {
            for (index, becomes) in places.into_iter().zip(settled) {
                plan[index] = Some(becomes);
            }
        }
    }
    plan
}

/// What [`deduplicate`] makes of the instruments standing together, in
/// order: each instrument as it is to stand, or nothing where it goes.
/// Nothing at all where they are left as they are.
///
/// Where the instruments disagree on the part's name or on their own, they
/// are left alone. Otherwise every instrument kept takes the one name each
/// of them gives, and of instruments all of one kind only the first is
/// kept, while of instruments of several kinds the bare `Instrument`s go.
pub(crate) fn settle(instruments: &[&Instrument]) -> Option<Vec<Option<Instrument>>> {
    if instruments.len() <= 1 {
        return None;
    }
    let mut part_names: Vec<&str> = instruments
        .iter()
        .filter_map(|instrument| instrument.part_name())
        .collect();
    part_names.sort_unstable();
    part_names.dedup();
    let mut names: Vec<&str> = instruments
        .iter()
        .filter_map(|instrument| instrument.name())
        .collect();
    names.sort_unstable();
    names.dedup();
    if part_names.len() > 1 || names.len() > 1 {
        return None;
    }
    let part_name = part_names.first().map(|name| name.to_string());
    let name = names.first().map(|name| name.to_string());
    let mut kinds: Vec<&str> = instruments
        .iter()
        .map(|instrument| instrument.kind())
        .collect();
    kinds.sort_unstable();
    kinds.dedup();
    let one_kind = kinds.len() == 1;
    Some(
        instruments
            .iter()
            .enumerate()
            .map(|(index, instrument)| {
                let goes = if one_kind {
                    index > 0
                } else {
                    instrument.kind() == "Instrument"
                };
                (!goes).then(|| {
                    let mut named = (*instrument).clone();
                    named.set_part_name(part_name.clone());
                    named.set_name(name.clone());
                    named
                })
            })
            .collect(),
    )
}

/// Folds together the instruments a part says more than once: music21's
/// `instrument.deduplicate`.
///
/// Each part of a score is done on its own, or the stream itself where it
/// holds no parts, and the instruments standing at one offset of it, in a
/// measure or the part itself, are settled together. Where they agree on
/// the part's name and on their own, or say nothing, all of them take those
/// names; then of instruments all of one kind only the first is kept, and of
/// instruments of several kinds the bare `Instrument`s go.
///
/// ```
/// use music21_rs::{instrument::deduplicate, Instrument, Stream, StreamElement};
///
/// let mut named = Instrument::new();
/// named.set_name(Some("Semi-Hollow Body".to_string()));
/// let mut guitar = Instrument::new();
/// guitar.set_part_name(Some("Electric Guitar".to_string()));
/// let mut stream = Stream::new();
/// stream.insert(4.0, named);
/// stream.insert(4.0, guitar);
/// deduplicate(&mut stream);
/// let [event] = stream.events() else { panic!("one instrument is left") };
/// let StreamElement::Instrument(left) = event.element() else { panic!() };
/// assert_eq!(left.part_name(), Some("Electric Guitar"));
/// assert_eq!(left.name(), Some("Semi-Hollow Body"));
/// # Ok::<(), music21_rs::Error>(())
/// ```
pub fn deduplicate(stream: &mut Stream) {
    // Each leaf's part, for the leaves in one.
    let mut part_of: Vec<Option<usize>> = Vec::new();
    if stream.has_part_like_streams() {
        for (index, event) in stream.events().iter().enumerate() {
            match event.element() {
                StreamElement::Stream(inner) => {
                    part_of.extend(std::iter::repeat_n(Some(index), inner.leaves().len()));
                }
                _ => part_of.push(None),
            }
        }
    } else {
        part_of = vec![Some(0); stream.leaves().len()];
    }

    let leaves = stream.leaves();
    let mut plan: Vec<Option<Option<Instrument>>> = vec![None; leaves.len()];
    let mut parts: Vec<usize> = part_of.iter().flatten().copied().collect();
    parts.dedup();
    for part in parts {
        let places: Vec<usize> = (0..leaves.len())
            .filter(|&position| {
                part_of[position] == Some(part)
                    && matches!(leaves[position].1, StreamElement::Instrument(_))
            })
            .collect();
        let held: Vec<(FloatType, &Instrument)> = places
            .iter()
            .filter_map(|&position| match leaves[position].1 {
                StreamElement::Instrument(instrument) => {
                    Some((leaves[position].0, instrument.as_ref()))
                }
                _ => None,
            })
            .collect();
        for (position, becomes) in places.into_iter().zip(settle_by_offset(&held)) {
            plan[position] = becomes;
        }
    }
    stream.retain_leaves(&mut |position, element| match plan[position].take() {
        None => true,
        Some(None) => false,
        Some(Some(becomes)) => {
            *element = becomes.into();
            true
        }
    });
}

/// Splits a score into a part for each instrument playing in it, each part
/// gathering what is played on that instrument from every part of the
/// score: music21's `partitionByInstrument`.
///
/// Each part of the score is flattened, or the stream itself where it holds
/// none. An instrument is in force from where it stands until the next one
/// in its part, the last until the part ends, and whatever starts while it
/// is in force goes to the part for its name; instruments with one name
/// share a part, the first of them standing at its start, and a part is
/// named by its instrument's name. What a part plays under two instruments
/// of one name is gathered once. A stream with no instrument in it comes
/// back as a score of its parts flattened.
///
/// The parts come out as music21's do, with no measures; spanners are not
/// carried across, as the places they name move.
///
/// ```
/// use music21_rs::{instrument::partition_by_instrument, Instrument, Note, Stream};
///
/// let mut part = Stream::new();
/// part.insert(0.0, Instrument::of_kind("Piccolo")?);
/// part.insert(0.0, Note::from_name("C4")?);
/// part.insert(1.0, Instrument::of_kind("Trombone")?);
/// part.insert(1.0, Note::from_name("D4")?);
/// part.insert(2.0, Note::from_name("E4")?);
/// let score = partition_by_instrument(&part);
/// let names: Vec<Option<&str>> = score.parts().iter().map(|part| part.id()).collect();
/// assert_eq!(names, [Some("Piccolo"), Some("Trombone")]);
/// assert_eq!(score.parts()[1].notes().len(), 2);
/// # Ok::<(), music21_rs::Error>(())
/// ```
pub fn partition_by_instrument(stream: &Stream) -> Stream {
    let flattened: Vec<Stream> = if stream.has_part_like_streams() {
        stream
            .events()
            .iter()
            .filter_map(|event| event.element().as_stream())
            .map(Stream::flatten)
            .collect()
    } else {
        vec![stream.flatten()]
    };
    let flattened: Vec<Stream> = flattened
        .into_iter()
        .map(|part| {
            let events = sorted_events(part.events().to_vec());
            part.with_events(events)
        })
        .collect();

    // One part for each name, in the order the names are met, holding the
    // first instrument of that name -- except that an instrument with no
    // name at all takes the place of the one before it, as music21's
    // `names[None or '']` does.
    let mut names: Vec<(String, Instrument)> = Vec::new();
    for part in &flattened {
        for event in part.events() {
            let StreamElement::Instrument(instrument) = event.element() else {
                continue;
            };
            let key = instrument.name().unwrap_or("").to_string();
            match names.iter_mut().find(|(name, _)| *name == key) {
                Some((_, held)) if instrument.name().is_none() => {
                    *held = instrument.as_ref().clone();
                }
                Some(_) => {}
                None => names.push((key, instrument.as_ref().clone())),
            }
        }
    }
    if names.is_empty() {
        let mut score = Stream::with_kind(StreamKind::Score);
        for part in flattened {
            score.insert(0.0, part);
        }
        return score;
    }

    let mut gathered: Vec<Vec<StreamEvent>> = names
        .iter()
        .map(|(_, instrument)| vec![StreamEvent::new(0.0, instrument.clone())])
        .collect();
    // What each new part already holds, as the part and event it came from.
    let mut taken: Vec<Vec<(usize, usize)>> = vec![Vec::new(); names.len()];
    for (source, part) in flattened.iter().enumerate() {
        let events = part.events();
        let end = part.end_offset();
        let starts: Vec<(usize, FloatType)> = events
            .iter()
            .enumerate()
            .filter(|(_, event)| matches!(event.element(), StreamElement::Instrument(_)))
            .map(|(index, event)| (index, event.offset()))
            .collect();
        for (which, &(index, start)) in starts.iter().enumerate() {
            let StreamElement::Instrument(instrument) = events[index].element() else {
                continue;
            };
            let stop = starts.get(which + 1).map_or(end, |next| next.1);
            let key = instrument.name().unwrap_or("");
            let Some(target) = names.iter().position(|(name, _)| name == key) else {
                continue;
            };
            for (place, event) in events.iter().enumerate() {
                if matches!(event.element(), StreamElement::Instrument(_))
                    || !in_span(event, start, stop)
                    || taken[target].contains(&(source, place))
                {
                    continue;
                }
                taken[target].push((source, place));
                gathered[target].push(event.clone());
            }
        }
    }

    let mut score = Stream::with_kind(StreamKind::Score);
    for ((name, _), events) in names.into_iter().zip(gathered) {
        let mut part = Stream::with_kind(StreamKind::Part).with_events(sorted_events(events));
        part.set_id(Some(name));
        score.insert(0.0, part);
    }
    score
}

/// Whether an event starts inside an instrument's span: music21's
/// `getElementsByOffset` with `mustBeginInSpan`, not counting what starts at
/// the end, except that a span of no length takes in what lasts no time at
/// its start.
fn in_span(event: &StreamEvent, start: FloatType, stop: FloatType) -> bool {
    let offset = event.offset();
    if stop <= start {
        return offset == start && event.element().quarter_length() == 0.0;
    }
    offset >= start && offset < stop
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Note;

    #[test]
    fn bundling_puts_back_what_unbundling_took_out() {
        let mut stream = Stream::new();
        let mut first = Note::from_name("C4").unwrap();
        first.set_stored_instrument(Some(Instrument::of_kind("BassDrum").unwrap()));
        stream.insert(0.0, first);
        stream.insert(1.0, Note::from_name("D4").unwrap());
        let mut last = Note::from_name("E4").unwrap();
        last.set_stored_instrument(Some(Instrument::of_kind("Cowbell").unwrap()));
        stream.insert(2.0, last);

        unbundle_instruments(&mut stream);
        assert_eq!(stream.len(), 5);
        bundle_instruments(&mut stream);
        let kinds: Vec<Option<String>> = stream
            .events()
            .iter()
            .map(|event| stored(event.element()).map(|held| held.kind().to_string()))
            .collect();
        // music21's own example: the note between takes the bass drum.
        assert_eq!(
            kinds,
            [
                Some("BassDrum".to_string()),
                Some("BassDrum".to_string()),
                Some("Cowbell".to_string())
            ]
        );
    }

    #[test]
    fn a_spanner_follows_its_notes_when_instruments_come_and_go() {
        let mut stream = Stream::new();
        let mut note = Note::from_name("C4").unwrap();
        note.set_stored_instrument(Some(Instrument::of_kind("Violin").unwrap()));
        stream.insert(0.0, note);
        stream.insert(1.0, Note::from_name("D4").unwrap());
        stream.add_spanner(crate::Spanner::slur(vec![0, 1]));
        unbundle_instruments(&mut stream);
        assert_eq!(stream.spanners()[0].spanned(), [Some(1), Some(2)]);
        bundle_instruments(&mut stream);
        assert_eq!(stream.spanners()[0].spanned(), [Some(0), Some(1)]);
    }

    #[test]
    fn a_stream_with_no_instrument_is_its_parts_flattened() {
        let mut part = Stream::with_kind(StreamKind::Part);
        part.insert(0.0, Note::from_name("C4").unwrap());
        let mut score = Stream::with_kind(StreamKind::Score);
        score.insert(0.0, part);
        let partitioned = partition_by_instrument(&score);
        assert_eq!(partitioned.parts().len(), 1);
        assert_eq!(partitioned.parts()[0].notes().len(), 1);
    }
}
