//! Capella's CapXML: music21's `capella.fromCapellaXML`.
//!
//! A `.capx` file is a zip archive holding the score as `score.xml`;
//! [`from_capella`] reads that document's text, the caller having unpacked
//! it, as this crate unpacks nothing.

use crate::bar::{Barline, BarlineType, RepeatDirection};
use crate::chord::Chord;
use crate::clef::{Clef, ClefKind};
use crate::defaults::{FloatType, IntegerType};
use crate::duration::{Duration, Tuplet};
use crate::error::{Error, Result};
use crate::key::KeySignature;
use crate::makenotation::{make_measures, op_frac};
use crate::meter::TimeSignature;
use crate::notation::{Lyric, Syllabic, Tie, TieType};
use crate::note::Note;
use crate::pitch::Pitch;
use crate::pitch::accidental::Accidental;
use crate::rest::Rest;
use crate::stream::{Stream, StreamElement, StreamEvent, StreamKind};
use crate::xml::Xml;

fn error(message: impl Into<String>) -> Error {
    Error::Capella(message.into())
}

/// What a staff holds, each element at its offset.
type Placed = Vec<(FloatType, StreamElement)>;

/// A staff's part id and what it holds.
type Staff = (String, Placed);

/// Reads the text of a Capella `score.xml` into a score, as music21's
/// `CapellaImporter.scoreFromFile` reads a `.capx`.
///
/// The score is read system by system, each staff of a system being a
/// part's share of it, and a part's shares are put end to end, each system
/// starting where the longest staff of the one before ended. A staff of one
/// voice is read: clefs, key signatures, meters, rests, notes and chords
/// with their accidentals, ties, triplets and other tuplets and lyrics, and
/// barlines. A clef or key signature restating the one before is dropped,
/// and each part is then cut into measures by its meters.
///
/// music21's limits are kept: a staff of more than one voice is read as
/// empty, a barline is a mark standing in the measure rather than the
/// measure's own (where a MusicXML writer passes it over), a `single`
/// barline is refused as music21 refuses it, and a tuplet's `prolong` is
/// never read.
///
/// ```
/// use music21_rs::capella::from_capella;
///
/// let score = from_capella(
///     "<score><systems><system><staves><staff layout=\"S\"><voices><voice><noteObjects>\
///      <clefSign clef=\"treble\"/><timeSign time=\"2/4\"/>\
///      <chord><duration base=\"1/4\"/><heads><head pitch=\"C5\"/></heads></chord>\
///      <chord><duration base=\"1/4\"/><heads><head pitch=\"E5\"/></heads></chord>\
///      </noteObjects></voice></voices></staff></staves></system></systems></score>",
/// )?;
/// let pitches = score.pitches();
/// assert_eq!(pitches[0].name_with_octave(), "C4");
/// # Ok::<(), music21_rs::Error>(())
/// ```
///
/// # Errors
///
/// A document that is not XML, one with no systems or with a system, staff
/// or voice missing what music21 asks for, a chord with no heads or with
/// more than one duration, a head with no pitch, a barline or meter music21
/// does not know, or a number that is not one.
pub fn from_capella(document: &str) -> Result<Stream> {
    let document = document.strip_prefix('\u{feff}').unwrap_or(document);
    let root = without_namespaces(Xml::parse(document)?);

    let mut systems_found = children(&root, "systems");
    let systems = match (systems_found.next(), systems_found.next()) {
        (Some(systems), None) => systems,
        (None, _) => return Err(error("Cannot find a <systems> tag in the <score> object")),
        (Some(_), Some(_)) => {
            return Err(error(
                "Found more than one <systems> tag in the <score> object, what does this mean?",
            ));
        }
    };
    let systems: Vec<&Xml> = children(systems, "system").collect();
    if systems.is_empty() {
        return Err(error(
            "Cannot find any <system> tags in the <systems> tag in the <score> object",
        ));
    }

    // Each part by its id, in the order first seen, with the place it held
    // in the system where it was, and what it holds.
    let mut parts: Vec<(String, usize, Placed)> = Vec::new();
    let mut system_offset = 0.0;
    for system in systems {
        let staves = read_system(system)?;
        let length = staves
            .iter()
            .map(|(_, elements)| end_of(elements))
            .fold(0.0, FloatType::max);
        for (place, (id, elements)) in staves.into_iter().enumerate() {
            let index = match parts.iter().position(|(known, _, _)| *known == id) {
                Some(index) => index,
                None => {
                    parts.push((id, place, Vec::new()));
                    parts.len() - 1
                }
            };
            for (offset, element) in elements {
                parts[index]
                    .2
                    .push((op_frac(offset + system_offset), element));
            }
        }
        system_offset += length;
    }

    // music21 puts each part at the place it held where it was first seen,
    // in a list as long as there are parts.
    let mut slots: Vec<Option<Staff>> = (0..parts.len()).map(|_| None).collect();
    for (id, place, elements) in parts {
        let slot = slots
            .get_mut(place)
            .ok_or_else(|| error("part entries do not match partDict!"))?;
        *slot = Some((id, elements));
    }
    let mut score = Stream::with_kind(StreamKind::Score);
    for (id, elements) in slots.into_iter().flatten() {
        let mut part = Stream::with_kind(StreamKind::Part);
        for (offset, element) in elements {
            part.insert_sorted(vec![StreamEvent::new(offset, element)]);
        }
        let part = without_restatements(part);
        let mut measured = make_measures(&part)?;
        measured.set_id(Some(id));
        score.insert(0.0, measured);
    }
    Ok(score)
}

