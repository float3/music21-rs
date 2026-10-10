//! Writing ABC: a score as the text `from_abc` reads back.
//!
//! music21 has no ABC writer, so there is no text of its to match. What is
//! matched is its *reader*: wherever ABC can be written two ways, the one
//! written is the one music21's `abcFormat` reads back as the score it was
//! written from, and the notes on what is where below are about that reader.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use super::read::class_sort_order;
use crate::articulations::{Articulation, ArticulationKind, Finger};
use crate::bar::{Barline, BarlineType, Ending, RepeatDirection};
use crate::chord::Chord;
use crate::chordsymbol::ChordSymbol;
use crate::clef::Clef;
use crate::defaults::{FloatType, IntegerType, UnsignedIntegerType};
use crate::duration::{Duration, TupletType, limited_fraction};
use crate::error::{Error, Result};
use crate::expressions::{ArpeggioType, Expression, FermataType, OrnamentKind};
use crate::key::KeySignature;
use crate::metadata::Metadata;
use crate::meter::TimeSignature;
use crate::notation::{BeamType, Beams, Lyric, Placement, TieType};
use crate::note::Note;
use crate::pitch::Pitch;
use crate::repeat::RepeatExpressionKind;
use crate::rest::Rest;
use crate::spanner::SpannerKind;
use crate::stream::{Stream, StreamElement, StreamKind};
use crate::tempo::MetronomeMark;

fn abc_error(message: impl Into<String>) -> Error {
    Error::Abc(message.into())
}

/// How [`to_abc`] writes a score.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportOptions {
    /// The unit note length of the `L:` field as a fraction of a whole note,
    /// `(1, 8)` for an eighth. `None` takes the one the tune's lengths are
    /// shortest to write against.
    pub unit_length: Option<(UnsignedIntegerType, UnsignedIntegerType)>,
    /// How many measures go on a line of music.
    pub measures_per_line: usize,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            unit_length: None,
            measures_per_line: 4,
        }
    }
}

/// Writes a score as ABC.
///
/// A score is one tune and an opus one tune for each score it holds, under
/// the `X:` number its metadata gives it. A part alone, or a stream of loose
/// notes, is a tune of one voice. The header carries the title, composer and
/// origin, the meter, the unit note length, the tempo and the key with its
/// mode; a score of several parts has a `V:` voice for each, and a part
/// whose measures hold voices of their own has one for each of those. A
/// voice's clef is said on its `V:` line, unless it is a treble clef the
/// notes fit, which is what a reader takes where none is said, and a part's
/// name goes there too; a tune of one voice has a `V:` line only for those.
/// A part opening with an instrument that has a General MIDI program says so
/// with abc2midi's `%%MIDI program`, before its music.
///
/// What ABC has no way to say is refused with [`Error::Abc`] rather than
/// left out: a microtone, a tuplet inside a tuplet, an unpitched stroke, a
/// chord symbol or a dynamic standing where no note starts, a pedal with its
/// bounces and gaps, an octave line, a glissando, a tremolo between notes, a
/// rehearsal mark or a metric modulation. A barline inside a measure is
/// drawn and not heard, and is written as nothing.
///
/// Three things are written as music21's reader takes them, since
/// [`from_abc`](crate::abc::from_abc) is that reader. An accidental is
/// written on every note that shows one and carries no further into the
/// bar. A tuplet's count of notes takes in any grace notes written among
/// them. And a field changing the meter, the key or the tempo at a barline
/// stands on lines of its own after that barline, where two fields together
/// are read as belonging to the measure that follows.
///
/// ```
/// use music21_rs::abc::{ExportOptions, from_abc, to_abc};
///
/// let tune = "X:1\nT:Scale\nM:4/4\nL:1/4\nK:G\nGABc|defg|a4|]\n";
/// let score = from_abc(tune)?;
/// let written = to_abc(&score, &ExportOptions::default())?;
/// assert_eq!(
///     written,
///     "X:1\nT:Scale\nM:4/4\nL:1/4\nK:G\nG A B c | d e f g | a4 |]\n"
/// );
/// // What was written reads back as the score it was written from.
/// assert_eq!(to_abc(&from_abc(&written)?, &ExportOptions::default())?, written);
/// # Ok::<(), music21_rs::Error>(())
/// ```
pub fn to_abc(stream: &Stream, options: &ExportOptions) -> Result<String> {
    if stream.kind() == StreamKind::Opus {
        let mut tunes = Vec::new();
        for (index, event) in stream.events().iter().enumerate() {
            let score = event
                .element()
                .as_stream()
                .ok_or_else(|| abc_error("an opus holds something that is not a score"))?;
            tunes.push(Tune::new(score, index as IntegerType + 1)?.write(options)?);
        }
        return Ok(tunes.join("\n"));
    }
    Tune::new(stream, 1)?.write(options)
}

const EPSILON: FloatType = 1e-6;

/// A length as a fraction, where it is one a note could have.
fn fraction(value: FloatType) -> Result<(i128, i128)> {
    limited_fraction(value, 4096)
        .filter(|(numerator, denominator)| {
            (*numerator as FloatType / *denominator as FloatType - value).abs() < 1e-7
        })
        .ok_or_else(|| {
            abc_error(format!(
                "ABC cannot write a length of {value} quarter notes"
            ))
        })
}

/// A length against the unit note length, as it follows a note.
fn length_text(written: FloatType, unit: FloatType) -> Result<String> {
    let (numerator, denominator) = fraction(written / unit)?;
    Ok(match (numerator, denominator) {
        (1, 1) => String::new(),
        (numerator, 1) => numerator.to_string(),
        (1, 2) => "/".to_string(),
        (1, denominator) => format!("/{denominator}"),
        (numerator, denominator) => format!("{numerator}/{denominator}"),
    })
}

/// Text that can stand in a field: on one line.
fn plain(text: &str) -> String {
    text.replace(['\n', '\r'], " ").trim().to_string()
}

/// Text that can stand between quotation marks.
fn quoted(text: &str) -> String {
    plain(text).replace('"', "'")
}

/// How far each of the seven letters, C to B, is moved by a signature.
type Alters = [IntegerType; 7];

fn letter_index(letter: char) -> usize {
    match letter {
        'C' => 0,
        'D' => 1,
        'E' => 2,
        'F' => 3,
        'G' => 4,
        'A' => 5,
        _ => 6,
    }
}

fn alters_of(signature: &KeySignature) -> Alters {
    let mut alters = [0; 7];
    for pitch in signature.altered_pitches().unwrap_or_default() {
        alters[letter_index(pitch.step().as_char())] = pitch.alter().round() as IntegerType;
    }
    alters
}

/// The signature a key or key signature element writes.
fn signature_of(element: &StreamElement) -> Option<KeySignature> {
    match element {
        StreamElement::Key(key) => Some(key.key_signature()),
        StreamElement::KeySignature(signature) => Some(signature.clone()),
        _ => None,
    }
}

fn abc_tonic(pitch: &Pitch) -> String {
    pitch.name().replace('-', "b")
}

/// The clef as ABC names one.
fn clef_name(clef: &Clef) -> Result<String> {
    let mut name = match (clef.sign(), clef.line()) {
        (Some("G"), Some(2) | None) => "treble".to_string(),
        (Some("G"), Some(line)) => format!("treble{line}"),
        (Some("F"), Some(4) | None) => "bass".to_string(),
        (Some("F"), Some(line)) => format!("bass{line}"),
        (Some("C"), Some(3) | None) => "alto".to_string(),
        (Some("C"), Some(4)) => "tenor".to_string(),
        (Some("C"), Some(line)) => format!("alto{line}"),
        (Some("percussion"), _) => "perc".to_string(),
        (Some("none"), _) | (None, _) => "none".to_string(),
        (Some(sign), _) => {
            return Err(abc_error(format!("ABC has no {sign} clef")));
        }
    };
    match clef.octave_change() {
        0 => {}
        1 => name.push_str("+8"),
        -1 => name.push_str("-8"),
        other => {
            return Err(abc_error(format!(
                "ABC has no clef moved by {other} octaves"
            )));
        }
    }
    Ok(name)
}

/// The data of a `K:` field for a key or a key signature, with a clef after
/// it where one is to be named.
fn key_field(element: &StreamElement, clef: Option<&Clef>) -> Result<String> {
    let clef = clef.map(clef_name).transpose()?;
    let mode = match element {
        StreamElement::Key(key) => match key.mode() {
            "major" => Some(""),
            "minor" => Some("m"),
            "dorian" => Some("dor"),
            "phrygian" => Some("phr"),
            "lydian" => Some("lyd"),
            "mixolydian" => Some("mix"),
            "aeolian" => Some("aeo"),
            "ionian" => Some("ion"),
            "locrian" => Some("loc"),
            _ => None,
        },
        _ => None,
    };
    let mut text = match (element, mode) {
        (StreamElement::Key(key), Some(mode)) => {
            // A clef after a bare tonic would be read as the mode.
            let mode = if mode.is_empty() && clef.is_some() {
                " maj"
            } else {
                mode
            };
            format!("{}{mode}", abc_tonic(key.tonic_pitch()))
        }
        _ => {
            // A signature with no mode: the major tonic it would have, and
            // then every accidental said, which leaves no mode to read.
            let signature = signature_of(element)
                .ok_or_else(|| abc_error("a key field written for what is no key"))?;
            let tonic = match signature.sharps() {
                Some(sharps) => {
                    let key = KeySignature::new(sharps).as_key("major");
                    abc_tonic(key.tonic_pitch())
                }
                None => "C".to_string(),
            };
            let mut text = format!("{tonic} exp");
            for pitch in signature.altered_pitches()? {
                let _ = write!(
                    text,
                    " {}{}",
                    accidental_marks(pitch.alter().round() as IntegerType)?,
                    pitch.step().as_char().to_ascii_lowercase()
                );
            }
            text
        }
    };
    if let Some(clef) = clef {
        let _ = write!(text, " clef={clef}");
    }
    Ok(text)
}

