//! Volpiano, the font chant is written in for the CANTUS database: music21's
//! `volpiano`.
//!
//! A Volpiano string is a row of tokens, one character each: `1` and `2` a
//! treble and a bass clef, a letter a note by where it sits on the staff (a
//! capital one liquescent), `w`, `x`, `i`, `y` and `z` a flat on an E or a B
//! from there on and their capitals a natural, `3` and `4` a barline and a
//! double barline, one, two or three `7`s a line, page or column break, and
//! hyphens the space between notes, one space or more ending a neume.
//! [`from_volpiano`] reads one into a part and [`to_volpiano`] writes any
//! stream as one.

use std::collections::HashMap;

use crate::bar::{Barline, BarlineType};
use crate::clef::Clef;
use crate::defaults::IntegerType;
use crate::duration::Duration;
use crate::error::{Error, Result};
use crate::notation::{Notehead, StemDirection, Syllabic};
use crate::note::Note;
use crate::pitch::Pitch;
use crate::pitch::accidental::Accidental;
use crate::spanner::{Spanner, SpannerKind};
use crate::stepname::StepName;
use crate::stream::{Stream, StreamElement, StreamEvent, StreamKind};

/// Where a manuscript breaks: music21's volpiano `LineBreak`, `PageBreak`
/// and `ColumnBreak`, written as one, two and three `7`s.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Break {
    /// The line breaks here.
    Line,
    /// The page breaks here.
    Page,
    /// The column breaks here.
    Column,
}

impl Break {
    /// music21's class name for the break.
    pub fn class_name(self) -> &'static str {
        match self {
            Self::Line => "LineBreak",
            Self::Page => "PageBreak",
            Self::Column => "ColumnBreak",
        }
    }

    /// The break written as that many `7`s, where there is one.
    fn of_count(count: usize) -> Option<Self> {
        match count {
            1 => Some(Self::Line),
            2 => Some(Self::Page),
            3 => Some(Self::Column),
            _ => None,
        }
    }

    fn token(self) -> &'static str {
        match self {
            Self::Line => "7",
            Self::Page => "77",
            Self::Column => "777",
        }
    }
}

/// The notes from two spaces below the lowest line up, each a step above the
/// last.
const NORMAL_PITCHES: &str = "9abcdefghjklmnopqrs";
/// The same notes liquescent.
const LIQUESCENT_PITCHES: &str = ")ABCDEFGHJKLMNOPQRS";
const E_FLAT_TOKENS: &str = "wx";
const B_FLAT_TOKENS: &str = "iyz";

/// How many steps below the lowest line the first of the pitch tokens is.
const BELOW_LOWEST_LINE: IntegerType = 5;

