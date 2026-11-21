//! MuseData, the Center for Computer Assisted Research in the Humanities'
//! encoding: music21's `musedata` and `musedata.translate`.
//!
//! A MuseData file holds one part or several, each ended by a `/END` line,
//! and a work is usually a file per part. [`from_musedata`] reads the files
//! of a work into a score of a part each. Both of the encoding's stages are
//! read: stage 2, with its `$` attribute records and its columns for beams,
//! accidentals, notations and lyrics, and the older stage 1.

pub mod base12_26;
pub mod base40;

use std::collections::HashMap;

use crate::articulations::{Articulation, ArticulationKind};
use crate::bar::{Barline, BarlineType, RepeatDirection};
use crate::chord::Chord;
use crate::clef::{Clef, ClefKind};
use crate::defaults::{FloatType, IntegerType};
use crate::duration::Duration;
use crate::dynamics::Dynamic;
use crate::error::{Error, Position, Result};
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
use crate::spanner::Spanner;
use crate::stream::{Stream, StreamElement, StreamEvent, StreamKind};
use crate::tempo::TempoText;

fn error(message: impl Into<String>) -> Error {
    Error::muse_data(message.into())
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
/// and its accidentals. Stage-2 slur marks become slurs. The work's title,
/// number and movement are kept.
///
/// music21's own habits are kept: every measure starts with a barline, the
/// first measure is numbered one (a pickup before it zero), a measure's number is every digit of its
/// record (so `mheavy2 12` is measure 212), and a pitch of double sharps
/// reads as a single sharp, of double flats as a single flat. Cue and grace
/// notes are passed over, as is an attribute record after the first.
///
/// ```
/// use music21::musedata::from_musedata;
///
/// let part = "\n\n\n01/01/01 x\nWK#:1 MV#:1\nsource\nWork\nMovement\nFlute\n\
/// 1 0\nGroup memberships: score\nscore: part 1 of 1\n\
/// $  K:0   Q:2   T:2/4   C:4\nC5     2        q     u\nD5     2        q     u\n\
/// measure 2\nE5     4        h     u\n/END\n";
/// let score = from_musedata(&[part])?;
/// assert_eq!(score.parts()[0].measures().len(), 2);
/// # Ok::<(), music21::Error>(())
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
    score_of(&parts, Header::Read)
}

/// Whether [`from_musedata_parts`] reads the work's title, number and
/// movement into the score's metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Header {
    /// Read, as music21's `museDataWorkToStreamScore` reads it.
    Read,
    /// Left out, as music21's `musedataPartToStreamPart` leaves it.
    Skipped,
}

/// Reads parts already split from their files, each the lines of one part
/// as music21's `MuseDataPart.src` holds them, into a score: music21's
/// `museDataWorkToStreamScore`, or with [`Header::Skipped`] its
/// `musedataPartToStreamPart` for each.
///
/// # Errors
///
/// As [`from_musedata`], and a part of one line or none.
pub fn from_musedata_parts(parts: &[Vec<String>], header: Header) -> Result<Stream> {
    let mut read = Vec::new();
    for lines in parts {
        let numbers = (1..=lines.len()).collect();
        read.push(Part::new(lines.clone(), numbers)?);
    }
    score_of(&read, header)
}

/// The score of a work's parts, each holding its slurs.
fn score_of(parts: &[Part], header: Header) -> Result<Stream> {
    let first = parts
        .first()
        .ok_or_else(|| error("a MuseData work with no parts"))?;
    let mut score = Stream::with_kind(StreamKind::Score);
    if header == Header::Read {
        score.set_metadata(Some(work_metadata(first)));
    }
    for part in parts {
        let (mut made, joined) = part.to_part()?;
        // Each slur goes in its part, naming its notes by their places
        // among the part's notes and chords in the order they were read.
        let read: Vec<usize> = made
            .leaves()
            .into_iter()
            .enumerate()
            .filter(|(_, (_, element))| {
                matches!(element, StreamElement::Note(_) | StreamElement::Chord(_))
            })
            .map(|(position, _)| position)
            .collect();
        for (start, stop) in joined {
            if let (Some(first), Some(last)) = (read.get(start), read.get(stop)) {
                made.add_spanner(Spanner::slur(vec![*first, *last]));
            }
        }
        score.insert(0.0, made);
    }
    Ok(score)
}

/// The work's title, number and movement, as the first part names them.
fn work_metadata(first: &Part) -> Metadata {
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
    metadata
}

/// The slurs of one part, as music21's `_SlurCollector` builds them: the
/// marks ( ) [ ] { } and z x start and stop slurs 1 to 4, a stop closing the
/// open slur of its number. Notes and chords are counted in the order they
/// are read.
#[derive(Clone, Debug, Default)]
pub struct Slurs {
    open: [Option<usize>; 4],
    joined: Vec<(usize, usize)>,
    read: usize,
}

impl Slurs {
    const STARTS: [char; 4] = ['(', '[', '{', 'z'];
    const STOPS: [char; 4] = [')', ']', '}', 'x'];

