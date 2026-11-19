//! ABC's tokens and the handler that reads them: music21's `abcFormat`.
//!
//! The text is cut into tokens one character at a time ([`Handler::tokenize`]),
//! each token is told what is in force where it stands -- the default length,
//! the key, a tuplet, a tie -- and then read ([`Handler::process`]). A
//! processed handler is cut into tunes, voices and measures by token index,
//! so a caller keeping tokens of its own can cut those the same way. The
//! order and the oddities are music21's throughout.
//!
//! Each step is one music21 exposes: a token starts as music21's does, with
//! only its text, and its fields are filled by the step music21 calls
//! (`pre_parse`, `parse`, `update_ratio`), so a token can be read, changed and
//! read on between steps.

use std::ops::Range;

use crate::bar::{Barline, BarlineType, RepeatDirection};
use crate::clef::Clef;
use crate::defaults::{FloatType, IntegerType};
use crate::duration::{Duration, Tuplet};
use crate::error::{Error, FormatErrorKind, Position, Result};
use crate::key::KeySignature;
use crate::meter::TimeSignature;
use crate::notation::TieType;
use crate::pitch::Pitch;
use crate::spanner::SpannerKind;
use crate::stream::StreamElement;
use crate::tempo::MetronomeMark;

pub(super) fn abc_error(message: impl Into<String>) -> Error {
    Error::abc(message.into())
}

/// The ABC version a file is read as where it names none: music21's
/// `defaults.abcVersionDefault`.
pub const DEFAULT_VERSION: (u32, u32, u32) = (1, 3, 0);

/// The version from which accidentals carry through a measure.
const PROPAGATING_VERSION: (u32, u32, u32) = (2, 0, 0);

/// Every bar token and what music21 calls it, in the order music21 tries
/// them: music21's `ABC_BARS`.
pub const BARS: [(&str, &str); 14] = [
    (":|1", "light-heavy-repeat-end-first"),
    (":|2", "light-heavy-repeat-end-second"),
    ("|]", "light-heavy"),
    ("||", "light-light"),
    ("[|", "heavy-light"),
    ("[1", "regular-first"),
    ("[2", "regular-second"),
    ("|1", "regular-first"),
    ("|2", "regular-second"),
    (":|", "light-heavy-repeat-end"),
    ("|:", "heavy-light-repeat-start"),
    ("::", "heavy-heavy-repeat-bidirectional"),
    ("|", "regular"),
    (":", "dotted"),
];

/// The marks that may stand before a note's letter.
const DECORATIONS: &str = ".~^=_HLMOPSTuv";

/// The marks that alter a note's pitch.
const ACCIDENTALS: &str = "^=_";

/// The articulations a note can carry, in the order music21 gives them.
const ARTICULATIONS: [&str; 6] = [
    "staccato",
    "upbow",
    "downbow",
    "accent",
    "strongaccent",
    "tenuto",
];

/// music21's `opFrac`, near enough for lengths: the nearest fraction with a
/// small denominator where the float is within a hair of one.
pub(super) fn op_frac(value: FloatType) -> FloatType {
    if (value * 32768.0).fract() == 0.0 {
        return value;
    }
    for denominator in 1..=65535u32 {
        let scaled = value * FloatType::from(denominator);
        if (scaled - scaled.round()).abs() < 1e-9 * FloatType::from(denominator).max(1.0) {
            return scaled.round() / FloatType::from(denominator);
        }
    }
    value
}

/// music21's `ABCToken.stripComment`: the text before any `%`.
///
/// ```
/// use music21::abc::step::strip_comment;
///
/// assert_eq!(strip_comment("asdf  %     234"), "asdf  ");
/// ```
pub fn strip_comment(src: &str) -> &str {
    src.split('%').next().unwrap_or("")
}

// ------------------------------------------------------------------ tokens

/// A bar token: music21's `ABCBar`. Its fields are empty until
/// [`Bar::parse`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bar {
    /// The text it was read from.
    pub src: String,
    /// `repeat` or `barline`; empty where the text is no bar at all.
    pub bar_type: String,
    /// `regular`, `light-heavy` and the like.
    pub bar_style: String,
    /// `end`, `start`, `first` or `second`.
    pub repeat_form: String,
}

impl Bar {
    /// An unread bar of `src`.
    pub fn new(src: &str) -> Self {
        Self {
            src: src.to_string(),
            ..Self::default()
        }
    }

    /// A bar of `src`, read.
    pub fn parsed(src: &str) -> Self {
        let mut bar = Self::new(src);
        bar.parse();
        bar
    }

    /// music21's `ABCBar.parse`: the type, style and repeat form its text
    /// names.
    ///
    /// ```
    /// use music21::abc::step::Bar;
    ///
    /// let bar = Bar::parsed("|:");
    /// assert_eq!((bar.bar_type.as_str(), bar.bar_style.as_str()), ("repeat", "heavy-light"));
    /// assert_eq!(bar.repeat_form, "start");
    /// ```
    pub fn parse(&mut self) {
        let Some((_, described)) = BARS.iter().find(|(text, _)| *text == self.src.trim()) else {
            return;
        };
        let parts: Vec<&str> = described.split('-').collect();
        self.bar_type = if parts.contains(&"repeat") {
            "repeat"
        } else {
            "barline"
        }
        .to_string();
        if parts.len() == 1 {
            self.bar_style = parts[0].to_string();
        } else if parts.contains(&"first") {
            self.bar_style = "regular".to_string();
            self.repeat_form = "first".to_string();
        } else if parts.contains(&"second") {
            self.bar_style = "regular".to_string();
            self.repeat_form = "second".to_string();
        } else {
            self.bar_style = format!("{}-{}", parts[0], parts[1]);
        }
        if parts.len() > 2 {
            self.repeat_form = parts[3].to_string();
        }
    }

    /// music21's `isRepeat`.
    pub fn is_repeat(&self) -> bool {
        self.bar_type == "repeat"
    }

    /// music21's `isRegular`: a single light barline.
    pub fn is_regular(&self) -> bool {
        !self.is_repeat() && self.bar_style == "regular"
    }

    /// music21's `isRepeatBracket`: which ending the bar opens, where it
    /// opens one.
    pub fn repeat_bracket(&self) -> Option<u32> {
        match self.repeat_form.as_str() {
            "first" => Some(1),
            "second" => Some(2),
            _ => None,
        }
    }

    /// music21's `getBarObject`: nothing for a plain barline or an ending.
    pub fn barline(&self) -> Option<Barline> {
        if self.is_repeat() {
            return match self.repeat_form.as_str() {
                "start" => Some(Barline::repeat(RepeatDirection::Start, None)),
                "end" => Some(Barline::repeat(RepeatDirection::End, None)),
                _ => None,
            };
        }
        if self.bar_style == "regular" || self.bar_style.is_empty() {
            return None;
        }
        if self.repeat_bracket().is_some() {
            return None;
        }
        BarlineType::from_name(&self.bar_style)
            .ok()
            .map(Barline::new)
    }
}

/// A line of metadata, `T:Title` and the like: music21's `ABCMetadata`. Its
/// tag and data are empty until [`AbcMetadata::pre_parse`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AbcMetadata {
    /// The text it was read from.
    pub src: String,
    /// The capital letter, or `w`, before the colon.
    pub tag: String,
    /// What follows the colon, trimmed, a comment taken off.
    pub data: String,
    /// The line of the text it stands on, counted from one; nought where
    /// unknown.
    pub line: usize,
}

impl AbcMetadata {
    /// An unread line of `src`.
    pub fn new(src: &str) -> Self {
        Self {
            src: src.to_string(),
            ..Self::default()
        }
    }

    /// music21's `preParse`: the tag and the data split apart.
    ///
    /// ```
    /// use music21::abc::step::AbcMetadata;
    ///
    /// let mut line = AbcMetadata::new("T:tagData % a comment");
    /// line.pre_parse();
    /// assert_eq!((line.tag.as_str(), line.data.as_str()), ("T", "tagData"));
    /// ```
    pub fn pre_parse(&mut self) {
        let chars: Vec<char> = self.src.chars().collect();
        let is_tag = chars.len() >= 2
            && (chars[0].is_ascii_uppercase() || chars[0] == 'w')
            && chars[1] == ':';
        if !is_tag {
            return;
        }
        let mut pieces = strip_comment(&self.src).chars();
        self.tag = pieces.by_ref().take(1).collect();
        self.data = pieces.skip(1).collect::<String>().trim().to_string();
    }

    fn is(&self, tag: &str) -> bool {
        self.tag == tag
    }

