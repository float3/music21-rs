//! Which language a text is in, by the letters that follow each pair of
//! letters: music21's `text.LanguageDetector` and `text.Trigram`.
//!
//! Each language is known by a long text in it, the excerpts music21
//! carries from Project Gutenberg (in `src/features/language/`, as music21
//! has them). A text is compared with each by how alike their counts of
//! letter-after-two-letters are, and is in the language it is most like.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::defaults::FloatType;
use crate::error::{Error, Result};

/// The languages known, in music21's order: English, French, Italian,
/// German, Chinese, Latin and Dutch.
pub const LANGUAGE_CODES: [&str; 7] = ["en", "fr", "it", "de", "cn", "la", "nl"];

/// The name of each language music21 knows, by its code.
pub fn language_name(code: &str) -> Option<&'static str> {
    Some(match code {
        "en" => "English",
        "fr" => "French",
        "it" => "Italian",
        "de" => "German",
        "cn" => "Chinese",
        "la" => "Latin",
        "nl" => "Dutch",
        _ => return None,
    })
}

fn excerpt(code: &str) -> &'static str {
    match code {
        "en" => include_str!("language/en.txt"),
        "fr" => include_str!("language/fr.txt"),
        "it" => include_str!("language/it.txt"),
        "de" => include_str!("language/de.txt"),
        "cn" => include_str!("language/cn.txt"),
        "la" => include_str!("language/la.txt"),
        _ => include_str!("language/nl.txt"),
    }
}

/// How often each letter follows each pair of letters in a text: music21's
/// `Trigram`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Trigram {
    counts: HashMap<(char, char), HashMap<char, u64>>,
    length: FloatType,
}

impl Trigram {
    /// The trigrams of a text, letter by letter, starting after two spaces.
    pub fn from_text(text: &str) -> Self {
        Self::counted(text.chars())
    }

    /// The trigrams of a list of words, each stripped and followed by a
    /// space: how music21 reads its excerpts.
    pub fn from_words<'a>(words: impl IntoIterator<Item = &'a str>) -> Self {
        Self::counted(
            words
                .into_iter()
                .flat_map(|word| word.trim().chars().chain(std::iter::once(' '))),
        )
    }

    fn counted(letters: impl Iterator<Item = char>) -> Self {
        let mut counts: HashMap<(char, char), HashMap<char, u64>> = HashMap::new();
        let mut pair = (' ', ' ');
        for letter in letters {
            *counts.entry(pair).or_default().entry(letter).or_insert(0) += 1;
            pair = (pair.1, letter);
        }
        let total: u64 = counts
            .values()
            .flat_map(HashMap::values)
            .map(|count| count * count)
            .sum();
        Self {
            counts,
            // Python's `total ** 0.5`.
            length: (total as FloatType).powf(0.5),
        }
    }

    /// The square root of the sum of every count squared: music21's
    /// `length`.
    pub fn length(&self) -> FloatType {
        self.length
    }

    /// How alike two texts' trigrams are, from nought to one: the cosine of
    /// their counts, music21's `similarity`.
    pub fn similarity(&self, other: &Trigram) -> FloatType {
        let mut total: u64 = 0;
        for (pair, ours) in &self.counts {
            if let Some(theirs) = other.counts.get(pair) {
                for (letter, count) in ours {
                    if let Some(their) = theirs.get(letter) {
                        total += count * their;
                    }
                }
            }
        }
        total as FloatType / (self.length * other.length)
    }

    /// One less [`Self::similarity`]: music21's `-`.
    pub fn distance(&self, other: &Trigram) -> FloatType {
        1.0 - self.similarity(other)
    }
}

/// The trigrams of each language's excerpt, read once.
fn known() -> &'static [Trigram; 7] {
    static KNOWN: OnceLock<[Trigram; 7]> = OnceLock::new();
    KNOWN.get_or_init(|| {
        LANGUAGE_CODES.map(|code| Trigram::from_words(excerpt(code).split_whitespace()))
    })
}

/// The language a text is most like, by its code, or nothing for an empty
/// text or one like none of them: music21's
/// `LanguageDetector.mostLikelyLanguage`. Where two are as alike, the first
/// of [`LANGUAGE_CODES`] wins.
///
/// ```
/// use music21_rs::features::language::most_likely_language;
///
/// let text = "Ich habe ein Buch, und ich gehe nach Hause mit meinem Bruder.";
/// assert_eq!(most_likely_language(text), Some("de"));
/// assert_eq!(most_likely_language(""), None);
/// ```
pub fn most_likely_language(text: &str) -> Option<&'static str> {
    if text.is_empty() {
        return None;
    }
    let trigram = Trigram::from_text(text);
    let mut best = None;
    let mut least = 1.0;
    for (code, known) in LANGUAGE_CODES.iter().zip(known()) {
        let distance = known.distance(&trigram);
        if distance < least {
            best = Some(*code);
            least = distance;
        }
    }
    best
}

/// The language a text is most like, as one more than its place in
/// [`LANGUAGE_CODES`], or nought for an empty text: music21's
/// `mostLikelyLanguageNumeric`.
///
/// # Errors
///
/// A text like none of the languages, which music21 refuses.
pub fn most_likely_language_numeric(text: &str) -> Result<usize> {
    if text.is_empty() {
        return Ok(0);
    }
    let code = most_likely_language(text).ok_or_else(|| {
        Error::Text("got a language that was not in the codes; should not happen".to_string())
    })?;
    Ok(LANGUAGE_CODES
        .iter()
        .position(|known| *known == code)
        .map_or(0, |at| at + 1))
}
