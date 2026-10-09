//! Ornaments recognized from the notes they are played as: music21's
//! `alpha.analysis.ornamentRecognizer`.
//!
//! The notes an ornament is played as are its *busy* notes; the note it is
//! written on, where one is given, is its *simple* note. A recognizer
//! answers the ornament the busy notes play, or nothing where they play
//! none.
//!
//! ```
//! use music21_rs::alpha::analysis::ornament_recognizer::TrillRecognizer;
//! use music21_rs::expressions::OrnamentKind;
//! use music21_rs::{Duration, Note, StreamElement};
//!
//! let played: Vec<StreamElement> = ["G4", "A4", "G4", "A4"]
//!     .iter()
//!     .map(|name| Ok(Note::from_name(name)?.with_duration(Duration::new(0.25)?).into()))
//!     .collect::<music21_rs::Result<_>>()?;
//! let trill = TrillRecognizer::default().recognize(&played, &[])?.expect("a trill");
//! assert_eq!(trill.kind(), OrnamentKind::Trill);
//! assert_eq!(trill.quarter_length(), 0.25);
//! # Ok::<(), music21_rs::Error>(())
//! ```

use crate::defaults::{FloatType, IntegerType};
use crate::error::{Error, Result};
use crate::expressions::{Ornament, OrnamentKind};
use crate::interval::{Interval, IntervalDirection};
use crate::makenotation::op_frac;
use crate::note::Note;
use crate::stream::StreamElement;

/// How long all the busy notes last together: the first simple note's
/// length where one is given, else the busy notes' lengths added up.
/// music21's `calculateOrnamentTotalQl`.
pub fn ornament_total_quarter_length(
    busy: &[StreamElement],
    simple: &[StreamElement],
) -> FloatType {
    match simple.first() {
        Some(note) => note.quarter_length(),
        None => op_frac(busy.iter().map(StreamElement::quarter_length).sum()),
    }
}

/// How long each ornamental note lasts, the busy notes taken as one
/// ornament: music21's `calculateOrnamentNoteQl`.
pub fn ornament_note_quarter_length(busy: &[StreamElement], simple: &[StreamElement]) -> FloatType {
    op_frac(ornament_total_quarter_length(busy, simple) / busy.len() as FloatType)
}

/// What music21 says reading the pitch of something that has none.
fn note_of(element: &StreamElement) -> Result<&Note> {
    match element {
        StreamElement::Note(note) => Ok(note),
        other => {
            let class = match other {
                StreamElement::Rest(_) => "Rest",
                StreamElement::Chord(_) => "Chord",
                StreamElement::Unpitched(_) => "Unpitched",
                StreamElement::PercussionChord(_) => "PercussionChord",
                StreamElement::ChordSymbol(_) => "ChordSymbol",
                _ => "Music21Object",
            };
            Err(Error::Analysis(format!(
                "'{class}' object has no attribute 'pitch'"
            )))
        }
    }
}

/// Recognizes busy notes as a trill: music21's `TrillRecognizer`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TrillRecognizer {
    /// Whether the last notes may leave the trill as a nachschlag:
    /// music21's `checkNachschlag`.
    pub check_nachschlag: bool,
    /// The most semitones the two trilled notes may be apart:
    /// `acceptableInterval`.
    pub acceptable_interval: IntegerType,
    /// The fewest busy notes a trill with a nachschlag may have:
    /// `minimumLengthForNachschlag`.
    pub minimum_length_for_nachschlag: usize,
}

impl Default for TrillRecognizer {
    fn default() -> Self {
        Self {
            check_nachschlag: false,
            acceptable_interval: 3,
            minimum_length_for_nachschlag: 5,
        }
    }
}

