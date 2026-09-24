//! Harmonic function labels and the Roman numerals they stand for:
//! music21's `analysis.harmonicFunction`.
//!
//! A label is a function — tonic, subdominant or dominant — in major
//! (capital) or minor (small), optionally with its Parallelklang (`p`/`P`,
//! a third below) or Gegenklang (`g`/`G`, a third above):
//!
//! ```text
//!   T  I     Tp vi     Tg iii
//!   S  IV    Sp ii     Sg vi
//!   D  V     Dp iii    Dg bvii
//! ```
//!
//! Where the figure depends on the mode — the third, sixth and seventh
//! degrees — a minor key has its own table.

use crate::{
    error::{Error, Result},
    key::Key,
    roman::RomanNumeral,
};

/// The mode whose table is used where a figure differs by mode.
const MINOR: &str = "minor";

/// One of the eighteen harmonic function labels.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HarmonicFunction {
    /// `T`
    TonicMajor,
    /// `Tp`
    TonicMajorParallelklangMinor,
    /// `Tg`
    TonicMajorGegenklangMinor,
    /// `t`
    TonicMinor,
    /// `tP`
    TonicMinorParallelklangMajor,
    /// `tG`
    TonicMinorGegenklangMajor,
    /// `S`
    SubdominantMajor,
    /// `Sp`
    SubdominantMajorParallelklangMinor,
    /// `Sg`
    SubdominantMajorGegenklangMinor,
    /// `s`
    SubdominantMinor,
    /// `sP`
    SubdominantMinorParallelklangMajor,
    /// `sG`
    SubdominantMinorGegenklangMajor,
    /// `D`
    DominantMajor,
    /// `Dp`
    DominantMajorParallelklangMinor,
    /// `Dg`
    DominantMajorGegenklangMinor,
    /// `d`
    DominantMinor,
    /// `dP`
    DominantMinorParallelklangMajor,
    /// `dG`
    DominantMinorGegenklangMajor,
}

use HarmonicFunction::*;

/// The figures that are the same in either mode, in music21's order.
const KEY_NEUTRAL: [(HarmonicFunction, &str); 11] = [
    (TonicMajor, "I"),
    (TonicMinor, "i"),
    (SubdominantMinorGegenklangMajor, "bII"),
    (SubdominantMajorParallelklangMinor, "ii"),
    (SubdominantMajor, "IV"),
    (SubdominantMinor, "iv"),
    (DominantMajor, "V"),
    (DominantMinor, "v"),
    (TonicMajorParallelklangMinor, "vi"),
    (SubdominantMajorGegenklangMinor, "vi"),
    (DominantMajorGegenklangMinor, "bvii"),
];

/// The figures a major key has of its own, listed before the neutral ones
/// as music21 merges them: the first match wins a reverse lookup, so `iii`
/// reads as `Dp` and not `Tg`.
const MAJOR_ONLY: [(HarmonicFunction, &str); 7] = [
    (TonicMinorParallelklangMajor, "bIII"),
    (DominantMinorGegenklangMajor, "bIII"),
    (DominantMajorParallelklangMinor, "iii"),
    (TonicMajorGegenklangMinor, "iii"),
    (SubdominantMinorParallelklangMajor, "bVI"),
    (TonicMinorGegenklangMajor, "bVI"),
    (DominantMinorParallelklangMajor, "bVII"),
];

/// The figures a minor key has of its own, likewise.
const MINOR_ONLY: [(HarmonicFunction, &str); 7] = [
    (TonicMinorParallelklangMajor, "III"),
    (DominantMinorGegenklangMajor, "III"),
    (DominantMajorParallelklangMinor, "#iii"),
    (TonicMajorGegenklangMinor, "#iii"),
    (SubdominantMinorParallelklangMajor, "VI"),
    (TonicMinorGegenklangMajor, "VI"),
    (DominantMinorParallelklangMajor, "VII"),
];

/// Whether a lookup keeps the whole label or only its main function.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FunctionDetail {
    /// The whole label: `sP`.
    #[default]
    Full,
    /// Only the Hauptfunktion, its first letter: `s`. music21's
    /// `onlyHauptHarmonicFunction`.
    MainOnly,
}

impl HarmonicFunction {
    /// All eighteen, in music21's order.
    pub const ALL: [Self; 18] = [
        TonicMajor,
        TonicMajorParallelklangMinor,
        TonicMajorGegenklangMinor,
        TonicMinor,
        TonicMinorParallelklangMajor,
        TonicMinorGegenklangMajor,
        SubdominantMajor,
        SubdominantMajorParallelklangMinor,
        SubdominantMajorGegenklangMinor,
        SubdominantMinor,
        SubdominantMinorParallelklangMajor,
        SubdominantMinorGegenklangMajor,
        DominantMajor,
        DominantMajorParallelklangMinor,
        DominantMajorGegenklangMinor,
        DominantMinor,
        DominantMinorParallelklangMajor,
        DominantMinorGegenklangMajor,
    ];

