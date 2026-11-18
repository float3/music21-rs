//! music21's per-element steps of reading MusicXML: what its
//! `MeasureParser` makes of one `<clef>`, one `<note>`, one `<measure>`.
//!
//! Each takes the element's text and reads it as the whole-document reader
//! reads it in place. A step that depends on what came before it in the
//! measure -- a note's duration on the divisions in force, a rest on the
//! measure it fills -- reads the element inside a measure of its own, so
//! that the reader's own measure logic decides it.
//!
//! ```
//! use music21::musicxml::step;
//!
//! let clef = step::read_clef("<clef><sign>F</sign><line>4</line></clef>")?;
//! assert_eq!(clef.sign(), Some("F"));
//!
//! let measure = step::read_measure(
//!     "<measure><note><pitch><step>D</step><octave>5</octave></pitch>\
//!      <duration>2</duration><type>half</type></note></measure>",
//!     Some(2),
//! )?;
//! assert_eq!(measure.pitches()[0].name_with_octave(), "D5");
//! # Ok::<(), music21::Error>(())
//! ```

use super::read;
use crate::articulations::Articulation;
use crate::clef::Clef;
use crate::defaults::IntegerType;
use crate::error::{Error, Result};
use crate::interval::Interval;
use crate::meter::TimeSignature;
use crate::notation::{Beams, Lyric, Tie};
use crate::pitch::{Accidental, Pitch};
use crate::stream::{Stream, StreamElement};
use crate::xml::Xml;

/// The score a lone measure or part is read inside: one part, with no name.
const SCORE_SHELL: &str = "<score-partwise><part-list><score-part id=\"P1\"/></part-list>\
                           <part id=\"P1\"/></score-partwise>";

/// An element's text read as XML, refused unless its tag is `tag`.
fn element(text: &str, tag: &str) -> Result<Xml> {
    let read = Xml::parse(text)?;
    if read.tag != tag {
        return Err(Error::music_xml(format!(
            "expected a <{tag}>, not a <{}>",
            read.tag
        )));
    }
    Ok(read)
}

/// music21's `xmlToClef`: a `<clef>`.
pub fn read_clef(text: &str) -> Result<Clef> {
    read::clef_of(&element(text, "clef")?)
}

/// music21's `xmlToKeySignature`: a `<key>`, as a key signature, or a key
/// where it names a mode.
pub fn read_key(text: &str) -> Result<StreamElement> {
    read::key_signature(&element(text, "key")?)
}

/// music21's `xmlToTimeSignature`: a `<time>`; `None` for one that only
/// hides the meter.
pub fn read_time(text: &str) -> Result<Option<TimeSignature>> {
    read::time_signature(&element(text, "time")?)
}

/// music21's `xmlTransposeToInterval`: a `<transpose>`.
pub fn read_transpose(text: &str) -> Result<Interval> {
    read::transpose_interval(&element(text, "transpose")?)
}

/// music21's `xmlToTempoIndication`: a `<metronome>`.
pub fn read_metronome(text: &str) -> Result<StreamElement> {
    read::metronome(&element(text, "metronome")?, None)
}

/// music21's `xmlToPitch`: the pitch of a `<note>`, C4 where it has none.
pub fn read_pitch(text: &str) -> Result<Pitch> {
    read::pitch_of(&element(text, "note")?)
}

/// music21's `xmlToAccidental`: an `<accidental>`, or an alteration
/// written by another tag's text.
pub fn read_accidental(text: &str) -> Result<Accidental> {
    read::accidental_of(&Xml::parse(text)?)
}

/// music21's `xmlToTie`: the tie of a `<note>`, where it has one.
pub fn read_tie(text: &str) -> Result<Option<Tie>> {
    Ok(read::tie_of(&element(text, "note")?))
}

/// music21's `xmlToBeams`: `<beam>`s, in order.
pub fn read_beams(texts: &[&str]) -> Result<Beams> {
    let read: Vec<Xml> = texts
        .iter()
        .map(|text| element(text, "beam"))
        .collect::<Result<_>>()?;
    let written: Vec<&Xml> = read.iter().collect();
    read::beams_of(&written)
}

/// music21's `xmlToLyric`: a `<lyric>`.
pub fn read_lyric(text: &str) -> Result<Option<Lyric>> {
    let read = element(text, "lyric")?;
    Ok(read::lyrics_from(std::iter::once(&read))?.pop())
}

