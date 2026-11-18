//! One element written alone: what music21's `MeasureExporter` methods
//! (`noteToXml`, `chordToXml`, `clefToXml`, `dynamicToXml` and the rest)
//! hand back for an element given to them outside any score.

use std::collections::HashMap;

use super::tree::Element;
use super::{
    MeasureExporter, PartContext, barline_element, clef_element, key_element,
    time_signature_element,
};
use crate::defaults::FloatType;
use crate::error::Result;
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
/// and a barline is its `<barline>` with no location. What a measure
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