    /// The label: `T`, `Tp`, `sG` and so on.
    pub fn symbol(self) -> &'static str {
        match self {
            TonicMajor => "T",
            TonicMajorParallelklangMinor => "Tp",
            TonicMajorGegenklangMinor => "Tg",
            TonicMinor => "t",
            TonicMinorParallelklangMajor => "tP",
            TonicMinorGegenklangMajor => "tG",
            SubdominantMajor => "S",
            SubdominantMajorParallelklangMinor => "Sp",
            SubdominantMajorGegenklangMinor => "Sg",
            SubdominantMinor => "s",
            SubdominantMinorParallelklangMajor => "sP",
            SubdominantMinorGegenklangMajor => "sG",
            DominantMajor => "D",
            DominantMajorParallelklangMinor => "Dp",
            DominantMajorGegenklangMinor => "Dg",
            DominantMinor => "d",
            DominantMinorParallelklangMajor => "dP",
            DominantMinorGegenklangMajor => "dG",
        }
    }

    /// music21's member name: `TONIC_MAJOR`, `SUBDOMINANT_MINOR_GEGENKLANG_MAJOR`.
    pub fn music21_name(self) -> &'static str {
        match self {
            TonicMajor => "TONIC_MAJOR",
            TonicMajorParallelklangMinor => "TONIC_MAJOR_PARALLELKLANG_MINOR",
            TonicMajorGegenklangMinor => "TONIC_MAJOR_GEGENKLANG_MINOR",
            TonicMinor => "TONIC_MINOR",
            TonicMinorParallelklangMajor => "TONIC_MINOR_PARALLELKLANG_MAJOR",
            TonicMinorGegenklangMajor => "TONIC_MINOR_GEGENKLANG_MAJOR",
            SubdominantMajor => "SUBDOMINANT_MAJOR",
            SubdominantMajorParallelklangMinor => "SUBDOMINANT_MAJOR_PARALLELKLANG_MINOR",
            SubdominantMajorGegenklangMinor => "SUBDOMINANT_MAJOR_GEGENKLANG_MINOR",
            SubdominantMinor => "SUBDOMINANT_MINOR",
            SubdominantMinorParallelklangMajor => "SUBDOMINANT_MINOR_PARALLELKLANG_MAJOR",
            SubdominantMinorGegenklangMajor => "SUBDOMINANT_MINOR_GEGENKLANG_MAJOR",
            DominantMajor => "DOMINANT_MAJOR",
            DominantMajorParallelklangMinor => "DOMINANT_MAJOR_PARALLELKLANG_MINOR",
            DominantMajorGegenklangMinor => "DOMINANT_MAJOR_GEGENKLANG_MINOR",
            DominantMinor => "DOMINANT_MINOR",
            DominantMinorParallelklangMajor => "DOMINANT_MINOR_PARALLELKLANG_MAJOR",
            DominantMinorGegenklangMajor => "DOMINANT_MINOR_GEGENKLANG_MAJOR",
        }
    }

    /// The function a label names, case and all.
    ///
    /// # Errors
    ///
    /// A label that is none of the eighteen, `TPG` say.
    pub fn from_symbol(symbol: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|function| function.symbol() == symbol)
            .ok_or_else(|| Error::Value(format!("'{symbol}' is not a valid HarmonicFunction")))
    }

    /// The main function alone, the label's first letter: `sP` is `s`.
    pub fn main_function(self) -> Self {
        match self.symbol().chars().next() {
            Some('T') => TonicMajor,
            Some('t') => TonicMinor,
            Some('S') => SubdominantMajor,
            Some('s') => SubdominantMinor,
            Some('D') => DominantMajor,
            _ => DominantMinor,
        }
    }

    /// The Roman numeral figure this function has in a key of `mode`:
    /// `sP` is `bVI` in major and `VI` in minor.
    pub fn figure(self, mode: &str) -> &'static str {
        table(mode)
            .find(|(function, _)| *function == self)
            .map(|(_, figure)| figure)
            .unwrap_or_default()
    }

    /// The Roman numeral this function is in `key`: music21's
    /// `functionToRoman`.
    ///
    /// ```
    /// use music21_rs::{Key, analysis::harmonic_function::HarmonicFunction};
    ///
    /// let minor = Key::from_tonic_mode("a", "minor")?;
    /// let numeral = HarmonicFunction::SubdominantMinorParallelklangMajor.to_roman(&minor)?;
    /// assert_eq!(numeral.figure(), "VI");
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// None in practice: every figure in the tables is one a Roman numeral
    /// reads.
    pub fn to_roman(self, key: &Key) -> Result<RomanNumeral> {
        RomanNumeral::new(self.figure(key.mode()), key.clone())
    }

    /// The function a Roman numeral has, read off its numeral and front
    /// alteration alone, so inversions and figures do not count: music21's
    /// `romanToFunction`. Where two functions share a numeral, the first in
    /// music21's table wins: `vi` is `Tp`. Nothing for a numeral no function
    /// has.
    pub fn from_roman(numeral: &RomanNumeral, detail: FunctionDetail) -> Option<Self> {
        let written = numeral.roman_numeral();
        let (found, _) = table(numeral.key().mode()).find(|(_, figure)| *figure == written)?;

        match detail {
            FunctionDetail::Full => Some(found),
            FunctionDetail::MainOnly => Some(found.main_function()),
        }
    }
}

