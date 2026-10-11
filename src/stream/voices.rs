//! Moving music between voices and parts: music21's `voicesToParts` and
//! `explode`, `partsToVoices` and `implode`.

use super::{Stream, StreamElement, StreamEvent, StreamKind};
use crate::clef::Clef;
use crate::defaults::FloatType;
use crate::error::{Error, Result};
use crate::makenotation::event_order;
use crate::notation::StemDirection;
use crate::pitch::Pitch;

fn stream_error(message: impl Into<String>) -> Error {
    Error::Stream(message.into())
}

/// Which parts [`Stream::parts_to_voices`] puts together as the voices of
/// one part: music21's `voiceAllocation`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VoiceAllocation {
    /// Parts taken in order, this many to a part, the last part taking what
    /// is left.
    Each(usize),
    /// The parts, by index, of each new part in turn.
    Groups(Vec<Vec<usize>>),
}

impl Default for VoiceAllocation {
    /// Two voices to a part, as music21 has it by default.
    fn default() -> Self {
        Self::Each(2)
    }
}

/// A stream's id as music21 writes it into a new part's id.
fn id_text(stream: &Stream) -> String {
    stream.id().unwrap_or_default().to_string()
}

/// The streams this stream holds itself, with their offsets.
fn held(stream: &Stream, kind: StreamKind) -> Vec<(FloatType, &Stream)> {
    stream
        .events()
        .iter()
        .filter_map(|event| match event.element() {
            StreamElement::Stream(inner) if inner.kind() == kind => {
                Some((event.offset(), &**inner))
            }
            _ => None,
        })
        .collect()
}

/// A new measure with what music21's `Measure.mergeAttributes` takes from
/// `measure`: its id, number and padding.
fn measure_like(measure: &Stream) -> Stream {
    let mut out = Stream::with_kind(StreamKind::Measure);
    out.labels.id = measure.labels.id.clone();
    out.labels.number = measure.labels.number;
    out.labels.number_suffix = measure.labels.number_suffix.clone();
    out.labels.padding_left = measure.labels.padding_left;
    out.labels.padding_right = measure.labels.padding_right;
    out.labels.at_sounding_pitch = measure.labels.at_sounding_pitch;
    out
}

/// Adds the elements of `from` that `keep` passes to `into`, at the offsets
/// they have in `from`, as music21's `mergeElements` with a class filter;
/// barlines come with them when `barlines` is set.
fn merge_elements(
    into: &mut Stream,
    from: &Stream,
    keep: impl Fn(&StreamElement) -> bool,
    barlines: bool,
) {
    for event in from.events() {
        if keep(event.element()) {
            into.events.push(event.clone());
        }
    }
    if barlines {
        if from.labels.left_barline.is_some() {
            into.labels.left_barline = from.labels.left_barline.clone();
        }
        if from.labels.right_barline.is_some() {
            into.labels.right_barline = from.labels.right_barline.clone();
        }
    }
    into.sort_events();
}

/// A measure's barlines, meters, clefs and keys: what music21 carries into
/// each measure made from one holding voices.
fn is_measure_setting(element: &StreamElement) -> bool {
    ["Barline", "TimeSignature", "Clef", "KeySignature"]
        .into_iter()
        .any(|class| element.is_instance(class))
}

/// The measure `active` with all of `voice` added, as music21's
/// `mergeElements(voice)`; the voice's spanners come along.
fn with_voice(active: &Stream, voice: &Stream) -> Stream {
    let mut out = voice.clone();
    out.kind = StreamKind::Measure;
    let spanners = std::mem::take(&mut out.labels.spanners);
    out.labels = active.labels.clone();
    out.labels.spanners = spanners;
    if voice.labels.left_barline.is_some() {
        out.labels.left_barline = voice.labels.left_barline.clone();
    }
    if voice.labels.right_barline.is_some() {
        out.labels.right_barline = voice.labels.right_barline.clone();
    }
    let mut events: Vec<(StreamEvent, Option<usize>)> = active
        .events()
        .iter()
        .map(|event| (event.clone(), None))
        .collect();
    events.extend(
        voice
            .events()
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, event)| (event, Some(index))),
    );
    events.sort_by(|a, b| event_order(&a.0, &b.0));
    out.replace_events(events);
    out
}

