//! Streams searched for runs of elements, and turned into text to compare:
//! music21's `search.base`.

use crate::{
    defaults::{FloatType, IntegerType},
    error::{Error, Result},
    makenotation::op_frac,
    notation::{Tie, TieType},
    pitch::Pitch,
    stream::{Stream, StreamElement, StreamKind},
};

use super::difflib;

/// One element of a stream as a search meets it, with where it is: what
/// music21's stream iterators hand a search.
#[derive(Clone, Copy, Debug)]
pub struct Searched<'a> {
    /// The element.
    pub element: &'a StreamElement,
    /// Where it starts in the stream searched.
    pub offset: FloatType,
    /// The number of the measure it is in, if it is in one: music21's
    /// `measureNumber`.
    pub measure: Option<IntegerType>,
}

/// Every element of a stream, nested ones included, each before what it
/// holds: music21's `recurse()`.
pub fn recursed(stream: &Stream) -> Vec<Searched<'_>> {
    let mut out = Vec::new();
    let measure = (stream.kind() == StreamKind::Measure).then(|| stream.number());
    walk(stream, 0.0, measure, true, &mut out);
    out
}

/// The elements of a stream itself, not of the streams it holds: music21's
/// `iter()`.
pub fn iterated(stream: &Stream) -> Vec<Searched<'_>> {
    let mut out = Vec::new();
    let measure = (stream.kind() == StreamKind::Measure).then(|| stream.number());
    walk(stream, 0.0, measure, false, &mut out);
    out
}

/// The notes, chords and rests of [`recursed`]: music21's
/// `recurse().notesAndRests`.
pub fn notes_and_rests(stream: &Stream) -> Vec<Searched<'_>> {
    recursed(stream)
        .into_iter()
        .filter(|found| is_general_note(found.element))
        .collect()
}

fn walk<'a>(
    stream: &'a Stream,
    base: FloatType,
    measure: Option<IntegerType>,
    deep: bool,
    out: &mut Vec<Searched<'a>>,
) {
    for event in stream.events() {
        let offset = op_frac(base + event.offset());
        let element = event.element();
        out.push(Searched {
            element,
            offset,
            measure,
        });
        if deep && let StreamElement::Stream(inner) = element {
            let inner_measure = if inner.kind() == StreamKind::Measure {
                Some(inner.number())
            } else {
                measure
            };
            walk(inner, offset, inner_measure, true, out);
        }
    }
}

/// Whether an element is one of music21's `GeneralNote`s: a note, chord,
/// rest, unpitched stroke, percussion chord or chord symbol.
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

/// One place in what is searched for: music21's search list entry.
#[derive(Clone, Debug)]
pub enum SearchTerm {
    /// Anything at all: music21's `Wildcard`.
    Wildcard,
    /// An element, compared as the search's algorithms compare it.
    Element(StreamElement),
    /// An element whose length does not matter: one with music21's
    /// `WildcardDuration`.
    AnyLength(StreamElement),
}

impl SearchTerm {
    fn element(&self) -> Option<&StreamElement> {
        match self {
            Self::Wildcard => None,
            Self::Element(element) | Self::AnyLength(element) => Some(element),
        }
    }

    /// Whether any length matches it: a wildcard, whose length is music21's
    /// `WildcardDuration` too, or an element given any length.
    fn any_length(&self) -> bool {
        matches!(self, Self::Wildcard | Self::AnyLength(_))
    }
}

impl From<StreamElement> for SearchTerm {
    fn from(element: StreamElement) -> Self {
        Self::Element(element)
    }
}

/// The name music21 gives an element, where it has one: a note's pitch
/// name, `rest`, a clef's or a key's name.
fn name(element: &StreamElement) -> Option<String> {
    match element {
        StreamElement::Note(note) => Some(note.pitch().name()),
        StreamElement::Rest(_) => Some("rest".to_string()),
        StreamElement::Clef(clef) => Some(clef.name()),
        StreamElement::Key(key) => Some(format!("{} {}", key.tonic().name(), key.mode())),
        StreamElement::TextExpression(_) => Some("text expression".to_string()),
        _ => None,
    }
}

