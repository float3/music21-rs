//! The harmonic annotations a `**harm` spine carries: music21's
//! `humdrum.harmParser`.
//!
//! A `**harm` token is a scale degree with what is written round it --
//! `V7b`, `#IVo7`, `Gnd`, `V/V/V`, `(I)`, `I[V]` -- and the two notations
//! are not the same, so music21 converts one into the other: a `V7b` there
//! is a `V65` to a [`RomanNumeral`](crate::roman::RomanNumeral), the
//! inversion letter standing for the figured bass. The representation is
//! described at <https://www.humdrum.org/rep/harm/>.
//!
//! music21 reads a token with a regular expression; the crate has none, so
//! [`Harm::parse`] reads it by hand in the same order -- the accidental,
//! the degree, the quality, the intervals, the inversion, an alternative
//! reading in brackets, and a secondary function after a slash. The two
//! agree on every token music21's own tests and docstrings name, and on the
//! ones they do not: `harm::tests` holds forty more read off music21.
//!
//! The degree is matched longest first where music21's alternation tries
//! the shortest and backtracks. The two come to the same thing, since no
//! prefix of a degree (`i`, `v`, `I`, `V`) is a quality, an interval, an
//! inversion, a bracket or a slash, so a shorter degree never leaves
//! something the rest of the token can read.

/// The degrees and special chords a `**harm` token may name, longest first.
const ROOTS: [&str; 19] = [
    "iii", "vii", "III", "VII", "iv", "vi", "ii", "IV", "VI", "II", "Gn", "Lt", "Fr", "Tr", "i",
    "v", "I", "V", "N",
];

/// A `**harm` annotation read: music21's `HarmParser.parse`, whose answer is
/// a dictionary of these same parts.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Harm {
    accidental: String,
    root: String,
    attribute: Option<char>,
    intervals: Option<String>,
    inversion: Option<char>,
    alternative: Option<Box<Harm>>,
    secondary: Option<Box<Harm>>,
    implied: bool,
}

impl Harm {
    /// Reads a `**harm` token, or nothing where it is not one: music21's
    /// `HarmParser.parse`, which answers an empty dictionary there.
    ///
    /// ```
    /// use music21_rs::humdrum::harm::Harm;
    ///
    /// let read = Harm::parse("#IV7b").expect("a harm token");
    /// assert_eq!(read.accidental(), "#");
    /// assert_eq!(read.root(), "IV");
    /// assert_eq!(read.intervals(), Some("7"));
    /// assert_eq!(read.inversion(), Some('b'));
    /// assert_eq!(read.roman_figure(), "#IV65");
    /// assert_eq!(Harm::parse("VIII"), None);
    /// ```
    pub fn parse(token: &str) -> Option<Self> {
        // Harmony the annotator thought implied is written in parentheses
        // round the whole token, and nothing else may be: music21's own
        // pattern matches the whole string or fails.
        if let Some(inner) = token
            .strip_prefix('(')
            .and_then(|rest| rest.strip_suffix(')'))
            && !inner.is_empty()
            && !inner.contains('(')
            && !inner.contains(')')
        {
            let mut found = Self::parse(inner)?;
            found.implied = true;
            return Some(found);
        }

        let mut rest = token;
        let mut read = Self::default();

        // The accidental: up to two of `#` or `-`, as many as are written.
        // Taking both can cost nothing, since no degree begins with either.
        while read.accidental.len() < 2 {
            let Some(mark) = rest
                .chars()
                .next()
                .filter(|mark| *mark == '#' || *mark == '-')
            else {
                break;
            };
            read.accidental.push(mark);
            rest = &rest[mark.len_utf8()..];
        }

        let root = ROOTS.iter().find(|root| rest.starts_with(**root))?;
        read.root = (*root).to_string();
        rest = &rest[root.len()..];

        // Diminished or augmented, where the token says so.
        if let Some(quality) = rest
            .chars()
            .next()
            .filter(|mark| *mark == 'o' || *mark == '+')
        {
            read.attribute = Some(quality);
            rest = &rest[quality.len_utf8()..];
        }

        // The intervals written over the degree, as many as the annotator
        // wrote. music21 keeps the last of them, its group having matched
        // once per interval, and that is what it reads the seventh off.
        while let Some((interval, after)) = next_interval(rest) {
            read.intervals = Some(interval.to_string());
            rest = after;
        }

        if let Some(inversion) = rest
            .chars()
            .next()
            .filter(|mark| ('b'..='d').contains(mark))
        {
            read.inversion = Some(inversion);
            rest = &rest[inversion.len_utf8()..];
        }

        // An alternative reading in brackets, which music21 reads and the
        // figure then says nothing of.
        if let Some(after) = rest.strip_prefix('[') {
            let (inner, after) = after.split_once(']')?;
            if inner.is_empty() || inner.contains('[') {
                return None;
            }
            read.alternative = Self::parse(inner).map(Box::new);
            rest = after;
        }

        // A secondary function after a slash: the degree the one before it
        // is read in. One that cannot be read is kept as nothing, which is
        // the empty dictionary music21 keeps and then passes over.
        if let Some(after) = rest.strip_prefix('/') {
            if after.is_empty() {
                return None;
            }
            read.secondary = Self::parse(after).map(Box::new);
            rest = "";
        }

        rest.is_empty().then_some(read)
    }