/// The element with every tag's namespace taken off, as music21 strips
/// them.
fn without_namespaces(mut element: Xml) -> Xml {
    if let Some((_, local)) = element.tag.rsplit_once(':') {
        element.tag = local.to_string();
    }
    element.children = element
        .children
        .into_iter()
        .map(without_namespaces)
        .collect();
    element
}

fn children<'a>(element: &'a Xml, tag: &'a str) -> impl Iterator<Item = &'a Xml> {
    element
        .children
        .iter()
        .filter(move |child| child.tag == tag)
}

/// The one child of a tag, where there is exactly one.
fn only<'a>(element: &'a Xml, tag: &'a str, missing: &str, several: &str) -> Result<&'a Xml> {
    let mut found = children(element, tag);
    match (found.next(), found.next()) {
        (Some(child), None) => Ok(child),
        (None, _) => Err(error(missing)),
        (Some(_), Some(_)) => Err(error(several)),
    }
}

fn end_of(elements: &[(FloatType, StreamElement)]) -> FloatType {
    elements
        .iter()
        .map(|(offset, element)| offset + element.quarter_length())
        .fold(0.0, FloatType::max)
}

/// `systemFromSystem`: each staff's id and what its one voice holds.
fn read_system(system: &Xml) -> Result<Vec<Staff>> {
    let staves = only(
        system,
        "staves",
        "No <staves> tag found in this <system> element",
        "More than one <staves> tag found in this <system> element",
    )?;
    let mut out = Vec::new();
    for staff in children(staves, "staff") {
        let id = staff.get("layout").unwrap_or("UnknownPart").to_string();
        let voices = children(staff, "voices").next().ok_or_else(|| {
            error(
                "No <voices> tag found in the <staff> tag for the <staves> element for this \
                 <system> element",
            )
        })?;
        let voice_list: Vec<&Xml> = children(voices, "voice").collect();
        if voice_list.is_empty() {
            return Err(error(
                "No <voice> tag found in the <voices> tag for the <staff> tag for the <staves> \
                 element for this <system> element",
            ));
        }
        let mut elements = Vec::new();
        if let [voice] = voice_list.as_slice() {
            let objects = only(
                voice,
                "noteObjects",
                "No <noteObjects> tag found in the <voice> tag",
                "More than one <noteObjects> tag found in the <voice> tag",
            )?;
            for object in &objects.children {
                for element in note_object(object)? {
                    let at = end_of(&elements);
                    elements.push((at, element));
                }
            }
        }
        out.push((id, elements));
    }
    Ok(out)
}

