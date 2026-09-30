//! Humdrum: music21's `humdrum.spineParser`, which reads `**kern` scores.
//!
//! A Humdrum file is a table. Each column is a *spine* -- a part -- and each
//! line a moment; a cell holds a note (`4c#` is a quarter C sharp above
//! middle C), a barline (`=12`), an instruction (`*clefG2`, `*M3/4`) or a
//! dot where nothing new happens. A spine may split into two voices (`*^`)
//! and come together again (`*v`). `**dynam` spines carry dynamics and
//! `**text` spines the words sung, each said by `*staff` to belong to a
//! `**kern` spine.

mod tables;

use std::collections::BTreeMap;

use crate::articulations::{Articulation, ArticulationKind};
use crate::bar::{Barline, BarlineType, RepeatDirection};
use crate::chord::Chord;
use crate::clef::{Clef, ClefKind};
use crate::defaults::{FloatType, IntegerType};
use crate::duration::{Duration, DurationType, Grace, Tuplet, TupletType};
use crate::dynamics::Dynamic;
use crate::error::{Error, Result};
use crate::expressions::{Expression, Fermata, Ornament, OrnamentKind};
use crate::instrument::Instrument;
use crate::key::{Key, KeySignature};
use crate::makenotation::op_frac;
use crate::metadata::{Metadata, is_contributor_unique_name};
use crate::meter::TimeSignature;
use crate::notation::{BeamDirection, BeamType, StemDirection, Tie, TieType};
use crate::note::Note;
use crate::pitch::{Accidental, PitchOptions};
use crate::rest::Rest;
use crate::stream::{Stream, StreamElement, StreamEvent, StreamKind};
use crate::tempo::MetronomeMark;

use tables::{INSTRUMENT_CLASSES, INSTRUMENTS, REFERENCE_NAMES};

fn humdrum_error(message: impl Into<String>) -> Error {
    Error::Humdrum(message.into())
}

/// The tokens that say where a spine goes rather than what it holds.
const SPINE_PATHS: [&str; 6] = ["*+", "*-", "*^", "*v", "*x", "*"];

/// A barline as a spine states it: the measure it opens.
#[derive(Clone, Debug, Default)]
struct Measure {
    number: IntegerType,
    suffix: Option<String>,
    left: Option<Barline>,
    right: Option<Barline>,
}

/// What a cell of a spine came to.
// Nearly every thing is an element, so boxing it would cost more than the
// few smaller variants waste.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
enum Thing {
    Element(StreamElement),
    Measure(Measure),
    /// An instruction with no meaning here, kept for `*staff`.
    Tandem(String),
    /// A word of a `**text` spine.
    Word(String),
}

/// A thing in a spine's stream: where it stands, the line it was written
/// on -- which music21 sorts by ahead of everything but the offset -- and
/// the voice it belongs to while a spine is split.
#[derive(Clone, Debug)]
struct Placed {
    offset: FloatType,
    line: usize,
    /// Nought for a thing no line can be asked for any more.
    priority: usize,
    voice: Option<usize>,
    thing: Thing,
}

impl Placed {
    fn length(&self) -> FloatType {
        match &self.thing {
            Thing::Element(element) => element.quarter_length(),
            _ => 0.0,
        }
    }

    fn class_order(&self) -> i32 {
        match &self.thing {
            Thing::Element(element) => element.class_sort_order(),
            Thing::Measure(_) => -20,
            _ => 0,
        }
    }
}

/// music21's stream order: offset, then the priority, then the class.
fn sort(things: &mut [Placed]) {
    things.sort_by(|left, right| {
        left.offset
            .total_cmp(&right.offset)
            .then(left.priority.cmp(&right.priority))
            .then(left.class_order().cmp(&right.class_order()))
    });
}

fn highest(things: &[Placed]) -> FloatType {
    things
        .iter()
        .map(|thing| thing.offset + thing.length())
        .fold(0.0, FloatType::max)
}

/// One spine: music21's `HumdrumSpine`.
#[derive(Clone, Debug, Default)]
struct Spine {
    events: Vec<(String, usize)>,
    parent: Option<usize>,
    first_voice: bool,
    /// The spines this one splits into, by the line it splits on.
    splits: BTreeMap<usize, Vec<usize>>,
    kind: String,
    stream: Vec<Placed>,
}

/// The lengths a number names: music21's `typeFromNumDict`.
fn type_of_number(number: u64) -> Option<DurationType> {
    ((1..=2048).contains(&number) && number.is_power_of_two())
        .then(|| {
            DurationType::ALL
                .into_iter()
                .find(|kind| kind.quarter_length() == 4.0 / number as FloatType)
        })
        .flatten()
}

fn first_number(text: &str) -> Option<&str> {
    let start = text.find(|c: char| c.is_ascii_digit())?;
    let rest = &text[start..];
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    Some(&rest[..end])
}

