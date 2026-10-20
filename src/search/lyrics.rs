//! Lyrics searched as text: music21's `search.lyrics`.

use crate::{
    defaults::IntegerType,
    error::{Error, Result},
    notation::{Lyric, Syllabic},
    stream::{Stream, StreamElement, StreamKind},
};

/// What music21 writes between the lyrics of one verse and the next in the
/// text it searches: `LINEBREAK_TOKEN`.
pub const LINE_BREAK: &str = " // ";

/// Which verse a lyric belongs to: the name a score gave it or, failing
/// one, its number, which music21 keeps apart even where the name is a
/// number written out.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LyricIdentifier {
    /// The verse's own name.
    Name(String),
    /// The verse's number, where it has no name.
    Number(IntegerType),
}

impl LyricIdentifier {
    fn of(lyric: &Lyric) -> Self {
        match lyric.explicit_identifier() {
            Some(name) => Self::Name(name.to_string()),
            None => Self::Number(lyric.number()),
        }
    }
}

/// One lyric of a stream and where its text sits in the text searched:
/// music21's `IndexedLyric`. Positions count characters, as Python's do.
#[derive(Clone, Debug)]
pub struct IndexedLyric<'a> {
    /// The note or chord the lyric is sung to.
    pub element: &'a StreamElement,
    /// Where the lyric starts in its verse's text.
    pub start: usize,
    /// Where it ends in its verse's text.
    pub end: usize,
    /// The number of the measure the note is in, if it is in one.
    pub measure: Option<IntegerType>,
    /// The lyric itself.
    pub lyric: &'a Lyric,
    /// Its text.
    pub text: String,
    /// Its verse.
    pub identifier: LyricIdentifier,
    /// Where it starts in the text of every verse.
    pub absolute_start: usize,
    /// Where it ends in the text of every verse.
    pub absolute_end: usize,
}

/// A stretch of lyrics a search found: music21's `search.lyrics`
/// `SearchMatch`.
#[derive(Clone, Debug)]
pub struct LyricMatch<'a> {
    /// The measure of the first lyric matched, if it is in one.
    pub measure_start: Option<IntegerType>,
    /// The measure of the last.
    pub measure_end: Option<IntegerType>,
    /// The text matched.
    pub text: String,
    /// Every lyric the match touches.
    pub indices: Vec<IndexedLyric<'a>>,
    /// The verse of the first.
    pub identifier: LyricIdentifier,
}

impl<'a> LyricMatch<'a> {
    /// The notes and chords the matched lyrics are sung to.
    pub fn elements(&self) -> Vec<&'a StreamElement> {
        self.indices.iter().map(|index| index.element).collect()
    }
}

/// Which lyric of which note, by position: the note's place among
/// [`Stream::leaves`] and which of its lyrics is sung there.
///
/// It is the by-position twin of [`IndexedLyric::element`] and
/// [`IndexedLyric::lyric`], for a caller holding the objects the stream was
/// read from rather than the stream's own values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LyricPlace {
    /// Where the note stands among [`Stream::leaves`].
    pub position: usize,
    /// Which of that note's lyrics is sung here.
    pub lyric: usize,
}

/// A stretch of lyrics a search found, as the text found and the run of
/// [`LyricSearcher::indexed`] entries it touches: the by-position twin of
/// [`LyricMatch`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LyricSpan {
    /// The text found, which for a search of a plain string is that string.
    pub text: String,
    /// The entries of [`LyricSearcher::indexed`] the text is sung to, which
    /// are always a run: the index is in the order the text reads.
    pub indices: core::ops::Range<usize>,
}

/// One verse as it is read: its name or number, the text of it so far,
/// each of its lyrics with the place it is sung at, and how the last one
/// ended, which says whether the next starts a word.
type Verse<'a> = (
    LyricIdentifier,
    String,
    Vec<(IndexedLyric<'a>, LyricPlace)>,
    Option<Syllabic>,
);

