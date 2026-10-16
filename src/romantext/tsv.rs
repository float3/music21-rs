//! The tabular format for harmonic analysis the Digital and Cognitive
//! Musicology Lab annotates its corpora in: music21's
//! `romanText.tsvConverter`.
//!
//! A DCML table is one row a chord: the measure and the beat it falls on,
//! the global and local keys, and the chord in parts -- the numeral, the
//! form, the figured bass, the added tones and the degree it is read in --
//! beside the label the annotator wrote. [`from_tsv`] reads a table into a
//! score of [`RomanNumeral`](crate::roman::RomanNumeral)s and [`to_tsv`]
//! writes one back out.
//!
//! The two standards the lab has published are both read: version 1 as the
//! Annotated Beethoven Corpus was first released, and version 2 as it is
//! now, whose columns are named differently and whose labels carry the
//! added tones in parentheses. [`Version`] says which.
//!
//! The two notations differ in more than their columns, and the swaps are
//! music21's: a half-diminished chord is `%` there and `ø` here, a seventh
//! says its quality there and not here, a minor key's degrees are written
//! in upper case there where music21 writes the mode's own, and the added
//! tones of version 2 become music21's `[no5][add6]` brackets.

use crate::bar::Ending;
use crate::chordsymbol::ChordSymbol;
use crate::defaults::{FloatType, IntegerType};
use crate::duration::{Duration, limited_fraction};
use crate::error::{Error, Result};
use crate::expressions::python_float;
use crate::key::Key;
use crate::makenotation::op_frac;
use crate::meter::TimeSignature;
use crate::roman::{Minor67Default, RomanNumeral};
use crate::stream::{Stream, StreamElement, StreamEvent, StreamKind};

use super::music21_key_name;

/// Which of the lab's two standards a table is written in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Version {
    /// The columns of the Annotated Beethoven Corpus as first released.
    #[default]
    One,
    /// The columns as the lab writes them now, the chord label carrying its
    /// added tones.
    Two,
}

impl Version {
    /// The standard a number names: music21's `dcml_version`.
    ///
    /// # Errors
    ///
    /// Any number but one or two.
    pub fn from_number(number: u8) -> Result<Self> {
        match number {
            1 => Ok(Self::One),
            2 => Ok(Self::Two),
            other => Err(Error::RomanText(format!(
                "dcml_version {other} is not in (1, 2)"
            ))),
        }
    }

    /// The number music21 calls it by.
    pub fn number(self) -> u8 {
        match self {
            Self::One => 1,
            Self::Two => 2,
        }
    }

    /// The columns this package reads, in the order music21 lists them:
    /// music21's `V1_HEADERS` and `V2_HEADERS`. A column named here is read
    /// into the row; any other is kept beside it.
    pub fn read_columns(self) -> &'static [&'static str] {
        match self {
            Self::One => &[
                "chord",
                "altchord",
                "measure",
                "beat",
                "totbeat",
                "timesig",
                "length",
                "global_key",
                "local_key",
                "pedal",
                "numeral",
                "form",
                "figbass",
                "changes",
                "relativeroot",
                "phraseend",
            ],
            Self::Two => &[
                "chord",
                "mn",
                "mn_onset",
                "timesig",
                "volta",
                "globalkey",
                "localkey",
                "pedal",
                "numeral",
                "form",
                "figbass",
                "changes",
                "relativeroot",
                "phraseend",
                "label",
            ],
        }
    }

    /// Every column the lab's standard has, in its own order, which is what
    /// a table is written with: music21's `DCML_V1_HEADERS` and
    /// `DCML_V2_HEADERS`.
    pub fn columns(self) -> &'static [&'static str] {
        match self {
            Self::One => &[
                "chord",
                "altchord",
                "measure",
                "beat",
                "totbeat",
                "timesig",
                "op",
                "no",
                "mov",
                "length",
                "global_key",
                "local_key",
                "pedal",
                "numeral",
                "form",
                "figbass",
                "changes",
                "relativeroot",
                "phraseend",
            ],
            Self::Two => &[
                "mc",
                "mn",
                "mc_onset",
                "mn_onset",
                "timesig",
                "staff",
                "voice",
                "volta",
                "label",
                "globalkey",
                "localkey",
                "pedal",
                "chord",
                "special",
                "numeral",
                "form",
                "figbass",
                "changes",
                "relativeroot",
                "cadence",
                "phraseend",
                "chord_type",
                "globalkey_is_minor",
                "localkey_is_minor",
                "chord_tones",
                "added_tones",
                "root",
                "bass_note",
            ],
        }
    }
}

/// Which way a label is being read: music21's `direction`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// From the lab's notation into music21's.
    IntoMusic21,
    /// From music21's into the lab's.
    IntoDcml,
}

/// Whether a key is minor, which the lab says by writing it in lower case:
/// music21's `isMinor`.
///
/// ```
/// use music21_rs::romantext::tsv::is_minor;
///
/// assert!(!is_minor("F"));
/// assert!(is_minor("f"));
/// assert!(!is_minor("F#"));
/// assert!(is_minor("bb"));
/// ```
pub fn is_minor(key: &str) -> bool {
    key == key.to_lowercase()
}

/// The swaps between the two notations: music21's `characterSwaps`.
///
/// A half-diminished chord is `%` in the lab's tables and `ø` in music21's
/// figures, and the lab says a seventh's quality where music21 does not.
///
/// ```
/// use music21_rs::romantext::tsv::{Direction, character_swaps};
///
/// assert_eq!(character_swaps("ii%", Direction::IntoMusic21), "ii\u{f8}");
/// assert_eq!(character_swaps("ii\u{f8}", Direction::IntoDcml), "ii%");
/// assert_eq!(character_swaps("VM7", Direction::IntoMusic21), "V7");
/// ```
pub fn character_swaps(written: &str, direction: Direction) -> String {
    match direction {
        Direction::IntoDcml => written.replace("/o", "%").replace('ø', "%"),
        Direction::IntoMusic21 => written.replace('%', "ø").replace("M7", "7"),
    }
}

