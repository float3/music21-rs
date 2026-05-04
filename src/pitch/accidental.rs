use super::Pitch;

use crate::defaults::{FloatType, IntegerType};
use crate::display::{DisplayLocation, DisplaySize, DisplayStyle, DisplayType};
use crate::error::{Error, Result};

use std::fmt::{Display, Formatter};
use std::str::FromStr;
use std::sync::Arc;

enum AccidentalEnum {
    Natural,
    HalfSharp,
    Sharp,
    OneAndAHalfSharp,
    DoubleSharp,
    TripleSharp,
    QuadrupleSharp,
    HalfFlat,
    Flat,
    OneAndAHalfFlat,
    DoubleFlat,
    TripleFlat,
    QuadrupleFlat,
}

impl AccidentalEnum {
    fn to_name(&self) -> &'static str {
        match self {
            AccidentalEnum::Natural => "natural",
            AccidentalEnum::HalfSharp => "half-sharp",
            AccidentalEnum::Sharp => "sharp",
            AccidentalEnum::OneAndAHalfSharp => "one-and-a-half-sharp",
            AccidentalEnum::DoubleSharp => "double-sharp",
            AccidentalEnum::TripleSharp => "triple-sharp",
            AccidentalEnum::QuadrupleSharp => "quadruple-sharp",
            AccidentalEnum::HalfFlat => "half-flat",
            AccidentalEnum::Flat => "flat",
            AccidentalEnum::OneAndAHalfFlat => "one-and-a-half-flat",
            AccidentalEnum::DoubleFlat => "double-flat",
            AccidentalEnum::TripleFlat => "triple-flat",
            AccidentalEnum::QuadrupleFlat => "quadruple-flat",
        }
    }

    fn from_name(s: &str) -> Option<Self> {
        match s {
            "natural" => Some(AccidentalEnum::Natural),
            "half-sharp" => Some(AccidentalEnum::HalfSharp),
            "sharp" => Some(AccidentalEnum::Sharp),
            "one-and-a-half-sharp" => Some(AccidentalEnum::OneAndAHalfSharp),
            "double-sharp" => Some(AccidentalEnum::DoubleSharp),
            "triple-sharp" => Some(AccidentalEnum::TripleSharp),
            "quadruple-sharp" => Some(AccidentalEnum::QuadrupleSharp),
            "half-flat" => Some(AccidentalEnum::HalfFlat),
            "flat" => Some(AccidentalEnum::Flat),
            "one-and-a-half-flat" => Some(AccidentalEnum::OneAndAHalfFlat),
            "double-flat" => Some(AccidentalEnum::DoubleFlat),
            "triple-flat" => Some(AccidentalEnum::TripleFlat),
            "quadruple-flat" => Some(AccidentalEnum::QuadrupleFlat),
            _ => None,
        }
    }

    fn to_alter(&self) -> FloatType {
        match self {
            AccidentalEnum::Natural => 0.0,
            AccidentalEnum::HalfSharp => 0.5,
            AccidentalEnum::Sharp => 1.0,
            AccidentalEnum::OneAndAHalfSharp => 1.5,
            AccidentalEnum::DoubleSharp => 2.0,
            AccidentalEnum::TripleSharp => 3.0,
            AccidentalEnum::QuadrupleSharp => 4.0,
            AccidentalEnum::HalfFlat => -0.5,
            AccidentalEnum::Flat => -1.0,
            AccidentalEnum::OneAndAHalfFlat => -1.5,
            AccidentalEnum::DoubleFlat => -2.0,
            AccidentalEnum::TripleFlat => -3.0,
            AccidentalEnum::QuadrupleFlat => -4.0,
        }
    }

    fn from_alter_str(s: &str) -> Option<Self> {
        match s {
            "-4.0" => Some(AccidentalEnum::QuadrupleFlat),
            "-3.0" => Some(AccidentalEnum::TripleFlat),
            "-2.0" => Some(AccidentalEnum::DoubleFlat),
            "-1.5" => Some(AccidentalEnum::OneAndAHalfFlat),
            "-1.0" => Some(AccidentalEnum::Flat),
            "-0.5" => Some(AccidentalEnum::HalfFlat),
            "0.0" => Some(AccidentalEnum::Natural),
            "0.5" => Some(AccidentalEnum::HalfSharp),
            "1.0" => Some(AccidentalEnum::Sharp),
            "1.5" => Some(AccidentalEnum::OneAndAHalfSharp),
            "2.0" => Some(AccidentalEnum::DoubleSharp),
            "3.0" => Some(AccidentalEnum::TripleSharp),
            "4.0" => Some(AccidentalEnum::QuadrupleSharp),
            _ => None,
        }
    }

    fn to_modifier(&self) -> &'static str {
        match self {
            AccidentalEnum::Natural => "",
            AccidentalEnum::HalfSharp => "~",
            AccidentalEnum::Sharp => "#",
            AccidentalEnum::OneAndAHalfSharp => "#~",
            AccidentalEnum::DoubleSharp => "##",
            AccidentalEnum::TripleSharp => "###",
            AccidentalEnum::QuadrupleSharp => "####",
            AccidentalEnum::HalfFlat => "`",
            AccidentalEnum::Flat => "-",
            AccidentalEnum::OneAndAHalfFlat => "-`",
            AccidentalEnum::DoubleFlat => "--",
            AccidentalEnum::TripleFlat => "---",
            AccidentalEnum::QuadrupleFlat => "----",
        }
    }

    fn from_modifier(s: &str) -> Option<Self> {
        match s {
            "" => Some(AccidentalEnum::Natural),
            "~" => Some(AccidentalEnum::HalfSharp),
            "#" => Some(AccidentalEnum::Sharp),
            "#~" => Some(AccidentalEnum::OneAndAHalfSharp),
            "##" => Some(AccidentalEnum::DoubleSharp),
            "###" => Some(AccidentalEnum::TripleSharp),
            "####" => Some(AccidentalEnum::QuadrupleSharp),
            "`" => Some(AccidentalEnum::HalfFlat),
            "-" => Some(AccidentalEnum::Flat),
            "-`" => Some(AccidentalEnum::OneAndAHalfFlat),
            "--" => Some(AccidentalEnum::DoubleFlat),
            "---" => Some(AccidentalEnum::TripleFlat),
            "----" => Some(AccidentalEnum::QuadrupleFlat),
            _ => None,
        }
    }

    const fn to_unicode(&self) -> &'static str {
        match self {
            AccidentalEnum::QuadrupleSharp => "\u{1d12a}\u{1d12a}",
            AccidentalEnum::TripleSharp => "\u{266f}\u{1d12a}",
            AccidentalEnum::DoubleSharp => "\u{1d12a}",
            AccidentalEnum::OneAndAHalfSharp => "\u{266f}\u{1d132}",
            AccidentalEnum::Sharp => "\u{266f}",
            AccidentalEnum::HalfSharp => "\u{1d132}",
            AccidentalEnum::QuadrupleFlat => "\u{1d12b}\u{1d12b}",
            AccidentalEnum::TripleFlat => "\u{266d}",
            AccidentalEnum::DoubleFlat => "\u{1d12b}",
            AccidentalEnum::OneAndAHalfFlat => "\u{266d}\u{1d132}",
            AccidentalEnum::Flat => "\u{266d}",
            AccidentalEnum::HalfFlat => "\u{1d132}",
            AccidentalEnum::Natural => "\u{266e}",
        }
    }

    #[allow(unreachable_patterns)]
    fn from_unicode(s: &str) -> Option<Self> {
        match s {
            "\u{1d12a}\u{1d12a}" => Some(AccidentalEnum::QuadrupleSharp),
            "\u{266f}\u{1d12a}" => Some(AccidentalEnum::TripleSharp),
            "\u{1d12a}" => Some(AccidentalEnum::DoubleSharp),
            "\u{266f}\u{1d132}" => Some(AccidentalEnum::OneAndAHalfSharp),
            "\u{266f}" => Some(AccidentalEnum::Sharp),
            "\u{1d12b}\u{1d12b}" => Some(AccidentalEnum::QuadrupleFlat),
            "\u{266d}" => Some(AccidentalEnum::Flat),
            "\u{1d12b}" => Some(AccidentalEnum::DoubleFlat),
            "\u{266d}\u{1d132}" => Some(AccidentalEnum::OneAndAHalfFlat),
            "\u{1d132}" => Some(AccidentalEnum::HalfSharp),
            "\u{266e}" => Some(AccidentalEnum::Natural),
            _ => None,
        }
    }

    fn from_alternate_name(s: &str) -> Option<Self> {
        match s {
            "n" => Some(AccidentalEnum::Natural),
            "is" => Some(AccidentalEnum::Sharp),
            "isis" => Some(AccidentalEnum::DoubleSharp),
            "isisis" => Some(AccidentalEnum::TripleSharp),
            "isisisis" => Some(AccidentalEnum::QuadrupleSharp),
            "ih" | "quarter-sharp" | "semisharp" => Some(AccidentalEnum::HalfSharp),
            "isih" | "three-quarter-sharp" | "three-quarters-sharp" | "sesquisharp" => {
                Some(AccidentalEnum::OneAndAHalfSharp)
            }
            "b" | "es" => Some(AccidentalEnum::Flat),
            "eses" => Some(AccidentalEnum::DoubleFlat),
            "eseses" => Some(AccidentalEnum::TripleFlat),
            "eseseses" => Some(AccidentalEnum::QuadrupleFlat),
            "eh" | "quarter-flat" | "semiflat" => Some(AccidentalEnum::HalfFlat),
            "eseh" | "three-quarter-flat" | "three-quarters-flat" | "sesquiflat" => {
                Some(AccidentalEnum::OneAndAHalfFlat)
            }
            _ => None,
        }
    }

    fn from_float(f: FloatType) -> Option<Self> {
        match f {
            -4.0 => Some(AccidentalEnum::QuadrupleFlat),
            -3.0 => Some(AccidentalEnum::TripleFlat),
            -2.0 => Some(AccidentalEnum::DoubleFlat),
            -1.5 => Some(AccidentalEnum::OneAndAHalfFlat),
            -1.0 => Some(AccidentalEnum::Flat),
            -0.5 => Some(AccidentalEnum::HalfFlat),
            0.0 => Some(AccidentalEnum::Natural),
            0.5 => Some(AccidentalEnum::HalfSharp),
            1.0 => Some(AccidentalEnum::Sharp),
            1.5 => Some(AccidentalEnum::OneAndAHalfSharp),
            2.0 => Some(AccidentalEnum::DoubleSharp),
            3.0 => Some(AccidentalEnum::TripleSharp),
            4.0 => Some(AccidentalEnum::QuadrupleSharp),
            _ => None,
        }
    }

    fn from_string(s: &str) -> Option<Self> {
        AccidentalEnum::from_name(s)
            .or_else(|| AccidentalEnum::from_modifier(s))
            .or_else(|| AccidentalEnum::from_alter_str(s))
            .or_else(|| AccidentalEnum::from_unicode(s))
            .or_else(|| AccidentalEnum::from_alternate_name(s))
    }
}

