//! What the readers read, as the crate's score, written as ABC by the
//! crate's own writer. The editor decides what the crate cannot know from
//! a guitar tab or a MIDI file -- each note's spelling in the key, the clef
//! a part reads in, the key a score that states none suggests, rests
//! filling a voice out to the bar -- and the crate writes the rest: voices
//! and their staves, ties, tuplets, beams, accidentals, repeats and endings.

use super::super::spell_in_key;
use super::{Beat, Imported, KeySig, Part, TICKS};
use music21_rs::abc::{ExportOptions, to_abc as write_abc};
use music21_rs::bar::{Barline, BarlineType, Ending, RepeatDirection};
use music21_rs::clef::ClefKind;
use music21_rs::expressions::TextExpression;
use music21_rs::metadata::Metadata;
use music21_rs::notation::{Placement, Tie, TieType};
use music21_rs::{
    Chord, ChordSymbol, Clef, Duration, DurationType, Instrument, Key, KeySignature, MetronomeMark,
    Note, Pitch, Rest, Stream, StreamElement, StreamKind, TimeSignature, Tuplet,
    estimate_key_from_pitches,
};
use std::collections::BTreeSet;

/// The score as ABC: the crate's writer, against an eighth, four bars to a
/// line.
pub(super) fn to_abc(imported: &Imported) -> Result<String, String> {
    let score = to_stream(imported)?;
    let options = ExportOptions {
        unit_length: Some((1, 8)),
        measures_per_line: 4,
    };
    write_abc(&score, &options).map_err(|err| err.to_string())
}

fn has_notes(part: &Part, voice: usize) -> bool {
    part.measures.iter().any(|measure| {
        measure
            .voices
            .get(voice)
            .is_some_and(|beats| beats.iter().any(|beat| !beat.notes.is_empty()))
    })
}

/// The clef a part is written in: the one its source names, a guitar's or a
/// bass's an octave above where they sound, otherwise by where it lies.
fn clef_of(part: &Part) -> Result<Clef, String> {
    let name = match &part.clef {
        Some(clef) => clef.clone(),
        None => {
            let fretted = match part.program {
                Some(program) => (24..=39).contains(&program),
                None => !part.tuning.is_empty(),
            };
            if fretted {
                let bass = part
                    .program
                    .is_some_and(|program| (32..=39).contains(&program))
                    || part.tuning.first().is_some_and(|&lowest| lowest < 36);
                if bass { "bass-8" } else { "treble-8" }.to_string()
            } else {
                let pitches: Vec<i32> = part
                    .measures
                    .iter()
                    .flat_map(|measure| measure.voices.iter().flatten())
                    .flat_map(|beat| beat.notes.iter().map(|note| note.midi))
                    .collect();
                let mean = pitches.iter().sum::<i32>() as f64 / pitches.len().max(1) as f64;
                if mean < 57.0 { "bass" } else { "treble" }.to_string()
            }
        }
    };
    let kind = match name.as_str() {
        "treble-8" => ClefKind::Treble8vbClef,
        "bass-8" => ClefKind::Bass8vbClef,
        "bass" => ClefKind::BassClef,
        "tenor" => ClefKind::TenorClef,
        "alto" => ClefKind::AltoClef,
        _ => ClefKind::TrebleClef,
    };
    Ok(Clef::of_kind(kind))
}

fn key_of(key: KeySig) -> Result<Key, String> {
    KeySignature::new(key.sharps.clamp(-7, 7))
        .try_as_key(Some(if key.minor { "minor" } else { "major" }), None)
        .map_err(|err| err.to_string())
}

