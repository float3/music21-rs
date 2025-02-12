use std::convert::TryFrom;
use std::fmt::{self, Display};
use std::sync::LazyLock;

use crate::defaults::UnsignedIntegerType;
use crate::exception::Exception;

macro_rules! define_ordinals {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $($variant:ident => ($display:expr, $lower:expr)),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone, PartialEq, Eq)]
        #[repr(u8)]
        $vis enum $name {
            $($variant),*
        }

        impl TryFrom<UnsignedIntegerType> for $name {
            type Error = Exception;
            fn try_from(value: UnsignedIntegerType) -> Result<Self, Self::Error> {
                const VARIANTS: &[$name] = &[$($name::$variant),*];
                let idx = value as usize;
                if idx < VARIANTS.len() {
                    Ok(VARIANTS[idx])
                } else {
                    Err(Exception::Ordinal(format!(
                        "Invalid {} value: {}",
                        stringify!($name),
                        value
                    )))
                }
            }
        }

        impl TryFrom<&str> for $name {
            type Error = Exception;
            fn try_from(value: &str) -> Result<Self, Self::Error> {
                match value.to_lowercase().as_str() {
                    $(
                        $lower => Ok($name::$variant),
                    )*
                    _ => Err(Exception::Ordinal(format!(
                        "Invalid {} string: {}",
                        stringify!($name),
                        value
                    ))),
                }
            }
        }

        impl Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let s = match self {
                    $(
                        $name::$variant => $display,
                    )*
                };
                write!(f, "{}", s)
            }
        }
    };
}

// Define the regular ordinals
define_ordinals! {
    pub(crate) enum Ordinal {
        Zeroth      => ("Zeroth", "zeroth"),
        First       => ("First", "first"),
        Second      => ("Second", "second"),
        Third       => ("Third", "third"),
        Fourth      => ("Fourth", "fourth"),
        Fifth       => ("Fifth", "fifth"),
        Sixth       => ("Sixth", "sixth"),
        Seventh     => ("Seventh", "seventh"),
        Eighth      => ("Eighth", "eighth"),
        Ninth       => ("Ninth", "ninth"),
        Tenth       => ("Tenth", "tenth"),
        Eleventh    => ("Eleventh", "eleventh"),
        Twelfth     => ("Twelfth", "twelfth"),
        Thirteenth  => ("Thirteenth", "thirteenth"),
        Fourteenth  => ("Fourteenth", "fourteenth"),
        Fifteenth   => ("Fifteenth", "fifteenth"),
        Sixteenth   => ("Sixteenth", "sixteenth"),
        Seventeenth => ("Seventeenth", "seventeenth"),
        Eighteenth  => ("Eighteenth", "eighteenth"),
        Nineteenth  => ("Nineteenth", "nineteenth"),
        Twentieth   => ("Twentieth", "twentieth"),
        TwentyFirst => ("Twenty-first", "twenty-first"),
        TwentySecond=> ("Twenty-second", "twenty-second"),
    }
}

// Define the musical ordinals (override a few names)
define_ordinals! {
    pub(crate) enum MusicOrdinals {
        Zeroth      => ("Zeroth", "zeroth"),
        Unison      => ("Unison", "unison"),          // instead of First
        Second      => ("Second", "second"),
        Third       => ("Third", "third"),
        Fourth      => ("Fourth", "fourth"),
        Fifth       => ("Fifth", "fifth"),
        Sixth       => ("Sixth", "sixth"),
        Seventh     => ("Seventh", "seventh"),
        Octave      => ("Octave", "octave"),           // instead of Eighth
        Ninth       => ("Ninth", "ninth"),
        Tenth       => ("Tenth", "tenth"),
        Eleventh    => ("Eleventh", "eleventh"),
        Twelfth     => ("Twelfth", "twelfth"),
        Thirteenth  => ("Thirteenth", "thirteenth"),
        Fourteenth  => ("Fourteenth", "fourteenth"),
        DoubleOctave=> ("Double-octave", "double-octave"), // instead of Fifteenth
        Sixteenth   => ("Sixteenth", "sixteenth"),
        Seventeenth => ("Seventeenth", "seventeenth"),
        Eighteenth  => ("Eighteenth", "eighteenth"),
        Nineteenth  => ("Nineteenth", "nineteenth"),
        Twentieth   => ("Twentieth", "twentieth"),
        TwentyFirst => ("Twenty-first", "twenty-first"),
        TripleOctave=> ("Triple-octave", "triple-octave"), // instead of Twenty-second
    }
}

// Define static arrays similar to the original code.
pub(crate) static ORDINAL_STRINGS: LazyLock<[String; 23]> = LazyLock::new(|| {
    std::array::from_fn(|i| {
        let val: UnsignedIntegerType = i as UnsignedIntegerType;
        let ordinal = Ordinal::try_from(val).unwrap();
        format!("{}", ordinal)
    })
});

