//! WebAssembly bindings for the browser examples.

use music21_rs::{
    ALL_TUNING_SYSTEMS, COMMON_TWELVE_TONE_TUNING_SYSTEMS, Chord, ChordResolutionSuggestion, Key,
    KnownChordType, Pitch, Polyrhythm, Result, pitch_class_name,
};
use serde::Serialize;
use std::collections::BTreeSet;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct TuningFrequencyInfo {
    name: &'static str,
    frequency_hz: f64,
    cents_from_equal_temperament: f64,
}

#[derive(Serialize)]
struct PitchInfo {
    index: usize,
    name: String,
    name_with_octave: String,
    midi: i32,
    octave: Option<i32>,
    pitch_space: f64,
    pitch_class: u8,
    alter: f64,
    frequency_hz: f64,
    tuning_frequencies: Vec<TuningFrequencyInfo>,
}

#[derive(Serialize)]
struct ResolutionChordInfo {
    pitched_common_name: String,
    key_context: String,
    pitch_names: Vec<String>,
    pitch_classes: Vec<u8>,
}

#[derive(Serialize)]
struct ChordAnalysis {
    input: String,
    common_name: String,
    common_names: Vec<String>,
    pitched_common_name: String,
    pitched_common_names: Vec<String>,
    chord_symbol: Option<String>,
    chord_symbols: Vec<String>,
    pitch_classes: Vec<u8>,
    root_pitch_name: Option<String>,
    bass_pitch_name: Option<String>,
    forte_class: Option<String>,
    normal_form: Option<Vec<u8>>,
    interval_class_vector: Option<Vec<u8>>,
    inversion: Option<u8>,
    inversion_name: Option<String>,
    key_context: Option<String>,
    key_estimate: Option<String>,
    guitar_fingering: Option<GuitarFingeringInfo>,
    polyrhythm_input: String,
    resolution_chords: Vec<ResolutionChordInfo>,
    pitches: Vec<PitchInfo>,
}

#[derive(Serialize)]
struct GuitarFingeringInfo {
    strings: Vec<GuitarStringFingeringInfo>,
    base_fret: u8,
    fret_span: u8,
    covered_pitch_spaces: Vec<i32>,
    omitted_pitch_spaces: Vec<i32>,
    covered_pitch_classes: Vec<u8>,
    omitted_pitch_classes: Vec<u8>,
}

#[derive(Serialize)]
struct GuitarStringFingeringInfo {
    string_number: u8,
    string_name: String,
    open_pitch_space: i32,
    open_pitch_class: u8,
    fret: Option<u8>,
    finger: Option<u8>,
    pitch_space: Option<i32>,
    pitch_class: Option<u8>,
    pitch_name: Option<String>,
}

#[derive(Serialize)]
struct KnownChordInfo {
    id: String,
    primary_common_name: String,
    common_names: Vec<String>,
    chord_symbol: Option<String>,
    key_estimate: Option<String>,
    resolution_chords: Vec<ResolutionChordInfo>,
    inversion_labels: Vec<Option<String>>,
    cardinality: u8,
    forte_class: String,
    normal_form: Vec<u8>,
    interval_class_vector: Vec<u8>,
    pitch_classes: Vec<u8>,
    pitch_names: Vec<String>,
    display_pitch_names: Vec<String>,
    chord_input: String,
}

#[derive(Serialize)]
struct TuningSystemInfo {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    octave_size: u32,
    root_frequency_hz: f64,
    degrees: Vec<TuningDegreeInfo>,
}

#[derive(Serialize)]
struct TuningDegreeInfo {
    degree: u32,
    label: String,
    ratio: f64,
    ratio_label: String,
    frequency_hz: f64,
    cents_from_equal_temperament: f64,
}

#[derive(Serialize)]
struct PolyrhythmAnalysisInfo {
    components: Vec<u32>,
    base: u32,
    tempo: u32,
    cycle: u32,
    tick_duration: f64,
    component_intervals: Vec<u32>,
    hit_events: Vec<PolyrhythmEventInfo>,
    ratio_tones: Vec<PolyrhythmRatioToneInfo>,
    pitches: Vec<String>,
    chord_input: String,
}

#[derive(Serialize)]
struct PolyrhythmEventInfo {
    tick: u32,
    time_seconds: f64,
    triggers: Vec<bool>,
}

