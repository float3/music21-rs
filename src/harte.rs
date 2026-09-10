//! Harte chord notation, as read by
//! [harte-library](https://github.com/andreamust/harte-library): `C:maj7/3`,
//! `Bb:(b3,5,b7,9)`, `F:sus4(*3,9)`, `N`.
//!
//! A label is a root, an optional shorthand, an optional list of degrees in
//! parentheses and an optional bass degree after a slash. `*3` in the
//! degrees removes the third. A degree is written the Harte way, `b3`, `#11`,
//! `bb7`; [`convert_interval`] gives the music21 interval it stands for, and
//! [`Harte`] builds the chord the label sounds.

use std::fmt;
use std::str::FromStr;

use crate::chord::Chord;
use crate::defaults::{FloatType, IntegerType};
use crate::error::{Error, Result};
use crate::interval::Interval;
use crate::pitch::Pitch;

/// A shorthand beside the degrees it stands for.
pub type Shorthand = (&'static str, &'static [&'static str]);

/// The degrees each Harte shorthand stands for, the root included.
pub const SHORTHAND_DEGREES: [Shorthand; 48] = [
    ("maj", &["1", "3", "5"]),
    ("min", &["1", "b3", "5"]),
    ("aug", &["1", "3", "#5"]),
    ("dim", &["1", "b3", "b5"]),
    ("7", &["1", "3", "5", "b7"]),
    ("maj7", &["1", "3", "5", "7"]),
    ("minmaj7", &["1", "b3", "5", "7"]),
    ("min7", &["1", "b3", "5", "b7"]),
    ("augmaj7", &["1", "3", "#5", "7"]),
    ("aug7", &["1", "3", "#5", "b7"]),
    ("hdim7", &["1", "b3", "b5", "b7"]),
    ("hdim", &["1", "b3", "b5", "b7"]),
    ("dim7", &["1", "b3", "b5", "bb7"]),
    ("dom7dim5", &["1", "3", "b5", "b7"]),
    ("maj6", &["1", "3", "5", "6"]),
    ("min6", &["1", "b3", "5", "6"]),
    ("maj9", &["1", "3", "5", "7", "9"]),
    ("9", &["1", "3", "5", "b7", "9"]),
    ("minmaj9", &["1", "b3", "5", "7", "9"]),
    ("min9", &["1", "b3", "5", "b7", "9"]),
    ("augmaj9", &["1", "3", "#5", "7", "9"]),
    ("aug9", &["1", "3", "#5", "b7", "9"]),
    ("hdim9", &["1", "b3", "b5", "b7", "9"]),
    ("hdimmin9", &["1", "b3", "b5", "b7", "b9"]),
    ("dim9", &["1", "b3", "b5", "bb7", "9"]),
    ("dimmin9", &["1", "b3", "b5", "bb7", "b9"]),
    ("11", &["1", "3", "5", "b7", "9", "11"]),
    ("maj11", &["1", "3", "5", "7", "9", "11"]),
    ("minmaj11", &["1", "b3", "5", "7", "9", "11"]),
    ("min11", &["1", "b3", "5", "b7", "9", "11"]),
    ("augmaj11", &["1", "3", "#5", "7", "9", "11"]),
    ("aug11", &["1", "3", "#5", "b7", "9", "11"]),
    ("hdim11", &["1", "b3", "b5", "b7", "b9", "11"]),
    ("dim11", &["1", "b3", "b5", "bb7", "b9", "b11"]),
    ("maj13", &["1", "3", "5", "7", "9", "11", "13"]),
    ("13", &["1", "3", "5", "b7", "9", "11", "13"]),
    ("minmaj13", &["1", "b3", "5", "7", "9", "11", "13"]),
    ("min13", &["1", "b3", "5", "b7", "9", "11", "13"]),
    ("augmaj13", &["1", "3", "#5", "7", "9", "11", "13"]),
    ("hdim13", &["1", "b3", "b5", "b7", "9", "11", "13"]),
    ("sus2", &["1", "2", "5"]),
    ("sus4", &["1", "4", "5"]),
    ("7sus4", &["1", "4", "5", "b7"]),
    ("power", &["1", "5"]),
    ("pedal", &["1"]),
    ("1", &["1"]),
    ("5", &["1", "5"]),
    ("6", &["1", "3", "5", "6"]),
];

/// The shorthands [`Harte::prettify`] folds a set of degrees into, tried in
/// this order, so a ninth is written as one before its seventh is.
const DEGREE_SHORTHANDS: &[(&[&str], &str)] = &[
    (&["3", "5", "b7", "9"], "9"),
    (&["3", "5", "7", "9"], "maj9"),
    (&["b3", "5", "b7", "9"], "min9"),
    (&["3", "5", "b7"], "7"),
    (&["3", "5", "6"], "maj6"),
    (&["b3", "5", "6"], "min6"),
    (&["3", "5", "7"], "maj7"),
    (&["b3", "b5", "bb7"], "dim7"),
    (&["b3", "5", "b7"], "min7"),
    (&["b3", "b5", "b7"], "hdim7"),
    (&["b3", "5", "7"], "minmaj7"),
    (&["3", "5"], "maj"),
    (&["b3", "5"], "min"),
    (&["b3", "b5"], "dim"),
    (&["3", "#5"], "aug"),
    (&["4", "5"], "sus4"),
];

/// The degrees a shorthand stands for, or `None` for a name that is not one.
pub fn shorthand_degrees(shorthand: &str) -> Option<&'static [&'static str]> {
    SHORTHAND_DEGREES
        .iter()
        .find(|(name, _)| *name == shorthand)
        .map(|(_, degrees)| *degrees)
}

/// The music21 interval a Harte degree stands for: `b3` is `m3`, `#11` is
/// `A4`, `bb7` is `d7`. Compound degrees fold to simple ones, and a double
/// sharp or flat on a perfect degree is read as the neighbouring degree with
/// one accidental fewer, as harte-library reads them.
pub fn convert_interval(harte: &str) -> Result<String> {
    let cannot = || Error::Harte(format!("The degree {harte} cannot be parsed."));
    let (sharps, flats, number) = accidentals_and_number(harte).ok_or_else(cannot)?;
    let base = if number < 8 { number } else { number - 7 };
    let specifier = if matches!(base, 1 | 4 | 5) {
        match (sharps, flats) {
            (0, 0) => "P",
            (1, 0) => "A",
            (0, 1) => "d",
            (2, 0) => return convert_interval(&format!("{}", base + 1)),
            (0, 2) if base == 1 => return convert_interval("b7"),
            (0, 2) => return convert_interval(&format!("{}", base - 1)),
            _ => return Err(cannot()),
        }
    } else {
        match (sharps, flats) {
            (0, 0) => "M",
            (1, 0) => "A",
            (0, 1) => "m",
            (0, 2) => "d",
            _ => return Err(cannot()),
        }
    };
    Ok(format!("{specifier}{base}"))
}

/// The first run of sharps, then flats, then digits in a degree, read the
/// way the library's `([#]+)?([b]+)?(\d+)` search reads it: a run that ends
/// without a digit is skipped over, so `b#3` is one sharp above a third.
fn accidentals_and_number(degree: &str) -> Option<(usize, usize, IntegerType)> {
    let text: Vec<char> = degree.chars().collect();
    (0..text.len()).find_map(|start| {
        let sharps = text[start..].iter().take_while(|c| **c == '#').count();
        let flats = text[start + sharps..]
            .iter()
            .take_while(|c| **c == 'b')
            .count();
        let digits: String = text[start + sharps + flats..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        digits.parse().ok().map(|number| (sharps, flats, number))
    })
}

/// Where a degree sorts among the others: by its number, a flat just below
/// it and a sharp just above, so `b3` comes after `2` and before `3`.
pub fn degree_sort_key(degree: &str) -> FloatType {
    let number: FloatType = degree
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap_or(0.0);
    if degree.starts_with('b') {
        number - 0.49
    } else if degree.starts_with('#') {
        number + 0.49
    } else {
        number
    }
}

/// A Harte degree beside the music21 interval it stands for.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct HarteInterval {
    written: String,
    interval: Interval,
}

impl HarteInterval {
    /// Reads a Harte degree such as `b3`.
    pub fn new(written: &str) -> Result<Self> {
        let interval = Interval::from_name(&convert_interval(written)?)
            .map_err(|_| Error::Harte(format!("Harte Interval {written} cannot be converted")))?;
        Ok(Self {
            written: written.to_string(),
            interval,
        })
    }

    /// The degree as written.
    pub fn written(&self) -> &str {
        &self.written
    }

    /// The interval it stands for.
    pub fn interval(&self) -> &Interval {
        &self.interval
    }

    /// The pitch this degree names above a root.
    pub fn transpose_pitch(&self, root: &Pitch) -> Result<Pitch> {
        self.interval.transpose_pitch(root)
    }
}

impl fmt::Display for HarteInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.written)
    }
}

