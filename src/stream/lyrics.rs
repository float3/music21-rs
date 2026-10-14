//! The lyrics sung in a stream, gathered by their number: music21's
//! `Stream.lyrics`.

use std::collections::BTreeMap;

use super::{Stream, StreamElement, StreamKind};
use crate::defaults::IntegerType;
use crate::notation::{Lyric, TieType};

/// What stands at one note's place in a line of lyrics: music21's
/// `RecursiveLyricList`.
#[derive(Clone, Debug, PartialEq)]
pub enum SungLyric {
    /// The lyric the note carries under this number.
    Lyric(Lyric),
    /// A note carrying no lyric under this number, which keeps its place
    /// in the line so that the lyrics stay level with the notes.
    Unsung,
    /// What a nested stream answered, where the search recursed into it.
    Nested(Vec<SungLyric>),
}

/// How a stream's lyrics are gathered: music21's `lyrics` keywords.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LyricsOptions {
    /// Whether the notes of a measure are read as though the measure were
    /// not there: `ignoreBarlines`, which music21 takes as true, so that
    /// lyrics are gathered through the measures of a part without asking
    /// to recurse.
    pub ignore_barlines: bool,
    /// Whether every nested stream is read, each answering a list of its
    /// own inside its holder's: `recurse`.
    pub recurse: bool,
    /// Whether a note continuing a tie is passed over, as the syllable
    /// was sung at the note that started it: `skipTies`.
    pub skip_ties: bool,
}

impl Default for LyricsOptions {
    /// music21's own defaults: measures read through, nothing else
    /// recursed, and tied notes counted.
    fn default() -> Self {
        Self {
            ignore_barlines: true,
            recurse: false,
            skip_ties: false,
        }
    }
}

impl Stream {
    /// The lyrics sung in this stream, gathered by the number each is sung
    /// under: music21's `lyrics`.
    ///
    /// Each number's line has one place for every note read, holding the
    /// lyric sung there or [`SungLyric::Unsung`] where that note sings
    /// nothing under that number, so the lines stay level with the notes.
    /// A number first met partway through is padded with the notes before
    /// it, as music21 pads it.
    ///
    /// ```
    /// use music21_rs::stream::{LyricsOptions, SungLyric};
    /// use music21_rs::notation::Lyric;
    /// use music21_rs::{Note, Stream};
    ///
    /// let mut line = Stream::new();
    /// for (name, word) in [("C4", "this"), ("D4", "is"), ("E4", "sung")] {
    ///     let mut note = Note::from_name(name)?;
    ///     note.lyrics_mut().push(Lyric::new(word));
    ///     line.push(note);
    /// }
    ///
    /// let sung = line.lyrics(&LyricsOptions::default());
    /// let words: Vec<String> = sung[&1]
    ///     .iter()
    ///     .map(|sung| match sung {
    ///         SungLyric::Lyric(lyric) => lyric.text(),
    ///         _ => String::new(),
    ///     })
    ///     .collect();
    /// assert_eq!(words, ["this", "is", "sung"]);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    pub fn lyrics(&self, options: &LyricsOptions) -> BTreeMap<IntegerType, Vec<SungLyric>> {
        let mut lines: BTreeMap<IntegerType, Vec<SungLyric>> = BTreeMap::new();
        let mut read = 0;
        for event in self.events() {
            match event.element() {
                StreamElement::Stream(inner)
                    if options.ignore_barlines && inner.kind() == StreamKind::Measure =>
                {
                    // music21 reads a measure's notes as if the measure
                    // were not there, and its own notes alone -- not those
                    // of a voice inside it.
                    for inner_event in inner.events() {
                        if is_sung_by(inner_event.element()) {
                            take(inner_event.element(), options, &mut lines, &mut read);
                        }
                    }
                }
                StreamElement::Stream(inner) if options.recurse => {
                    for (number, line) in inner.lyrics(options) {
                        lines
                            .entry(number)
                            .or_default()
                            .push(SungLyric::Nested(line));
                    }
                }
                element if is_sung_by(element) => take(element, options, &mut lines, &mut read),
                // Anything else says nothing about the lyrics, and a
                // nested stream nobody asked to read is left alone.
                _ => {}
            }
        }
        lines
    }
}

/// Whether an element is one that carries lyrics: music21 reads the
/// `NotRest`s, so a rest sings nothing and keeps no place.
fn is_sung_by(element: &StreamElement) -> bool {
    element.is_instance("NotRest")
}

/// The lyrics one element carries.
fn lyrics_of(element: &StreamElement) -> &[Lyric] {
    match element {
        StreamElement::Note(note) => note.lyrics(),
        StreamElement::Chord(chord) => chord.lyrics(),
        StreamElement::ChordSymbol(symbol) => symbol.lyrics(),
        StreamElement::Unpitched(stroke) => stroke.written().lyrics(),
        StreamElement::PercussionChord(chord) => chord.written().lyrics(),
        _ => &[],
    }
}