#[derive(Serialize)]
struct PolyrhythmRatioToneInfo {
    component: u32,
    offset: i32,
    ratio: f64,
}

#[wasm_bindgen]
/// Analyzes a chord input string or MIDI-number list for the chord analyzer page.
pub fn analyze_chord(input: &str) -> Result<JsValue, JsValue> {
    analyze_chord_inner(input, None)
}

#[wasm_bindgen]
/// Analyzes a chord input string with an optional key context for resolution suggestions.
pub fn analyze_chord_with_key(input: &str, key_context: &str) -> Result<JsValue, JsValue> {
    analyze_chord_inner(input, Some(key_context))
}

fn analyze_chord_inner(input: &str, key_context: Option<&str>) -> Result<JsValue, JsValue> {
    let midi_numbers = parse_midi_input(input);
    let chord = if let Some(midi_numbers) = midi_numbers.as_deref() {
        Chord::new(midi_numbers)
    } else {
        input.parse::<Chord>()
    }
    .map_err(|err| JsValue::from_str(&err.to_string()))?;
    let common_name = chord.common_name();
    let root_pitch_name = chord.root_pitch_name();
    let chord_symbols = chord.chord_symbols();
    let chord_symbol = chord_symbols.first().cloned();
    let key_context = parse_key_context(key_context)?;
    let key_context_display = key_context.as_ref().map(display_key_context);
    let key_estimate = key_estimate_for_chord(&chord);
    let resolution_chords = match key_context.as_ref() {
        Some(key) => chord.resolution_suggestions_in_key(key),
        None => chord.resolution_suggestions(),
    }
    .map_err(|err| JsValue::from_str(&err.to_string()))?
    .into_iter()
    .map(resolution_chord_info)
    .collect();

    let pitches =
        display_pitch_infos(chord.pitches()).map_err(|err| JsValue::from_str(&err.to_string()))?;

    serde_wasm_bindgen::to_value(&ChordAnalysis {
        input: input.to_string(),
        common_name,
        common_names: chord.common_names(),
        pitched_common_name: chord.pitched_common_name(),
        pitched_common_names: chord.pitched_common_names(),
        chord_symbol,
        chord_symbols,
        pitch_classes: chord.pitch_classes(),
        root_pitch_name,
        bass_pitch_name: chord.bass_pitch_name(),
        forte_class: chord.forte_class(),
        normal_form: chord.normal_form(),
        interval_class_vector: chord.interval_class_vector(),
        inversion: chord.inversion(),
        inversion_name: chord.inversion_name(),
        key_context: key_context_display,
        key_estimate,
        guitar_fingering: chord.guitar_fingering().map(guitar_fingering_info),
        polyrhythm_input: chord.polyrhythm_ratio_string(),
        resolution_chords,
        pitches,
    })
    .map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen]
/// Returns the precomputed known-chord browser data.
pub fn known_chords() -> Result<JsValue, JsValue> {
    let chords = Chord::known_chord_types()
        .into_iter()
        .flat_map(|chord| {
            dyad_browser_variants(&chord).unwrap_or_else(|| vec![known_chord_info_for_type(&chord)])
        })
        .collect::<Vec<_>>();

    serde_wasm_bindgen::to_value(&chords).map_err(|err| JsValue::from_str(&err.to_string()))
}

fn known_chord_info_for_type(chord: &KnownChordType) -> KnownChordInfo {
    let primary_common_name = chord
        .common_names
        .first()
        .cloned()
        .unwrap_or_else(|| format!("forte class {}", chord.forte_class));
    known_chord_info(
        chord,
        primary_common_name,
        chord.common_names.clone(),
        chord.normal_form.clone(),
        "normal",
    )
}