    /// The accidental written in front of the degree: `#`, `--`, or nothing.
    pub fn accidental(&self) -> &str {
        &self.accidental
    }

    /// The degree or special chord: `V`, `ii`, `Gn`, `N`.
    pub fn root(&self) -> &str {
        &self.root
    }

    /// `o` for a diminished reading, `+` for an augmented one.
    pub fn attribute(&self) -> Option<char> {
        self.attribute
    }

    /// The last of the intervals written over the degree, as music21 keeps
    /// them: `7` of `V7`, `AA6` of `VAA6`, `9` of `V79`.
    pub fn intervals(&self) -> Option<&str> {
        self.intervals.as_deref()
    }

    /// The inversion letter: `b` for the first inversion, `c` for the
    /// second, `d` for the third.
    pub fn inversion(&self) -> Option<char> {
        self.inversion
    }

    /// The reading written in brackets beside this one, which the figure
    /// says nothing of.
    pub fn alternative(&self) -> Option<&Harm> {
        self.alternative.as_deref()
    }

    /// The degree this one is read in, written after a slash.
    pub fn secondary(&self) -> Option<&Harm> {
        self.secondary.as_deref()
    }

    /// Whether the annotator wrote the token in parentheses, meaning the
    /// harmony is implied rather than sounded.
    pub fn implied(&self) -> bool {
        self.implied
    }

    /// The figure a [`RomanNumeral`](crate::roman::RomanNumeral) is built
    /// from: music21's `convertHarmToRoman`.
    ///
    /// The inversion letter becomes the figured bass it stands for -- a
    /// seventh chord's `b` is `65` and a triad's is `6` -- a German or
    /// French sixth is read as a seventh chord whatever intervals were
    /// written, and only the *degrees* of the secondary functions are
    /// carried across, as music21 carries them.
    pub fn roman_figure(&self) -> String {
        let (degree, intervals) = match self.root.as_str() {
            "Gn" => ("Ger", Some("7")),
            "Lt" => ("It", self.intervals()),
            "Fr" => ("Fr", Some("7")),
            root => (root, self.intervals()),
        };
        let seventh = intervals.is_some_and(|intervals| intervals.contains('7'));
        let inversion = match self.inversion {
            None => {
                if seventh {
                    "7"
                } else {
                    ""
                }
            }
            Some('b') => {
                if seventh {
                    "65"
                } else {
                    "6"
                }
            }
            Some('c') => {
                if seventh {
                    "43"
                } else {
                    "64"
                }
            }
            // A third inversion is a seventh chord's, or a special chord's.
            _ => "2",
        };
        let mut figure = format!(
            "{}{degree}{}{inversion}",
            self.accidental,
            self.attribute.map(String::from).unwrap_or_default()
        );
        let mut secondary = self.secondary();
        while let Some(found) = secondary {
            figure.push('/');
            figure.push_str(&found.root);
            secondary = found.secondary();
        }
        figure
    }
}