    /// music21's `getTimeSignatureParameters`: the meter's numerator,
    /// denominator and symbol (`normal`, `common` or `cut`), nothing for
    /// `M:none`.
    pub fn meter_parameters(&self) -> Result<Option<(u32, u32, &'static str)>> {
        if !self.is("M") {
            return Err(abc_error("no time signature associated with this metadata"));
        }
        let data = self.data.as_str();
        if data.to_lowercase() == "none" {
            return Ok(None);
        }
        match data {
            "C" => return Ok(Some((4, 4, "common"))),
            "C|" => return Ok(Some((2, 2, "cut"))),
            _ => {}
        }
        let (numerator, denominator) = data
            .split_once('/')
            .ok_or_else(|| abc_error(format!("cannot read the meter {data:?}")))?;
        let digits = |text: &str| -> Result<u32> {
            text.chars()
                .filter(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .map_err(|_| abc_error(format!("cannot read the meter {data:?}")))
        };
        Ok(Some((digits(numerator)?, digits(denominator)?, "normal")))
    }

    /// music21's `getTimeSignatureObject`.
    pub fn meter(&self) -> Result<Option<TimeSignature>> {
        if !self.is("M") {
            return Err(abc_error(
                "no time signature associated with this non-metrical metadata.",
            ));
        }
        match self.meter_parameters()? {
            Some((numerator, denominator, _)) => {
                Ok(Some(TimeSignature::new(numerator, denominator)?))
            }
            None => Ok(None),
        }
    }

    /// music21's `getKeySignatureParameters`: the signed count of sharps and
    /// the mode, where the key names one.
    ///
    /// ```
    /// use music21::abc::step::AbcMetadata;
    ///
    /// let mut line = AbcMetadata::new("K:Eb Lydian");
    /// line.pre_parse();
    /// assert_eq!(line.key_parameters()?, (-2, Some("lydian")));
    /// # Ok::<(), music21::Error>(())
    /// ```
    pub fn key_parameters(&self) -> Result<(IntegerType, Option<&'static str>)> {
        if !self.is("K") {
            return Err(abc_error("no key signature associated with this metadata."));
        }
        key_parameters(&self.data)
    }

    /// music21's `getKeySignatureObject`: a key where there is a mode, a
    /// signature otherwise.
    pub fn key(&self) -> Result<StreamElement> {
        if !self.is("K") {
            return Err(abc_error("no key signature associated with this metadata"));
        }
        let (sharps, mode) = key_parameters(&self.data)?;
        Ok(key_element(sharps, mode))
    }

    /// music21's `getClefObject`: the clef a key line names, and how far the
    /// notes under it are moved.
    pub fn clef(&self) -> Result<Option<(Clef, IntegerType)>> {
        if !self.is("K") {
            return Err(abc_error(
                "no key signature associated with this metadata; needed for getting Clef Object",
            ));
        }
        let lower = self.data.to_lowercase();
        if lower.contains("-8va") {
            return Ok(Clef::from_string("treble8vb", 0)
                .ok()
                .map(|clef| (clef, -12)));
        }
        if lower.contains("bass") {
            return Ok(Some((Clef::bass(), -24)));
        }
        Ok(None)
    }

    /// music21's `getMetronomeMarkObject`.
    pub fn tempo(&self) -> Result<MetronomeMark> {
        if !self.is("Q") {
            return Err(abc_error("no tempo associated with this metadata"));
        }
        tempo_of(&self.data)
    }

    /// music21's `getDefaultQuarterLength`, for an `L:` or an `M:` line.
    ///
    /// ```
    /// use music21::abc::step::AbcMetadata;
    ///
    /// let mut line = AbcMetadata::new("M:2/4");
    /// line.pre_parse();
    /// assert_eq!(line.default_quarter_length()?, 0.25);
    /// # Ok::<(), music21::Error>(())
    /// ```
    pub fn default_quarter_length(&self) -> Result<FloatType> {
        let data = self.data.as_str();
        if self.is("L") && data.contains('/') {
            let (numerator, denominator) = data.split_once('/').unwrap_or(("1", "8"));
            let read = |text: &str| -> Result<FloatType> {
                text.trim()
                    .parse()
                    .map_err(|_| abc_error(format!("cannot read the default length {data:?}")))
            };
            // `L:1/G` is in some of the Essen files.
            let denominator = if denominator == "G" {
                4.0
            } else {
                read(denominator)?
            };
            return Ok(read(numerator)? * 4.0 / denominator);
        }
        if self.is("M") {
            return Ok(match self.meter_parameters()? {
                None => 0.5,
                // Under three quarters of a whole note the unit is a sixteenth.
                Some((numerator, denominator, _))
                    if FloatType::from(numerator) / FloatType::from(denominator) < 0.75 =>
                {
                    0.25
                }
                Some(_) => 0.5,
            });
        }
        Err(abc_error(format!(
            "no quarter length associated with this metadata: {data}"
        )))
    }
}

/// A key where there is a mode, a signature otherwise, as music21's
/// `KeySignature(sharps).asKey(mode)`.
pub(super) fn key_element(sharps: IntegerType, mode: Option<&str>) -> StreamElement {
    let signature = KeySignature::new(sharps);
    let Some(mode) = mode else {
        return signature.into();
    };
    match signature.try_as_key(Some(mode), None) {
        Ok(key) => key.into(),
        Err(_) => signature.into(),
    }
}

fn key_parameters(data: &str) -> Result<(IntegerType, Option<&'static str>)> {
    const NAMES: [&str; 20] = [
        "c", "g", "d", "a", "e", "b", "f#", "g#", "a#", "f", "bb", "eb", "d#", "ab", "e#", "db",
        "c#", "gb", "cb", "hp",
    ];
    let mut names: Vec<&str> = NAMES.to_vec();
    // Longest first, and the sort is stable.
    names.sort_by_key(|name| std::cmp::Reverse(name.len()));
    let chars: Vec<char> = data.chars().collect();
    let mut standard = "C".to_string();
    let mut remain = String::new();
    for target in names {
        let length = target.chars().count();
        let head: String = chars.iter().take(length).collect();
        if head.to_lowercase() == target {
            standard = head;
            remain = chars.iter().skip(length).collect();
            break;
        }
    }
    let letters: Vec<char> = standard.chars().collect();
    if letters.len() > 1 && letters[1] == 'b' {
        standard = format!("{}-", letters[0]);
    }
    let remain = remain.trim().to_lowercase();
    let mut mode: Option<&'static str> = if remain.is_empty() {
        Some("major")
    } else {
        [
            ("dor", "dorian"),
            ("phr", "phrygian"),
            ("lyd", "lydian"),
            ("mix", "mixolydian"),
            ("maj", "major"),
            ("ion", "ionian"),
            ("aeo", "aeolian"),
            ("m", "minor"),
        ]
        .into_iter()
        .find(|(prefix, _)| remain.starts_with(prefix))
        .map(|(_, mode)| mode)
    };
    // The Highland pipes: no signature written, or two sharps and a natural.
    if standard == "HP" {
        standard = "C".to_string();
        mode = None;
    } else if standard == "Hp" {
        standard = "D".to_string();
        mode = None;
    }
    let tonic = Pitch::from_name(&standard)?;
    let sharps = crate::key::pitch_to_sharps(&tonic, mode)?;
    Ok((sharps, mode))
}

fn tempo_of(data: &str) -> Result<MetronomeMark> {
    let mut text: Option<String> = None;
    let non_text = if data.contains('"') {
        let mut quoted = String::new();
        let mut outside = String::new();
        let mut open = false;
        for character in data.chars() {
            if character == '"' {
                open = !open;
            } else if open {
                quoted.push(character);
            } else {
                outside.push(character);
            }
        }
        text = Some(quoted.trim().to_string()).filter(|text| !text.is_empty());
        outside.trim().to_string()
    } else {
        data.trim().to_string()
    };
    let read = |number: &str| -> Result<FloatType> {
        number
            .trim()
            .parse::<FloatType>()
            .map_err(|_| abc_error(format!("cannot read the tempo {data:?}")))
    };
    let mut number: Option<FloatType> = None;
    let mut referent: Option<FloatType> = None;
    if !non_text.is_empty() {
        if let Some((lengths, count)) = non_text.split_once('=') {
            number = Some(read(count)?);
            let mut quarters = 0.0;
            for length in lengths.split(' ') {
                let (numerator, denominator) = length.split_once('/').unwrap_or(("1", "1"));
                quarters += read(numerator)? / read(denominator)? * 4.0;
            }
            referent = Some(quarters);
        } else {
            number = Some(read(&non_text)?);
        }
    }
    let mut mark = match (number, text) {
        (Some(number), Some(text)) => MetronomeMark::with_number_and_text(number, text),
        (Some(number), None) => MetronomeMark::new(number),
        (None, Some(text)) => MetronomeMark::from_text(text),
        (None, None) => MetronomeMark::default(),
    };
    if let Some(referent) = referent {
        mark.set_referent(Duration::new(referent)?);
    }
    Ok(mark)
}

/// A tuplet's opening, `(3` or `(p:q:r`: music21's `ABCTuplet`. Its ratio
/// is unknown until [`AbcTuplet::update_ratio`].
#[derive(Clone, Debug, PartialEq)]
pub struct AbcTuplet {
    /// The text it was read from.
    pub src: String,
    /// The line of the text it stands on, counted from one; nought where unknown.
    pub line: usize,
    /// How many notes it still takes; -1 until counted.
    pub note_count: i32,
    /// The notes played: 3 in 3:2; -1 until known.
    pub actual: i32,
    /// The notes they take the time of: 2 in 3:2.
    pub normal: i32,
}

impl AbcTuplet {
    /// An unread tuplet of `src`.
    pub fn new(src: &str) -> Self {
        Self {
            src: src.to_string(),
            line: 0,
            note_count: -1,
            actual: -1,
            normal: 1,
        }
    }

    fn pieces(&self) -> Vec<&str> {
        self.src.trim().split(':').collect()
    }

