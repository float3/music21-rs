//! The signs of braille music: music21's `braille.lookup`.
//!
//! A cell is named by its raised dots, written as one number with the dots
//! in order: `1245` is dots one, two, four and five, `⠛`. The signs follow
//! Mary Turner De Garmo's *Introduction to Braille Music Transcription*
//! (2005), with some from Bettye Krolick's *New International Manual of
//! Braille Music Notation* where De Garmo has none.

/// The cell with these dots raised: music21's `brailleDotDict`. Nought is
/// the blank cell.
///
/// # Panics
///
/// A digit outside one to six.
pub const fn cell(dots: u32) -> char {
    let mut rest = dots;
    let mut bits = 0;
    while rest > 0 {
        let dot = rest % 10;
        assert!(dot >= 1 && dot <= 6, "a braille cell has dots one to six");
        bits |= 1 << (dot - 1);
        rest /= 10;
    }
    match char::from_u32(0x2800 + bits) {
        Some(cell) => cell,
        None => unreachable!(),
    }
}

/// The cells with these dots, one after another.
pub fn cells(dots: &[u32]) -> String {
    dots.iter().map(|dots| cell(*dots)).collect()
}

/// The cell holding every dot of these numbers together: music21's
/// `dotsAdd`.
pub fn dots_add(dots: &[u32]) -> char {
    let mut all: Vec<u32> = dots
        .iter()
        .flat_map(|number| {
            number
                .to_string()
                .chars()
                .filter_map(|digit| digit.to_digit(10))
                .filter(|digit| *digit != 0)
                .collect::<Vec<_>>()
        })
        .collect();
    all.sort_unstable();
    let joined = all.iter().fold(0, |number, digit| number * 10 + digit);
    cell(joined)
}

/// The dots a note's letter is written with.
fn step_dots(step: char) -> Option<u32> {
    Some(match step {
        'C' => 145,
        'D' => 15,
        'E' => 124,
        'F' => 1245,
        'G' => 125,
        'A' => 24,
        'B' => 245,
        _ => return None,
    })
}

/// A note of this letter and note value: music21's `pitchNameToNotes`. A
/// value and its sixteenfold share a sign; a breve and a longa are a whole
/// note written around one and two breve signs.
pub fn pitch_name_to_note(step: char, duration_type: &str) -> Option<String> {
    let step = step_dots(step)?;
    let value = |dots: u32| dots_add(&[step, dots]);
    let whole = value(36);
    Some(match duration_type {
        "128th" | "eighth" => value(0).to_string(),
        "64th" | "quarter" => value(6).to_string(),
        "32nd" | "half" => value(3).to_string(),
        "16th" | "whole" => whole.to_string(),
        "breve" => format!("{whole}{}{}{whole}", cell(45), cell(14)),
        "longa" => format!(
            "{whole}{}{}{}{}{whole}",
            cell(45),
            cell(14),
            cell(45),
            cell(14)
        ),
        _ => return None,
    })
}

/// The sign of an octave from nought to eight: music21's `octaves`.
pub fn octave(octave: i64) -> Option<String> {
    Some(match octave {
        0 => cells(&[4, 4]),
        1 => cells(&[4]),
        2 => cells(&[45]),
        3 => cells(&[456]),
        4 => cells(&[5]),
        5 => cells(&[46]),
        6 => cells(&[56]),
        7 => cells(&[6]),
        8 => cells(&[6, 6]),
        _ => return None,
    })
}

/// The sign of an accidental by music21's name for it: music21's
/// `accidentals`.
pub fn accidental(name: &str) -> Option<String> {
    let sharp = cell(146);
    let flat = cell(126);
    Some(match name {
        "sharp" => sharp.to_string(),
        "double-sharp" => format!("{sharp}{sharp}"),
        "triple-sharp" => format!("{sharp}{sharp}{sharp}"),
        "quadruple-sharp" => format!("{sharp}{sharp}{sharp}{sharp}"),
        "half-sharp" => format!("{}{sharp}", cell(4)),
        "one-and-a-half-sharp" => format!("{}{sharp}", cell(456)),
        "flat" => flat.to_string(),
        "double-flat" => format!("{flat}{flat}"),
        "triple-flat" => format!("{flat}{flat}{flat}"),
        "quadruple-flat" => format!("{flat}{flat}{flat}{flat}"),
        "half-flat" => format!("{}{flat}", cell(4)),
        "one-and-a-half-flat" => format!("{}{flat}", cell(456)),
        "natural" => cell(16).to_string(),
        _ => return None,
    })
}

