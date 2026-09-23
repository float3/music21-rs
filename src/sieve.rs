//! Xenakis sieves, ported from music21's `sieve` module.
//!
//! A sieve is a logical expression over *residual classes*. `3@0` selects every
//! integer congruent to 0 modulo 3; `|`, `&` and `^` combine classes as union,
//! intersection and symmetric difference; `-` complements one; and `{}` or `()`
//! group. Applied to semitones, the resulting integer set is a scale — the
//! major scale is `(-3@2 & 4) | (-3@1 & 4@1) | (3@2 & 4@2) | (-3 & 4@3)`.
//!
//! What is here is the sieve itself — parsing an expression, testing
//! membership, the segment formats and the interval widths of one period —
//! and the number helpers music21 keeps beside it: primes by
//! [`eratosthenes`] and [`rabin_miller`], and the unit-interval spacings
//! [`unit_norm_range`], [`unit_norm_equal`] and [`unit_norm_step`].
//! [`Sieve::compressed`] is the compressed reading; pitch-range realization
//! stays music21's.

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

    /// Whether the expression is more than runs of `&` joined by `|`: a
    /// group, a `^`, or a complement of anything but a single class.
    ///
    /// music21 compresses such a sieve from its members over a range rather
    /// than by intersecting its classes.
    fn is_bound(&self) -> bool {
        match self {
            Self::Residual { .. } => false,
            Self::Group(_) | Self::Xor(..) => true,
            Self::Not(inner) => !matches!(**inner, Self::Residual { .. }),
            Self::And(left, right) | Self::Or(left, right) => left.is_bound() || right.is_bound(),
        }
    }

    /// The runs of `&` an unbound expression joins with `|`, in the order
    /// written, each as the classes it intersects.
    fn collect_or_groups<'a>(&'a self, out: &mut Vec<Vec<&'a Node>>) {
        if let Self::Or(left, right) = self {
            left.collect_or_groups(out);
            right.collect_or_groups(out);
        } else {
            let mut group = Vec::new();
            self.collect_and_leaves(&mut group);
            out.push(group);
        }
    }

    fn collect_and_leaves<'a>(&'a self, out: &mut Vec<&'a Node>) {
        if let Self::And(left, right) = self {
            left.collect_and_leaves(out);
            right.collect_and_leaves(out);
        } else {
            out.push(self);
        }
    }

    /// The one class a run of `&` meets in. A class standing alone is kept
    /// as written, complemented or not; a complemented one cannot be
    /// intersected, as it cannot upstream.
    fn intersection_of(leaves: Vec<&Node>) -> Result<Node> {
        if let [alone] = leaves[..] {
            return Ok(alone.clone());
        }
        let mut met: Option<(i128, i128)> = None;
        for leaf in leaves {
            let Self::Residual { modulus, shift } = leaf else {
                return Err(Error::Sieve(
                    "complemented residual classes cannot be intersected".to_string(),
                ));
            };
            let next = (i128::from(*modulus), i128::from(*shift));
            met = Some(match met {
                None => next,
                Some(so_far) => intersect_classes(so_far, next)?,
            });
        }
        let (modulus, shift) = met.expect("a run of `&` holds at least one class");
        let modulus = UnsignedIntegerType::try_from(modulus)
            .map_err(|_| Error::Sieve(format!("the modulus {modulus} is too large")))?;
        Ok(Self::Residual {
            modulus,
            shift: shift as UnsignedIntegerType,
        })
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
    /// `a.intersection(&b)` is written `{a}&{b}`. Note that music21's
    /// `a & b` writes the two the other way round, `{b}&{a}`.
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

    /// The compressed reading of the sieve: a union of residual classes with
    /// the same members, which is music21's `cmp` state.
    ///
    /// A sieve written with nothing but `&` and `|` is compressed by
    /// intersection: each run of `&` becomes the one class its residuals
    /// meet in, whatever the range. Anything written with a group, a `^` or
    /// a complemented group is compressed from its members in `low..=high`
    /// instead, by finding for each member in turn the smallest modulus
    /// whose class holds nothing the sieve does not, so that reading depends
    /// on the range it was taken over.
    ///
    /// It is an error where there is no such reading: a complemented class
    /// intersected with another, classes that never meet, or fewer than two
    /// members in the range.
    ///
    /// ```
    /// use music21_rs::Sieve;
    ///
    /// assert_eq!(Sieve::parse("2&4&8|5")?.compressed(0, 99)?.to_string(), "8@0|5@0");
    /// assert_eq!(Sieve::parse("(5|2)&4&8")?.compressed(0, 99)?.to_string(), "8@0");
    /// assert_eq!(
    ///     Sieve::parse("3@0^4@0")?.compressed(0, 99)?.to_string(),
    ///     "6@3|12@4|12@6|12@8"
    /// );
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    pub fn compressed(&self, low: IntegerType, high: IntegerType) -> Result<Self> {
        let classes = if self.root.is_bound() {
            self.classes_of_segment(low, high)?
        } else {
            let mut groups = Vec::new();
            self.root.collect_or_groups(&mut groups);
            groups
                .into_iter()
                .map(Node::intersection_of)
                .collect::<Result<Vec<_>>>()?
        };
        let root = classes
            .into_iter()
            .reduce(|left, right| Node::Or(Box::new(left), Box::new(right)))
            .ok_or_else(|| Error::Sieve(format!("sieve {self} has no residual classes")))?;
        Ok(Self { root })
    }

    /// music21's `CompressionSegment` over the members in `low..=high`.
    fn classes_of_segment(&self, low: IntegerType, high: IntegerType) -> Result<Vec<Node>> {
        let members = self.segment(low, high);
        if members.len() < 2 {
            return Err(Error::Sieve(format!(
                "sieve {self} has {} member(s) in {low}..={high}; compressing a segment takes more than one",
                members.len()
            )));
        }
        classes_of_members(&members, low, high)
    }
}