fn known_chord_info(
    chord: &KnownChordType,
    primary_common_name: String,
    common_names: Vec<String>,
    pitch_classes: Vec<u8>,
    id_suffix: &str,
) -> KnownChordInfo {
    let pitch_names = pitch_classes
        .iter()
        .map(|pitch_class| pitch_class_name(*pitch_class).to_string())
        .collect::<Vec<_>>();
    let display_pitch_names = pitch_names
        .iter()
        .map(|name| display_pitch_name(name))
        .collect::<Vec<_>>();
    let realized_chord = Chord::new(pitch_names.as_slice()).ok();
    let chord_symbol = realized_chord
        .as_ref()
        .and_then(|chord| chord.chord_symbol_with_root(0).ok().flatten());
    let key_estimate = realized_chord.as_ref().and_then(key_estimate_for_chord);
    let resolution_chords = realized_chord
        .as_ref()
        .and_then(|chord| chord.resolution_suggestions().ok())
        .unwrap_or_default()
        .into_iter()
        .map(resolution_chord_info)
        .collect();
    let inversion_labels =
        browser_inversion_labels(&primary_common_name, &common_names, &pitch_classes);
    KnownChordInfo {
        id: format!("{}:{id_suffix}:{primary_common_name}", chord.forte_class),
        primary_common_name,
        common_names,
        chord_symbol,
        key_estimate,
        resolution_chords,
        inversion_labels,
        cardinality: chord.cardinality,
        forte_class: chord.forte_class.clone(),
        normal_form: chord.normal_form.clone(),
        interval_class_vector: chord.interval_class_vector.clone(),
        pitch_classes,
        chord_input: pitch_names.join(" "),
        pitch_names,
        display_pitch_names,
    }
}

fn dyad_browser_variants(chord: &KnownChordType) -> Option<Vec<KnownChordInfo>> {
    if chord.cardinality != 2 || chord.normal_form.len() != 2 {
        return None;
    }

    let span = (chord.normal_form[1] + 12 - chord.normal_form[0]) % 12;
    let interval_class = span.min(12 - span);
    let variants = match interval_class {
        1 => vec![
            (
                "minor second",
                vec!["m2", "half step", "semitone", "interval class 1"],
                vec![0, 1],
            ),
            ("major seventh", vec!["M7", "interval class 1"], vec![0, 11]),
        ],
        2 => vec![
            (
                "major second",
                vec!["M2", "whole step", "whole tone", "interval class 2"],
                vec![0, 2],
            ),
            ("minor seventh", vec!["m7", "interval class 2"], vec![0, 10]),
        ],
        3 => vec![
            ("minor third", vec!["m3", "interval class 3"], vec![0, 3]),
            ("major sixth", vec!["M6", "interval class 3"], vec![0, 9]),
        ],
        4 => vec![
            ("major third", vec!["M3", "interval class 4"], vec![0, 4]),
            ("minor sixth", vec!["m6", "interval class 4"], vec![0, 8]),
        ],
        5 => vec![
            ("perfect fourth", vec!["P4", "interval class 5"], vec![0, 5]),
            ("perfect fifth", vec!["P5", "interval class 5"], vec![0, 7]),
        ],
        6 => vec![(
            "tritone",
            vec![
                "diminished fifth",
                "augmented fourth",
                "d5",
                "A4",
                "interval class 6",
            ],
            vec![0, 6],
        )],
        _ => return None,
    };

    Some(
        variants
            .into_iter()
            .map(|(primary, aliases, pitch_classes)| {
                let names = ordered_unique_names(
                    std::iter::once(primary.to_string())
                        .chain(aliases.into_iter().map(str::to_string)),
                );
                known_chord_info(chord, primary.to_string(), names, pitch_classes, primary)
            })
            .collect(),
    )
}

fn ordered_unique_names(names: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut unique = Vec::new();
    for name in names {
        if !unique.contains(&name) {
            unique.push(name);
        }
    }
    unique
}

fn browser_inversion_labels(
    primary_common_name: &str,
    common_names: &[String],
    pitch_classes: &[u8],
) -> Vec<Option<String>> {
    let own_names = std::iter::once(primary_common_name)
        .chain(common_names.iter().map(String::as_str))
        .map(normalized_chord_name)
        .collect::<BTreeSet<_>>();

    (0..pitch_classes.len())
        .map(|inversion| {
            if inversion == 0 {
                return None;
            }

            let chord = browser_realized_chord(pitch_classes, inversion)?;
            let common_name = chord.common_name();
            let normalized = normalized_chord_name(&common_name);
            if normalized == "unknown chord" || own_names.contains(&normalized) {
                return None;
            }

            Some(display_inversion_name(&common_name))
        })
        .collect()
}