/// The pitches music21's `bestClef` weighs: those of every note, chord and
/// chord symbol, however deep.
fn clef_pitches(stream: &Stream) -> Vec<Pitch> {
    let mut pitches = Vec::new();
    for (_, element) in stream.recurse() {
        match element {
            StreamElement::Note(note) => pitches.push(note.pitch().clone()),
            StreamElement::Chord(chord) => pitches.extend(chord.pitches()),
            StreamElement::PercussionChord(chord) => pitches.extend(chord.pitches()),
            StreamElement::ChordSymbol(symbol) => {
                pitches.extend(symbol.pitches().unwrap_or_default())
            }
            _ => {}
        }
    }
    pitches
}

/// Puts `clef` in place of the clef at the start of the part's first
/// measure, or at its start where it has none: music21's `Measure.clef`
/// setter.
fn set_first_clef(part: &mut Stream, clef: Clef) {
    let Some(first) = part
        .events
        .iter_mut()
        .filter_map(|event| match &mut event.element {
            StreamElement::Stream(inner) if inner.kind() == StreamKind::Measure => Some(inner),
            _ => None,
        })
        .next()
    else {
        return;
    };
    let old = first.events().iter().position(|event| {
        event.offset() == 0.0 && matches!(event.element(), StreamElement::Clef(_))
    });
    let mut events: Vec<(StreamEvent, Option<usize>)> = first
        .events()
        .iter()
        .cloned()
        .enumerate()
        .filter(|(index, _)| Some(*index) != old)
        .map(|(index, event)| (event, Some(index)))
        .collect();
    events.push((StreamEvent::new(0.0, clef), None));
    events.sort_by(|a, b| event_order(&a.0, &b.0));
    first.replace_events(events);
}

impl Stream {
    /// The most voices a measure of this part has, or the part itself, at
    /// least one: music21's `_maxVoiceCount`. With `by_id`, the ids of the
    /// voices too, in the order first met, and the count is at least theirs.
    fn max_voice_count(&self, by_id: bool) -> (usize, Vec<Option<String>>) {
        let mut count = 1;
        let mut ids: Vec<Option<String>> = Vec::new();
        let mut note = |voices: Vec<(FloatType, &Stream)>, count: &mut usize| {
            if by_id {
                for (_, voice) in voices {
                    let id = voice.id().map(str::to_string);
                    if !ids.contains(&id) {
                        ids.push(id);
                    }
                }
            } else {
                *count = (*count).max(voices.len());
            }
        };
        let measures = held(self, StreamKind::Measure);
        if !measures.is_empty() {
            for (_, measure) in measures {
                note(held(measure, StreamKind::Voice), &mut count);
            }
        } else if self.has_voices() {
            let voices = held(self, StreamKind::Voice);
            if by_id {
                note(voices, &mut count);
            } else {
                count = voices.len();
            }
        }
        (count.max(ids.len()), ids)
    }

