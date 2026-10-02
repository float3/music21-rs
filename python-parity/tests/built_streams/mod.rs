//! Streams built the same way twice, once as music21's objects and once as
//! the crate's, for the tests that hold a stream walk to music21's.
//!
//! A [`Spec`] says what a stream holds. [`build`] makes the crate's stream of
//! it and [`literal`] writes it as a Python literal that `BUILD`'s `build`
//! makes music21's of, so the two start out the same; [`outline`] and
//! `BUILD`'s `outline` then write what each holds as the same text.

// Each test using this module uses its own part of it.
#![allow(dead_code)]

use music21_rs::expressions::{Expression, Fermata, Ornament, OrnamentKind};
use music21_rs::{
    Chord, Clef, Duration, Dynamic, Instrument, Note, Rest, Stream, StreamElement, StreamKind,
    TimeSignature,
};
use music21_rs_python_parity::music21_name;

/// What a note's instrument name is told to be.
#[derive(Clone, Copy, Debug)]
pub enum Named {
    /// Left as its class names it.
    AsMade,
    /// Taken away.
    Nothing,
    /// Given.
    As(&'static str),
}

/// One thing a built stream holds.
#[derive(Clone, Debug)]
pub enum Spec {
    /// A stream of a kind, with its number where it is a measure, an id
    /// where it has one, and what it holds at which offsets.
    Stream {
        kind: StreamKind,
        id: Option<&'static str>,
        number: i32,
        held: Vec<(f64, Spec)>,
    },
    /// A note: its pitch, its length, the instrument it keeps as its own,
    /// the expressions on it, and whether it is a grace note.
    Note {
        name: &'static str,
        length: f64,
        stored: Option<&'static str>,
        marks: &'static [&'static str],
        grace: bool,
    },
    /// A chord of a length.
    Chord { names: &'static str, length: f64 },
    /// A rest of a length.
    Rest(f64),
    /// An instrument of a class, with its part name and its own.
    Instrument {
        kind: &'static str,
        part_name: Option<&'static str>,
        name: Named,
    },
    /// A dynamic mark.
    Dynamic(&'static str),
    /// A treble or a bass clef.
    Clef(&'static str),
    /// A meter, as `3/4`.
    Meter(&'static str),
}

/// A plain note of a length.
pub fn note(name: &'static str, length: f64) -> Spec {
    Spec::Note {
        name,
        length,
        stored: None,
        marks: &[],
        grace: false,
    }
}

/// A note keeping an instrument as its own.
pub fn played_on(name: &'static str, kind: &'static str) -> Spec {
    Spec::Note {
        name,
        length: 1.0,
        stored: Some(kind),
        marks: &[],
        grace: false,
    }
}

/// A note with expressions on it.
pub fn marked(name: &'static str, length: f64, marks: &'static [&'static str]) -> Spec {
    Spec::Note {
        name,
        length,
        stored: None,
        marks,
        grace: false,
    }
}

/// An instrument as its class makes it.
pub fn instrument(kind: &'static str) -> Spec {
    Spec::Instrument {
        kind,
        part_name: None,
        name: Named::AsMade,
    }
}

/// A stream of a kind holding these.
pub fn stream(kind: StreamKind, held: Vec<(f64, Spec)>) -> Spec {
    Spec::Stream {
        kind,
        id: None,
        number: 0,
        held,
    }
}

/// A measure of a number holding these.
pub fn measure(number: i32, held: Vec<(f64, Spec)>) -> Spec {
    Spec::Stream {
        kind: StreamKind::Measure,
        id: None,
        number,
        held,
    }
}

fn kind_name(kind: StreamKind) -> &'static str {
    match kind {
        StreamKind::Stream => "Stream",
        StreamKind::Voice => "Voice",
        StreamKind::Measure => "Measure",
        StreamKind::Part => "Part",
        StreamKind::PartStaff => "PartStaff",
        StreamKind::Score => "Score",
        StreamKind::Opus => "Opus",
    }
}

fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn optional(text: Option<&str>) -> String {
    text.map_or_else(|| "None".to_string(), quoted)
}

/// The spec as the Python literal `BUILD`'s `build` reads.
pub fn literal(spec: &Spec) -> String {
    match spec {
        Spec::Stream {
            kind,
            id,
            number,
            held,
        } => {
            let held: Vec<String> = held
                .iter()
                .map(|(at, inner)| format!("({at:?}, {})", literal(inner)))
                .collect();
            format!(
                "('stream', '{}', {}, {number}, [{}])",
                kind_name(*kind),
                optional(*id),
                held.join(", ")
            )
        }
        Spec::Note {
            name,
            length,
            stored,
            marks,
            grace,
        } => {
            let marks: Vec<String> = marks.iter().map(|mark| quoted(mark)).collect();
            format!(
                "('note', {}, {length:?}, {}, [{}], {})",
                quoted(name),
                optional(*stored),
                marks.join(", "),
                if *grace { "True" } else { "False" }
            )
        }
        Spec::Chord { names, length } => format!("('chord', {}, {length:?})", quoted(names)),
        Spec::Rest(length) => format!("('rest', {length:?})"),
        Spec::Instrument {
            kind,
            part_name,
            name,
        } => {
            let name = match name {
                Named::AsMade => "'as made'".to_string(),
                Named::Nothing => "None".to_string(),
                Named::As(name) => format!("('as', {})", quoted(name)),
            };
            format!(
                "('instrument', {}, {}, {name})",
                quoted(kind),
                optional(*part_name)
            )
        }
        Spec::Dynamic(value) => format!("('dynamic', {})", quoted(value)),
        Spec::Clef(kind) => format!("('clef', {})", quoted(kind)),
        Spec::Meter(ratio) => format!("('meter', {})", quoted(ratio)),
    }
}

fn expression(mark: &str) -> Expression {
    match mark {
        "Fermata" => Fermata::new().into(),
        other => Ornament::of_kind(
            OrnamentKind::from_class_name(other).unwrap_or_else(|| panic!("no ornament {other}")),
        )
        .into(),
    }
}

/// The crate's instrument of a spec.
pub fn build_instrument(kind: &str, part_name: Option<&str>, name: Named) -> Instrument {
    let mut made = Instrument::of_kind(kind).expect("an instrument class");
    made.set_part_name(part_name.map(str::to_string));
    match name {
        Named::AsMade => {}
        Named::Nothing => made.set_name(None),
        Named::As(name) => made.set_name(Some(name.to_string())),
    }
    made
}

/// The crate's element of a spec.
pub fn build(spec: &Spec) -> StreamElement {
    match spec {
        Spec::Stream {
            kind,
            id,
            number,
            held,
        } => {
            let mut made = Stream::with_kind(*kind);
            made.set_id(id.map(str::to_string));
            if *kind == StreamKind::Measure {
                made.set_number(*number);
            }
            for (at, inner) in held {
                made.insert(*at, build(inner));
            }
            made.into()
        }
        Spec::Note {
            name,
            length,
            stored,
            marks,
            grace,
        } => {
            let mut made = Note::from_name(name)
                .expect("a pitch")
                .with_duration(Duration::new(*length).expect("a length"));
            made.set_stored_instrument(
                stored.map(|kind| Instrument::of_kind(kind).expect("an instrument class")),
            );
            for mark in *marks {
                made.expressions_mut().push(expression(mark));
            }
            if *grace {
                made = made.grace_note(false);
            }
            made.into()
        }
        Spec::Chord { names, length } => Chord::new(*names)
            .expect("a chord")
            .with_duration(Duration::new(*length).expect("a length"))
            .into(),
        Spec::Rest(length) => Rest::new(Duration::new(*length).expect("a length")).into(),
        Spec::Instrument {
            kind,
            part_name,
            name,
        } => build_instrument(kind, *part_name, *name).into(),
        Spec::Dynamic(value) => Dynamic::new(*value).into(),
        Spec::Clef(kind) => match *kind {
            "bass" => Clef::bass().into(),
            _ => Clef::treble().into(),
        },
        Spec::Meter(ratio) => TimeSignature::from_ratio_string(ratio)
            .expect("a meter")
            .into(),
    }
}

/// The crate's stream of a spec that is one.
pub fn build_stream(spec: &Spec) -> Stream {
    match build(spec) {
        StreamElement::Stream(made) => *made,
        other => panic!("not a stream: {other:?}"),
    }
}

/// `build(spec)` makes music21's object of a spec written by [`literal`],
/// and `outline(stream)` writes a stream as [`outline`] writes the crate's.
pub const BUILD: &str = r#"
from music21 import chord, clef, dynamics, expressions, instrument, meter, note, stream

def build(spec):
    tag = spec[0]
    if tag == 'stream':
        _, kind, ident, number, held = spec
        made = getattr(stream, kind)()
        if ident is not None:
            made.id = ident
        if kind == 'Measure':
            made.number = number
        for at, inner in held:
            made.insert(at, build(inner))
        return made
    if tag == 'note':
        _, name, length, stored, marks, grace = spec
        made = note.Note(name, quarterLength=length)
        if stored is not None:
            made.storedInstrument = getattr(instrument, stored)()
        for mark in marks:
            made.expressions.append(getattr(expressions, mark)())
        if grace:
            made = made.getGrace()
        return made
    if tag == 'chord':
        return chord.Chord(spec[1], quarterLength=spec[2])
    if tag == 'rest':
        return note.Rest(quarterLength=spec[1])
    if tag == 'instrument':
        _, kind, part_name, name = spec
        made = getattr(instrument, kind)()
        made.partName = part_name
        if name is None:
            made.instrumentName = None
        elif name != 'as made':
            made.instrumentName = name[1]
        return made
    if tag == 'dynamic':
        return dynamics.Dynamic(spec[1])
    if tag == 'clef':
        return clef.BassClef() if spec[1] == 'bass' else clef.TrebleClef()
    if tag == 'meter':
        return meter.TimeSignature(spec[1])
    raise ValueError(tag)

def number(value):
    return f'{float(value):.5f}'

def said(text):
    return '-' if text is None else str(text)

def describe(e):
    classes = e.classes
    if 'Note' in classes:
        stored = e.storedInstrument.classes[0] if e.storedInstrument else None
        marks = ','.join(type(mark).__name__ for mark in e.expressions)
        tie = e.tie.type if e.tie else None
        return f'Note {e.nameWithOctave} stored={said(stored)} marks=[{marks}] tie={said(tie)}'
    if 'Chord' in classes:
        return 'Chord ' + ' '.join(p.nameWithOctave for p in e.pitches)
    if 'Rest' in classes:
        return 'Rest'
    if 'Instrument' in classes:
        return f'Instrument {classes[0]} part={said(e.partName)} name={said(e.instrumentName)}'
    if 'Dynamic' in classes:
        return f'Dynamic {e.value}'
    if 'Clef' in classes:
        return f'Clef {e.sign}{e.line}'
    if 'TimeSignature' in classes:
        return f'Meter {e.ratioString}'
    return None

def outline(held, depth=0):
    out = []
    pad = '  ' * depth
    for e in held:
        at = number(held.elementOffset(e))
        if e.isStream:
            ident = e.id if isinstance(e.id, str) and not e.id.endswith('_flat') else None
            numbered = e.number if 'Measure' in e.classes else 0
            out.append(f'{pad}{at} {e.classes[0]} id={said(ident)} n={numbered}')
            out.extend(outline(e, depth + 1))
            continue
        what = describe(e)
        if what is not None:
            out.append(f'{pad}{at} {number(e.duration.quarterLength)} {what}')
    return out if depth else '\n'.join(out)
"#;

fn number(value: f64) -> String {
    format!("{value:.5}")
}

fn said(text: Option<&str>) -> String {
    text.map_or_else(|| "-".to_string(), str::to_string)
}

fn mark_name(expression: &Expression) -> String {
    match expression {
        Expression::Ornament(ornament) => ornament.kind().class_name().to_string(),
        Expression::Fermata(_) => "Fermata".to_string(),
        Expression::Arpeggio(_) => "ArpeggioMark".to_string(),
    }
}

fn describe(element: &StreamElement) -> Option<String> {
    Some(match element {
        StreamElement::Note(note) => {
            let marks: Vec<String> = note.expressions().iter().map(mark_name).collect();
            format!(
                "Note {} stored={} marks=[{}] tie={}",
                music21_name(&note.pitch().name_with_octave()),
                said(note.stored_instrument().map(Instrument::kind)),
                marks.join(","),
                said(note.tie().map(|tie| tie.tie_type().as_str()))
            )
        }
        StreamElement::Chord(chord) => format!(
            "Chord {}",
            chord
                .pitches()
                .iter()
                .map(|pitch| music21_name(&pitch.name_with_octave()))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        StreamElement::Rest(_) => "Rest".to_string(),
        StreamElement::Instrument(instrument) => format!(
            "Instrument {} part={} name={}",
            instrument.kind(),
            said(instrument.part_name()),
            said(instrument.name())
        ),
        StreamElement::Dynamic(dynamic) => format!("Dynamic {}", dynamic.value()),
        StreamElement::Clef(clef) => format!(
            "Clef {}{}",
            clef.sign().unwrap_or("None"),
            clef.line()
                .map_or_else(|| "None".to_string(), |line| line.to_string())
        ),
        StreamElement::TimeSignature(meter) => format!("Meter {}", meter.ratio_string()),
        _ => return None,
    })
}

fn outline_into(held: &Stream, depth: usize, out: &mut Vec<String>) {
    let pad = "  ".repeat(depth);
    for event in held.events() {
        let at = number(event.offset());
        if let Some(inner) = event.element().as_stream() {
            out.push(format!(
                "{pad}{at} {} id={} n={}",
                kind_name(inner.kind()),
                said(inner.id()),
                if inner.kind() == StreamKind::Measure {
                    inner.number()
                } else {
                    0
                }
            ));
            outline_into(inner, depth + 1, out);
            continue;
        }
        if let Some(what) = describe(event.element()) {
            out.push(format!(
                "{pad}{at} {} {what}",
                number(event.element().quarter_length())
            ));
        }
    }
}

/// What a stream holds, as `BUILD`'s `outline` writes music21's.
pub fn outline(held: &Stream) -> String {
    let mut out = Vec::new();
    outline_into(held, 0, &mut out);
    out.join("\n")
}
