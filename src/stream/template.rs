//! A stream's skeleton without its notes, a note added into what already
//! sounds, and voices that hold one line taken back out: music21's
//! `template`, `insertIntoNoteOrChord` and `flattenUnnecessaryVoices`.

use super::{Stream, StreamElement, StreamEvent, StreamKind};
use crate::{
    chord::Chord,
    defaults::FloatType,
    duration::Duration,
    error::{Error, Result},
    makenotation::{event_order, op_frac},
    notation::StemDirection,
    note::Note,
    pitch::Pitch,
    rest::Rest,
};

/// Where an event of a template came from: a leaf by its old position, a
/// stream by where its leaves started and where each went, or nowhere, for
/// a rest put in.
enum Origin {
    Leaf(usize),
    Stream(usize, Vec<Option<usize>>),
    New,
}

/// What [`Stream::template`] keeps: music21's `template` arguments.
#[derive(Clone, Copy, Debug)]
pub struct TemplateOptions {
    /// Whether what is taken away leaves rests in its place: music21's
    /// `fillWithRests`.
    pub fill_with_rests: bool,
    /// Which elements are taken away: music21's `removeClasses`, by default
    /// notes, chords, rests and the like, dynamics and expressions.
    pub remove: fn(&StreamElement) -> bool,
    /// Whether voices are kept, emptied as everything else is: music21's
    /// `retainVoices`. Without, a voice goes as a note does.
    pub retain_voices: bool,
    /// Whether everything but the streams is taken away: music21's
    /// `removeAll`.
    pub remove_all: bool,
    /// Elements kept whatever else says: music21's `exemptFromRemove`.
    pub exempt: Option<fn(&StreamElement) -> bool>,
}

impl Default for TemplateOptions {
    fn default() -> Self {
        Self {
            fill_with_rests: true,
            remove: removed_by_default,
            retain_voices: true,
            remove_all: false,
            exempt: None,
        }
    }
}

/// What music21's `template` takes away unless told otherwise: its
/// `GeneralNote`s -- notes, chords, rests, unpitched strokes, percussion
/// chords and chord symbols -- its dynamics, and its expressions written
/// on their own: text expressions, rehearsal marks and repeat expressions.
pub fn removed_by_default(element: &StreamElement) -> bool {
    matches!(
        element,
        StreamElement::Note(_)
            | StreamElement::Chord(_)
            | StreamElement::Rest(_)
            | StreamElement::Unpitched(_)
            | StreamElement::PercussionChord(_)
            | StreamElement::ChordSymbol(_)
            | StreamElement::Dynamic(_)
            | StreamElement::TextExpression(_)
            | StreamElement::RehearsalMark(_)
            | StreamElement::RepeatExpression(_)
    )
}

impl Stream {
    /// The stream and every stream inside it with what `options` says
    /// taken away, a rest standing in for each stretch of what went where
    /// asked: music21's `template`. Each run of elements taken away between
    /// two kept is one rest, from where the first starts to where the
    /// longest of them ends. Spanners stay, naming nothing for what is gone,
    /// unless `remove_all` takes everything.
    pub fn template(&self, options: &TemplateOptions) -> Stream {
        self.template_moved(options).0
    }