impl TrillRecognizer {
    /// The trill the busy notes play, or nothing: music21's `recognize`.
    ///
    /// More than two notes, alternating between two pitches no more than
    /// [`TrillRecognizer::acceptable_interval`] apart, are a trill up from
    /// the first, or down where the second is lower; with
    /// [`TrillRecognizer::check_nachschlag`], the second half may leave the
    /// alternation. Given a simple note sounding one of the two pitches,
    /// the trill is from that pitch to the other. Each trilled note lasts
    /// as long as [`ornament_note_quarter_length`] says, and the trill is
    /// spelled with the accidental of the note it goes to.
    ///
    /// # Errors
    ///
    /// A simple note that is not a note, as music21 cannot read its pitch.
    pub fn recognize(
        &self,
        busy: &[StreamElement],
        simple: &[StreamElement],
    ) -> Result<Option<Ornament>> {
        if busy.len() <= 2 {
            return Ok(None);
        }
        let (StreamElement::Note(first), StreamElement::Note(second)) = (&busy[0], &busy[1]) else {
            return Ok(None);
        };
        if (first.pitch().midi() - second.pitch().midi()).abs() > self.acceptable_interval {
            return Ok(None);
        }
        let mut alternating = true;
        let mut reached = 0;
        for (index, element) in busy.iter().enumerate() {
            reached = index;
            let StreamElement::Note(note) = element else {
                return Ok(None);
            };
            let expected = if index % 2 == 0 { first } else { second };
            if note.pitch() != expected.pitch() {
                alternating = false;
                break;
            }
        }
        let nachschlag = if alternating {
            false
        } else if !self.check_nachschlag {
            return Ok(None);
        } else if busy.len() >= self.minimum_length_for_nachschlag
            && reached as FloatType >= busy.len() as FloatType / 2.0
        {
            true
        } else {
            return Ok(None);
        };

        let (start, end) = match simple.first() {
            None => (first, second),
            Some(written) => {
                let midi = note_of(written)?.pitch().midi();
                if midi != first.pitch().midi() && midi != second.pitch().midi() {
                    return Ok(None);
                }
                if midi == second.pitch().midi() {
                    (second, first)
                } else {
                    (first, second)
                }
            }
        };
        let mut trill = Ornament::of_kind(if start.pitch().midi() <= end.pitch().midi() {
            OrnamentKind::Trill
        } else {
            OrnamentKind::InvertedTrill
        });
        trill.set_quarter_length(ornament_note_quarter_length(busy, simple));
        trill.set_nachschlag(nachschlag);
        if let Some(accidental) = end.pitch().written_accidental() {
            trill.set_accidental(Some(accidental.clone()))?;
        }
        Ok(Some(trill))
    }
}

/// Recognizes busy notes as a turn: music21's `TurnRecognizer`.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TurnRecognizer {
    /// The steps a turn may take: music21's `acceptableIntervals`, minor,
    /// major and augmented seconds up and down.
    pub acceptable_intervals: Vec<Interval>,
}

impl Default for TurnRecognizer {
    fn default() -> Self {
        Self {
            acceptable_intervals: ["M2", "M-2", "m2", "m-2", "A2", "A-2"]
                .iter()
                .map(|name| Interval::from_name(*name).expect("a second"))
                .collect(),
        }
    }
}

impl TurnRecognizer {
    /// Whether a turn may take this step: music21's `isAcceptableInterval`.
    pub fn is_acceptable_interval(&self, step: &Interval) -> bool {
        self.acceptable_intervals.contains(step)
    }

