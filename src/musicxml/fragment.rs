//! One element written alone: what music21's `MeasureExporter` methods
//! (`noteToXml`, `chordToXml`, `clefToXml`, `dynamicToXml` and the rest)
//! hand back for an element given to them outside any score.

use std::collections::HashMap;

use super::tree::Element;
use super::{
    ExportOptions, MeasureExporter, PartContext, barline_element, clef_element, credit_element,
    key_element, midi_instrument, page_layout_element, score_element, score_instrument,
    staff_details_element, staff_layout_element, system_layout_element, time_signature_element,
    transpose_element,
};
use crate::defaults::FloatType;
use crate::error::Result;
use crate::instrument::Instrument;
use crate::interval::Interval;
use crate::layout::Layout;
use crate::stream::{Stream, StreamElement, StreamKind};

/// The MusicXML music21's `MeasureExporter` writes for one element, each
/// top-level element as a document of its own.
///
/// The element stands at `offset` in `measure`, whose meter and clefs are
/// what decide, as they do for music21, whether a rest fills its bar and
/// where it is displayed; with no measure it stands alone in one of its
/// own. The writer has written nothing of the measure before it.
///
/// A note is one `<note>`, a chord a `<note>` for each of its pitches, a
/// dynamic or a text mark a `<direction>`; a clef, a key or a meter is the
/// `<clef>`, `<key>` or `<time>` that goes in a measure's `<attributes>`,
/// a barline is its `<barline>` with no location, and a text box the
/// `<credit>` a score writes for it. What a measure
/// writes nothing for, such as an instrument, writes nothing here.
///
/// ```
/// use music21::{Duration, Note, StreamElement};
/// use music21::musicxml::element_xml;
///
/// let note = Note::from_name("D#4")?.with_duration(Duration::half());
/// let written = element_xml(&StreamElement::from(note), None, 0.0)?;
/// assert_eq!(written.len(), 1);
/// assert!(written[0].starts_with("<note>"));
/// assert!(written[0].contains("<type>half</type>"));
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// What the measure writer refuses for the element: a duration no single
/// note value writes, a stream, a colour CSS has no name for.
pub fn element_xml(
    element: &StreamElement,
    measure: Option<&Stream>,
    offset: FloatType,
) -> Result<Vec<String>> {
    // The marks a measure writes into its `<attributes>`, alone.
    match element {
        StreamElement::Clef(clef) => return Ok(vec![clef_element(clef)?.dump()]),
        StreamElement::TimeSignature(meter) => {
            return Ok(vec![time_signature_element(meter)?.dump()]);
        }
        StreamElement::Key(_) | StreamElement::KeySignature(_) => {
            return key_element(element)
                .transpose()
                .map(|key| key.iter().map(Element::dump).collect());
        }
        StreamElement::TextBox(text) => return Ok(vec![credit_element(text)?.dump()]),
        StreamElement::Barline(barline) => {
            let mut written = barline_element(Some(barline), "", None);
            written.remove_attribute("location");
            return Ok(vec![written.dump()]);
        }
        _ => {}
    }

    // Everything else, as a measure with nothing else in it writes it.
    let alone = Stream::with_kind(StreamKind::Measure);
    let measure = measure.unwrap_or(&alone);
    let own_clefs = measure
        .events()
        .iter()
        .filter_map(|event| match event.element() {
            StreamElement::Clef(clef) => Some((event.offset(), clef)),
            _ => None,
        })
        .collect();
    let voice_bases = HashMap::new();
    let mut context = PartContext {
        instruments: &[],
        own_instruments: Vec::new(),
        own_clefs,
        meter: None,
        last_divisions: None,
        spanners: Vec::new(),
        clef: None,
        previous: None,
        voice_bases: &voice_bases,
        modulations: Vec::new(),
    };
    let mut exporter = MeasureExporter::new(measure, 0.0, &mut context);
    exporter.write_element(offset, element, element as *const StreamElement)?;
    Ok(exporter.root.elements().map(Element::dump).collect())
}

/// What music21's `ScoreExporter` writes before a score's parts, element
/// by element: the work and movement titles, `<identification>`,
/// `<defaults>`, a `<credit>` for each text box and the `<part-list>`.
///
/// A score with nothing in it is written as a part of one empty measure,
/// rather than as the blank page [`super::to_musicxml`] writes for it, so
/// that what is written is what the score and `options` say.
///
/// ```
/// use music21::{Stream, StreamKind};
/// use music21::musicxml::{ExportOptions, score_header_xml};
///
/// let header = score_header_xml(&Stream::with_kind(StreamKind::Score), &ExportOptions::default())?;
/// assert!(header.iter().any(|element| element.starts_with("<identification>")));
/// assert!(header.last().unwrap().starts_with("<part-list>"));
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// What writing the score refuses.
pub fn score_header_xml(score: &Stream, options: &ExportOptions) -> Result<Vec<String>> {
    let placeholder;
    let score = if score.is_empty() {
        let mut measure = Stream::with_kind(StreamKind::Measure);
        measure.set_number(1);
        let mut part = Stream::with_kind(StreamKind::Part);
        part.push(measure);
        let mut filled = Stream::with_kind(score.kind());
        filled.set_metadata(score.metadata().cloned());
        filled.push(part);
        placeholder = filled;
        &placeholder
    } else {
        score
    };
    let root = score_element(score, options)?;
    Ok(root
        .elements()
        .filter(|element| element.tag() != "part")
        .map(Element::dump)
        .collect())
}

