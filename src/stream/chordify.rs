//! A score's parts as one line of chords: music21's `Stream.chordify` and
//! the `Verticality.makeElement` it builds each chord with.

use super::{Stream, StreamElement, StreamEvent, StreamKind};
use crate::articulations::Articulation;
use crate::chord::Chord;
use crate::defaults::FloatType;
use crate::duration::Duration;
use crate::error::Result;
use crate::expressions::Expression;
use crate::makenotation::op_frac;
use crate::notation::{StemDirection, Tie, TieType};
use crate::note::Note;
use crate::percussion::PercussionNote;
use crate::rest::Rest;

/// How [`Stream::chordify_with`] makes its chords: music21's keyword
/// arguments to `chordify`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChordifyOptions {
    /// Whether a note sounding on from one chord into the next is tied
    /// across: music21's `addTies`.
    pub add_ties: bool,
    /// Whether a pitch two parts sound at once is one note of the chord
    /// rather than two: music21's `removeRedundantPitches`.
    pub remove_redundant_pitches: bool,
    /// Whether a score written at the pitch its instruments read is turned
    /// to the pitch it sounds first: music21's `toSoundingPitch`.
    pub to_sounding_pitch: bool,
}

impl Default for ChordifyOptions {
    fn default() -> Self {
        Self {
            add_ties: true,
            remove_redundant_pitches: true,
            to_sounding_pitch: true,
        }
    }
}

/// One element sounding over a stretch of time: music21's timespan.
pub(crate) struct Span<'a> {
    pub(crate) start: FloatType,
    pub(crate) end: FloatType,
    pub(crate) element: &'a StreamElement,
    /// Which of the streams read the element came from.
    pub(crate) part: usize,
}

impl Stream {
    /// The stream as one line of chords, as music21's `chordify` makes it:
    /// [`Stream::chordify_with`] with music21's defaults.
    ///
    /// ```
    /// use music21_rs::{Duration, Note, Stream, StreamKind};
    ///
    /// let mut upper = Stream::with_kind(StreamKind::Part);
    /// upper.push(Note::from_name("E4")?.with_duration(Duration::half()));
    /// let mut lower = Stream::with_kind(StreamKind::Part);
    /// lower.push(Note::from_name("C4")?.with_duration(Duration::quarter()));
    /// lower.push(Note::from_name("G3")?.with_duration(Duration::quarter()));
    /// let mut score = Stream::with_kind(StreamKind::Score);
    /// score.insert(0.0, upper);
    /// score.insert(0.0, lower);
    ///
    /// let chords = score.chordify()?;
    /// let names: Vec<String> = chords
    ///     .pitches()
    ///     .iter()
    ///     .map(|pitch| pitch.name_with_octave())
    ///     .collect();
    /// assert_eq!(names, ["C4", "E4", "G3", "E4"]);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// A score that cannot be turned to sounding pitch, or a length no
    /// duration has.
    pub fn chordify(&self) -> Result<Stream> {
        self.chordify_with(&ChordifyOptions::default())
    }

    /// The stream as one line of chords: at every point where a note,
    /// chord or rest in any part starts or stops, a chord of every pitch
    /// sounding there, or a rest where none is.
    ///
    /// The result is laid out as the first part is -- its measures, clefs,
    /// keys, meters and the rest of what it holds, but none of its notes,
    /// rests or voices -- and each measure is made from the measure at the
    /// same place in every part. A note sounding on past the end of a chord
    /// is tied into the next, a pitch sounding in two parts at once is one
    /// note, and a chord carries the articulations and expressions its
    /// notes do, one of each kind. Rests standing together in a measure are
    /// one rest. A chord symbol's notes sound where it stands, as music21's
    /// do. Spanners and the lyrics of the notes are not carried across, and
    /// every accidental is left for the notation to decide again.
    ///
    /// # Errors
    ///
    /// As [`Stream::chordify`].
    pub fn chordify_with(&self, options: &ChordifyOptions) -> Result<Stream> {
        Ok(self.chordify_tracking(options)?.0)
    }