/// music21's `hdStringToDuration`: `4` is a quarter, `8.` a dotted eighth,
/// `12` an eighth of a triplet, `0` a breve, and `q` a grace note.
fn duration_of(contents: &str) -> Result<Duration> {
    let dots = contents.matches('.').count() as u32;
    let has_grace = contents.contains('q') || contents.contains('Q');
    let number = first_number(contents);
    let rational = contents.contains('%').then(|| {
        let at = contents.find('%')?;
        let before = &contents[..at];
        let start = before
            .rfind(|c: char| !c.is_ascii_digit())
            .map_or(0, |index| index + 1);
        let first = before[start..].parse::<FloatType>().ok()?;
        let second = first_number(&contents[at + 1..])
            .filter(|_| contents[at + 1..].starts_with(|c: char| c.is_ascii_digit()))?
            .parse::<FloatType>()
            .ok()?;
        Some((first, second))
    });
    let mut duration = if let Some(Some((first, second))) = rational {
        let plain = Duration::new(op_frac(4.0 * second / first))?;
        match plain.type_and_dots() {
            Some((kind, _)) if dots > 0 => Duration::from_type_with_dots(kind, dots),
            _ => plain,
        }
    } else if let Some(number) = number {
        let value: u64 = number
            .parse()
            .map_err(|_| humdrum_error(format!("Could not find a duration in {contents:?}")))?;
        if value == 0 {
            let kind = match number {
                "000" => DurationType::Maxima,
                "00" => DurationType::Longa,
                _ => DurationType::Breve,
            };
            Duration::from_type_with_dots(kind, dots)
        } else if let Some(kind) = type_of_number(value) {
            Duration::from_type_with_dots(kind, dots)
        } else {
            // A number that is no power of two is that many in the time of
            // the power of two below it: twelfths are triplet eighths.
            let base = 1_u64 << value.ilog2();
            let kind = type_of_number(base)
                .ok_or_else(|| humdrum_error(format!("no note value for {value}")))?;
            let divisor = gcd(value, base);
            let mut tuplet =
                Tuplet::new((value / divisor) as u32, (base / divisor) as u32, kind, 0);
            if dots > 0 {
                tuplet.set_duration_normal(Some((kind, dots)));
            }
            let mut duration = Duration::from_type(kind);
            duration.append_tuplet(tuplet);
            duration
        }
    } else if has_grace {
        Duration::from_type(DurationType::Eighth)
    } else {
        return Err(humdrum_error(format!(
            "Could not find a duration in {contents:?}"
        )));
    };
    if has_grace {
        duration = duration.grace_duration();
        let mut grace = Grace::new();
        if contents.matches('q').count() != 1 {
            grace.set_slash(false);
        }
        duration.set_grace(Some(grace));
    } else if contents.contains('P') {
        duration = duration.grace_duration();
        duration.set_grace(Some(Grace::appoggiatura()));
    }
    Ok(duration)
}

fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left
}

/// What is read off a note's token besides the note itself.
struct Marks {
    tie: Option<TieType>,
    expressions: Vec<Expression>,
    articulations: Vec<Articulation>,
    stem: StemDirection,
    beams: Vec<(BeamType, Option<BeamDirection>)>,
}

fn marks_of(contents: &str) -> Marks {
    let has = |mark: char| contents.contains(mark);
    let tie = if has('[') {
        Some(TieType::Start)
    } else if has(']') {
        Some(TieType::Stop)
    } else if has('_') {
        Some(TieType::Continue)
    } else {
        None
    };
    let ornament = |kind: OrnamentKind| Expression::from(Ornament::of_kind(kind));
    let mut expressions = Vec::new();
    if has('t') {
        expressions.push(ornament(OrnamentKind::HalfStepTrill));
    } else if has('T') {
        expressions.push(ornament(OrnamentKind::WholeStepTrill));
    }
    if has('w') {
        expressions.push(ornament(OrnamentKind::HalfStepInvertedMordent));
    } else if has('W') {
        expressions.push(ornament(OrnamentKind::WholeStepInvertedMordent));
    } else if has('m') {
        expressions.push(ornament(OrnamentKind::HalfStepMordent));
    } else if has('M') {
        expressions.push(ornament(OrnamentKind::WholeStepMordent));
    }
    if has('S') {
        expressions.push(ornament(OrnamentKind::Turn));
    } else if has('$') {
        expressions.push(ornament(OrnamentKind::InvertedTurn));
    } else if has('R') {
        expressions.push(ornament(OrnamentKind::Turn));
    }
    if has('O') {
        expressions.push(ornament(OrnamentKind::Ornament));
    }
    let mut articulations = Vec::new();
    let mut articulation = |class: &str| {
        if let Some(kind) = ArticulationKind::from_class_name(class) {
            articulations.push(Articulation::of_kind(kind));
        }
    };
    if has('\'') {
        articulation("Staccato");
    }
    if has('"') {
        articulation("Pizzicato");
    }
    if has('`') {
        articulation("Staccatissimo");
    }
    if has('~') {
        articulation("Tenuto");
    }
    if has('^') {
        articulation("Accent");
    }
    if has(';') {
        expressions.push(Expression::Fermata(Fermata::new()));
    }
    if has('v') {
        articulation("UpBow");
    } else if has('u') {
        articulation("DownBow");
    }
    let stem = if has('/') {
        StemDirection::Up
    } else if has('\\') {
        StemDirection::Down
    } else {
        StemDirection::Unspecified
    };
    let mut beams = Vec::new();
    for (mark, beam, direction) in [
        ('L', BeamType::Start, None),
        ('J', BeamType::Stop, None),
        ('k', BeamType::PartialBeam, Some(BeamDirection::Left)),
        ('K', BeamType::PartialBeam, Some(BeamDirection::Right)),
    ] {
        for _ in 0..contents.matches(mark).count() {
            beams.push((beam, direction));
        }
    }
    Marks {
        tie,
        expressions,
        articulations,
        stem,
        beams,
    }
}

/// music21's `hdStringToNote`: a note or a rest from its token.
fn note_of(contents: &str, given: Option<&Duration>) -> Result<StreamElement> {
    // A token with no length is a quarter, as music21 makes it with a
    // warning.
    let duration = match given {
        Some(duration) => duration.clone(),
        None => duration_of(contents).unwrap_or_else(|_| Duration::quarter()),
    };
    let marks = marks_of(contents);
    if contents.contains('r') {
        let mut rest = Rest::new(duration);
        rest.set_tie(marks.tie.map(Tie::new));
        *rest.expressions_mut() = marks.expressions;
        *rest.articulations_mut() = marks.articulations;
        return Ok(rest.into());
    }
    let letters = |c: char| matches!(c, 'a'..='g' | 'A'..='G');
    let start = contents
        .find(letters)
        .ok_or_else(|| humdrum_error(format!("Could not parse {contents} for note information")))?;
    let name: Vec<char> = contents[start..]
        .chars()
        .take_while(|c| letters(*c))
        .collect();
    let first = name[0];
    let count = name.len() as IntegerType;
    let octave = if first.is_ascii_lowercase() {
        3 + count
    } else {
        4 - count
    };
    let mut pitch = PitchOptions::new()
        .step(first.to_ascii_uppercase())
        .octave(octave)
        .build()?;
    let sharps = contents.matches('#').count() as IntegerType;
    let flats = contents.matches('-').count() as IntegerType;
    if sharps > 0 {
        pitch.set_written_accidental(Some(Accidental::new(sharps)?));
    } else if flats > 0 {
        pitch.set_written_accidental(Some(Accidental::new(-flats)?));
    } else if contents.contains('n') {
        pitch.set_written_accidental(Some(Accidental::new("n")?));
    }
    let mut note = Note::from_pitch(pitch).with_duration(duration);
    note.set_tie(marks.tie.map(Tie::new));
    *note.expressions_mut() = marks.expressions;
    *note.articulations_mut() = marks.articulations;
    note.set_stem_direction(marks.stem);
    let mut beams = note.beams().clone();
    for (beam, direction) in marks.beams {
        beams.append(beam, direction);
    }
    note.set_beams(beams);
    Ok(note.into())
}