/// The local key in its own terms: music21's `getLocalKey`, which reads a
/// degree against the global key and answers the key it names -- `V` of `G`
/// is `D`, and `ii` of `C` is `d`.
///
/// With `from_dcml` the degree is read the lab's way first, where a minor
/// key's degrees are written in upper case.
///
/// # Errors
///
/// A degree that names no major or minor triad.
pub fn local_key(degree: &str, global_key: &str, from_dcml: bool) -> Result<String> {
    let degree = if from_dcml {
        character_swaps(degree, Direction::IntoMusic21)
    } else {
        degree.to_string()
    };
    let numeral = RomanNumeral::with_minor_defaults(
        &degree,
        Key::from_tonic(global_key)?,
        Minor67Default::Flat,
        Minor67Default::Flat,
    )?;
    let chord = numeral.to_chord()?;
    let root = chord
        .root()
        .ok_or_else(|| Error::RomanText("a local key with no root".to_string()))?
        .name();
    // Only the letter takes the case, the accidental having none: a major
    // key on E double flat is `Ebb` and a minor one `ebb`. music21 writes
    // the flats `--`, which [`to_tsv`] writes back.
    let cased = |upper: bool| -> String {
        let mut letters = root.chars();
        let step = letters.next().unwrap_or_default();
        let step = if upper {
            step.to_ascii_uppercase()
        } else {
            step.to_ascii_lowercase()
        };
        format!("{step}{}", letters.as_str())
    };
    if chord.is_major_triad() {
        return Ok(cased(true));
    }
    if chord.is_minor_triad() {
        return Ok(cased(false));
    }
    Err(Error::RomanText(
        "local key must be major or minor".to_string(),
    ))
}

/// The degree one key stands on in another: music21's `localKeyAsRn`, which
/// is how a table says where the music has gone -- C major is the `VI` of e
/// minor.
///
/// # Errors
///
/// A key whose own tonic triad names no degree of the other.
pub fn local_key_as_roman(local: &Key, global: &Key) -> Result<String> {
    let letter = local.tonic_pitch_name_with_case();
    let figure = if letter.chars().next().is_some_and(char::is_lowercase) {
        "i"
    } else {
        "I"
    };
    let tonic = RomanNumeral::new(figure, local.clone())?.to_chord()?;
    let found = crate::roman::roman_numeral_from_chord(&tonic, Some(global))?.ok_or_else(|| {
        Error::RomanText(format!(
            "{letter} names no degree of {}",
            global.tonic_pitch_name_with_case()
        ))
    })?;
    Ok(found.roman_numeral().to_string())
}

/// The key a secondary degree establishes: music21's `getSecondaryKey`,
/// which reads the part after the slash of a `V/vi` in the local key.
///
/// # Errors
///
/// A degree that names no major or minor triad.
pub fn secondary_key(numeral: &str, local: &str) -> Result<String> {
    match numeral.split_once('/') {
        Some((_, secondary)) => local_key(secondary, local, false),
        None => Ok(local.to_string()),
    }
}

/// The `form` column for a numeral: music21's `getForm`, which reads it off
/// the figure -- `%` for a half-diminished chord, `o` for a diminished one,
/// `+M` for an augmented major seventh, `+` for an augmented chord and `M`
/// for a major seventh.
pub fn form_of(figure: &str) -> &'static str {
    if figure.contains('ø') {
        return "%";
    }
    if figure.contains('o') {
        return "o";
    }
    if figure.contains("+M") {
        return "+M";
    }
    if figure.contains('+') {
        return "+";
    }
    if figure.contains('M') {
        return "M";
    }
    ""
}

/// The figured bass the lab writes, which is music21's figure from its
/// first digit on: `M43` is `43`, and a figure with no digit in it says
/// nothing.
pub fn figured_bass_of(figures_written: &str) -> &str {
    match figures_written.find(|mark: char| mark.is_ascii_digit()) {
        Some(at) => &figures_written[at..],
        None => "",
    }
}

/// An added tone as the lab writes one inside the parentheses of a label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AddedTone<'a> {
    /// `+` to add without replacing, `-` to take a factor out.
    added_or_removed: Option<char>,
    /// `^` to replace the factor above, `v` the one below.
    above_or_below: Option<char>,
    /// The sharps or flats written in front of the figure.
    alteration: &'a str,
    /// The figure itself, nought to nineteen.
    factor: &'a str,
}

/// The added tones written in a label's parentheses, as music21's own
/// pattern finds them: each an optional `+` or `-`, an optional `^` or `v`,
/// any number of sharps or flats, and a figure of one or two digits.
fn added_tones(written: &str) -> Vec<AddedTone<'_>> {
    let bytes = written.as_bytes();
    let mut found = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let start = at;
        let mut cursor = at;
        let added_or_removed = match bytes.get(cursor) {
            Some(mark @ (b'+' | b'-')) => {
                cursor += 1;
                Some(*mark as char)
            }
            _ => None,
        };
        let above_or_below = match bytes.get(cursor) {
            Some(mark @ (b'^' | b'v')) => {
                cursor += 1;
                Some(*mark as char)
            }
            _ => None,
        };
        let alteration_start = cursor;
        if let Some(mark @ (b'#' | b'b')) = bytes.get(cursor) {
            while bytes.get(cursor) == Some(mark) {
                cursor += 1;
            }
        }
        let alteration = &written[alteration_start..cursor];
        // One digit, or two where the first is a one: music21's `1\d|\d`.
        let digits = match (bytes.get(cursor), bytes.get(cursor + 1)) {
            (Some(b'1'), Some(second)) if second.is_ascii_digit() => 2,
            (Some(first), _) if first.is_ascii_digit() => 1,
            _ => 0,
        };
        if digits == 0 {
            // Nothing here is a tone; music21's own scan moves on a
            // character and tries again.
            at = start + 1;
            continue;
        }
        found.push(AddedTone {
            added_or_removed,
            above_or_below,
            alteration,
            factor: &written[cursor..cursor + digits],
        });
        at = cursor + digits;
    }
    found
}

