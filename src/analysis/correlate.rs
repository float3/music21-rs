//! How what a stream's notes sound goes with the dynamics they sound under:
//! music21's `analysis.correlate`.

use crate::{
    defaults::FloatType,
    dynamics::SHORT_NAMES,
    error::{Error, Result},
    pitch::Pitch,
    stream::{Stream, StreamElement},
};

/// Each pitch of each note and chord of a stream flattened, with each
/// dynamic it sounds under -- the dynamic's place in [`SHORT_NAMES`], from
/// `pppppp` at nought to `ffffff` -- one pair for each: music21's
/// `ActivityMatch.pitchToDynamic`. A dynamic holds from its offset to the
/// next dynamic's, the last to the end of the stream, and a note starting
/// just where one dynamic ends and the next begins sounds under both.
/// music21 reads where a dynamic ends as its offset plus its length in
/// Python's arithmetic, which rounds a sum of an offset in quarters and a
/// length in thirds, so a note on a third of a beat can fall just after
/// the end and sound under the next dynamic alone; this reads them so too.
///
/// # Errors
///
/// A stream with no notes or chords, or no dynamics, or a dynamic not one
/// of [`SHORT_NAMES`], such as `sfz`, which music21 cannot place either.
pub fn pitch_to_dynamic(stream: &Stream) -> Result<Vec<(FloatType, usize)>> {
    let flat = stream.flatten();
    let events = flat.events();
    let notes: Vec<(FloatType, Vec<Pitch>)> = events
        .iter()
        .filter_map(|event| match event.element() {
            StreamElement::Note(note) => Some((event.offset(), vec![note.pitch().clone()])),
            StreamElement::Chord(chord) => Some((event.offset(), chord.pitches())),
            StreamElement::ChordSymbol(symbol) => {
                Some((event.offset(), symbol.pitches().unwrap_or_default()))
            }
            _ => None,
        })
        .collect();
    if notes.is_empty() {
        return Err(missing(
            "(<class 'music21.note.Note'>, <class 'music21.chord.Chord'>)",
        ));
    }
    let dynamics: Vec<(FloatType, &str)> = events
        .iter()
        .filter_map(|event| match event.element() {
            StreamElement::Dynamic(dynamic) => Some((event.offset(), dynamic.value())),
            _ => None,
        })
        .collect();
    if dynamics.is_empty() {
        return Err(missing("<class 'music21.dynamics.Dynamic'>"));
    }
    let end = Offset::of(flat.end_offset());
    let mut under: Vec<Vec<usize>> = vec![Vec::new(); notes.len()];
    for (index, (start, value)) in dynamics.iter().enumerate() {
        let start = Offset::of(*start);
        let next = dynamics
            .get(index + 1)
            .map_or(end, |(next, _)| Offset::of(*next));
        // music21 lengthens each dynamic to the next and reads where it
        // stops as its offset plus that length, in Python's arithmetic.
        let stop = start.plus(next.minus(start).op_frac());
        let level = SHORT_NAMES
            .iter()
            .position(|name| name == value)
            .ok_or_else(|| Error::Analysis(format!("'{value}' is not in list")))?;
        for ((offset, _), heard) in notes.iter().zip(&mut under) {
            let offset = Offset::of(*offset);
            if start.at_most(offset) && offset.at_most(stop) {
                heard.push(level);
            }
        }
    }
    Ok(notes
        .iter()
        .zip(&under)
        .flat_map(|((_, pitches), heard)| {
            pitches
                .iter()
                .flat_map(move |pitch| heard.iter().map(move |level| (pitch.ps(), *level)))
        })
        .collect())
}

/// The same pairs, each once, with how many times it comes, in the order
/// first met: music21's `pitchToDynamic(dataPoints=False)`.
///
/// # Errors
///
/// As [`pitch_to_dynamic`].
pub fn pitch_to_dynamic_counts(stream: &Stream) -> Result<Vec<(FloatType, usize, usize)>> {
    let mut counts: Vec<(FloatType, usize, usize)> = Vec::new();
    for (pitch, level) in pitch_to_dynamic(stream)? {
        match counts
            .iter_mut()
            .find(|(known, known_level, _)| *known == pitch && *known_level == level)
        {
            Some((_, _, count)) => *count += 1,
            None => counts.push((pitch, level, 1)),
        }
    }
    Ok(counts)
}

/// An offset as music21 holds it: a float where its denominator is a
/// power of two, an exact fraction where not, which Python adds and
/// compares by its own rules.
#[derive(Clone, Copy, Debug)]
enum Offset {
    Float(FloatType),
    Fraction(i128, i128),
}

