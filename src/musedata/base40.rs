//! Walter Hewlett's base-40 pitch numbers: music21's `musedata.base40`.
//!
//! Base 40 numbers every pitch from a double flat to a double sharp on each
//! of the seven letters, forty to the octave, leaving a number unused
//! between two letters a whole step apart. Its gift is that the distance
//! between two numbers names the interval between the pitches, spelling
//! and all: 23 is always a perfect fifth.
//!
//! ```
//! use music21::musedata::base40;
//!
//! assert_eq!(base40::pitch_name(186), Some("G"));
//! assert_eq!(base40::from_pitch_name("F##", 4), Some(142));
//! assert_eq!(base40::delta_to_interval(-52).unwrap().directed_name(), "M-10");
//! ```

use std::fmt;

use crate::interval::Interval;
use crate::pitch::Pitch;

/// music21's `base40Equivalent`: the pitch name each number of an octave
/// stands for, from 1 to 40, and the empty name for the five unused.
pub const EQUIVALENTS: [(i32, &str); 40] = [
    (1, "C--"),
    (2, "C-"),
    (3, "C"),
    (4, "C#"),
    (5, "C##"),
    (6, ""),
    (7, "D--"),
    (8, "D-"),
    (9, "D"),
    (10, "D#"),
    (11, "D##"),
    (12, ""),
    (13, "E--"),
    (14, "E-"),
    (15, "E"),
    (16, "E#"),
    (17, "E##"),
    (18, "F--"),
    (19, "F-"),
    (20, "F"),
    (21, "F#"),
    (22, "F##"),
    (23, ""),
    (24, "G--"),
    (25, "G-"),
    (26, "G"),
    (27, "G#"),
    (28, "G##"),
    (29, ""),
    (30, "A--"),
    (31, "A-"),
    (32, "A"),
    (33, "A#"),
    (34, "A##"),
    (35, ""),
    (36, "B--"),
    (37, "B-"),
    (38, "B"),
    (39, "B#"),
    (40, "B##"),
];

/// music21's `base40IntervalTable`: the simple interval each distance
/// within an octave is. A distance missing here is no interval base 40
/// can say.
pub const INTERVALS: [(i32, &str); 25] = [
    (0, "P1"),
    (1, "A1"),
    (4, "d2"),
    (5, "m2"),
    (6, "M2"),
    (7, "A2"),
    (10, "d3"),
    (11, "m3"),
    (12, "M3"),
    (13, "A3"),
    (16, "d4"),
    (17, "P4"),
    (18, "A4"),
    (22, "d5"),
    (23, "P5"),
    (24, "A5"),
    (27, "d6"),
    (28, "m6"),
    (29, "M6"),
    (30, "A6"),
    (33, "d7"),
    (34, "m7"),
    (35, "M7"),
    (36, "A7"),
    (39, "d8"),
];

/// Why a base-40 question has no answer, in music21's words: what music21
/// raises as `Base40Exception`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Base40Error(String);

impl Base40Error {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }

    /// What went wrong, as music21 says it.
    pub fn message(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Base40Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Base40Error {}

/// The pitch name a number of the octave (1 to 40) stands for, the empty
/// name for one unused, and `None` for a number outside the octave.
pub fn equivalent(number: i32) -> Option<&'static str> {
    EQUIVALENTS
        .iter()
        .find(|(known, _)| *known == number)
        .map(|(_, name)| *name)
}

/// The pitch name a base-40 number stands for, in whatever octave: `None`
/// for one of the unused numbers.
pub fn pitch_name(number: i32) -> Option<&'static str> {
    equivalent((number - 1).rem_euclid(40) + 1).filter(|name| !name.is_empty())
}

/// music21's `base40Representation`: the number of the octave a pitch name
/// is, for the thirty-five names base 40 has.
pub fn representation(name: &str) -> Option<i32> {
    EQUIVALENTS
        .iter()
        .find(|(_, known)| !known.is_empty() && *known == name)
        .map(|(number, _)| *number)
}

/// The base-40 number of a pitch name in an octave, as music21's
/// `pitchToBase40` numbers it: octave 1's C double flat is 1.
pub fn from_pitch_name(name: &str, octave: i32) -> Option<i32> {
    representation(name).map(|number| 40 * (octave - 1) + number)
}

/// The octave and the number within it music21's `base40ToPitch` reads a
/// base-40 number as. music21 divides with a float and lets the octave
/// setter drop the fraction, so the octave is the quotient cut toward
/// nought, and the number within it is what is left after that many
/// octaves.
pub fn octave_and_number(number: i32) -> (i32, i32) {
    let octave = (f64::from(number - 1) / 40.0 + 1.0).trunc() as i32;
    (octave, number - 40 * (octave - 1))
}