/// A label's parentheses turned into music21's brackets: music21's
/// `handleAddedTones`.
///
/// The lab writes the tones a chord adds in parentheses and leaves what
/// they replace to be worked out; music21 writes both, so a `V(6)` becomes
/// `V[no5][add6]`. A tone over a chord already seven deep replaces nothing,
/// a `+` says to add without replacing, a `-` says to take a factor out,
/// and `^` or `v` says which neighbour a step replaces. A `V(64)` is the
/// cadential six-four, which music21 has a figure of its own for.
///
/// ```
/// use music21_rs::romantext::tsv::handle_added_tones;
///
/// assert_eq!(handle_added_tones("V(64)"), "Cad64");
/// assert_eq!(handle_added_tones("i(4+2)"), "i[no3][add4][add2]");
/// assert_eq!(handle_added_tones("V"), "V");
/// ```
pub fn handle_added_tones(label: &str) -> String {
    // music21's pattern: everything up to the first parenthesis, the tones
    // inside it, and a secondary degree after it.
    let Some(open) = label.find('(') else {
        return label.to_string();
    };
    let primary = &label[..open];
    let rest = &label[open + 1..];
    // The last parenthesis that leaves either nothing or a secondary
    // degree behind it, since music21's `.*` takes as much as it can.
    let Some(close) = rest.rfind(')').filter(|close| {
        let after = &rest[close + 1..];
        after.is_empty() || after.starts_with('/')
    }) else {
        return label.to_string();
    };
    let inside = &rest[..close];
    let secondary = &rest[close + 1..];
    if primary == "V" && inside == "64" {
        return format!("Cad64{secondary}");
    }
    // The figure the chord already carries, which says how deep it is: the
    // digits and slashes the primary ends with.
    let figure_at = primary
        .rfind(|mark: char| !mark.is_ascii_digit() && mark != '/')
        .map_or(0, |at| {
            at + primary[at..].chars().next().map_or(1, char::len_utf8)
        });
    let figure = &primary[figure_at..];
    let threshold = match figure {
        "" | "5" | "53" | "5/3" | "3" | "7" => 7,
        _ => 8,
    };
    let mut omissions = String::new();
    let mut additions = String::new();
    for tone in added_tones(inside) {
        if tone.added_or_removed == Some('-') {
            omissions.push_str(&format!("[no{}]", tone.factor));
            continue;
        }
        let factor: i32 = tone.factor.parse().unwrap_or(0);
        let replace_above = if tone.added_or_removed == Some('+')
            || factor >= threshold
            || matches!(factor, 1 | 3 | 5)
        {
            // A tone the chord does not reach, one written `+` to say it
            // replaces nothing, and a factor the chord already has all
            // replace nothing.
            None
        } else if matches!(factor, 2 | 4 | 6) {
            // A second, a fourth or a sixth replaces the note below it
            // unless it is sharpened or marked as replacing the one above.
            Some(tone.alteration == "#" || tone.above_or_below == Some('^'))
        } else {
            Some(tone.alteration != "b" && tone.above_or_below != Some('v'))
        };
        if let Some(above) = replace_above {
            let replaced = if above { factor + 1 } else { factor - 1 };
            omissions.push_str(&format!("[no{replaced}]"));
        }
        // A nought says the seventh replaces the root.
        let factor = if factor == 0 { 7 } else { factor };
        additions.push_str(&format!("[add{}{factor}]", tone.alteration));
    }
    format!("{primary}{omissions}{additions}{secondary}")
}

/// Whether a local key is written as a degree rather than as a key: music21
/// asks whether it holds a roman numeral, which any `i` or `v` in it is.
fn is_degree(local_key: &str) -> bool {
    local_key
        .chars()
        .any(|mark| matches!(mark.to_ascii_lowercase(), 'i' | 'v'))
}

/// Every digit of a label prefixed with `d`, as music21 prefixes the
/// figures of a major-minor seventh on a degree other than the fifth: the
/// lab writes `II7` where music21 writes `IId7`.
fn prefix_sevenths(label: &str) -> String {
    let bytes = label.as_bytes();
    let mut out = String::with_capacity(label.len() + 2);
    let mut at = 0;
    while at < bytes.len() {
        if !bytes[at].is_ascii_digit() {
            out.push(bytes[at] as char);
            at += 1;
            continue;
        }
        let start = at;
        while bytes.get(at).is_some_and(u8::is_ascii_digit) {
            at += 1;
        }
        // music21 leaves a run alone where a digit or a closing bracket
        // follows it, so that an `[add13]` does not become `[addd13]`.
        if bytes.get(at) == Some(&b']') {
            out.push_str(&label[start..at]);
        } else {
            out.push('d');
            out.push_str(&label[start..at]);
        }
    }
    out
}

/// A chord of no harmony at all, which the lab writes `@none`: music21's
/// `NoChord`.
pub(crate) fn no_chord() -> ChordSymbol {
    ChordSymbol::no_chord(None)
}

/// The length a beat stands at, in quarters from the start of its measure:
/// the lab counts beats from one, in quarter notes.
pub(crate) fn offset_in_measure(beat: FloatType) -> FloatType {
    beat - 1.0
}

/// One row of a DCML table, read: music21's `TabChord` and `TabChordV2`.
///
/// The columns of the two standards differ in their names and in what they
/// carry, and a row of either is read into this one type -- what the lab's
/// version 1 calls `measure` and `beat` its version 2 calls `mn` and
/// `mn_onset`, and counts the onset in whole notes rather than in beats.
/// Every column the standard does not name is kept beside the row, as
/// music21 keeps its `extra`.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TabChord {
    /// Which standard the row was read in.
    pub version: Version,
    /// The label the annotator wrote: the lab's `chord`.
    pub chord: String,
    /// The degree alone.
    pub numeral: String,
    /// The degree the chord is read in, written after a slash.
    pub relativeroot: Option<String>,
    /// `o`, `+`, `%` or `M`: how the chord is built over the degree.
    pub form: Option<String>,
    /// The figured bass under it.
    pub figbass: Option<String>,
    /// The tones it adds or leaves out.
    pub changes: Option<String>,
    /// Whether a phrase ends here.
    pub phraseend: Option<String>,
    /// The note held under it.
    pub pedal: Option<String>,
    /// The key in force, as a degree of the global key or as a key.
    pub local_key: String,
    /// The key the piece is in.
    pub global_key: String,
    /// The beat of the measure the chord falls on, counted from one.
    pub beat: FloatType,
    /// The measure it falls in.
    pub measure: i32,
    /// The meter in force, as a ratio.
    pub timesig: String,
    /// Which time through a repeat the row belongs to, where the lab's
    /// version 2 says.
    pub volta: Option<String>,
    /// How long the chord sounds, where the lab's version 1 says.
    pub length: Option<FloatType>,
    /// Every other column of the row, by name.
    pub extra: Vec<(String, String)>,
}