/// The key the notes suggest, for a score that states none.
fn estimated_key(parts: &[&Part]) -> KeySig {
    let pitches: Vec<Pitch> = parts
        .iter()
        .flat_map(|part| part.measures.iter())
        .flat_map(|measure| measure.voices.iter().flatten())
        .flat_map(|beat| beat.notes.iter())
        .filter_map(|note| Pitch::from_midi(note.midi).ok())
        .collect();
    estimate_key_from_pitches(&pitches)
        .ok()
        .and_then(|estimates| estimates.first().map(|estimate| estimate.key().clone()))
        .map_or(
            KeySig {
                sharps: 0,
                minor: false,
            },
            |key| KeySig {
                sharps: key.sharps(),
                minor: key.mode() == "minor",
            },
        )
}

/// A written length in ticks as a note value and its dots, where it is one.
fn value_of(written: i64) -> Option<(DurationType, u32)> {
    (0..=4).find_map(|dots: u32| {
        // A value with `dots` dots is (2 - 1/2^dots) of the plain one.
        let plain = written * (1 << dots) / ((2 << dots) - 1);
        if plain * ((2 << dots) - 1) != written * (1 << dots) {
            return None;
        }
        let quarter_length = plain as f64 / TICKS as f64;
        DurationType::from_quarter_length(quarter_length).map(|value| (value, dots))
    })
}

/// A beat's duration: its written value, inside its tuplet.
fn duration_of(beat: &Beat) -> Result<Duration, String> {
    match value_of(beat.written) {
        Some((value, dots)) => {
            let mut duration = Duration::from_type_with_dots(value, dots);
            if let Some((actual, normal)) = beat.tuplet {
                duration.append_tuplet(Tuplet::new(actual, normal, value, dots));
            }
            Ok(duration)
        }
        None => Duration::new(beat.length() as f64 / TICKS as f64).map_err(|err| err.to_string()),
    }
}

/// Rests filling `ticks`, each a plain note value from a whole to a 64th;
/// what is left under a 64th, the crumbs of an irregular tuplet, is dropped,
/// since no value could be written for it.
pub(super) fn rests(ticks: i64, hidden: bool) -> Vec<(i64, Rest)> {
    let mut left = ticks;
    let mut at = 0;
    let mut out = Vec::new();
    while let Some(piece) = (0..7)
        .map(|shift| (TICKS * 4) >> shift)
        .find(|&value| value <= left)
    {
        let mut rest =
            Rest::new(Duration::new(piece as f64 / TICKS as f64).expect("a plain value"));
        rest.set_hidden(hidden);
        out.push((at, rest));
        at += piece;
        left -= piece;
    }
    out
}

/// The notes of each beat of a voice tied on into the next beat, by measure,
/// beat and note index.
fn ties_out(part: &Part, voice: usize) -> BTreeSet<(usize, usize, usize)> {
    let mut sounding: Vec<(usize, usize)> = Vec::new();
    for (m, measure) in part.measures.iter().enumerate() {
        if let Some(beats) = measure.voices.get(voice) {
            for (b, beat) in beats.iter().enumerate() {
                if !beat.grace {
                    sounding.push((m, b));
                }
            }
        }
    }
    let beat = |(m, b): (usize, usize)| &part.measures[m].voices[voice][b];
    let mut ties = BTreeSet::new();
    for pair in sounding.windows(2) {
        let (from, to) = (beat(pair[0]), beat(pair[1]));
        for (n, note) in from.notes.iter().enumerate() {
            if to
                .notes
                .iter()
                .any(|next| next.tied && next.midi == note.midi)
            {
                ties.insert((pair[0].0, pair[0].1, n));
            }
        }
    }
    ties
}

/// What every voice's measure shares: the meter and key in force, and the
/// length every voice is filled to, in ticks.
struct Frame {
    meter: (u32, u32),
    key: KeySig,
    target: i64,
}

/// Text safe inside an ABC annotation.
fn quoted(text: &str) -> String {
    text.replace(['"', '\n', '\r'], " ").trim().to_string()
}

fn text(content: &str, placement: Placement) -> StreamElement {
    let mut expression = TextExpression::new(quoted(content));
    expression.set_placement(Some(placement));
    StreamElement::TextExpression(expression)
}

