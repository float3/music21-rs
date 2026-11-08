//! The base-12 and base-26 orders of Hewlett's system, which music21's
//! `musedata.base12_26` keeps as tables only: base 12 numbers the
//! natural notes of the twelve semitones, base 26 every letter with one
//! flat or sharp.

/// music21's `base12Equivalent`: the natural note each of the twelve
/// numbers is, and the empty name for the five between.
pub const BASE12_EQUIVALENTS: [(i32, &str); 12] = [
    (0, "C"),
    (1, ""),
    (2, "D"),
    (3, ""),
    (4, "E"),
    (5, "F"),
    (6, ""),
    (7, "G"),
    (8, ""),
    (9, "A"),
    (10, ""),
    (11, "B"),
];

/// music21's `Base12IntervalTable`: the interval each distance in base 12
/// is; the tritone, 6, is not there.
pub const BASE12_INTERVALS: [(i32, &str); 11] = [
    (0, "P1"),
    (1, "m2"),
    (2, "M2"),
    (3, "m3"),
    (4, "M3"),
    (5, "P4"),
    (7, "P5"),
    (8, "m6"),
    (9, "M6"),
    (10, "m7"),
    (11, "M7"),
];

/// music21's `base26Equivalent`: the pitch name each of the twenty-six
/// numbers is, and the empty name for the four unused.
pub const BASE26_EQUIVALENTS: [(i32, &str); 26] = [
    (0, "C-"),
    (1, "C"),
    (2, "C#"),
    (3, ""),
    (4, "D-"),
    (5, "D"),
    (6, "D#"),
    (7, ""),
    (8, "E-"),
    (9, "E"),
    (10, "E#"),
    (11, "F-"),
    (12, "F"),
    (13, "F#"),
    (14, ""),
    (15, "G-"),
    (16, "G"),
    (17, "G#"),
    (18, ""),
    (19, "A-"),
    (20, "A"),
    (21, "A#"),
    (22, ""),
    (23, "B-"),
    (24, "B"),
    (25, "B#"),
];

/// music21's `Base26IntervalTable`: the interval each distance in base 26
/// is, for the eleven it names.
pub const BASE26_INTERVALS: [(i32, &str); 11] = [
    (0, "P1"),
    (3, "m2"),
    (4, "M2"),
    (7, "m3"),
    (8, "M3"),
    (11, "P4"),
    (15, "P5"),
    (18, "m6"),
    (19, "M6"),
    (22, "m7"),
    (23, "M7"),
];

/// The named rows of a table, which is what music21's `Representation`
/// dictionaries hold, turned round: each name against its number.
pub fn named<'a>(
    table: &'a [(i32, &'static str)],
) -> impl Iterator<Item = (&'static str, i32)> + 'a {
    table
        .iter()
        .filter(|(_, name)| !name.is_empty())
        .map(|(number, name)| (*name, *number))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_representations_leave_the_unused_numbers_out() {
        assert_eq!(named(&BASE12_EQUIVALENTS).count(), 7);
        assert_eq!(named(&BASE26_EQUIVALENTS).count(), 21);
        assert!(named(&BASE26_EQUIVALENTS).any(|row| row == ("F-", 11)));
    }
}
