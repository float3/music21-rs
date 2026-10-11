//! Changing a stream as music21's editing methods change it: ties carried
//! on, offsets and lengths scaled, notes inverted, elements shifted,
//! repeated or put in with room made for them, and a pickup padded.

use super::exact::{Exact, at_offset};
use super::{Stream, StreamElement, StreamEvent};
use crate::defaults::{FloatType, IntegerType};
use crate::duration::Duration;
use crate::error::{Error, Result};
use crate::key::KeySignature;
use crate::makenotation::{event_order, op_frac};
use crate::meter::{TimeSignature, best_time_signature};
use crate::notation::{Tie, TieType};
use crate::pitch::Pitch;

fn stream_error(message: impl Into<String>) -> Error {
    Error::Stream(message.into())
}

/// Where [`Stream::scale_offsets`] holds still while it scales: music21's
/// `anchorZero`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    /// The first element stays where it is: `'lowest'`.
    Lowest,
    /// The last element stays where it is: `'highest'`.
    Highest,
    /// Offset nought stays where it is: `None`.
    Zero,
}

/// Whether a flattened element sorts before another, as music21's
/// `flatten` sorts them: by offset, by class, grace notes first.
fn flat_order(
    a: (FloatType, &StreamElement),
    b: (FloatType, &StreamElement),
) -> std::cmp::Ordering {
    let grace = |element: &StreamElement| element.duration().is_some_and(Duration::is_grace);
    a.0.total_cmp(&b.0)
        .then(a.1.class_sort_order().cmp(&b.1.class_sort_order()))
        .then(grace(b.1).cmp(&grace(a.1)))
}

/// One pitch a note or chord sounds, as [`Stream::extend_ties`] ties it:
/// the leaf it is in, which note of a chord, and the tie it has now.
#[derive(Clone, Debug)]
struct Sounded {
    leaf: usize,
    note: usize,
    name: String,
    tie: Option<Tie>,
    /// Whether the tie can be kept: a chord symbol's notes are not.
    kept: bool,
}

fn sounded(leaf: usize, element: &StreamElement) -> Option<Vec<Sounded>> {
    let of = |note: usize, pitch: &Pitch, tie: Option<&Tie>, kept: bool| Sounded {
        leaf,
        note,
        name: pitch.name_with_octave(),
        tie: tie.cloned(),
        kept,
    };
    Some(match element {
        StreamElement::Note(note) => vec![of(0, note.pitch(), note.tie(), true)],
        StreamElement::Chord(chord) => chord
            .notes()
            .iter()
            .enumerate()
            .map(|(index, note)| of(index, note.pitch(), note.tie(), true))
            .collect(),
        StreamElement::ChordSymbol(symbol) => symbol
            .pitches()
            .unwrap_or_default()
            .iter()
            .enumerate()
            .map(|(index, pitch)| of(index, pitch, None, false))
            .collect(),
        _ => return None,
    })
}