/// One voice's measure as the elements it holds, at their offsets in ticks.
fn voice_measure(
    part: &Part,
    voice: usize,
    index: usize,
    key: &Key,
    ties: &BTreeSet<(usize, usize, usize)>,
    tied_in: &BTreeSet<(usize, usize, usize)>,
    hidden_fill: bool,
    target: i64,
) -> Result<Vec<(i64, StreamElement)>, String> {
    let beats: &[Beat] = part
        .measures
        .get(index)
        .and_then(|measure| measure.voices.get(voice))
        .map_or(&[], Vec::as_slice);
    let scale = key.pitches().map_err(|err| err.to_string())?;
    let spell = |midi: i32| -> Result<Pitch, String> {
        spell_in_key(midi, key, &scale).map_err(|err| format!("{err:?}"))
    };
    let mut out: Vec<(i64, StreamElement)> = Vec::new();
    let mut position = 0_i64;
    for (b, beat) in beats.iter().enumerate() {
        if beat.grace {
            if let Some(note) = beat.notes.iter().max_by_key(|note| note.midi) {
                let written = Note::from_pitch(spell(note.midi)?).with_duration(duration_of(beat)?);
                out.push((position, StreamElement::Note(written.grace_note(true))));
            }
            continue;
        }
        if let Some(chord) = &beat.chord {
            let element = match ChordSymbol::parse(quoted(chord)) {
                Ok(symbol) => StreamElement::ChordSymbol(symbol),
                // A name no chord symbol reads stays the words it is.
                Err(_) => text(chord, Placement::Above),
            };
            out.push((position, element));
        }
        if let Some(words) = &beat.text {
            out.push((position, text(words, Placement::Above)));
        }
        if let Some(figure) = &beat.figure {
            out.push((position, text(figure, Placement::Below)));
        }
        let duration = duration_of(beat)?;
        if beat.notes.is_empty() {
            out.push((position, StreamElement::Rest(Rest::new(duration))));
        } else {
            let mut midis: Vec<(usize, i32)> = beat
                .notes
                .iter()
                .map(|note| note.midi)
                .enumerate()
                .collect();
            midis.sort_by_key(|&(_, midi)| midi);
            midis.dedup_by_key(|(_, midi)| *midi);
            let mut notes = Vec::new();
            for &(n, midi) in &midis {
                let mut note = Note::from_pitch(spell(midi)?);
                let starts = ties.contains(&(index, b, n));
                let stops = tied_in.contains(&(index, b, n));
                let tie = match (stops, starts) {
                    (true, true) => Some(TieType::Continue),
                    (true, false) => Some(TieType::Stop),
                    (false, true) => Some(TieType::Start),
                    (false, false) => None,
                };
                note.set_tie(tie.map(Tie::new));
                notes.push(note);
            }
            let element = if notes.len() == 1 {
                StreamElement::Note(notes.remove(0).with_duration(duration))
            } else {
                let mut chord = Chord::new(&notes[..]).map_err(|err| err.to_string())?;
                chord.set_duration(duration);
                StreamElement::Chord(chord)
            };
            out.push((position, element));
        }
        position += beat.length();
    }
    if position < target {
        for (at, rest) in rests(target - position, hidden_fill) {
            out.push((position + at, StreamElement::Rest(rest)));
        }
    }
    Ok(out)
}

/// The notes tied *into* from the beat before, by measure, beat and note.
fn ties_in(part: &Part, voice: usize) -> BTreeSet<(usize, usize, usize)> {
    let mut set = BTreeSet::new();
    let mut previous: Option<&Beat> = None;
    for (m, measure) in part.measures.iter().enumerate() {
        let Some(beats) = measure.voices.get(voice) else {
            continue;
        };
        for (b, beat) in beats.iter().enumerate() {
            if beat.grace {
                continue;
            }
            for (n, note) in beat.notes.iter().enumerate() {
                if note.tied
                    && previous
                        .is_some_and(|before| before.notes.iter().any(|old| old.midi == note.midi))
                {
                    set.insert((m, b, n));
                }
            }
            previous = Some(beat);
        }
    }
    set
}