/// How a [`StreamSearcher`] compares an element of the stream with the
/// search term it is lined up with: each answers that they match, that
/// they do not, or nothing, leaving it to the next.
#[derive(Clone, Copy, Debug)]
pub enum Algorithm {
    /// A wildcard matches anything: music21's `wildcardAlgorithm`.
    Wildcard,
    /// A term of any length matches; one of another length does not:
    /// music21's `rhythmAlgorithm`.
    Rhythm,
    /// A term or element with no name, or of another name, does not match:
    /// music21's `noteNameAlgorithm`.
    NoteName,
    /// A comparison of one's own.
    Custom(fn(&StreamElement, &SearchTerm) -> Option<bool>),
}

impl Algorithm {
    fn compare(self, element: &StreamElement, term: &SearchTerm) -> Option<bool> {
        match self {
            Self::Wildcard => matches!(term, SearchTerm::Wildcard).then_some(true),
            Self::Rhythm => {
                if term.any_length() {
                    return Some(true);
                }
                let wanted = term.element()?.quarter_length();
                (wanted != element.quarter_length()).then_some(false)
            }
            Self::NoteName => {
                let wanted = term.element().and_then(name);
                match (wanted, name(element)) {
                    (Some(wanted), Some(found)) if wanted == found => None,
                    _ => Some(false),
                }
            }
            Self::Custom(compare) => compare(element, term),
        }
    }
}

/// Which elements of the stream a [`StreamSearcher`] looks at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    /// Notes and chords alone.
    Notes,
    /// Notes, chords and rests: music21's `GeneralNote`s.
    NotesAndRests,
}

/// A run of elements a search found: music21's `search.base`
/// `SearchMatch`.
#[derive(Clone, Debug)]
pub struct SearchMatch<'a> {
    /// Where the run starts among the elements searched.
    pub index: usize,
    /// The elements, as many as were searched for.
    pub elements: Vec<Searched<'a>>,
}

/// Searches a stream for runs of elements matching a list of terms, the
/// terms compared by a list of algorithms: music21's `StreamSearcher`. A
/// run matches unless an algorithm says one of its elements does not;
/// with only the wildcard algorithm, as it starts, every run matches.
///
/// ```
/// use music21_rs::duration::Duration;
/// use music21_rs::note::Note;
/// use music21_rs::search::{Algorithm, Filter, SearchTerm, StreamSearcher};
/// use music21_rs::stream::StreamElement;
/// use music21_rs::tinynotation::from_tiny_notation;
///
/// let line = from_tiny_notation("3/4 c4. d8 e4 g4. a8 f4 c'8 d'4.")?;
/// let mut searcher = StreamSearcher::new(vec![
///     SearchTerm::Element(StreamElement::Note(
///         Note::from_name("C4")?.with_duration(Duration::new(1.5)?),
///     )),
///     SearchTerm::Element(StreamElement::Note(
///         Note::from_name("C4")?.with_duration(Duration::new(0.5)?),
///     )),
/// ]);
/// searcher.recurse = true;
/// searcher.filter = Some(Filter::Notes);
/// searcher.algorithms.push(Algorithm::Rhythm);
/// let found = searcher.run(&line)?;
/// assert_eq!(found.iter().map(|found| found.index).collect::<Vec<_>>(), [0, 3]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct StreamSearcher {
    /// What to look for, in order.
    pub search: Vec<SearchTerm>,
    /// Whether to look inside the streams the stream holds.
    pub recurse: bool,
    /// Which elements to look at; all of them for none.
    pub filter: Option<Filter>,
    /// How to compare, in order, each asked until one answers.
    pub algorithms: Vec<Algorithm>,
}

impl StreamSearcher {
    /// A search for these terms, of the stream's own elements, by the
    /// wildcard algorithm alone.
    pub fn new(search: Vec<SearchTerm>) -> Self {
        Self {
            search,
            recurse: false,
            filter: None,
            algorithms: vec![Algorithm::Wildcard],
        }
    }