/// Input accepted by [`Accidental::new`] and [`Accidental::set`].
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AccidentalSpecifier {
    /// An accidental name, modifier, alternate name, or unicode accidental.
    Name(String),
    /// A semitone alteration such as `1.0` for sharp or `-0.5` for half-flat.
    Alter(FloatType),
    /// An existing accidental to clone.
    Accidental(Accidental),
}

impl From<&str> for AccidentalSpecifier {
    fn from(value: &str) -> Self {
        Self::Name(value.to_string())
    }
}

impl From<String> for AccidentalSpecifier {
    fn from(value: String) -> Self {
        Self::Name(value)
    }
}

impl From<i8> for AccidentalSpecifier {
    fn from(value: i8) -> Self {
        Self::Alter(value as FloatType)
    }
}

impl From<IntegerType> for AccidentalSpecifier {
    fn from(value: IntegerType) -> Self {
        Self::Alter(value as FloatType)
    }
}

impl From<FloatType> for AccidentalSpecifier {
    fn from(value: FloatType) -> Self {
        Self::Alter(value)
    }
}

impl From<Accidental> for AccidentalSpecifier {
    fn from(value: Accidental) -> Self {
        Self::Accidental(value)
    }
}

impl Display for AccidentalSpecifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Name(name) => write!(f, "{name}"),
            Self::Alter(alter) => write!(f, "{alter}"),
            Self::Accidental(accidental) => write!(f, "{accidental}"),
        }
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Symbolic and numerical accidental information for a pitch.
///
/// This mirrors the main behavior of Python music21's
/// `music21.pitch.Accidental`: a standard accidental has a `name`, `modifier`,
/// and semitone `alter`; names compare by spelling while ordering uses `alter`.
pub struct Accidental {
    _display_type: DisplayType,
    _display_status: Option<bool>,
    display_style: DisplayStyle,
    display_size: DisplaySize,
    display_location: DisplayLocation,
    #[cfg_attr(feature = "serde", serde(skip))]
    _client: Option<Arc<Pitch>>,
    _name: String,
    _modifier: String,
    pub(crate) _alter: FloatType,
}