/// Reads a Volpiano string into a part, as music21's `toPart` does.
///
/// Each note lasts a quarter, has no stem, and is liquescent with an `x`
/// notehead where its token is a capital. A part opens in the treble clef
/// until a clef token says otherwise; a flat token flats every E or B from
/// there on, and its capital takes the flat away again. A barline closes
/// the measure, every measure numbered nought. Two notes with nothing
/// between them are a neume, a [`SpannerKind::Neume`] the part holds, and a
/// neume takes in every note of its run. A break goes at the end of the
/// measure being read. Tokens that are none of these are passed over.
///
/// ```
/// use music21_rs::volpiano::from_volpiano;
///
/// let part = from_volpiano("1---c--d---fg---3")?;
/// let names: Vec<String> = part
///     .pitches()
///     .iter()
///     .map(|pitch| pitch.name_with_octave())
///     .collect();
/// assert_eq!(names, ["C4", "D4", "F4", "G4"]);
/// assert_eq!(part.spanners().len(), 1);
/// # Ok::<(), music21_rs::Error>(())
/// ```
///
/// # Errors
///
/// A note further from its clef than a pitch can be spelled.
pub fn from_volpiano(text: &str) -> Result<Stream> {
    let mut measures: Vec<Stream> = Vec::new();
    let mut measure = Stream::with_kind(StreamKind::Measure);
    // Neumes, as the ordinals of their notes among every note read.
    let mut neumes: Vec<Vec<usize>> = Vec::new();
    let mut current_neume: Option<Vec<usize>> = None;
    let mut waiting: Option<usize> = None;
    let mut notes_read = 0;
    let mut lowest_line = Clef::treble().lowest_line().unwrap_or(31);
    let mut breaks = 0;
    let mut b_is_flat = false;
    let mut e_is_flat = false;

    for token in text.chars() {
        if token == '7' {
            breaks += 1;
            continue;
        }
        if let Some(kind) = Break::of_count(breaks) {
            append(&mut measure, StreamElement::Break(kind));
        }
        breaks = 0;
        if token == '-' || "1234".contains(token) {
            waiting = None;
            if let Some(neume) = current_neume.take() {
                neumes.push(neume);
            }
            if token == '-' {
                continue;
            }
        }
        match token {
            '1' | '2' => {
                let clef = if token == '1' {
                    Clef::treble()
                } else {
                    Clef::bass()
                };
                lowest_line = clef.lowest_line().unwrap_or(lowest_line);
                append(&mut measure, StreamElement::Clef(clef));
            }
            '3' | '4' => {
                let barline = if token == '4' {
                    Barline::new(BarlineType::Double)
                } else {
                    Barline::new(BarlineType::Regular)
                };
                measure.set_right_barline(Some(barline));
                measures.push(std::mem::replace(
                    &mut measure,
                    Stream::with_kind(StreamKind::Measure),
                ));
            }
            _ => {
                let normal = NORMAL_PITCHES.chars().position(|known| known == token);
                let liquescent = LIQUESCENT_PITCHES.chars().position(|known| known == token);
                if let Some(index) = normal.or(liquescent) {
                    let mut pitch = Pitch::from_name("C4")?;
                    pitch.set_diatonic_note_number(
                        lowest_line + index as IntegerType - BELOW_LOWEST_LINE,
                    )?;
                    let step = pitch.step();
                    if (step == StepName::B && b_is_flat) || (step == StepName::E && e_is_flat) {
                        pitch.set_accidental(Accidental::new("flat")?);
                    }
                    let mut note = Note::from_pitch(pitch).with_duration(Duration::quarter());
                    note.set_stem_direction(StemDirection::NoStem);
                    if liquescent.is_some() {
                        note.set_notehead(Notehead::X);
                    }
                    append(&mut measure, StreamElement::Note(note));
                    let this = notes_read;
                    notes_read += 1;
                    // Notes with no hyphen between them are one neume.
                    if let Some(neume) = current_neume.as_mut() {
                        neume.push(this);
                    } else if let Some(before) = waiting.take() {
                        current_neume = Some(vec![before, this]);
                    } else {
                        waiting = Some(this);
                    }
                } else if let Some(lower) = flat_token(token) {
                    let flat = !token.is_uppercase();
                    if E_FLAT_TOKENS.contains(lower) {
                        e_is_flat = flat;
                    } else {
                        b_is_flat = flat;
                    }
                }
            }
        }
    }
    if let Some(neume) = current_neume {
        neumes.push(neume);
    }
    if let Some(kind) = Break::of_count(breaks) {
        append(&mut measure, StreamElement::Break(kind));
    }
    if !measure.is_empty() {
        measures.push(measure);
    }

    let mut part = Stream::with_kind(StreamKind::Part);
    for measure in measures {
        part.push(measure);
    }
    // Each note's place among the part's leaves.
    let places: Vec<usize> = part
        .leaves()
        .iter()
        .enumerate()
        .filter(|(_, (_, element))| matches!(element, StreamElement::Note(_)))
        .map(|(place, _)| place)
        .collect();
    for neume in neumes {
        part.add_spanner(Spanner::new(
            SpannerKind::Neume,
            neume.into_iter().map(|ordinal| places[ordinal]).collect(),
        ));
    }
    Ok(part)
}

/// music21's `append`: the element where the measure ends, sorted among
/// what already stands there.
fn append(measure: &mut Stream, element: StreamElement) {
    let at = measure.end_offset();
    measure.insert_sorted(vec![StreamEvent::new(at, element)]);
}

/// The flat token a token is, lower case, where it is one or its capital.
fn flat_token(token: char) -> Option<char> {
    let lower = token.to_ascii_lowercase();
    (E_FLAT_TOKENS.contains(lower) || B_FLAT_TOKENS.contains(lower)).then_some(lower)
}

