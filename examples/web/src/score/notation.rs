//! The pitch arithmetic behind the score editor's text edits: staff
//! positions, ABC pitch tokens, fretted strings and the generated chord
//! part. The page tokenizes the ABC text and splices it; every pitch it
//! reads or writes is decided here, with the crate's `Pitch`.
//!
//! A staff position counts diatonic steps from middle C:
//!
//! ```text
//!   position  -7  -1   0   1   2 ...  7
//!   pitch     C3  B3  C4  D4  E4 ...  C5
//!   ABC       C,  B,  C   D   E  ...  c
//! ```

use super::{
    EPSILON, MIDDLE_C_DNN, Score, ScoreInput, WrittenKey, abc_pitch, chord_symbol_voicing_notes,
    js_error, key_alters, natural_at, spell_midi,
};
use music21_rs::{Pitch, abc_duration, abc_note, pitch_name_from_abc_note};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use wasm_bindgen::prelude::*;

const OCTAVE_STEPS: i32 = 7;
const OCTAVE_SEMITONES: i32 = 12;
/// The highest fret counted as within an instrument's reach when deciding
/// whether a staff gets tablature at all.
const HIGHEST_FRET: i32 = 15;
/// Chord part lengths are written against `L:1/8`.
const EIGHTHS_PER_QUARTER: f64 = 2.0;
/// A length no number of eighths holds is written in twelfths of one.
const TWELFTHS: u32 = 12;

/// Whole octaves a written note sounds away from its staff position.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Octaves {
    Written,
    Down,
}

impl Octaves {
    /// A clef ending `-8` (`treble-8`, a guitar's) sounds an octave below
    /// where it is written.
    fn of_clef(clef: &str) -> Self {
        if clef.ends_with("-8") {
            Self::Down
        } else {
            Self::Written
        }
    }

    fn steps(self) -> i32 {
        match self {
            Self::Written => 0,
            Self::Down => -OCTAVE_STEPS,
        }
    }
}

fn staff_of(pitch: &Pitch) -> i32 {
    pitch.diatonic_note_number() - MIDDLE_C_DNN
}

/// Reads an ABC pitch token (`^F,`, `c'`), accidental and all.
fn parse_abc(token: &str) -> Result<Pitch, JsValue> {
    let name = pitch_name_from_abc_note(token)
        .map_err(js_error)?
        .ok_or_else(|| js_error(format!("{token:?} is a rest, not a pitch")))?;
    Pitch::from_name(name).map_err(js_error)
}

/// The accidental written in front of an ABC pitch token: `^`, `__`, `=`.
fn accidental_of(token: &str) -> &str {
    let letter = token
        .find(|ch: char| ch.is_ascii_alphabetic())
        .unwrap_or(token.len());
    &token[..letter]
}

/// Semitones between `midi` and the natural at `staff`, whole octaves
/// taken out: `-1` where a flat makes the staff position sound it.
fn alter_at(staff: i32, midi: i32) -> Result<i32, JsValue> {
    let offset = midi - natural_at(staff)?.midi();
    Ok(offset - OCTAVE_SEMITONES * octaves_in(offset))
}

fn octaves_in(semitones: i32) -> i32 {
    (semitones as f64 / OCTAVE_SEMITONES as f64).round() as i32
}

fn abc_accidental_text(alter: i32) -> Option<&'static str> {
    match alter {
        -2 => Some("__"),
        -1 => Some("_"),
        0 => Some("="),
        1 => Some("^"),
        2 => Some("^^"),
        _ => None,
    }
}

fn shift(token: &str, steps: i32) -> Result<String, JsValue> {
    let staff = staff_of(&parse_abc(token)?) + steps;
    let letter = abc_note(&natural_at(staff)?).map_err(js_error)?;

    // A step lets the note follow the key; an octave keeps its accidental.
    let accidental = if steps % OCTAVE_STEPS == 0 {
        accidental_of(token)
    } else {
        ""
    };
    Ok(format!("{accidental}{letter}"))
}

/// A string tuning as written for a staff with `clef`: an octave down for a
/// clef that sounds an octave below where it is written.
fn tuning_for(strings: &[String], clef: &str) -> Result<Vec<String>, JsValue> {
    let steps = Octaves::of_clef(clef).steps();
    strings.iter().map(|note| shift(note, steps)).collect()
}