fn accidental_marks(alter: IntegerType) -> Result<&'static str> {
    Ok(match alter {
        -2 => "__",
        -1 => "_",
        0 => "=",
        1 => "^",
        2 => "^^",
        other => {
            return Err(abc_error(format!(
                "ABC has no accidental moving a note by {other} semitones"
            )));
        }
    })
}

fn meter_field(meter: &TimeSignature) -> String {
    match meter.symbol() {
        Some("common") => "C".to_string(),
        Some("cut") => "C|".to_string(),
        _ => meter.ratio_string(),
    }
}

fn number_text(number: FloatType) -> String {
    if number.fract() == 0.0 {
        format!("{number:.0}")
    } else {
        number.to_string()
    }
}

/// The data of a `Q:` field, or nothing for a mark that says nothing.
fn tempo_field(mark: &MetronomeMark) -> Result<Option<String>> {
    let mut parts = Vec::new();
    if let Some(text) = mark.text().filter(|_| !mark.text_implicit()) {
        parts.push(format!("\"{}\"", quoted(text)));
    }
    if let Some(number) = mark.number().filter(|_| !mark.number_implicit()) {
        let (numerator, denominator) = fraction(mark.referent().quarter_length() / 4.0)?;
        parts.push(format!("{numerator}/{denominator}={}", number_text(number)));
    }
    Ok((!parts.is_empty()).then(|| parts.join(" ")))
}

/// One element where a voice holds it: its offset in its measure and its
/// place among the leaves of the score, which is how a spanner names it.
#[derive(Clone, Copy, Debug)]
struct Held<'a> {
    offset: FloatType,
    element: &'a StreamElement,
    leaf: usize,
}

/// One measure of one voice, or the whole of a voice that has no measures.
#[derive(Clone, Debug, Default)]
struct Bar<'a> {
    measure: Option<&'a Stream>,
    held: Vec<Held<'a>>,
    /// A voice its measure does not have is silent for this long.
    filler: Option<FloatType>,
}

/// What ABC calls a voice: a part, or one voice of a part's measures.
#[derive(Clone, Debug)]
struct Voice<'a> {
    part: &'a Stream,
    bars: Vec<Bar<'a>>,
    measured: bool,
    /// Which voice of its part this is.
    within: usize,
}

/// The General MIDI program of the instrument a part opens with: the first
/// standing in it no later than its first note.
fn program_of(part: &Stream) -> Option<u8> {
    let leaves = part.leaves();
    let first_note = leaves
        .iter()
        .filter(|(_, element)| is_sounding(element))
        .map(|(offset, _)| *offset)
        .fold(FloatType::INFINITY, FloatType::min);
    leaves.iter().find_map(|(offset, element)| match element {
        StreamElement::Instrument(instrument) if *offset <= first_note + EPSILON => {
            instrument.midi_program()
        }
        _ => None,
    })
}

fn is_sounding(element: &StreamElement) -> bool {
    matches!(
        element,
        StreamElement::Note(_)
            | StreamElement::Chord(_)
            | StreamElement::Rest(_)
            | StreamElement::Unpitched(_)
            | StreamElement::PercussionChord(_)
    )
}

fn is_grace(element: &StreamElement) -> bool {
    is_sounding(element) && element.duration().is_some_and(Duration::is_grace)
}

/// The elements of a stream and of every stream inside it, in the order
/// `Stream::leaves` counts them.
fn gather<'a>(stream: &'a Stream, base: FloatType, leaf: &mut usize, out: &mut Vec<Held<'a>>) {
    for event in stream.events() {
        match event.element() {
            StreamElement::Stream(inner) => gather(inner, base + event.offset(), leaf, out),
            element => {
                out.push(Held {
                    offset: base + event.offset(),
                    element,
                    leaf: *leaf,
                });
                *leaf += 1;
            }
        }
    }
}

fn sort_held(held: &mut [Held<'_>]) {
    held.sort_by(|left, right| {
        left.offset
            .total_cmp(&right.offset)
            .then(class_sort_order(left.element).cmp(&class_sort_order(right.element)))
            .then(is_grace(right.element).cmp(&is_grace(left.element)))
            .then(left.leaf.cmp(&right.leaf))
    });
}

/// The voices a part is written as, counting its leaves on from `leaf`.
fn voices_of<'a>(part: &'a Stream, leaf: &mut usize) -> Result<Vec<Voice<'a>>> {
    let has_measures = part.events().iter().any(|event| {
        event
            .element()
            .as_stream()
            .is_some_and(|inner| inner.kind() == StreamKind::Measure)
    });
    if !has_measures {
        let mut held = Vec::new();
        gather(part, 0.0, leaf, &mut held);
        return Ok(vec![Voice {
            part,
            bars: vec![Bar {
                measure: None,
                held,
                filler: None,
            }],
            measured: false,
            within: 0,
        }]);
    }

    // Each measure as what stands in it outside any voice, and its voices.
    type Split<'a> = (&'a Stream, Vec<Held<'a>>, Vec<Vec<Held<'a>>>);
    let mut measures: Vec<Split<'a>> = Vec::new();
    let mut loose: Vec<Held<'a>> = Vec::new();
    for event in part.events() {
        match event.element() {
            StreamElement::Stream(measure) => {
                let mut common = Vec::new();
                let mut voices = Vec::new();
                for inner in measure.events() {
                    match inner.element() {
                        StreamElement::Stream(voice) => {
                            let mut held = Vec::new();
                            gather(voice, inner.offset(), leaf, &mut held);
                            voices.push(held);
                        }
                        element => {
                            common.push(Held {
                                offset: inner.offset(),
                                element,
                                leaf: *leaf,
                            });
                            *leaf += 1;
                        }
                    }
                }
                measures.push((measure, common, voices));
            }
            element => {
                loose.push(Held {
                    offset: event.offset(),
                    element,
                    leaf: *leaf,
                });
                *leaf += 1;
            }
        }
    }
    if loose.iter().any(|held| is_sounding(held.element)) {
        return Err(abc_error(
            "a part holds notes outside its measures as well as inside them",
        ));
    }

    let count = measures
        .iter()
        .map(|(_, _, voices)| voices.len())
        .max()
        .unwrap_or(0)
        .max(1);
    let mut voices: Vec<Voice<'a>> = (0..count)
        .map(|within| Voice {
            part,
            bars: Vec::new(),
            measured: true,
            within,
        })
        .collect();
    for (index, (measure, common, inner)) in measures.into_iter().enumerate() {
        let length = common
            .iter()
            .chain(inner.iter().flatten())
            .map(|held| held.offset + held.element.quarter_length())
            .fold(0.0, FloatType::max);
        let plain_measure = inner.is_empty();
        let mut inner = inner.into_iter();
        for (within, voice) in voices.iter_mut().enumerate() {
            let own = inner.next();
            let mut held = Vec::new();
            if within == 0 {
                held.extend(common.iter().copied());
                // What stands in the part before any measure is written at
                // the head of the first.
                if index == 0 {
                    held.extend(loose.iter().filter(|held| held.offset < EPSILON).copied());
                }
            } else {
                // The clef, key and meter standing outside the voices are
                // every voice's: each voice of ABC states its own.
                let shared = |held: &&Held<'a>| {
                    matches!(
                        held.element,
                        StreamElement::Clef(_)
                            | StreamElement::Key(_)
                            | StreamElement::KeySignature(_)
                            | StreamElement::TimeSignature(_)
                    )
                };
                held.extend(common.iter().filter(shared).copied());
                if index == 0 {
                    held.extend(
                        loose
                            .iter()
                            .filter(|held| held.offset < EPSILON)
                            .filter(shared)
                            .copied(),
                    );
                }
            }
            let filler = match own {
                Some(own) => {
                    held.extend(own);
                    None
                }
                None if within == 0 && plain_measure => None,
                None => Some(length),
            };
            if within > 0 || !plain_measure || index == 0 {
                sort_held(&mut held);
            }
            voice.bars.push(Bar {
                measure: Some(measure),
                held,
                filler,
            });
        }
    }
    Ok(voices)
}

/// The meter a measure states at its start.
fn meter_of<'a>(bar: &Bar<'a>) -> Option<&'a TimeSignature> {
    bar.held.iter().find_map(|held| match held.element {
        StreamElement::TimeSignature(meter) if held.offset < EPSILON => Some(meter),
        _ => None,
    })
}

/// How long what a measure holds lasts.
fn length_of(bar: &Bar<'_>) -> FloatType {
    bar.held
        .iter()
        .map(|held| held.offset + held.element.quarter_length())
        .fold(0.0, FloatType::max)
}

/// What the measure after gives the last note of a measure: the second half
/// of a note music21 cut at the barline, written back onto the first.
#[derive(Clone, Debug, Default)]
struct Tail {
    /// The length of the second half, in quarter notes.
    extra: FloatType,
    /// Whether the note is tied on past its second half.
    tied: bool,
    /// The slurs and hairpins closing on the second half.
    closes: String,
}

/// How one measure is written where it is one of two music21 cut from one.
#[derive(Clone, Debug, Default)]
struct Plan {
    /// The meter is the cut's to bring, not the text's.
    drop_meter: bool,
    /// The last note takes on the first of the next measure.
    tail: Option<Tail>,
    /// The first note is written on the measure before.
    skip_head: bool,
}

fn first_sounding(bar: &Bar<'_>) -> Option<usize> {
    bar.held.iter().position(|held| is_sounding(held.element))
}

fn last_sounding(bar: &Bar<'_>) -> Option<usize> {
    bar.held
        .iter()
        .rposition(|held| is_sounding(held.element) && !is_grace(held.element))
}

/// The notes of a note or a chord.
fn notes_of(element: &StreamElement) -> Option<&[Note]> {
    match element {
        StreamElement::Note(note) => Some(std::slice::from_ref(note)),
        StreamElement::Chord(chord) => Some(chord.notes()),
        _ => None,
    }
}