/// What music21's `instrumentToXmlScoreInstrument` and
/// `instrumentToXmlMidiInstrument` write for an instrument: its
/// `<score-instrument>` and its `<midi-instrument>`.
///
/// ```
/// use music21::Instrument;
/// use music21::musicxml::instrument_xml;
///
/// let (score, midi) = instrument_xml(&Instrument::new());
/// assert!(score.starts_with("<score-instrument"));
/// assert!(midi.starts_with("<midi-instrument"));
/// ```
pub fn instrument_xml(instrument: &Instrument) -> (String, String) {
    (
        score_instrument(instrument).dump(),
        midi_instrument(instrument).dump(),
    )
}

/// What music21's `intervalToXmlTranspose` writes for an interval a part
/// is transposed by: its `<transpose>`.
///
/// ```
/// use music21::Interval;
/// use music21::musicxml::transpose_xml;
///
/// let written = transpose_xml(&Interval::from_name("-M2")?);
/// assert!(written.contains("<diatonic>-1</diatonic>"));
/// assert!(written.contains("<chromatic>-2</chromatic>"));
/// # Ok::<(), music21::Error>(())
/// ```
pub fn transpose_xml(interval: &Interval) -> String {
    transpose_element(interval).dump()
}

/// The element music21 writes a page, system or staff layout as:
/// `<page-layout>`, `<system-layout>` or `<staff-layout>`, as
/// `pageLayoutToXmlPageLayout` and its neighbours write them. `None` for
/// a score layout, which is written as the parts of `<defaults>`.
///
/// ```
/// use music21::layout::{Layout, StaffLayout};
/// use music21::musicxml::layout_xml;
///
/// let written = layout_xml(&Layout::Staff(StaffLayout::default()));
/// assert_eq!(written.as_deref(), Some("<staff-layout />"));
/// ```
pub fn layout_xml(layout: &Layout) -> Option<String> {
    let element = match layout {
        Layout::Page(page) => page_layout_element(page),
        Layout::System(system) => system_layout_element(system),
        Layout::Staff(staff) => staff_layout_element(staff),
        Layout::Score(_) => return None,
    };
    Some(element.dump())
}

/// What music21's `staffLayoutToXmlStaffDetails` writes for a staff
/// layout: its `<staff-details>`.
///
/// ```
/// use music21::layout::StaffLayout;
/// use music21::musicxml::staff_details_xml;
///
/// assert!(staff_details_xml(&StaffLayout::default()).starts_with("<staff-details"));
/// ```
pub fn staff_details_xml(staff: &crate::layout::StaffLayout) -> String {
    staff_details_element(staff).dump()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chord::Chord;
    use crate::clef::Clef;
    use crate::duration::Duration;
    use crate::instrument::Instrument;

    #[test]
    fn a_chord_is_a_note_for_each_pitch() {
        let chord = Chord::new("C4 E4 G4").unwrap();
        let written = element_xml(&StreamElement::from(chord), None, 0.0).unwrap();
        assert_eq!(written.len(), 3);
        assert!(written[1].contains("<chord />"));
    }

    #[test]
    fn a_clef_is_the_clef_alone() {
        let written = element_xml(&StreamElement::from(Clef::treble()), None, 0.0).unwrap();
        assert_eq!(
            written,
            ["<clef>\n  <sign>G</sign>\n  <line>2</line>\n</clef>"]
        );
    }

    /// music21's `restToXml` docstring: the measure's meter decides.
    #[test]
    fn a_rest_filling_its_measure_is_a_measure_rest() {
        let mut measure = Stream::with_kind(StreamKind::Measure);
        measure.insert(0.0, crate::meter::TimeSignature::new(2, 4).unwrap());
        let rest = StreamElement::from(crate::rest::Rest::new(Duration::half()));
        let written = element_xml(&rest, Some(&measure), 0.0).unwrap();
        assert!(
            written[0].contains("<rest measure=\"yes\" />"),
            "{}",
            written[0]
        );
    }

    /// music21's `dynamicToXml` docstring: the style moves the mark.
    #[test]
    fn a_dynamic_is_written_where_its_style_puts_it() {
        let mut dynamic = crate::dynamics::Dynamic::new("ppp");
        let mut style = dynamic.style().clone();
        style.relative_y = Some(crate::style::StyleValue::Whole(-10));
        dynamic.set_style(style);
        let written = element_xml(&StreamElement::from(dynamic), None, 0.0).unwrap();
        assert!(
            written[0]
                .contains("<dynamics default-x=\"-36\" default-y=\"-80\" relative-y=\"-10\">"),
            "{}",
            written[0]
        );
    }

    /// music21's `keySignatureToXml` docstring: a mode set by hand.
    #[test]
    fn a_signature_given_a_mode_writes_it() {
        let mut signature = crate::key::KeySignature::new(-3);
        signature.set_mode(Some("major".to_string()));
        let written = element_xml(&StreamElement::from(signature), None, 0.0).unwrap();
        assert_eq!(
            written,
            ["<key>\n  <fifths>-3</fifths>\n  <mode>major</mode>\n</key>"]
        );
    }

    #[test]
    fn an_instrument_writes_nothing() {
        let written = element_xml(&StreamElement::from(Instrument::new()), None, 0.0).unwrap();
        assert!(written.is_empty());
    }

    #[test]
    fn a_length_no_value_writes_is_refused() {
        let note = crate::note::Note::from_name("C4")
            .unwrap()
            .with_duration(Duration::new(2.5).unwrap());
        assert!(element_xml(&StreamElement::from(note), None, 0.0).is_err());
    }
}