fn midis(strings: &[String]) -> Result<Vec<i32>, JsValue> {
    strings
        .iter()
        .map(|note| parse_abc(note).map(|pitch| pitch.midi()))
        .collect()
}

fn to_js<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(value).map_err(js_error)
}

#[wasm_bindgen]
/// The MIDI number of the natural at a staff position: `0` is 60.
pub fn staff_midi(staff: i32) -> Result<i32, JsValue> {
    Ok(natural_at(staff)?.midi())
}

#[wasm_bindgen]
/// The staff position of an ABC pitch token, its accidental aside: `c` is 7.
pub fn abc_staff(token: &str) -> Result<i32, JsValue> {
    Ok(staff_of(&parse_abc(token)?))
}

#[wasm_bindgen]
/// The MIDI number an ABC pitch token sounds by its own accidental alone.
pub fn abc_midi(token: &str) -> Result<i32, JsValue> {
    Ok(parse_abc(token)?.midi())
}

#[wasm_bindgen]
/// Moves an ABC pitch token `steps` staff positions. A step drops the
/// accidental so the note follows the key; an octave keeps it.
pub fn abc_shift(token: &str, steps: i32) -> Result<String, JsValue> {
    shift(token, steps)
}

#[wasm_bindgen]
/// The ABC accidental that makes a staff position sound `midi`, whole
/// octaves aside, or `undefined` when more than a double one would be needed.
pub fn abc_accidental(staff: i32, midi: i32) -> Result<Option<String>, JsValue> {
    Ok(abc_accidental_text(alter_at(staff, midi)?).map(str::to_string))
}

#[wasm_bindgen]
/// Whole octaves `midi` sounds from the natural at a staff position: `-1`
/// on a staff that sounds an octave below where it is written.
pub fn written_octaves(staff: i32, midi: i32) -> Result<i32, JsValue> {
    Ok(octaves_in(midi - natural_at(staff)?.midi()))
}

#[wasm_bindgen]
/// Spells `midi` in the written key (`{ tonic, mode, sharps }`, or null for
/// C major) as ABC, with an accidental only where the signature differs.
pub fn abc_in_key(midi: i32, key: JsValue) -> Result<String, JsValue> {
    let key: Option<WrittenKey> = serde_wasm_bindgen::from_value(key).map_err(js_error)?;
    let (tonic, mode, sharps) = match &key {
        Some(key) => (key.tonic.as_str(), key.mode.as_str(), key.sharps),
        None => ("C", "major", 0),
    };
    let pitch = spell_midi(midi, tonic, mode)?;
    abc_pitch(&pitch, &key_alters(sharps), &mut BTreeMap::new())
}

#[derive(Serialize)]
struct KeyName {
    tonic: String,
    mode: &'static str,
}

#[wasm_bindgen]
/// A key signature as abcjs reads it (`root` `B`, `acc` `b`, `mode` `Dor`)
/// as the crate names it, `{ tonic: "B-", mode: "dorian" }`, or null when
/// it has no tonic (`K:none`).
pub fn abc_key(root: &str, acc: &str, mode: &str) -> Result<JsValue, JsValue> {
    if !matches!(root, "A" | "B" | "C" | "D" | "E" | "F" | "G") {
        return Ok(JsValue::NULL);
    }
    let accidental = match acc {
        "#" => "#",
        "b" => "-",
        _ => "",
    };
    let abbreviation: String = mode.to_lowercase().chars().take(3).collect();
    let mode = match abbreviation.as_str() {
        "m" | "min" | "aeo" => "minor",
        "dor" => "dorian",
        "phr" => "phrygian",
        "lyd" => "lydian",
        "mix" => "mixolydian",
        "loc" => "locrian",
        _ => "major",
    };
    to_js(&KeyName {
        tonic: format!("{root}{accidental}"),
        mode,
    })
}

// ------------------------------------------------------------ tablature

#[wasm_bindgen]
/// The open strings (ABC, lowest first) as MIDI numbers for a staff with
/// `clef`: an octave down where the clef sounds an octave down.
pub fn tab_strings(strings: Vec<String>, clef: &str) -> Result<Vec<i32>, JsValue> {
    midis(&tuning_for(&strings, clef)?)
}

