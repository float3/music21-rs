//! Clefs: music21's `clef` module.
//!
//! A clef says which line of the staff a note is written on. music21 has a
//! class for each clef it knows, and each differs from the others only in
//! its sign, the line the sign sits on, how many octaves it transposes and
//! the note on its lowest line, so here a [`Clef`] is those values and a
//! [`ClefKind`] names the class they started as.

use crate::defaults::IntegerType;
use crate::error::{Error, Result};
use crate::notation::StemDirection;
use crate::pitch::Pitch;

/// One of music21's clef classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ClefKind {
    /// The base class, which says nothing.
    Clef,
    /// No clef at all.
    NoClef,
    /// A clef for unpitched percussion.
    PercussionClef,
    /// Any clef that places pitches.
    PitchClef,
    /// A C clef on no particular line.
    CClef,
    /// An F clef on no particular line.
    FClef,
    /// A G clef on no particular line.
    GClef,
    /// The clef of jianpu, numbered notation, which is no clef.
    JianpuClef,
    /// A tablature clef.
    TabClef,
    /// C clef on the third line.
    AltoClef,
    /// F clef on the fourth line, an octave up.
    Bass8vaClef,
    /// F clef on the fourth line, an octave down.
    Bass8vbClef,
    /// F clef on the fourth line.
    BassClef,
    /// C clef on the fifth line.
    CBaritoneClef,
    /// F clef on the third line.
    FBaritoneClef,
    /// G clef on the first line.
    FrenchViolinClef,
    /// G clef on the third line.
    GSopranoClef,
    /// C clef on the second line.
    MezzoSopranoClef,
    /// C clef on the first line.
    SopranoClef,
    /// F clef on the fifth line.
    SubBassClef,
    /// C clef on the fourth line.
    TenorClef,
    /// G clef on the second line.
    TrebleClef,
    /// G clef on the second line, an octave up.
    Treble8vaClef,
    /// G clef on the second line, an octave down.
    Treble8vbClef,
}

/// What a clef of one kind starts out as.
struct KindRow {
    kind: ClefKind,
    class: &'static str,
    /// Its ancestors up to `Clef`, nearest first.
    parents: &'static [&'static str],
    sign: Option<&'static str>,
    line: Option<u8>,
    octave_change: IntegerType,
    lowest_line: Option<IntegerType>,
}

