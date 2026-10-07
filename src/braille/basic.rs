//! One element of music in braille: music21's `braille.basic`.
//!
//! Each function answers a [`Transcription`]: the braille, and the English
//! music21 writes beside it, one line a sign, which a segment's debugging
//! listing shows.

use super::lookup::{self, symbol};
use crate::{
    bar::Barline,
    chord::Chord,
    clef::Clef,
    defaults::{FloatType, IntegerType},
    duration::Duration,
    dynamics::Dynamic,
    error::{Error, Result},
    expressions::{Expression, TextExpression},
    instrument::Instrument,
    meter::TimeSignature,
    notation::TieType,
    note::Note,
    pitch::Pitch,
    rest::Rest,
    tempo::{MetronomeMark, TempoText},
};

/// An element in braille, and the English music21 writes for it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Transcription {
    /// The braille.
    pub braille: String,
    /// What each sign says, a line each: music21's `brailleEnglish`.
    pub english: Vec<String>,
}

impl Transcription {
    fn exception(english: Vec<String>) -> Self {
        Self {
            braille: symbol("basic_exception"),
            english,
        }
    }
}

/// How a note stands among its neighbours, which a segment works out and
/// the note's braille shows: the attributes music21 sets on a note for the
/// time it is transcribed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoteContext {
    /// The note begins a slur long enough to be written with brackets.
    pub begin_long_bracket_slur: bool,
    /// The note ends such a slur.
    pub end_long_bracket_slur: bool,
    /// The note begins a long slur written doubled.
    pub begin_long_double_slur: bool,
    /// The note ends such a slur.
    pub end_long_double_slur: bool,
    /// The note is under a short slur, which is written after each note.
    pub short_slur: bool,
    /// The note begins a beamed group, which shows a tuplet's sign.
    pub beam_start: bool,
    /// The note continues a beamed group, written as an eighth.
    pub beam_continue: bool,
}

/// music21's name for a duration's value: `complex` for one of several
/// values and `zero` for none.
pub(crate) fn duration_type(duration: &Duration) -> String {
    let values = duration.written_values();
    match values.as_slice() {
        [] => "zero".to_string(),
        [one] => one.duration_type().map_or_else(
            || "inexpressible".to_string(),
            |value| value.music21_name().to_string(),
        ),
        _ => "complex".to_string(),
    }
}

/// The dots of a duration's first value: music21's `dots`.
pub(crate) fn duration_dots(duration: &Duration) -> u32 {
    duration
        .written_values()
        .first()
        .map_or(0, |value| value.dots())
}

/// A length as music21's `repr` writes it: a float, or a fraction where
/// music21 keeps one.
pub(crate) fn quarter_length_repr(length: FloatType) -> String {
    match crate::duration::limited_fraction(length, 65535) {
        Some((numerator, denominator))
            if denominator != 0
                && !(denominator as u64).is_power_of_two()
                && ((numerator as FloatType / denominator as FloatType) - length).abs() < 1e-9 =>
        {
            format!("{numerator}/{denominator}")
        }
        _ => crate::statistics::python_repr(length),
    }
}

/// Text centred in so many characters as Python's `str.center` centres it,
/// the odd one of padding to the left only where the width and the padding
/// are both odd.
pub(crate) fn center(text: &str, width: usize, fill: &str) -> String {
    let length = text.chars().count();
    if width <= length {
        return text.to_string();
    }
    let margin = width - length;
    let left = margin / 2 + (margin & width & 1);
    format!("{}{text}{}", fill.repeat(left), fill.repeat(margin - left))
}

/// A barline: music21's `barlineToBraille`. A barline of no braille sign
/// is the sign of an untranscribable element.
pub fn barline_to_braille(barline: &Barline) -> Transcription {
    let name = barline.bar_type().as_str();
    match lookup::barline(name) {
        Some(braille) => Transcription {
            english: vec![format!("Barline {name} {braille}")],
            braille,
        },
        None => Transcription::exception(vec![format!("Barline {name} None")]),
    }
}

/// The octave sign of an octave, repeated past the ends: music21's
/// `pitchToOctave`.
pub fn pitch_to_octave(octave: IntegerType) -> String {
    let octave = i64::from(octave);
    if let Some(sign) = lookup::octave(octave) {
        sign
    } else if octave < 1 {
        lookup::octave(1)
            .unwrap_or_default()
            .repeat((2 - octave) as usize)
    } else {
        lookup::octave(7)
            .unwrap_or_default()
            .repeat((octave - 6) as usize)
    }
}

/// Whether braille writes this pitch's accidental: one music21 has not
/// hidden.
fn handle_pitch_with_accidental(pitch: &Pitch, braille: &mut String, english: &mut Vec<String>) {
    let Some(accidental) = pitch.written_accidental() else {
        return;
    };
    if accidental.display_status() == Some(false) {
        return;
    }
    let parenthesis = accidental.display_style() == "parentheses";
    if parenthesis {
        let sign = symbol("braille-music-parenthesis");
        braille.push_str(&sign);
        english.push(format!("Parenthesis {sign}"));
    }
    // An accidental braille has no sign for is left out, as music21 leaves
    // it out after warning.
    let Some(sign) = lookup::accidental(accidental.name()) else {
        return;
    };
    braille.push_str(&sign);
    english.push(format!("Accidental {} {sign}", accidental.name()));
    if parenthesis {
        let sign = symbol("braille-music-parenthesis");
        braille.push_str(&sign);
        english.push(format!("Parenthesis {sign}"));
    }
}

