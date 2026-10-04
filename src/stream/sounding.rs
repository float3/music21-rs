//! A stream at sounding pitch and at written pitch: music21's
//! `atSoundingPitch`, `toSoundingPitch` and `toWrittenPitch`.

use super::{Stream, StreamElement, StreamKind};
use crate::defaults::FloatType;
use crate::error::Result;
use crate::interval::Interval;

// Offsets from a flattened walk are sums of floats; two that music21 holds as
// one fraction may differ by a rounding error.
const NEAR: FloatType = 1e-9;

impl Stream {
    /// Whether this stream's notes are written at the pitch they sound
    /// (`Some(true)`), at the pitch a transposing instrument reads
    /// (`Some(false)`), or nothing has said (`None`): music21's
    /// `atSoundingPitch`, whose `'unknown'` is `None`.
    ///
    /// Setting it moves no notes; [`Stream::to_written_pitch`] and
    /// [`Stream::to_sounding_pitch`] do that. A stream that says nothing is
    /// read as the nearest stream holding it that does.
    pub fn at_sounding_pitch(&self) -> Option<bool> {
        self.labels.at_sounding_pitch
    }

    /// Says whether this stream's notes are at sounding pitch.
    pub fn set_at_sounding_pitch(&mut self, at_sounding_pitch: Option<bool>) {
        self.labels.at_sounding_pitch = at_sounding_pitch;
    }

    /// This stream with its notes at the pitch each part's instruments read
    /// them at: music21's `toWrittenPitch`.
    ///
    /// A part at sounding pitch is transposed by the reverse of each of its
    /// instruments' transpositions -- the notes, chords, chord symbols and
    /// key signatures from where the instrument stands to where the next one
    /// does -- and then says it is at written pitch. A part already at
    /// written pitch, or one that says nothing and stands in nothing that
    /// does, is left as it is. A score or an opus is taken part by part, and
    /// then says it is at written pitch too.
    ///
    /// ```
    /// use music21_rs::{Instrument, Note, Stream, StreamElement, StreamKind};
    ///
    /// let mut part = Stream::with_kind(StreamKind::Part);
    /// part.push(StreamElement::Instrument(Box::new(Instrument::of_kind(
    ///     "Clarinet",
    /// )?)));
    /// part.push(StreamElement::Note(Note::from_name("Bb4")?));
    /// part.set_at_sounding_pitch(Some(true));
    ///
    /// let written = part.to_written_pitch()?;
    /// let StreamElement::Note(note) = written.leaves()[1].1 else { unreachable!() };
    /// assert_eq!(note.pitch().name_with_octave(), "C5");
    /// assert_eq!(written.at_sounding_pitch(), Some(false));
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    pub fn to_written_pitch(&self) -> Result<Stream> {
        let mut written = self.clone();
        written.make_written_pitch()?;
        Ok(written)
    }

    /// This stream with its notes at the pitch they sound: music21's
    /// `toSoundingPitch`, [`Stream::to_written_pitch`] the other way round.
    /// A part at written pitch is transposed by each of its instruments'
    /// transpositions, and then says it is at sounding pitch.
    pub fn to_sounding_pitch(&self) -> Result<Stream> {
        let mut sounding = self.clone();
        sounding.make_sounding(None)?;
        Ok(sounding)
    }

    /// [`Stream::to_written_pitch`] in place.
    pub(crate) fn make_written_pitch(&mut self) -> Result<()> {
        self.make_written(None)
    }

    /// `toWrittenPitch(inPlace=True)`, `held_in` being what the nearest
    /// stream holding this one says.
    fn make_written(&mut self, held_in: Option<bool>) -> Result<()> {
        if self.has_part_like_streams() || self.kind == StreamKind::Opus {
            let here = self.at_sounding_pitch().or(held_in);
            for event in &mut self.events {
                if let StreamElement::Stream(inner) = &mut event.element {
                    inner.make_written(here)?;
                }
            }
            self.set_at_sounding_pitch(Some(false));
        } else if self.treat_at_sounding_pitch(held_in)? == Some(true) {
            self.transpose_by_instrument(true)?;
            self.say_everywhere(false);
        }
        Ok(())
    }

    /// `toSoundingPitch(inPlace=True)`.
    fn make_sounding(&mut self, held_in: Option<bool>) -> Result<()> {
        if self.has_part_like_streams() || self.kind == StreamKind::Opus {
            let here = self.at_sounding_pitch().or(held_in);
            for event in &mut self.events {
                if let StreamElement::Stream(inner) = &mut event.element {
                    inner.make_sounding(here)?;
                }
            }
            self.set_at_sounding_pitch(Some(true));
        } else if self.treat_at_sounding_pitch(held_in)? == Some(false) {
            self.transpose_by_instrument(false)?;
            self.say_everywhere(true);
        }
        Ok(())
    }

    /// music21's `_treatAtSoundingPitch`: what this stream says, or what the
    /// nearest stream holding it says; and with that settled, a stream
    /// inside it saying otherwise is brought round to agree.
    fn treat_at_sounding_pitch(&mut self, held_in: Option<bool>) -> Result<Option<bool>> {
        let Some(at) = self.at_sounding_pitch().or(held_in) else {
            return Ok(None);
        };
        self.bring_round(at)?;
        Ok(Some(at))
    }

    /// Every stream inside this one that says the opposite of `at` is
    /// transposed to agree, however deep.
    fn bring_round(&mut self, at: bool) -> Result<()> {
        for event in &mut self.events {
            if let StreamElement::Stream(inner) = &mut event.element {
                match inner.at_sounding_pitch() {
                    Some(said) if said != at && at => inner.make_sounding(None)?,
                    Some(said) if said != at => inner.make_written(None)?,
                    _ => {}
                }
                inner.bring_round(at)?;
            }
        }
        Ok(())
    }