/// music21's `hdStringToMeasure`: the measure a barline opens, and the
/// barline it puts at the end of the measure before.
fn measure_of(contents: &str, has_previous: bool) -> (Measure, Barline) {
    let mut measure = Measure::default();
    if let Some(number) = first_number(contents) {
        measure.number = number.parse().unwrap_or(0);
        let after = &contents[contents.find(number).unwrap_or(0) + number.len()..];
        measure.suffix = after
            .chars()
            .next()
            .filter(char::is_ascii_lowercase)
            .map(String::from);
    }
    let bar_type = if contents.contains('-') {
        Some(BarlineType::None)
    } else if contents.contains('\'') {
        Some(BarlineType::Short)
    } else if contents.contains('`') {
        Some(BarlineType::Tick)
    } else if contents.contains("||") || contents.contains("==") {
        Some(BarlineType::Double)
    } else if contents.contains("!!") {
        Some(BarlineType::HeavyHeavy)
    } else if contents.contains("|!") {
        Some(BarlineType::Final)
    } else if contents.contains("!|") {
        Some(BarlineType::HeavyLight)
    } else if contents.contains('|') {
        Some(BarlineType::Regular)
    } else {
        None
    };
    let chars: Vec<char> = contents.chars().collect();
    let beside = |colon_first: bool| {
        chars.windows(2).any(|pair| {
            let (colon, bar) = if colon_first {
                (pair[0], pair[1])
            } else {
                (pair[1], pair[0])
            };
            colon == ':' && matches!(bar, '|' | '!' | '=')
        })
    };
    let both = contents.matches(':').count() > 1;
    let end_repeat = both || beside(true);
    let start_repeat = both || beside(false);
    let make = |direction: Option<RepeatDirection>| {
        let mut barline = match direction {
            Some(direction) => Barline::repeat(direction, None),
            None => Barline::new(BarlineType::Regular),
        };
        if let Some(bar_type) = bar_type {
            barline.set_bar_type(bar_type);
        }
        barline
    };
    let right = make(end_repeat.then_some(RepeatDirection::End));
    if has_previous {
        if start_repeat {
            measure.left = Some(make(Some(RepeatDirection::Start)));
        }
    } else {
        measure.left = Some(if start_repeat {
            make(Some(RepeatDirection::Start))
        } else {
            right.clone()
        });
    }
    (measure, right)
}

/// music21's `kernTandemToObject`: what an instruction in a `**kern` spine
/// stands for.
fn tandem_of(tandem: &str) -> Result<Option<Thing>> {
    let element = |element: StreamElement| Ok(Some(Thing::Element(element)));
    if SPINE_PATHS.contains(&tandem) {
        return Ok(None);
    }
    if let Some(kind) = tandem.strip_prefix("*clef") {
        let clef = match kind {
            "-" => Clef::of_kind(ClefKind::NoClef),
            "X" => Clef::of_kind(ClefKind::PercussionClef),
            "Gv2" | "Gv" => Clef::of_kind(ClefKind::Treble8vbClef),
            "G^2" | "G^" => Clef::of_kind(ClefKind::Treble8vaClef),
            "Fv4" | "Fv" => Clef::of_kind(ClefKind::Bass8vbClef),
            other => Clef::from_string(other, 0)
                .map_err(|_| humdrum_error(format!("Unknown clef type {tandem} found")))?,
        };
        return element(clef.into());
    }
    if let Some(text) = tandem.strip_prefix("*MM") {
        let mark = match text.parse::<FloatType>() {
            Ok(number) => MetronomeMark::new(number),
            Err(_) => {
                let text = text.strip_prefix('[').unwrap_or(text);
                let text = text.trim_end();
                MetronomeMark::from_text(text.strip_suffix(']').unwrap_or(text))
            }
        };
        return element(mark.into());
    }
    if let Some(meter) = tandem.strip_prefix("*M") {
        let digits = |text: &str| text.chars().take_while(char::is_ascii_digit).count();
        let top = digits(meter);
        let bottom = meter
            .get(top + 1..)
            .filter(|_| top > 0 && meter[top..].starts_with('/'))
            .map(|rest| &rest[..digits(rest)])
            .filter(|bottom| !bottom.is_empty())
            .ok_or_else(|| humdrum_error(format!("Incorrect meter: {tandem} found")))?;
        let numerator: u32 = meter[..top]
            .parse()
            .map_err(|_| humdrum_error(format!("Incorrect meter: {tandem} found")))?;
        let signature = match bottom {
            "0" => TimeSignature::new(numerator * 2, 1)?,
            "00" => TimeSignature::new(numerator * 4, 1)?,
            "000" => TimeSignature::new(numerator * 8, 1)?,
            _ => TimeSignature::from_ratio_string(meter)?,
        };
        return element(signature.into());
    }
    let instrument = |table: &[(&str, &str)], code: &str| -> Option<Instrument> {
        let class = table.iter().find(|(held, _)| *held == code)?.1;
        Instrument::of_kind(class).ok()
    };
    if let Some(class) = tandem.strip_prefix("*IC") {
        return Ok(Some(match instrument(INSTRUMENT_CLASSES, class) {
            Some(instrument) => Thing::Element(instrument.into()),
            None => Thing::Tandem(class.to_string()),
        }));
    }
    if tandem.starts_with("*IG") || tandem.starts_with("*ITr") {
        return Ok(Some(Thing::Tandem(tandem.to_string())));
    }
    if let Some(code) = tandem.strip_prefix("*I") {
        return Ok(Some(match instrument(INSTRUMENTS, code) {
            Some(instrument) => Thing::Element(instrument.into()),
            None => Thing::Tandem(code.to_string()),
        }));
    }
    if tandem.starts_with("*k") {
        let mut sharps = tandem.matches('#').count() as IntegerType;
        if sharps == 0 {
            sharps = -(tandem.matches('-').count() as IntegerType);
        }
        return element(KeySignature::new(sharps).into());
    }
    if let Some(key) = tandem.strip_suffix(':') {
        return element(StreamElement::Key(Key::from_tonic(&key[1..])?));
    }
    Ok(Some(Thing::Tandem(tandem.to_string())))
}