/// The distance in steps between two pitches, counting both: music21's
/// undirected generic interval.
fn generic_distance(from: &Pitch, to: &Pitch) -> i64 {
    (i64::from(to.diatonic_note_number()) - i64::from(from.diatonic_note_number())).abs() + 1
}

/// A chord, its lowest or highest note written in full and each other as
/// the interval from it, octaves marked where they are needed: music21's
/// `chordToBraille`. The written note is a note of the chord's length and
/// nothing else, as music21 writes it.
pub fn chord_to_braille(chord: &Chord, descending: bool, show_octave: bool) -> Transcription {
    let mut english = Vec::new();
    let mut pitches = chord.pitches();
    pitches.sort_by(|left, right| left.ps().total_cmp(&right.ps()));
    let direction = if descending {
        pitches.reverse();
        "Descending"
    } else {
        "Ascending"
    };
    let Some(base) = pitches.first().cloned() else {
        return Transcription::exception(vec![format!("{direction} Chord None")]);
    };
    let length = chord.duration().map_or(1.0, Duration::quarter_length);
    let mut first = Note::from_pitch(base.clone());
    if let Ok(duration) = Duration::new(length) {
        first.set_duration(duration);
    }
    let written = note_to_braille(&first, show_octave, true, NoteContext::default());
    if written.braille == symbol("basic_exception") {
        let names: Vec<String> = chord
            .pitches()
            .iter()
            .map(Pitch::name_with_octave)
            .collect();
        return Transcription::exception(vec![format!(
            "<music21.chord.Chord {}> None",
            names.join(" ")
        )]);
    }
    let mut braille = written.braille;
    english.push(format!(
        "{direction} Chord:\n{}",
        written.english.join("\n")
    ));
    for index in 1..pitches.len() {
        let current = &pitches[index];
        handle_pitch_with_accidental(current, &mut braille, &mut english);
        let mut distance = generic_distance(&base, current);
        let octave = current.octave().unwrap_or(4);
        if distance > 8 {
            distance = (distance - 1) % 7 + 1;
            let marked = index == 1 || generic_distance(&pitches[index - 1], current) >= 8;
            if marked {
                let sign = pitch_to_octave(octave);
                braille.push_str(&sign);
                english.push(format!("Octave {octave} {sign}"));
            }
        } else if distance == 1 {
            let sign = pitch_to_octave(octave);
            braille.push_str(&sign);
            english.push(format!("Octave {octave} {sign}"));
        }
        if distance == 1 {
            distance = 8;
        }
        let sign = lookup::interval(distance).unwrap_or_default();
        braille.push(sign);
        english.push(format!("Interval {distance} {sign}"));
    }
    Transcription { braille, english }
}

/// The names music21 gives its clefs in braille's English.
fn clef_english_name(class_name: &str) -> Option<&'static str> {
    Some(match class_name {
        "FrenchViolinClef" => "French Violin",
        "TrebleClef" => "Treble",
        "GSopranoClef" => "G-soprano",
        "SopranoClef" => "Soprano",
        "MezzoSopranoClef" => "Mezzo-soprano",
        "AltoClef" => "Alto",
        "TenorClef" => "Tenor",
        "CBaritoneClef" => "C-baritone",
        "FBaritoneClef" => "F-baritone",
        "BassClef" => "Bass",
        "SubBassClef" => "Sub-bass",
        _ => return None,
    })
}

/// A clef: music21's `clefToBraille`. A treble or bass clef, and the clefs
/// music21 derives from them, is written for the other hand when the
/// keyboard's hands are switched.
pub fn clef_to_braille(clef: &Clef, keyboard_hand_switched: bool) -> Transcription {
    let class_name = clef.kind().class_name();
    let repr = format!("<music21.clef.{class_name}>");
    if class_name == "NoClef" {
        return Transcription {
            braille: String::new(),
            english: vec![format!("No Clef {repr}")],
        };
    }
    let sign = clef
        .sign()
        .zip(clef.line())
        .and_then(|(sign, line)| lookup::clef(sign, i64::from(line)));
    let Some(sign) = sign else {
        return Transcription::exception(vec![format!("{repr} None")]);
    };
    let mut braille = format!("{}{sign}", lookup::clef_prefix());
    let mut english = vec![match clef_english_name(class_name) {
        Some(name) => format!("{name} Clef {braille}"),
        None => format!("Unnamed Clef {repr}"),
    }];
    let treble_or_bass = matches!(
        class_name,
        "TrebleClef"
            | "Treble8vbClef"
            | "Treble8vaClef"
            | "BassClef"
            | "Bass8vbClef"
            | "Bass8vaClef"
    );
    if treble_or_bass {
        braille.push(lookup::clef_suffix(keyboard_hand_switched));
        if keyboard_hand_switched {
            english.push(" Keyboard hand switched".to_string());
        }
    } else {
        braille.push(lookup::clef_suffix(false));
    }
    Transcription { braille, english }
}