/// The tuning a staff is tabbed against, given every staff position written
/// on it, or `None` when no tuning reaches them. Guitar music is written an
/// octave above where it sounds and is tabbed that way first; a staff
/// written at the pitch it sounds, as piano or choir music is, falls below
/// that and is tabbed against the strings as they really sound. abcjs
/// fingers the sounding pitch, so a clef that transposes is taken out first.
fn staff_tuning(
    strings: &[String],
    clef: &str,
    positions: &[i32],
) -> Result<Option<Vec<String>>, JsValue> {
    let shift = match clef {
        clef if clef.ends_with("-8") => -OCTAVE_SEMITONES,
        clef if clef.ends_with("+8") => OCTAVE_SEMITONES,
        _ => 0,
    };
    let (Some(&low), Some(&high)) = (positions.iter().min(), positions.iter().max()) else {
        return tuning_for(strings, clef).map(Some);
    };
    let low = natural_at(low)?.midi() + shift;
    let high = natural_at(high)?.midi() + shift;

    let written = tuning_for(strings, clef)?;
    let sounding = tuning_for(&written, "-8")?;
    for candidate in [written, sounding] {
        let open = midis(&candidate)?;
        let (Some(&lowest), Some(&highest)) = (open.first(), open.last()) else {
            continue;
        };
        if low >= lowest && high <= highest + HIGHEST_FRET {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

#[wasm_bindgen]
/// The tuning abcjs should tab a staff against, given the staff positions
/// written on it, or null when the instrument cannot reach them.
pub fn tab_tuning(
    strings: Vec<String>,
    clef: &str,
    positions: Vec<i32>,
) -> Result<JsValue, JsValue> {
    to_js(&staff_tuning(&strings, clef, &positions)?)
}

/// The pitches that move when the frets of a chord are carried `moved`
/// strings across, each keeping its fret, as `(pitch index, MIDI)`. `None`
/// when a fret would leave the neck or `open` does not account for them.
fn carry(open: &[i32], midis: &[i32], frets: &[i32], moved: i32) -> Option<Vec<(usize, i32)>> {
    let mut used_pitch = vec![false; midis.len()];
    let mut used_string = vec![false; open.len()];
    let mut found = 0;
    let mut out = Vec::new();
    let mut unreachable = false;
    for &fret in frets {
        for string in (0..open.len()).rev() {
            if used_string[string] {
                continue;
            }
            let Some(index) =
                (0..midis.len()).find(|&i| !used_pitch[i] && midis[i] == open[string] + fret)
            else {
                continue;
            };
            used_pitch[index] = true;
            used_string[string] = true;
            found += 1;
            match usize::try_from(string as i32 + moved)
                .ok()
                .and_then(|t| open.get(t))
            {
                Some(target) => out.push((index, target + fret)),
                None => unreachable = true,
            }
            break;
        }
    }
    if found != frets.len() || unreachable || out.is_empty() {
        return None;
    }
    Some(out)
}

#[wasm_bindgen]
/// Moves the frets shown under a note across `moved` strings, each keeping
/// its fret: a 3 dropped from the A string onto the D string sounds the D
/// string's third fret. `midis` are the note's pitches and `clef` its
/// staff's. Gives `[pitch index, MIDI]` pairs to change, or null.
pub fn move_frets(
    strings: Vec<String>,
    clef: &str,
    midis: Vec<i32>,
    frets: Vec<i32>,
    moved: i32,
) -> Result<JsValue, JsValue> {
    // The staff was tabbed against the written or the sounding strings; the
    // tuning that accounts for every fret shown is the one it was.
    let written = tab_strings(strings, clef)?;
    let sounding: Vec<i32> = written.iter().map(|midi| midi - OCTAVE_SEMITONES).collect();
    for open in [written, sounding] {
        let accounted = carry(&open, &midis, &frets, 0).is_some();
        if !accounted {
            continue;
        }
        return to_js(&carry(&open, &midis, &frets, moved));
    }
    Ok(JsValue::NULL)
}

// ----------------------------------------------------------- chord part

#[derive(Deserialize)]
struct Symbol {
    start: f64,
    name: String,
}

#[derive(Serialize)]
struct Bar {
    start: f64,
    abc: String,
}

/// An ABC length for `quarters` against `L:1/8`.
fn abc_length(quarters: f64) -> Result<String, JsValue> {
    let eighths = quarters * EIGHTHS_PER_QUARTER;
    if (eighths - eighths.round()).abs() < EPSILON {
        return abc_duration(eighths.round() as u32, 1).map_err(js_error);
    }
    let twelfths = (eighths * TWELFTHS as f64).round() as u32;
    abc_duration(twelfths, TWELFTHS).map_err(js_error)
}

/// What a stroke of a comping figure plays of the chord.
#[derive(Clone, Copy)]
enum Stroke {
    /// The thumb on the chord's lowest note.
    Root,
    /// The thumb on the fifth below the lowest note where the strings reach,
    /// above it where they do not, and on the bass again over a named bass.
    Fifth,
    /// The fingers on the rest of the chord.
    Chord,
}

/// A way of playing the chord part other than holding each chord: a bar
/// length in quarters and the figure of each bar in turn, each stroke the
/// quarter it falls on and what it plays. A bar of any other length is held.
struct Comping {
    bar: f64,
    bars: &'static [&'static [(f64, Stroke)]],
}

/// The bossa nova: the thumb on one and three; the fingers on one, two and
/// the and of three, then on the and of one and of two.
const BOSSA: Comping = Comping {
    bar: 4.0,
    bars: &[
        &[
            (0.0, Stroke::Root),
            (1.0, Stroke::Chord),
            (2.0, Stroke::Fifth),
            (2.5, Stroke::Chord),
        ],
        &[
            (0.0, Stroke::Root),
            (0.5, Stroke::Chord),
            (1.5, Stroke::Chord),
            (2.0, Stroke::Fifth),
        ],
    ],
};

/// The comping figure a page names, or `None` to hold each chord.
fn comping_named(name: &str) -> Option<&'static Comping> {
    match name {
        "bossa" => Some(&BOSSA),
        _ => None,
    }
}

/// A chord symbol and where it holds, in quarters.
struct Span<'a> {
    start: f64,
    end: f64,
    name: &'a str,
}