/// The sign of an interval from a second to an octave: music21's
/// `intervals`.
pub fn interval(size: i64) -> Option<char> {
    Some(cell(match size {
        2 => 34,
        3 => 346,
        4 => 3456,
        5 => 35,
        6 => 356,
        7 => 25,
        8 => 36,
        _ => return None,
    }))
}

/// The key signature of so many sharps, or flats below nought: music21's
/// `keySignatures`.
pub fn key_signature(sharps: i64) -> Option<String> {
    let (count, sign) = if sharps < 0 {
        (-sharps, 126)
    } else {
        (sharps, 146)
    };
    signs_of(count, sign)
}

/// So many naturals cancelling a key signature: music21's `naturals`.
pub fn naturals(count: i64) -> Option<String> {
    signs_of(count, 16)
}

/// One to three signs written out, four to seven as a number and the sign.
fn signs_of(count: i64, sign: u32) -> Option<String> {
    Some(match count {
        0 => String::new(),
        1..=3 => cells(&vec![sign; count as usize]),
        4 => cells(&[3456, 145, sign]),
        5 => cells(&[3456, 15, sign]),
        6 => cells(&[3456, 124, sign]),
        7 => cells(&[3456, 1245, sign]),
        _ => return None,
    })
}

/// A digit in the upper cells, as a number is written: music21's
/// `numbersUpper`.
pub fn number_upper(digit: u32) -> Option<char> {
    Some(cell(match digit {
        0 => 245,
        1 => 1,
        2 => 12,
        3 => 14,
        4 => 145,
        5 => 15,
        6 => 124,
        7 => 1245,
        8 => 125,
        9 => 24,
        _ => return None,
    }))
}

/// A digit in the lower cells, as a time signature's denominator is written:
/// music21's `numbersLower`.
pub fn number_lower(digit: u32) -> Option<char> {
    Some(cell(match digit {
        0 => 356,
        1 => 2,
        2 => 23,
        3 => 25,
        4 => 256,
        5 => 26,
        6 => 235,
        7 => 2356,
        8 => 236,
        9 => 35,
        _ => return None,
    }))
}

/// The rest of a note value: music21's `rests`. `dummy` is the dot written
/// for a rest standing in for a missing part.
pub fn rest(duration_type: &str) -> Option<String> {
    Some(match duration_type {
        "dummy" => cells(&[3]),
        "128th" | "eighth" => cells(&[1346]),
        "64th" | "quarter" => cells(&[1236]),
        "32nd" | "half" => cells(&[136]),
        "16th" | "whole" => cells(&[134]),
        "breve" => cells(&[134, 45, 14, 134]),
        "longa" => cells(&[134, 45, 14, 45, 14, 134]),
        _ => return None,
    })
}

/// The sign saying which half of the note values follows: music21's
/// `lengthPrefixes`, `larger` for whole to eighth, `smaller` for 16th to
/// 128th and `xsmall` for shorter.
pub fn length_prefix(size: &str) -> Option<String> {
    Some(match size {
        "larger" => cells(&[45, 126, 2]),
        "smaller" => cells(&[6, 126, 2]),
        "xsmall" => cells(&[56, 126, 2]),
        _ => return None,
    })
}

/// The sign of a barline by music21's type: music21's `barlines`.
pub fn barline(barline_type: &str) -> Option<String> {
    Some(match barline_type {
        "final" => cells(&[126, 13]),
        "double" => cells(&[126, 13, 3]),
        "dashed" => cells(&[13]),
        "heavy" => cells(&[123]),
        _ => return None,
    })
}

/// The sign of a finger, one to five: music21's `fingerMarks`.
pub fn finger_mark(finger: &str) -> Option<char> {
    Some(cell(match finger {
        "1" => 1,
        "2" => 12,
        "3" => 123,
        "4" => 2,
        "5" => 13,
        _ => return None,
    }))
}

/// The sign starting a clef: music21's `clefs['prefix']`.
pub fn clef_prefix() -> char {
    cell(345)
}

/// The sign of a clef of this sign on this line: music21's `clefs`.
pub fn clef(sign: &str, line: i64) -> Option<String> {
    let (base, own_line) = match sign {
        "G" => (34, 2),
        "C" => (346, 3),
        "F" => (3456, 4),
        _ => return None,
    };
    if !(1..=5).contains(&line) {
        return None;
    }
    let mut written = cell(base).to_string();
    if line != own_line {
        written.push_str(&octave(line)?);
    }
    Some(written)
}