    /// music21's `updateRatio`: the ratio, which for 5, 7 and 9 depends on
    /// whether the meter in force is compound.
    ///
    /// ```
    /// use music21::abc::step::AbcTuplet;
    /// use music21::TimeSignature;
    ///
    /// let mut tuplet = AbcTuplet::new("(5");
    /// tuplet.update_ratio(Some(&TimeSignature::new(6, 8)?))?;
    /// assert_eq!((tuplet.actual, tuplet.normal), (5, 3));
    /// # Ok::<(), music21::Error>(())
    /// ```
    pub fn update_ratio(&mut self, meter: Option<&TimeSignature>) -> Result<()> {
        let switch = match meter {
            Some(meter) if meter.beat_division_count() == 3 => 3,
            _ => 2,
        };
        let pieces = self.pieces();
        let (actual, normal) = match pieces[0] {
            "(1" => (1, 1),
            "(2" => (2, 3),
            "(3" => (3, 2),
            "(4" => (4, 3),
            "(5" => (5, switch),
            "(6" => (6, 2),
            "(7" => (7, switch),
            "(8" => (8, 3),
            "(9" => (9, switch),
            other => {
                return Err(abc_error(format!(
                    "cannot handle tuplet of form: '{other}'"
                )));
            }
        };
        let given = pieces.get(1).and_then(|piece| piece.parse::<i32>().ok());
        self.actual = actual;
        self.normal = given.unwrap_or(normal);
        Ok(())
    }

    /// music21's `updateNoteCount`: how many notes the tuplet takes, and the
    /// tuplet itself.
    pub fn update_note_count(&mut self) -> Result<Tuplet> {
        if self.actual == -1 {
            return Err(abc_error("must set numberNotesActual with updateRatio()"));
        }
        let given = self
            .pieces()
            .get(2)
            .and_then(|piece| piece.parse::<i32>().ok());
        self.note_count = given.unwrap_or(self.actual);
        Ok(Tuplet::ratio(self.actual as u32, self.normal.max(0) as u32))
    }
}

/// A broken rhythm, `>` to `<<<`: music21's `ABCBrokenRhythmMarker`. Its
/// data is empty until [`BrokenRhythm::pre_parse`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BrokenRhythm {
    /// The text it was read from.
    pub src: String,
    /// The mark, trimmed.
    pub data: String,
}

impl BrokenRhythm {
    /// music21's `preParse`.
    pub fn pre_parse(&mut self) {
        self.data = self.src.trim().to_string();
    }
}

/// Which side of a broken-rhythm mark a note stands on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// Before the mark.
    Left,
    /// After the mark.
    Right,
}

impl Side {
    /// music21's name for the side.
    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}

/// A note, a rest, or with `is_chord` a chord: music21's `ABCNote` and
/// `ABCChord`. Its context is filled by [`Handler::process`], its pitch and
/// length by [`AbcNote::parse`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AbcNote {
    /// The text it was read from.
    pub src: String,
    /// The line of the text it starts on; nought where unknown.
    pub line: usize,
    /// Whether it is a chord.
    pub is_chord: bool,
    /// The accidental carried from earlier in the measure.
    pub carried_accidental: String,
    /// The chord symbols written over it, quotes kept.
    pub chord_symbols: Vec<String>,
    /// Whether it is a grace note.
    pub in_grace: bool,
    /// The default length in force, in quarter notes.
    pub default_quarter_length: Option<FloatType>,
    /// The broken-rhythm mark and the side of it the note stands on.
    pub broken_rhythm: Option<(String, Side)>,
    /// The key in force.
    pub key: Option<KeySignature>,
    /// The tuplet it is in.
    pub tuplet: Option<Tuplet>,
    /// The spanners in force, by where they stand among the handler's.
    pub spanners: Vec<usize>,
    /// The tie it starts, continues or stops.
    pub tie: Option<TieType>,
    /// music21's names: `staccato`, `upbow` and the rest.
    pub articulations: Vec<String>,
    /// Whether the accidental is shown: nothing where undecided.
    pub accidental_display: Option<bool>,
    /// Whether it is a rest.
    pub is_rest: bool,
    /// The pitch as music21 names one; nothing for a rest or a chord.
    pub pitch_name: Option<String>,
    /// The length in quarter notes.
    pub quarter_length: FloatType,
    /// A chord's notes, once parsed.
    pub sub_tokens: Vec<AbcNote>,
}

impl AbcNote {
    /// An unread note of `src`, carrying the accidental `carried`.
    pub fn new(src: &str, carried: &str) -> Self {
        Self {
            src: src.to_string(),
            carried_accidental: carried.to_string(),
            ..Self::default()
        }
    }

    /// An unread chord of `src`.
    pub fn chord(src: &str) -> Self {
        Self {
            src: src.to_string(),
            is_chord: true,
            ..Self::default()
        }
    }

    /// music21's `_splitChordSymbols`: the quoted strings, and what is left
    /// after the last of them.
    ///
    /// ```
    /// use music21::abc::step::AbcNote;
    ///
    /// let (symbols, rest) = AbcNote::split_chord_symbols("\"D7\"\"D\"d2");
    /// assert_eq!((symbols, rest.as_str()), (vec!["\"D7\"".to_string(), "\"D\"".to_string()], "d2"));
    /// ```
    pub fn split_chord_symbols(src: &str) -> (Vec<String>, String) {
        if !src.contains('"') {
            return (Vec::new(), src.to_string());
        }
        let chars: Vec<char> = src.chars().collect();
        let mut symbols = Vec::new();
        let mut index = 0;
        let mut after = 0;
        while index < chars.len() {
            if chars[index] == '"'
                && let Some(length) = chars[index + 1..].iter().position(|c| *c == '"')
            {
                let close = index + 1 + length;
                symbols.push(chars[index..=close].iter().collect::<String>());
                index = close + 1;
                after = index;
                continue;
            }
            index += 1;
        }
        if symbols.is_empty() {
            return (Vec::new(), src.to_string());
        }
        (symbols, chars[after..].iter().collect())
    }

    /// music21's `getPitchName`: the pitch as music21 names one, or nothing
    /// for a rest, and whether its accidental is shown. `forced_key` stands
    /// in for the note's own key.
    ///
    /// ```
    /// use music21::abc::step::AbcNote;
    /// use music21::KeySignature;
    ///
    /// let note = AbcNote::new("", "");
    /// assert_eq!(note.pitch_name("_g''", None)?, (Some("G-7".to_string()), Some(true)));
    /// let key = KeySignature::new(3);
    /// assert_eq!(note.pitch_name("c", Some(&key))?, (Some("C#5".to_string()), Some(false)));
    /// # Ok::<(), music21::Error>(())
    /// ```
    pub fn pitch_name(
        &self,
        src: &str,
        forced_key: Option<&KeySignature>,
    ) -> Result<(Option<String>, Option<bool>)> {
        let chars: Vec<char> = src.chars().collect();
        let mut text: String = if chars.len() > 1 && matches!(chars[0], 'u' | 'T') {
            chars[1..].iter().collect()
        } else {
            src.to_string()
        };
        text = text.replace('T', "");
        let name = text
            .chars()
            .find(|c| matches!(c, 'a'..='g' | 'A'..='G' | 'z'))
            .ok_or_else(|| abc_error(format!("cannot find any pitch information in: '{text}'")))?;
        if name == 'z' {
            return Ok((None, None));
        }
        let key = forced_key.or(self.key.as_ref());
        let count = |wanted: char| text.chars().filter(|c| *c == wanted).count();
        let mut octave: i32 = if name.is_lowercase() { 5 } else { 4 };
        octave -= count(',') as i32;
        octave += count('\'') as i32;
        let marks = |source: &str| -> String {
            let count = |wanted: char| source.chars().filter(|c| *c == wanted).count();
            format!(
                "{}{}{}",
                "-".repeat(count('_')),
                "#".repeat(count('^')),
                "n".repeat(count('='))
            )
        };
        let written = marks(&text);
        let carried = marks(&self.carried_accidental);
        if !carried.is_empty() && !written.is_empty() {
            return Err(abc_error("Carried accidentals not rendered moot."));
        }
        let mut name = name.to_uppercase().to_string();
        let display = if !carried.is_empty() {
            None
        } else if !written.is_empty() {
            Some(true)
        } else {
            match key {
                None => None,
                Some(key) => {
                    // The signature's own spelling of the note, not shown.
                    if let Ok(altered) = key.altered_pitches()
                        && let Some(pitch) = altered
                            .iter()
                            .find(|pitch| pitch.step().as_char().to_string() == name)
                    {
                        name = pitch.name();
                    }
                    Some(false)
                }
            }
        };
        let accidental = if carried.is_empty() { written } else { carried };
        Ok((Some(format!("{name}{accidental}{octave}")), display))
    }