/// A dynamic as a word, after the word sign where asked: music21's
/// `dynamicToBraille`.
pub fn dynamic_to_braille(dynamic: &Dynamic, precede_by_word_sign: bool) -> Transcription {
    let mut english = Vec::new();
    let mut braille = String::new();
    if precede_by_word_sign {
        let word = symbol("word");
        english.push(format!("Word: {word}"));
        braille.push_str(&word);
    }
    match word_to_braille(dynamic.value(), true) {
        Ok(word) => {
            english.push(format!("Dynamic {} {word}", dynamic.value()));
            braille.push_str(&word);
            Transcription { braille, english }
        }
        Err(_) => {
            english.push(format!("Dynamic {} None", dynamic.value()));
            Transcription::exception(english)
        }
    }
}

/// An instrument's best name, word by word: music21's
/// `instrumentToBraille`.
///
/// # Errors
///
/// An instrument with no name at all, which music21 cannot split.
pub fn instrument_to_braille(instrument: &Instrument) -> Result<Transcription> {
    let name = instrument.best_name().ok_or_else(|| {
        Error::Instrument("'NoneType' object has no attribute 'split'".to_string())
    })?;
    let words: Result<Vec<String>> = name
        .split_whitespace()
        .map(|word| word_to_braille(word, false))
        .collect();
    Ok(match words {
        Ok(words) => {
            let braille = words.join(&symbol("space"));
            Transcription {
                english: vec![format!("Instrument {name} {braille}")],
                braille,
            }
        }
        Err(_) => Transcription::exception(vec![format!("Instrument {name} None")]),
    })
}

/// music21's `repr` of a key signature.
fn key_signature_repr(sharps: Option<IntegerType>) -> String {
    match sharps {
        Some(0) => "<music21.key.KeySignature of no sharps or flats>".to_string(),
        Some(1) => "<music21.key.KeySignature of 1 sharp>".to_string(),
        Some(-1) => "<music21.key.KeySignature of 1 flat>".to_string(),
        Some(sharps) if sharps > 0 => format!("<music21.key.KeySignature of {sharps} sharps>"),
        Some(sharps) => format!("<music21.key.KeySignature of {} flats>", -sharps),
        None => "<music21.key.KeySignature of pitches: []>".to_string(),
    }
}

/// A key signature of so many sharps, after the naturals cancelling the
/// one before it where one is given: music21's `keySigToBraille`.
pub fn key_sig_to_braille(
    sharps: Option<IntegerType>,
    outgoing: Option<Option<IntegerType>>,
) -> Transcription {
    let mut english = Vec::new();
    let Some(incoming) = sharps else {
        return Transcription::exception(vec![format!(
            "Key Signature {} cannot be transcribed",
            key_signature_repr(sharps)
        )]);
    };
    let Some(signature) = lookup::key_signature(i64::from(incoming)) else {
        return Transcription::exception(vec![format!(
            "Key Signature {} cannot be transcribed",
            key_signature_repr(sharps)
        )]);
    };
    if incoming > 0 {
        english.push(format!("Key Signature {incoming} sharp(s) {signature}"));
    } else {
        english.push(format!(
            "Key Signature {} flat(s) {signature}",
            incoming.abs()
        ));
    }
    let Some(outgoing) = outgoing else {
        return Transcription {
            braille: signature,
            english,
        };
    };
    let Some(out) = outgoing else {
        english.push(format!("{} naturals=None", key_signature_repr(outgoing)));
        return Transcription {
            braille: signature,
            english,
        };
    };
    let (absolute_out, absolute_in) = (i64::from(out).abs(), i64::from(incoming).abs());
    let mut braille = String::new();
    let cancel = if incoming == 0 || out == 0 || out.signum() != incoming.signum() {
        Some(absolute_out)
    } else if absolute_out >= absolute_in {
        Some(i64::from(out - incoming).abs())
    } else {
        None
    };
    if let Some(count) = cancel {
        let Some(naturals) = lookup::naturals(count) else {
            english.push(format!("{} naturals=None", key_signature_repr(outgoing)));
            return Transcription {
                braille: signature,
                english,
            };
        };
        braille.push_str(&naturals);
        english.insert(0, format!("Key Signature {out} naturals {naturals}"));
    }
    braille.push_str(&signature);
    Transcription { braille, english }
}

/// A metronome mark: its note value, the metronome sign and its number:
/// music21's `metronomeMarkToBraille`. A mark with no number is nothing,
/// and says nothing in English.
pub fn metronome_mark_to_braille(mark: &MetronomeMark) -> Option<Transcription> {
    let number = mark.number()?;
    let mut english = Vec::new();
    let mut note = Note::from_pitch(Pitch::from_name("C4").expect("C4 is a pitch"));
    if let Ok(duration) = Duration::new(mark.referent().quarter_length()) {
        note.set_duration(duration);
    }
    let written = note_to_braille(&note, false, true, NoteContext::default());
    let mut braille = written.braille;
    english.push(format!("Metronome Note {}", written.english.join(" ")));
    let metronome = symbol("metronome");
    braille.push_str(&metronome);
    english.push(format!("Metronome symbol {metronome}"));
    let text = crate::statistics::python_repr(number);
    let text = text.strip_suffix(".0").unwrap_or(&text);
    match number_to_braille(text, true, false) {
        Ok(digits) => {
            braille.push_str(&digits);
            english.push(format!("Metronome number {text} {digits}"));
            Some(Transcription { braille, english })
        }
        Err(_) => Some(Transcription::exception(vec![format!(
            "{} None",
            metronome_mark_repr(mark)
        )])),
    }
}

/// music21's `repr` of a metronome mark, as far as its number shows.
fn metronome_mark_repr(mark: &MetronomeMark) -> String {
    let number = mark
        .number()
        .map(crate::statistics::python_repr)
        .unwrap_or_default();
    format!("<music21.tempo.MetronomeMark {number}>")
}

