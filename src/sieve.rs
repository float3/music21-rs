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

use std::fmt;

use crate::defaults::{FloatType, IntegerType, UnsignedIntegerType};
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
#[must_use]
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
    /// A group the expression was written with, `{}` or `()`.
    ///
    /// Kept because music21 writes a sieve back out as it was given, with
    /// only the residuals normalized, so `(5|2)&4&8` reads back as
    /// `{5@0|2@0}&4@0&8@0` and a combined sieve wraps each side in braces
    /// whether or not the precedence needs them.
    Group(Box<Node>),
}

impl Node {
    fn contains(&self, z: IntegerType) -> bool {
        match self {
            Self::Residual { modulus, shift } => {
                z.rem_euclid(*modulus as IntegerType) == *shift as IntegerType
            }
            Self::Group(inner) => inner.contains(z),
            Self::Not(inner) => !inner.contains(z),
            Self::And(left, right) => left.contains(z) && right.contains(z),
            Self::Or(left, right) => left.contains(z) || right.contains(z),
            Self::Xor(left, right) => left.contains(z) != right.contains(z),
        }
    }

    /// The same tree with every residual's shift moved on by `n`.
    ///
    /// music21 passes an `n` down to each residual when it reads a segment,
    /// which is the same thing: `3@2` read at `n = 10` selects the multiples
    /// of three.
    fn shifted(&self, n: IntegerType) -> Node {
        match self {
            Self::Residual { modulus, shift } => Self::Residual {
                modulus: *modulus,
                shift: (*shift as IntegerType + n).rem_euclid(*modulus as IntegerType)
                    as UnsignedIntegerType,
            },
            Self::Group(inner) => Self::Group(Box::new(inner.shifted(n))),
            Self::Not(inner) => Self::Not(Box::new(inner.shifted(n))),
            Self::And(left, right) => {
                Self::And(Box::new(left.shifted(n)), Box::new(right.shifted(n)))
            }
            Self::Or(left, right) => {
                Self::Or(Box::new(left.shifted(n)), Box::new(right.shifted(n)))
            }
            Self::Xor(left, right) => {
                Self::Xor(Box::new(left.shifted(n)), Box::new(right.shifted(n)))
            }
        }
    }

    fn collect_moduli(&self, out: &mut Vec<UnsignedIntegerType>) {
        match self {
            Self::Residual { modulus, .. } => out.push(*modulus),
            Self::Group(inner) | Self::Not(inner) => inner.collect_moduli(out),
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

    /// The same sieve with every residual's shift moved on by `n`.
    ///
    /// This is the `n` music21 takes beside a range when it reads a segment.
    pub fn shifted(&self, n: IntegerType) -> Self {
        Self {
            root: self.root.shifted(n),
        }
    }

    /// The sieve over `low..=high` as ones and noughts, one per integer in
    /// the range rather than one per member.
    ///
    /// music21's `segmentFormat='binary'`.
    pub fn segment_binary(&self, low: IntegerType, high: IntegerType) -> Vec<IntegerType> {
        (low..=high)
            .map(|z| IntegerType::from(self.contains(z)))
            .collect()
    }

    /// The widths between consecutive members over `low..=high`, one shorter
    /// than the segment itself.
    ///
    /// music21's `segmentFormat='width'`. Unlike [`Sieve::interval_widths`]
    /// this reads whatever range it is given rather than one period, so it
    /// says nothing about where the pattern repeats.
    pub fn segment_widths(&self, low: IntegerType, high: IntegerType) -> Vec<IntegerType> {
        let members = self.segment(low, high);
        members.windows(2).map(|pair| pair[1] - pair[0]).collect()
    }

    /// Each member's place in `low..=high` as a fraction of the way across
    /// it, so the range's own ends are nought and one.
    ///
    /// music21's `segmentFormat='unit'`. A range with no width answers nought
    /// for every member, as music21 does rather than dividing by it.
    pub fn segment_unit(&self, low: IntegerType, high: IntegerType) -> Vec<FloatType> {
        let members = self.segment(low, high);
        if members.len() < 2 {
            return vec![0.0; members.len().min(1)];
        }
        let span = FloatType::from(high - low);
        if span == 0.0 {
            return vec![0.0; members.len()];
        }
        members
            .into_iter()
            .map(|member| FloatType::from(member - low) / span)
            .collect()
    }

    /// The first `length` members at or above `z_minimum`, reading the sieve
    /// shifted on by `n`.
    ///
    /// music21's `collect`, which walks upward a hundred integers at a time
    /// until it has enough. The walk is bounded, so a sieve with too few
    /// members to fill the length is an error rather than a loop that never
    /// ends.
    pub fn collect(
        &self,
        n: IntegerType,
        z_minimum: IntegerType,
        length: usize,
    ) -> Result<Vec<IntegerType>> {
        const STEP: IntegerType = 100;
        const ROUNDS: usize = 10_000;

        let shifted = self.shifted(n);
        let mut found = Vec::with_capacity(length);
        let mut low = z_minimum;
        for _ in 0..ROUNDS {
            found.extend(shifted.segment(low, low + STEP - 1));
            if found.len() >= length {
                found.truncate(length);
                return Ok(found);
            }
            low += STEP;
        }
        Err(Error::Sieve(format!(
            "desired length of {length} cannot be found in sieve {self}"
        )))
    }

    /// The members of both sieves, written the way music21 writes a combined
    /// sieve: each side in braces around the operator.
    ///
    /// Note the order. music21's `a & b` answers `{b}&{a}`, so the facade
    /// calls this the other way round; the crate keeps the order a reader
    /// would expect.
    pub fn intersection(&self, other: &Self) -> Self {
        self.combined(other, Node::And)
    }

    /// The members of either sieve. See [`Sieve::intersection`] for the
    /// bracketing and the order.
    pub fn union(&self, other: &Self) -> Self {
        self.combined(other, Node::Or)
    }

    /// The members of one sieve or the other but not both. See
    /// [`Sieve::intersection`] for the bracketing and the order.
    pub fn symmetric_difference(&self, other: &Self) -> Self {
        self.combined(other, Node::Xor)
    }

    fn combined(&self, other: &Self, join: fn(Box<Node>, Box<Node>) -> Node) -> Self {
        Self {
            root: join(
                Box::new(Node::Group(Box::new(self.root.clone()))),
                Box::new(Node::Group(Box::new(other.root.clone()))),
            ),
        }
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Residual { modulus, shift } => write!(f, "{modulus}@{shift}"),
            Self::Group(inner) => write!(f, "{{{inner}}}"),
            Self::Not(inner) => write!(f, "-{inner}"),
            Self::And(left, right) => write!(f, "{left}&{right}"),
            Self::Or(left, right) => write!(f, "{left}|{right}"),
            Self::Xor(left, right) => write!(f, "{left}^{right}"),
        }
    }
}

/// Writes the sieve as music21 writes it: the expression it was given, with
/// each residual normalized to `modulus@shift` and the groups it was written
/// with kept.
///
/// ```
/// use music21_rs::Sieve;
///
/// assert_eq!(Sieve::parse("3@11")?.to_string(), "3@2");
/// assert_eq!(Sieve::parse("(5|2)&4&8")?.to_string(), "{5@0|2@0}&4@0&8@0");
/// # Ok::<(), music21_rs::Error>(())
/// ```
impl fmt::Display for Sieve {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.root)
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
            return Ok(Node::Group(Box::new(inner)));
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