// music21's defaults, class by class. Three of its octave clefs set their
// lowest line to a value that ignores their octave change: Treble8vaClef has
// 24 where TrebleClef has 31, and Bass8vaClef and Bass8vbClef keep
// BassClef's 19. They are music21's values and are kept as they are.
const KINDS: [KindRow; 24] = [
    KindRow {
        kind: ClefKind::Clef,
        class: "Clef",
        parents: &[],
        sign: None,
        line: None,
        octave_change: 0,
        lowest_line: None,
    },
    KindRow {
        kind: ClefKind::NoClef,
        class: "NoClef",
        parents: &["Clef"],
        sign: Some("none"),
        line: None,
        octave_change: 0,
        lowest_line: None,
    },
    KindRow {
        kind: ClefKind::PercussionClef,
        class: "PercussionClef",
        parents: &["Clef"],
        sign: Some("percussion"),
        line: None,
        octave_change: 0,
        lowest_line: Some(31),
    },
    KindRow {
        kind: ClefKind::PitchClef,
        class: "PitchClef",
        parents: &["Clef"],
        sign: None,
        line: None,
        octave_change: 0,
        lowest_line: Some(31),
    },
    KindRow {
        kind: ClefKind::CClef,
        class: "CClef",
        parents: &["PitchClef", "Clef"],
        sign: Some("C"),
        line: None,
        octave_change: 0,
        lowest_line: Some(31),
    },
    KindRow {
        kind: ClefKind::FClef,
        class: "FClef",
        parents: &["PitchClef", "Clef"],
        sign: Some("F"),
        line: None,
        octave_change: 0,
        lowest_line: Some(31),
    },
    KindRow {
        kind: ClefKind::GClef,
        class: "GClef",
        parents: &["PitchClef", "Clef"],
        sign: Some("G"),
        line: None,
        octave_change: 0,
        lowest_line: Some(31),
    },
    KindRow {
        kind: ClefKind::JianpuClef,
        class: "JianpuClef",
        parents: &["NoClef", "Clef"],
        sign: Some("jianpu"),
        line: None,
        octave_change: 0,
        lowest_line: None,
    },
    KindRow {
        kind: ClefKind::TabClef,
        class: "TabClef",
        parents: &["PitchClef", "Clef"],
        sign: Some("TAB"),
        line: Some(5),
        octave_change: 0,
        lowest_line: Some(31),
    },
    KindRow {
        kind: ClefKind::AltoClef,
        class: "AltoClef",
        parents: &["CClef", "PitchClef", "Clef"],
        sign: Some("C"),
        line: Some(3),
        octave_change: 0,
        lowest_line: Some(25),
    },
    KindRow {
        kind: ClefKind::Bass8vaClef,
        class: "Bass8vaClef",
        parents: &["FClef", "PitchClef", "Clef"],
        sign: Some("F"),
        line: Some(4),
        octave_change: 1,
        lowest_line: Some(19),
    },
    KindRow {
        kind: ClefKind::Bass8vbClef,
        class: "Bass8vbClef",
        parents: &["FClef", "PitchClef", "Clef"],
        sign: Some("F"),
        line: Some(4),
        octave_change: -1,
        lowest_line: Some(19),
    },
    KindRow {
        kind: ClefKind::BassClef,
        class: "BassClef",
        parents: &["FClef", "PitchClef", "Clef"],
        sign: Some("F"),
        line: Some(4),
        octave_change: 0,
        lowest_line: Some(19),
    },
    KindRow {
        kind: ClefKind::CBaritoneClef,
        class: "CBaritoneClef",
        parents: &["CClef", "PitchClef", "Clef"],
        sign: Some("C"),
        line: Some(5),
        octave_change: 0,
        lowest_line: Some(21),
    },
    KindRow {
        kind: ClefKind::FBaritoneClef,
        class: "FBaritoneClef",
        parents: &["FClef", "PitchClef", "Clef"],
        sign: Some("F"),
        line: Some(3),
        octave_change: 0,
        lowest_line: Some(21),
    },
    KindRow {
        kind: ClefKind::FrenchViolinClef,
        class: "FrenchViolinClef",
        parents: &["GClef", "PitchClef", "Clef"],
        sign: Some("G"),
        line: Some(1),
        octave_change: 0,
        lowest_line: Some(33),
    },
    KindRow {
        kind: ClefKind::GSopranoClef,
        class: "GSopranoClef",
        parents: &["GClef", "PitchClef", "Clef"],
        sign: Some("G"),
        line: Some(3),
        octave_change: 0,
        lowest_line: Some(29),
    },
    KindRow {
        kind: ClefKind::MezzoSopranoClef,
        class: "MezzoSopranoClef",
        parents: &["CClef", "PitchClef", "Clef"],
        sign: Some("C"),
        line: Some(2),
        octave_change: 0,
        lowest_line: Some(27),
    },
    KindRow {
        kind: ClefKind::SopranoClef,
        class: "SopranoClef",
        parents: &["CClef", "PitchClef", "Clef"],
        sign: Some("C"),
        line: Some(1),
        octave_change: 0,
        lowest_line: Some(29),
    },
    KindRow {
        kind: ClefKind::SubBassClef,
        class: "SubBassClef",
        parents: &["FClef", "PitchClef", "Clef"],
        sign: Some("F"),
        line: Some(5),
        octave_change: 0,
        lowest_line: Some(17),
    },
    KindRow {
        kind: ClefKind::TenorClef,
        class: "TenorClef",
        parents: &["CClef", "PitchClef", "Clef"],
        sign: Some("C"),
        line: Some(4),
        octave_change: 0,
        lowest_line: Some(23),
    },
    KindRow {
        kind: ClefKind::TrebleClef,
        class: "TrebleClef",
        parents: &["GClef", "PitchClef", "Clef"],
        sign: Some("G"),
        line: Some(2),
        octave_change: 0,
        lowest_line: Some(31),
    },
    KindRow {
        kind: ClefKind::Treble8vaClef,
        class: "Treble8vaClef",
        parents: &["TrebleClef", "GClef", "PitchClef", "Clef"],
        sign: Some("G"),
        line: Some(2),
        octave_change: 1,
        lowest_line: Some(24),
    },
    KindRow {
        kind: ClefKind::Treble8vbClef,
        class: "Treble8vbClef",
        parents: &["TrebleClef", "GClef", "PitchClef", "Clef"],
        sign: Some("G"),
        line: Some(2),
        octave_change: -1,
        lowest_line: Some(24),
    },
];