/// The articulations braille writes before a note, bowings first, then
/// staccatos, then the rest by name: music21's
/// `yieldBrailleArticulations`.
fn braille_articulations(note: &Note) -> Vec<(String, String)> {
    let mut articulations: Vec<_> = note.articulations().iter().collect();
    articulations.sort_by_key(|articulation| {
        (
            !articulation.is_a("Bowing"),
            !articulation.is_a("Staccato"),
            articulation.name(),
        )
    });
    articulations
        .into_iter()
        .filter_map(|articulation| {
            let name = articulation.name();
            lookup::bowing(&name)
                .or_else(|| lookup::before_note_expression(&name))
                .map(|sign| (name, sign))
        })
        .collect()
}

/// A note: its slurs and tuplet opening, articulations, accidental,
/// octave where shown, name and value, dots, fingering, fermata, slur and
/// tie: music21's `noteToBraille`.
pub fn note_to_braille(
    note: &Note,
    show_octave: bool,
    upper_first_in_fingering: bool,
    context: NoteContext,
) -> Transcription {
    let mut context = context;
    let mut english: Vec<String> = Vec::new();
    let mut braille = String::new();
    let push = |braille: &mut String, english: &mut Vec<String>, sign: String, text: String| {
        english.push(text);
        braille.push_str(&sign);
    };
    if context.begin_long_bracket_slur {
        let sign = symbol("opening_bracket_slur");
        push(
            &mut braille,
            &mut english,
            sign.clone(),
            format!("Opening bracket slur {sign}"),
        );
    } else if context.begin_long_double_slur {
        let sign = symbol("opening_double_slur");
        push(
            &mut braille,
            &mut english,
            sign.clone(),
            format!("Opening double slur {sign}"),
        );
    }
    if context.end_long_bracket_slur && context.begin_long_bracket_slur {
        let sign = symbol("closing_bracket_slur");
        push(
            &mut braille,
            &mut english,
            sign.clone(),
            format!("Closing bracket slur {sign}"),
        );
    }
    let duration = note.duration().cloned().unwrap_or_default();
    let tuplets = duration.tuplets();
    if let Some(tuplet) = tuplets.first() {
        if context.beam_start {
            // music21 marks a tuplet whose number is hidden, and writes it
            // with the transcriber's sign, where its number is shown as
            // `'none'`; its reader hides a number as `None`, so every
            // tuplet is written as shown.
            let last = if tuplet.full_name() == "Triplet" {
                symbol("triplet")
            } else {
                let number = number_to_braille(&tuplet.actual().to_string(), false, true)
                    .unwrap_or_default();
                format!("{}{number}{}", symbol("tuplet_prefix"), symbol("dot"))
            };
            braille.push_str(&last);
            english.push(format!("{} {last}", tuplet.full_name()));
        } else if context.beam_continue {
            context.beam_continue = false;
        }
    }
    for (name, sign) in braille_articulations(note) {
        push(
            &mut braille,
            &mut english,
            sign.clone(),
            format!("Articulation {name} {sign}"),
        );
    }
    handle_pitch_with_accidental(note.pitch(), &mut braille, &mut english);
    let octave = note.pitch().octave();
    if show_octave {
        let sign = pitch_to_octave(octave.unwrap_or(4));
        let shown = octave.map_or_else(|| "None".to_string(), |octave| octave.to_string());
        push(
            &mut braille,
            &mut english,
            sign.clone(),
            format!("Octave {shown} {sign}"),
        );
    }
    let step = note.pitch().step().as_char();
    if duration.is_grace() {
        let sign = lookup::pitch_name_to_note(step, "eighth").unwrap_or_default();
        push(
            &mut braille,
            &mut english,
            sign.clone(),
            format!("{step} eighth Gracenote--not supported {sign}"),
        );
    } else {
        let value = duration_type(&duration);
        let (sign, text) = if context.beam_continue {
            let sign = lookup::pitch_name_to_note(step, "eighth").unwrap_or_default();
            (Some(sign.clone()), format!("{step} beam {sign}"))
        } else {
            match lookup::pitch_name_to_note(step, &value) {
                Some(sign) => (Some(sign.clone()), format!("{step} {value} {sign}")),
                None => (None, String::new()),
            }
        };
        let Some(sign) = sign else {
            english.push(format!(
                "Duration <music21.duration.Duration {}> None",
                quarter_length_repr(duration.quarter_length())
            ));
            return Transcription::exception(english);
        };
        push(&mut braille, &mut english, sign, text);
        for _ in 0..duration_dots(&duration) {
            let dot = symbol("dot");
            push(
                &mut braille,
                &mut english,
                dot.clone(),
                format!("Dot {dot}"),
            );
        }
    }
    for articulation in note.articulations() {
        if !articulation.is_a("Fingering") {
            continue;
        }
        let finger = match articulation.finger() {
            Some(crate::articulations::Finger::Number(number)) => number.to_string(),
            Some(crate::articulations::Finger::Written(text)) => text.clone(),
            None => continue,
        };
        if let Ok(sign) = transcribe_note_fingering(&finger, upper_first_in_fingering) {
            braille.push_str(&sign);
        }
    }
    handle_expressions(note.expressions(), &mut braille, &mut english);
    if context.short_slur {
        let sign = symbol("opening_single_slur");
        push(
            &mut braille,
            &mut english,
            sign.clone(),
            format!("Opening single slur {sign}"),
        );
    }
    if !(context.end_long_bracket_slur && context.begin_long_bracket_slur) {
        if context.end_long_double_slur {
            let sign = symbol("closing_double_slur");
            push(
                &mut braille,
                &mut english,
                sign.clone(),
                format!("Closing bracket slur {sign}"),
            );
        } else if context.end_long_bracket_slur {
            let sign = symbol("closing_bracket_slur");
            push(
                &mut braille,
                &mut english,
                sign.clone(),
                format!("Closing bracket slur {sign}"),
            );
        }
    }
    if note
        .tie()
        .is_some_and(|tie| tie.tie_type() != TieType::Stop)
    {
        let sign = symbol("tie");
        push(
            &mut braille,
            &mut english,
            sign.clone(),
            format!("Tie {sign}"),
        );
    }
    Transcription { braille, english }
}