/// The parts of a label as written.
#[derive(Clone, Debug, Default, PartialEq)]
struct Parsed {
    root: Option<String>,
    shorthand: Option<String>,
    degrees: Vec<String>,
    bass: Option<String>,
}

/// Reads a label the way harte-library's grammar does: spaces are ignored
/// anywhere, `N` and `X` are no chord, and a degree is a number from 1 to
/// 13 with any accidentals in front of it and an optional `*` before those.
fn parse(label: &str) -> Result<Parsed> {
    let text: Vec<char> = label.chars().filter(|c| *c != ' ').collect();
    let error = |at: usize| {
        Error::Harte(format!(
            "The input chord {label} is not a valid Harte chord (at character {at})"
        ))
    };
    if text.is_empty() {
        return Err(error(0));
    }
    if text.len() == 1 && matches!(text[0], 'N' | 'X') {
        return Ok(Parsed::default());
    }
    let mut at = 0;
    if !matches!(text[0], 'A'..='G') {
        return Err(error(0));
    }
    at += 1;
    while at < text.len() && matches!(text[at], 'b' | '#') {
        at += 1;
    }
    let mut parsed = Parsed {
        root: Some(text[..at].iter().collect()),
        ..Parsed::default()
    };

    let degree = |at: &mut usize| -> Result<String> {
        let start = *at;
        if *at < text.len() && text[*at] == '*' {
            *at += 1;
        }
        while *at < text.len() && matches!(text[*at], 'b' | '#') {
            *at += 1;
        }
        let digits = *at;
        while *at < text.len() && text[*at].is_ascii_digit() {
            *at += 1;
        }
        let number: String = text[digits..*at].iter().collect();
        if !(1..=13).contains(&number.parse::<u8>().unwrap_or(0)) {
            return Err(error(digits));
        }
        Ok(text[start..*at].iter().collect())
    };
    let degree_list = |at: &mut usize| -> Result<Vec<String>> {
        let mut degrees = vec![degree(at)?];
        while *at < text.len() && text[*at] == ',' {
            *at += 1;
            degrees.push(degree(at)?);
        }
        if *at >= text.len() || text[*at] != ')' {
            return Err(error(*at));
        }
        *at += 1;
        Ok(degrees)
    };

    if at < text.len() && text[at] == ':' {
        at += 1;
        if at < text.len() && text[at] == '(' {
            at += 1;
            parsed.degrees = degree_list(&mut at)?;
        } else {
            let start = at;
            while at < text.len() && text[at].is_ascii_alphanumeric() {
                at += 1;
            }
            let shorthand: String = text[start..at].iter().collect();
            if shorthand_degrees(&shorthand).is_none() {
                return Err(error(start));
            }
            parsed.shorthand = Some(shorthand);
            if at < text.len() && text[at] == '(' {
                at += 1;
                parsed.degrees = degree_list(&mut at)?;
            }
        }
    }
    if at < text.len() && text[at] == '/' {
        at += 1;
        parsed.bass = Some(degree(&mut at)?);
    }
    if at != text.len() {
        return Err(error(at));
    }
    Ok(parsed)
}