/// The diatonic note number of the treble staff's middle line, B4: where a
/// clef that places no pitches reads a stem from.
const TREBLE_MIDDLE_LINE: IntegerType = 35;

impl ClefKind {
    /// Every kind, in the order music21's table lists them.
    pub const ALL: [ClefKind; 24] = [
        ClefKind::Clef,
        ClefKind::NoClef,
        ClefKind::PercussionClef,
        ClefKind::PitchClef,
        ClefKind::CClef,
        ClefKind::FClef,
        ClefKind::GClef,
        ClefKind::JianpuClef,
        ClefKind::TabClef,
        ClefKind::AltoClef,
        ClefKind::Bass8vaClef,
        ClefKind::Bass8vbClef,
        ClefKind::BassClef,
        ClefKind::CBaritoneClef,
        ClefKind::FBaritoneClef,
        ClefKind::FrenchViolinClef,
        ClefKind::GSopranoClef,
        ClefKind::MezzoSopranoClef,
        ClefKind::SopranoClef,
        ClefKind::SubBassClef,
        ClefKind::TenorClef,
        ClefKind::TrebleClef,
        ClefKind::Treble8vaClef,
        ClefKind::Treble8vbClef,
    ];

    fn row(self) -> &'static KindRow {
        KINDS
            .iter()
            .find(|row| row.kind == self)
            .expect("every kind has a row")
    }

    /// music21's class name for the kind: `"TrebleClef"`.
    pub fn class_name(self) -> &'static str {
        self.row().class
    }

    /// The kind music21 names by that class.
    pub fn from_class_name(class: &str) -> Option<Self> {
        KINDS
            .iter()
            .find(|row| row.class == class)
            .map(|row| row.kind)
    }

    /// The classes above this one, nearest first, up to `Clef`.
    pub fn parents(self) -> &'static [&'static str] {
        self.row().parents
    }
}

/// A clef: its sign, the line it sits on, the octaves it transposes and the
/// note written on its lowest line.
///
/// ```
/// use music21_rs::clef::{Clef, ClefKind};
///
/// let alto = Clef::from_string("C3", 0)?;
/// assert_eq!(alto.kind(), ClefKind::AltoClef);
/// assert_eq!(alto.lowest_line(), Some(25));
/// assert_eq!(alto.name(), "alto");
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Clef {
    kind: ClefKind,
    sign: Option<String>,
    line: Option<u8>,
    octave_change: IntegerType,
    lowest_line: Option<IntegerType>,
}

impl Default for Clef {
    /// music21's bare `Clef`, which says nothing.
    fn default() -> Self {
        Self::of_kind(ClefKind::Clef)
    }
}

impl Clef {
    /// A clef as music21's class for `kind` starts out.
    pub fn of_kind(kind: ClefKind) -> Self {
        let row = kind.row();
        Self {
            kind,
            sign: row.sign.map(str::to_string),
            line: row.line,
            octave_change: row.octave_change,
            lowest_line: row.lowest_line,
        }
    }

    /// The treble clef.
    pub fn treble() -> Self {
        Self::of_kind(ClefKind::TrebleClef)
    }

    /// The bass clef.
    pub fn bass() -> Self {
        Self::of_kind(ClefKind::BassClef)
    }

    /// The kind of clef this started as.
    pub fn kind(&self) -> ClefKind {
        self.kind
    }

    /// Whether this is a clef of that music21 class or one below it:
    /// `is_a("GClef")` holds for a treble clef.
    pub fn is_a(&self, class: &str) -> bool {
        self.kind.class_name() == class || self.kind.parents().contains(&class)
    }

    /// Whether the clef places pitches, as every one below music21's
    /// `PitchClef` does.
    pub fn places_pitches(&self) -> bool {
        self.is_a("PitchClef")
    }

    /// music21's `name`: the class name without `Clef`, first letter lower
    /// case, so a treble clef an octave down is `"treble8vb"`.
    pub fn name(&self) -> String {
        let class = self.kind.class_name().replace("Clef", "");
        let mut letters = class.chars();
        match letters.next() {
            Some(first) => first.to_lowercase().chain(letters).collect(),
            None => String::new(),
        }
    }

    /// The sign: `G`, `F`, `C`, `TAB`, `percussion`, `none` or `jianpu`.
    pub fn sign(&self) -> Option<&str> {
        self.sign.as_deref()
    }

    /// Changes the sign.
    pub fn set_sign(&mut self, sign: Option<String>) {
        self.sign = sign;
    }