    /// music21's `getQuarterLength`: the length `src` writes, against the
    /// default length (or `forced_default`), stretched by a broken rhythm.
    ///
    /// ```
    /// use music21::abc::step::{AbcNote, Side};
    ///
    /// let mut note = AbcNote::new("", "");
    /// note.default_quarter_length = Some(0.5);
    /// assert_eq!(note.quarter_length_of("A3/2", None)?, 0.75);
    /// note.broken_rhythm = Some(("<<".to_string(), Side::Right));
    /// assert_eq!(note.quarter_length_of("A", None)?, 0.875);
    /// # Ok::<(), music21::Error>(())
    /// ```
    pub fn quarter_length_of(
        &self,
        src: &str,
        forced_default: Option<FloatType>,
    ) -> Result<FloatType> {
        let default = forced_default
            .or(self.default_quarter_length)
            .ok_or_else(|| {
                abc_error("cannot calculate quarter length without a default quarter length")
            })?;
        let number: String = src
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '/')
            .collect();
        let whole = |text: &str| -> Result<FloatType> {
            text.trim()
                .parse::<FloatType>()
                .map_err(|_| abc_error(format!("cannot read the length in {src:?}")))
        };
        let mut length = match number.as_str() {
            "" => default,
            "/" => default * 0.5,
            "//" => default * 0.25,
            "///" => default * 0.125,
            text if text.starts_with('/') => {
                op_frac(default / whole(text.split('/').nth(1).unwrap_or(""))?)
            }
            text if text.ends_with('/') => {
                default * whole(text.split('/').next().unwrap_or(""))? / 2.0
            }
            text if text.matches('/').count() == 2 => 1.0,
            text if text.contains('/') => {
                let (numerator, denominator) = text.split_once('/').unwrap_or(("1", "1"));
                op_frac(default * whole(numerator)? / whole(denominator)?)
            }
            text => default * whole(text)?,
        };
        if let Some((symbol, side)) = &self.broken_rhythm {
            let (first, second) = match symbol.as_str() {
                ">" => (1.5, 0.5),
                "<" => (0.5, 1.5),
                ">>" => (1.75, 0.25),
                "<<" => (0.25, 1.75),
                ">>>" => (1.875, 0.125),
                "<<<" => (0.125, 1.875),
                _ => (1.0, 1.0),
            };
            length *= match side {
                Side::Left => first,
                Side::Right => second,
            };
        }
        Ok(length)
    }

    /// music21's `ABCNote.parse` and `ABCChord.parse`: the chord symbols,
    /// the pitch and the length, or for a chord its notes and length.
    /// `forced_default` and `forced_key` stand in for the note's own.
    pub fn parse(
        &mut self,
        forced_default: Option<FloatType>,
        forced_key: Option<&KeySignature>,
    ) -> Result<()> {
        let (symbols, rest) = Self::split_chord_symbols(&self.src);
        self.chord_symbols = symbols;
        if self.is_chord {
            return self.parse_chord(&rest, forced_key);
        }
        // A note music21 can find no pitch in is read as a C.
        let (name, display) = match self.pitch_name(&rest, forced_key) {
            Ok(read) => read,
            Err(Error::Abc(read)) if read.message.starts_with("cannot find any pitch") => {
                (Some("C".to_string()), Some(false))
            }
            Err(error) => return Err(error),
        };
        self.is_rest = name.is_none();
        self.pitch_name = name;
        self.accidental_display = display;
        self.quarter_length = self.quarter_length_of(&rest, forced_default)?;
        Ok(())
    }

    /// `ABCChord.parse`: the notes inside the brackets, each read alone,
    /// and the length of the first times the length after the brackets.
    fn parse_chord(&mut self, rest: &str, forced_key: Option<&KeySignature>) -> Result<()> {
        let chars: Vec<char> = rest.chars().collect();
        let close = chars.iter().position(|c| *c == ']').ok_or_else(|| {
            abc_error(format!(
                "Bad chord indicator: {}: no closing bracket found.",
                self.src
            ))
        })?;
        let outer: String = chars[close + 1..].iter().collect();
        let inner: String = chars[1..close].iter().collect();
        let outer_length = self.quarter_length_of(&outer, Some(1.0))?;
        let key = forced_key.cloned().or_else(|| self.key.clone());
        let mut handler = Handler::new(DEFAULT_VERSION);
        // What goes wrong inside is placed where the chord stands.
        handler.read_tokens(&inner).map_err(|(_, error)| error)?;
        let mut inner_length = 0.0;
        for token in handler.tokens {
            let Token::Note(mut note) = token else {
                continue;
            };
            note.parse(self.default_quarter_length, key.as_ref())?;
            if note.is_rest {
                continue;
            }
            if inner_length == 0.0 {
                inner_length = note.quarter_length;
            }
            self.sub_tokens.push(*note);
        }
        self.quarter_length = outer_length * inner_length;
        Ok(())
    }
}

/// What a closing mark closes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Closes {
    /// `)`: the latest slur.
    Slur,
    /// `!crescendo)!` or `!diminuendo)!`: the latest hairpin.
    Hairpin,
}

/// One token of ABC: music21's `ABCToken` and its subclasses.
#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    /// `T:Title` and the like.
    Metadata(AbcMetadata),
    /// `|`, `:|` and the like.
    Bar(Bar),
    /// `(3` and the like.
    Tuplet(AbcTuplet),
    /// `-`.
    Tie,
    /// A slur or a hairpin opening, and once processed the spanner it
    /// starts, by where it stands among the handler's.
    SpannerStart(SpannerKind, Option<usize>),
    /// `)`, `!crescendo)!` or `!diminuendo)!`.
    ParenStop(Closes),
    /// `.`.
    Staccato,
    /// `u`.
    Upbow,
    /// `v`.
    Downbow,
    /// `K`.
    Accent,
    /// `k`.
    Straccent,
    /// `M`.
    Tenuto,
    /// `{`.
    GraceStart,
    /// `}`.
    GraceStop,
    /// `>` to `<<<`.
    BrokenRhythm(BrokenRhythm),
    /// A note, rest or chord.
    Note(Box<AbcNote>),
}

impl Token {
    /// The text the token was read from, as music21 keeps it.
    pub fn src(&self) -> String {
        let fixed = match self {
            Self::Metadata(line) => return line.src.clone(),
            Self::Bar(bar) => return bar.src.clone(),
            Self::Tuplet(tuplet) => return tuplet.src.clone(),
            Self::BrokenRhythm(mark) => return mark.src.clone(),
            Self::Note(note) => return note.src.clone(),
            Self::Tie => "-",
            Self::SpannerStart(SpannerKind::Slur, _) => "(",
            Self::SpannerStart(..) => "!",
            Self::ParenStop(Closes::Slur) => ")",
            Self::ParenStop(Closes::Hairpin) => "!",
            Self::Staccato => ".",
            Self::Upbow => "u",
            Self::Downbow => "v",
            Self::Accent => "K",
            Self::Straccent => "k",
            Self::Tenuto => "M",
            Self::GraceStart => "{",
            Self::GraceStop => "}",
        };
        fixed.to_string()
    }

    /// Whether the token is a note, rest or chord.
    pub fn is_note(&self) -> bool {
        matches!(self, Self::Note(_))
    }

    /// The tag and data of a line of metadata.
    pub fn metadata(&self) -> Option<(&str, &str)> {
        match self {
            Self::Metadata(line) => Some((&line.tag, &line.data)),
            _ => None,
        }
    }

    /// The line of the text the token starts on, where it is one that can
    /// be refused.
    pub(super) fn line(&self) -> Option<usize> {
        match self {
            Self::Metadata(line) => Some(line.line),
            Self::Tuplet(tuplet) => Some(tuplet.line),
            Self::Note(note) => Some(note.line),
            _ => None,
        }
    }
}

/// `error`, placed at `line` where the token knows one.
pub(super) fn at_line(error: Error, line: Option<usize>) -> Error {
    match line {
        Some(line) if line > 0 => error.placed(|| Position::line(line)),
        _ => error,
    }
}

// ------------------------------------------------------------ text helpers

/// The items before, at, after and two after a place.
pub type Context<T> = (Option<T>, T, Option<T>, Option<T>);

/// music21's `_getLinearContext`: the items before, at, after and two after
/// `index`.
///
/// ```
/// use music21::abc::step::linear_context;
///
/// let chars: Vec<char> = "12345".chars().collect();
/// assert_eq!(linear_context(&chars, 3)?, (Some('3'), '4', Some('5'), None));
/// # Ok::<(), music21::Error>(())
/// ```
pub fn linear_context<T: Clone>(source: &[T], index: usize) -> Result<Context<T>> {
    let Some(here) = source.get(index) else {
        return Err(abc_error(format!(
            "bad index value {index}, max is {}",
            source.len() as i64 - 1
        )));
    };
    let before = index.checked_sub(1).and_then(|at| source.get(at)).cloned();
    Ok((
        before,
        here.clone(),
        source.get(index + 1).cloned(),
        source.get(index + 2).cloned(),
    ))
}

/// music21's `_getNextLineBreak`: where the next line break after `index`
/// stands, or the end.
pub fn next_line_break(chars: &[char], index: usize) -> usize {
    (index + 1..chars.len())
        .find(|at| chars[*at] == '\n')
        .unwrap_or(chars.len())
}

/// music21's `startsMetadata`: a capital letter, or `w`, then a colon, then
/// something other than a barline.
pub fn starts_metadata(c: char, next: Option<char>, next_next: Option<char>) -> bool {
    if next != Some(':') {
        return false;
    }
    match next_next {
        None | Some('|') => false,
        Some(_) => c == 'w' || (c.is_alphabetic() && c.is_uppercase()),
    }
}

/// A comment read by [`comment_at`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comment {
    /// How many characters after the `%` the comment runs on.
    pub skip: usize,
    /// A `%%key value` directive's key and value.
    pub directive: Option<(String, String)>,
}

/// music21's `processComment`: the comment at `pos`, to the end of its line.
pub fn comment_at(chars: &[char], pos: usize) -> Comment {
    let end = next_line_break(chars, pos);
    let skip = end.saturating_sub(pos + 1);
    let line: String = chars[pos.min(end)..end].iter().collect();
    Comment {
        skip,
        directive: directive_of(&line),
    }
}

/// `^%%([a-z\-]+)\s+(\S+)`: a directive's key and value.
fn directive_of(line: &str) -> Option<(String, String)> {
    let directive = line.strip_prefix("%%")?;
    let key: String = directive
        .chars()
        .take_while(|c| c.is_ascii_lowercase() || *c == '-')
        .collect();
    let after = &directive[key.len()..];
    if key.is_empty() || !after.starts_with(char::is_whitespace) {
        return None;
    }
    let value = after.split_whitespace().next()?;
    Some((key, value.to_string()))
}

