//! Xenakis sieves, ported from music21's `sieve` module.
//!
//! A sieve is a logical expression over *residual classes*. `3@0` selects every
//! integer congruent to 0 modulo 3; `|`, `&` and `^` combine classes as union,
//! intersection and symmetric difference; `-` complements one; and `{}` or `()`
//! group. Applied to semitones, the resulting integer set is a scale — the
//! major scale is `(-3@2 & 4) | (-3@1 & 4@1) | (3@2 & 4@2) | (-3 & 4@3)`.
//!
//! Only the part music21's `SieveScale` actually needs is ported: parsing an
//! expression, testing membership, and reading off the interval widths of one
//! period. music21's `sieve.py` is a 2,000-line module that also does sieve
//! compression, `Zeroth`/`Sieve` segment formats and pitch-range realization,
//! none of which has a caller here.

use crate::defaults::{IntegerType, UnsignedIntegerType};
use crate::error::{Error, Result};

/// A parsed Xenakis sieve.
///
/// ```
/// use music21_rs::Sieve;
///
/// // Every third semitone: a cycle of minor thirds.
/// let sieve = Sieve::parse("3@0")?;
/// assert_eq!(sieve.period(), 3);
/// assert_eq!(sieve.interval_widths()?, [3]);
///
/// // The major scale, as Xenakis would write it.
/// let major = Sieve::parse("(-3@2 & 4) | (-3@1 & 4@1) | (3@2 & 4@2) | (-3 & 4@3)")?;
/// assert_eq!(major.interval_widths()?, [2, 2, 1, 2, 2, 2, 1]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sieve {
    root: Node,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Node {
    /// Integers congruent to `shift` modulo `modulus`.
    Residual {
        modulus: UnsignedIntegerType,
        shift: UnsignedIntegerType,
    },
    Not(Box<Node>),
    And(Box<Node>, Box<Node>),
    Or(Box<Node>, Box<Node>),
    Xor(Box<Node>, Box<Node>),
}

impl Node {
    fn contains(&self, z: IntegerType) -> bool {
        match self {
            Self::Residual { modulus, shift } => {
                z.rem_euclid(*modulus as IntegerType) == *shift as IntegerType
            }
            Self::Not(inner) => !inner.contains(z),
            Self::And(left, right) => left.contains(z) && right.contains(z),
            Self::Or(left, right) => left.contains(z) || right.contains(z),
            Self::Xor(left, right) => left.contains(z) != right.contains(z),
        }
    }

    fn collect_moduli(&self, out: &mut Vec<UnsignedIntegerType>) {
        match self {
            Self::Residual { modulus, .. } => out.push(*modulus),
            Self::Not(inner) => inner.collect_moduli(out),
            Self::And(left, right) | Self::Or(left, right) | Self::Xor(left, right) => {
                left.collect_moduli(out);
                right.collect_moduli(out);
            }
        }
    }
}

impl Sieve {
    /// Parses a sieve expression such as `"3@0|4@1"`.
    ///
    /// A bare modulus means a shift of zero, so `"5"` is `"5@0"`. Whitespace is
    /// ignored, `{}` and `()` both group, and `&` binds tighter than `^`, which
    /// binds tighter than `|` — matching music21, where `3@0|4@0&6@0` parses as
    /// `3@0|{4@0&6@0}`.
    pub fn parse(expression: &str) -> Result<Self> {
        let tokens = tokenize(expression)?;
        let mut parser = Parser {
            tokens: &tokens,
            position: 0,
        };
        let root = parser.parse_or()?;
        if parser.position != tokens.len() {
            return Err(Error::Sieve(format!(
                "trailing input in sieve {expression:?} at token {}",
                parser.position
            )));
        }
        Ok(Self { root })
    }

    /// Returns whether an integer is in the sieve.
    pub fn contains(&self, z: IntegerType) -> bool {
        self.root.contains(z)
    }

    /// Returns the period: the least common multiple of every modulus.
    ///
    /// The sieve's membership pattern repeats with this length.
    pub fn period(&self) -> UnsignedIntegerType {
        let mut moduli = Vec::new();
        self.root.collect_moduli(&mut moduli);
        moduli.into_iter().fold(1, lcm)
    }

    /// Returns the members of the sieve in `low..=high`.
    pub fn segment(&self, low: IntegerType, high: IntegerType) -> Vec<IntegerType> {
        (low..=high).filter(|z| self.contains(*z)).collect()
    }

