//! Notation attached to a note or a chord: ties, noteheads, stem direction,
//! colour and lyrics.
//!
//! These are the parts of music21's `note.NotRest` and `note.GeneralNote`
//! that carry musical intent rather than layout. A tie says two written notes
//! sound as one; a diamond notehead says a string harmonic; a lyric is the
//! text sung on the note. None of them need a stream to mean something, which
//! is why they live in the data model here alongside pitch and duration.

use std::fmt;
use std::str::FromStr;

use crate::{
    defaults::IntegerType,
    error::{Error, Result},
};

/// How a tie joins this note to its neighbours: music21's `tie.Tie.type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TieType {
    /// The note begins a tie.
    Start,
    /// The note ends a tie.
    Stop,
    /// The note is tied both from the one before and to the one after.
    Continue,
    /// The note rings on with no written end, as a piano pedal note does.
    LetRing,
    /// A `continue` that also lets the sound ring on.
    ContinueLetRing,
}

impl TieType {
    /// Every tie type, in music21's order.
    pub const ALL: [TieType; 5] = [
        TieType::Start,
        TieType::Stop,
        TieType::Continue,
        TieType::LetRing,
        TieType::ContinueLetRing,
    ];

    /// music21's name for the type.
    pub fn as_str(self) -> &'static str {
        match self {
            TieType::Start => "start",
            TieType::Stop => "stop",
            TieType::Continue => "continue",
            TieType::LetRing => "let-ring",
            TieType::ContinueLetRing => "continue-let-ring",
        }
    }

    /// Reads music21's name for the type.
    pub fn from_name(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.as_str() == name)
            .ok_or_else(|| {
                let valid = Self::ALL
                    .iter()
                    .map(|candidate| format!("'{}'", candidate.as_str()))
                    .collect::<Vec<_>>()
                    .join(", ");
                Error::Notation(format!("Type must be one of ({valid}), not {name}"))
            })
    }
}

impl fmt::Display for TieType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How the tie is drawn: music21's `tie.Tie.style`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TieStyle {
    /// A solid tie.
    #[default]
    Normal,
    /// A dotted tie.
    Dotted,
    /// A dashed tie.
    Dashed,
    /// A tie that sounds but is not drawn.
    Hidden,
}

impl TieStyle {
    /// Every tie style.
    pub const ALL: [TieStyle; 4] = [
        TieStyle::Normal,
        TieStyle::Dotted,
        TieStyle::Dashed,
        TieStyle::Hidden,
    ];

    /// music21's name for the style.
    pub fn as_str(self) -> &'static str {
        match self {
            TieStyle::Normal => "normal",
            TieStyle::Dotted => "dotted",
            TieStyle::Dashed => "dashed",
            TieStyle::Hidden => "hidden",
        }
    }

    /// Reads music21's name for the style.
    pub fn from_name(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.as_str() == name)
            .ok_or_else(|| Error::Notation(format!("not a valid tie style: {name}")))
    }
}

impl fmt::Display for TieStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Which side of the note a mark sits on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Placement {
    /// Above the note.
    Above,
    /// Below the note.
    Below,
}

impl Placement {
    /// music21's name for the placement.
    pub fn as_str(self) -> &'static str {
        match self {
            Placement::Above => "above",
            Placement::Below => "below",
        }
    }

    /// Reads music21's name for the placement.
    pub fn from_name(name: &str) -> Result<Self> {
        match name {
            "above" => Ok(Placement::Above),
            "below" => Ok(Placement::Below),
            other => Err(Error::Notation(format!("not a valid placement: {other}"))),
        }
    }
}

impl fmt::Display for Placement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A tie joining a note to its neighbours: music21's `tie.Tie`.
///
/// A tie on the first of two notes is enough to say they sound as one; the
/// matching `Stop` on the second is what a MusicXML writer needs, not what
/// the music needs.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tie {
    tie_type: TieType,
    style: TieStyle,
    placement: Option<Placement>,
}