    /// The line the sign sits on, counted from the bottom.
    pub fn line(&self) -> Option<u8> {
        self.line
    }

    /// Changes the line.
    pub fn set_line(&mut self, line: Option<u8>) {
        self.line = line;
    }

    /// How many octaves the clef transposes what is written: music21's
    /// `octaveChange`.
    pub fn octave_change(&self) -> IntegerType {
        self.octave_change
    }

    /// Changes how many octaves the clef transposes. On a clef that places
    /// pitches this moves its lowest line by the octaves it changed by, as
    /// music21's does; setting the lowest line moves nothing.
    ///
    /// ```
    /// use music21_rs::clef::Clef;
    ///
    /// let mut treble = Clef::treble();
    /// treble.set_octave_change(1);
    /// assert_eq!(treble.lowest_line(), Some(38));
    /// ```
    pub fn set_octave_change(&mut self, octave_change: IntegerType) {
        if self.places_pitches()
            && let Some(lowest) = &mut self.lowest_line
        {
            *lowest += (octave_change - self.octave_change) * 7;
        }
        self.octave_change = octave_change;
    }

    /// The diatonic note number of the note on the lowest line: music21's
    /// `lowestLine`.
    pub fn lowest_line(&self) -> Option<IntegerType> {
        self.lowest_line
    }

    /// Changes the note on the lowest line.
    pub fn set_lowest_line(&mut self, lowest_line: Option<IntegerType>) {
        self.lowest_line = lowest_line;
    }

    /// Which way a stem goes for these pitches under this clef: music21's
    /// `getStemDirectionForPitches`. Each pitch counts by how far it sits
    /// from the middle line; with `first_last_only`, as in a beamed group,
    /// only the first and last count, and with `extreme_pitch_only` only the
    /// lowest and the highest. Guitar tablature's stems always go down.
    ///
    /// ```
    /// use music21_rs::{clef::Clef, Pitch, StemDirection};
    ///
    /// let pitches: Vec<Pitch> = ["C3", "B3", "C3"]
    ///     .iter()
    ///     .map(|name| Pitch::from_name(name))
    ///     .collect::<Result<_, _>>()?;
    /// let bass = Clef::bass();
    /// assert_eq!(bass.stem_direction_for_pitches(&pitches, true, false)?, StemDirection::Up);
    /// assert_eq!(bass.stem_direction_for_pitches(&pitches, false, false)?, StemDirection::Down);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// No pitches at all.
    pub fn stem_direction_for_pitches(
        &self,
        pitches: &[Pitch],
        first_last_only: bool,
        extreme_pitch_only: bool,
    ) -> Result<StemDirection> {
        if self.is_a("TabClef") {
            return Ok(StemDirection::Down);
        }
        if pitches.is_empty() {
            return Err(Error::Value(
                "getStemDirectionForPitches cannot operate on an empty list".to_string(),
            ));
        }
        let numbers: Vec<IntegerType> = pitches.iter().map(Pitch::diatonic_note_number).collect();
        let relevant: Vec<IntegerType> = if extreme_pitch_only {
            let lowest = numbers.iter().copied().min().unwrap_or_default();
            let highest = numbers.iter().copied().max().unwrap_or_default();
            vec![lowest, highest]
        } else if first_last_only && numbers.len() > 1 {
            vec![numbers[0], numbers[numbers.len() - 1]]
        } else {
            numbers
        };
        let middle = match self.lowest_line {
            Some(lowest) if self.places_pitches() || self.is_a("PercussionClef") => lowest + 4,
            _ => TREBLE_MIDDLE_LINE,
        };
        let distance: IntegerType = relevant.iter().map(|number| number - middle).sum();
        Ok(if distance >= 0 {
            StemDirection::Down
        } else {
            StemDirection::Up
        })
    }

