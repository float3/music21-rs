//! Counting a stream's pitches: music21's `analysis.pitchAnalysis`.

use crate::{
    error::Result,
    pitch::Pitch,
    stream::{Stream, StreamElement},
};

/// How often each value of some property of a stream's pitches comes, in
/// the order each value is first met: music21's `pitchAttributeCount`, with
/// the property a function of the pitch rather than the name of one.
///
/// The pitches are music21's `Stream.pitches`: those of every note, chord,
/// chord symbol and pitched member of a percussion chord, through every
/// nested stream, in the order the streams hold them. A key's pitches are
/// not among them.
///
/// ```
/// use music21_rs::analysis::pitch_analysis::pitch_attribute_count;
/// use music21_rs::tinynotation::from_tiny_notation;
///
/// let line = from_tiny_notation("4/4 c4 d c e")?;
/// let counts = pitch_attribute_count(&line, |pitch| pitch.name())?;
/// assert_eq!(counts, [("C".to_string(), 2), ("D".to_string(), 1), ("E".to_string(), 1)]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
///
/// # Errors
///
/// A chord symbol whose notes cannot be worked out.
pub fn pitch_attribute_count<K: PartialEq>(
    stream: &Stream,
    attribute: impl Fn(&Pitch) -> K,
) -> Result<Vec<(K, usize)>> {
    let mut counts: Vec<(K, usize)> = Vec::new();
    for pitch in music21_pitches(stream)? {
        let value = attribute(&pitch);
        match counts.iter_mut().find(|(known, _)| *known == value) {
            Some((_, count)) => *count += 1,
            None => counts.push((value, 1)),
        }
    }
    Ok(counts)
}

fn music21_pitches(stream: &Stream) -> Result<Vec<Pitch>> {
    let mut pitches = Vec::new();
    for event in stream.events() {
        match event.element() {
            StreamElement::Stream(inner) => pitches.extend(music21_pitches(inner)?),
            StreamElement::ChordSymbol(symbol) => pitches.extend(symbol.pitches()?),
            element => pitches.extend(element.pitches()),
        }
    }
    Ok(pitches)
}