    /// [`Stream::template`], and where each of this stream's leaves went in
    /// it, by old position.
    fn template_moved(&self, options: &TemplateOptions) -> (Stream, Vec<Option<usize>>) {
        // Each event of the template with where it came from.
        let mut events: Vec<(StreamEvent, Origin)> = Vec::new();
        // The stretch of what has been taken away since the last kept.
        let mut pending: Option<(FloatType, FloatType)> = None;
        let close = |events: &mut Vec<(StreamEvent, Origin)>,
                     pending: &mut Option<(FloatType, FloatType)>| {
            if let Some((start, end)) = pending.take()
                && let Ok(duration) = Duration::new(op_frac(end - start))
            {
                events.push((StreamEvent::new(start, Rest::new(duration)), Origin::New));
            }
        };
        let mut first_leaf = 0;
        for event in self.events() {
            let element = event.element();
            let leaves = leaf_count(element);
            let start = first_leaf;
            first_leaf += leaves;
            if let StreamElement::Stream(inner) = element
                && (options.retain_voices || !is_voice(element))
            {
                close(&mut events, &mut pending);
                let (inner, moved) = inner.template_moved(options);
                events.push((
                    StreamEvent::new(event.offset(), inner),
                    Origin::Stream(start, moved),
                ));
                continue;
            }
            let mut skip = options.remove_all
                || (options.remove)(element)
                || (!options.retain_voices && is_voice(element));
            if options.exempt.is_some_and(|exempt| exempt(element)) {
                skip = false;
            }
            if skip {
                if !options.fill_with_rests {
                    continue;
                }
                let length = element.quarter_length();
                if length != 0.0 {
                    let end = event.offset() + length;
                    match &mut pending {
                        None => pending = Some((event.offset(), end)),
                        Some((_, existing)) if end > *existing => *existing = end,
                        Some(_) => {}
                    }
                }
            } else {
                close(&mut events, &mut pending);
                let origin = match element {
                    StreamElement::Stream(_) => {
                        Origin::Stream(start, (0..leaves).map(Some).collect())
                    }
                    _ => Origin::Leaf(start),
                };
                events.push((event.clone(), origin));
            }
        }
        close(&mut events, &mut pending);
        events.sort_by(|left, right| event_order(&left.0, &right.0));
        let mut moved: Vec<Option<usize>> = vec![None; first_leaf];
        let mut placed = 0;
        for (event, origin) in &events {
            match origin {
                Origin::Leaf(old) => moved[*old] = Some(placed),
                Origin::Stream(start, inner) => {
                    for (offset, place) in inner.iter().enumerate() {
                        moved[start + offset] = place.map(|place| place + placed);
                    }
                }
                Origin::New => {}
            }
            placed += leaf_count(event.element());
        }
        // music21's `cloneEmpty` keeps a measure's number and padding and a
        // part's name, but not whether either is shown.
        let mut template = self.with_events(events.into_iter().map(|(event, _)| event).collect());
        template.set_number_hidden(false);
        template.set_name_hidden(false);
        template.set_abbreviation_hidden(false);
        if !options.remove_all {
            for spanner in self.spanners() {
                let mut spanner = spanner.clone();
                spanner.move_places(&moved);
                template.add_spanner(spanner);
            }
        }
        (template, moved)
    }

    /// Adds a note or chord where something already starts within its
    /// length, merging the two into one chord: music21's
    /// `insertIntoNoteOrChord`. Where a rest starts there it gives way to
    /// the note or chord; where nothing does, the note or chord is simply
    /// inserted. The merged chord, or the note or rest a merge of one pitch
    /// or none makes, keeps the length, articulations, expressions, lyrics,
    /// stem and notehead fill of what stood there; with `chords_only`, a
    /// merge of one pitch is still a chord. An unpitched note or percussion
    /// chord standing there takes no pitches in, and becomes a rest, as in
    /// music21.
    ///
    /// # Errors
    ///
    /// More than one note, chord or rest starting within the length, which
    /// music21 refuses; a chord symbol whose pitches cannot be read; or
    /// lyrics to carry onto a chord of no pitches.
    pub fn insert_into_note_or_chord(
        &mut self,
        offset: FloatType,
        element: StreamElement,
        chords_only: bool,
    ) -> Result<()> {
        let end = op_frac(offset + element.quarter_length());
        let targets: Vec<usize> = self
            .events()
            .iter()
            .enumerate()
            .filter(|(_, event)| {
                matches!(
                    event.element(),
                    StreamElement::Note(_)
                        | StreamElement::Chord(_)
                        | StreamElement::Rest(_)
                        | StreamElement::Unpitched(_)
                        | StreamElement::PercussionChord(_)
                        | StreamElement::ChordSymbol(_)
                ) && offset <= event.offset()
                    && (event.offset() < end
                        || (end <= offset
                            && event.offset() == offset
                            && event.element().quarter_length() == 0.0))
            })
            .map(|(index, _)| index)
            .collect();
        let placed = match targets.as_slice() {
            [] => element,
            [index] => {
                let target = self.events[*index].element().clone();
                let placed = merged(&target, &element, chords_only)?;
                let position: usize = self.events[..*index]
                    .iter()
                    .map(|event| leaf_count(event.element()))
                    .sum();
                self.retain_leaves(&mut |leaf, _| leaf != position);
                placed
            }
            _ => {
                return Err(Error::Stream(
                    "more than one element found at the specified offset".to_string(),
                ));
            }
        };
        self.insert_sorted(vec![StreamEvent::new(offset, placed)]);
        Ok(())
    }