    /// The turn the busy notes play, or nothing: music21's `recognize`.
    ///
    /// Four notes a second apart, the second and fourth sounding the same
    /// pitch, the first two steps one way and the last the other way, are
    /// a turn where they start above and an inverted turn where they start
    /// below. Given a simple note, it must sound the turn's main pitch and
    /// last as long as the four notes, within a tenth of a quarter.
    ///
    /// # Errors
    ///
    /// A busy note or a simple note that is not a note, as music21 cannot
    /// read its pitch.
    pub fn recognize(
        &self,
        busy: &[StreamElement],
        simple: &[StreamElement],
    ) -> Result<Option<Ornament>> {
        if busy.len() != 4 {
            return Ok(None);
        }
        if let Some(written) = simple.first() {
            let played: FloatType = busy.iter().map(StreamElement::quarter_length).sum();
            if (written.quarter_length() - played).abs() > 0.1 {
                return Ok(None);
            }
        }
        let main = note_of(&busy[1])?.pitch().midi();
        if main != note_of(&busy[3])?.pitch().midi() {
            return Ok(None);
        }
        if let Some(written) = simple.first()
            && note_of(written)?.pitch().midi() != main
        {
            return Ok(None);
        }
        let mut steps = Vec::new();
        for pair in busy.windows(2) {
            let step =
                Interval::between_pitches(note_of(&pair[0])?.pitch(), note_of(&pair[1])?.pitch())?;
            if !self.is_acceptable_interval(&step) {
                return Ok(None);
            }
            steps.push(step);
        }
        if steps[0].direction() != steps[1].direction()
            || steps[1].direction() == steps[2].direction()
        {
            return Ok(None);
        }
        let mut turn =
            Ornament::of_kind(if steps[0].direction() == IntervalDirection::Descending {
                OrnamentKind::Turn
            } else {
                OrnamentKind::InvertedTurn
            });
        turn.set_quarter_length(ornament_note_quarter_length(busy, simple));
        Ok(Some(turn))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::duration::Duration;
    use crate::key::KeySignature;
    use crate::rest::Rest;

    fn notes(names: &[&str], length: FloatType) -> Vec<StreamElement> {
        names
            .iter()
            .map(|name| {
                StreamElement::Note(
                    Note::from_name(name)
                        .unwrap()
                        .with_duration(Duration::new(length).unwrap()),
                )
            })
            .collect()
    }

    fn quarter(name: &str) -> Vec<StreamElement> {
        notes(&[name], 1.0)
    }

    /// music21's testRecognizeTurn.
    #[test]
    fn turns_are_recognized_as_music21_recognizes_them() {
        let recognizer = TurnRecognizer::default();
        let kind = |busy: &[StreamElement], simple: &[StreamElement]| {
            recognizer
                .recognize(busy, simple)
                .unwrap()
                .map(|turn| turn.kind())
        };
        let even = notes(&["G", "F#", "E", "F#"], 0.25);
        assert_eq!(kind(&even, &[]), Some(OrnamentKind::Turn));
        assert_eq!(kind(&even, &quarter("F#")), Some(OrnamentKind::Turn));
        assert_eq!(kind(&even, &quarter("G-")), Some(OrnamentKind::Turn));
        assert_eq!(kind(&even, &quarter("G")), None);
        assert_eq!(kind(&even, &quarter("A")), None);
        let mut rubato = notes(&["G", "F#", "E", "F#"], 0.25);
        for (element, length) in rubato.iter_mut().zip([0.25, 0.15, 0.2, 0.4]) {
            if let StreamElement::Note(note) = element {
                note.set_duration(Duration::new(length).unwrap());
            }
        }
        assert_eq!(kind(&rubato, &[]), Some(OrnamentKind::Turn));
        assert_eq!(
            kind(&notes(&["E", "F#", "G", "F#"], 0.25), &[]),
            Some(OrnamentKind::InvertedTurn)
        );
        for wrong in [
            &["G", "F#", "E", "D"][..],
            &["E", "G", "A", "G"],
            &["G", "F#", "G", "F#"],
            &["G", "F#", "E", "F#", "E"],
            &["G", "F#", "E"],
        ] {
            assert_eq!(kind(&notes(wrong, 0.25), &[]), None, "{wrong:?}");
        }
        // Four quarters are longer than the quarter they ornament.
        assert_eq!(
            kind(&notes(&["G", "F#", "E", "F#"], 1.0), &quarter("F#")),
            None
        );
    }

    /// music21's testRecognizeTrill.
    #[test]
    fn trills_are_recognized_as_music21_recognizes_them() {
        let plain = TrillRecognizer::default();
        let size = |trill: &Ornament, from: &str| {
            trill
                .size(
                    &crate::pitch::Pitch::from_name(from).unwrap(),
                    &KeySignature::new(0),
                )
                .unwrap()
                .directed_name()
        };
        let gaga = notes(&["G", "A", "G", "A"], 0.25);
        let trill = plain.recognize(&gaga, &[]).unwrap().unwrap();
        assert_eq!(trill.kind(), OrnamentKind::Trill);
        assert_eq!(size(&trill, "G"), "M2");
        let trill = plain.recognize(&gaga, &quarter("A")).unwrap().unwrap();
        assert_eq!(trill.kind(), OrnamentKind::InvertedTrill);
        assert_eq!(size(&trill, "A"), "M-2");
        assert!(plain.recognize(&gaga, &quarter("G##")).unwrap().is_some());
        assert!(plain.recognize(&gaga, &quarter("E")).unwrap().is_none());
        let mut with_rest = gaga.clone();
        with_rest[2] = StreamElement::Rest(Rest::new(Duration::new(0.25).unwrap()));
        assert!(plain.recognize(&with_rest, &[]).unwrap().is_none());

        let a_g = notes(&["A", "G#", "A", "G#", "A"], 0.4);
        let trill = plain.recognize(&a_g, &[]).unwrap().unwrap();
        assert_eq!(trill.kind(), OrnamentKind::InvertedTrill);
        assert_eq!(size(&trill, "A"), "m-2");

        let nachschlag = notes(&["C5", "B", "C5", "B", "C5", "D5", "E5", "F5"], 0.125);
        assert!(plain.recognize(&nachschlag, &[]).unwrap().is_none());
        let checking = TrillRecognizer {
            check_nachschlag: true,
            ..TrillRecognizer::default()
        };
        let trill = checking.recognize(&nachschlag, &[]).unwrap().unwrap();
        assert!(trill.nachschlag());
        assert_eq!(size(&trill, "C5"), "m-2");

        assert!(plain.recognize(&notes(&["A"], 0.5), &[]).unwrap().is_none());
        assert!(
            plain
                .recognize(&notes(&["A", "G"], 0.5), &[])
                .unwrap()
                .is_none()
        );
        assert!(
            plain
                .recognize(&notes(&["A", "C", "A", "C"], 0.5), &[])
                .unwrap()
                .is_none()
        );
        assert!(
            plain
                .recognize(&notes(&["F", "E", "F", "G"], 0.5), &[])
                .unwrap()
                .is_none()
        );
    }
}
