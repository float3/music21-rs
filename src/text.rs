//! Text in scores: music21's `text` module. The lyrics of a stream joined
//! into words and lines, and the articles of a title moved to its front or
//! its end.
//!
//! ```
//! use music21_rs::text::{assemble_lyrics, prepend_article};
//! use music21_rs::tinynotation::from_tiny_notation;
//!
//! let mut line = from_tiny_notation("4/4 c4_Hi d4_there")?;
//! assert_eq!(assemble_lyrics(&line, 1, " "), "Hi there");
//! assert_eq!(prepend_article("Ale is Dear, The", None)?, "The Ale is Dear");
//! # Ok::<(), music21_rs::Error>(())
//! ```

use crate::{
    defaults::IntegerType,
    error::{Error, Result},
    notation::{Lyric, Syllabic},
    stream::{Stream, StreamElement},
};

/// The articles of each language music21 knows, by its language code:
/// music21's `articleReference`.
pub const ARTICLES: [(&str, &[&str]); 8] = [
    ("ar", &["al-"]),
    ("en", &["the", "a", "an"]),
    (
        "de",
        &[
            "der", "die", "das", "des", "dem", "den", "ein", "eine", "einer", "einem", "einen",
        ],
    ),
    ("nl", &["de", "het", "'t", "een"]),
    (
        "es",
        &["el", "la", "los", "las", "un", "una", "unos", "unas"],
    ),
    ("pt", &["o", "a", "os", "as", "um", "uma", "uns", "umas"]),
    (
        "fr",
        &[
            "le", "la", "les", "l'", "un", "une", "des", "du", "de la", "des",
        ],
    ),
    (
        "it",
        &[
            "il", "lo", "la", "l'", "i", "gli", "le", "un'", "un", "uno", "una", "del", "dello",
            "della", "dei", "degli", "delle",
        ],
    ),
];

/// The articles of a language, or of every language for none.
fn articles(language: Option<&str>) -> Result<Vec<&'static str>> {
    match language {
        None => Ok(ARTICLES
            .iter()
            .flat_map(|(_, articles)| articles.iter().copied())
            .collect()),
        Some(code) => ARTICLES
            .iter()
            .find(|(known, _)| *known == code)
            .map(|(_, articles)| articles.to_vec())
            .ok_or_else(|| Error::Text(format!("no articles known for language {code:?}"))),
    }
}

/// A title with an article written after a comma at its end put back in
/// front, `The Ale is Dear` for `Ale is Dear, The`: music21's
/// `prependArticle`. `language` is a code from [`ARTICLES`]; with none,
/// every language's articles are looked for. Text with no such article is
/// handed back as it is.
///
/// # Errors
///
/// A language [`ARTICLES`] has no articles for.
pub fn prepend_article(text: &str, language: Option<&str>) -> Result<String> {
    if !text.contains(',') {
        return Ok(text.to_string());
    }
    let articles = articles(language)?;
    let Some((before, trailing)) = text.rsplit_once(',') else {
        return Ok(text.to_string());
    };
    let trailing = trailing.trim();
    if articles.contains(&trailing.to_lowercase().as_str()) {
        Ok(format!("{trailing} {before}"))
    } else {
        Ok(text.to_string())
    }
}

/// A title with the article it starts with moved after a comma at its end,
/// `Ale is Dear, The` for `The Ale is Dear`: music21's `postpendArticle`.
/// `language` is read as [`prepend_article`] reads it.
///
/// # Errors
///
/// A language [`ARTICLES`] has no articles for.
pub fn postpend_article(text: &str, language: Option<&str>) -> Result<String> {
    if !text.contains(' ') {
        return Ok(text.to_string());
    }
    let articles = articles(language)?;
    let Some((leading, after)) = text.split_once(' ') else {
        return Ok(text.to_string());
    };
    let leading = leading.trim();
    if articles.contains(&leading.to_lowercase().as_str()) {
        Ok(format!("{after}, {leading}"))
    } else {
        Ok(text.to_string())
    }
}

