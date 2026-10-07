//! Elements compared as music21's `==` compares them, as braille compares
//! measures to find repeats: each class's `equalityAttributes`, the
//! duration first, and for notes their tie, the kinds of their
//! articulations and expressions, noteheads, beams and pitch.

use crate::{
    duration::Duration, expressions::Expression, notation::Beams, note::Note, pitch::Pitch,
    stream::StreamElement,
};

/// Whether two durations are equal as music21's `Duration.__eq__` has it:
/// by length alone where both were worked out from it, otherwise by their
/// written values and tuplets too.
fn duration_equal(left: &Duration, right: &Duration) -> bool {
    if left.is_grace() != right.is_grace() || left.linked() != right.linked() {
        return false;
    }
    if left.expression_is_inferred() && right.expression_is_inferred() {
        return left.quarter_length() == right.quarter_length();
    }
    let (ours, theirs) = (left.written_values(), right.written_values());
    if left.is_complex() != right.is_complex() || ours.len() != theirs.len() {
        return false;
    }
    if ours.is_empty() {
        return true;
    }
    super::basic::duration_type(left) == super::basic::duration_type(right)
        && super::basic::duration_dots(left) == super::basic::duration_dots(right)
        && tuplets_equal(left, right)
        && left.quarter_length() == right.quarter_length()
}

fn tuplets_equal(left: &Duration, right: &Duration) -> bool {
    let (ours, theirs) = (left.tuplets(), right.tuplets());
    ours.len() == theirs.len()
        && ours.iter().zip(&theirs).all(|(a, b)| {
            a.actual() == b.actual()
                && a.normal() == b.normal()
                && a.duration_type() == b.duration_type()
                && a.dots() == b.dots()
        })
}

fn optional_duration_equal(left: Option<&Duration>, right: Option<&Duration>) -> bool {
    let default = Duration::default();
    duration_equal(left.unwrap_or(&default), right.unwrap_or(&default))
}

/// The beams as music21's `repr` writes them, which is how it compares
/// them.
fn beams_key(beams: &Beams) -> Vec<(Option<u32>, Option<String>, Option<String>)> {
    beams
        .beams()
        .iter()
        .map(|beam| {
            (
                beam.number(),
                beam.beam_type().map(|kind| kind.as_str().to_string()),
                beam.direction().map(|direction| format!("{direction:?}")),
            )
        })
        .collect()
}

/// The class of an expression, as music21 compares a note's expressions.
fn expression_class(expression: &Expression) -> String {
    match expression {
        Expression::Ornament(ornament) => format!("{:?}", ornament.kind()),
        Expression::Fermata(_) => "Fermata".to_string(),
        Expression::Arpeggio(_) => "ArpeggioMark".to_string(),
    }
}

/// Whether two lists hold the same classes as music21 compares them: as
/// many, and the same set.
fn same_classes(left: Vec<String>, right: Vec<String>) -> bool {
    use std::collections::BTreeSet;
    left.len() == right.len()
        && left.iter().collect::<BTreeSet<_>>() == right.iter().collect::<BTreeSet<_>>()
}

fn pitch_equal(left: &Pitch, right: &Pitch) -> bool {
    left == right
}

fn note_equal(left: &Note, right: &Note) -> bool {
    optional_duration_equal(left.duration(), right.duration())
        && left.tie().map(|tie| tie.tie_type()) == right.tie().map(|tie| tie.tie_type())
        && same_classes(
            left.articulations()
                .iter()
                .map(|a| a.kind().class_name().to_string())
                .collect(),
            right
                .articulations()
                .iter()
                .map(|a| a.kind().class_name().to_string())
                .collect(),
        )
        && same_classes(
            left.expressions().iter().map(expression_class).collect(),
            right.expressions().iter().map(expression_class).collect(),
        )
        && left.notehead() == right.notehead()
        && left.notehead_fill() == right.notehead_fill()
        && left.notehead_parenthesis() == right.notehead_parenthesis()
        && beams_key(left.beams()) == beams_key(right.beams())
        && pitch_equal(left.pitch(), right.pitch())
}

/// Whether two elements are equal as music21's `==` has them.
pub(crate) fn equal(left: &StreamElement, right: &StreamElement) -> bool {
    match (left, right) {
        (StreamElement::Note(left), StreamElement::Note(right)) => note_equal(left, right),
        (StreamElement::Rest(left), StreamElement::Rest(right)) => {
            duration_equal(left.duration(), right.duration())
                && left.tie().map(|tie| tie.tie_type()) == right.tie().map(|tie| tie.tie_type())
                && same_classes(
                    left.articulations()
                        .iter()
                        .map(|a| a.kind().class_name().to_string())
                        .collect(),
                    right
                        .articulations()
                        .iter()
                        .map(|a| a.kind().class_name().to_string())
                        .collect(),
                )
                && same_classes(
                    left.expressions().iter().map(expression_class).collect(),
                    right.expressions().iter().map(expression_class).collect(),
                )
        }
        (StreamElement::Chord(left), StreamElement::Chord(right)) => {
            use std::collections::BTreeSet;
            let keys = |pitches: Vec<Pitch>| -> BTreeSet<String> {
                pitches.iter().map(crate::tree::pitch_set_key).collect()
            };
            optional_duration_equal(left.duration(), right.duration())
                && left.tie().map(|tie| tie.tie_type()) == right.tie().map(|tie| tie.tie_type())
                && same_classes(
                    left.articulations()
                        .iter()
                        .map(|a| a.kind().class_name().to_string())
                        .collect(),
                    right
                        .articulations()
                        .iter()
                        .map(|a| a.kind().class_name().to_string())
                        .collect(),
                )
                && same_classes(
                    left.expressions().iter().map(expression_class).collect(),
                    right.expressions().iter().map(expression_class).collect(),
                )
                && left.notehead() == right.notehead()
                && left.notehead_fill() == right.notehead_fill()
                && left.notehead_parenthesis() == right.notehead_parenthesis()
                && beams_key(left.beams()) == beams_key(right.beams())
                && keys(left.pitches()) == keys(right.pitches())
        }
        (StreamElement::ChordSymbol(left), StreamElement::ChordSymbol(right)) => {
            // A chord symbol compares as a chord: its length and the set of
            // pitches it stands for, its notes otherwise all alike.
            use std::collections::BTreeSet;
            let keys = |pitches: Vec<Pitch>| -> BTreeSet<String> {
                pitches.iter().map(crate::tree::pitch_set_key).collect()
            };
            duration_equal(left.duration(), right.duration())
                && keys(left.pitches().unwrap_or_default())
                    == keys(right.pitches().unwrap_or_default())
        }
        (StreamElement::Clef(left), StreamElement::Clef(right)) => {
            left.kind() == right.kind()
                && left.sign() == right.sign()
                && left.line() == right.line()
                && left.octave_change() == right.octave_change()
        }
        (StreamElement::Barline(left), StreamElement::Barline(right)) => {
            left.bar_type() == right.bar_type()
                && left.repeat_direction() == right.repeat_direction()
        }
        (StreamElement::Dynamic(_), StreamElement::Dynamic(_))
        | (StreamElement::TextExpression(_), StreamElement::TextExpression(_)) => true,
        _ => false,
    }
}
