//! music21's `harmony.CHORD_TYPES`: every kind of chord a lead-sheet symbol
//! names, with its notation over the root and the abbreviations it is
//! written with, and the intervals that notation stands for.

use super::*;

/// A chord type from music21's `harmony.CHORD_TYPES` table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Music21ChordType {
    /// music21's kind name, such as `"dominant-seventh"`.
    pub kind: &'static str,
    /// Scale-degree notation, such as `"1,3,5,-7"`.
    pub notation: &'static str,
    /// music21's first abbreviation for this kind, such as `"7"`, which is
    /// the one it uses when writing a figure.
    pub abbreviation: &'static str,
    /// Every abbreviation music21 accepts for this kind, first one first.
    pub abbreviations: &'static [&'static str],
}

/// Returns every chord type this crate knows from music21's harmony tables.
///
/// The table mirrors music21's `harmony.CHORD_TYPES` and is verified against it
/// by `python-parity`'s `chord_type_parity` test.
pub fn known_chord_symbol_types() -> &'static [Music21ChordType] {
    MUSIC21_CHORD_TYPES
}

pub(super) fn chord_type_named(kind: &str) -> Option<&'static Music21ChordType> {
    MUSIC21_CHORD_TYPES
        .iter()
        .find(|chord_type| chord_type.kind == kind)
}

/// Every abbreviation music21 accepts for a chord kind: music21's
/// `getAbbreviationListGivenChordType`, so `dominant-seventh` gives `7` and
/// `dom7`. `None` for an unknown kind.
pub fn abbreviations_for_kind(kind: &str) -> Option<&'static [&'static str]> {
    chord_type_named(kind).map(|chord_type| chord_type.abbreviations)
}

/// The scale-degree notation of a chord kind: music21's
/// `getNotationStringGivenChordType`, `1,3,5,-7` for `dominant-seventh`.
pub fn notation_for_kind(kind: &str) -> Option<&'static str> {
    chord_type_named(kind).map(|chord_type| chord_type.notation)
}

/// The abbreviation music21 writes a chord kind with: its
/// `getCurrentAbbreviationFor`, the first of [`abbreviations_for_kind`].
pub fn current_abbreviation_for_kind(kind: &str) -> Option<&'static str> {
    chord_type_named(kind).map(|chord_type| chord_type.abbreviation)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Music21Degree {
    degree: u8,
    semitone: u8,
}