fn browser_realized_chord(pitch_classes: &[u8], inversion: usize) -> Option<Chord> {
    let names = browser_input_names(pitch_classes, inversion);
    if names.is_empty() {
        return None;
    }
    Chord::new(names.as_slice()).ok()
}

fn browser_input_names(pitch_classes: &[u8], inversion: usize) -> Vec<String> {
    if pitch_classes.is_empty() {
        return Vec::new();
    }

    let root_position = pitch_classes
        .iter()
        .map(|pitch_class| 60 + i32::from(*pitch_class))
        .collect::<Vec<_>>();
    let rotation = inversion % root_position.len();
    root_position[rotation..]
        .iter()
        .copied()
        .chain(root_position[..rotation].iter().map(|midi| midi + 12))
        .map(browser_pitch_name)
        .collect()
}

fn browser_pitch_name(midi: i32) -> String {
    let pitch_class = midi.rem_euclid(12) as u8;
    let octave = midi.div_euclid(12) - 1;
    format!("{}{octave}", pitch_class_name(pitch_class))
}

fn normalized_chord_name(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn display_inversion_name(name: &str) -> String {
    let normalized = name.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = normalized.chars();
    let Some(first) = chars.next() else {
        return normalized;
    };
    first.to_lowercase().chain(chars).collect()
}

#[wasm_bindgen]
/// Returns pitch-frequency tables for the supported tuning systems.
pub fn tuning_systems(root_frequency_hz: f64) -> Result<JsValue, JsValue> {
    if !root_frequency_hz.is_finite() || root_frequency_hz <= 0.0 {
        return Err(JsValue::from_str(
            "root frequency must be a positive number",
        ));
    }

    let systems = ALL_TUNING_SYSTEMS
        .into_iter()
        .map(|tuning_system| {
            let octave_size = tuning_system.octave_size();
            let label_base = octave_size * 5;
            let degrees = (0..=octave_size)
                .map(|degree| {
                    let fraction = tuning_system.fraction(degree as usize);
                    let ratio = fraction.ratio();
                    TuningDegreeInfo {
                        degree,
                        label: tuning_system.label(label_base + degree),
                        ratio,
                        ratio_label: fraction.label(),
                        frequency_hz: root_frequency_hz * ratio,
                        cents_from_equal_temperament: tuning_system.cents(degree),
                    }
                })
                .collect();

            TuningSystemInfo {
                id: tuning_system.id(),
                name: tuning_system.display_name(),
                description: tuning_system.description(),
                octave_size,
                root_frequency_hz,
                degrees,
            }
        })
        .collect::<Vec<_>>();

    serde_wasm_bindgen::to_value(&systems).map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen]
/// Analyzes a polyrhythm and maps its ratio tones onto pitches.
pub fn analyze_polyrhythm(
    components: JsValue,
    base: u32,
    tempo: u32,
    root: &str,
) -> Result<JsValue, JsValue> {
    let components = serde_wasm_bindgen::from_value::<Vec<u32>>(components)
        .map_err(|err| JsValue::from_str(&err.to_string()))?;
    let polyrhythm = Polyrhythm::from_time_signature(base, tempo, &components)
        .map_err(|err| JsValue::from_str(&err.to_string()))?;
    let analysis = polyrhythm
        .analysis()
        .map_err(|err| JsValue::from_str(&err.to_string()))?;
    let pitches = polyrhythm
        .ratio_pitches(root)
        .map_err(|err| JsValue::from_str(&err.to_string()))?
        .into_iter()
        .map(|pitch| pitch.name_with_octave())
        .collect::<Vec<_>>();
    let chord_input = pitches.join(" ");

    serde_wasm_bindgen::to_value(&PolyrhythmAnalysisInfo {
        components: analysis.components,
        base: analysis.base,
        tempo: analysis.tempo,
        cycle: analysis.cycle,
        tick_duration: analysis.tick_duration,
        component_intervals: analysis.component_intervals,
        hit_events: analysis
            .hit_events
            .into_iter()
            .map(|event| PolyrhythmEventInfo {
                tick: event.tick,
                time_seconds: event.time_seconds,
                triggers: event.triggers,
            })
            .collect(),
        ratio_tones: analysis
            .ratio_tones
            .into_iter()
            .map(|tone| PolyrhythmRatioToneInfo {
                component: tone.component,
                offset: tone.offset,
                ratio: tone.ratio,
            })
            .collect(),
        pitches,
        chord_input,
    })
    .map_err(|err| JsValue::from_str(&err.to_string()))
}

fn display_pitch_infos(pitches: Vec<Pitch>) -> Result<Vec<PitchInfo>> {
    let mut last_pitch_space: Option<i32> = None;
    let mut infos = Vec::with_capacity(pitches.len());

    for (index, pitch) in pitches.into_iter().enumerate() {
        let display_pitch = display_pitch_for_sequence(pitch, &mut last_pitch_space)?;
        let pitch_space = display_pitch.ps();
        let tuning_frequencies = COMMON_TWELVE_TONE_TUNING_SYSTEMS
            .iter()
            .copied()
            .map(|tuning_system| TuningFrequencyInfo {
                name: tuning_system.display_name(),
                frequency_hz: display_pitch.frequency_hz_in(tuning_system),
                cents_from_equal_temperament: tuning_system.cents_at(pitch_space),
            })
            .collect();

        infos.push(PitchInfo {
            index,
            name: display_pitch.name(),
            name_with_octave: display_pitch.name_with_octave(),
            midi: pitch_space.round() as i32,
            octave: display_pitch.octave(),
            pitch_space,
            pitch_class: (pitch_space.round() as i32).rem_euclid(12) as u8,
            alter: display_pitch.alter(),
            frequency_hz: display_pitch.frequency_hz(),
            tuning_frequencies,
        });
    }

    Ok(infos)
}

fn parse_midi_input(input: &str) -> Option<Vec<i32>> {
    let trimmed = input.trim();
    let lower = trimmed.to_ascii_lowercase();
    let trimmed = if lower.starts_with("midi:") {
        trimmed[5..].trim()
    } else if lower.starts_with("midi ") {
        trimmed[4..].trim()
    } else {
        trimmed
    };
    if trimmed.is_empty() {
        return None;
    }

    let tokens = trimmed
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        return None;
    }

    tokens
        .into_iter()
        .map(str::parse::<i32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()
}

fn key_estimate_for_chord(chord: &Chord) -> Option<String> {
    music21_rs::estimate_key_from_chords(std::slice::from_ref(chord))
        .ok()?
        .first()
        .map(|estimate| {
            let estimated_tonic = estimate.key().tonic();
            let tonic_name = chord
                .root_pitch_name()
                .and_then(|root_name| {
                    let root = Pitch::from_name(&root_name).ok()?;
                    same_pitch_class(&root, &estimated_tonic)
                        .then(|| display_pitch_name(&root_name))
                })
                .unwrap_or_else(|| display_pitch_name(&estimated_tonic.name()));
            format!("{} {}", tonic_name, estimate.key().mode())
        })
}

fn same_pitch_class(left: &Pitch, right: &Pitch) -> bool {
    (left.ps().round() as i32).rem_euclid(12) == (right.ps().round() as i32).rem_euclid(12)
}

fn parse_key_context(key_context: Option<&str>) -> Result<Option<Key>, JsValue> {
    let Some(key_context) = key_context.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    key_context
        .parse::<Key>()
        .map(Some)
        .map_err(|err| JsValue::from_str(&format!("Key context: {err}")))
}

fn display_key_context(key: &Key) -> String {
    format!("{} {}", display_pitch_name(&key.tonic().name()), key.mode())
}

fn guitar_fingering_info(fingering: music21_rs::GuitarFingering) -> GuitarFingeringInfo {
    GuitarFingeringInfo {
        strings: fingering
            .strings
            .into_iter()
            .map(|string| GuitarStringFingeringInfo {
                string_number: string.string_number,
                string_name: string.string_name,
                open_pitch_space: string.open_pitch_space,
                open_pitch_class: string.open_pitch_class,
                fret: string.fret,
                finger: string.finger,
                pitch_space: string.pitch_space,
                pitch_class: string.pitch_class,
                pitch_name: string
                    .pitch_class
                    .map(|pitch_class| display_pitch_name(pitch_class_name(pitch_class))),
            })
            .collect(),
        base_fret: fingering.base_fret,
        fret_span: fingering.fret_span,
        covered_pitch_spaces: fingering.covered_pitch_spaces,
        omitted_pitch_spaces: fingering.omitted_pitch_spaces,
        covered_pitch_classes: fingering.covered_pitch_classes,
        omitted_pitch_classes: fingering.omitted_pitch_classes,
    }
}

fn resolution_chord_info(suggestion: ChordResolutionSuggestion) -> ResolutionChordInfo {
    let chord = suggestion.chord;
    ResolutionChordInfo {
        pitched_common_name: chord.pitched_common_name(),
        key_context: suggestion.key_context,
        pitch_names: chord
            .pitches()
            .iter()
            .map(|pitch| pitch.name_with_octave())
            .collect(),
        pitch_classes: chord.pitch_classes(),
    }
}

fn display_pitch_name(name: &str) -> String {
    name.replace('-', "b")
}

fn display_pitch_for_sequence(pitch: Pitch, last_pitch_space: &mut Option<i32>) -> Result<Pitch> {
    if pitch.octave().is_some() {
        *last_pitch_space = Some(pitch.ps().round() as i32);
        return Ok(pitch);
    }

    let pitch_class = (pitch.ps().round() as i32).rem_euclid(12);
    let mut pitch_space = 60 + pitch_class;
    while last_pitch_space.is_some_and(|last| pitch_space <= last) {
        pitch_space += 12;
    }

    *last_pitch_space = Some(pitch_space);
    format!("{}{}", pitch.name(), (pitch_space / 12) - 1).parse()
}

#[cfg(test)]
mod tests {
    use super::{key_estimate_for_chord, known_chord_info, parse_midi_input};
    use music21_rs::{Chord, KnownChordType};

    #[test]
    fn parse_midi_input_accepts_plain_prefixed_and_csv_values() {
        assert_eq!(parse_midi_input("60 64 67"), Some(vec![60, 64, 67]));
        assert_eq!(parse_midi_input("midi: 60,64,67"), Some(vec![60, 64, 67]));
        assert_eq!(parse_midi_input("MIDI 60 64 67"), Some(vec![60, 64, 67]));
        assert_eq!(parse_midi_input("C E G"), None);
    }

    #[test]
    fn known_chord_info_includes_symbol_and_key_estimate() {
        let chord = KnownChordType {
            cardinality: 3,
            forte_class: "3-11".to_string(),
            normal_form: vec![0, 4, 7],
            interval_class_vector: vec![0, 0, 1, 1, 1, 0],
            common_names: vec!["major triad".to_string()],
        };
        let info = known_chord_info(
            &chord,
            "major triad".to_string(),
            chord.common_names.clone(),
            vec![0, 4, 7],
            "normal",
        );

        assert_eq!(info.chord_symbol.as_deref(), Some("C"));
        assert_eq!(info.key_estimate.as_deref(), Some("C major"));
        assert_eq!(info.inversion_labels.len(), 3);
        assert!(info.resolution_chords.is_empty());
    }

    #[test]
    fn known_chord_info_uses_c_root_for_chord_symbols() {
        let cases = [
            ("major sixth", vec![0, 9], "C add(13)"),
            ("tritone", vec![0, 6], "C add(b5)"),
            ("major second", vec![0, 2], "C add(9)"),
            (
                "incomplete dominant-seventh chord",
                vec![0, 3, 5],
                "C add(b3,11)",
            ),
        ];

        for (name, pitch_classes, expected_symbol) in cases {
            let chord = KnownChordType {
                cardinality: pitch_classes.len() as u8,
                forte_class: "test".to_string(),
                normal_form: pitch_classes.clone(),
                interval_class_vector: vec![0, 0, 0, 0, 0, 0],
                common_names: vec![name.to_string()],
            };
            let info = known_chord_info(
                &chord,
                name.to_string(),
                chord.common_names.clone(),
                pitch_classes,
                "normal",
            );

            assert_eq!(info.chord_symbol.as_deref(), Some(expected_symbol));
            assert!(
                info.chord_symbol
                    .as_deref()
                    .is_some_and(|symbol| symbol.starts_with('C')),
                "{name} should be rooted at C in the browser table"
            );
        }
    }

    #[test]
    fn key_estimate_prefers_chord_root_spelling() {
        let chord = Chord::new("D-4 F4 A-4").unwrap();

        assert_eq!(key_estimate_for_chord(&chord).as_deref(), Some("Db major"));
    }
}