/// Whether the note a measure ends with and the one the next begins with
/// are the two halves of one, cut at the barline: the same pitches, tied
/// from the one to the other. The answer inside is whether writing them
/// apart would be read differently: a chord's ties are not read at all, and
/// the second half does not show an accidental the first does.
fn cut_note(first: &Bar<'_>, second: &Bar<'_>) -> Option<bool> {
    let before = first.held[last_sounding(first)?];
    let after = second.held[first_sounding(second)?];
    if is_grace(after.element)
        || after.offset > EPSILON
        || (before.offset + before.element.quarter_length() - length_of(first)).abs() > EPSILON
    {
        return None;
    }
    let is_chord = matches!(before.element, StreamElement::Chord(_));
    if is_chord != matches!(after.element, StreamElement::Chord(_)) {
        return None;
    }
    let (ending, starting) = (notes_of(before.element)?, notes_of(after.element)?);
    let said = |element: &StreamElement| {
        element
            .duration()
            .and_then(Duration::said_tuplets)
            .is_some_and(|tuplets| !tuplets.is_empty())
    };
    if ending.len() != starting.len() || said(before.element) || said(after.element) {
        return None;
    }
    let tie = |note: &Note| note.tie().map(crate::notation::Tie::tie_type);
    let halves = ending.iter().zip(starting).all(|(one, other)| {
        one.pitch().name_with_octave() == other.pitch().name_with_octave()
            && matches!(tie(one), Some(TieType::Start | TieType::Continue))
            && matches!(tie(other), Some(TieType::Stop | TieType::Continue))
            && other.lyrics().is_empty()
    });
    if !halves {
        return None;
    }
    let hidden = ending.iter().zip(starting).any(|(one, other)| {
        shows_accidental(one.pitch())
            && other
                .pitch()
                .written_accidental()
                .is_some_and(|accidental| accidental.display_status() == Some(false))
    });
    Some(is_chord || hidden)
}

/// What a note-like element is written from.
struct Sounding<'a> {
    duration: Duration,
    /// The written length before any tuplet, in quarter notes.
    written: FloatType,
    /// A tuplet said on the note: so many notes in the time of so many.
    tuplet: Option<(UnsignedIntegerType, UnsignedIntegerType)>,
    bracket: Option<TupletType>,
    grace: bool,
    held: Held<'a>,
}

fn sounding_of<'a>(held: Held<'a>) -> Result<Option<Sounding<'a>>> {
    let duration = match held.element {
        StreamElement::Note(note) => note.duration().cloned().unwrap_or_else(Duration::quarter),
        StreamElement::Chord(chord) => chord.duration().cloned().unwrap_or_else(Duration::quarter),
        StreamElement::Rest(rest) => rest.duration().clone(),
        StreamElement::Unpitched(_) | StreamElement::PercussionChord(_) => {
            return Err(abc_error("ABC has no unpitched notes"));
        }
        _ => return Ok(None),
    };
    let grace = duration.is_grace();
    let said = duration.said_tuplets().unwrap_or(&[]);
    if said.len() > 1 {
        return Err(abc_error("ABC cannot write a tuplet inside a tuplet"));
    }
    let (tuplet, bracket) = match said.first().filter(|_| !grace) {
        Some(tuplet) => {
            let multiplier = tuplet.multiplier();
            let numerator = *multiplier.numer().unwrap_or(&0) as FloatType;
            let denominator = *multiplier.denom().unwrap_or(&1) as FloatType;
            let normal = FloatType::from(tuplet.actual()) * numerator / denominator;
            if normal < 1.0 || (normal - normal.round()).abs() > EPSILON {
                return Err(abc_error(format!(
                    "ABC cannot write {} notes in the time of {normal}",
                    tuplet.actual()
                )));
            }
            (
                Some((tuplet.actual(), normal.round() as UnsignedIntegerType)),
                tuplet.tuplet_type(),
            )
        }
        None => (None, None),
    };
    let written = if grace {
        duration
            .written_values()
            .iter()
            .filter_map(|value| {
                Some(
                    value
                        .duration_type()?
                        .quarter_length_with_dots(value.dots()),
                )
            })
            .sum()
    } else if tuplet.is_some() {
        duration.quarter_length_no_tuplets()
    } else {
        duration.quarter_length()
    };
    Ok(Some(Sounding {
        duration,
        written,
        tuplet,
        bracket,
        grace,
        held,
    }))
}

/// The marker opening a tuplet: `p` notes in the time of `q`, for the next
/// `count` notes.
fn tuplet_marker(actual: u32, normal: u32, count: usize) -> String {
    let usual = match actual {
        2 | 4 | 8 => Some(3),
        3 | 6 => Some(2),
        _ => None,
    };
    let mut marker = format!("({actual}");
    let says_normal = usual != Some(normal);
    if says_normal {
        let _ = write!(marker, ":{normal}");
    }
    if count != actual as usize {
        if !says_normal {
            marker.push(':');
        }
        let _ = write!(marker, ":{count}");
    }
    marker
}

/// Where the tuplet markers of a voice go: before which note-like element,
/// counted through the voice, and what each says.
fn tuplet_markers(
    tokens: &[Sounding<'_>],
    opens_on: &dyn Fn(&Sounding<'_>) -> bool,
) -> BTreeMap<usize, String> {
    let mut markers = BTreeMap::new();
    let mut index = 0;
    while index < tokens.len() {
        let Some((actual, normal)) = tokens[index].tuplet else {
            index += 1;
            continue;
        };
        // Grace notes before the first note are written before the marker
        // and are none of its notes, unless a slur opens on one of them:
        // music21 closes whichever of a slur and a tuplet it met last, so
        // the marker then goes first, and counts them.
        let mut start = index;
        while start > 0 && tokens[start - 1].grace {
            start -= 1;
        }
        if !tokens[start..index].iter().any(opens_on) {
            start = index;
        }
        let mut last = index;
        let mut filled = tokens[index].written;
        loop {
            // A marker with no count takes as many notes as it names, and
            // fewer where they already fill the time: a quarter and an
            // eighth are a whole triplet of eighths.
            let whole = filled / FloatType::from(actual);
            let filled_up = whole > 0.0 && (whole.log2() - whole.log2().round()).abs() < 1e-9;
            let complete = last - start + 1 >= actual as usize || (filled_up && last > index);
            if complete
                || matches!(
                    tokens[last].bracket,
                    Some(TupletType::Stop | TupletType::StartStop)
                )
            {
                break;
            }
            let mut next = last + 1;
            while next < tokens.len() && tokens[next].grace {
                next += 1;
            }
            // music21 reads one digit for the count.
            if next >= tokens.len()
                || tokens[next].tuplet != Some((actual, normal))
                || matches!(
                    tokens[next].bracket,
                    Some(TupletType::Start | TupletType::StartStop)
                )
                || next - start + 1 > 9
            {
                break;
            }
            filled += tokens[next].written;
            last = next;
        }
        markers.insert(start, tuplet_marker(actual, normal, last - start + 1));
        index = last + 1;
    }
    markers
}

/// How a note, chord or rest is played.
fn articulations_of(element: &StreamElement) -> &[Articulation] {
    match element {
        StreamElement::Note(note) => note.articulations(),
        StreamElement::Chord(chord) => chord.articulations(),
        StreamElement::Rest(rest) => rest.articulations(),
        _ => &[],
    }
}

/// Whether an articulation is written as one character, which music21
/// reads as a token of its own.
fn is_lettered(articulation: &Articulation) -> bool {
    matches!(
        articulation.kind(),
        ArticulationKind::Staccato
            | ArticulationKind::UpBow
            | ArticulationKind::DownBow
            | ArticulationKind::Accent
            | ArticulationKind::StrongAccent
            | ArticulationKind::Tenuto
    )
}

/// The decoration an articulation is written as.
fn articulation_text(articulation: &Articulation) -> Result<String> {
    Ok(match articulation.kind() {
        ArticulationKind::Staccato => ".".to_string(),
        ArticulationKind::UpBow => "u".to_string(),
        ArticulationKind::DownBow => "v".to_string(),
        // The three letters the header gives these meanings to.
        ArticulationKind::Accent => "K".to_string(),
        ArticulationKind::StrongAccent => "k".to_string(),
        ArticulationKind::Tenuto => "M".to_string(),
        ArticulationKind::Staccatissimo => "!wedge!".to_string(),
        ArticulationKind::BreathMark => "!breath!".to_string(),
        ArticulationKind::OpenString => "!open!".to_string(),
        ArticulationKind::Stopped => "!+!".to_string(),
        ArticulationKind::SnapPizzicato => "!snap!".to_string(),
        ArticulationKind::Fingering => match articulation.finger() {
            Some(Finger::Number(number)) if (0..=5).contains(number) => format!("!{number}!"),
            _ => return Err(abc_error("ABC writes fingers nought to five and no others")),
        },
        other => {
            return Err(abc_error(format!(
                "ABC has no decoration for {}",
                other.class_name()
            )));
        }
    })
}

fn expression_text(expression: &Expression) -> Result<&'static str> {
    Ok(match expression {
        Expression::Fermata(fermata) => match fermata.fermata_type() {
            FermataType::Inverted => "!fermata!",
            FermataType::Upright => "!invertedfermata!",
        },
        Expression::Arpeggio(ArpeggioType::Normal) => "!arpeggio!",
        Expression::Arpeggio(_) => {
            return Err(abc_error("ABC has one arpeggio sign, the plain one"));
        }
        Expression::Ornament(ornament) => match ornament.kind() {
            OrnamentKind::Trill | OrnamentKind::HalfStepTrill | OrnamentKind::WholeStepTrill => {
                "!trill!"
            }
            OrnamentKind::Mordent
            | OrnamentKind::HalfStepMordent
            | OrnamentKind::WholeStepMordent => "!lowermordent!",
            OrnamentKind::InvertedMordent
            | OrnamentKind::HalfStepInvertedMordent
            | OrnamentKind::WholeStepInvertedMordent => "!uppermordent!",
            OrnamentKind::Turn => "!turn!",
            OrnamentKind::InvertedTurn => "!invertedturn!",
            other => {
                return Err(abc_error(format!(
                    "ABC has no decoration for {}",
                    other.class_name()
                )));
            }
        },
    })
}

fn decorations(articulations: &[Articulation], expressions: &[Expression]) -> Result<String> {
    let mut text = String::new();
    for expression in expressions {
        text.push_str(expression_text(expression)?);
    }
    // The long ones first, so the single letters stand against the note.
    let mut written = articulations
        .iter()
        .map(articulation_text)
        .collect::<Result<Vec<_>>>()?;
    written.sort_by_key(|text| std::cmp::Reverse(text.len()));
    text.push_str(&written.concat());
    Ok(text)
}

/// The start of a spanner as ABC opens one.
fn spanner_open(kind: SpannerKind) -> Result<&'static str> {
    Ok(match kind {
        SpannerKind::Slur => "(",
        SpannerKind::Crescendo => "!crescendo(!",
        SpannerKind::Diminuendo => "!diminuendo(!",
        other => {
            return Err(abc_error(format!("ABC has no {}", other.class_name())));
        }
    })
}