pub(super) const MUSIC21_CHORD_TYPES: &[Music21ChordType] = &[
    Music21ChordType {
        kind: "major",
        notation: "1,3,5",
        abbreviation: "",
        abbreviations: &["", "M", "maj"],
    },
    Music21ChordType {
        kind: "minor",
        notation: "1,-3,5",
        abbreviation: "m",
        abbreviations: &["m", "min"],
    },
    Music21ChordType {
        kind: "augmented",
        notation: "1,3,#5",
        abbreviation: "+",
        abbreviations: &["+", "aug"],
    },
    Music21ChordType {
        kind: "diminished",
        notation: "1,-3,-5",
        abbreviation: "dim",
        abbreviations: &["dim", "o"],
    },
    Music21ChordType {
        kind: "dominant-seventh",
        notation: "1,3,5,-7",
        abbreviation: "7",
        abbreviations: &["7", "dom7"],
    },
    Music21ChordType {
        kind: "major-seventh",
        notation: "1,3,5,7",
        abbreviation: "maj7",
        abbreviations: &["maj7", "M7"],
    },
    Music21ChordType {
        kind: "minor-major-seventh",
        notation: "1,-3,5,7",
        abbreviation: "mM7",
        abbreviations: &["mM7", "m#7", "minmaj7"],
    },
    Music21ChordType {
        kind: "minor-seventh",
        notation: "1,-3,5,-7",
        abbreviation: "m7",
        abbreviations: &["m7", "min7"],
    },
    Music21ChordType {
        kind: "augmented-major-seventh",
        notation: "1,3,#5,7",
        abbreviation: "+M7",
        abbreviations: &["+M7", "augmaj7"],
    },
    Music21ChordType {
        kind: "augmented-seventh",
        notation: "1,3,#5,-7",
        abbreviation: "7+",
        abbreviations: &["7+", "+7", "aug7"],
    },
    Music21ChordType {
        kind: "half-diminished-seventh",
        notation: "1,-3,-5,-7",
        abbreviation: "ø7",
        abbreviations: &["ø7", "m7b5"],
    },
    Music21ChordType {
        kind: "diminished-seventh",
        notation: "1,-3,-5,--7",
        abbreviation: "o7",
        abbreviations: &["o7", "dim7"],
    },
    Music21ChordType {
        kind: "seventh-flat-five",
        notation: "1,3,-5,-7",
        abbreviation: "dom7dim5",
        abbreviations: &["dom7dim5"],
    },
    Music21ChordType {
        kind: "major-sixth",
        notation: "1,3,5,6",
        abbreviation: "6",
        abbreviations: &["6"],
    },
    Music21ChordType {
        kind: "minor-sixth",
        notation: "1,-3,5,6",
        abbreviation: "m6",
        abbreviations: &["m6", "min6"],
    },
    Music21ChordType {
        kind: "major-ninth",
        notation: "1,3,5,7,9",
        abbreviation: "M9",
        abbreviations: &["M9", "Maj9"],
    },
    Music21ChordType {
        kind: "dominant-ninth",
        notation: "1,3,5,-7,9",
        abbreviation: "9",
        abbreviations: &["9", "dom9"],
    },
    Music21ChordType {
        kind: "minor-major-ninth",
        notation: "1,-3,5,7,9",
        abbreviation: "mM9",
        abbreviations: &["mM9", "minmaj9"],
    },
    Music21ChordType {
        kind: "minor-ninth",
        notation: "1,-3,5,-7,9",
        abbreviation: "m9",
        abbreviations: &["m9", "min9"],
    },
    Music21ChordType {
        kind: "augmented-major-ninth",
        notation: "1,3,#5,7,9",
        abbreviation: "+M9",
        abbreviations: &["+M9", "augmaj9"],
    },
    Music21ChordType {
        kind: "augmented-dominant-ninth",
        notation: "1,3,#5,-7,9",
        abbreviation: "9#5",
        abbreviations: &["9#5", "+9", "aug9"],
    },
    Music21ChordType {
        kind: "half-diminished-ninth",
        notation: "1,-3,-5,-7,9",
        abbreviation: "ø9",
        abbreviations: &["ø9"],
    },
    Music21ChordType {
        kind: "half-diminished-minor-ninth",
        notation: "1,-3,-5,-7,-9",
        abbreviation: "øb9",
        abbreviations: &["øb9"],
    },
    Music21ChordType {
        kind: "diminished-ninth",
        notation: "1,-3,-5,--7,9",
        abbreviation: "o9",
        abbreviations: &["o9", "dim9"],
    },
    Music21ChordType {
        kind: "diminished-minor-ninth",
        notation: "1,-3,-5,--7,-9",
        abbreviation: "ob9",
        abbreviations: &["ob9", "dimb9"],
    },
    Music21ChordType {
        kind: "dominant-11th",
        notation: "1,3,5,-7,9,11",
        abbreviation: "11",
        abbreviations: &["11", "dom11"],
    },
    Music21ChordType {
        kind: "major-11th",
        notation: "1,3,5,7,9,11",
        abbreviation: "M11",
        abbreviations: &["M11", "Maj11"],
    },
    Music21ChordType {
        kind: "minor-major-11th",
        notation: "1,-3,5,7,9,11",
        abbreviation: "mM11",
        abbreviations: &["mM11", "minmaj11"],
    },
    Music21ChordType {
        kind: "minor-11th",
        notation: "1,-3,5,-7,9,11",
        abbreviation: "m11",
        abbreviations: &["m11", "min11"],
    },
    Music21ChordType {
        kind: "augmented-major-11th",
        notation: "1,3,#5,7,9,11",
        abbreviation: "+M11",
        abbreviations: &["+M11", "augmaj11"],
    },
    Music21ChordType {
        kind: "augmented-11th",
        notation: "1,3,#5,-7,9,11",
        abbreviation: "+11",
        abbreviations: &["+11", "aug11"],
    },
    Music21ChordType {
        kind: "half-diminished-11th",
        notation: "1,-3,-5,-7,9,11",
        abbreviation: "ø11",
        abbreviations: &["ø11"],
    },
    Music21ChordType {
        kind: "diminished-11th",
        notation: "1,-3,-5,--7,9,11",
        abbreviation: "o11",
        abbreviations: &["o11", "dim11"],
    },
    Music21ChordType {
        kind: "major-13th",
        notation: "1,3,5,7,9,11,13",
        abbreviation: "M13",
        abbreviations: &["M13", "Maj13"],
    },
    Music21ChordType {
        kind: "dominant-13th",
        notation: "1,3,5,-7,9,11,13",
        abbreviation: "13",
        abbreviations: &["13", "dom13"],
    },
    Music21ChordType {
        kind: "minor-major-13th",
        notation: "1,-3,5,7,9,11,13",
        abbreviation: "mM13",
        abbreviations: &["mM13", "minmaj13"],
    },
    Music21ChordType {
        kind: "minor-13th",
        notation: "1,-3,5,-7,9,11,13",
        abbreviation: "m13",
        abbreviations: &["m13", "min13"],
    },
    Music21ChordType {
        kind: "augmented-major-13th",
        notation: "1,3,#5,7,9,11,13",
        abbreviation: "+M13",
        abbreviations: &["+M13", "augmaj13"],
    },
    Music21ChordType {
        kind: "augmented-dominant-13th",
        notation: "1,3,#5,-7,9,11,13",
        abbreviation: "+13",
        abbreviations: &["+13", "aug13"],
    },
    Music21ChordType {
        kind: "half-diminished-13th",
        notation: "1,-3,-5,-7,9,11,13",
        abbreviation: "ø13",
        abbreviations: &["ø13"],
    },
    Music21ChordType {
        kind: "suspended-second",
        notation: "1,2,5",
        abbreviation: "sus2",
        abbreviations: &["sus2"],
    },
    Music21ChordType {
        kind: "suspended-fourth",
        notation: "1,4,5",
        abbreviation: "sus",
        abbreviations: &["sus", "sus4"],
    },
    Music21ChordType {
        kind: "suspended-fourth-seventh",
        notation: "1,4,5,-7",
        abbreviation: "7sus",
        abbreviations: &["7sus", "7sus4"],
    },
    Music21ChordType {
        kind: "Neapolitan",
        notation: "1,-2,3,-5",
        abbreviation: "N6",
        abbreviations: &["N6"],
    },
    Music21ChordType {
        kind: "Italian",
        notation: "1,#4,-6",
        abbreviation: "It+6",
        abbreviations: &["It+6", "It"],
    },
    Music21ChordType {
        kind: "French",
        notation: "1,2,#4,-6",
        abbreviation: "Fr+6",
        abbreviations: &["Fr+6", "Fr"],
    },
    Music21ChordType {
        kind: "German",
        notation: "1,-3,#4,-6",
        abbreviation: "Gr+6",
        abbreviations: &["Gr+6", "Ger"],
    },
    Music21ChordType {
        kind: "pedal",
        notation: "1",
        abbreviation: "pedal",
        abbreviations: &["pedal"],
    },
    Music21ChordType {
        kind: "power",
        notation: "1,5",
        abbreviation: "power",
        abbreviations: &["power"],
    },
    Music21ChordType {
        kind: "Tristan",
        notation: "1,#4,#6,#9",
        abbreviation: "tristan",
        abbreviations: &["tristan"],
    },
];

