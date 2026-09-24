//! The score editor's bindings: a score handed over from abcjs, analysed with
//! the crate and written out as MIDI or MusicXML, and a MIDI file read back
//! into ABC for the editor to open.
//!
//! abcjs parses the ABC text, lays it out and plays it; what crosses into
//! Rust is only what it sounded — each note's start and length, its MIDI
//! number, the staff position it was written on, and where in the text it
//! came from. Everything musical is decided here.

use music21_rs::{
    Chord, DurationType, Interval, Key, MidiNote, Minor67Default, Pitch, RomanNumeral,
    TimeSignature, TuningSystem, VoiceLeadingQuartet, abc_duration, abc_note,
    chord_symbol_figure_from_chord, estimate_key_from_pitches, read_midi_bytes_with_tempo,
    roman_numeral_from_chord, tonal_certainty, write_midi_bytes,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt::Write as _};
use wasm_bindgen::prelude::*;

/// Positions are snapped to this many parts of a quarter note, which holds
/// every binary value and triplets, quintuplets and septuplets of them, and
/// absorbs the six-decimal rounding abcjs gives tuplet timings.
const GRID: f64 = 5040.0;
/// MusicXML `divisions`: every notatable value `notatable` offers is a whole
/// number of these.
const DIVISIONS: f64 = 3360.0;
const EPSILON: f64 = 1e-6;
const STEPS: [char; 7] = ['C', 'D', 'E', 'F', 'G', 'A', 'B'];
const STEP_SEMITONES: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];

fn snap(quarters: f64) -> f64 {
    (quarters * GRID).round() / GRID
}

fn js_error(err: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&err.to_string())
}

// ------------------------------------------------------------------ input

/// A score as the page hands it over, in quarter lengths.
#[derive(Clone, Deserialize)]
pub struct ScoreInput {
    #[serde(default)]
    title: String,
    #[serde(default = "default_tempo")]
    tempo_bpm: f64,
    #[serde(default)]
    key: Option<WrittenKey>,
    #[serde(default)]
    meter: Option<[u32; 2]>,
    #[serde(default)]
    pickup: f64,
    /// Whether a note keeps sounding past its written end where nothing
    /// stops it, as a plucked or struck string does.
    #[serde(default = "default_let_ring")]
    let_ring: bool,
    voices: Vec<VoiceInput>,
}

fn default_tempo() -> f64 {
    120.0
}

/// The key signature as written: its tonic (`B-`), mode (`major`, `minor`,
/// `dorian`, ...) and signed count of sharps.
#[derive(Clone, Deserialize)]
struct WrittenKey {
    tonic: String,
    mode: String,
    sharps: i32,
}

#[derive(Clone, Deserialize)]
struct VoiceInput {
    #[serde(default)]
    name: String,
    #[serde(default)]
    clef: String,
    notes: Vec<NoteInput>,
}

#[derive(Clone, Deserialize)]
struct NoteInput {
    start: f64,
    duration: f64,
    #[serde(default = "default_velocity")]
    velocity: u8,
    pitches: Vec<PitchInput>,
    #[serde(default)]
    char_start: Option<usize>,
    #[serde(default)]
    char_end: Option<usize>,
}

fn default_velocity() -> u8 {
    90
}

fn default_let_ring() -> bool {
    true
}

/// How far apart two notes must be to count as clear of each other rather
/// than struck in the same register: a major third. A guitar's thumb plays
/// its bass a good deal further below the chord than that.
const REGISTER_CLEARANCE: f64 = 4.0;

/// A sounding MIDI number and, where it was written on a staff, the diatonic
/// position it was written at: `0` is middle C, `1` the D above it.
#[derive(Clone, Deserialize)]
struct PitchInput {
    midi: i32,
    #[serde(default)]
    staff: Option<i32>,
}

// ------------------------------------------------------------------ model

struct Event {
    start: f64,
    end: f64,
    pitches: Vec<Pitch>,
    velocity: u8,
    chars: Option<[usize; 2]>,
}

struct Voice {
    name: String,
    clef: String,
    events: Vec<Event>,
}

struct Score {
    title: String,
    tempo_bpm: f64,
    key: Option<WrittenKey>,
    meter: TimeSignature,
    pickup: f64,
    voices: Vec<Voice>,
}

/// Spells a sounding MIDI number on the staff position it was written at, so
/// `F#` stays `F#` and a written `G-` is not respelled `F#`. A staff that
/// sounds whole octaves from where it is written (`treble-8`, a guitar's) is
/// spelled on its letter in the octave it sounds in. A position that cannot
/// carry the MIDI number with at most a double accidental falls back to the
/// crate's own spelling.
fn spell(input: &PitchInput) -> Result<Pitch, JsValue> {
    if let Some(staff) = input.staff {
        let index = staff.rem_euclid(7) as usize;
        let written_octave = 4 + staff.div_euclid(7);
        let natural = 12 * (written_octave + 1) + STEP_SEMITONES[index];
        let octaves = ((input.midi - natural) as f64 / 12.0).round() as i32;
        let alter = input.midi - natural - 12 * octaves;
        let octave = written_octave + octaves;
        if (-2..=2).contains(&alter) {
            let accidental = if alter < 0 {
                "-".repeat(alter.unsigned_abs() as usize)
            } else {
                "#".repeat(alter as usize)
            };
            return Pitch::from_name(format!("{}{accidental}{octave}", STEPS[index]))
                .map_err(js_error);
        }
    }
    Pitch::from_midi(input.midi).map_err(js_error)
}

impl Score {
    fn from_input(input: ScoreInput) -> Result<Self, JsValue> {
        let meter = match input.meter {
            Some([numerator, denominator]) if numerator > 0 && denominator > 0 => {
                TimeSignature::new(numerator, denominator).map_err(js_error)?
            }
            _ => TimeSignature::common(),
        };
        let mut voices = Vec::with_capacity(input.voices.len());
        for voice in input.voices {
            let mut events = Vec::with_capacity(voice.notes.len());
            for note in voice.notes {
                if note.pitches.is_empty() || note.duration <= 0.0 {
                    continue;
                }
                let mut pitches = note
                    .pitches
                    .iter()
                    .map(spell)
                    .collect::<Result<Vec<_>, _>>()?;
                pitches.sort_by(|left, right| left.ps().total_cmp(&right.ps()));
                pitches.dedup_by(|left, right| left.name_with_octave() == right.name_with_octave());
                let start = snap(note.start.max(0.0));
                events.push(Event {
                    start,
                    end: snap(note.start.max(0.0) + note.duration).max(start + 1.0 / GRID),
                    pitches,
                    velocity: note.velocity.min(127),
                    chars: note.char_start.zip(note.char_end).map(|(a, b)| [a, b]),
                });
            }
            events.sort_by(|left, right| left.start.total_cmp(&right.start));
            voices.push(Voice {
                name: voice.name,
                clef: voice.clef,
                events,
            });
        }
        let mut score = Self {
            title: input.title,
            tempo_bpm: if input.tempo_bpm.is_finite() && input.tempo_bpm > 0.0 {
                input.tempo_bpm
            } else {
                default_tempo()
            },
            key: input.key,
            meter,
            pickup: snap(input.pickup.max(0.0)),
            voices,
        };
        if input.let_ring {
            score.let_ring();
        }
        Ok(score)
    }

    /// Lets a note go on sounding until something is struck in its own
    /// register, which is what a guitar or a piano does: the bass under a
    /// chord rings on, and the chord rings over the bass note after it.
    ///
    /// It applies only where a voice mixes single notes with chords, one
    /// part playing bass and harmony at once. A voice of single notes is a
    /// line, whose notes stop at the next one however far it leaps, and is
    /// left alone. Nothing rings past the end of its bar.
    fn let_ring(&mut self) {
        let bar = self.bar();
        let pickup = self.anacrusis();
        let bar_end = |offset: f64| {
            if offset < pickup - EPSILON {
                pickup
            } else {
                pickup + ((offset - pickup) / bar + EPSILON).floor() * bar + bar
            }
        };
        for voice in &mut self.voices {
            let spans: Vec<(f64, f64, f64, usize)> = voice
                .events
                .iter()
                .map(|event| {
                    let low = event.pitches.iter().map(Pitch::ps).fold(f64::MAX, f64::min);
                    let high = event.pitches.iter().map(Pitch::ps).fold(f64::MIN, f64::max);
                    (event.start, low, high, event.pitches.len())
                })
                .collect();
            for index in 0..spans.len() {
                let (start, low, high, count) = spans[index];
                let mut ends = bar_end(start);
                for &(next_start, next_low, next_high, next_count) in &spans[index + 1..] {
                    if next_start <= start + EPSILON {
                        continue;
                    }
                    let clear = next_low > high + REGISTER_CLEARANCE
                        || next_high < low - REGISTER_CLEARANCE;
                    // Two single notes are a line, whatever the leap.
                    if !clear || (count == 1 && next_count == 1) {
                        ends = next_start;
                        break;
                    }
                }
                let event = &mut voice.events[index];
                event.end = event.end.max(ends.min(bar_end(start)));
            }
        }
    }

    fn end(&self) -> f64 {
        self.voices
            .iter()
            .flat_map(|voice| voice.events.iter().map(|event| event.end))
            .fold(0.0, f64::max)
    }

    fn bar(&self) -> f64 {
        self.meter.bar_quarter_length()
    }

    /// The pickup, if it is shorter than a bar; a "pickup" of a whole bar is
    /// just the first bar.
    fn anacrusis(&self) -> f64 {
        if self.pickup > EPSILON && self.pickup < self.bar() - EPSILON {
            self.pickup
        } else {
            0.0
        }
    }

    /// Measure number and beat of an offset, counting a pickup as measure 0
    /// the way music21 does.
    fn position(&self, offset: f64) -> (i32, f64) {
        let bar = self.bar();
        let beat = self.meter.beat_quarter_length().unwrap_or(1.0);
        let pickup = self.anacrusis();
        if offset < pickup - EPSILON {
            return (0, 1.0 + (bar - pickup + offset) / beat);
        }
        let since = offset - pickup;
        let measure = ((since + EPSILON) / bar).floor();
        (1 + measure as i32, 1.0 + (since - measure * bar) / beat)
    }