/// music21's `xmlToArticulation`: one mark of `<articulations>`, `None`
/// for one music21 does not know.
pub fn read_articulation(text: &str) -> Result<Option<Articulation>> {
    Ok(read::articulation_of(&Xml::parse(text)?))
}

/// music21's `xmlTechnicalToArticulation`: one mark of `<technical>`,
/// `None` for one music21 does not know.
pub fn read_technical(text: &str) -> Result<Option<Articulation>> {
    Ok(read::technical_of(&Xml::parse(text)?))
}

/// music21's `getStaffNumber`: the staff an element is written on, nought
/// where it does not say.
pub fn read_staff(text: &str) -> Result<i32> {
    Ok(read::staff_number(&Xml::parse(text)?))
}

/// music21's `MeasureParser.parse`: a `<measure>`, read with `divisions`
/// to the quarter where it sets none of its own.
pub fn read_measure(text: &str, divisions: Option<IntegerType>) -> Result<Stream> {
    let mut measure = element(text, "measure")?;
    if let Some(divisions) = divisions {
        let attributes = Xml::parse(&format!(
            "<attributes><divisions>{divisions}</divisions></attributes>"
        ))?;
        measure.children.insert(0, attributes);
    }

    // Read as the only measure of a score's only part.
    let mut root = Xml::parse(SCORE_SHELL)?;
    if let Some(part) = root.children.iter_mut().find(|child| child.tag == "part") {
        part.children.push(measure);
    }
    let score = read::score_of(&root)?;
    let measure = score
        .parts()
        .first()
        .and_then(|part| part.measures().first().map(|measure| (*measure).clone()));
    measure.ok_or_else(|| Error::music_xml("a measure read to nothing"))
}

/// music21's `PartParser.parse`: a `<part>` with its `<score-part>`, as
/// the part, or a part for each staff where it is written on several.
pub fn read_part(part: &str, score_part: Option<&str>) -> Result<Vec<Stream>> {
    let part = element(part, "part")?;
    let id = part.get("id").unwrap_or("P1").to_string();
    // The score part is the part's, whatever id it was written with.
    let mut score_part = match score_part {
        Some(text) => element(text, "score-part")?,
        None => Xml::parse("<score-part/>")?,
    };
    score_part.set("id", &id);
    let mut root = Xml::parse(&format!(
        "<score-partwise><part-list/><part id=\"{id}\"/></score-partwise>"
    ))?;
    root.children[0].children.push(score_part);
    root.children[1] = part;
    let score = read::score_of(&root)?;
    Ok(score.parts().into_iter().cloned().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clef_is_read_as_music21_reads_it() {
        let clef = read_clef(
            "<clef><sign>G</sign><line>2</line><clef-octave-change>-1</clef-octave-change></clef>",
        )
        .unwrap();
        assert_eq!(clef.octave_change(), -1);
    }

    #[test]
    fn an_element_of_another_tag_is_refused() {
        assert!(read_clef("<key><fifths>2</fifths></key>").is_err());
    }

    #[test]
    fn a_measure_takes_the_divisions_given() {
        let measure = read_measure(
            "<measure><note><rest/><duration>40320</duration></note></measure>",
            Some(10080),
        )
        .unwrap();
        assert_eq!(measure.end_offset(), 4.0);
    }

    #[test]
    fn a_part_on_two_staves_is_two_parts() {
        let parts = read_part(
            "<part id=\"P2\"><measure number=\"1\"><attributes><divisions>1</divisions>\
             <staves>2</staves></attributes>\
             <note><pitch><step>C</step><octave>5</octave></pitch><duration>4</duration>\
             <staff>1</staff></note><backup><duration>4</duration></backup>\
             <note><pitch><step>C</step><octave>3</octave></pitch><duration>4</duration>\
             <staff>2</staff></note></measure></part>",
            None,
        )
        .unwrap();
        assert_eq!(parts.len(), 2);
    }

    #[test]
    fn a_staff_is_read_off_a_note() {
        assert_eq!(read_staff("<note><staff>2</staff></note>").unwrap(), 2);
        assert_eq!(read_staff("<note/>").unwrap(), 0);
    }
}