/// `streamFromNoteObjects`' mapping: what one note object is, if anything.
fn note_object(object: &Xml) -> Result<Vec<StreamElement>> {
    Ok(match object.tag.as_str() {
        "clefSign" => clef(object)?.map(StreamElement::Clef).into_iter().collect(),
        "keySign" => match object.get("fifths") {
            Some(fifths) => vec![StreamElement::KeySignature(KeySignature::new(integer(
                fifths,
            )?))],
            None => Vec::new(),
        },
        "timeSign" => match object.get("time") {
            Some(time) if time != "infinite" => {
                vec![StreamElement::TimeSignature(
                    TimeSignature::from_ratio_string(time)?,
                )]
            }
            _ => Vec::new(),
        },
        "rest" => {
            let written = children(object, "duration")
                .next()
                .ok_or_else(|| error("a rest with no duration"))?;
            vec![StreamElement::Rest(Rest::new(duration(written)?))]
        }
        "chord" => vec![chord(object)?],
        "barline" => barlines(object)?,
        _ => Vec::new(),
    })
}

fn integer(text: &str) -> Result<IntegerType> {
    text.trim()
        .parse()
        .map_err(|_| error(format!("{text:?} is not a number")))
}

/// `clefFromClefSign`.
fn clef(object: &Xml) -> Result<Option<Clef>> {
    let Some(value) = object.get("clef") else {
        return Ok(None);
    };
    let kind = match value {
        "treble" => Some(ClefKind::TrebleClef),
        "bass" => Some(ClefKind::BassClef),
        "alto" => Some(ClefKind::AltoClef),
        "tenor" => Some(ClefKind::TenorClef),
        "G2-" => Some(ClefKind::Treble8vbClef),
        _ => None,
    };
    if let Some(kind) = kind {
        return Ok(Some(Clef::of_kind(kind)));
    }
    if value.starts_with('p') {
        return Ok(Some(Clef::of_kind(ClefKind::PercussionClef)));
    }
    let letters: Vec<char> = value.chars().collect();
    if letters.len() > 1 {
        let shift = match letters.get(2) {
            Some('+') => 1,
            Some('-') => -1,
            _ => 0,
        };
        let sign_and_line: String = letters[..2].iter().collect();
        return Clef::from_string(&sign_and_line, shift).map(Some);
    }
    Ok(None)
}

/// `chordOrNoteFromChord`.
fn chord(object: &Xml) -> Result<StreamElement> {
    let durations: Vec<&Xml> = children(object, "duration").collect();
    let heads: Vec<&Xml> = children(object, "heads").collect();
    let ([written], [heads]) = (durations.as_slice(), heads.as_slice()) else {
        return Err(error("Malformed chord!"));
    };
    let mut notes = Vec::new();
    for head in children(heads, "head") {
        notes.push(note(head)?);
    }
    if notes.is_empty() {
        return Err(error("Malformed chord!"));
    }
    let duration = duration(written)?;
    let lyrics: Vec<Lyric> = match children(object, "lyric").next() {
        Some(lyric) => children(lyric, "verse")
            .map(verse)
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect(),
        None => Vec::new(),
    };
    Ok(if notes.len() == 1 {
        let mut note = notes.remove(0);
        note.set_duration(duration);
        *note.lyrics_mut() = lyrics;
        StreamElement::Note(note)
    } else {
        let mut chord = Chord::new(notes)?.with_duration(duration);
        if let Some(first) = chord.notes_mut().first_mut() {
            *first.lyrics_mut() = lyrics;
        }
        StreamElement::Chord(chord)
    })
}

