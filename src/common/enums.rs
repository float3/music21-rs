//! The members of music21's `common.enums`, and the hexadecimal repr its
//! `HexEnum` gives.
//!
//! The enums themselves are Python classes; what is here is their members,
//! by music21's names and values, so that anything building them builds
//! music21's.

/// A member's value: music21's enums hold text, except `GatherSpanners`,
/// two of whose members are `True` and `False`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberValue {
    /// A boolean value.
    Bool(bool),
    /// A text value.
    Text(&'static str),
}

/// One member: its name and its value.
pub type Member = (&'static str, MemberValue);

use MemberValue::{Bool, Text};

/// `ElementSearch`: the directions `getContextByClass` can search in, each
/// the name of the `Stream` method it searches with.
pub const ELEMENT_SEARCH: &[Member] = &[
    ("BEFORE", Text("getElementBefore")),
    ("AFTER", Text("getElementAfter")),
    ("AT_OR_BEFORE", Text("getElementAtOrBefore")),
    ("AT_OR_AFTER", Text("getElementAtOrAfter")),
    ("BEFORE_OFFSET", Text("getElementBeforeOffset")),
    ("AFTER_OFFSET", Text("getElementAfterOffset")),
    ("AT_OR_BEFORE_OFFSET", Text("getElementAtOrBeforeOffset")),
    ("AT_OR_AFTER_OFFSET", Text("getElementAtOrAfterOffset")),
    ("BEFORE_NOT_SELF", Text("getElementBeforeNotSelf")),
    ("AFTER_NOT_SELF", Text("getElementAfterNotSelf")),
    ("ALL", Text("all")),
];

/// `OffsetSpecial`: offsets that are not numbers.
pub const OFFSET_SPECIAL: &[Member] = &[
    ("AT_END", Text("highestTime")),
    ("LOWEST_OFFSET", Text("lowestOffset")),
    ("HIGHEST_OFFSET", Text("highestOffset")),
];

/// `GatherSpanners`: which spanners an excerpt gathers.
pub const GATHER_SPANNERS: &[Member] = &[
    ("ALL", Bool(true)),
    ("NONE", Bool(false)),
    ("COMPLETE_ONLY", Text("completeOnly")),
];

/// `AppendSpanners`: whether the spanners an object starts or ends go out
/// with it when it is written.
pub const APPEND_SPANNERS: &[Member] = &[
    ("NORMAL", Text("normal")),
    ("RELATED_ONLY", Text("related_only")),
    ("NONE", Text("none")),
];

/// `OrnamentDelay`: the delays an ornament can have that are not a length.
pub const ORNAMENT_DELAY: &[Member] = &[
    ("NO_DELAY", Text("noDelay")),
    ("DEFAULT_DELAY", Text("defaultDelay")),
];

/// `MeterDivision`: how a time signature is divided.
pub const METER_DIVISION: &[Member] = &[
    ("FAST", Text("fast")),
    ("SLOW", Text("slow")),
    ("NONE", Text("none")),
];

/// The text with every run of decimal digits written in upper-case
/// hexadecimal after `0x`, which is how `HexEnum` turns an `IntEnum`'s
/// repr into its own. Runs of any length are converted.
///
/// ```
/// use music21::common::enums::hex_numbers;
///
/// assert_eq!(
///     hex_numbers("<ChannelVoiceMessages.NOTE_ON: 144>"),
///     "<ChannelVoiceMessages.NOTE_ON: 0x90>"
/// );
/// ```
pub fn hex_numbers(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut digits = String::new();
    for character in text.chars() {
        if character.is_ascii_digit() {
            digits.push(character);
            continue;
        }
        if !digits.is_empty() {
            out.push_str("0x");
            out.push_str(&decimal_to_hex(&digits));
            digits.clear();
        }
        out.push(character);
    }
    if !digits.is_empty() {
        out.push_str("0x");
        out.push_str(&decimal_to_hex(&digits));
    }
    out
}

/// A run of decimal digits in hexadecimal, however long the run.
fn decimal_to_hex(digits: &str) -> String {
    let mut number: Vec<u8> = digits.bytes().map(|digit| digit - b'0').collect();
    let mut hex = Vec::new();
    while number.iter().any(|&digit| digit != 0) {
        let mut remainder = 0u32;
        for digit in &mut number {
            let value = remainder * 10 + u32::from(*digit);
            *digit = u8::try_from(value / 16).unwrap_or(0);
            remainder = value % 16;
        }
        hex.push(
            char::from_digit(remainder, 16)
                .unwrap_or('0')
                .to_ascii_uppercase(),
        );
    }
    if hex.is_empty() {
        return "0".to_string();
    }
    hex.iter().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every run is converted, a class name's digits too, and a run too long
    /// for any integer type is converted all the same.
    #[test]
    fn every_digit_run_is_written_in_hex() {
        assert_eq!(hex_numbers("<A2.B: 0>"), "<A0x2.B: 0x0>");
        assert_eq!(hex_numbers("255"), "0xFF");
        assert_eq!(hex_numbers("007"), "0x7");
        assert_eq!(
            hex_numbers("340282366920938463463374607431768211456"),
            "0x100000000000000000000000000000000"
        );
        assert_eq!(hex_numbers("no digits ß"), "no digits ß");
    }

    /// Names are unique within each enum.
    #[test]
    fn member_names_are_unique() {
        for table in [
            ELEMENT_SEARCH,
            OFFSET_SPECIAL,
            GATHER_SPANNERS,
            APPEND_SPANNERS,
            ORNAMENT_DELAY,
            METER_DIVISION,
        ] {
            let mut names: Vec<_> = table.iter().map(|(name, _)| name).collect();
            names.sort();
            names.dedup();
            assert_eq!(names.len(), table.len());
        }
    }
}