    /// A score with a part for each voice of this part, as music21's
    /// `voicesToParts` makes it; a score has each of its parts so split.
    ///
    /// Each measure holding voices gives each part a measure of its own,
    /// with the measure's barlines, meters, clefs and keys and one voice's
    /// music; parts with no voice in that measure get the settings alone. A
    /// measure without voices goes whole to the first part, and the others
    /// get its barlines, meters and keys. Every part has the original's
    /// instruments, and its first measure the clef that best fits its notes
    /// ([`Clef::best_for`]). The parts are numbered by voice, `-v0` after the
    /// part's id and so on, or with `separate_by_id`, named after the voices'
    /// ids, each voice going to the part of its id whatever its place in the
    /// measure.
    ///
    /// ```
    /// use music21_rs::{Note, Stream, StreamKind};
    ///
    /// let mut upper = Stream::with_kind(StreamKind::Voice);
    /// upper.insert(0.0, Note::from_name("E5")?);
    /// let mut lower = Stream::with_kind(StreamKind::Voice);
    /// lower.insert(0.0, Note::from_name("C3")?);
    /// let mut measure = Stream::with_kind(StreamKind::Measure);
    /// measure.insert(0.0, upper);
    /// measure.insert(0.0, lower);
    /// let mut part = Stream::with_kind(StreamKind::Part);
    /// part.set_id(Some("P1".to_string()));
    /// part.insert(0.0, measure);
    ///
    /// let score = part.voices_to_parts(false)?;
    /// let parts = score.parts();
    /// assert_eq!(parts.len(), 2);
    /// assert_eq!(parts[1].id(), Some("P1-v1"));
    /// assert_eq!(parts[1].pitches()[0].name_with_octave(), "C3");
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// With `separate_by_id`, a part with measures none of which hold a
    /// voice, which music21 gives no part to put its music in.
    pub fn voices_to_parts(&self, separate_by_id: bool) -> Result<Stream> {
        let mut score = Stream::with_kind(StreamKind::Score);
        if self.has_part_like_streams() {
            for event in self.events() {
                if let StreamElement::Stream(inner) = event.element() {
                    for part in inner.voices_to_parts(separate_by_id)?.events {
                        score.events.push(StreamEvent::new(0.0, part.element));
                    }
                }
            }
            score.sort_events();
            return Ok(score);
        }

        let (count, ids) = self.max_voice_count(separate_by_id);
        if separate_by_id && ids.len() < count {
            return Err(stream_error("list index out of range"));
        }
        let mut parts: Vec<Stream> = (0..count)
            .map(|index| {
                let mut part = Stream::with_kind(StreamKind::Part);
                let suffix = if separate_by_id {
                    ids[index].clone().unwrap_or_default()
                } else {
                    format!("v{index}")
                };
                part.labels.id = Some(format!("{}-{suffix}", id_text(self)));
                merge_elements(
                    &mut part,
                    self,
                    |element| element.is_instance("Instrument"),
                    false,
                );
                part
            })
            .collect();
        let part_of = |index: usize, voice: &Stream| -> usize {
            if separate_by_id {
                let id = voice.id().map(str::to_string);
                ids.iter().position(|known| *known == id).unwrap_or(index)
            } else {
                index
            }
        };

        let measures = held(self, StreamKind::Measure);
        if !measures.is_empty() {
            for (offset, measure) in measures {
                let voices = held(measure, StreamKind::Voice);
                if voices.is_empty() {
                    let mut whole = measure_like(measure);
                    merge_elements(&mut whole, measure, |_| true, true);
                    whole.labels.spanners = measure.labels.spanners.clone();
                    parts[0].events.push(StreamEvent::new(offset, whole));
                    for part in parts.iter_mut().skip(1) {
                        let mut empty = measure_like(measure);
                        merge_elements(
                            &mut empty,
                            measure,
                            |element| {
                                ["Barline", "TimeSignature", "KeySignature"]
                                    .into_iter()
                                    .any(|class| element.is_instance(class))
                            },
                            true,
                        );
                        part.events.push(StreamEvent::new(offset, empty));
                    }
                    continue;
                }
                let mut active = measure_like(measure);
                merge_elements(&mut active, measure, is_measure_setting, true);
                let mut filled = vec![false; count];
                for (index, (_, voice)) in voices.iter().enumerate() {
                    let target = part_of(index, voice);
                    filled[target] = true;
                    parts[target]
                        .events
                        .push(StreamEvent::new(offset, with_voice(&active, voice)));
                }
                for (target, part) in parts.iter_mut().enumerate() {
                    if !filled[target] {
                        part.events.push(StreamEvent::new(offset, active.clone()));
                    }
                }
            }
        } else if self.has_voices() {
            for (index, (_, voice)) in held(self, StreamKind::Voice).into_iter().enumerate() {
                if let Some(part) = parts.get_mut(index) {
                    merge_elements(part, voice, |_| true, true);
                }
            }
        } else {
            merge_elements(&mut parts[0], self, |_| true, true);
        }

        for mut part in parts {
            part.sort_events();
            if !held(&part, StreamKind::Measure).is_empty() {
                let clef = Clef::best_for(&clef_pitches(&part), false);
                set_first_clef(&mut part, clef);
            }
            score.events.push(StreamEvent::new(0.0, part));
        }
        Ok(score)
    }

    /// A score with a part for each voice: music21's `explode`, which is
    /// [`Stream::voices_to_parts`] by place.
    ///
    /// # Errors
    ///
    /// As [`Stream::voices_to_parts`].
    pub fn explode(&self) -> Result<Stream> {
        self.voices_to_parts(false)
    }

