//! A score read by optical music recognition corrected by a performance of
//! it: music21's `alpha.analysis.fixer`.
//!
//! The two are aligned by [`StreamAligner`], its target the performance
//! (MIDI) and its source the reading (OMR), and each fixer reads the
//! changes the alignment found: [`fix_enharmonics`] respells a note read
//! with the wrong accidental, [`delete_measures`] takes out a measure
//! holding a note read wrong, and an [`OrnamentFixer`] writes the trill or
//! turn a performance plays where the reading has a single note.

use crate::alpha::analysis::aligner::{Change, ChangeOp, StreamAligner};
use crate::alpha::analysis::hasher::HashReference;
use crate::alpha::analysis::ornament_recognizer::{TrillRecognizer, TurnRecognizer};
use crate::braille::equality::equal;
use crate::defaults::FloatType;
use crate::error::{Error, Result};
use crate::expressions::{Expression, Ornament};
use crate::interval::notes_to_chromatic;
use crate::note::Note;
use crate::pitch::Accidental;
use crate::stream::{Stream, StreamElement, StreamKind};

/// What a reference names: an element, or a note of a chord hashed note by
/// note.
enum Named<'a> {
    Element(&'a StreamElement),
    InChord(&'a Note),
}

impl Named<'_> {
    fn note(&self) -> Option<&Note> {
        match self {
            Self::Element(StreamElement::Note(note)) => Some(note),
            Self::InChord(note) => Some(*note),
            Self::Element(_) => None,
        }
    }

    fn quarter_length(&self) -> FloatType {
        match self {
            Self::Element(element) => element.quarter_length(),
            Self::InChord(note) => StreamElement::Note((*note).clone()).quarter_length(),
        }
    }

    fn element(&self) -> StreamElement {
        match self {
            Self::Element(element) => (*element).clone(),
            Self::InChord(note) => StreamElement::Note((*note).clone()),
        }
    }
}

/// The element at a place in [`Stream::recurse`], walking the stream as it
/// does: each element, then what it holds.
fn element_at(stream: &Stream, place: usize) -> Option<&StreamElement> {
    stream.recurse().get(place).map(|(_, element)| *element)
}

/// The same, to be changed.
fn element_at_mut(stream: &mut Stream, place: usize) -> Option<&mut StreamElement> {
    fn walk<'a>(
        stream: &'a mut Stream,
        place: usize,
        seen: &mut usize,
    ) -> Option<&'a mut StreamElement> {
        for event in stream.events_mut() {
            if *seen == place {
                return Some(event.element_mut());
            }
            *seen += 1;
            if let StreamElement::Stream(inner) = event.element_mut()
                && let Some(found) = walk(inner, place, seen)
            {
                return Some(found);
            }
        }
        None
    }
    walk(stream, place, &mut 0)
}

/// The stream directly holding the element at a place in
/// [`Stream::recurse`], and the element's place among its events.
fn container_of(stream: &Stream, place: usize) -> Option<(&Stream, usize)> {
    fn walk<'a>(stream: &'a Stream, place: usize, seen: &mut usize) -> Option<(&'a Stream, usize)> {
        for (index, event) in stream.events().iter().enumerate() {
            if *seen == place {
                return Some((stream, index));
            }
            *seen += 1;
            if let StreamElement::Stream(inner) = event.element()
                && let Some(found) = walk(inner, place, seen)
            {
                return Some(found);
            }
        }
        None
    }
    walk(stream, place, &mut 0)
}

fn named(stream: &Stream, reference: HashReference) -> Result<Named<'_>> {
    let element = element_at(stream, reference.element).ok_or_else(|| {
        Error::Analysis(format!(
            "no element at place {} of the stream",
            reference.element
        ))
    })?;
    match (reference.component, element) {
        (None, element) => Ok(Named::Element(element)),
        (Some(component), StreamElement::Chord(chord)) => chord
            .notes()
            .get(component)
            .map(Named::InChord)
            .ok_or_else(|| Error::Analysis(format!("the chord has no note {component}"))),
        (Some(_), _) => Err(Error::Analysis(
            "a reference to a note of something that is not a chord".to_string(),
        )),
    }
}