/// The chord symbols realized as bars of ABC, one per bar of the score:
/// each symbol voiced on `strings` and held until the next, tied over the
/// barlines, or played in the `comping` figure where one is named. A staff
/// whose `clef` sounds an octave down is written an octave above the
/// voicing.
fn chord_bars(
    input: ScoreInput,
    symbols: Vec<Symbol>,
    strings: &[String],
    clef: &str,
    comping: &str,
) -> Result<Vec<Bar>, JsValue> {
    let score = Score::from_input(input)?;
    let lines = score.barlines();
    let end = lines.last().copied().unwrap_or(0.0);
    let (tonic, mode, sharps) = match &score.key {
        Some(key) => (key.tonic.as_str(), key.mode.as_str(), key.sharps),
        None => ("C", "major", 0),
    };
    let alters = key_alters(sharps);
    let octaves = Octaves::of_clef(clef);
    let open = tab_strings(strings.to_vec(), clef)?;
    let lift = match octaves {
        Octaves::Written => 0,
        Octaves::Down => OCTAVE_SEMITONES,
    };
    let lowest_string = open.first().copied().unwrap_or_default();
    let pattern = comping_named(comping);

    let mut symbols = symbols;
    symbols.sort_by(|a, b| a.start.total_cmp(&b.start));
    let spans: Vec<Span<'_>> = symbols
        .iter()
        .enumerate()
        .map(|(index, symbol)| Span {
            start: symbol.start,
            end: symbols.get(index + 1).map_or(end, |next| next.start),
            name: &symbol.name,
        })
        .collect();
    let mut voicings: BTreeMap<&str, Option<Vec<i32>>> = BTreeMap::new();
    for span in &spans {
        voicings.entry(span.name).or_insert_with(|| {
            chord_symbol_voicing_notes(span.name, &open)
                .ok()
                .filter(|notes| !notes.is_empty())
        });
    }
    let voicing = |name: &str| voicings.get(name).and_then(Option::as_ref);

    // Spells sounding MIDI numbers as ABC, with the accidentals the bar
    // needs.
    let write = |notes: &[i32], in_force: &mut BTreeMap<(char, i32), i32>| {
        let mut written = String::new();
        for &midi in notes {
            let pitch = spell_midi(midi + lift, tonic, mode)?;
            written.push_str(&abc_pitch(&pitch, &alters, in_force)?);
        }
        Ok::<_, JsValue>(written)
    };

    let mut bars = Vec::new();
    let mut patterned = 0;
    for pair in lines.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        let mut in_force = BTreeMap::new();
        let mut tokens = Vec::new();

        // A bar the figure fits is played in it, stroke by stroke.
        if let Some(pattern) = pattern.filter(|pattern| (to - from - pattern.bar).abs() < EPSILON) {
            let figure = pattern.bars[patterned % pattern.bars.len()];
            patterned += 1;
            for (index, &(at, stroke)) in figure.iter().enumerate() {
                let start = from + at;
                let stop = from + figure.get(index + 1).map_or(pattern.bar, |next| next.0);
                let length = abc_length(stop - start)?;
                let span = spans
                    .iter()
                    .find(|span| span.start <= start + EPSILON && start < span.end - EPSILON);
                let Some((span, voiced)) = span.and_then(|span| Some((span, voicing(span.name)?)))
                else {
                    tokens.push(format!("z{length}"));
                    continue;
                };
                let (low, upper) = (voiced[0], &voiced[1..]);
                let fifth = if span.name.contains('/') {
                    low
                } else if low - 5 >= lowest_string {
                    low - 5
                } else {
                    low + 7
                };
                let notes = match stroke {
                    Stroke::Root => vec![low],
                    Stroke::Fifth => vec![fifth],
                    Stroke::Chord => upper.to_vec(),
                };
                if notes.is_empty() {
                    tokens.push(format!("z{length}"));
                    continue;
                }
                let written = write(&notes, &mut in_force)?;
                if notes.len() > 1 {
                    tokens.push(format!("[{written}]{length}"));
                } else {
                    tokens.push(format!("{written}{length}"));
                }
            }
            bars.push(Bar {
                start: from,
                abc: tokens.join(" "),
            });
            continue;
        }

        let mut cursor = from;
        for span in &spans {
            let start = span.start.max(from);
            let stop = span.end.min(to);
            if stop <= start + EPSILON {
                continue;
            }
            if start > cursor + EPSILON {
                tokens.push(format!("z{}", abc_length(start - cursor)?));
            }
            cursor = stop;
            let length = abc_length(stop - start)?;
            let Some(voiced) = voicing(span.name) else {
                tokens.push(format!("z{length}"));
                continue;
            };
            // A held chord ties over the barline, except in a part played
            // in a figure, whose next bar strikes it again.
            let tie = if span.end > to + EPSILON && pattern.is_none() {
                "-"
            } else {
                ""
            };
            tokens.push(format!("[{}]{length}{tie}", write(voiced, &mut in_force)?));
        }
        if to > cursor + EPSILON {
            tokens.push(format!("z{}", abc_length(to - cursor)?));
        }
        bars.push(Bar {
            start: from,
            abc: tokens.join(" "),
        });
    }
    Ok(bars)
}