/// `%abc-(\d+)\.(\d+)\.?(\d+)?` searched for: the first ABC version named.
pub fn version_in(text: &str) -> Option<(u32, u32, u32)> {
    let digits = |text: &str| -> usize { text.chars().take_while(char::is_ascii_digit).count() };
    for (at, _) in text.match_indices("%abc-") {
        let rest = &text[at + 5..];
        let major_end = digits(rest);
        if major_end == 0 || !rest[major_end..].starts_with('.') {
            continue;
        }
        let minor = &rest[major_end + 1..];
        let minor_end = digits(minor);
        if minor_end == 0 {
            continue;
        }
        let tail = minor[minor_end..]
            .strip_prefix('.')
            .unwrap_or(&minor[minor_end..]);
        let patch_end = digits(tail);
        let (Ok(major), Ok(minor_number)) = (rest[..major_end].parse(), minor[..minor_end].parse())
        else {
            continue;
        };
        let patch = tail[..patch_end].parse().unwrap_or(0);
        return Some((major, minor_number, patch));
    }
    None
}

/// music21's `_buildMeasureBoundaryIndices`: the start and end of each
/// measure from the places of its barlines, a span of one passed over.
///
/// ```
/// use music21::abc::step::measure_boundaries;
///
/// assert_eq!(measure_boundaries(&[8, 12, 13, 16], 20), [(0, 8), (8, 12), (13, 16), (16, 20)]);
/// ```
pub fn measure_boundaries(positions: &[usize], last_valid: usize) -> Vec<(usize, usize)> {
    let Some(first) = positions.first().copied() else {
        return Vec::new();
    };
    let mut pairs = vec![(0, first)];
    let mut start = first;
    for end in positions.iter().skip(1).copied() {
        if end == start + 1 {
            start = end;
            continue;
        }
        pairs.push((start, end));
        start = end;
    }
    if start != last_valid {
        pairs.push((start, last_valid));
    }
    pairs
}

/// One handler of [`merge_plan`]'s.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Merge {
    /// Every handler joined into one.
    Whole,
    /// The handler at this place, as it is.
    Kept(usize),
    /// The handler at this place joined to the one after it.
    Pair(usize),
}

/// music21's `mergeLeadingMetaData`, given whether each bar's handler holds
/// notes: a handler of nothing but metadata is joined to the one after it,
/// and a tune of one measure or none is one handler.
pub fn merge_plan(has_notes: &[bool]) -> Vec<Merge> {
    let sounding = has_notes.iter().filter(|holds| **holds).count();
    if sounding <= 1 {
        return vec![Merge::Whole];
    }
    let mut plan = Vec::new();
    let mut index = 0;
    while index < has_notes.len() {
        if !has_notes[index] && index != has_notes.len() - 1 {
            plan.push(Merge::Pair(index));
            index += 2;
        } else {
            plan.push(Merge::Kept(index));
            index += 1;
        }
    }
    plan
}

/// music21's `ABCFile.extractReferenceNumber`: the lines of one tune, and
/// how many lines of the text stand before them.
pub fn extract_reference_number(text: &str, number: IntegerType) -> Result<(usize, String)> {
    let mut collected = Vec::new();
    let mut gathering = false;
    let mut before = 0;
    for line in text.split('\n') {
        let starts = line.trim().starts_with("X:");
        if starts && !gathering {
            let named = line.replace(' ', "");
            let named = named.trim_end().trim_start_matches("X:");
            if named.parse::<IntegerType>() == Ok(number) {
                gathering = true;
            }
        } else if starts && gathering {
            break;
        }
        if gathering {
            collected.push(line);
        } else if collected.is_empty() {
            before += 1;
        }
    }
    if collected.is_empty() {
        return Err(abc_error(format!(
            "cannot find requested reference number in source file: {number}"
        ))
        .of_kind(FormatErrorKind::NotFound));
    }
    Ok((before, collected.join("\n")))
}

// ----------------------------------------------------------------- handler

/// A spanner as the handler makes one: what kind, and the notes it joins.
#[derive(Clone, Debug)]
pub struct Spanning {
    /// A slur, crescendo or diminuendo.
    pub kind: SpannerKind,
    pub(super) uids: Vec<usize>,
    /// The notes it took that no stream holds, by their uid: a grace
    /// note's own, which music21 copies as the grace it places.
    pub(super) strays: Vec<(usize, StreamElement)>,
    /// Where its part had got to when it opened, which is where music21
    /// appends it to the part.
    pub(super) offset: FloatType,
}

impl Spanning {
    /// A spanner of `kind` joining nothing yet.
    pub fn new(kind: SpannerKind) -> Self {
        Self {
            kind,
            uids: Vec::new(),
            strays: Vec::new(),
            offset: 0.0,
        }
    }
}

/// A measure of [`Handler::split_by_measure`]'s, by token index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeasureSplit {
    /// The tokens inside the measure.
    pub tokens: Range<usize>,
    /// The bar opening it.
    pub left: Option<usize>,
    /// The bar closing it.
    pub right: Option<usize>,
}

/// music21's `ABCHandler`: the tokens of a text and what reading them keeps.
#[derive(Clone, Debug, Default)]
pub struct Handler {
    /// The ABC version read as: `(1, 3, 0)` and the like.
    pub version: (u32, u32, u32),
    /// The `%%key value` directives read, in order.
    pub directives: Vec<(String, String)>,
    /// The tokens, in the order of the text.
    pub tokens: Vec<Token>,
    /// The spanners still open after [`Handler::process`], by where they
    /// stand among those it handed back.
    pub active: Vec<usize>,
}

impl Handler {
    /// A handler of no tokens, reading as ABC `version`.
    pub fn new(version: (u32, u32, u32)) -> Self {
        Self {
            version,
            ..Self::default()
        }
    }

    /// music21's `_accidentalPropagation`: `not`, `pitch` or `octave`.
    pub fn accidental_propagation(&self) -> &str {
        if self.version < PROPAGATING_VERSION {
            return "not";
        }
        self.directives
            .iter()
            .rev()
            .find(|(key, _)| key == "propagate-accidentals")
            .map_or("pitch", |(_, value)| value.as_str())
    }

    /// music21's `parseHeaderForVersionInformation`: the version named by
    /// `%abc-2.1` in `text`.
    pub fn read_version(&mut self, text: &str) {
        if let Some(version) = version_in(text) {
            self.version = version;
        }
    }

    /// music21's `barlineTokenFilter`: a bar written as two in one, each
    /// unread.
    pub fn bar_tokens(text: &str) -> Vec<Bar> {
        let pair = match text {
            "::" => Some((":|", "|:")),
            "|1" => Some(("|", "[1")),
            "|2" => Some(("|", "[2")),
            ":|1" => Some((":|", "[1")),
            ":|2" => Some((":|", "[2")),
            _ => None,
        };
        match pair {
            Some((first, second)) => vec![Bar::new(first), Bar::new(second)],
            None => vec![Bar::new(text)],
        }
    }

    /// music21's `tokenize`: the tokens of `source` added to the handler's,
    /// what goes wrong placed at its line and column.
    pub fn tokenize(&mut self, source: &str) -> Result<()> {
        self.read_tokens(source).map_err(|(at, error)| {
            let byte = source
                .char_indices()
                .nth(at)
                .map_or(source.len(), |(byte, _)| byte);
            error.placed(|| Position::in_text(source, byte))
        })
    }

