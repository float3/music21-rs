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
        sung_notes(stream, None, &mut notes);

        // Each verse's text and lyrics, verses in the order first sung.
        let mut verses: Vec<(
            LyricIdentifier,
            String,
            Vec<IndexedLyric<'a>>,
            Option<Syllabic>,
        )> = Vec::new();
        for (element, lyrics, measure) in notes {
            for lyric in lyrics {
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
                indexed.push(IndexedLyric {
                    element,
                    start,
                    end: start + length,
                    measure,
                    lyric,
                    text,
                    identifier,
                    absolute_start: 0,
                    absolute_end: 0,
                });
                *last_syllabic = if lyric.is_composite() {
                    lyric.components().last().and_then(Lyric::explicit_syllabic)
                } else {
                    lyric.explicit_syllabic()
                };
            }
        }

        let mut index = Vec::new();
        let mut last_end = 0;
        for (_, _, indexed, _) in &verses {
            // A verse after one that ended with something starts after the
            // line break.
            let shift = if last_end != 0 {
                last_end + LINE_BREAK.chars().count()
            } else {
                last_end
            };
            for lyric in indexed {
                let mut lyric = lyric.clone();
                lyric.absolute_start = lyric.start + shift;
                lyric.absolute_end = lyric.end + shift;
                last_end = lyric.absolute_end;
                index.push(lyric);
            }
        }
        let index_text = verses
            .iter()
            .map(|(_, text, ..)| text.as_str())
            .collect::<Vec<_>>()
            .join(LINE_BREAK);
        Self { index_text, index }
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

    /// Each place `text` comes in the lyrics, left to right, none
    /// overlapping the one before: music21's `search` of a plain string.
    ///
    /// # Errors
    ///
    /// A match that touches no lyric, which music21 refuses: one that lies
    /// wholly in the space between words or verses.
    pub fn search(&self, text: &str) -> Result<Vec<LyricMatch<'a>>> {
        let mut matches = Vec::new();
        for (byte_start, found) in self.index_text.match_indices(text) {
            let start = self.index_text[..byte_start].chars().count();
            let end = start + found.chars().count();
            // music21 asks for every lyric reaching past the start and
            // starting by the last character found.
            let last = end as isize - 1;
            let indices: Vec<IndexedLyric<'a>> = self
                .index
                .iter()
                .filter(|lyric| lyric.absolute_end > start && lyric.absolute_start as isize <= last)
                .cloned()
                .collect();
            let (Some(first), Some(final_lyric)) = (indices.first(), indices.last()) else {
                return Err(Error::Search(format!(
                    "Could not find position {start} in text"
                )));
            };
            matches.push(LyricMatch {
                measure_start: first.measure,
                measure_end: final_lyric.measure,
                text: found.to_string(),
                identifier: first.identifier.clone(),
                indices,
            });
        }
        Ok(matches)
    }
}

/// The notes and chords of a stream that carry lyrics, nested ones
/// included, in order, each with the number of the measure it is in.
fn sung_notes<'a>(
    stream: &'a Stream,
    measure: Option<IntegerType>,
    out: &mut Vec<(&'a StreamElement, &'a [Lyric], Option<IntegerType>)>,
) {
    for event in stream.events() {
        match event.element() {
            element @ StreamElement::Note(note) => {
                out.push((element, note.lyrics(), measure));
            }
            element @ StreamElement::Chord(chord) => {
                out.push((element, chord.lyrics(), measure));
            }
            StreamElement::Stream(inner) => {
                let measure = if inner.kind() == StreamKind::Measure {
                    Some(inner.number())
                } else {
                    measure
                };
                sung_notes(inner, measure, out);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tinynotation::from_tiny_notation;

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
