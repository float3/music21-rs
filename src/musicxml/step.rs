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
//!     &mut step::MeasureContext::with_divisions(2),
//! )?;
//! assert_eq!(measure.pitches()[0].name_with_octave(), "D5");
//! # Ok::<(), music21::Error>(())
//! ```

use super::read;
pub use super::read::{
    ActiveTuplets, DirectionTypeRead, Encoding, FitAsked, MeasureFit, SpannerId, SpannerState,
    measure_fit,
};
use crate::articulations::Articulation;
use crate::bar::Barline;
use crate::clef::Clef;
use crate::defaults::IntegerType;
use crate::error::{Error, Result};
use crate::interval::Interval;
use crate::layout::{PageLayout, StaffLayout, SystemLayout};
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
pub fn read_lyric(text: &str) -> Result<Lyric> {
    read::lyric_of(&element(text, "lyric")?)
}

/// music21's `updateLyricsFromList`: `<lyric>`s of one note, one numbered
/// nought taking its place among them.
pub fn read_lyrics(texts: &[&str]) -> Result<Vec<Lyric>> {
    let read: Vec<Xml> = texts
        .iter()
        .map(|text| element(text, "lyric"))
        .collect::<Result<_>>()?;
    read::lyrics_from(read.iter())
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

/// music21's `xmlToBarline` and `xmlToRepeat`: what a `<barline>` draws,
/// a repeat in the direction written.
pub fn read_barline(text: &str) -> Result<Barline> {
    read::barline_of(&element(text, "barline")?)
}

/// music21's `xmlDirectionTypeToSpanners`: one `<wedge>`, `<bracket>`,
/// `<dashes>`, `<octave-shift>` or `<pedal>`, read within `state`, which
/// keeps the spanners from one to the next. One that ends a spanner joins
/// `last`, a uid of the state's, to it; a pedal's bounces and gaps stand
/// at `offset` on `staff`.
pub fn read_direction_type(
    text: &str,
    staff: i32,
    offset: crate::defaults::FloatType,
    last: Option<usize>,
    state: &mut SpannerState,
) -> Result<DirectionTypeRead> {
    read::direction_type_within(&Xml::parse(text)?, staff, offset, last, state)
}

/// music21's `getStaffNumber`: the staff an element is written on, nought
/// where it does not say.
pub fn read_staff(text: &str) -> Result<i32> {
    Ok(read::staff_number(&Xml::parse(text)?))
}

/// music21's `processEncoding`: an `<encoding>`.
pub fn read_encoding(text: &str) -> Result<Encoding> {
    Ok(read::encoding_of(&element(text, "encoding")?))
}

/// music21's `xmlPrintToPageLayout`: the page a `<print>` starts.
pub fn read_print_page(text: &str) -> Result<PageLayout> {
    Ok(read::print_page_layout(&element(text, "print")?))
}

/// music21's `xmlPrintToSystemLayout`: the system a `<print>` starts.
pub fn read_print_system(text: &str) -> Result<SystemLayout> {
    Ok(read::print_system_layout(&element(text, "print")?))
}

/// music21's `xmlPageLayoutToPageLayout`: a `<page-layout>`.
pub fn read_page_layout(text: &str) -> Result<PageLayout> {
    let mut read = PageLayout::default();
    read::read_page_layout(&element(text, "page-layout")?, &mut read);
    Ok(read)
}

/// music21's `xmlSystemLayoutToSystemLayout`: a `<system-layout>`.
pub fn read_system_layout(text: &str) -> Result<SystemLayout> {
    let mut read = SystemLayout::default();
    read::read_system_layout(&element(text, "system-layout")?, &mut read);
    Ok(read)
}

/// music21's `xmlStaffLayoutToStaffLayout`: a `<staff-layout>`.
pub fn read_staff_layout(text: &str) -> Result<StaffLayout> {
    Ok(read::staff_layout(&element(text, "staff-layout")?))
}

/// What a `<measure>` read on its own is read within: what music21's
/// `MeasureParser` takes from the measures before it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeasureContext {
    /// The divisions to the quarter, where the measure sets none.
    pub divisions: Option<IntegerType>,
    /// The tuplets it starts inside, which reading leaves as it ends.
    pub tuplets: ActiveTuplets,
}

impl MeasureContext {
    /// A measure read with `divisions` to the quarter, inside no tuplet.
    pub fn with_divisions(divisions: IntegerType) -> Self {
        Self {
            divisions: Some(divisions),
            ..Self::default()
        }
    }
}