    /// Barline offsets from the start to the end of the score, both ends
    /// included. The last bar ends where the music does, so a piece that
    /// opens with a pickup closes on the short bar that balances it.
    fn barlines(&self) -> Vec<f64> {
        let end = self.end();
        let mut lines = vec![0.0];
        let mut next = if self.anacrusis() > 0.0 {
            self.anacrusis()
        } else {
            self.bar()
        };
        while next < end - EPSILON {
            lines.push(next);
            next += self.bar();
        }
        lines.push(if end > EPSILON { end } else { next });
        lines
    }

    fn written_key(&self) -> Option<Key> {
        let key = self.key.as_ref()?;
        Key::from_tonic_mode(&key.tonic, key.mode.as_str()).ok()
    }
}

// --------------------------------------------------------------- analysis

#[derive(Serialize)]
struct KeyEstimateInfo {
    key: String,
    score: f64,
}

#[derive(Serialize)]
struct SliceInfo {
    offset: f64,
    duration: f64,
    measure: i32,
    beat: f64,
    pitches: Vec<String>,
    pitch_classes: Vec<u8>,
    common_name: String,
    pitched_common_name: String,
    chord_symbol: Option<String>,
    /// The numeral as music21 writes it: a flat or sharp in front of a minor
    /// key's sixth or seventh degree as a caution, and every figure written
    /// out, `bVII7` and `vio64#2`.
    numeral: Option<String>,
    /// The same numeral as a harmony textbook writes it: minor's natural
    /// sixth and seventh unmarked, and the inversion as its usual figure,
    /// `VII7` and `vio42`.
    textbook_numeral: Option<String>,
    inversion: Option<u8>,
    root: Option<String>,
    bass: Option<String>,
    consonant: bool,
    /// Whether the pitch classes differ from the slice before, so a page can
    /// label only the changes of harmony.
    changed: bool,
    /// Text ranges of the notes that start at this offset.
    attacks: Vec<[usize; 2]>,
    /// Text ranges of every note sounding at this offset.
    sounding: Vec<[usize; 2]>,
    /// The text range of the note carrying the lowest pitch, when that note
    /// starts here: where a label under the music belongs.
    bass_attack: Option<[usize; 2]>,
}

#[derive(Serialize)]
struct IssueInfo {
    kind: &'static str,
    severity: &'static str,
    offset: f64,
    measure: i32,
    beat: f64,
    voices: Vec<usize>,
    detail: String,
    chars: Vec<[usize; 2]>,
}

#[derive(Serialize)]
struct VoiceInfo {
    name: String,
    notes: usize,
    lowest: Option<String>,
    highest: Option<String>,
    range: Option<String>,
    largest_leap: Option<String>,
}

#[derive(Serialize)]
struct ScoreAnalysis {
    title: String,
    tempo_bpm: f64,
    meter: String,
    written_key: Option<String>,
    analysis_key: Option<String>,
    key_estimates: Vec<KeyEstimateInfo>,
    tonal_certainty: f64,
    measures: i32,
    quarter_length: f64,
    note_count: usize,
    pitch_class_weights: [f64; 12],
    voices: Vec<VoiceInfo>,
    slices: Vec<SliceInfo>,
    issues: Vec<IssueInfo>,
}

fn key_label(key: &Key) -> String {
    format!("{} {}", key.tonic_pitch().name(), key.mode())
}

fn pitch_class(pitch: &Pitch) -> u8 {
    (pitch.ps().round() as i32).rem_euclid(12) as u8
}

/// What each voice sounds at one offset: its highest pitch, the text range
/// of the note it belongs to, and whether that note starts there.
#[derive(Clone)]
struct Sounding<'a> {
    top: &'a Pitch,
    event: &'a Event,
}

fn sounding_at(voice: &Voice, offset: f64) -> Option<Sounding<'_>> {
    voice
        .events
        .iter()
        .filter(|event| event.start <= offset + EPSILON && event.end > offset + EPSILON)
        .max_by(|left, right| left.start.total_cmp(&right.start))
        .and_then(|event| event.pitches.last().map(|top| Sounding { top, event }))
}

fn analyse(score: &Score) -> Result<ScoreAnalysis, JsValue> {
    let end = score.end();
    let mut onsets: Vec<f64> = score
        .voices
        .iter()
        .flat_map(|voice| voice.events.iter().map(|event| event.start))
        .collect();
    onsets.sort_by(f64::total_cmp);
    onsets.dedup_by(|left, right| (*left - *right).abs() < EPSILON);

    // Key: the pitches weighted by how long they sound, in sixteenths.
    let mut weighted = Vec::new();
    let mut pitch_class_weights = [0.0; 12];
    let mut note_count = 0;
    for voice in &score.voices {
        for event in &voice.events {
            let length = event.end - event.start;
            for pitch in &event.pitches {
                note_count += 1;
                pitch_class_weights[pitch_class(pitch) as usize] += length;
                let copies = ((length * 4.0).round() as usize).clamp(1, 16);
                weighted.extend(std::iter::repeat_n(pitch.clone(), copies));
            }
        }
    }
    let estimates = if weighted.is_empty() {
        Vec::new()
    } else {
        estimate_key_from_pitches(&weighted).map_err(js_error)?
    };
    let written_key = score.written_key();
    let analysis_key = match &written_key {
        Some(key) if matches!(key.mode(), "major" | "minor") => Some(key.clone()),
        _ => estimates.first().map(|estimate| estimate.key().clone()),
    };

    let slices = harmonic_slices(score, &onsets, end, analysis_key.as_ref())?;
    let mut issues = voice_leading_issues(score, &onsets);
    issues.extend(melodic_issues(score));
    issues.sort_by(|left, right| left.offset.total_cmp(&right.offset));

    let voices = score.voices.iter().map(describe_voice).collect();
    let measures = if end > 0.0 {
        score.position(end - 1.0 / GRID).0.max(1)
    } else {
        0
    };

    Ok(ScoreAnalysis {
        title: score.title.clone(),
        tempo_bpm: score.tempo_bpm,
        meter: score.meter.ratio_string(),
        written_key: score.key.as_ref().map(|key| match &written_key {
            Some(parsed) => key_label(parsed),
            None => format!("{} {}", key.tonic, key.mode),
        }),
        analysis_key: analysis_key.as_ref().map(key_label),
        key_estimates: estimates
            .iter()
            .take(5)
            .map(|estimate| KeyEstimateInfo {
                key: key_label(estimate.key()),
                score: estimate.score(),
            })
            .collect(),
        tonal_certainty: tonal_certainty(&estimates),
        measures,
        quarter_length: end,
        note_count,
        pitch_class_weights,
        voices,
        slices,
        issues,
    })
}

fn harmonic_slices(
    score: &Score,
    onsets: &[f64],
    end: f64,
    key: Option<&Key>,
) -> Result<Vec<SliceInfo>, JsValue> {
    let mut slices = Vec::new();
    let mut previous_classes: Option<Vec<u8>> = None;
    for (index, &offset) in onsets.iter().enumerate() {
        let next = onsets.get(index + 1).copied().unwrap_or(end);
        let mut pitches: Vec<Pitch> = Vec::new();
        let mut attacks = Vec::new();
        let mut sounding = Vec::new();
        let mut lowest: Option<(f64, &Event)> = None;
        for voice in &score.voices {
            for event in &voice.events {
                if event.start <= offset + EPSILON && event.end > offset + EPSILON {
                    if let Some(low) = event.pitches.first()
                        && lowest.is_none_or(|(ps, _)| low.ps() < ps)
                    {
                        lowest = Some((low.ps(), event));
                    }
                    pitches.extend(event.pitches.iter().cloned());
                    if let Some(chars) = event.chars {
                        sounding.push(chars);
                        if (event.start - offset).abs() < EPSILON {
                            attacks.push(chars);
                        }
                    }
                }
            }
        }
        if pitches.is_empty() {
            previous_classes = None;
            continue;
        }
        pitches.sort_by(|left, right| left.ps().total_cmp(&right.ps()));
        pitches.dedup_by(|left, right| left.name_with_octave() == right.name_with_octave());
        let mut classes: Vec<u8> = pitches.iter().map(pitch_class).collect();
        classes.sort_unstable();
        classes.dedup();

        let chord = Chord::new(pitches.clone()).map_err(js_error)?;
        let harmonic = classes.len() >= 3;
        let found = match key {
            Some(key) if harmonic => roman_numeral_from_chord(&chord, Some(key)).ok().flatten(),
            _ => None,
        };
        let numeral = found.as_ref().map(|numeral| numeral.figure().to_string());
        let textbook_numeral = found.as_ref().map(|numeral| {
            textbook_figure(numeral, &chord).unwrap_or_else(|| numeral.figure().to_string())
        });
        let chord_symbol = if harmonic {
            chord_symbol_figure_from_chord(&chord).ok().flatten()
        } else {
            None
        };
        let (measure, beat) = score.position(offset);
        let changed = previous_classes.as_ref() != Some(&classes);
        previous_classes = Some(classes.clone());
        slices.push(SliceInfo {
            offset,
            duration: next - offset,
            measure,
            beat,
            pitches: pitches.iter().map(Pitch::name_with_octave).collect(),
            pitch_classes: classes,
            common_name: chord.common_name(),
            pitched_common_name: chord.pitched_common_name(),
            chord_symbol,
            numeral,
            textbook_numeral,
            inversion: if harmonic { chord.inversion() } else { None },
            root: chord.root().map(Pitch::name),
            bass: chord.bass().map(Pitch::name),
            consonant: chord.is_consonant(),
            changed,
            attacks,
            sounding,
            bass_attack: lowest
                .filter(|(_, event)| (event.start - offset).abs() < EPSILON)
                .and_then(|(_, event)| event.chars),
        });
    }
    Ok(slices)
}

