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
            // Python writes a whole float with its point.
            Self::Decimal(value) if value.is_finite() && value.fract() == 0.0 => {
                write!(f, "{value:.1}")
            }
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

/// How a whole score is drawn, past what its layouts say: music21's
/// `StreamStyle`, the part MusicXML's `<defaults>` writes.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StreamStyle {
    /// Line widths by what they draw, in tenths: `lineWidths`.
    pub line_widths: Vec<(String, StyleValue)>,
    /// Note sizes by kind, as percentages: `noteSizes`.
    pub note_sizes: Vec<(String, StyleValue)>,
    /// Distances by what they separate, in tenths: `distances`.
    pub distances: Vec<(String, StyleValue)>,
    /// Any other appearance by its type: `otherAppearances`.
    pub other_appearances: Vec<(String, StyleValue)>,
    /// The font music is set in: `musicFont`.
    pub music_font: Option<TextStyle>,
    /// The font words are set in: `wordFont`.
    pub word_font: Option<TextStyle>,
    /// A font for the lyrics of each name: `lyricFonts`.
    pub lyric_fonts: Vec<(Option<String>, Option<TextStyle>)>,
    /// A language for the lyrics of each name: `lyricLanguages`.
    pub lyric_languages: Vec<(Option<String>, Option<String>)>,
}

impl StreamStyle {
    /// Whether it says anything MusicXML's `<appearance>` writes.
    pub fn has_appearance(&self) -> bool {
        !(self.line_widths.is_empty()
            && self.note_sizes.is_empty()
            && self.distances.is_empty()
            && self.other_appearances.is_empty())
    }
}

/// The hex value of a CSS3 colour name, as `#FF0000` for `red`, whatever
/// case the name is written in; `None` for a name CSS does not have.
///
/// ```
/// use music21::style::css_color_hex;
///
/// assert_eq!(css_color_hex("Red"), Some("#FF0000"));
/// assert_eq!(css_color_hex("grey"), css_color_hex("gray"));
/// assert_eq!(css_color_hex("reddish"), None);
/// ```
pub fn css_color_hex(name: &str) -> Option<&'static str> {
    let name = name.to_ascii_lowercase();
    CSS_COLORS
        .binary_search_by(|(known, _)| known.cmp(&name.as_str()))
        .ok()
        .map(|index| CSS_COLORS[index].1)
}