impl Tie {
    /// A tie of the given type, drawn normally, with no placement of its own.
    pub fn new(tie_type: TieType) -> Self {
        Self {
            tie_type,
            style: TieStyle::default(),
            placement: None,
        }
    }

    /// Reads a tie from music21's type name, `"start"` through
    /// `"continue-let-ring"`.
    pub fn from_name(name: &str) -> Result<Self> {
        Ok(Self::new(TieType::from_name(name)?))
    }

    /// The tie type.
    pub fn tie_type(&self) -> TieType {
        self.tie_type
    }

    /// Replaces the tie type.
    pub fn set_tie_type(&mut self, tie_type: TieType) {
        self.tie_type = tie_type;
    }

    /// How the tie is drawn.
    pub fn style(&self) -> TieStyle {
        self.style
    }

    /// Sets how the tie is drawn.
    pub fn set_style(&mut self, style: TieStyle) {
        self.style = style;
    }

    /// Which side of the note the tie sits on, when it was said.
    pub fn placement(&self) -> Option<Placement> {
        self.placement
    }

    /// Sets which side of the note the tie sits on.
    pub fn set_placement(&mut self, placement: Option<Placement>) {
        self.placement = placement;
    }
}

impl Default for Tie {
    /// A starting tie, as music21's `Tie()` is.
    fn default() -> Self {
        Self::new(TieType::Start)
    }
}

impl fmt::Display for Tie {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tie {}", self.tie_type)
    }
}

impl FromStr for Tie {
    type Err = Error;

    fn from_str(name: &str) -> Result<Self> {
        Self::from_name(name)
    }
}

/// The shape drawn for a note head: music21's `noteheadTypeNames`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Notehead {
    /// `arrow down`
    ArrowDown,
    /// `arrow up`
    ArrowUp,
    /// `back slashed`
    BackSlashed,
    /// `circle dot`
    CircleDot,
    /// `circle-x`
    CircleX,
    /// `circled`
    Circled,
    /// `cluster`
    Cluster,
    /// `cross`
    Cross,
    /// `diamond`, which is how a string harmonic is written.
    Diamond,
    /// `do`
    Do,
    /// `fa`
    Fa,
    /// `fa up`
    FaUp,
    /// `inverted triangle`
    InvertedTriangle,
    /// `la`
    La,
    /// `left triangle`
    LeftTriangle,
    /// `mi`
    Mi,
    /// `none`
    NoneShape,
    /// `normal`, the default.
    #[default]
    Normal,
    /// `other`
    Other,
    /// `re`
    Re,
    /// `rectangle`
    Rectangle,
    /// `slash`
    Slash,
    /// `slashed`
    Slashed,
    /// `so`
    So,
    /// `square`
    Square,
    /// `ti`
    Ti,
    /// `triangle`
    Triangle,
    /// `x`
    X,
}

impl Notehead {
    /// Every notehead shape, in music21's order.
    pub const ALL: [Notehead; 28] = [
        Notehead::ArrowDown,
        Notehead::ArrowUp,
        Notehead::BackSlashed,
        Notehead::CircleDot,
        Notehead::CircleX,
        Notehead::Circled,
        Notehead::Cluster,
        Notehead::Cross,
        Notehead::Diamond,
        Notehead::Do,
        Notehead::Fa,
        Notehead::FaUp,
        Notehead::InvertedTriangle,
        Notehead::La,
        Notehead::LeftTriangle,
        Notehead::Mi,
        Notehead::NoneShape,
        Notehead::Normal,
        Notehead::Other,
        Notehead::Re,
        Notehead::Rectangle,
        Notehead::Slash,
        Notehead::Slashed,
        Notehead::So,
        Notehead::Square,
        Notehead::Ti,
        Notehead::Triangle,
        Notehead::X,
    ];

