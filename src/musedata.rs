//! MuseData, the Center for Computer Assisted Research in the Humanities'
//! encoding: music21's `musedata` and `musedata.translate`.
//!
//! A MuseData file holds one part or several, each ended by a `/END` line,
//! and a work is usually a file per part. [`from_musedata`] reads the files
//! of a work into a score of a part each. Both of the encoding's stages are
//! read: stage 2, with its `$` attribute records and its columns for beams,
//! accidentals, notations and lyrics, and the older stage 1.

use crate::articulations::{Articulation, ArticulationKind};
use crate::bar::{Barline, BarlineType, RepeatDirection};
use crate::chord::Chord;
use crate::clef::{Clef, ClefKind};
use crate::defaults::{FloatType, IntegerType};
use crate::duration::Duration;
use crate::dynamics::Dynamic;
use crate::error::{Error, Result};
use crate::expressions::{Expression, Fermata, Ornament, OrnamentKind};
use crate::interval::Interval;
use crate::key::KeySignature;
use crate::makenotation::{make_accidentals, make_beams, op_frac};
use crate::metadata::Metadata;
use crate::meter::TimeSignature;
use crate::notation::{BeamDirection, BeamType, Beams, Tie, TieType};
use crate::note::Note;
use crate::pitch::Pitch;
use crate::pitch::accidental::Accidental;
use crate::rest::Rest;
use crate::stream::{Stream, StreamElement, StreamEvent, StreamKind};
use crate::tempo::TempoText;

fn error(message: impl Into<String>) -> Error {
    Error::MuseData(message.into())
}

/// Reads the files of a MuseData work, in order, into a score, as music21's
/// `converter.parse` reads a work's directory or a single file.
///
/// Each part is a part of the score, named as its header names it, in
/// measures as its `measure` records divide it, each measure opening with
/// the barline its record draws and the first with the clef, key, meter and
/// any tempo word the part's attribute record gives. Notes, chords and
/// rests take their lengths from the divisions of a quarter note; a stage-2
/// file also gives each note its beams, the accidental it shows, its
/// articulations, fermatas, ornaments and dynamics, and its lyrics, and a
/// `back` record starts another voice. A part written for a transposing
/// instrument (`X:`) is turned to the pitch it sounds, and its accidentals
/// are worked out again; a stage-1 part is given its best clef, its beams
/// and its accidentals. The work's title, number and movement are kept.
///
/// music21's own habits are kept: every measure starts with a barline, the
/// first measure is numbered one, a measure's number is every digit of its
/// record (so `mheavy2 12` is measure 212), and a pitch of double sharps
/// reads as a single sharp, of double flats as a single flat. Cue and grace
/// notes are passed over, as is an attribute record after the first.
///
/// ```
/// use music21_rs::musedata::from_musedata;
///
/// let part = "\n\n\n01/01/01 x\nWK#:1 MV#:1\nsource\nWork\nMovement\nFlute\n\
/// 1 0\nGroup memberships: score\nscore: part 1 of 1\n\
/// $  K:0   Q:2   T:2/4   C:4\nC5     2        q     u\nD5     2        q     u\n\
/// measure 2\nE5     4        h     u\n/END\n";
/// let score = from_musedata(&[part])?;
/// assert_eq!(score.parts()[0].measures().len(), 2);
/// # Ok::<(), music21_rs::Error>(())
/// ```
///
/// # Errors
///
/// A file with no part in it, an attribute record music21 cannot read, a
/// clef or barline it does not know, a record whose length is not a number,
/// or a transposition it has no interval for.
pub fn from_musedata(files: &[&str]) -> Result<Stream> {
    let mut parts = Vec::new();
    for text in files {
        parts.extend(split_parts(text)?);
    }
    let first = parts
        .first()
        .ok_or_else(|| error("a MuseData work with no parts"))?;
    let mut metadata = Metadata::new();
    if let Some(title) = first.work_title() {
        metadata.add_text("title", title);
    }
    if let Some(number) = first.movement_number() {
        metadata.add_text("movementNumber", number);
    }
    if let Some(name) = first.movement_title() {
        metadata.add_text("movementName", name);
    }
    if let Some(number) = first.work_number() {
        metadata.add_text("number", number);
    }
    let mut score = Stream::with_kind(StreamKind::Score);
    score.set_metadata(Some(metadata));
    for part in &parts {
        score.insert(0.0, part.to_part()?);
    }
    Ok(score)
}