    /// Takes away the voices holding nothing and, where one voice is left,
    /// puts what it holds back in the stream itself: music21's
    /// `flattenUnnecessaryVoices`. With `force`, every voice left is
    /// flattened so.
    pub fn flatten_unnecessary_voices(&mut self, force: bool) {
        if !self.events.iter().any(|event| is_voice(event.element())) {
            return;
        }
        self.events.retain(|event| {
            let empty = event
                .element()
                .as_stream()
                .is_some_and(|inner| inner.events().is_empty());
            !(is_voice(event.element()) && empty)
        });
        let voices = self
            .events
            .iter()
            .filter(|event| is_voice(event.element()))
            .count();
        if voices == 1 || force {
            // Each event with where its leaves started, and the voices'
            // spanners named by the stream's own leaves.
            let mut held: Vec<(StreamEvent, usize)> = Vec::new();
            let mut lifted = Vec::new();
            let mut first = 0;
            for event in std::mem::take(&mut self.events) {
                match event.element() {
                    StreamElement::Stream(inner) if inner.kind() == StreamKind::Voice => {
                        let count = inner.leaves().len();
                        let shift: Vec<Option<usize>> = (first..first + count).map(Some).collect();
                        for spanner in inner.spanners() {
                            let mut spanner = spanner.clone();
                            spanner.move_places(&shift);
                            lifted.push(spanner);
                        }
                        for inner_event in inner.events() {
                            let leaves = leaf_count(inner_event.element());
                            held.push((
                                StreamEvent::new(
                                    op_frac(event.offset() + inner_event.offset()),
                                    inner_event.element().clone(),
                                ),
                                first,
                            ));
                            first += leaves;
                        }
                    }
                    element => {
                        let leaves = leaf_count(element);
                        held.push((event, first));
                        first += leaves;
                    }
                }
            }
            held.sort_by(|left, right| event_order(&left.0, &right.0));
            let mut moved: Vec<Option<usize>> = vec![None; first];
            let mut placed = 0;
            for (event, start) in &held {
                let leaves = leaf_count(event.element());
                for offset in 0..leaves {
                    moved[start + offset] = Some(placed + offset);
                }
                placed += leaves;
            }
            self.events = held.into_iter().map(|(event, _)| event).collect();
            let mut spanners: Vec<_> = self.spanners().to_vec();
            spanners.extend(lifted);
            self.clear_spanners();
            for mut spanner in spanners {
                spanner.move_places(&moved);
                self.add_spanner(spanner);
            }
        }
    }
}

/// Whether an element is a voice.
fn is_voice(element: &StreamElement) -> bool {
    matches!(element, StreamElement::Stream(inner) if inner.kind() == StreamKind::Voice)
}

/// How many of [`Stream::leaves`] an element is.
fn leaf_count(element: &StreamElement) -> usize {
    match element {
        StreamElement::Stream(inner) => inner.leaves().len(),
        _ => 1,
    }
}

/// The pitches and notes of a note, chord or chord symbol, none for
/// anything else.
fn parts_of(element: &StreamElement) -> Result<(Vec<Pitch>, Vec<Note>)> {
    Ok(match element {
        StreamElement::Note(note) => (vec![note.pitch().clone()], vec![note.clone()]),
        StreamElement::Chord(chord) => (chord.pitches(), chord.notes().to_vec()),
        StreamElement::ChordSymbol(symbol) => {
            let pitches = symbol.pitches()?;
            let notes = pitches.iter().cloned().map(Note::from_pitch).collect();
            (pitches, notes)
        }
        _ => (Vec::new(), Vec::new()),
    })
}