    /// music21's name for the shape.
    pub fn as_str(self) -> &'static str {
        match self {
            Notehead::ArrowDown => "arrow down",
            Notehead::ArrowUp => "arrow up",
            Notehead::BackSlashed => "back slashed",
            Notehead::CircleDot => "circle dot",
            Notehead::CircleX => "circle-x",
            Notehead::Circled => "circled",
            Notehead::Cluster => "cluster",
            Notehead::Cross => "cross",
            Notehead::Diamond => "diamond",
            Notehead::Do => "do",
            Notehead::Fa => "fa",
            Notehead::FaUp => "fa up",
            Notehead::InvertedTriangle => "inverted triangle",
            Notehead::La => "la",
            Notehead::LeftTriangle => "left triangle",
            Notehead::Mi => "mi",
            Notehead::NoneShape => "none",
            Notehead::Normal => "normal",
            Notehead::Other => "other",
            Notehead::Re => "re",
            Notehead::Rectangle => "rectangle",
            Notehead::Slash => "slash",
            Notehead::Slashed => "slashed",
            Notehead::So => "so",
            Notehead::Square => "square",
            Notehead::Ti => "ti",
            Notehead::Triangle => "triangle",
            Notehead::X => "x",
        }
    }

    /// Reads music21's name for the shape.
    pub fn from_name(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.as_str() == name)
            .ok_or_else(|| Error::Notation(format!("not a valid notehead type name: '{name}'")))
    }
}

impl fmt::Display for Notehead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Which way the stem points: music21's `stemDirectionNames`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum StemDirection {
    /// Stems both up and down, for a divided part.
    Double,
    /// Stem down.
    Down,
    /// Written with no stem at all, as a harmonic sounding pitch is.
    NoStem,
    /// Not said, the default.
    #[default]
    Unspecified,
    /// Stem up.
    Up,
}

impl StemDirection {
    /// Every stem direction a note can hold.
    ///
    /// One shorter than music21's [`StemDirection::NAMES`]: `"none"` is an
    /// input spelling of `"noStem"` that music21's setter rewrites, so no
    /// note ever reads back as `"none"`.
    pub const ALL: [StemDirection; 5] = [
        StemDirection::Double,
        StemDirection::Down,
        StemDirection::NoStem,
        StemDirection::Unspecified,
        StemDirection::Up,
    ];

    /// music21's `stemDirectionNames`: the names the setter accepts, which
    /// includes the `"none"` spelling of `"noStem"`.
    pub const NAMES: [&'static str; 6] = ["double", "down", "noStem", "none", "unspecified", "up"];

    /// music21's name for the direction.
    pub fn as_str(self) -> &'static str {
        match self {
            StemDirection::Double => "double",
            StemDirection::Down => "down",
            StemDirection::NoStem => "noStem",
            StemDirection::Unspecified => "unspecified",
            StemDirection::Up => "up",
        }
    }

    /// Reads music21's name for the direction.
    ///
    /// `"none"` reads as [`StemDirection::NoStem`], which is what music21's
    /// setter stores for it.
    pub fn from_name(name: &str) -> Result<Self> {
        if name == "none" {
            return Ok(StemDirection::NoStem);
        }
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.as_str() == name)
            .ok_or_else(|| Error::Notation(format!("not a valid stem direction name: {name}")))
    }
}

impl fmt::Display for StemDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where a lyric sits in a word: music21's `Lyric.syllabic`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Syllabic {
    /// The whole word.
    Single,
    /// The first syllable of a word, hyphenated to what follows.
    Begin,
    /// A middle syllable, hyphenated on both sides.
    Middle,
    /// The last syllable of a word.
    End,
    /// A syllable that carries no hyphens, written as `text` in MusicXML.
    Composite,
}

impl Syllabic {
    /// Every syllabic position.
    pub const ALL: [Syllabic; 5] = [
        Syllabic::Single,
        Syllabic::Begin,
        Syllabic::Middle,
        Syllabic::End,
        Syllabic::Composite,
    ];