/// The interval at the front of what is left, if one is written there:
/// `AA6`, `DD7`, `M7` or a plain `7`, as music21's own alternation reads
/// them.
fn next_interval(rest: &str) -> Option<(&str, &str)> {
    let digits = |from: usize| -> Option<usize> {
        let count = rest[from..]
            .chars()
            .take_while(char::is_ascii_digit)
            .count();
        (count > 0).then_some(from + count)
    };
    for prefix in ["AA", "DD"] {
        if rest.starts_with(prefix)
            && let Some(end) = digits(prefix.len())
        {
            return Some((&rest[..end], &rest[end..]));
        }
    }
    if rest
        .chars()
        .next()
        .is_some_and(|first| "mMPAD".contains(first))
        && let Some(end) = digits(1)
    {
        return Some((&rest[..end], &rest[end..]));
    }
    digits(0).map(|end| (&rest[..end], &rest[end..]))
}

/// The figure a `**harm` token stands for, or nothing where it is no token:
/// music21's `convertHarmToRoman`.
///
/// ```
/// use music21_rs::humdrum::harm::convert_harm_to_roman;
///
/// assert_eq!(convert_harm_to_roman("V7b").as_deref(), Some("V65"));
/// assert_eq!(convert_harm_to_roman("Gnd").as_deref(), Some("Ger2"));
/// assert_eq!(convert_harm_to_roman("x"), None);
/// ```
pub fn convert_harm_to_roman(token: &str) -> Option<String> {
    Harm::parse(token).map(|harm| harm.roman_figure())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every figure here is music21's own for the same token: the triads,
    /// sevenths, special chords and alterations of its own tests, the
    /// docstrings' examples, and forty more read off music21 beside them.
    #[test]
    fn a_harm_token_reads_as_music21_reads_it() {
        for (token, figure) in [
            // The docstrings' own.
            ("I", Some("I")),
            ("Vc", Some("V64")),
            ("Ib", Some("I6")),
            ("iib", Some("ii6")),
            ("V", Some("V")),
            ("viiob", Some("viio6")),
            ("vib", Some("vi6")),
            ("V7", Some("V7")),
            ("viio7c", Some("viio43")),
            ("V7d", Some("V2")),
            ("viio7b", Some("viio65")),
            ("V7c", Some("V43")),
            ("Lt", Some("It")),
            ("Ltb", Some("It6")),
            ("Ltc", Some("It64")),
            ("Fr", Some("Fr7")),
            ("Frb", Some("Fr65")),
            ("Frc", Some("Fr43")),
            ("Frd", Some("Fr2")),
            ("Gn", Some("Ger7")),
            ("Gnb", Some("Ger65")),
            ("Gnc", Some("Ger43")),
            ("Gnd", Some("Ger2")),
            // Its unit tests' special chords and alterations.
            ("N", Some("N")),
            ("Nb", Some("N6")),
            ("Nc", Some("N64")),
            ("-IIb", Some("-II6")),
            ("--IIc", Some("--II64")),
            ("#IV7", Some("#IV7")),
            ("##IV7b", Some("##IV65")),
            // Every degree, in both cases.
            ("i", Some("i")),
            ("iv", Some("iv")),
            ("vii", Some("vii")),
            ("III", Some("III")),
            ("VI", Some("VI")),
            ("IV", Some("IV")),
            ("ib", Some("i6")),
            ("ivc", Some("iv64")),
            ("viid", Some("vii2")),
            ("I7", Some("I7")),
            ("I7b", Some("I65")),
            ("I7c", Some("I43")),
            ("I7d", Some("I2")),
            ("i7", Some("i7")),
            // The qualities.
            ("viio", Some("viio")),
            ("viio7", Some("viio7")),
            ("V+", Some("V+")),
            ("ii+7", Some("ii+7")),
            ("iiio", Some("iiio")),
            ("iio", Some("iio")),
            ("iio7", Some("iio7")),
            ("iiiob", Some("iiio6")),
            ("Io7", Some("Io7")),
            ("V+7b", Some("V+65")),
            ("vii+", Some("vii+")),
            // The intervals: only a seventh among them changes the figure,
            // and the quality letter in front of one is not carried over.
            ("VM7", Some("V7")),
            ("Vm7", Some("V7")),
            ("VD7", Some("V7")),
            ("VA6", Some("V")),
            ("VAA6", Some("V")),
            ("VDD7", Some("V7")),
            ("V9", Some("V")),
            ("V11", Some("V")),
            ("V13", Some("V")),
            ("V79", Some("V7")),
            ("V0", Some("V")),
            ("I8", Some("I")),
            ("IM7b", Some("I65")),
            ("Vm7c", Some("V43")),
            ("TrD7", Some("Tr7")),
            ("N7", Some("N7")),
            ("Lt7", Some("It7")),
            ("Ltd", Some("It2")),
            // A German or French sixth is a seventh chord whatever is
            // written over it.
            ("Gn7b", Some("Ger65")),
            ("Fr7b", Some("Fr65")),
            // The secondary functions, of which only the degrees carry.
            ("V/V", Some("V/V")),
            ("V7/IV", Some("V7/IV")),
            ("viio/ii", Some("viio/ii")),
            ("V/V/V", Some("V/V/V")),
            ("viioD7/iv/v", Some("viio7/iv/v")),
            ("V7b/ii", Some("V65/ii")),
            ("V/#iv", Some("V/iv")),
            ("V/ivb", Some("V/iv")),
            ("V/iv7", Some("V/iv")),
            ("V7/V7", Some("V7/V")),
            ("V/N", Some("V/N")),
            ("V/Gn", Some("V/Gn")),
            // A secondary that cannot be read is passed over, as music21
            // passes over the empty dictionary its own parse answers.
            ("V//V", Some("V")),
            // Implied harmony, and an alternative reading.
            ("(I)", Some("I")),
            ("(V/V)", Some("V/V")),
            ("(Ib)", Some("I6")),
            ("I[V]", Some("I")),
            ("I[V/IV]", Some("I")),
            ("V7[viio]", Some("V7")),
            ("I[V]/V", Some("I/V")),
            // Both accidentals, in either order, and on a special chord.
            ("#-I", Some("#-I")),
            ("-#I", Some("-#I")),
            ("#N", Some("#N")),
            ("-Lt", Some("-It")),
            ("Tr", Some("Tr")),
            ("Trb", Some("Tr6")),
            // What is no token at all.
            ("", None),
            (".", None),
            ("x", None),
            ("VIII", None),
            ("I/", None),
            ("/V", None),
            ("Ie", None),
            ("I!", None),
            ("I-", None),
            ("I7o", None),
            ("iib7", None),
            ("I#", None),
            ("I(", None),
            ("#", None),
            ("-", None),
            ("o", None),
            ("+", None),
            ("(I)/V", None),
        ] {
            assert_eq!(
                convert_harm_to_roman(token).as_deref(),
                figure,
                "**harm token {token}"
            );
        }
    }

    /// The parts music21's own `parse` answers with, for the tokens whose
    /// dictionary its docstrings show.
    #[test]
    fn the_parts_of_a_token_are_music21s() {
        let read = Harm::parse("iib").expect("a token");
        assert_eq!((read.root(), read.inversion()), ("ii", Some('b')));
        assert!(!read.implied());

        let read = Harm::parse("#IV7").expect("a token");
        assert_eq!(read.accidental(), "#");
        assert_eq!(read.intervals(), Some("7"));

        let read = Harm::parse("VAA6").expect("a token");
        assert_eq!(read.intervals(), Some("AA6"));

        let read = Harm::parse("(I)").expect("a token");
        assert!(read.implied());
        assert_eq!(read.root(), "I");

        let read = Harm::parse("I[V]").expect("a token");
        assert_eq!(read.alternative().map(Harm::root), Some("V"));
        assert!(read.secondary().is_none());

        // The chain of secondary functions, which the figure walks: a
        // degree read in a degree read in the key.
        let read = Harm::parse("V/V/V").expect("a token");
        assert_eq!(read.secondary().map(Harm::root), Some("V"));
        assert_eq!(
            read.secondary().and_then(Harm::secondary).map(Harm::root),
            Some("V")
        );
        assert!(
            read.secondary()
                .and_then(Harm::secondary)
                .and_then(Harm::secondary)
                .is_none()
        );
    }
}