#[wasm_bindgen]
/// The score's chord symbols (`[{ start, name }]`, in quarters) realized as
/// a part for a fretted instrument whose open strings are `strings` (ABC,
/// lowest first) on a staff with `clef`: `[{ start, abc }]`, one per bar.
/// `comping` names a figure to play the chords in (`bossa`), or is empty
/// to hold them.
pub fn chord_part(
    input: JsValue,
    symbols: JsValue,
    strings: Vec<String>,
    clef: &str,
    comping: &str,
) -> Result<JsValue, JsValue> {
    let input: ScoreInput = serde_wasm_bindgen::from_value(input).map_err(js_error)?;
    let symbols: Vec<Symbol> = serde_wasm_bindgen::from_value(symbols).map_err(js_error)?;
    to_js(&chord_bars(input, symbols, &strings, clef, comping)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::{NoteInput, PitchInput, VoiceInput};

    /// Two bars of 4/4 in quarters under a C chord symbol, as a guitar's
    /// chord part in `comping`.
    fn c_chord_part(comping: &str) -> Vec<String> {
        let notes = (0..8)
            .map(|beat| NoteInput {
                start: f64::from(beat),
                duration: 1.0,
                velocity: 90,
                pitches: vec![PitchInput {
                    midi: 60,
                    staff: Some(0),
                }],
                char_start: None,
                char_end: None,
            })
            .collect();
        let input = ScoreInput {
            title: String::new(),
            tempo_bpm: 120.0,
            key: None,
            meter: Some([4, 4]),
            pickup: 0.0,
            let_ring: false,
            voices: vec![VoiceInput {
                name: String::new(),
                clef: String::new(),
                notes,
            }],
        };
        let symbols = vec![Symbol {
            start: 0.0,
            name: "C".to_string(),
        }];
        let guitar = strings(&["E,", "A,", "D", "G", "B", "e"]);
        chord_bars(input, symbols, &guitar, "treble-8", comping)
            .unwrap()
            .into_iter()
            .map(|bar| bar.abc)
            .collect()
    }

    #[test]
    fn a_held_chord_ties_over_the_barline() {
        let bars = c_chord_part("");
        assert_eq!(bars.len(), 2);
        assert!(bars[0].ends_with("8-"), "{bars:?}");
        assert!(!bars[1].contains('-'), "{bars:?}");
    }

    #[test]
    fn a_bossa_plays_each_bar_in_its_figure() {
        let bars = c_chord_part("bossa");
        // Bass on one, chord on two, fifth on three, chord on its and; then
        // the bass, the and of one and of two, and the fifth.
        let strokes: Vec<usize> = bars.iter().map(|bar| bar.split(' ').count()).collect();
        assert_eq!(strokes, [4, 4], "{bars:?}");
        assert!(bars.iter().all(|bar| !bar.contains('-')), "{bars:?}");
        let first: Vec<&str> = bars[0].split(' ').collect();
        assert!(first[1].starts_with('['), "{bars:?}");
        assert!(!first[0].starts_with('['), "{bars:?}");
    }

    fn strings(notes: &[&str]) -> Vec<String> {
        notes.iter().map(|note| note.to_string()).collect()
    }

    #[test]
    fn staff_positions_count_from_middle_c() {
        assert_eq!(natural_at(0).unwrap().midi(), 60);
        assert_eq!(natural_at(-1).unwrap().midi(), 59);
        assert_eq!(natural_at(7).unwrap().midi(), 72);
        assert_eq!(staff_of(&parse_abc("c").unwrap()), 7);
        assert_eq!(staff_of(&parse_abc("^F,").unwrap()), -4);
        assert_eq!(parse_abc("_B,,").unwrap().midi(), 46);
    }

    #[test]
    fn a_step_drops_the_accidental_and_an_octave_keeps_it() {
        assert_eq!(shift("^F", 1).unwrap(), "G");
        assert_eq!(shift("B", 1).unwrap(), "c");
        assert_eq!(shift("_e", -7).unwrap(), "_E");
        assert_eq!(shift("C", -8).unwrap(), "B,,");
    }

    #[test]
    fn the_accidental_comes_from_the_nearest_octave() {
        assert_eq!(abc_accidental_text(alter_at(3, 66).unwrap()), Some("^"));
        assert_eq!(abc_accidental_text(alter_at(3, 54).unwrap()), Some("^"));
        assert_eq!(abc_accidental_text(alter_at(0, 64).unwrap()), None);
    }

    #[test]
    fn a_staff_out_of_reach_gets_no_tab() {
        let guitar = strings(&["E,", "A,", "D", "G", "B", "e"]);
        // Written guitar music, E3 up to the twelfth fret: the written strings.
        let tuned = staff_tuning(&guitar, "", &[-5, 14]).unwrap();
        assert_eq!(tuned, Some(guitar.clone()));
        // Piano music at pitch, down to E2, goes to the strings as they sound.
        let low = staff_tuning(&guitar, "", &[-12]).unwrap().unwrap();
        // Nothing reaches D1.
        assert_eq!(staff_tuning(&guitar, "", &[-20]).unwrap(), None);
        assert_eq!(low[0], "E,,");
    }

    #[test]
    fn frets_carried_across_keep_their_numbers() {
        let open = [40, 45, 50, 55, 59, 64];
        // Third fret on the A string (C3) moved up a string: F3.
        assert_eq!(carry(&open, &[48], &[3], 1), Some(vec![(0, 53)]));
        assert_eq!(carry(&open, &[67], &[3], 1), None);
        assert_eq!(carry(&open, &[61], &[3], 0), None);
    }

    #[test]
    fn lengths_are_eighths_or_twelfths_of_one() {
        assert_eq!(abc_length(0.5).unwrap(), "");
        assert_eq!(abc_length(4.0).unwrap(), "8");
        assert_eq!(abc_length(1.0 / 3.0).unwrap(), "2/3");
    }
}
