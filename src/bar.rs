//! Barlines: music21's `bar` module.
//!
//! A measure has a barline at each end, and a repeat sign is a barline that
//! also says which way the music repeats. music21 keeps them as the
//! measure's `leftBarline` and `rightBarline`, which is where
//! [`crate::Stream::left_barline`] and [`crate::Stream::right_barline`] keep
//! them here.

use crate::error::{Error, Result};

/// How a barline is drawn: music21's `bar.barTypeList`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BarlineType {
    /// A single thin line.
    #[default]
    Regular,
    /// A dotted line.
    Dotted,
    /// A dashed line.
    Dashed,
    /// A single thick line.
    Heavy,
    /// Two thin lines.
    Double,
    /// A thin line and a thick one, which ends a piece.
    Final,
    /// A thick line and a thin one.
    HeavyLight,
    /// Two thick lines.
    HeavyHeavy,
    /// A short stroke through the top line.
    Tick,
    /// A line through the middle of the staff only.
    Short,
    /// No line at all.
    None,
}

impl BarlineType {
    /// Every type, in music21's order.
    pub const ALL: [BarlineType; 11] = [
        Self::Regular,
        Self::Dotted,
        Self::Dashed,
        Self::Heavy,
        Self::Double,
        Self::Final,
        Self::HeavyLight,
        Self::HeavyHeavy,
        Self::Tick,
        Self::Short,
        Self::None,
    ];

    /// music21's name for the type.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::Dotted => "dotted",
            Self::Dashed => "dashed",
            Self::Heavy => "heavy",
            Self::Double => "double",
            Self::Final => "final",
            Self::HeavyLight => "heavy-light",
            Self::HeavyHeavy => "heavy-heavy",
            Self::Tick => "tick",
            Self::Short => "short",
            Self::None => "none",
        }
    }

    /// Reads a type by music21's name or by the MusicXML name music21
    /// allows for two of them: music21's `standardizeBarType`.
    pub fn from_name(name: &str) -> Result<Self> {
        let name = name.to_lowercase();
        let name = match name.as_str() {
            "light-light" => "double",
            "light-heavy" => "final",
            other => other,
        };
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.as_str() == name)
            .ok_or_else(|| Error::Value(format!("cannot process style: {name}")))
    }

    /// The name MusicXML draws it by: music21's `musicXMLBarStyle`, which
    /// is the type's own name but for `double` and `final`.
    pub fn musicxml_bar_style(self) -> &'static str {
        match self {
            Self::Double => "light-light",
            Self::Final => "light-heavy",
            other => other.as_str(),
        }
    }
}

/// Which way a repeat sign sends the music.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RepeatDirection {
    /// The passage to repeat starts here.
    Start,
    /// The passage to repeat ends here, and goes back.
    End,
}

impl RepeatDirection {
    /// music21's name for the direction.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::End => "end",
        }
    }
}

/// A barline: music21's `bar.Barline`, or its `bar.Repeat` where it is a
/// repeat sign.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Barline {
    bar_type: BarlineType,
    repeat: Option<(RepeatDirection, Option<u32>)>,
}

impl Barline {
    /// A plain barline of a type.
    pub fn new(bar_type: BarlineType) -> Self {
        Self {
            bar_type,
            repeat: None,
        }
    }

    /// A repeat sign, drawn as music21 draws one: `heavy-light` where a
    /// repeat starts and `final` where it ends. `times` is how many times the
    /// passage is played, where the score says.
    pub fn repeat(direction: RepeatDirection, times: Option<u32>) -> Self {
        Self {
            bar_type: match direction {
                RepeatDirection::Start => BarlineType::HeavyLight,
                RepeatDirection::End => BarlineType::Final,
            },
            repeat: Some((direction, times)),
        }
    }

    /// How the barline is drawn.
    pub fn bar_type(&self) -> BarlineType {
        self.bar_type
    }

    /// Changes how the barline is drawn.
    pub fn set_bar_type(&mut self, bar_type: BarlineType) {
        self.bar_type = bar_type;
    }

    /// Which way the music repeats, where this is a repeat sign.
    pub fn repeat_direction(&self) -> Option<RepeatDirection> {
        self.repeat.map(|(direction, _)| direction)
    }

    /// How many times the passage is played, where a repeat sign says.
    pub fn repeat_times(&self) -> Option<u32> {
        self.repeat.and_then(|(_, times)| times)
    }
}

/// Where a measure stands in an alternative ending: music21's
/// `RepeatBracket`, the bracket drawn over the measures played the first
/// time through, or the second.
///
/// music21 keeps the bracket as a spanner over the measures; a measure here
/// says which ending it is in and whether the bracket opens or closes on it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Ending {
    numbers: Vec<u32>,
    starts: bool,
    stops: bool,
}

impl Ending {
    /// An ending played the times numbered, opening on this measure where
    /// `starts` and closing on it where `stops`. Nought as the only number
    /// is an ending that gives none.
    pub fn new(numbers: Vec<u32>, starts: bool, stops: bool) -> Self {
        Self {
            numbers,
            starts,
            stops,
        }
    }

    /// The times through the ending is played: music21's `numberRange`.
    pub fn numbers(&self) -> &[u32] {
        &self.numbers
    }

    /// Whether the bracket opens on this measure.
    pub fn starts(&self) -> bool {
        self.starts
    }

    /// Whether the bracket closes on this measure.
    pub fn stops(&self) -> bool {
        self.stops
    }

    /// The numbers as MusicXML writes them, `1,2`; an ending numbered
    /// nought writes nothing.
    pub fn number_text(&self) -> String {
        let mut text = match self.numbers.first() {
            Some(0) | None => String::new(),
            Some(first) => first.to_string(),
        };
        for number in self.numbers.iter().skip(1) {
            text.push(',');
            text.push_str(&number.to_string());
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_types_have_musicxml_names_of_their_own() {
        assert_eq!(BarlineType::Final.musicxml_bar_style(), "light-heavy");
        assert_eq!(BarlineType::Double.musicxml_bar_style(), "light-light");
        assert_eq!(BarlineType::Tick.musicxml_bar_style(), "tick");
        assert_eq!(
            BarlineType::from_name("light-heavy").unwrap(),
            BarlineType::Final
        );
        assert_eq!(
            BarlineType::from_name("Heavy-Light").unwrap(),
            BarlineType::HeavyLight
        );
        assert!(BarlineType::from_name("wiggly").is_err());
    }

    #[test]
    fn a_repeat_is_drawn_by_its_direction() {
        let end = Barline::repeat(RepeatDirection::End, Some(3));
        assert_eq!(end.bar_type(), BarlineType::Final);
        assert_eq!(end.repeat_times(), Some(3));
        let start = Barline::repeat(RepeatDirection::Start, None);
        assert_eq!(start.bar_type(), BarlineType::HeavyLight);
        assert_eq!(start.repeat_direction(), Some(RepeatDirection::Start));
        assert_eq!(Barline::default().repeat_direction(), None);
    }
}
