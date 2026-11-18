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
    ActiveTuplets, DuplicateIds, Encoding, FitAsked, MeasureFit, SpannerId, SpannerState,
    SpannerStep, SpannersRead, measure_fit,
};
use crate::articulations::Articulation;
use crate::bar::Barline;
use crate::clef::Clef;
use crate::defaults::IntegerType;
use crate::error::{Error, Result};
use crate::interval::Interval;
use crate::key::KeySignature;
use crate::layout::{PageLayout, StaffLayout, SystemLayout};
use crate::metadata::MetadataValue;
use crate::meter::TimeSignature;
use crate::notation::{Beams, Lyric, Tie};
use crate::pitch::{Accidental, Pitch};
use crate::stream::{Stream, StreamElement};
use crate::style::StreamStyle;
use crate::text::TextBox;
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

/// music21's `updateVoiceInformation`: the voices a `<measure>`'s notes
/// and forwards are written in, sorted.
pub fn read_voice_ids(text: &str) -> Result<Vec<String>> {
    Ok(read::voice_ids(&element(text, "measure")?))
}

/// music21's `nonTraditionalKeySignature`: a `<key>` of steps and their
/// alterations, its octaves read too.
pub fn read_non_traditional_key(text: &str) -> Result<KeySignature> {
    let written = element(text, "key")?;
    let mut signature = read::non_traditional_key(&written)?;
    read::key_octaves(&written, &mut signature)?;
    Ok(signature)
}