/// Writes a stream as Volpiano, as music21's `fromStream` does.
///
/// Every element is taken in turn, however deep: treble and bass clefs (and
/// the treble and bass clefs an octave off) as `1` and `2`, any other clef
/// changing only where the notes sit, barlines, breaks, and notes, a
/// liquescent one being one with an `x` notehead. A note is followed by a
/// hyphen, or by two where it has a lyric and three where that lyric says
/// it ends a word, and by nothing where a neume carries on past it. A flat is written
/// before the first flatted B or E, and a natural before the first one
/// after that is not.
///
/// What Volpiano cannot write is left out, as music21 leaves it out: chords,
/// rests, a note off the edge of what the tokens reach, and an accidental
/// other than a flat on a B or an E (the note is written, with no hyphen
/// after it).
///
/// ```
/// use music21_rs::volpiano::{from_volpiano, to_volpiano};
///
/// let part = from_volpiano("1--c--d---f--d---ed--c")?;
/// assert_eq!(to_volpiano(&part)?, "1---c-d-f-d-ed-c-");
/// # Ok::<(), music21_rs::Error>(())
/// ```
///
/// # Errors
///
/// A note under a clef that places no pitches.
pub fn to_volpiano(stream: &Stream) -> Result<String> {
    let mut inside = HashMap::new();
    neumes_of(stream, 0, &mut inside);
    let mut writer = Writer {
        tokens: String::new(),
        lowest_line: Clef::treble().lowest_line(),
        b_is_flat: false,
        e_is_flat: false,
        position: 0,
        inside,
    };
    writer.walk(stream)?;
    Ok(writer.tokens)
}

/// For each leaf of `stream` in a neume, whether a neume carries on past it:
/// music21 asks the first neume holding the note.
fn neumes_of(stream: &Stream, base: usize, inside: &mut HashMap<usize, bool>) {
    for spanner in stream.spanners() {
        if spanner.kind() != SpannerKind::Neume {
            continue;
        }
        let last = spanner.spanned().last().copied().flatten();
        for place in spanner.spanned().iter().flatten() {
            inside.entry(base + place).or_insert(Some(*place) != last);
        }
    }
    let mut seen = base;
    for event in stream.events() {
        match event.element() {
            StreamElement::Stream(inner) => {
                neumes_of(inner, seen, inside);
                seen += inner.leaves().len();
            }
            _ => seen += 1,
        }
    }
}

struct Writer {
    tokens: String,
    lowest_line: Option<IntegerType>,
    b_is_flat: bool,
    e_is_flat: bool,
    /// The place in the stream's leaves of the element being written.
    position: usize,
    inside: HashMap<usize, bool>,
}

impl Writer {
    fn walk(&mut self, stream: &Stream) -> Result<()> {
        if stream.left_barline().is_some() {
            self.barline(stream.left_barline());
        }
        for event in stream.events() {
            match event.element() {
                StreamElement::Stream(inner) => self.walk(inner)?,
                element => {
                    self.element(element)?;
                    self.position += 1;
                }
            }
        }
        if stream.right_barline().is_some() {
            self.barline(stream.right_barline());
        }
        Ok(())
    }

    fn barline(&mut self, barline: Option<&Barline>) {
        let double = barline.is_some_and(|barline| {
            matches!(barline.bar_type(), BarlineType::Double | BarlineType::Final)
        });
        self.tokens.push_str(if double { "---4" } else { "---3" });
    }

    fn pop_hyphens(&mut self) {
        while self.tokens.ends_with('-') {
            self.tokens.pop();
        }
    }

    /// music21's `setAccFromPitch`: the flat or natural token for a B or an
    /// E so far above the lowest line, where there is one.
    fn accidental_at(&mut self, distance: IntegerType, natural: bool) {
        let token = match distance {
            -3 => 'y',
            0 => 'w',
            4 => 'i',
            7 => 'x',
            11 => 'z',
            _ => return,
        };
        self.tokens.push(if natural {
            token.to_ascii_uppercase()
        } else {
            token
        });
    }

    fn element(&mut self, element: &StreamElement) -> Result<()> {
        match element {
            StreamElement::Clef(clef) => {
                self.lowest_line = clef.lowest_line();
                if clef.is_a("TrebleClef") {
                    self.tokens.push_str("1---");
                } else if clef.is_a("BassClef") {
                    self.tokens.push_str("2---");
                }
            }
            StreamElement::Barline(barline) => self.barline(Some(barline)),
            StreamElement::Note(note) => self.note(note)?,
            StreamElement::Break(kind) => {
                self.pop_hyphens();
                self.tokens.push_str(kind.token());
                self.tokens.push_str("---");
            }
            _ => {}
        }
        Ok(())
    }