    /// The tokens of `source`, or what went wrong and the character it
    /// went wrong at.
    fn read_tokens(&mut self, source: &str) -> std::result::Result<(), (usize, Error)> {
        let src: Vec<char> = source.chars().collect();
        let len = src.len();
        // Where each line starts, to tell a token its line.
        let line_starts: Vec<usize> = std::iter::once(0)
            .chain((0..len).filter(|at| src[*at] == '\n').map(|at| at + 1))
            .collect();
        let line_of = |at: usize| line_starts.partition_point(|start| *start <= at);
        let at = |index: usize| src.get(index).copied();
        let slice =
            |from: usize, to: usize| -> String { src[from.min(len)..to.min(len)].iter().collect() };

        let mut active_chord_symbol = String::new();
        let mut accidentalized: Vec<(String, String)> = Vec::new();
        let mut accidental = String::new();
        let mut abc_pitch = String::new();
        let mut pos = 0usize;
        while pos < len {
            let c = src[pos];
            let next = at(pos + 1);
            let next_next = at(pos + 2);
            // What is taken past this character.
            let mut skip = 0usize;

            // A comment, or a directive, to the end of its line.
            if c == '%' {
                let comment = comment_at(&src, pos);
                if let Some(directive) = comment.directive {
                    self.directives.push(directive);
                }
                pos += comment.skip + 1;
                continue;
            }

            if starts_metadata(c, next, next_next) {
                let end = next_line_break(&src, pos);
                let mut line = AbcMetadata::new(slice(pos, end).trim());
                line.line = line_of(pos);
                self.tokens.push(Token::Metadata(line));
                pos = end;
                continue;
            }

            if !c.is_whitespace() && !c.is_alphanumeric() && c != '~' && c != '(' {
                let mut matched = None;
                for (text, _) in BARS {
                    let wanted: Vec<char> = text.chars().collect();
                    let fits = match wanted.len() {
                        3 => next.is_some() && next_next.is_some() && slice(pos, pos + 3) == text,
                        2 => next.is_some() && slice(pos, pos + 2) == text,
                        _ => c == wanted[0],
                    };
                    if fits {
                        matched = Some(wanted.len());
                        break;
                    }
                }
                if let Some(length) = matched {
                    accidentalized.clear();
                    accidental.clear();
                    for bar in Self::bar_tokens(&slice(pos, pos + length)) {
                        self.tokens.push(Token::Bar(bar));
                    }
                    pos += length;
                    continue;
                }
            }

            if c == '(' && next.is_some_and(|c| c.is_ascii_digit()) {
                let mut j = pos + 2;
                if j >= len {
                    return Err((
                        pos,
                        abc_error(format!("bad index value {j}, max is {}", len - 1)),
                    ));
                }
                if at(j) == Some(':') {
                    j += 1;
                    if at(j).is_some_and(|c| c.is_ascii_digit()) {
                        j += 1;
                    }
                    if at(j) == Some(':') {
                        j += 1;
                        if at(j).is_some_and(|c| c.is_ascii_digit()) {
                            j += 1;
                        }
                    }
                }
                let mut tuplet = AbcTuplet::new(&slice(pos, j));
                tuplet.line = line_of(pos);
                self.tokens.push(Token::Tuplet(tuplet));
                pos = j;
                continue;
            }

            if c == '<' || c == '>' {
                let mut j = pos + 1;
                while j + 1 < len && matches!(src[j], '<' | '>') {
                    j += 1;
                }
                self.tokens.push(Token::BrokenRhythm(BrokenRhythm {
                    src: slice(pos, j),
                    data: String::new(),
                }));
                pos = j;
                continue;
            }

            if c == '!' {
                let mut j = pos + 1;
                while j < pos + 20 && j < len {
                    if src[j] == '!' {
                        match slice(pos, j + 1).as_str() {
                            "!crescendo(!" => self
                                .tokens
                                .push(Token::SpannerStart(SpannerKind::Crescendo, None)),
                            "!diminuendo(!" => self
                                .tokens
                                .push(Token::SpannerStart(SpannerKind::Diminuendo, None)),
                            "!crescendo)!" | "!diminuendo)!" => {
                                self.tokens.push(Token::ParenStop(Closes::Hairpin));
                            }
                            _ => {}
                        }
                        skip = j - pos;
                        break;
                    }
                    j += 1;
                }
                pos += skip + 1;
                continue;
            }

            if c == '(' && next.is_some() {
                self.tokens
                    .push(Token::SpannerStart(SpannerKind::Slur, None));
                pos += 1;
                continue;
            }
            let simple = match c {
                ')' => Some(Token::ParenStop(Closes::Slur)),
                '-' => Some(Token::Tie),
                _ => None,
            };
            if let Some(token) = simple {
                self.tokens.push(token);
                pos += 1;
                continue;
            }

            if c == '"' {
                let mut j = pos + 1;
                while j + 1 < len && src[j] != '"' {
                    j += 1;
                }
                j += 1;
                active_chord_symbol.push_str(&slice(pos, j));
                pos = j.max(pos + 1);
                continue;
            }

            if c == '[' {
                let mut j = pos + 1;
                while j + 1 < len && src[j] != ']' {
                    j += 1;
                }
                j += 1;
                while j < len && (src[j].is_ascii_digit() || src[j] == '/') {
                    j += 1;
                }
                let collected = format!("{}{}", active_chord_symbol, slice(pos, j));
                active_chord_symbol.clear();
                let mut chord = AbcNote::chord(&collected);
                chord.line = line_of(pos);
                self.tokens.push(Token::Note(Box::new(chord)));
                pos = j.max(pos + 1);
                continue;
            }

            let mark = match c {
                '.' => Some(Token::Staccato),
                'u' => Some(Token::Upbow),
                '{' => Some(Token::GraceStart),
                '}' => Some(Token::GraceStop),
                'v' => Some(Token::Downbow),
                'K' => Some(Token::Accent),
                'k' => Some(Token::Straccent),
                'M' => Some(Token::Tenuto),
                _ => None,
            };
            if let Some(token) = mark {
                self.tokens.push(token);
                pos += 1;
                continue;
            }

            if c.is_alphabetic() || "~^=_".contains(c) {
                let mut found = c.is_alphabetic() && !DECORATIONS.contains(c);
                if found {
                    abc_pitch = c.to_string();
                }
                if ACCIDENTALS.contains(c) {
                    accidental = c.to_string();
                }
                let mut j = pos + 1;
                while j < len {
                    let here = src[j];
                    if !found && DECORATIONS.contains(here) {
                        j += 1;
                        // music21 looks at the character after the one it
                        // has just passed.
                        match at(j) {
                            Some(after) if ACCIDENTALS.contains(after) => accidental.push(after),
                            Some(_) => {}
                            None => {
                                return Err((pos, abc_error("a decoration ends the text")));
                            }
                        }
                    } else if !found && here.is_alphabetic() && !"~wuvhHLTSN".contains(here) {
                        found = true;
                        abc_pitch = here.to_string();
                        j += 1;
                    } else if here.is_ascii_digit() || ",/'".contains(here) {
                        if here == ',' || here == '\'' {
                            abc_pitch.push(here);
                        }
                        j += 1;
                    } else {
                        break;
                    }
                }
                let collected = format!("{}{}", active_chord_symbol, slice(pos, j));
                active_chord_symbol.clear();

                if !is_passed_over(&collected) {
                    let mut carried = String::new();
                    if !abc_pitch.is_empty() {
                        let class: String = abc_pitch
                            .chars()
                            .take(1)
                            .flat_map(char::to_uppercase)
                            .collect();
                        let key = match self.accidental_propagation() {
                            "octave" => Some(abc_pitch.clone()),
                            "pitch" => Some(class),
                            _ => None,
                        };
                        if !accidental.is_empty() {
                            if let Some(key) = key {
                                accidentalized.retain(|(known, _)| *known != key);
                                accidentalized.push((key, accidental.clone()));
                            }
                            accidental.clear();
                        } else if let Some(key) = key
                            && let Some((_, held)) =
                                accidentalized.iter().find(|(known, _)| *known == key)
                        {
                            carried = held.clone();
                        }
                    }
                    let mut note = AbcNote::new(&collected, &carried);
                    note.line = line_of(pos);
                    self.tokens.push(Token::Note(Box::new(note)));
                }
                pos = j.max(pos + 1);
                continue;
            }

            pos += 1;
        }
        Ok(())
    }

    /// music21's `tokenProcess`: each token pre-parsed, told what is in
    /// force where it stands, and then parsed. Hands back the spanners the
    /// tokens started; those left open are in `active`.
    pub fn process(&mut self) -> Result<Vec<Spanning>> {
        for token in &mut self.tokens {
            match token {
                Token::Metadata(line) => line.pre_parse(),
                Token::BrokenRhythm(mark) => mark.pre_parse(),
                _ => {}
            }
        }

        let mut spanners: Vec<Spanning> = Vec::new();
        let mut active_parens: Vec<SpannerKind> = Vec::new();
        self.active.clear();
        let mut default_length: Option<FloatType> = None;
        let mut key: Option<KeySignature> = None;
        let mut meter: Option<TimeSignature> = None;
        // The tuplet token in force and its tuplet.
        let mut tuplet: Option<(usize, Tuplet)> = None;
        let mut tie_waiting = false;
        let mut marks: Vec<&'static str> = Vec::new();
        let mut in_grace = false;
        let mut last_note: Option<usize> = None;

        for index in 0..self.tokens.len() {
            let previous_is_note = index > 0 && self.tokens[index - 1].is_note();
            let next_is_note = self.tokens.get(index + 1).is_some_and(Token::is_note);
            let here = |error: Error, tokens: &[Token]| at_line(error, tokens[index].line());
            match self.tokens[index].clone() {
                Token::Metadata(line) => {
                    if line.tag == "M" {
                        meter = line.meter().map_err(|e| here(e, &self.tokens))?;
                    }
                    if line.tag == "L" || (line.tag == "M" && default_length.is_none()) {
                        let read = line.default_quarter_length();
                        default_length = Some(read.map_err(|e| here(e, &self.tokens))?);
                    } else if line.tag == "K" {
                        let (sharps, _) =
                            line.key_parameters().map_err(|e| here(e, &self.tokens))?;
                        key = Some(KeySignature::new(sharps));
                    }
                    if line.tag == "X" {
                        active_parens.clear();
                        self.active.clear();
                    }
                }
                Token::BrokenRhythm(mark) => {
                    if previous_is_note && next_is_note {
                        if let Token::Note(note) = &mut self.tokens[index - 1] {
                            note.broken_rhythm = Some((mark.data.clone(), Side::Left));
                        }
                        if let Token::Note(note) = &mut self.tokens[index + 1] {
                            note.broken_rhythm = Some((mark.data, Side::Right));
                        }
                    }
                }
                Token::Tuplet(_) => {
                    let Token::Tuplet(held) = &mut self.tokens[index] else {
                        continue;
                    };
                    let made = held
                        .update_ratio(meter.as_ref())
                        .and_then(|()| held.update_note_count());
                    let made = made.map_err(|e| here(e, &self.tokens))?;
                    tuplet = Some((index, made));
                }
                Token::SpannerStart(kind, _) => {
                    spanners.push(Spanning::new(kind));
                    let made = spanners.len() - 1;
                    self.tokens[index] = Token::SpannerStart(kind, Some(made));
                    self.active.push(made);
                    active_parens.push(kind);
                }
                Token::ParenStop(closes) => {
                    // Slurs and hairpins may overlap, so each stop ends the
                    // latest of its own kind.
                    let closed = |kind: &SpannerKind| match closes {
                        Closes::Slur => *kind == SpannerKind::Slur,
                        Closes::Hairpin => *kind != SpannerKind::Slur,
                    };
                    if let Some(at) = active_parens.iter().rposition(closed) {
                        active_parens.remove(at);
                        self.active.remove(at);
                    }
                }
                Token::Tie => {
                    if let Some(last) = last_note
                        && let Token::Note(note) = &mut self.tokens[last]
                    {
                        note.tie = Some(if note.tie == Some(TieType::Stop) {
                            TieType::Continue
                        } else {
                            TieType::Start
                        });
                    }
                    tie_waiting = true;
                }
                Token::Staccato => marks.push("staccato"),
                Token::Upbow => marks.push("upbow"),
                Token::Downbow => marks.push("downbow"),
                Token::Accent => marks.push("accent"),
                Token::Straccent => marks.push("strongaccent"),
                Token::Tenuto => marks.push("tenuto"),
                Token::GraceStart => in_grace = true,
                Token::GraceStop => in_grace = false,
                Token::Bar(_) => {}
                Token::Note(_) => {
                    let Some(default_length) = default_length else {
                        return Err(here(
                            abc_error(
                                "no active default note length provided for note processing.",
                            ),
                            &self.tokens,
                        ));
                    };
                    // The tuplet's count, run down; nought ends it.
                    let mut in_tuplet = None;
                    if let Some((at, made)) = tuplet
                        && let Token::Tuplet(held) = &mut self.tokens[at]
                    {
                        if held.note_count == 0 {
                            tuplet = None;
                        } else {
                            held.note_count -= 1;
                            in_tuplet = Some(made);
                        }
                    }
                    let Token::Note(note) = &mut self.tokens[index] else {
                        continue;
                    };
                    note.default_quarter_length = Some(default_length);
                    note.key = key.clone();
                    note.spanners = self.active.clone();
                    if std::mem::take(&mut tie_waiting) {
                        note.tie = Some(TieType::Stop);
                    }
                    // Each kind of mark once, in music21's order.
                    for mark in ARTICULATIONS {
                        if marks.contains(&mark) {
                            note.articulations.push(mark.to_string());
                        }
                    }
                    marks.clear();
                    if in_grace {
                        note.in_grace = true;
                    }
                    if in_tuplet.is_some() {
                        note.tuplet = in_tuplet;
                    }
                    last_note = Some(index);
                }
            }
        }

        // The last pass: bars and notes read.
        for token in &mut self.tokens {
            match token {
                Token::Bar(bar) => bar.parse(),
                Token::Note(note) => {
                    let line = Some(note.line);
                    note.parse(None, None)
                        .map_err(|error| at_line(error, line))?;
                }
                _ => {}
            }
        }
        Ok(spanners)
    }