    /// A score whose parts each hold several of this score's parts as
    /// voices, as music21's `partsToVoices` makes it.
    ///
    /// The parts are grouped by `allocation`. A group of one part is kept as
    /// it is unless `permit_one_voice_per_part`. Otherwise each measure of
    /// the new part has the first part's measure's number, barlines, meters,
    /// clefs and keys, and a voice for each part of the group, numbered from
    /// nought, holding the notes, chords and rests that measure holds
    /// itself -- not those in its own voices. With `set_stems`, the notes of
    /// the first, third ... voice have their stems up and the others down.
    /// The parts' spanners come along; the score keeps its metadata and
    /// nothing else.
    ///
    /// ```
    /// use music21_rs::{Note, Stream, StreamKind};
    /// use music21_rs::stream::VoiceAllocation;
    ///
    /// let mut score = Stream::with_kind(StreamKind::Score);
    /// for name in ["E5", "C3"] {
    ///     let mut measure = Stream::with_kind(StreamKind::Measure);
    ///     measure.insert(0.0, Note::from_name(name)?);
    ///     let mut part = Stream::with_kind(StreamKind::Part);
    ///     part.insert(0.0, measure);
    ///     score.insert(0.0, part);
    /// }
    /// let joined = score.parts_to_voices(&VoiceAllocation::Each(2), false, true)?;
    /// assert_eq!(joined.parts().len(), 1);
    /// assert_eq!(joined.parts()[0].measures()[0].voices().len(), 2);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// A group naming a part the score has not got, or a later part of a
    /// group with more measures than the first.
    pub fn parts_to_voices(
        &self,
        allocation: &VoiceAllocation,
        permit_one_voice_per_part: bool,
        set_stems: bool,
    ) -> Result<Stream> {
        let parts = self.parts();
        let bundle: Vec<Vec<&Stream>> = match allocation {
            VoiceAllocation::Each(each) => {
                if *each == 0 {
                    return Err(stream_error("integer division or modulo by zero"));
                }
                parts.chunks(*each).map(<[&Stream]>::to_vec).collect()
            }
            VoiceAllocation::Groups(groups) => groups
                .iter()
                .map(|group| {
                    group
                        .iter()
                        .map(|index| {
                            parts
                                .get(*index)
                                .copied()
                                .ok_or_else(|| stream_error("list index out of range"))
                        })
                        .collect::<Result<Vec<_>>>()
                })
                .collect::<Result<_>>()?,
        };

        let mut out = Stream::with_kind(self.kind());
        out.labels.id = self.labels.id.clone();
        out.labels.metadata = self.labels.metadata.clone();
        for group in bundle {
            if group.len() == 1 && !permit_one_voice_per_part {
                out.events.push(StreamEvent::new(0.0, group[0].clone()));
                continue;
            }
            out.events
                .push(StreamEvent::new(0.0, join_as_voices(&group, set_stems)?));
        }
        out.sort_events();
        Ok(out)
    }

    /// Parts joined two to a part as voices: music21's `implode`, which is
    /// [`Stream::parts_to_voices`] with its defaults.
    ///
    /// # Errors
    ///
    /// As [`Stream::parts_to_voices`].
    pub fn implode(&self) -> Result<Stream> {
        self.parts_to_voices(&VoiceAllocation::default(), false, true)
    }
}