    fn note(&mut self, note: &Note) -> Result<()> {
        let pitch = note.pitch();
        let lowest_line = self.lowest_line.ok_or_else(|| {
            Error::Stream("a note under a clef that places no pitches has no Volpiano".to_string())
        })?;
        let distance = pitch.diatonic_note_number() - lowest_line;
        let Ok(index) = usize::try_from(distance + BELOW_LOWEST_LINE) else {
            return Ok(());
        };
        let pitches = if note.notehead() == Notehead::X {
            LIQUESCENT_PITCHES
        } else {
            NORMAL_PITCHES
        };
        let Some(token) = pitches.chars().nth(index) else {
            return Ok(());
        };
        let step = pitch.step();
        match pitch.written_accidental() {
            Some(accidental) if accidental.alter() != 0.0 => {
                if !matches!(step, StepName::B | StepName::E) || accidental.alter() != -1.0 {
                    self.tokens.push(token);
                    return Ok(());
                }
                if step == StepName::B && !self.b_is_flat {
                    self.accidental_at(distance, false);
                    self.b_is_flat = true;
                } else if step == StepName::E && !self.e_is_flat {
                    self.accidental_at(distance, false);
                    self.e_is_flat = true;
                }
            }
            _ if step == StepName::B && self.b_is_flat => {
                self.accidental_at(distance, true);
                self.b_is_flat = false;
            }
            _ if step == StepName::E && self.e_is_flat => {
                self.accidental_at(distance, true);
                self.e_is_flat = false;
            }
            _ => {}
        }
        self.tokens.push(token);
        if self.inside.get(&self.position) == Some(&true) {
            return Ok(());
        }
        match note.lyric().filter(|sung| !sung.is_empty()) {
            None => self.tokens.push('-'),
            Some(_) => {
                // A lyric that says nothing of its place in a word ends none.
                let ends = note.lyrics().first().is_some_and(|lyric| {
                    matches!(
                        lyric.explicit_syllabic(),
                        Some(Syllabic::Single | Syllabic::End)
                    )
                });
                self.tokens.push_str(if ends { "---" } else { "--" });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(part: &Stream) -> Vec<String> {
        part.pitches()
            .iter()
            .map(|pitch| pitch.name_with_octave())
            .collect()
    }

    fn notes(part: &Stream) -> Vec<Note> {
        part.leaves()
            .into_iter()
            .filter_map(|(_, element)| match element {
                StreamElement::Note(note) => Some(note.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_chant_is_read_and_written_back_as_music21_writes_it() {
        let part = from_volpiano("1--c--d---f--d---ed--c--d---f---g--h--j---hgf--g--h---").unwrap();
        assert_eq!(
            to_volpiano(&part).unwrap(),
            "1---c-d-f-d-ed-c-d-f-g-h-j-hgf-g-h-"
        );
    }

    #[test]
    fn a_clef_moves_the_notes_after_it() {
        let part = from_volpiano("1---c--2---c").unwrap();
        assert_eq!(names(&part), ["C4", "E2"]);
    }

    #[test]
    fn flats_and_naturals_last_until_they_are_taken_back() {
        let part = from_volpiano("1---e--we--e--We--e").unwrap();
        assert_eq!(names(&part), ["E4", "Eb4", "Eb4", "E4", "E4"]);
        assert_eq!(to_volpiano(&part).unwrap(), "1---e-we-e-We-e-");
    }

    #[test]
    fn a_liquescent_note_has_an_x_notehead() {
        let part = from_volpiano("1---e-E--").unwrap();
        let notes = notes(&part);
        assert_eq!(notes[0].notehead(), Notehead::Normal);
        assert_eq!(notes[1].notehead(), Notehead::X);
        assert_eq!(to_volpiano(&part).unwrap(), "1---e-E-");
    }

    #[test]
    fn breaks_and_neumes_go_in_the_measure_being_read() {
        let part = from_volpiano("1---e-3-ef-g-7-e-4-gh--j-77").unwrap();
        let measures = part.measures();
        assert_eq!(measures.len(), 3);
        let breaks: Vec<Break> = measures[1]
            .events()
            .iter()
            .filter_map(|event| match event.element() {
                StreamElement::Break(kind) => Some(*kind),
                _ => None,
            })
            .collect();
        assert_eq!(breaks, [Break::Line]);
        assert_eq!(part.spanners().len(), 2);
        assert_eq!(
            to_volpiano(&part).unwrap(),
            "1---e----3ef-g7---e----4gh-j77---"
        );
    }

    #[test]
    fn a_neume_takes_every_note_of_its_run() {
        let part = from_volpiano("1---cdef-g-").unwrap();
        assert_eq!(part.spanners().len(), 1);
        // The clef is the part's first leaf.
        assert_eq!(
            part.spanners()[0].spanned(),
            [Some(1), Some(2), Some(3), Some(4)]
        );
        assert_eq!(to_volpiano(&part).unwrap(), "1---cdef-g-");
    }

    #[test]
    fn a_neume_ends_at_a_barline_or_the_end() {
        let part = from_volpiano("1---ef3-gh").unwrap();
        let lengths: Vec<usize> = part
            .spanners()
            .iter()
            .map(|neume| neume.spanned().len())
            .collect();
        assert_eq!(lengths, [2, 2]);
        assert_eq!(to_volpiano(&part).unwrap(), "1---ef----3gh-");
    }
}