/// The note a reference names, to be changed, where it names a note.
fn note_mut(stream: &mut Stream, reference: HashReference) -> Option<&mut Note> {
    match (
        reference.component,
        element_at_mut(stream, reference.element)?,
    ) {
        (None, StreamElement::Note(note)) => Some(note),
        (Some(component), StreamElement::Chord(chord)) => chord.notes_mut().get_mut(component),
        _ => None,
    }
}

/// The references a change makes, which music21 always has: a hash made
/// with no reference has nothing to fix.
fn references(change: &Change) -> Result<(HashReference, HashReference)> {
    match (change.target, change.source) {
        (Some(midi), Some(omr)) => Ok((midi, omr)),
        _ => Err(Error::Analysis(
            "'NoteHash' object has no attribute 'reference'".to_string(),
        )),
    }
}

/// Colours what a reference names, where it can be coloured: a note, a
/// note of a chord, or a chord.
fn colour(stream: &mut Stream, reference: HashReference, colour: &str) {
    let colour = Some(colour.to_string());
    if reference.component.is_some() {
        if let Some(note) = note_mut(stream, reference) {
            note.set_color(colour);
        }
        return;
    }
    match element_at_mut(stream, reference.element) {
        Some(StreamElement::Note(note)) => note.set_color(colour),
        Some(StreamElement::Chord(chord)) => chord.set_color(colour),
        _ => {}
    }
}

/// Respells each note of the reading the performance plays enharmonically
/// to it: music21's `EnharmonicFixer`.
///
/// Every element the changes name in the reading is coloured black. Of
/// two notes changed into each other no more than a fifth apart, by
/// interval class:
///
/// 1. a reading's natural is dropped where the notes sound alike, and
///    where the reading is a semitone off it becomes the flat above or the
///    sharp below the note played;
/// 2. a reading's sharp or flat on the letter played takes the played
///    note's accidental;
/// 3. a sharp read a whole tone above the note played becomes a flat, and
///    a flat read a whole tone below a sharp;
/// 4. a note played sharp or flat on the letter read, but read otherwise,
///    takes the played pitch.
///
/// # Errors
///
/// A change with no references, or one naming nothing in its stream.
pub fn fix_enharmonics(changes: &[Change], midi: &Stream, omr: &mut Stream) -> Result<()> {
    for change in changes {
        let (midi_reference, omr_reference) = references(change)?;
        colour(omr, omr_reference, "black");
        let Some(played) = named(midi, midi_reference)?.note().cloned() else {
            continue;
        };
        let Some(read) = named(omr, omr_reference)?.note().cloned() else {
            continue;
        };
        if change.op == ChangeOp::NoChange {
            continue;
        }
        if notes_to_chromatic(played.pitch(), read.pitch())?.interval_class() > 5 {
            continue;
        }
        let Some(fixed) = note_mut(omr, omr_reference) else {
            continue;
        };
        let (played, read) = (played.pitch(), read.pitch());
        let accidental = read.written_accidental().map(Accidental::name);
        let has_sharp_or_flat = accidental.is_some_and(|name| name != "natural");
        let same_step = played.step() == read.step();
        let mut pitch = read.clone();
        if accidental == Some("natural") {
            if played.ps() == read.ps() {
                pitch.set_written_accidental(None);
            } else if read.ps() > played.ps() {
                if read.ps() - 1.0 == played.ps() {
                    pitch.set_written_accidental(Some(Accidental::new("flat")?));
                }
            } else if read.ps() < played.ps() && read.ps() + 1.0 == played.ps() {
                pitch.set_written_accidental(Some(Accidental::new("sharp")?));
            }
        } else if has_sharp_or_flat && same_step {
            pitch.set_written_accidental(played.written_accidental().cloned());
        } else if has_sharp_or_flat {
            if read.ps() > played.ps() {
                if accidental == Some("sharp") && read.ps() - 2.0 == played.ps() {
                    pitch.set_written_accidental(Some(Accidental::new("flat")?));
                }
            } else if read.ps() < played.ps()
                && accidental == Some("flat")
                && read.ps() + 2.0 == played.ps()
            {
                pitch.set_written_accidental(Some(Accidental::new("sharp")?));
            }
        } else if read != played
            && played
                .written_accidental()
                .is_some_and(|accidental| accidental.name() != "natural")
            && same_step
        {
            pitch = played.clone();
        }
        fixed.set_pitch(pitch);
    }
    Ok(())
}

