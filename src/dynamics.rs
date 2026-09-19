//! Dynamic marks, ported from music21's `dynamics` module.
//!
//! A [`Dynamic`] is a mark such as `mf` and the loudness it stands for, a
//! scalar between nought and one. Placed in a [`Stream`](crate::Stream) it is
//! in force until the next one, which is what
//! [`realize_volume`](crate::volume::realize_volume) reads.

use std::fmt;

use crate::{
    defaults::FloatType,
    error::{Error, Result},
};

/// The marks music21 lists, softest first: its `shortNames`.
pub const SHORT_NAMES: [&str; 16] = [
    "pppppp", "ppppp", "pppp", "ppp", "pp", "p", "mp", "mf", "f", "fp", "sf", "ff", "fff", "ffff",
    "fffff", "ffffff",
];

/// The Italian word for a mark, where music21 gives one: its `longNames`.
const LONG_NAMES: [(&str, &str); 10] = [
    ("ppp", "pianississimo"),
    ("pp", "pianissimo"),
    ("p", "piano"),
    ("mp", "mezzopiano"),
    ("mf", "mezzoforte"),
    ("f", "forte"),
    ("fp", "fortepiano"),
    ("sf", "sforzando"),
    ("ff", "fortissimo"),
    ("fff", "fortississimo"),
];

/// What a mark means in English, where music21 says: its `englishNames`.
const ENGLISH_NAMES: [(&str, &str); 8] = [
    ("ppp", "extremely soft"),
    ("pp", "very soft"),
    ("p", "soft"),
    ("mp", "moderately soft"),
    ("mf", "moderately loud"),
    ("f", "loud"),
    ("ff", "very loud"),
    ("fff", "extremely loud"),
];

/// The loudness each mark stands for: music21's `dynamicStrToScalar`.
const SCALARS: [(&str, FloatType); 13] = [
    ("n", 0.0),
    ("pppp", 0.1),
    ("ppp", 0.15),
    ("pp", 0.25),
    ("p", 0.35),
    ("mp", 0.45),
    ("mf", 0.55),
    ("f", 0.7),
    ("fp", 0.75),
    ("sf", 0.85),
    ("ff", 0.85),
    ("fff", 0.9),
    ("ffff", 0.95),
];

/// The loudness of a mark music21 has no scalar for.
const DEFAULT_SCALAR: FloatType = 0.5;

fn looked_up<T: Copy>(table: &[(&str, T)], mark: &str) -> Option<T> {
    table
        .iter()
        .find(|(name, _)| *name == mark)
        .map(|(_, value)| *value)
}

/// The mark a loudness between nought and one falls under: music21's
/// `dynamicStrFromDecimal`. Nought is `n`, niente, and `pppp` and `fff` are
/// the ends of what it answers.
pub fn dynamic_str_from_decimal(value: FloatType) -> &'static str {
    match value {
        value if value <= 0.0 => "n",
        value if value < 0.11 => "pppp",
        value if value < 0.16 => "ppp",
        value if value < 0.26 => "pp",
        value if value < 0.36 => "p",
        value if value < 0.5 => "mp",
        value if value < 0.65 => "mf",
        value if value < 0.8 => "f",
        value if value < 0.9 => "ff",
        _ => "fff",
    }
}

/// A dynamic mark and the loudness it stands for.
///
/// ```
/// use music21_rs::Dynamic;
///
/// let forte = Dynamic::new("f");
/// assert_eq!(forte.volume_scalar(), 0.7);
/// assert_eq!(forte.long_name(), Some("forte"));
///
/// // A mark with no loudness of its own is read without its `s` and `z`.
/// assert_eq!(Dynamic::new("sfz").volume_scalar(), 0.7);
///
/// // A loudness names the mark it falls under and keeps its own value.
/// let soft = Dynamic::from_scalar(0.4)?;
/// assert_eq!(soft.value(), "mp");
/// assert_eq!(soft.volume_scalar(), 0.4);
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Dynamic {
    value: String,
    /// A loudness a caller gave, which wins over the mark's own.
    #[cfg_attr(feature = "serde", serde(default))]
    volume_scalar: Option<FloatType>,
}

impl Dynamic {
    /// A dynamic from its mark. Any text is a mark, as it is upstream; one
    /// music21 has no loudness for sounds at the middle of the range.
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            volume_scalar: None,
        }
    }

    /// A dynamic from a loudness between nought and one, named as the mark
    /// that loudness falls under and keeping the loudness itself.
    pub fn from_scalar(scalar: FloatType) -> Result<Self> {
        let mut dynamic = Self::new(dynamic_str_from_decimal(scalar));
        dynamic.set_volume_scalar(scalar)?;
        Ok(dynamic)
    }

    /// The mark as written, `mf`.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Writes another mark. A loudness a caller gave is kept.
    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
    }

    /// The Italian word for the mark, `mezzoforte`, where music21 has one.
    pub fn long_name(&self) -> Option<&'static str> {
        looked_up(&LONG_NAMES, &self.value)
    }

    /// What the mark means in English, where music21 says.
    pub fn english_name(&self) -> Option<&'static str> {
        looked_up(&ENGLISH_NAMES, &self.value)
    }

    /// The loudness the dynamic stands for, between nought and one:
    /// music21's `volumeScalar`.
    ///
    /// The one a caller gave, else the mark's own. A mark with none of its
    /// own is tried again without a leading letter where it has an `s` in
    /// it and without a closing `z`, so `sfz` is as loud as `f`; failing
    /// that it is the middle of the range.
    pub fn volume_scalar(&self) -> FloatType {
        if let Some(scalar) = self.volume_scalar {
            return scalar;
        }
        if let Some(scalar) = looked_up(&SCALARS, &self.value) {
            return scalar;
        }
        // music21 drops the first character wherever the mark has an `s`
        // anywhere in it, which is what is reproduced.
        let mut mark = self.value.as_str();
        if mark.contains('s') {
            let mut characters = mark.chars();
            characters.next();
            mark = characters.as_str();
        }
        let mark = mark.strip_suffix('z').unwrap_or(mark);
        looked_up(&SCALARS, mark).unwrap_or(DEFAULT_SCALAR)
    }

    /// Gives the dynamic a loudness of its own, overriding its mark's.
    pub fn set_volume_scalar(&mut self, scalar: FloatType) -> Result<()> {
        if !(0.0..=1.0).contains(&scalar) {
            return Err(Error::Volume(format!(
                "cannot set as volume scalar to: {scalar}"
            )));
        }
        self.volume_scalar = Some(scalar);
        Ok(())
    }
}

impl fmt::Display for Dynamic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read off music21; the tables themselves are held to it by
    /// `table_parity`.
    #[test]
    fn a_mark_sounds_as_loud_as_music21_says() {
        for (mark, scalar) in [
            ("sf", 0.85),
            ("sfz", 0.7),
            ("fp", 0.75),
            ("rfz", 0.5),
            ("sffz", 0.85),
            ("ppppp", 0.5),
            ("n", 0.0),
        ] {
            assert_eq!(Dynamic::new(mark).volume_scalar(), scalar, "{mark}");
        }
        assert_eq!(Dynamic::new("sf").long_name(), Some("sforzando"));
        assert_eq!(Dynamic::new("sf").english_name(), None);
        assert_eq!(Dynamic::new("mf").english_name(), Some("moderately loud"));
        assert!(Dynamic::from_scalar(1.5).is_err());
        assert_eq!(dynamic_str_from_decimal(0.0), "n");
        assert_eq!(dynamic_str_from_decimal(0.95), "fff");
    }
}