    /// The stream chordified with every pitch kept, and how many parts
    /// sound a note in each chord made, alone or in a chord, in order,
    /// parts sharing an id counted once: the groups music21's
    /// `chordify(addPartIdAsGroup=True)` writes on the chord's pitches.
    pub(crate) fn chordify_parts_sounding(&self) -> Result<(Stream, Vec<usize>)> {
        let options = ChordifyOptions {
            remove_redundant_pitches: false,
            ..ChordifyOptions::default()
        };
        let ids: Vec<Option<String>> = self
            .events
            .iter()
            .filter_map(|event| match &event.element {
                StreamElement::Stream(inner) if inner.kind() != StreamKind::Voice => {
                    Some(inner.id().map(str::to_string))
                }
                _ => None,
            })
            .collect();
        let (chordified, heard) = self.chordify_tracking(&options)?;
        let sounding = heard
            .into_iter()
            .map(|parts| {
                let mut named: Vec<String> = Vec::new();
                for part in parts {
                    let name = ids
                        .get(part)
                        .cloned()
                        .flatten()
                        .unwrap_or_else(|| format!("#{part}"));
                    if !named.contains(&name) {
                        named.push(name);
                    }
                }
                named.len()
            })
            .collect();
        Ok((chordified, sounding))
    }

    /// The chords and, beside them, the streams sounding in each chord made.
    fn chordify_tracking(&self, options: &ChordifyOptions) -> Result<(Stream, Vec<Vec<usize>>)> {
        let mut heard: Vec<Vec<usize>> = Vec::new();
        let first_stream = |stream: &Stream| -> Option<Stream> {
            stream.events.iter().find_map(|event| match &event.element {
                StreamElement::Stream(inner) => Some((**inner).clone()),
                _ => None,
            })
        };
        let parted = self.has_part_like_streams();
        let work = if options.to_sounding_pitch
            && ((parted
                && first_stream(self)
                    .is_some_and(|first| first.at_sounding_pitch() == Some(false)))
                || (!parted && self.at_sounding_pitch() == Some(false)))
        {
            self.to_sounding_pitch()?
        } else {
            self.clone()
        };
        let source = if parted {
            first_stream(&work).unwrap_or_else(|| work.clone())
        } else {
            work.clone()
        };
        let mut template = template_of(&source);

        if template.measures().is_empty() {
            let spans = spans_of(&[&work]);
            fill(&mut template, &spans, options, &mut heard)?;
        } else {
            // Each part's measures, by their place among its measures.
            let parts: Vec<&Stream> = if parted {
                work.events
                    .iter()
                    .filter_map(|event| match &event.element {
                        StreamElement::Stream(inner) if inner.kind() != StreamKind::Voice => {
                            Some(&**inner)
                        }
                        _ => None,
                    })
                    .collect()
            } else {
                vec![&work]
            };
            let measures_of: Vec<Vec<&Stream>> = parts.iter().map(|part| part.measures()).collect();
            let mut index = 0;
            for event in &mut template.events {
                let StreamElement::Stream(measure) = &mut event.element else {
                    continue;
                };
                if measure.kind() != StreamKind::Measure {
                    continue;
                }
                let sources: Vec<&Stream> = measures_of
                    .iter()
                    .filter_map(|measures| measures.get(index).copied())
                    .collect();
                index += 1;
                let spans = spans_of(&sources);
                fill(measure, &spans, options, &mut heard)?;
            }
        }

        // Every accidental is decided afresh by whatever writes the chords.
        template.for_each_mut(&mut |_, element| {
            let notes = match element {
                StreamElement::Chord(chord) => chord.notes_mut(),
                StreamElement::Note(note) => std::slice::from_mut(note),
                _ => return,
            };
            for note in notes {
                let mut pitch = note.pitch().clone();
                if let Some(accidental) = pitch.written_accidental_mut()
                    && accidental.display_type() != "even-tied"
                {
                    accidental.set_display_status(None);
                    note.set_pitch(pitch);
                }
            }
        });
        if parted && let Some(metadata) = work.metadata() {
            template.set_metadata(Some(metadata.clone()));
        }
        Ok((template, heard))
    }
}