/// Takes out of the reading each measure holding a note changed from the
/// performance's: music21's `DeleteFixer`, which "does really weird things
/// still".
///
/// Only a measure standing in the reading itself is taken out, as
/// music21's `remove` finds nothing deeper, so the measures of a score's
/// parts stay where they are.
///
/// # Errors
///
/// A changed note in no measure, which music21 asks to remove nothing, or
/// a change with no references.
pub fn delete_measures(changes: &[Change], midi: &Stream, omr: &mut Stream) -> Result<()> {
    let mut removed: Vec<usize> = Vec::new();
    let mut result = Ok(());
    for change in changes {
        let (midi_reference, omr_reference) = references(change)?;
        if named(midi, midi_reference)?.note().is_none()
            || named(omr, omr_reference)?.note().is_none()
            || change.op == ChangeOp::NoChange
        {
            continue;
        }
        match measure_holding(omr, omr_reference.element) {
            Some(Some(top)) => {
                if !removed.contains(&top) {
                    removed.push(top);
                }
            }
            Some(None) => {}
            None => {
                result = Err(Error::Analysis(
                    "None is not a Music21Object; got <class 'NoneType'>".to_string(),
                ));
                break;
            }
        }
    }
    removed.sort_unstable();
    for top in removed.into_iter().rev() {
        omr.remove_event(top);
    }
    result
}

/// The measure holding the element at a place in [`Stream::recurse`], as
/// its place among the stream's own events where it stands there, and
/// nothing where it stands deeper; `None` where no measure holds it.
fn measure_holding(stream: &Stream, place: usize) -> Option<Option<usize>> {
    /// The places among their streams' events of the element and of each
    /// stream holding it, outermost first.
    fn path(stream: &Stream, place: usize, seen: &mut usize, out: &mut Vec<usize>) -> bool {
        for (index, event) in stream.events().iter().enumerate() {
            out.push(index);
            if *seen == place {
                return true;
            }
            *seen += 1;
            if let StreamElement::Stream(inner) = event.element()
                && path(inner, place, seen, out)
            {
                return true;
            }
            out.pop();
        }
        false
    }
    let mut places = Vec::new();
    if !path(stream, place, &mut 0, &mut places) {
        return None;
    }
    // The streams holding the element, each with its depth.
    let mut holders = Vec::new();
    let mut current = stream;
    for (depth, &index) in places[..places.len() - 1].iter().enumerate() {
        let StreamElement::Stream(inner) = current.events()[index].element() else {
            return None;
        };
        holders.push((depth, inner.kind()));
        current = inner;
    }
    let (depth, _) = holders
        .into_iter()
        .rev()
        .find(|(_, kind)| *kind == StreamKind::Measure)?;
    Some((depth == 0).then_some(places[0]))
}

/// The notes, rests and chords from one on, as many as fit in a length,
/// each a copy: music21's `getNotesWithinDuration`, read from the stream
/// directly holding the first.
///
/// Nothing where the first is longer than the length; else it and each
/// that follows it in its stream while the length left holds it.
pub fn notes_within_duration(
    container: &Stream,
    index: usize,
    total: FloatType,
) -> Vec<StreamElement> {
    let events = container.events();
    let Some(first) = events.get(index) else {
        return Vec::new();
    };
    let first = first.element();
    if first.quarter_length() > total {
        return Vec::new();
    }
    let mut left = total - first.quarter_length();
    let mut out = vec![first.clone()];
    for event in &events[index + 1..] {
        let element = event.element();
        if !is_general_note(element) {
            continue;
        }
        if left < element.quarter_length() {
            break;
        }
        left -= element.quarter_length();
        out.push(element.clone());
    }
    out
}

/// music21's `GeneralNote`: anything written that sounds or rests.
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

/// The busy notes a reference names the start of, as many as last no
/// longer than `total`.
fn busy_notes(
    stream: &Stream,
    reference: HashReference,
    total: FloatType,
) -> Result<Vec<StreamElement>> {
    if reference.component.is_some() {
        // A note of a chord stands in no stream, so nothing follows it.
        let note = named(stream, reference)?;
        return Ok(if note.quarter_length() > total {
            Vec::new()
        } else {
            vec![note.element()]
        });
    }
    let (container, index) = container_of(stream, reference.element).ok_or_else(|| {
        Error::Analysis(format!(
            "no element at place {} of the stream",
            reference.element
        ))
    })?;
    Ok(notes_within_duration(container, index, total))
}