fn spanner_close(kind: SpannerKind) -> &'static str {
    match kind {
        SpannerKind::Crescendo => "!crescendo)!",
        SpannerKind::Diminuendo => "!diminuendo)!",
        _ => ")",
    }
}

/// What reading the tune keeps from one note to the next, across voices.
struct Reading {
    /// The signature music21 would read an unmarked note under here.
    alters: Alters,
}

/// One measure as it is written: the lines of fields before and after its
/// music, the music, and what its notes are sung to.
#[derive(Default)]
struct Written<'a> {
    /// Fields standing after the barline that opens the measure.
    opening: Vec<String>,
    music: String,
    /// Fields standing before the barline that closes it.
    closing: Vec<String>,
    /// The syllables of each note a `w:` line counts.
    sung: Vec<&'a [Lyric]>,
    /// Whether the music begins with a note, with nothing before it that
    /// music21 counts as a token of its own.
    starts_with_note: bool,
}

/// A tune: one score, its voices and what joins their notes.
struct Tune<'a> {
    metadata: Option<&'a Metadata>,
    voices: Vec<Voice<'a>>,
    /// The parts in order, each with how many voices it is written as.
    parts: Vec<(&'a Stream, usize)>,
    opens: BTreeMap<usize, Vec<SpannerKind>>,
    closes: BTreeMap<usize, Vec<SpannerKind>>,
    number: IntegerType,
}

impl<'a> Tune<'a> {
    fn new(stream: &'a Stream, number: IntegerType) -> Result<Self> {
        let mut leaf = 0;
        let mut voices = Vec::new();
        let mut parts = Vec::new();
        let holds_parts = stream.events().iter().any(|event| {
            event.element().as_stream().is_some_and(|inner| {
                matches!(inner.kind(), StreamKind::Part | StreamKind::PartStaff)
            })
        });
        if holds_parts {
            for event in stream.events() {
                match event.element() {
                    StreamElement::Stream(part) => {
                        let made = voices_of(part, &mut leaf)?;
                        parts.push((part.as_ref(), made.len()));
                        voices.extend(made);
                    }
                    element if is_sounding(element) => {
                        return Err(abc_error(
                            "a score holds notes outside its parts as well as inside them",
                        ));
                    }
                    _ => leaf += 1,
                }
            }
        } else if stream.kind() == StreamKind::Measure {
            // A measure alone is a tune of that one measure.
            let mut held = Vec::new();
            gather(stream, 0.0, &mut leaf, &mut held);
            parts.push((stream, 1));
            voices.push(Voice {
                part: stream,
                bars: vec![Bar {
                    measure: None,
                    held,
                    filler: None,
                }],
                measured: false,
                within: 0,
            });
        } else {
            let made = voices_of(stream, &mut leaf)?;
            parts.push((stream, made.len()));
            voices.extend(made);
        }

        // Where each slur and hairpin opens and closes, by leaf.
        let leaves = stream.leaves();
        let graced = |leaf: usize| {
            leaves
                .get(leaf)
                .is_some_and(|(_, element)| is_grace(element))
        };
        let mut opens: BTreeMap<usize, Vec<(usize, SpannerKind)>> = BTreeMap::new();
        let mut closes: BTreeMap<usize, Vec<(usize, SpannerKind)>> = BTreeMap::new();
        for spanner in stream.spanners() {
            spanner_open(spanner.kind())?;
            let spanned = spanner.spanned();
            let placed: Vec<usize> = spanned.iter().flatten().copied().collect();
            let (Some(first), Some(last)) = (placed.iter().min(), placed.iter().max()) else {
                continue;
            };
            // An element the score does not hold is the note a grace note
            // was copied from, which stood where the grace note stands.
            let mut start = *first;
            for _ in spanned.iter().take_while(|place| place.is_none()) {
                if start > 0 && graced(start - 1) {
                    start -= 1;
                }
            }
            let mut end = *last;
            for _ in spanned.iter().rev().take_while(|place| place.is_none()) {
                if graced(end + 1) {
                    end += 1;
                }
            }
            opens.entry(start).or_default().push((end, spanner.kind()));
            closes.entry(end).or_default().push((start, spanner.kind()));
        }
        // The spanner closing last opens first, and the one opened last
        // closes first.
        let ordered = |mut list: Vec<(usize, SpannerKind)>| {
            list.sort_by_key(|(other, _)| std::cmp::Reverse(*other));
            list.into_iter().map(|(_, kind)| kind).collect::<Vec<_>>()
        };
        Ok(Self {
            metadata: stream.metadata(),
            voices,
            parts,
            opens: opens
                .into_iter()
                .map(|(leaf, list)| (leaf, ordered(list)))
                .collect(),
            closes: closes
                .into_iter()
                .map(|(leaf, list)| (leaf, ordered(list)))
                .collect(),
            number,
        })
    }

    /// The unit note length in quarter notes: the one asked for, or the
    /// one that leaves the least to write after the notes.
    fn unit(&self, options: &ExportOptions) -> Result<(FloatType, String)> {
        if let Some((numerator, denominator)) = options.unit_length {
            if numerator == 0 || denominator == 0 {
                return Err(abc_error("the unit note length cannot be nought"));
            }
            return Ok((
                4.0 * FloatType::from(numerator) / FloatType::from(denominator),
                format!("{numerator}/{denominator}"),
            ));
        }
        const UNITS: [(FloatType, &str); 6] = [
            (0.5, "1/8"),
            (1.0, "1/4"),
            (0.25, "1/16"),
            (2.0, "1/2"),
            (0.125, "1/32"),
            (4.0, "1/1"),
        ];
        // The unit that leaves the least to write after the notes.
        let mut written = Vec::new();
        for held in self
            .voices
            .iter()
            .flat_map(|voice| &voice.bars)
            .flat_map(|bar| &bar.held)
        {
            if let Some(sounding) = sounding_of(*held)? {
                written.push(sounding.written);
            }
        }
        let mut place = 0;
        let mut least = usize::MAX;
        for (index, (unit, _)) in UNITS.iter().enumerate() {
            let cost: usize = written
                .iter()
                .map(|length| length_text(*length, *unit).map_or(8, |text| text.len()))
                .sum();
            if cost < least {
                least = cost;
                place = index;
            }
        }
        Ok((UNITS[place].0, UNITS[place].1.to_string()))
    }

    fn write(&self, options: &ExportOptions) -> Result<String> {
        let (unit, unit_text) = self.unit(options)?;
        let mut out = String::new();
        let number = self
            .metadata
            .and_then(|metadata| metadata.first_text("number"))
            .and_then(|text| text.trim().parse::<IntegerType>().ok())
            .unwrap_or(self.number);
        let _ = writeln!(out, "X:{number}");
        if let Some(metadata) = self.metadata {
            for (name, tag) in [
                ("title", 'T'),
                ("alternativeTitle", 'T'),
                ("composer", 'C'),
                ("localeOfComposition", 'O'),
            ] {
                for value in metadata.get(name) {
                    let _ = writeln!(out, "{tag}:{}", plain(value.text()));
                }
            }
        }

        // The letters ABC leaves to be given a meaning, given the meanings
        // music21 reads them with.
        for (kind, field) in [
            (ArticulationKind::Accent, "U:K=!accent!"),
            (ArticulationKind::StrongAccent, "U:k=!marcato!"),
            (ArticulationKind::Tenuto, "U:M=!tenuto!"),
        ] {
            let used = self
                .voices
                .iter()
                .flat_map(|voice| &voice.bars)
                .flat_map(|bar| &bar.held)
                .flat_map(|held| articulations_of(held.element))
                .any(|articulation| articulation.kind() == kind);
            if used {
                let _ = writeln!(out, "{field}");
            }
        }

        // What every voice opens with goes in the header, and what only
        // some do after their own `V:` line.
        let first = self.voices.first();
        let opening = |voice: &Voice<'a>| -> Opening<'a> {
            Opening::of(voice.bars.first().map_or(&[], |bar| bar.held.as_slice()))
        };
        let shared = first.map(opening).unwrap_or_default();
        let openings: Vec<Opening<'a>> = self.voices.iter().map(opening).collect();
        let all = |same: &dyn Fn(&Opening<'a>) -> bool| openings.iter().all(same);
        let shared_meter = all(&|other| other.meter == shared.meter);
        let shared_tempo = all(&|other| other.tempos == shared.tempos);

        if shared_meter {
            let _ = writeln!(
                out,
                "M:{}",
                shared.meter.map_or("none".to_string(), meter_field)
            );
        }
        let _ = writeln!(out, "L:{unit_text}");
        if shared_tempo {
            for mark in &shared.tempos {
                if let Some(field) = tempo_field(mark)? {
                    let _ = writeln!(out, "Q:{field}");
                }
            }
        }
        let clef_of = |voice: &Voice<'a>, opening: &Opening<'a>| -> Option<&'a Clef> {
            let clef = opening.clef?;
            let pitches: Vec<Pitch> = voice
                .bars
                .iter()
                .flat_map(|bar| &bar.held)
                .flat_map(|held| match held.element {
                    StreamElement::ChordSymbol(symbol) => symbol.pitches().unwrap_or_default(),
                    element => element.pitches(),
                })
                .collect();
            // A clef is left unsaid only where it is treble, ABC's own
            // default, and fits the notes best, which is what music21's
            // reader takes where none is written.
            let unsaid = clef.kind() == crate::clef::ClefKind::TrebleClef
                && *clef == Clef::best_for(&pitches, false);
            (!unsaid).then_some(clef)
        };
        // A voice's opening clef is said on its `V:` line: music21's reader
        // takes a bass clef named in the header's `K:` as lowering every
        // note two octaves, and one on a `V:` line as the clef alone.
        let key_line = |opening: &Opening<'a>| -> Result<String> {
            match opening.key {
                Some(key) => key_field(key, None),
                None => Ok("none".to_string()),
            }
        };
        let voice_line =
            |index: usize, voice: &Voice<'a>, opening: &Opening<'a>| -> Result<String> {
                let mut line = format!("V:{}", index + 1);
                if let Some(clef) = clef_of(voice, opening) {
                    let _ = write!(line, " clef={}", clef_name(clef)?);
                }
                if let Some(name) = voice.part.name().filter(|_| voice.within == 0) {
                    let _ = write!(line, " name=\"{}\"", quoted(name));
                }
                Ok(line)
            };
        let mut reading = Reading { alters: [0; 7] };
        let several = self.voices.len() > 1;
        // A first voice stating no key leaves the key to the voices that
        // state one, each after its own `V:` line.
        let key_heads = !several || shared.key.is_some();
        if !key_heads {
        } else if first.is_some() {
            let _ = writeln!(out, "K:{}", key_line(&shared)?);
            reading.alters = shared
                .key
                .and_then(signature_of)
                .map_or([0; 7], |signature| alters_of(&signature));
        } else {
            let _ = writeln!(out, "K:none");
        }