impl PartialEq for Accidental {
    fn eq(&self, other: &Self) -> bool {
        self._name == other._name
    }
}

impl PartialOrd for Accidental {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self._alter.partial_cmp(&other._alter)
    }
}

impl FromStr for Accidental {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<&str> for Accidental {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<String> for Accidental {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<IntegerType> for Accidental {
    type Error = Error;

    fn try_from(value: IntegerType) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<i8> for Accidental {
    type Error = Error;

    fn try_from(value: i8) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<FloatType> for Accidental {
    type Error = Error;

    fn try_from(value: FloatType) -> Result<Self> {
        Self::new(value)
    }
}

impl Accidental {
    /// Creates an accidental from a name, modifier, alternate name, unicode
    /// symbol, semitone alteration, or another accidental.
    pub fn new(specifier: impl Into<AccidentalSpecifier>) -> Result<Self> {
        let specifier = specifier.into();
        if let AccidentalSpecifier::Accidental(accidental) = specifier {
            return Ok(accidental);
        }

        let mut acci = Self {
            _display_type: DisplayType::Normal,
            _display_status: None,
            display_style: DisplayStyle::Normal,
            display_size: DisplaySize::Full,
            display_location: DisplayLocation::Normal,
            _client: None,
            _name: "".to_string(),
            _modifier: "".to_string(),
            _alter: 0.0,
        };

        acci.set_specifier(specifier, false)?;
        Ok(acci)
    }

    /// Returns the standard accidental names known to music21.
    pub fn list_names() -> Vec<&'static str> {
        let mut names = [
            "double-flat",
            "double-sharp",
            "flat",
            "half-flat",
            "half-sharp",
            "natural",
            "one-and-a-half-flat",
            "one-and-a-half-sharp",
            "quadruple-flat",
            "quadruple-sharp",
            "sharp",
            "triple-flat",
            "triple-sharp",
        ];
        names.sort_unstable();
        names.to_vec()
    }

    /// Returns whether `name` is a supported accidental name, modifier, or
    /// alternate music21/LilyPond-style accidental name.
    pub fn is_valid_name(name: &str) -> bool {
        AccidentalEnum::from_string(&name.to_lowercase()).is_some()
    }

    /// Converts a supported accidental spelling to its standard music21 name.
    pub fn standardize_name(name: &str) -> Result<String> {
        AccidentalEnum::from_string(&name.to_lowercase())
            .map(|accidental| accidental.to_name().to_string())
            .ok_or_else(|| {
                Error::Accidental(format!("{name:?} is not a supported accidental type"))
            })
    }

    /// Changes this accidental to a supported accidental.
    pub fn set(&mut self, specifier: impl Into<AccidentalSpecifier>) -> Result<()> {
        self.set_specifier(specifier.into(), false)
    }

    /// Changes this accidental, preserving non-standard names or alteration
    /// values in the same way Python music21's `allowNonStandardValue=True`
    /// does.
    pub fn set_allowing_non_standard_value(
        &mut self,
        specifier: impl Into<AccidentalSpecifier>,
    ) -> Result<()> {
        self.set_specifier(specifier.into(), true)
    }

    fn set_specifier(
        &mut self,
        specifier: AccidentalSpecifier,
        allow_non_standard_value: bool,
    ) -> Result<()> {
        if let AccidentalSpecifier::Accidental(accidental) = specifier {
            *self = accidental;
            self.inform_client();
            return Ok(());
        }

        if let Some(accidental) = Self::specifier_to_standard(&specifier) {
            self._name = accidental.to_name().to_string();
            self._alter = accidental.to_alter();
            self._modifier = accidental.to_modifier().to_string();
            self.inform_client();
            return Ok(());
        }

        if !allow_non_standard_value {
            return Err(Error::Accidental(format!(
                "{specifier} is not a supported accidental type"
            )));
        }

        match specifier {
            AccidentalSpecifier::Name(name) => self._name = name.to_lowercase(),
            AccidentalSpecifier::Alter(alter) => self._alter = alter,
            AccidentalSpecifier::Accidental(_) => unreachable!(),
        }
        self.inform_client();
        Ok(())
    }

    fn specifier_to_standard(specifier: &AccidentalSpecifier) -> Option<AccidentalEnum> {
        match specifier {
            AccidentalSpecifier::Name(name) => AccidentalEnum::from_string(&name.to_lowercase()),
            AccidentalSpecifier::Alter(alter) => AccidentalEnum::from_float(*alter),
            AccidentalSpecifier::Accidental(accidental) => {
                AccidentalEnum::from_name(accidental.name())
            }
        }
    }

    fn inform_client(&self) {
        if let Some(client) = &self._client {
            client.inform_client();
        }
    }

    /// Returns the standard or non-standard accidental name.
    pub fn name(&self) -> &str {
        &self._name
    }

    /// Sets the accidental name. Standard names update `alter` and `modifier`;
    /// non-standard names are preserved without changing them.
    pub fn set_name(&mut self, name: impl Into<String>) -> Result<()> {
        self.set_allowing_non_standard_value(name.into())
    }

    /// Returns the semitone alteration from the natural step.
    pub fn alter(&self) -> FloatType {
        self._alter
    }

    /// Sets the semitone alteration. Standard values update `name` and
    /// `modifier`; non-standard values are preserved without changing them.
    pub fn set_alter(&mut self, alter: FloatType) -> Result<()> {
        self.set_allowing_non_standard_value(alter)
    }

    /// Returns the music21 modifier string, such as `#`, `-`, `##`, or `~`.
    pub fn modifier(&self) -> &str {
        &self._modifier
    }

    /// Sets the modifier. Unknown modifiers are preserved without changing the
    /// name or alteration, matching Python music21.
    pub fn set_modifier(&mut self, modifier: impl Into<String>) {
        let modifier = modifier.into();
        if let Some(accidental) = AccidentalEnum::from_modifier(&modifier) {
            self._name = accidental.to_name().to_string();
            self._alter = accidental.to_alter();
            self._modifier = accidental.to_modifier().to_string();
        } else {
            self._modifier = modifier;
        }
        self.inform_client();
    }

    /// Returns a unicode representation of the accidental.
    pub fn unicode(&self) -> String {
        AccidentalEnum::from_modifier(&self._modifier)
            .map(|accidental| accidental.to_unicode().to_string())
            .unwrap_or_else(|| self._modifier.clone())
    }

    /// Returns the most complete accidental name. This is currently the same as
    /// [`Self::name`], like Python music21.
    pub fn full_name(&self) -> &str {
        self.name()
    }

    /// Returns whether this accidental describes a twelve-tone alteration.
    pub fn is_twelve_tone(&self) -> bool {
        !matches!(
            self._name.as_str(),
            "half-sharp" | "one-and-a-half-sharp" | "half-flat" | "one-and-a-half-flat"
        )
    }

    /// Sets `name` without updating `alter` or `modifier`.
    pub fn set_name_independently(&mut self, name: impl Into<String>) {
        self._name = name.into();
        self.inform_client();
    }

    /// Sets `alter` without updating `name` or `modifier`.
    pub fn set_alter_independently(&mut self, alter: FloatType) {
        self._alter = alter;
        self.inform_client();
    }

    /// Sets `modifier` without updating `name` or `alter`.
    pub fn set_modifier_independently(&mut self, modifier: impl Into<String>) {
        self._modifier = modifier.into();
        self.inform_client();
    }

    /// Copies display-related settings from another accidental.
    pub fn inherit_display(&mut self, other: &Accidental) {
        self._display_type = other._display_type.clone();
        self._display_status = other._display_status;
        self.display_style = other.display_style.clone();
        self.display_size = other.display_size.clone();
        self.display_location = other.display_location.clone();
        self.inform_client();
    }

    /// Returns the accidental display type.
    pub fn display_type(&self) -> &'static str {
        display_type_to_str(&self._display_type)
    }