/// music21's `MuseDataFile.readstr`: the parts of one file, comments and
/// `@` lines left out.
fn split_parts(text: &str) -> Result<Vec<Part>> {
    let mut parts = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    let mut comment = false;
    for line in text.split('\n') {
        if line.starts_with('&') {
            comment = !comment;
            continue;
        }
        if comment || line.starts_with('@') {
            continue;
        }
        if line.starts_with("/END") || line.starts_with("END") {
            if lines.len() <= 1 {
                lines.clear();
                continue;
            }
            parts.push(Part::new(std::mem::take(&mut lines))?);
        } else if !line.starts_with("/eof") {
            lines.push(line.to_string());
        }
    }
    Ok(parts)
}

/// Every digit of a string, in order: music21's `getNumFromStr`.
fn digits(text: &str) -> String {
    text.chars().filter(char::is_ascii_digit).collect()
}

/// A part's lines and the stage they are written in.
struct Part {
    lines: Vec<String>,
    stage: u8,
}

impl Part {
    /// music21's `MuseDataPart`: stage 2 where any line is an attribute
    /// record, stage 1 otherwise, whose leading blank lines are dropped.
    fn new(lines: Vec<String>) -> Result<Self> {
        if lines.iter().any(|line| line.starts_with('$')) {
            return Ok(Self { lines, stage: 2 });
        }
        if lines.len() <= 1 {
            return Err(error("cannot scrub empty source"));
        }
        let lines = lines
            .into_iter()
            .skip_while(|line| line.trim().is_empty())
            .collect();
        Ok(Self { lines, stage: 1 })
    }

    fn line(&self, index: usize) -> Result<&str> {
        self.lines
            .get(index)
            .map(String::as_str)
            .ok_or_else(|| error("a header shorter than MuseData's"))
    }

    /// `_getDigitsFollowingTag`: the digits after a tag, spaces passed
    /// over, `-` and `/` kept, anything else ending it.
    fn digits_after(line: &str, tag: &str) -> String {
        let Some(at) = line.find(tag) else {
            return String::new();
        };
        let mut out = String::new();
        for c in line[at + tag.len()..].chars() {
            if c.is_ascii_digit() || c == '-' || c == '/' {
                out.push(c);
            } else if !c.is_whitespace() {
                break;
            }
        }
        out
    }

    /// `_getAlphasFollowingTag` with `keepSpace` and `keepCase`: the letters,
    /// spaces and commas after a tag.
    fn words_after(line: &str, tag: &str) -> String {
        let Some(at) = line.find(tag) else {
            return String::new();
        };
        line[at + tag.len()..]
            .chars()
            .take_while(|c| c.is_alphabetic() || c.is_whitespace() || *c == ',')
            .collect()
    }

    fn work_number(&self) -> Option<String> {
        if self.stage == 1 {
            let data: String = self.lines.get(1)?.chars().take(6).collect();
            let data = data.trim();
            Some(if data.contains(',') {
                data.split(',').nth(1).unwrap_or("").to_string()
            } else {
                data.to_string()
            })
        } else {
            Some(Self::digits_after(self.lines.get(4)?, "WK#:"))
        }
    }

    fn movement_number(&self) -> Option<String> {
        if self.stage == 1 {
            Some(
                self.lines
                    .get(1)?
                    .chars()
                    .skip(6)
                    .collect::<String>()
                    .trim()
                    .to_string(),
            )
        } else {
            Some(Self::digits_after(self.lines.get(4)?, "MV#:"))
        }
    }

    fn work_title(&self) -> Option<String> {
        if self.stage == 1 {
            Some(self.lines.first()?.trim().to_string())
        } else {
            self.lines.get(6).cloned()
        }
    }

    fn movement_title(&self) -> Option<String> {
        if self.stage == 1 {
            None
        } else {
            self.lines.get(7).cloned()
        }
    }

    fn part_name(&self) -> Option<String> {
        if self.stage == 1 {
            None
        } else {
            self.lines.get(8).map(|name| name.trim().to_string())
        }
    }

    /// `_getAttributesRecord`.
    fn attributes(&self) -> Result<String> {
        if self.stage == 1 {
            return Ok(format!("{} {}", self.line(6)?.trim(), self.line(7)?.trim()));
        }
        self.lines
            .iter()
            .skip(11)
            .find(|line| line.starts_with('$'))
            .cloned()
            .ok_or_else(|| error("a part with no attribute record"))
    }