/// The fermatas of a note or rest: music21's `handleExpressions`.
fn handle_expressions(expressions: &[Expression], braille: &mut String, english: &mut Vec<String>) {
    for expression in expressions {
        if let Expression::Fermata(fermata) = expression {
            let shape = fermata.shape().unwrap_or("normal");
            if let Some(sign) = lookup::fermata(shape) {
                english.push(format!("Note-fermata: Shape {shape}: {sign}"));
                braille.push_str(&sign);
            }
        }
    }
}

/// A rest, a whole rest where it stands for its measure: music21's
/// `restToBraille`.
pub fn rest_to_braille(rest: &Rest) -> Transcription {
    let mut english = Vec::new();
    let (value, dots) = if rest.full_measure() == Some(true) {
        ("whole".to_string(), 0)
    } else {
        (
            duration_type(rest.duration()),
            duration_dots(rest.duration()),
        )
    };
    let Some(sign) = lookup::rest(&value) else {
        return Transcription::exception(vec![format!(
            "Rest {} None",
            duration_type(rest.duration())
        )]);
    };
    let mut braille = sign.clone();
    english.push(format!("Rest {value} {sign}"));
    for _ in 0..dots {
        let dot = symbol("dot");
        braille.push_str(&dot);
        english.push(format!("Dot {dot}"));
    }
    handle_expressions(rest.expressions(), &mut braille, &mut english);
    Transcription { braille, english }
}

/// Tempo words, phrase by comma-separated phrase, a line broken before a
/// word that would run past the line less six, ending with a full stop:
/// music21's `tempoTextToBraille`.
pub fn tempo_text_to_braille(text: &str, max_line_length: usize) -> Transcription {
    let space = symbol("space");
    let mut phrases = Vec::new();
    for phrase in text.split(',') {
        let mut pieces: Vec<String> = Vec::new();
        for word in phrase.split_whitespace() {
            let Ok(braille) = word_to_braille(word, false) else {
                return Transcription::exception(vec![format!("Tempo Text {text} None")]);
            };
            let mut width = 0;
            for piece in &pieces {
                if piece == "\n" {
                    width = 0;
                } else {
                    width += piece.chars().count();
                }
            }
            if width + braille.chars().count() + 1 > max_line_length.saturating_sub(6) {
                pieces.push("\n".to_string());
            }
            pieces.push(braille);
            pieces.push(space.clone());
        }
        pieces.pop();
        phrases.push(pieces.concat());
    }
    let joiner = format!("{}\n", lookup::alphabet(',').unwrap_or_default());
    let braille = phrases.join(&joiner) + &lookup::alphabet('.').unwrap_or_default();
    Transcription {
        english: vec![format!("Tempo Text {text} {braille}")],
        braille,
    }
}

/// A tempo text: [`tempo_text_to_braille`] of its text.
pub fn tempo_to_braille(tempo: &TempoText, max_line_length: usize) -> Transcription {
    tempo_text_to_braille(tempo.text(), max_line_length)
}

/// Words written as text, after the word sign where asked and before
/// another where there are several: music21's `textExpressionToBraille`.
/// The hairpin words have signs of their own.
pub fn text_expression_to_braille(
    expression: &TextExpression,
    precede_by_word_sign: bool,
) -> Transcription {
    let words = expression.content();
    if let Some(sign) = lookup::text_expression(words) {
        return Transcription {
            english: vec![format!("Text Expression {words} {sign}")],
            braille: sign,
        };
    }
    let all: Vec<&str> = words.split_whitespace().collect();
    let written: Result<Vec<String>> = all.iter().map(|word| word_to_braille(word, true)).collect();
    let Ok(written) = written else {
        return Transcription::exception(vec![format!("Text Expression {words} None")]);
    };
    let mut braille = written.join(&symbol("space"));
    let mut english = vec![format!("Text Expression {words} {braille}")];
    let word = symbol("word");
    if precede_by_word_sign {
        braille = format!("{word}{braille}");
        english.insert(0, format!("Word {word}"));
    }
    if all.len() > 1 {
        braille.push_str(&word);
        english.push(format!("Word {word}"));
    }
    Transcription { braille, english }
}