impl TabChord {
    /// A row read against a table's own header: music21's
    /// `populateFromRow`, which reads the columns its standard names and
    /// keeps the rest beside them.
    pub fn from_row(header: &[String], row: &[String], version: Version) -> Result<Self> {
        let mut read = Self {
            version,
            ..Self::default()
        };
        let read_columns = version.read_columns();
        for (index, name) in header.iter().enumerate() {
            let Some(cell) = row.get(index) else {
                continue;
            };
            if !read_columns.contains(&name.as_str()) {
                if !cell.is_empty() {
                    read.extra.push((name.clone(), cell.clone()));
                }
                continue;
            }
            let text = cell.clone();
            match name.as_str() {
                "chord" => read.chord = text,
                "numeral" => read.numeral = text,
                "relativeroot" => read.relativeroot = Some(text),
                "form" => read.form = Some(text),
                "figbass" => read.figbass = Some(text),
                "changes" => read.changes = Some(text),
                "phraseend" => read.phraseend = Some(text),
                "pedal" => read.pedal = Some(text),
                "timesig" => read.timesig = text,
                "volta" => read.volta = Some(text),
                "local_key" | "localkey" => read.local_key = text,
                "global_key" | "globalkey" => read.global_key = text,
                "measure" | "mn" => {
                    read.measure = text.parse().map_err(|_| {
                        Error::RomanText(format!("{name} is not a measure number: {text}"))
                    })?;
                }
                "beat" => {
                    read.beat = text
                        .parse()
                        .map_err(|_| Error::RomanText(format!("beat is not a number: {text}")))?;
                }
                // The lab's version 2 counts the onset in whole notes from
                // the start of the measure, sometimes as a fraction.
                "mn_onset" => read.beat = float_or_fraction(&text)? * 4.0 + 1.0,
                // music21 reads the column with `float`, which refuses a
                // cell saying nothing rather than reading it as no length.
                "length" => {
                    read.length = Some(text.parse().map_err(|_| {
                        Error::RomanText(format!("length is not a number: {text}"))
                    })?);
                }
                // `altchord` and `totbeat` music21 reads onto attributes of
                // the row that nothing asks again, and `label` onto no
                // attribute at all, so none of the three is kept beside the
                // row -- which is what the writer reads for the columns it
                // cannot work out itself.
                _ => {}
            }
        }
        Ok(read)
    }

    /// The label as music21 writes one, which is what a
    /// [`RomanNumeral`](crate::roman::RomanNumeral) is built from: music21's
    /// `_changeRepresentation` and the first half of its `tabToM21`.
    fn music21_label(&self) -> String {
        // music21's `_changeRepresentation` branches on whether the local
        // key and the degree it is read in are minor, but every branch
        // calls one function whose `minor` argument it never reads, so the
        // swaps are the same either way.
        let numeral = character_swaps(&self.numeral, Direction::IntoMusic21);
        let relativeroot = self
            .relativeroot
            .as_deref()
            .filter(|root| !root.is_empty())
            .map(|root| character_swaps(root, Direction::IntoMusic21));
        let form = self
            .form
            .as_deref()
            .map(|form| form.replace('%', "ø"))
            .filter(|form| !form.is_empty());
        if self.version == Version::Two && !self.chord.is_empty() {
            let mut label = handle_added_tones(&self.chord.replace('%', "ø"));
            // A major-minor seventh away from the fifth degree says so in
            // music21's figure and not in the lab's.
            if self
                .extra
                .iter()
                .any(|(name, value)| name == "chord_type" && value == "Mm7")
                && self.numeral != "V"
            {
                label = prefix_sevenths(&label);
            }
            return label;
        }
        let mut label = format!(
            "{numeral}{}{}",
            form.unwrap_or_default(),
            self.figbass.clone().unwrap_or_default()
        );
        if let Some(root) = relativeroot.filter(|root| !root.is_empty()) {
            label.push('/');
            label.push_str(&root);
        }
        label
    }

    /// The key the chord is read in: the local key as a key of its own,
    /// where the table wrote it as a degree of the global key.
    ///
    /// # Errors
    ///
    /// A local key that names no major or minor triad of the global one.
    pub fn key(&self) -> Result<Key> {
        let local = character_swaps(&self.local_key, Direction::IntoMusic21);
        let named = if is_degree(&local) {
            local_key(&local, &self.global_key, false)?
        } else {
            local
        };
        Key::from_tonic(&named)
    }

    /// The chord the row stands for: a numeral in the key in force, or the
    /// chord of no harmony the lab writes `@none`.
    ///
    /// The sixth and the seventh of a minor key are read flat, as music21
    /// reads a table's.
    ///
    /// # Errors
    ///
    /// A label or a key the crate cannot read.
    pub fn element(&self) -> Result<StreamElement> {
        if self.numeral.is_empty() || self.numeral == "@none" {
            return Ok(no_chord().into());
        }
        let numeral = RomanNumeral::with_minor_defaults(
            self.music21_label(),
            self.key()?,
            Minor67Default::Flat,
            Minor67Default::Flat,
        )?;
        let chord = numeral
            .to_chord()?
            .with_duration(Duration::new(0.0)?)
            .with_numeral(numeral);
        Ok(chord.into())
    }
}

/// A number written plainly or as a fraction: music21's `_float_or_frac`,
/// which the lab's version 2 needs for an onset of `1/2`.
fn float_or_fraction(text: &str) -> Result<FloatType> {
    if let Ok(number) = text.parse::<FloatType>() {
        return Ok(number);
    }
    let (numerator, denominator) = text
        .split_once('/')
        .ok_or_else(|| Error::RomanText(format!("not a number: {text}")))?;
    let numerator: FloatType = numerator
        .parse()
        .map_err(|_| Error::RomanText(format!("not a number: {text}")))?;
    let denominator: FloatType = denominator
        .parse()
        .map_err(|_| Error::RomanText(format!("not a number: {text}")))?;
    if denominator == 0.0 {
        return Err(Error::RomanText(format!("not a number: {text}")));
    }
    Ok(numerator / denominator)
}

/// The rows of a table, each read: music21's `TsvHandler.tsvToChords`.
///
/// # Errors
///
/// A table with no header, or a cell the standard cannot read.
pub fn tsv_to_chords(text: &str, version: Version) -> Result<Vec<TabChord>> {
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let header: Vec<String> = match lines.next() {
        Some(line) => cells(line),
        None => return Err(Error::RomanText("a table with no header".to_string())),
    };
    lines
        .map(|line| TabChord::from_row(&header, &cells(line), version))
        .collect()
}

/// A line of a table, its cells unquoted as a reader of tab-separated
/// values unquotes them.
fn cells(line: &str) -> Vec<String> {
    line.split('\t')
        .map(|cell| {
            let cell = cell.trim_end_matches('\r');
            match cell
                .strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
            {
                Some(inside) => inside.replace("\"\"", "\""),
                None => cell.to_string(),
            }
        })
        .collect()
}