/// music21's `mxKeyOctaves`: the octaves a `<key>`'s `<key-octave>`s put
/// `signature`'s altered pitches in.
pub fn read_key_octaves(text: &str, signature: &mut KeySignature) -> Result<()> {
    read::key_octaves(&element(text, "key")?, signature)
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
/// `None` for one music21 does not know; a bend's release is read in
/// `divisions` to the quarter.
pub fn read_technical(
    text: &str,
    divisions: crate::defaults::FloatType,
) -> Result<Option<Articulation>> {
    Ok(read::technical_of(&Xml::parse(text)?, divisions))
}

/// music21's `xmlToBarline` and `xmlToRepeat`: what a `<barline>` draws,
/// a repeat in the direction written.
pub fn read_barline(text: &str) -> Result<Barline> {
    read::barline_of(&element(text, "barline")?)
}

/// One of music21's spanner-reading steps on an element -- a direction
/// type, a `<notations>`, or one slur or other spanner mark -- within
/// `state`, which keeps the spanners from one to the next. `target`, a
/// uid of the state's, is the note it joins or ends a spanner with.
pub fn read_spanners(
    text: &str,
    target: Option<usize>,
    step: SpannerStep,
    state: &mut SpannerState,
) -> Result<SpannersRead> {
    read::spanners_within(&Xml::parse(text)?, target, step, state)
}

/// music21's `creatorToContributor`: a `<creator>`, its type its role.
pub fn read_creator(text: &str) -> Result<MetadataValue> {
    Ok(read::creator_value(&element(text, "creator")?))
}

/// music21's `rightsToCopyright`: a `<rights>`, its type its role.
pub fn read_rights(text: &str) -> Result<MetadataValue> {
    Ok(read::rights_value(&element(text, "rights")?))
}

/// music21's `xmlCreditToTextBox`: a `<credit>`.
pub fn read_credit(text: &str) -> Result<TextBox> {
    read::text_box(&element(text, "credit")?)
}

/// music21's `styleFromXmlDefaults`: the appearance, fonts and lyric
/// languages a `<defaults>` gives.
pub fn read_defaults_style(text: &str) -> Result<StreamStyle> {
    read::stream_style(&element(text, "defaults")?)
}

/// music21's `xmlAppearanceToStyle`: an `<appearance>`.
pub fn read_appearance(text: &str) -> Result<StreamStyle> {
    let mut style = StreamStyle::default();
    read::read_appearance(&element(text, "appearance")?, &mut style);
    Ok(style)
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
        let begun = read_spanners(
            "<wedge type=\"crescendo\" number=\"2\"/>",
            Some(note),
            SpannerStep::DirectionType {
                staff: 0,
                offset: 0.0,
            },
            &mut state,
        )
        .unwrap();
        assert_eq!(begun.started.len(), 1);
        let wedge = begun.started[0];
        assert!(!state.spanner(wedge).unwrap().complete());
        let ended = read_spanners(
            "<wedge type=\"stop\" number=\"2\"/>",
            Some(note),
            SpannerStep::DirectionType {
                staff: 0,
                offset: 0.0,
            },
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
            read_spanners(
                text,
                None,
                SpannerStep::DirectionType { staff: 1, offset },
                state,
            )
            .unwrap()
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
    fn defaults_give_a_scores_appearance_fonts_and_lyric_languages() {
        let style = read_defaults_style(
            "<defaults><appearance><line-width type=\"beam\">5</line-width>\
             <other-appearance type=\"sharps\">dotted</other-appearance></appearance>\
             <music-font font-family=\"Maestro, Opus\" font-weight=\"bold\"/>\
             <lyric-language name=\"verse\" xml:lang=\"fr\"/>\
             <lyric-language name=\"chorus\"/></defaults>",
        )
        .unwrap();
        assert_eq!(style.line_widths[0].0, "beam");
        assert_eq!(
            style.other_appearances[0].1,
            crate::style::StyleValue::Text("dotted".to_string())
        );
        let font = style.music_font.unwrap();
        assert_eq!(
            font.font_family,
            Some(vec!["Maestro".to_string(), "Opus".to_string()])
        );
        assert_eq!(
            style.lyric_languages,
            vec![
                (Some("verse".to_string()), Some("fr".to_string())),
                (Some("chorus".to_string()), Some("en".to_string())),
            ]
        );
    }

    #[test]
    fn a_bend_and_a_harmonic_say_how_they_are_played() {
        let bend = read_technical(
            "<bend><bend-alter>-1</bend-alter><pre-bend/><with-bar>dip</with-bar>\
             <release offset=\"2\"/></bend>",
            4.0,
        )
        .unwrap()
        .unwrap();
        assert_eq!(bend.bend_alter().map(|alter| alter.semitones()), Some(-1.0));
        assert!(bend.pre_bend());
        assert_eq!(bend.with_bar(), Some("dip"));
        assert_eq!(bend.release(), Some(0.5));
        let harmonic = read_technical("<harmonic><artificial/><sounding-pitch/></harmonic>", 1.0)
            .unwrap()
            .unwrap();
        assert_eq!(harmonic.harmonic_type(), Some("artificial"));
        assert_eq!(harmonic.pitch_type(), Some("sounding"));
    }

    #[test]
    fn a_keys_octaves_place_its_altered_pitches() {
        let mut signature = KeySignature::new(-4);
        read_key_octaves(
            "<key><fifths>-4</fifths><key-octave number=\"1\">3</key-octave>\
             <key-octave number=\"4\">3</key-octave></key>",
            &mut signature,
        )
        .unwrap();
        let names: Vec<String> = signature
            .altered_pitches()
            .unwrap()
            .iter()
            .map(Pitch::name_with_octave)
            .collect();
        assert_eq!(names, ["Bb3", "Eb", "Ab", "Db3"]);
        let sharp = read_non_traditional_key(
            "<key><key-step>G</key-step><key-alter>1</key-alter>\
             <key-accidental>sharp</key-accidental></key>",
        )
        .unwrap();
        assert_eq!(sharp.altered_pitches().unwrap()[0].name(), "G#");
    }

    #[test]
    fn a_slur_begun_on_one_note_ends_on_another() {
        let mut state = SpannerState::default();
        let (first, second) = (state.new_uid(), state.new_uid());
        let begun = read_spanners(
            "<notations><slur type=\"start\" number=\"1\"/></notations>",
            Some(first),
            SpannerStep::Notations,
            &mut state,
        )
        .unwrap();
        assert_eq!(begun.started.len(), 1);
        let one = SpannerStep::One {
            kind: crate::spanner::SpannerKind::Slur,
            duplicates: DuplicateIds::Join,
        };
        let ended = read_spanners(
            "<slur type=\"stop\" number=\"1\"/>",
            Some(second),
            one,
            &mut state,
        )
        .unwrap();
        assert_eq!(ended.returned, begun.started);
        assert!(ended.started.is_empty());
        assert_eq!(state.joined(begun.started[0]), &[first, second]);
        assert!(state.spanner(begun.started[0]).unwrap().complete());
    }

    #[test]
    fn a_staff_is_read_off_a_note() {
        assert_eq!(read_staff("<note><staff>2</staff></note>").unwrap(), 2);
        assert_eq!(read_staff("<note/>").unwrap(), 0);
    }
}
