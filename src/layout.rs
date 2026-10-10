//! How a score is laid out on the page: music21's `layout` objects.
//!
//! A [`ScoreLayout`] gives the scaling and the page, system and staff
//! layouts a score starts with, and stands in the score itself; a
//! [`PageLayout`], [`SystemLayout`] or [`StaffLayout`] stands in a measure
//! and says how the page, system or staff changes there, or that a new page
//! or system starts. They are read from and written to MusicXML's
//! `<defaults>`, `<print>` and `<staff-details>`. Margins and distances are
//! in tenths of a staff space, as MusicXML gives them, kept as the
//! integer or the decimal the file wrote.

use crate::defaults::FloatType;

/// A length as MusicXML writes it: a whole number, or one with a fraction.
/// music21 keeps `120` an `int` and `120.5` a `float`, and writes each back
/// as it was.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Tenths {
    /// A whole number.
    Whole(i64),
    /// A number with a fraction.
    Fraction(FloatType),
}

impl Tenths {
    /// The length as a number.
    pub fn value(self) -> FloatType {
        match self {
            Self::Whole(whole) => whole as FloatType,
            Self::Fraction(value) => value,
        }
    }

    /// music21's `_floatOrIntStr`: the number the text says, whole where
    /// it has no fraction -- `120.0` is `120` -- and nothing where the text
    /// is no number.
    pub fn parse(text: &str) -> Option<Self> {
        let value = text.trim().parse::<FloatType>().ok()?;
        if value.is_finite() && value.fract() == 0.0 && value.abs() < 9.0e15 {
            Some(Self::Whole(value as i64))
        } else {
            Some(Self::Fraction(value))
        }
    }
}

impl std::fmt::Display for Tenths {
    /// As Python writes the `int` or `float`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Whole(whole) => write!(f, "{whole}"),
            Self::Fraction(value) if value.fract() == 0.0 && value.is_finite() => {
                write!(f, "{value:.1}")
            }
            Self::Fraction(value) => write!(f, "{value}"),
        }
    }
}

/// The scaling and the layouts a score starts with: music21's
/// `ScoreLayout`, read from MusicXML's `<defaults>`.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScoreLayout {
    /// How many millimetres [`ScoreLayout::scaling_tenths`] tenths are:
    /// `scalingMillimeters`.
    pub scaling_millimeters: Option<Tenths>,
    /// How many tenths [`ScoreLayout::scaling_millimeters`] millimetres
    /// are: `scalingTenths`.
    pub scaling_tenths: Option<Tenths>,
    /// The page the score starts on: `pageLayout`.
    pub page_layout: Option<PageLayout>,
    /// The systems the score starts with: `systemLayout`.
    pub system_layout: Option<SystemLayout>,
    /// The staves the score starts with: `staffLayoutList`.
    pub staff_layouts: Vec<StaffLayout>,
}

impl ScoreLayout {
    /// How many millimetres a number of tenths is, rounded to six places,
    /// or nought where the scaling is not known: music21's
    /// `tenthsToMillimeters`.
    pub fn tenths_to_millimeters(&self, tenths: FloatType) -> FloatType {
        let (Some(millimeters), Some(per)) = (self.scaling_millimeters, self.scaling_tenths) else {
            return 0.0;
        };
        let value = millimeters.value() / per.value() * tenths;
        (value * 1e6).round() / 1e6
    }
}

/// How a page is laid out, and whether one starts here: music21's
/// `PageLayout`, MusicXML's `<print new-page>` and `<page-layout>`.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PageLayout {
    /// The page's number: `pageNumber`.
    pub page_number: Option<i64>,
    /// `leftMargin`.
    pub left_margin: Option<Tenths>,
    /// `rightMargin`.
    pub right_margin: Option<Tenths>,
    /// `topMargin`.
    pub top_margin: Option<Tenths>,
    /// `bottomMargin`.
    pub bottom_margin: Option<Tenths>,
    /// `pageHeight`.
    pub page_height: Option<Tenths>,
    /// `pageWidth`.
    pub page_width: Option<Tenths>,
    /// Whether a new page starts here: `isNew`.
    pub is_new: Option<bool>,
}