/// A DCML table read as a score of one part: music21's
/// `TsvHandler.toM21Stream`, which prepares the measures from the table's
/// own measure numbers and meters and then puts each chord in the measure
/// it falls in.
///
/// The score is the analysis alone -- the numerals and the keys, with no
/// notes of the music they describe -- and each numeral lasts until the
/// next, the last to the end of its measure, which is music21's
/// `extendDuration`.
///
/// ```
/// use music21_rs::romantext::tsv::{Version, from_tsv};
///
/// let table = "chord\tmeasure\tbeat\ttimesig\tglobal_key\tlocal_key\tnumeral\n\
///              .C.I\t1\t1.0\t4/4\tC\tI\tI\n\
///              V\t2\t1.0\t4/4\tC\tI\tV\n";
/// let read = from_tsv(table, Version::One)?;
/// assert_eq!(read.parts().len(), 1);
/// # Ok::<(), music21_rs::Error>(())
/// ```
///
/// # Errors
///
/// A table with no rows, a meter or a key it cannot read, or a label no
/// numeral can be built from.
pub fn from_tsv(text: &str, version: Version) -> Result<Stream> {
    let rows = tsv_to_chords(text, version)?;
    let Some(first) = rows.first() else {
        return Err(Error::RomanText("a table with no rows".to_string()));
    };
    let mut part = Stream::with_kind(StreamKind::Part);
    let key = Key::from_tonic(&character_swaps(&first.global_key, Direction::IntoMusic21))?;
    let mut meter = TimeSignature::from_ratio_string(&first.timesig)?;
    let mut said_timesig = first.timesig.clone();
    let mut measure_length = meter.bar_duration().quarter_length();
    let mut offset = 0.0;
    let mut previous_measure = first.measure - 1;
    let mut previous_volta = String::new();
    // Where each measure of the part stands and what it is numbered, so
    // that a chord can be put in the measure it falls in.
    let mut measures: Vec<(String, FloatType, Stream)> = Vec::new();
    let new_measure = |number: i32, volta: Option<&str>, offset: FloatType| {
        let mut measure = Stream::with_kind(StreamKind::Measure);
        measure.set_number(number);
        let suffix = volta.and_then(volta_suffix);
        if let Some(suffix) = &suffix {
            measure.set_number_suffix(Some(suffix.clone()));
        }
        let name = format!("{number}{}", suffix.unwrap_or_default());
        (name, offset, measure)
    };
    for row in &rows {
        let volta = row.volta.as_deref().filter(|volta| !volta.is_empty());
        if version == Version::Two && row.volta.as_deref().unwrap_or("") != previous_volta {
            previous_volta = row.volta.clone().unwrap_or_default();
        } else if row.measure == previous_measure {
            // Only a change of measure, or of the time through a repeat,
            // makes a measure.
            continue;
        }
        if row.measure > previous_measure + 1 {
            // Not every measure has a chord of its own.
            for number in (previous_measure + 1)..=row.measure {
                offset += measure_length;
                measures.push(new_measure(number, None, offset));
                previous_measure = number;
            }
            continue;
        }
        offset += measure_length;
        let (name, at, mut measure) = new_measure(row.measure, volta, offset);
        // A measure played one time through a repeat carries the bracket
        // drawn over it: music21 keeps a `RepeatBracket` spanning the
        // measures of each ending, and a measure here says which ending it
        // is in and that the bracket opens and closes on it, there being
        // one measure to an ending in a table.
        if let Some(volta) = volta.and_then(|volta| volta.parse::<u32>().ok()) {
            measure.set_ending(Some(Ending::new(vec![volta], true, true)));
        }
        if row.timesig != said_timesig {
            let said = TimeSignature::from_ratio_string(&row.timesig)?;
            measure.insert(offset_in_measure(row.beat), said.clone());
            said_timesig = row.timesig.clone();
            measure_length = said.bar_duration().quarter_length();
            meter = said;
        }
        measures.push((name, at, measure));
        previous_measure = row.measure;
    }
    let _ = &meter;
    // The key and the meter stand at the start of the first measure.
    if let Some((_, _, measure)) = measures.first_mut() {
        measure.insert(0.0, key);
        measure.insert(0.0, TimeSignature::from_ratio_string(&first.timesig)?);
    }
    for row in &rows {
        let volta = row.volta.as_deref().filter(|volta| !volta.is_empty());
        let name = format!(
            "{}{}",
            row.measure,
            volta.and_then(volta_suffix).unwrap_or_default()
        );
        let Some((_, _, measure)) = measures.iter_mut().find(|(known, _, _)| *known == name) else {
            return Err(Error::RomanText(format!("no measure {name} to analyse")));
        };
        measure.insert(offset_in_measure(row.beat), row.element()?);
    }
    for (_, at, measure) in measures {
        part.insert(at, measure);
    }
    lengthen_harmonies(&mut part)?;
    let mut score = Stream::with_kind(StreamKind::Score);
    score.insert(0.0, part);
    Ok(score)
}

/// The suffix a volta writes on a measure number: the first time through is
/// `a`, the second `b`, as music21 letters them.
fn volta_suffix(volta: &str) -> Option<String> {
    let number: usize = volta.parse().ok()?;
    let letter = char::from(b'a' + u8::try_from(number.checked_sub(1)?).ok()?);
    letter.is_ascii_lowercase().then(|| letter.to_string())
}

/// Every harmony lasting until the next, and the last to the end of the
/// part: music21's `extendDuration` over `harmony.Harmony`, and the length
/// it writes onto the last chord itself afterwards.
///
/// music21 extends the durations over the part *flattened*, so a chord
/// lasts across the barline into the next measure's chord, and then reads
/// the part's length again -- which the extending has just changed -- to
/// give the last chord the rest of the piece. Both steps are kept, in that
/// order, since the second reads what the first wrote.
fn lengthen_harmonies(part: &mut Stream) -> Result<()> {
    // Where each harmony stands in the part, measure by measure.
    let places = harmony_places(part);
    let total = part.end_offset();
    for (index, (offset, measure, event)) in places.iter().enumerate() {
        let until = places.get(index + 1).map_or(total, |(next, _, _)| *next);
        set_length(part, *measure, *event, op_frac(until - offset))?;
    }
    // The part is longer now than it was, and music21 gives the last chord
    // what is left of it.
    if let Some((offset, measure, event)) = places.last() {
        let total = part.end_offset();
        set_length(part, *measure, *event, op_frac(total - offset))?;
    }
    Ok(())
}