    /// The stage-1 attribute record's fields, split as music21 splits it.
    fn stage_one_field(&self, index: usize) -> Result<String> {
        self.attributes()?
            .split(' ')
            .nth(index)
            .map(str::to_string)
            .ok_or_else(|| error("a stage-1 attribute record too short"))
    }

    fn integer(text: &str) -> Result<IntegerType> {
        text.trim()
            .parse()
            .map_err(|_| error(format!("{text:?} is not a number")))
    }

    fn key_signature(&self) -> Result<KeySignature> {
        let sharps = if self.stage == 1 {
            Self::integer(&self.stage_one_field(1)?)?
        } else {
            Self::integer(&Self::digits_after(&self.attributes()?, "K:"))?
        };
        Ok(KeySignature::new(sharps))
    }

    fn time_signature(&self) -> Result<TimeSignature> {
        let (numerator, denominator) = if self.stage == 1 {
            (
                Self::integer(&self.stage_one_field(4)?)?,
                Self::integer(&self.stage_one_field(5)?)?,
            )
        } else {
            let written = Self::digits_after(&self.attributes()?, "T:");
            let (numerator, denominator) = written
                .split_once('/')
                .filter(|(_, denominator)| !denominator.contains('/'))
                .ok_or_else(|| error(format!("no meter in {written:?}")))?;
            (Self::integer(numerator)?, Self::integer(denominator)?)
        };
        let ratio = if (numerator == 1 && denominator == 1) || denominator == 0 {
            "4/4".to_string()
        } else {
            format!("{numerator}/{denominator}")
        };
        TimeSignature::from_ratio_string(&ratio)
    }

    fn clef(&self) -> Result<Option<Clef>> {
        if self.stage == 1 {
            return Ok(None);
        }
        let line = self.attributes()?;
        let mut written = Self::digits_after(&line, "C:");
        if written.is_empty() {
            let staves = match Self::digits_after(&line, "S:").as_str() {
                "" => 1,
                raw => Self::integer(raw)?,
            };
            written = (1..=staves)
                .map(|staff| Self::digits_after(&line, &format!("C{staff}:")))
                .find(|raw| !raw.is_empty())
                .ok_or_else(|| error("a part with no clef"))?;
        }
        let kind = match Self::integer(&written)?.to_string().as_str() {
            "5" => ClefKind::FrenchViolinClef,
            "4" => ClefKind::TrebleClef,
            "34" => ClefKind::Treble8vbClef,
            "64" => ClefKind::Treble8vaClef,
            "3" => ClefKind::GSopranoClef,
            "11" => ClefKind::CBaritoneClef,
            "12" => ClefKind::TenorClef,
            "13" => ClefKind::AltoClef,
            "14" => ClefKind::MezzoSopranoClef,
            "15" => ClefKind::SopranoClef,
            "22" => ClefKind::BassClef,
            "23" => ClefKind::FBaritoneClef,
            "52" => ClefKind::Bass8vbClef,
            "82" => ClefKind::Bass8vaClef,
            other => return Err(error(format!("cannot determine clef from: {other}"))),
        };
        Ok(Some(Clef::of_kind(kind)))
    }

    fn directive(&self) -> Result<Option<String>> {
        if self.stage == 1 {
            return Ok(None);
        }
        let words = Self::words_after(&self.attributes()?, "D:");
        Ok((!words.is_empty()).then(|| words.trim().to_string()))
    }

    /// The interval a transposing part is written at, in base 40 (`X:`).
    fn transposition(&self) -> Result<Option<Interval>> {
        if self.stage == 1 {
            return Ok(None);
        }
        match Self::digits_after(&self.attributes()?, "X:").as_str() {
            "" => Ok(None),
            raw => base40_interval(Self::integer(raw)?).map(Some),
        }
    }

    /// Divisions of a quarter note: `Q:`, or in stage 1 the divisions of a
    /// bar over the bar's length in quarters.
    fn divisions(&self) -> Result<FloatType> {
        if self.stage == 1 {
            let bar = Self::integer(&self.stage_one_field(2)?)?;
            Ok(FloatType::from(bar) / self.time_signature()?.bar_duration().quarter_length())
        } else {
            Ok(FloatType::from(Self::integer(&Self::digits_after(
                &self.attributes()?,
                "Q:",
            ))?))
        }
    }