    /// The clef a string like `"G2"` or `"F4"` names: music21's
    /// `clefFromString`. `tab`, `percussion`, `none` and `jianpu` name those
    /// clefs, a sign alone takes its usual line, a longer string may be any
    /// clef's class name with or without `Clef`, and `octave_shift`
    /// transposes the result, giving the octave treble and bass clefs their
    /// own kinds. Case does not matter.
    ///
    /// # Errors
    ///
    /// A string that names no clef, or a line outside one to five.
    pub fn from_string(text: &str, octave_shift: IntegerType) -> Result<Self> {
        let written = text.trim();
        let lower = written.to_lowercase();
        match lower.as_str() {
            "tab" => return Ok(Self::of_kind(ClefKind::TabClef)),
            "percussion" => return Ok(Self::of_kind(ClefKind::PercussionClef)),
            "none" => return Ok(Self::of_kind(ClefKind::NoClef)),
            "jianpu" => return Ok(Self::of_kind(ClefKind::JianpuClef)),
            _ => {}
        }
        let mut characters = written.chars();
        let (sign, line) = match written.chars().count() {
            0 => {
                return Err(Error::Clef(
                    "Entry has clef info but no clef specified".to_string(),
                ));
            }
            1 => {
                let sign = characters.next().map(|sign| sign.to_ascii_uppercase());
                let line = match sign {
                    Some('G') => Some(2),
                    Some('F') => Some(4),
                    Some('C') => Some(3),
                    _ => None,
                };
                (sign, line)
            }
            2 => {
                let sign = characters.next().map(|sign| sign.to_ascii_uppercase());
                let line = characters
                    .next()
                    .and_then(|line| line.to_digit(10))
                    .and_then(|line| u8::try_from(line).ok());
                if line.is_none() {
                    return Err(Error::Clef(format!(
                        "cannot read {written} as clef str, should be G2, F4, etc."
                    )));
                }
                (sign, line)
            }
            _ => {
                return KINDS
                    .iter()
                    .find(|row| {
                        let class = row.class.to_lowercase();
                        class == lower || class == format!("{lower}clef")
                    })
                    .map(|row| Self::of_kind(row.kind))
                    .ok_or_else(|| Error::Clef(format!("Could not find clef {written}")));
            }
        };
        if octave_shift != 0 {
            let shifted = match (sign, line, octave_shift) {
                (Some('G'), Some(2), -1) => Some(ClefKind::Treble8vbClef),
                (Some('G'), Some(2), 1) => Some(ClefKind::Treble8vaClef),
                (Some('F'), Some(4), -1) => Some(ClefKind::Bass8vbClef),
                (Some('F'), Some(4), 1) => Some(ClefKind::Bass8vaClef),
                _ => None,
            };
            if let Some(kind) = shifted {
                return Ok(Self::of_kind(kind));
            }
        }
        let (Some(sign), Some(line)) = (sign, line) else {
            return Err(Error::Clef(format!(
                "cannot read {written} as clef str, should be G2, F4, etc."
            )));
        };
        if !(1..=5).contains(&line) {
            return Err(Error::Clef(format!(
                "line number (second character) must be 1-5; do not use this function for clefs on special staves such as '{written}'"
            )));
        }
        let named = match (sign, line) {
            ('G', 1) => Some(ClefKind::FrenchViolinClef),
            ('G', 2) => Some(ClefKind::TrebleClef),
            ('G', 3) => Some(ClefKind::GSopranoClef),
            ('C', 1) => Some(ClefKind::SopranoClef),
            ('C', 2) => Some(ClefKind::MezzoSopranoClef),
            ('C', 3) => Some(ClefKind::AltoClef),
            ('C', 4) => Some(ClefKind::TenorClef),
            ('C', 5) => Some(ClefKind::CBaritoneClef),
            ('F', 3) => Some(ClefKind::FBaritoneClef),
            ('F', 4) => Some(ClefKind::BassClef),
            ('F', 5) => Some(ClefKind::SubBassClef),
            _ => None,
        };
        let mut clef = match named {
            Some(kind) => Self::of_kind(kind),
            None => {
                let family = match sign {
                    'G' => ClefKind::GClef,
                    'F' => ClefKind::FClef,
                    'C' => ClefKind::CClef,
                    _ => ClefKind::PitchClef,
                };
                let mut clef = Self::of_kind(family);
                if family == ClefKind::PitchClef {
                    clef.sign = Some(sign.to_string());
                }
                clef.line = Some(line);
                clef
            }
        };
        if octave_shift != 0 {
            clef.set_octave_change(octave_shift);
        }
        Ok(clef)
    }