/// One part holding each of `group` as a voice.
fn join_as_voices(group: &[&Stream], set_stems: bool) -> Result<Stream> {
    let mut active = Stream::with_kind(StreamKind::Part);
    // Where each voice stands, by measure and part, to move spanners to.
    let mut measures: Vec<(FloatType, Stream)> = Vec::new();
    // For each part, where each of its leaves went: (measure, voice
    // event index within the voice) by the part's own leaf position.
    let mut placed: Vec<Vec<Option<(usize, usize)>>> = Vec::new();
    for (index, part) in group.iter().enumerate() {
        let first = measures.is_empty();
        let mut moved = vec![None; part.leaf_total()];
        let mut leaf = 0;
        let mut measure_index = 0;
        for event in part.events() {
            let StreamElement::Stream(measure) = event.element() else {
                leaf += 1;
                continue;
            };
            if measure.kind() != StreamKind::Measure {
                leaf += measure.leaf_total();
                continue;
            }
            if first {
                let mut made = measure_like(measure);
                merge_elements(&mut made, measure, is_measure_setting, true);
                measures.push((event.offset(), made));
            } else if measure_index >= measures.len() {
                return Err(stream_error("index out of range"));
            }
            let mut voice = Stream::with_kind(StreamKind::Voice);
            voice.labels.id = Some(index.to_string());
            voice.labels.spanners = measure.labels.spanners.clone();
            let mut kept: Vec<(StreamEvent, Option<usize>)> = Vec::new();
            for (position, inner) in measure.events().iter().enumerate() {
                let count = super::leaf_count(inner.element());
                if inner.element().is_instance("GeneralNote") {
                    let mut kept_event = inner.clone();
                    if set_stems && let StreamElement::Note(note) = &mut kept_event.element {
                        note.set_stem_direction(if index % 2 == 0 {
                            StemDirection::Up
                        } else {
                            StemDirection::Down
                        });
                    }
                    moved[leaf] = Some((measure_index, kept.len()));
                    kept.push((kept_event, Some(position)));
                }
                leaf += count;
            }
            voice.events = measure.events.clone();
            voice.replace_events(kept);
            measures[measure_index].1.insert(0.0, voice);
            measure_index += 1;
        }
        placed.push(moved);
    }

    // Where each voice's first leaf now stands in the part.
    let mut starts: Vec<Vec<usize>> = Vec::new();
    let mut leaf = 0;
    for (_, measure) in &measures {
        let mut by_voice = vec![0; group.len()];
        for event in measure.events() {
            if let StreamElement::Stream(voice) = event.element()
                && voice.kind() == StreamKind::Voice
                && let Some(index) = voice.id().and_then(|id| id.parse::<usize>().ok())
                && index < by_voice.len()
            {
                by_voice[index] = leaf;
            }
            leaf += super::leaf_count(event.element());
        }
        starts.push(by_voice);
    }
    for (index, part) in group.iter().enumerate() {
        let moved: Vec<Option<usize>> = placed[index]
            .iter()
            .map(|place| place.map(|(measure, within)| starts[measure][index] + within))
            .collect();
        for spanner in part.spanners() {
            let mut spanner = spanner.clone();
            spanner.move_places(&moved);
            active.labels.spanners.push(spanner);
        }
    }
    for (offset, measure) in measures {
        active.events.push(StreamEvent::new(offset, measure));
    }
    active.sort_events();
    Ok(active)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note::Note;
    use crate::spanner::Spanner;

    fn part_of(names: &[&str]) -> Result<Stream> {
        let mut measure = Stream::with_kind(StreamKind::Measure);
        measure.insert(0.0, Clef::best_for(&[], false));
        for (offset, name) in names.iter().enumerate() {
            measure.insert(offset as FloatType, Note::from_name(name)?);
        }
        let mut part = Stream::with_kind(StreamKind::Part);
        part.insert(0.0, measure);
        Ok(part)
    }

    fn slurred(stream: &Stream) -> Vec<Vec<String>> {
        let leaves = stream.leaves();
        stream
            .spanners()
            .iter()
            .map(|spanner| {
                spanner
                    .spanned()
                    .iter()
                    .map(|place| match place.map(|place| leaves[place].1) {
                        Some(StreamElement::Note(note)) => note.pitch().name_with_octave(),
                        _ => "-".to_string(),
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn a_parts_spanners_follow_its_notes_into_a_voice() -> Result<()> {
        let mut upper = part_of(&["C5", "D5"])?;
        upper.add_spanner(Spanner::slur(vec![1, 2]));
        let mut lower = part_of(&["E3", "F3"])?;
        lower.add_spanner(Spanner::slur(vec![1, 2]));
        let mut score = Stream::with_kind(StreamKind::Score);
        score.insert(0.0, upper);
        score.insert(0.0, lower);

        let joined = score.implode()?;
        let parts = joined.parts();
        assert_eq!(parts.len(), 1);
        assert_eq!(
            slurred(parts[0]),
            vec![
                vec!["C5".to_string(), "D5".to_string()],
                vec!["E3".to_string(), "F3".to_string()]
            ]
        );
        Ok(())
    }

    #[test]
    fn what_music21_refuses_is_refused() -> Result<()> {
        // A part with measures but no voices has no id to name a part by.
        assert!(part_of(&["C4"])?.voices_to_parts(true).is_err());
        let mut score = Stream::with_kind(StreamKind::Score);
        score.insert(0.0, part_of(&["C4"])?);
        assert!(
            score
                .parts_to_voices(&VoiceAllocation::Groups(vec![vec![0, 1]]), false, true)
                .is_err()
        );
        assert!(
            score
                .parts_to_voices(&VoiceAllocation::Each(0), false, true)
                .is_err()
        );
        Ok(())
    }
}