    /// music21's name for the position.
    pub fn as_str(self) -> &'static str {
        match self {
            Syllabic::Single => "single",
            Syllabic::Begin => "begin",
            Syllabic::Middle => "middle",
            Syllabic::End => "end",
            Syllabic::Composite => "composite",
        }
    }

    /// Reads music21's name for the position.
    pub fn from_name(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.as_str() == name)
            .ok_or_else(|| Error::Notation(format!("not a valid syllabic value: {name}")))
    }
}

impl fmt::Display for Syllabic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A syllable of text sung on a note: music21's `note.Lyric`.
///
/// The hyphens in the written text say where the syllable falls in its word,
/// which is why [`Self::from_raw_text`] reads `"-ci-"` as a middle syllable
/// and [`Self::raw_text`] writes the hyphens back.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Lyric {
    text: String,
    number: IntegerType,
    syllabic: Syllabic,
    identifier: Option<String>,
}

impl Lyric {
    /// A lyric on the first verse, spelled as a whole word.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            number: 1,
            syllabic: Syllabic::Single,
            identifier: None,
        }
    }

    /// Reads a lyric from text whose hyphens say where it falls in its word:
    /// `"-ci-"` is a middle syllable, `"ci-"` a beginning, `"-us"` an end.
    pub fn from_raw_text(raw_text: &str) -> Self {
        let mut lyric = Self::new("");
        lyric.set_raw_text(raw_text);
        lyric
    }

    /// The syllable without its hyphens.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replaces the syllable text, leaving its position in the word alone.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    /// The verse number, counting from one.
    pub fn number(&self) -> IntegerType {
        self.number
    }

    /// Sets the verse number, which must be positive as music21 requires.
    pub fn set_number(&mut self, number: IntegerType) -> Result<()> {
        if number <= 0 {
            return Err(Error::Notation(format!(
                "Number best be number {number}, not a string"
            )));
        }
        self.number = number;
        Ok(())
    }

    /// Where the syllable falls in its word.
    pub fn syllabic(&self) -> Syllabic {
        self.syllabic
    }

    /// Sets where the syllable falls in its word.
    pub fn set_syllabic(&mut self, syllabic: Syllabic) {
        self.syllabic = syllabic;
    }

    /// The name this verse goes by, which music21 falls back to the number
    /// for when none was given.
    pub fn identifier(&self) -> String {
        self.identifier
            .clone()
            .unwrap_or_else(|| self.number.to_string())
    }

    /// Names this verse, or clears the name so the number stands in.
    pub fn set_identifier(&mut self, identifier: Option<String>) {
        self.identifier = identifier;
    }

    /// The syllable written with the hyphens its position implies.
    pub fn raw_text(&self) -> String {
        match self.syllabic {
            Syllabic::Begin => format!("{}-", self.text),
            Syllabic::Middle => format!("-{}-", self.text),
            Syllabic::End => format!("-{}", self.text),
            Syllabic::Single | Syllabic::Composite => self.text.clone(),
        }
    }

    /// Reads the syllable and its position out of hyphenated text.
    pub fn set_raw_text(&mut self, raw_text: &str) {
        let starts = raw_text.starts_with('-');
        let ends = raw_text.ends_with('-') && raw_text.len() > 1;
        let trimmed = raw_text.strip_prefix('-').unwrap_or(raw_text).to_string();
        let trimmed = if ends {
            trimmed.strip_suffix('-').unwrap_or(&trimmed).to_string()
        } else {
            trimmed
        };
        self.syllabic = match (starts, ends) {
            (true, true) => Syllabic::Middle,
            (true, false) => Syllabic::End,
            (false, true) => Syllabic::Begin,
            (false, false) => Syllabic::Single,
        };
        self.text = trimmed;
    }
}