/// Every harmony of a part: where it stands in the part, which measure
/// holds it, and which of that measure's events it is.
fn harmony_places(part: &Stream) -> Vec<(FloatType, usize, usize)> {
    let mut places = Vec::new();
    for (measure, event) in part.events().iter().enumerate() {
        let Some(inner) = event.element().as_stream() else {
            continue;
        };
        for (index, held) in inner.events().iter().enumerate() {
            if held.element().is_instance("Harmony") {
                places.push((op_frac(event.offset() + held.offset()), measure, index));
            }
        }
    }
    places.sort_by(|left, right| left.0.total_cmp(&right.0));
    places
}

/// Writes one held element's length.
fn set_length(
    part: &mut Stream,
    measure: usize,
    event: usize,
    quarter_length: FloatType,
) -> Result<()> {
    let Some(StreamElement::Stream(inner)) = part
        .events_mut()
        .get_mut(measure)
        .map(StreamEvent::element_mut)
    else {
        return Ok(());
    };
    let Some(held) = inner.events_mut().get_mut(event) else {
        return Ok(());
    };
    let duration = Duration::new(quarter_length.max(0.0))?;
    match held.element_mut() {
        StreamElement::Chord(chord) => chord.set_duration(duration),
        StreamElement::ChordSymbol(symbol) => symbol.set_duration(duration),
        _ => {}
    }
    Ok(())
}

/// A score written as one of the lab's tables: music21's `M21toTSV`, which
/// writes a row for every numeral of the score, in the order a walk of it
/// meets them.
///
/// Every column the standard names is written, in the standard's own order,
/// and what the standard has no answer for is written empty, as music21
/// writes it: where a phrase ends, the note held under a chord, the tones a
/// chord changes. The columns music21 fills from a numeral's `editorial` --
/// the cells its reader did not read and kept beside the row -- are empty
/// here too; [`to_tsv_with_extra`] is how a caller hands them back.
///
/// ```
/// use music21_rs::romantext::tsv::{Version, from_tsv, to_tsv};
///
/// let table = "chord\tmeasure\tbeat\ttimesig\tglobal_key\tlocal_key\tnumeral\n\
///              I\t1\t1.0\t4/4\tC\tC\tI\n";
/// let written = to_tsv(&from_tsv(table, Version::One)?, Version::One)?;
/// let mut lines = written.lines();
/// assert!(lines.next().is_some_and(|header| header.starts_with("chord\t")));
/// assert!(lines.next().is_some_and(|row| row.starts_with("I\t\t1\t1.0")));
/// # Ok::<(), music21_rs::Error>(())
/// ```
///
/// # Errors
///
/// A version 1 table with no row at all -- music21 reads the global key off
/// the first numeral and asks its walk for one -- or a numeral standing
/// where no meter is in force, which has no beat.
pub fn to_tsv(stream: &Stream, version: Version) -> Result<String> {
    to_tsv_with_extra(stream, version, &[])
}

/// A score written as a table, with the cells music21 keeps on each
/// numeral's `editorial` given back: one list per chord the walk meets, in
/// that order, which for a score [`from_tsv`] read is the order of the
/// table's own rows.
///
/// music21's reader writes every column it does not read onto the numeral
/// it made, and its writer reads those cells back for the columns it cannot
/// work out itself -- which is how a table's `mc`, `staff`, `chord_tones`
/// and the rest survive a round trip. A chord here carries nothing of the
/// kind, so they are the caller's to keep: [`TabChord::extra`] is what
/// [`tsv_to_chords`] reads them into.
///
/// # Errors
///
/// As [`to_tsv`].
pub fn to_tsv_with_extra(
    stream: &Stream,
    version: Version,
    extra: &[Vec<(String, String)>],
) -> Result<String> {
    let written = written_chords(stream, version);
    let mut text = String::new();
    write_row(
        &mut text,
        version.columns().iter().map(|&name| name.to_string()),
    );
    // The global key is the key the first numeral is read in.
    let Some(global) = written.iter().find_map(|written| written.numeral) else {
        if version == Version::One {
            // music21 asks its walk for the next numeral and is answered
            // with a raise.
            return Err(Error::RomanText(
                "a score with no roman numeral to write".to_string(),
            ));
        }
        return Ok(text);
    };
    let global_key = global.key().clone();
    let global_name = music21_key_name(&global_key);
    for (index, written) in written.iter().enumerate() {
        let attributes = match version {
            Version::One => v1_attributes(written, &global_name)?,
            Version::Two => v2_attributes(written, &global_name, &global_key)?,
        };
        let kept = extra.get(index).map(Vec::as_slice).unwrap_or_default();
        let cells = version.columns().iter().map(|name| {
            // music21 reads a column off the row it built where that row
            // has an attribute of the name, and off the numeral's editorial
            // where it has none.
            cell_of(&attributes, name)
                .or_else(|| cell_of(kept, name))
                .unwrap_or_default()
        });
        write_row(&mut text, cells);
    }
    Ok(text)
}

/// One cell of a list of them, by column name.
fn cell_of<Name: AsRef<str>>(cells: &[(Name, String)], name: &str) -> Option<String> {
    cells
        .iter()
        .find(|(known, _)| known.as_ref() == name)
        .map(|(_, value)| value.clone())
}

/// The columns music21's `TabChord` holds for a numeral: the first half of
/// its `_m21ToTsv_v1`.
fn v1_attributes(written: &Written<'_>, global_key: &str) -> Result<Vec<(&'static str, String)>> {
    let numeral = written
        .numeral
        .ok_or_else(|| Error::RomanText("a chord of no harmony has no row here".to_string()))?;
    // music21 writes the degree again as the `altchord` where the numeral
    // standing for it was built in the chord's own key, which a secondary
    // numeral always is.
    let relativeroot = numeral.secondary().unwrap_or_default().to_string();
    Ok(vec![
        ("chord", numeral.figure().to_string()),
        ("altchord", relativeroot.clone()),
        ("measure", written.measure.unwrap_or(1).to_string()),
        ("beat", python_float(written.beat()?)),
        ("totbeat", String::new()),
        ("timesig", written.timesig()),
        ("length", python_number(written.length)),
        ("global_key", global_key.to_string()),
        ("local_key", music21_key_name(numeral.key())),
        ("pedal", String::new()),
        ("numeral", numeral.roman_numeral()),
        ("form", form_of(numeral.figure()).to_string()),
        (
            "figbass",
            figured_bass_of(numeral.figures_written()).to_string(),
        ),
        ("changes", String::new()),
        ("relativeroot", relativeroot),
        ("phraseend", String::new()),
    ])
}