/// music21's `base40ToPitch`: the pitch a base-40 number is.
///
/// # Errors
///
/// A number base 40 leaves unused.
pub fn to_pitch(number: i32) -> Result<Pitch, Base40Error> {
    let refused = || {
        Base40Error::new(format!(
            "Pitch name not assigned to this Base40 number {number}"
        ))
    };
    let (octave, within) = octave_and_number(number);
    let name = equivalent(within)
        .filter(|name| !name.is_empty())
        .ok_or_else(refused)?;
    Pitch::from_name_and_octave(name, octave).map_err(|_| refused())
}

/// music21's `pitchToBase40`: a pitch's base-40 number.
///
/// # Errors
///
/// A pitch base 40 has no number for: a triple sharp, a quarter tone, or
/// one with no octave.
pub fn from_pitch(pitch: &Pitch) -> Result<i32, Base40Error> {
    let number = pitch
        .octave()
        .and_then(|octave| from_pitch_name(&music21_name(pitch)?, octave));
    number.ok_or_else(|| {
        Base40Error::new(format!(
            "Base40 cannot handle this pitch {}",
            pitch.name_with_octave()
        ))
    })
}

/// A pitch's name spelled as music21 spells it, a flat as `-`, for the
/// accidentals base 40 has: none for a quarter tone or a triple.
fn music21_name(pitch: &Pitch) -> Option<String> {
    let alter = pitch.alter();
    let modifier = match alter {
        -2.0 => "--",
        -1.0 => "-",
        0.0 => "",
        1.0 => "#",
        2.0 => "##",
        _ => return None,
    };
    let step = pitch.name().chars().next()?;
    Some(format!("{step}{modifier}"))
}

/// music21's `base40DeltaToInterval`: the interval a distance between two
/// base-40 numbers is, its octaves counted and its direction kept.
///
/// # Errors
///
/// A distance that is no interval base 40 can say.
pub fn delta_to_interval(delta: i32) -> Result<Interval, Base40Error> {
    let simple = delta.abs() % 40;
    let name = INTERVALS
        .iter()
        .find(|(distance, _)| *distance == simple)
        .map(|(_, name)| *name)
        .ok_or_else(|| Base40Error::new(format!("Interval not handled by Base40 {simple}")))?;
    let (specifier, number) = name.split_at(1);
    let number: i32 = number.parse().unwrap_or(1);
    let generic = number + 7 * (delta.abs() / 40);
    let generic = if delta < 0 { -generic } else { generic };
    Interval::from_name(format!("{specifier}{generic}"))
        .map_err(|error| Base40Error::new(error.to_string()))
}

/// music21's `base40Interval`: the interval between two base-40 numbers,
/// read from their distance alone.
///
/// # Errors
///
/// Either number unused, or a distance base 40 cannot say -- which it
/// cannot past an augmented or diminished interval on one letter.
pub fn interval(from: i32, to: i32) -> Result<Interval, Base40Error> {
    let first = pitch_name(from);
    let second = pitch_name(to);
    match (first, second) {
        (None, None) => {
            return Err(Base40Error::new(format!(
                "Pitch name not assigned to these Base40 numbers {from} and {to} \
                 Interval does not exist"
            )));
        }
        (None, _) => {
            return Err(Base40Error::new(format!(
                "Pitch name not assigned to this Base40 number {from} Interval does not exist"
            )));
        }
        (_, None) => {
            return Err(Base40Error::new(format!(
                "Pitch name not assigned to this Base40 number {to} Interval does not exist"
            )));
        }
        _ => {}
    }
    let delta = to - from;
    let same_letter =
        first.and_then(|name| name.chars().next()) == second.and_then(|name| name.chars().next());
    if delta > 3 && same_letter {
        return Err(Base40Error::new(format!(
            "Base40 cannot compute interval between {from} and {to}."
        )));
    }
    delta_to_interval(delta)
}

/// music21's `base40ActualInterval`: the interval between the pitches two
/// base-40 numbers are, worked out from the pitches rather than read from
/// the distance.
///
/// # Errors
///
/// Either number unused, or an interval no specifier can name.
pub fn actual_interval(from: i32, to: i32) -> Result<Interval, Base40Error> {
    let start = to_pitch(from)?;
    let end = to_pitch(to)?;
    Interval::between_pitches(&start, &end)
        .map_err(|_| Base40Error::new("Unusual interval - Limitation of music21.interval"))
}