/// `noteFromHead`: Capella writes its octaves one higher than music21.
fn note(head: &Xml) -> Result<Note> {
    let written = head
        .get("pitch")
        .ok_or_else(|| error("Cannot deal with <head> element without pitch!"))?;
    let mut pitch = Pitch::from_name(written)?;
    let octave = pitch.octave().unwrap_or(4);
    pitch.set_octave(Some(octave - 1));
    let alters: Vec<&Xml> = children(head, "alter").collect();
    match alters.as_slice() {
        [] => {}
        [alter] => pitch.set_accidental(accidental(alter)?),
        _ => return Err(error("Cannot deal with multiple <alter> elements!")),
    }
    let mut note = Note::from_pitch(pitch);
    let ties: Vec<&Xml> = children(head, "tie").collect();
    match ties.as_slice() {
        [] => {}
        [tie] => note.set_tie(tie_of(tie)),
        _ => return Err(error("Cannot deal with multiple <tie> elements!")),
    }
    Ok(note)
}

/// `accidentalFromAlter`.
fn accidental(alter: &Xml) -> Result<Accidental> {
    let steps = match alter.get("step") {
        Some(step) => integer(step)?,
        None => 0,
    };
    let name = match steps {
        0 => "natural",
        1 => "sharp",
        -1 => "flat",
        2 => "double-sharp",
        -2 => "double-flat",
        3 => "triple-sharp",
        -3 => "triple-flat",
        4 => "quadruple-sharp",
        -4 => "quadruple-flat",
        other => return Err(error(format!("no accidental of {other} steps"))),
    };
    let mut accidental = Accidental::new(name)?;
    if alter.get("display") == Some("suppress") {
        accidental.set_display_type("never")?;
    }
    Ok(accidental)
}

/// `tieFromTie`.
fn tie_of(tie: &Xml) -> Option<Tie> {
    let begins = tie.get("begin") == Some("true");
    let ends = tie.get("end") == Some("true");
    let kind = match (begins, ends) {
        (true, true) => TieType::Continue,
        (true, false) => TieType::Start,
        (false, true) => TieType::Stop,
        (false, false) => return None,
    };
    Some(Tie::new(kind))
}

/// `lyricFromVerse`: a verse's syllable, numbered from one, hyphenated to
/// the next where it says so; nothing for a verse with no text.
fn verse(verse: &Xml) -> Result<Option<Lyric>> {
    let text = match verse.text() {
        Some(text) if !text.is_empty() => text,
        _ => return Ok(None),
    };
    let mut lyric = Lyric::new(text);
    if let Some(index) = verse.get("i") {
        lyric.set_number(integer(index)? + 1);
    }
    if verse.get("hyphen") == Some("true") {
        lyric.set_syllabic(Syllabic::Begin);
    }
    Ok(Some(lyric))
}

/// `durationFromDuration`: a fraction of a whole note, its dots and its
/// tuplets.
fn duration(written: &Xml) -> Result<Duration> {
    let mut length = 0.0;
    if let Some(base) = written.get("base")
        && let Some((numerator, denominator)) = base.split_once('/')
    {
        length =
            4.0 * FloatType::from(integer(numerator)?) / FloatType::from(integer(denominator)?);
    }
    let mut duration = Duration::new(length)?;
    if let Some(dots) = written.get("dots") {
        let dots = u32::try_from(integer(dots)?).map_err(|_| error("a negative count of dots"))?;
        if let Some((value, _)) = duration.type_and_dots() {
            duration = Duration::from_type_with_dots(value, dots);
            duration.set_expression_is_inferred(true);
        }
    }
    for tuplet in children(written, "tuplet") {
        duration.append_tuplet(tuplet_of(tuplet)?);
    }
    Ok(duration)
}

/// `tupletFromTuplet`: `count` notes in the time of the power of two below
/// it. music21 reads `prolong` only where `count` says `true`, which no
/// count does.
fn tuplet_of(tuplet: &Xml) -> Result<Tuplet> {
    let mut actual = 1;
    let mut normal = 1;
    if let Some(count) = tuplet.get("count") {
        actual = u32::try_from(integer(count)?).map_err(|_| error("a negative tuplet"))?;
        while actual > normal * 2 {
            normal *= 2;
        }
    }
    if tuplet.get("prolong").is_some() && tuplet.get("count") == Some("true") {
        normal *= 2;
    }
    Ok(Tuplet::ratio(actual, normal))
}

