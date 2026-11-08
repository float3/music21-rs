//! music21's `common.stringTools`: names recased and split, text compared
//! and cleaned, and brackets matched.

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::canonical_combining_class;

use crate::error::{Error, Result};

/// Whether Python's `str.isspace` counts a character as white space, which
/// is Unicode's white space and the four ASCII separators below it.
pub fn is_python_space(character: char) -> bool {
    character.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&character)
}

/// A string as Python's `repr` writes it, in single quotes unless it holds
/// one and no double quote.
///
/// ```
/// use music21::common::stringtools::str_repr;
///
/// assert_eq!(str_repr("(\n"), "'(\\n'");
/// assert_eq!(str_repr("it's"), "\"it's\"");
/// ```
pub fn str_repr(text: &str) -> String {
    let quote = if text.contains('\'') && !text.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut written = String::new();
    written.push(quote);
    for character in text.chars() {
        match character {
            '\\' => written.push_str("\\\\"),
            '\n' => written.push_str("\\n"),
            '\r' => written.push_str("\\r"),
            '\t' => written.push_str("\\t"),
            _ if character == quote => {
                written.push('\\');
                written.push(character);
            }
            _ if character.is_control() || (is_python_space(character) && character != ' ') => {
                let code = u32::from(character);
                if code < 0x100 {
                    written.push_str(&format!("\\x{code:02x}"));
                } else if code < 0x10000 {
                    written.push_str(&format!("\\u{code:04x}"));
                } else {
                    written.push_str(&format!("\\U{code:08x}"));
                }
            }
            _ => written.push(character),
        }
    }
    written.push(quote);
    written
}

/// Whether two strings are the same once all their white space is taken
/// out: music21's `whitespaceEqual`.
///
/// ```
/// use music21::common::stringtools::whitespace_equal;
///
/// assert!(whitespace_equal("    hello \n there ", "hello there"));
/// assert!(!whitespace_equal("    hello \n there ", " bye there "));
/// ```
pub fn whitespace_equal(a: &str, b: &str) -> bool {
    a.chars()
        .filter(|character| !is_python_space(*character))
        .eq(b.chars().filter(|character| !is_python_space(*character)))
}

/// The characters of a string that are among `numbers`, and the rest,
/// each in order: music21's `getNumFromStr`, whose `numbers` are the ten
/// digits.
///
/// ```
/// use music21::common::stringtools::get_num_from_str;
///
/// let (numbers, rest) = get_num_from_str("23a954Hello", "0123456789");
/// assert_eq!((numbers.as_str(), rest.as_str()), ("23954", "aHello"));
/// ```
pub fn get_num_from_str(usr_str: &str, numbers: &str) -> (String, String) {
    let mut found = String::new();
    let mut remain = String::new();
    for ch in usr_str.chars() {
        if numbers.contains(ch) {
            found.push(ch);
        } else {
            remain.push(ch);
        }
    }
    (found, remain)
}

/// music21's `common.camelCaseToHyphen`: a camel-cased name as words joined
/// by `replacement`, lower case. A capital that starts a run of lower-case
/// letters is set apart first, then any capital following a lower-case
/// letter or a digit, which is the pair of substitutions music21 makes.
///
/// ```
/// use music21::common::stringtools::camel_case_to_hyphen;
///
/// assert_eq!(camel_case_to_hyphen("MovementName", '-'), "movement-name");
/// assert_eq!(camel_case_to_hyphen("fileName", '_'), "file_name");
/// ```
pub fn camel_case_to_hyphen(text: &str, replacement: char) -> String {
    let characters: Vec<char> = text.chars().collect();
    let mut first = String::new();
    let mut index = 0;
    while index < characters.len() {
        // The pattern's `.` is any character but a line break.
        let starts_word = characters[index] != '\n'
            && characters
                .get(index + 1)
                .is_some_and(char::is_ascii_uppercase)
            && characters
                .get(index + 2)
                .is_some_and(char::is_ascii_lowercase);
        if starts_word {
            first.push(characters[index]);
            first.push(replacement);
            first.push(characters[index + 1]);
            index += 2;
            while let Some(letter) = characters
                .get(index)
                .filter(|letter| letter.is_ascii_lowercase())
            {
                first.push(*letter);
                index += 1;
            }
        } else {
            first.push(characters[index]);
            index += 1;
        }
    }
    let characters: Vec<char> = first.chars().collect();
    let mut second = String::new();
    let mut index = 0;
    while index < characters.len() {
        let joins = (characters[index].is_ascii_lowercase() || characters[index].is_ascii_digit())
            && characters
                .get(index + 1)
                .is_some_and(char::is_ascii_uppercase);
        second.push(characters[index]);
        if joins {
            second.push(replacement);
            second.push(characters[index + 1]);
            index += 2;
        } else {
            index += 1;
        }
    }
    second.to_lowercase()
}