/// music21's `CompressionSegment`: sorted, distinct `members` of `low..=high`
/// written as residual classes, each run taken with the smallest modulus that
/// stays inside them.
fn classes_of_members(
    members: &[IntegerType],
    low: IntegerType,
    high: IntegerType,
) -> Result<Vec<Node>> {
    let is_member = |z: IntegerType| members.binary_search(&z).is_ok();
    // music21 tries every modulus below the length of the range.
    let span = high - low + 1;

    let mut remaining = members.to_vec();
    let mut classes: Vec<(IntegerType, IntegerType)> = Vec::new();
    while let Some(&n) = remaining.first() {
        let modulus = (1..span)
            .find(|m| {
                let first = low + (n - low).rem_euclid(*m);
                (first..=high).step_by(*m as usize).all(is_member)
            })
            .ok_or_else(|| Error::Sieve(format!("a mod was not found less than {span} for {n}")))?;
        remaining.retain(|z| (z - n).rem_euclid(modulus) != 0);
        classes.push((modulus, n.rem_euclid(modulus)));
    }
    classes.sort_unstable();
    Ok(classes
        .into_iter()
        .map(|(modulus, shift)| Node::Residual {
            modulus: modulus as UnsignedIntegerType,
            shift: shift as UnsignedIntegerType,
        })
        .collect())
}