fn to_stream(imported: &Imported) -> Result<Stream, String> {
    let parts: Vec<&Part> = imported
        .parts
        .iter()
        .filter(|part| {
            (0..part
                .measures
                .iter()
                .map(|m| m.voices.len())
                .max()
                .unwrap_or(0))
                .any(|v| has_notes(part, v))
        })
        .collect();
    let Some(lead) = parts.first() else {
        return Err(if imported.skipped.is_empty() {
            "there are no notes to import".to_string()
        } else {
            format!(
                "there are no notes to import; left out: {}",
                imported.skipped.join(", ")
            )
        });
    };
    let count = parts
        .iter()
        .map(|part| part.measures.len())
        .max()
        .unwrap_or(0);
    let frames: Vec<super::Measure> = (0..count)
        .map(|index| {
            let mut frame = lead.measures.get(index).cloned().unwrap_or_default();
            frame.voices.clear();
            frame
        })
        .collect();

    // The meter and key in force in each measure, and what every voice is
    // filled to: a short first or last bar is a pickup or a closing bar, any
    // other is filled out to the bar.
    let first_key = frames
        .first()
        .and_then(|frame| frame.key)
        .unwrap_or_else(|| estimated_key(&parts));
    let mut layout: Vec<Frame> = Vec::with_capacity(count);
    let (mut meter, mut key) = ((4, 4), first_key);
    for (index, frame) in frames.iter().enumerate() {
        meter = frame.meter.unwrap_or(meter);
        key = frame.key.unwrap_or(key);
        let bar = meter.0 as i64 * 4 * TICKS / meter.1.max(1) as i64;
        let content = parts
            .iter()
            .flat_map(|part| part.measures.get(index))
            .flat_map(|measure| measure.voices.iter())
            .map(|beats| beats.iter().map(Beat::length).sum::<i64>())
            .max()
            .unwrap_or(0);
        let partial = content > 0 && content < bar && (index == 0 || index + 1 == count);
        layout.push(Frame {
            meter,
            key,
            target: if partial { content } else { bar.max(content) },
        });
    }
    let keys: Vec<Key> = layout
        .iter()
        .map(|frame| key_of(frame.key))
        .collect::<Result<_, _>>()?;

    let mut score = Stream::with_kind(StreamKind::Score);
    let mut metadata = Metadata::new();
    let title = imported.title.trim();
    metadata.add_text(
        "title",
        if title.is_empty() {
            "Imported score"
        } else {
            title
        },
    );
    score.set_metadata(Some(metadata));

    for (p, part) in parts.iter().enumerate() {
        let mut stream = Stream::with_kind(StreamKind::Part);
        if !part.name.is_empty() {
            stream.set_name(Some(part.name.clone()));
        }
        if let Some(program) = part.program {
            let mut instrument = Instrument::new();
            instrument.set_midi_program(Some(program));
            stream.insert(0.0, StreamElement::Instrument(Box::new(instrument)));
        }
        let clef = clef_of(part)?;
        let widest = part
            .measures
            .iter()
            .map(|measure| measure.voices.len())
            .max()
            .unwrap_or(0);
        let voices: Vec<usize> = (0..widest).filter(|&v| has_notes(part, v)).collect();
        let ties: Vec<_> = voices.iter().map(|&v| ties_out(part, v)).collect();
        let tied_in: Vec<_> = voices.iter().map(|&v| ties_in(part, v)).collect();

        let mut offset = 0.0;
        for index in 0..count {
            let frame = &layout[index];
            let mut measure = Stream::with_kind(StreamKind::Measure);
            measure.set_number(index as i32 + 1);
            // What opens the measure, in the order music21 sorts it: the
            // clef, the key, the meter, the tempo.
            if index == 0 {
                measure.insert(0.0, StreamElement::Clef(clef.clone()));
            }
            let previous = index.checked_sub(1).map(|before| &layout[before]);
            if previous.is_none_or(|before| before.key != frame.key) {
                measure.insert(0.0, StreamElement::Key(keys[index].clone()));
            }
            if previous.is_none_or(|before| before.meter != frame.meter) {
                let meter = TimeSignature::new(frame.meter.0, frame.meter.1)
                    .map_err(|err| err.to_string())?;
                measure.insert(0.0, StreamElement::TimeSignature(meter));
            }
            if index == 0
                && p == 0
                && let Some(tempo) = imported.tempo
            {
                measure.insert(
                    0.0,
                    StreamElement::MetronomeMark(MetronomeMark::new(tempo.round().max(1.0))),
                );
            }

            let this = &frames[index];
            let next = frames.get(index + 1);
            if this.repeat_start {
                measure.set_left_barline(Some(Barline::repeat(RepeatDirection::Start, None)));
            }
            if this.repeat_end > 0 {
                measure.set_right_barline(Some(Barline::repeat(
                    RepeatDirection::End,
                    Some(this.repeat_end),
                )));
            } else if next.is_none() {
                measure.set_right_barline(Some(Barline::new(BarlineType::Final)));
            } else if !this.ending.is_empty() && next.is_some_and(|next| next.ending.is_empty()) {
                measure.set_right_barline(Some(Barline::new(BarlineType::Double)));
            }
            if !this.ending.is_empty() {
                let starts = index == 0 || frames[index - 1].ending != this.ending;
                let stops = next.is_none_or(|next| next.ending != this.ending);
                measure.set_ending(Some(Ending::new(this.ending.clone(), starts, stops)));
            }
            // The section's name and how often a repeat is played, over the
            // first part's first note.
            if p == 0 {
                if let Some(marker) = &this.marker {
                    measure.insert(0.0, text(marker, Placement::Above));
                }
                if this.repeat_end > 2 {
                    measure.insert(
                        0.0,
                        text(&format!("×{}", this.repeat_end), Placement::Above),
                    );
                }
            }

            for (order, &v) in voices.iter().enumerate() {
                let elements = voice_measure(
                    part,
                    v,
                    index,
                    &keys[index],
                    &ties[order],
                    &tied_in[order],
                    order > 0,
                    frame.target,
                )?;
                let ticks = |at: i64| at as f64 / TICKS as f64;
                if voices.len() > 1 {
                    let mut voice = Stream::with_kind(StreamKind::Voice);
                    for (at, element) in elements {
                        voice.insert(ticks(at), element);
                    }
                    measure.insert(0.0, StreamElement::Stream(Box::new(voice)));
                } else {
                    for (at, element) in elements {
                        measure.insert(ticks(at), element);
                    }
                }
            }
            stream.insert(offset, StreamElement::Stream(Box::new(measure)));
            offset += frame.target as f64 / TICKS as f64;
        }
        // The notation the measures leave unsaid: which accidentals are
        // written, the beams, and the tuplets' brackets. The measures and
        // their ties are made here, so nothing is cut or tied again.
        music21_rs::makenotation::make_accidentals(&mut stream);
        // A meter no beaming rule fits leaves its notes unbeamed.
        let _ = music21_rs::makenotation::make_beams(&mut stream);
        for event in stream.events_mut() {
            if let StreamElement::Stream(measure) = event.element_mut() {
                music21_rs::makenotation::make_tuplet_brackets(measure);
                for inner in measure.events_mut() {
                    if let StreamElement::Stream(voice) = inner.element_mut() {
                        music21_rs::makenotation::make_tuplet_brackets(voice);
                    }
                }
            }
        }
        score.insert(0.0, StreamElement::Stream(Box::new(stream)));
    }
    Ok(score)
}