        if self.parts.iter().any(|(_, voices)| *voices > 1) {
            // Voices of one part share its staff.
            let mut next = 1;
            let groups: Vec<String> = self
                .parts
                .iter()
                .map(|(_, voices)| {
                    let ids: Vec<String> = (next..next + voices).map(|id| id.to_string()).collect();
                    next += voices;
                    if ids.len() > 1 {
                        format!("({})", ids.join(" "))
                    } else {
                        ids.concat()
                    }
                })
                .collect();
            let _ = writeln!(out, "%%score {}", groups.join(" "));
        }
        for (index, voice) in self.voices.iter().enumerate() {
            let opening = &openings[index];
            // A tune of one voice has a `V:` line only for a clef to say or
            // a part's name, which no reader takes for the part's but every
            // renderer draws.
            let line = voice_line(index, voice, opening)?;
            if several || line != "V:1" {
                let _ = writeln!(out, "{line}");
            }
            if several {
                if !shared_meter {
                    let _ = writeln!(
                        out,
                        "M:{}",
                        opening.meter.map_or("none".to_string(), meter_field)
                    );
                }
                if !shared_tempo {
                    for mark in &opening.tempos {
                        if let Some(field) = tempo_field(mark)? {
                            let _ = writeln!(out, "Q:{field}");
                        }
                    }
                }
                // The key is said again wherever the voice before left
                // another in force, or this voice opens in one of its own.
                let alters = opening
                    .key
                    .and_then(signature_of)
                    .map_or([0; 7], |signature| alters_of(&signature));
                let line = key_line(opening)?;
                let stated = opening.key.is_some();
                let differs = !key_heads || alters != reading.alters || line != key_line(&shared)?;
                if stated && differs && (index > 0 || !key_heads) {
                    let _ = writeln!(out, "K:{line}");
                }
                reading.alters = alters;
            }
            // The part's instrument, as abc2midi and abcjs take it: no
            // reader of ABC music21's or this crate's reads it back.
            if let Some(program) = program_of(voice.part) {
                let _ = writeln!(out, "%%MIDI program {program}");
            }
            self.write_voice(voice, unit, &unit_text, options, &mut reading, &mut out)?;
        }
        Ok(out)
    }

    fn write_voice(
        &self,
        voice: &Voice<'a>,
        unit: FloatType,
        unit_text: &str,
        options: &ExportOptions,
        reading: &mut Reading,
        out: &mut String,
    ) -> Result<()> {
        // The tuplet markers, over the voice from end to end: a tuplet may
        // run across a barline.
        let mut tokens = Vec::new();
        let mut places = Vec::new();
        for (bar_index, bar) in voice.bars.iter().enumerate() {
            for (held_index, held) in bar.held.iter().enumerate() {
                if let Some(sounding) = sounding_of(*held)? {
                    places.push((bar_index, held_index));
                    tokens.push(sounding);
                }
            }
        }
        let opens_on = |sounding: &Sounding<'_>| self.opens.contains_key(&sounding.held.leaf);
        let markers: BTreeMap<(usize, usize), String> = tuplet_markers(&tokens, &opens_on)
            .into_iter()
            .map(|(token, marker)| (places[token], marker))
            .collect();

        // Measures music21 cut in two are written as the one they were cut
        // from, so that it cuts them again: the barline that closed the long
        // measure is at the start of its second half, where no barline
        // written there would be read, and the meters the cut brought with
        // it are the cut's to bring again.
        let count = voice.bars.len();
        let mut joined = vec![false; count];
        let mut plans = vec![Plan::default(); count];
        if voice.measured {
            let mut meter: Option<&TimeSignature> = None;
            for index in 0..count {
                let before = meter;
                if let Some(own) = meter_of(&voice.bars[index]) {
                    meter = Some(own);
                }
                if index == 0 || joined[index - 1] {
                    continue;
                }
                let (first, second) = (&voice.bars[index - 1], &voice.bars[index]);
                let moved = second
                    .measure
                    .and_then(Stream::left_barline)
                    .is_some_and(|barline| {
                        barline.repeat_direction() != Some(RepeatDirection::Start)
                    });
                let closed = first.measure.and_then(Stream::right_barline).is_some()
                    || second.measure.and_then(Stream::right_barline).is_some();
                let bar_length = before.map(TimeSignature::bar_quarter_length);
                let full = bar_length.is_some_and(|bar| (length_of(first) - bar).abs() < EPSILON);
                // What is left of a cut measure is a whole bar or states the
                // meter that fits it.
                let remainder = meter_of(second).is_some()
                    || bar_length.is_some_and(|bar| (length_of(second) - bar).abs() < EPSILON);
                let cut = cut_note(first, second);
                if (moved || (cut == Some(true) && remainder))
                    && !closed
                    && full
                    && first.filler.is_none()
                    && second.filler.is_none()
                {
                    joined[index] = true;
                    plans[index].drop_meter = true;
                    if cut.is_some()
                        && let Some(head) = first_sounding(second)
                    {
                        let head = second.held[head];
                        plans[index].skip_head = true;
                        plans[index - 1].tail = Some(Tail {
                            extra: head.element.quarter_length(),
                            tied: notes_of(head.element).is_some_and(|notes| {
                                notes.iter().any(|note| {
                                    note.tie().map(crate::notation::Tie::tie_type)
                                        == Some(TieType::Continue)
                                })
                            }),
                            closes: self
                                .closes
                                .get(&head.leaf)
                                .map_or(&[][..], Vec::as_slice)
                                .iter()
                                .map(|kind| spanner_close(*kind))
                                .collect(),
                        });
                    }
                    // The measure after is given the old meter back.
                    if let (Some(next), Some(old)) = (voice.bars.get(index + 1), before)
                        && meter_of(next)
                            .is_some_and(|given| given.ratio_string() == old.ratio_string())
                    {
                        plans[index + 1].drop_meter = true;
                    }
                    meter = before;
                }
            }
        }

        let mut written = Vec::new();
        for (index, bar) in voice.bars.iter().enumerate() {
            written.push(self.write_bar(
                voice,
                index,
                bar,
                unit,
                unit_text,
                &markers,
                reading,
                &plans[index],
            )?);
        }
        // The barline after each measure, with the ending the next opens.
        let barline_after = |index: usize| -> (String, bool) {
            if !voice.measured {
                return (String::new(), false);
            }
            let bar = &voice.bars[index];
            let right = if joined[index] {
                bar.measure.and_then(Stream::left_barline)
            } else {
                bar.measure.and_then(Stream::right_barline)
            };
            let next = voice.bars.get(index + 1).and_then(|bar| bar.measure);
            // After the last measure there is nothing to keep a barline off.
            let field_can_follow = written
                .get(index + 1)
                .is_none_or(|next| next.opening.is_empty() && next.starts_with_note);
            barline_text(
                right,
                next.and_then(Stream::left_barline),
                next.and_then(Stream::ending),
                false,
                field_can_follow,
            )
        };

        let has_lyrics = written
            .iter()
            .any(|bar| bar.sung.iter().any(|lyrics| !lyrics.is_empty()));
        let per_line = options.measures_per_line.max(1);
        let last = written.len().saturating_sub(1);
        let mut line = String::new();
        let mut on_line = 0;
        let mut sung: Vec<&[Lyric]> = Vec::new();
        if voice.measured {
            let first = voice.bars.first().and_then(|bar| bar.measure);
            line.push_str(
                &barline_text(
                    None,
                    first.and_then(Stream::left_barline),
                    first.and_then(Stream::ending),
                    true,
                    false,
                )
                .0,
            );
        }
        for (index, bar) in written.iter().enumerate() {
            if !line.is_empty() && !line.ends_with(' ') && !bar.music.is_empty() {
                line.push(' ');
            }
            line.push_str(&bar.music);
            sung.extend(bar.sung.iter().copied());
            if joined.get(index + 1) == Some(&true) {
                continue;
            }
            on_line += 1;
            let next = written.get(index + 1);
            let (barline, keep_off) = barline_after(index);
            // The fields the next measure opens with. A barline that is not
            // the next measure's to take is kept off it by a field between
            // them, which music21 passes over: the unit note length again.
            let said_again = [format!("L:{unit_text}")];
            let opening: &[String] = match next {
                Some(next) if !next.opening.is_empty() => &next.opening,
                Some(_) if keep_off => &said_again,
                _ => &[],
            };
            let fields_follow = !opening.is_empty();
            // A word is not broken across lines of words.
            let mid_word = sung
                .iter()
                .rev()
                .find_map(|lyrics| lyrics.first())
                .is_some_and(|lyric| lyric.raw_text().ends_with('-'));
            let breaks = index == last
                || !bar.closing.is_empty()
                || fields_follow
                || (on_line >= per_line && !(has_lyrics && mid_word));
            if !breaks {
                if !barline.is_empty() {
                    let _ = write!(line, " {barline}");
                }
                continue;
            }
            let words = if has_lyrics {
                lyric_lines(&sung)
            } else {
                Vec::new()
            };
            sung.clear();
            on_line = 0;
            // Words and fields stand before the barline: after it and before
            // a note, music21 passes a field over.
            let before_bar = index != last && !words.is_empty() || !bar.closing.is_empty();
            if !before_bar && !barline.is_empty() {
                let _ = write!(line, " {barline}");
            }
            push_line(out, &line);
            line.clear();
            for words in words {
                let _ = writeln!(out, "w:{words}");
            }
            for field in &bar.closing {
                let _ = writeln!(out, "{field}");
            }
            if before_bar {
                line.push_str(&barline);
            }
            if !opening.is_empty() {
                push_line(out, &line);
                line.clear();
                for field in opening {
                    let _ = writeln!(out, "{field}");
                }
            }
        }
        push_line(out, &line);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn write_bar(
        &self,
        voice: &Voice<'a>,
        index: usize,
        bar: &Bar<'a>,
        unit: FloatType,
        unit_text: &str,
        markers: &BTreeMap<(usize, usize), String>,
        reading: &mut Reading,
        plan: &Plan,
    ) -> Result<Written<'a>> {
        let mut written = Written::default();
        if let Some(length) = bar.filler {
            if length > EPSILON {
                written.music = format!("x{}", length_text(length, unit)?);
            }
            return Ok(written);
        }
        let end = bar
            .held
            .iter()
            .map(|held| held.offset + held.element.quarter_length())
            .fold(0.0, FloatType::max);
        let first = index == 0;
        let skipped = first_sounding(bar).filter(|_| plan.skip_head);
        let tailed = last_sounding(bar).filter(|_| plan.tail.is_some());

        // How the key is said, where the measure states one: after the
        // barline, where the measure's own notes are read under it, or
        // before the barline closing it, where they are read under the old.
        let stated = bar
            .held
            .iter()
            .find(|held| held.offset < EPSILON && signature_of(held.element).is_some());
        let mut key_closes = false;
        let mut silent_key = None;
        if voice.measured && !first {
            let unmarked = unmarked_notes(bar, skipped);
            let fits = |alters: &Alters| {
                unmarked
                    .iter()
                    .all(|(letter, alter)| alters[*letter] == *alter)
            };
            match stated.and_then(|held| signature_of(held.element)) {
                Some(signature) => {
                    let alters = alters_of(&signature);
                    if !fits(&alters) && fits(&reading.alters) {
                        key_closes = true;
                    } else {
                        reading.alters = alters;
                    }
                }
                None if !fits(&reading.alters) => {
                    // The notes were read under a key the measure does not
                    // state: one said alone after a barline, which music21
                    // reads the notes under and then passes over.
                    let in_force = self.key_in_force(voice, index);
                    let candidates = in_force.into_iter().chain((-7..=7).map(KeySignature::new));
                    silent_key = candidates
                        .map(|signature| (alters_of(&signature), signature))
                        .find(|(alters, _)| fits(alters));
                }
                None => {}
            }
        }

        // The fields the measure opens with. Those of the first measure are
        // in the header.
        let mut opening_clef = None;
        for held in &bar.held {
            if held.offset > EPSILON || first || !voice.measured {
                continue;
            }
            match held.element {
                StreamElement::TimeSignature(meter) if !plan.drop_meter => {
                    written.opening.push(format!("M:{}", meter_field(meter)));
                }
                StreamElement::MetronomeMark(mark) if end > EPSILON => {
                    if let Some(field) = tempo_field(mark)? {
                        written.opening.push(format!("Q:{field}"));
                    }
                }
                StreamElement::Clef(clef) => opening_clef = Some(clef),
                StreamElement::Key(_) | StreamElement::KeySignature(_) => {
                    let field = format!("K:{}", key_field(held.element, None)?);
                    if key_closes {
                        written.closing.push(field);
                    } else {
                        written.opening.push(field);
                    }
                }
                _ => {}
            }
        }

        let mut music = Vec::new();
        let mut cursor = 0.0;
        // What waits for the next note: the chord symbol and the words and
        // signs written before it, and the grace notes leaning on it.
        let mut symbol: Option<String> = None;
        let mut before = String::new();
        let mut waiting_since: Option<FloatType> = None;
        let mut graces = String::new();
        let mut grace_slash = false;
        let mut grace_closes = String::new();
        let mut lead = String::new();
        let mut first_token = true;
        if let Some(clef) = opening_clef {
            lead.push_str(&format!("[K:clef={}]", clef_name(clef)?));
        }

        for (held_index, held) in bar.held.iter().enumerate() {
            let at_start = held.offset < EPSILON;
            let header = at_start && (first || voice.measured);
            match held.element {
                StreamElement::Stream(_) | StreamElement::Instrument(_) => {}
                StreamElement::Clef(clef) => {
                    if !header {
                        lead.push_str(&format!("[K:clef={}]", clef_name(clef)?));
                    }
                }
                StreamElement::TimeSignature(meter) => {
                    if !header {
                        self.field(
                            voice,
                            &mut music,
                            &mut lead,
                            format!("M:{}", meter_field(meter)),
                        );
                    }
                }
                StreamElement::Key(_) | StreamElement::KeySignature(_) => {
                    if !header {
                        let field = format!("K:{}", key_field(held.element, None)?);
                        self.field(voice, &mut music, &mut lead, field);
                        if !voice.measured
                            && let Some(signature) = signature_of(held.element)
                        {
                            reading.alters = alters_of(&signature);
                        }
                    }
                }
                StreamElement::MetronomeMark(mark) => {
                    let Some(field) = tempo_field(mark)? else {
                        continue;
                    };
                    if header && (first || end > EPSILON) {
                        // In the header, or among the measure's opening
                        // fields.
                    } else if voice.measured && (held.offset - end).abs() < EPSILON {
                        written.closing.push(format!("Q:{field}"));
                    } else {
                        self.field(voice, &mut music, &mut lead, format!("Q:{field}"));
                    }
                }
                StreamElement::TempoText(text) => {
                    let field = format!("Q:\"{}\"", quoted(text.text()));
                    self.field(voice, &mut music, &mut lead, field);
                }
                StreamElement::ChordSymbol(chord_symbol) => {
                    if symbol.is_some() {
                        return Err(abc_error("ABC writes one chord symbol on a note"));
                    }
                    symbol = Some(format!("\"{}\"", symbol_text(chord_symbol)));
                    waiting_since.get_or_insert(held.offset);
                }
                StreamElement::Dynamic(dynamic) => {
                    let value = dynamic.value();
                    if value.is_empty() || !value.chars().all(|c| c.is_ascii_alphabetic()) {
                        return Err(abc_error(format!("ABC has no dynamic {value:?}")));
                    }
                    let _ = write!(before, "!{value}!");
                    waiting_since.get_or_insert(held.offset);
                }
                StreamElement::TextExpression(expression) => {
                    let side = if expression.placement() == Some(Placement::Below) {
                        '_'
                    } else {
                        '^'
                    };
                    let _ = write!(before, "\"{side}{}\"", quoted(expression.content()));
                    waiting_since.get_or_insert(held.offset);
                }
                StreamElement::RepeatExpression(mark) => {
                    let sign = match mark.kind() {
                        RepeatExpressionKind::Coda => "!coda!".to_string(),
                        RepeatExpressionKind::Segno => "!segno!".to_string(),
                        RepeatExpressionKind::Fine => "!fine!".to_string(),
                        RepeatExpressionKind::DaCapo => "!D.C.!".to_string(),
                        RepeatExpressionKind::DalSegno => "!D.S.!".to_string(),
                        _ => format!("\"^{}\"", quoted(mark.text())),
                    };
                    before.push_str(&sign);
                    waiting_since.get_or_insert(held.offset);
                }
                // A barline inside a measure is drawn and not heard, and
                // music21's exporters write nothing for one either.
                // How the page is laid out is not the tune's to say.
                StreamElement::Barline(_)
                | StreamElement::Layout(_)
                | StreamElement::TextBox(_) => {}
                StreamElement::PedalObject(_) => {
                    return Err(abc_error("ABC has no pedal bounces or gaps"));
                }
                StreamElement::RehearsalMark(_) => {
                    return Err(abc_error("ABC has no rehearsal marks"));
                }
                StreamElement::Break(_) => {
                    return Err(abc_error(
                        "ABC writes no manuscript's line, page or column breaks",
                    ));
                }
                StreamElement::MetricModulation(_) => {
                    return Err(abc_error("ABC has no metric modulations"));
                }
                StreamElement::Unpitched(_) | StreamElement::PercussionChord(_) => {
                    return Err(abc_error("ABC has no unpitched notes"));
                }
                StreamElement::Note(_) | StreamElement::Chord(_) | StreamElement::Rest(_) => {
                    if skipped == Some(held_index) {
                        cursor = held.offset + held.element.quarter_length();
                        continue;
                    }
                    let tail = plan.tail.as_ref().filter(|_| tailed == Some(held_index));
                    let sounding = sounding_of(*held)?
                        .ok_or_else(|| abc_error("a note with nothing to write it from"))?;
                    let opens = self.opens.get(&held.leaf).map_or(&[][..], Vec::as_slice);
                    let closes: String = self
                        .closes
                        .get(&held.leaf)
                        .map_or(&[][..], Vec::as_slice)
                        .iter()
                        .map(|kind| spanner_close(*kind))
                        .collect();
                    let marker = markers.get(&(index, held_index));
                    if marker.is_some() || !opens.is_empty() {
                        first_token = false;
                    }
                    let mut opening = String::new();
                    for kind in opens {
                        opening.push_str(spanner_open(*kind)?);
                    }
                    if sounding.grace {
                        if graces.is_empty() {
                            lead.push_str(marker.map_or("", String::as_str));
                            lead.push_str(&opening);
                            grace_slash = sounding
                                .duration
                                .grace()
                                .is_some_and(crate::duration::Grace::slash);
                        } else if marker.is_some() || !opening.is_empty() {
                            // A slur opened on a later grace note is opened
                            // before the group.
                            lead.push_str(&opening);
                        }
                        graces.push_str(&self.note_text(&sounding, unit, &reading.alters, None)?);
                        grace_closes.push_str(&closes);
                        first_token = false;
                        continue;
                    }

                    // A rest for time nothing fills.
                    if held.offset > cursor + EPSILON {
                        music.push(format!("x{}", length_text(held.offset - cursor, unit)?));
                        first_token = false;
                    } else if held.offset < cursor - EPSILON {
                        return Err(abc_error(
                            "notes sound together outside a chord; ABC writes one line to a voice",
                        ));
                    }
                    if waiting_since.is_some_and(|since| since < held.offset - EPSILON) {
                        return Err(abc_error(
                            "a chord symbol, dynamic or text stands where no note starts",
                        ));
                    }
                    waiting_since = None;

                    let mut token = std::mem::take(&mut lead);
                    if !graces.is_empty() {
                        let _ = write!(
                            token,
                            "{{{}{}}}{}",
                            if grace_slash { "/" } else { "" },
                            std::mem::take(&mut graces),
                            std::mem::take(&mut grace_closes)
                        );
                        token.push_str(marker.map_or("", String::as_str));
                    } else if let Some(marker) = marker {
                        token.push_str(marker);
                    }
                    token.push_str(&opening);
                    if first_token
                        && token.is_empty()
                        && !articulations_of(held.element).iter().any(is_lettered)
                    {
                        written.starts_with_note = true;
                    }
                    first_token = false;
                    token.push_str(symbol.take().as_deref().unwrap_or(""));
                    token.push_str(&std::mem::take(&mut before));
                    token.push_str(&self.note_text(&sounding, unit, &reading.alters, tail)?);
                    token.push_str(&closes);
                    token.push_str(tail.map_or("", |tail| tail.closes.as_str()));
                    music.push(token);
                    cursor = held.offset + held.element.quarter_length();

                    match held.element {
                        StreamElement::Note(note) => written.sung.push(note.lyrics()),
                        StreamElement::Chord(chord) => written.sung.push(chord.lyrics()),
                        _ => {}
                    }
                    // Notes under one beam are written without a space.
                    let beams = match held.element {
                        StreamElement::Note(note) => Some(note.beams()),
                        StreamElement::Chord(chord) => Some(chord.beams()),
                        _ => None,
                    };
                    if !beams.is_some_and(beamed_on) {
                        music.push(" ".to_string());
                    }
                }
            }
        }
        if symbol.is_some() || !before.is_empty() {
            return Err(abc_error(
                "a chord symbol, dynamic or text stands where no note starts",
            ));
        }
        // Grace notes with no note after them, and whatever else waited.
        if !graces.is_empty() {
            lead.push_str(&format!(
                "{{{}{graces}}}{grace_closes}",
                if grace_slash { "/" } else { "" }
            ));
        }
        if !lead.is_empty() {
            music.push(lead);
        }
        written.music = music.concat().trim().to_string();
        if written.music.is_empty() && voice.measured {
            // A measure of nothing is a space.
            written.music = "y".to_string();
        }

        // One field alone after a barline and before a note is passed over
        // by music21; with the unit note length said again after it, it and
        // the measure are read together.
        if let Some((alters, signature)) =
            silent_key.filter(|_| written.opening.is_empty() && written.starts_with_note)
        {
            {
                written
                    .opening
                    .push(format!("K:{}", key_field(&signature.into(), None)?));
                reading.alters = alters;
                // The notes were written before the key was known to be
                // readable under; write them again.
                let again =
                    self.write_bar(voice, index, bar, unit, unit_text, markers, reading, plan)?;
                written.music = again.music;
            }
        } else if written.opening.len() == 1 {
            written.opening.push(format!("L:{unit_text}"));
        }
        if key_closes && let Some(signature) = stated.and_then(|held| signature_of(held.element)) {
            reading.alters = alters_of(&signature);
        }
        Ok(written)
    }

    /// A field in the middle of the music: on a line of its own where the
    /// voice has no measures, in brackets where it has.
    fn field(&self, voice: &Voice<'a>, music: &mut Vec<String>, lead: &mut String, field: String) {
        if voice.measured {
            let _ = write!(lead, "[{field}]");
        } else {
            music.push(format!("\n{field}\n"));
        }
    }

    /// The key signature in force at a measure of a voice.
    fn key_in_force(&self, voice: &Voice<'a>, index: usize) -> Option<KeySignature> {
        voice.bars[..=index]
            .iter()
            .rev()
            .flat_map(|bar| bar.held.iter().rev())
            .find_map(|held| signature_of(held.element))
    }

    /// A note, chord or rest with its length, its decorations before it and
    /// its tie after it.
    fn note_text(
        &self,
        sounding: &Sounding<'a>,
        unit: FloatType,
        alters: &Alters,
        tail: Option<&Tail>,
    ) -> Result<String> {
        let length = length_text(sounding.written + tail.map_or(0.0, |tail| tail.extra), unit)?;
        let tied = |note: &Note| match tail {
            Some(tail) => tail.tied,
            None => matches!(
                note.tie().map(crate::notation::Tie::tie_type),
                Some(TieType::Start | TieType::Continue)
            ),
        };
        Ok(match sounding.held.element {
            StreamElement::Note(note) => {
                // A grace note carries its own decorations, inside the
                // braces: `{vfga}` bows the first grace note down.
                let marks = decorations(note.articulations(), note.expressions())?;
                format!(
                    "{marks}{}{length}{}",
                    pitch_text(note.pitch(), alters)?,
                    if tied(note) { "-" } else { "" }
                )
            }
            StreamElement::Chord(chord) => chord_text(chord, alters, &length, tied)?,
            StreamElement::Rest(rest) => rest_text(rest, &length)?,
            _ => String::new(),
        })
    }
}