pub(crate) static MUSICAL_ORDINAL_STRINGS: LazyLock<[String; 23]> = LazyLock::new(|| {
    std::array::from_fn(|i| {
        let val: UnsignedIntegerType = i as UnsignedIntegerType;
        let ordinal = MusicOrdinals::try_from(val).unwrap();
        format!("{}", ordinal)
    })
});

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryFrom;

    // Tests for Ordinal
    #[test]
    fn test_ordinal_try_from_integer_valid() {
        let cases = [
            (0, Ordinal::Zeroth),
            (1, Ordinal::First),
            (2, Ordinal::Second),
            (22, Ordinal::TwentySecond),
        ];
        for (input, expected) in cases {
            let ordinal = Ordinal::try_from(input).unwrap();
            assert_eq!(ordinal, expected);
        }
    }

    #[test]
    fn test_ordinal_try_from_integer_invalid() {
        let result = Ordinal::try_from(23);
        assert!(result.is_err());
        if let Err(Exception::Ordinal(msg)) = result {
            assert!(msg.contains("Invalid Ordinal value: 23"));
        } else {
            panic!("Expected Exception::Ordinal error");
        }
    }

    #[test]
    fn test_ordinal_try_from_str_valid() {
        let cases = [
            ("zeroth", Ordinal::Zeroth),
            ("First", Ordinal::First),
            ("SECOND", Ordinal::Second),
            ("twenty-first", Ordinal::TwentyFirst),
            ("Twenty-Second", Ordinal::TwentySecond),
        ];
        for (input, expected) in cases {
            let ordinal = Ordinal::try_from(input).unwrap();
            assert_eq!(ordinal, expected);
        }
    }

    #[test]
    fn test_ordinal_try_from_str_invalid() {
        let result = Ordinal::try_from("invalid");
        assert!(result.is_err());
        if let Err(Exception::Ordinal(msg)) = result {
            assert!(msg.contains("Invalid Ordinal string: invalid"));
        } else {
            panic!("Expected Exception::Ordinal error");
        }
    }

    #[test]
    fn test_ordinal_display() {
        let ordinal = Ordinal::TwentyFirst;
        assert_eq!(format!("{}", ordinal), "Twenty-first");
    }

    #[test]
    fn test_music_ordinal_try_from_integer_valid() {
        let cases = [
            (0, MusicOrdinals::Zeroth),
            (1, MusicOrdinals::Unison),
            (2, MusicOrdinals::Second),
            (8, MusicOrdinals::Octave),
            (15, MusicOrdinals::DoubleOctave),
            (22, MusicOrdinals::TripleOctave),
        ];
        for (input, expected) in cases {
            let ordinal = MusicOrdinals::try_from(input).unwrap();
            assert_eq!(ordinal, expected);
        }
    }

    #[test]
    fn test_music_ordinal_try_from_integer_invalid() {
        let result = MusicOrdinals::try_from(23);
        assert!(result.is_err());
        if let Err(Exception::Ordinal(msg)) = result {
            assert!(msg.contains("Invalid MusicOrdinals value: 23"));
        } else {
            panic!("Expected Exception::Ordinal error");
        }
    }

    #[test]
    fn test_music_ordinal_try_from_str_valid() {
        let cases = [
            ("zeroth", MusicOrdinals::Zeroth),
            ("Unison", MusicOrdinals::Unison),
            ("SECOND", MusicOrdinals::Second),
            ("octave", MusicOrdinals::Octave),
            ("double-octave", MusicOrdinals::DoubleOctave),
            ("triple-octave", MusicOrdinals::TripleOctave),
        ];
        for (input, expected) in cases {
            let ordinal = MusicOrdinals::try_from(input).unwrap();
            assert_eq!(ordinal, expected);
        }
    }

    #[test]
    fn test_music_ordinal_try_from_str_invalid() {
        let result = MusicOrdinals::try_from("invalid");
        assert!(result.is_err());
        if let Err(Exception::Ordinal(msg)) = result {
            assert!(msg.contains("Invalid MusicOrdinals string: invalid"));
        } else {
            panic!("Expected Exception::Ordinal error");
        }
    }

    #[test]
    fn test_music_ordinal_display() {
        let ordinal = MusicOrdinals::TwentyFirst;
        assert_eq!(format!("{}", ordinal), "Twenty-first");
    }

    #[test]
    fn test_static_arrays() {
        let ordinal_strings: Vec<_> = ORDINAL_STRINGS.iter().cloned().collect();
        assert_eq!(ordinal_strings[0], "Zeroth");
        assert_eq!(ordinal_strings[1], "First");
        assert_eq!(ordinal_strings[22], "Twenty-second");

        let musical_strings: Vec<_> = MUSICAL_ORDINAL_STRINGS.iter().cloned().collect();
        assert_eq!(musical_strings[0], "Zeroth");
        assert_eq!(musical_strings[1], "Unison");
        assert_eq!(musical_strings[8], "Octave");
        assert_eq!(musical_strings[15], "Double-octave");
        assert_eq!(musical_strings[22], "Triple-octave");
    }
}