    /// `_getMeasureBoundaryIndices`: the lines of each measure, from its
    /// first up to the one after its last.
    fn measure_bounds(&self) -> Result<Vec<(usize, usize)>> {
        let lines = &self.lines;
        let end = lines.len();
        let mut marks = Vec::new();
        let mut first_after: Option<usize> = None;
        let mut attributes = 0;
        for (index, line) in lines.iter().enumerate() {
            if self.stage == 2 {
                if line.starts_with('$') {
                    attributes += 1;
                    continue;
                }
                if attributes > 0 && first_after.is_none() {
                    first_after = Some(index);
                }
            }
            if line.starts_with('m') {
                marks.push(index);
            }
        }
        let first_mark = *marks
            .first()
            .ok_or_else(|| error("a part with no measure records"))?;
        if self.stage == 1 {
            first_after = Some(first_mark);
        }
        let mut bounds = Vec::new();
        let start = if Some(first_mark) == first_after {
            if marks.len() == 1 {
                bounds.push((first_mark, end));
                None
            } else {
                bounds.push((first_mark, marks[1]));
                Some(1)
            }
        } else {
            let first_after =
                first_after.ok_or_else(|| error("a part with nothing after its attributes"))?;
            bounds.push((first_after, first_mark));
            Some(0)
        };
        if let Some(start) = start {
            for index in start..marks.len() {
                bounds.push((marks[index], marks.get(index + 1).copied().unwrap_or(end)));
            }
        }
        Ok(bounds)
    }

    /// music21's `musedataPartToStreamPart`.
    fn to_part(&self) -> Result<Stream> {
        let mut part = Stream::with_kind(StreamKind::Part);
        let name = self.part_name();
        part.set_id(name.clone());
        part.set_name(name);
        let divisions = self.divisions()?;
        let bounds = self.measure_bounds()?;
        let measures: Vec<&[String]> = bounds
            .iter()
            .map(|(start, end)| self.lines.get(*start..*end).unwrap_or(&[]))
            .collect();
        let mut previous: Option<Option<TieType>> = None;
        let mut bars = 0;
        for (index, lines) in measures.iter().enumerate() {
            if !has_notes(lines) {
                continue;
            }
            let mut measure = measure_object(lines)?;
            if let Some(next) = measures.get(index + 1)
                && !has_notes(next)
            {
                measure.set_right_barline(Some(bar_object(next)?));
            }
            if bars == 0 {
                if let Some(clef) = self.clef()? {
                    append_at(&mut measure, 0.0, StreamElement::Clef(clef));
                }
                append_at(
                    &mut measure,
                    0.0,
                    StreamElement::TimeSignature(self.time_signature()?),
                );
                append_at(
                    &mut measure,
                    0.0,
                    StreamElement::KeySignature(self.key_signature()?),
                );
                if let Some(directive) = self.directive()? {
                    let words = TempoText::new(directive);
                    if words.is_common_tempo_text() {
                        append_at(
                            &mut measure,
                            0.0,
                            StreamElement::MetronomeMark(words.metronome_mark()),
                        );
                    }
                }
            }
            self.fill(&mut measure, lines, divisions, &mut previous)?;
            if bars == 0 {
                let bar = self.time_signature()?.bar_duration().quarter_length();
                let length = measure.end_offset();
                if length / bar < 1.0 {
                    measure.set_padding_left(bar - length);
                }
            }
            part.push(measure);
            bars += 1;
        }
        if let Some(interval) = self.transposition()? {
            let mut failure = None;
            part.for_each_mut(&mut |_, element| {
                let moved = match element {
                    StreamElement::Note(note) => note.transpose(&interval).map(StreamElement::Note),
                    StreamElement::Chord(chord) => {
                        chord.transpose(&interval).map(StreamElement::Chord)
                    }
                    StreamElement::KeySignature(signature) => signature
                        .transpose(&interval)
                        .map(StreamElement::KeySignature),
                    _ => return,
                };
                match moved {
                    Ok(moved) => *element = moved,
                    Err(error) => failure = Some(error),
                }
            });
            if let Some(error) = failure {
                return Err(error);
            }
            make_accidentals(&mut part);
        }
        if self.stage == 1 {
            // A stage-1 part states no clef for this to replace.
            let clef = Clef::best_for(&part.pitches(), false);
            if let Some(StreamElement::Stream(first)) = part
                .events_mut()
                .iter_mut()
                .map(StreamEvent::element_mut)
                .find(|element| {
                    matches!(element, StreamElement::Stream(inner) if inner.kind() == StreamKind::Measure)
                })
            {
                append_at(first, 0.0, StreamElement::Clef(clef));
            }
            // music21 warns and carries on where a meter cannot beam its bar.
            let _ = make_beams(&mut part);
            make_accidentals(&mut part);
        }
        Ok(part)
    }

