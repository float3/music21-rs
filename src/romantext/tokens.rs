//! RomanText's tokens as music21's `romanText.rtObjects` reads them: a
//! tagged line, a measure line and the atoms of a measure, each sorted by
//! the patterns music21 sorts them by.
//!
//! The reader in this crate's [`super`] module reads a whole document into
//! a score; these are the single steps, for those who look at the tokens
//! themselves.

use crate::common::numbertools::{OffsetQL, add_float_precision, float_ratio, op_frac};
use crate::common::stringtools::get_num_from_str;
use crate::error::Result;

use super::{measure_tag_end, roman_text_error};

/// What a tagged line's tag says it is: music21's `RTTagged.isComposer`,
/// `isTitle` and the rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tag {
    /// `Composer`.
    Composer,
    /// `Title`.
    Title,
    /// `Piece`.
    Piece,
    /// `Analyst`.
    Analyst,
    /// `Proofreader` or `Proof Reader`.
    Proofreader,
    /// `TimeSignature` or `Time Signature`, found among measures too.
    TimeSignature,
    /// `KeySignature` or `Key Signature`: so many sharps, or `Bb`.
    KeySignature,
    /// `Note`.
    Note,
    /// `Form`, found in a header now and then.
    Form,
    /// `Pedal`.
    Pedal,
    /// `RTVersion`: the version of RomanText the file follows.
    Version,
    /// `Work`, or `Madrigal` as it once was.
    Work,
    /// `Movement`: a new movement starts.
    Movement,
    /// A setting for how `vi` and `VI` read in minor.
    SixthMinor,
    /// A setting for how `vii` and `VII` read in minor.
    SeventhMinor,
}

/// The tag a line's tag names, any case, or nothing for another.
///
/// ```
/// use music21::romantext::tokens::{Tag, tag_of};
///
/// assert_eq!(tag_of("Time Signature"), Some(Tag::TimeSignature));
/// assert_eq!(tag_of("madrigal"), Some(Tag::Work));
/// assert_eq!(tag_of("Nothing"), None);
/// ```
pub fn tag_of(tag: &str) -> Option<Tag> {
    let tag = tag.to_lowercase();
    let found = match tag.as_str() {
        "composer" => Tag::Composer,
        "title" => Tag::Title,
        "piece" => Tag::Piece,
        "analyst" => Tag::Analyst,
        "proofreader" | "proof reader" => Tag::Proofreader,
        "timesignature" | "time signature" => Tag::TimeSignature,
        "keysignature" | "key signature" => Tag::KeySignature,
        "note" => Tag::Note,
        "form" => Tag::Form,
        "pedal" => Tag::Pedal,
        "rtversion" => Tag::Version,
        "work" | "madrigal" => Tag::Work,
        "movement" => Tag::Movement,
        "sixthminor" | "sixth minor" => Tag::SixthMinor,
        "seventhminor" | "seventh minor" => Tag::SeventhMinor,
        _ => return None,
    };
    Some(found)
}

/// A tagged line's tag and data, either side of its first colon and
/// stripped; a line without one is all data.
///
/// ```
/// use music21::romantext::tokens::split_tagged;
///
/// assert_eq!(split_tagged("Title: Die Jahrzeiten"), ("Title".into(), "Die Jahrzeiten".into()));
/// assert_eq!(split_tagged("no tag"), (String::new(), "no tag".into()));
/// ```
pub fn split_tagged(src: &str) -> (String, String) {
    match src.split_once(':') {
        Some((tag, data)) => (tag.trim().to_string(), data.trim().to_string()),
        None => (String::new(), src.to_string()),
    }
}

/// How much of a line its measure tag takes, `m3-4` of `m3-4 = m1-2`, or
/// nothing where it starts with none.
pub fn measure_tag_length(line: &str) -> Option<usize> {
    measure_tag_end(line)
}

/// The index of a document's first measure line, where its header ends.
pub fn header_end<S: AsRef<str>>(lines: &[S]) -> Option<usize> {
    lines
        .iter()
        .position(|line| measure_tag_end(line.as_ref().trim()).is_some())
}

