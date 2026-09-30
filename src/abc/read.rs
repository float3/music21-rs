//! Reading ABC: music21's `abcFormat`, its tokens and handlers, and
//! `abcFormat.translate`, which turns them into a score.
//!
//! The text is cut into tokens one character at a time, each token is told
//! what is in force where it stands -- the default length, the key, a tuplet,
//! a tie -- and the tokens of each voice are then read off measure by
//! measure. The order and the oddities are music21's throughout, since the
//! score it makes is what this one is held to.

use crate::articulations::{Articulation, ArticulationKind};
use crate::bar::{Barline, BarlineType, Ending, RepeatDirection};
use crate::chord::Chord;
use crate::chordsymbol::ChordSymbol;
use crate::clef::Clef;
use crate::defaults::{FloatType, IntegerType};
use crate::duration::{Duration, Tuplet};
use crate::error::{Error, Result};
use crate::interval::Interval;
use crate::key::KeySignature;
use crate::makenotation::split_element;
use crate::metadata::Metadata;
use crate::meter::TimeSignature;
use crate::notation::{Tie, TieStyle, TieType};
use crate::note::Note;
use crate::pitch::Pitch;
use crate::rest::Rest;
use crate::spanner::{Spanner, SpannerKind};
use crate::stream::{Stream, StreamElement, StreamEvent, StreamKind};
use crate::tempo::MetronomeMark;

fn abc_error(message: impl Into<String>) -> Error {
    Error::Abc(message.into())
}

/// The ABC version a file is read as where it names none: music21's
/// `defaults.abcVersionDefault`.
const DEFAULT_VERSION: (u32, u32, u32) = (1, 3, 0);