/// The columns music21's `TabChordV2` holds: the first half of its
/// `_m21ToTsv_v2`, where the label is the whole figure and the chord of no
/// harmony is written `@none`.
fn v2_attributes(
    written: &Written<'_>,
    global_key: &str,
    global: &Key,
) -> Result<Vec<(&'static str, String)>> {
    let mut local_key = String::new();
    let mut form = String::new();
    let mut figbass = String::new();
    let mut relativeroot = String::new();
    let (chord, numeral_text) = match written.numeral {
        None => ("@none".to_string(), "@none".to_string()),
        Some(numeral) => {
            local_key = local_key_as_roman(numeral.key(), global)?;
            if let Some(root) = numeral.secondary() {
                relativeroot = character_swaps(root, Direction::IntoDcml);
            }
            form = form_of(numeral.figure()).to_string();
            figbass = figured_bass_of(numeral.figures_written()).to_string();
            (
                // The lab does not write the `d` music21 puts in front of
                // the figures of a major-minor seventh away from the fifth
                // degree.
                numeral.figure().replacen('d', "", 1),
                numeral.roman_numeral(),
            )
        }
    };
    Ok(vec![
        (
            "mn",
            written
                .measure
                .map(|number| number.to_string())
                .unwrap_or_default(),
        ),
        // The lab's version 2 counts the onset in whole notes from the
        // start of the measure, which music21 reads off the offset rather
        // than off the beat.
        ("mn_onset", python_number(written.offset / 4.0)),
        ("timesig", written.timesig()),
        ("volta", String::new()),
        ("globalkey", global_key.to_string()),
        ("localkey", local_key),
        ("pedal", String::new()),
        ("chord", chord),
        ("numeral", numeral_text),
        ("form", form),
        ("figbass", figbass),
        ("changes", String::new()),
        ("relativeroot", relativeroot),
        ("phraseend", String::new()),
    ])
}

/// One chord a table has a row for: the numeral it stands for, or none for
/// the chord of no harmony, and where in the score it stands.
#[derive(Debug)]
struct Written<'a> {
    numeral: Option<&'a RomanNumeral>,
    measure: Option<IntegerType>,
    offset: FloatType,
    padding: FloatType,
    length: FloatType,
    meter: Option<TimeSignature>,
}

impl Written<'_> {
    /// The meter in force as the lab writes it, and nothing where none is.
    fn timesig(&self) -> String {
        self.meter
            .as_ref()
            .map(TimeSignature::ratio_string)
            .unwrap_or_default()
    }

    /// The beat of the measure the chord falls on, counted from one and
    /// taking the measure's padding in, as music21's `beat` counts it.
    ///
    /// # Errors
    ///
    /// A chord standing where no meter is in force, which music21's `beat`
    /// raises for.
    fn beat(&self) -> Result<FloatType> {
        let meter = self.meter.as_ref().ok_or_else(|| {
            Error::RomanText("a chord with no meter in force does not have a beat".to_string())
        })?;
        meter.beat_proportion(op_frac(self.offset + self.padding))
    }
}

/// Every chord a table has a row for, in the order music21's `recurse`
/// meets them: a chord standing for a numeral, and under the lab's version
/// 2 the chord of no harmony beside it, which its version 1 writes no row
/// for.
fn written_chords(stream: &Stream, version: Version) -> Vec<Written<'_>> {
    let mut found = Vec::new();
    let mut meter = None;
    read_chords(stream, version, None, &mut meter, &mut found);
    found
}

/// The walk itself, which carries the meter in force and the measure being
/// read down into every stream it meets.
fn read_chords<'a>(
    stream: &'a Stream,
    version: Version,
    measure: Option<(IntegerType, FloatType)>,
    meter: &mut Option<TimeSignature>,
    found: &mut Vec<Written<'a>>,
) {
    for event in stream.events() {
        match event.element() {
            // A meter sorts before a chord standing at its own offset, so
            // the one in force is the last met.
            StreamElement::TimeSignature(said) => *meter = Some(said.clone()),
            StreamElement::Stream(inner) => {
                let inside = if inner.kind() == StreamKind::Measure {
                    Some((inner.number(), inner.padding_left()))
                } else {
                    measure
                };
                read_chords(inner, version, inside, meter, found);
            }
            element => {
                let numeral = match element {
                    StreamElement::Chord(chord) => chord.numeral(),
                    _ => None,
                };
                let no_chord = matches!(
                    element,
                    StreamElement::ChordSymbol(symbol) if symbol.is_no_chord()
                );
                if numeral.is_none() && !(version == Version::Two && no_chord) {
                    continue;
                }
                found.push(Written {
                    numeral,
                    measure: measure.map(|(number, _)| number),
                    offset: event.offset(),
                    padding: measure.map_or(0.0, |(_, padding)| padding),
                    length: element.quarter_length(),
                    meter: meter.clone(),
                });
            }
        }
    }
}

/// A row as Python's `csv.writer` writes one: the cells joined by tabs,
/// each quoted only where it has to be, and the line ended as that writer
/// ends it.
fn write_row(text: &mut String, cells: impl Iterator<Item = String>) {
    let mut first = true;
    for cell in cells {
        if !first {
            text.push('\t');
        }
        first = false;
        if cell.contains(['\t', '"', '\r', '\n']) {
            text.push('"');
            text.push_str(&cell.replace('"', "\"\""));
            text.push('"');
        } else {
            text.push_str(&cell);
        }
    }
    text.push_str("\r\n");
}

/// A length or an onset as Python prints the number music21 holds it as: a
/// fraction where `opFrac` could not keep it a float, and otherwise a
/// float, which always shows a decimal point.
fn python_number(value: FloatType) -> String {
    if is_fraction(value)
        && let Some((numerator, denominator)) = limited_fraction(value, DENOMINATOR_LIMIT)
    {
        return format!("{numerator}/{denominator}");
    }
    python_float(value)
}

/// Whether music21 holds a number as a `Fraction` rather than as a float:
/// `opFrac` keeps a float only where its denominator is a power of two no
/// larger than the limit.
fn is_fraction(value: FloatType) -> bool {
    (value * (DENOMINATOR_LIMIT as FloatType + 1.0)).fract() != 0.0
}