/// What a voice's first measure opens with.
#[derive(Clone, Default)]
struct Opening<'a> {
    meter: Option<&'a TimeSignature>,
    key: Option<&'a StreamElement>,
    clef: Option<&'a Clef>,
    tempos: Vec<&'a MetronomeMark>,
}

impl<'a> Opening<'a> {
    fn of(held: &[Held<'a>]) -> Self {
        let mut opening = Self::default();
        for held in held.iter().filter(|held| held.offset < EPSILON) {
            match held.element {
                StreamElement::TimeSignature(meter) => opening.meter = Some(meter),
                StreamElement::Key(_) | StreamElement::KeySignature(_) => {
                    opening.key = Some(held.element);
                }
                StreamElement::Clef(clef) => opening.clef = Some(clef),
                StreamElement::MetronomeMark(mark) => opening.tempos.push(mark),
                _ => {}
            }
        }
        opening
    }
}

/// Whether a note is beamed to the one after it.
fn beamed_on(beams: &Beams) -> bool {
    matches!(
        beams
            .beams()
            .first()
            .and_then(crate::notation::Beam::beam_type),
        Some(BeamType::Start | BeamType::Continue)
    )
}

/// The notes of a measure music21 would read with no accidental written:
/// each letter and how far it is moved.
fn unmarked_notes(bar: &Bar<'_>, skipped: Option<usize>) -> Vec<(usize, IntegerType)> {
    let mut notes = Vec::new();
    let mut take = |pitch: &Pitch| {
        if !shows_accidental(pitch) {
            notes.push((
                letter_index(pitch.step().as_char()),
                pitch.alter().round() as IntegerType,
            ));
        }
    };
    for (index, held) in bar.held.iter().enumerate() {
        if skipped == Some(index) {
            continue;
        }
        match held.element {
            StreamElement::Note(note) => take(note.pitch()),
            StreamElement::Chord(chord) => {
                chord.notes().iter().map(Note::pitch).for_each(&mut take)
            }
            _ => {}
        }
    }
    notes
}

/// Whether a pitch carries an accidental that is to be seen.
fn shows_accidental(pitch: &Pitch) -> bool {
    pitch
        .written_accidental()
        .is_some_and(|accidental| accidental.display_status() != Some(false))
}

/// A pitch as ABC writes one: an accidental where it shows one or the key
/// in force would give it another, the letter, and the octave.
fn pitch_text(pitch: &Pitch, alters: &Alters) -> Result<String> {
    if pitch
        .microtone()
        .is_some_and(|microtone| microtone.alter() != 0.0)
    {
        return Err(abc_error("ABC has no microtones"));
    }
    let alter = pitch.alter();
    if alter.fract() != 0.0 {
        return Err(abc_error("ABC has no quarter-tone accidentals"));
    }
    let alter = alter as IntegerType;
    let letter = pitch.step().as_char();
    let marks = if shows_accidental(pitch) || alters[letter_index(letter)] != alter {
        accidental_marks(alter)?
    } else {
        ""
    };
    let octave = pitch.octave().unwrap_or(4);
    Ok(if octave >= 5 {
        format!(
            "{marks}{}{}",
            letter.to_ascii_lowercase(),
            "'".repeat((octave - 5) as usize)
        )
    } else {
        format!("{marks}{letter}{}", ",".repeat((4 - octave) as usize))
    })
}

fn chord_text(
    chord: &Chord,
    alters: &Alters,
    length: &str,
    tied: impl Fn(&Note) -> bool,
) -> Result<String> {
    let mut text = decorations(chord.articulations(), chord.expressions())?;
    text.push('[');
    for note in chord.notes() {
        text.push_str(&pitch_text(note.pitch(), alters)?);
        // A tie written after the chord would be read onto the next note.
        if tied(note) {
            text.push('-');
        }
    }
    text.push(']');
    text.push_str(length);
    Ok(text)
}

fn rest_text(rest: &Rest, length: &str) -> Result<String> {
    let marks = decorations(rest.articulations(), rest.expressions())?;
    let tied = matches!(
        rest.tie().map(crate::notation::Tie::tie_type),
        Some(TieType::Start | TieType::Continue)
    );
    Ok(format!(
        "{marks}{}{length}{}",
        if rest.hidden() { 'x' } else { 'z' },
        if tied { "-" } else { "" }
    ))
}

/// A chord symbol as it is written between quotation marks: a flat as `b`.
fn symbol_text(symbol: &ChordSymbol) -> String {
    if symbol.is_no_chord() {
        return quoted(symbol.kind_text().unwrap_or("N.C."));
    }
    let figure: Vec<char> = symbol.figure().chars().collect();
    let mut text = String::new();
    for (index, character) in figure.iter().enumerate() {
        let after_letter = index > 0 && matches!(figure[index - 1], 'A'..='G');
        text.push(if *character == '-' && after_letter {
            'b'
        } else {
            *character
        });
    }
    quoted(&text)
}

/// The barline between two measures, or before the first or after the last,
/// with the ending the second opens: from the barline closing the one and
/// the barline and ending opening the other. The second answer is whether
/// the measure after must be kept from taking the barline as its own.
///
/// One barline written is read as closing the measure before it and opening
/// the one after, unless it is a repeat, which is read on its own side only.
/// Two different ones are written one after the other.
fn barline_text(
    right: Option<&Barline>,
    left: Option<&Barline>,
    ending: Option<&Ending>,
    opens_tune: bool,
    field_can_follow: bool,
) -> (String, bool) {
    let token = |barline: Option<&Barline>| -> Option<&'static str> {
        let barline = barline?;
        match barline.repeat_direction() {
            Some(RepeatDirection::End) => Some(":|"),
            Some(RepeatDirection::Start) => Some("|:"),
            None => match barline.bar_type() {
                BarlineType::Double => Some("||"),
                BarlineType::Final => Some("|]"),
                BarlineType::HeavyLight => Some("[|"),
                BarlineType::Dotted | BarlineType::Dashed => Some(":"),
                _ => None,
            },
        }
    };
    let repeats = |barline: Option<&Barline>| {
        barline.is_some_and(|barline| barline.repeat_direction().is_some())
    };
    let ending = ending.filter(|ending| ending.starts() && !ending.numbers().is_empty());
    let mut keep_off = false;
    let mut text = match (token(right), token(left)) {
        (Some(":|"), Some("|:")) => "::".to_string(),
        (Some(closing), Some(opening)) if closing == opening => closing.to_string(),
        // A dotted barline before another would be read with it as a repeat.
        (Some(":"), Some(opening)) => format!(": {opening}"),
        (Some(closing), Some(opening)) => format!("{closing}{opening}"),
        (Some(closing), None) if repeats(right) || opens_tune => closing.to_string(),
        // A barline that is not a repeat is read as the next measure's too.
        // A field after it keeps it off, and so does a plain barline.
        (Some(closing), None) if field_can_follow && ending.is_none() => {
            keep_off = true;
            closing.to_string()
        }
        (Some(closing), None) => format!("{closing} |"),
        (None, Some(opening)) if opens_tune || repeats(left) => opening.to_string(),
        // The plain barline closes the measure before and the other opens
        // the one after.
        (None, Some(opening)) => format!("| {opening}"),
        // Nothing is written before the first measure but what it opens
        // with.
        (None, None) if opens_tune => String::new(),
        (None, None) => "|".to_string(),
    };
    if let Some(ending) = ending {
        let numbers = ending
            .numbers()
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        if !numbers.is_empty() {
            // `|1` and `:|2` are the forms music21 reads; after any other
            // barline the ending is opened by its bracket.
            if matches!(text.as_str(), "|" | ":|") && matches!(numbers.as_str(), "1" | "2") {
                text.push_str(&numbers);
            } else if matches!(numbers.as_str(), "1" | "2") {
                let _ = write!(text, "[{numbers}");
            } else if text.is_empty() {
                let _ = write!(text, "|{numbers}");
            } else {
                text.push_str(&numbers);
            }
        }
    }
    (text, keep_off)
}