/// What a `**kern` spine keeps from one note to the next: music21's
/// `KernSpine` state.
#[derive(Default)]
struct KernState {
    in_tuplet: bool,
    last_note: Option<usize>,
    beams: usize,
    tuplet_length: FloatType,
    wanted_length: FloatType,
}

fn with_duration(element: &mut StreamElement, change: impl FnOnce(&mut Duration)) {
    let Some(mut duration) = element.duration().cloned() else {
        return;
    };
    change(&mut duration);
    match element {
        StreamElement::Note(note) => note.set_duration(duration),
        StreamElement::Chord(chord) => chord.set_duration(duration),
        StreamElement::Rest(rest) => rest.set_duration(duration),
        _ => {}
    }
}

fn mark_first_tuplet(element: &mut StreamElement, kind: TupletType) {
    with_duration(element, |duration| {
        let mut tuplets = duration.tuplets();
        if let Some(first) = tuplets.first_mut() {
            first.set_tuplet_type(Some(kind));
        }
        duration.set_tuplets(tuplets);
    });
}

impl KernState {
    /// music21's `setBeamsForNote`: a note between the start and the end of
    /// a beam carries it on.
    fn beam(&mut self, element: &mut StreamElement) {
        let beams = match element {
            StreamElement::Note(note) => note.beams().clone(),
            StreamElement::Chord(chord) => chord.beams().clone(),
            _ => return,
        };
        if self.beams != 0 && beams.is_empty() {
            let mut carried = beams;
            for _ in 0..self.beams {
                carried.append(BeamType::Continue, None);
            }
            match element {
                StreamElement::Note(note) => note.set_beams(carried),
                StreamElement::Chord(chord) => chord.set_beams(carried),
                _ => {}
            }
        } else if let Some(first) = beams.beams().first() {
            if first.beam_type() == Some(BeamType::Stop) {
                self.beams = 0;
            } else {
                self.beams += beams
                    .beams()
                    .iter()
                    .filter(|beam| beam.beam_type() != Some(BeamType::Stop))
                    .count();
            }
        }
    }

    /// music21's `setTupletTypeForNote`: where a run of tuplet notes starts
    /// and where it has filled its bracket.
    fn tuplet(&mut self, stream: &mut [Placed], element: &mut StreamElement) {
        let Some(duration) = element.duration().cloned() else {
            return;
        };
        let tuplets = duration.tuplets();
        if !self.in_tuplet && !tuplets.is_empty() {
            self.in_tuplet = true;
            self.wanted_length = tuplets[0].total_tuplet_length();
            self.tuplet_length = duration.quarter_length();
            mark_first_tuplet(element, TupletType::Start);
        } else if self.in_tuplet && tuplets.is_empty() {
            self.in_tuplet = false;
            self.wanted_length = 0.0;
            self.tuplet_length = 0.0;
            if let Some(last) = self.last_note
                && let Thing::Element(last) = &mut stream[last].thing
            {
                mark_first_tuplet(last, TupletType::Stop);
            }
        } else if self.in_tuplet {
            self.tuplet_length = op_frac(self.tuplet_length + duration.quarter_length());
            if self.tuplet_length >= self.wanted_length
                && self.tuplet_length % self.wanted_length == 0.0
            {
                mark_first_tuplet(element, TupletType::Stop);
                self.in_tuplet = false;
                self.tuplet_length = 0.0;
                self.wanted_length = 0.0;
            }
        }
    }
}

/// music21's `processChordEvent`: the notes of a cell holding several.
fn chord_of(contents: &str) -> Result<StreamElement> {
    let mut notes: Vec<Note> = Vec::new();
    let mut first: Option<(Duration, String)> = None;
    for token in contents.split_whitespace() {
        let length: String = token
            .chars()
            .filter(|c| c.is_ascii_digit() || matches!(c, '.' | '%'))
            .collect();
        let bare: String = token
            .chars()
            .filter(|c| !(c.is_ascii_digit() || matches!(c, '.' | '%')))
            .collect();
        let element = match &first {
            Some((duration, written)) if length.is_empty() || length == *written => {
                note_of(&bare, Some(duration))?
            }
            _ => {
                let element = note_of(token, None)?;
                if first.is_none()
                    && let Some(duration) = element.duration()
                {
                    first = Some((duration.clone(), length));
                }
                element
            }
        };
        if let StreamElement::Note(note) = element {
            notes.push(note);
        }
    }
    let last_beams = notes
        .last()
        .map(|note| note.beams().clone())
        .ok_or_else(|| humdrum_error("a chord with no notes in it"))?;
    let duration = first.map(|(duration, _)| duration).unwrap_or_default();
    let mut chord = Chord::new(notes)?.with_duration(duration);
    chord.set_beams(last_beams);
    Ok(chord.into())
}

/// Appends a thing to a spine's stream, after all it holds.
fn append(stream: &mut Vec<Placed>, line: usize, priority: usize, thing: Thing) -> usize {
    let offset = highest(stream);
    stream.push(Placed {
        offset,
        line,
        priority,
        voice: None,
        thing,
    });
    stream.len() - 1
}