/// `barlineListFromBarline`: a barline, or a repeat's end and start.
fn barlines(object: &Xml) -> Result<Vec<StreamElement>> {
    let Some(kind) = object.get("type") else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    if kind.starts_with("rep") {
        let (end, start) = match kind {
            "repEnd" => (true, false),
            "repBegin" => (false, true),
            "repEndBegin" => (true, true),
            _ => (false, false),
        };
        if end {
            out.push(StreamElement::Barline(Barline::repeat(
                RepeatDirection::End,
                None,
            )));
        }
        if start {
            out.push(StreamElement::Barline(Barline::repeat(
                RepeatDirection::Start,
                None,
            )));
        }
    } else {
        let bar_type = match kind {
            // music21 names this `normal`, which is no barline type it knows.
            "single" => return Err(error("cannot process style: normal")),
            "double" => BarlineType::Double,
            "end" => BarlineType::Final,
            _ => return Ok(Vec::new()),
        };
        out.push(StreamElement::Barline(Barline::new(bar_type)));
    }
    Ok(out)
}

/// A part with every clef and key signature dropped that restates the one
/// before it, as `partScoreFromSystemScore` drops them.
fn without_restatements(part: Stream) -> Stream {
    let mut last_clef: Option<Clef> = None;
    let mut last_key: Option<KeySignature> = None;
    let mut kept = Vec::new();
    for event in part.events() {
        match event.element() {
            StreamElement::Clef(clef) => {
                if last_clef.as_ref().is_some_and(|last| same_clef(last, clef)) {
                    continue;
                }
                last_clef = Some(clef.clone());
            }
            StreamElement::KeySignature(signature) => {
                // music21 compares two key signatures by what they hold.
                if last_key
                    .as_ref()
                    .is_some_and(|last| last.sharps() == signature.sharps())
                {
                    continue;
                }
                last_key = Some(signature.clone());
            }
            _ => {}
        }
        kept.push(event.clone());
    }
    part.with_events(kept)
}

/// music21's `Clef.__eq__`: the same class, sign, line and octave change.
fn same_clef(left: &Clef, right: &Clef) -> bool {
    left.kind() == right.kind()
        && left.sign() == right.sign()
        && left.line() == right.line()
        && left.octave_change() == right.octave_change()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn staff(objects: &str) -> String {
        format!(
            "<score xmlns=\"http://www.capella.de/CapXML/2.0\"><systems><system><staves>\
             <staff layout=\"S\"><voices><voice><noteObjects>{objects}</noteObjects></voice>\
             </voices></staff></staves></system></systems></score>"
        )
    }

    #[test]
    fn a_capella_octave_is_one_higher_than_music21_s() {
        let score = from_capella(&staff(
            "<chord><duration base=\"1/2\"/><heads><head pitch=\"G5\"><alter step=\"1\"/>\
             </head></heads></chord>",
        ))
        .unwrap();
        assert_eq!(score.pitches()[0].name_with_octave(), "G#4");
    }

    #[test]
    fn a_tuplet_count_takes_the_power_of_two_below_it() {
        let tuplet = Xml::parse("<tuplet count=\"5\"/>").unwrap();
        assert_eq!(tuplet_of(&tuplet).unwrap(), Tuplet::ratio(5, 4));
    }

    #[test]
    fn a_single_barline_is_refused_as_music21_refuses_it() {
        assert!(from_capella(&staff("<barline type=\"single\"/>")).is_err());
        assert!(from_capella(&staff("<barline/>")).is_ok());
    }

    #[test]
    fn a_document_with_no_systems_is_refused() {
        assert!(from_capella("<score/>").is_err());
    }
}