/// The largest denominator `opFrac` keeps.
const DENOMINATOR_LIMIT: i128 = 65535;

#[cfg(test)]
mod tests {
    use super::*;

    /// A cell the standard reads as a number and finds nothing in: music21
    /// coerces those columns with `int` and `float`, which refuse a cell
    /// saying nothing, so a table with one is read by neither side.
    #[test]
    fn a_number_a_table_leaves_out_is_refused() {
        let header = "chord\tmeasure\tbeat\ttimesig\tglobal_key\tlocal_key\tnumeral\tlength\n";
        let whole = format!("{header}I\t1\t1.0\t4/4\tC\tC\tI\t4.0\n");
        assert!(tsv_to_chords(&whole, Version::One).is_ok());
        for missing in [
            "I\t\t1.0\t4/4\tC\tC\tI\t4.0\n",
            "I\t1\t\t4/4\tC\tC\tI\t4.0\n",
            "I\t1\t1.0\t4/4\tC\tC\tI\t\n",
        ] {
            let table = format!("{header}{missing}");
            assert!(
                tsv_to_chords(&table, Version::One).is_err(),
                "a table with a number left out reads: {missing:?}"
            );
        }
    }

    /// Every answer here is music21's own for the same input.
    #[test]
    fn the_notations_swap_as_music21_swaps_them() {
        assert_eq!(character_swaps("ii%", Direction::IntoMusic21), "iiø");
        assert_eq!(character_swaps("iiø", Direction::IntoDcml), "ii%");
        assert_eq!(character_swaps("ii/o", Direction::IntoDcml), "ii%");
        assert_eq!(character_swaps("VM7", Direction::IntoMusic21), "V7");
        assert_eq!(character_swaps("viio7", Direction::IntoMusic21), "viio7");
        assert!(!is_minor("F"));
        assert!(is_minor("f"));
        assert!(!is_minor("F#"));
        assert!(is_minor("bb"));
    }

    /// Every form here is music21's own for the same figure.
    #[test]
    fn the_form_column_reads_off_the_figure() {
        for (figure, form) in [
            ("V", ""),
            ("viio7", "o"),
            ("IVM7", "M"),
            ("III+", "+"),
            ("IV+M7", "+M"),
            ("viiø7", "%"),
            ("I", ""),
            ("V7", ""),
            ("viio", "o"),
        ] {
            assert_eq!(form_of(figure), form, "the form of {figure}");
        }
        assert_eq!(figured_bass_of("M43"), "43");
        assert_eq!(figured_bass_of("43"), "43");
        assert_eq!(figured_bass_of("M"), "");
        assert_eq!(figured_bass_of(""), "");
    }

    /// Every label here is music21's own for the same label.
    #[test]
    fn added_tones_become_music21s_brackets() {
        for (label, written) in [
            ("V(64)", "Cad64"),
            ("i(4+2)", "i[no3][add4][add2]"),
            ("Viio7(b4-5)/V", "Viio7[no3][no5][addb4]/V"),
            ("vi(#74)", "vi[no3][add#7][add4]"),
            ("ii6(11#7b6)", "ii6[no8][no5][add11][add#7][addb6]"),
            ("i(#0)", "i[no1][add#7]"),
            ("V", "V"),
            ("V7(9)", "V7[add9]"),
            ("I(+6)", "I[add6]"),
            ("V(6)", "V[no5][add6]"),
            ("viio(b9)", "viio[addb9]"),
            ("I6(4)", "I6[no3][add4]"),
            ("V43(2)", "V43[no1][add2]"),
            ("i(v4)", "i[no3][add4]"),
            ("i(^4)", "i[no5][add4]"),
            ("V(13)", "V[add13]"),
        ] {
            assert_eq!(handle_added_tones(label), written, "the label {label}");
        }
    }

    /// Every key here is music21's own for the same degree.
    #[test]
    fn a_local_key_is_read_in_its_own_terms() {
        for (degree, global, from_dcml, named) in [
            ("V", "G", false, "D"),
            ("ii", "C", false, "d"),
            ("i", "C", false, "c"),
            ("#vii", "a", false, "g#"),
            ("vii", "a", true, "g"),
            // The crate spells a flat `b` where music21 writes `-`, as
            // every value here does; `to_tsv` writes music21's spelling.
            ("bIII", "c", false, "Ebb"),
            ("V", "f", false, "C"),
            ("iv", "Eb", false, "ab"),
        ] {
            assert_eq!(
                local_key(degree, global, from_dcml).unwrap(),
                named,
                "{degree} of {global}"
            );
        }
    }

    /// Every degree here is music21's own for the same pair of keys.
    #[test]
    fn a_key_names_a_degree_of_another() {
        for (local, global, degree) in [
            ("C", "e", "VI"),
            ("C#", "e", "#VI"),
            ("e", "C", "iii"),
            ("f", "A-", "vi"),
            ("g#", "a", "#vii"),
        ] {
            let local = Key::from_tonic(local).unwrap();
            let global = Key::from_tonic(global).unwrap();
            assert_eq!(
                local_key_as_roman(&local, &global).unwrap(),
                degree,
                "the degree of the pair"
            );
        }
        assert_eq!(secondary_key("V/vi", "C").unwrap(), "a");
        assert_eq!(secondary_key("V", "C").unwrap(), "C");
        assert_eq!(secondary_key("viio7/ii", "G").unwrap(), "a");
    }

    /// music21 prefixes the figures of a major-minor seventh away from the
    /// fifth degree, and leaves an added tone's figure alone.
    #[test]
    fn a_seventh_away_from_the_dominant_says_so() {
        assert_eq!(prefix_sevenths("II7"), "IId7");
        assert_eq!(prefix_sevenths("II65"), "IId65");
        assert_eq!(prefix_sevenths("I[add13]"), "I[add13]");
        assert_eq!(prefix_sevenths("II7[add9]"), "IId7[add9]");
        assert_eq!(prefix_sevenths("V"), "V");
    }

    #[test]
    fn a_version_is_one_or_two() {
        assert_eq!(Version::from_number(1).unwrap(), Version::One);
        assert_eq!(Version::from_number(2).unwrap().number(), 2);
        assert!(Version::from_number(3).is_err());
        assert!(Version::One.columns().contains(&"global_key"));
        assert!(Version::Two.columns().contains(&"globalkey"));
    }
}