impl Sieve {
    /// The sieve that is these integers, as music21 builds one from a list:
    /// each run of them a residual class, found smallest modulus first, over
    /// `range` or else from the least of them to the greatest.
    ///
    /// ```
    /// use music21_rs::Sieve;
    ///
    /// assert_eq!(Sieve::from_segment(&[2, 4, 6, 8], None)?.to_string(), "2@0");
    /// assert_eq!(
    ///     Sieve::from_segment(&[2, 4, 6, 8], Some((0, 19)))?.to_string(),
    ///     "12@8|14@6|16@4|18@2"
    /// );
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Fewer than two distinct integers, or one outside `range`.
    pub fn from_segment(
        members: &[IntegerType],
        range: Option<(IntegerType, IntegerType)>,
    ) -> Result<Self> {
        let mut members = members.to_vec();
        members.sort_unstable();
        members.dedup();
        let (&least, &greatest) = match (members.first(), members.last()) {
            (Some(least), Some(greatest)) if members.len() > 1 => (least, greatest),
            _ => {
                return Err(Error::Sieve(
                    "segment must have more than one element".to_string(),
                ));
            }
        };
        let (low, high) = range.unwrap_or((least, greatest));
        if least < low || greatest > high {
            return Err(Error::Sieve(
                "z range must be a superset of desired segment".to_string(),
            ));
        }
        let root = classes_of_members(&members, low, high)?
            .into_iter()
            .reduce(|left, right| Node::Or(Box::new(left), Box::new(right)))
            .ok_or_else(|| Error::Sieve("segment has no residual classes".to_string()))?;
        Ok(Self { root })
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

/// The least common multiple, with nought where either side is nought.
fn lcm(a: UnsignedIntegerType, b: UnsignedIntegerType) -> UnsignedIntegerType {
    if a == 0 || b == 0 {
        return 0;
    }
    num::integer::lcm(a, b)
}

/// The class two residual classes meet in, each given as modulus and shift:
/// music21's `Residual.__and__`, after Xenakis.
///
/// Two classes whose moduli share a factor meet only when their shifts agree
/// modulo it, and otherwise not at all.
fn intersect_classes((m1, n1): (i128, i128), (m2, n2): (i128, i128)) -> Result<(i128, i128)> {
    let d = num::integer::gcd(m1, m2);
    if (n1 - n2) % d != 0 {
        return Err(Error::Sieve(format!(
            "the classes {m1}@{n1} and {m2}@{n2} never meet"
        )));
    }
    let (c1, c2) = (m1 / d, m2 / d);
    let modulus = c1 * c2 * d;
    // The g with g * c1 = 1 (mod c2), which exists because c1 and c2 share
    // no factor.
    let g = num::Integer::extended_gcd(&c1, &c2).x.rem_euclid(c2);
    Ok((modulus, (n1 + g * (n2 - n1) * c1).rem_euclid(modulus)))
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
            // music21 writes a residual as `M@N`, `M,N` or `MsubN`.
            '@' | ',' => {
                chars.next();
                tokens.push(Token::At);
            }
            // music21's other spellings: `and` and Xenakis's `*` intersect,
            // `or` and `+` unite, `not` complements, and `sub` joins a
            // modulus to its shift.
            'a'..='z' => {
                let mut word = String::new();
                while let Some(&letter) = chars.peek() {
                    if !letter.is_ascii_lowercase() {
                        break;
                    }
                    word.push(letter);
                    chars.next();
                }
                tokens.push(match word.as_str() {
                    "sub" => Token::At,
                    "and" => Token::And,
                    "or" => Token::Or,
                    "not" => Token::Not,
                    _ => {
                        return Err(Error::Sieve(format!(
                            "unexpected word {word:?} in sieve {expression:?}"
                        )));
                    }
                });
            }
            '*' => {
                chars.next();
                tokens.push(Token::And);
            }
            '+' => {
                chars.next();
                tokens.push(Token::Or);
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
            '{' | '(' | '[' => {
                chars.next();
                tokens.push(Token::Open);
            }
            '}' | ')' | ']' => {
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

/// The primes in order from `first_candidate` up: music21's `eratosthenes`.
///
/// An incremental sieve: each prime found is filed under its next multiple,
/// so a candidate that is nobody's multiple is prime and the primes need no
/// upper bound. The iterator is endless.
pub fn eratosthenes(first_candidate: u64) -> impl Iterator<Item = u64> {
    let mut composites: std::collections::HashMap<u64, u64> = std::collections::HashMap::new();
    let mut candidate: u64 = 2;
    std::iter::from_fn(move || {
        loop {
            let found = candidate;
            candidate += 1;
            match composites.remove(&found) {
                Some(prime) => {
                    let mut next = found + prime;
                    while composites.contains_key(&next) {
                        next += prime;
                    }
                    composites.insert(next, prime);
                }
                None => {
                    composites.insert(found * found, found);
                    if found >= first_candidate {
                        return Some(found);
                    }
                }
            }
        }
    })
}

/// The witnesses that make Miller-Rabin exact for every number below
/// 2<sup>64</sup>, so the answer is never a probability.
const MILLER_RABIN_WITNESSES: [u64; 12] = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37];

fn power_mod(mut base: u64, mut exponent: u64, modulus: u64) -> u64 {
    let mut result: u64 = 1;
    base %= modulus;
    while exponent > 0 {
        if exponent & 1 == 1 {
            result = ((u128::from(result) * u128::from(base)) % u128::from(modulus)) as u64;
        }
        base = ((u128::from(base) * u128::from(base)) % u128::from(modulus)) as u64;
        exponent >>= 1;
    }
    result
}

/// Whether a number is prime: music21's `rabinMiller`, answered for the
/// number's magnitude, so a negative number is as prime as its opposite.
///
/// music21 tests random witnesses and answers "probably"; the witnesses
/// here are the ones that decide every 64-bit number exactly.
pub fn rabin_miller(n: i64) -> bool {
    let n = n.unsigned_abs();
    if n < 2 {
        return false;
    }
    if n < 4 {
        return true;
    }
    if !matches!(n % 6, 1 | 5) {
        return false;
    }
    for witness in MILLER_RABIN_WITNESSES {
        if n == witness {
            return true;
        }
        if n.is_multiple_of(witness) {
            return false;
        }
    }
    let (mut odd, mut rounds) = (n - 1, 0);
    while odd.is_multiple_of(2) {
        odd /= 2;
        rounds += 1;
    }
    'witnesses: for witness in MILLER_RABIN_WITNESSES {
        let mut x = power_mod(witness, odd, n);
        if x == 1 || x == n - 1 {
            continue;
        }
        for _ in 1..rounds {
            x = ((u128::from(x) * u128::from(x)) % u128::from(n)) as u64;
            if x == n - 1 {
                continue 'witnesses;
            }
        }
        return false;
    }
    true
}

/// A set of integers as a run of ones and zeros over its range: music21's
/// `discreteBinaryPad`, so `[3, 10, 12]` is a one, six noughts, a one, a
/// nought and a one.
///
/// The range is the smallest to the largest member unless `fix_range` gives
/// another, in which case its own smallest and largest bound the run. An
/// empty series with no range is an error, since it has no range of its own.
pub fn discrete_binary_pad(
    series: &[IntegerType],
    fix_range: Option<&[IntegerType]>,
) -> Result<Vec<u8>> {
    let bounds = fix_range.unwrap_or(series);
    let (Some(lowest), Some(highest)) = (bounds.iter().min(), bounds.iter().max()) else {
        return Err(Error::Sieve(
            "a binary pad needs a range: give a series or a fixRange".to_string(),
        ));
    };
    Ok((*lowest..=*highest)
        .map(|value| u8::from(series.contains(&value)))
        .collect())
}

/// Numbers spaced across the unit interval in proportion to where each
/// falls between the smallest and the largest: music21's `unitNormRange`,
/// so `[0, 3, 4]` is `[0, 0.75, 1]`.
///
/// `fix_range` bounds the interval with a range other than the series' own.
/// A series of one number answers nought, and so does every number of a
/// series with no spread at all.
pub fn unit_norm_range(series: &[FloatType], fix_range: Option<&[FloatType]>) -> Vec<FloatType> {
    let bounds = fix_range.unwrap_or(series);
    let lowest = bounds
        .iter()
        .copied()
        .fold(FloatType::INFINITY, FloatType::min);
    let highest = bounds
        .iter()
        .copied()
        .fold(FloatType::NEG_INFINITY, FloatType::max);
    let span = highest - lowest;
    if series.len() <= 1 {
        return vec![0.0];
    }
    series
        .iter()
        .map(|value| {
            if span == 0.0 {
                0.0
            } else {
                (value - lowest) / span
            }
        })
        .collect()
}

/// The unit interval cut into `parts` points, nought and one included:
/// music21's `unitNormEqual`, so three parts are `[0, 0.5, 1]`. One part or
/// none is a single nought.
pub fn unit_norm_equal(parts: usize) -> Vec<FloatType> {
    match parts {
        0 | 1 => vec![0.0],
        2 => vec![0.0, 1.0],
        _ => {
            let step = 1.0 / (parts - 1) as FloatType;
            let mut unit: Vec<FloatType> = (0..parts - 1).map(|y| y as FloatType * step).collect();
            unit.push(1.0);
            unit
        }
    }
}

/// The values a step of `step` reaches from `a` to `b` inclusive, either as
/// they are or normalized onto the unit interval: music21's `unitNormStep`.
/// A range of no width answers nothing; a step of no width cannot cross one
/// and is an error.
pub fn unit_norm_step(
    step: FloatType,
    a: FloatType,
    b: FloatType,
    normalized: bool,
) -> Result<Vec<FloatType>> {
    if a == b {
        return Ok(Vec::new());
    }
    if step.is_nan() || step <= 0.0 {
        return Err(Error::Sieve(format!(
            "a step of {step} never crosses the range"
        )));
    }
    let (lowest, highest) = if a < b { (a, b) } else { (b, a) };
    let mut values = Vec::new();
    let mut x = lowest;
    while x <= highest {
        values.push(x);
        x += step;
    }
    Ok(if normalized {
        unit_norm_equal(values.len())
    } else {
        values
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every answer here was read off music21's own docstrings.
    #[test]
    fn the_number_helpers_answer_what_music21_answers() {
        assert_eq!(
            eratosthenes(2).take(5).collect::<Vec<_>>(),
            [2, 3, 5, 7, 11]
        );
        assert_eq!(eratosthenes(95).take(2).collect::<Vec<_>>(), [97, 101]);

        assert!(!rabin_miller(234));
        assert!(rabin_miller(5));
        assert!(!rabin_miller(4));
        assert!(!rabin_miller(97 * 2));
        assert!(rabin_miller(6_i64.pow(4) + 1));
        assert!(!rabin_miller(123_986_234_193));
        assert!(rabin_miller(-7));
        assert!(!rabin_miller(1));
        assert!(rabin_miller(1_000_000_007));
        assert!(!rabin_miller(1_000_000_007 * 3));

        assert_eq!(
            discrete_binary_pad(&[3, 10, 12], None).unwrap(),
            [1, 0, 0, 0, 0, 0, 0, 1, 0, 1]
        );
        assert_eq!(discrete_binary_pad(&[3, 4, 5], None).unwrap(), [1, 1, 1]);
        assert_eq!(
            discrete_binary_pad(&[4], Some(&[2, 5])).unwrap(),
            [0, 0, 1, 0]
        );
        assert!(discrete_binary_pad(&[], None).is_err());

        assert_eq!(unit_norm_range(&[0.0, 3.0, 4.0], None), [0.0, 0.75, 1.0]);
        let thirds = unit_norm_range(&[1.0, 3.0, 4.0], None);
        assert!((thirds[1] - 2.0 / 3.0).abs() < 1e-12);
        assert_eq!(unit_norm_range(&[5.0], None), [0.0]);
        assert_eq!(unit_norm_range(&[2.0, 2.0], None), [0.0, 0.0]);
        assert_eq!(unit_norm_range(&[1.0, 2.0], Some(&[0.0, 4.0])), [0.25, 0.5]);

        assert_eq!(unit_norm_equal(3), [0.0, 0.5, 1.0]);
        assert_eq!(unit_norm_equal(1), [0.0]);
        assert_eq!(unit_norm_equal(2), [0.0, 1.0]);

        assert_eq!(
            unit_norm_step(0.5, 0.0, 1.0, true).unwrap(),
            [0.0, 0.5, 1.0]
        );
        assert_eq!(
            unit_norm_step(0.5, -1.0, 1.0, true).unwrap(),
            [0.0, 0.25, 0.5, 0.75, 1.0]
        );
        assert_eq!(
            unit_norm_step(0.5, -1.0, 1.0, false).unwrap(),
            [-1.0, -0.5, 0.0, 0.5, 1.0]
        );
        assert_eq!(unit_norm_step(0.25, 0.0, 20.0, true).unwrap().len(), 81);
        assert_eq!(unit_norm_step(0.25, 0.0, 20.0, false).unwrap().len(), 81);
        assert!(unit_norm_step(0.5, 1.0, 1.0, true).unwrap().is_empty());
        assert!(unit_norm_step(0.0, 0.0, 1.0, true).is_err());
    }

    /// music21 writes a sieve back out as it was given, with the residuals
    /// normalized and the groups kept. Every string here was read off
    /// music21 11.0.0b9.
    #[test]
    fn compression_answers_what_music21_answers() {
        let compressed = |expression: &str, low, high| {
            Sieve::parse(expression)
                .unwrap()
                .compressed(low, high)
                .map(|sieve| sieve.to_string())
        };
        // By intersection, whatever the range.
        assert_eq!(compressed("3@11", 0, 99).unwrap(), "3@2");
        assert_eq!(compressed("2&4&8|5", 0, 99).unwrap(), "8@0|5@0");
        assert_eq!(compressed("3@2&5@1", 0, 99).unwrap(), "15@11");
        assert_eq!(compressed("4@1&6@3", 0, 99).unwrap(), "12@9");
        assert_eq!(compressed("5@2&4@1&3@0|7", 0, 99).unwrap(), "60@57|7@0");
        assert_eq!(compressed("3@1|3@1&2", 0, 99).unwrap(), "3@1|6@4");
        assert_eq!(compressed("-3@0|5", 0, 99).unwrap(), "-3@0|5@0");
        // From the members of a range.
        assert_eq!(compressed("(5|2)&4&8", 0, 99).unwrap(), "8@0");
        assert_eq!(compressed("(5|2)&4&8", 20, 49).unwrap(), "8@0");
        assert_eq!(compressed("3@0^4@0", 0, 99).unwrap(), "6@3|12@4|12@6|12@8");
        assert_eq!(compressed("-(3|4)", 0, 99).unwrap(), "6@1|6@5|12@2|12@10");
        assert_eq!(compressed("-{3}", 0, 99).unwrap(), "3@1|3@2");
        assert_eq!(compressed("7@3|{-3@0&5}", 0, 99).unwrap(), "7@3|15@5|15@10");
        assert_eq!(compressed("{-4@1}&{3@0}", 0, 99).unwrap(), "6@0|12@3");
        assert_eq!(compressed("{7}", 0, 7).unwrap(), "7@0");

        let sieve = Sieve::parse("(5|2)&4&8").unwrap();
        let compressed_sieve = sieve.compressed(0, 99).unwrap();
        assert_eq!(compressed_sieve.period(), 8);
        assert_eq!(compressed_sieve.segment_widths(0, 99), [8; 12]);
        assert_eq!(compressed_sieve.segment(0, 99), sieve.segment(0, 99));

        // No such reading.
        assert!(compressed("-3@0&5", 0, 99).is_err());
        assert!(compressed("4@1&6@2", 0, 99).is_err());
        assert!(compressed("{7}", 5, 8).is_err());
    }

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

    /// music21 reads brackets as a group, and a residual written `M,N` or
    /// `MsubN` as `M@N`; the answers and the written forms are music21's.
    #[test]
    fn every_spelling_music21_reads_is_read() {
        let bracketed = Sieve::parse("[(8@0 | 8@1 | 8@7) & (5@1 | 5@3)] | [8@3 & 5@0]").unwrap();
        assert_eq!(bracketed.segment(0, 39), [1, 8, 16, 23, 31, 33, 35]);
        assert_eq!(bracketed.to_string(), "{{8@0|8@1|8@7}&{5@1|5@3}}|{8@3&5@0}");

        let sub = Sieve::parse("-5 | 4 & 4sub3 & 6").unwrap();
        assert_eq!(
            sub.segment(0, 29),
            [
                1, 2, 3, 4, 6, 7, 8, 9, 11, 12, 13, 14, 16, 17, 18, 19, 21, 22, 23, 24, 26, 27, 28,
                29
            ]
        );
        assert_eq!(sub.to_string(), "-5@0|4@0&4@3&6@0");

        let comma = Sieve::parse("3,1 | 4").unwrap();
        assert_eq!(comma.segment(0, 19), [0, 1, 4, 7, 8, 10, 12, 13, 16, 19]);
        assert_eq!(comma.to_string(), "3@1|4@0");

        assert!(Sieve::parse("4su3").is_err());

        let words = Sieve::parse("2 or 4 and 4 & 6 or 4 & 4").unwrap();
        assert_eq!(words.to_string(), "2@0|4@0&4@0&6@0|4@0&4@0");
        assert_eq!(words.segment(0, 9), [0, 2, 4, 6, 8]);
        let xenakis = Sieve::parse("3 * 1 + not 4").unwrap();
        assert_eq!(xenakis.to_string(), "3@0&1@0|-4@0");
        assert_eq!(xenakis.segment(0, 9), [0, 1, 2, 3, 5, 6, 7, 9]);
        assert!(Sieve::parse("3 xor 4").is_err());
    }

    /// music21's `Sieve` given a list of integers, with and without a range.
    #[test]
    fn a_segment_is_written_as_music21_writes_it() {
        let written = |members: &[IntegerType], range| {
            Sieve::from_segment(members, range).map(|sieve| sieve.to_string())
        };
        assert_eq!(written(&[2, 4, 6, 8], None).unwrap(), "2@0");
        assert_eq!(written(&[1, 6, 11, 16, 17], None).unwrap(), "5@1|11@6");
        assert_eq!(written(&[0, 3, 4, 9], None).unwrap(), "5@4|6@3|9@0");
        assert_eq!(
            written(&[2, 4, 6, 8], Some((0, 19))).unwrap(),
            "12@8|14@6|16@4|18@2"
        );
        assert_eq!(
            Sieve::from_segment(&[1, 6, 11, 16, 17], None)
                .unwrap()
                .segment(0, 29),
            [1, 6, 11, 16, 17, 21, 26, 28]
        );
        assert!(written(&[5], None).is_err());
        assert!(written(&[2, 40], Some((0, 19))).is_err());
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