/// [`camel_case_to_hyphen`] with music21's checks on the replacement: one
/// character, and not a capital, which the result would lose.
///
/// # Errors
///
/// A replacement of more or less than one character, or a capital.
pub fn camel_case_to_hyphen_checked(text: &str, replacement: &str) -> Result<String> {
    let mut characters = replacement.chars();
    let (Some(character), None) = (characters.next(), characters.next()) else {
        return Err(Error::Value(
            "Replacement must be a single character.".to_string(),
        ));
    };
    if !character.to_lowercase().eq([character]) {
        return Err(Error::Value(
            "Replacement cannot be an uppercase character.".to_string(),
        ));
    }
    Ok(camel_case_to_hyphen(text, character))
}

/// A word with its first character in title case and the rest lowered, as
/// Python's `str.capitalize` writes it.
fn capitalize(word: &str) -> String {
    let mut characters = word.chars();
    let Some(first) = characters.next() else {
        return String::new();
    };
    let title = match first {
        'Ǆ' | 'ǅ' | 'ǆ' => "ǅ".to_string(),
        'Ǉ' | 'ǈ' | 'ǉ' => "ǈ".to_string(),
        'Ǌ' | 'ǋ' | 'ǌ' => "ǋ".to_string(),
        'Ǳ' | 'ǲ' | 'ǳ' => "ǲ".to_string(),
        _ => first.to_uppercase().collect(),
    };
    title + &characters.as_str().to_lowercase()
}

/// A `replacement`-joined name, `movement-name`, as a camel-cased one,
/// `movementName`: music21's `hyphenToCamelCase`.
///
/// ```
/// use music21::common::stringtools::hyphen_to_camel_case;
///
/// assert_eq!(hyphen_to_camel_case("movement_name", "_")?, "movementName");
/// assert_eq!(hyphen_to_camel_case("music-21", "-")?, "music21");
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// An empty replacement, which nothing can be split at.
pub fn hyphen_to_camel_case(text: &str, replacement: &str) -> Result<String> {
    if replacement.is_empty() {
        return Err(Error::Value("empty separator".to_string()));
    }
    let mut words = text.split(replacement);
    let mut joined = words.next().unwrap_or_default().to_string();
    for word in words {
        joined.push_str(&capitalize(word));
    }
    Ok(joined)
}

/// A camel-cased name, or one of letters and numbers, spaced into words:
/// music21's `spaceCamelCase`. A space goes before each capital and where
/// numbers start or stop; each of `keep_together`, spaced out, is joined up
/// again, and with `replace_underscore` an underscore is a space too.
/// music21 keeps `PMFC` together where it is given nothing to keep.
///
/// ```
/// use music21::common::stringtools::space_camel_case;
///
/// assert_eq!(space_camel_case("opus41no1", true, &["PMFC"]), "opus 41 no 1");
/// assert_eq!(space_camel_case("PMFC22", true, &["PMFC"]), "PMFC 22");
/// assert_eq!(space_camel_case("hello_myke", false, &["PMFC"]), "hello_myke");
/// ```
pub fn space_camel_case(text: &str, replace_underscore: bool, keep_together: &[&str]) -> String {
    const NUMBERS: &str = "0123456789.";
    let mut spaced = String::new();
    let mut last_is_number = false;
    for (index, character) in text.chars().enumerate() {
        let is_number = NUMBERS.contains(character);
        let first_number = is_number && !last_is_number;
        let first_letter = !is_number && last_is_number;
        if index > 0 && (character.is_uppercase() || first_number || first_letter) {
            spaced.push(' ');
        }
        spaced.push(character);
        last_is_number = is_number;
    }
    for kept in keep_together {
        let letters: Vec<String> = kept.chars().map(String::from).collect();
        spaced = spaced.replace(&letters.join(" "), kept);
    }
    if replace_underscore {
        spaced = spaced.replace('_', " ");
    }
    spaced
}