    /// The runs of a stream's elements that match: music21's `run`.
    ///
    /// # Errors
    ///
    /// Nothing to search for.
    pub fn run<'a>(&self, stream: &'a Stream) -> Result<Vec<SearchMatch<'a>>> {
        let elements = if self.recurse {
            recursed(stream)
        } else {
            iterated(stream)
        };
        let elements: Vec<Searched<'a>> = elements
            .into_iter()
            .filter(|found| match self.filter {
                None => true,
                Some(Filter::Notes) => matches!(
                    found.element,
                    StreamElement::Note(_)
                        | StreamElement::Chord(_)
                        | StreamElement::ChordSymbol(_)
                ),
                Some(Filter::NotesAndRests) => is_general_note(found.element),
            })
            .collect();
        self.run_over(&elements)
    }

    /// The runs of these elements that match: music21's `run` of a stream
    /// iterator.
    ///
    /// # Errors
    ///
    /// Nothing to search for.
    pub fn run_over<'a>(&self, elements: &[Searched<'a>]) -> Result<Vec<SearchMatch<'a>>> {
        if self.search.is_empty() {
            return Err(empty_search());
        }
        let mut found = Vec::new();
        if self.search.len() > elements.len() {
            return Ok(found);
        }
        for (index, window) in elements.windows(self.search.len()).enumerate() {
            let refused = window.iter().zip(&self.search).any(|(searched, term)| {
                self.algorithms
                    .iter()
                    .find_map(|algorithm| algorithm.compare(searched.element, term))
                    == Some(false)
            });
            if !refused {
                found.push(SearchMatch {
                    index,
                    elements: window.to_vec(),
                });
            }
        }
        Ok(found)
    }
}

fn empty_search() -> Error {
    Error::Search("the search Stream or list cannot be empty".to_string())
}