impl Stream {
    /// Ties each note to a note of the same name and octave that starts
    /// where it ends, a chord's notes each to its own, as music21's
    /// `extendTies` does. With `ignore_rests`, a note is tied to the next
    /// note whatever lies between.
    ///
    /// ```
    /// use music21_rs::tinynotation::from_tiny_notation;
    ///
    /// let mut line = from_tiny_notation("4/4 c4 c4 d4 d4")?;
    /// line.extend_ties(false);
    /// let ties: Vec<_> = line
    ///     .notes()
    ///     .iter()
    ///     .map(|(_, note)| match note {
    ///         music21_rs::StreamElement::Note(note) => note.tie().map(|tie| tie.tie_type()),
    ///         _ => None,
    ///     })
    ///     .collect();
    /// assert_eq!(ties[0], Some(music21_rs::notation::TieType::Start));
    /// assert_eq!(ties[1], Some(music21_rs::notation::TieType::Stop));
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    pub fn extend_ties(&mut self, ignore_rests: bool) {
        // music21's `flatten().notes`: the notes and chords, flattened and
        // sorted, each with the leaf it is.
        let leaves = self.leaves();
        let mut flat: Vec<(usize, FloatType, &StreamElement)> = leaves
            .iter()
            .enumerate()
            .filter(|(_, (_, element))| element.is_instance("NotRest"))
            .map(|(leaf, (offset, element))| (leaf, *offset, *element))
            .collect();
        flat.sort_by(|a, b| flat_order((a.1, a.2), (b.1, b.2)));
        let mut pitches: Vec<Option<Vec<Sounded>>> = flat
            .iter()
            .map(|(leaf, _, element)| sounded(*leaf, element))
            .collect();
        for i in 0..flat.len() {
            if pitches[i].is_none() || i + 1 == flat.len() {
                continue;
            }
            // Where music21 looks: the note's offset and length added as
            // Python adds them, or with `ignore_rests` the next note's.
            let target = if ignore_rests {
                Exact::held(flat[i + 1].1)
            } else {
                Exact::held(flat[i].1).plus(Exact::held(flat[i].2.quarter_length()))
            };
            let connections: Vec<usize> = (0..flat.len())
                .filter(|&j| at_offset(flat[j].1, flat[j].2.quarter_length(), target))
                .collect();
            let sources = pitches[i].as_ref().map_or(0, Vec::len);
            for p in 0..sources {
                for &m in &connections {
                    let count = pitches[m].as_ref().map_or(0, Vec::len);
                    for q in 0..count {
                        let (ours, theirs) = match (&pitches[i], &pitches[m]) {
                            (Some(ours), Some(theirs)) => (&ours[p].name, &theirs[q].name),
                            _ => continue,
                        };
                        if ours != theirs {
                            continue;
                        }
                        if let Some(sources) = pitches[i].as_mut() {
                            let source = &mut sources[p];
                            match source.tie.as_mut() {
                                None => source.tie = Some(Tie::new(TieType::Start)),
                                Some(tie) if tie.tie_type() == TieType::Stop => {
                                    tie.set_tie_type(TieType::Continue);
                                }
                                Some(_) => {}
                            }
                        }
                        if let Some(targets) = pitches[m].as_mut() {
                            targets[q].tie = Some(Tie::new(TieType::Stop));
                        }
                        break;
                    }
                }
            }
        }
        let mut ties: std::collections::HashMap<(usize, usize), Option<Tie>> =
            std::collections::HashMap::new();
        for list in pitches.into_iter().flatten() {
            for sounded in list.into_iter().filter(|sounded| sounded.kept) {
                ties.insert((sounded.leaf, sounded.note), sounded.tie);
            }
        }
        let mut leaf = 0;
        self.for_each_mut(&mut |_, element| {
            match element {
                StreamElement::Note(note) => {
                    if let Some(tie) = ties.get(&(leaf, 0)) {
                        note.set_tie(tie.clone());
                    }
                }
                StreamElement::Chord(chord) => {
                    for (index, note) in chord.notes_mut().iter_mut().enumerate() {
                        if let Some(tie) = ties.get(&(leaf, index)) {
                            note.set_tie(tie.clone());
                        }
                    }
                }
                _ => {}
            }
            leaf += 1;
        });
    }