    /// Sets the accidental display type.
    pub fn set_display_type(&mut self, value: &str) -> Result<()> {
        self._display_type = display_type_from_str(value).ok_or_else(|| {
            Error::Accidental(format!("Supplied display type is not supported: {value:?}"))
        })?;
        self.inform_client();
        Ok(())
    }

    /// Returns whether notation processing has decided to display this
    /// accidental. `None` means no decision has been made.
    pub fn display_status(&self) -> Option<bool> {
        self._display_status
    }

    /// Sets the display status.
    pub fn set_display_status(&mut self, value: Option<bool>) {
        self._display_status = value;
        self.inform_client();
    }

    /// Returns the display style.
    pub fn display_style(&self) -> &'static str {
        display_style_to_str(&self.display_style)
    }

    /// Sets the display style.
    pub fn set_display_style(&mut self, value: &str) -> Result<()> {
        self.display_style = display_style_from_str(value).ok_or_else(|| {
            Error::Accidental(format!(
                "Supplied display style is not supported: {value:?}"
            ))
        })?;
        self.inform_client();
        Ok(())
    }

    /// Returns the display size.
    pub fn display_size(&self) -> String {
        display_size_to_string(&self.display_size)
    }

    /// Sets the display size.
    pub fn set_display_size(&mut self, value: &str) -> Result<()> {
        self.display_size = display_size_from_str(value)?;
        self.inform_client();
        Ok(())
    }