    /// music21 writes a sieve back out as it was given, with the residuals
    /// normalized and the groups kept. Every string here was read off
    /// music21 11.0.0b9.
    #[test]
    fn a_sieve_is_written_the_way_music21_writes_one() {
        for (written, expected) in [
            ("3@11", "3@2"),
            ("2&4&8|5", "2@0&4@0&8@0|5@0"),
            ("(5|2)&4&8", "{5@0|2@0}&4@0&8@0"),
            ("3@2|7@1", "3@2|7@1"),
        ] {
            let sieve = Sieve::parse(written).expect("the expression parses");
            assert_eq!(sieve.to_string(), expected, "writing {written}");
        }
    }

    /// music21's `a & b` puts the right operand first and braces both sides.
    #[test]
    fn combining_two_sieves_braces_each_side() {
        let a = Sieve::parse("3@11").expect("a parses");
        let b = Sieve::parse("2&4&8|5").expect("b parses");
        assert_eq!(b.intersection(&a).to_string(), "{2@0&4@0&8@0|5@0}&{3@2}");
        assert_eq!(b.union(&a).to_string(), "{2@0&4@0&8@0|5@0}|{3@2}");
        assert_eq!(
            b.symmetric_difference(&a).to_string(),
            "{2@0&4@0&8@0|5@0}^{3@2}"
        );
    }

    #[test]
    fn collecting_reads_the_sieve_shifted_on_from_a_starting_point() {
        let sieve = Sieve::parse("3@11").expect("the expression parses");
        assert_eq!(
            sieve.collect(10, 100, 10).expect("ten members are found"),
            [102, 105, 108, 111, 114, 117, 120, 123, 126, 129]
        );
    }

    #[test]
    fn the_segment_formats_answer_what_music21_answers() {
        let sieve = Sieve::parse("3@2|7@1").expect("the expression parses");
        assert_eq!(
            sieve.segment_binary(0, 99)[..12],
            [0, 1, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1]
        );
        assert_eq!(
            sieve.segment_widths(0, 99)[..12],
            [1, 3, 3, 3, 3, 1, 2, 3, 2, 1, 3, 3]
        );
        let unit = sieve.segment_unit(0, 99);
        assert_eq!(unit.len(), 43);
        assert_eq!(unit[0], 1.0 / 99.0);
        assert_eq!(*unit.last().expect("the segment is not empty"), 1.0);
    }

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