/// A time signature, its numerator high and denominator low, or its common
/// or cut sign: music21's `timeSigToBraille`.
pub fn time_sig_to_braille(meter: &TimeSignature) -> Transcription {
    if let Some(sign @ ("common" | "cut")) = meter.symbol() {
        let braille = symbol(sign);
        return Transcription {
            english: vec![format!("Time Signature {sign} {braille}")],
            braille,
        };
    }
    let (numerator, denominator) = (meter.numerator(), meter.denominator());
    let written = number_to_braille(&numerator.to_string(), true, false).and_then(|high| {
        number_to_braille(&denominator.to_string(), false, true).map(|low| high + &low)
    });
    match written {
        Ok(braille) => Transcription {
            english: vec![format!(
                "Time Signature {numerator}/{denominator} {braille}"
            )],
            braille,
        },
        Err(_) => Transcription::exception(vec![format!(
            "<music21.meter.TimeSignature {numerator}/{denominator}> None"
        )]),
    }
}

/// Whether a note shows its octave after the one before: always after
/// none, after a sixth or more, and after a fourth or fifth into another
/// octave: music21's `showOctaveWithNote`.
pub fn show_octave_with_note(previous: Option<&Pitch>, current: &Pitch) -> bool {
    let Some(previous) = previous else {
        return true;
    };
    let distance = generic_distance(previous, current);
    distance >= 6 || (matches!(distance, 4 | 5) && previous.octave() != current.octave())
}

/// A key and time signature in braille, the key cancelling the one before
/// where given: music21's `transcribeSignatures`. Nothing for no time
/// signature and a key of no sharps with nothing to cancel.
pub fn transcribe_signatures(
    sharps: Option<Option<IntegerType>>,
    meter: Option<&TimeSignature>,
    outgoing: Option<Option<IntegerType>>,
) -> Transcription {
    let empty_key = match sharps {
        None => true,
        Some(sharps) => sharps == Some(0) && outgoing.is_none(),
    };
    if meter.is_none() && empty_key {
        return Transcription::default();
    }
    let mut out = Transcription::default();
    if let Some(sharps) = sharps {
        let key = key_sig_to_braille(sharps, outgoing);
        out.braille.push_str(&key.braille);
        out.english.extend(key.english);
    }
    if let Some(meter) = meter {
        let time = time_sig_to_braille(meter);
        out.braille.push_str(&time.braille);
        out.english.extend(time.english);
    }
    out
}

/// A heading: the tempo words above, then the metronome mark, key and time
/// signature, each line centred: music21's `transcribeHeading`.
///
/// # Errors
///
/// Nothing to head with.
pub fn transcribe_heading(
    sharps: Option<Option<IntegerType>>,
    meter: Option<&TimeSignature>,
    tempo: Option<&TempoText>,
    metronome: Option<&MetronomeMark>,
    max_line_length: usize,
) -> Result<String> {
    if sharps.is_none() && meter.is_none() && tempo.is_none() && metronome.is_none() {
        return Err(Error::Notation("No heading can be made.".to_string()));
    }
    let space = symbol("space");
    let tempo_text = tempo.map(|tempo| tempo_to_braille(tempo, max_line_length).braille);
    let centred_lines = |lines: &[String]| -> String {
        lines
            .iter()
            .map(|line| center(line, max_line_length, &space))
            .collect::<Vec<_>>()
            .join("\n")
    };
    if sharps.is_none() && meter.is_none() && metronome.is_none() {
        let tempo_text = tempo_text.unwrap_or_default();
        let lines: Vec<String> = python_lines(&tempo_text);
        return Ok(
            if tempo_text.chars().count() <= max_line_length.saturating_sub(6) {
                center(&lines.concat(), max_line_length, &space)
            } else {
                centred_lines(&lines)
            },
        );
    }
    let mut other = String::new();
    if let Some(metronome) = metronome {
        other.push_str(
            &metronome_mark_to_braille(metronome)
                .map(|mark| mark.braille)
                .unwrap_or_default(),
        );
    }
    let signatures = transcribe_signatures(sharps, meter, None);
    if metronome.is_some() {
        other.push_str(&space);
    }
    other.push_str(&signatures.braille);
    let Some(tempo_text) = tempo_text else {
        return Ok(center(&other, max_line_length, &space));
    };
    let mut lines = python_lines(&tempo_text);
    lines.push(other.clone());
    while lines.len() > 1 && lines[0].is_empty() {
        lines.remove(0);
    }
    while lines.len() > 1 && lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    Ok(
        if tempo_text.chars().count() + other.chars().count() < max_line_length.saturating_sub(6) {
            center(&lines.join(&space), max_line_length, &space)
        } else {
            centred_lines(&lines)
        },
    )
}