/// The way an enharmonic is looked for: a diminished second up or down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Higher spellings: the next letter up.
    Up,
    /// Lower spellings: the next letter down.
    Down,
}

/// music21's `_quickEnharmonicString`: the other spellings of a pitch name
/// one way, each a diminished second from the last, until a letter comes
/// round again or base 40 runs out of names. `double_accidentals` false
/// leaves the double sharps and flats out.
pub fn quick_enharmonics(
    name: &str,
    direction: Direction,
    double_accidentals: bool,
) -> Vec<String> {
    let Some(mut number) = representation(name) else {
        return Vec::new();
    };
    let step = match direction {
        Direction::Up => 4,
        Direction::Down => -4,
    };
    let letter = name.chars().next();
    let mut found: Vec<String> = Vec::new();
    loop {
        number = (number + step).rem_euclid(40);
        if number == 0 {
            number = 40;
        }
        let spelled = equivalent(number)
            .filter(|spelled| double_accidentals || spelled.len() <= 2)
            .unwrap_or("");
        let first = spelled.chars().next();
        if spelled.is_empty() || first == letter {
            return found;
        }
        if found.iter().any(|known| known.chars().next() == first) {
            return found;
        }
        found.push(spelled.to_string());
    }
}

/// music21's `quickEnharmonicString`: the lower spellings, then the higher.
pub fn quick_enharmonic_string(name: &str, double_accidentals: bool) -> Vec<String> {
    let mut found = quick_enharmonics(name, Direction::Down, double_accidentals);
    found.extend(quick_enharmonics(name, Direction::Up, double_accidentals));
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_is_a_pitch_and_back() {
        let lowest = to_pitch(1).unwrap();
        assert_eq!((lowest.alter(), lowest.octave()), (-2.0, Some(1)));
        assert_eq!(to_pitch(40).unwrap().name_with_octave(), "B##1");
        assert_eq!(from_pitch(&Pitch::from_name("B-4").unwrap()).unwrap(), 157);
        assert_eq!(to_pitch(186).unwrap().name_with_octave(), "G5");
        assert_eq!(
            to_pitch(23).unwrap_err().message(),
            "Pitch name not assigned to this Base40 number 23"
        );
        let pitch = Pitch::from_name("C--5").unwrap();
        assert_eq!(from_pitch(&pitch).unwrap(), 161);
        let pitch = Pitch::from_name("F###4").unwrap();
        assert_eq!(
            from_pitch(&pitch).unwrap_err().message(),
            "Base40 cannot handle this pitch F###4"
        );
    }

    #[test]
    fn the_octave_is_cut_toward_nought() {
        assert_eq!(octave_and_number(0), (0, 40));
        assert_eq!(octave_and_number(-40), (0, 0));
        assert_eq!(octave_and_number(41), (2, 1));
    }

    #[test]
    fn a_distance_is_an_interval_with_its_octaves() {
        assert_eq!(delta_to_interval(-11).unwrap().directed_name(), "m-3");
        assert_eq!(delta_to_interval(63).unwrap().directed_name(), "P12");
        assert_eq!(
            delta_to_interval(77).unwrap_err().message(),
            "Interval not handled by Base40 37"
        );
    }

    #[test]
    fn an_interval_between_numbers_needs_both_named() {
        assert_eq!(interval(163, 191).unwrap().directed_name(), "m6");
        assert_eq!(interval(186, 174).unwrap().directed_name(), "M-3");
        assert_eq!(
            interval(1, 5).unwrap_err().message(),
            "Base40 cannot compute interval between 1 and 5."
        );
        assert_eq!(
            interval(12, 6).unwrap_err().message(),
            "Pitch name not assigned to these Base40 numbers 12 and 6 Interval does not exist"
        );
        assert_eq!(actual_interval(1, 5).unwrap().directed_name(), "AAAA1");
    }

    #[test]
    fn enharmonics_stop_where_a_letter_comes_round() {
        assert_eq!(quick_enharmonics("C##", Direction::Up, true), ["D", "E--"]);
        assert_eq!(quick_enharmonics("C##", Direction::Up, false), ["D"]);
        assert_eq!(quick_enharmonics("C-", Direction::Down, true), ["B", "A##"]);
        assert_eq!(quick_enharmonic_string("G", true), ["F##", "A--"]);
        assert!(quick_enharmonics("C###", Direction::Up, true).is_empty());
    }
}