    /// Says `at` of this stream and of every stream inside it.
    fn say_everywhere(&mut self, at: bool) {
        self.set_at_sounding_pitch(Some(at));
        for event in &mut self.events {
            if let StreamElement::Stream(inner) = &mut event.element {
                inner.say_everywhere(at);
            }
        }
    }

    /// music21's `_transposeByInstrument`: each instrument's transposition,
    /// reversed for `reverse`, applied to the notes, chords, chord symbols
    /// and key signatures that begin from where it stands up to where the
    /// next instrument does, the last up to the end of the stream.
    fn transpose_by_instrument(&mut self, reverse: bool) -> Result<()> {
        let leaves = self.leaves();
        let end_of_stream = leaves
            .iter()
            .map(|(offset, element)| offset + element.quarter_length())
            .fold(0.0, FloatType::max);
        let mut instruments: Vec<(FloatType, Option<Interval>)> = leaves
            .iter()
            .filter_map(|(offset, element)| match element {
                StreamElement::Instrument(instrument) => {
                    Some((*offset, instrument.transposition().cloned()))
                }
                _ => None,
            })
            .collect();
        // Stable, so instruments at one offset keep the order they stand in.
        instruments.sort_by(|left, right| left.0.total_cmp(&right.0));
        let mut spans: Vec<(FloatType, FloatType, Interval)> = Vec::new();
        for (index, (start, transposition)) in instruments.iter().enumerate() {
            let Some(transposition) = transposition else {
                continue;
            };
            let end = instruments
                .get(index + 1)
                .map_or(end_of_stream, |next| next.0);
            let interval = if reverse {
                transposition.reversed()?
            } else {
                transposition.clone()
            };
            spans.push((*start, end, interval));
        }
        if spans.is_empty() {
            return Ok(());
        }
        let mut failure = None;
        self.for_each_mut(&mut |offset, element| {
            if failure.is_some() {
                return;
            }
            let Some((_, _, interval)) = spans
                .iter()
                .find(|(start, end, _)| offset > start - NEAR && offset < end - NEAR)
            else {
                return;
            };
            if let Err(error) = transpose_element(element, interval) {
                failure = Some(error);
            }
        });
        failure.map_or(Ok(()), Err)
    }
}

/// An element moved by an instrument's transposition: what music21's
/// `classFilterList` names, a note, a chord (a chord symbol among them) and a
/// key signature (a key among them).
fn transpose_element(element: &mut StreamElement, interval: &Interval) -> Result<()> {
    match element {
        StreamElement::Note(note) => *note = note.transpose(interval)?,
        StreamElement::Chord(chord) => *chord = chord.transpose(interval)?,
        StreamElement::ChordSymbol(symbol) => *symbol = symbol.transpose(interval)?,
        StreamElement::Key(key) => *key = key.transpose(interval)?,
        StreamElement::KeySignature(signature) => *signature = signature.transpose(interval)?,
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::instrument::Instrument;
    use crate::note::Note;
    use crate::stream::{Stream, StreamElement, StreamKind};

    fn names(stream: &Stream) -> Vec<String> {
        stream
            .leaves()
            .iter()
            .filter_map(|(_, element)| match element {
                StreamElement::Note(note) => Some(note.pitch().name_with_octave()),
                _ => None,
            })
            .collect()
    }

    fn part_with(kind: &str, notes: &[&str]) -> Stream {
        let mut part = Stream::with_kind(StreamKind::Part);
        part.push(StreamElement::Instrument(Box::new(
            Instrument::of_kind(kind).unwrap(),
        )));
        for name in notes {
            part.push(StreamElement::Note(Note::from_name(name).unwrap()));
        }
        part
    }

    #[test]
    fn a_part_at_sounding_pitch_is_written_up_for_a_clarinet_and_back() {
        let mut part = part_with("Clarinet", &["Bb4", "D5"]);
        part.set_at_sounding_pitch(Some(true));
        let written = part.to_written_pitch().unwrap();
        assert_eq!(names(&written), ["C5", "E5"]);
        assert_eq!(written.at_sounding_pitch(), Some(false));
        let sounding = written.to_sounding_pitch().unwrap();
        assert_eq!(names(&sounding), ["Bb4", "D5"]);
        assert_eq!(sounding.at_sounding_pitch(), Some(true));
    }

    #[test]
    fn a_part_that_says_nothing_is_left_as_it_is() {
        let part = part_with("Clarinet", &["Bb4"]);
        assert_eq!(names(&part.to_written_pitch().unwrap()), ["Bb4"]);
        assert_eq!(part.to_written_pitch().unwrap().at_sounding_pitch(), None);
    }

    #[test]
    fn a_part_takes_what_the_score_holding_it_says() {
        let mut score = Stream::with_kind(StreamKind::Score);
        score.push(StreamElement::Stream(Box::new(part_with("Horn", &["C4"]))));
        score.set_at_sounding_pitch(Some(true));
        let written = score.to_written_pitch().unwrap();
        // A horn in F sounds a fifth below what it reads.
        assert_eq!(names(&written), ["G4"]);
        assert_eq!(written.at_sounding_pitch(), Some(false));
    }

    #[test]
    fn each_instrument_moves_the_notes_from_where_it_stands() {
        let mut part = part_with("Clarinet", &["Bb4"]);
        part.push(StreamElement::Instrument(Box::new(
            Instrument::of_kind("Flute").unwrap(),
        )));
        part.push(StreamElement::Note(Note::from_name("Bb4").unwrap()));
        part.set_at_sounding_pitch(Some(true));
        assert_eq!(names(&part.to_written_pitch().unwrap()), ["C5", "Bb4"]);
    }
}