pub(super) fn chord_degrees_for_notation(notation: &str) -> Option<Vec<u8>> {
    notation
        .split(',')
        .filter(|token| *token != "1")
        .map(|token| parse_music21_degree(token).map(|degree| degree.semitone))
        .collect()
}

pub(super) fn degree_numbers_for_notation(notation: &str) -> Option<Vec<u8>> {
    notation
        .split(',')
        .map(|token| parse_music21_degree(token).map(|degree| degree.degree))
        .collect()
}

pub(super) fn parse_music21_degree(token: &str) -> Option<Music21Degree> {
    let alteration = token.chars().fold(0_i32, |sum, ch| match ch {
        '#' => sum + 1,
        '-' => sum - 1,
        _ => sum,
    });
    let degree = token
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>()
        .parse::<u8>()
        .ok()?;
    let semitone = (base_semitone_for_degree(degree)? + alteration).rem_euclid(12) as u8;

    Some(Music21Degree { degree, semitone })
}

pub(super) fn base_semitone_for_degree(degree: u8) -> Option<IntegerType> {
    match degree {
        1 => Some(0),
        2 | 9 => Some(2),
        3 => Some(4),
        4 | 11 => Some(5),
        5 => Some(7),
        6 | 13 => Some(9),
        7 => Some(11),
        _ => None,
    }
}

/// The names of the notes a chord kind's notation stands for above a root:
/// what music21's `ChordSymbol` realizes for the kind alone, and what the
/// figure writer compares a chord against.
pub(super) fn kind_pitch_names(root: &Pitch, notation: &str) -> Result<BTreeSet<String>> {
    notation_intervals(notation)?
        .iter()
        .map(|(_, name)| Ok(Interval::from_name(name)?.transpose_pitch(root)?.name()))
        .collect()
}

/// The interval above the root each degree of a kind's notation stands for,
/// `1,3,#5,-7` being `P1`, `M3`, `a5` and `m7`: the major-scale interval of
/// the degree, raised or lowered by each `#` or `-` written against it.
pub(super) fn notation_intervals(notation: &str) -> Result<Vec<(u8, String)>> {
    notation
        .split(',')
        .map(|token| {
            let degree: u8 = token
                .trim_matches(['#', '-'])
                .parse()
                .map_err(|_| Error::Chord(format!("{token} is not a chord degree")))?;
            let alter = token.matches('#').count() as IntegerType
                - token.matches('-').count() as IntegerType;
            let perfect = matches!(degree % 7, 1 | 4 | 5);
            let quality = match (perfect, alter) {
                (true, 0) => "P".to_string(),
                (false, 0) => "M".to_string(),
                (_, raised) if raised > 0 => "a".repeat(raised as usize),
                (true, lowered) => "d".repeat(lowered.unsigned_abs() as usize),
                (false, -1) => "m".to_string(),
                (false, lowered) => "d".repeat((lowered.unsigned_abs() - 1) as usize),
            };
            Ok((degree, format!("{quality}{degree}")))
        })
        .collect()
}