    /// Returns the display location.
    pub fn display_location(&self) -> &'static str {
        display_location_to_str(&self.display_location)
    }

    /// Sets the display location.
    pub fn set_display_location(&mut self, value: &str) -> Result<()> {
        self.display_location = display_location_from_str(value).ok_or_else(|| {
            Error::Accidental(format!(
                "Supplied display location is not supported: {value:?}"
            ))
        })?;
        self.inform_client();
        Ok(())
    }

    /// Returns a natural accidental.
    pub fn natural() -> Accidental {
        let x = Accidental::new("natural");
        assert!(x.is_ok());
        match x {
            Ok(val) => val,
            Err(err) => panic!("creating a natural Accidental should never fail: {err}"),
        }
    }

    /// Returns a flat accidental.
    pub fn flat() -> Accidental {
        let x = Accidental::new("flat");
        assert!(x.is_ok());
        match x {
            Ok(val) => val,
            Err(err) => panic!("creating a flat Accidental should never fail: {err}"),
        }
    }

    /// Returns a sharp accidental.
    pub fn sharp() -> Accidental {
        let x = Accidental::new("sharp");
        assert!(x.is_ok());
        match x {
            Ok(val) => val,
            Err(err) => panic!("creating a sharp Accidental should never fail: {err}"),
        }
    }
}

fn display_type_to_str(value: &DisplayType) -> &'static str {
    match value {
        DisplayType::Normal => "normal",
        DisplayType::Always => "always",
        DisplayType::Never => "never",
        DisplayType::UnlessRepeated => "unless-repeated",
        DisplayType::EvenTied => "even-tied",
        DisplayType::IfAbsolutelyNecessary => "if-absolutely-necessary",
    }
}