/// The MD5 digest of some bytes, in lower-case hexadecimal: music21's
/// `getMd5`, which hashes a string's UTF-8.
///
/// ```
/// use music21::common::stringtools::md5_hex;
///
/// assert_eq!(md5_hex(b"test"), "098f6bcd4621d373cade4e832627b4f6");
/// ```
pub fn md5_hex(data: &[u8]) -> String {
    const SHIFTS: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    let constants: Vec<u32> = (0..64)
        .map(|index: i32| (f64::from(index + 1).sin().abs() * 4_294_967_296.0) as u32)
        .collect();
    let mut message = data.to_vec();
    let bit_length = (data.len() as u64).wrapping_mul(8);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_length.to_le_bytes());
    let mut state: [u32; 4] = [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476];
    for block in message.chunks(64) {
        let words: Vec<u32> = block
            .chunks(4)
            .map(|word| u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
            .collect();
        let [mut a, mut b, mut c, mut d] = state;
        for index in 0..64 {
            let (mixed, word) = match index / 16 {
                0 => ((b & c) | (!b & d), index),
                1 => ((d & b) | (!d & c), (5 * index + 1) % 16),
                2 => (b ^ c ^ d, (3 * index + 5) % 16),
                _ => (c ^ (b | !d), (7 * index) % 16),
            };
            let rotated = a
                .wrapping_add(mixed)
                .wrapping_add(constants[index])
                .wrapping_add(words[word])
                .rotate_left(SHIFTS[index]);
            (a, d, c) = (d, c, b);
            b = b.wrapping_add(rotated);
        }
        for (held, added) in state.iter_mut().zip([a, b, c, d]) {
            *held = held.wrapping_add(added);
        }
    }
    state
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Text with its accents taken off: music21's `stripAccents`. Each
/// character is decomposed for compatibility and its combining marks are
/// dropped; `ß` becomes `ss` and curly quotes straight ones. A character
/// with nothing to take off, like `音`, is left as it is.
///
/// ```
/// use music21::common::stringtools::strip_accents;
///
/// assert_eq!(strip_accents("trés vite"), "tres vite");
/// assert_eq!(strip_accents("Süss, “êtré”"), "Suss, \"etre\"");
/// ```
pub fn strip_accents(text: &str) -> String {
    text.nfkd()
        .collect::<String>()
        .replace('ß', "ss")
        .replace(['“', '”'], "\"")
        .replace(['‘', '’'], "'")
        .chars()
        .filter(|character| canonical_combining_class(*character) == 0)
        .collect()
}

/// A name made safe as a file name: accents stripped, anything else
/// outside ASCII dropped, and what is not a letter, a digit, `_` or `-`
/// made `_`, all but a four-character extension: music21's
/// `normalizeFilename`.
///
/// ```
/// use music21::common::stringtools::normalize_filename;
///
/// assert_eq!(
///     normalize_filename("03-Niccolò all’lessandra.not really.xml"),
///     "03-Niccolo_all_lessandra_not_really.xml"
/// );
/// ```
pub fn normalize_filename(name: &str) -> String {
    let characters: Vec<char> = name.chars().collect();
    let length = characters.len();
    let (stem, extension) = if length > 5 && characters[length - 4] == '.' {
        (
            characters[..length - 4].iter().collect::<String>(),
            Some(characters[length - 4..].iter().collect::<String>()),
        )
    } else {
        (name.to_string(), None)
    };
    let mut normalized: String = strip_accents(&stem)
        .chars()
        .filter(char::is_ascii)
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect();
    if let Some(extension) = extension {
        normalized.push_str(&extension);
    }
    normalized
}

/// Text with ASCII's punctuation taken out: music21's
/// `removePunctuation`.
///
/// ```
/// use music21::common::stringtools::remove_punctuation;
///
/// assert_eq!(remove_punctuation("This, is! my (face)."), "This is my face");
/// ```
pub fn remove_punctuation(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_ascii_punctuation())
        .collect()
}