/// A chord read from a Harte label, with the [`Chord`] it sounds.
///
/// The chord's pitches stand in degree order above the root in octave 4,
/// the bass an octave lower where it is not the root, and the root and bass
/// are fixed on the chord as the label names them. A label of `N` or `X` is
/// no chord at all: it has no root and an empty chord.
///
/// Two labels are equal when they sound the same chord: the same root, the
/// same degrees once the shorthand is unwrapped, and the same bass. `C:maj`
/// equals `C:(3,5)`.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Harte {
    label: String,
    root: Option<String>,
    shorthand: Option<String>,
    degrees: Vec<String>,
    bass: Option<String>,
    all_degrees: Vec<String>,
    chord: Chord,
}

impl Harte {
    /// Reads a label and builds the chord it names.
    pub fn new(label: &str) -> Result<Self> {
        let parsed = parse(label)?;
        let Some(root) = parsed.root else {
            return Ok(Self {
                label: label.to_string(),
                root: None,
                shorthand: None,
                degrees: Vec::new(),
                bass: None,
                all_degrees: Vec::new(),
                chord: Chord::empty(),
            });
        };
        let bass = parsed.bass.unwrap_or_else(|| "1".to_string());
        let removed: Vec<&str> = parsed
            .degrees
            .iter()
            .filter_map(|degree| degree.strip_prefix('*'))
            .collect();
        let mut all: Vec<String> = vec!["1".to_string()];
        if let Some(shorthand) = &parsed.shorthand {
            let named = shorthand_degrees(shorthand)
                .ok_or_else(|| Error::Harte("The Harte shorthand is not valid.".to_string()))?;
            all.extend(named.iter().map(|degree| (*degree).to_string()));
            all.extend(parsed.degrees.iter().cloned());
        } else if parsed.degrees.len() > removed.len() {
            all.extend(parsed.degrees.iter().cloned());
        } else {
            all.extend(["3".to_string(), "5".to_string()]);
        }
        all.retain(|degree| !removed.contains(&degree.as_str()) && !degree.starts_with('*'));
        all.push(bass.clone());
        all.sort_by(|a, b| {
            degree_sort_key(a)
                .partial_cmp(&degree_sort_key(b))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.cmp(b))
        });
        all.dedup();