/// Every bar token, in the order music21 tries them.
const BARS: [(&str, &str); 14] = [
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

/// music21's `opFrac`, near enough for lengths: the nearest fraction with a
/// small denominator where the float is within a hair of one.
fn op_frac(value: FloatType) -> FloatType {
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

// ------------------------------------------------------------------ tokens

/// music21's `ABCBar`.
#[derive(Clone, Debug, Default, PartialEq)]
struct Bar {
    src: String,
    /// `repeat` or `barline`, nothing where the text is no bar at all.
    repeat: bool,
    style: String,
    /// `end`, `start`, `first` or `second`.
    form: String,
}

impl Bar {
    fn new(src: &str) -> Self {
        let mut bar = Self {
            src: src.to_string(),
            ..Self::default()
        };
        let Some((_, described)) = BARS.iter().find(|(text, _)| *text == src.trim()) else {
            return bar;
        };
        let parts: Vec<&str> = described.split('-').collect();
        bar.repeat = parts.contains(&"repeat");
        if parts.len() == 1 {
            bar.style = parts[0].to_string();
        } else if parts.contains(&"first") {
            bar.style = "regular".to_string();
            bar.form = "first".to_string();
        } else if parts.contains(&"second") {
            bar.style = "regular".to_string();
            bar.form = "second".to_string();
        } else {
            bar.style = format!("{}-{}", parts[0], parts[1]);
        }
        if parts.len() > 2 {
            bar.form = parts[3].to_string();
        }
        bar
    }

    fn is_regular(&self) -> bool {
        !self.repeat && self.style == "regular"
    }

    /// Which ending the bar opens, where it opens one.
    fn repeat_bracket(&self) -> Option<u32> {
        match self.form.as_str() {
            "first" => Some(1),
            "second" => Some(2),
            _ => None,
        }
    }

    /// music21's `getBarObject`.
    fn barline(&self) -> Option<Barline> {
        if self.repeat {
            return match self.form.as_str() {
                "start" => Some(Barline::repeat(RepeatDirection::Start, None)),
                "end" => Some(Barline::repeat(RepeatDirection::End, None)),
                _ => None,
            };
        }
        if self.style == "regular" || self.style.is_empty() || self.repeat_bracket().is_some() {
            return None;
        }
        BarlineType::from_name(&self.style).ok().map(Barline::new)
    }
}

/// music21's `ABCNote`, and `ABCChord` where it holds notes of its own.
#[derive(Clone, Debug, Default)]
struct AbcNote {
    src: String,
    is_chord: bool,
    carried_accidental: String,
    chord_symbols: Vec<String>,
    in_grace: bool,
    default_quarter_length: Option<FloatType>,
    /// The mark and whether this note stands to its left.
    broken_rhythm: Option<(String, bool)>,
    key: Option<KeySignature>,
    tuplet: Option<Tuplet>,
    /// The spanners in force, by where they stand among the handler's.
    spanners: Vec<usize>,
    tie: Option<TieType>,
    articulations: Vec<&'static str>,
    accidental_display: Option<bool>,
    is_rest: bool,
    pitch_name: Option<String>,
    quarter_length: FloatType,
    sub_tokens: Vec<AbcNote>,
}

#[derive(Clone, Debug)]
enum Token {
    Metadata {
        tag: String,
        data: String,
    },
    Bar(Bar),
    Tuplet {
        src: String,
    },
    Tie,
    /// A slur or a hairpin opening: the spanner it starts.
    SpannerStart(SpannerKind, Option<usize>),
    ParenStop,
    Staccato,
    Upbow,
    Downbow,
    Accent,
    Straccent,
    Tenuto,
    GraceStart,
    GraceStop,
    BrokenRhythm(String),
    Note(Box<AbcNote>),
}

impl Token {
    fn is_note(&self) -> bool {
        matches!(self, Self::Note(_))
    }

    fn metadata(&self) -> Option<(&str, &str)> {
        match self {
            Self::Metadata { tag, data } => Some((tag, data)),
            _ => None,
        }
    }
}

/// music21's `ABCMetadata.preParse`: the tag before the colon and what
/// follows it, a comment taken off.
fn metadata_token(src: &str) -> Token {
    let chars: Vec<char> = src.chars().collect();
    let is_tag =
        chars.len() >= 2 && (chars[0].is_ascii_uppercase() || chars[0] == 'w') && chars[1] == ':';
    if !is_tag {
        return Token::Metadata {
            tag: String::new(),
            data: String::new(),
        };
    }
    let stripped = src.split('%').next().unwrap_or("");
    let mut pieces = stripped.chars();
    let tag: String = pieces.by_ref().take(1).collect();
    let data: String = pieces.skip(1).collect();
    Token::Metadata {
        tag,
        data: data.trim().to_string(),
    }
}

/// music21's `getTimeSignatureParameters`.
fn meter_parameters(data: &str) -> Result<Option<(u32, u32)>> {
    if data.to_lowercase() == "none" {
        return Ok(None);
    }
    match data {
        "C" => return Ok(Some((4, 4))),
        "C|" => return Ok(Some((2, 2))),
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
    Ok(Some((digits(numerator)?, digits(denominator)?)))
}

fn meter_of(data: &str) -> Result<Option<TimeSignature>> {
    match meter_parameters(data)? {
        Some((numerator, denominator)) => Ok(Some(TimeSignature::new(numerator, denominator)?)),
        None => Ok(None),
    }
}

/// music21's `getKeySignatureParameters`: the signed count of sharps and the
/// mode, where the key names one.
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

/// music21's `getKeySignatureObject`: a key where there is a mode, a
/// signature otherwise.
fn key_of(data: &str) -> Result<StreamElement> {
    let (sharps, mode) = key_parameters(data)?;
    let signature = KeySignature::new(sharps);
    Ok(match mode {
        Some(mode) => match signature.try_as_key(Some(mode), None) {
            Ok(key) => key.into(),
            Err(_) => signature.into(),
        },
        None => signature.into(),
    })
}

/// music21's `getClefObject`: the clef a key line names, and how far the
/// notes under it are moved.
fn clef_of(data: &str) -> Option<(Clef, IntegerType)> {
    let lower = data.to_lowercase();
    if lower.contains("-8va") {
        Clef::from_string("treble8vb", 0)
            .ok()
            .map(|clef| (clef, -12))
    } else if lower.contains("bass") {
        Some((Clef::bass(), -24))
    } else {
        None
    }
}

/// music21's `getMetronomeMarkObject`.
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

/// music21's `getDefaultQuarterLength`, for an `L:` or an `M:` line.
fn default_quarter_length(tag: &str, data: &str) -> Result<FloatType> {
    if tag == "L" && data.contains('/') {
        let (numerator, denominator) = data.split_once('/').unwrap_or(("1", "8"));
        let numerator: FloatType = numerator
            .trim()
            .parse()
            .map_err(|_| abc_error(format!("cannot read the default length {data:?}")))?;
        let denominator: FloatType = if denominator == "G" {
            4.0
        } else {
            denominator
                .trim()
                .parse()
                .map_err(|_| abc_error(format!("cannot read the default length {data:?}")))?
        };
        return Ok(numerator * 4.0 / denominator);
    }
    if tag == "M" {
        return Ok(match meter_parameters(data)? {
            None => 0.5,
            // Under three quarters of a whole note the unit is a sixteenth.
            Some((numerator, denominator))
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

impl AbcNote {
    fn new(src: &str, carried_accidental: &str, is_chord: bool) -> Self {
        Self {
            src: src.to_string(),
            is_chord,
            carried_accidental: carried_accidental.to_string(),
            ..Self::default()
        }
    }

    /// music21's `_splitChordSymbols`: the quoted strings, and what is left
    /// after the last of them.
    fn split_chord_symbols(src: &str) -> (Vec<String>, String) {
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
    /// for a rest, and whether its accidental is shown.
    fn pitch_name(
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
            .ok_or_else(|| abc_error(format!("cannot find any pitch information in: {text:?}")))?;
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

    /// music21's `getQuarterLength`.
    fn read_quarter_length(
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
        if let Some((symbol, left)) = &self.broken_rhythm {
            let (first, second) = match symbol.as_str() {
                ">" => (1.5, 0.5),
                "<" => (0.5, 1.5),
                ">>" => (1.75, 0.25),
                "<<" => (0.25, 1.75),
                ">>>" => (1.875, 0.125),
                "<<<" => (0.125, 1.875),
                _ => (1.0, 1.0),
            };
            length *= if *left { first } else { second };
        }
        Ok(length)
    }

    /// music21's `ABCNote.parse` and `ABCChord.parse`.
    fn parse(
        &mut self,
        forced_default: Option<FloatType>,
        forced_key: Option<&KeySignature>,
    ) -> Result<()> {
        let (symbols, rest) = Self::split_chord_symbols(&self.src);
        self.chord_symbols = symbols;
        if self.is_chord {
            let chars: Vec<char> = rest.chars().collect();
            let close = chars.iter().position(|c| *c == ']').ok_or_else(|| {
                abc_error(format!(
                    "Bad chord indicator: {}: no closing bracket found.",
                    self.src
                ))
            })?;
            let outer: String = chars[close + 1..].iter().collect();
            let inner: String = chars[1..close].iter().collect();
            let outer_length = self.read_quarter_length(&outer, Some(1.0))?;
            let key = forced_key.cloned().or_else(|| self.key.clone());
            let mut handler = Handler::new(DEFAULT_VERSION);
            handler.tokenize(&inner)?;
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
            return Ok(());
        }
        // A note music21 can find no pitch in is read as a C.
        let (name, display) = match self.pitch_name(&rest, forced_key) {
            Ok(read) => read,
            Err(Error::Abc(message)) if message.starts_with("cannot find any pitch") => {
                (Some("C".to_string()), Some(false))
            }
            Err(error) => return Err(error),
        };
        self.is_rest = name.is_none();
        self.pitch_name = name;
        self.accidental_display = display;
        self.quarter_length = self.read_quarter_length(&rest, forced_default)?;
        Ok(())
    }
}

// ----------------------------------------------------------------- handler

/// A spanner as the handler makes one: what kind, and the notes it joins.
#[derive(Clone, Debug)]
struct Spanning {
    kind: SpannerKind,
    uids: Vec<usize>,
}

/// music21's `ABCHandler`.
#[derive(Clone, Debug)]
struct Handler {
    version: (u32, u32, u32),
    directives: Vec<(String, String)>,
    tokens: Vec<Token>,
}

impl Handler {
    fn new(version: (u32, u32, u32)) -> Self {
        Self {
            version,
            directives: Vec::new(),
            tokens: Vec::new(),
        }
    }

    /// music21's `_accidentalPropagation`.
    fn accidental_propagation(&self) -> &str {
        if self.version < (2, 0, 0) {
            return "not";
        }
        self.directives
            .iter()
            .rev()
            .find(|(key, _)| key == "propagate-accidentals")
            .map_or("pitch", |(_, value)| value.as_str())
    }

    /// music21's `parseHeaderForVersionInformation`: `%abc-2.1` in the first
    /// hundred characters.
    fn read_version(&mut self, text: &str) {
        let head: String = text.chars().take(100).collect();
        let Some(at) = head.find("%abc-") else {
            return;
        };
        let rest = &head[at + 5..];
        let mut numbers = rest
            .split(|c: char| !c.is_ascii_digit())
            .take(3)
            .map(|piece| piece.parse::<u32>().ok());
        // `major.minor` at the least.
        let (Some(Some(major)), Some(Some(minor))) = (numbers.next(), numbers.next()) else {
            return;
        };
        if !rest[major.to_string().len()..].starts_with('.') {
            return;
        }
        let patch = numbers.next().flatten().unwrap_or(0);
        self.version = (major, minor, patch);
    }

    /// music21's `barlineTokenFilter`: a bar written as two in one.
    fn bar_tokens(text: &str) -> Vec<Bar> {
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

    /// music21's `tokenize`.
    fn tokenize(&mut self, source: &str) -> Result<()> {
        let src: Vec<char> = source.chars().collect();
        let len = src.len();
        let at = |index: usize| src.get(index).copied();
        let slice =
            |from: usize, to: usize| -> String { src[from.min(len)..to.min(len)].iter().collect() };
        let next_line_break =
            |from: usize| -> usize { (from + 1..len).find(|j| src[*j] == '\n').unwrap_or(len) };
        const DECORATIONS: &str = ".~^=_HLMOPSTuv";
        const ACCIDENTALS: &str = "^=_";

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

            if c == '%' {
                let end = next_line_break(pos);
                let line = slice(pos, end);
                if let Some(directive) = line.strip_prefix("%%") {
                    let key: String = directive
                        .chars()
                        .take_while(|c| c.is_ascii_lowercase() || *c == '-')
                        .collect();
                    let after = &directive[key.len()..];
                    if !key.is_empty()
                        && after.starts_with(char::is_whitespace)
                        && let Some(value) = after.split_whitespace().next()
                    {
                        self.directives.push((key, value.to_string()));
                    }
                }
                pos = end;
                continue;
            }

            let starts_metadata = next == Some(':')
                && next_next.is_some()
                && next_next != Some('|')
                && (c == 'w' || (c.is_alphabetic() && c.is_uppercase()));
            if starts_metadata {
                let end = next_line_break(pos);
                self.tokens.push(metadata_token(slice(pos, end).trim()));
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
                    return Err(abc_error(format!(
                        "bad index value {j}, max is {}",
                        len - 1
                    )));
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
                self.tokens.push(Token::Tuplet { src: slice(pos, j) });
                pos = j;
                continue;
            }

            if c == '<' || c == '>' {
                let mut j = pos + 1;
                while j + 1 < len && matches!(src[j], '<' | '>') {
                    j += 1;
                }
                self.tokens
                    .push(Token::BrokenRhythm(slice(pos, j).trim().to_string()));
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
                            "!crescendo)!" | "!diminuendo)!" => self.tokens.push(Token::ParenStop),
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
                ')' => Some(Token::ParenStop),
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
                self.tokens
                    .push(Token::Note(Box::new(AbcNote::new(&collected, "", true))));
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
                                return Err(abc_error("a decoration ends the text"));
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

                const PASSED: [&str; 38] = [
                    "w", "u", "v", "v.", "h", "H", "vk", "uk", "U", "~", ".", "=", "V", "v.", "S",
                    "s", "i", "I", "ui", "u.", "Q", "Hy", "Hx", "r", "m", "M", "n", "N", "o", "O",
                    "P", "l", "L", "R", "y", "T", "t", "x",
                ];
                let chars: Vec<char> = collected.chars().collect();
                let last = chars.last().copied().unwrap_or(' ');
                let passed = PASSED.contains(&collected.as_str())
                    || collected == "Z"
                    || (collected.starts_with('"')
                        && ("uvkKQ.yTwhx".contains(last) || collected.ends_with("v.")))
                    || collected.starts_with('x')
                    || collected.starts_with('H')
                    || collected.starts_with('Z')
                    || (chars.len() > 1 && chars[0] == '=' && chars[1].is_ascii_digit());
                if !passed {
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
                    self.tokens.push(Token::Note(Box::new(AbcNote::new(
                        &collected, &carried, false,
                    ))));
                }
                pos = j.max(pos + 1);
                continue;
            }

            pos += 1;
        }
        Ok(())
    }

    /// music21's `tokenProcess`: each token told what is in force where it
    /// stands, and then read. Hands back the spanners the tokens started.
    fn process(&mut self) -> Result<Vec<Spanning>> {
        let mut spanners: Vec<Spanning> = Vec::new();
        let mut active_spanners: Vec<usize> = Vec::new();
        let mut active_parens: Vec<&'static str> = Vec::new();
        let mut default_length: Option<FloatType> = None;
        let mut key: Option<KeySignature> = None;
        let mut meter: Option<TimeSignature> = None;
        // The tuplet in force and how many notes it still takes.
        let mut tuplet: Option<(Tuplet, i32)> = None;
        let mut tie_waiting = false;
        let mut marks: Vec<&'static str> = Vec::new();
        let mut in_grace = false;
        let mut last_note: Option<usize> = None;

        for index in 0..self.tokens.len() {
            let previous_is_note = index > 0 && self.tokens[index - 1].is_note();
            let next_is_note = self.tokens.get(index + 1).is_some_and(Token::is_note);
            match self.tokens[index].clone() {
                Token::Metadata { tag, data } => {
                    if tag == "M" {
                        meter = meter_of(&data)?;
                    }
                    if tag == "L" || (tag == "M" && default_length.is_none()) {
                        default_length = Some(default_quarter_length(&tag, &data)?);
                    } else if tag == "K" {
                        let (sharps, _) = key_parameters(&data)?;
                        key = Some(KeySignature::new(sharps));
                    }
                    if tag == "X" {
                        active_parens.clear();
                        active_spanners.clear();
                    }
                }
                Token::BrokenRhythm(symbol) => {
                    if previous_is_note && next_is_note {
                        if let Token::Note(note) = &mut self.tokens[index - 1] {
                            note.broken_rhythm = Some((symbol.clone(), true));
                        }
                        if let Token::Note(note) = &mut self.tokens[index + 1] {
                            note.broken_rhythm = Some((symbol, false));
                        }
                    }
                }
                Token::Tuplet { src } => {
                    let (made, count) = tuplet_of(&src, meter.as_ref())?;
                    tuplet = Some((made, count));
                    active_parens.push("Tuplet");
                }
                Token::SpannerStart(kind, _) => {
                    spanners.push(Spanning {
                        kind,
                        uids: Vec::new(),
                    });
                    let made = spanners.len() - 1;
                    self.tokens[index] = Token::SpannerStart(kind, Some(made));
                    active_spanners.push(made);
                    active_parens.push(match kind {
                        SpannerKind::Crescendo => "Crescendo",
                        SpannerKind::Diminuendo => "Diminuendo",
                        _ => "Slur",
                    });
                }
                Token::ParenStop => {
                    if let Some(closed) = active_parens.pop()
                        && closed != "Tuplet"
                    {
                        active_spanners.pop();
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
                        return Err(abc_error(
                            "no active default note length provided for note processing.",
                        ));
                    };
                    let Token::Note(note) = &mut self.tokens[index] else {
                        continue;
                    };
                    note.default_quarter_length = Some(default_length);
                    note.key = key.clone();
                    note.spanners = active_spanners.clone();
                    if std::mem::take(&mut tie_waiting) {
                        note.tie = Some(TieType::Stop);
                    }
                    // Each kind of mark once, in music21's order.
                    for mark in [
                        "staccato",
                        "upbow",
                        "downbow",
                        "accent",
                        "strongaccent",
                        "tenuto",
                    ] {
                        if marks.contains(&mark) {
                            note.articulations.push(mark);
                        }
                    }
                    marks.clear();
                    if in_grace {
                        note.in_grace = true;
                    }
                    match &mut tuplet {
                        None => {}
                        Some((_, 0)) => tuplet = None,
                        Some((made, count)) => {
                            *count -= 1;
                            note.tuplet = Some(*made);
                        }
                    }
                    last_note = Some(index);
                }
            }
        }
        for token in &mut self.tokens {
            if let Token::Note(note) = token {
                note.parse(None, None)?;
            }
        }
        Ok(spanners)
    }

    /// music21's `definesReferenceNumbers`: more than one `X:`.
    fn defines_reference_numbers(&self) -> bool {
        self.tokens
            .iter()
            .filter(|token| token.metadata().is_some_and(|(tag, _)| tag == "X"))
            .count()
            > 1
    }

    /// music21's `splitByReferenceNumber`, in order of the numbers.
    fn split_by_reference_number(&self) -> Vec<(IntegerType, Handler)> {
        let mut before: Vec<Token> = Vec::new();
        let mut tunes: Vec<(IntegerType, Vec<Token>)> = Vec::new();
        for token in &self.tokens {
            if let Some(("X", data)) = token.metadata() {
                let number = data.trim().parse::<IntegerType>().unwrap_or(0);
                tunes.retain(|(known, _)| *known != number);
                tunes.push((number, Vec::new()));
            }
            match tunes.last_mut() {
                Some((_, tokens)) => tokens.push(token.clone()),
                None => before.push(token.clone()),
            }
        }
        tunes.sort_by_key(|(number, _)| *number);
        tunes
            .into_iter()
            .map(|(number, tokens)| {
                let mut handler = Handler::new(self.version);
                handler.tokens = before.iter().cloned().chain(tokens).collect();
                (number, handler)
            })
            .collect()
    }

    /// music21's `definesMeasures`: two plain barlines or more.
    fn defines_measures(&self) -> bool {
        self.tokens
            .iter()
            .filter(|token| matches!(token, Token::Bar(bar) if bar.is_regular()))
            .count()
            >= 2
    }

    fn has_notes(tokens: &[Token]) -> bool {
        tokens.iter().any(Token::is_note)
    }

    /// music21's `splitByVoice`: the header, and the tokens of each voice
    /// whose `V:` line starts with a digit.
    fn split_by_voice(&self) -> Vec<Vec<Token>> {
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
            return vec![self.tokens.clone()];
        }
        let mut pieces = vec![self.tokens[..positions[0]].to_vec()];
        for pair in positions.windows(2) {
            pieces.push(self.tokens[pair[0]..pair[1]].to_vec());
        }
        pieces.push(self.tokens[positions[positions.len() - 1]..].to_vec());
        pieces
    }
}

/// music21's `ABCTuplet.updateRatio` and `updateNoteCount`: the tuplet and
/// how many notes it takes.
fn tuplet_of(src: &str, meter: Option<&TimeSignature>) -> Result<(Tuplet, i32)> {
    let normal_switch = match meter {
        Some(meter) if meter.beat_division_count() == 3 => 3,
        _ => 2,
    };
    let pieces: Vec<&str> = src.trim().split(':').collect();
    let (actual, normal) = match pieces[0] {
        "(1" => (1, 1),
        "(2" => (2, 3),
        "(3" => (3, 2),
        "(4" => (4, 3),
        "(5" => (5, normal_switch),
        "(6" => (6, 2),
        "(7" => (7, normal_switch),
        "(8" => (8, 3),
        "(9" => (9, normal_switch),
        other => {
            return Err(abc_error(format!(
                "cannot handle tuplet of form: {other:?}"
            )));
        }
    };
    let read = |piece: Option<&&str>| piece.and_then(|piece| piece.parse::<u32>().ok());
    let normal = read(pieces.get(1)).unwrap_or(normal);
    let count = read(pieces.get(2)).unwrap_or(actual);
    Ok((Tuplet::ratio(actual, normal), count as i32))
}

/// A bar's worth of tokens with the barlines either side: music21's
/// `ABCHandlerBar`.
#[derive(Clone, Debug, Default)]
struct BarHandler {
    tokens: Vec<Token>,
    left: Option<Bar>,
    right: Option<Bar>,
}

impl BarHandler {
    /// music21's `ABCHandlerBar.__add__`.
    fn joined(&self, other: &BarHandler) -> BarHandler {
        BarHandler {
            tokens: self
                .tokens
                .iter()
                .cloned()
                .chain(other.tokens.iter().cloned())
                .collect(),
            left: other.left.clone().or_else(|| self.left.clone()),
            right: other.right.clone().or_else(|| self.right.clone()),
        }
    }
}

/// music21's `splitByMeasure`.
fn split_by_measure(tokens: &[Token]) -> Vec<BarHandler> {
    // Where a bar stands, or a line of metadata a note follows.
    let mut positions: Vec<usize> = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let note_follows = tokens.get(index + 1).is_some_and(Token::is_note);
        if matches!(token, Token::Bar(_)) || (token.metadata().is_some() && note_follows) {
            positions.push(index);
        }
    }
    if positions.is_empty() || tokens.is_empty() {
        return Vec::new();
    }
    let last_valid = tokens.len() - 1;
    let mut pairs: Vec<(usize, usize)> = vec![(0, positions[0])];
    let mut start = positions[0];
    for end in positions.iter().skip(1).copied() {
        // A span of one is passed over.
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

    let mut handlers = Vec::new();
    for (x, y) in pairs {
        let mut handler = BarHandler::default();
        let mut from = x;
        let mut to = y;
        match &tokens[x] {
            Token::Bar(bar) => {
                // A repeat's end belongs to the measure before.
                if !(bar.repeat && bar.form == "end") {
                    handler.left = Some(bar.clone());
                }
                from = x + 1;
            }
            Token::Metadata { .. } if x != 0 => from = x + 1,
            _ => {}
        }
        match &tokens[y] {
            Token::Bar(bar) => {
                if !(bar.repeat && bar.form == "start") {
                    handler.right = Some(bar.clone());
                }
                to = y.wrapping_sub(1);
            }
            Token::Metadata { .. } => {}
            token => {
                if !(token.is_note() && y == tokens.len() - 1) {
                    to = y.wrapping_sub(1);
                }
            }
        }
        if to == usize::MAX || from > to {
            continue;
        }
        handler.tokens = tokens[from..=to].to_vec();
        if handler.tokens.is_empty() {
            continue;
        }
        handlers.push(handler);
    }
    handlers
}

/// music21's `mergeLeadingMetaData`: a handler of nothing but metadata is
/// joined to the one after it, and a tune of one measure is one handler.
fn merge_leading_metadata(handlers: Vec<BarHandler>) -> Vec<BarHandler> {
    let sounding = handlers
        .iter()
        .filter(|handler| Handler::has_notes(&handler.tokens))
        .count();
    if sounding <= 1 {
        let mut whole = BarHandler::default();
        for handler in &handlers {
            whole = whole.joined(handler);
        }
        return vec![whole];
    }
    let mut merged = Vec::new();
    let mut index = 0;
    while index < handlers.len() {
        let silent = !Handler::has_notes(&handlers[index].tokens);
        if silent && index != handlers.len() - 1 {
            merged.push(handlers[index].joined(&handlers[index + 1]));
            index += 2;
        } else {
            merged.push(handlers[index].clone());
            index += 1;
        }
    }
    merged
}

// --------------------------------------------------------------- translate

/// One element with where it stands and the order it was put in.
#[derive(Clone, Debug)]
struct Item {
    offset: FloatType,
    seq: usize,
    uid: Option<usize>,
    element: StreamElement,
}

/// A measure or a part while it is being filled: what it holds, and where
/// the next thing appended goes.
#[derive(Clone, Debug, Default)]
struct Line {
    items: Vec<Item>,
    end: FloatType,
    seq: usize,
}

impl Line {
    /// music21's `coreAppend`.
    fn append(&mut self, element: impl Into<StreamElement>, uid: Option<usize>) {
        let element = element.into();
        let length = element.quarter_length();
        self.items.push(Item {
            offset: self.end,
            seq: self.seq,
            uid,
            element,
        });
        self.seq += 1;
        self.end = op_frac(self.end + length);
    }

    /// A signature or a clef put at the start in place of the one there,
    /// as music21's `measure.timeSignature = ...` does.
    fn set_at_start(&mut self, element: StreamElement) {
        let same = |held: &StreamElement| {
            matches!(
                (held, &element),
                (
                    StreamElement::TimeSignature(_),
                    StreamElement::TimeSignature(_)
                ) | (StreamElement::Clef(_), StreamElement::Clef(_))
                    | (
                        StreamElement::KeySignature(_) | StreamElement::Key(_),
                        StreamElement::KeySignature(_) | StreamElement::Key(_)
                    )
            )
        };
        self.items.retain(|item| !same(&item.element));
        self.items.push(Item {
            offset: 0.0,
            seq: self.seq,
            uid: None,
            element,
        });
        self.seq += 1;
    }

    fn meter(&self) -> Option<&TimeSignature> {
        self.items.iter().find_map(|item| match &item.element {
            StreamElement::TimeSignature(meter) => Some(meter),
            _ => None,
        })
    }

    /// The elements in the order music21 sorts a stream into.
    fn sorted(&self) -> Vec<&Item> {
        let mut sorted: Vec<&Item> = self.items.iter().collect();
        sorted.sort_by(|left, right| {
            let grace = |item: &Item| item.element.duration().is_some_and(Duration::is_grace);
            left.offset
                .total_cmp(&right.offset)
                .then(class_sort_order(&left.element).cmp(&class_sort_order(&right.element)))
                .then(grace(right).cmp(&grace(left)))
                .then(left.seq.cmp(&right.seq))
        });
        sorted
    }
}

/// music21's `classSortOrder`.
fn class_sort_order(element: &StreamElement) -> i32 {
    match element {
        StreamElement::TextExpression(_) => -30,
        StreamElement::Instrument(_) => -25,
        StreamElement::Stream(_) => -20,
        StreamElement::Clef(_) => 0,
        StreamElement::MetronomeMark(_) | StreamElement::TempoText(_) => 1,
        StreamElement::KeySignature(_) | StreamElement::Key(_) => 2,
        StreamElement::TimeSignature(_) => 4,
        StreamElement::Dynamic(_) => 10,
        StreamElement::ChordSymbol(_) => 19,
        _ => 20,
    }
}

#[derive(Clone, Debug, Default)]
struct MeasureIr {
    line: Line,
    number: IntegerType,
    left: Option<Barline>,
    right: Option<Barline>,
}

/// An ending while its measures are being met.
#[derive(Clone, Debug)]
struct Bracket {
    number: u32,
    measures: Vec<usize>,
    complete: bool,
}

/// What reading one voice keeps between its measures.
struct PartReader {
    spanners: Vec<Spanning>,
    next_uid: usize,
    /// The notes a line of words is sung to, by measure and place.
    lyric_targets: Vec<(Option<usize>, usize)>,
    post_transposition: IntegerType,
}

impl PartReader {
    fn uid(&mut self) -> usize {
        self.next_uid += 1;
        self.next_uid
    }
}

/// music21's `addABCLyrics`: the words of a `w:` line, shared out among the
/// notes since the line before.
fn add_lyrics(data: &str, targets: &mut [&mut StreamElement]) -> Result<()> {
    let mut at = 0usize;
    for word in data.split_whitespace() {
        if word == "*" || word == "-" {
            at += 1;
            continue;
        }
        let syllables: Vec<&str> = word.split('-').collect();
        for (index, syllable) in syllables.iter().enumerate() {
            if syllable.is_empty() {
                at += 1;
                continue;
            }
            let Some(target) = targets.get_mut(at) else {
                return Ok(());
            };
            let text = format!(
                "{}{syllable}{}",
                if index > 0 { "-" } else { "" },
                if index + 1 < syllables.len() { "-" } else { "" }
            );
            match target {
                StreamElement::Note(note) => note.add_lyric(&text, None, false)?,
                StreamElement::Chord(chord) => chord.add_lyric(&text, None, false)?,
                _ => {}
            }
            at += 1;
        }
    }
    Ok(())
}

/// An element and the number spanners know it by.
type Placed = (StreamElement, Option<usize>);

/// music21's `parseABCNote`: the note, chord or rest a token stands for,
/// after the chord symbol written over it.
fn note_elements(
    token: &AbcNote,
    reader: &mut PartReader,
) -> Result<(Option<ChordSymbol>, Option<Placed>)> {
    let mut symbol = None;
    if let Some(first) = token.chord_symbols.first() {
        let mut name: String = first.replace('"', "").trim().to_string();
        name = name.replace(['(', ')'], "");
        // A flat written `b` after a note letter is music21's `-`.
        let chars: Vec<char> = name.chars().collect();
        let mut cleaned = String::new();
        for (index, character) in chars.iter().enumerate() {
            let after_letter = index > 0 && matches!(chars[index - 1], 'A'..='G' | 'a'..='g');
            cleaned.push(if *character == 'b' && after_letter {
                '-'
            } else {
                *character
            });
        }
        if matches!(cleaned.as_str(), "NC" | "N.C." | "No Chord" | "None") {
            symbol = Some(ChordSymbol::no_chord(Some(cleaned)));
        } else if cleaned.starts_with('>') {
            return Ok((None, None));
        } else if let Ok(read) = ChordSymbol::parse_music21(cleaned) {
            symbol = Some(read);
        }
    }

    let with_tuplet = |duration: &mut Duration, tuplet: Option<Tuplet>| {
        if let Some(mut tuplet) = tuplet {
            if tuplet.duration_normal().is_none()
                && let Some((kind, dots)) = duration.type_and_dots()
            {
                tuplet.set_duration_type(kind, dots);
            }
            duration.append_tuplet(tuplet);
        }
    };

    if token.is_chord {
        if token.sub_tokens.is_empty() {
            return Ok((symbol, None));
        }
        let mut members = Vec::new();
        for sub in &token.sub_tokens {
            let Some(name) = &sub.pitch_name else {
                continue;
            };
            let mut pitch = Pitch::from_name(name)?;
            if let Some(accidental) = pitch.written_accidental_mut() {
                accidental.set_display_status(sub.accidental_display);
            }
            members.push(Note::from_pitch(pitch));
        }
        let mut duration = Duration::new(token.quarter_length)?;
        with_tuplet(&mut duration, token.tuplet);
        let chord = Chord::new(members)?.with_duration(duration);
        let uid = reader.uid();
        return Ok((symbol, Some((chord.into(), Some(uid)))));
    }

    let mut duration = Duration::new(token.quarter_length)?;
    with_tuplet(&mut duration, token.tuplet);
    if token.is_rest {
        let uid = reader.uid();
        for spanner in &token.spanners {
            reader.spanners[*spanner].uids.push(uid);
        }
        let duration = if token.in_grace {
            grace_of(&duration)
        } else {
            duration
        };
        // A grace rest is a copy made after the spanners took the first.
        let placed = if token.in_grace { reader.uid() } else { uid };
        let mut rest = Rest::new(duration);
        rest.set_tie(tie_of(token.tie));
        *rest.articulations_mut() = articulations_of(&token.articulations);
        return Ok((symbol, Some((rest.into(), Some(placed)))));
    }
    let name = token
        .pitch_name
        .as_deref()
        .ok_or_else(|| abc_error("a note with no pitch"))?;
    let mut pitch = Pitch::from_name(name)?;
    if let Some(accidental) = pitch.written_accidental_mut() {
        accidental.set_display_status(token.accidental_display);
    }
    let mut note = Note::from_pitch(pitch);
    note.set_tie(tie_of(token.tie));
    let uid = reader.uid();
    for spanner in &token.spanners {
        if !reader.spanners[*spanner].uids.contains(&uid) {
            reader.spanners[*spanner].uids.push(uid);
        }
    }
    // A grace note is a copy made after the spanners took the note itself,
    // which no stream then holds.
    let placed = if token.in_grace {
        duration = grace_of(&duration);
        reader.uid()
    } else {
        uid
    };
    note.set_duration(duration);
    *note.articulations_mut() = articulations_of(&token.articulations);
    Ok((symbol, Some((note.into(), Some(placed)))))
}

/// The tie a token asks for, on a note or a rest alike.
fn tie_of(tie: Option<TieType>) -> Option<Tie> {
    match tie? {
        tie_type @ (TieType::Start | TieType::Continue) => {
            let mut tie = Tie::new(tie_type);
            tie.set_style(TieStyle::Normal);
            Some(tie)
        }
        tie_type => Some(Tie::new(tie_type)),
    }
}

/// The marks a token carries, last written first: music21 pops them off
/// the end of its list.
fn articulations_of(marks: &[&'static str]) -> Vec<Articulation> {
    marks
        .iter()
        .rev()
        .filter_map(|mark| {
            let class = match *mark {
                "staccato" => "Staccato",
                "upbow" => "UpBow",
                "downbow" => "DownBow",
                "accent" => "Accent",
                "strongaccent" => "StrongAccent",
                "tenuto" => "Tenuto",
                _ => return None,
            };
            ArticulationKind::from_class_name(class).map(Articulation::of_kind)
        })
        .collect()
}

/// A length as a grace note's: the written value, lasting nothing and in no
/// tuplet, whatever bracket the note was written inside.
fn grace_of(duration: &Duration) -> Duration {
    duration.grace_duration()
}

/// music21's `abcToStreamPart`: one voice as a part, with the spanners it
/// holds, each naming its notes by the numbers the part's leaves carry.
fn part_of(
    tokens: &[Token],
    spanners: &[Spanning],
) -> Result<(Stream, Vec<Option<usize>>, Vec<Spanning>)> {
    let handler = Handler {
        version: DEFAULT_VERSION,
        directives: Vec::new(),
        tokens: tokens.to_vec(),
    };
    let merged: Vec<BarHandler> = if handler.defines_measures() {
        merge_leading_metadata(split_by_measure(tokens))
    } else {
        vec![BarHandler {
            tokens: tokens.to_vec(),
            ..BarHandler::default()
        }]
    };
    let use_measures = merged.len() > 1;

    let mut reader = PartReader {
        spanners: spanners.to_vec(),
        next_uid: 0,
        lyric_targets: Vec::new(),
        post_transposition: 0,
    };
    // Which of the handler's spanners this voice starts, in order.
    let mut started: Vec<usize> = Vec::new();
    let mut part = Line::default();
    let mut measures: Vec<MeasureIr> = Vec::new();
    let mut brackets: Vec<Bracket> = Vec::new();
    let mut bar_count = 0;
    let mut measure_number = 1;

    for handler in &merged {
        let in_measure = use_measures && Handler::has_notes(&handler.tokens);
        let mut measure = MeasureIr::default();
        if in_measure {
            let index = measures.len();
            if let Some(left) = &handler.left {
                measure.left = left.barline();
                if let Some(number) = left.repeat_bracket() {
                    match brackets.iter_mut().find(|bracket| !bracket.complete) {
                        None => brackets.push(Bracket {
                            number,
                            measures: vec![index],
                            complete: number == 2,
                        }),
                        Some(open) => {
                            if !open.measures.contains(&index) {
                                open.measures.push(index);
                            }
                            open.complete = true;
                        }
                    }
                }
            }
            if let Some(right) = &handler.right {
                // A repeat closing a measure ends the passage.
                measure.right = right.barline().map(|barline| {
                    if barline.repeat_direction() == Some(RepeatDirection::Start) {
                        Barline::repeat(RepeatDirection::End, None)
                    } else {
                        barline
                    }
                });
                if right.repeat
                    && let Some(open) = brackets.iter_mut().find(|bracket| !bracket.complete)
                {
                    if !open.measures.contains(&index) {
                        open.measures.push(index);
                    }
                    open.complete = true;
                }
            }
            bar_count += 1;
        }

        // music21's `parseTokens`.
        reader.post_transposition = 0;
        let place = in_measure.then_some(measures.len());
        {
            let line = if in_measure {
                &mut measure.line
            } else {
                &mut part
            };
            for token in &handler.tokens {
                match token {
                    Token::Metadata { tag, data } => {
                        if tag == "w" {
                            let wanted = std::mem::take(&mut reader.lyric_targets);
                            let mut targets: Vec<&mut StreamElement> = Vec::new();
                            // Notes of this line only; those of measures
                            // already closed are reached below.
                            let mut here: Vec<usize> = wanted
                                .iter()
                                .filter(|(held, _)| *held == place)
                                .map(|(_, index)| *index)
                                .collect();
                            here.sort_unstable();
                            let outside = wanted.iter().any(|(held, _)| *held != place);
                            if !outside {
                                let mut rest: &mut [Item] = &mut line.items;
                                let mut passed = 0;
                                for index in here {
                                    let (_, tail) = rest.split_at_mut(index - passed);
                                    let (item, tail) = tail
                                        .split_first_mut()
                                        .ok_or_else(|| abc_error("a lyric's note is missing"))?;
                                    targets.push(&mut item.element);
                                    rest = tail;
                                    passed = index + 1;
                                }
                                add_lyrics(data, &mut targets)?;
                            } else {
                                add_lyrics_across(data, &wanted, &mut measures, line, place)?;
                            }
                        }
                        match tag.as_str() {
                            "M" => {
                                if let Some(meter) = meter_of(data)? {
                                    if use_measures {
                                        line.set_at_start(meter.into());
                                    } else {
                                        line.append(meter, None);
                                    }
                                }
                            }
                            "K" => {
                                let key = key_of(data)?;
                                if use_measures {
                                    line.set_at_start(key);
                                } else {
                                    line.append(key, None);
                                }
                                if let Some((clef, transposition)) = clef_of(data) {
                                    if use_measures {
                                        line.set_at_start(clef.into());
                                    } else {
                                        line.append(clef, None);
                                    }
                                    reader.post_transposition = transposition;
                                }
                            }
                            "Q" => line.append(tempo_of(data)?, None),
                            _ => {}
                        }
                    }
                    Token::Note(note) => {
                        let (symbol, made) = note_elements(note, &mut reader)?;
                        if let Some(symbol) = symbol {
                            line.append(symbol, None);
                        }
                        if let Some((element, uid)) = made {
                            let sung = !matches!(element, StreamElement::Rest(_)) && !note.in_grace;
                            line.append(element, uid);
                            if sung {
                                reader.lyric_targets.push((place, line.items.len() - 1));
                            }
                        }
                    }
                    Token::SpannerStart(_, Some(index)) => started.push(*index),
                    _ => {}
                }
            }
        }

        if in_measure {
            if bar_count == 1 && measure.line.meter().is_some() {
                // A short first measure is a pickup, numbered nought.
                measure.number = 0;
            } else {
                measure.number = measure_number;
                measure_number += 1;
            }
            // Lyric targets name a measure by its place among them.
            measures.push(measure);
        }
    }

    re_bar(&mut measures, &mut brackets);

    // Endings, from the brackets over the measures.
    let mut endings: Vec<Option<Ending>> = vec![None; measures.len()];
    // An ending never closed is left out, as music21 leaves it.
    for bracket in brackets.iter().filter(|bracket| bracket.complete) {
        for (place, index) in bracket.measures.iter().enumerate() {
            if let Some(slot) = endings.get_mut(*index)
                && slot.is_none()
            {
                *slot = Some(Ending::new(
                    vec![bracket.number],
                    place == 0,
                    place + 1 == bracket.measures.len(),
                ));
            }
        }
    }

    // A part with no clef takes the one that fits its notes best.
    let has_clef = measures
        .iter()
        .flat_map(|measure| &measure.line.items)
        .chain(&part.items)
        .any(|item| matches!(item.element, StreamElement::Clef(_)));
    if !has_clef {
        let mut pitches: Vec<Pitch> = Vec::new();
        for item in measures
            .iter()
            .flat_map(|measure| measure.line.sorted())
            .chain(part.sorted())
        {
            match &item.element {
                StreamElement::Note(note) => pitches.push(note.pitch().clone()),
                StreamElement::Chord(chord) => pitches.extend(chord.pitches()),
                StreamElement::ChordSymbol(symbol) => {
                    pitches.extend(symbol.pitches().unwrap_or_default());
                }
                _ => {}
            }
        }
        let clef = Clef::best_for(&pitches, false);
        match measures.first_mut() {
            Some(first) if use_measures => first.line.set_at_start(clef.into()),
            _ => {
                part.items.push(Item {
                    offset: 0.0,
                    seq: part.seq,
                    uid: None,
                    element: clef.into(),
                });
                part.seq += 1;
            }
        }
    }

    if reader.post_transposition != 0 {
        let interval = Interval::from_name(if reader.post_transposition == -12 {
            "P-8"
        } else {
            "P-15"
        })?;
        for item in measures
            .iter_mut()
            .flat_map(|measure| &mut measure.line.items)
            .chain(&mut part.items)
        {
            if item.element.is_note_or_chord() {
                item.element = item.element.transpose(&interval)?;
            }
        }
    }

    // Put together: the measures one after another, or the part's own line.
    let mut uids: Vec<Option<usize>> = Vec::new();
    let mut events = Vec::new();
    let mut offset = 0.0;
    let has_meter = part.meter().is_some()
        || measures
            .iter()
            .any(|measure| measure.line.meter().is_some());
    for (index, measure) in measures.into_iter().enumerate() {
        let mut inner = Vec::new();
        for item in measure.line.sorted() {
            uids.push(item.uid);
            inner.push(StreamEvent::new(item.offset, item.element.clone()));
        }
        let mut stream = Stream::from_events(inner);
        stream.set_kind(StreamKind::Measure);
        stream.set_number(measure.number);
        stream.set_left_barline(measure.left);
        stream.set_right_barline(measure.right);
        stream.set_ending(endings[index].clone());
        let length = measure.line.end;
        events.push(StreamEvent::new(offset, stream));
        offset = op_frac(offset + length);
    }
    for item in part.sorted() {
        uids.push(item.uid);
        events.push(StreamEvent::new(item.offset, item.element.clone()));
    }
    let mut stream = Stream::from_events(events);
    stream.set_kind(StreamKind::Part);

    if use_measures && has_meter {
        // music21 warns and carries on where a measure cannot be beamed.
        let _ = crate::makenotation::make_beams(&mut stream);
    }

    let held: Vec<Spanning> = started
        .into_iter()
        .map(|index| reader.spanners[index].clone())
        .collect();
    Ok((stream, uids, held))
}

/// Words sung to notes of measures already closed as well as this one.
fn add_lyrics_across(
    data: &str,
    wanted: &[(Option<usize>, usize)],
    measures: &mut [MeasureIr],
    line: &mut Line,
    place: Option<usize>,
) -> Result<()> {
    // One mutable borrow per note: each is reached in turn by where it is.
    let mut elements: Vec<StreamElement> = Vec::new();
    for (held, index) in wanted {
        let item = if *held == place {
            line.items.get(*index)
        } else {
            held.and_then(|measure| measures.get(measure))
                .and_then(|measure| measure.line.items.get(*index))
        };
        elements.push(
            item.map(|item| item.element.clone())
                .ok_or_else(|| abc_error("a lyric's note is missing"))?,
        );
    }
    {
        let mut targets: Vec<&mut StreamElement> = elements.iter_mut().collect();
        add_lyrics(data, &mut targets)?;
    }
    for ((held, index), element) in wanted.iter().zip(elements) {
        let slot = if *held == place {
            line.items.get_mut(*index)
        } else {
            held.and_then(|measure| measures.get_mut(measure))
                .and_then(|measure| measure.line.items.get_mut(*index))
        };
        if let Some(slot) = slot {
            slot.element = element;
        }
    }
    Ok(())
}

/// A line as the measure it will be, for the questions asked of a stream.
fn measure_stream(line: &Line) -> Stream {
    let mut stream = Stream::from_events(
        line.sorted()
            .into_iter()
            .map(|item| StreamEvent::new(item.offset, item.element.clone())),
    );
    stream.set_kind(StreamKind::Measure);
    stream
}

/// music21's `reBar`: a measure holding more than its bar is cut in two, the
/// second taking the meter that fits it where it is not a whole bar, and
/// every later measure is numbered one further on.
///
/// music21 gives up where it stands on a measure with no meter in force or
/// one no meter fits, keeping what it had done; that measure's overflow is
/// then lost, as it is there.
fn re_bar(measures: &mut Vec<MeasureIr>, brackets: &mut [Bracket]) {
    let mut meter: Option<TimeSignature> = None;
    let mut shift = 0;
    // Where each measure read stands once the cut ones are in.
    let mut places: Vec<usize> = (0..measures.len()).collect();
    let mut index = 0;
    let mut original = 0;
    let count = measures.len();
    'measures: while original < count {
        if let Some(own) = measures[index].line.meter() {
            meter = Some(own.clone());
        }
        let Some(current) = meter.clone() else {
            break;
        };
        places[original] = index;
        let bar = op_frac(current.bar_quarter_length());
        let end = measures[index].line.end;
        measures[index].number += shift;
        if end > bar + 1e-9 {
            let mut second = Line::default();
            let mut kept: Vec<Item> = Vec::new();
            let mut moved: Vec<Item> = Vec::new();
            let items = std::mem::take(&mut measures[index].line.items);
            let mut broken = false;
            for item in items {
                let length = item.element.quarter_length();
                if (item.offset - bar).abs() <= 1e-9 && length == 0.0 {
                    // Something with no length standing on the barline ends
                    // where the new measure starts, and stays behind.
                    kept.push(item);
                } else if item.offset >= bar - 1e-9 {
                    moved.push(item);
                } else if item.offset + length > bar + 1e-9 {
                    match split_element(&item.element, op_frac(bar - item.offset)) {
                        Ok((first, remain)) => {
                            second.items.push(Item {
                                offset: 0.0,
                                seq: second.seq,
                                uid: None,
                                element: remain,
                            });
                            second.seq += 1;
                            kept.push(Item {
                                element: first,
                                ..item
                            });
                        }
                        Err(_) => {
                            broken = true;
                            kept.push(item);
                        }
                    }
                } else {
                    kept.push(item);
                }
            }
            measures[index].line.items = kept;
            measures[index].line.end = bar;
            if broken {
                break 'measures;
            }
            for item in moved {
                second.items.push(Item {
                    offset: op_frac(item.offset - bar),
                    seq: second.seq,
                    ..item
                });
                second.seq += 1;
            }
            second.end = op_frac(end - bar);
            if (bar - second.end).abs() > 1e-9 {
                let Ok(best) = crate::meter::best_time_signature(&measure_stream(&second)) else {
                    break 'measures;
                };
                second.set_at_start(best.into());
                if original + 1 < count && measures[index + 1].line.meter().is_none() {
                    measures[index + 1]
                        .line
                        .set_at_start(current.clone().into());
                }
            }
            let number = measures[index].number + 1;
            shift += 1;
            // The barline closing the measure is moved with the notes and
            // lands at the start of the new one.
            let left = measures[index].right.take();
            measures.insert(
                index + 1,
                MeasureIr {
                    line: second,
                    number,
                    left,
                    right: None,
                },
            );
            index += 1;
        }
        index += 1;
        original += 1;
    }
    for (passed, place) in places.iter_mut().skip(original).enumerate() {
        *place = index + passed;
    }
    for bracket in brackets {
        for measure in &mut bracket.measures {
            *measure = places[*measure];
        }
    }
}

/// music21's `abcToStreamScore`.
fn score_of(handler: &Handler, spanners: &[Spanning]) -> Result<Stream> {
    let mut metadata = Metadata::new();
    let mut titles = 0;
    for token in &handler.tokens {
        let Some((tag, data)) = token.metadata() else {
            continue;
        };
        match tag {
            "T" => {
                metadata.add_text(
                    if titles == 0 {
                        "title"
                    } else {
                        "alternativeTitle"
                    },
                    data,
                );
                titles += 1;
            }
            "C" => metadata.add_contributor("composer", data),
            "O" => metadata.add_text("localeOfComposition", data),
            "X" => {
                let number = data.trim().parse::<IntegerType>().map_err(|_| {
                    abc_error(format!("the reference number {data:?} is no number"))
                })?;
                metadata.add_text("number", number.to_string());
            }
            _ => {}
        }
    }

    let voices = handler.split_by_voice();
    let parts: Vec<Vec<Token>> = if voices.len() == 1 {
        voices
    } else {
        // The header goes in front of each voice.
        voices[1..]
            .iter()
            .map(|voice| {
                voices[0]
                    .iter()
                    .cloned()
                    .chain(voice.iter().cloned())
                    .collect()
            })
            .collect()
    };

    let mut events = Vec::new();
    let mut uids: Vec<Option<(usize, usize)>> = Vec::new();
    let mut held: Vec<(usize, Spanning)> = Vec::new();
    for (index, tokens) in parts.iter().enumerate() {
        let (part, part_uids, part_spanners) = part_of(tokens, spanners)?;
        uids.extend(part_uids.into_iter().map(|uid| uid.map(|uid| (index, uid))));
        held.extend(part_spanners.into_iter().map(|spanner| (index, spanner)));
        events.push(StreamEvent::new(0.0, part));
    }
    let mut score = Stream::from_events(events);
    score.set_kind(StreamKind::Score);
    score.set_metadata(Some(metadata));
    for (part, spanner) in held {
        let positions: Vec<Option<usize>> = spanner
            .uids
            .iter()
            .map(|uid| uids.iter().position(|held| *held == Some((part, *uid))))
            .collect();
        let made = match spanner.kind {
            SpannerKind::Crescendo | SpannerKind::Diminuendo => {
                let template = Spanner::wedge(spanner.kind, Vec::new());
                let mut wedge = Spanner::with_unplaced(spanner.kind, positions);
                wedge.set_placement(template.placement());
                wedge.set_spread(template.spread());
                wedge
            }
            kind => Spanner::with_unplaced(kind, positions),
        };
        score.add_spanner(made);
    }
    Ok(score)
}

/// music21's `extractReferenceNumber`: the lines of one tune.
fn extract_reference_number(text: &str, number: IntegerType) -> Result<String> {
    let mut collected = Vec::new();
    let mut gathering = false;
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
        }
    }
    if collected.is_empty() {
        return Err(abc_error(format!(
            "cannot find requested reference number in source file: {number}"
        )));
    }
    Ok(collected.join("\n"))
}

fn handler_of(text: &str) -> Result<(Handler, Vec<Spanning>)> {
    let mut handler = Handler::new(DEFAULT_VERSION);
    handler.read_version(text);
    handler.tokenize(text)?;
    let spanners = handler.process()?;
    Ok((handler, spanners))
}

/// Reads ABC text into a score: music21's `converter.parse` for the format.
///
/// A file holding one tune comes back as a score, with a part for each of
/// its voices. A file holding several tunes, each under its own `X:`
/// reference number, comes back as an opus of scores in the order of their
/// numbers.
///
/// ```
/// use music21_rs::abc::from_abc;
///
/// let score = from_abc("X:1\nT:Scale\nM:4/4\nL:1/4\nK:G\nG A B c | d e f g |\n")?;
/// let part = score.parts()[0];
/// assert_eq!(part.measures().len(), 2);
/// // The key's F sharp is read into the note.
/// assert_eq!(part.measures()[1].pitches()[2].name_with_octave(), "F#5");
/// # Ok::<(), music21_rs::Error>(())
/// ```
pub fn from_abc(text: &str) -> Result<Stream> {
    let (handler, spanners) = handler_of(text)?;
    if handler.defines_reference_numbers() {
        let mut events = Vec::new();
        let mut offset = 0.0;
        for (_, tune) in handler.split_by_reference_number() {
            let score = score_of(&tune, &spanners)?;
            let length = score.end_offset();
            events.push(StreamEvent::new(offset, score));
            offset += length;
        }
        let mut opus = Stream::from_events(events);
        opus.set_kind(StreamKind::Opus);
        return Ok(opus);
    }
    score_of(&handler, &spanners)
}

/// Reads one tune out of ABC text holding several, by its `X:` reference
/// number: music21's `converter.parse(..., number=n)`.
pub fn from_abc_number(text: &str, number: IntegerType) -> Result<Stream> {
    let (handler, spanners) = handler_of(&extract_reference_number(text, number)?)?;
    score_of(&handler, &spanners)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens_of(text: &str) -> Vec<Token> {
        let mut handler = Handler::new(DEFAULT_VERSION);
        handler.tokenize(text).unwrap();
        handler.tokens
    }

    #[test]
    fn a_bar_says_what_it_is() {
        // music21's `ABCBar.parse` docstring.
        let bar = Bar::new("|");
        assert!(bar.is_regular() && !bar.repeat);
        let end = Bar::new(":|");
        assert!(end.repeat && end.form == "end" && end.style == "light-heavy");
        let first = Bar::new("[1");
        assert_eq!(first.repeat_bracket(), Some(1));
        assert_eq!(
            Bar::new("||").barline().unwrap().bar_type(),
            BarlineType::Double
        );
    }

    #[test]
    fn text_is_cut_into_tokens_as_music21_cuts_it() {
        let tokens = tokens_of("M:6/8\nL:1/8\nK:G\nB3 A3 | (3GAB c2 :|\n");
        let kinds: Vec<&str> = tokens
            .iter()
            .map(|token| match token {
                Token::Metadata { .. } => "meta",
                Token::Bar(_) => "bar",
                Token::Tuplet { .. } => "tuplet",
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
    }

    #[test]
    fn a_length_is_read_against_the_default() {
        // music21's `getQuarterLength` docstring.
        let mut note = AbcNote::new("", "", false);
        note.default_quarter_length = Some(0.5);
        assert_eq!(note.read_quarter_length("f", None).unwrap(), 0.5);
        assert_eq!(note.read_quarter_length("f2", None).unwrap(), 1.0);
        assert_eq!(note.read_quarter_length("f/", None).unwrap(), 0.25);
        assert_eq!(note.read_quarter_length("f3/2", None).unwrap(), 0.75);
        note.broken_rhythm = Some((">".to_string(), true));
        assert_eq!(note.read_quarter_length("A", None).unwrap(), 0.75);
    }

    #[test]
    fn a_pitch_takes_its_accidental_from_the_key() {
        let mut note = AbcNote::new("f", "", false);
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
    }

    #[test]
    fn a_tune_is_a_score_of_measures() {
        let score =
            from_abc("X:1\nT:Tune\nC:Anon\nM:3/4\nL:1/4\nK:D\nD E F | G A B | c d2 |]\n").unwrap();
        assert_eq!(score.kind(), StreamKind::Score);
        let metadata = score.metadata().unwrap();
        assert_eq!(metadata.first_text("title"), Some("Tune"));
        let part = score.parts()[0];
        let measures = part.measures();
        assert_eq!(measures.len(), 3);
        let names: Vec<String> = measures[0]
            .pitches()
            .iter()
            .map(|pitch| pitch.name_with_octave())
            .collect();
        assert_eq!(names, ["D4", "E4", "F#4"]);
        assert_eq!(
            measures[2].right_barline().unwrap().bar_type(),
            BarlineType::Final
        );
    }

    #[test]
    fn several_tunes_are_an_opus_and_one_can_be_asked_for() {
        let text = "X:1\nT:One\nM:2/4\nL:1/4\nK:C\nC D | E F |]\n\nX:2\nT:Two\nM:2/4\nL:1/4\nK:C\nG A | B c |]\n";
        let opus = from_abc(text).unwrap();
        assert_eq!(opus.kind(), StreamKind::Opus);
        assert_eq!(opus.events().len(), 2);
        let second = from_abc_number(text, 2).unwrap();
        assert_eq!(second.metadata().unwrap().first_text("title"), Some("Two"));
        assert!(from_abc_number(text, 3).is_err());
    }

    #[test]
    fn a_measure_holding_more_than_its_bar_is_cut_in_two() {
        // Six quarters between barlines in 4/4: a whole bar, then a 2/4 bar
        // holding the tied remainder of the long note, and the meter put
        // back on the measure after.
        let score = from_abc(
            "X:1
M:4/4
L:1/4
K:C
CDEF|GAB4|CDEF|
",
        )
        .unwrap();
        let part = score.events()[0].element().as_stream().unwrap();
        let measures = part.measures();
        let numbers: Vec<_> = measures.iter().map(|measure| measure.number()).collect();
        assert_eq!(numbers, [0, 1, 2, 3]);
        let meter = |measure: &Stream| {
            measure
                .events()
                .iter()
                .find_map(|event| match event.element() {
                    StreamElement::TimeSignature(meter) => Some(meter.ratio_string()),
                    _ => None,
                })
        };
        assert_eq!(meter(measures[1]), None);
        assert_eq!(meter(measures[2]).as_deref(), Some("2/4"));
        assert_eq!(meter(measures[3]).as_deref(), Some("4/4"));
        let ties: Vec<_> = measures[1..3]
            .iter()
            .flat_map(|measure| measure.events())
            .filter_map(|event| match event.element() {
                StreamElement::Note(note) if note.pitch().name() == "B" => Some((
                    event.element().quarter_length(),
                    note.tie().map(Tie::tie_type),
                )),
                _ => None,
            })
            .collect();
        assert_eq!(
            ties,
            [(2.0, Some(TieType::Start)), (2.0, Some(TieType::Stop))]
        );
    }
}