/// A bracketed stretch of text that [`parentheses_match`] found: music21's
/// `ParenthesesMatch`. Positions count characters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParenthesesMatch {
    /// Where the text inside starts, after the opening bracket.
    pub start: usize,
    /// Where the closing bracket starts.
    pub end: usize,
    /// The text between the brackets.
    pub text: String,
    /// The matches inside this one, in order.
    pub nested: Vec<ParenthesesMatch>,
}

/// Every bracketed stretch of a string, each with those nested in it:
/// music21's `parenthesesMatch`. A bracket after a backslash is not one,
/// and either bracket may be longer than a character.
///
/// ```
/// use music21::common::stringtools::parentheses_match;
///
/// let found = parentheses_match("[not [mix] very] [well.]", "[", "]")?;
/// assert_eq!(found[0].text, "not [mix] very");
/// assert_eq!(found[0].nested[0].text, "mix");
/// assert_eq!((found[1].start, found[1].end), (18, 23));
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// An empty bracket, or one left open or closed that was never opened.
pub fn parentheses_match(text: &str, open: &str, close: &str) -> Result<Vec<ParenthesesMatch>> {
    if open.is_empty() || close.is_empty() {
        return Err(Error::Value(
            "Neither open nor close can be empty.".to_string(),
        ));
    }
    let characters: Vec<char> = text.chars().collect();
    let open: Vec<char> = open.chars().collect();
    let close: Vec<char> = close.chars().collect();
    let at = |index: usize, bracket: &[char]| {
        characters.get(index..index + bracket.len()) == Some(bracket)
    };
    let mut stack = vec![ParenthesesMatch {
        start: 0,
        end: 0,
        text: String::new(),
        nested: Vec::new(),
    }];
    let mut escaped = false;
    let mut index = 0;
    while index < characters.len() {
        if !escaped && at(index, &open) {
            stack.push(ParenthesesMatch {
                start: index + open.len(),
                end: 0,
                text: String::new(),
                nested: Vec::new(),
            });
            index += open.len();
            continue;
        }
        if !escaped && at(index, &close) {
            if stack.len() <= 1 {
                return Err(Error::Value(format!(
                    "Closing {} without {} at index {index}.",
                    str_repr(&close.iter().collect::<String>()),
                    str_repr(&open.iter().collect::<String>())
                )));
            }
            let mut found = stack.pop().unwrap_or_else(|| unreachable!());
            found.end = index;
            found.text = characters[found.start..index].iter().collect();
            if let Some(outer) = stack.last_mut() {
                outer.nested.push(found);
            }
            index += close.len();
            continue;
        }
        escaped = characters[index] == '\\' && !escaped;
        index += 1;
    }
    if let Some(unclosed) = stack.get(1) {
        return Err(Error::Value(format!(
            "Opening {} at index {} was never closed",
            str_repr(&open.iter().collect::<String>()),
            unclosed.start - 1
        )));
    }
    Ok(stack.pop().map(|outer| outer.nested).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// music21's own examples for `camelCaseToHyphen`.
    #[test]
    fn camel_case_is_split_as_music21_splits_it() {
        assert_eq!(camel_case_to_hyphen("movementName", '-'), "movement-name");
        assert_eq!(
            camel_case_to_hyphen("movementNameName", '_'),
            "movement_name_name"
        );
        assert_eq!(camel_case_to_hyphen("fileName", '-'), "file-name");
        assert_eq!(camel_case_to_hyphen("fileNameABC", '-'), "file-name-abc");
        assert_eq!(camel_case_to_hyphen("Movement", '-'), "movement");
        assert_eq!(camel_case_to_hyphen("SnapPizzicato", ' '), "snap pizzicato");
        assert_eq!(camel_case_to_hyphen("OrganHeel", ' '), "organ heel");
        assert_eq!(camel_case_to_hyphen("ABCd", '-'), "ab-cd");
        assert_eq!(camel_case_to_hyphen("AbCdEf", '-'), "ab-cd-ef");
        assert_eq!(camel_case_to_hyphen("x2Y", '-'), "x2-y");
    }

    #[test]
    fn a_line_break_starts_no_word() {
        assert_eq!(camel_case_to_hyphen("a\nBc", '-'), "a\nbc");
    }

    #[test]
    fn replacements_music21_refuses() {
        assert_eq!(
            camel_case_to_hyphen_checked("fileName", "NotFound")
                .unwrap_err()
                .to_string(),
            "Value error: Replacement must be a single character."
        );
        assert!(camel_case_to_hyphen_checked("fileName", "A").is_err());
        assert_eq!(
            camel_case_to_hyphen_checked("fileName", "_").unwrap(),
            "file_name"
        );
        assert!(hyphen_to_camel_case("a-b", "").is_err());
        assert_eq!(hyphen_to_camel_case("voice", "-").unwrap(), "voice");
    }

    #[test]
    fn camel_case_is_spaced_as_music21_spaces_it() {
        assert_eq!(
            space_camel_case("thisIsATest", true, &["PMFC"]),
            "this Is A Test"
        );
        assert_eq!(space_camel_case("movement3", true, &["PMFC"]), "movement 3");
        assert_eq!(
            space_camel_case("opus23402no219235", true, &["PMFC"]),
            "opus 23402 no 219235"
        );
        assert_eq!(
            space_camel_case("hello_myke", true, &["PMFC"]),
            "hello myke"
        );
    }

    #[test]
    fn md5_of_rfc_1321s_examples() {
        assert_eq!(md5_hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5_hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
        let long =
            b"12345678901234567890123456789012345678901234567890123456789012345678901234567890";
        assert_eq!(md5_hex(long), "57edf4a22be3c955ac49da2e2107b67a");
    }

    #[test]
    fn accents_come_off_and_the_rest_stays() {
        assert_eq!(strip_accents("Muß"), "Muss");
        assert_eq!(strip_accents("音楽"), "音楽");
        assert!(whitespace_equal("a\u{1c}b", "ab"));
    }

    #[test]
    fn brackets_matched_or_refused() {
        let found = parentheses_match(
            r"Bologne wrote (a (whole) (lot) \(of\)) sym\(ph(on)ies\) concertantes.",
            "(",
            ")",
        )
        .unwrap();
        assert_eq!((found[0].start, found[0].end), (15, 37));
        assert_eq!(found[0].nested.len(), 2);
        assert_eq!(
            (found[1].start, found[1].end, found[1].text.as_str()),
            (47, 49, "on")
        );
        let found = parentheses_match(
            "Did you eat <<beans>> today <<Pythagoreas<<?>>>>",
            "<<",
            ">>",
        )
        .unwrap();
        assert_eq!(found[1].text, "Pythagoreas<<?>>");
        assert_eq!(found[1].nested[0].start, 43);
        let unclosed = parentheses_match("My (parentheses (sometimes (continue", "(", ")");
        assert_eq!(
            unclosed.unwrap_err().to_string(),
            "Value error: Opening '(' at index 3 was never closed"
        );
        let unopened = parentheses_match("This is a <bad> example>", "<", ">");
        assert_eq!(
            unopened.unwrap_err().to_string(),
            "Value error: Closing '>' without '<' at index 23."
        );
    }

    #[test]
    fn test_basic() {
        let (nums, rest) = get_num_from_str("23a", "0123456789");
        assert_eq!(nums, "23");
        assert_eq!(rest, "a");
    }

    #[test]
    fn test_multiple_numbers() {
        let (nums, rest) = get_num_from_str("23a954Hello", "0123456789");
        assert_eq!(nums, "23954");
        assert_eq!(rest, "aHello");
    }

    #[test]
    fn test_empty_string() {
        let (nums, rest) = get_num_from_str("", "0123456789");
        assert_eq!(nums, "");
        assert_eq!(rest, "");
    }

    #[test]
    fn test_no_numbers() {
        let (nums, rest) = get_num_from_str("Hello", "0123456789");
        assert_eq!(nums, "");
        assert_eq!(rest, "Hello");
    }
}