/// What music21's `insertIntoNoteOrChord` makes of what stood there and
/// what is put in with it.
fn merged(
    target: &StreamElement,
    added: &StreamElement,
    chords_only: bool,
) -> Result<StreamElement> {
    // A rest gives way to what is added and a note, chord or chord symbol
    // takes it in; anything else, unpitched, takes nothing in.
    let (mut pitches, mut components) = parts_of(target)?;
    if matches!(
        target,
        StreamElement::Rest(_)
            | StreamElement::Note(_)
            | StreamElement::Chord(_)
            | StreamElement::ChordSymbol(_)
    ) {
        let (added_pitches, added_components) = parts_of(added)?;
        pitches.extend(added_pitches);
        components.extend(added_components);
    }
    let duration = target.duration().cloned().unwrap_or_default();
    let (articulations, expressions, lyrics, stem, fill) = match target {
        StreamElement::Note(note) => (
            note.articulations().to_vec(),
            note.expressions().to_vec(),
            note.lyrics().to_vec(),
            Some(note.stem_direction()),
            Some(note.notehead_fill()),
        ),
        StreamElement::Chord(chord) => (
            chord.articulations().to_vec(),
            chord.expressions().to_vec(),
            chord.lyrics().to_vec(),
            Some(chord.stem_direction()),
            Some(chord.notehead_fill()),
        ),
        StreamElement::Rest(rest) => (
            rest.articulations().to_vec(),
            rest.expressions().to_vec(),
            rest.lyrics().to_vec(),
            None,
            None,
        ),
        StreamElement::ChordSymbol(symbol) => (
            Vec::new(),
            Vec::new(),
            symbol.lyrics().to_vec(),
            Some(StemDirection::default()),
            Some(None),
        ),
        StreamElement::Unpitched(unpitched) => {
            let written = unpitched.written();
            (
                written.articulations().to_vec(),
                written.expressions().to_vec(),
                written.lyrics().to_vec(),
                Some(written.stem_direction()),
                Some(written.notehead_fill()),
            )
        }
        StreamElement::PercussionChord(chord) => {
            let written = chord.written();
            (
                written.articulations().to_vec(),
                written.expressions().to_vec(),
                written.lyrics().to_vec(),
                Some(written.stem_direction()),
                Some(written.notehead_fill()),
            )
        }
        _ => (Vec::new(), Vec::new(), Vec::new(), None, None),
    };
    let texts: Vec<String> = lyrics
        .iter()
        .filter_map(|lyric| lyric.explicit_text())
        .filter(|text| !text.is_empty())
        .collect();
    if pitches.len() > 1 || chords_only {
        let mut chord = Chord::new(pitches.as_slice())?.with_duration(duration);
        chord.articulations_mut().extend(articulations);
        chord.expressions_mut().extend(expressions);
        for text in &texts {
            chord.add_lyric(text, None, false)?;
        }
        if let Some(stem) = stem {
            chord.set_stem_direction(stem);
        }
        if let Some(fill) = fill {
            chord.set_notehead_fill(fill);
        }
        for (note, component) in chord.notes_mut().iter_mut().zip(&components) {
            note.set_notehead_fill(component.notehead_fill());
        }
        Ok(StreamElement::Chord(chord))
    } else if let [pitch] = pitches.as_slice() {
        let mut note = Note::from_pitch(pitch.clone()).with_duration(duration);
        note.articulations_mut().extend(articulations);
        note.expressions_mut().extend(expressions);
        for text in &texts {
            note.add_lyric(text, None, false)?;
        }
        if let Some(stem) = stem {
            note.set_stem_direction(stem);
        }
        if let Some(fill) = fill {
            note.set_notehead_fill(fill);
        }
        Ok(StreamElement::Note(note))
    } else {
        let mut rest = Rest::new(duration);
        rest.articulations_mut().extend(articulations);
        rest.expressions_mut().extend(expressions);
        for text in &texts {
            rest.add_lyric(text, None, false);
        }
        Ok(StreamElement::Rest(rest))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tinynotation::from_tiny_notation;

    #[test]
    fn a_template_keeps_the_skeleton_and_rests_where_notes_were() -> Result<()> {
        let line = from_tiny_notation("4/4 c4 d e f g1")?;
        let template = line.template(&TemplateOptions::default());
        let rests: Vec<(FloatType, FloatType)> = template
            .recurse()
            .into_iter()
            .filter_map(|(offset, element)| match element {
                StreamElement::Rest(rest) => Some((offset, rest.duration().quarter_length())),
                _ => None,
            })
            .collect();
        // music21: one rest a measure, as long as what it held.
        assert_eq!(rests, [(0.0, 4.0), (4.0, 4.0)]);
        Ok(())
    }

    #[test]
    fn a_note_is_merged_into_what_starts_there() -> Result<()> {
        let mut line = from_tiny_notation("4/4 c4 r d e")?;
        let measure = match line.events_mut()[0].element_mut() {
            StreamElement::Stream(measure) => measure,
            _ => panic!("a measure"),
        };
        let note = StreamElement::Note(Note::from_name("G4")?);
        measure.insert_into_note_or_chord(0.0, note.clone(), false)?;
        measure.insert_into_note_or_chord(1.0, note, false)?;
        let kinds: Vec<String> = measure
            .events()
            .iter()
            .filter_map(|event| match event.element() {
                StreamElement::Chord(chord) => Some(format!("chord {}", chord.pitches().len())),
                StreamElement::Note(note) => Some(format!("note {}", note.pitch().name())),
                StreamElement::Rest(_) => Some("rest".to_string()),
                _ => None,
            })
            .collect();
        assert_eq!(kinds, ["chord 2", "note G", "note D", "note E"]);
        Ok(())
    }
}