/// A recognizer an [`OrnamentFixer`] tries.
#[derive(Clone, Debug, PartialEq)]
pub enum Recognizer {
    /// A trill.
    Trill(TrillRecognizer),
    /// A turn.
    Turn(TurnRecognizer),
}

impl Recognizer {
    fn recognize(
        &self,
        busy: &[StreamElement],
        simple: &[StreamElement],
    ) -> Result<Option<Ornament>> {
        match self {
            Self::Trill(recognizer) => recognizer.recognize(busy, simple),
            Self::Turn(recognizer) => recognizer.recognize(busy, simple),
        }
    }
}

/// Writes the ornaments a performance plays on the notes of a reading
/// that has none: music21's `OrnamentFixer`.
#[derive(Clone, Debug, PartialEq)]
pub struct OrnamentFixer {
    /// The recognizers tried, in order.
    pub recognizers: Vec<Recognizer>,
    /// The colour a note given an ornament is marked in: music21's
    /// `markChangeColor`.
    pub mark_change_color: String,
}

impl OrnamentFixer {
    /// Recognizes trills, plain and then with a nachschlag: music21's
    /// `TrillFixer`.
    pub fn trills() -> Self {
        Self {
            recognizers: vec![
                Recognizer::Trill(TrillRecognizer::default()),
                Recognizer::Trill(TrillRecognizer {
                    check_nachschlag: true,
                    ..TrillRecognizer::default()
                }),
            ],
            mark_change_color: "blue".to_string(),
        }
    }

    /// Recognizes turns: music21's `TurnFixer`.
    pub fn turns() -> Self {
        Self {
            recognizers: vec![Recognizer::Turn(TurnRecognizer::default())],
            mark_change_color: "blue".to_string(),
        }
    }

    /// The ornament the first recognizer to find one finds: music21's
    /// `findOrnament`.
    ///
    /// # Errors
    ///
    /// As the recognizers.
    pub fn find_ornament(
        &self,
        busy: &[StreamElement],
        simple: &[StreamElement],
    ) -> Result<Option<Ornament>> {
        for recognizer in &self.recognizers {
            if let Some(found) = recognizer.recognize(busy, simple)? {
                return Ok(Some(found));
            }
        }
        Ok(None)
    }

    /// Gives each note of the reading an inserted or substituted note
    /// stands for the ornament the performance plays there, from the
    /// performed note on for as long as the read note lasts: music21's
    /// `fix` in place. A read note given an ornament is not tried again,
    /// nor are performed notes already made into one -- each compared as
    /// music21 compares notes, by what they are rather than which they are.
    /// A note with an ornament already keeps it; with `mark`, one given an
    /// ornament is coloured [`OrnamentFixer::mark_change_color`].
    ///
    /// # Errors
    ///
    /// A change with no references or naming nothing, or a recognizer's
    /// error.
    pub fn fix(
        &self,
        changes: &[Change],
        midi: &Stream,
        omr: &mut Stream,
        mark: bool,
    ) -> Result<()> {
        let mut labelled: Vec<StreamElement> = Vec::new();
        let mut used: Vec<StreamElement> = Vec::new();
        for change in changes {
            if matches!(change.op, ChangeOp::NoChange | ChangeOp::Deletion) {
                continue;
            }
            let (midi_reference, omr_reference) = references(change)?;
            let read = named(omr, omr_reference)?;
            let read_element = read.element();
            if labelled.iter().any(|done| equal(done, &read_element)) {
                continue;
            }
            let busy = busy_notes(midi, midi_reference, read.quarter_length())?;
            if busy
                .iter()
                .any(|note| used.iter().any(|done| equal(done, note)))
            {
                continue;
            }
            let Some(ornament) = self.find_ornament(&busy, std::slice::from_ref(&read_element))?
            else {
                continue;
            };
            used.extend(busy);
            if let Some(note) = note_mut(omr, omr_reference) {
                let already = note
                    .expressions()
                    .iter()
                    .any(|expression| matches!(expression, Expression::Ornament(_)));
                if !already {
                    note.expressions_mut()
                        .push(Expression::Ornament(Box::new(ornament)));
                    if mark {
                        note.set_color(Some(self.mark_change_color.clone()));
                    }
                }
                labelled.push(StreamElement::Note(note.clone()));
            }
        }
        Ok(())
    }