/// The figures a key of `mode` reads its functions as, in music21's order:
/// the mode's own first, then the neutral ones.
fn table(mode: &str) -> impl Iterator<Item = (HarmonicFunction, &'static str)> {
    let own = if mode == MINOR {
        MINOR_ONLY
    } else {
        MAJOR_ONLY
    };
    own.into_iter().chain(KEY_NEUTRAL)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(tonic: &str, mode: &str) -> Key {
        Key::from_tonic_mode(tonic, mode).unwrap()
    }

    fn numeral(figure: &str, tonic: &str, mode: &str) -> RomanNumeral {
        RomanNumeral::new(figure, key(tonic, mode)).unwrap()
    }

    #[test]
    fn every_function_has_a_figure_in_either_mode() {
        for function in HarmonicFunction::ALL {
            assert!(!function.figure("major").is_empty(), "{function:?}");
            assert!(!function.figure(MINOR).is_empty(), "{function:?}");
            assert_eq!(
                HarmonicFunction::from_symbol(function.symbol()).unwrap(),
                function
            );
        }
        assert!(HarmonicFunction::from_symbol("TPG").is_err());
    }

    #[test]
    fn functions_read_as_music21s_numerals() {
        let c = key("C", "major");
        assert_eq!(TonicMajor.to_roman(&c).unwrap().figure(), "I");
        assert_eq!(TonicMinor.to_roman(&c).unwrap().figure(), "i");
        assert_eq!(
            SubdominantMinorParallelklangMajor
                .to_roman(&c)
                .unwrap()
                .figure(),
            "bVI"
        );
        assert_eq!(
            SubdominantMinorParallelklangMajor
                .to_roman(&key("a", MINOR))
                .unwrap()
                .figure(),
            "VI"
        );
        assert_eq!(
            TonicMajorParallelklangMinor.to_roman(&c).unwrap().figure(),
            "vi"
        );
        assert_eq!(
            SubdominantMajorGegenklangMinor
                .to_roman(&c)
                .unwrap()
                .figure(),
            "vi"
        );
    }

    #[test]
    fn numerals_read_as_music21s_functions() {
        let full = FunctionDetail::Full;
        let vi = numeral("vi", "C", "major");
        assert_eq!(
            HarmonicFunction::from_roman(&vi, full),
            Some(TonicMajorParallelklangMinor)
        );

        let sixth = numeral("VI", "a", MINOR);
        assert_eq!(
            HarmonicFunction::from_roman(&sixth, full),
            Some(SubdominantMinorParallelklangMajor)
        );
        assert_eq!(
            HarmonicFunction::from_roman(&sixth, FunctionDetail::MainOnly),
            Some(SubdominantMinor)
        );

        let neapolitan = numeral("bII6", "g", MINOR);
        assert_eq!(
            HarmonicFunction::from_roman(&neapolitan, full),
            Some(SubdominantMinorGegenklangMajor)
        );

        let mediant = numeral("III", "f", MINOR);
        assert_eq!(
            HarmonicFunction::from_roman(&mediant, full),
            Some(TonicMinorParallelklangMajor)
        );
        assert_eq!(
            HarmonicFunction::from_roman(&numeral("i6", "C", "major"), full),
            Some(TonicMinor)
        );
        assert_eq!(
            HarmonicFunction::from_roman(&numeral("#iv", "C", "major"), full),
            None
        );
    }

    #[test]
    fn each_label_names_its_main_function() {
        let mains: Vec<&str> = HarmonicFunction::ALL
            .into_iter()
            .map(|function| function.main_function().symbol())
            .collect();
        assert_eq!(
            mains,
            [
                "T", "T", "T", "t", "t", "t", "S", "S", "S", "s", "s", "s", "D", "D", "D", "d",
                "d", "d"
            ]
        );
        assert_eq!(
            DominantMinorGegenklangMajor.music21_name(),
            "DOMINANT_MINOR_GEGENKLANG_MAJOR"
        );
    }
}