/// `numeral` written as a harmony textbook writes it: a triad's inversion as
/// `6` or `64` and a seventh chord's as `65`, `43` or `42`, and a flat or
/// sharp in front only where the numeral read the textbook way, with minor's
/// natural sixth and seventh unmarked, would otherwise be another chord.
/// Every candidate is read back and kept only if it is `chord` over the same
/// bass. `None` for what a textbook has no shorter way of writing — an
/// applied chord, a named one such as `It6`, a ninth.
fn textbook_figure(numeral: &RomanNumeral, chord: &Chord) -> Option<String> {
    if numeral.secondary().is_some() {
        return None;
    }
    let alone = numeral.roman_numeral_alone();
    let unaltered = numeral.figure().trim_start_matches(['b', '#', '-']);
    if alone.is_empty() || !unaltered.starts_with(&alone) {
        return None;
    }
    let mark = if chord.is_diminished_triad() || chord.is_diminished_seventh() {
        "o"
    } else if chord.is_half_diminished_seventh() {
        "ø"
    } else if chord.is_augmented_triad() {
        "+"
    } else {
        ""
    };
    let inversion = usize::from(numeral.inversion());
    let figures = if chord.is_triad() {
        ["", "6", "64"].get(inversion)?
    } else if chord.is_seventh() {
        ["7", "65", "43", "42"].get(inversion)?
    } else {
        return None;
    };
    let classes = |chord: &Chord| {
        let mut classes = chord.pitch_classes();
        classes.sort_unstable();
        classes.dedup();
        classes
    };
    let wanted = (classes(chord), chord.bass().map(pitch_class));
    ["", "b", "#", "bb", "##"].into_iter().find_map(|front| {
        let figure = format!("{front}{alone}{mark}{figures}");
        let read = RomanNumeral::with_minor_defaults(
            figure.as_str(),
            numeral.key().clone(),
            Minor67Default::Quality,
            Minor67Default::Quality,
        )
        .ok()?
        .to_chord()
        .ok()?;
        (classes(&read), read.bass().map(pitch_class))
            .eq(&wanted)
            .then_some(figure)
    })
}