    /// The reading with its ornaments written, the two streams copied and
    /// aligned afresh by a [`StreamAligner`]: music21's `fix` with
    /// `inPlace=False`, whose new fixer holds this reading.
    ///
    /// # Errors
    ///
    /// As [`OrnamentFixer::fix`], or streams that cannot be aligned.
    pub fn fixed(&self, midi: &Stream, omr: &Stream) -> Result<Stream> {
        let mut aligner = StreamAligner::new();
        aligner.align(midi, omr)?;
        let mut fixed = omr.clone();
        self.fix(&aligner.changes, midi, &mut fixed, false)?;
        Ok(fixed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::duration::Duration;
    use crate::expressions::OrnamentKind;

    fn note(name: &str, length: FloatType) -> Note {
        Note::from_name(name)
            .unwrap()
            .with_duration(Duration::new(length).unwrap())
    }

    fn measure(notes: &[(&str, FloatType)]) -> Stream {
        let mut measure = Stream::with_kind(StreamKind::Measure);
        for (name, length) in notes {
            measure.push(note(name, *length));
        }
        measure
    }

    fn reference(element: usize) -> Option<HashReference> {
        Some(HashReference {
            element,
            component: None,
        })
    }

    /// music21's EnharmonicFixer doctests: a played note and the note
    /// read, and what the read note becomes.
    #[test]
    fn enharmonics_are_respelled_as_music21_respells_them() {
        for (played, read, fixed) in [
            ("B-4", "A#4", "A#4"),
            ("A#4", "A#4", "A#4"),
            ("A4", "An4", "A4"),
            ("G#4", "An4", "A-4"),
            ("G-4", "Fn4", "F#4"),
            ("G#4", "Gn4", "G#4"),
            ("G#4", "G-4", "G#4"),
            ("G#4", "A#4", "A-4"),
            ("A-4", "G-4", "G#4"),
        ] {
            let midi = measure(&[(played, 1.0)]);
            let mut omr = measure(&[(read, 1.0)]);
            let op = if played == read {
                ChangeOp::NoChange
            } else {
                ChangeOp::Substitution
            };
            let changes = [Change {
                target: reference(0),
                source: reference(0),
                op,
            }];
            fix_enharmonics(&changes, &midi, &mut omr).unwrap();
            let StreamElement::Note(result) = omr.events()[0].element() else {
                unreachable!("the measure holds a note")
            };
            let named = result.pitch().name_with_octave();
            let expected = Note::from_name(fixed).unwrap().pitch().name_with_octave();
            assert_eq!(named, expected, "{played} played, {read} read");
            assert_eq!(result.color(), Some("black"));
            if read == "An4" && played == "A4" {
                assert!(result.pitch().written_accidental().is_none());
            }
        }
    }

    /// music21's testGetNotesWithinDuration, its measure m2.
    #[test]
    fn notes_within_a_length_are_the_ones_that_fit() {
        let m2 = measure(&[("C", 1.0), ("D", 0.5), ("E", 0.5)]);
        let names = |total: FloatType| -> Vec<String> {
            notes_within_duration(&m2, 0, total)
                .iter()
                .map(|element| match element {
                    StreamElement::Note(note) => note.pitch().name(),
                    _ => "other".to_string(),
                })
                .collect()
        };
        assert_eq!(names(1.0), ["C"]);
        assert_eq!(names(2.0), ["C", "D", "E"]);
        assert_eq!(names(4.0), ["C", "D", "E"]);
        assert_eq!(names(1.5), ["C", "D"]);
        assert_eq!(names(1.75), ["C", "D"]);
        assert!(names(0.5).is_empty());
    }

    fn ornaments(stream: &Stream) -> Vec<Vec<(OrnamentKind, FloatType, bool)>> {
        stream
            .events()
            .iter()
            .map(|event| match event.element() {
                StreamElement::Note(note) => note
                    .expressions()
                    .iter()
                    .filter_map(|expression| match expression {
                        Expression::Ornament(ornament) => Some((
                            ornament.kind(),
                            ornament.quarter_length(),
                            ornament.nachschlag(),
                        )),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            })
            .collect()
    }

    fn fixed(
        fixer: &OrnamentFixer,
        midi: &Stream,
        omr: &Stream,
    ) -> Vec<Vec<(OrnamentKind, FloatType, bool)>> {
        // In place and on copies alike.
        let copied = fixer.fixed(midi, omr).unwrap();
        let mut aligner = StreamAligner::new();
        aligner.align(midi, omr).unwrap();
        let mut in_place = omr.clone();
        fixer
            .fix(&aligner.changes, midi, &mut in_place, false)
            .unwrap();
        assert_eq!(ornaments(&copied), ornaments(&in_place));
        ornaments(&in_place)
    }

    /// music21's testTrillFixer.
    #[test]
    fn trills_are_written_as_music21_writes_them() {
        use OrnamentKind::Trill;
        let trills = OrnamentFixer::trills();
        let mut played = measure(&[("G", 0.25), ("A", 0.25), ("G", 0.25), ("A", 0.25)]);
        for name in ["C", "B3", "C", "B3", "C", "B3", "C", "B3"] {
            played.push(note(name, 0.0625));
        }
        let read = measure(&[("G", 1.0), ("B3", 1.0)]);
        assert_eq!(
            fixed(&trills, &played, &read),
            // Each trilled note lasts the read note's length shared among
            // the eight played.
            [vec![(Trill, 0.25, false)], vec![(Trill, 0.125, false)]]
        );

        let wrong = measure(&[("C", 0.25), ("A", 0.25), ("C", 0.25), ("A", 0.25)]);
        assert_eq!(fixed(&trills, &wrong, &measure(&[("C", 1.0)])), [vec![]]);
        let other = measure(&[("C", 0.25), ("D", 0.25), ("C", 0.25), ("D", 0.25)]);
        assert_eq!(fixed(&trills, &other, &measure(&[("A", 1.0)])), [vec![]]);

        let nachschlag = measure(&[
            ("E", 0.125),
            ("F", 0.125),
            ("E", 0.125),
            ("F", 0.125),
            ("E", 0.125),
            ("F", 0.125),
            ("E", 0.125),
            ("D", 0.125),
        ]);
        assert_eq!(
            fixed(&trills, &nachschlag, &measure(&[("E", 1.0)])),
            [vec![(Trill, 0.125, true)]]
        );

        let mut trilled = measure(&[("F", 1.0)]);
        let mut written = Ornament::of_kind(Trill);
        written.set_quarter_length(0.125);
        if let StreamElement::Note(note) = trilled.events_mut()[0].element_mut() {
            note.expressions_mut()
                .push(Expression::Ornament(Box::new(written)));
        }
        let played = measure(&[("F", 0.125), ("G", 0.125), ("F", 0.125), ("G", 0.125)]);
        assert_eq!(
            fixed(&trills, &played, &trilled),
            [vec![(Trill, 0.125, false)]]
        );
    }

    /// music21's testTurnFixer.
    #[test]
    fn turns_are_written_as_music21_writes_them() {
        use OrnamentKind::{InvertedTurn, Turn};
        let turns = OrnamentFixer::turns();
        let played = measure(&[("G", 1.0), ("F", 1.0), ("E", 1.0), ("F", 1.0)]);
        assert_eq!(
            fixed(&turns, &played, &measure(&[("F", 4.0)])),
            [vec![(Turn, 1.0, false)]]
        );

        let read = measure(&[("B-", 1.0), ("G", 1.0), ("B-", 1.0)]);
        let played = measure(&[
            ("A", 0.25),
            ("B-", 0.25),
            ("C5", 0.25),
            ("B-", 0.25),
            ("G", 1.0),
            ("G#", 0.25),
            ("A#", 0.25),
            ("B", 0.25),
            ("A#", 0.25),
        ]);
        assert_eq!(
            fixed(&turns, &played, &read),
            [
                vec![(InvertedTurn, 0.25, false)],
                vec![],
                vec![(InvertedTurn, 0.25, false)]
            ]
        );

        let played = measure(&[("B", 1.0), ("A", 1.0), ("G", 1.0), ("F", 1.0)]);
        assert_eq!(fixed(&turns, &played, &measure(&[("A", 4.0)])), [vec![]]);
    }
}