/// The `w:` lines for the notes of one line of music, a line for each verse.
fn lyric_lines(sung: &[&[Lyric]]) -> Vec<String> {
    let mut numbers: Vec<IntegerType> = sung
        .iter()
        .flat_map(|lyrics| lyrics.iter().map(Lyric::number))
        .collect();
    numbers.sort_unstable();
    numbers.dedup();
    if numbers.is_empty() {
        // Notes with no words still take their place, so the next line of
        // words starts at its own notes.
        numbers.push(1);
    }
    numbers
        .into_iter()
        .map(|number| {
            let mut words: Vec<String> = Vec::new();
            // The syllables of the word being written, a skipped note an
            // empty one.
            let mut word: Option<Vec<String>> = None;
            for lyrics in sung {
                let lyric = lyrics.iter().find(|lyric| lyric.number() == number);
                match lyric {
                    None => match &mut word {
                        Some(syllables) => syllables.push(String::new()),
                        None => words.push("*".to_string()),
                    },
                    Some(lyric) => {
                        let raw = lyric.raw_text();
                        let continues = raw.len() > 1 && raw.ends_with('-');
                        let syllable = lyric.text().replace(' ', "~").replace('-', "\\-");
                        let syllable = if syllable.is_empty() {
                            "*".to_string()
                        } else {
                            syllable
                        };
                        let joins = raw.len() > 1 && raw.starts_with('-');
                        match &mut word {
                            Some(syllables) if joins => syllables.push(syllable),
                            _ => {
                                if let Some(syllables) = word.take() {
                                    words.push(syllables.join("-"));
                                }
                                word = Some(vec![syllable]);
                            }
                        }
                        if !continues && let Some(syllables) = word.take() {
                            words.push(syllables.join("-"));
                        }
                    }
                }
            }
            if let Some(mut syllables) = word.take() {
                // A word left open says so with the hyphen after it.
                syllables.push(String::new());
                while syllables.len() > 2 && syllables[syllables.len() - 2].is_empty() {
                    syllables.pop();
                }
                words.push(syllables.join("-"));
            }
            words.join(" ")
        })
        .collect()
}