/// The lyrics of one line of a stream, nested streams included, joined into
/// words, the syllables of each word run together: music21's
/// `assembleLyrics`. `line_number` counts the lyrics on each note from
/// one, as music21 does, so nought reads each note's last lyric; a note
/// without that many lyrics is passed over, and so is a syllable that is
/// only an underscore, as many scores write a held syllable.
pub fn assemble_lyrics(stream: &Stream, line_number: IntegerType, word_separator: &str) -> String {
    let mut word: Vec<String> = Vec::new();
    let mut words: Vec<String> = Vec::new();
    for event in stream.flatten().events() {
        let lyrics: &[Lyric] = match event.element() {
            StreamElement::Note(note) => note.lyrics(),
            StreamElement::Chord(chord) => chord.lyrics(),
            StreamElement::Rest(rest) => rest.lyrics(),
            _ => continue,
        };
        let Some(lyric) = line(lyrics, line_number) else {
            continue;
        };
        let text = lyric.explicit_text();
        if text.as_deref() == Some("_") {
            continue;
        }
        match lyric.explicit_syllabic() {
            Some(Syllabic::Begin | Syllabic::Middle) => word.extend(text),
            Some(Syllabic::Composite) => {
                word.extend(text);
                let last = lyric.components().last().and_then(Lyric::explicit_syllabic);
                if matches!(last, None | Some(Syllabic::End | Syllabic::Single)) {
                    words.push(word.concat());
                    word.clear();
                }
            }
            None | Some(Syllabic::End | Syllabic::Single) => {
                word.extend(text);
                words.push(word.concat());
                word.clear();
            }
        }
    }
    words.join(word_separator)
}

/// The lyric at a line, counted from one, nought or less counting back
/// from the last as a Python index does.
fn line(lyrics: &[Lyric], line_number: IntegerType) -> Option<&Lyric> {
    let index = i64::from(line_number) - 1;
    let index = if index < 0 {
        index + lyrics.len() as i64
    } else {
        index
    };
    usize::try_from(index)
        .ok()
        .and_then(|index| lyrics.get(index))
}

/// Every line of a stream's lyrics, each assembled by [`assemble_lyrics`],
/// one after another with `lyric_separation` before each but the first:
/// music21's `assembleAllLyrics`. Lines one to `max_lyrics - 1` are read,
/// as music21 reads them, and a line with no lyrics at all is left out,
/// though a separator still comes before each line after the first.
pub fn assemble_all_lyrics(
    stream: &Stream,
    max_lyrics: IntegerType,
    lyric_separation: &str,
    word_separator: &str,
) -> String {
    let mut lyrics = String::new();
    for line_number in 1..max_lyrics {
        let line = assemble_lyrics(stream, line_number, word_separator);
        if !line.is_empty() {
            if line_number > 1 {
                lyrics.push_str(lyric_separation);
            }
            lyrics.push_str(&line);
        }
    }
    lyrics
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note::Note;

    #[test]
    fn articles_move_to_the_front_and_back() -> Result<()> {
        // Each read off music21's text.prependArticle and postpendArticle.
        assert_eq!(
            prepend_article("Ale is Dear, The", None)?,
            "The Ale is Dear"
        );
        assert_eq!(prepend_article("The Ale is Dear", None)?, "The Ale is Dear");
        assert_eq!(
            prepend_article("Ale is Dear, The", Some("fr"))?,
            "Ale is Dear, The"
        );
        assert_eq!(
            prepend_article("Combattimento di Tancredi e Clorinda, Il", Some("it"))?,
            "Il Combattimento di Tancredi e Clorinda"
        );
        assert_eq!(
            postpend_article("The Ale is Dear", None)?,
            "Ale is Dear, The"
        );
        assert_eq!(
            postpend_article("Ale is Dear, The", Some("en"))?,
            "Ale is Dear, The"
        );
        assert!(prepend_article("Ale, The", Some("xx")).is_err());
        Ok(())
    }

    #[test]
    fn syllables_are_joined_into_words() {
        let mut stream = Stream::new();
        for (offset, (text, syllabic)) in [
            ("Hi", Syllabic::Single),
            ("the", Syllabic::Begin),
            ("re", Syllabic::End),
            ("_", Syllabic::Single),
        ]
        .into_iter()
        .enumerate()
        {
            let mut lyric = Lyric::new(text);
            lyric.set_syllabic(syllabic);
            let mut note = Note::from_name("C4").expect("a note");
            note.lyrics_mut().push(lyric);
            stream.insert(offset as crate::defaults::FloatType, note);
        }
        assert_eq!(assemble_lyrics(&stream, 1, " "), "Hi there");
        assert_eq!(assemble_lyrics(&stream, 0, "-"), "Hi-there");
        assert_eq!(assemble_lyrics(&stream, 2, " "), "");
        assert_eq!(assemble_all_lyrics(&stream, 10, "\n", " "), "Hi there");
    }
}