/// One note or chord a walk met that may carry lyrics: where it stands
/// among the leaves, itself, its lyrics and the measure it is in.
type Sung<'a> = (usize, &'a StreamElement, &'a [Lyric], Option<IntegerType>);

/// Finds text in a stream's lyrics: music21's `LyricSearcher`. Each
/// verse's lyrics are read as one text, syllables of a word run together
/// and words apart, and the verses one after another with [`LINE_BREAK`]
/// between them; a search finds each place the text comes and the lyrics
/// and notes there.
///
/// ```
/// use music21_rs::search::LyricSearcher;
/// use music21_rs::tinynotation::from_tiny_notation;
///
/// let line = from_tiny_notation("4/4 c4_Et d_in f_terra a_pax")?;
/// let searcher = LyricSearcher::new(&line);
/// assert_eq!(searcher.index_text(), "Et in terra pax");
/// let found = searcher.search("in ter")?;
/// assert_eq!(found[0].elements().len(), 2);
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct LyricSearcher<'a> {
    index_text: String,
    index: Vec<IndexedLyric<'a>>,
    places: Vec<LyricPlace>,
}

impl<'a> LyricSearcher<'a> {
    /// The lyrics of a stream, nested streams included, read with a space
    /// between words.
    pub fn new(stream: &'a Stream) -> Self {
        Self::with_word_separator(stream, " ")
    }

    /// The lyrics of a stream read with this between words: music21's
    /// `LyricSearcher(s, wordSeparator=...)`, indexed as its `index` does.
    pub fn with_word_separator(stream: &'a Stream, word_separator: &str) -> Self {
        let mut notes = Vec::new();
        sung_notes(stream, None, &mut 0, &mut notes);

        // Each verse's text and lyrics, verses in the order first sung,
        // each lyric beside the place it is sung at.
        let mut verses: Vec<Verse<'a>> = Vec::new();
        for (position, element, lyrics, measure) in notes {
            for (which, lyric) in lyrics.iter().enumerate() {
                let Some(text) = lyric.explicit_text().filter(|text| !text.is_empty()) else {
                    continue;
                };
                let identifier = LyricIdentifier::of(lyric);
                let verse = match verses.iter().position(|(known, ..)| *known == identifier) {
                    Some(verse) => verse,
                    None => {
                        verses.push((identifier.clone(), String::new(), Vec::new(), None));
                        verses.len() - 1
                    }
                };
                let (_, verse_text, indexed, last_syllabic) = &mut verses[verse];
                let mut start = verse_text.chars().count();
                if matches!(
                    last_syllabic,
                    None | Some(Syllabic::Begin | Syllabic::Middle)
                ) {
                    verse_text.push_str(&text);
                } else {
                    verse_text.push_str(word_separator);
                    verse_text.push_str(&text);
                    start += word_separator.chars().count();
                }
                let length = text.chars().count();
                indexed.push((
                    IndexedLyric {
                        element,
                        start,
                        end: start + length,
                        measure,
                        lyric,
                        text,
                        identifier,
                        absolute_start: 0,
                        absolute_end: 0,
                    },
                    LyricPlace {
                        position,
                        lyric: which,
                    },
                ));
                *last_syllabic = if lyric.is_composite() {
                    lyric.components().last().and_then(Lyric::explicit_syllabic)
                } else {
                    lyric.explicit_syllabic()
                };
            }
        }

        let mut index = Vec::new();
        let mut places = Vec::new();
        let mut last_end = 0;
        for (_, _, indexed, _) in &verses {
            // A verse after one that ended with something starts after the
            // line break.
            let shift = if last_end != 0 {
                last_end + LINE_BREAK.chars().count()
            } else {
                last_end
            };
            for (lyric, place) in indexed {
                let mut lyric = lyric.clone();
                lyric.absolute_start = lyric.start + shift;
                lyric.absolute_end = lyric.end + shift;
                last_end = lyric.absolute_end;
                index.push(lyric);
                places.push(*place);
            }
        }
        let index_text = verses
            .iter()
            .map(|(_, text, ..)| text.as_str())
            .collect::<Vec<_>>()
            .join(LINE_BREAK);
        Self {
            index_text,
            index,
            places,
        }
    }