/// Whether an element brings a pitched note to a chord made: a note, or a
/// chord with a note among its members.
fn sounds_a_note(element: &StreamElement) -> bool {
    match element {
        StreamElement::Note(_) => true,
        StreamElement::Chord(chord) => !chord.notes().is_empty(),
        StreamElement::ChordSymbol(symbol) => {
            symbol.pitches().is_ok_and(|pitches| !pitches.is_empty())
        }
        StreamElement::PercussionChord(chord) => chord
            .members()
            .iter()
            .any(|member| matches!(member, PercussionNote::Note(_))),
        _ => false,
    }
}

/// Whether an element is one of music21's `GeneralNote`s, which chordify
/// takes out of the template and reads the time of.
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

/// music21's `template(fillWithRests=False, removeClasses=('GeneralNote',),
/// retainVoices=False)`: the stream and every stream inside it, voices left
/// out, with everything but notes, chords and rests kept where it stood.
fn template_of(stream: &Stream) -> Stream {
    let events = stream
        .events
        .iter()
        .filter_map(|event| match &event.element {
            StreamElement::Stream(inner) if inner.kind() == StreamKind::Voice => None,
            StreamElement::Stream(inner) => {
                Some(StreamEvent::new(event.offset, template_of(inner)))
            }
            element if is_general_note(element) => None,
            _ => Some(event.clone()),
        })
        .collect();
    // music21's `cloneEmpty` keeps a measure's number and padding and a
    // part's name, but not whether either is shown, nor how measures are
    // numbered.
    let mut template = stream.with_events(events);
    template.set_number_hidden(false);
    template.set_measure_numbering(None);
    template.set_name_hidden(false);
    template.set_abbreviation_hidden(false);
    template
}

/// music21's `asTimespans(classList=(GeneralNote,))` over the streams, one
/// after another, each read from its own start.
fn spans_of<'a>(streams: &[&'a Stream]) -> Vec<Span<'a>> {
    let mut spans = Vec::new();
    for (part, stream) in streams.iter().enumerate() {
        for (offset, element) in stream.leaves() {
            if is_general_note(element) {
                let start = op_frac(offset);
                spans.push(Span {
                    start,
                    end: op_frac(start + element.quarter_length()),
                    element,
                    part,
                });
            }
        }
    }
    spans
}

/// `chordifyOneMeasure`: a chord or rest at every time point of the spans,
/// put in the stream, and the rests standing together there joined.
fn fill(
    stream: &mut Stream,
    spans: &[Span<'_>],
    options: &ChordifyOptions,
    heard: &mut Vec<Vec<usize>>,
) -> Result<()> {
    let mut points: Vec<FloatType> = spans
        .iter()
        .flat_map(|span| [span.start, span.end])
        .collect();
    points.sort_by(FloatType::total_cmp);
    points.dedup();
    if !points.contains(&0.0) {
        points.insert(0, 0.0);
    }
    let mut made = Vec::new();
    for pair in points.windows(2) {
        let (offset, end) = (pair[0], pair[1]);
        if (end - offset).abs() <= 1e-7 {
            continue;
        }
        let element = make_element(spans, offset, op_frac(end - offset), options)?;
        if matches!(element, StreamElement::Chord(_)) {
            let mut parts = Vec::new();
            for span in sounding_at(spans, offset) {
                if sounds_a_note(span.element) && !parts.contains(&span.part) {
                    parts.push(span.part);
                }
            }
            heard.push(parts);
        }
        made.push(StreamEvent::new(op_frac(offset), element));
    }
    stream.insert_sorted(made);
    consolidate_rests(stream)
}

/// The spans sounding at `offset`, as a verticality lists them: those
/// starting there by where they end, then those started earlier by where
/// they start and end.
fn sounding_at<'a, 'b>(spans: &'b [Span<'a>], offset: FloatType) -> Vec<&'b Span<'a>> {
    let mut starting: Vec<&Span<'a>> = spans.iter().filter(|span| span.start == offset).collect();
    starting.sort_by(|left, right| left.end.total_cmp(&right.end));
    let mut overlapping: Vec<&Span<'a>> = spans
        .iter()
        .filter(|span| span.start < offset && offset < span.end)
        .collect();
    overlapping.sort_by(|left, right| {
        left.start
            .total_cmp(&right.start)
            .then(left.end.total_cmp(&right.end))
    });
    starting.extend(overlapping);
    starting
}