/// The lines of a text as Python's `splitlines` gives them.
pub(crate) fn python_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\r' => {
                if characters.peek() == Some(&'\n') {
                    characters.next();
                }
                lines.push(std::mem::take(&mut current));
            }
            '\n' | '\u{b}' | '\u{c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}' | '\u{2028}'
            | '\u{2029}' => lines.push(std::mem::take(&mut current)),
            other => current.push(other),
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// A note's fingering: one finger, a change of finger as `1-2`, or a choice
/// of fingerings as `1,2` (either may be missing) or `1|2`: music21's
/// `transcribeNoteFingering`.
///
/// # Errors
///
/// A finger braille has no sign for, or more than two choices.
pub fn transcribe_note_fingering(
    fingering: &str,
    upper_first_in_fingering: bool,
) -> Result<String> {
    let refuse = || Error::Notation(format!("Cannot translate note fingering: {fingering}"));
    if fingering.chars().count() == 1 {
        return lookup::finger_mark(fingering)
            .map(String::from)
            .ok_or_else(refuse);
    }
    let mut choice: Vec<&str> = fingering.split(',').collect();
    let allow_absence = match choice.len() {
        2 => true,
        1 => {
            choice = fingering.split('|').collect();
            false
        }
        _ => return Err(Error::Notation(format!("KeyError: {fingering}"))),
    };
    if !upper_first_in_fingering {
        choice.reverse();
    }
    let mut braille = String::new();
    for (index, one) in choice.iter().enumerate() {
        let change: Vec<&str> = one.split('-').collect();
        match change.as_slice() {
            [from, to] => {
                braille.push(lookup::finger_mark(from).ok_or_else(refuse)?);
                braille.push_str(&symbol("finger_change"));
                braille.push(lookup::finger_mark(to).ok_or_else(refuse)?);
            }
            [mark] => match lookup::finger_mark(mark) {
                Some(sign) => braille.push(sign),
                None if allow_absence => braille.push_str(&symbol(if index == 0 {
                    "first_set_missing_fingermark"
                } else {
                    "second_set_missing_fingermark"
                })),
                None => return Err(refuse()),
            },
            _ => {}
        }
    }
    Ok(braille)
}

/// Braille written as braille ASCII: music21's `brailleUnicodeToBrailleAscii`.
///
/// # Errors
///
/// A character that is not a braille cell.
pub fn braille_unicode_to_braille_ascii(braille: &str) -> Result<String> {
    python_lines(braille)
        .iter()
        .map(|line| {
            line.chars()
                .map(|cell| {
                    lookup::ascii_char(cell)
                        .ok_or_else(|| Error::Notation(format!("KeyError: {cell:?}")))
                })
                .collect::<Result<String>>()
        })
        .collect::<Result<Vec<_>>>()
        .map(|lines| lines.join("\n"))
}

/// Braille ASCII written as braille: music21's `brailleAsciiToBrailleUnicode`.
///
/// # Errors
///
/// A character braille ASCII has no cell for.
pub fn braille_ascii_to_braille_unicode(ascii: &str) -> Result<String> {
    python_lines(ascii)
        .iter()
        .map(|line| {
            line.chars()
                .map(|character| {
                    let upper = character.to_ascii_uppercase();
                    (0..64u32)
                        .map(|bits| char::from_u32(0x2800 + bits).expect("a braille cell"))
                        .find(|cell| lookup::ascii_char(*cell) == Some(upper))
                        .ok_or_else(|| Error::Notation(format!("KeyError: {upper:?}")))
                })
                .collect::<Result<String>>()
        })
        .collect::<Result<Vec<_>>>()
        .map(|lines| lines.join("\n"))
}

/// Braille drawn as dots, each cell two columns of three rows, filled and
/// empty: music21's `brailleUnicodeToSymbols`.
///
/// # Errors
///
/// A character that is not a braille cell.
pub fn braille_unicode_to_symbols(braille: &str, filled: &str, empty: &str) -> Result<String> {
    let draw = |dots: &str| -> String {
        dots.chars()
            .map(|dot| if dot == '1' { filled } else { empty })
            .collect()
    };
    let mut lines: Vec<String> = Vec::new();
    for line in python_lines(braille) {
        let mut rows = [Vec::new(), Vec::new(), Vec::new()];
        for cell in line.chars() {
            let dots = lookup::binary_dots(cell)
                .ok_or_else(|| Error::Notation(format!("KeyError: {cell:?}")))?;
            for (row, dots) in rows.iter_mut().zip(dots) {
                row.push(draw(dots));
            }
        }
        for row in rows {
            lines.push(row.join("  "));
        }
        lines.push(String::new());
    }
    lines.pop();
    Ok(lines.join("\n"))
}

/// A dot for each row of a cell whose left dot is raised: music21's
/// `yieldDots`, which looks at the left column only.
pub fn yield_dots(cell: char) -> Vec<String> {
    lookup::binary_dots(cell)
        .map(|rows| {
            rows.iter()
                .filter(|dots| dots.starts_with('1'))
                .map(|_| symbol("dot"))
                .collect()
        })
        .unwrap_or_default()
}

/// A character as music21's `stripAccents` leaves it: a Latin letter
/// without its diacritics, a spacing character as a space, a curly quote
/// straight and a superscript digit as the digit, as its compatibility
/// decomposition does.
fn strip_accents(letter: &str) -> String {
    letter
        .chars()
        .map(|character| match character {
            '\u{a0}' | '\u{2000}'..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}' => ' ',
            '\u{2018}' | '\u{2019}' => '\'',
            '\u{201c}' | '\u{201d}' => '"',
            '\u{b9}' => '1',
            '\u{b2}' => '2',
            '\u{b3}' => '3',
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
            'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
            'ď' => 'd',
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
            'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
            'ĥ' => 'h',
            'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' => 'i',
            'ĵ' => 'j',
            'ķ' => 'k',
            'ĺ' | 'ļ' | 'ľ' => 'l',
            'ñ' | 'ń' | 'ņ' | 'ň' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ō' | 'ŏ' | 'ő' => 'o',
            'ŕ' | 'ŗ' | 'ř' => 'r',
            'ś' | 'ŝ' | 'ş' | 'š' => 's',
            'ţ' | 'ť' => 't',
            'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
            'ŵ' => 'w',
            'ý' | 'ÿ' | 'ŷ' => 'y',
            'ź' | 'ż' | 'ž' => 'z',
            '\u{300}'..='\u{36f}' => '\0',
            other => other,
        })
        .filter(|character| *character != '\0')
        .collect()
}