    /// The text searched: each verse's lyrics, the verses joined with
    /// [`LINE_BREAK`]: music21's `indexText`.
    pub fn index_text(&self) -> &str {
        &self.index_text
    }

    /// Every lyric with text and where it sits in [`Self::index_text`]:
    /// music21's `indexTuples`.
    pub fn indexed(&self) -> &[IndexedLyric<'a>] {
        &self.index
    }

    /// Where each of [`Self::indexed`] is sung, in the same order: the
    /// by-position twin of the index, for a caller holding the objects the
    /// stream was read from.
    pub fn places(&self) -> &[LyricPlace] {
        &self.places
    }

    /// The entries of [`Self::indexed`] a stretch of [`Self::index_text`]
    /// is sung to, counting characters from `start` up to but not
    /// including `end`: music21's `_findObjsInIndexByPos`.
    ///
    /// # Errors
    ///
    /// A stretch no lyric is sung to, which music21 refuses: one lying
    /// wholly in the space between words or verses.
    pub fn indices_between(&self, start: usize, end: usize) -> Result<core::ops::Range<usize>> {
        // music21 asks for every lyric reaching past the start and
        // starting by the last character of the stretch.
        let touched =
            |lyric: &IndexedLyric<'a>| lyric.absolute_end > start && lyric.absolute_start < end;
        let first = self.index.iter().position(touched);
        let Some(first) = first else {
            return Err(Error::Search(format!(
                "Could not find position {start} in text"
            )));
        };
        let last = self.index.iter().rposition(touched).unwrap_or(first);
        Ok(first..last + 1)
    }

    /// Each place `text` comes in the lyrics, left to right, as the text
    /// found and the run of [`Self::indexed`] it is sung to: the
    /// by-position twin of [`Self::search`], which is built on it.
    ///
    /// # Errors
    ///
    /// As [`Self::indices_between`].
    pub fn search_spans(&self, text: &str) -> Result<Vec<LyricSpan>> {
        let mut spans = Vec::new();
        for (byte_start, found) in self.index_text.match_indices(text) {
            let start = self.index_text[..byte_start].chars().count();
            let end = start + found.chars().count();
            spans.push(LyricSpan {
                text: found.to_string(),
                indices: self.indices_between(start, end)?,
            });
        }
        Ok(spans)
    }

    /// Each place `text` comes in the lyrics, left to right, none
    /// overlapping the one before: music21's `search` of a plain string.
    ///
    /// # Errors
    ///
    /// A match that touches no lyric, which music21 refuses: one that lies
    /// wholly in the space between words or verses.
    pub fn search(&self, text: &str) -> Result<Vec<LyricMatch<'a>>> {
        let mut matches = Vec::new();
        for span in self.search_spans(text)? {
            let indices: Vec<IndexedLyric<'a>> = self.index[span.indices].to_vec();
            // `search_spans` answers no empty run, so both ends are there.
            let (Some(first), Some(final_lyric)) = (indices.first(), indices.last()) else {
                continue;
            };
            matches.push(LyricMatch {
                measure_start: first.measure,
                measure_end: final_lyric.measure,
                text: span.text,
                identifier: first.identifier.clone(),
                indices,
            });
        }
        Ok(matches)
    }
}

