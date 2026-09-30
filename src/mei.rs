//! MEI: music21's `mei.base`, which reads a score encoded by the Music
//! Encoding Initiative's schema.
//!
//! An MEI score is measures, each holding a `<staff>` for every part and in
//! it a `<layer>` for every voice; `<scoreDef>` and `<staffDef>` say the
//! meter, key, clef and instrument. Slurs, ties, beams and tuplets may be
//! written on the notes or apart from them, naming the notes they join by
//! their `xml:id`.

use std::collections::HashMap;

use crate::articulations::{Articulation, ArticulationKind};
use crate::bar::{Barline, BarlineType, RepeatDirection};
use crate::chord::Chord;
use crate::clef::{Clef, ClefKind};
use crate::defaults::{FloatType, IntegerType};
use crate::duration::{Duration, DurationType, Tuplet, TupletType};
use crate::error::{Error, Result};
use crate::expressions::{Expression, Ornament, OrnamentKind};
use crate::instrument::{Instrument, SearchLanguage};
use crate::interval::Interval;
use crate::key::{Key, KeySignature};
use crate::makenotation::op_frac;
use crate::metadata::Metadata;
use crate::meter::TimeSignature;
use crate::notation::{BeamType, Lyric, Syllabic, Tie, TieType};
use crate::note::Note;
use crate::pitch::{Accidental, Pitch, PitchOptions};
use crate::rest::Rest;
use crate::spanner::{Spanner, SpannerKind};
use crate::stream::{Stream, StreamElement, StreamEvent, StreamKind};
use crate::xml::Xml;

fn mei_error(message: impl Into<String>) -> Error {
    Error::Mei(message.into())
}

/// A tag with any namespace prefix taken off.
fn local(tag: &str) -> &str {
    tag.rsplit(':').next().unwrap_or(tag)
}

fn children<'a>(element: &'a Xml, tag: &'a str) -> impl Iterator<Item = &'a Xml> {
    element
        .children
        .iter()
        .filter(move |child| local(&child.tag) == tag)
}

fn child<'a>(element: &'a Xml, tag: &str) -> Option<&'a Xml> {
    element
        .children
        .iter()
        .find(|child| local(&child.tag) == tag)
}

/// Every element under this one of a tag, in document order.
fn descendants<'a>(element: &'a Xml, tag: &str, out: &mut Vec<&'a Xml>) {
    for held in &element.children {
        if local(&held.tag) == tag {
            out.push(held);
        }
        descendants(held, tag, out);
    }
}

fn without_hash(id: &str) -> &str {
    id.strip_prefix('#').unwrap_or(id)
}

/// A tuplet a `<tupletSpan>` runs from one note to another, waiting for
/// the layer to say which notes lie between.
#[derive(Clone, Debug)]
struct Search {
    end: &'static str,
    number: u32,
    base: u32,
}

/// A thing a layer holds: an element, and what is known of it that the
/// element itself cannot say.
#[derive(Clone, Debug)]
struct Thing {
    element: StreamElement,
    /// The number slurs know it by.
    uid: usize,
    search: Option<Search>,
    /// A measure rest with no length of its own.
    whole_measure: bool,
}

/// What a `<staffDef>` says: music21's dictionary of instrument, meter, key
/// and clef, in that order.
#[derive(Clone, Debug, Default)]
struct StaffDef {
    instrument: Option<Instrument>,
    meter: Option<TimeSignature>,
    key: Option<Key>,
    clef: Option<Clef>,
}

impl StaffDef {
    fn elements(&self) -> Vec<StreamElement> {
        let mut out: Vec<StreamElement> = Vec::new();
        if let Some(instrument) = &self.instrument {
            out.push(instrument.clone().into());
        }
        if let Some(meter) = &self.meter {
            out.push(meter.clone().into());
        }
        if let Some(key) = &self.key {
            out.push(StreamElement::Key(key.clone()));
        }
        if let Some(clef) = &self.clef {
            out.push(clef.clone().into());
        }
        out
    }
}

/// A measure of one staff while it is put together.
#[derive(Clone, Debug)]
struct Measure {
    number: IntegerType,
    /// Each voice with its name and what it holds.
    voices: Vec<(String, Vec<Thing>)>,
    /// What stands at the start outside any voice, in the order inserted.
    own: Vec<StreamElement>,
    left: Option<Barline>,
    right: Option<Barline>,
    /// The length a measure rest fixed it at.
    length: Option<FloatType>,
}

/// What a part holds in order: an instrument before its measures, then the
/// measures.
#[derive(Clone, Debug)]
enum Held {
    Instrument(Box<Instrument>),
    Measure(Box<Measure>),
}

/// music21's `MeiToM21Converter`: the document, and what the elements that
/// stand apart from the notes say of each note by its id.
struct Reader {
    /// For each `xml:id`, the attributes to be put on the element.
    extra: HashMap<String, Vec<(&'static str, String)>>,
    /// The slurs, each with the name it is known by and the notes it joins.
    slurs: Vec<(String, Vec<usize>)>,
    next_uid: usize,
    next_slur: usize,
}

const DURATIONS: [(&str, FloatType); 14] = [
    ("long", 16.0),
    ("breve", 8.0),
    ("1", 4.0),
    ("2", 2.0),
    ("4", 1.0),
    ("8", 0.5),
    ("16", 0.25),
    ("32", 0.125),
    ("64", 0.0625),
    ("128", 0.03125),
    ("256", 0.015625),
    ("512", 0.0078125),
    ("1024", 0.00390625),
    ("2048", 0.001953125),
];

/// The length a `<mRest>` with no `dur` is given until its measure says
/// how long it is.
const NO_DURATION: FloatType = 0.00390625;

fn accidental_of(attribute: Option<&str>) -> Result<Option<&'static str>> {
    let Some(attribute) = attribute else {
        return Ok(None);
    };
    Ok(Some(match attribute {
        "s" | "ns" => "#",
        "f" | "nf" => "-",
        "ss" | "x" => "##",
        "ff" => "--",
        "xs" | "ts" => "###",
        "tf" => "---",
        "n" => "n",
        "su" => "#~",
        "sd" | "nu" => "~",
        "fu" | "nd" => "`",
        "fd" => "-`",
        other => {
            return Err(mei_error(format!(
                "Unexpected value for \"accid\" attribute: {other}"
            )));
        }
    }))
}

fn gestural_accidental_of(attribute: &str) -> Result<&'static str> {
    Ok(match attribute {
        "s" => "#",
        "f" => "-",
        "ss" => "##",
        "ff" => "--",
        "n" => "n",
        "su" => "#~",
        "sd" => "~",
        "fu" => "`",
        "fd" => "-`",
        other => {
            return Err(mei_error(format!(
                "Unexpected value for \"accid.ges\" attribute: {other}"
            )));
        }
    })
}

fn length_of(attribute: Option<&str>) -> Result<FloatType> {
    let Some(attribute) = attribute else {
        return Ok(NO_DURATION);
    };
    DURATIONS
        .iter()
        .find(|(name, _)| *name == attribute)
        .map(|(_, length)| *length)
        .ok_or_else(|| {
            mei_error(format!(
                "Unexpected value for \"dur\" attribute: {attribute}"
            ))
        })
}