/// The notes an element sounds, each with the articulations and expressions
/// of the chord as a whole added to its first: what `makeElement` reads.
fn notes_of(element: &StreamElement) -> Result<Vec<Note>> {
    Ok(match element {
        StreamElement::Note(note) => vec![note.clone()],
        StreamElement::Chord(chord) => with_chord_marks(
            chord.notes().to_vec(),
            chord.articulations(),
            chord.expressions(),
        ),
        StreamElement::ChordSymbol(symbol) => {
            let duration = symbol.duration().clone();
            symbol
                .pitches()?
                .into_iter()
                .map(|pitch| Note::from_pitch(pitch).with_duration(duration.clone()))
                .collect()
        }
        StreamElement::PercussionChord(chord) => {
            let written = chord.written();
            let notes = chord
                .members()
                .into_iter()
                .filter_map(|member| match member {
                    PercussionNote::Note(note) => Some(note),
                    PercussionNote::Unpitched(_) => None,
                })
                .collect();
            with_chord_marks(notes, written.articulations(), written.expressions())
        }
        _ => Vec::new(),
    })
}

fn with_chord_marks(
    mut notes: Vec<Note>,
    articulations: &[Articulation],
    expressions: &[Expression],
) -> Vec<Note> {
    if let Some(first) = notes.first_mut() {
        first
            .articulations_mut()
            .extend(articulations.iter().cloned());
        first.expressions_mut().extend(expressions.iter().cloned());
    }
    notes
}