/// The notes and chords of a stream that carry lyrics, nested ones
/// included, in order, each with where it stands among [`Stream::leaves`]
/// and the number of the measure it is in.
///
/// `leaves` counts every element that is not a stream, which is what
/// [`Stream::leaves`] lists, so the number handed out here is a position in
/// that list: the two walks read a stream in the same order.
fn sung_notes<'a>(
    stream: &'a Stream,
    measure: Option<IntegerType>,
    leaves: &mut usize,
    out: &mut Vec<Sung<'a>>,
) {
    for event in stream.events() {
        match event.element() {
            element @ StreamElement::Note(note) => {
                out.push((*leaves, element, note.lyrics(), measure));
                *leaves += 1;
            }
            element @ StreamElement::Chord(chord) => {
                out.push((*leaves, element, chord.lyrics(), measure));
                *leaves += 1;
            }
            StreamElement::Stream(inner) => {
                let measure = if inner.kind() == StreamKind::Measure {
                    Some(inner.number())
                } else {
                    measure
                };
                sung_notes(inner, measure, leaves, out);
            }
            // Anything else is a leaf too, and takes a place in the list
            // even where nothing is sung to it.
            _ => *leaves += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tinynotation::from_tiny_notation;

    #[test]
    fn every_indexed_lyric_says_where_it_is_sung() -> Result<()> {
        // A line with a rest in it, so the notes do not stand where they
        // would if nothing else took a place, and a second verse on one
        // note, so a place must say which lyric as well as which note.
        let mut line = from_tiny_notation("4/4 c4_la r d_li e_lu f_le")?;
        line.for_each_mut(&mut |_, element| {
            if let StreamElement::Note(note) = element
                && note
                    .lyrics()
                    .first()
                    .is_some_and(|lyric| lyric.text() == "la")
            {
                let mut second = Lyric::from_raw_text("LA");
                second.set_number(2);
                note.lyrics_mut().push(second);
            }
        });

        let searcher = LyricSearcher::new(&line);
        let leaves = line.leaves();
        assert_eq!(searcher.indexed().len(), searcher.places().len());
        for (indexed, place) in searcher.indexed().iter().zip(searcher.places()) {
            let (_, element) = leaves[place.position];
            // The place names the very element the index holds, and the
            // very lyric of it.
            assert!(core::ptr::eq(element, indexed.element));
            let StreamElement::Note(note) = element else {
                panic!("a lyric is sung to a note here");
            };
            assert!(core::ptr::eq(&note.lyrics()[place.lyric], indexed.lyric));
        }
        // The second verse is the second lyric of the first note, which
        // stands third among the leaves: the clef and the meter the reader
        // gave the measure take places of their own.
        let second: Vec<LyricPlace> = searcher
            .indexed()
            .iter()
            .zip(searcher.places())
            .filter(|(indexed, _)| indexed.identifier == LyricIdentifier::Number(2))
            .map(|(_, place)| *place)
            .collect();
        assert_eq!(
            second,
            [LyricPlace {
                position: 2,
                lyric: 1
            }]
        );
        Ok(())
    }

    #[test]
    fn lyrics_are_indexed_as_music21_indexes_them() -> Result<()> {
        // music21's LyricSearcher doctest: "Et in ter-ra pax" over three
        // measures.
        let mut line = from_tiny_notation("2/4 c2_Et d4_in f_ter f_ra a_pax")?;
        for (position, syllabic) in [(2, Syllabic::Begin), (3, Syllabic::End)] {
            let mut notes = 0;
            line.for_each_mut(&mut |_, element| {
                if let StreamElement::Note(note) = element {
                    if notes == position {
                        note.lyrics_mut()[0].set_syllabic(syllabic);
                    }
                    notes += 1;
                }
            });
        }
        let searcher = LyricSearcher::new(&line);
        assert_eq!(searcher.index_text(), "Et in terra pax");
        let spans: Vec<(usize, usize, Option<IntegerType>, &str)> = searcher
            .indexed()
            .iter()
            .map(|lyric| (lyric.start, lyric.end, lyric.measure, lyric.text.as_str()))
            .collect();
        assert_eq!(
            spans,
            [
                (0, 2, Some(1), "Et"),
                (3, 5, Some(2), "in"),
                (6, 9, Some(2), "ter"),
                (9, 11, Some(3), "ra"),
                (12, 15, Some(3), "pax"),
            ]
        );
        let found = searcher.search("pax")?;
        assert_eq!(found.len(), 1);
        assert_eq!(
            (found[0].measure_start, found[0].measure_end),
            (Some(3), Some(3))
        );
        assert_eq!(found[0].text, "pax");
        Ok(())
    }
}