/// music21's `makeDuration`: a length, then its dots.
fn duration_of(base: FloatType, dots: u32) -> Result<Duration> {
    let plain = Duration::new(base)?;
    Ok(match plain.type_and_dots() {
        Some((kind, _)) => Duration::from_type_with_dots(kind, dots),
        None => plain,
    })
}

fn dots_of(element: &Xml) -> Result<u32> {
    match element.get("dots") {
        None => Ok(0),
        Some(dots) => dots
            .parse()
            .map_err(|_| mei_error(format!("the dots {dots:?} are no number"))),
    }
}

/// music21's `_makeArticList`: the marks an `artic` attribute names.
fn articulations_of(attribute: &str) -> Result<Vec<Articulation>> {
    let mut marks = Vec::new();
    for name in attribute.split(' ') {
        let classes: &[&str] = match name {
            "marc-stacc" => &["StrongAccent", "Staccato"],
            "ten-stacc" => &["Tenuto", "Staccato"],
            "acc" => &["Accent"],
            "stacc" => &["Staccato"],
            "ten" => &["Tenuto"],
            "stacciss" => &["Staccatissimo"],
            "marc" => &["StrongAccent"],
            "spicc" => &["Spiccato"],
            "doit" => &["Doit"],
            "plop" => &["Plop"],
            "fall" => &["Falloff"],
            "dnbow" => &["DownBow"],
            "upbow" => &["UpBow"],
            "harm" => &["Harmonic"],
            "snap" => &["SnapPizzicato"],
            "stop" => &["Stopped"],
            "open" => &["OpenString"],
            "dbltongue" => &["DoubleTongue"],
            "toe" => &["OrganToe"],
            "trpltongue" => &["TripleTongue"],
            "heel" => &["OrganHeel"],
            "tap" | "lhpizz" | "dot" | "stroke" | "rip" | "bend" | "flip" | "smear"
            | "fingernail" | "damp" | "dampall" => &["Articulation"],
            other => {
                return Err(mei_error(format!(
                    "Unexpected value for \"artic\" attribute: {other}"
                )));
            }
        };
        for class in classes {
            let kind = ArticulationKind::from_class_name(class)
                .ok_or_else(|| mei_error(format!("no articulation called {class}")))?;
            marks.push(Articulation::of_kind(kind));
        }
    }
    Ok(marks)
}

/// music21's `_barlineFromAttr`: a barline, or the two a repeat both ways
/// is, the one closing a measure and the one opening the next.
fn barline_of(attribute: &str) -> Result<(Barline, Option<Barline>)> {
    Ok(match attribute {
        "rptboth" => (
            Barline::repeat(RepeatDirection::End, Some(2)),
            Some(Barline::repeat(RepeatDirection::Start, None)),
        ),
        "rptend" => (Barline::repeat(RepeatDirection::End, Some(2)), None),
        other if other.starts_with("rpt") => (Barline::repeat(RepeatDirection::Start, None), None),
        other => {
            let kind = match other {
                "dashed" => BarlineType::Dashed,
                "dotted" => BarlineType::Dotted,
                "dbl" => BarlineType::Double,
                "end" => BarlineType::Final,
                "invis" => BarlineType::None,
                "single" => BarlineType::Regular,
                other => {
                    return Err(mei_error(format!(
                        "Unexpected value for \"right\" attribute: {other}"
                    )));
                }
            };
            (Barline::new(kind), None)
        }
    })
}

fn tie_of(attribute: &str) -> Tie {
    let has = |mark: char| attribute.contains(mark);
    Tie::new(if has('m') || (has('t') && has('i')) {
        TieType::Continue
    } else if has('i') {
        TieType::Start
    } else {
        TieType::Stop
    })
}

fn meter_of(element: &Xml) -> Result<TimeSignature> {
    TimeSignature::from_ratio_string(&format!(
        "{}/{}",
        element.get("meter.count").unwrap_or("None"),
        element.get("meter.unit").unwrap_or("None")
    ))
}

/// music21's `_keySigFromAttrs`: a key named by its tonic, or by how many
/// sharps or flats it is written with.
fn key_of(element: &Xml) -> Result<Key> {
    if let Some(step) = element.get("key.pname") {
        let tonic = format!(
            "{step}{}",
            accidental_of(element.get("key.accid"))?.unwrap_or("")
        );
        return Key::from_tonic_mode(
            &tonic,
            element.get("key.mode").filter(|mode| !mode.is_empty()),
        );
    }
    let signature = element.get("key.sig").unwrap_or("0");
    let count = || -> Result<IntegerType> {
        signature[..1]
            .parse()
            .map_err(|_| mei_error(format!("the key signature {signature:?} has no number")))
    };
    let sharps = if signature.starts_with('0') {
        0
    } else if signature.ends_with('s') {
        count()?
    } else {
        -count()?
    };
    Ok(KeySignature::new(sharps).as_key(element.get("key.mode").unwrap_or("major")))
}

/// music21's `_transpositionFromAttrs`: the interval an instrument sounds
/// away from where it is written.
fn transposition_of(element: &Xml) -> Result<Interval> {
    let read = |name: &str| -> Result<IntegerType> {
        match element.get(name) {
            None => Ok(0),
            Some(text) => text
                .parse()
                .map_err(|_| mei_error(format!("{name} is no number: {text:?}"))),
        }
    };
    let mut diatonic = read("trans.diat")?;
    let semitones = read("trans.semi")?;
    // A transposition of more than an octave written with its steps inside
    // one has the octaves put back.
    if (semitones - diatonic).abs() > 5 * (semitones.abs() / 12 + 1) {
        if semitones < 0 {
            diatonic -= 7 * (semitones.abs() / 12);
        } else if semitones > 0 {
            diatonic += 7 * (semitones.abs() / 12);
        }
    }
    diatonic += diatonic.signum();
    Interval::from_generic_and_chromatic(diatonic, semitones)
}

fn clef_of(
    shape: Option<&str>,
    line: Option<&str>,
    displaced: Option<&str>,
    place: Option<&str>,
) -> Result<Clef> {
    match shape {
        Some("perc") => Ok(Clef::of_kind(ClefKind::PercussionClef)),
        Some("TAB") => Ok(Clef::of_kind(ClefKind::TabClef)),
        _ => {
            let octaves = match displaced {
                None => 0,
                Some("8") => 1,
                Some("15") => 2,
                Some("22") => 3,
                Some(other) => {
                    return Err(mei_error(format!("a clef displaced by {other}")));
                }
            };
            let shift = if place == Some("below") {
                -octaves
            } else {
                octaves
            };
            let (Some(shape), Some(line)) = (shape, line) else {
                return Err(mei_error("a clef with no shape or no line"));
            };
            Clef::from_string(&format!("{shape}{line}"), shift)
        }
    }
}

fn clef_element(element: &Xml) -> Result<Clef> {
    clef_of(
        element.get("shape"),
        element.get("line"),
        element.get("dis"),
        element.get("dis.place"),
    )
}