/// music21's `Verticality.makeElement`: a chord of every pitch sounding at
/// `offset`, lasting `length`, or a rest where nothing sounds.
pub(crate) fn make_element(
    spans: &[Span<'_>],
    offset: FloatType,
    length: FloatType,
    options: &ChordifyOptions,
) -> Result<StreamElement> {
    let duration = Duration::new(length)?;
    let sounding = sounding_at(spans, offset);
    let mut notes: Vec<(String, Note)> = Vec::new();
    let mut bust = 0;
    let mut any_pitch = false;
    for span in &sounding {
        for note in notes_of(span.element)? {
            any_pitch = true;
            let key = note.pitch().name_with_octave();
            let made = || new_note(span, &note, &duration, offset, length, options);
            match notes.iter().position(|(known, _)| *known == key) {
                None => notes.push((key, made())),
                Some(_) if !options.remove_redundant_pitches => {
                    notes.push((format!("{key}{bust}"), made()));
                    bust += 1;
                }
                Some(_) if !options.add_ties => {}
                Some(place) => {
                    let old = notes[place].1.tie().map(Tie::tie_type);
                    if old == Some(TieType::Continue) {
                        continue;
                    }
                    let candidate = made();
                    let Some(new) = candidate.tie().map(Tie::tie_type) else {
                        continue;
                    };
                    match old {
                        None => notes[place].1 = candidate,
                        Some(old) if is_start_and_stop(old, new) => {
                            if let Some(mut tie) = notes[place].1.tie().cloned() {
                                tie.set_tie_type(TieType::Continue);
                                notes[place].1.set_tie(Some(tie));
                            }
                        }
                        Some(_) if new == TieType::Continue => notes[place].1 = candidate,
                        Some(_) => {}
                    }
                }
            }
        }
    }
    if !any_pitch {
        return Ok(StreamElement::Rest(Rest::new(duration)));
    }

    let mut ordered: Vec<Note> = notes.into_iter().map(|(_, note)| note).collect();
    // music21 adds the notes by pitch space, and its `Chord.add` sorts the
    // chord by staff step and then pitch space after each: `D#4` before
    // `E-4`. Both sorts are stable, as Python's are.
    ordered.sort_by(|left, right| left.pitch().ps().total_cmp(&right.pitch().ps()));
    ordered.sort_by(|left, right| {
        left.pitch()
            .diatonic_note_number()
            .cmp(&right.pitch().diatonic_note_number())
            .then(left.pitch().ps().total_cmp(&right.pitch().ps()))
    });
    let mut articulations: Vec<Articulation> = Vec::new();
    let mut expressions: Vec<Expression> = Vec::new();
    for note in &ordered {
        let tie = note.tie().map(Tie::tie_type);
        let kept = |attach: &str| match attach {
            "first" => tie.is_none_or(|tie| tie == TieType::Start),
            "last" => tie.is_none_or(|tie| tie == TieType::Stop),
            _ => true,
        };
        for articulation in note.articulations() {
            if kept(articulation.tie_attach())
                && !articulations
                    .iter()
                    .any(|seen| seen.kind() == articulation.kind())
            {
                articulations.push(articulation.clone());
            }
        }
        for expression in note.expressions() {
            if kept(expression.tie_attach())
                && !expressions
                    .iter()
                    .any(|seen| same_kind_of_expression(seen, expression))
            {
                expressions.push(expression.clone());
            }
        }
    }
    let mut chord = Chord::new(ordered)?.with_duration(duration);
    chord.articulations_mut().extend(articulations);
    chord.expressions_mut().extend(expressions);
    Ok(StreamElement::Chord(chord))
}

fn is_start_and_stop(left: TieType, right: TieType) -> bool {
    matches!(
        (left, right),
        (TieType::Start, TieType::Stop) | (TieType::Stop, TieType::Start)
    )
}

/// music21 keeps one expression of each class.
fn same_kind_of_expression(left: &Expression, right: &Expression) -> bool {
    match (left, right) {
        (Expression::Ornament(left), Expression::Ornament(right)) => left.kind() == right.kind(),
        (Expression::Fermata(_), Expression::Fermata(_))
        | (Expression::Arpeggio(_), Expression::Arpeggio(_)) => true,
        _ => false,
    }
}

/// `newNote`: a copy of the note lasting as long as the chord, tied as it
/// sounds on from before or past it, its stem left to the chord and its
/// lyrics left behind.
fn new_note(
    span: &Span<'_>,
    note: &Note,
    duration: &Duration,
    offset: FloatType,
    length: FloatType,
    options: &ChordifyOptions,
) -> Note {
    let mut made = note.clone().with_duration(duration.clone());
    made.lyrics_mut().clear();
    if made.stem_direction() != StemDirection::NoStem {
        made.set_stem_direction(StemDirection::Unspecified);
    }
    if !options.add_ties {
        return made;
    }
    let before = op_frac(offset - span.start);
    let after = op_frac(span.end - (offset + length));
    let added = if before == 0.0 && after <= 0.0 {
        None
    } else if before > 0.0 {
        Some(if after > 0.0 {
            TieType::Continue
        } else {
            TieType::Stop
        })
    } else {
        Some(TieType::Start)
    };
    match (made.tie().cloned(), added) {
        (Some(mut tie), Some(added)) if is_start_and_stop(tie.tie_type(), added) => {
            tie.set_tie_type(TieType::Continue);
            made.set_tie(Some(tie));
        }
        (Some(mut tie), _) if tie.tie_type() == TieType::Continue => {
            tie.set_placement(None);
            made.set_tie(Some(tie));
        }
        (Some(mut tie), None) => {
            tie.set_placement(None);
            made.set_tie(Some(tie));
        }
        (_, Some(added)) => made.set_tie(Some(Tie::new(added))),
        (None, None) => {}
    }
    made
}

/// `consolidateRests`: every run of rests standing one after another among
/// the notes, chords and rests of the stream is one rest as long as all of
/// them, where the first stood.
fn consolidate_rests(stream: &mut Stream) -> Result<()> {
    let sounding: Vec<usize> = (0..stream.events.len())
        .filter(|place| {
            matches!(
                stream.events[*place].element,
                StreamElement::Note(_) | StreamElement::Chord(_) | StreamElement::Rest(_)
            )
        })
        .collect();
    let mut runs: Vec<Vec<usize>> = Vec::new();
    let mut run: Vec<usize> = Vec::new();
    for place in sounding {
        if matches!(stream.events[place].element, StreamElement::Rest(_)) {
            run.push(place);
        } else if !run.is_empty() {
            runs.push(std::mem::take(&mut run));
        }
    }
    runs.push(run);
    let runs: Vec<Vec<usize>> = runs.into_iter().filter(|run| run.len() >= 2).collect();
    if runs.is_empty() {
        return Ok(());
    }
    let mut joined = Vec::new();
    for run in &runs {
        let total: FloatType = run
            .iter()
            .map(|place| stream.events[*place].element.quarter_length())
            .sum();
        let at = stream.events[run[0]].offset;
        joined.push(StreamEvent::new(
            at,
            StreamElement::Rest(Rest::new(Duration::new(total)?)),
        ));
    }
    let dropped: Vec<usize> = runs.into_iter().flatten().collect();
    let mut place = 0;
    stream.events.retain(|_| {
        let keep = !dropped.contains(&place);
        place += 1;
        keep
    });
    stream.insert_sorted(joined);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pitch::Pitch;

    fn part(notes: &[(&str, FloatType)]) -> Stream {
        let mut part = Stream::with_kind(StreamKind::Part);
        for (name, length) in notes {
            let element: StreamElement = if *name == "r" {
                Rest::new(Duration::new(*length).unwrap()).into()
            } else {
                Note::from_name(name)
                    .unwrap()
                    .with_duration(Duration::new(*length).unwrap())
                    .into()
            };
            part.push(element);
        }
        part
    }

    fn score(parts: Vec<Stream>) -> Stream {
        let mut score = Stream::with_kind(StreamKind::Score);
        for part in parts {
            score.insert(0.0, part);
        }
        score
    }

    #[test]
    fn a_held_note_is_tied_across_the_chords_it_sounds_through() {
        let chords = score(vec![
            part(&[("C5", 2.0)]),
            part(&[("E4", 1.0), ("G4", 1.0)]),
        ])
        .chordify()
        .unwrap();
        let ties: Vec<Vec<Option<TieType>>> = chords
            .events()
            .iter()
            .filter_map(|event| match event.element() {
                StreamElement::Chord(chord) => Some(
                    chord
                        .notes()
                        .iter()
                        .map(|note| note.tie().map(Tie::tie_type))
                        .collect(),
                ),
                _ => None,
            })
            .collect();
        assert_eq!(
            ties,
            [
                vec![None, Some(TieType::Start)],
                vec![None, Some(TieType::Stop)]
            ]
        );
    }

    #[test]
    fn a_pitch_two_parts_sound_is_one_note_and_rests_together_are_one_rest() {
        let chords = score(vec![
            part(&[("C4", 1.0), ("r", 1.0), ("r", 1.0)]),
            part(&[("C4", 1.0), ("r", 2.0)]),
        ])
        .chordify()
        .unwrap();
        let events = chords.events();
        assert_eq!(events.len(), 2);
        let StreamElement::Chord(chord) = events[0].element() else {
            panic!("a chord first");
        };
        assert_eq!(chord.pitches(), [Pitch::from_name("C4").unwrap()]);
        assert!(matches!(events[1].element(), StreamElement::Rest(_)));
        assert_eq!(events[1].element().quarter_length(), 2.0);
    }
}