    fn reference_lines(&self) -> impl Iterator<Item = &str> {
        self.tokens
            .iter()
            .filter_map(Token::metadata)
            .filter(|(tag, _)| *tag == "X")
            .map(|(_, data)| data)
    }

    /// music21's `definesReferenceNumbers`: more than one `X:`.
    pub fn defines_reference_numbers(&self) -> bool {
        self.reference_lines().count() > 1
    }

    /// music21's `getReferenceNumber`: the first `X:` line's data.
    pub fn reference_number(&self) -> Option<&str> {
        self.reference_lines().next()
    }

    /// music21's `getTitle`: the first `T:` line's data.
    pub fn title(&self) -> Option<&str> {
        self.tokens
            .iter()
            .filter_map(Token::metadata)
            .find(|(tag, _)| *tag == "T")
            .map(|(_, data)| data)
    }

    /// music21's `splitByReferenceNumber`: each tune's tokens by index, the
    /// tokens before the first `X:` in front of each, keyed by number in
    /// the order the numbers first stand. A number met again takes the
    /// later tune. With no `X:` at all, the one tune has no number.
    pub fn split_by_reference_number(&self) -> Result<Vec<(Option<IntegerType>, Vec<usize>)>> {
        let mut before: Vec<usize> = Vec::new();
        let mut tunes: Vec<(Option<IntegerType>, Vec<usize>)> = Vec::new();
        let mut current: Option<usize> = None;
        for (index, token) in self.tokens.iter().enumerate() {
            if let Some(("X", data)) = token.metadata() {
                let number = data.trim().parse::<IntegerType>().map_err(|_| {
                    let refused = abc_error(format!("the reference number {data:?} is no number"));
                    at_line(refused, token.line())
                })?;
                current = match tunes.iter().position(|(known, _)| *known == Some(number)) {
                    Some(place) => {
                        tunes[place].1.clear();
                        Some(place)
                    }
                    None => {
                        tunes.push((Some(number), Vec::new()));
                        Some(tunes.len() - 1)
                    }
                };
            }
            match current {
                Some(place) => tunes[place].1.push(index),
                None => before.push(index),
            }
        }
        if tunes.is_empty() {
            tunes.push((None, Vec::new()));
        }
        for (_, indices) in &mut tunes {
            let mut joined = before.clone();
            joined.append(indices);
            *indices = joined;
        }
        Ok(tunes)
    }

    /// music21's `definesMeasures`: two plain barlines or more.
    pub fn defines_measures(&self) -> bool {
        self.tokens
            .iter()
            .filter(|token| matches!(token, Token::Bar(bar) if bar.is_regular()))
            .count()
            >= 2
    }

    /// music21's `hasNotes`.
    pub fn has_notes(&self) -> bool {
        has_notes(&self.tokens)
    }

    /// music21's `splitByVoice`: the header, and the tokens of each voice
    /// whose `V:` line starts with a digit; all the tokens where there are
    /// fewer than two voices.
    pub fn split_by_voice(&self) -> Vec<Range<usize>> {
        let positions: Vec<usize> = self
            .tokens
            .iter()
            .enumerate()
            .filter(|(_, token)| {
                token.metadata().is_some_and(|(tag, data)| {
                    tag == "V" && data.chars().next().is_some_and(|c| c.is_ascii_digit())
                })
            })
            .map(|(index, _)| index)
            .collect();
        if positions.len() <= 1 {
            return std::iter::once(0..self.tokens.len()).collect();
        }
        let mut pieces = Vec::with_capacity(positions.len() + 1);
        pieces.push(0..positions[0]);
        for pair in positions.windows(2) {
            pieces.push(pair[0]..pair[1]);
        }
        pieces.push(positions[positions.len() - 1]..self.tokens.len());
        pieces
    }

    /// music21's `tokensToBarIndices`: where each bar stands, and each line
    /// of metadata a note follows.
    pub fn bar_indices(&self) -> Vec<usize> {
        let tokens = &self.tokens;
        (0..tokens.len())
            .filter(|index| {
                let note_follows = tokens.get(index + 1).is_some_and(Token::is_note);
                match &tokens[*index] {
                    Token::Bar(_) => true,
                    Token::Metadata(_) => note_follows,
                    _ => false,
                }
            })
            .collect()
    }

    /// music21's `splitByMeasure`: the tokens of each measure and the bars
    /// either side, a repeat's end kept for the measure before it and a
    /// repeat's start for the one after.
    pub fn split_by_measure(&self) -> Vec<MeasureSplit> {
        let tokens = &self.tokens;
        if tokens.is_empty() {
            return Vec::new();
        }
        let mut splits = Vec::new();
        for (x, y) in measure_boundaries(&self.bar_indices(), tokens.len() - 1) {
            let mut split = MeasureSplit {
                tokens: 0..0,
                left: None,
                right: None,
            };
            let mut from = x;
            let mut to = y as isize;
            match &tokens[x] {
                Token::Bar(bar) => {
                    if !(bar.is_repeat() && bar.repeat_form == "end") {
                        split.left = Some(x);
                    }
                    from = x + 1;
                }
                Token::Metadata(_) if x != 0 => from = x + 1,
                _ => {}
            }
            match &tokens[y] {
                Token::Bar(bar) => {
                    if !(bar.is_repeat() && bar.repeat_form == "start") {
                        split.right = Some(y);
                    }
                    to = y as isize - 1;
                }
                Token::Metadata(_) => {}
                token => {
                    if !(token.is_note() && y == tokens.len() - 1) {
                        to = y as isize - 1;
                    }
                }
            }
            let end = (to + 1).max(0) as usize;
            if from >= end {
                continue;
            }
            split.tokens = from..end;
            splits.push(split);
        }
        splits
    }
}

/// Whether any of the tokens is a note, rest or chord.
pub(super) fn has_notes(tokens: &[Token]) -> bool {
    tokens.iter().any(Token::is_note)
}