    /// A copy of the stream with each element's offset scaled by `amount`
    /// from the anchor, and those of the streams it holds from theirs, as
    /// music21's `scaleOffsets` does.
    ///
    /// # Errors
    ///
    /// An amount of nought or less.
    pub fn scale_offsets(
        &self,
        amount: FloatType,
        anchor: Anchor,
        nested: Anchor,
    ) -> Result<Stream> {
        if amount.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
            return Err(stream_error("amountToScale must be greater than zero"));
        }
        let mut out = self.clone();
        out.scale_offsets_in_place(amount, anchor, nested);
        Ok(out)
    }

    fn scale_offsets_in_place(&mut self, amount: FloatType, anchor: Anchor, nested: Anchor) {
        let shift = match anchor {
            Anchor::Lowest => self.lowest_offset(),
            Anchor::Highest => self.highest_offset(),
            Anchor::Zero => 0.0,
        };
        for event in self.events_mut() {
            let offset = op_frac((event.offset() - shift) * amount + shift);
            let mut element = event.element().clone();
            if let StreamElement::Stream(inner) = &mut element {
                inner.scale_offsets_in_place(amount, nested, nested);
            }
            *event = StreamEvent::new(offset, element);
        }
    }

    /// A copy of the stream with every length scaled by `amount`, those in
    /// the streams it holds too: music21's `scaleDurations`.
    ///
    /// # Errors
    ///
    /// An amount of nought or less, or a length no duration has.
    pub fn scale_durations(&self, amount: FloatType) -> Result<Stream> {
        if amount.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
            return Err(stream_error("amountToScale must be greater than zero"));
        }
        let mut out = self.clone();
        let mut failed = None;
        out.for_each_mut(&mut |_, element| {
            if failed.is_some() {
                return;
            }
            if let Err(error) = scale_duration(element, amount) {
                failed = Some(error);
            }
        });
        failed.map_or(Ok(out), Err)
    }

    /// A copy of the stream with its offsets scaled from the first element
    /// and its lengths scaled, by `amount`: music21's
    /// `augmentOrDiminish`.
    ///
    /// ```
    /// use music21_rs::tinynotation::from_tiny_notation;
    ///
    /// let line = from_tiny_notation("4/4 c4 d8 e8")?;
    /// let longer = line.augment_or_diminish(2.0)?;
    /// assert_eq!(longer.notes()[2].0, 3.0);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// As [`Stream::scale_durations`].
    pub fn augment_or_diminish(&self, amount: FloatType) -> Result<Stream> {
        self.scale_offsets(amount, Anchor::Lowest, Anchor::Zero)?
            .scale_durations(amount)
    }

    /// A copy of the stream with each note turned upside down about
    /// `inversion` (C4 where none is given) by step, its accidental that of
    /// the key -- the stream's one key signature, the one in force at the
    /// note where it has several, C major where it has none: music21's
    /// `invertDiatonic`. A chord's notes are left as they are.
    ///
    /// # Errors
    ///
    /// A note turned past the steps a pitch can stand on.
    pub fn invert_diatonic(&self, inversion: Option<&Pitch>) -> Result<Stream> {
        let inversion = match inversion {
            Some(pitch) => pitch.diatonic_note_number(),
            None => Pitch::from_name("C4")?.diatonic_note_number(),
        };
        let signatures: Vec<KeySignature> = self
            .recurse()
            .into_iter()
            .filter_map(|(_, element)| signature_of(element))
            .collect();
        let only = match signatures.len() {
            0 => Some(KeySignature::new(0)),
            1 => signatures.into_iter().next(),
            _ => None,
        };
        // The key signature in force at each note, in the order music21
        // finds them.
        let leaves = self.leaves();
        let mut order: Vec<usize> = (0..leaves.len()).collect();
        order.sort_by(|a, b| flat_order(leaves[*a], leaves[*b]));
        let mut in_force: std::collections::HashMap<usize, KeySignature> =
            std::collections::HashMap::new();
        let mut current: Option<KeySignature> = None;
        for leaf in order {
            if let Some(signature) = signature_of(leaves[leaf].1) {
                current = Some(signature);
            }
            if let Some(signature) = &current {
                in_force.insert(leaf, signature.clone());
            }
        }
        let mut out = self.clone();
        let mut leaf = 0;
        let mut failed = None;
        out.for_each_mut(&mut |_, element| {
            if let StreamElement::Note(note) = element
                && failed.is_none()
            {
                let signature = only.as_ref().or_else(|| in_force.get(&leaf));
                if let Err(error) = invert_note(note, inversion, signature) {
                    failed = Some(error);
                }
            }
            leaf += 1;
        });
        failed.map_or(Ok(out), Err)
    }

    /// Moves the elements this stream holds itself, those starting at or
    /// after `start` and before `end` where given, by `offset`: music21's
    /// `shiftElements`.
    pub fn shift_elements(
        &mut self,
        offset: FloatType,
        start: Option<FloatType>,
        end: Option<FloatType>,
    ) {
        let events: Vec<(StreamEvent, Option<usize>)> = self
            .events()
            .iter()
            .enumerate()
            .map(|(index, event)| {
                let at = event.offset();
                let moves = start.is_none_or(|start| at >= start) && end.is_none_or(|end| at < end);
                let offset = if moves { op_frac(at + offset) } else { at };
                (
                    StreamEvent::new(offset, event.element().clone()),
                    Some(index),
                )
            })
            .collect();
        self.reorder_events(events);
    }

    /// Puts the events in place of this stream's own in music21's order,
    /// those that sort alike in the order given.
    fn reorder_events(&mut self, mut events: Vec<(StreamEvent, Option<usize>)>) {
        events.sort_by(|a, b| event_order(&a.0, &b.0));
        self.replace_events(events);
    }

    /// Appends a copy of `element` this many times, each after the last:
    /// music21's `repeatAppend`.
    pub fn repeat_append(&mut self, element: &StreamElement, times: usize) {
        for _ in 0..times {
            let offset = self.end_offset();
            self.insert_sorted(vec![StreamEvent::new(offset, element.clone())]);
        }
    }

    /// Puts a copy of `element` at each of `offsets`: music21's
    /// `repeatInsert`.
    pub fn repeat_insert(&mut self, element: &StreamElement, offsets: &[FloatType]) {
        self.insert_sorted(
            offsets
                .iter()
                .map(|offset| StreamEvent::new(op_frac(*offset), element.clone()))
                .collect(),
        );
    }

    /// Puts `element` at `offset`, first moving everything from the
    /// element nearest after that offset onwards far enough that it starts
    /// where the new element ends: music21's `insertAndShift`.
    pub fn insert_and_shift(&mut self, offset: FloatType, element: StreamElement) {
        let end = offset + element.quarter_length();
        let nearest = self
            .events()
            .iter()
            .map(StreamEvent::offset)
            .filter(|at| at - offset >= 0.0)
            .fold(None, |best: Option<FloatType>, at| {
                Some(best.map_or(at, |best| {
                    if at - offset < best - offset {
                        at
                    } else {
                        best
                    }
                }))
            });
        let shift = nearest.map_or(0.0, |nearest| end - nearest);
        if shift > 0.0 {
            let events: Vec<(StreamEvent, Option<usize>)> = self
                .events()
                .iter()
                .enumerate()
                .map(|(index, event)| {
                    let at = event.offset();
                    let moved = if at - offset >= 0.0 {
                        op_frac(at + shift)
                    } else {
                        at
                    };
                    (
                        StreamEvent::new(moved, event.element().clone()),
                        Some(index),
                    )
                })
                .collect();
            self.reorder_events(events);
        }
        self.insert_sorted(vec![StreamEvent::new(op_frac(offset), element)]);
    }

    /// How much of its bar a measure fills, as a proportion: music21's
    /// `barDurationProportion`. The bar is that of the measure's own time
    /// signature, or of `meter` where it has none; failing both, of the time
    /// signature that best fits its notes
    /// ([`best_time_signature`](crate::meter::best_time_signature)), and
    /// failing that the measure's own length.
    ///
    /// # Errors
    ///
    /// A time signature whose bar cannot be worked out, or a bar of no
    /// length: an empty measure with no time signature.
    pub fn bar_duration_proportion(&self, meter: Option<&TimeSignature>) -> Result<FloatType> {
        let bar = self.bar_length(meter)?;
        self.proportion_of(bar)
    }

    fn proportion_of(&self, bar: FloatType) -> Result<FloatType> {
        if bar == 0.0 {
            return Err(stream_error("a bar of no length"));
        }
        Ok(op_frac(self.end_offset() / bar))
    }

    fn bar_length(&self, meter: Option<&TimeSignature>) -> Result<FloatType> {
        let own = self
            .events()
            .iter()
            .take_while(|event| event.offset() == 0.0)
            .find_map(|event| match event.element() {
                StreamElement::TimeSignature(found) => Some(found.clone()),
                _ => None,
            });
        let meter = match (own, meter) {
            (Some(own), _) => own,
            (None, Some(given)) => given.clone(),
            (None, None) => match best_time_signature(self) {
                Ok(best) => best,
                Err(_) => return Ok(self.end_offset()),
            },
        };
        Ok(meter.bar_quarter_length())
    }

    /// Pads a measure that does not fill its bar on the left by what it
    /// lacks, as a pickup: music21's `padAsAnacrusis`. With
    /// `use_initial_rests`, the rests it starts with are taken out first
    /// and what follows moved back. The bar is as
    /// [`Stream::bar_duration_proportion`] has it.
    ///
    /// # Errors
    ///
    /// A time signature whose bar cannot be worked out.
    pub fn pad_as_anacrusis(
        &mut self,
        meter: Option<&TimeSignature>,
        use_initial_rests: bool,
    ) -> Result<()> {
        if use_initial_rests {
            let rests: Vec<usize> = self
                .events()
                .iter()
                .enumerate()
                .filter(|(_, event)| event.element().is_instance("GeneralNote"))
                .take_while(|(_, event)| matches!(event.element(), StreamElement::Rest(_)))
                .map(|(index, _)| index)
                .collect();
            if !rests.is_empty() {
                self.remove_shifting(&rests);
            }
        }
        let bar = self.bar_length(meter)?;
        let proportion = self.proportion_of(bar)?;
        if proportion < 1.0 {
            self.set_padding_left(op_frac(bar * (1.0 - proportion)));
        }
        Ok(())
    }

    /// Takes out these events, in order, each moving what follows it up to
    /// the next back by all that has been taken out: music21's `remove`
    /// with `shiftOffsets`.
    fn remove_shifting(&mut self, indices: &[usize]) {
        let mut offsets: Vec<FloatType> = self.events().iter().map(StreamEvent::offset).collect();
        let mut gone = vec![false; offsets.len()];
        let mut shift = 0.0;
        for (i, &index) in indices.iter().enumerate() {
            gone[index] = true;
            let length = self.events()[index].element().quarter_length();
            let region_start = offsets[index] + length;
            let region_end = match indices.get(i + 1) {
                Some(&next) => offsets[next],
                None => self
                    .events()
                    .iter()
                    .zip(&offsets)
                    .zip(&gone)
                    .filter(|(_, gone)| !**gone)
                    .map(|((event, offset), _)| op_frac(offset + event.element().quarter_length()))
                    .fold(0.0, FloatType::max),
            };
            shift += length;
            if shift != 0.0 {
                let span = super::queries::OffsetSpan {
                    include_end_boundary: false,
                    ..super::queries::OffsetSpan::new(region_start, Some(region_end))
                };
                for (index, at) in offsets.iter_mut().enumerate() {
                    if gone[index] {
                        continue;
                    }
                    if span.holds(*at, self.events[index].element().quarter_length()) {
                        *at = op_frac(*at - shift);
                    }
                }
            }
        }
        let events: Vec<(StreamEvent, Option<usize>)> = self
            .events()
            .iter()
            .enumerate()
            .filter(|(index, _)| !gone[*index])
            .map(|(index, event)| {
                (
                    StreamEvent::new(offsets[index], event.element().clone()),
                    Some(index),
                )
            })
            .collect();
        self.reorder_events(events);
    }
}