    /// The notes, chords and rests of one measure's records, in voices where
    /// a `back` record says to start another.
    fn fill(
        &self,
        measure: &mut Stream,
        lines: &[String],
        divisions: FloatType,
        previous: &mut Option<Option<TieType>>,
    ) -> Result<()> {
        let voices = lines.iter().any(|line| line.starts_with("back"));
        let mut voice = voices.then(|| Stream::with_kind(StreamKind::Voice));
        let mut pending: Vec<Record<'_>> = Vec::new();
        for line in lines {
            let record = Record {
                line,
                stage: self.stage,
            };
            if record.is_back() {
                self.flush(&mut pending, measure, voice.as_mut(), divisions, previous)?;
                if let Some(done) = voice.replace(Stream::with_kind(StreamKind::Voice)) {
                    append_at(measure, 0.0, StreamElement::Stream(Box::new(done)));
                }
            }
            if record.is_rest() {
                self.flush(&mut pending, measure, voice.as_mut(), divisions, previous)?;
                let rest = Rest::new(Duration::new(record.quarter_length(divisions)?)?);
                let target = voice.as_mut().unwrap_or(&mut *measure);
                append(target, StreamElement::Rest(rest));
                *previous = Some(None);
            } else if record.is_chord() {
                pending.push(record);
            } else if record.is_note() {
                self.flush(&mut pending, measure, voice.as_mut(), divisions, previous)?;
                pending.push(record);
            }
        }
        self.flush(&mut pending, measure, voice.as_mut(), divisions, previous)?;
        if let Some(voice) = voice
            && !voice.is_empty()
        {
            append_at(measure, 0.0, StreamElement::Stream(Box::new(voice)));
        }
        Ok(())
    }

    /// music21's `_processPending`: the records waiting turned into a note
    /// or chord, its dynamics put beside it.
    fn flush(
        &self,
        pending: &mut Vec<Record<'_>>,
        measure: &mut Stream,
        voice: Option<&mut Stream>,
        divisions: FloatType,
        previous: &mut Option<Option<TieType>>,
    ) -> Result<()> {
        if pending.is_empty() {
            return Ok(());
        }
        let records = std::mem::take(pending);
        let (element, dynamics, tie) = note_or_chord(&records, divisions, *previous)?;
        *previous = Some(tie);
        let target = voice.unwrap_or(measure);
        let at = target.end_offset();
        append_at(target, at, element);
        for dynamic in dynamics {
            append_at(target, at, StreamElement::Dynamic(dynamic));
        }
        Ok(())
    }
}

/// music21's `append`: at the end of the stream, sorted among what stands
/// there.
fn append(stream: &mut Stream, element: StreamElement) {
    let at = stream.end_offset();
    append_at(stream, at, element);
}

/// music21's `insert`: sorted among what already stands at the offset.
fn append_at(stream: &mut Stream, offset: FloatType, element: StreamElement) {
    stream.insert_sorted(vec![StreamEvent::new(offset, element)]);
}

/// `MuseDataMeasure.hasNotes`.
fn has_notes(lines: &[String]) -> bool {
    lines.iter().any(|line| {
        line.chars()
            .next()
            .is_some_and(|c| "ABCDEFGrgc".contains(c))
    })
}

/// The record opening a measure, as music21 reads it: its own line where it
/// is a measure record, `measure` otherwise.
fn measure_line(lines: &[String]) -> Result<String> {
    let line = lines
        .first()
        .ok_or_else(|| error("a measure with no records"))?
        .trim()
        .to_string();
    Ok(if line.starts_with('m') {
        line
    } else {
        "measure".to_string()
    })
}

/// `MuseDataMeasure.getBarObject`.
fn bar_object(lines: &[String]) -> Result<Barline> {
    let line = measure_line(lines)?;
    let kind: String = line.chars().skip(1).take(6).collect();
    let bar_type = match kind.as_str() {
        "easure" => BarlineType::Regular,
        "dotted" => BarlineType::Dotted,
        "double" => BarlineType::Double,
        "heavy1" | "heavy" => BarlineType::Heavy,
        "heavy2" => BarlineType::Final,
        "heavy3" => BarlineType::HeavyLight,
        "heavy4" | "heave4" => BarlineType::HeavyHeavy,
        other => {
            return Err(error(format!(
                "cannot process bar data definition: {other}"
            )));
        }
    };
    let flag: String = line.chars().skip(16).collect();
    let flag = flag.trim();
    let mut barline = Barline::new(bar_type);
    if line.chars().count() > 16 && !flag.is_empty() {
        let direction = if flag.contains(":|") {
            Some(RepeatDirection::End)
        } else if flag.contains("|:") {
            Some(RepeatDirection::Start)
        } else {
            None
        };
        if let Some(direction) = direction {
            barline = Barline::repeat(direction, None);
            barline.set_bar_type(bar_type);
        }
    }
    Ok(barline)
}