/// The sign ending a clef, the other for a keyboard hand switched: music21's
/// `clefs['suffix']`.
pub fn clef_suffix(keyboard_hand_switched: bool) -> char {
    if keyboard_hand_switched {
        cell(13)
    } else {
        cell(123)
    }
}

/// The sign of a bowing by music21's name: music21's `bowingSymbols`.
pub fn bowing(name: &str) -> Option<String> {
    Some(match name {
        "down bow" => cells(&[126, 12]),
        "up bow" => cells(&[126, 3]),
        _ => return None,
    })
}

/// The sign of an articulation written before its note, by music21's name:
/// music21's `beforeNoteExpr`.
pub fn before_note_expression(name: &str) -> Option<String> {
    Some(match name {
        "staccato" => cells(&[236]),
        "accent" => cells(&[46, 236]),
        "tenuto" => cells(&[456, 236]),
        "staccatissimo" => cells(&[6, 236]),
        "strong accent" => cells(&[56, 236]),
        "detached legato" => cells(&[5, 236]),
        _ => return None,
    })
}

/// The signs of the hairpin words written as text: music21's
/// `textExpressions`.
pub fn text_expression(words: &str) -> Option<String> {
    Some(match words {
        "crescendo" | "cresc." | "cr." => cells(&[345, 14, 1235, 3]),
        "decrescendo" | "decresc." | "decr." => cells(&[345, 145, 15, 14, 1235, 3]),
        _ => return None,
    })
}

/// A letter, a digit or a punctuation mark: music21's `alphabet`.
pub fn alphabet(character: char) -> Option<String> {
    if let Some(digit) = character.to_digit(10)
        && character.is_ascii_digit()
    {
        return number_upper(digit).map(String::from);
    }
    Some(match character {
        'a' => cells(&[1]),
        'b' => cells(&[12]),
        'c' => cells(&[14]),
        'd' => cells(&[145]),
        'e' => cells(&[15]),
        'f' => cells(&[124]),
        'g' => cells(&[1245]),
        'h' => cells(&[125]),
        'i' => cells(&[24]),
        'j' => cells(&[245]),
        'k' => cells(&[13]),
        'l' => cells(&[123]),
        'm' => cells(&[134]),
        'n' => cells(&[1345]),
        'o' => cells(&[135]),
        'p' => cells(&[1234]),
        'q' => cells(&[12345]),
        'r' => cells(&[1235]),
        's' => cells(&[234]),
        't' => cells(&[2345]),
        'u' => cells(&[136]),
        'v' => cells(&[1236]),
        'w' => cells(&[2456]),
        'x' => cells(&[1346]),
        'y' => cells(&[13456]),
        'z' => cells(&[1356]),
        ' ' => cell(0).to_string(),
        '!' => cells(&[235]),
        '\'' => cells(&[3]),
        ',' => cells(&[2]),
        '-' => cells(&[356]),
        '.' => cells(&[256]),
        '/' => cells(&[34]),
        ':' => cells(&[25]),
        '?' => cells(&[236]),
        '(' | ')' => cells(&[2356]),
        '^' => cells(&[4]),
        '[' => cells(&[6, 2356]),
        ']' => cells(&[2356, 3]),
        '*' => cells(&[35, 35]),
        _ => return None,
    })
}

/// A sign of a chord symbol: music21's `chordSymbols`.
pub fn chord_symbol(name: &str) -> Option<String> {
    Some(match name {
        "plus" => cells(&[346]),
        "minus" => cells(&[46]),
        "diminished_circle" => cells(&[256]),
        "half_diminished_circle" => cells(&[256, 3]),
        "triangle" => cells(&[356]),
        "triangle_line" => cells(&[356, 3]),
        "italics_seven" => cells(&[46, 3456, 1246]),
        "slash" => cells(&[34]),
        "parentheses" => cells(&[2356]),
        _ => return None,
    })
}

