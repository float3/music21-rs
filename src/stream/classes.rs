//! music21's class names for the elements of a stream: what music21's
//! `classes` lists, the most particular first, for the questions that ask
//! for elements by class.

use super::{StreamElement, StreamKind};

impl StreamElement {
    /// music21's class names of the element, the most particular first,
    /// ending with `Music21Object`.
    pub fn music21_classes(&self) -> Vec<&'static str> {
        let mut names: Vec<&'static str> = match self {
            StreamElement::Note(_) => vec!["Note", "NotRest", "GeneralNote"],
            StreamElement::Unpitched(_) => vec!["Unpitched", "NotRest", "GeneralNote"],
            StreamElement::Chord(chord) if chord.numeral().is_some() => vec![
                "RomanNumeral",
                "Harmony",
                "Chord",
                "ChordBase",
                "NotRest",
                "GeneralNote",
            ],
            StreamElement::Chord(_) => vec!["Chord", "ChordBase", "NotRest", "GeneralNote"],
            StreamElement::PercussionChord(_) => {
                vec!["PercussionChord", "ChordBase", "NotRest", "GeneralNote"]
            }
            StreamElement::ChordSymbol(symbol) => {
                let mut names = vec![
                    "ChordSymbol",
                    "Harmony",
                    "Chord",
                    "ChordBase",
                    "NotRest",
                    "GeneralNote",
                ];
                if symbol.is_no_chord() {
                    names.insert(0, "NoChord");
                }
                names
            }
            StreamElement::Rest(_) => vec!["Rest", "GeneralNote"],
            StreamElement::Stream(stream) => match stream.kind() {
                StreamKind::Measure => vec!["Measure", "Stream", "StreamCore"],
                StreamKind::Voice => vec!["Voice", "Stream", "StreamCore"],
                StreamKind::Part => vec!["Part", "Stream", "StreamCore"],
                StreamKind::PartStaff => vec!["PartStaff", "Part", "Stream", "StreamCore"],
                StreamKind::Score => vec!["Score", "Stream", "StreamCore"],
                StreamKind::Opus => vec!["Opus", "Stream", "StreamCore"],
                StreamKind::Stream => vec!["Stream", "StreamCore"],
            },
            StreamElement::Clef(clef) => {
                let kind = clef.kind();
                let mut names = vec![kind.class_name()];
                names.extend(kind.parents().iter().copied());
                if !names.contains(&"Clef") {
                    names.push("Clef");
                }
                names
            }
            StreamElement::Instrument(instrument) => {
                // An instrument is of its own class and of every family
                // above it: a flute is a `WoodwindInstrument`.
                let mut names: Vec<&'static str> = Vec::new();
                if let Some(class) = instrument.class_name() {
                    names.push(class);
                }
                names.extend(instrument.families().iter().copied());
                if !names.contains(&"Instrument") {
                    names.push("Instrument");
                }
                names
            }
            StreamElement::KeySignature(_) => vec!["KeySignature"],
            // music21's `Key` is a `DiatonicScale`, which is where
            // everything it answers about degrees comes from.
            StreamElement::Key(_) => vec![
                "Key",
                "KeySignature",
                "DiatonicScale",
                "ConcreteScale",
                "Scale",
            ],
            StreamElement::TimeSignature(_) => vec!["TimeSignature", "TimeSignatureBase"],
            StreamElement::MetronomeMark(_) => vec!["MetronomeMark", "TempoIndication"],
            StreamElement::TempoText(_) => vec!["TempoText", "TempoIndication"],
            StreamElement::MetricModulation(_) => vec!["MetricModulation", "TempoIndication"],
            StreamElement::TextExpression(_) => vec!["TextExpression", "Expression"],
            StreamElement::RepeatExpression(_) => vec!["RepeatExpression", "Expression"],
            StreamElement::Dynamic(_) => vec!["Dynamic"],
            StreamElement::Barline(_) => vec!["Barline"],
            StreamElement::PedalObject(_) => vec!["PedalObject"],
            StreamElement::RehearsalMark(_) => vec!["RehearsalMark"],
            StreamElement::Break(_) => vec!["Break"],
            StreamElement::Layout(layout) => vec![layout.class_name(), "LayoutBase"],
            StreamElement::TextBox(_) => vec!["TextBox"],
            StreamElement::Variant(_) => vec!["Variant"],
        };
        names.push("Music21Object");
        names
    }

    /// Whether the element is of a class, by music21's name for it:
    /// music21's `isinstance` by class name.
    ///
    /// ```
    /// use music21_rs::{Note, StreamElement};
    ///
    /// let note: StreamElement = Note::from_name("C4")?.into();
    /// assert!(note.is_instance("NotRest"));
    /// assert!(note.is_instance("GeneralNote"));
    /// assert!(!note.is_instance("Rest"));
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    pub fn is_instance(&self, class: &str) -> bool {
        self.music21_classes().contains(&class)
    }
}