/// A measure number, or the two of a range, and the repeat letter of each:
/// music21's `RTMeasure._getMeasureNumberData`.
///
/// ```
/// use music21::romantext::tokens::measure_number_data;
///
/// assert_eq!(measure_number_data("m77")?, (vec![77], vec![String::new()]));
/// assert_eq!(
///     measure_number_data("m123b-432b")?,
///     (vec![123, 432], vec!["b".to_string(), "b".to_string()])
/// );
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// More than one hyphen, or a part with no number in it.
pub fn measure_number_data(src: &str) -> Result<(Vec<i64>, Vec<String>)> {
    let parts: Vec<&str> = src.split('-').collect();
    if parts.len() > 2 {
        return Err(roman_text_error(format!(
            "too many values to unpack (expected 2) in {src:?}"
        )));
    }
    let mut numbers = Vec::new();
    let mut letters = Vec::new();
    for part in parts {
        let (digits, rest) = get_num_from_str(part, "0123456789");
        let number = digits.parse::<i64>().map_err(|_| {
            roman_text_error(format!(
                "invalid literal for int() with base 10: {digits:?}"
            ))
        })?;
        numbers.push(number);
        letters.push(rest.replace('m', ""));
    }
    Ok((numbers, letters))
}

/// A measure line read as music21's `RTMeasure` reads one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeasureLine {
    /// The measure number or range, `m3-4`.
    pub tag: String,
    /// What follows the tag and any variant mark.
    pub data: String,
    /// One or two measure numbers.
    pub number: Vec<i64>,
    /// The repeat letter of each number, empty for none.
    pub repeat_letter: Vec<String>,
    /// `var1`'s number: a variant reading of one measure.
    pub variant_number: Option<i64>,
    /// `varA`'s letters: a variant reading of a longer passage.
    pub variant_letter: Option<String>,
    /// Whether it copies other measures: `m4 = m1`.
    pub is_copy_definition: bool,
}

/// The length of a run at the start of `text` of characters that pass.
fn run_of(text: &str, test: impl Fn(char) -> bool) -> usize {
    text.char_indices()
        .find(|(_, character)| !test(*character))
        .map_or(text.len(), |(index, _)| index)
}

/// A measure line's parts.
///
/// ```
/// use music21::romantext::tokens::read_measure_line;
///
/// let line = read_measure_line("m15a V6 b1.5 V6/5 b2 I b3 viio6")?;
/// assert_eq!(line.data, "V6 b1.5 V6/5 b2 I b3 viio6");
/// assert_eq!(line.number, [15]);
/// assert_eq!(line.repeat_letter, ["a"]);
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// A line that starts with no measure tag, or whose tag's numbers cannot
/// be read.
pub fn read_measure_line(src: &str) -> Result<MeasureLine> {
    let Some(end) = measure_tag_end(src) else {
        return Err(roman_text_error(format!("found no measure tag: {src}")));
    };
    let tag = src[..end].trim().to_string();
    let raw = src[end..].trim();
    let (number, repeat_letter) = measure_number_data(&tag)?;
    let mut data = raw.to_string();
    let mut variant_number = None;
    let mut variant_letter = None;
    if let Some(rest) = raw.strip_prefix("var") {
        let digits = run_of(rest, |character| character.is_ascii_digit());
        let capitals = run_of(rest, |character| character.is_ascii_uppercase());
        if digits > 0 {
            let (found, _) = get_num_from_str(&rest[..digits], "0123456789");
            variant_number = found.parse().ok();
            data = rest[digits..].trim().to_string();
        }
        if capitals > 0 {
            variant_letter = Some(rest[..capitals].to_string());
            data = rest[capitals..].trim().to_string();
        }
    }
    let is_copy_definition = data.starts_with('=');
    Ok(MeasureLine {
        tag,
        data,
        number,
        repeat_letter,
        variant_number,
        variant_letter,
        is_copy_definition,
    })
}

/// The measures a copying measure line copies, from its data: music21's
/// `RTMeasure.getCopyTarget`.
///
/// # Errors
///
/// As [`measure_number_data`].
pub fn copy_target(data: &str) -> Result<(Vec<i64>, Vec<String>)> {
    measure_number_data(data.replace('=', "").trim())
}

/// What one word of a measure is: music21's subclasses of `RTAtom`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtomKind {
    /// `||`.
    PhraseBoundary,
    /// `(`.
    OpenParens,
    /// `)`.
    CloseParens,
    /// `b2.5`: the beat what follows stands on.
    Beat,
    /// `?(Bb:`: an optional key's start.
    OptionalKeyOpen,
    /// `?)Bb:`: an optional key's end.
    OptionalKeyClose,
    /// `g;:`: a key and its signature.
    Key,
    /// `g:`: the key chords are read in.
    AnalyticKey,
    /// `KS-2`: a key signature of so many sharps.
    KeySignature,
    /// `||:`.
    RepeatStart,
    /// `:||`.
    RepeatStop,
    /// `NC`, `N.C.` or `nc`.
    NoChord,
    /// Anything else: a roman numeral.
    Chord,
}