/// A word in braille: music21's `wordToBraille`. As text in the music a
/// word is spelled letter by letter; otherwise capitals and digits take
/// their signs, and a letter after a digit the letter sign. A letter with
/// diacritics is written as the accent sign and the letter.
///
/// # Errors
///
/// A character braille has no sign for.
pub fn word_to_braille(word: &str, is_text_expression: bool) -> Result<String> {
    let refuse = |letter: char| {
        Error::Notation(if is_text_expression {
            format!(
                "Character '{letter}' in Text Expression '{word}' cannot be transcribed to braille."
            )
        } else {
            format!("Character '{letter}' in word '{word}' cannot be transcribed to braille.")
        })
    };
    let mut braille = String::new();
    let add_letter = |braille: &mut String, letter: &str| -> bool {
        let mut characters = letter.chars();
        if let (Some(single), None) = (characters.next(), characters.next())
            && let Some(sign) = lookup::alphabet(single)
        {
            braille.push_str(&sign);
            return true;
        }
        let stripped = strip_accents(letter);
        if stripped == letter {
            return false;
        }
        let mut characters = stripped.chars();
        match (characters.next(), characters.next()) {
            (Some(single), None) => match lookup::alphabet(single) {
                Some(sign) => {
                    braille.push_str(&lookup::alphabet('^').unwrap_or_default());
                    braille.push_str(&sign);
                    true
                }
                None => false,
            },
            _ => false,
        }
    };
    if is_text_expression {
        for letter in word.chars() {
            let added = if letter.is_uppercase() {
                add_letter(&mut braille, &letter.to_lowercase().to_string())
            } else if letter == '.' {
                braille.push_str(&symbol("dot"));
                true
            } else if letter == 'ß' {
                add_letter(&mut braille, "s") && add_letter(&mut braille, "s")
            } else {
                add_letter(&mut braille, &letter.to_string())
            };
            if !added {
                return Err(refuse(letter));
            }
        }
        return Ok(braille);
    }
    let mut last_was_number = false;
    for letter in word.chars() {
        let digit = letter.is_ascii_digit();
        if digit && !last_was_number {
            braille.push_str(&symbol("number"));
            last_was_number = true;
        } else if letter.is_alphabetic() && last_was_number {
            braille.push_str(&symbol("letter_sign"));
            last_was_number = false;
        }
        let added = if letter.is_uppercase() {
            braille.push_str(&symbol("uppercase"));
            add_letter(&mut braille, &letter.to_lowercase().to_string())
        } else if digit {
            braille.push(
                lookup::number_upper(letter.to_digit(10).unwrap_or_default()).unwrap_or_default(),
            );
            true
        } else if letter == 'ß' {
            add_letter(&mut braille, "s") && add_letter(&mut braille, "s")
        } else {
            add_letter(&mut braille, &letter.to_string())
        };
        if !added {
            return Err(refuse(letter));
        }
    }
    Ok(braille)
}

/// A number in braille, its digits high or low, after the number sign where
/// asked: music21's `numberToBraille`.
///
/// # Errors
///
/// A character that is not a digit.
pub fn number_to_braille(number: &str, with_number_sign: bool, lower: bool) -> Result<String> {
    let mut braille = String::new();
    if with_number_sign {
        braille.push_str(&symbol("number"));
    }
    for digit in number.chars() {
        let sign = digit
            .to_digit(10)
            .filter(|_| digit.is_ascii_digit())
            .and_then(|digit| {
                if lower {
                    lookup::number_lower(digit)
                } else {
                    lookup::number_upper(digit)
                }
            })
            .ok_or_else(|| {
                Error::Notation(format!(
                    "Digit '{digit}' in number '{number}' cannot be transcribed to braille."
                ))
            })?;
        braille.push(sign);
    }
    Ok(braille)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_and_numbers_are_written_as_music21_writes_them() -> Result<()> {
        // music21's doctests of wordToBraille and numberToBraille.
        assert_eq!(word_to_braille("Andante", false)?, "⠠⠁⠝⠙⠁⠝⠞⠑");
        assert_eq!(word_to_braille("Café", false)?, "⠠⠉⠁⠋⠈⠑");
        assert_eq!(number_to_braille("12", true, false)?, "⠼⠁⠃");
        assert_eq!(number_to_braille("7", false, true)?, "⠶");
        assert_eq!(transcribe_note_fingering("1-3", true)?, "⠁⠉⠇");
        assert_eq!(transcribe_note_fingering("2,x", false)?, "⠠⠃");
        assert_eq!(center("ab", 5, "."), "..ab.");
        assert_eq!(center("abc", 6, "."), ".abc..");
        Ok(())
    }

    #[test]
    fn notes_and_rests_are_written_with_their_values() -> Result<()> {
        let mut note = Note::from_name("C4")?;
        note.set_duration(Duration::new(1.5)?);
        let written = note_to_braille(&note, true, true, NoteContext::default());
        assert_eq!(written.braille, "⠐⠹⠄");
        assert_eq!(written.english, ["Octave 4 ⠐", "C quarter ⠹", "Dot ⠄"]);
        let rest = Rest::new(Duration::new(2.0)?);
        assert_eq!(rest_to_braille(&rest).braille, "⠥");
        Ok(())
    }
}