fn chars_of(soundings: &[&Sounding<'_>]) -> Vec<[usize; 2]> {
    let mut chars: Vec<[usize; 2]> = soundings
        .iter()
        .filter_map(|sounding| sounding.event.chars)
        .collect();
    chars.sort_unstable();
    chars.dedup();
    chars
}

fn motion_detail(upper: [&Pitch; 2], lower: [&Pitch; 2]) -> String {
    format!(
        "{} → {} over {} → {}",
        upper[0].name_with_octave(),
        upper[1].name_with_octave(),
        lower[0].name_with_octave(),
        lower[1].name_with_octave()
    )
}

/// Parallel and hidden fifths and octaves, and voices crossing, between
/// every pair of voices from one onset to the next. Voices are taken in the
/// order they were written, the first being the highest; hidden intervals
/// are only a fault between the outer voices, so only those are reported.
fn voice_leading_issues(score: &Score, onsets: &[f64]) -> Vec<IssueInfo> {
    let count = score.voices.len();
    let mut issues = Vec::new();
    if count < 2 {
        return issues;
    }
    let states: Vec<Vec<Option<Sounding<'_>>>> = onsets
        .iter()
        .map(|&offset| {
            score
                .voices
                .iter()
                .map(|voice| sounding_at(voice, offset))
                .collect()
        })
        .collect();
    let mut crossed = vec![vec![false; count]; count];

    for index in 1..onsets.len() {
        let (measure, beat) = score.position(onsets[index]);
        for upper in 0..count {
            for lower in upper + 1..count {
                let (Some(u1), Some(u2), Some(l1), Some(l2)) = (
                    states[index - 1][upper].as_ref(),
                    states[index][upper].as_ref(),
                    states[index - 1][lower].as_ref(),
                    states[index][lower].as_ref(),
                ) else {
                    crossed[upper][lower] = false;
                    continue;
                };
                let is_crossed = u2.top.ps() < l2.top.ps();
                if is_crossed && !crossed[upper][lower] {
                    issues.push(IssueInfo {
                        kind: "Voice crossing",
                        severity: "info",
                        offset: onsets[index],
                        measure,
                        beat,
                        voices: vec![upper, lower],
                        detail: format!(
                            "{} is below {}",
                            u2.top.name_with_octave(),
                            l2.top.name_with_octave()
                        ),
                        chars: chars_of(&[u2, l2]),
                    });
                }
                crossed[upper][lower] = is_crossed;

                let moved = std::ptr::eq(u1.event, u2.event) && std::ptr::eq(l1.event, l2.event);
                if moved {
                    continue;
                }
                let Ok(quartet) = VoiceLeadingQuartet::new(
                    u1.top.clone(),
                    u2.top.clone(),
                    l1.top.clone(),
                    l2.top.clone(),
                ) else {
                    continue;
                };
                let outer = upper == 0 && lower == count - 1;
                // music21 counts a fifth that opens to a twelfth by contrary
                // motion as a parallel fifth; it is reported, but apart.
                let contrary = (u2.top.ps() - u1.top.ps()) * (l2.top.ps() - l1.top.ps()) < 0.0;
                let found = if quartet.parallel_fifth() && contrary {
                    Some(("Fifths by contrary motion", "info"))
                } else if quartet.parallel_fifth() {
                    Some(("Parallel fifths", "warning"))
                } else if quartet.parallel_octave() && contrary {
                    Some(("Octaves by contrary motion", "info"))
                } else if quartet.parallel_octave() {
                    Some(("Parallel octaves", "warning"))
                } else if outer && quartet.hidden_fifth() {
                    Some(("Hidden fifths", "info"))
                } else if outer && quartet.hidden_octave() {
                    Some(("Hidden octaves", "info"))
                } else {
                    None
                };
                if let Some((kind, severity)) = found {
                    issues.push(IssueInfo {
                        kind,
                        severity,
                        offset: onsets[index],
                        measure,
                        beat,
                        voices: vec![upper, lower],
                        detail: motion_detail([u1.top, u2.top], [l1.top, l2.top]),
                        chars: chars_of(&[u1, u2, l1, l2]),
                    });
                }
            }
        }
    }
    issues
}

/// Consecutive notes of one voice, taking a chord's top note as its line.
fn melodic_steps(voice: &Voice) -> impl Iterator<Item = (&Event, &Event, Interval)> {
    voice.events.windows(2).filter_map(|pair| {
        let (first, second) = (&pair[0], &pair[1]);
        if (second.start - first.end).abs() > EPSILON {
            return None;
        }
        let interval =
            Interval::between_pitches(first.pitches.last()?, second.pitches.last()?).ok()?;
        Some((first, second, interval))
    })
}

fn melodic_issues(score: &Score) -> Vec<IssueInfo> {
    let mut issues = Vec::new();
    for (index, voice) in score.voices.iter().enumerate() {
        for (first, second, interval) in melodic_steps(voice) {
            let name = interval.short_name();
            let semitones = interval.semitones().abs();
            let found = if semitones > 12.0 + EPSILON {
                Some(("Leap wider than an octave", "info"))
            } else if (name.starts_with('A') || name.starts_with('d'))
                && interval.generic_number().abs() > 1
            {
                // An augmented unison is a chromatic half step, F to F#,
                // not a leap.
                Some(("Augmented or diminished leap", "warning"))
            } else {
                None
            };
            if let Some((kind, severity)) = found {
                let (measure, beat) = score.position(second.start);
                issues.push(IssueInfo {
                    kind,
                    severity,
                    offset: second.start,
                    measure,
                    beat,
                    voices: vec![index],
                    detail: format!(
                        "{} from {} to {}",
                        leap_name(&interval),
                        first
                            .pitches
                            .last()
                            .map(Pitch::name_with_octave)
                            .unwrap_or_default(),
                        second
                            .pitches
                            .last()
                            .map(Pitch::name_with_octave)
                            .unwrap_or_default()
                    ),
                    chars: first.chars.into_iter().chain(second.chars).collect(),
                });
            }
        }
    }
    issues
}

/// A melodic interval as a reader says it: `perfect fifth down`.
fn leap_name(interval: &Interval) -> String {
    let direction = if interval.semitones() < 0.0 {
        "down"
    } else {
        "up"
    };
    format!("{} {direction}", interval.name().to_lowercase())
}

fn describe_voice(voice: &Voice) -> VoiceInfo {
    let all = || voice.events.iter().flat_map(|event| event.pitches.iter());
    let lowest = all().min_by(|left, right| left.ps().total_cmp(&right.ps()));
    let highest = all().max_by(|left, right| left.ps().total_cmp(&right.ps()));
    let range = lowest
        .zip(highest)
        .and_then(|(low, high)| Interval::between_pitches(low, high).ok())
        .map(|interval| interval.name());
    let largest_leap = melodic_steps(voice)
        .max_by(|left, right| {
            left.2
                .semitones()
                .abs()
                .total_cmp(&right.2.semitones().abs())
        })
        .filter(|(_, _, interval)| interval.semitones().abs() > EPSILON)
        .map(|(_, _, interval)| leap_name(&interval));
    VoiceInfo {
        name: voice.name.clone(),
        notes: voice.events.iter().map(|event| event.pitches.len()).sum(),
        lowest: lowest.map(Pitch::name_with_octave),
        highest: highest.map(Pitch::name_with_octave),
        range,
        largest_leap,
    }
}

// ------------------------------------------------------------------- MIDI

fn midi_bytes(score: &Score) -> Result<Vec<u8>, JsValue> {
    let mut notes = Vec::new();
    for (index, voice) in score.voices.iter().enumerate() {
        // Channel 10 is percussion in General MIDI; step over it.
        let channel = match index % 15 {
            skip if skip >= 9 => skip + 1,
            other => other,
        } as u8;
        for event in &voice.events {
            for pitch in &event.pitches {
                let midi = pitch.midi();
                if !(0..=127).contains(&midi) {
                    continue;
                }
                notes.push(
                    MidiNote::with_channel(
                        midi as u8,
                        event.start,
                        event.end - event.start,
                        event.velocity,
                        channel,
                    )
                    .map_err(js_error)?,
                );
            }
        }
    }
    write_midi_bytes(&notes, score.tempo_bpm).map_err(js_error)
}

// --------------------------------------------------------------- MusicXML

/// A written value: its length, type name, dots and tuplet ratio.
#[derive(Clone, Copy)]
struct Notatable {
    quarters: f64,
    type_name: &'static str,
    dots: u32,
    tuplet: Option<(u32, u32)>,
}

/// Every value the writer spells a length with, longest first: plain and
/// dotted values from a whole note down, then the same types inside
/// triplets, quintuplets and septuplets.
fn notatable() -> Vec<Notatable> {
    let types = [
        DurationType::Whole,
        DurationType::Half,
        DurationType::Quarter,
        DurationType::Eighth,
        DurationType::Sixteenth,
        DurationType::ThirtySecond,
        DurationType::SixtyFourth,
        DurationType::HundredTwentyEighth,
    ];
    let mut values = Vec::new();
    for duration_type in types {
        let max_dots = match duration_type {
            DurationType::HundredTwentyEighth => 0,
            DurationType::SixtyFourth => 1,
            _ => 2,
        };
        for dots in 0..=max_dots {
            values.push(Notatable {
                quarters: duration_type.quarter_length_with_dots(dots),
                type_name: duration_type.music21_name(),
                dots,
                tuplet: None,
            });
        }
        for (actual, normal) in [(3, 2), (5, 4), (7, 4)] {
            values.push(Notatable {
                quarters: duration_type.quarter_length() * normal as f64 / actual as f64,
                type_name: duration_type.music21_name(),
                dots: 0,
                tuplet: Some((actual, normal)),
            });
        }
    }
    values.sort_by(|left, right| right.quarters.total_cmp(&left.quarters));
    values
}

/// Spells a length as a run of tied written values. A length that plain
/// values can express is written without tuplets.
fn spell_length(length: f64, values: &[Notatable]) -> Vec<Notatable> {
    let binary = ((length * 32.0) - (length * 32.0).round()).abs() < 1e-4;
    let mut remaining = length;
    let mut out = Vec::new();
    while remaining > 1.0 / GRID {
        let pick = values
            .iter()
            .filter(|value| !binary || value.tuplet.is_none())
            .find(|value| value.quarters <= remaining + 1e-4)
            .or_else(|| values.last())
            .copied();
        let Some(value) = pick else { break };
        out.push(value);
        remaining -= value.quarters;
        if out.len() > 32 {
            break;
        }
    }
    out
}

struct XmlNote {
    pitches: Vec<Pitch>,
    value: Notatable,
    tie_start: bool,
    tie_stop: bool,
    tuplet_start: bool,
    tuplet_stop: bool,
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn step_alter(pitch: &Pitch) -> (char, i32, i32) {
    let name = pitch.name();
    let step = name.chars().next().unwrap_or('C');
    let alter = name
        .chars()
        .skip(1)
        .fold(0, |alter, modifier| match modifier {
            '#' => alter + 1,
            '-' => alter - 1,
            _ => alter,
        });
    (step, alter, pitch.octave().unwrap_or(4))
}

/// How the key signature alters each step, from its signed count of sharps.
fn key_alters(sharps: i32) -> BTreeMap<char, i32> {
    const SHARP_ORDER: [char; 7] = ['F', 'C', 'G', 'D', 'A', 'E', 'B'];
    let mut alters = BTreeMap::new();
    for (index, step) in SHARP_ORDER.iter().enumerate() {
        if (index as i32) < sharps {
            alters.insert(*step, 1);
        }
        if (index as i32) < -sharps {
            alters.insert(SHARP_ORDER[6 - index], -1);
        }
    }
    alters
}

fn clef_xml(clef: &str, voice: &Voice) -> (&'static str, u8) {
    match clef {
        name if name.starts_with("bass") => ("F", 4),
        name if name.starts_with("alto") => ("C", 3),
        name if name.starts_with("tenor") => ("C", 4),
        name if name.starts_with("treble") => ("G", 2),
        _ => {
            let pitches: Vec<f64> = voice
                .events
                .iter()
                .flat_map(|event| event.pitches.iter().map(Pitch::ps))
                .collect();
            let mean = pitches.iter().sum::<f64>() / pitches.len().max(1) as f64;
            if !pitches.is_empty() && mean < 57.0 {
                ("F", 4)
            } else {
                ("G", 2)
            }
        }
    }
}

/// One voice as a run of chords and rests: notes that start together are
/// one chord, and each is cut short where the next begins.
fn voice_timeline(voice: &Voice) -> Vec<(f64, f64, Vec<Pitch>)> {
    let mut timeline: Vec<(f64, f64, Vec<Pitch>)> = Vec::new();
    for event in &voice.events {
        match timeline.last_mut() {
            Some(last) if (last.0 - event.start).abs() < EPSILON => {
                last.1 = last.1.max(event.end);
                last.2.extend(event.pitches.iter().cloned());
            }
            _ => timeline.push((event.start, event.end, event.pitches.clone())),
        }
    }
    for index in 1..timeline.len() {
        let next_start = timeline[index].0;
        if timeline[index - 1].1 > next_start {
            timeline[index - 1].1 = next_start;
        }
    }
    for entry in &mut timeline {
        entry
            .2
            .sort_by(|left, right| left.ps().total_cmp(&right.ps()));
        entry
            .2
            .dedup_by(|left, right| left.name_with_octave() == right.name_with_octave());
    }
    timeline
}

fn musicxml(score: &Score) -> String {
    let values = notatable();
    let barlines = score.barlines();
    let sharps = score.key.as_ref().map_or(0, |key| key.sharps).clamp(-7, 7);
    let mode = score
        .key
        .as_ref()
        .map_or("major".to_string(), |key| key.mode.clone());

    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>\n");
    xml.push_str("<!DOCTYPE score-partwise PUBLIC \"-//Recordare//DTD MusicXML 4.0 Partwise//EN\" \"http://www.musicxml.org/dtds/partwise.dtd\">\n");
    xml.push_str("<score-partwise version=\"4.0\">\n");
    if !score.title.is_empty() {
        let _ = writeln!(
            xml,
            "  <work><work-title>{}</work-title></work>",
            xml_escape(&score.title)
        );
    }
    xml.push_str("  <identification><encoding><software>music21-rs score editor</software></encoding></identification>\n");
    xml.push_str("  <part-list>\n");
    for (index, voice) in score.voices.iter().enumerate() {
        let name = if voice.name.is_empty() {
            format!("Voice {}", index + 1)
        } else {
            voice.name.clone()
        };
        let _ = writeln!(
            xml,
            "    <score-part id=\"P{}\"><part-name>{}</part-name></score-part>",
            index + 1,
            xml_escape(&name)
        );
    }
    xml.push_str("  </part-list>\n");

    for (part_index, voice) in score.voices.iter().enumerate() {
        let _ = writeln!(xml, "  <part id=\"P{}\">", part_index + 1);
        let timeline = voice_timeline(voice);
        for (measure_index, bounds) in barlines.windows(2).enumerate() {
            let (measure_start, measure_end) = (bounds[0], bounds[1]);
            let number = if score.anacrusis() > 0.0 {
                measure_index
            } else {
                measure_index + 1
            };
            let implicit = score.anacrusis() > 0.0 && measure_index == 0;
            let _ = writeln!(
                xml,
                "    <measure number=\"{number}\"{}>",
                if implicit { " implicit=\"yes\"" } else { "" }
            );
            if measure_index == 0 {
                let (sign, line) = clef_xml(&voice.clef, voice);
                // A clef written an octave from where it sounds; the pitches
                // below are the sounding ones, as MusicXML has them.
                let octave_change = match voice.clef.as_str() {
                    clef if clef.ends_with("-8") => "<clef-octave-change>-1</clef-octave-change>",
                    clef if clef.ends_with("+8") => "<clef-octave-change>1</clef-octave-change>",
                    _ => "",
                };
                let _ = writeln!(
                    xml,
                    "      <attributes><divisions>{}</divisions><key><fifths>{sharps}</fifths><mode>{}</mode></key><time><beats>{}</beats><beat-type>{}</beat-type></time><clef><sign>{sign}</sign><line>{line}</line>{octave_change}</clef></attributes>",
                    DIVISIONS as u32,
                    xml_escape(&mode),
                    score.meter.numerator(),
                    score.meter.denominator()
                );
                if part_index == 0 {
                    let _ = writeln!(
                        xml,
                        "      <direction placement=\"above\"><direction-type><metronome><beat-unit>quarter</beat-unit><per-minute>{}</per-minute></metronome></direction-type><sound tempo=\"{}\"/></direction>",
                        score.tempo_bpm.round(),
                        score.tempo_bpm
                    );
                }
            }

            let notes = measure_notes(&timeline, measure_start, measure_end, &values);
            write_measure_notes(&mut xml, &notes, sharps);
            xml.push_str("    </measure>\n");
        }
        xml.push_str("  </part>\n");
    }
    xml.push_str("</score-partwise>\n");
    xml
}

fn measure_notes(
    timeline: &[(f64, f64, Vec<Pitch>)],
    measure_start: f64,
    measure_end: f64,
    values: &[Notatable],
) -> Vec<XmlNote> {
    let mut notes = Vec::new();
    let mut cursor = measure_start;
    let push_span = |notes: &mut Vec<XmlNote>,
                     pitches: &[Pitch],
                     length: f64,
                     tied_in: bool,
                     tied_out: bool| {
        let pieces = spell_length(length, values);
        let last = pieces.len().saturating_sub(1);
        for (index, value) in pieces.into_iter().enumerate() {
            let sounds = !pitches.is_empty();
            notes.push(XmlNote {
                pitches: pitches.to_vec(),
                value,
                tie_stop: sounds && (tied_in || index > 0),
                tie_start: sounds && (tied_out || index < last),
                tuplet_start: false,
                tuplet_stop: false,
            });
        }
    };
    for (start, end, pitches) in timeline {
        let (start, end) = (*start, *end);
        if end <= measure_start + EPSILON || start >= measure_end - EPSILON {
            continue;
        }
        let from = start.max(measure_start);
        let to = end.min(measure_end);
        if from > cursor + EPSILON {
            push_span(&mut notes, &[], from - cursor, false, false);
        }
        push_span(
            &mut notes,
            pitches,
            to - from,
            start < measure_start - EPSILON,
            end > measure_end + EPSILON,
        );
        cursor = to;
    }
    if measure_end > cursor + EPSILON {
        push_span(&mut notes, &[], measure_end - cursor, false, false);
    }
    mark_tuplets(&mut notes);
    notes
}

/// Brackets each run of tuplet values, closing a bracket once it has filled
/// the span its ratio describes: three triplet eighths make one quarter.
fn mark_tuplets(notes: &mut [XmlNote]) {
    let mut open: Option<(usize, (u32, u32), f64, f64)> = None;
    for index in 0..notes.len() {
        let value = notes[index].value;
        if let Some((first, ratio, _, _)) = open
            && value.tuplet != Some(ratio)
        {
            notes[first].tuplet_start = true;
            notes[index - 1].tuplet_stop = true;
            open = None;
        }
        let Some(ratio) = value.tuplet else { continue };
        let (first, span, filled) = match open {
            Some((first, _, span, filled)) => (first, span, filled),
            None => {
                let base = value.quarters * ratio.0 as f64 / ratio.1 as f64;
                (index, base * ratio.1 as f64, 0.0)
            }
        };
        let filled = filled + value.quarters;
        if filled >= span - 1e-4 {
            notes[first].tuplet_start = true;
            notes[index].tuplet_stop = true;
            open = None;
        } else {
            open = Some((first, ratio, span, filled));
        }
    }
    if let Some((first, _, _, _)) = open {
        notes[first].tuplet_start = true;
        if let Some(last) = notes.last_mut() {
            last.tuplet_stop = true;
        }
    }
}

fn write_measure_notes(xml: &mut String, notes: &[XmlNote], sharps: i32) {
    let key = key_alters(sharps);
    let mut in_force: BTreeMap<(char, i32), i32> = BTreeMap::new();
    for note in notes {
        let duration = (note.value.quarters * DIVISIONS).round() as u32;
        let chord_members: Vec<Option<&Pitch>> = if note.pitches.is_empty() {
            vec![None]
        } else {
            note.pitches.iter().map(Some).collect()
        };
        for (member, pitch) in chord_members.into_iter().enumerate() {
            xml.push_str("      <note>");
            if member > 0 {
                xml.push_str("<chord/>");
            }
            let mut accidental = None;
            match pitch {
                None => xml.push_str("<rest/>"),
                Some(pitch) => {
                    let (step, alter, octave) = step_alter(pitch);
                    let current = in_force
                        .get(&(step, octave))
                        .copied()
                        .unwrap_or_else(|| key.get(&step).copied().unwrap_or(0));
                    if current != alter && !note.tie_stop {
                        accidental = Some(match alter {
                            -2 => "flat-flat",
                            -1 => "flat",
                            1 => "sharp",
                            2 => "double-sharp",
                            _ => "natural",
                        });
                    }
                    in_force.insert((step, octave), alter);
                    let _ = write!(xml, "<pitch><step>{step}</step>");
                    if alter != 0 {
                        let _ = write!(xml, "<alter>{alter}</alter>");
                    }
                    let _ = write!(xml, "<octave>{octave}</octave></pitch>");
                }
            }
            let _ = write!(xml, "<duration>{duration}</duration>");
            if note.tie_stop {
                xml.push_str("<tie type=\"stop\"/>");
            }
            if note.tie_start {
                xml.push_str("<tie type=\"start\"/>");
            }
            let _ = write!(xml, "<voice>1</voice><type>{}</type>", note.value.type_name);
            for _ in 0..note.value.dots {
                xml.push_str("<dot/>");
            }
            if let Some(accidental) = accidental {
                let _ = write!(xml, "<accidental>{accidental}</accidental>");
            }
            if let Some((actual, normal)) = note.value.tuplet {
                let _ = write!(
                    xml,
                    "<time-modification><actual-notes>{actual}</actual-notes><normal-notes>{normal}</normal-notes></time-modification>"
                );
            }
            let tuplet_marks = member == 0 && (note.tuplet_start || note.tuplet_stop);
            if note.tie_start || note.tie_stop || tuplet_marks {
                xml.push_str("<notations>");
                if note.tie_stop {
                    xml.push_str("<tied type=\"stop\"/>");
                }
                if note.tie_start {
                    xml.push_str("<tied type=\"start\"/>");
                }
                if member == 0 && note.tuplet_start {
                    xml.push_str("<tuplet type=\"start\" bracket=\"yes\"/>");
                }
                if member == 0 && note.tuplet_stop {
                    xml.push_str("<tuplet type=\"stop\"/>");
                }
                xml.push_str("</notations>");
            }
            xml.push_str("</note>\n");
        }
    }
}

// ------------------------------------------------------------ MIDI to ABC

/// Spells a MIDI number in a key: as the scale spells it when the key has
/// that pitch class, otherwise with sharps in a sharp key and flats in a
/// flat one.
fn spell_in_key(midi: i32, key: &Key, scale: &[Pitch]) -> Result<Pitch, JsValue> {
    let class = midi.rem_euclid(12);
    let name = match scale
        .iter()
        .find(|pitch| (pitch.ps().round() as i32).rem_euclid(12) == class)
    {
        Some(pitch) => pitch.name(),
        None if key.sharps() < 0 => [
            "C", "D-", "D", "E-", "E", "F", "G-", "G", "A-", "A", "B-", "B",
        ][class as usize]
            .to_string(),
        None => [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ][class as usize]
            .to_string(),
    };
    let step = name.chars().next().unwrap_or('C');
    let index = STEPS
        .iter()
        .position(|candidate| *candidate == step)
        .unwrap_or(0);
    let alter = name
        .chars()
        .skip(1)
        .fold(0, |alter, modifier| match modifier {
            '#' => alter + 1,
            '-' => alter - 1,
            _ => alter,
        });
    let octave = (midi - alter - STEP_SEMITONES[index]).div_euclid(12) - 1;
    Pitch::from_name(format!("{name}{octave}")).map_err(js_error)
}

fn abc_key_name(key: &Key) -> String {
    let tonic = key.tonic_pitch().name().replace('-', "b");
    if key.mode() == "minor" {
        format!("{tonic}m")
    } else {
        tonic
    }
}

/// Writes a pitch as ABC with the accidental the bar needs: none where the
/// key signature or an earlier accidental in the bar already gives it.
fn abc_pitch(
    pitch: &Pitch,
    key: &BTreeMap<char, i32>,
    in_force: &mut BTreeMap<(char, i32), i32>,
) -> Result<String, JsValue> {
    let (step, alter, octave) = step_alter(pitch);
    let natural = Pitch::from_name(format!("{step}{octave}")).map_err(js_error)?;
    let letter = abc_note(&natural).map_err(js_error)?;
    let current = in_force
        .get(&(step, octave))
        .copied()
        .unwrap_or_else(|| key.get(&step).copied().unwrap_or(0));
    in_force.insert((step, octave), alter);
    if current == alter {
        return Ok(letter);
    }
    let accidental = match alter {
        -2 => "__",
        -1 => "_",
        1 => "^",
        2 => "^^",
        _ => "=",
    };
    Ok(format!("{accidental}{letter}"))
}

fn midi_to_abc_text(bytes: &[u8]) -> Result<String, JsValue> {
    const SIXTEENTHS_PER_BAR: i64 = 16;
    let (notes, tempo) = read_midi_bytes_with_tempo(bytes).map_err(js_error)?;
    if notes.is_empty() {
        return Err(JsValue::from_str("the MIDI file has no notes"));
    }
    let pitches: Vec<Pitch> = notes
        .iter()
        .filter_map(|note| Pitch::from_midi(note.pitch as i32).ok())
        .collect();
    let key = estimate_key_from_pitches(&pitches)
        .map_err(js_error)?
        .first()
        .map(|estimate| estimate.key().clone())
        .ok_or_else(|| JsValue::from_str("no key could be estimated"))?;
    let scale = key.pitches().map_err(js_error)?;
    let key_signature = key_alters(key.sharps());

    let mut channels: BTreeMap<u8, Vec<MidiNote>> = BTreeMap::new();
    for note in notes {
        channels.entry(note.channel).or_default().push(note);
    }

    let mut abc = String::new();
    let _ = writeln!(abc, "X:1\nT:Imported MIDI\nM:4/4\nL:1/8");
    let _ = writeln!(abc, "Q:1/4={}", tempo.unwrap_or(120.0).round());
    let _ = writeln!(abc, "K:{}", abc_key_name(&key));

    for (voice_number, (_, mut channel_notes)) in channels.into_iter().enumerate() {
        // Quantized to sixteenths: (start, end, midi numbers).
        channel_notes.sort_by(|left, right| left.start.total_cmp(&right.start));
        let mut chords: Vec<(i64, i64, Vec<i32>)> = Vec::new();
        for note in &channel_notes {
            let start = (note.start * 4.0).round() as i64;
            let end = ((note.start + note.duration) * 4.0)
                .round()
                .max(start as f64 + 1.0) as i64;
            match chords.last_mut() {
                Some(last) if last.0 == start => {
                    last.1 = last.1.max(end);
                    last.2.push(note.pitch as i32);
                }
                _ => chords.push((start, end, vec![note.pitch as i32])),
            }
        }
        for index in 1..chords.len() {
            let next = chords[index].0;
            if chords[index - 1].1 > next {
                chords[index - 1].1 = next;
            }
        }
        let mean = channel_notes
            .iter()
            .map(|note| note.pitch as f64)
            .sum::<f64>()
            / channel_notes.len() as f64;
        let _ = writeln!(
            abc,
            "V:{} clef={}",
            voice_number + 1,
            if mean < 57.0 { "bass" } else { "treble" }
        );

        let mut line = String::new();
        let mut in_force = BTreeMap::new();
        let mut cursor = 0_i64;
        let mut bars_on_line = 0;
        let mut spans: Vec<(i64, i64, Vec<i32>)> = Vec::new();
        for (start, end, midi) in chords {
            if start > cursor {
                spans.push((cursor, start, Vec::new()));
            }
            spans.push((start, end, midi));
            cursor = end;
        }
        let total_bars = (cursor + SIXTEENTHS_PER_BAR - 1) / SIXTEENTHS_PER_BAR;
        if cursor < total_bars * SIXTEENTHS_PER_BAR {
            spans.push((cursor, total_bars * SIXTEENTHS_PER_BAR, Vec::new()));
        }

        for (start, end, midi) in spans {
            let mut from = start;
            while from < end {
                let bar_end = (from / SIXTEENTHS_PER_BAR + 1) * SIXTEENTHS_PER_BAR;
                let to = end.min(bar_end);
                let length = abc_duration((to - from) as u32, 2).map_err(js_error)?;
                if midi.is_empty() {
                    let _ = write!(line, "z{length} ");
                } else {
                    let mut spelled = midi
                        .iter()
                        .map(|&number| spell_in_key(number, &key, &scale))
                        .collect::<Result<Vec<_>, _>>()?;
                    spelled.sort_by(|left, right| left.ps().total_cmp(&right.ps()));
                    let written = spelled
                        .iter()
                        .map(|pitch| abc_pitch(pitch, &key_signature, &mut in_force))
                        .collect::<Result<Vec<_>, _>>()?;
                    let tie = if to < end { "-" } else { "" };
                    if written.len() == 1 {
                        let _ = write!(line, "{}{length}{tie} ", written[0]);
                    } else {
                        let _ = write!(line, "[{}]{length}{tie} ", written.concat());
                    }
                }
                from = to;
                if from == bar_end {
                    line.push_str("| ");
                    in_force.clear();
                    bars_on_line += 1;
                    if bars_on_line == 4 {
                        abc.push_str(line.trim_end());
                        abc.push('\n');
                        line.clear();
                        bars_on_line = 0;
                    }
                }
            }
        }
        if !line.trim().is_empty() {
            abc.push_str(line.trim_end());
            abc.push('\n');
        }
    }
    Ok(abc)
}

// ------------------------------------------------------------- bindings

fn score_from(input: JsValue) -> Result<Score, JsValue> {
    let input: ScoreInput = serde_wasm_bindgen::from_value(input).map_err(js_error)?;
    Score::from_input(input)
}

#[wasm_bindgen]
/// Analyses a score: key, the harmony at every onset, and voice-leading and
/// melodic faults, each tied back to the text ranges of its notes.
pub fn analyze_score(input: JsValue) -> Result<JsValue, JsValue> {
    let score = score_from(input)?;
    let analysis = analyse(&score)?;
    serde_wasm_bindgen::to_value(&analysis).map_err(js_error)
}

#[wasm_bindgen]
/// Writes a score as a Standard MIDI File, one channel per voice.
pub fn score_to_midi(input: JsValue) -> Result<Vec<u8>, JsValue> {
    midi_bytes(&score_from(input)?)
}

#[wasm_bindgen]
/// Writes a score as MusicXML 4.0, one part per voice.
pub fn score_to_musicxml(input: JsValue) -> Result<String, JsValue> {
    Ok(musicxml(&score_from(input)?))
}

#[wasm_bindgen]
/// Reads a Standard MIDI File into ABC, one voice per channel, quantized to
/// sixteenths and spelled in the key the notes suggest.
pub fn midi_to_abc(bytes: &[u8]) -> Result<String, JsValue> {
    midi_to_abc_text(bytes)
}

// ----------------------------------------------------------------- tuning

/// Cents from equal temperament for a note `semitones` above the tuning's
/// root. A twelve-tone table is read at that degree; any other division is
/// read at the degree nearest in pitch, since the notes are twelve-tone.
fn fixed_cents(system: TuningSystem, semitones: usize) -> f64 {
    let target = semitones as f64 * 100.0;
    let size = system.degrees_per_period() as usize;
    if size == 12 {
        return 1200.0 * system.ratio(semitones).log2() - target;
    }
    (0..=size)
        .map(|degree| 1200.0 * system.ratio(degree).log2() - target)
        .min_by(|left, right| left.abs().total_cmp(&right.abs()))
        .unwrap_or(0.0)
}

#[derive(Serialize)]
struct TuningCents {
    /// Cents by pitch class, for a tuning that does not depend on context.
    pitch_class_cents: Option<[f64; 12]>,
    /// Cents for every pitch of the input, voice by note by pitch, in the
    /// order they were handed over.
    notes: Vec<Vec<Vec<f64>>>,
}

/// The pitch class of each onset's harmonic root, keyed by grid position.
fn roots_by_onset(score: &Score) -> BTreeMap<i64, i32> {
    let mut onsets: Vec<f64> = score
        .voices
        .iter()
        .flat_map(|voice| voice.events.iter().map(|event| event.start))
        .collect();
    onsets.sort_by(f64::total_cmp);
    onsets.dedup_by(|left, right| (*left - *right).abs() < EPSILON);
    let mut roots = BTreeMap::new();
    for offset in onsets {
        let pitches: Vec<Pitch> = score
            .voices
            .iter()
            .flat_map(|voice| voice.events.iter())
            .filter(|event| event.start <= offset + EPSILON && event.end > offset + EPSILON)
            .flat_map(|event| event.pitches.iter().cloned())
            .collect();
        if let Some(root) = Chord::new(pitches)
            .ok()
            .and_then(|chord| chord.root().map(|root| i32::from(pitch_class(root))))
        {
            roots.insert((offset * GRID).round() as i64, root);
        }
    }
    roots
}

fn tuning_cents(input: ScoreInput, tuning: &str, tonic: &str) -> Result<TuningCents, JsValue> {
    let tonic = Pitch::from_name(tonic.trim())
        .map(|pitch| i32::from(pitch_class(&pitch)))
        .unwrap_or(0);
    let class_above = |midi: i32, root: i32| (midi - root).rem_euclid(12);
    if tuning == "RecursiveJustIntonation" {
        use music21_rs::tuningsystem::adaptive::RECURSIVE_JI;
        let roots = roots_by_onset(&Score::from_input(input.clone())?);
        let notes = input
            .voices
            .iter()
            .map(|voice| {
                voice
                    .notes
                    .iter()
                    .map(|note| {
                        let key = (snap(note.start.max(0.0)) * GRID).round() as i64;
                        note.pitches
                            .iter()
                            .map(|pitch| {
                                let root = roots
                                    .get(&key)
                                    .copied()
                                    .unwrap_or(pitch.midi.rem_euclid(12));
                                RECURSIVE_JI.cents_at(
                                    f64::from(class_above(root, tonic)),
                                    f64::from(class_above(pitch.midi, root)),
                                )
                            })
                            .collect()
                    })
                    .collect()
            })
            .collect();
        return Ok(TuningCents {
            pitch_class_cents: None,
            notes,
        });
    }
    let system = crate::all_playable()
        .find(|system| crate::tuning_is_named(*system, tuning))
        .ok_or_else(|| JsValue::from_str(&format!("no tuning system called {tuning:?}")))?;
    let mut by_class = [0.0; 12];
    for (class, cents) in by_class.iter_mut().enumerate() {
        *cents = fixed_cents(system, class_above(class as i32, tonic) as usize);
    }
    let notes = input
        .voices
        .iter()
        .map(|voice| {
            voice
                .notes
                .iter()
                .map(|note| {
                    note.pitches
                        .iter()
                        .map(|pitch| by_class[pitch.midi.rem_euclid(12) as usize])
                        .collect()
                })
                .collect()
        })
        .collect();
    Ok(TuningCents {
        pitch_class_cents: Some(by_class),
        notes,
    })
}

#[wasm_bindgen]
/// How far each note of a score sounds from equal temperament, in cents, in
/// a tuning system rooted on `tonic`. `RecursiveJustIntonation` tunes each
/// note against the root of the harmony sounding when it starts; every other
/// id is one of the fixed tuning systems.
pub fn score_tuning_cents(input: JsValue, tuning: &str, tonic: &str) -> Result<JsValue, JsValue> {
    let input: ScoreInput = serde_wasm_bindgen::from_value(input).map_err(js_error)?;
    serde_wasm_bindgen::to_value(&tuning_cents(input, tuning, tonic)?).map_err(js_error)
}

// --------------------------------------------------------- chord symbols

/// The notes a fretted instrument plays for a chord symbol: the crate's
/// suggested fingering on the given open strings (MIDI numbers, lowest
/// first), and the symbol's own chord where no fingering covers it.
fn chord_symbol_voicing_notes(figure: &str, open_strings: &[i32]) -> Result<Vec<i32>, JsValue> {
    let chord = music21_rs::ChordSymbol::parse(figure)
        .and_then(|symbol| symbol.to_chord())
        .map_err(js_error)?;
    voicing_notes(&chord, open_strings)
}

/// The notes the fingers play for a chord symbol on the strings above a
/// thumb that plays the pitch classes in `thumb`, the bass first: every note
/// of the chord but the thumb's bass, and not the thumb's other notes either
/// when there are more notes left than strings to play them on.
fn chord_symbol_voicing_above_notes(
    figure: &str,
    open_strings: &[i32],
    thumb: &[i32],
) -> Result<Vec<i32>, JsValue> {
    let chord = music21_rs::ChordSymbol::parse(figure)
        .and_then(|symbol| symbol.to_chord())
        .map_err(js_error)?;
    let pitches = chord.pitches();
    let class = |pitch: &Pitch| pitch.midi().rem_euclid(12);
    let classes = pitches
        .iter()
        .map(class)
        .collect::<std::collections::BTreeSet<_>>();
    let mut left_out = thumb
        .iter()
        .take(1)
        .map(|pc| pc.rem_euclid(12))
        .collect::<Vec<_>>();
    if classes.iter().filter(|pc| !left_out.contains(pc)).count() > open_strings.len() {
        left_out.extend(thumb.iter().skip(1).map(|pc| pc.rem_euclid(12)));
    }
    let kept = pitches
        .into_iter()
        .filter(|pitch| !left_out.contains(&class(pitch)))
        .collect::<Vec<_>>();
    if kept.is_empty() || open_strings.is_empty() {
        return voicing_notes(&chord, open_strings);
    }
    let upper = music21_rs::Chord::new(kept.as_slice()).map_err(js_error)?;
    voicing_notes(&upper, open_strings)
}

/// The crate's suggested fingering of a chord on the given open strings, as
/// the notes it sounds, or the chord's own notes where no fingering covers it.
fn voicing_notes(chord: &music21_rs::Chord, open_strings: &[i32]) -> Result<Vec<i32>, JsValue> {
    let names = open_strings
        .iter()
        .map(|&midi| Pitch::from_midi(midi).map(|pitch| pitch.name_with_octave()))
        .collect::<music21_rs::Result<Vec<_>>>()
        .map_err(js_error)?;
    let fingering = music21_rs::GuitarTuning::new(names)
        .ok()
        .and_then(|tuning| chord.guitar_fingering_with_tuning(&tuning));
    let mut notes: Vec<i32> = match fingering {
        Some(fingering) => fingering.covered_pitch_spaces,
        None => chord.pitches().iter().map(Pitch::midi).collect(),
    };
    notes.sort_unstable();
    notes.dedup();
    Ok(notes)
}

#[wasm_bindgen]
/// Voices a chord symbol (`G`, `D7`, `F#m7b5`) for a fretted instrument
/// whose open strings are the given MIDI numbers, lowest first, as the MIDI
/// numbers to play.
pub fn chord_symbol_voicing(figure: &str, open_strings: Vec<i32>) -> Result<Vec<i32>, JsValue> {
    chord_symbol_voicing_notes(figure, &open_strings)
}

#[wasm_bindgen]
/// Voices a chord symbol for the fingers on the strings above a thumb that
/// plays the pitch classes `thumb`, its bass first: the chord without the
/// thumb's bass, and without its other notes as well when more notes are left
/// than there are strings.
pub fn chord_symbol_voicing_above(
    figure: &str,
    open_strings: Vec<i32>,
    thumb: Vec<i32>,
) -> Result<Vec<i32>, JsValue> {
    chord_symbol_voicing_above_notes(figure, &open_strings, &thumb)
}

fn spell_midi(midi: i32, tonic: &str, mode: &str) -> Result<Pitch, JsValue> {
    let key = Key::from_tonic_mode(tonic, mode)
        .or_else(|_| Key::from_tonic_mode("C", "major"))
        .map_err(js_error)?;
    let scale = key.pitches().map_err(js_error)?;
    spell_in_key(midi, &key, &scale)
}

#[wasm_bindgen]
/// Spells a MIDI number in a key, as a name with octave (`F#4`): as the scale
/// spells it when the key has that pitch class, otherwise with sharps in a
/// sharp key and flats in a flat one. A key the crate cannot read is C major.
pub fn spell_midi_in_key(midi: i32, tonic: &str, mode: &str) -> Result<String, JsValue> {
    Ok(spell_midi(midi, tonic, mode)?.name_with_octave())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One voice per list, each note a quarter long at the start given.
    type Notes<'a> = &'a [(f64, &'a [(i32, i32)])];

    fn input_of(voices: &[Notes<'_>]) -> ScoreInput {
        ScoreInput {
            title: String::new(),
            tempo_bpm: 100.0,
            key: None,
            meter: Some([4, 4]),
            pickup: 0.0,
            let_ring: false,
            voices: voices
                .iter()
                .map(|notes| VoiceInput {
                    name: String::new(),
                    clef: String::new(),
                    notes: notes
                        .iter()
                        .map(|(start, pitches)| note(*start, 1.0, pitches))
                        .collect(),
                })
                .collect(),
        }
    }

    #[test]
    fn a_fixed_tuning_gives_each_pitch_class_its_cents() {
        let input = input_of(&[&[(0.0, &[(60, 0), (64, 2), (67, 4)])]]);
        let equal = tuning_cents(input.clone(), "EqualTemperament", "C").expect("tunes");
        assert!(equal.notes[0][0].iter().all(|cents| cents.abs() < 1e-9));
        let just = tuning_cents(input.clone(), "FiveLimit", "C").expect("tunes");
        assert!(
            (just.notes[0][0][1] - -13.686).abs() < 0.01,
            "{:?}",
            just.notes
        );
        assert!(
            (just.notes[0][0][2] - 1.955).abs() < 0.01,
            "{:?}",
            just.notes
        );
        // Rooted on E, E itself is the untempered root.
        let on_e = tuning_cents(input, "FiveLimit", "E").expect("tunes");
        assert!(on_e.notes[0][0][1].abs() < 1e-9);
        assert!(on_e.pitch_class_cents.is_some());
    }

    #[test]
    fn adaptive_just_intonation_tunes_against_the_chord_root() {
        // E G B over C: the E-minor chord's root E sits a just major third
        // above C, and its G a just minor third above that.
        let input = input_of(&[&[(0.0, &[(64, 2), (67, 4), (71, 6)])]]);
        let cents = tuning_cents(input, "RecursiveJustIntonation", "C").expect("tunes");
        let e = cents.notes[0][0][0];
        let g = cents.notes[0][0][1];
        assert!((e - -13.686).abs() < 0.01, "{e}");
        assert!((g - (-13.686 + 15.641)).abs() < 0.01, "{g}");
        assert!(cents.pitch_class_cents.is_none());
    }

    #[test]
    fn a_chord_symbol_is_voiced_on_the_strings() {
        let guitar = [40, 45, 50, 55, 59, 64];
        let g = chord_symbol_voicing_notes("G", &guitar).expect("voices G");
        assert!(!g.is_empty());
        assert!(
            g.iter()
                .all(|midi| [7, 11, 2].contains(&midi.rem_euclid(12))),
            "{g:?}"
        );
        assert!(g.iter().all(|midi| *midi >= 40), "{g:?}");
        let d7 = chord_symbol_voicing_notes("D7", &guitar).expect("voices D7");
        let classes: std::collections::BTreeSet<i32> =
            d7.iter().map(|midi| midi.rem_euclid(12)).collect();
        assert!(classes.contains(&0), "D7 has its C: {d7:?}");
    }

    #[test]
    fn the_fingers_play_what_the_thumb_does_not() {
        let classes = |notes: &[i32]| {
            notes
                .iter()
                .map(|midi| midi.rem_euclid(12))
                .collect::<std::collections::BTreeSet<i32>>()
        };
        // Gm7 over a thumb on G and D: its third and seventh on the top strings.
        let gm7 =
            chord_symbol_voicing_above_notes("Gm7", &[55, 59, 64], &[7, 2]).expect("voices Gm7");
        assert_eq!(classes(&gm7), [10, 2, 5].into(), "{gm7:?}");
        assert!(gm7.iter().all(|midi| *midi >= 55), "{gm7:?}");
        // C9 over a thumb on C and G, on four strings: E, B-flat and D, and
        // no C.
        let c9 =
            chord_symbol_voicing_above_notes("C9", &[50, 55, 59, 64], &[0, 7]).expect("voices C9");
        for pc in [4, 10, 2] {
            assert!(classes(&c9).contains(&pc), "{c9:?}");
        }
        assert!(!classes(&c9).contains(&0), "{c9:?}");
    }

    #[test]
    fn a_midi_number_is_spelled_in_the_key() {
        let name = |midi, tonic, mode| {
            spell_midi(midi, tonic, mode)
                .expect("spells")
                .name_with_octave()
        };
        assert_eq!(name(66, "D", "major"), "F#4");
        assert_eq!(name(63, "D", "major"), "D#4");
        assert_eq!(name(63, "B-", "major"), "E-4");
        assert_eq!(name(60, "B", "major"), "C4");
        assert_eq!(name(60, "C#", "major"), "B#3");
        assert_eq!(name(61, "nonsense", "major"), "C#4");
    }

    fn note(start: f64, duration: f64, midi: &[(i32, i32)]) -> NoteInput {
        NoteInput {
            start,
            duration,
            velocity: 90,
            pitches: midi
                .iter()
                .map(|&(midi, staff)| PitchInput {
                    midi,
                    staff: Some(staff),
                })
                .collect(),
            char_start: Some((start * 10.0) as usize),
            char_end: Some((start * 10.0) as usize + 2),
        }
    }

    /// Voices of two notes each, a quarter apart, as `(midi, staff)` pairs.
    /// A written pitch: its MIDI number and staff position.
    type Written = (i32, i32);

    fn two_chords(voices: &[(Written, Written)]) -> Score {
        Score::from_input(ScoreInput {
            title: "Test".to_string(),
            tempo_bpm: 90.0,
            key: Some(WrittenKey {
                tonic: "C".to_string(),
                mode: "major".to_string(),
                sharps: 0,
            }),
            meter: Some([4, 4]),
            pickup: 0.0,
            let_ring: false,
            voices: voices
                .iter()
                .enumerate()
                .map(|(index, &(first, second))| VoiceInput {
                    name: format!("V{index}"),
                    clef: String::new(),
                    notes: vec![note(0.0, 1.0, &[first]), note(1.0, 1.0, &[second])],
                })
                .collect(),
        })
        .expect("the test score builds")
    }

    /// C major, I to ii in four voices moving in parallel: soprano and bass
    /// a twelfth apart (parallel fifths), tenor and bass an octave apart.
    fn chorale() -> Score {
        two_chords(&[
            ((67, 4), (69, 5)),
            ((64, 2), (65, 3)),
            ((60, 0), (62, 1)),
            ((48, -7), (50, -6)),
        ])
    }

    #[test]
    fn a_pitch_is_spelled_on_the_staff_position_it_was_written_at() {
        let flat = spell(&PitchInput {
            midi: 70,
            staff: Some(6),
        })
        .expect("B-flat spells");
        assert_eq!(flat.name_with_octave(), "B-4");
        let sharp = spell(&PitchInput {
            midi: 70,
            staff: Some(5),
        })
        .expect("A-sharp spells");
        assert_eq!(sharp.name_with_octave(), "A#4");
        let low = spell(&PitchInput {
            midi: 43,
            staff: Some(-10),
        })
        .expect("G2 spells");
        assert_eq!(low.name_with_octave(), "G2");
        // A treble-8 staff: written A3, sounding A2.
        let guitar = spell(&PitchInput {
            midi: 45,
            staff: Some(-2),
        })
        .expect("A2 spells");
        assert_eq!(guitar.name_with_octave(), "A2");
        let guitar_sharp = spell(&PitchInput {
            midi: 51,
            staff: Some(1),
        })
        .expect("D#3 spells");
        assert_eq!(guitar_sharp.name_with_octave(), "D#3");
    }

    #[test]
    fn parallel_fifths_and_octaves_are_found() {
        let analysis = analyse(&chorale()).expect("analysis runs");
        let kinds: Vec<&str> = analysis.issues.iter().map(|issue| issue.kind).collect();
        assert!(kinds.contains(&"Parallel octaves"), "{kinds:?}");
        assert!(kinds.contains(&"Parallel fifths"), "{kinds:?}");
        assert_eq!(analysis.analysis_key.as_deref(), Some("C major"));
        assert_eq!(analysis.slices[0].numeral.as_deref(), Some("I"));
        assert_eq!(analysis.slices[1].numeral.as_deref(), Some("ii"));
        assert!(!kinds.contains(&"Fifths by contrary motion"), "{kinds:?}");
    }

    #[test]
    fn a_textbook_numeral_leaves_minors_natural_degrees_unmarked() {
        let cases = [
            ("b", "minor", "A2 E3 G3 C#4", "bVII7", "VII7"),
            ("b", "minor", "F2 D3 G#3 B3", "vio64#2", "vio42"),
            ("b", "minor", "F#2 D3 A3 B3", "i43", "i43"),
            ("b", "minor", "E2 D3 G3 B3", "iv7", "iv7"),
            ("c", "minor", "A-3 C4 E-4", "bVI", "VI"),
            ("c", "minor", "B3 D4 F4", "viio", "viio"),
            ("C", "major", "E3 G3 C4", "I6", "I6"),
            ("C", "major", "B-3 D4 F4", "bVII", "bVII"),
        ];
        for (tonic, mode, notes, music21, textbook) in cases {
            let key = Key::from_tonic_mode(tonic, mode).expect("key");
            let chord = Chord::new(notes).expect("chord");
            let numeral = roman_numeral_from_chord(&chord, Some(&key))
                .expect("analysed")
                .expect("a numeral");
            assert_eq!(numeral.figure(), music21, "{notes} in {tonic} {mode}");
            assert_eq!(
                textbook_figure(&numeral, &chord).as_deref(),
                Some(textbook),
                "{notes} in {tonic} {mode}"
            );
        }
    }

    #[test]
    fn a_fifth_opening_to_a_twelfth_is_not_called_parallel() {
        // C5 over F4 is a fifth; D5 over G3, reached in contrary motion, a
        // twelfth. music21 calls that a parallel fifth.
        let analysis = analyse(&two_chords(&[((72, 7), (74, 8)), ((65, 3), (55, -3))]))
            .expect("analysis runs");
        let kinds: Vec<&str> = analysis.issues.iter().map(|issue| issue.kind).collect();
        assert_eq!(kinds, vec!["Fifths by contrary motion"]);
    }

    #[test]
    fn measures_and_beats_count_a_pickup_as_measure_nought() {
        let mut score = chorale();
        score.pickup = 1.0;
        assert_eq!(score.position(0.0), (0, 4.0));
        assert_eq!(score.position(1.0), (1, 1.0));
        assert_eq!(score.position(3.5), (1, 3.5));
        assert_eq!(score.position(5.0), (2, 1.0));
        score.pickup = 0.0;
        assert_eq!(score.position(12.0), (4, 1.0));
        assert_eq!(score.position(16.0 - 1.0 / GRID).0, 4);
    }

    #[test]
    fn midi_round_trips_through_the_crate_reader() {
        let bytes = midi_bytes(&chorale()).expect("MIDI writes");
        let (notes, tempo) = read_midi_bytes_with_tempo(&bytes).expect("MIDI reads");
        assert_eq!(notes.len(), 8);
        assert_eq!(tempo.map(f64::round), Some(90.0));
    }

    #[test]
    fn musicxml_ties_a_note_across_the_barline() {
        let mut score = chorale();
        score.voices[3].events[1].end = 6.0;
        let xml = musicxml(&score);
        assert!(xml.contains("<measure number=\"2\">"));
        assert!(xml.contains("<tie type=\"start\"/>"));
        assert!(xml.contains("<tie type=\"stop\"/>"));
        assert_eq!(xml.matches("<part id=").count(), 4);
    }

    #[test]
    fn a_bass_note_rings_under_the_chord_struck_over_it() {
        // A guitar's thumb and fingers in one voice: F#2, then D3 A3 B3 an
        // eighth later. The bass is still sounding under the chord.
        let mut input = input_of(&[&[(0.0, &[(42, -4)]), (0.5, &[(50, 1), (57, 5), (59, 6)])]]);
        input.let_ring = true;
        input.voices[0].notes[0].duration = 0.5;
        let score = Score::from_input(input).expect("builds");
        assert_eq!(
            score.voices[0].events[0].end, 4.0,
            "the bass rings to the end of the bar"
        );
        let analysis = analyse(&score).expect("analyses");
        assert_eq!(analysis.slices[1].pitches, ["F#2", "D3", "A3", "B3"]);
        assert_eq!(
            analysis.slices[1].pitched_common_name,
            "B-minor seventh chord"
        );
    }

    #[test]
    fn a_line_of_single_notes_does_not_ring() {
        // A chorale bass leaping a fifth: the first note stops at the second.
        let mut input = input_of(&[&[(0.0, &[(48, -7)]), (1.0, &[(41, -11)])]]);
        input.let_ring = true;
        let score = Score::from_input(input).expect("builds");
        assert_eq!(score.voices[0].events[0].end, 1.0);
    }

    #[test]
    fn nothing_rings_past_its_own_bar() {
        let mut input = input_of(&[&[(3.5, &[(42, -4)]), (5.0, &[(50, 1), (57, 5), (59, 6)])]]);
        input.let_ring = true;
        input.voices[0].notes[0].duration = 0.5;
        let score = Score::from_input(input).expect("builds");
        assert_eq!(score.voices[0].events[0].end, 4.0);
    }

    #[test]
    fn a_chromatic_half_step_is_not_an_augmented_leap() {
        // E F F# G#: an augmented unison, then a major second.
        let input = input_of(&[&[
            (0.0, &[(52, -5)]),
            (1.0, &[(53, -4)]),
            (2.0, &[(54, -4)]),
            (3.0, &[(56, -3)]),
        ]]);
        let analysis = analyse(&Score::from_input(input).expect("builds")).expect("analyses");
        assert!(
            analysis.issues.is_empty(),
            "{:?}",
            analysis
                .issues
                .iter()
                .map(|issue| issue.kind)
                .collect::<Vec<_>>()
        );
        let leap = input_of(&[&[(0.0, &[(53, -4)]), (1.0, &[(56, -3)])]]);
        let analysis = analyse(&Score::from_input(leap).expect("builds")).expect("analyses");
        assert_eq!(analysis.issues.len(), 1, "F to G# is an augmented second");
    }

    #[test]
    fn a_pickup_is_balanced_by_a_short_last_bar() {
        let mut score = chorale();
        score.pickup = 1.0;
        assert_eq!(score.barlines(), vec![0.0, 1.0, 2.0]);
        let xml = musicxml(&score);
        assert!(xml.contains("<measure number=\"0\" implicit=\"yes\">"));
        assert!(!xml.contains("<rest/>"), "{xml}");
    }

    #[test]
    fn lengths_are_spelled_as_plain_values_before_tuplets() {
        let values = notatable();
        let names = |length| {
            spell_length(length, &values)
                .iter()
                .map(|value| (value.type_name, value.dots, value.tuplet))
                .collect::<Vec<_>>()
        };
        assert_eq!(names(1.5), vec![("quarter", 1, None)]);
        assert_eq!(names(2.5), vec![("half", 0, None), ("eighth", 0, None)]);
        assert_eq!(names(1.0 / 3.0), vec![("eighth", 0, Some((3, 2)))]);
    }

    #[test]
    fn triplets_are_bracketed_in_threes() {
        let values = notatable();
        let timeline: Vec<(f64, f64, Vec<Pitch>)> = (0..3)
            .map(|index| {
                let start = index as f64 / 3.0;
                (
                    start,
                    start + 1.0 / 3.0,
                    vec![Pitch::from_name("C4").expect("C4")],
                )
            })
            .collect();
        let notes = measure_notes(&timeline, 0.0, 1.0, &values);
        assert_eq!(notes.len(), 3);
        assert!(notes[0].tuplet_start && notes[2].tuplet_stop);
        assert!(!notes[1].tuplet_start && !notes[1].tuplet_stop);
    }

    #[test]
    fn midi_is_read_back_as_abc_in_the_key_it_suggests() {
        let notes: Vec<MidiNote> = [67, 71, 74, 66, 67]
            .iter()
            .enumerate()
            .map(|(index, &pitch)| MidiNote::new(pitch, index as f64, 1.0, 90).expect("valid note"))
            .collect();
        let bytes = write_midi_bytes(&notes, 100.0).expect("MIDI writes");
        let abc = midi_to_abc_text(&bytes).expect("ABC writes");
        assert!(abc.contains("K:G"), "{abc}");
        assert!(abc.contains("Q:1/4=100"), "{abc}");
        // F# is in the key signature, so it needs no accidental.
        assert!(abc.contains("F2"), "{abc}");
        assert!(!abc.contains("^F"), "{abc}");
    }
}
