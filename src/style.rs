//! How an object is drawn: music21's `style` objects, as far as the crate
//! keeps them.
//!
//! A [`TextStyle`] says where a piece of text stands, in what font and
//! colour, and how it is aligned and justified, as MusicXML's print-style,
//! alignment and justify attributes say it.

use crate::defaults::FloatType;
use crate::error::{Error, Result};

/// A value of a style as music21 keeps the attribute it read: a whole
/// number where the text is one, to within a millionth, a number with a
/// fraction where it is that, and the text itself where it is no number --
/// music21's `numToIntOrFloat`, which a MusicXML font size written as a CSS
/// size, `medium`, passes over.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum StyleValue {
    /// A whole number.
    Whole(i64),
    /// A number with a fraction.
    Decimal(FloatType),
    /// Text that is no number.
    Text(String),
}

impl StyleValue {
    /// The value an attribute's text gives.
    pub fn read(text: &str) -> Self {
        let Ok(value) = text.trim().parse::<FloatType>() else {
            return Self::Text(text.to_string());
        };
        let whole = value.round();
        if value.is_finite() && (whole - value).abs() <= 1e-6 && whole.abs() < 9.0e15 {
            Self::Whole(whole as i64)
        } else {
            Self::Decimal(value)
        }
    }

    /// The value as a number, where it is one.
    pub fn number(&self) -> Option<FloatType> {
        match self {
            Self::Whole(whole) => Some(*whole as FloatType),
            Self::Decimal(value) => Some(*value),
            Self::Text(_) => None,
        }
    }
}

impl std::fmt::Display for StyleValue {
    /// As Python writes the `int`, `float` or string.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Whole(whole) => write!(f, "{whole}"),
            Self::Decimal(value) => write!(f, "{value}"),
            Self::Text(text) => f.write_str(text),
        }
    }
}

fn format_error(message: impl Into<String>) -> Error {
    Error::Text(message.into())
}

/// Where a piece of text stands and how it is drawn: music21's
/// `TextStyle`, the part of it MusicXML's print style, alignment and
/// justification say.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TextStyle {
    /// Across from where it is placed, in tenths: `absoluteX`,
    /// MusicXML's `default-x`.
    pub absolute_x: Option<StyleValue>,
    /// Up from where it is placed: `absoluteY`, `default-y`.
    pub absolute_y: Option<StyleValue>,
    /// Across from its default place: `relativeX`, `relative-x`.
    pub relative_x: Option<StyleValue>,
    /// Up from its default place: `relativeY`, `relative-y`.
    pub relative_y: Option<StyleValue>,
    /// The fonts it may be set in, first choice first: `fontFamily`.
    pub font_family: Option<Vec<String>>,
    font_style: Option<String>,
    /// Its size, in points or as a CSS size: `fontSize`.
    pub font_size: Option<StyleValue>,
    font_weight: Option<String>,
    /// Its colour, as written: `color`.
    pub color: Option<String>,
    justify: Option<String>,
    align_horizontal: Option<String>,
    align_vertical: Option<String>,
}

impl TextStyle {
    /// `italic`, `normal`, `bold` or `bolditalic`: music21's `fontStyle`.
    pub fn font_style(&self) -> Option<&str> {
        self.font_style.as_deref()
    }

    /// Sets the font style, read without regard to case.
    ///
    /// # Errors
    ///
    /// A style music21 does not name.
    pub fn set_font_style(&mut self, value: Option<&str>) -> Result<()> {
        self.font_style = checked(
            value,
            &["italic", "normal", "bold", "bolditalic"],
            true,
            |value| format!("Not a supported fontStyle: {value:?}"),
        )?;
        Ok(())
    }

    /// `normal` or `bold`: music21's `fontWeight`.
    pub fn font_weight(&self) -> Option<&str> {
        self.font_weight.as_deref()
    }

    /// Sets the font weight, read without regard to case.
    ///
    /// # Errors
    ///
    /// A weight music21 does not name.
    pub fn set_font_weight(&mut self, value: Option<&str>) -> Result<()> {
        self.font_weight = checked(value, &["normal", "bold"], true, |value| {
            format!("Not a supported fontWeight: {value}")
        })?;
        Ok(())
    }

    /// `left`, `center`, `right` or `full`: music21's `justify`.
    pub fn justify(&self) -> Option<&str> {
        self.justify.as_deref()
    }

    /// Sets the justification, read without regard to case.
    ///
    /// # Errors
    ///
    /// A justification music21 does not name.
    pub fn set_justify(&mut self, value: Option<&str>) -> Result<()> {
        self.justify = checked(value, &["left", "center", "right", "full"], true, |value| {
            format!("Not a supported justification: {value:?}")
        })?;
        Ok(())
    }

    /// `left`, `right` or `center`: music21's `alignHorizontal`, MusicXML's
    /// `halign`.
    pub fn align_horizontal(&self) -> Option<&str> {
        self.align_horizontal.as_deref()
    }

    /// Sets the horizontal alignment.
    ///
    /// # Errors
    ///
    /// An alignment music21 does not name.
    pub fn set_align_horizontal(&mut self, value: Option<&str>) -> Result<()> {
        self.align_horizontal = checked(value, &["left", "right", "center"], false, |value| {
            format!("Invalid horizontal align: {value:?}")
        })?;
        Ok(())
    }

    /// `top`, `middle`, `bottom` or `baseline`: music21's `alignVertical`,
    /// MusicXML's `valign`.
    pub fn align_vertical(&self) -> Option<&str> {
        self.align_vertical.as_deref()
    }

    /// Sets the vertical alignment.
    ///
    /// # Errors
    ///
    /// An alignment music21 does not name.
    pub fn set_align_vertical(&mut self, value: Option<&str>) -> Result<()> {
        self.align_vertical = checked(
            value,
            &["top", "middle", "bottom", "baseline"],
            false,
            |value| format!("Invalid vertical align: {value:?}"),
        )?;
        Ok(())
    }
}

/// A value checked against the ones music21 accepts, lowercased first
/// where music21 lowercases it.
fn checked(
    value: Option<&str>,
    allowed: &[&str],
    lowercase: bool,
    message: impl Fn(&str) -> String,
) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let read = if lowercase {
        value.to_lowercase()
    } else {
        value.to_string()
    };
    if allowed.contains(&read.as_str()) {
        Ok(Some(read))
    } else {
        Err(format_error(message(value)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_is_read_as_music21_reads_it() {
        assert_eq!(StyleValue::read("12"), StyleValue::Whole(12));
        assert_eq!(StyleValue::read("12.0000001"), StyleValue::Whole(12));
        assert_eq!(StyleValue::read("12.5"), StyleValue::Decimal(12.5));
        assert_eq!(
            StyleValue::read("medium"),
            StyleValue::Text("medium".to_string())
        );
        assert_eq!(StyleValue::Decimal(12.5).to_string(), "12.5");
    }

    #[test]
    fn styles_music21_does_not_name_are_refused() {
        let mut style = TextStyle::default();
        style.set_justify(Some("Center")).unwrap();
        assert_eq!(style.justify(), Some("center"));
        assert!(style.set_justify(Some("middle")).is_err());
        assert!(style.set_align_horizontal(Some("Center")).is_err());
        style.set_align_vertical(Some("baseline")).unwrap();
        assert!(style.set_font_style(Some("oblique")).is_err());
        style.set_font_weight(Some("BOLD")).unwrap();
        assert_eq!(style.font_weight(), Some("bold"));
    }
}