/// Where each run of elements starts whose every element `matches` the term
/// lined up with it: music21's `streamSearchBase`.
fn search_with(
    elements: &[Searched<'_>],
    search: &[SearchTerm],
    matches: impl Fn(&StreamElement, &SearchTerm) -> bool,
) -> Result<Vec<usize>> {
    if search.is_empty() {
        return Err(empty_search());
    }
    if search.len() > elements.len() {
        return Ok(Vec::new());
    }
    Ok(elements
        .windows(search.len())
        .enumerate()
        .filter(|(_, window)| {
            window
                .iter()
                .zip(search)
                .all(|(searched, term)| matches(searched.element, term))
        })
        .map(|(index, _)| index)
        .collect())
}

/// Where each run of elements starts whose lengths are those searched for,
/// a wildcard or a term of any length matching any: music21's
/// `rhythmicSearch`.
///
/// # Errors
///
/// Nothing to search for.
pub fn rhythmic_search(elements: &[Searched<'_>], search: &[SearchTerm]) -> Result<Vec<usize>> {
    search_with(elements, search, |element, term| {
        term.any_length()
            || term
                .element()
                .is_some_and(|wanted| wanted.quarter_length() == element.quarter_length())
    })
}

fn same_name(element: &StreamElement, term: &SearchTerm) -> bool {
    matches!(
        (term.element().and_then(name), name(element)),
        (Some(wanted), Some(found)) if wanted == found
    )
}

/// Where each run of elements starts whose names -- a note's pitch name,
/// `rest` -- are those searched for, a wildcard matching any: music21's
/// `noteNameSearch`.
///
/// # Errors
///
/// Nothing to search for.
pub fn note_name_search(elements: &[Searched<'_>], search: &[SearchTerm]) -> Result<Vec<usize>> {
    search_with(elements, search, |element, term| {
        matches!(term, SearchTerm::Wildcard) || same_name(element, term)
    })
}

/// Where each run of elements starts whose names and lengths are those
/// searched for: music21's `noteNameRhythmicSearch`.
///
/// # Errors
///
/// Nothing to search for.
pub fn note_name_rhythmic_search(
    elements: &[Searched<'_>],
    search: &[SearchTerm],
) -> Result<Vec<usize>> {
    search_with(elements, search, |element, term| {
        if matches!(term, SearchTerm::Wildcard) {
            return true;
        }
        same_name(element, term)
            && (term.any_length()
                || term
                    .element()
                    .is_some_and(|wanted| wanted.quarter_length() == element.quarter_length()))
    })
}

/// The first pitch of a note, chord or chord symbol, which music21's
/// translations read.
fn first_pitch(element: &StreamElement) -> Option<Pitch> {
    match element {
        StreamElement::Note(note) => Some(note.pitch().clone()),
        StreamElement::Chord(chord) => chord.pitches().into_iter().next(),
        StreamElement::ChordSymbol(symbol) => symbol.pitches().ok()?.into_iter().next(),
        _ => element.pitches().into_iter().next(),
    }
}

/// The tie an element carries, if it can carry one.
fn tie_of(element: &StreamElement) -> Option<&Tie> {
    match element {
        StreamElement::Note(note) => note.tie(),
        StreamElement::Chord(chord) => chord.tie(),
        StreamElement::Rest(rest) => rest.tie(),
        StreamElement::Unpitched(stroke) => stroke.written().tie(),
        StreamElement::PercussionChord(chord) => chord.written().tie(),
        _ => None,
    }
}

/// An element as one character: a note's MIDI number, a chord's first
/// pitch's, and 127 for anything else -- a rest, an unpitched stroke, a
/// chord of nothing: music21's `translateNoteToByte`.
pub fn translate_note_to_byte(element: &StreamElement) -> char {
    let midi = match element {
        StreamElement::Note(note) => Some(note.pitch().midi()),
        StreamElement::Chord(_) | StreamElement::ChordSymbol(_) => {
            first_pitch(element).map(|pitch| pitch.midi())
        }
        _ => None,
    };
    midi.and_then(|midi| u32::try_from(midi).ok())
        .and_then(char::from_u32)
        .unwrap_or('\u{7f}')
}

/// An element's length as one character, ten times the base-two logarithm
/// of 256 times its quarter length, kept within one to 127: music21's
/// `translateDurationToBytes`.
pub fn translate_duration_to_bytes(element: &StreamElement) -> char {
    let length = element.quarter_length();
    let mut code = 1;
    if length != 0.0 {
        code = ((length * 256.0).log2() * 10.0) as i64;
        code = code.clamp(1, 127);
    }
    char::from_u32(code as u32).unwrap_or('\u{1}')
}

/// An element's tie as text: `s` for one starting, `c` for one going on,
/// `e` for one ending, and nothing otherwise: music21's
/// `translateNoteTieToByte`.
pub fn translate_note_tie_to_byte(element: &StreamElement) -> &'static str {
    match tie_of(element).map(Tie::tie_type) {
        Some(TieType::Start) => "s",
        Some(TieType::Continue) => "c",
        Some(TieType::Stop) => "e",
        _ => "",
    }
}

/// An element as its pitch, length and, if asked, tie characters:
/// music21's `translateNoteWithDurationToBytes`.
pub fn translate_note_with_duration_to_bytes(element: &StreamElement, include_tie: bool) -> String {
    let mut text = String::new();
    text.push(translate_note_to_byte(element));
    text.push(translate_duration_to_bytes(element));
    if include_tie {
        text.push_str(translate_note_tie_to_byte(element));
    }
    text
}

/// Notes and rests as text to search, and the measure each piece of it
/// comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Translation {
    /// The text.
    pub text: String,
    /// The measure of each note or rest the text was made from, in order:
    /// music21's `returnMeasures`.
    pub measures: Vec<Option<IntegerType>>,
}

/// Notes and rests as text, each its pitch, length and tie characters:
/// music21's `translateStreamToString`.
pub fn translate_stream_to_string(elements: &[Searched<'_>]) -> Translation {
    Translation {
        text: elements
            .iter()
            .map(|found| translate_note_with_duration_to_bytes(found.element, true))
            .collect(),
        measures: elements.iter().map(|found| found.measure).collect(),
    }
}

/// Notes and rests as text of their pitches alone: music21's
/// `translateStreamToStringNoRhythm`.
pub fn translate_stream_to_string_no_rhythm(elements: &[Searched<'_>]) -> Translation {
    Translation {
        text: elements
            .iter()
            .map(|found| translate_note_to_byte(found.element))
            .collect(),
        measures: elements.iter().map(|found| found.measure).collect(),
    }
}

/// Notes and rests as text of their lengths alone: music21's
/// `translateStreamToStringOnlyRhythm`.
pub fn translate_stream_to_string_only_rhythm(elements: &[Searched<'_>]) -> Translation {
    Translation {
        text: elements
            .iter()
            .map(|found| translate_duration_to_bytes(found.element))
            .collect(),
        measures: elements.iter().map(|found| found.measure).collect(),
    }
}

/// Walks notes and rests as music21's diatonic and interval translations
/// do: a run of rests read once as `rest`, a note tied on from the one
/// before passed over, and each other note handed its first pitch and
/// whether it is as long as the note before (nought), shorter (one) or
/// longer (two).
fn translate_lines(
    elements: &[Searched<'_>],
    rest: char,
    mut note: impl FnMut(&Pitch, usize) -> char,
) -> Result<Translation> {
    let mut text = String::new();
    let mut measures = Vec::new();
    let mut previous_rest = false;
    let mut previous_tie = false;
    let mut previous_length: Option<FloatType> = None;
    for found in elements {
        if matches!(found.element, StreamElement::Rest(_)) {
            if !previous_rest {
                previous_rest = true;
                text.push(rest);
                measures.push(found.measure);
            }
            continue;
        }
        previous_rest = false;
        let tie = tie_of(found.element);
        if previous_tie {
            if tie.is_none_or(|tie| tie.tie_type() == TieType::Stop) {
                previous_tie = false;
            }
            continue;
        } else if tie.is_some() {
            previous_tie = true;
        }
        let length = found.element.quarter_length();
        let change = match previous_length {
            None => 0,
            Some(previous) if previous == length => 0,
            Some(previous) if previous > length => 1,
            Some(_) => 2,
        };
        previous_length = Some(length);
        let pitch = first_pitch(found.element)
            .ok_or_else(|| Error::Search("list index out of range".to_string()))?;
        text.push(note(&pitch, change));
        measures.push(found.measure);
    }
    Ok(Translation { text, measures })
}

/// Notes and rests as text of their steps, a letter for each note --
/// `A` to `G` where it is as long as the note before, seven letters on
/// where it is longer, fourteen where it is shorter -- and `Z` for a run of
/// rests: music21's `translateDiatonicStreamToString`. A note tied on from
/// the one before is left out.
///
/// # Errors
///
/// An element with no pitch that is not a rest.
pub fn translate_diatonic_stream_to_string(elements: &[Searched<'_>]) -> Result<Translation> {
    translate_lines(elements, 'Z', |pitch, change| {
        let shift = [0, 14, 7][change];
        char::from_u32(pitch.step().as_char() as u32 + shift).unwrap_or('?')
    })
}

/// Notes and rests as text of the intervals between them, up to thirteen
/// semitones either way, and whether each is as long as, shorter or longer
/// than the one before, a space for a run of rests: music21's
/// `translateIntervalsAndSpeed`. The first note's interval is from the
/// first note.
///
/// # Errors
///
/// An element with no pitch that is not a rest.
pub fn translate_intervals_and_speed(elements: &[Searched<'_>]) -> Result<Translation> {
    let mut previous_midi = elements
        .iter()
        .find_map(|found| match found.element {
            StreamElement::Note(note) => Some(note.pitch().midi()),
            _ => None,
        })
        .unwrap_or(60);
    translate_lines(elements, ' ', |pitch, change| {
        let shift = [27 + 14, 27 * 2 + 14, 14][change];
        let midi = pitch.midi();
        let difference = (previous_midi - midi).clamp(-13, 13);
        previous_midi = midi;
        char::from_u32((32 + difference + shift) as u32).unwrap_or('?')
    })
}

/// How alike two stretches of text are, as music21's approximate searches
/// measure it: Python's `difflib.SequenceMatcher(None, a, b).ratio()`.
pub fn similarity(a: &str, b: &str) -> FloatType {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    difflib::ratio(&a, &b)
}

/// What `read` makes of the notes and rests of a stream flattened, every
/// part's together in the order they sound: what music21's approximate
/// searches translate.
fn with_flat_notes<R>(stream: &Stream, read: impl FnOnce(&[Searched<'_>]) -> R) -> R {
    let flat = stream.flatten();
    let notes: Vec<Searched<'_>> = iterated(&flat)
        .into_iter()
        .filter(|found| is_general_note(found.element))
        .collect();
    read(&notes)
}

/// The streams, best match first, each with how alike it is: music21 sorts
/// by one less the likeness, keeping the order of equals.
fn ranked(mut ratios: Vec<(usize, FloatType)>) -> Vec<(usize, FloatType)> {
    ratios.sort_by(|a, b| (1.0 - a.1).total_cmp(&(1.0 - b.1)));
    ratios
}

/// How alike each other stream is to this one, as the text of their notes
/// and rests -- pitch, length and tie -- best first: music21's
/// `approximateNoteSearch`. Each is the other stream's index and its
/// likeness, music21's `matchProbability`.
pub fn approximate_note_search(this: &Stream, others: &[&Stream]) -> Vec<(usize, FloatType)> {
    approximate_by(this, others, |elements| {
        translate_stream_to_string(elements).text
    })
}

/// The same, by pitch alone: music21's `approximateNoteSearchNoRhythm`.
pub fn approximate_note_search_no_rhythm(
    this: &Stream,
    others: &[&Stream],
) -> Vec<(usize, FloatType)> {
    approximate_by(this, others, |elements| {
        translate_stream_to_string_no_rhythm(elements).text
    })
}

/// The same, by length alone: music21's `approximateNoteSearchOnlyRhythm`.
pub fn approximate_note_search_only_rhythm(
    this: &Stream,
    others: &[&Stream],
) -> Vec<(usize, FloatType)> {
    approximate_by(this, others, |elements| {
        translate_stream_to_string_only_rhythm(elements).text
    })
}

fn approximate_by(
    this: &Stream,
    others: &[&Stream],
    translate: impl Fn(&[Searched<'_>]) -> String,
) -> Vec<(usize, FloatType)> {
    let this_text = with_flat_notes(this, &translate);
    ranked(
        others
            .iter()
            .enumerate()
            .map(|(index, other)| {
                let other_text = with_flat_notes(other, &translate);
                (index, similarity(&this_text, &other_text))
            })
            .collect(),
    )
}

/// The same, three parts by pitch to one by length: music21's
/// `approximateNoteSearchWeighted`.
pub fn approximate_note_search_weighted(
    this: &Stream,
    others: &[&Stream],
) -> Vec<(usize, FloatType)> {
    let texts = |notes: &[Searched<'_>]| {
        (
            translate_stream_to_string_no_rhythm(notes).text,
            translate_stream_to_string_only_rhythm(notes).text,
        )
    };
    let (pitches, lengths) = with_flat_notes(this, texts);
    ranked(
        others
            .iter()
            .enumerate()
            .map(|(index, other)| {
                let (other_pitches, other_lengths) = with_flat_notes(other, texts);
                let by_pitch = similarity(&pitches, &other_pitches);
                let by_length = similarity(&lengths, &other_lengths);
                (index, (3.0 * by_pitch + by_length) / 4.0)
            })
            .collect(),
    )
}

/// A rhythm the measures of a stream share, and which measures share it.
#[derive(Clone, Debug)]
pub struct MeasureRhythm<'a> {
    /// How many measures have it.
    pub count: usize,
    /// The rhythm as [`translate_stream_to_string_only_rhythm`] writes it.
    pub rhythm: String,
    /// The measures, in order.
    pub measures: Vec<&'a Stream>,
}

/// The rhythms of a stream's measures, nested ones included, most common
/// first, those as common in the order first met: music21's
/// `mostCommonMeasureRhythms`, without the copy of each first measure
/// transposed to start on C5 that music21 keeps beside it.
pub fn most_common_measure_rhythms(stream: &Stream) -> Vec<MeasureRhythm<'_>> {
    let mut rhythms: Vec<MeasureRhythm<'_>> = Vec::new();
    for found in recursed(stream) {
        let StreamElement::Stream(measure) = found.element else {
            continue;
        };
        if measure.kind() != StreamKind::Measure {
            continue;
        }
        let own: Vec<Searched<'_>> = iterated(measure)
            .into_iter()
            .filter(|found| is_general_note(found.element))
            .collect();
        let rhythm = translate_stream_to_string_only_rhythm(&own).text;
        match rhythms.iter_mut().find(|known| known.rhythm == rhythm) {
            Some(known) => {
                known.count += 1;
                known.measures.push(measure);
            }
            None => rhythms.push(MeasureRhythm {
                count: 1,
                rhythm,
                measures: vec![measure],
            }),
        }
    }
    rhythms.sort_by_key(|rhythm| std::cmp::Reverse(rhythm.count));
    rhythms
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note::Note;
    use crate::tinynotation::from_tiny_notation;

    fn note(name: &str, length: FloatType) -> SearchTerm {
        SearchTerm::Element(StreamElement::Note(
            Note::from_name(name)
                .expect("a note")
                .with_duration(crate::duration::Duration::new(length).expect("a length")),
        ))
    }

    #[test]
    fn runs_are_found_by_rhythm_and_name() -> Result<()> {
        // music21's StreamSearcher, rhythmicSearch and noteNameSearch
        // doctests, on "3/4 c4. d8 e4 g4. a8 f4 c'8 d'4.".
        let line = from_tiny_notation("3/4 c4. d8 e4 g4. a8 f4 c'8 d'4.")?;
        let notes = notes_and_rests(&line);
        assert_eq!(
            rhythmic_search(&notes, &[note("C4", 1.5), note("C4", 0.5)])?,
            [0, 3]
        );
        assert_eq!(
            note_name_search(
                &notes,
                &[note("D4", 1.0), SearchTerm::Wildcard, note("G4", 1.0)]
            )?,
            [1]
        );
        let mut searcher = StreamSearcher::new(vec![
            note("D4", 0.5),
            SearchTerm::Wildcard,
            SearchTerm::AnyLength(note_element("G4")),
        ]);
        searcher.recurse = true;
        searcher.filter = Some(Filter::Notes);
        searcher.algorithms.push(Algorithm::Rhythm);
        let found = searcher.run(&line)?;
        assert_eq!(
            found.iter().map(|found| found.index).collect::<Vec<_>>(),
            [1, 4]
        );
        assert!(StreamSearcher::new(Vec::new()).run(&line).is_err());
        Ok(())
    }

    fn note_element(name: &str) -> StreamElement {
        StreamElement::Note(Note::from_name(name).expect("a note"))
    }

    #[test]
    fn notes_are_translated_as_music21_translates_them() -> Result<()> {
        // Each read off music21's translate functions for the same line.
        let line = from_tiny_notation("3/4 c4 d8 r8 r4 e4~ e8 f8")?;
        let notes = notes_and_rests(&line);
        assert_eq!(
            translate_stream_to_string(&notes).text,
            "<P>F\u{7f}F\u{7f}P@Ps@FeAF"
        );
        assert_eq!(translate_diatonic_stream_to_string(&notes)?.text, "CRZLT");
        assert_eq!(translate_intervals_and_speed(&notes)?.text, "Ib ,c");
        Ok(())
    }
}