    /// Stops, then starts, the slurs the marks name on the next note or
    /// chord, so one note can end a slur and begin the next of its number.
    /// Hands back the slurs it stops, each as the places of its first and
    /// last notes among those added.
    pub fn add(&mut self, marks: &str) -> Vec<(usize, usize)> {
        let at = self.read;
        self.read += 1;
        let mut stopped = Vec::new();
        for mark in marks.chars() {
            if let Some(number) = Self::STOPS.iter().position(|stop| *stop == mark)
                && let Some(start) = self.open[number].take()
            {
                stopped.push((start, at));
            }
        }
        self.joined.extend(stopped.iter().copied());
        for mark in marks.chars() {
            if let Some(number) = Self::STARTS.iter().position(|start| *start == mark) {
                self.open[number] = Some(at);
            }
        }
        stopped
    }

    /// The open slurs, by number from nought, each as the place of its first
    /// note.
    pub fn open(&self) -> &[Option<usize>; 4] {
        &self.open
    }
}

/// music21's `MuseDataFile.readstr`: the parts of one file, comments and
/// `@` lines left out, read the same with Windows line endings.
///
/// # Errors
///
/// A stage-1 part of one line.
pub fn split_parts(text: &str) -> Result<Vec<Part>> {
    let mut parts = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    // The line of the file each of them is, counted from one.
    let mut numbers: Vec<usize> = Vec::new();
    let mut comment = false;
    for (index, line) in text.replace("\r\n", "\n").split('\n').enumerate() {
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
                numbers.clear();
                continue;
            }
            let at = numbers.first().copied().unwrap_or(index + 1);
            let part = Part::new(std::mem::take(&mut lines), std::mem::take(&mut numbers))
                .map_err(|error| error.placed(|| Position::line(at)))?;
            parts.push(part);
        } else if !line.starts_with("/eof") {
            lines.push(line.to_string());
            numbers.push(index + 1);
        }
    }
    Ok(parts)
}

/// Every digit of a string, in order: music21's `getNumFromStr`.
fn digits(text: &str) -> String {
    text.chars().filter(char::is_ascii_digit).collect()
}

/// Whether [`Part::letters_after`] keeps the spaces it passes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spaces {
    /// Passed over.
    Dropped,
    /// Kept.
    Kept,
}

/// Whether [`Part::letters_after`] reads the line as written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    /// As written.
    Kept,
    /// Lowered, tag and line alike.
    Lowered,
}

/// The stage of the encoding a part is written in: stage 2 has `$`
/// attribute records and columns stage 1 lacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// The older encoding, without attribute records.
    One,
    /// The encoding with `$` attribute records.
    Two,
}

/// A part's lines and the stage they are written in: music21's
/// `MuseDataPart`.
#[derive(Clone, Debug)]
pub struct Part {
    lines: Vec<String>,
    /// The line of its file each line is, counted from one.
    numbers: Vec<usize>,
    stage: Stage,
}

impl Part {
    /// A part of `lines` already read, in `stage`: music21's
    /// `MuseDataPart(src, stage)` for a source it keeps as it is.
    pub fn with_stage(lines: Vec<String>, stage: Stage) -> Self {
        let numbers = (1..=lines.len()).collect();
        Self {
            lines,
            numbers,
            stage,
        }
    }

    /// music21's `_determineStage`: stage 2 where any line is an attribute
    /// record.
    pub fn stage_of(lines: &[String]) -> Stage {
        if lines.iter().any(|line| line.starts_with('$')) {
            Stage::Two
        } else {
            Stage::One
        }
    }

    /// music21's `_scrubStage1`: the lines without the blank ones they open
    /// with.
    ///
    /// # Errors
    ///
    /// Lines of one or none.
    pub fn scrubbed(lines: &[String]) -> Result<Vec<String>> {
        if lines.len() <= 1 {
            return Err(error("cannot scrub empty source"));
        }
        Ok(lines
            .iter()
            .skip_while(|line| line.trim().is_empty())
            .cloned()
            .collect())
    }

    /// The part's lines, a stage-1 part's leading blank lines dropped.
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// The stage the part is written in.
    pub fn stage(&self) -> Stage {
        self.stage
    }

    /// music21's `MuseDataPart`: stage 2 where any line is an attribute
    /// record, stage 1 otherwise, whose leading blank lines are dropped.
    fn new(lines: Vec<String>, numbers: Vec<usize>) -> Result<Self> {
        if lines.iter().any(|line| line.starts_with('$')) {
            return Ok(Self {
                lines,
                numbers,
                stage: Stage::Two,
            });
        }
        if lines.len() <= 1 {
            return Err(error("cannot scrub empty source"));
        }
        let (lines, numbers) = lines
            .into_iter()
            .zip(numbers)
            .skip_while(|(line, _)| line.trim().is_empty())
            .unzip();
        Ok(Self {
            lines,
            numbers,
            stage: Stage::One,
        })
    }

    /// Where the line at `index` stands in the file, in this part.
    fn place(&self, index: usize) -> Position {
        let position = Position {
            line: self.numbers.get(index).copied(),
            ..Position::default()
        };
        match self.part_name() {
            Some(name) => position.in_part(name),
            None => position,
        }
    }

    /// Where the attribute record stands: what goes wrong reading a
    /// part's key, meter, clef or divisions is placed there.
    fn attributes_place(&self) -> Position {
        let index = match self.stage {
            Stage::One => Some(6),
            Stage::Two => self
                .lines
                .iter()
                .skip(11)
                .position(|line| line.starts_with('$'))
                .map(|index| index + 11),
        };
        match index {
            Some(index) => self.place(index),
            None => self.place(usize::MAX),
        }
    }

    fn line(&self, index: usize) -> Result<&str> {
        self.lines
            .get(index)
            .map(String::as_str)
            .ok_or_else(|| error("a header shorter than MuseData's"))
    }

