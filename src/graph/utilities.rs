//! music21's `graph.utilities`: colours as matplotlib takes them, and
//! accidentals as its fonts can draw them.

use crate::defaults::FloatType;
use crate::error::{Error, Result};
use crate::style::css_color_hex;

/// matplotlib's one-letter colours and the names they stand for.
const ABBREVIATIONS: [(&str, &str); 8] = [
    ("b", "blue"),
    ("g", "green"),
    ("r", "red"),
    ("c", "cyan"),
    ("m", "magenta"),
    ("y", "yellow"),
    ("k", "black"),
    ("w", "white"),
];

/// The accidentals matplotlib's default fonts draw, in the order music21
/// tries them: `pitch.unicodeFromModifier`'s order.
const DRAWN_ACCIDENTALS: [(&str, &str); 2] = [("#", "\u{266f}"), ("-", "\u{266d}")];

/// The most an RGB channel takes.
const CHANNEL_MAX: i64 = 255;

/// A label with its sharps or its flats drawn as music symbols: only one
/// of the two, the first music21's table has that the label holds.
///
/// ```
/// use music21::graph::utilities::accidental_label_to_unicode;
///
/// assert_eq!(accidental_label_to_unicode("B-4"), "B\u{266d}4");
/// assert_eq!(accidental_label_to_unicode("B--4"), "B\u{266d}\u{266d}4");
/// assert_eq!(accidental_label_to_unicode("C4"), "C4");
/// ```
pub fn accidental_label_to_unicode(label: &str) -> String {
    for (modifier, drawn) in DRAWN_ACCIDENTALS {
        if label.contains(modifier) {
            return label.replace(modifier, drawn);
        }
    }
    label.to_string()
}

/// One channel of a colour given as numbers: Python's int or float, which
/// music21 tells apart when it writes the colour as hex.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Level {
    /// A whole number.
    Whole(i64),
    /// A number with a fraction.
    Real(FloatType),
}

impl Level {
    fn value(self) -> FloatType {
        match self {
            Level::Whole(whole) => whole as FloatType,
            Level::Real(real) => real,
        }
    }
}

/// A colour as music21's `getColor` takes one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Color<'a> {
    /// A name, a matplotlib letter or a hex code.
    Text(&'a str),
    /// One number for a grey, or one per channel: fractions of one where
    /// any is below one, else values up to 255.
    Levels(&'a [Level]),
}

/// A colour as the lowercase hex code matplotlib takes: music21's
/// `getColor`.
///
/// ```
/// use music21::graph::utilities::{Color, Level, color_hex};
///
/// assert_eq!(color_hex(Color::Text("r"))?, "#ff0000");
/// assert_eq!(color_hex(Color::Text("Steel Blue"))?, "#4682b4");
/// assert_eq!(color_hex(Color::Text("#f50"))?, "#ff5500");
/// assert_eq!(color_hex(Color::Levels(&[Level::Real(0.8)]))?, "#cccccc");
/// assert_eq!(color_hex(Color::Levels(&[Level::Whole(255); 3]))?, "#ffffff");
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// An unknown letter or name, a malformed hex code, or numbers that make
/// no colour.
pub fn color_hex(color: Color<'_>) -> Result<String> {
    match color {
        Color::Text(text) => text_hex(text),
        Color::Levels(levels) => levels_hex(levels),
    }
}

fn text_hex(text: &str) -> Result<String> {
    if text.starts_with('#') {
        return normalize_hex(text);
    }

    // A letter of matplotlib's, or a name.
    let mut name = text.to_lowercase().replace(' ', "");
    if name.chars().count() == 1 {
        let Some((_, full)) = ABBREVIATIONS.iter().find(|(letter, _)| *letter == name) else {
            return Err(Error::Graph(format!("invalid color abbreviation: {name}")));
        };
        name = (*full).to_string();
    }
    css_color_hex(&name)
        .map(str::to_lowercase)
        .ok_or_else(|| Error::Graph(format!("invalid color name: {name}")))
}

/// `#rgb` or `#rrggbb` as six lowercase digits: webcolors' `normalize_hex`.
fn normalize_hex(text: &str) -> Result<String> {
    let digits = &text[1..];
    let valid = matches!(digits.len(), 3 | 6) && digits.chars().all(|c| c.is_ascii_hexdigit());
    if !valid {
        return Err(Error::Value(format!(
            "'{text}' is not a valid hexadecimal color value."
        )));
    }
    let digits = digits.to_lowercase();
    if digits.len() == 3 {
        let doubled: String = digits.chars().flat_map(|c| [c, c]).collect();
        return Ok(format!("#{doubled}"));
    }
    Ok(format!("#{digits}"))
}

fn levels_hex(levels: &[Level]) -> Result<String> {
    // A single number is a grey.
    let tripled;
    let levels = if levels.len() == 1 {
        tripled = [levels[0]; 3];
        &tripled[..]
    } else {
        levels
    };
    if levels.len() != 3 {
        return Err(Error::Value(format!(
            "a colour takes three channels, not {}",
            levels.len()
        )));
    }

    // Fractions where any is below one, as webcolors reads percentages.
    if levels.iter().any(|level| level.value() < 1.0) {
        let channels: Vec<i64> = levels
            .iter()
            .map(|level| {
                let percent = (level.value() * 100.0).clamp(0.0, 100.0);
                (percent / 100.0 * CHANNEL_MAX as FloatType).round_ties_even() as i64
            })
            .collect();
        return Ok(hex_of(&channels));
    }
    let mut channels = Vec::new();
    for level in levels {
        match level {
            Level::Whole(whole) => channels.push((*whole).clamp(0, CHANNEL_MAX)),
            Level::Real(real) if *real > CHANNEL_MAX as FloatType => channels.push(CHANNEL_MAX),
            Level::Real(_) => {
                return Err(Error::Value(
                    "Unknown format code 'x' for object of type 'float'".to_string(),
                ));
            }
        }
    }
    Ok(hex_of(&channels))
}

fn hex_of(channels: &[i64]) -> String {
    let digits: String = channels.iter().map(|c| format!("{c:02x}")).collect();
    format!("#{digits}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_read_as_music21_reads_them() {
        let grey = [Level::Real(0.5); 3];
        assert_eq!(color_hex(Color::Levels(&grey)).unwrap(), "#808080");
        assert_eq!(
            color_hex(Color::Levels(&[Level::Real(0.5)])).unwrap(),
            "#808080"
        );
        assert_eq!(
            color_hex(Color::Levels(&[Level::Whole(255)])).unwrap(),
            "#ffffff"
        );
        assert_eq!(color_hex(Color::Text("red")).unwrap(), "#ff0000");
    }

    #[test]
    fn bad_colors_are_refused() {
        let error = color_hex(Color::Text("l")).unwrap_err();
        assert_eq!(
            error.to_string(),
            "Graph error: invalid color abbreviation: l"
        );
        let error = color_hex(Color::Text("chalky white")).unwrap_err();
        assert_eq!(
            error.to_string(),
            "Graph error: invalid color name: chalkywhite"
        );
        assert!(color_hex(Color::Text("#ff")).is_err());
    }
}