/// A float's exact value as a fraction over a power of two: Python's
/// `float.as_integer_ratio`.
fn integer_ratio(value: FloatType) -> (i128, i128) {
    if value == 0.0 {
        return (0, 1);
    }
    let bits = value.to_bits();
    let sign = if bits >> 63 == 1 { -1 } else { 1 };
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let mut mantissa = (bits & ((1 << 52) - 1)) as i128;
    let mut power = if exponent == 0 {
        -1074
    } else {
        mantissa |= 1 << 52;
        exponent - 1075
    };
    while mantissa % 2 == 0 && power < 0 {
        mantissa /= 2;
        power += 1;
    }
    if power >= 0 {
        (sign * (mantissa << power), 1)
    } else {
        (sign * mantissa, 1 << -power)
    }
}

fn gcd(a: i128, b: i128) -> i128 {
    if b == 0 { a.abs() } else { gcd(b, a % b) }
}

fn power_of_two(denominator: i128) -> bool {
    denominator > 0 && denominator & (denominator - 1) == 0
}

impl Offset {
    /// The offset music21 holds where the crate holds this float: music21's
    /// `opFrac` of it.
    fn of(value: FloatType) -> Self {
        Self::Float(value).op_frac()
    }

    /// music21's `opFrac`: a float whose exact denominator is above 65535
    /// read as the nearest fraction with one that is not, and a fraction
    /// over a power of two made a float.
    fn op_frac(self) -> Self {
        match self {
            Self::Float(value) => {
                let (_, denominator) = integer_ratio(value);
                if denominator <= 65535 {
                    return self;
                }
                match crate::duration::limited_fraction(value, 65535) {
                    Some((numerator, denominator)) if !power_of_two(denominator) => {
                        Self::Fraction(numerator, denominator)
                    }
                    Some((numerator, denominator)) => {
                        Self::Float(numerator as FloatType / denominator as FloatType)
                    }
                    None => self,
                }
            }
            Self::Fraction(numerator, denominator) if power_of_two(denominator) => {
                Self::Float(numerator as FloatType / denominator as FloatType)
            }
            fraction => fraction,
        }
    }

    fn as_float(self) -> FloatType {
        match self {
            Self::Float(value) => value,
            Self::Fraction(numerator, denominator) => {
                numerator as FloatType / denominator as FloatType
            }
        }
    }

    /// Python's sum: exact for two fractions, a float otherwise.
    fn plus(self, other: Self) -> Self {
        match (self, other) {
            (Self::Fraction(a, b), Self::Fraction(c, d)) => {
                let (numerator, denominator) = (a * d + c * b, b * d);
                let divisor = gcd(numerator, denominator);
                Self::Fraction(numerator / divisor, denominator / divisor)
            }
            _ => Self::Float(self.as_float() + other.as_float()),
        }
    }

    fn minus(self, other: Self) -> Self {
        let negated = match other {
            Self::Float(value) => Self::Float(-value),
            Self::Fraction(numerator, denominator) => Self::Fraction(-numerator, denominator),
        };
        self.plus(negated)
    }

    /// The exact value as a fraction.
    fn exact(self) -> (i128, i128) {
        match self {
            Self::Float(value) => integer_ratio(value),
            Self::Fraction(numerator, denominator) => (numerator, denominator),
        }
    }

    /// Python's `<=`, which compares a float and a fraction exactly.
    fn at_most(self, other: Self) -> bool {
        let ((a, b), (c, d)) = (self.exact(), other.exact());
        a * d <= c * b
    }
}

fn missing(class: &str) -> Error {
    Error::Analysis(format!(
        "cannot create correlation: an object that is not found in the Stream: {class}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dynamics::Dynamic;
    use crate::tinynotation::from_tiny_notation;

    #[test]
    fn pitches_are_paired_with_the_dynamics_they_sound_under() -> Result<()> {
        let mut line = from_tiny_notation("4/4 c4 d e f")?;
        line.insert(0.0, Dynamic::new("p"));
        line.insert(2.0, Dynamic::new("f"));
        // music21: the note at 2.0 sounds under both, the p's span ending
        // where the f begins.
        assert_eq!(
            pitch_to_dynamic(&line)?,
            [(60.0, 5), (62.0, 5), (64.0, 5), (64.0, 8), (65.0, 8)]
        );
        assert_eq!(pitch_to_dynamic_counts(&line)?.len(), 5);
        assert!(pitch_to_dynamic(&from_tiny_notation("4/4 c4")?).is_err());
        Ok(())
    }
}