    /// music21's `_getDigitsFollowingTag`: the digits after a tag, spaces
    /// passed over, `-` and `/` kept, anything else ending it.
    ///
    /// ```
    /// use music21::musedata::Part;
    ///
    /// assert_eq!(Part::digits_after("$ K:-3   Q:4   T:3/4   C:22", "T:"), "3/4");
    /// ```
    pub fn digits_after(line: &str, tag: &str) -> String {
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
        Self::letters_after(line, tag, Spaces::Kept, Case::Kept)
    }

    /// music21's `_getAlphasFollowingTag`: the letters and commas after a
    /// tag, spaces passed over or kept, anything else ending it; with
    /// [`Case::Lowered`] line and tag are lowered first.
    ///
    /// ```
    /// use music21::musedata::{Case, Part, Spaces};
    ///
    /// let line = "Group memberships: sound, score";
    /// let read = Part::letters_after(line, "Group memberships:", Spaces::Dropped, Case::Lowered);
    /// assert_eq!(read, "sound,score");
    /// ```
    pub fn letters_after(line: &str, tag: &str, spaces: Spaces, case: Case) -> String {
        let (line, tag) = match case {
            Case::Kept => (line.to_string(), tag.to_string()),
            Case::Lowered => (line.to_lowercase(), tag.to_lowercase()),
        };
        let Some(at) = line.find(&tag) else {
            return String::new();
        };
        let mut out = String::new();
        for c in line[at + tag.len()..].chars() {
            if c.is_alphabetic() || c == ',' {
                out.push(c);
            } else if c.is_whitespace() {
                if spaces == Spaces::Kept {
                    out.push(c);
                }
            } else {
                break;
            }
        }
        out
    }

    /// music21's `getSource`: where the encoding was taken from.
    pub fn source(&self) -> Option<String> {
        if self.stage == Stage::Two {
            return self.lines.get(5).cloned();
        }
        let mut joined = String::new();
        for index in 2..=4 {
            joined.push_str(self.lines.get(index)?.trim());
        }
        Some(joined)
    }

    /// music21's `getGroupMemberships`: the groups the part belongs to.
    pub fn group_memberships(&self) -> Vec<String> {
        if self.stage == Stage::One {
            return Vec::new();
        }
        let Some(line) = self.lines.get(10) else {
            return Vec::new();
        };
        Self::letters_after(line, "group memberships:", Spaces::Dropped, Case::Lowered)
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(str::to_string)
            .collect()
    }

    /// The `membership:` line among those before the attribute record, and
    /// the digits after `tag` in it.
    fn membership_digits(&self, membership: &str, tag: &str) -> Option<String> {
        self.lines
            .iter()
            .skip(11)
            .take_while(|line| !line.starts_with('$'))
            .find(|line| line.starts_with(membership))
            .map(|line| Self::digits_after(line, tag))
    }

    /// music21's `getGroupMembershipsTotal`: how many parts the group has.
    ///
    /// # Errors
    ///
    /// A count that is no number.
    pub fn group_total(&self, membership: &str) -> Result<Option<IntegerType>> {
        if self.stage == Stage::One {
            let line = self.line(5)?;
            return Self::integer(line.split(' ').next().unwrap_or("")).map(Some);
        }
        self.membership_digits(membership, "of")
            .map(|raw| Self::integer(&raw))
            .transpose()
    }

    /// music21's `getGroupMembershipNumber`: the part's place in the group,
    /// as written. music21 hands stage 1's back as text, stage 2's as a
    /// number.
    ///
    /// # Errors
    ///
    /// A stage-1 header too short.
    pub fn group_number(&self, membership: &str) -> Result<Option<String>> {
        if self.stage == Stage::One {
            let line = self.line(5)?;
            return Ok(line.split(' ').nth(1).map(str::to_string));
        }
        Ok(self.membership_digits(membership, "part"))
    }