/// Whether music21 drops what it collected as a note: decorations it does
/// not read, and marks it reads as errors.
fn is_passed_over(collected: &str) -> bool {
    const PASSED: [&str; 38] = [
        "w", "u", "v", "v.", "h", "H", "vk", "uk", "U", "~", ".", "=", "V", "v.", "S", "s", "i",
        "I", "ui", "u.", "Q", "Hy", "Hx", "r", "m", "M", "n", "N", "o", "O", "P", "l", "L", "R",
        "y", "T", "t", "x",
    ];
    let chars: Vec<char> = collected.chars().collect();
    let last = chars.last().copied().unwrap_or(' ');
    PASSED.contains(&collected)
        || collected == "Z"
        || (collected.starts_with('"')
            && ("uvkKQ.yTwhx".contains(last) || collected.ends_with("v.")))
        || collected.starts_with('x')
        || collected.starts_with('H')
        || collected.starts_with('Z')
        || (chars.len() > 1 && chars[0] == '=' && chars[1].is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens_of(text: &str) -> Vec<Token> {
        let mut handler = Handler::new(DEFAULT_VERSION);
        handler.tokenize(text).unwrap();
        handler.tokens
    }

    fn processed(text: &str) -> Handler {
        let mut handler = Handler::new(DEFAULT_VERSION);
        handler.tokenize(text).unwrap();
        handler.process().unwrap();
        handler
    }

    #[test]
    fn a_bar_says_what_it_is() {
        // music21's `ABCBar.parse` docstring.
        let bar = Bar::parsed("|");
        assert!(bar.is_regular() && !bar.is_repeat());
        assert_eq!(bar.bar_type, "barline");
        let end = Bar::parsed(":|");
        assert!(end.is_repeat() && end.repeat_form == "end" && end.bar_style == "light-heavy");
        assert_eq!(Bar::parsed("[1").repeat_bracket(), Some(1));
        assert_eq!(
            Bar::parsed("||").barline().unwrap().bar_type(),
            BarlineType::Double
        );
        // Unread, or no bar at all, it is nothing.
        assert_eq!(Bar::new("|").bar_type, "");
        assert_eq!(Bar::parsed("hi").bar_type, "");
    }

    #[test]
    fn text_is_cut_into_tokens_as_music21_cuts_it() {
        let tokens = tokens_of("M:6/8\nL:1/8\nK:G\nB3 A3 | (3GAB c2 :|\n");
        let kinds: Vec<&str> = tokens
            .iter()
            .map(|token| match token {
                Token::Metadata(_) => "meta",
                Token::Bar(_) => "bar",
                Token::Tuplet(_) => "tuplet",
                Token::Note(_) => "note",
                _ => "other",
            })
            .collect();
        assert_eq!(
            kinds,
            [
                "meta", "meta", "meta", "note", "note", "bar", "tuplet", "note", "note", "note",
                "note", "bar"
            ]
        );
        // A token keeps its text, and metadata is unread until processed.
        assert_eq!(tokens[0].src(), "M:6/8");
        assert_eq!(tokens[0].metadata(), Some(("", "")));
        let srcs: Vec<String> = tokens_of("(6::2f").iter().map(Token::src).collect();
        assert_eq!(srcs, ["(6::2", "f"]);
    }

    #[test]
    fn a_comment_sets_a_directive_and_runs_to_the_line_end() {
        // music21's `processComment` docstring.
        let chars: Vec<char> = "\nHello % this is a comment\nBye\n".chars().collect();
        assert_eq!(comment_at(&chars, 6).skip, 19);
        let chars: Vec<char> = "%%abc-hello world".chars().collect();
        assert_eq!(
            comment_at(&chars, 0).directive,
            Some(("abc-hello".to_string(), "world".to_string()))
        );
        assert_eq!(version_in("x %abc-2.3.2"), Some((2, 3, 2)));
        assert_eq!(version_in("%abc-1.7"), Some((1, 7, 0)));
        assert_eq!(version_in("%abc-x %abc-2.1"), Some((2, 1, 0)));
    }

    #[test]
    fn a_tuplet_counts_its_notes_down() {
        let handler = processed("L:1/8\n(3abcd\n");
        let Token::Tuplet(tuplet) = &handler.tokens[1] else {
            panic!("no tuplet");
        };
        assert_eq!((tuplet.actual, tuplet.normal, tuplet.note_count), (3, 2, 0));
        let in_tuplet: Vec<bool> = handler
            .tokens
            .iter()
            .filter_map(|token| match token {
                Token::Note(note) => Some(note.tuplet.is_some()),
                _ => None,
            })
            .collect();
        assert_eq!(in_tuplet, [true, true, true, false]);
        let mut bad = AbcTuplet::new("(10");
        let error = bad.update_ratio(None).unwrap_err();
        assert_eq!(
            error.to_string(),
            "ABC error: cannot handle tuplet of form: '(10'"
        );
    }

    #[test]
    fn a_handler_is_cut_by_tune_voice_and_measure() {
        // music21's `splitByReferenceNumber` docstring.
        let text = "X:5\nM:6/8\nL:1/8\nK:G\nB3 A3 | G6 | B3 A3 | G6 ||\
                    X:6\nM:6/8\nL:1/8\nK:G\nB3 A3 | G6 | B3 A3 | G6 ||";
        let handler = processed(&format!("O: Irish\n{text}"));
        let tunes = handler.split_by_reference_number().unwrap();
        let numbers: Vec<_> = tunes.iter().map(|(number, _)| *number).collect();
        assert_eq!(numbers, [Some(5), Some(6)]);
        assert_eq!(tunes[0].1.len(), 15);
        assert_eq!(tunes[1].1[0], 0);
        assert_eq!(handler.reference_number(), Some("5"));

        let voices = processed("M:6/8\nL:1/8\nK:G\nV:1\nB3 A3 | G6 ||\nV:2\nD3 D3 | D6 ||");
        assert_eq!(voices.split_by_voice(), [0..3, 3..9, 9..15]);

        let tune = processed("M:6/8\nL:1/8\nK:G\nc1D2");
        let splits = tune.split_by_measure();
        assert_eq!(splits.len(), 2);
        assert_eq!(splits[1].tokens, 3..5);
        let has: Vec<bool> = splits
            .iter()
            .map(|split| has_notes(&tune.tokens[split.tokens.clone()]))
            .collect();
        assert_eq!(merge_plan(&has), [Merge::Whole]);
        assert_eq!(
            merge_plan(&[false, true, true, false]),
            [Merge::Pair(0), Merge::Kept(2), Merge::Kept(3)]
        );
    }

    /// Each note's source and the kinds of the spanners in force on it.
    fn spanners_of(body: &str) -> Vec<(String, Vec<SpannerKind>)> {
        let mut handler = Handler::new(DEFAULT_VERSION);
        handler
            .tokenize(&format!("X:1\nM:4/4\nL:1/8\nK:C\n{body}\n"))
            .unwrap();
        let spanners = handler.process().unwrap();
        handler
            .tokens
            .iter()
            .filter_map(|token| match token {
                Token::Note(note) => Some((
                    note.src.trim().to_string(),
                    note.spanners
                        .iter()
                        .map(|index| spanners[*index].kind)
                        .collect(),
                )),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_closing_mark_ends_the_latest_spanner_of_its_kind() {
        // music21's `testSlurAroundTuplet` (cuthbertLab #1798).
        use SpannerKind::{Crescendo, Diminuendo, Slur};
        let expect = |pairs: &[(&str, &[SpannerKind])]| -> Vec<(String, Vec<SpannerKind>)> {
            pairs
                .iter()
                .map(|(src, kinds)| (src.to_string(), kinds.to_vec()))
                .collect()
        };
        // A tuplet has no closing parenthesis, so ')' ends the slur.
        assert_eq!(
            spanners_of("(a(3bcd e) f|"),
            expect(&[
                ("a", &[Slur]),
                ("b", &[Slur]),
                ("c", &[Slur]),
                ("d", &[Slur]),
                ("e", &[Slur]),
                ("f", &[]),
            ])
        );
        assert_eq!(
            spanners_of("!crescendo(! (3abc d !crescendo)! e|"),
            expect(&[
                ("a", &[Crescendo]),
                ("b", &[Crescendo]),
                ("c", &[Crescendo]),
                ("d", &[Crescendo]),
                ("e", &[]),
            ])
        );
        // A slur and a crescendo can overlap without nesting.
        assert_eq!(
            spanners_of("(a !crescendo(! b) c !crescendo)! d|"),
            expect(&[
                ("a", &[Slur]),
                ("b", &[Slur, Crescendo]),
                ("c", &[Crescendo]),
                ("d", &[]),
            ])
        );
        assert_eq!(
            spanners_of("!diminuendo(! a (b !diminuendo)! c) d|"),
            expect(&[
                ("a", &[Diminuendo]),
                ("b", &[Diminuendo, Slur]),
                ("c", &[Slur]),
                ("d", &[]),
            ])
        );
    }

    #[test]
    fn a_key_line_gives_its_sharps_and_mode() {
        // music21's `getKeySignatureParameters` docstring.
        assert_eq!(key_parameters("Eb").unwrap(), (-3, Some("major")));
        assert_eq!(
            key_parameters("A mixolydian").unwrap(),
            (2, Some("mixolydian"))
        );
        assert_eq!(key_parameters("F#m").unwrap(), (3, Some("minor")));
        assert_eq!(key_parameters("HP").unwrap(), (0, None));
        assert_eq!(key_parameters("Hp").unwrap(), (2, None));
        // Only a key line has a key.
        assert!(AbcMetadata::new("Q:40").key_parameters().is_err());
    }

    #[test]
    fn a_length_is_read_against_the_default() {
        // music21's `getQuarterLength` docstring.
        let mut note = AbcNote::new("", "");
        note.default_quarter_length = Some(0.5);
        assert_eq!(note.quarter_length_of("f", None).unwrap(), 0.5);
        assert_eq!(note.quarter_length_of("f2", None).unwrap(), 1.0);
        assert_eq!(note.quarter_length_of("f/", None).unwrap(), 0.25);
        assert_eq!(note.quarter_length_of("f3/2", None).unwrap(), 0.75);
        note.broken_rhythm = Some((">".to_string(), Side::Left));
        assert_eq!(note.quarter_length_of("A", None).unwrap(), 0.75);
    }

    #[test]
    fn a_pitch_takes_its_accidental_from_the_key() {
        let mut note = AbcNote::new("f", "");
        note.key = Some(KeySignature::new(1));
        assert_eq!(
            note.pitch_name("f", None).unwrap(),
            (Some("F#5".to_string()), Some(false))
        );
        assert_eq!(
            note.pitch_name("=f", None).unwrap(),
            (Some("Fn5".to_string()), Some(true))
        );
        assert_eq!(note.pitch_name("z", None).unwrap(), (None, None));
        assert_eq!(
            note.pitch_name("C,,", None).unwrap(),
            (Some("C2".to_string()), Some(false))
        );
        assert_eq!(
            note.pitch_name("x", None).unwrap_err().to_string(),
            "ABC error: cannot find any pitch information in: 'x'"
        );
    }
}