/// The length of a key's name at the start of a word: note letters, then
/// flats and sharps.
fn key_name_length(word: &str) -> usize {
    let letters = run_of(word, |character| matches!(character, 'A'..='G' | 'a'..='g'));
    if letters == 0 {
        return 0;
    }
    letters + run_of(&word[letters..], |character| matches!(character, 'b' | '#'))
}

/// Whether a word starts with a key's name and then `ending`.
fn starts_with_key(word: &str, ending: &str) -> bool {
    let length = key_name_length(word);
    length > 0 && word[length..].starts_with(ending)
}

/// What music21 reads a word of a measure as, tried in music21's order.
fn atom_kind(word: &str) -> AtomKind {
    let characters: Vec<char> = word.chars().collect();
    let is_beat = characters.first() == Some(&'b')
        && characters
            .get(1)
            .is_some_and(|next| matches!(next, '1'..='9' | '.'));
    let is_key_signature = word.strip_prefix("KS").is_some_and(|rest| {
        rest.strip_prefix('-')
            .unwrap_or(rest)
            .starts_with(|next: char| ('0'..='7').contains(&next))
    });
    // `.` in music21's pattern is any character.
    let is_no_chord = word.starts_with("NC")
        || word.starts_with("nc")
        || (characters.len() >= 4 && characters[0] == 'N' && characters[2] == 'C');
    match word {
        "||" => AtomKind::PhraseBoundary,
        "(" => AtomKind::OpenParens,
        ")" => AtomKind::CloseParens,
        _ if is_beat => AtomKind::Beat,
        _ if word
            .strip_prefix("?(")
            .is_some_and(|rest| starts_with_key(rest, ":")) =>
        {
            AtomKind::OptionalKeyOpen
        }
        _ if word
            .strip_prefix("?)")
            .is_some_and(|rest| key_name_length(rest) > 0) =>
        {
            AtomKind::OptionalKeyClose
        }
        _ if starts_with_key(word, ";:") => AtomKind::Key,
        _ if starts_with_key(word, ":") => AtomKind::AnalyticKey,
        _ if is_key_signature => AtomKind::KeySignature,
        _ if word.starts_with("||:") => AtomKind::RepeatStart,
        _ if word.starts_with(":||") => AtomKind::RepeatStop,
        _ if is_no_chord => AtomKind::NoChord,
        _ => AtomKind::Chord,
    }
}

/// The atoms of a measure's data, each word with what it is: music21's
/// `RTHandler.tokenizeAtoms`. A lone `=` ends them, since what follows it
/// is copied.
///
/// ```
/// use music21::romantext::tokens::{AtomKind, atoms};
///
/// assert_eq!(
///     atoms("IV b3 g: ii7"),
///     [
///         (AtomKind::Chord, "IV".to_string()),
///         (AtomKind::Beat, "b3".to_string()),
///         (AtomKind::AnalyticKey, "g:".to_string()),
///         (AtomKind::Chord, "ii7".to_string()),
///     ]
/// );
/// assert!(atoms("= m3").is_empty());
/// ```
pub fn atoms(line: &str) -> Vec<(AtomKind, String)> {
    let mut out = Vec::new();
    for word in line.split(' ') {
        let word = word.trim();
        if word.is_empty() {
            continue;
        }
        if word == "=" {
            break;
        }
        out.push((atom_kind(word), word.to_string()));
    }
    out
}

/// Python's `float()` of a decimal written `.digits`.
fn decimal(digits: &str) -> Result<f64> {
    format!(".{digits}")
        .parse::<f64>()
        .map_err(|_| roman_text_error(format!("could not convert string to float: '.{digits}'")))
}