        // music21 writes a flat as `-`, and several flats as several.
        let root_pitch = Pitch::from_name(format!("{}4", root.replace('b', "-")))?;
        let mut pitches = Vec::with_capacity(all.len());
        for degree in &all {
            pitches.push(HarteInterval::new(degree)?.transpose_pitch(&root_pitch)?);
        }
        let bass_pitch = HarteInterval::new(&bass)?.transpose_pitch(&root_pitch)?;
        let mut chord = Chord::new(pitches.as_slice())?;
        // The bass sounds an octave below the rest, unless it is the root.
        let bass_pitch = if bass_pitch.name() == root_pitch.name() {
            root_pitch.clone()
        } else {
            let mut lowered = bass_pitch;
            lowered.set_octave(Some(3));
            if let Some(note) = chord
                .notes_mut()
                .iter_mut()
                .find(|note| note.pitch().name() == lowered.name())
            {
                note.set_pitch(lowered.clone());
            }
            lowered
        };
        chord.set_root(Some(root_pitch));
        chord.set_bass(Some(bass_pitch));

        Ok(Self {
            label: label.to_string(),
            root: Some(root),
            shorthand: parsed.shorthand,
            degrees: parsed.degrees,
            bass: Some(bass),
            all_degrees: all,
            chord,
        })
    }

    /// The label as it was given.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Whether the label names no chord, `N` or `X`.
    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    /// The chord the label sounds.
    pub fn chord(&self) -> &Chord {
        &self.chord
    }

    /// The chord, handed over.
    pub fn into_chord(self) -> Chord {
        self.chord
    }

    /// The root as written, `Bb`; harte-library's `get_root`.
    pub fn root_name(&self) -> Option<&str> {
        self.root.as_deref()
    }

    /// The bass as a degree above the root, `1` when none was written;
    /// harte-library's `get_bass`.
    pub fn bass_degree(&self) -> Option<&str> {
        self.bass.as_deref()
    }

    /// The shorthand, where the label carries one; harte-library's
    /// `get_shorthand`.
    pub fn shorthand(&self) -> Option<&str> {
        self.shorthand.as_deref()
    }

    /// The degrees written in parentheses, `*3` included, or `None` when
    /// there were none; harte-library's `get_degrees`.
    pub fn degrees(&self) -> Option<&[String]> {
        (!self.degrees.is_empty()).then_some(self.degrees.as_slice())
    }

    /// Every degree the chord sounds, the shorthand's and the written ones
    /// together with the root and the bass, in order; harte-library's
    /// `unwrap_shorthand`, which answers the written degrees alone for a
    /// label with no shorthand and nothing for one with neither.
    pub fn unwrap_shorthand(&self) -> Option<&[String]> {
        if self.shorthand.is_some() {
            Some(&self.all_degrees)
        } else {
            self.degrees()
        }
    }

    /// Every degree the chord sounds, the root and the bass among them, in
    /// order; empty for no chord.
    pub fn sounding_degrees(&self) -> &[String] {
        &self.all_degrees
    }

    /// Whether the bass is the root.
    pub fn bass_is_root(&self) -> bool {
        self.root.is_some() && self.bass.as_deref() == Some("1")
    }

    /// Whether the label carries a shorthand.
    pub fn contains_shorthand(&self) -> bool {
        self.shorthand.is_some()
    }

    /// The MIDI numbers of the pitches, lowest first.
    pub fn midi_pitches(&self) -> Vec<IntegerType> {
        let mut midi: Vec<IntegerType> = self.chord.pitches().iter().map(Pitch::midi).collect();
        midi.sort_unstable();
        midi
    }

    /// Which of the twelve pitch classes the chord sounds, one flag each,
    /// counted from C, or from the root with `transpose`.
    pub fn multi_hot_encoding(&self, transpose: bool) -> [u8; 12] {
        let shift = if transpose {
            self.chord.root().map_or(0, crate::chord::root::pitch_class)
        } else {
            0
        };
        let mut flags = [0u8; 12];
        for pitch in self.chord.pitches() {
            let class = (crate::chord::root::pitch_class(&pitch) + 12 - shift) % 12;
            flags[usize::from(class)] = 1;
        }
        flags
    }

    /// The label rewritten with the largest shorthand its degrees contain,
    /// the degrees left over in parentheses and the bass after a slash:
    /// `C:(b3,5)` is `C:min`. A label whose degrees fit no shorthand comes
    /// back as it was written; so does `N`.
    pub fn prettify(&self) -> String {
        let Some(root) = &self.root else {
            return self.label.clone();
        };
        let degrees: Vec<&str> = self
            .all_degrees
            .iter()
            .map(String::as_str)
            .filter(|degree| *degree != "1")
            .collect();
        let Some((grades, shorthand)) = DEGREE_SHORTHANDS
            .iter()
            .find(|(grades, _)| grades.iter().all(|grade| degrees.contains(grade)))
        else {
            return self.label.clone();
        };
        let left: Vec<&str> = degrees
            .iter()
            .copied()
            .filter(|degree| !grades.contains(degree))
            .collect();
        let mut pretty = format!("{root}:{shorthand}");
        if !left.is_empty() {
            pretty.push('(');
            pretty.push_str(&left.join(","));
            pretty.push(')');
        }
        if let Some(bass) = self.bass.as_deref().filter(|bass| *bass != "1") {
            pretty.push('/');
            pretty.push_str(bass);
        }
        pretty
    }
}