/// music21's `KernSpine.parse`.
fn parse_kern(events: &[(String, usize)]) -> Vec<Placed> {
    let mut stream: Vec<Placed> = Vec::new();
    let mut state = KernState::default();
    // The measure a barline closes, by its place in the stream.
    let mut last_measure: Option<usize> = None;
    for (contents, line) in events {
        let made: Result<Option<Thing>> = (|| {
            if contents == "." || contents.is_empty() {
                return Ok(None);
            }
            if contents.starts_with('*') {
                return tandem_of(contents);
            }
            if contents.starts_with('=') {
                // music21 starts every spine off with a measure nought of
                // its own, so the first barline always has one before it.
                let (measure, right) = measure_of(contents, true);
                if let Some(previous) = last_measure
                    && let Thing::Measure(previous) = &mut stream[previous].thing
                {
                    previous.right = Some(right);
                }
                return Ok(Some(Thing::Measure(measure)));
            }
            if contents.starts_with('!') {
                return Ok(None);
            }
            let mut element = if contents.contains(' ') {
                chord_of(contents)?
            } else {
                note_of(contents, None)?
            };
            state.beam(&mut element);
            state.tuplet(&mut stream, &mut element);
            Ok(Some(Thing::Element(element)))
        })();
        // A cell that cannot be read is passed over, as music21 passes over
        // it with a warning.
        let Ok(Some(thing)) = made else {
            continue;
        };
        let is_measure = matches!(thing, Thing::Measure(_));
        let sounds = matches!(
            &thing,
            Thing::Element(
                StreamElement::Note(_) | StreamElement::Chord(_) | StreamElement::Rest(_)
            )
        );
        let index = append(&mut stream, *line, *line, thing);
        if is_measure {
            last_measure = Some(index);
        }
        if sounds {
            state.last_note = Some(index);
        }
    }
    stream
}

/// music21's `DynamSpine.parse` and the generic one: the marks and words of
/// a spine that is no `**kern`, each with its line.
fn parse_other(events: &[(String, usize)], dynamics: bool) -> Vec<Placed> {
    let mut stream = Vec::new();
    for (contents, line) in events {
        let thing = if contents == "." || contents.is_empty() {
            continue;
        } else if contents.starts_with('*') {
            if SPINE_PATHS.contains(&contents.as_str()) {
                continue;
            }
            Thing::Tandem(contents.clone())
        } else if contents.starts_with('=') || contents.starts_with('!') {
            continue;
        } else if dynamics {
            if contents.starts_with('<') || contents.starts_with('>') {
                continue;
            }
            Thing::Element(Dynamic::new(contents.as_str()).into())
        } else {
            Thing::Word(contents.clone())
        };
        append(&mut stream, *line, 0, thing);
    }
    stream
}

/// music21's `createHumdrumSpines`: the spines a table's columns run
/// through, splitting, joining and changing places as the table says.
fn spines_of(lines: &[(usize, Vec<String>)], width: usize) -> Result<Vec<Spine>> {
    let mut spines: Vec<Spine> = Vec::new();
    let mut current: Vec<Option<usize>> = Vec::new();
    for (line, cells) in lines {
        if current.len() < width {
            current.resize(width, None);
        }
        for (column, cell) in cells.iter().enumerate().take(width) {
            let spine = match current[column] {
                Some(spine) => spine,
                None => {
                    spines.push(Spine::default());
                    current[column] = Some(spines.len() - 1);
                    spines.len() - 1
                }
            };
            spines[spine].events.push((cell.clone(), *line));
        }
        if !cells
            .iter()
            .any(|cell| SPINE_PATHS.contains(&cell.as_str()))
        {
            continue;
        }
        let mut next: Vec<Option<usize>> = Vec::new();
        let mut merging = false;
        let mut merge_parent: Option<usize> = None;
        let mut exchange: Option<usize> = None;
        for (column, spine) in current.clone().into_iter().enumerate().take(width) {
            let Some(cell) = cells.get(column) else {
                if spine.is_some() {
                    next.push(spine);
                }
                continue;
            };
            let Some(spine) = spine else {
                return Err(humdrum_error(format!(
                    "spine-path token at line {line} has no active spine"
                )));
            };
            match cell.as_str() {
                "*-" => {}
                "*^" => {
                    let mut voices = Vec::new();
                    for first in [true, false] {
                        spines.push(Spine {
                            parent: Some(spine),
                            first_voice: first,
                            ..Spine::default()
                        });
                        voices.push(spines.len() - 1);
                        next.push(Some(spines.len() - 1));
                    }
                    spines[spine].splits.insert(*line, voices);
                }
                "*v" => {
                    if merging {
                        let parent = spines[spine].parent.or(merge_parent);
                        match parent {
                            Some(parent) => next.push(Some(parent)),
                            None => {
                                spines.push(Spine::default());
                                next.push(Some(spines.len() - 1));
                            }
                        }
                        merging = false;
                        merge_parent = None;
                    } else {
                        merging = true;
                        merge_parent = spines[spine].parent;
                    }
                }
                "*x" => match exchange.take() {
                    None => exchange = Some(spine),
                    Some(waiting) => {
                        next.push(Some(spine));
                        next.push(Some(waiting));
                    }
                },
                _ => next.push(Some(spine)),
            }
        }
        if exchange.is_some() {
            return Err(humdrum_error(format!(
                "ProtoSpine found with unpaired exchange instruction at line {line}"
            )));
        }
        current = next;
    }
    Ok(spines)
}

/// The kind of a spine: what its `**` heading says, or its parent's.
fn kind_of(spines: &[Spine], index: usize) -> Result<String> {
    let mut at = Some(index);
    while let Some(spine) = at {
        if let Some(kind) = spines[spine]
            .events
            .iter()
            .find_map(|(contents, _)| contents.strip_prefix("**"))
        {
            return Ok(kind.to_string());
        }
        at = spines[spine].parent;
    }
    Err(humdrum_error(format!(
        "Could not determine spineType for spine with id {index}"
    )))
}

/// music21's `performInsertions`: the voices a spine split into, put back
/// into it where it split.
fn insert_voices(spines: &mut [Spine], index: usize) {
    let mut points: Vec<usize> = spines[index].splits.keys().copied().collect();
    if points.is_empty() {
        return;
    }
    let old = std::mem::take(&mut spines[index].stream);
    let mut new: Vec<Placed> = Vec::new();
    let insert = |new: &mut Vec<Placed>, spines: &[Spine], point: usize| {
        let start = highest(new);
        for (number, child) in spines[index].splits[&point].iter().enumerate() {
            for thing in &spines[*child].stream {
                if !spines[*child].first_voice && matches!(thing.thing, Thing::Measure(_)) {
                    continue;
                }
                let mut thing = thing.clone();
                thing.offset += start;
                thing.voice.get_or_insert(number + 1);
                new.push(thing);
            }
        }
    };
    let mut last: Option<usize> = None;
    for thing in old {
        // Python walks the list it is taking things out of, so the point
        // after one that is used is passed over until the next element.
        let mut at = 0;
        while at < points.len() {
            let point = points[at];
            if last.is_some_and(|last| last > point) || thing.line < point {
                at += 1;
                continue;
            }
            insert(&mut new, spines, point);
            points.remove(at);
            at += 1;
        }
        last = Some(thing.line);
        let offset = highest(&new);
        new.push(Placed { offset, ..thing });
    }
    for point in points {
        insert(&mut new, spines, point);
    }
    spines[index].stream = new;
}