/// A beat atom's beat, independent of any meter: music21's
/// `RTBeat.getBeatFloatOrFrac`. `b1.66` is beat one and two thirds, and a
/// third part is that much of the second's denominator: `b1.66.5` is half
/// a third further on.
///
/// ```
/// use music21::common::numbertools::OffsetQL;
/// use music21::romantext::tokens::beat_float_or_frac;
///
/// assert_eq!(beat_float_or_frac("b1.5")?, OffsetQL::Float(1.5));
/// assert_eq!(
///     beat_float_or_frac("b1.66.5")?,
///     OffsetQL::Fraction { numerator: 11, denominator: 6 }
/// );
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// More than three parts, or a part that is not a number.
pub fn beat_float_or_frac(src: &str) -> Result<OffsetQL> {
    let text = src.replace('b', "");
    let parts: Vec<&str> = text.split('.').collect();
    let main = parts[0].trim().parse::<i64>().map_err(|_| {
        roman_text_error(format!(
            "invalid literal for int() with base 10: {:?}",
            parts[0]
        ))
    })?;
    let mut fraction = OffsetQL::Float(0.0);
    if let Some(digits) = parts.get(1) {
        let value = decimal(digits)?;
        fraction = add_float_precision(value, 1e-2).unwrap_or(OffsetQL::Float(value));
    }
    let mut further = 0.0;
    if let Some(digits) = parts.get(2) {
        let divisor = decimal(digits)?;
        let denominator = match fraction {
            OffsetQL::Fraction { denominator, .. } => denominator as f64,
            OffsetQL::Float(value) => float_ratio(value)
                .and_then(|(_, denominator)| num::ToPrimitive::to_f64(&denominator))
                .unwrap_or(1.0),
        };
        further = op_frac(1.0 / (denominator / divisor))?.to_f64();
    }
    if parts.len() > 3 {
        return Err(roman_text_error(format!(
            "cannot handle specification: {src}"
        )));
    }
    op_frac(main as f64 + fraction.to_f64() + further)
}

/// The key an optional key's start or end names, as music21 writes a key:
/// `?(Bb:` names `B-`.
///
/// ```
/// use music21::romantext::tokens::optional_key_name;
///
/// assert_eq!(optional_key_name("?(Bb:"), "B-");
/// assert_eq!(optional_key_name("?)b:"), "b");
/// ```
pub fn optional_key_name(src: &str) -> String {
    if matches!(src, "?(b:" | "?)b:" | "?)b") {
        return "b".to_string();
    }
    src.replace('b', "-").replace([':', '?', '(', ')'], "")
}

/// The sharps a key signature atom gives, negative for flats: `KS-2` is
/// two flats.
///
/// # Errors
///
/// What follows `KS` not being a whole number.
pub fn key_signature_sharps(src: &str) -> Result<i64> {
    let digits = src.get(2..).unwrap_or_default();
    digits.trim().parse::<i64>().map_err(|_| {
        roman_text_error(format!(
            "invalid literal for int() with base 10: {digits:?}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_measure_line_reads_its_variant_and_copy() {
        let line = read_measure_line("m17var1 vi b2 IV").unwrap();
        assert_eq!(line.variant_number, Some(1));
        assert_eq!(line.data, "vi b2 IV");
        let line = read_measure_line("m17varC vi").unwrap();
        assert_eq!(line.variant_letter.as_deref(), Some("C"));
        assert_eq!(line.data, "vi");
        let line = read_measure_line("m3-4 = m1-2").unwrap();
        assert_eq!(line.number, [3, 4]);
        assert!(line.is_copy_definition);
        assert_eq!(copy_target(&line.data).unwrap().0, [1, 2]);
        assert!(read_measure_line("Title: x").is_err());
    }

    #[test]
    fn atoms_are_sorted_as_music21_sorts_them() {
        let kinds: Vec<AtomKind> = atoms("g;: ||: V b2 ?(Bb: VII7 ?)Bb: KS-2 :|| N.C. bVII (")
            .into_iter()
            .map(|(kind, _)| kind)
            .collect();
        assert_eq!(
            kinds,
            [
                AtomKind::Key,
                AtomKind::RepeatStart,
                AtomKind::Chord,
                AtomKind::Beat,
                AtomKind::OptionalKeyOpen,
                AtomKind::Chord,
                AtomKind::OptionalKeyClose,
                AtomKind::KeySignature,
                AtomKind::RepeatStop,
                AtomKind::NoChord,
                AtomKind::Chord,
                AtomKind::OpenParens,
            ]
        );
    }

    #[test]
    fn beats_read_thirds_and_parts_of_them() {
        let fraction = |numerator, denominator| OffsetQL::Fraction {
            numerator,
            denominator,
        };
        assert_eq!(beat_float_or_frac("b1.33").unwrap(), fraction(4, 3));
        assert_eq!(beat_float_or_frac("b1.2").unwrap(), fraction(6, 5));
        assert_eq!(
            beat_float_or_frac("b1.66.25").unwrap(),
            OffsetQL::Float(1.75)
        );
        assert_eq!(beat_float_or_frac("b1.66.75").unwrap(), fraction(23, 12));
        assert_eq!(beat_float_or_frac("b1.33.5").unwrap(), OffsetQL::Float(1.5));
        assert!(beat_float_or_frac("b1.2.3.4").is_err());
    }
}