/// Whether a note continues a tie, which `skip_ties` passes over: music21
/// keeps the note that starts one and the note that carries none.
fn continues_a_tie(element: &StreamElement) -> bool {
    let tie = match element {
        StreamElement::Note(note) => note.tie(),
        StreamElement::Chord(chord) => chord.tie(),
        StreamElement::Unpitched(stroke) => stroke.written().tie(),
        StreamElement::PercussionChord(chord) => chord.written().tie(),
        _ => None,
    };
    tie.is_some_and(|tie| tie.tie_type() != TieType::Start)
}

/// Takes one note's lyrics into the lines, every other line taking a place
/// of its own so they stay level: music21's `appendLyricsFromNote`.
fn take(
    element: &StreamElement,
    options: &LyricsOptions,
    lines: &mut BTreeMap<IntegerType, Vec<SungLyric>>,
    read: &mut usize,
) {
    if options.skip_ties && continues_a_tie(element) {
        return;
    }
    let sung = lyrics_of(element);
    if sung.is_empty() {
        for line in lines.values_mut() {
            line.push(SungLyric::Unsung);
        }
        *read += 1;
        return;
    }
    let mut taken: Vec<IntegerType> = Vec::new();
    for lyric in sung {
        // A number first met now starts level with the notes before it.
        lines
            .entry(lyric.number())
            .or_insert_with(|| vec![SungLyric::Unsung; *read])
            .push(SungLyric::Lyric(lyric.clone()));
        taken.push(lyric.number());
    }
    for (number, line) in &mut *lines {
        if !taken.contains(number) {
            line.push(SungLyric::Unsung);
        }
    }
    *read += 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notation::{Tie, TieType};
    use crate::note::Note;

    fn sung(name: &str, words: &[(IntegerType, &str)]) -> Note {
        let mut note = Note::from_name(name).expect("a note name");
        for (number, text) in words {
            let mut lyric = Lyric::new(*text);
            lyric.set_number(*number);
            note.lyrics_mut().push(lyric);
        }
        note
    }

    fn words(line: &[SungLyric]) -> Vec<String> {
        line.iter()
            .map(|sung| match sung {
                SungLyric::Lyric(lyric) => lyric.text(),
                SungLyric::Unsung => "-".to_string(),
                SungLyric::Nested(inner) => format!("[{}]", words(inner).join(" ")),
            })
            .collect()
    }

    #[test]
    fn every_note_keeps_its_place_in_each_line() {
        let mut line = Stream::new();
        line.push(sung("C4", &[(1, "this")]));
        line.push(sung("D4", &[]));
        line.push(sung("E4", &[(1, "sung")]));
        let found = line.lyrics(&LyricsOptions::default());
        assert_eq!(words(&found[&1]), ["this", "-", "sung"]);
    }

    #[test]
    fn a_number_first_met_partway_starts_level_with_the_notes_before_it() {
        let mut line = Stream::new();
        line.push(sung("C4", &[(1, "one")]));
        line.push(sung("D4", &[(1, "two"), (2, "second")]));
        let found = line.lyrics(&LyricsOptions::default());
        assert_eq!(words(&found[&1]), ["one", "two"]);
        // The second line is padded for the note it was not sung under.
        assert_eq!(words(&found[&2]), ["-", "second"]);
    }

    #[test]
    fn the_notes_of_a_measure_are_read_through_it() {
        let mut part = Stream::with_kind(StreamKind::Part);
        for word in ["first", "second"] {
            let mut measure = Stream::with_kind(StreamKind::Measure);
            measure.push(sung("C4", &[(1, word)]));
            part.push(measure);
        }
        let found = part.lyrics(&LyricsOptions::default());
        assert_eq!(words(&found[&1]), ["first", "second"]);

        // Asked not to read through the barlines, and not to recurse,
        // nothing is found at all.
        let shallow = LyricsOptions {
            ignore_barlines: false,
            ..LyricsOptions::default()
        };
        assert!(part.lyrics(&shallow).is_empty());

        // Asked to recurse, each measure answers a list of its own.
        let nested = LyricsOptions {
            ignore_barlines: false,
            recurse: true,
            ..LyricsOptions::default()
        };
        assert_eq!(words(&part.lyrics(&nested)[&1]), ["[first]", "[second]"]);
    }

    #[test]
    fn a_note_continuing_a_tie_is_passed_over_where_asked() {
        let mut line = Stream::new();
        line.push(sung("C4", &[(1, "held")]));
        let mut tied = sung("C4", &[(1, "over")]);
        tied.set_tie(Some(Tie::new(TieType::Stop)));
        line.push(tied);
        assert_eq!(
            words(&line.lyrics(&LyricsOptions::default())[&1]),
            ["held", "over"]
        );
        let skipping = LyricsOptions {
            skip_ties: true,
            ..LyricsOptions::default()
        };
        assert_eq!(words(&line.lyrics(&skipping)[&1]), ["held"]);
    }
}