impl PartialEq for Harte {
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root && self.all_degrees == other.all_degrees && self.bass == other.bass
    }
}

impl fmt::Display for Harte {
    /// The label in its canonical spelling: root, shorthand, degrees and
    /// bass, without spaces; `N` for no chord.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some(root) = &self.root else {
            return f.write_str("N");
        };
        f.write_str(root)?;
        if let Some(shorthand) = &self.shorthand {
            write!(f, ":{shorthand}")?;
        }
        if !self.degrees.is_empty() {
            if self.shorthand.is_none() {
                f.write_str(":")?;
            }
            write!(f, "({})", self.degrees.join(","))?;
        }
        if let Some(bass) = self.bass.as_deref().filter(|bass| *bass != "1") {
            write!(f, "/{bass}")?;
        }
        Ok(())
    }
}

impl FromStr for Harte {
    type Err = Error;

    fn from_str(label: &str) -> Result<Self> {
        Self::new(label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// harte-library's `test_harte_interval`, every case.
    #[test]
    fn harte_degrees_convert_to_music21_intervals() {
        for (harte, music21) in [
            ("b1", "d1"),
            ("1", "P1"),
            ("#1", "A1"),
            ("b2", "m2"),
            ("2", "M2"),
            ("#2", "A2"),
            ("b3", "m3"),
            ("3", "M3"),
            ("#3", "A3"),
            ("b4", "d4"),
            ("4", "P4"),
            ("#4", "A4"),
            ("b5", "d5"),
            ("5", "P5"),
            ("#5", "A5"),
            ("b6", "m6"),
            ("6", "M6"),
            ("#6", "A6"),
            ("b7", "m7"),
            ("7", "M7"),
            ("#7", "A7"),
            ("b8", "d1"),
            ("8", "P1"),
            ("#8", "A1"),
            ("b9", "m2"),
            ("9", "M2"),
            ("#9", "A2"),
            ("b10", "m3"),
            ("10", "M3"),
            ("#10", "A3"),
            ("b11", "d4"),
            ("11", "P4"),
            ("#11", "A4"),
            ("b12", "d5"),
            ("12", "P5"),
            ("#12", "A5"),
            ("b13", "m6"),
            ("13", "M6"),
            ("#13", "A6"),
            ("b14", "m7"),
            ("14", "M7"),
            ("#14", "A7"),
        ] {
            assert_eq!(convert_interval(harte).unwrap(), music21, "{harte}");
        }
        assert_eq!(convert_interval("bb7").unwrap(), "d7");
        assert_eq!(convert_interval("##4").unwrap(), "P5");
        assert_eq!(convert_interval("bb5").unwrap(), "P4");
        assert_eq!(convert_interval("bb1").unwrap(), "m7");
        assert_eq!(convert_interval("b#3").unwrap(), "A3");
        assert_eq!(convert_interval("*3").unwrap(), "M3");
        assert!(convert_interval("###4").is_err());
        assert!(convert_interval("x").is_err());
    }

    /// harte-library's `test_interval_extraction`.
    #[test]
    fn the_intervals_of_a_chord_are_read_off_it() {
        for (label, intervals) in [
            ("C", vec!["P5", "M3"]),
            ("A", vec!["P5", "M3"]),
            ("C:maj", vec!["P5", "M3"]),
            ("C:min", vec!["P5", "m3"]),
            ("C:dim", vec!["d5", "m3"]),
            ("C:aug", vec!["A5", "M3"]),
            ("N", vec![]),
        ] {
            let harte = Harte::new(label).unwrap();
            let mut annotated = harte.chord().annotate_intervals(false, false).unwrap();
            annotated.sort();
            let mut wanted: Vec<String> = intervals.iter().map(|s| s.to_string()).collect();
            wanted.sort();
            assert_eq!(annotated, wanted, "{label}");
        }
    }

    /// harte-library's `test_ordering_of_degrees`.
    #[test]
    fn degrees_sound_in_order_whatever_order_they_were_written_in() {
        for (label, pitches) in [
            ("F:(b3, 5, b7, 11)", ["F", "A-", "C", "E-", "B-"]),
            ("F:(b3, 11, b7, 5)", ["F", "A-", "C", "E-", "B-"]),
            ("F:maj7(#11)", ["F", "A", "C", "E", "B"]),
        ] {
            assert_eq!(
                Harte::new(label).unwrap().chord().pitch_names(),
                pitches,
                "{label}"
            );
        }
    }

    #[test]
    fn a_label_is_read_into_its_parts_and_the_chord_it_sounds() {
        let names = |label: &str| -> Vec<String> {
            Harte::new(label)
                .unwrap()
                .chord()
                .pitches()
                .iter()
                .map(Pitch::name_with_octave)
                .collect()
        };
        assert_eq!(names("Ab:maj(9)/9"), ["A-4", "C5", "E-5", "B-3"]);
        assert_eq!(names("Bb:7/b7"), ["B-4", "D5", "F5", "A-3"]);
        assert_eq!(names("C:maj(*3)"), ["C4", "G4"]);
        assert_eq!(names("C:sus4(*3,9)"), ["C4", "F4", "G4", "D4"]);
        assert_eq!(names("Db"), ["D-4", "F4", "A-4"]);
        assert_eq!(names("C/5"), ["C4", "E4", "G3"]);

        let harte = Harte::new("Ab:maj(9)/9").unwrap();
        assert_eq!(harte.root_name(), Some("Ab"));
        assert_eq!(harte.bass_degree(), Some("9"));
        assert_eq!(harte.shorthand(), Some("maj"));
        assert_eq!(harte.degrees().unwrap(), ["9"]);
        assert_eq!(harte.unwrap_shorthand().unwrap(), ["1", "3", "5", "9"]);
        assert!(!harte.bass_is_root());
        assert!(harte.contains_shorthand());
        assert_eq!(harte.chord().root().unwrap().name_with_octave(), "A-4");
        assert_eq!(harte.chord().bass().unwrap().name_with_octave(), "B-3");
        assert_eq!(harte.midi_pitches(), [58, 68, 72, 75]);
        assert_eq!(harte.to_string(), "Ab:maj(9)/9");
        assert_eq!(harte.prettify(), "Ab:maj(9)/9");

        let seventh = Harte::new("F:maj7(#11)").unwrap();
        assert_eq!(seventh.midi_pitches(), [65, 69, 71, 72, 76]);
        assert_eq!(
            seventh.multi_hot_encoding(false),
            [1, 0, 0, 0, 1, 1, 0, 0, 0, 1, 0, 1]
        );
        assert_eq!(
            seventh.multi_hot_encoding(true),
            [1, 0, 0, 0, 1, 0, 1, 1, 0, 0, 0, 1]
        );
        assert_eq!(seventh.prettify(), "F:maj7(#11)");

        let written_out = Harte::new("C:(b3,5)").unwrap();
        assert_eq!(written_out.prettify(), "C:min");
        assert_eq!(written_out.unwrap_shorthand().unwrap(), ["b3", "5"]);
        assert!(!written_out.contains_shorthand());
        assert_eq!(written_out.to_string(), "C:(b3,5)");
        assert_eq!(Harte::new("C:sus4(*3,9)").unwrap().prettify(), "C:sus4(9)");
        assert_eq!(Harte::new("C:maj(*3)").unwrap().prettify(), "C:maj(*3)");
        assert_eq!(Harte::new("C:maj(*3)").unwrap().degrees().unwrap(), ["*3"]);
        assert!(Harte::new("C").unwrap().bass_is_root());
        assert_eq!(Harte::new("C").unwrap().unwrap_shorthand(), None);
    }

    #[test]
    fn no_chord_is_empty_and_a_bad_label_is_refused() {
        for label in ["N", "X"] {
            let none = Harte::new(label).unwrap();
            assert!(none.is_empty());
            assert!(none.chord().is_empty());
            assert_eq!(none.root_name(), None);
            assert_eq!(none.bass_degree(), None);
            assert_eq!(none.degrees(), None);
            assert_eq!(none.unwrap_shorthand(), None);
            assert_eq!(none.midi_pitches(), Vec::<IntegerType>::new());
            assert_eq!(none.multi_hot_encoding(false), [0; 12]);
            assert_eq!(none.to_string(), "N");
            assert_eq!(none.prettify(), label);
        }
        for label in [
            "",
            "H",
            "C:",
            "C:foo",
            "C:maj(",
            "C:maj(0)",
            "C:maj(14)",
            "G:9(113)",
            "C:maj/",
            "C x",
        ] {
            assert!(Harte::new(label).is_err(), "{label}");
        }
        assert_eq!(
            "C:min7/b3".parse::<Harte>().unwrap().to_string(),
            "C:min7/b3"
        );
        assert_eq!(Harte::new("C:maj").unwrap(), Harte::new("C:(3,5)").unwrap());
        assert_ne!(Harte::new("C:maj").unwrap(), Harte::new("C:min").unwrap());
        assert_ne!(Harte::new("C:maj").unwrap(), Harte::new("C:maj/3").unwrap());
    }
}