fn push_line(out: &mut String, line: &str) {
    // A field in the middle of a voice with no measures brings its own
    // line breaks.
    for piece in line.split('\n') {
        let piece = piece.trim();
        if !piece.is_empty() {
            out.push_str(piece);
            out.push('\n');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abc::from_abc;

    /// What a tune is written as, having checked that it reads back as the
    /// score it was written from.
    fn round_trip(tune: &str) -> String {
        let score = from_abc(tune).unwrap();
        let written = to_abc(&score, &ExportOptions::default()).unwrap();
        assert_eq!(
            format!("{:#?}", from_abc(&written).unwrap()),
            format!("{score:#?}"),
            "read back differently:\n{written}"
        );
        written
    }

    #[test]
    fn a_tune_is_written_with_its_header() {
        let written = round_trip(
            "X:7\nT:Tune\nT:Other\nC:Anon\nO:Here\nM:6/8\nL:1/8\nQ:3/8=120\nK:Ador\nABc def|gfe dcB|A6|]\n",
        );
        assert_eq!(
            written,
            "X:7\nT:Tune\nT:Other\nC:Anon\nO:Here\nM:6/8\nL:1/8\nQ:3/8=120\nK:Ador\nABc def | gfe dcB | A6 |]\n"
        );
    }

    #[test]
    fn accidentals_octaves_and_lengths_are_written() {
        let written = round_trip("X:1\nM:4/4\nL:1/4\nK:G\n^C,/ _e'3/2 =F f/4 z3/4|c4|C4|]\n");
        assert!(
            written.contains("L:1/8\nK:G\n^C, _e'3 =F2 f/ z3/2 | c8 | C8 |]"),
            "{written}"
        );
    }

    /// A part of whole notes in measures, holding what `opening` gives its
    /// first measure.
    fn part_of(names: &[&str], opening: Vec<StreamElement>) -> Stream {
        let mut part = Stream::with_kind(StreamKind::Part);
        for (index, name) in names.iter().enumerate() {
            let mut measure = Stream::with_kind(StreamKind::Measure);
            if index == 0 {
                for element in &opening {
                    measure.insert(0.0, element.clone());
                }
            }
            measure.insert(
                0.0,
                Note::from_name(name)
                    .unwrap()
                    .with_duration(Duration::whole()),
            );
            part.insert(index as FloatType * 4.0, measure);
        }
        part
    }

    #[test]
    fn a_part_says_its_name_and_its_instrument_s_program() {
        let mut part = part_of(
            &["C4", "D4"],
            vec![StreamElement::TimeSignature(
                TimeSignature::new(4, 4).unwrap(),
            )],
        );
        part.set_name(Some("Guitar".to_string()));
        let mut guitar = crate::instrument::Instrument::new();
        guitar.set_midi_program(Some(24));
        part.insert(0.0, StreamElement::Instrument(Box::new(guitar)));
        let written = to_abc(&part, &ExportOptions::default()).unwrap();
        assert!(
            written.contains(
                "V:1 name=\"Guitar\"
%%MIDI program 24
"
            ),
            "{written}"
        );
    }

    #[test]
    fn a_clef_other_than_treble_is_said_even_where_the_notes_imply_it() {
        let low = part_of(
            &["C2", "G2"],
            vec![
                StreamElement::Clef(Clef::of_kind(crate::clef::ClefKind::BassClef)),
                StreamElement::TimeSignature(TimeSignature::new(4, 4).unwrap()),
            ],
        );
        let written = to_abc(&low, &ExportOptions::default()).unwrap();
        assert!(written.contains("clef=bass"), "{written}");
        let high = part_of(
            &["C5", "G5"],
            vec![
                StreamElement::Clef(Clef::of_kind(crate::clef::ClefKind::TrebleClef)),
                StreamElement::TimeSignature(TimeSignature::new(4, 4).unwrap()),
            ],
        );
        let written = to_abc(&high, &ExportOptions::default()).unwrap();
        assert!(!written.contains("clef="), "{written}");
    }
}