    /// Returns the widths between consecutive members of one period.
    ///
    /// This is music21's `PitchSieve.getIntervalSequence`, in semitones: the
    /// sieve is evaluated over `0..=period` and the consecutive differences
    /// taken. A sieve with fewer than two members in that window has no widths
    /// and is an error, exactly as music21 raises for `3@1`.
    pub fn interval_widths(&self) -> Result<Vec<IntegerType>> {
        let period = self.period();
        let members = self.segment(0, period as IntegerType);
        if members.len() < 2 {
            return Err(Error::Sieve(format!(
                "sieve has {} member(s) in its period of {period}, so it defines no intervals",
                members.len()
            )));
        }
        Ok(members.windows(2).map(|pair| pair[1] - pair[0]).collect())
    }
}

fn lcm(a: UnsignedIntegerType, b: UnsignedIntegerType) -> UnsignedIntegerType {
    if a == 0 || b == 0 {
        return 0;
    }
    a / gcd(a, b) * b
}

fn gcd(mut a: UnsignedIntegerType, mut b: UnsignedIntegerType) -> UnsignedIntegerType {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Token {
    Number(UnsignedIntegerType),
    At,
    Not,
    And,
    Or,
    Xor,
    Open,
    Close,
}

fn tokenize(expression: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut chars = expression.chars().peekable();

    while let Some(&character) = chars.peek() {
        match character {
            ' ' | '\t' | '\n' | '\r' => {
                chars.next();
            }
            '0'..='9' => {
                let mut value: UnsignedIntegerType = 0;
                while let Some(&digit) = chars.peek() {
                    let Some(digit) = digit.to_digit(10) else {
                        break;
                    };
                    value = value
                        .checked_mul(10)
                        .and_then(|value| value.checked_add(digit))
                        .ok_or_else(|| {
                            Error::Sieve(format!("number overflows in sieve {expression:?}"))
                        })?;
                    chars.next();
                }
                tokens.push(Token::Number(value));
            }
            '@' => {
                chars.next();
                tokens.push(Token::At);
            }
            '-' => {
                chars.next();
                tokens.push(Token::Not);
            }
            '&' => {
                chars.next();
                tokens.push(Token::And);
            }
            '|' => {
                chars.next();
                tokens.push(Token::Or);
            }
            '^' => {
                chars.next();
                tokens.push(Token::Xor);
            }
            '{' | '(' => {
                chars.next();
                tokens.push(Token::Open);
            }
            '}' | ')' => {
                chars.next();
                tokens.push(Token::Close);
            }
            other => {
                return Err(Error::Sieve(format!(
                    "unexpected character {other:?} in sieve {expression:?}"
                )));
            }
        }
    }

    if tokens.is_empty() {
        return Err(Error::Sieve("sieve expression is empty".to_string()));
    }
    Ok(tokens)
}

struct Parser<'a> {
    tokens: &'a [Token],
    position: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<Token> {
        self.tokens.get(self.position).copied()
    }

    fn eat(&mut self, token: Token) -> bool {
        if self.peek() == Some(token) {
            self.position += 1;
            return true;
        }
        false
    }

    fn parse_or(&mut self) -> Result<Node> {
        let mut left = self.parse_xor()?;
        while self.eat(Token::Or) {
            let right = self.parse_xor()?;
            left = Node::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_xor(&mut self) -> Result<Node> {
        let mut left = self.parse_and()?;
        while self.eat(Token::Xor) {
            let right = self.parse_and()?;
            left = Node::Xor(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Node> {
        let mut left = self.parse_unary()?;
        while self.eat(Token::And) {
            let right = self.parse_unary()?;
            left = Node::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Node> {
        if self.eat(Token::Not) {
            return Ok(Node::Not(Box::new(self.parse_unary()?)));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Node> {
        if self.eat(Token::Open) {
            let inner = self.parse_or()?;
            if !self.eat(Token::Close) {
                return Err(Error::Sieve("unclosed group in sieve".to_string()));
            }
            return Ok(inner);
        }

        let Some(Token::Number(modulus)) = self.peek() else {
            return Err(Error::Sieve(format!(
                "expected a modulus in sieve at token {}",
                self.position
            )));
        };
        self.position += 1;

        if modulus == 0 {
            return Err(Error::Sieve("sieve modulus must be non-zero".to_string()));
        }

        // A bare modulus means a shift of zero, as music21's `5` is `5@0`.
        let shift = if self.eat(Token::At) {
            let Some(Token::Number(shift)) = self.peek() else {
                return Err(Error::Sieve(
                    "expected a shift after `@` in sieve".to_string(),
                ));
            };
            self.position += 1;
            shift
        } else {
            0
        };

        Ok(Node::Residual {
            modulus,
            shift: shift % modulus,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn widths(expression: &str) -> Vec<IntegerType> {
        Sieve::parse(expression)
            .expect("sieve parses")
            .interval_widths()
            .expect("sieve has intervals")
    }

    #[test]
    fn a_single_residual_class_cycles_at_its_modulus() {
        assert_eq!(widths("3@0"), [3]);
        assert_eq!(widths("4@0"), [4]);
        assert_eq!(widths("2@0"), [2]);
        assert_eq!(widths("12@0"), [12]);
        // A bare modulus is a shift of zero.
        assert_eq!(widths("5"), [5]);
    }

    #[test]
    fn the_major_scale_is_a_sieve() {
        assert_eq!(
            widths("(-3@2 & 4) | (-3@1 & 4@1) | (3@2 & 4@2) | (-3 & 4@3)"),
            [2, 2, 1, 2, 2, 2, 1]
        );
    }

    #[test]
    fn union_intersection_and_symmetric_difference_match_music21() {
        assert_eq!(widths("3@0|7@0"), [3, 3, 1, 2, 3, 2, 1, 3, 3]);
        assert_eq!(widths("{3@0|4@0}"), [3, 1, 2, 2, 1, 3]);
        assert_eq!(widths("3@0&4@0"), [12]);
        assert_eq!(widths("3@0^4@0"), [1, 2, 2, 1]);
        assert_eq!(widths("5@2|7@3"), [1, 4, 3, 2, 5, 5, 2, 3, 4, 1]);
    }

    #[test]
    fn negation_applies_to_residuals_and_to_groups() {
        assert_eq!(widths("-3@0"), [1]);
        assert_eq!(widths("-5@2"), [1, 2, 1, 1]);
        assert_eq!(widths("-{3@0|4@0}"), [1, 3, 2, 3, 1]);
    }

    #[test]
    fn and_binds_tighter_than_or() {
        // music21 parses 3@0|4@0&6@0 as 3@0|{4@0&6@0}.
        assert_eq!(widths("3@0|4@0&6@0"), widths("3@0|{4@0&6@0}"));
        assert_eq!(widths("3@0|4@0&6@0"), [3, 3, 3, 3]);
        assert_ne!(widths("3@0|4@0&6@0"), widths("{3@0|4@0}&6@0"));
        assert_eq!(widths("{3@0|4@0}&6@0"), [6, 6]);
    }

    #[test]
    fn parentheses_and_braces_group_alike() {
        assert_eq!(widths("(3@0|4@0)"), widths("{3@0|4@0}"));
    }

    #[test]
    fn period_is_the_lcm_of_the_moduli() {
        assert_eq!(Sieve::parse("3@0").unwrap().period(), 3);
        assert_eq!(Sieve::parse("3@0|7@0").unwrap().period(), 21);
        assert_eq!(Sieve::parse("5@2|7@3").unwrap().period(), 35);
        assert_eq!(Sieve::parse("3@0|4@0").unwrap().period(), 12);
    }

    #[test]
    fn a_sieve_too_sparse_for_intervals_errors() {
        // music21 raises "interval segment has no values" for this: 3@1 has
        // only the member 1 in 0..=3.
        assert!(Sieve::parse("3@1").unwrap().interval_widths().is_err());
    }

    #[test]
    fn malformed_expressions_error_instead_of_panicking() {
        for bad in [
            "", "  ", "@", "3@", "|3@0", "3@0|", "(3@0", "3@0)", "0@0", "3@0 & ", "x", "3@@0",
        ] {
            let parsed = Sieve::parse(bad);
            assert!(parsed.is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn membership_wraps_for_negative_integers() {
        let sieve = Sieve::parse("3@1").unwrap();
        assert!(sieve.contains(1));
        assert!(sieve.contains(4));
        assert!(sieve.contains(-2));
        assert!(!sieve.contains(0));
    }
}