fn display_type_from_str(value: &str) -> Option<DisplayType> {
    match value {
        "normal" => Some(DisplayType::Normal),
        "always" => Some(DisplayType::Always),
        "never" => Some(DisplayType::Never),
        "unless-repeated" => Some(DisplayType::UnlessRepeated),
        "even-tied" => Some(DisplayType::EvenTied),
        "if-absolutely-necessary" => Some(DisplayType::IfAbsolutelyNecessary),
        _ => None,
    }
}

fn display_style_to_str(value: &DisplayStyle) -> &'static str {
    match value {
        DisplayStyle::Normal => "normal",
        DisplayStyle::Parentheses => "parentheses",
        DisplayStyle::Bracket => "bracket",
        DisplayStyle::Both => "both",
    }
}

fn display_style_from_str(value: &str) -> Option<DisplayStyle> {
    match value {
        "normal" => Some(DisplayStyle::Normal),
        "parentheses" => Some(DisplayStyle::Parentheses),
        "bracket" => Some(DisplayStyle::Bracket),
        "both" => Some(DisplayStyle::Both),
        _ => None,
    }
}

fn display_size_to_string(value: &DisplaySize) -> String {
    match value {
        DisplaySize::Full => "full".to_string(),
        DisplaySize::Cue => "cue".to_string(),
        DisplaySize::Large => "large".to_string(),
        DisplaySize::Percentage(percentage) => percentage.to_string(),
    }
}

fn display_size_from_str(value: &str) -> Result<DisplaySize> {
    match value {
        "full" => Ok(DisplaySize::Full),
        "cue" => Ok(DisplaySize::Cue),
        "large" => Ok(DisplaySize::Large),
        _ => value
            .parse::<FloatType>()
            .map(DisplaySize::Percentage)
            .map_err(|_| {
                Error::Accidental(format!("Supplied display size is not supported: {value:?}"))
            }),
    }
}

fn display_location_to_str(value: &DisplayLocation) -> &'static str {
    match value {
        DisplayLocation::Normal => "normal",
        DisplayLocation::Above => "above",
        DisplayLocation::Ficta => "ficta",
        DisplayLocation::Below => "below",
    }
}

fn display_location_from_str(value: &str) -> Option<DisplayLocation> {
    match value {
        "normal" => Some(DisplayLocation::Normal),
        "above" => Some(DisplayLocation::Above),
        "ficta" => Some(DisplayLocation::Ficta),
        "below" => Some(DisplayLocation::Below),
        _ => None,
    }
}

impl Default for Accidental {
    fn default() -> Self {
        Self::natural()
    }
}

impl std::fmt::Display for Accidental {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self._name)
    }
}

pub(crate) trait IntoAccidental: Display + Clone {
    fn is_accidental(&self) -> bool;
    fn into_accidental(self) -> Result<Accidental>;
    fn accidental(self) -> Accidental;
}

impl IntoAccidental for i8 {
    fn is_accidental(&self) -> bool {
        false
    }

    fn into_accidental(self) -> Result<Accidental> {
        Accidental::new(self)
    }

    fn accidental(self) -> Accidental {
        panic!("call into_accidental instead")
    }
}

impl IntoAccidental for IntegerType {
    fn is_accidental(&self) -> bool {
        false
    }

    fn into_accidental(self) -> Result<Accidental> {
        Accidental::new(self)
    }

    fn accidental(self) -> Accidental {
        panic!("call into_accidental instead")
    }
}

impl IntoAccidental for FloatType {
    fn is_accidental(&self) -> bool {
        false
    }

    fn into_accidental(self) -> Result<Accidental> {
        Accidental::new(self)
    }

    fn accidental(self) -> Accidental {
        panic!("call into_accidental instead")
    }
}

impl IntoAccidental for &str {
    fn is_accidental(&self) -> bool {
        false
    }

    fn into_accidental(self) -> Result<Accidental> {
        Accidental::new(self)
    }

    fn accidental(self) -> Accidental {
        panic!("call into_accidental instead")
    }
}

impl IntoAccidental for String {
    fn is_accidental(&self) -> bool {
        false
    }

    fn into_accidental(self) -> Result<Accidental> {
        Accidental::new(self)
    }

    fn accidental(self) -> Accidental {
        panic!("call into_accidental instead")
    }
}

impl IntoAccidental for Accidental {
    fn is_accidental(&self) -> bool {
        true
    }

    fn into_accidental(self) -> Result<Accidental> {
        panic!("don't call into_accidental on an accidental");
    }

    fn accidental(self) -> Accidental {
        self
    }
}

impl IntoAccidental for AccidentalSpecifier {
    fn is_accidental(&self) -> bool {
        matches!(self, AccidentalSpecifier::Accidental(_))
    }

    fn into_accidental(self) -> Result<Accidental> {
        Accidental::new(self)
    }