/// How a system is laid out, and whether one starts here: music21's
/// `SystemLayout`, MusicXML's `<print new-system>` and `<system-layout>`.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SystemLayout {
    /// `leftMargin`.
    pub left_margin: Option<Tenths>,
    /// `rightMargin`.
    pub right_margin: Option<Tenths>,
    /// `topMargin`.
    pub top_margin: Option<Tenths>,
    /// `bottomMargin`.
    pub bottom_margin: Option<Tenths>,
    /// How far the system stands from the one before: `distance`.
    pub distance: Option<Tenths>,
    /// How far the first system of a page stands from its top:
    /// `topDistance`.
    pub top_distance: Option<Tenths>,
    /// Whether a new system starts here: `isNew`.
    pub is_new: Option<bool>,
}

/// What a staff is for: music21's `StaffType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum StaffType {
    /// An ordinary staff.
    #[default]
    Regular,
    /// One the player need not play.
    Ossia,
    /// Cue notes.
    Cue,
    /// An editorial addition.
    Editorial,
    /// Notes for another instrument the player reads.
    Alternate,
    /// Anything else.
    Other,
}

impl StaffType {
    /// The value MusicXML writes in `<staff-type>`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::Ossia => "ossia",
            Self::Cue => "cue",
            Self::Editorial => "editorial",
            Self::Alternate => "alternate",
            Self::Other => "other",
        }
    }

    /// The staff type a `<staff-type>` names.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "regular" => Self::Regular,
            "ossia" => Self::Ossia,
            "cue" => Self::Cue,
            "editorial" => Self::Editorial,
            "alternate" => Self::Alternate,
            "other" => Self::Other,
            _ => return None,
        })
    }
}

/// How a staff stands from the one above it and what it is: music21's
/// `StaffLayout`, which MusicXML splits into `<staff-layout>` and
/// `<staff-details>`.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StaffLayout {
    /// How far the staff stands from the one above: `distance`.
    pub distance: Option<Tenths>,
    /// Which staff of its part it is, where it says: `staffNumber`.
    pub staff_number: Option<i64>,
    /// Its size, a percentage of the ordinary staff's: `staffSize`.
    pub staff_size: Option<FloatType>,
    /// How many lines it has, where not five: `staffLines`.
    pub staff_lines: Option<i64>,
    /// Whether it is hidden: `hidden`, nothing to inherit it.
    pub hidden: Option<bool>,
    /// What it is for: `staffType`.
    pub staff_type: StaffType,
}

/// One of music21's layout objects, as it stands in a stream.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Layout {
    /// A [`ScoreLayout`].
    Score(Box<ScoreLayout>),
    /// A [`PageLayout`].
    Page(PageLayout),
    /// A [`SystemLayout`].
    System(SystemLayout),
    /// A [`StaffLayout`].
    Staff(StaffLayout),
}

impl Layout {
    /// The music21 class it is.
    pub fn class_name(&self) -> &'static str {
        match self {
            Self::Score(_) => "ScoreLayout",
            Self::Page(_) => "PageLayout",
            Self::System(_) => "SystemLayout",
            Self::Staff(_) => "StaffLayout",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_length_is_read_and_written_as_music21_keeps_it() {
        assert_eq!(Tenths::parse("120"), Some(Tenths::Whole(120)));
        assert_eq!(Tenths::parse("30.25"), Some(Tenths::Fraction(30.25)));
        assert_eq!(Tenths::parse("120.0"), Some(Tenths::Whole(120)));
        assert_eq!(Tenths::parse("x"), None);
        assert_eq!(Tenths::Whole(120).to_string(), "120");
        assert_eq!(Tenths::Fraction(30.25).to_string(), "30.25");
        assert_eq!(Tenths::Fraction(7.0).to_string(), "7.0");
    }

    /// music21's `tenthsToMillimeters` doctest.
    #[test]
    fn tenths_are_turned_into_millimeters() {
        let layout = ScoreLayout {
            scaling_millimeters: Some(Tenths::Whole(7)),
            scaling_tenths: Some(Tenths::Whole(40)),
            ..ScoreLayout::default()
        };
        assert_eq!(layout.tenths_to_millimeters(40.0), 7.0);
        assert_eq!(ScoreLayout::default().tenths_to_millimeters(40.0), 0.0);
    }
}