/// The CSS3 colour names and their hex values, sorted by name: the table
/// music21 reads a colour name out of, through `webcolors`.
pub const CSS_COLORS: [(&str, &str); 147] = [
    ("aliceblue", "#F0F8FF"),
    ("antiquewhite", "#FAEBD7"),
    ("aqua", "#00FFFF"),
    ("aquamarine", "#7FFFD4"),
    ("azure", "#F0FFFF"),
    ("beige", "#F5F5DC"),
    ("bisque", "#FFE4C4"),
    ("black", "#000000"),
    ("blanchedalmond", "#FFEBCD"),
    ("blue", "#0000FF"),
    ("blueviolet", "#8A2BE2"),
    ("brown", "#A52A2A"),
    ("burlywood", "#DEB887"),
    ("cadetblue", "#5F9EA0"),
    ("chartreuse", "#7FFF00"),
    ("chocolate", "#D2691E"),
    ("coral", "#FF7F50"),
    ("cornflowerblue", "#6495ED"),
    ("cornsilk", "#FFF8DC"),
    ("crimson", "#DC143C"),
    ("cyan", "#00FFFF"),
    ("darkblue", "#00008B"),
    ("darkcyan", "#008B8B"),
    ("darkgoldenrod", "#B8860B"),
    ("darkgray", "#A9A9A9"),
    ("darkgreen", "#006400"),
    ("darkgrey", "#A9A9A9"),
    ("darkkhaki", "#BDB76B"),
    ("darkmagenta", "#8B008B"),
    ("darkolivegreen", "#556B2F"),
    ("darkorange", "#FF8C00"),
    ("darkorchid", "#9932CC"),
    ("darkred", "#8B0000"),
    ("darksalmon", "#E9967A"),
    ("darkseagreen", "#8FBC8F"),
    ("darkslateblue", "#483D8B"),
    ("darkslategray", "#2F4F4F"),
    ("darkslategrey", "#2F4F4F"),
    ("darkturquoise", "#00CED1"),
    ("darkviolet", "#9400D3"),
    ("deeppink", "#FF1493"),
    ("deepskyblue", "#00BFFF"),
    ("dimgray", "#696969"),
    ("dimgrey", "#696969"),
    ("dodgerblue", "#1E90FF"),
    ("firebrick", "#B22222"),
    ("floralwhite", "#FFFAF0"),
    ("forestgreen", "#228B22"),
    ("fuchsia", "#FF00FF"),
    ("gainsboro", "#DCDCDC"),
    ("ghostwhite", "#F8F8FF"),
    ("gold", "#FFD700"),
    ("goldenrod", "#DAA520"),
    ("gray", "#808080"),
    ("green", "#008000"),
    ("greenyellow", "#ADFF2F"),
    ("grey", "#808080"),
    ("honeydew", "#F0FFF0"),
    ("hotpink", "#FF69B4"),
    ("indianred", "#CD5C5C"),
    ("indigo", "#4B0082"),
    ("ivory", "#FFFFF0"),
    ("khaki", "#F0E68C"),
    ("lavender", "#E6E6FA"),
    ("lavenderblush", "#FFF0F5"),
    ("lawngreen", "#7CFC00"),
    ("lemonchiffon", "#FFFACD"),
    ("lightblue", "#ADD8E6"),
    ("lightcoral", "#F08080"),
    ("lightcyan", "#E0FFFF"),
    ("lightgoldenrodyellow", "#FAFAD2"),
    ("lightgray", "#D3D3D3"),
    ("lightgreen", "#90EE90"),
    ("lightgrey", "#D3D3D3"),
    ("lightpink", "#FFB6C1"),
    ("lightsalmon", "#FFA07A"),
    ("lightseagreen", "#20B2AA"),
    ("lightskyblue", "#87CEFA"),
    ("lightslategray", "#778899"),
    ("lightslategrey", "#778899"),
    ("lightsteelblue", "#B0C4DE"),
    ("lightyellow", "#FFFFE0"),
    ("lime", "#00FF00"),
    ("limegreen", "#32CD32"),
    ("linen", "#FAF0E6"),
    ("magenta", "#FF00FF"),
    ("maroon", "#800000"),
    ("mediumaquamarine", "#66CDAA"),
    ("mediumblue", "#0000CD"),
    ("mediumorchid", "#BA55D3"),
    ("mediumpurple", "#9370DB"),
    ("mediumseagreen", "#3CB371"),
    ("mediumslateblue", "#7B68EE"),
    ("mediumspringgreen", "#00FA9A"),
    ("mediumturquoise", "#48D1CC"),
    ("mediumvioletred", "#C71585"),
    ("midnightblue", "#191970"),
    ("mintcream", "#F5FFFA"),
    ("mistyrose", "#FFE4E1"),
    ("moccasin", "#FFE4B5"),
    ("navajowhite", "#FFDEAD"),
    ("navy", "#000080"),
    ("oldlace", "#FDF5E6"),
    ("olive", "#808000"),
    ("olivedrab", "#6B8E23"),
    ("orange", "#FFA500"),
    ("orangered", "#FF4500"),
    ("orchid", "#DA70D6"),
    ("palegoldenrod", "#EEE8AA"),
    ("palegreen", "#98FB98"),
    ("paleturquoise", "#AFEEEE"),
    ("palevioletred", "#DB7093"),
    ("papayawhip", "#FFEFD5"),
    ("peachpuff", "#FFDAB9"),
    ("peru", "#CD853F"),
    ("pink", "#FFC0CB"),
    ("plum", "#DDA0DD"),
    ("powderblue", "#B0E0E6"),
    ("purple", "#800080"),
    ("red", "#FF0000"),
    ("rosybrown", "#BC8F8F"),
    ("royalblue", "#4169E1"),
    ("saddlebrown", "#8B4513"),
    ("salmon", "#FA8072"),
    ("sandybrown", "#F4A460"),
    ("seagreen", "#2E8B57"),
    ("seashell", "#FFF5EE"),
    ("sienna", "#A0522D"),
    ("silver", "#C0C0C0"),
    ("skyblue", "#87CEEB"),
    ("slateblue", "#6A5ACD"),
    ("slategray", "#708090"),
    ("slategrey", "#708090"),
    ("snow", "#FFFAFA"),
    ("springgreen", "#00FF7F"),
    ("steelblue", "#4682B4"),
    ("tan", "#D2B48C"),
    ("teal", "#008080"),
    ("thistle", "#D8BFD8"),
    ("tomato", "#FF6347"),
    ("turquoise", "#40E0D0"),
    ("violet", "#EE82EE"),
    ("wheat", "#F5DEB3"),
    ("white", "#FFFFFF"),
    ("whitesmoke", "#F5F5F5"),
    ("yellow", "#FFFF00"),
    ("yellowgreen", "#9ACD32"),
];

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

    /// Python writes a whole `float` with its point: music21's
    /// `<line-width type="beam">5.0</line-width>`.
    #[test]
    fn a_whole_decimal_is_written_with_its_point() {
        assert_eq!(StyleValue::Decimal(5.0).to_string(), "5.0");
        assert_eq!(StyleValue::Decimal(0.1).to_string(), "0.1");
        assert_eq!(StyleValue::Whole(75).to_string(), "75");
    }
}