    /// The clef that best fits these pitches: music21's `bestClef` over the
    /// pitches of the notes and chords it finds. Pitches above A4 and below
    /// F3 count a little further out than they are, and with
    /// `allow_treble_8vb` notes around middle C may take the treble clef an
    /// octave down.
    ///
    /// ```
    /// use music21_rs::{clef::{Clef, ClefKind}, Pitch};
    ///
    /// let d4 = [Pitch::from_name("D4")?];
    /// assert_eq!(Clef::best_for(&d4, false).kind(), ClefKind::TrebleClef);
    /// assert_eq!(Clef::best_for(&d4, true).kind(), ClefKind::Treble8vbClef);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    pub fn best_for(pitches: &[Pitch], allow_treble_8vb: bool) -> Self {
        let average = if pitches.is_empty() {
            29.0
        } else {
            let total: IntegerType = pitches
                .iter()
                .map(|pitch| {
                    let height = pitch.diatonic_note_number();
                    if height > 33 {
                        height + 3
                    } else if height < 24 {
                        height - 3
                    } else {
                        height
                    }
                })
                .sum();
            f64::from(total) / pitches.len() as f64
        };
        let kind = if average > 49.0 {
            ClefKind::Treble8vaClef
        } else if (allow_treble_8vb && average > 32.0) || (!allow_treble_8vb && average > 28.0) {
            ClefKind::TrebleClef
        } else if allow_treble_8vb && average > 26.0 {
            ClefKind::Treble8vbClef
        } else if average > 10.0 {
            ClefKind::BassClef
        } else {
            ClefKind::Bass8vbClef
        };
        Self::of_kind(kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pitches(names: &[&str]) -> Vec<Pitch> {
        names
            .iter()
            .map(|name| Pitch::from_name(name).unwrap())
            .collect()
    }

    /// Every answer here is music21's own, from its clef docstrings.
    #[test]
    fn a_clef_answers_what_music21_s_answers() {
        assert_eq!(Clef::treble().lowest_line(), Some(31));
        assert_eq!(Clef::of_kind(ClefKind::Treble8vbClef).octave_change(), -1);
        assert_eq!(Clef::of_kind(ClefKind::Treble8vbClef).name(), "treble8vb");
        assert_eq!(
            Clef::of_kind(ClefKind::MezzoSopranoClef).name(),
            "mezzoSoprano"
        );
        assert_eq!(Clef::default().name(), "");

        let mut treble = Clef::treble();
        treble.set_octave_change(1);
        assert_eq!(treble.lowest_line(), Some(38));
        treble.set_octave_change(-1);
        assert_eq!(treble.lowest_line(), Some(24));

        let bass = Clef::bass();
        assert_eq!(
            bass.stem_direction_for_pitches(&pitches(&["C3", "B3", "C3"]), true, false)
                .unwrap(),
            StemDirection::Up
        );
        assert_eq!(
            bass.stem_direction_for_pitches(&pitches(&["C3", "B3", "C3"]), false, false)
                .unwrap(),
            StemDirection::Down
        );
        assert!(bass.stem_direction_for_pitches(&[], true, false).is_err());
    }

    #[test]
    fn a_clef_is_read_from_a_string_as_music21_reads_one() {
        assert_eq!(
            Clef::from_string("G2", 0).unwrap().kind(),
            ClefKind::TrebleClef
        );
        assert_eq!(
            Clef::from_string("f4", 0).unwrap().kind(),
            ClefKind::BassClef
        );
        assert_eq!(
            Clef::from_string("C", 0).unwrap().kind(),
            ClefKind::AltoClef
        );
        assert_eq!(
            Clef::from_string("tab", 0).unwrap().kind(),
            ClefKind::TabClef
        );
        assert_eq!(
            Clef::from_string("G2", -1).unwrap().kind(),
            ClefKind::Treble8vbClef
        );
        assert_eq!(
            Clef::from_string("mezzoSoprano", 0).unwrap().kind(),
            ClefKind::MezzoSopranoClef
        );
        assert_eq!(
            Clef::from_string("tenorclef", 0).unwrap().kind(),
            ClefKind::TenorClef
        );
        let g5 = Clef::from_string("G5", 0).unwrap();
        assert_eq!((g5.kind(), g5.line()), (ClefKind::GClef, Some(5)));
        assert!(Clef::from_string("F6", 0).is_err());
        assert!(Clef::from_string("", 0).is_err());
        assert!(Clef::from_string("nonsense", 0).is_err());
    }

    #[test]
    fn the_best_clef_is_music21_s() {
        assert_eq!(
            Clef::best_for(&pitches(&["D7"]), false).kind(),
            ClefKind::Treble8vaClef
        );
        assert_eq!(
            Clef::best_for(&pitches(&["C0"]), false).kind(),
            ClefKind::Bass8vbClef
        );
        assert_eq!(Clef::best_for(&[], false).kind(), ClefKind::TrebleClef);
        assert_eq!(
            Clef::best_for(&pitches(&["A2", "C3"]), false).kind(),
            ClefKind::BassClef
        );
    }
}