/// A sign by music21's name for it: music21's `symbols`.
///
/// # Panics
///
/// A name music21 has no sign for: every caller asks for one it has.
pub fn symbol(name: &str) -> String {
    match name {
        "space" => cells(&[0]),
        "double_space" => cells(&[0, 0]),
        "number" => cells(&[3456]),
        "letter_sign" => cells(&[56]),
        "dot" => cells(&[3]),
        "tie" => cells(&[4, 14]),
        "uppercase" => cells(&[6]),
        "metronome" => cells(&[2356]),
        "common" => cells(&[46, 14]),
        "cut" => cells(&[456, 14]),
        "music_hyphen" | "transcriber-added_sign" => cells(&[5]),
        "music_asterisk" => cells(&[345, 26, 35]),
        "rh_keyboard" => cells(&[46, 345]),
        "lh_keyboard" => cells(&[456, 345]),
        "word" => cells(&[345]),
        "triplet" => cells(&[23]),
        "tuplet_prefix" => cells(&[456]),
        "finger_change" => cells(&[14]),
        "first_set_missing_fingermark" => cells(&[6]),
        "second_set_missing_fingermark" => cells(&[3]),
        "opening_single_slur" => cells(&[14]),
        "opening_double_slur" => cells(&[14, 14]),
        "closing_double_slur" => cells(&[14]),
        "opening_bracket_slur" => cells(&[56, 12]),
        "closing_bracket_slur" => cells(&[45, 23]),
        "basic_exception" => cells(&[345, 236]),
        "full_inaccord" => cells(&[126, 345]),
        "repeat" => cells(&[2356]),
        "print-pagination" => cells(&[5, 25]),
        "braille-music-parenthesis" => cells(&[6, 3]),
        other => panic!("music21 has no braille sign named {other:?}"),
    }
}

/// The sign of a fermata of this shape: music21's `fermatas['shape']`.
pub fn fermata(shape: &str) -> Option<String> {
    Some(match shape {
        "normal" => cells(&[126, 123]),
        "angled" => cells(&[45, 126, 123]),
        "square" => cells(&[56, 126, 123]),
        _ => return None,
    })
}

/// The ASCII character standing for a cell in braille ASCII: music21's
/// `ascii_chars`.
pub fn ascii_char(braille: char) -> Option<char> {
    let bits = (braille as u32)
        .checked_sub(0x2800)
        .filter(|bits| *bits < 64)?;
    Some(ASCII[bits as usize])
}

/// Braille ASCII by the cell's dots as bits, dot one lowest.
const ASCII: [char; 64] = [
    ' ', 'A', '1', 'B', '\'', 'K', '2', 'L', '@', 'C', 'I', 'F', '/', 'M', 'S', 'P', '"', 'E', '3',
    'H', '9', 'O', '6', 'R', '^', 'D', 'J', 'G', '>', 'N', 'T', 'Q', ',', '*', '5', '<', '-', 'U',
    '8', 'V', '.', '%', '[', '$', '+', 'X', '!', '&', ';', ':', '4', '\\', '0', 'Z', '7', '(', '_',
    '?', 'W', ']', '#', 'Y', ')', '=',
];

/// Each row of a cell's dots, left and right, as `0` and `1`: music21's
/// `binary_dots`. music21 writes dots one, two, three, five and six as
/// `01 11 11`, the first row backwards, and so does this.
pub fn binary_dots(braille: char) -> Option<[&'static str; 3]> {
    let bits = (braille as u32)
        .checked_sub(0x2800)
        .filter(|bits| *bits < 64)?;
    if bits == 0b110111 {
        return Some(["01", "11", "11"]);
    }
    let row = |left: u32, right: u32| match (bits >> left & 1, bits >> right & 1) {
        (0, 0) => "00",
        (0, _) => "01",
        (_, 0) => "10",
        _ => "11",
    };
    Some([row(0, 3), row(1, 4), row(2, 5)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_are_named_by_their_dots() {
        // music21's makeBrailleDictionary doctest.
        assert_eq!(cell(124), '⠋');
        assert_eq!(cell(126), '⠣');
        assert_eq!(cell(0), '⠀');
        assert_eq!(dots_add(&[1245, 36]), '⠿');
        assert_eq!(pitch_name_to_note('C', "eighth").as_deref(), Some("⠙"));
        assert_eq!(pitch_name_to_note('C', "whole").as_deref(), Some("⠽"));
        assert_eq!(ascii_char(cell(1246)), Some('$'));
        assert_eq!(binary_dots(cell(12356)), Some(["01", "11", "11"]));
        assert_eq!(binary_dots(cell(1456)), Some(["11", "01", "01"]));
    }
}