fn signature_of(element: &StreamElement) -> Option<KeySignature> {
    match element {
        StreamElement::KeySignature(signature) => Some(signature.clone()),
        StreamElement::Key(key) => Some(key.key_signature()),
        _ => None,
    }
}

fn invert_note(
    note: &mut crate::note::Note,
    inversion: IntegerType,
    signature: Option<&KeySignature>,
) -> Result<()> {
    let mut pitch = note.pitch().clone();
    let number = 2 * inversion - pitch.diatonic_note_number();
    pitch.set_diatonic_note_number(number)?;
    if let Some(signature) = signature {
        let step = pitch.name().chars().next().unwrap_or('C');
        let mut accidental = signature.accidental_by_step(step)?;
        if let Some(accidental) = accidental.as_mut() {
            accidental.set_display_status(None);
        }
        pitch.set_written_accidental(accidental);
    }
    note.set_pitch(pitch);
    Ok(())
}

/// Scales an element's length, as music21's `augmentOrDiminish` of its
/// duration.
fn scale_duration(element: &mut StreamElement, amount: FloatType) -> Result<()> {
    match element {
        StreamElement::Note(note) => {
            if let Some(duration) = note.duration() {
                note.set_duration(duration.augment_or_diminish(amount)?);
            }
        }
        StreamElement::Chord(chord) => {
            if let Some(duration) = chord.duration() {
                chord.set_duration(duration.augment_or_diminish(amount)?);
            }
        }
        StreamElement::Rest(rest) => {
            let duration = rest.duration().augment_or_diminish(amount)?;
            rest.set_duration(duration);
        }
        StreamElement::Unpitched(stroke) => {
            if let Some(duration) = stroke.written().duration() {
                let scaled = duration.augment_or_diminish(amount)?;
                stroke.written_mut().set_duration(scaled);
            }
        }
        StreamElement::PercussionChord(chord) => {
            if let Some(duration) = chord.written().duration() {
                let scaled = duration.augment_or_diminish(amount)?;
                chord.written_mut().set_duration(scaled);
            }
        }
        StreamElement::ChordSymbol(symbol) => {
            let scaled = symbol.duration().augment_or_diminish(amount)?;
            symbol.set_duration(scaled);
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note::Note;
    use crate::rest::Rest;
    use crate::stream::StreamKind;

    fn note(name: &str, quarter_length: FloatType) -> Result<StreamElement> {
        Ok(Note::from_name(name)?
            .with_duration(Duration::new(quarter_length)?)
            .into())
    }

    fn placed(stream: &Stream) -> Vec<(String, FloatType)> {
        stream
            .events()
            .iter()
            .filter_map(|event| match event.element() {
                StreamElement::Note(note) => {
                    Some((note.pitch().name_with_octave(), event.offset()))
                }
                StreamElement::Rest(_) => Some(("rest".to_string(), event.offset())),
                _ => None,
            })
            .collect()
    }

    fn named(pairs: &[(&str, FloatType)]) -> Vec<(String, FloatType)> {
        pairs
            .iter()
            .map(|(name, offset)| ((*name).to_string(), *offset))
            .collect()
    }

    // The expected values are music21's, from the same streams.

    #[test]
    fn insert_and_shift_makes_room() -> Result<()> {
        let mut stream = Stream::new();
        stream.insert(0.0, note("C4", 2.0)?);
        stream.insert(2.0, note("D4", 2.0)?);
        stream.insert_and_shift(1.0, note("E4", 2.0)?);
        assert_eq!(
            placed(&stream),
            named(&[("C4", 0.0), ("E4", 1.0), ("D4", 3.0)])
        );
        Ok(())
    }

    #[test]
    fn shift_elements_moves_a_range() -> Result<()> {
        let mut stream = Stream::new();
        for (offset, name) in ["C4", "D4", "E4", "F4"].into_iter().enumerate() {
            stream.insert(offset as FloatType, note(name, 1.0)?);
        }
        stream.shift_elements(1.0, Some(1.0), Some(3.0));
        assert_eq!(
            placed(&stream),
            named(&[("C4", 0.0), ("D4", 2.0), ("E4", 3.0), ("F4", 3.0)])
        );
        Ok(())
    }

    #[test]
    fn scale_offsets_holds_its_anchor() -> Result<()> {
        let mut stream = Stream::new();
        for (offset, name) in ["C4", "D4", "E4"].into_iter().enumerate() {
            stream.insert(offset as FloatType + 1.0, note(name, 1.0)?);
        }
        let high = stream.scale_offsets(2.0, Anchor::Highest, Anchor::Zero)?;
        assert_eq!(
            placed(&high),
            named(&[("C4", -1.0), ("D4", 1.0), ("E4", 3.0)])
        );
        let zero = stream.scale_offsets(2.0, Anchor::Zero, Anchor::Zero)?;
        assert_eq!(
            placed(&zero),
            named(&[("C4", 2.0), ("D4", 4.0), ("E4", 6.0)])
        );
        Ok(())
    }

    #[test]
    fn a_pickup_is_padded_to_its_bar() -> Result<()> {
        let mut measure = Stream::with_kind(StreamKind::Measure);
        measure.insert(0.0, TimeSignature::new(3, 4)?);
        measure.insert(0.0, note("C4", 2.0)?);
        assert_eq!(measure.bar_duration_proportion(None)?, op_frac(2.0 / 3.0));

        let mut measure = Stream::with_kind(StreamKind::Measure);
        measure.insert(0.0, TimeSignature::new(3, 4)?);
        measure.insert(0.0, note("C4", 1.0)?);
        measure.pad_as_anacrusis(None, false)?;
        assert_eq!(measure.padding_left(), 2.0);
        Ok(())
    }

    #[test]
    fn without_a_time_signature_the_notes_say_the_bar() -> Result<()> {
        let mut measure = Stream::with_kind(StreamKind::Measure);
        measure.insert(0.0, Rest::new(Duration::quarter()));
        measure.insert(1.0, note("C4", 1.0)?);
        measure.pad_as_anacrusis(None, true)?;
        assert_eq!(measure.padding_left(), 0.0);
        assert_eq!(placed(&measure), named(&[("C4", 0.0)]));
        assert!(
            Stream::with_kind(StreamKind::Measure)
                .bar_duration_proportion(None)
                .is_err()
        );
        Ok(())
    }
}