/// A measure while elements are gathered into it.
struct Gathered {
    measure: Measure,
    offset: FloatType,
    things: Vec<Placed>,
}

/// music21's `moveElementsIntoMeasures`: the barlines of a spine as the
/// measures its notes are in.
fn into_measures(mut stream: Vec<Placed>) -> (Vec<Gathered>, Vec<Placed>) {
    sort(&mut stream);
    let mut measures: Vec<Gathered> = Vec::new();
    let mut loose: Vec<Placed> = Vec::new();
    let mut current = Gathered {
        measure: Measure::default(),
        offset: 0.0,
        things: Vec::new(),
    };
    let mut number = 0;
    let mut has_one = false;
    for thing in stream {
        match thing.thing {
            Thing::Measure(measure) => {
                if number != 0 || !current.things.is_empty() {
                    measures.push(current);
                }
                number = measure.number;
                has_one |= number == 1;
                current = Gathered {
                    measure,
                    offset: thing.offset,
                    things: Vec::new(),
                };
            }
            _ => {
                if number != 0 || thing.length() != 0.0 {
                    let offset = thing.offset - current.offset;
                    current.things.push(Placed { offset, ..thing });
                } else {
                    loose.push(thing);
                }
            }
        }
    }
    if !current.things.is_empty() {
        measures.push(current);
    }
    if let Some(first) = measures.first_mut() {
        if !has_one {
            first.measure.number = 1;
        }
        // What stands at the very start outside any measure goes into the
        // first, but for the instructions that name nothing.
        let (moved, kept): (Vec<Placed>, Vec<Placed>) = loose
            .into_iter()
            .partition(|thing| thing.offset == 0.0 && matches!(thing.thing, Thing::Element(_)));
        first.things.extend(moved);
        loose = kept;
    }
    (measures, loose)
}