/// Whether a length is written with a flag, and so may be beamed.
fn is_beamable(duration: &Duration) -> Option<DurationType> {
    match duration.components().as_slice() {
        [(kind, _)] if kind.quarter_length() < 1.0 => Some(*kind),
        _ => None,
    }
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

fn is_sounding(element: &StreamElement) -> bool {
    matches!(
        element,
        StreamElement::Note(_) | StreamElement::Chord(_) | StreamElement::Rest(_)
    )
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

/// music21's `scaleToTuplet` for a tuplet whose numbers are known: so many
/// notes in the time of so many, of the value the note is written in.
fn scale_to_tuplet(element: &mut StreamElement, number: u32, base: u32) {
    if !is_sounding(element) {
        return;
    }
    with_duration(element, |duration| {
        let kind = match duration.components().as_slice() {
            [(kind, _)] => *kind,
            _ => DurationType::Quarter,
        };
        duration.append_tuplet(Tuplet::new(number, base, kind, 0));
    });
}

/// music21's `beamTogether`: the first thing that can carry a beam starts
/// it, those after carry it on, and the last ends it.
fn beam_together(things: &mut [Thing]) -> Result<()> {
    let mut last: Option<usize> = None;
    for (index, thing) in things.iter_mut().enumerate() {
        let (beams, duration) = match &thing.element {
            StreamElement::Note(note) => (note.beams().clone(), note.duration().cloned()),
            StreamElement::Chord(chord) => (chord.beams().clone(), chord.duration().cloned()),
            _ => continue,
        };
        let Some(kind) = duration.as_ref().and_then(is_beamable) else {
            continue;
        };
        if !beams.is_empty() {
            continue;
        }
        let mut filled = beams;
        filled.fill(
            kind,
            Some(if last.is_none() {
                BeamType::Start
            } else {
                BeamType::Continue
            }),
        )?;
        match &mut thing.element {
            StreamElement::Note(note) => note.set_beams(filled),
            StreamElement::Chord(chord) => chord.set_beams(filled),
            _ => {}
        }
        last = Some(index);
    }
    // Where nothing was beamed music21 reaches for the last thing of all.
    let closing = last.or(things.len().checked_sub(1));
    if let Some(thing) = closing.and_then(|index| things.get_mut(index)) {
        match &mut thing.element {
            StreamElement::Note(note) => {
                let mut beams = note.beams().clone();
                beams.set_all(BeamType::Stop, None);
                note.set_beams(beams);
            }
            StreamElement::Chord(chord) => {
                let mut beams = chord.beams().clone();
                beams.set_all(BeamType::Stop, None);
                chord.set_beams(beams);
            }
            _ => {}
        }
    }
    Ok(())
}

impl Reader {
    /// An attribute as music21 reads it once the spanning elements have
    /// written theirs onto the notes: what the element says, then what was
    /// said of it.
    fn attribute(&self, element: &Xml, name: &str) -> Option<String> {
        let added = element
            .get("xml:id")
            .and_then(|id| self.extra.get(id))
            .and_then(|extra| {
                extra
                    .iter()
                    .find(|(held, _)| *held == name)
                    .map(|(_, value)| value.clone())
            });
        match (element.get(name), added) {
            (Some(own), Some(added)) => Some(format!("{own}{added}")),
            (Some(own), None) => Some(own.to_string()),
            (None, added) => added,
        }
    }

    fn set(&mut self, id: &str, name: &'static str, value: String) {
        let extra = self.extra.entry(without_hash(id).to_string()).or_default();
        match extra.iter_mut().find(|(held, _)| *held == name) {
            Some((_, held)) => *held = value,
            None => extra.push((name, value)),
        }
    }

    /// music21's `_ppSlurs`, `_ppTies`, `_ppBeams` and `_ppTuplets`: what
    /// the spanning elements of the score say, written against the ids of
    /// the notes they name.
    fn preprocess(&mut self, score: &Xml) {
        let mut found = Vec::new();
        descendants(score, "slur", &mut found);
        for slur in std::mem::take(&mut found) {
            if let (Some(start), Some(end)) = (slur.get("startid"), slur.get("endid")) {
                let name = format!("slur-{}", self.next_slur);
                self.next_slur += 1;
                self.slurs.push((name.clone(), Vec::new()));
                self.set(start, "m21SlurStart", name.clone());
                self.set(end, "m21SlurEnd", name);
            }
        }
        descendants(score, "tie", &mut found);
        for tie in std::mem::take(&mut found) {
            if let (Some(start), Some(end)) = (tie.get("startid"), tie.get("endid")) {
                self.set(start, "tie", "i".to_string());
                self.set(end, "tie", "t".to_string());
            }
        }
        descendants(score, "beamSpan", &mut found);
        for beam in std::mem::take(&mut found) {
            let (Some(start), Some(end)) = (beam.get("startid"), beam.get("endid")) else {
                continue;
            };
            self.set(start, "m21Beam", "start".to_string());
            self.set(end, "m21Beam", "stop".to_string());
            for id in beam.get("plist").unwrap_or("").split(' ') {
                let id = without_hash(id);
                let has = self
                    .extra
                    .get(id)
                    .is_some_and(|extra| extra.iter().any(|(name, _)| *name == "m21Beam"));
                if !has {
                    self.set(id, "m21Beam", "continue".to_string());
                }
            }
        }
        descendants(score, "tupletSpan", &mut found);
        for tuplet in found {
            let number = tuplet.get("num").unwrap_or("None").to_string();
            let base = tuplet.get("numbase").unwrap_or("None").to_string();
            if let Some(list) = tuplet.get("plist") {
                for id in list.split(' ') {
                    if !without_hash(id).is_empty() {
                        self.set(id, "m21TupletNum", number.clone());
                        self.set(id, "m21TupletNumbase", base.clone());
                    }
                }
            } else if let (Some(start), Some(end)) = (tuplet.get("startid"), tuplet.get("endid")) {
                for (id, which) in [(start, "start"), (end, "end")] {
                    self.set(id, "m21TupletSearch", which.to_string());
                    self.set(id, "m21TupletNum", number.clone());
                    self.set(id, "m21TupletNumbase", base.clone());
                }
            }
        }
    }

    fn uid(&mut self) -> usize {
        self.next_uid += 1;
        self.next_uid - 1
    }

    /// music21's `addSlurs`: the slurs an element starts or ends.
    fn add_slurs(&mut self, element: &Xml, uid: usize) -> Result<()> {
        let join = |slurs: &mut Vec<(String, Vec<usize>)>, name: &str| {
            if let Some((_, held)) = slurs.iter_mut().find(|(held, _)| held == name) {
                held.push(uid);
            }
        };
        if let Some(name) = self.attribute(element, "m21SlurStart") {
            join(&mut self.slurs, &name);
        }
        if let Some(name) = self.attribute(element, "m21SlurEnd") {
            join(&mut self.slurs, &name);
        }
        if let Some(slurs) = element.get("slur") {
            for slur in slurs.split(' ') {
                let chars: Vec<char> = slur.chars().collect();
                let [number, kind] = chars.as_slice() else {
                    return Err(mei_error(format!("cannot read the slur {slur:?}")));
                };
                let name = number.to_string();
                match kind {
                    'i' => self.slurs.push((name, vec![uid])),
                    't' => join(&mut self.slurs, &name),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// What the tuplet numbers written against an element do to it:
    /// music21's `scaleToTuplet`.
    fn tuplet(&self, element: &Xml, thing: &mut Thing) -> Result<()> {
        let Some(number) = self.attribute(element, "m21TupletNum") else {
            return Ok(());
        };
        let base = self
            .attribute(element, "m21TupletNumbase")
            .unwrap_or_default();
        let read = |text: &str| -> Result<u32> {
            text.parse()
                .map_err(|_| mei_error(format!("a tuplet of {text:?} notes")))
        };
        let (number, base) = (read(&number)?, read(&base)?);
        match self.attribute(element, "m21TupletSearch").as_deref() {
            Some(end) => {
                thing.search = Some(Search {
                    end: if end.contains("start") {
                        "start"
                    } else {
                        "end"
                    },
                    number,
                    base,
                });
            }
            None => {
                scale_to_tuplet(&mut thing.element, number, base);
                if let Some(written) = element.get("tuplet") {
                    if written.starts_with('i') {
                        mark_first_tuplet(&mut thing.element, TupletType::Start);
                    } else if written.starts_with('t') {
                        mark_first_tuplet(&mut thing.element, TupletType::Stop);
                    }
                }
            }
        }
        Ok(())
    }

    /// music21's `noteFromElement`. The second answer is the number slurs
    /// know the note by.
    fn note(&mut self, element: &Xml) -> Result<(Note, usize)> {
        let name = element.get("pname").unwrap_or("");
        let mut pitch = if name.is_empty() {
            Pitch::from_name("C4")?
        } else {
            let step = name
                .chars()
                .next()
                .map(|c| c.to_ascii_uppercase())
                .ok_or_else(|| mei_error("a note with no name"))?;
            let mut options = PitchOptions::new().step(step);
            if let Some(octave) = element.get("oct").filter(|octave| !octave.is_empty()) {
                options = options.octave(
                    octave
                        .parse()
                        .map_err(|_| mei_error(format!("the octave {octave:?} is no number")))?,
                );
            }
            options.build()?
        };
        if let Some(accidental) = accidental_of(element.get("accid"))? {
            pitch.set_written_accidental(Some(Accidental::new(accidental)?));
        }
        let length = length_of(element.get("dur"))?;
        let mut note =
            Note::from_pitch(pitch).with_duration(duration_of(length, dots_of(element)?)?);

        let mut dot_elements = 0;
        for held in &element.children {
            match local(&held.tag) {
                "dot" => dot_elements += 1,
                "artic" => {
                    if let Some(marks) = held.get("artic") {
                        note.articulations_mut().extend(articulations_of(marks)?);
                    }
                }
                "accid" => {
                    let accidental = match held.get("accid.ges") {
                        Some(gestural) => Some(gestural_accidental_of(gestural)?),
                        None => accidental_of(held.get("accid"))?,
                    };
                    if let Some(accidental) = accidental {
                        let mut pitch = note.pitch().clone();
                        pitch.set_written_accidental(Some(Accidental::new(accidental)?));
                        note.set_pitch(pitch);
                    }
                }
                "syl" => *note.lyrics_mut() = vec![syllable_of(held)?],
                _ => {}
            }
        }
        if let Some(gestural) = element.get("accid.ges") {
            let mut pitch = note.pitch().clone();
            pitch.set_written_accidental(Some(Accidental::new(gestural_accidental_of(gestural)?)?));
            note.set_pitch(pitch);
        }
        let uid = self.uid();
        self.add_slurs(element, uid)?;
        if let Some(id) = element.get("xml:id") {
            note.set_id(Some(id.to_string()));
        }
        if let Some(marks) = element.get("artic") {
            note.articulations_mut().extend(articulations_of(marks)?);
        }
        if let Some(tie) = self.attribute(element, "tie") {
            note.set_tie(Some(tie_of(&tie)));
        }
        if dot_elements > 0 {
            note.set_duration(duration_of(length, dot_elements)?);
        }
        if element.get("grace").is_some() {
            let grace = note
                .duration()
                .map(Duration::grace_duration)
                .unwrap_or_default();
            note.set_duration(grace);
        }
        if let Some(beam) = self.attribute(element, "m21Beam")
            && let Some(kind) = note.duration().and_then(is_beamable)
        {
            let mut beams = note.beams().clone();
            beams.fill(kind, Some(beam_type_of(&beam)?))?;
            note.set_beams(beams);
        }
        let verses: Vec<&Xml> = children(element, "verse").collect();
        if !verses.is_empty() {
            let mut lyrics = Vec::new();
            for (index, verse) in verses.iter().enumerate() {
                let number = match verse.get("n") {
                    Some(number) => number.parse::<IntegerType>().ok(),
                    None => Some(index as IntegerType + 1),
                };
                for syllable in children(verse, "syl") {
                    let mut lyric = syllable_of(syllable)?;
                    if let Some(number) = number {
                        lyric.set_number(number);
                    }
                    lyrics.push(lyric);
                }
            }
            *note.lyrics_mut() = lyrics;
        }
        Ok((note, uid))
    }

    fn note_thing(&mut self, element: &Xml) -> Result<Thing> {
        let (note, uid) = self.note(element)?;
        let mut thing = Thing {
            element: note.into(),
            uid,
            search: None,
            whole_measure: false,
        };
        self.tuplet(element, &mut thing)?;
        Ok(thing)
    }

    /// music21's `chordFromElement`.
    fn chord(&mut self, element: &Xml) -> Result<Thing> {
        let mut notes = Vec::new();
        for held in children(element, "note") {
            // A note of a chord takes its own tuplet, as music21 gives it.
            let thing = self.note_thing(held)?;
            if let StreamElement::Note(note) = thing.element {
                notes.push(note);
            }
        }
        let mut chord = Chord::new(notes)?.with_duration(duration_of(
            length_of(element.get("dur"))?,
            dots_of(element)?,
        )?);
        for held in children(element, "artic") {
            if let Some(marks) = held.get("artic") {
                chord.articulations_mut().extend(articulations_of(marks)?);
            }
        }
        let uid = self.uid();
        self.add_slurs(element, uid)?;
        if let Some(marks) = element.get("artic") {
            chord.articulations_mut().extend(articulations_of(marks)?);
        }
        if let Some(tie) = self.attribute(element, "tie") {
            chord.set_tie(Some(tie_of(&tie)));
        }
        if element.get("grace").is_some() {
            let grace = chord
                .duration()
                .map(Duration::grace_duration)
                .unwrap_or_default();
            chord.set_duration(grace);
        }
        if let Some(beam) = self.attribute(element, "m21Beam")
            && let Some(kind) = chord.duration().and_then(is_beamable)
        {
            let mut beams = chord.beams().clone();
            beams.fill(kind, Some(beam_type_of(&beam)?))?;
            chord.set_beams(beams);
        }
        let mut thing = Thing {
            element: chord.into(),
            uid,
            search: None,
            whole_measure: false,
        };
        self.tuplet(element, &mut thing)?;
        Ok(thing)
    }

    /// music21's `restFromElement` and `spaceFromElement`: a rest, hidden
    /// where it is only space.
    fn rest(&mut self, element: &Xml, hidden: bool, whole_measure: bool) -> Result<Thing> {
        let mut rest = Rest::new(duration_of(
            length_of(element.get("dur"))?,
            dots_of(element)?,
        )?);
        rest.set_hidden(hidden);
        let mut thing = Thing {
            element: rest.into(),
            uid: self.uid(),
            search: None,
            whole_measure: whole_measure && element.get("dur").is_none(),
        };
        self.tuplet(element, &mut thing)?;
        Ok(thing)
    }

    /// What a layer, a beam or a tuplet holds, in order.
    fn contents(&mut self, element: &Xml, in_layer: bool) -> Result<Vec<Thing>> {
        let mut things = Vec::new();
        for held in &element.children {
            match local(&held.tag) {
                "clef" => things.push(Thing {
                    element: clef_element(held)?.into(),
                    uid: self.uid(),
                    search: None,
                    whole_measure: false,
                }),
                "chord" => things.push(self.chord(held)?),
                "note" => things.push(self.note_thing(held)?),
                "rest" => things.push(self.rest(held, false, false)?),
                "mRest" if in_layer => things.push(self.rest(held, false, true)?),
                "space" => things.push(self.rest(held, true, false)?),
                "mSpace" if in_layer => things.push(self.rest(held, true, true)?),
                "beam" => {
                    let mut beamed = self.contents(held, false)?;
                    beam_together(&mut beamed)?;
                    things.extend(beamed);
                }
                "tuplet" => things.extend(self.tuplet_element(held)?),
                "bTrem" => things.extend(self.tremolo(held)?),
                // A barline inside a layer is checked and then has nowhere
                // to stand.
                "barLine" => {
                    let _ = barline_of(held.get("rend").unwrap_or("single"))?;
                }
                _ => {}
            }
        }
        Ok(things)
    }

    /// music21's `tupletFromElement`.
    fn tuplet_element(&mut self, element: &Xml) -> Result<Vec<Thing>> {
        let (Some(number), Some(base)) = (element.get("num"), element.get("numbase")) else {
            return Err(mei_error(
                "Both @num and @numbase attributes are required on <tuplet> tags.",
            ));
        };
        let read = |text: &str| -> Result<u32> {
            text.parse()
                .map_err(|_| mei_error(format!("a tuplet of {text:?} notes")))
        };
        let (number, base) = (read(number)?, read(base)?);
        let mut members = self.contents(element, false)?;
        for member in &mut members {
            scale_to_tuplet(&mut member.element, number, base);
        }
        let sounding: Vec<usize> = (0..members.len())
            .filter(|index| is_sounding(&members[*index].element))
            .collect();
        let first = *sounding
            .first()
            .ok_or_else(|| mei_error("a tuplet with no notes in it"))?;
        mark_first_tuplet(&mut members[first].element, TupletType::Start);
        let last = sounding.last().copied().filter(|last| *last != first);
        mark_first_tuplet(
            &mut members[last.unwrap_or(first)].element,
            TupletType::Stop,
        );
        beam_together(&mut members)?;
        Ok(members)
    }

    /// music21's `bTremFromElement`: notes played as repeated strokes.
    fn tremolo(&mut self, element: &Xml) -> Result<Vec<Thing>> {
        let mut things = Vec::new();
        for held in &element.children {
            match local(&held.tag) {
                "chord" => things.push(self.chord(held)?),
                "note" => things.push(self.note_thing(held)?),
                _ => {}
            }
        }
        let marks = match element.get("unitdur") {
            Some("8") => Some(1),
            Some("16") => Some(2),
            Some("32") => Some(3),
            Some("64") => Some(4),
            Some("128") => Some(5),
            Some("256") => Some(6),
            Some("512") => Some(7),
            Some("1024") => Some(8),
            _ => None,
        };
        if let Some(marks) = marks {
            for thing in &mut things {
                let mut tremolo = Ornament::of_kind(OrnamentKind::Tremolo);
                tremolo.set_number_of_marks(marks)?;
                let expression = Expression::from(tremolo);
                match &mut thing.element {
                    StreamElement::Note(note) => note.expressions_mut().push(expression),
                    StreamElement::Chord(chord) => chord.expressions_mut().push(expression),
                    _ => {}
                }
            }
        }
        Ok(things)
    }

    /// music21's `layerFromElement`, with `_guessTuplets`: the notes between
    /// the two a tuplet span names are in the tuplet too.
    fn layer(&mut self, element: &Xml) -> Result<Vec<Thing>> {
        let mut things = self.contents(element, true)?;
        let mut active: Option<(u32, u32)> = None;
        for thing in &mut things {
            if !is_sounding(&thing.element) {
                continue;
            }
            let search = thing.search.take();
            if let Some(search) = &search
                && search.end == "start"
            {
                active = Some((search.number, search.base));
            }
            if let Some((number, base)) = active {
                scale_to_tuplet(&mut thing.element, number, base);
                if search.is_some_and(|search| search.end == "end") {
                    mark_first_tuplet(&mut thing.element, TupletType::Stop);
                    active = None;
                }
            }
        }
        Ok(things)
    }

    /// music21's `staffDefFromElement`.
    fn staff_def(&mut self, element: &Xml) -> Result<StaffDef> {
        let instrument = match child(element, "instrDef") {
            Some(definition) => Some(match definition.get("midi.instrnum") {
                Some(number) => Instrument::from_midi_program(number.parse().map_err(|_| {
                    mei_error(format!("the MIDI program {number:?} is no number"))
                })?)?,
                None => {
                    let name = definition.get("midi.instrname");
                    name.and_then(|name| Instrument::from_name(name, SearchLanguage::All).ok())
                        .unwrap_or_else(|| {
                            let mut generic = Instrument::new();
                            generic.set_part_name(Some(name.unwrap_or("").to_string()));
                            generic
                        })
                }
            }),
            None => {
                Instrument::from_name(element.get("label").unwrap_or(""), SearchLanguage::All).ok()
            }
        };
        let mut found = StaffDef {
            instrument,
            ..StaffDef::default()
        };
        if let Some(instrument) = &mut found.instrument {
            instrument.set_part_name(element.get("label").map(str::to_string));
            instrument.set_part_abbreviation(element.get("label.abbr").map(str::to_string));
            instrument.set_part_id(element.get("n").map(str::to_string));
        }
        if element.get("trans.semi").is_some() {
            found
                .instrument
                .get_or_insert_with(Instrument::new)
                .set_transposition(Some(transposition_of(element)?));
        }
        if element.get("meter.count").is_some() {
            found.meter = Some(meter_of(element)?);
        }
        if element.get("key.pname").is_some() || element.get("key.sig").is_some() {
            found.key = Some(key_of(element)?);
        }
        if element.get("clef.shape").is_some() {
            found.clef = Some(clef_of(
                element.get("clef.shape"),
                element.get("clef.line"),
                element.get("clef.dis"),
                element.get("clef.dis.place"),
            )?);
        }
        for held in children(element, "clef") {
            found.clef = Some(clef_element(held)?);
        }
        Ok(found)
    }

    /// music21's `measureFromElement`: a measure of every part.
    fn measure(
        &mut self,
        element: &Xml,
        backup: IntegerType,
        parts: &[String],
        meter: Option<&TimeSignature>,
    ) -> Result<(HashMap<String, Measure>, Option<Barline>)> {
        let number = match element.get("n") {
            Some(number) => number
                .parse()
                .map_err(|_| mei_error(format!("the measure number {number:?} is no number")))?,
            None => backup,
        };
        let mut staves: HashMap<String, Measure> = HashMap::new();
        let mut waiting: Vec<(String, StaffDef)> = Vec::new();
        let mut longest: Option<FloatType> = None;
        for held in &element.children {
            match local(&held.tag) {
                "staff" => {
                    let mut voices = Vec::new();
                    for (count, layer) in children(held, "layer").enumerate() {
                        voices.push(((count + 1).to_string(), self.layer(layer)?));
                    }
                    let length = voices
                        .iter()
                        .map(|(_, things)| {
                            things
                                .iter()
                                .map(|thing| thing.element.quarter_length())
                                .sum::<FloatType>()
                        })
                        .fold(0.0, FloatType::max);
                    longest = Some(longest.map_or(length, |held| held.max(length)));
                    staves.insert(
                        held.get("n").unwrap_or("").to_string(),
                        Measure {
                            number,
                            voices,
                            own: Vec::new(),
                            left: None,
                            right: None,
                            length: None,
                        },
                    );
                }
                "staffDef" => {
                    if let Some(name) = held.get("n") {
                        waiting.push((name.to_string(), self.staff_def(held)?));
                    }
                }
                _ => {}
            }
        }
        for (name, definition) in waiting {
            let measure = staves.get_mut(&name).ok_or_else(|| {
                mei_error(format!("a <staffDef> for staff {name}, which is not here"))
            })?;
            measure.own.extend(definition.elements());
        }
        // A part with nothing written for this measure rests through it.
        for name in parts {
            if !staves.contains_key(name) {
                let length = longest.ok_or_else(|| mei_error("a measure with no staff in it"))?;
                staves.insert(
                    name.clone(),
                    Measure {
                        number,
                        voices: vec![(
                            "1".to_string(),
                            vec![Thing {
                                element: Rest::new(Duration::new(length)?).into(),
                                uid: self.uid(),
                                search: None,
                                whole_measure: true,
                            }],
                        )],
                        own: Vec::new(),
                        left: None,
                        right: None,
                        length: None,
                    },
                );
            }
        }
        // A measure rest lasts as long as the longest staff, or the bar
        // where no staff says.
        let target = match (longest, meter) {
            (Some(longest), Some(meter))
                if longest == NO_DURATION && longest != meter.bar_quarter_length() =>
            {
                Some(meter.bar_quarter_length())
            }
            (longest, _) => longest,
        };
        if let Some(target) = target {
            for measure in staves.values_mut() {
                for (_, things) in &mut measure.voices {
                    for thing in things {
                        if std::mem::take(&mut thing.whole_measure) {
                            let duration = Duration::new(target)?;
                            with_duration(&mut thing.element, |held| *held = duration);
                            measure.length = Some(target);
                        }
                    }
                }
            }
        }
        let mut next_left = None;
        if let Some(left) = element.get("left") {
            let (first, second) = barline_of(left)?;
            let barline = second.unwrap_or(first);
            for measure in staves.values_mut() {
                measure.left = Some(barline.clone());
            }
        }
        if let Some(right) = element.get("right") {
            let (barline, next) = barline_of(right)?;
            next_left = next;
            for measure in staves.values_mut() {
                measure.right = Some(barline.clone());
            }
        }
        Ok((staves, next_left))
    }

    /// music21's `sectionScoreCore`: what a `<score>` or a `<section>`
    /// holds, part by part.
    fn section(
        &mut self,
        element: &Xml,
        parts: &[String],
        state: &mut SectionState,
    ) -> Result<HashMap<String, Vec<Held>>> {
        let in_section = local(&element.tag) == "section";
        let mut parsed: HashMap<String, Vec<Held>> = parts
            .iter()
            .map(|name| (name.clone(), Vec::new()))
            .collect();
        let mut next: HashMap<String, Vec<StreamElement>> = parts
            .iter()
            .map(|name| (name.clone(), Vec::new()))
            .collect();
        for held in &element.children {
            match local(&held.tag) {
                "measure" if in_section => {
                    state.backup += 1;
                    let (mut staves, next_left) =
                        self.measure(held, state.backup, parts, state.meter.as_ref())?;
                    for name in parts {
                        let measure = staves
                            .get_mut(name)
                            .ok_or_else(|| mei_error(format!("no staff {name} in a measure")))?;
                        measure
                            .own
                            .extend(std::mem::take(next.entry(name.clone()).or_default()));
                        if let Some(left) = &state.next_left {
                            measure.left = Some(left.clone());
                        }
                        if let Some(measure) = staves.remove(name) {
                            parsed
                                .entry(name.clone())
                                .or_default()
                                .push(Held::Measure(Box::new(measure)));
                        }
                    }
                    state.next_left = next_left;
                }
                "scoreDef" => {
                    let mut everywhere: Vec<StreamElement> = Vec::new();
                    if held.get("meter.count").is_some() {
                        let meter = meter_of(held)?;
                        state.meter = Some(meter.clone());
                        everywhere.push(meter.into());
                    }
                    if held.get("key.pname").is_some() || held.get("key.sig").is_some() {
                        everywhere.push(StreamElement::Key(key_of(held)?));
                    }
                    let mut definitions: Vec<&Xml> = Vec::new();
                    for group in children(held, "staffGrp") {
                        descendants_in_groups(group, &mut definitions);
                    }
                    let mut own: HashMap<String, StaffDef> = HashMap::new();
                    for definition in definitions {
                        own.insert(
                            definition.get("n").unwrap_or("").to_string(),
                            self.staff_def(definition)?,
                        );
                    }
                    for name in parts {
                        let waiting = next.entry(name.clone()).or_default();
                        waiting.extend(everywhere.iter().cloned());
                        if let Some(definition) = own.get(name) {
                            waiting.extend(definition.elements());
                        }
                    }
                }
                "staffDef" => {
                    if let Some(name) = held.get("n") {
                        let definition = self.staff_def(held)?;
                        if let Some(meter) = &definition.meter {
                            state.meter = Some(meter.clone());
                        }
                        next.get_mut(name)
                            .ok_or_else(|| mei_error(format!("a <staffDef> for no part: {name}")))?
                            .extend(definition.elements());
                    }
                }
                "section" => {
                    let inner = self.section(held, parts, state)?;
                    for name in parts {
                        let mut list = inner.get(name).cloned().unwrap_or_default();
                        let mut waiting = std::mem::take(next.entry(name.clone()).or_default());
                        if !waiting.is_empty() {
                            // The instrument stands before the measures, and
                            // the rest at the start of the first of them.
                            if let Some(at) = waiting
                                .iter()
                                .position(|thing| matches!(thing, StreamElement::Instrument(_)))
                                && let StreamElement::Instrument(instrument) = waiting.remove(at)
                            {
                                list.insert(0, Held::Instrument(instrument));
                            }
                            if let Some(Held::Measure(first)) = list
                                .iter_mut()
                                .find(|thing| matches!(thing, Held::Measure(_)))
                            {
                                first.own.extend(waiting);
                            }
                        }
                        parsed.entry(name.clone()).or_default().extend(list);
                    }
                }
                _ => {}
            }
        }
        Ok(parsed)
    }
}

/// What carries from one section to the next: the meter in force, the
/// barline the next measure opens with, and the measure count.
#[derive(Default)]
struct SectionState {
    meter: Option<TimeSignature>,
    next_left: Option<Barline>,
    backup: IntegerType,
}

/// The `<staffDef>`s of a group and of the groups inside it.
fn descendants_in_groups<'a>(group: &'a Xml, out: &mut Vec<&'a Xml>) {
    for held in &group.children {
        match local(&held.tag) {
            "staffDef" => out.push(held),
            "staffGrp" => descendants_in_groups(held, out),
            _ => {}
        }
    }
}

fn beam_type_of(text: &str) -> Result<BeamType> {
    match text {
        "start" => Ok(BeamType::Start),
        "continue" => Ok(BeamType::Continue),
        "stop" => Ok(BeamType::Stop),
        other => Err(mei_error(format!("a beam that does {other:?}"))),
    }
}

/// music21's `sylFromElement`: a syllable, with the hyphen its place in the
/// word gives it.
fn syllable_of(element: &Xml) -> Result<Lyric> {
    let text = element
        .text()
        .ok_or_else(|| mei_error("a <syl> with no text"))?;
    let joiner = match element.get("con") {
        Some("s") => " ",
        Some("d") | None => "-",
        Some("t") => "~",
        Some("u") => "_",
        Some(other) => {
            return Err(mei_error(format!("a syllable joined by {other:?}")));
        }
    };
    let (text, syllabic) = match element.get("wordpos") {
        Some("i") => (format!("{text}{joiner}"), Some(Syllabic::Begin)),
        Some("m") => (format!("{joiner}{text}{joiner}"), Some(Syllabic::Middle)),
        Some("t") => (format!("{joiner}{text}"), Some(Syllabic::End)),
        None => (text.to_string(), None),
        Some(other) => {
            return Err(mei_error(format!("a syllable placed at {other:?}")));
        }
    };
    Ok(match syllabic {
        Some(syllabic) => {
            let mut lyric = Lyric::new(text);
            lyric.set_syllabic(syllabic);
            lyric
        }
        None => Lyric::from_raw_text(&text),
    })
}

/// music21's `makeMetadata`: the title, composer and date of the first
/// `<work>`.
fn metadata_of(root: &Xml) -> Metadata {
    let mut metadata = Metadata::new();
    let mut works = Vec::new();
    descendants(root, "work", &mut works);
    let Some(work) = works.first() else {
        return metadata;
    };
    let statement = child(work, "titleStmt");
    let mut title: Option<String> = None;
    let mut subtitle: Option<String> = None;
    for held in statement
        .into_iter()
        .flat_map(|held| children(held, "title"))
    {
        let text = held.text().map(str::to_string);
        if held.get("type") == Some("subtitle") {
            subtitle = text;
        } else if title.is_none() {
            title = text;
        }
    }
    if let Some(subtitle) = subtitle.filter(|subtitle| !subtitle.is_empty()) {
        title = Some(format!(
            "{} ({subtitle})",
            title.as_deref().unwrap_or("None")
        ));
    }
    if let Some(title) = title {
        metadata.add_text("title", title);
    }
    if let Some(tempo) = child(work, "tempo") {
        metadata.add_text("movementName", tempo.text().unwrap_or("None"));
    }
    let mut composers: Vec<String> = Vec::new();
    if let Some(statement) = statement {
        for responsibility in children(statement, "respStmt") {
            for person in children(responsibility, "persName") {
                if person.get("role") == Some("composer")
                    && let Some(text) = person.text().filter(|text| !text.is_empty())
                {
                    composers.push(text.to_string());
                }
            }
        }
        for composer in children(statement, "composer") {
            let text = composer
                .text()
                .filter(|text| !text.is_empty())
                .or_else(|| child(composer, "persName").and_then(Xml::text));
            if let Some(text) = text.filter(|text| !text.is_empty()) {
                composers.push(text.to_string());
            }
        }
    }
    for composer in composers {
        metadata.add_contributor("composer", composer);
    }
    let date = child(work, "history")
        .and_then(|history| child(history, "creation"))
        .and_then(|creation| child(creation, "date"));
    if let Some(date) = date {
        let written = date
            .get("isodate")
            .filter(|text| !text.is_empty())
            .or(date.text().filter(|text| !text.is_empty()));
        // A date as music21 writes one: year, month and day, with two
        // hyphens for what is not said.
        let said = |written: &str| -> Option<String> {
            let parts: Vec<&str> = written.split(['-', '/']).collect();
            if parts.len() > 3 {
                return None;
            }
            let mut fields = Vec::new();
            for (index, part) in parts.iter().enumerate() {
                let number: u32 = part.parse().ok()?;
                fields.push(if index == 0 {
                    format!("{number:04}")
                } else {
                    format!("{number:02}")
                });
            }
            fields.resize(3, "--".to_string());
            Some(fields.join("/"))
        };
        if let Some(written) = written {
            if let Some(date) = said(written) {
                metadata.add_text("dateCreated", date);
            }
        } else {
            let start = date.get("notbefore").or(date.get("startdate"));
            let end = date.get("notafter").or(date.get("enddate"));
            if let (Some(start), Some(end)) = (start, end)
                && let (Some(start), Some(end)) = (said(start), said(end))
            {
                metadata.add_text("dateCreated", format!("{start} to {end}"));
            }
        }
    }
    metadata
}

/// Reads an MEI document into a score: music21's `converter.parse` of one.
///
/// Every `<staffDef>` number is a part. A measure holds a voice for each
/// `<layer>`; a part with no staff in a measure rests through it, and a
/// measure rest lasts what the longest staff of its measure lasts. Beams,
/// tuplets, ties and slurs are read where they are written on the notes and
/// where they stand apart and name the notes by `xml:id`.
///
/// The text is the document's own, already decoded: the library opens no
/// file. A barline written inside a layer is passed over.
///
/// ```
/// use music21_rs::mei::from_mei;
///
/// let score = from_mei(
///     r#"<mei xmlns="http://www.music-encoding.org/ns/mei"><music><score>
///     <scoreDef meter.count="2" meter.unit="4">
///       <staffGrp><staffDef n="1" clef.shape="G" clef.line="2"/></staffGrp>
///     </scoreDef>
///     <section><measure n="1"><staff n="1"><layer n="1">
///       <note pname="c" oct="4" dur="4"/><note pname="e" oct="4" dur="4" accid="f"/>
///     </layer></staff></measure></section>
///     </score></music></mei>"#,
/// )?;
/// let names: Vec<String> = score.parts()[0]
///     .pitches()
///     .iter()
///     .map(|pitch| pitch.name_with_octave())
///     .collect();
/// assert_eq!(names, ["C4", "Eb4"]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
pub fn from_mei(document: &str) -> Result<Stream> {
    let root = Xml::parse(document).map_err(|_| mei_error("MEI document is not valid XML."))?;
    if local(&root.tag) != "mei" {
        return Err(mei_error(format!(
            "Root element should be <mei> in the MEI namespace, not <{}>.",
            root.tag
        )));
    }
    let score = child(&root, "music")
        .and_then(|music| {
            let mut found = Vec::new();
            descendants(music, "score", &mut found);
            found.first().copied()
        })
        .ok_or_else(|| mei_error("an MEI document with no <score> in its <music>"))?;

    let mut reader = Reader {
        extra: HashMap::new(),
        slurs: Vec::new(),
        next_uid: 0,
        next_slur: 0,
    };
    reader.preprocess(score);

    // music21's `allPartsPresent`: every staff number, in the order met.
    let mut definitions = Vec::new();
    descendants(score, "staffDef", &mut definitions);
    let mut parts: Vec<String> = Vec::new();
    for definition in definitions {
        let name = definition.get("n").unwrap_or("").to_string();
        if !parts.contains(&name) {
            parts.push(name);
        }
    }
    if parts.is_empty() {
        return Err(mei_error(
            "There appear to be no <staffDef> tags in this score.",
        ));
    }

    let mut state = SectionState::default();
    let parsed = reader.section(score, &parts, &mut state)?;

    // The parts, and the place among the score's leaves of every note a
    // slur may name.
    let mut events = Vec::new();
    let mut uids: Vec<Option<usize>> = Vec::new();
    for name in &parts {
        let mut held = Vec::new();
        let mut offset = 0.0;
        for thing in parsed.get(name).cloned().unwrap_or_default() {
            match thing {
                Held::Instrument(instrument) => {
                    uids.push(None);
                    held.push(StreamEvent::new(offset, *instrument));
                }
                Held::Measure(measure) => {
                    // What stands at the start sorts by its class, ahead of
                    // the voices.
                    let mut own: Vec<StreamEvent> = measure
                        .own
                        .into_iter()
                        .map(|element| StreamEvent::new(0.0, element))
                        .collect();
                    own.sort_by_key(|event| event.element().class_sort_order());
                    uids.extend(own.iter().map(|_| None));
                    let mut length: FloatType = 0.0;
                    for (id, things) in measure.voices {
                        let mut at = 0.0;
                        let mut inner = Vec::new();
                        for thing in things {
                            uids.push(Some(thing.uid));
                            let lasts = thing.element.quarter_length();
                            inner.push(StreamEvent::new(at, thing.element));
                            at = op_frac(at + lasts);
                        }
                        length = length.max(at);
                        let mut voice = Stream::with_kind(StreamKind::Voice).with_events(inner);
                        voice.set_id(Some(id));
                        own.push(StreamEvent::new(0.0, voice));
                    }
                    let mut stream = Stream::with_kind(StreamKind::Measure).with_events(own);
                    stream.set_number(measure.number);
                    stream.set_left_barline(measure.left);
                    stream.set_right_barline(measure.right);
                    held.push(StreamEvent::new(offset, stream));
                    offset = op_frac(offset + measure.length.unwrap_or(length));
                }
            }
        }
        events.push(StreamEvent::new(
            0.0,
            Stream::with_kind(StreamKind::Part).with_events(held),
        ));
    }
    let mut made = Stream::with_kind(StreamKind::Score).with_events(events);
    made.set_metadata(Some(metadata_of(&root)));
    for (_, joined) in reader.slurs {
        let positions: Vec<Option<usize>> = joined
            .iter()
            .map(|uid| uids.iter().position(|held| *held == Some(*uid)))
            .collect();
        made.add_spanner(Spanner::with_unplaced(SpannerKind::Slur, positions));
    }
    Ok(made)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(body: &str) -> String {
        format!(
            r#"<mei xmlns="http://www.music-encoding.org/ns/mei"><music><score>
            <scoreDef meter.count="4" meter.unit="4"><staffGrp>
            <staffDef n="1" clef.shape="G" clef.line="2"/>
            <staffDef n="2" clef.shape="F" clef.line="4"/>
            </staffGrp></scoreDef><section>{body}</section></score></music></mei>"#
        )
    }

    #[test]
    fn a_staff_with_nothing_written_rests_through_the_measure() {
        let score = from_mei(&document(
            r#"<measure n="1"><staff n="1"><layer n="1">
            <note pname="c" oct="4" dur="2"/><note pname="d" oct="4" dur="2"/>
            </layer></staff></measure>"#,
        ))
        .unwrap();
        let parts = score.parts();
        assert_eq!(parts.len(), 2);
        let rests: Vec<FloatType> = parts[1]
            .recurse()
            .into_iter()
            .filter(|(_, element)| matches!(element, StreamElement::Rest(_)))
            .map(|(_, element)| element.quarter_length())
            .collect();
        assert_eq!(rests, [4.0]);
    }

    #[test]
    fn a_beam_runs_from_its_first_note_to_its_last() {
        let score = from_mei(&document(
            r#"<measure n="1"><staff n="1"><layer n="1"><beam>
            <note pname="c" oct="4" dur="8"/><note pname="d" oct="4" dur="8"/>
            <note pname="e" oct="4" dur="8"/></beam></layer></staff>
            <staff n="2"><layer n="1"><mRest/></layer></staff></measure>"#,
        ))
        .unwrap();
        let beams: Vec<Option<BeamType>> = score.parts()[0]
            .recurse()
            .into_iter()
            .filter_map(|(_, element)| match element {
                StreamElement::Note(note) => {
                    Some(note.beams().by_number(1).and_then(|beam| beam.beam_type()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            beams,
            [
                Some(BeamType::Start),
                Some(BeamType::Continue),
                Some(BeamType::Stop)
            ]
        );
    }

    #[test]
    fn a_tuplet_scales_its_notes_and_brackets_them() {
        let score = from_mei(&document(
            r#"<measure n="1"><staff n="1"><layer n="1"><tuplet num="3" numbase="2">
            <note pname="c" oct="4" dur="4"/><note pname="d" oct="4" dur="4"/>
            <note pname="e" oct="4" dur="4"/></tuplet></layer></staff></measure>"#,
        ))
        .unwrap();
        let lengths: Vec<FloatType> = score.parts()[0]
            .notes()
            .into_iter()
            .map(|(_, element)| element.quarter_length())
            .collect();
        assert_eq!(lengths.len(), 3);
        assert!(
            lengths
                .iter()
                .all(|length| (length - 2.0 / 3.0).abs() < 1e-9)
        );
    }

    #[test]
    fn a_tie_written_apart_names_its_notes() {
        let score = from_mei(&document(
            r##"<measure n="1"><staff n="1"><layer n="1">
            <note xml:id="a" pname="c" oct="4" dur="2"/><note xml:id="b" pname="c" oct="4" dur="2"/>
            </layer></staff><tie startid="#a" endid="#b"/></measure>"##,
        ))
        .unwrap();
        let ties: Vec<Option<TieType>> = score.parts()[0]
            .notes()
            .into_iter()
            .filter_map(|(_, element)| match element {
                StreamElement::Note(note) => Some(note.tie().map(Tie::tie_type)),
                _ => None,
            })
            .collect();
        assert_eq!(ties, [Some(TieType::Start), Some(TieType::Stop)]);
    }

    #[test]
    fn what_is_no_mei_is_refused() {
        assert!(from_mei("<score-partwise/>").is_err());
        assert!(from_mei("not xml at all").is_err());
    }
}