    fn accidental(self) -> Accidental {
        match self {
            AccidentalSpecifier::Accidental(accidental) => accidental,
            _ => panic!("call into_accidental instead"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Accidental, AccidentalSpecifier, IntoAccidental};

    #[test]
    fn test_natural() {
        let acc = Accidental::natural();
        assert_eq!(acc._name, "natural");
        assert_eq!(acc._alter, 0.0);
        assert_eq!(acc.modifier(), "");
    }

    #[test]
    fn test_sharp() {
        let acc = Accidental::sharp();
        assert_eq!(acc._name, "sharp");
        assert_eq!(acc._alter, 1.0);
        assert_eq!(acc.modifier(), "#");
    }

    #[test]
    fn test_flat() {
        let acc = Accidental::flat();
        assert_eq!(acc._name, "flat");
        assert_eq!(acc._alter, -1.0);
        assert_eq!(acc.modifier(), "-");
    }

    #[test]
    fn test_creation_from_int() {
        let acc_sharp = 1.into_accidental().unwrap();
        assert_eq!(acc_sharp._name, "sharp");
        assert_eq!(acc_sharp._alter, 1.0);

        let acc_flat = (-1).into_accidental().unwrap();
        assert_eq!(acc_flat._name, "flat");
        assert_eq!(acc_flat._alter, -1.0);

        let acc_natural = 0.into_accidental().unwrap();
        assert_eq!(acc_natural._name, "natural");
        assert_eq!(acc_natural._alter, 0.0);
    }

    #[test]
    fn accidental_supports_rust_conversion_traits() {
        let parsed: Accidental = "#".parse().unwrap();
        assert_eq!(parsed.name(), "sharp");

        let from_str = Accidental::try_from("flat").unwrap();
        assert_eq!(from_str.modifier(), "-");

        let from_alter = Accidental::try_from(-0.5).unwrap();
        assert_eq!(from_alter.name(), "half-flat");
    }

    #[test]
    fn test_creation_from_float() {
        let acc_double_sharp: Accidental = 2.0.into_accidental().unwrap();
        assert_eq!(acc_double_sharp._name, "double-sharp");
        assert_eq!(acc_double_sharp._alter, 2.0);

        let acc_half_flat: Accidental = (-0.5).into_accidental().unwrap();
        assert_eq!(acc_half_flat._name, "half-flat");
        assert_eq!(acc_half_flat._alter, -0.5);
    }

    #[test]
    fn test_creation_from_str() {
        let acc1: Accidental = <&str>::into_accidental("sharp").unwrap();
        assert_eq!(acc1._name, "sharp");
        assert_eq!(acc1._alter, 1.0);

        // Case insensitivity: "Flat" should be accepted as "flat"
        let acc2: Accidental = <&str>::into_accidental("Flat").unwrap();
        assert_eq!(acc2._name, "flat");
        assert_eq!(acc2._alter, -1.0);
    }

    #[test]
    fn test_creation_from_string() {
        let acc: Accidental = String::into_accidental("double-flat".to_string()).unwrap();
        assert_eq!(acc._name, "double-flat");
        assert_eq!(acc._alter, -2.0);
    }

    #[test]
    fn test_invalid_accidental() {
        let result = Accidental::new("invalid");
        assert!(
            result.is_err(),
            "An invalid accidental should return an error"
        );
    }

    #[test]
    fn test_display_trait() {
        let acc = Accidental::sharp();
        assert_eq!(format!("{acc}"), "sharp");
    }

    #[test]
    fn test_equality() {
        let acc1 = Accidental::sharp();
        let acc2 = Accidental::sharp();
        let acc3 = Accidental::flat();
        assert_eq!(acc1, acc2);
        assert_ne!(acc1, acc3);
    }

    #[test]
    fn test_alternate_names() {
        // Using alternate names from AccidentalEnum::from_alternate_name
        let acc_sharp: Accidental = String::into_accidental("is".to_string()).unwrap();
        assert_eq!(acc_sharp._name, "sharp");
        assert_eq!(acc_sharp._alter, 1.0);

        let acc_double_sharp: Accidental = String::into_accidental("isis".to_string()).unwrap();
        assert_eq!(acc_double_sharp._name, "double-sharp");
        assert_eq!(acc_double_sharp._alter, 2.0);

        let acc_triple_sharp: Accidental = String::into_accidental("isisis".to_string()).unwrap();
        assert_eq!(acc_triple_sharp._name, "triple-sharp");
        assert_eq!(acc_triple_sharp._alter, 3.0);

        let acc_double_flat: Accidental = String::into_accidental("eses".to_string()).unwrap();
        assert_eq!(acc_double_flat._name, "double-flat");
        assert_eq!(acc_double_flat._alter, -2.0);
    }

    #[test]
    fn public_api_matches_music21_accidental_basics() {
        let mut accidental = Accidental::new("sharp").unwrap();
        assert_eq!(accidental.name(), "sharp");
        assert_eq!(accidental.alter(), 1.0);
        assert_eq!(accidental.modifier(), "#");
        assert_eq!(accidental.unicode(), "\u{266f}");
        assert_eq!(accidental.full_name(), "sharp");
        assert!(accidental.is_twelve_tone());

        accidental.set("--").unwrap();
        assert_eq!(accidental.name(), "double-flat");
        assert_eq!(accidental.alter(), -2.0);
        assert_eq!(accidental.modifier(), "--");

        accidental.set("quarter-sharp").unwrap();
        assert_eq!(accidental.name(), "half-sharp");
        assert_eq!(accidental.alter(), 0.5);
        assert!(!accidental.is_twelve_tone());
    }

    #[test]
    fn public_api_preserves_non_standard_values_when_requested() {
        let mut accidental = Accidental::new("flat").unwrap();
        accidental
            .set_allowing_non_standard_value("flat-flat-up")
            .unwrap();
        assert_eq!(accidental.name(), "flat-flat-up");
        assert_eq!(accidental.alter(), -1.0);
        assert_eq!(accidental.modifier(), "-");

        accidental.set_alter(-0.9).unwrap();
        assert_eq!(accidental.name(), "flat-flat-up");
        assert_eq!(accidental.alter(), -0.9);
        assert_eq!(accidental.modifier(), "-");

        accidental.set_modifier("&");
        assert_eq!(accidental.name(), "flat-flat-up");
        assert_eq!(accidental.alter(), -0.9);
        assert_eq!(accidental.modifier(), "&");
    }

    #[test]
    fn public_api_supports_display_metadata() {
        let mut source = Accidental::new("double-flat").unwrap();
        source.set_display_type("always").unwrap();
        source.set_display_status(Some(true));
        source.set_display_style("parentheses").unwrap();
        source.set_display_size("cue").unwrap();
        source.set_display_location("above").unwrap();

        let mut target = Accidental::new("sharp").unwrap();
        target.inherit_display(&source);
        assert_eq!(target.display_type(), "always");
        assert_eq!(target.display_status(), Some(true));
        assert_eq!(target.display_style(), "parentheses");
        assert_eq!(target.display_size(), "cue");
        assert_eq!(target.display_location(), "above");
    }

    #[test]
    fn public_api_covers_name_helpers_and_standardization_errors() {
        let names = Accidental::list_names();
        assert_eq!(names.first(), Some(&"double-flat"));
        assert!(names.contains(&"quadruple-sharp"));

        assert!(Accidental::is_valid_name("quarter-sharp"));
        assert!(Accidental::is_valid_name("es"));
        assert!(!Accidental::is_valid_name("not-an-accidental"));

        assert_eq!(Accidental::standardize_name("##").unwrap(), "double-sharp");
        let err = Accidental::standardize_name("not-an-accidental").unwrap_err();
        assert!(err.to_string().contains("not a supported accidental type"));
    }

    #[test]
    fn public_api_covers_setter_branches_and_independent_updates() {
        let mut accidental = Accidental::default();
        assert_eq!(accidental.name(), "natural");

        accidental.set(Accidental::flat()).unwrap();
        assert_eq!(accidental.name(), "flat");
        assert_eq!(accidental.unicode(), "\u{266d}");

        accidental.set_name("sharp").unwrap();
        assert_eq!(accidental.alter(), 1.0);
        assert_eq!(accidental.modifier(), "#");

        accidental.set_modifier("~~");
        assert_eq!(accidental.name(), "sharp");
        assert_eq!(accidental.modifier(), "~~");
        assert_eq!(accidental.unicode(), "~~");

        accidental.set_name_independently("custom");
        accidental.set_alter_independently(0.25);
        accidental.set_modifier_independently("^");
        assert_eq!(accidental.full_name(), "custom");
        assert_eq!(accidental.alter(), 0.25);
        assert_eq!(accidental.modifier(), "^");

        let cloned = Accidental::new(AccidentalSpecifier::from(accidental.clone())).unwrap();
        assert_eq!(cloned, accidental);
    }

    #[test]
    fn public_api_rejects_invalid_display_metadata() {
        let mut accidental = Accidental::new("natural").unwrap();
        assert!(accidental.set_display_type("sometimes").is_err());
        assert!(accidental.set_display_style("curly").is_err());
        assert!(accidental.set_display_size("huge").is_err());
        assert!(accidental.set_display_location("beside").is_err());

        accidental.set_display_type("never").unwrap();
        accidental.set_display_style("both").unwrap();
        accidental.set_display_size("125.5").unwrap();
        accidental.set_display_location("below").unwrap();

        assert_eq!(accidental.display_type(), "never");
        assert_eq!(accidental.display_style(), "both");
        assert_eq!(accidental.display_size(), "125.5");
        assert_eq!(accidental.display_location(), "below");
    }

    #[test]
    fn accidental_ordering_follows_alter() {
        assert!(Accidental::flat() < Accidental::natural());
        assert!(Accidental::sharp() > Accidental::natural());
    }
}