/// `MuseDataMeasure.getMeasureObject`: a measure numbered as its record
/// says, opening with its barline.
fn measure_object(lines: &[String]) -> Result<Stream> {
    let line = measure_line(lines)?;
    let rest: String = line.chars().skip(8).collect();
    let number = if line.chars().count() >= 9 && !rest.trim().is_empty() {
        digits(&line)
    } else {
        "1".to_string()
    };
    let mut measure = Stream::with_kind(StreamKind::Measure);
    measure.set_left_barline(Some(bar_object(lines)?));
    if !number.is_empty() {
        measure.set_number(
            number
                .parse()
                .map_err(|_| error(format!("measure number {number}")))?,
        );
    }
    Ok(measure)
}

/// One line of a measure: `MuseDataRecord`.
#[derive(Clone, Copy)]
struct Record<'a> {
    line: &'a str,
    stage: u8,
}

impl Record<'_> {
    fn first(&self) -> Option<char> {
        self.line.chars().next()
    }

    /// The character at a column, counted from nought.
    fn at(&self, column: usize) -> Option<char> {
        self.line.chars().nth(column)
    }

    /// The characters from one column up to another.
    fn columns(&self, from: usize, to: usize) -> String {
        self.line
            .chars()
            .skip(from)
            .take(to.saturating_sub(from))
            .collect()
    }

    fn is_rest(&self) -> bool {
        self.first() == Some('r')
    }

    fn is_note(&self) -> bool {
        self.first().is_some_and(|c| "ABCDEFG".contains(c))
    }

    fn is_chord(&self) -> bool {
        self.first() == Some(' ') && self.at(1).is_some_and(|c| "ABCDEFG".contains(c))
    }

    fn is_back(&self) -> bool {
        self.line.starts_with("back")
    }

    fn is_tied(&self) -> bool {
        let column = if self.stage == 1 { 7 } else { 8 };
        self.at(column) == Some('-')
    }

    /// `_getPitchParameters` and `getPitchObject`.
    fn pitch(&self) -> Result<Pitch> {
        let token = if self.is_note() {
            self.line
        } else {
            self.line.get(1..).unwrap_or("")
        };
        let data = token.split_whitespace().next().unwrap_or("");
        let step = data
            .chars()
            .next()
            .ok_or_else(|| error("a note with no pitch"))?;
        // music21 asks for one sharp before two, and one flat before two.
        let accidental = if data.contains('#') {
            "#"
        } else if data.contains('f') {
            "-"
        } else {
            ""
        };
        let mut pitch = Pitch::from_name(format!("{step}{accidental}{}", digits(data)))?;
        if self.stage == 1 {
            return Ok(pitch);
        }
        if let Some(accidental) = pitch.written_accidental_mut() {
            accidental.set_display_status(Some(false));
        }
        if let Some(shown) = self.shown_accidental()? {
            pitch.set_accidental(shown);
            if let Some(accidental) = pitch.written_accidental_mut() {
                accidental.set_display_type("always")?;
                accidental.set_display_status(Some(true));
            }
        }
        if self
            .notations()
            .is_some_and(|notations| notations.contains('+'))
            && let Some(accidental) = pitch.written_accidental_mut()
        {
            accidental.set_display_status(Some(true));
        }
        Ok(pitch)
    }

    /// `_getAccidentalObject`: the accidental column 19 shows.
    fn shown_accidental(&self) -> Result<Option<Accidental>> {
        if self.line.chars().count() <= 18 {
            return Ok(None);
        }
        let name = match self.at(18) {
            Some('#' | 'S') => "sharp",
            Some('n') => "natural",
            Some('f' | 'F') => "flat",
            Some('x' | 'X') => "double-sharp",
            Some('&') => "double-flat",
            _ => return Ok(None),
        };
        Accidental::new(name).map(Some)
    }

    /// `getQuarterLength`.
    fn quarter_length(&self, divisions: FloatType) -> Result<FloatType> {
        let mut count = if self.stage == 1 {
            Part::integer(&self.columns(5, 7))?
        } else {
            Part::integer(&self.columns(5, 8))?
        };
        if let Some(blank) = self.at(4)
            && blank != ' '
        {
            let hundreds = blank.to_digit(10).ok_or_else(|| {
                error(format!(
                    "Error in parsing: {}\n   Column 5 must be blank.",
                    self.line
                ))
            })?;
            count += 100 * IntegerType::try_from(hundreds).unwrap_or(0);
        }
        Ok(op_frac(FloatType::from(count) / divisions))
    }

    fn lyrics(&self) -> Option<Vec<String>> {
        if self.stage == 1 || self.line.chars().count() < 44 {
            return None;
        }
        Some(
            self.line
                .chars()
                .skip(43)
                .collect::<String>()
                .split('|')
                .map(|lyric| lyric.trim().to_string())
                .collect(),
        )
    }

    fn beams(&self) -> Option<String> {
        if self.stage == 1 {
            return None;
        }
        let data = self.columns(25, 31);
        (!data.is_empty()).then(|| data.trim_end().to_string())
    }

    /// `_getAdditionalNotations`: columns 32 to 43.
    fn notations(&self) -> Option<String> {
        if self.line.chars().count() < 31 {
            return None;
        }
        Some(self.columns(31, 43).trim().to_string())
    }

    fn articulations(&self) -> Vec<Articulation> {
        let Some(notations) = self.notations() else {
            return Vec::new();
        };
        notations
            .chars()
            .filter_map(|c| {
                let kind = match c {
                    'A' | 'V' => ArticulationKind::StrongAccent,
                    '>' => ArticulationKind::Accent,
                    '.' => ArticulationKind::Staccato,
                    '_' => ArticulationKind::Tenuto,
                    '=' => ArticulationKind::DetachedLegato,
                    'i' => ArticulationKind::Spiccato,
                    ',' => ArticulationKind::BreathMark,
                    _ => return None,
                };
                Some(Articulation::of_kind(kind))
            })
            .collect()
    }

    fn expressions(&self) -> Vec<Expression> {
        let Some(notations) = self.notations() else {
            return Vec::new();
        };
        notations
            .chars()
            .filter_map(|c| match c {
                'F' | 'E' => Some(Expression::from(Fermata::new())),
                't' => Some(Expression::from(Ornament::of_kind(OrnamentKind::Trill))),
                'r' => Some(Expression::from(Ornament::of_kind(OrnamentKind::Turn))),
                'M' => Some(Expression::from(Ornament::of_kind(OrnamentKind::Mordent))),
                _ => None,
            })
            .collect()
    }

    /// `getDynamicObjects`: each mark found once, in music21's order of
    /// looking, and taken out of what is left to look in.
    fn dynamics(&self) -> Vec<Dynamic> {
        let Some(mut data) = self.notations() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for target in [
            "ppp", "fff", "pp", "ff", "fp", "mp", "mf", "p", "f", "m", "Z", "Zp", "R",
        ] {
            let Some(at) = data.find(target) else {
                continue;
            };
            data.replace_range(at..at + target.len(), "");
            let value = match target {
                "m" => "mp",
                "Z" | "Zp" | "R" => "sf",
                other => other,
            };
            out.push(Dynamic::new(value));
        }
        out
    }
}