impl fmt::Display for Lyric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tie_types_round_trip_and_reject_others() {
        for tie_type in TieType::ALL {
            assert_eq!(TieType::from_name(tie_type.as_str()).unwrap(), tie_type);
        }
        assert_eq!(Tie::default().tie_type(), TieType::Start);
        assert_eq!(Tie::from_name("stop").unwrap().to_string(), "Tie stop");
        let error = TieType::from_name("hello").unwrap_err().to_string();
        assert!(error.contains("Type must be one of"), "{error}");
        assert!(error.ends_with("not hello"), "{error}");
    }

    #[test]
    fn tie_carries_style_and_placement() {
        let mut tie = Tie::new(TieType::Continue);
        assert_eq!(tie.style(), TieStyle::Normal);
        assert_eq!(tie.placement(), None);
        tie.set_style(TieStyle::Dashed);
        tie.set_placement(Some(Placement::Above));
        assert_eq!(tie.style().as_str(), "dashed");
        assert_eq!(tie.placement().map(Placement::as_str), Some("above"));
        assert!(TieStyle::from_name("wavy").is_err());
        assert!(Placement::from_name("sideways").is_err());
    }

    #[test]
    fn notehead_and_stem_names_round_trip() {
        for notehead in Notehead::ALL {
            assert_eq!(Notehead::from_name(notehead.as_str()).unwrap(), notehead);
        }
        for direction in StemDirection::ALL {
            assert_eq!(
                StemDirection::from_name(direction.as_str()).unwrap(),
                direction
            );
        }
        assert_eq!(Notehead::default(), Notehead::Normal);
        // "none" is an input spelling of "noStem", never a state of its own.
        assert_eq!(
            StemDirection::from_name("none").unwrap(),
            StemDirection::NoStem
        );
        assert_eq!(StemDirection::NAMES.len(), StemDirection::ALL.len() + 1);
        assert_eq!(StemDirection::default(), StemDirection::Unspecified);
        assert_eq!(Notehead::Diamond.to_string(), "diamond");
        assert_eq!(StemDirection::NoStem.to_string(), "noStem");
        assert!(Notehead::from_name("blah").is_err());
        assert!(StemDirection::from_name("sideways").is_err());
    }

    #[test]
    fn lyric_hyphens_say_where_the_syllable_falls() {
        let cases = [
            ("hello", Syllabic::Single, "hello"),
            ("dic-", Syllabic::Begin, "dic"),
            ("-ci-", Syllabic::Middle, "ci"),
            ("-us", Syllabic::End, "us"),
        ];
        for (raw, syllabic, text) in cases {
            let lyric = Lyric::from_raw_text(raw);
            assert_eq!(lyric.syllabic(), syllabic, "{raw}");
            assert_eq!(lyric.text(), text, "{raw}");
            assert_eq!(lyric.raw_text(), raw, "{raw}");
        }
        let mut lyric = Lyric::new("shine");
        assert_eq!(lyric.number(), 1);
        assert_eq!(lyric.identifier(), "1");
        lyric.set_number(3).unwrap();
        assert_eq!(lyric.identifier(), "3");
        lyric.set_identifier(Some("chorus".to_string()));
        assert_eq!(lyric.identifier(), "chorus");
        assert!(lyric.set_number(0).is_err());
        assert_eq!(lyric.to_string(), "shine");
        assert_eq!(Syllabic::from_name("middle").unwrap(), Syllabic::Middle);
        assert!(Syllabic::from_name("half").is_err());
    }

    #[test]
    fn a_lone_hyphen_is_its_own_syllable() {
        // "-" is a beginning-of-word marker in music21, not an end marker
        // with empty text, so the trailing hyphen is only stripped when
        // something else is there to strip it from.
        let lyric = Lyric::from_raw_text("-");
        assert_eq!(lyric.syllabic(), Syllabic::End);
        assert_eq!(lyric.text(), "");
    }
}