/// music21's `MeasureParser.parse`: a `<measure>`, read within `context`,
/// whose tuplets it leaves as the measure ends.
pub fn read_measure(text: &str, context: &mut MeasureContext) -> Result<Stream> {
    let mut measure = element(text, "measure")?;
    if let Some(divisions) = context.divisions {
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
    let score = read::score_within(&root, &mut context.tuplets)?;
    let measure = score
        .parts()
        .first()
        .and_then(|part| part.measures().first().map(|measure| (*measure).clone()));
    measure.ok_or_else(|| Error::music_xml("a measure read to nothing"))
}

/// A part read, and what music21 warns of as it reads it.
#[derive(Clone, Debug)]
pub struct PartRead {
    /// The part, or a part for each staff where it is written on several.
    pub parts: Vec<Stream>,
    /// What music21 warns of, in order: a percussion key outside the
    /// General MIDI map, which is read as a bare unpitched instrument.
    pub warnings: Vec<Error>,
}

/// music21's `PartParser.parse`: a `<part>` with its `<score-part>`.
pub fn read_part(part: &str, score_part: Option<&str>) -> Result<PartRead> {
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
    let (score, warnings) = read::score_warned(&root)?;
    Ok(PartRead {
        parts: score.parts().into_iter().cloned().collect(),
        warnings,
    })
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
            &mut MeasureContext::with_divisions(10080),
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
        assert_eq!(parts.parts.len(), 2);
    }

    #[test]
    fn an_encoding_says_who_wrote_it_and_what_it_marks() {
        let read = read_encoding(
            "<encoding><software>Finale v26.3 for Mac</software>\
             <supports attribute=\"new-system\" element=\"print\" type=\"yes\" value=\"no\"/>\
             <supports attribute=\"new-page\" element=\"print\" type=\"yes\"/></encoding>",
        )
        .unwrap();
        assert!(read.finale);
        assert!(!read.system_breaks);
        assert!(read.page_breaks);
        assert_eq!(read.software, vec!["Finale v26.3 for Mac".to_string()]);
    }

    #[test]
    fn a_print_starts_a_page_and_a_system() {
        let print = "<print new-page=\"yes\" page-number=\"5\"><page-layout>\
                     <page-height>4000</page-height></page-layout>\
                     <system-layout><system-distance>55</system-distance></system-layout></print>";
        let page = read_print_page(print).unwrap();
        assert_eq!(page.is_new, Some(true));
        assert_eq!(page.page_number, Some(5));
        let system = read_print_system(print).unwrap();
        assert_eq!(system.is_new, None);
        assert!(system.distance.is_some());
    }

    #[test]
    fn a_tuplet_started_in_one_reading_stops_in_the_next() {
        let note = |notations: &str| {
            format!(
                "<measure><note><pitch><step>C</step><octave>4</octave></pitch>\
                 <duration>1</duration><type>eighth</type><time-modification>\
                 <actual-notes>3</actual-notes><normal-notes>2</normal-notes>\
                 </time-modification>{notations}</note></measure>"
            )
        };
        let mut context = MeasureContext::with_divisions(3);
        let tuplet_type = |measure: &Stream| {
            measure.notes()[0].1.duration().expect("a note").tuplets()[0].tuplet_type()
        };
        let started = read_measure(
            &note("<notations><tuplet type=\"start\"/></notations>"),
            &mut context,
        )
        .unwrap();
        assert_eq!(
            tuplet_type(&started),
            Some(crate::duration::TupletType::Start)
        );
        let stopped = read_measure(
            &note("<notations><tuplet type=\"stop\"/></notations>"),
            &mut context,
        )
        .unwrap();
        assert_eq!(
            tuplet_type(&stopped),
            Some(crate::duration::TupletType::Stop)
        );
    }

    #[test]
    fn a_percussion_key_outside_the_map_is_warned_of() {
        let score_part = "<score-part><midi-instrument id=\"I1\"><midi-unpitched>23</midi-unpitched>\
                          </midi-instrument></score-part>";
        let read = read_part("<part><measure/></part>", Some(score_part)).unwrap();
        assert_eq!(read.warnings.len(), 1);
        assert!(
            read.warnings[0]
                .to_string()
                .contains("22 does not map to a valid instrument!")
        );
    }

    #[test]
    fn a_spanner_begun_in_one_reading_ends_in_the_next() {
        let mut state = SpannerState::default();
        let note = state.new_uid();
        let begun = read_direction_type(
            "<wedge type=\"crescendo\" number=\"2\"/>",
            0,
            0.0,
            Some(note),
            &mut state,
        )
        .unwrap();
        assert_eq!(begun.started.len(), 1);
        let wedge = begun.started[0];
        assert!(!state.spanner(wedge).unwrap().complete());
        let ended = read_direction_type(
            "<wedge type=\"stop\" number=\"2\"/>",
            0,
            0.0,
            Some(note),
            &mut state,
        )
        .unwrap();
        assert!(ended.started.is_empty());
        assert!(state.spanner(wedge).unwrap().complete());
        assert_eq!(state.joined(wedge), &[note]);
    }

    #[test]
    fn a_pedal_puts_its_bounces_and_gaps_where_they_are_read() {
        let mut state = SpannerState::default();
        let pedal = |text: &str, offset, state: &mut SpannerState| {
            read_direction_type(text, 1, offset, None, state).unwrap()
        };
        let begun = pedal(
            "<pedal type=\"sostenuto\" sign=\"yes\" number=\"2\"/>",
            0.5,
            &mut state,
        );
        assert_eq!(begun.started.len(), 1);
        assert!(
            pedal(
                "<pedal type=\"resume\" line=\"yes\" number=\"2\"/>",
                0.5,
                &mut state
            )
            .marks
            .is_empty()
        );
        let bounce = pedal(
            "<pedal type=\"change\" line=\"yes\" number=\"2\"/>",
            1.0,
            &mut state,
        );
        assert_eq!(bounce.marks.len(), 1);
        assert_eq!(bounce.marks[0].1, 1.0);
        assert_eq!(state.joined(begun.started[0]), &[bounce.marks[0].0]);
    }

    #[test]
    fn a_staff_is_read_off_a_note() {
        assert_eq!(read_staff("<note><staff>2</staff></note>").unwrap(), 2);
        assert_eq!(read_staff("<note/>").unwrap(), 0);
    }
}