/// `_musedataBeamToBeams`.
fn beams_of(written: &str) -> Beams {
    let mut beams = Beams::default();
    for c in written.chars() {
        let (kind, direction) = match c {
            '[' => (BeamType::Start, None),
            ']' => (BeamType::Stop, None),
            '=' => (BeamType::Continue, None),
            '/' => (BeamType::PartialBeam, Some(BeamDirection::Right)),
            '\\' => (BeamType::PartialBeam, Some(BeamDirection::Left)),
            _ => continue,
        };
        beams.append(kind, direction);
    }
    beams
}

/// `_musedataRecordListToNoteOrChord`: a note, or a chord of several
/// records, what the first record says of it, the dynamics it carries, and
/// the tie it leaves for the next.
fn note_or_chord(
    records: &[Record<'_>],
    divisions: FloatType,
    previous: Option<Option<TieType>>,
) -> Result<(StreamElement, Vec<Dynamic>, Option<TieType>)> {
    let first = records[0];
    let duration = Duration::new(first.quarter_length(divisions)?)?;
    let held = matches!(previous, Some(Some(TieType::Start | TieType::Continue)));
    let tie = if first.is_tied() {
        Some(if held {
            TieType::Continue
        } else {
            TieType::Start
        })
    } else if held {
        Some(TieType::Stop)
    } else {
        None
    };
    let mut pitches = Vec::new();
    for record in records {
        pitches.push(record.pitch()?);
    }
    let lyrics = first.lyrics();
    let beams = first.beams().map(|written| beams_of(&written));
    let element = if pitches.len() == 1 {
        let mut note = Note::from_pitch(pitches.remove(0)).with_duration(duration);
        for lyric in lyrics.iter().flatten() {
            note.add_lyric(lyric, None, false)?;
        }
        if let Some(beams) = beams {
            note.set_beams(beams);
        }
        note.articulations_mut().extend(first.articulations());
        note.expressions_mut().extend(first.expressions());
        note.set_tie(tie.map(Tie::new));
        StreamElement::Note(note)
    } else {
        let notes: Vec<Note> = pitches.into_iter().map(Note::from_pitch).collect();
        let mut chord = Chord::new(notes)?.with_duration(duration);
        for lyric in lyrics.iter().flatten() {
            chord.add_lyric(lyric, None, false)?;
        }
        if let Some(beams) = beams {
            chord.set_beams(beams);
        }
        chord.articulations_mut().extend(first.articulations());
        chord.expressions_mut().extend(first.expressions());
        for note in chord.notes_mut() {
            note.set_tie(tie.map(Tie::new));
        }
        StreamElement::Chord(chord)
    };
    Ok((element, first.dynamics(), tie))
}

/// music21's `base40IntervalTable`: the simple interval each distance in
/// base 40 is.
const BASE40_INTERVALS: [(i32, &str); 25] = [
    (0, "P1"),
    (1, "A1"),
    (4, "d2"),
    (5, "m2"),
    (6, "M2"),
    (7, "A2"),
    (10, "d3"),
    (11, "m3"),
    (12, "M3"),
    (13, "A3"),
    (16, "d4"),
    (17, "P4"),
    (18, "A4"),
    (22, "d5"),
    (23, "P5"),
    (24, "A5"),
    (27, "d6"),
    (28, "m6"),
    (29, "M6"),
    (30, "A6"),
    (33, "d7"),
    (34, "m7"),
    (35, "M7"),
    (36, "A7"),
    (39, "d8"),
];

/// music21's `base40DeltaToInterval`: a distance in base 40 as an interval,
/// its octaves counted and its direction kept.
fn base40_interval(delta: i32) -> Result<Interval> {
    let simple = delta.abs() % 40;
    let name = BASE40_INTERVALS
        .iter()
        .find(|(distance, _)| *distance == simple)
        .map(|(_, name)| *name)
        .ok_or_else(|| error(format!("Interval not handled by Base40 {simple}")))?;
    let (specifier, number) = name.split_at(1);
    let number: i32 = number.parse().unwrap_or(1);
    let octaves = delta.abs() / 40;
    let generic = number + 7 * octaves;
    let generic = if delta < 0 { -generic } else { generic };
    Interval::from_name(format!("{specifier}{generic}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_base40_distance_is_an_interval_with_its_octaves() {
        assert_eq!(base40_interval(-11).unwrap().directed_name(), "m-3");
        assert_eq!(base40_interval(23).unwrap().directed_name(), "P5");
        assert_eq!(base40_interval(63).unwrap().directed_name(), "P12");
        assert!(base40_interval(2).is_err());
    }

    #[test]
    fn a_measure_number_is_every_digit_of_its_record() {
        let lines = ["mheavy2 12".to_string(), "C4 1 q".to_string()];
        assert_eq!(measure_object(&lines).unwrap().number(), 212);
        let lines = ["C4 1 q".to_string()];
        assert_eq!(measure_object(&lines).unwrap().number(), 1);
    }

    #[test]
    fn beams_are_read_from_their_column() {
        let beams = beams_of("[[/");
        assert_eq!(beams.len(), 3);
    }

    #[test]
    fn a_dynamic_is_taken_out_before_the_next_is_looked_for() {
        let record = Record {
            line: "C5     3        e     d  [     pp.",
            stage: 2,
        };
        let values: Vec<String> = record
            .dynamics()
            .iter()
            .map(|dynamic| dynamic.value().to_string())
            .collect();
        assert_eq!(values, ["pp"]);
    }
}