    /// music21's `getWorkNumber`: the opus number, as written.
    pub fn work_number(&self) -> Option<String> {
        if self.stage == Stage::One {
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

    /// music21's `getMovementNumber`, as written.
    pub fn movement_number(&self) -> Option<String> {
        if self.stage == Stage::One {
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

    /// music21's `getWorkTitle`: for stage 1, the catalogue number.
    pub fn work_title(&self) -> Option<String> {
        if self.stage == Stage::One {
            Some(self.lines.first()?.trim().to_string())
        } else {
            self.lines.get(6).cloned()
        }
    }

    /// music21's `getMovementTitle`; none for stage 1.
    pub fn movement_title(&self) -> Option<String> {
        if self.stage == Stage::One {
            None
        } else {
            self.lines.get(7).cloned()
        }
    }

    /// music21's `getPartName`; none for stage 1.
    pub fn part_name(&self) -> Option<String> {
        if self.stage == Stage::One {
            None
        } else {
            self.lines.get(8).map(|name| name.trim().to_string())
        }
    }

    /// music21's `_getAttributesRecord`: the first line opening with `$`,
    /// or stage 1's two attribute lines joined.
    ///
    /// # Errors
    ///
    /// A part with no attribute record.
    pub fn attributes(&self) -> Result<String> {
        if self.stage == Stage::One {
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

    /// music21's `getKeyParameters`: the signature's sharps, flats below
    /// nought.
    ///
    /// # Errors
    ///
    /// A key that is no number.
    pub fn key_parameters(&self) -> Result<IntegerType> {
        if self.stage == Stage::One {
            return Self::integer(&self.stage_one_field(1)?);
        }
        Self::integer(&Self::digits_after(&self.attributes()?, "K:"))
    }

    /// music21's `getKeySignature`.
    ///
    /// # Errors
    ///
    /// As [`Part::key_parameters`].
    pub fn key_signature(&self) -> Result<KeySignature> {
        Ok(KeySignature::new(self.key_parameters()?))
    }

    /// music21's `getTimeSignatureObject`.
    ///
    /// # Errors
    ///
    /// As [`Part::time_signature_parameters`].
    pub fn time_signature(&self) -> Result<TimeSignature> {
        TimeSignature::from_ratio_string(&self.time_signature_parameters()?)
    }

    /// music21's `getTimeSignatureParameters`: the meter as `3/4`, a meter
    /// of `1/1` or over nought as `4/4`.
    ///
    /// # Errors
    ///
    /// A meter that is not two numbers.
    pub fn time_signature_parameters(&self) -> Result<String> {
        let (numerator, denominator) = if self.stage == Stage::One {
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
        if (numerator == 1 && denominator == 1) || denominator == 0 {
            return Ok("4/4".to_string());
        }
        Ok(format!("{numerator}/{denominator}"))
    }

    /// music21's `_getNumberOfStaves`: `S:`, one where unwritten.
    ///
    /// # Errors
    ///
    /// A count that is no number.
    pub fn staff_count(&self) -> Result<IntegerType> {
        if self.stage == Stage::One {
            return Ok(1);
        }
        match Self::digits_after(&self.attributes()?, "S:").as_str() {
            "" => Ok(1),
            raw => Self::integer(raw),
        }
    }

    /// music21's `_getClefParameters`: `C:`, or each staff's `Cn:`, as
    /// written; stage 1's ninth attribute field.
    ///
    /// # Errors
    ///
    /// A part with no attribute record.
    pub fn clef_parameters(&self) -> Result<Vec<String>> {
        if self.stage == Stage::One {
            return Ok(vec![self.stage_one_field(8)?]);
        }
        let line = self.attributes()?;
        let written = Self::digits_after(&line, "C:");
        if !written.is_empty() {
            return Ok(vec![written]);
        }
        let mut found = Vec::new();
        for staff in 1..=self.staff_count()? {
            let raw = Self::digits_after(&line, &format!("C{staff}:"));
            if !raw.is_empty() {
                found.push(raw);
            }
        }
        Ok(found)
    }

    fn clef(&self) -> Result<Option<Clef>> {
        self.clef_for(1)
    }

    /// music21's `getClefObject`: the clef of staff `voice`, counted from
    /// one; none for stage 1.
    ///
    /// # Errors
    ///
    /// No clef for the staff, or a code music21 does not know.
    pub fn clef_for(&self, voice: usize) -> Result<Option<Clef>> {
        if self.stage == Stage::One {
            return Ok(None);
        }
        let written = self
            .clef_parameters()?
            .get(voice.saturating_sub(1))
            .cloned()
            .ok_or_else(|| error("a part with no clef"))?;
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

    /// music21's `getDirective`: `D:`, usually a tempo word.
    ///
    /// # Errors
    ///
    /// A part with no attribute record.
    pub fn directive(&self) -> Result<Option<String>> {
        if self.stage == Stage::One {
            return Ok(None);
        }
        let words = Self::words_after(&self.attributes()?, "D:");
        Ok((!words.is_empty()).then(|| words.trim().to_string()))
    }

    /// music21's `getTranspositionIntervalObject`: the interval a
    /// transposing part is written at.
    ///
    /// # Errors
    ///
    /// A transposition with no interval.
    pub fn transposition(&self) -> Result<Option<Interval>> {
        self.transposition_parameters()?
            .map(base40_interval)
            .transpose()
    }

    /// music21's `_getTranspositionParameters`: `X:`, in base 40.
    ///
    /// # Errors
    ///
    /// A transposition that is no number.
    pub fn transposition_parameters(&self) -> Result<Option<IntegerType>> {
        if self.stage == Stage::One {
            return Ok(None);
        }
        match Self::digits_after(&self.attributes()?, "X:").as_str() {
            "" => Ok(None),
            raw => Self::integer(raw).map(Some),
        }
    }

    /// Divisions of a quarter note: `Q:`, or in stage 1 the divisions of a
    /// bar over the bar's length in quarters.
    pub fn divisions(&self) -> Result<FloatType> {
        if self.stage == Stage::One {
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
    pub fn measure_bounds(&self) -> Result<Vec<(usize, usize)>> {
        let lines = &self.lines;
        let end = lines.len();
        let mut marks = Vec::new();
        let mut first_after: Option<usize> = None;
        let mut attributes = 0;
        for (index, line) in lines.iter().enumerate() {
            if self.stage == Stage::Two {
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
        if self.stage == Stage::One {
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
    fn to_part(&self) -> Result<(Stream, Vec<(usize, usize)>)> {
        let mut part = Stream::with_kind(StreamKind::Part);
        let name = self.part_name();
        part.set_id(name.clone());
        part.set_name(name);
        let header = |error: Error| error.placed(|| self.attributes_place());
        let divisions = self.divisions().map_err(header)?;
        let bounds = self.measure_bounds()?;
        let measures: Vec<&[String]> = bounds
            .iter()
            .map(|(start, end)| self.lines.get(*start..*end).unwrap_or(&[]))
            .collect();
        // The last note, chord or rest of each voice, by its number, for the
        // ties across measures; a measure without voices continues voice 0.
        let mut last_by_voice: HashMap<usize, Option<Option<TieType>>> = HashMap::new();
        let mut slurs = Slurs::default();
        let mut bars = 0;
        for (index, lines) in measures.iter().enumerate() {
            if !has_notes(lines) {
                continue;
            }
            // What goes wrong inside a measure says which, by the number
            // its record gives it, and the line it opens on.
            let first = bounds[index].0;
            let here = |error: Error| {
                error.placed(|| {
                    let mut position = self.place(first);
                    position.measure = measure_number(lines).ok();
                    position
                })
            };
            let mut measure = measure_object(lines).map_err(here)?;
            if let Some(next) = measures.get(index + 1)
                && !has_notes(next)
            {
                let after = bounds[index + 1].0;
                let barline = bar_object(next).map_err(|error| error.placed(|| self.place(after)));
                measure.set_right_barline(Some(barline.map_err(here)?));
            }
            if bars == 0 {
                if let Some(clef) = self.clef().map_err(header)? {
                    append_at(&mut measure, 0.0, StreamElement::Clef(clef));
                }
                append_at(
                    &mut measure,
                    0.0,
                    StreamElement::TimeSignature(self.time_signature().map_err(header)?),
                );
                append_at(
                    &mut measure,
                    0.0,
                    StreamElement::KeySignature(self.key_signature().map_err(header)?),
                );
                if let Some(directive) = self.directive().map_err(header)? {
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
            self.fill(
                &mut measure,
                lines,
                first,
                divisions,
                &mut last_by_voice,
                &mut slurs,
            )
            .map_err(here)?;
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
        if let Some(interval) = self.transposition().map_err(header)? {
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
        if self.stage == Stage::One {
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
        Ok((part, slurs.joined))
    }

    /// The notes, chords and rests of one measure's records, in voices where
    /// a `back` record says to start another. Each voice ties on from the
    /// voice of its number in the measure before.
    fn fill(
        &self,
        measure: &mut Stream,
        lines: &[String],
        first: usize,
        divisions: FloatType,
        last_by_voice: &mut HashMap<usize, Option<Option<TieType>>>,
        slurs: &mut Slurs,
    ) -> Result<()> {
        let voices = lines.iter().any(|line| line.starts_with("back"));
        let mut voice = voices.then(|| Stream::with_kind(StreamKind::Voice));
        let mut pending: Vec<Record<'_>> = Vec::new();
        let mut number = 0;
        let mut last = last_by_voice.get(&number).copied().flatten();
        let previous = &mut last;
        for (index, line) in lines.iter().enumerate() {
            let record = Record {
                line,
                stage: self.stage,
                number: self.numbers.get(first + index).copied(),
            };
            if record.is_back() {
                self.flush(
                    &mut pending,
                    measure,
                    voice.as_mut(),
                    divisions,
                    previous,
                    slurs,
                )?;
                if let Some(done) = voice.replace(Stream::with_kind(StreamKind::Voice)) {
                    append_at(measure, 0.0, StreamElement::Stream(Box::new(done)));
                }
                last_by_voice.insert(number, *previous);
                number += 1;
                *previous = last_by_voice.get(&number).copied().flatten();
            }
            if record.is_rest() {
                self.flush(
                    &mut pending,
                    measure,
                    voice.as_mut(),
                    divisions,
                    previous,
                    slurs,
                )?;
                let length = record
                    .quarter_length(divisions)
                    .and_then(Duration::new)
                    .map_err(|error| record.placed(error))?;
                let rest = Rest::new(length);
                let target = voice.as_mut().unwrap_or(&mut *measure);
                append(target, StreamElement::Rest(rest));
                *previous = Some(None);
            } else if record.is_chord() {
                pending.push(record);
            } else if record.is_note() {
                self.flush(
                    &mut pending,
                    measure,
                    voice.as_mut(),
                    divisions,
                    previous,
                    slurs,
                )?;
                pending.push(record);
            }
        }
        self.flush(
            &mut pending,
            measure,
            voice.as_mut(),
            divisions,
            previous,
            slurs,
        )?;
        last_by_voice.insert(number, *previous);
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
        slurs: &mut Slurs,
    ) -> Result<()> {
        if pending.is_empty() {
            return Ok(());
        }
        let records = std::mem::take(pending);
        slurs.add(&records[0].slur_marks());
        let (element, dynamics, tie) = note_or_chord(&records, divisions, *previous)
            .map_err(|error| records[0].placed(error))?;
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

/// music21's `MuseDataMeasure.hasNotes`: whether a record of the measure
/// is a note, rest, cue or grace note.
pub fn has_notes(lines: &[String]) -> bool {
    lines.iter().any(|line| {
        line.chars()
            .next()
            .is_some_and(|c| "ABCDEFGrgc".contains(c))
    })
}

/// music21's `MuseDataMeasure.hasVoices`: whether a record of the measure
/// is a `back`.
pub fn has_voices(lines: &[String]) -> bool {
    lines.iter().any(|line| line.starts_with("back"))
}

/// music21's `MuseDataDirectory.isMusedataFile`: whether a path names a
/// MuseData file, by its name alone.
///
/// ```
/// use music21::musedata::is_musedata_file;
///
/// assert!(is_musedata_file("test01/01.md"));
/// assert!(!is_musedata_file("test01/mchan1"));
/// ```
pub fn is_musedata_file(path: &str) -> bool {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    if path.ends_with(".md") {
        return true;
    }
    if name.starts_with("mchan") {
        return false;
    }
    let skipped = path.ends_with(".py")
        || path.ends_with('/')
        || path.ends_with(std::path::MAIN_SEPARATOR)
        || name.starts_with('.')
        || path.ends_with(".svn-base");
    !skipped
}

/// music21's `MuseDataDirectory._prepareGroups`: the paths of a work's
/// parts, MuseData files with a digit in the name, the score file left
/// out where there are parts, sorted.
pub fn part_paths(paths: &[String]) -> Vec<String> {
    let name_of = |path: &str| path.rsplit(['/', '\\']).next().unwrap_or(path).to_string();
    let mut found: Vec<String> = paths
        .iter()
        .filter(|path| is_musedata_file(path) && !digits(&name_of(path)).is_empty())
        .cloned()
        .collect();
    if found.len() > 1 {
        found.retain(|path| {
            let letters: String = name_of(path)
                .chars()
                .filter(|c| !c.is_ascii_digit())
                .collect();
            !letters.starts_with('s')
        });
    }
    found.sort();
    found
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

/// music21's `MuseDataMeasure.getBarObject`: the barline the measure's
/// record draws.
///
/// # Errors
///
/// No records, or a barline music21 does not know.
pub fn bar_object(lines: &[String]) -> Result<Barline> {
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

/// The number a measure's record gives it, as music21 reads it: every
/// digit of the record, `1` where it gives none, and `0` for the notes
/// before the first record, a pickup.
fn measure_number(lines: &[String]) -> Result<String> {
    let line = measure_line(lines)?;
    let rest: String = line.chars().skip(8).collect();
    let opens_with_record = lines
        .first()
        .is_some_and(|first| first.trim().starts_with('m'));
    Ok(if line.chars().count() >= 9 && !rest.trim().is_empty() {
        digits(&line)
    } else if opens_with_record {
        "1".to_string()
    } else {
        "0".to_string()
    })
}

/// `MuseDataMeasure.getMeasureObject`: a measure numbered as its record
/// says, opening with its barline.
///
/// # Errors
///
/// As [`bar_object`].
pub fn measure_object(lines: &[String]) -> Result<Stream> {
    let number = measure_number(lines)?;
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

/// One line of a measure: music21's `MuseDataRecord`.
#[derive(Clone, Copy, Debug)]
pub struct Record<'a> {
    line: &'a str,
    stage: Stage,
    /// The line of the file it is, counted from one.
    number: Option<usize>,
}

impl<'a> Record<'a> {
    /// A record of `line`, written in `stage`.
    pub fn new(line: &'a str, stage: Stage) -> Self {
        Self {
            line,
            stage,
            number: None,
        }
    }
}

impl Record<'_> {
    /// `error`, placed at this record's line.
    fn placed(&self, error: Error) -> Error {
        match self.number {
            Some(number) => error.placed(|| Position::line(number)),
            None => error,
        }
    }

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

    /// music21's `isRest`.
    pub fn is_rest(&self) -> bool {
        self.first() == Some('r')
    }

    /// music21's `isNote`.
    pub fn is_note(&self) -> bool {
        self.first().is_some_and(|c| "ABCDEFG".contains(c))
    }

    /// music21's `isChord`: a further pitch of the note before.
    pub fn is_chord(&self) -> bool {
        self.first() == Some(' ') && self.at(1).is_some_and(|c| "ABCDEFG".contains(c))
    }

    /// music21's `isBack`.
    pub fn is_back(&self) -> bool {
        self.line.starts_with("back")
    }

    /// music21's `isTied`.
    pub fn is_tied(&self) -> bool {
        let column = if self.stage == Stage::One { 7 } else { 8 };
        self.at(column) == Some('-')
    }

    /// music21's `isCueOrGrace`.
    pub fn is_cue_or_grace(&self) -> bool {
        self.first().is_some_and(|c| c == 'c' || c == 'g')
    }

    /// music21's `_getPitchParameters`: the pitch's name as music21 writes
    /// it, `E-4`.
    ///
    /// # Errors
    ///
    /// A record that is no note, chord tone, cue or grace note.
    pub fn pitch_parameters(&self) -> Result<String> {
        let token = if self.is_note() {
            self.line
        } else if self.is_cue_or_grace() || self.is_chord() {
            self.line.get(1..).unwrap_or("")
        } else {
            return Err(error(
                "cannot get pitch parameters from this kind of record",
            ));
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
        Ok(format!("{step}{accidental}{}", digits(data)))
    }

    /// music21's `getPitchObject`: the pitch, with the accidental column 19
    /// shows.
    ///
    /// # Errors
    ///
    /// As [`Record::pitch_parameters`].
    pub fn pitch(&self) -> Result<Pitch> {
        let mut pitch = Pitch::from_name(self.pitch_parameters()?)?;
        if self.stage == Stage::One {
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
        if self.has_cautionary_accidental()
            && let Some(accidental) = pitch.written_accidental_mut()
        {
            accidental.set_display_status(Some(true));
        }
        Ok(pitch)
    }

    /// music21's `hasCautionaryAccidental`: a `+` among the notations.
    pub fn has_cautionary_accidental(&self) -> bool {
        self.notations()
            .is_some_and(|notations| notations.contains('+'))
    }

    /// music21's `getDots`: none for stage 1.
    pub fn dots(&self) -> Option<u8> {
        if self.stage == Stage::One {
            return None;
        }
        Some(match self.at(17) {
            Some('.') => 1,
            Some(':') => 2,
            _ => 0,
        })
    }

    /// music21's `_getAccidentalObject`: the accidental column 19 shows.
    ///
    /// # Errors
    ///
    /// None in practice: each name is one the crate knows.
    pub fn shown_accidental(&self) -> Result<Option<Accidental>> {
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

    /// music21's `getQuarterLength`, at `divisions` to the quarter.
    ///
    /// # Errors
    ///
    /// A length that is no number.
    pub fn quarter_length(&self, divisions: FloatType) -> Result<FloatType> {
        let mut count = if self.stage == Stage::One {
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

    /// music21's `getLyrics`: each verse's syllable.
    pub fn lyrics(&self) -> Option<Vec<String>> {
        if self.stage == Stage::One || self.line.chars().count() < 44 {
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

    /// music21's `getBeams`: the beam column, trailing spaces taken off.
    pub fn beams(&self) -> Option<String> {
        if self.stage == Stage::One {
            return None;
        }
        let data = self.columns(25, 31);
        (!data.is_empty()).then(|| data.trim_end().to_string())
    }

    /// `getSlurSymbols`: the slur marks among the notations, in the order
    /// they are written; a stage-1 record has none.
    pub fn slur_marks(&self) -> String {
        if self.stage == Stage::One {
            return String::new();
        }
        self.notations()
            .unwrap_or_default()
            .chars()
            .filter(|mark| "()[]{}zx".contains(*mark))
            .collect()
    }

    /// music21's `_getAdditionalNotations`: columns 32 to 43.
    pub fn notations(&self) -> Option<String> {
        if self.line.chars().count() < 31 {
            return None;
        }
        Some(self.columns(31, 43).trim().to_string())
    }

    /// music21's `getArticulationObjects`.
    pub fn articulations(&self) -> Vec<Articulation> {
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

    /// music21's `getExpressionObjects`.
    pub fn expressions(&self) -> Vec<Expression> {
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
    pub fn dynamics(&self) -> Vec<Dynamic> {
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

/// music21's `_musedataBeamToBeams`: the beams a record's beam column
/// writes, `[` starting, `]` stopping, `=` continuing and `/` and `\\`
/// partial to the right and left.
///
/// ```
/// use music21::musedata::beams_of;
///
/// assert_eq!(beams_of("]/").len(), 2);
/// ```
pub fn beams_of(written: &str) -> Beams {
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

/// music21's `_musedataRecordListToNoteOrChord`, over the lines of the
/// records: a note, or a chord of several, at `divisions` to the quarter,
/// with the dynamics the first record carries and the tie it leaves.
/// `previous` is the tie of the note before, where there is a note before.
///
/// # Errors
///
/// No records, a pitch or a length that cannot be read.
pub fn record_note(
    lines: &[String],
    stage: Stage,
    divisions: FloatType,
    previous: Option<Option<TieType>>,
) -> Result<(StreamElement, Vec<Dynamic>, Option<TieType>)> {
    let records: Vec<Record<'_>> = lines
        .iter()
        .map(|line| Record {
            line,
            stage,
            number: None,
        })
        .collect();
    if records.is_empty() {
        return Err(error("a note of no records"));
    }
    note_or_chord(&records, divisions, previous)
}

/// music21's `base40DeltaToInterval`, refused as a MuseData error.
fn base40_interval(delta: i32) -> Result<Interval> {
    base40::delta_to_interval(delta).map_err(|refused| error(refused.message()))
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
    fn a_measure_number_is_every_digit_of_its_record_and_a_pickup_is_0() {
        let lines = ["mheavy2 12".to_string(), "C4 1 q".to_string()];
        assert_eq!(measure_object(&lines).unwrap().number(), 212);
        let lines = ["measure".to_string(), "C4 1 q".to_string()];
        assert_eq!(measure_object(&lines).unwrap().number(), 1);
        let lines = ["C4 1 q".to_string()];
        assert_eq!(measure_object(&lines).unwrap().number(), 0);
    }

    #[test]
    fn a_refusal_says_its_line_measure_and_part() {
        let part = |attributes: &str, record: &str| {
            format!(
                "\n\n\n01/01/01 x\nWK#:1 MV#:1\nsource\nWork\nMovement\nFlute\n\
                 1 0\nGroup memberships: score\nscore: part 1 of 1\n\
                 & a comment\n&\n{attributes}\nC5     2        q     u\n\
                 D5     2        q     u\nmeasure 2\n{record}\n/END\n"
            )
        };
        let error =
            from_musedata(&[&part("$  K:0   Q:2   T:2/4   C:4", "E5     x        h")]).unwrap_err();
        assert_eq!(error.to_string(), "MuseData error: \"  x\" is not a number");
        let mut wanted = Position::line(19).in_part("Flute");
        wanted.measure = Some("2".to_string());
        assert_eq!(error.position(), Some(&wanted));

        let error =
            from_musedata(&[&part("$  K:0   Q:2   T:x/4   C:4", "E5     4        h")]).unwrap_err();
        assert_eq!(error.position(), Some(&Position::line(15).in_part("Flute")));
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
            stage: Stage::Two,
            number: None,
        };
        let values: Vec<String> = record
            .dynamics()
            .iter()
            .map(|dynamic| dynamic.value().to_string())
            .collect();
        assert_eq!(values, ["pp"]);
    }

    #[test]
    fn ties_stay_in_their_voice_across_a_back() {
        let part = "\n\n\n04/16/93 E. Correia\nWK#:581       MV#:3c\n\
            Breitkopf & Hartel, Vol. 13\nClarinet Quintet\nTrio II\nPiano\n1 0\n\
            Group memberships: score\nscore: part 1 of 1\n\
            $  K:0   Q:1   T:2/4   C:4\nmeasure 1\nC5     1        q     u\n\
            E5     1-       q     u\nback   2\nC4     1        q     d\n\
            G3     1-       q     d\nmeasure 2\nE5     2        h     u\n\
            back   2\nG3     1        q     d\nA3     1        q     d\n\
            measure 3\nF5     2        h\n/END\n";
        let score = from_musedata(&[part]).expect("the part is read");
        let ties = |stream: &Stream| -> Vec<Option<&'static str>> {
            stream
                .notes()
                .iter()
                .map(|(_, element)| match element {
                    StreamElement::Note(note) => note.tie().map(|tie| tie.tie_type().as_str()),
                    _ => None,
                })
                .collect()
        };
        let parts = score.parts();
        let measures = parts[0].measures();
        let first = measures[0].voices();
        let second = measures[1].voices();
        assert_eq!(ties(first[0]), [None, Some("start")]);
        assert_eq!(ties(first[1]), [None, Some("start")]);
        assert_eq!(ties(second[0]), [Some("stop")]);
        assert_eq!(ties(second[1]), [Some("stop"), None]);
        assert_eq!(ties(measures[2]), [None]);
    }

    #[test]
    fn windows_line_endings_are_read_as_unix_ones() {
        let unix = "Title\nSource\n$ K:0 Q:4 T:4/4 C:4\nC4 4 q\n/END\n";
        let windows = unix.replace('\n', "\r\n");
        let lines = |text: &str| -> Vec<Vec<String>> {
            split_parts(text)
                .expect("the parts are read")
                .into_iter()
                .map(|part| part.lines)
                .collect()
        };
        assert_eq!(lines(&windows), lines(unix));
        assert!(
            lines(&windows)
                .iter()
                .flatten()
                .all(|line| !line.contains('\r'))
        );
    }

    /// A stage-2 header as music21's test files write one.
    fn header_part() -> Part {
        let lines: Vec<String> = [
            "",
            "",
            "",
            "",
            "WK#:1         MV#:2",
            "Synthetic",
            "Air",
            "Second",
            "Upper",
            "",
            "Group memberships: sound, score",
            "sound: part 1 of 2",
            "score: part 1 of 2",
            "$  K:-1   Q:4   T:3/4   C:4   D:Andante",
            "measure 1",
            "C4     4        q     u",
        ]
        .iter()
        .map(|line| line.to_string())
        .collect();
        Part::with_stage(lines, Stage::Two)
    }

    #[test]
    fn a_part_answers_music21_s_header_questions() {
        let part = header_part();
        assert_eq!(part.work_number().as_deref(), Some("1"));
        assert_eq!(part.source().as_deref(), Some("Synthetic"));
        assert_eq!(part.group_memberships(), ["sound", "score"]);
        assert_eq!(part.group_total("score").unwrap(), Some(2));
        assert_eq!(part.group_number("sound").unwrap().as_deref(), Some("1"));
        assert_eq!(part.key_parameters().unwrap(), -1);
        assert_eq!(part.time_signature_parameters().unwrap(), "3/4");
        assert_eq!(part.clef_parameters().unwrap(), ["4"]);
        assert_eq!(part.directive().unwrap().as_deref(), Some("Andante"));
        assert_eq!(part.transposition_parameters().unwrap(), None);
        // music21's `_getMeasureBoundaryIndices` docstring, ends exclusive.
        let lines: Vec<String> = ["$", "m", "B", "C", "m", "D", "E", "m", "F", "D"]
            .iter()
            .map(|line| line.to_string())
            .collect();
        let bounds = Part::with_stage(lines, Stage::Two)
            .measure_bounds()
            .unwrap();
        assert_eq!(bounds, [(1, 4), (4, 7), (7, 10)]);
    }

    #[test]
    fn a_record_answers_music21_s_questions() {
        let record = Record::new("F#4    1        s #   d  ==", Stage::Two);
        assert_eq!(record.pitch_parameters().unwrap(), "F#4");
        assert!(record.shown_accidental().unwrap().is_some());
        assert_eq!(record.dots(), Some(0));
        let marked = Record::new("F5     3        t n   d  ==[   (+", Stage::Two);
        assert!(marked.has_cautionary_accidental());
        assert!(Record::new("back   4", Stage::Two).is_back());
        assert!(
            Record::new("back   4", Stage::Two)
                .pitch_parameters()
                .is_err()
        );
    }

    #[test]
    fn a_directory_keeps_its_part_files() {
        let names: Vec<String> = [
            "w/02.md", "w/s01", "w/01.md", "w/mchan1", "w/.svn", "w/notes",
        ]
        .iter()
        .map(|name| name.to_string())
        .collect();
        assert_eq!(part_paths(&names), ["w/01.md", "w/02.md"]);
    }
}