/// music21's `TupletFixer`: a run of tuplet notes that does not fill its
/// bracket is written in the longest or the shortest value among them.
fn fix_tuplets(things: &mut [Placed]) {
    let tuplets = |thing: &Placed| match &thing.thing {
        Thing::Element(
            element @ (StreamElement::Note(_) | StreamElement::Chord(_) | StreamElement::Rest(_)),
        ) => Some(
            element
                .duration()
                .map(Duration::tuplets)
                .unwrap_or_default(),
        ),
        _ => None,
    };
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    for (index, thing) in things.iter().enumerate() {
        let Some(held) = tuplets(thing) else {
            continue;
        };
        let Some(first) = held.first() else {
            if !current.is_empty() {
                groups.push(std::mem::take(&mut current));
            }
            continue;
        };
        current.push(index);
        if first.tuplet_type() == Some(TupletType::Stop) {
            groups.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        groups.push(current);
    }
    let ordinal = |kind: DurationType| DurationType::ALL.iter().position(|held| *held == kind);
    for group in groups {
        let Some(first) = tuplets(&things[group[0]]).and_then(|held| held.into_iter().next())
        else {
            continue;
        };
        let total = op_frac(first.total_tuplet_length());
        let mut length = 0.0;
        let mut smallest: Option<usize> = None;
        let mut largest: Option<usize> = None;
        for index in &group {
            length = op_frac(length + things[*index].length());
            let Some(kind) = tuplets(&things[*index])
                .and_then(|held| held.into_iter().next())
                .and_then(|tuplet| ordinal(tuplet.duration_type()))
            else {
                continue;
            };
            smallest = Some(smallest.map_or(kind, |held| held.max(kind)));
            largest = Some(largest.map_or(kind, |held| held.min(kind)));
        }
        if length == total || total == 0.0 {
            continue;
        }
        let excess = op_frac(length / total);
        let inverse = op_frac(1.0 / excess);
        let chosen = if excess.fract() == 0.0 {
            largest
        } else if inverse.fract() == 0.0 {
            smallest
        } else {
            None
        };
        let Some(kind) = chosen.map(|index| DurationType::ALL[index]) else {
            continue;
        };
        for index in group {
            if let Thing::Element(element) = &mut things[index].thing {
                with_duration(element, |duration| {
                    let mut held = duration.tuplets();
                    if let Some(first) = held.first_mut() {
                        let normal = first.duration_normal().map_or(0, |(_, dots)| dots);
                        let actual = first.duration_actual().map_or(0, |(_, dots)| dots);
                        first.set_duration_normal(Some((kind, normal)));
                        first.set_duration_actual(Some((kind, actual)));
                    }
                    duration.set_tuplets(held);
                });
            }
        }
    }
}

/// music21's `makeVoices`: the things of a split spine, in the voices they
/// came from.
fn measure_stream(gathered: Gathered) -> Stream {
    let mut things = gathered.things;
    sort(&mut things);
    fix_tuplets(&mut things);
    let lowest = things
        .iter()
        .find(|thing| thing.voice == Some(1))
        .map(|thing| thing.offset);
    let mut voices: BTreeMap<usize, Vec<Placed>> = BTreeMap::new();
    let mut own: Vec<Placed> = Vec::new();
    for thing in things {
        match (lowest, thing.voice) {
            (Some(lowest), Some(voice)) => voices.entry(voice).or_default().push(Placed {
                offset: thing.offset - lowest,
                ..thing
            }),
            _ => own.push(thing),
        }
    }
    let events = |things: Vec<Placed>| -> Vec<(FloatType, usize, i32, StreamElement)> {
        things
            .into_iter()
            .filter_map(|thing| match thing.thing {
                Thing::Element(element) => Some((
                    thing.offset,
                    thing.priority,
                    element.class_sort_order(),
                    element,
                )),
                _ => None,
            })
            .collect()
    };
    let mut held = events(own);
    for (_, things) in voices {
        let voice = Stream::with_kind(StreamKind::Voice).with_events(
            events(things)
                .into_iter()
                .map(|(offset, _, _, element)| StreamEvent::new(offset, element))
                .collect(),
        );
        held.push((lowest.unwrap_or(0.0), 0, 5, voice.into()));
    }
    held.sort_by(|left, right| {
        left.0
            .total_cmp(&right.0)
            .then(left.1.cmp(&right.1))
            .then(left.2.cmp(&right.2))
    });
    let mut measure = Stream::with_kind(StreamKind::Measure).with_events(
        held.into_iter()
            .map(|(offset, _, _, element)| StreamEvent::new(offset, element))
            .collect(),
    );
    measure.set_number(gathered.measure.number);
    measure.set_number_suffix(gathered.measure.suffix);
    measure.set_left_barline(gathered.measure.left);
    measure.set_right_barline(gathered.measure.right);
    measure
}

/// The staff a spine says it belongs to, or the staves.
fn staves_of(things: &[Placed]) -> Vec<IntegerType> {
    things
        .iter()
        .find_map(|thing| match &thing.thing {
            Thing::Tandem(tandem) => tandem.strip_prefix("*staff"),
            _ => None,
        })
        .map(|staves| {
            staves
                .split('/')
                .filter_map(|staff| staff.parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// music21's `GlobalReference.updateMetadata`: a `!!!` line as a fact about
/// the piece.
fn reference(metadata: &mut Metadata, line: &str) -> Result<()> {
    let text = line.trim_start_matches('!');
    let (code, value) = text
        .split_once(':')
        .ok_or_else(|| humdrum_error(format!("a reference line with no code: {line}")))?;
    let code = code.trim().replace("@@", "@");
    let code = code.split('@').next().unwrap_or("");
    let value = value.trim();
    match REFERENCE_NAMES.iter().find(|(held, _)| *held == code) {
        Some((_, "")) => metadata.add_text(format!("humdrum:{code}"), value),
        Some((_, name)) if is_contributor_unique_name(name) => {
            metadata.add_contributor(*name, value);
        }
        Some((_, name)) => metadata.add_text(*name, value),
        None => metadata.add_text(code, value),
    }
    Ok(())
}

/// music21's `parseNonOpus`: one table as a score.
fn score_of(lines: &[&str]) -> Result<Stream> {
    let mut metadata = Metadata::new();
    let mut rows: Vec<(usize, Vec<String>)> = Vec::new();
    let mut width = 0;
    for (index, line) in lines.iter().enumerate() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("!!!") {
            reference(&mut metadata, line)?;
        } else if line.starts_with("!!") {
            continue;
        } else {
            let cells: Vec<String> = line
                .split('\t')
                .enumerate()
                .filter(|(place, cell)| *place == 0 || !cell.is_empty())
                .map(|(_, cell)| cell.to_string())
                .collect();
            width = width.max(cells.len());
            rows.push((index + 1, cells));
        }
    }
    let mut spines = spines_of(&rows, width)?;
    for index in 0..spines.len() {
        spines[index].kind = kind_of(&spines, index)?;
    }
    for spine in &mut spines {
        spine.stream = match spine.kind.as_str() {
            "kern" => parse_kern(&spine.events),
            "dynam" => parse_other(&spine.events, true),
            _ => parse_other(&spine.events, false),
        };
    }
    for index in 0..spines.len() {
        if spines[index].parent.is_none() {
            insert_voices(&mut spines, index);
        }
    }

    // The top spines as measures, with what stands outside them.
    let mut parts: Vec<(usize, Vec<Gathered>, Vec<Placed>)> = Vec::new();
    for (index, spine) in spines.iter().enumerate() {
        if spine.parent.is_none() && spine.kind == "kern" {
            let (measures, loose) = into_measures(spine.stream.clone());
            parts.push((index, measures, loose));
        }
    }
    // Which part each staff is: the first `*staff` a spine leaves standing
    // outside its measures.
    let staff_of = |loose: &[Placed]| -> Option<IntegerType> {
        loose.iter().find_map(|thing| match &thing.thing {
            Thing::Tandem(tandem) => tandem
                .strip_prefix("*staff")
                .and_then(|staff| staff.parse().ok()),
            _ => None,
        })
    };
    for spine in spines
        .iter()
        .filter(|spine| spine.parent.is_none() && spine.kind != "kern")
    {
        let staves = staves_of(&spine.stream);
        let is_words = matches!(spine.kind.as_str(), "lyrics" | "text");
        if spine.kind != "dynam" && !is_words {
            continue;
        }
        for staff in staves {
            let Some((_, measures, _)) = parts
                .iter_mut()
                .find(|(_, _, loose)| staff_of(loose) == Some(staff))
            else {
                if spine.kind == "dynam" {
                    return Err(humdrum_error(format!("no staff {staff} for the dynamics")));
                }
                continue;
            };
            for thing in &spine.stream {
                for measure in measures.iter_mut() {
                    let found = measure
                        .things
                        .iter()
                        .position(|held| held.priority == thing.line);
                    match (&thing.thing, found) {
                        (Thing::Element(dynamic @ StreamElement::Dynamic(_)), Some(found)) => {
                            let offset = measure.things[found].offset;
                            measure.things.push(Placed {
                                offset,
                                line: thing.line,
                                priority: 0,
                                voice: None,
                                thing: Thing::Element(dynamic.clone()),
                            });
                            break;
                        }
                        (Thing::Word(word), Some(_)) => {
                            for held in &mut measure.things {
                                if held.priority != thing.line {
                                    continue;
                                }
                                match &mut held.thing {
                                    Thing::Element(StreamElement::Note(note)) => {
                                        note.set_lyric(Some(word))?;
                                    }
                                    Thing::Element(StreamElement::Rest(rest)) => {
                                        rest.set_lyric(Some(word));
                                    }
                                    Thing::Element(StreamElement::Chord(chord)) => {
                                        if let Some(singer) = chord.notes_mut().first_mut() {
                                            singer.set_lyric(Some(word))?;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // The parts come out in the order opposite to the file's, which is
    // written lowest part first.
    let mut events = Vec::new();
    for (_, measures, _) in parts.into_iter().rev() {
        let mut held = Vec::new();
        let mut offset = 0.0;
        for gathered in measures {
            let measure = measure_stream(gathered);
            let length = measure.end_offset();
            held.push(StreamEvent::new(offset, measure));
            offset = op_frac(offset + length);
        }
        events.push(StreamEvent::new(
            0.0,
            Stream::with_kind(StreamKind::Part).with_events(held),
        ));
    }
    let mut score = Stream::with_kind(StreamKind::Score).with_events(events);
    score.set_metadata(Some(metadata));
    Ok(score)
}

/// Reads a Humdrum file into a score: music21's `converter.parse` of one.
///
/// Every `**kern` spine is a part, the last in the file first; its barlines
/// are the measures, its instructions the clefs, keys, meters, tempos and
/// instruments, and a spine split in two is two voices in the measures where
/// it is split. `**dynam` and `**text` spines put their dynamics and words
/// on the notes of the staff they name. A cell that cannot be read is passed
/// over, as music21 passes over it.
///
/// A file holding several tables, each closed before the next opens, comes
/// back as an opus of scores. `**harm` spines, and a voice that splits
/// again, are not read.
///
/// ```
/// use music21_rs::humdrum::from_humdrum;
///
/// let score = from_humdrum("**kern\n*clefG2\n*M2/4\n=1\n4c\n4d\n=2\n2e\n==\n*-\n")?;
/// let part = score.parts()[0];
/// assert_eq!(part.measures().len(), 2);
/// assert_eq!(part.measures()[1].pitches()[0].name_with_octave(), "E4");
/// # Ok::<(), music21_rs::Error>(())
/// ```
pub fn from_humdrum(text: &str) -> Result<Stream> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return Err(humdrum_error("Need a list of lines to parse!"));
    }
    // music21's `determineIfDataStreamIsOpus`: a line of nothing but `**`
    // headings opens a table, one of nothing but `*-` closes it.
    let all = |line: &str, test: fn(&str) -> bool| {
        let line = line.trim_end();
        !line.is_empty() && line.split('\t').all(test)
    };
    let heading: fn(&str) -> bool = |cell| {
        cell.strip_prefix("**").is_some_and(|name| {
            !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_')
        })
    };
    let starts: Vec<usize> = (0..lines.len())
        .filter(|index| all(lines[*index], heading))
        .collect();
    let mut ends: Vec<usize> = (0..lines.len())
        .filter(|index| all(lines[*index], |cell| cell == "*-"))
        .collect();
    if starts.len() < 2 {
        return score_of(&lines);
    }
    if ends.last().copied().unwrap_or(0) <= starts[starts.len() - 1] {
        ends.push(lines.len());
    }
    let mut pieces: Vec<&[&str]> = Vec::new();
    for (index, end) in ends.iter().enumerate() {
        let start = if index == 0 { 0 } else { ends[index - 1] + 1 };
        let stop = if index + 1 == ends.len() {
            lines.len()
        } else {
            (*end + 1).min(lines.len())
        };
        pieces.push(&lines[start.min(stop)..stop]);
    }
    if pieces.len() < 2 {
        return Err(humdrum_error(
            "Malformed Humdrum data: possibly multiple **tags without closing information.",
        ));
    }
    let mut events = Vec::new();
    let mut offset = 0.0;
    for (index, piece) in pieces.iter().enumerate() {
        let mut score = score_of(piece)?;
        let mut metadata = score.metadata().cloned().unwrap_or_default();
        metadata.set(
            "number",
            vec![crate::metadata::MetadataValue::new((index + 1).to_string())],
        );
        score.set_metadata(Some(metadata));
        let length = score.end_offset();
        events.push(StreamEvent::new(offset, score));
        offset = op_frac(offset + length);
    }
    Ok(Stream::with_kind(StreamKind::Opus).with_events(events))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_length_is_a_number_and_its_dots() {
        assert_eq!(duration_of("4c").unwrap().quarter_length(), 1.0);
        assert_eq!(duration_of("8.d").unwrap().quarter_length(), 0.75);
        assert_eq!(duration_of("0r").unwrap().quarter_length(), 8.0);
        // Twelfths are eighths, three in the time of two.
        let triplet = duration_of("12e").unwrap();
        assert!((triplet.quarter_length() - 1.0 / 3.0).abs() < 1e-9);
        assert_eq!(triplet.tuplets()[0].actual(), 3);
        assert!(duration_of("qf").unwrap().is_grace());
        assert!(duration_of("c").is_err());
    }

    #[test]
    fn letters_say_the_octave() {
        let name = |token: &str| match note_of(token, None).unwrap() {
            StreamElement::Note(note) => note.pitch().name_with_octave(),
            _ => "rest".to_string(),
        };
        assert_eq!(name("4c"), "C4");
        assert_eq!(name("4cc#"), "C#5");
        assert_eq!(name("4C"), "C3");
        assert_eq!(name("4BB-"), "Bb2");
        assert_eq!(name("4r"), "rest");
    }

    #[test]
    fn a_barline_says_how_the_measure_before_ends() {
        let (measure, right) = measure_of("=12a:|!", true);
        assert_eq!(measure.number, 12);
        assert_eq!(measure.suffix.as_deref(), Some("a"));
        assert_eq!(right.repeat_direction(), Some(RepeatDirection::End));
        let (_, double) = measure_of("==", true);
        assert_eq!(double.bar_type(), BarlineType::Double);
    }

    #[test]
    fn spines_are_parts_and_the_last_comes_first() {
        let score = from_humdrum(
            "**kern\t**kern\n*clefF4\t*clefG2\n*M2/4\t*M2/4\n=1\t=1\n2C\t4c\n.\t4d\n=2\t=2\n\
             2D\t2e\n==\t==\n*-\t*-\n",
        )
        .unwrap();
        let parts = score.parts();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].measures()[0].pitches().len(), 2);
        assert_eq!(parts[1].measures()[0].pitches()[0].name_with_octave(), "C3");
    }

    #[test]
    fn a_split_spine_is_two_voices() {
        let score = from_humdrum("**kern\n*M2/4\n=1\n*^\n4c\t4e\n4d\t4f\n*v\t*v\n=2\n2g\n==\n*-\n")
            .unwrap();
        let part = score.parts()[0];
        let measures = part.measures();
        assert_eq!(measures[0].voices().len(), 2);
        assert!(measures[1].voices().is_empty());
    }
}
