//! WebAssembly bindings for the browser examples.

use music21_rs::{
    ALL_TUNING_SYSTEMS, Chord, ChordResolutionSuggestion, Error, GuitarTuning, Key, KnownChordType,
    Pitch, Polyrhythm, Result, ScalaArchive, ScalaScale, TuningSystem, abc_chord, abc_duration,
    pitch_class_name,
};
use serde::Serialize;
use std::{collections::BTreeSet, fmt};
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct TuningFrequencyInfo {
    id: String,
    name: String,
    frequency_hz: f64,
    cents_from_equal_temperament: f64,
}

#[derive(Serialize)]
struct PlayableTuningSystemInfo {
    id: String,
    name: String,
    description: &'static str,
}

/// Every built-in tuning system a page can offer: the ratio tables and
/// historical temperaments, and the equal temperaments beyond twelve.
fn all_playable() -> impl Iterator<Item = TuningSystem> {
    ALL_TUNING_SYSTEMS.into_iter().chain(
        music21_rs::tuningsystem::COMMON_EQUAL_TEMPERAMENTS
            .into_iter()
            .filter(|tuning_system| tuning_system.octave_size() != 12),
    )
}

/// An id that tells the equal temperaments apart: the twelve-tone one keeps
/// the bare name, so links that named it still work.
fn tuning_id(tuning_system: TuningSystem) -> String {
    match tuning_system {
        TuningSystem::EqualTemperament { octave_size } if octave_size != 12 => {
            format!("EqualTemperament{octave_size}")
        }
        other => other.id().to_string(),
    }
}

fn tuning_label(tuning_system: TuningSystem) -> String {
    match tuning_system {
        TuningSystem::EqualTemperament { octave_size } if octave_size != 12 => {
            format!("Equal temperament, {octave_size} per octave")
        }
        other => other.display_name().to_string(),
    }
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
struct RomanNumeralInfo {
    figure: String,
    key_context: String,
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
    roman_numeral_context: Option<RomanNumeralInfo>,
    roman_numeral_estimate: Option<RomanNumeralInfo>,
    guitar_fingering: Option<GuitarFingeringInfo>,
    polyrhythm_input: String,
    resolution_chords: Vec<ResolutionChordInfo>,
    pitches: Vec<PitchInfo>,
    abc_notation: String,
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
    roman_numeral_estimate: Option<RomanNumeralInfo>,
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
    id: String,
    name: String,
    description: &'static str,
    family: &'static str,
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
    chord_abc_notation: String,
    rhythm_abc_notation: String,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebAbcClef {
    Treble,
    Bass,
}

impl fmt::Display for WebAbcClef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Treble => f.write_str("treble"),
            Self::Bass => f.write_str("bass"),
        }
    }
}

fn abc_clef_for_pitches(pitches: &[Pitch]) -> WebAbcClef {
    if pitches.is_empty() {
        return WebAbcClef::Treble;
    }

    let midi_values = pitches.iter().map(Pitch::midi).collect::<Vec<_>>();
    let total = midi_values.iter().sum::<i32>();
    let average = total as f64 / midi_values.len() as f64;
    let lowest = midi_values.iter().min().copied().unwrap_or(60);

    if average < 60.0 || lowest < 48 {
        WebAbcClef::Bass
    } else {
        WebAbcClef::Treble
    }
}

fn abc_chord_bar_token(pitches: &[Pitch]) -> Result<String> {
    Ok(format!("{}{}", abc_chord(pitches)?, abc_duration(4, 1)?))
}

fn abc_chord_document(pitches: &[Pitch]) -> Result<String> {
    Ok(format!(
        "X:1\nL:1/4\nM:4/4\nK:C clef={}\n{} |]\n",
        abc_clef_for_pitches(pitches),
        abc_chord_bar_token(pitches)?
    ))
}

fn abc_chord_resolution_document(source: &[Pitch], target: &[Pitch]) -> Result<String> {
    let mut combined = Vec::with_capacity(source.len() + target.len());
    combined.extend_from_slice(source);
    combined.extend_from_slice(target);

    Ok(format!(
        "X:1\nL:1/4\nM:4/4\nK:C clef={}\n{} | {} |]\n",
        abc_clef_for_pitches(&combined),
        abc_chord_bar_token(source)?,
        abc_chord_bar_token(target)?
    ))
}

fn abc_polyrhythm_voice(component: u32, base: u32) -> Result<String> {
    if component == 0 || base == 0 {
        return Err(Error::Polyrhythm(
            "ABC polyrhythm components must be positive".to_string(),
        ));
    }

    if component == base {
        return Ok(std::iter::repeat_n("B", component as usize)
            .collect::<Vec<_>>()
            .join(" "));
    }

    if component == 1 {
        return Ok(format!("B{base}"));
    }

    if component <= 9 {
        let notes = std::iter::repeat_n("B", component as usize)
            .collect::<Vec<_>>()
            .join(" ");
        return Ok(format!("({component}:{base}:{component}{notes}"));
    }

    let duration = abc_duration(base, component)?;
    Ok((0..component)
        .map(|index| {
            let label = if index == 0 {
                format!("\"^{component}:{base}\"")
            } else {
                String::new()
            };
            format!("{label}B{duration}")
        })
        .collect::<Vec<_>>()
        .join(" "))
}

fn abc_polyrhythm_document(components: &[u32], base: u32) -> Result<String> {
    if components.is_empty() {
        return Err(Error::Polyrhythm(
            "ABC polyrhythm document requires at least one component".to_string(),
        ));
    }

    let mut lines = vec![
        "X:1".to_string(),
        "L:1/4".to_string(),
        format!("M:{base}/4"),
        "K:C clef=perc style=x".to_string(),
    ];

    for (index, component) in components.iter().enumerate() {
        lines.push(format!(
            "V:{} name=\"{}\" clef=perc style=x",
            index + 1,
            component
        ));
        lines.push(format!("{} |]", abc_polyrhythm_voice(*component, base)?));
    }

    Ok(format!("{}\n", lines.join("\n")))
}

#[wasm_bindgen]
/// Analyzes a chord input string or MIDI-number list for the chord analyzer page.
pub fn analyze_chord(input: &str) -> Result<JsValue, JsValue> {
    analyze_chord_inner(input, None, None)
}

#[wasm_bindgen]
/// Analyzes a chord input string with an optional key context for resolution suggestions.
pub fn analyze_chord_with_key(input: &str, key_context: &str) -> Result<JsValue, JsValue> {
    analyze_chord_inner(input, Some(key_context), None)
}

#[wasm_bindgen]
/// Analyzes a chord input string with key context and guitar tuning options.
pub fn analyze_chord_with_options(
    input: &str,
    key_context: &str,
    guitar_tuning: &str,
) -> Result<JsValue, JsValue> {
    analyze_chord_inner(input, Some(key_context), Some(guitar_tuning))
}

fn analyze_chord_inner(
    input: &str,
    key_context: Option<&str>,
    guitar_tuning: Option<&str>,
) -> Result<JsValue, JsValue> {
    let chord = chord_from_input(input).map_err(|err| JsValue::from_str(&err.to_string()))?;
    let common_name = chord.common_name();
    let root_pitch_name = chord.root_pitch_name();
    let chord_symbols = chord.chord_symbols();
    let chord_symbol = chord_symbols.first().cloned();
    let key_context = parse_key_context(key_context)?;
    let guitar_tuning = parse_guitar_tuning(guitar_tuning)
        .map_err(|err| JsValue::from_str(&format!("Guitar tuning: {err}")))?;
    let key_context_display = key_context.as_ref().map(display_key_context);
    let estimated_key = estimated_key_for_chord(&chord);
    let key_estimate = estimated_key.as_ref().map(display_key_context);
    let roman_numeral_context = key_context
        .as_ref()
        .and_then(|key| roman_numeral_for_chord(&chord, key));
    let roman_numeral_estimate = estimated_key
        .as_ref()
        .and_then(|key| roman_numeral_for_chord(&chord, key));
    let resolution_chords = match key_context.as_ref() {
        Some(key) => chord.resolution_suggestions_in_key(key),
        None => chord.resolution_suggestions(),
    }
    .map_err(|err| JsValue::from_str(&err.to_string()))?
    .into_iter()
    .map(resolution_chord_info)
    .collect();

    let display_pitches = display_pitches_for_sequence(chord.pitches())
        .map_err(|err| JsValue::from_str(&err.to_string()))?;
    let abc_notation =
        abc_chord_document(&display_pitches).map_err(|err| JsValue::from_str(&err.to_string()))?;
    let pitches = pitch_infos(&display_pitches);
    let guitar_fingering = match guitar_tuning.as_ref() {
        Some(tuning) => chord.guitar_fingering_with_tuning(tuning),
        None => chord.guitar_fingering(),
    };

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
        inversion_name: chord
            .inversion()
            .map(|_| chord.inversion_text().to_lowercase()),
        key_context: key_context_display,
        key_estimate,
        roman_numeral_context,
        roman_numeral_estimate,
        guitar_fingering: guitar_fingering.map(guitar_fingering_info),
        polyrhythm_input: chord.polyrhythm_ratio_string(),
        resolution_chords,
        pitches,
        abc_notation,
    })
    .map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen]
/// Returns the MIDI number represented by a pitch name or integer token.
pub fn pitch_midi_number(input: &str) -> Result<i32, JsValue> {
    parse_pitch_midi_number(input).map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen]
/// Returns a two-bar ABC excerpt showing one chord followed by another.
pub fn chord_resolution_abc(source: &str, target: &str) -> Result<String, JsValue> {
    let source = chord_from_input(source).map_err(|err| JsValue::from_str(&err.to_string()))?;
    let target = chord_from_input(target).map_err(|err| JsValue::from_str(&err.to_string()))?;
    let source_pitches = display_pitches_for_sequence(source.pitches())
        .map_err(|err| JsValue::from_str(&err.to_string()))?;
    let target_pitches = display_pitches_for_sequence(target.pitches())
        .map_err(|err| JsValue::from_str(&err.to_string()))?;

    abc_chord_resolution_document(&source_pitches, &target_pitches)
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
    let estimated_key = realized_chord.as_ref().and_then(estimated_key_for_chord);
    let key_estimate = estimated_key.as_ref().map(display_key_context);
    let roman_numeral_estimate = realized_chord
        .as_ref()
        .zip(estimated_key.as_ref())
        .and_then(|(chord, key)| {
            pitch_names
                .first()
                .and_then(|name| Pitch::from_name(name).ok())
                .and_then(|root| roman_numeral_for_chord_with_root(chord, key, &root))
        });
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
        roman_numeral_estimate,
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

    let systems = all_playable()
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
                id: tuning_id(tuning_system),
                name: tuning_label(tuning_system),
                description: tuning_system.description(),
                family: tuning_family(tuning_system),
                octave_size,
                root_frequency_hz,
                degrees,
            }
        })
        .collect::<Vec<_>>();

    serde_wasm_bindgen::to_value(&systems).map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen]
/// Returns every built-in tuning system a chord can be sounded in: the
/// twelve-tone ones by degree, the rest by the nearest degree in cents.
pub fn playable_tuning_systems() -> Result<JsValue, JsValue> {
    let systems = all_playable()
        .map(|tuning_system| PlayableTuningSystemInfo {
            id: tuning_id(tuning_system),
            name: tuning_label(tuning_system),
            description: tuning_system.description(),
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
    let ratio_pitches = polyrhythm
        .ratio_pitches(root)
        .map_err(|err| JsValue::from_str(&err.to_string()))?;
    let chord_abc_notation =
        abc_chord_document(&ratio_pitches).map_err(|err| JsValue::from_str(&err.to_string()))?;
    let rhythm_abc_notation = abc_polyrhythm_document(&analysis.components, analysis.base)
        .map_err(|err| JsValue::from_str(&err.to_string()))?;
    let pitches = ratio_pitches
        .iter()
        .map(Pitch::name_with_octave)
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
        chord_abc_notation,
        rhythm_abc_notation,
    })
    .map_err(|err| JsValue::from_str(&err.to_string()))
}

fn chord_from_input(input: &str) -> Result<Chord> {
    if let Some(midi_numbers) = parse_midi_input(input) {
        Chord::new(midi_numbers.as_slice())
    } else {
        input.parse()
    }
}

fn display_pitches_for_sequence(pitches: Vec<Pitch>) -> Result<Vec<Pitch>> {
    let mut last_pitch_space: Option<i32> = None;
    pitches
        .into_iter()
        .map(|pitch| display_pitch_for_sequence(pitch, &mut last_pitch_space))
        .collect()
}

fn pitch_infos(pitches: &[Pitch]) -> Vec<PitchInfo> {
    pitches
        .iter()
        .enumerate()
        .map(|(index, display_pitch)| {
            let pitch_space = display_pitch.ps();
            let tuning_frequencies = all_playable()
                .map(|tuning_system| {
                    let frequency_hz = frequency_in_system(tuning_system, pitch_space);
                    TuningFrequencyInfo {
                        id: tuning_id(tuning_system),
                        name: tuning_label(tuning_system),
                        frequency_hz,
                        cents_from_equal_temperament: 1200.0
                            * (frequency_hz / display_pitch.frequency_hz()).log2(),
                    }
                })
                .collect::<Vec<_>>();

            PitchInfo {
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
            }
        })
        .collect()
}

/// Which shelf a tuning system sits on in the explorer.
fn tuning_family(tuning_system: TuningSystem) -> &'static str {
    use music21_rs::tuningsystem::{
        COMMON_EQUAL_TEMPERAMENTS, COMMON_TWELVE_TONE_TUNING_SYSTEMS, HISTORICAL_TEMPERAMENTS,
    };
    if HISTORICAL_TEMPERAMENTS.contains(&tuning_system) {
        "Historical temperaments"
    } else if COMMON_EQUAL_TEMPERAMENTS.contains(&tuning_system) {
        "Equal temperaments"
    } else if COMMON_TWELVE_TONE_TUNING_SYSTEMS.contains(&tuning_system) {
        "Twelve-tone systems"
    } else {
        "Just intonation and others"
    }
}

#[derive(Serialize)]
struct GeneratorInfo {
    ratio: &'static str,
    cents: f64,
}

#[derive(Serialize)]
struct TemperamentInfo {
    name: &'static str,
    page: &'static str,
    subgroup: String,
    rank: usize,
    periods_per_equave: u32,
    equave: String,
    equave_cents: f64,
    generators: Vec<GeneratorInfo>,
    optimization: &'static str,
    commas: Vec<&'static str>,
    published_moments: Vec<&'static str>,
    /// The moment-of-symmetry sizes up to forty notes, for a rank-2
    /// temperament; a rank-3 one stacks two generators and has none.
    moments: Vec<u32>,
}

#[wasm_bindgen]
/// Lists the regular temperaments the crate carries from the Xenharmonic
/// Wiki, with the scale sizes each one makes.
pub fn regular_temperaments() -> Result<JsValue, JsValue> {
    use music21_rs::tuningsystem::WIKI_TEMPERAMENTS;
    let infos = WIKI_TEMPERAMENTS
        .iter()
        .map(|named| {
            let temperament = named.temperament().ok();
            let moments = temperament
                .as_ref()
                .and_then(|temperament| temperament.moments(40).ok())
                .unwrap_or_default();
            let equave_cents = temperament
                .as_ref()
                .map(|temperament| temperament.equave_cents())
                .unwrap_or(1200.0);
            TemperamentInfo {
                name: named.name,
                page: named.page,
                subgroup: named
                    .subgroup
                    .iter()
                    .map(|prime| prime.to_string())
                    .collect::<Vec<_>>()
                    .join("."),
                rank: named.generator_rows.len() + 1,
                periods_per_equave: named.periods_per_equave,
                equave: format!("{}/1", named.subgroup.first().copied().unwrap_or(2)),
                equave_cents,
                generators: named
                    .generator_ratios
                    .iter()
                    .zip(named.generator_cents.iter())
                    .map(|(ratio, cents)| GeneratorInfo {
                        ratio,
                        cents: *cents,
                    })
                    .collect(),
                optimization: named.optimization,
                commas: named.commas.to_vec(),
                published_moments: named.moments.to_vec(),
                moments,
            }
        })
        .collect::<Vec<_>>();
    serde_wasm_bindgen::to_value(&infos).map_err(|err| JsValue::from_str(&err.to_string()))
}

/// A scale given as cents above a root, in the shape the explorer renders
/// every scale in.
fn scale_from_cents(
    file: String,
    description: String,
    cents: &[f64],
    period_cents: f64,
    root_frequency_hz: f64,
    label: impl Fn(usize, f64) -> String,
) -> ScalaScaleInfo {
    let degrees = cents
        .iter()
        .enumerate()
        .map(|(degree, cents)| ScalaDegreeInfo {
            degree,
            label: label(degree, *cents),
            is_exact_ratio: false,
            ratio: (2.0_f64).powf(cents / 1200.0),
            cents: *cents,
            frequency_hz: root_frequency_hz * (2.0_f64).powf(cents / 1200.0),
        })
        .collect::<Vec<_>>();
    ScalaScaleInfo {
        file,
        description,
        degree_count: degrees.len().saturating_sub(1),
        period_ratio: (2.0_f64).powf(period_cents / 1200.0),
        period_cents,
        root_frequency_hz,
        degrees,
    }
}

#[wasm_bindgen]
/// Realizes a regular temperament as a moment-of-symmetry scale of `notes`
/// notes to the equave, from a root frequency.
pub fn temperament_scale(
    name: &str,
    notes: u32,
    root_frequency_hz: f64,
) -> Result<JsValue, JsValue> {
    use music21_rs::tuningsystem::WIKI_TEMPERAMENTS;
    check_root_frequency(root_frequency_hz)?;
    let named = WIKI_TEMPERAMENTS
        .iter()
        .find(|named| named.name == name)
        .ok_or_else(|| JsValue::from_str(&format!("no temperament named {name}")))?;
    let temperament = named
        .temperament()
        .map_err(|err| JsValue::from_str(&err.to_string()))?;
    let scale = temperament
        .mos(notes)
        .map_err(|err| JsValue::from_str(&err.to_string()))?;
    let periods = temperament.periods_per_equave().unsigned_abs().max(1);
    let period_cents = temperament.period_cents();
    let mut cents = Vec::new();
    for period in 0..periods {
        for degree in scale.degrees() {
            cents.push(degree + f64::from(period) * period_cents);
        }
    }
    cents.push(temperament.equave_cents());
    let pattern = scale
        .word()
        .map(|word| format!(", pattern {word}"))
        .unwrap_or_default();
    let generators = named
        .generator_ratios
        .iter()
        .zip(named.generator_cents.iter())
        .map(|(ratio, cents)| format!("{ratio} ({cents:.3}c)"))
        .collect::<Vec<_>>()
        .join(", ");
    let info = scale_from_cents(
        format!("temperament:{name}:{notes}"),
        format!(
            "{name} temperament in the {} subgroup, {notes} notes to the equave{pattern}; generator {generators}",
            temperament.subgroup_name()
        ),
        &cents,
        temperament.equave_cents(),
        root_frequency_hz,
        |_, cents| format!("{cents:.1}c"),
    );
    serde_wasm_bindgen::to_value(&info).map_err(|err| JsValue::from_str(&err.to_string()))
}

#[derive(Serialize)]
struct EqualDivisionPreset {
    label: String,
    divisions: u32,
    numerator: i32,
    denominator: i32,
    note: &'static str,
}

#[wasm_bindgen]
/// The equal divisions worth a shelf of their own: the common EDOs, and the
/// divisions of other intervals the literature names.
pub fn equal_division_presets() -> Result<JsValue, JsValue> {
    let presets = [
        (5, 2, 1, "five equal steps, the Javanese slendro's shape"),
        (7, 2, 1, "seven equal steps, the Thai scale's shape"),
        (12, 2, 1, "the twelve-tone equal temperament"),
        (15, 2, 1, "Blackwood's decatonic playground"),
        (17, 2, 1, "a neutral-third system with a sharp fifth"),
        (19, 2, 1, "a meantone system where the diesis is one step"),
        (
            22,
            2,
            1,
            "the Indian shruti count, and a superpyth and porcupine tuning",
        ),
        (24, 2, 1, "quarter tones"),
        (31, 2, 1, "Huygens and Fokker's near-quarter-comma meantone"),
        (41, 2, 1, "a near-Pythagorean system with good sevenths"),
        (53, 2, 1, "the Holderian comma, close to just intonation"),
        (72, 2, 1, "twelfth tones, a superset of 12, 24 and 36"),
        (13, 3, 1, "Bohlen-Pierce, thirteen steps of a twelfth"),
        (9, 3, 2, "Wendy Carlos's alpha, 78 cents a step"),
        (11, 3, 2, "Wendy Carlos's beta, 63.8 cents a step"),
        (20, 3, 2, "Wendy Carlos's gamma, 35.1 cents a step"),
        (8, 5, 2, "eight steps of a major tenth"),
    ]
    .into_iter()
    .map(|(divisions, numerator, denominator, note)| {
        let label =
            music21_rs::tuningsystem::EqualDivision::of_ratio(divisions, numerator, denominator)
                .map(|division| division.to_string())
                .unwrap_or_else(|_| format!("{divisions}ed{numerator}/{denominator}"));
        EqualDivisionPreset {
            label,
            divisions,
            numerator,
            denominator,
            note,
        }
    })
    .collect::<Vec<_>>();
    serde_wasm_bindgen::to_value(&presets).map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen]
/// Realizes an equal division of any interval, `divisions` equal steps of
/// `numerator/denominator`, from a root frequency.
pub fn equal_division_scale(
    divisions: u32,
    numerator: i32,
    denominator: i32,
    root_frequency_hz: f64,
) -> Result<JsValue, JsValue> {
    use music21_rs::tuningsystem::EqualDivision;
    check_root_frequency(root_frequency_hz)?;
    let division = EqualDivision::of_ratio(divisions, numerator, denominator)
        .map_err(|err| JsValue::from_str(&err.to_string()))?;
    let cents = (0..=divisions)
        .map(|degree| division.cents_at(degree as i32))
        .collect::<Vec<_>>();
    let info = scale_from_cents(
        division.to_string(),
        format!(
            "{divisions} equal steps of {numerator}/{denominator}, {:.3} cents each",
            division.step_cents()
        ),
        &cents,
        division.period_cents(),
        root_frequency_hz,
        |degree, _| format!("{degree}\\{divisions}"),
    );
    serde_wasm_bindgen::to_value(&info).map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen]
/// The frequency recursive just intonation sounds for a key, given the key
/// its context stands on: both are degrees of the twelve-tone keyboard above
/// C4, and the root frequency is C4's.
pub fn adaptive_frequency(
    context_degree: f64,
    degree: f64,
    root_frequency_hz: f64,
) -> Result<f64, JsValue> {
    use music21_rs::tuningsystem::{C4, adaptive::RECURSIVE_JI};
    check_root_frequency(root_frequency_hz)?;
    if !context_degree.is_finite() || !degree.is_finite() {
        return Err(JsValue::from_str("degrees must be finite"));
    }
    // The adaptive systems count degrees from C-1, five octaves below C4.
    let base = 60.0;
    let frequency = RECURSIVE_JI.frequency_at(context_degree + base, degree + base, None);
    Ok(frequency * root_frequency_hz / C4)
}

/// A pitch's frequency in a tuning system, which for a system of twelve
/// degrees is its own degree and for any other the degree nearest it in
/// cents within the octave.
fn frequency_in_system(tuning_system: TuningSystem, pitch_space: f64) -> f64 {
    let octave_size = tuning_system.octave_size();
    if octave_size == 12 {
        return tuning_system.frequency_at(pitch_space);
    }
    let cents = pitch_space.rem_euclid(12.0) * 100.0;
    let nearest = (0..octave_size)
        .min_by(|a, b| {
            let distance = |degree: &u32| {
                (1200.0 * tuning_system.fraction(*degree as usize).ratio().log2() - cents).abs()
            };
            distance(a)
                .partial_cmp(&distance(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(0);
    let octave = pitch_space.div_euclid(12.0);
    music21_rs::tuningsystem::CN1
        * (2.0_f64).powf(octave)
        * tuning_system.fraction(nearest as usize).ratio()
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

fn parse_pitch_midi_number(input: &str) -> Result<i32> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(Error::Pitch("pitch token cannot be empty".to_string()));
    }
    if trimmed.chars().all(|ch| ch == '-' || ch.is_ascii_digit()) {
        return trimmed
            .parse::<i32>()
            .map_err(|err| Error::Pitch(format!("invalid MIDI number {trimmed:?}: {err}")));
    }

    Pitch::from_name(trimmed).map(|pitch| pitch.midi())
}

fn parse_guitar_tuning(input: Option<&str>) -> Result<Option<GuitarTuning>> {
    let Some(input) = input.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let strings = input
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();

    GuitarTuning::new(strings).map(Some)
}

fn estimated_key_for_chord(chord: &Chord) -> Option<Key> {
    let estimated_key = music21_rs::estimate_key_from_chords(std::slice::from_ref(chord))
        .ok()?
        .first()
        .map(|estimate| estimate.key().clone())?;
    let estimated_tonic = estimated_key.tonic();
    let mode = estimated_key.mode().to_string();
    let respelled_key = chord
        .root_pitch_name()
        .and_then(|root_name| {
            let root = Pitch::from_name(&root_name).ok()?;
            same_pitch_class(&root, &estimated_tonic).then_some(root_name)
        })
        .and_then(|root_name| {
            Key::from_tonic_mode(&display_pitch_name(&root_name), Some(mode.as_str())).ok()
        });
    Some(respelled_key.unwrap_or(estimated_key))
}

fn roman_numeral_for_chord(chord: &Chord, key: &Key) -> Option<RomanNumeralInfo> {
    music21_rs::analyze_chord(chord, key.clone())
        .ok()
        .flatten()
        .map(roman_numeral_info)
}

fn roman_numeral_for_chord_with_root(
    chord: &Chord,
    key: &Key,
    root: &Pitch,
) -> Option<RomanNumeralInfo> {
    music21_rs::analyze_chord_with_root(chord, key.clone(), root)
        .ok()
        .flatten()
        .map(roman_numeral_info)
}

fn roman_numeral_info(roman_numeral: music21_rs::RomanNumeral) -> RomanNumeralInfo {
    RomanNumeralInfo {
        figure: roman_numeral.figure().to_string(),
        key_context: display_key_context(roman_numeral.key()),
    }
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
    use super::{
        chord_from_input, display_key_context, display_pitches_for_sequence,
        estimated_key_for_chord, known_chord_info, parse_guitar_tuning, parse_midi_input,
        parse_pitch_midi_number, pitch_infos,
    };
    use music21_rs::{Chord, KnownChordType, Pitch};

    #[test]
    fn parse_midi_input_accepts_plain_prefixed_and_csv_values() {
        assert_eq!(parse_midi_input("60 64 67"), Some(vec![60, 64, 67]));
        assert_eq!(parse_midi_input("midi: 60,64,67"), Some(vec![60, 64, 67]));
        assert_eq!(parse_midi_input("MIDI 60 64 67"), Some(vec![60, 64, 67]));
        assert_eq!(parse_midi_input("C E G"), None);
    }

    #[test]
    fn chord_input_helpers_parse_midi_and_pitch_names() {
        assert_eq!(
            chord_from_input("midi: 60 64 67").unwrap().pitch_classes(),
            vec![0, 4, 7]
        );
        assert_eq!(parse_pitch_midi_number("72").unwrap(), 72);
        assert_eq!(parse_pitch_midi_number("C5").unwrap(), 72);
        assert!(parse_pitch_midi_number("not-a-pitch").is_err());
    }

    #[test]
    fn parse_guitar_tuning_accepts_custom_pitch_lists() {
        let tuning = parse_guitar_tuning(Some("D2, A2 D3 G3 A3 D4"))
            .unwrap()
            .unwrap();

        assert_eq!(tuning.strings().len(), 6);
        assert_eq!(tuning.strings()[0].name, "D2");
        assert_eq!(tuning.strings()[5].name, "D4");
        assert!(parse_guitar_tuning(Some("")).unwrap().is_none());
        assert!(parse_guitar_tuning(Some("not-a-pitch")).is_err());
    }

    #[test]
    fn pitch_infos_include_every_tuning_system() {
        let pitch: Pitch = "C4".parse().unwrap();
        let infos = pitch_infos(&[pitch]);
        let ids = infos[0]
            .tuning_frequencies
            .iter()
            .map(|tuning| tuning.id.clone())
            .collect::<Vec<_>>();
        let expected_ids = super::all_playable()
            .map(super::tuning_id)
            .collect::<Vec<_>>();
        assert_eq!(ids, expected_ids);
        // A system with more than twelve degrees sounds the nearest one: a
        // major third in 19-EDO is six steps, 378.9 cents.
        let nineteen = infos[0]
            .tuning_frequencies
            .iter()
            .find(|tuning| tuning.id == "EqualTemperament19")
            .expect("19-EDO is a tuning system");
        assert!((nineteen.cents_from_equal_temperament).abs() < 1e-6);
    }

    #[test]
    fn display_pitches_for_sequence_adds_concrete_octaves() {
        let chord = Chord::new("C D E").unwrap();
        let names = display_pitches_for_sequence(chord.pitches())
            .unwrap()
            .into_iter()
            .map(|pitch| pitch.name_with_octave())
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["C4", "D4", "E4"]);
    }

    #[test]
    fn known_chord_info_includes_symbol_key_estimate_and_roman_numeral() {
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
        assert_eq!(
            info.roman_numeral_estimate
                .as_ref()
                .map(|roman| roman.figure.as_str()),
            Some("I")
        );
        assert_eq!(info.inversion_labels.len(), 3);
        assert!(info.resolution_chords.is_empty());
    }

    #[test]
    fn known_chord_info_uses_music21_figures_with_c_root() {
        let cases = [
            ("major triad", vec![0, 4, 7], Some("C")),
            ("dominant seventh chord", vec![0, 4, 7, 10], Some("C7")),
            ("power chord", vec![0, 7], Some("Cpower")),
            ("major sixth", vec![0, 9], None),
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

            assert_eq!(info.chord_symbol.as_deref(), expected_symbol);
            if expected_symbol.is_some() {
                assert!(
                    info.chord_symbol
                        .as_deref()
                        .is_some_and(|symbol| symbol.starts_with('C')),
                    "{name} should be rooted at C in the browser table"
                );
            }
        }
    }

    #[test]
    fn known_chord_info_labels_augmented_sixth_functionally() {
        let chord = KnownChordType {
            cardinality: 4,
            forte_class: "4-25".to_string(),
            normal_form: vec![0, 2, 6, 8],
            interval_class_vector: vec![0, 2, 0, 2, 0, 2],
            common_names: vec![
                "Messiaen's truncated mode 6".to_string(),
                "French augmented sixth chord".to_string(),
            ],
        };
        let info = known_chord_info(
            &chord,
            "Messiaen's truncated mode 6".to_string(),
            chord.common_names.clone(),
            chord.normal_form.clone(),
            "normal",
        );

        assert_eq!(info.key_estimate.as_deref(), Some("C minor"));
        assert_eq!(
            info.roman_numeral_estimate
                .as_ref()
                .map(|roman| roman.figure.as_str()),
            Some("Fr+6")
        );
    }

    #[test]
    fn key_estimate_prefers_chord_root_spelling() {
        let chord = Chord::new("D-4 F4 A-4").unwrap();

        assert_eq!(
            estimated_key_for_chord(&chord)
                .as_ref()
                .map(display_key_context)
                .as_deref(),
            Some("Db major")
        );
    }
}

/// One entry in the Scala archive listing, without its degrees.
///
/// Kept light on purpose: the browser lists thousands of these, and pulling the
/// degrees for all of them would move megabytes across the wasm boundary for a
/// list the user mostly scrolls past.
#[derive(Serialize)]
struct ScalaScaleSummary {
    file: String,
    description: String,
    degree_count: usize,
}

/// A realized Scala scale, with one entry per degree plus the closing period.
#[derive(Serialize)]
struct ScalaScaleInfo {
    file: String,
    description: String,
    degree_count: usize,
    period_ratio: f64,
    period_cents: f64,
    root_frequency_hz: f64,
    degrees: Vec<ScalaDegreeInfo>,
}

#[derive(Serialize)]
struct ScalaDegreeInfo {
    degree: usize,
    /// `"3/2"` for an exact ratio, or the cents value the file gave.
    label: String,
    /// Whether the archive wrote this degree as an exact ratio.
    is_exact_ratio: bool,
    ratio: f64,
    cents: f64,
    frequency_hz: f64,
}

fn archive() -> &'static ScalaArchive {
    use std::sync::OnceLock;
    static ARCHIVE: OnceLock<ScalaArchive> = OnceLock::new();
    ARCHIVE.get_or_init(ScalaArchive::bundled)
}

fn describe_scale(file: &str, scale: &ScalaScale, root_frequency_hz: f64) -> ScalaScaleInfo {
    let degrees = scale
        .degrees()
        .iter()
        .enumerate()
        .map(|(degree, entry)| ScalaDegreeInfo {
            degree,
            label: entry.to_string(),
            is_exact_ratio: entry.as_fraction().is_some(),
            ratio: entry.ratio(),
            cents: entry.cents(),
            frequency_hz: root_frequency_hz * entry.ratio(),
        })
        .collect();

    ScalaScaleInfo {
        file: file.to_string(),
        description: scale.description().to_string(),
        degree_count: scale.len(),
        period_ratio: scale.period().ratio(),
        period_cents: scale.period().cents(),
        root_frequency_hz,
        degrees,
    }
}

fn check_root_frequency(root_frequency_hz: f64) -> Result<(), JsValue> {
    if !root_frequency_hz.is_finite() || root_frequency_hz <= 0.0 {
        return Err(JsValue::from_str(
            "root frequency must be a positive number",
        ));
    }
    Ok(())
}

#[wasm_bindgen]
/// Returns how many Scala scales are bundled with the crate.
pub fn scala_archive_len() -> usize {
    archive().len()
}

#[wasm_bindgen]
/// Lists bundled Scala scales, optionally filtered by a search string.
///
/// An empty `query` lists everything. Matching follows music21's
/// `scale.scala.search`: spaces are ignored, an exact file name wins, and the
/// rest are substring hits against the name with and without separators.
pub fn scala_archive_index(query: &str, limit: usize) -> std::result::Result<JsValue, JsValue> {
    let archive = archive();
    let names: Vec<&str> = if query.trim().is_empty() {
        archive.names().collect()
    } else {
        archive.search(query)
    };

    let summaries = names
        .into_iter()
        .take(if limit == 0 { usize::MAX } else { limit })
        .filter_map(|file| {
            let scale = archive.get(file)?;
            Some(ScalaScaleSummary {
                file: file.to_string(),
                description: scale.description().to_string(),
                degree_count: scale.len(),
            })
        })
        .collect::<Vec<_>>();

    serde_wasm_bindgen::to_value(&summaries).map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen]
/// Realizes one bundled Scala scale from a root frequency.
pub fn scala_scale(file: &str, root_frequency_hz: f64) -> std::result::Result<JsValue, JsValue> {
    check_root_frequency(root_frequency_hz)?;
    let scale = archive()
        .get(file)
        .ok_or_else(|| JsValue::from_str(&format!("no bundled scale named {file}")))?;
    let info = describe_scale(file, scale, root_frequency_hz);
    serde_wasm_bindgen::to_value(&info).map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen]
/// Realizes a Scala scale from `.scl` text the user supplied.
///
/// Takes the file's text rather than a path: the browser has no filesystem, and
/// the crate does no IO of its own either.
pub fn parse_scala_scale(
    file_name: &str,
    contents: &str,
    root_frequency_hz: f64,
) -> std::result::Result<JsValue, JsValue> {
    check_root_frequency(root_frequency_hz)?;
    let scale = ScalaScale::parse(contents)
        .map_err(|error| JsValue::from_str(&format!("{file_name}: {error}")))?;
    let info = describe_scale(file_name, &scale, root_frequency_hz);
    serde_wasm_bindgen::to_value(&info).map_err(|err| JsValue::from_str(&err.to_string()))
}

// ---------------------------------------------------------------- Harte

#[derive(Serialize)]
struct HarteInfo {
    label: String,
    canonical: String,
    pretty: String,
    is_empty: bool,
    root: Option<String>,
    bass: Option<String>,
    shorthand: Option<String>,
    written_degrees: Vec<String>,
    sounding_degrees: Vec<String>,
    pitches: Vec<String>,
    midi: Vec<i32>,
    multi_hot: Vec<u8>,
    multi_hot_from_root: Vec<u8>,
    common_name: String,
    pitched_common_name: String,
    forte_class: Option<String>,
    chord_symbol: Option<String>,
}

#[wasm_bindgen]
/// Reads a chord label in Harte notation and describes the chord it sounds.
pub fn harte_chord(label: &str) -> Result<JsValue, JsValue> {
    let harte = music21_rs::Harte::new(label).map_err(|err| JsValue::from_str(&err.to_string()))?;
    let chord = harte.chord();
    let info = HarteInfo {
        label: label.to_string(),
        canonical: harte.to_string(),
        pretty: harte.prettify(),
        is_empty: harte.is_empty(),
        root: harte.root_name().map(str::to_string),
        bass: harte.bass_degree().map(str::to_string),
        shorthand: harte.shorthand().map(str::to_string),
        written_degrees: harte.degrees().map(<[String]>::to_vec).unwrap_or_default(),
        sounding_degrees: harte.sounding_degrees().to_vec(),
        pitches: chord
            .pitches()
            .iter()
            .map(Pitch::name_with_octave)
            .collect(),
        midi: harte.midi_pitches(),
        multi_hot: harte.multi_hot_encoding(false).to_vec(),
        multi_hot_from_root: harte.multi_hot_encoding(true).to_vec(),
        common_name: chord.common_name(),
        pitched_common_name: chord.pitched_common_name(),
        forte_class: chord.forte_class(),
        chord_symbol: chord.chord_symbol(),
    };
    serde_wasm_bindgen::to_value(&info).map_err(|err| JsValue::from_str(&err.to_string()))
}

#[derive(Serialize)]
struct ShorthandInfo {
    name: &'static str,
    degrees: Vec<&'static str>,
}

#[wasm_bindgen]
/// The shorthands Harte notation knows, each with the degrees it stands for.
pub fn harte_shorthands() -> Result<JsValue, JsValue> {
    let table = music21_rs::harte::SHORTHAND_DEGREES
        .iter()
        .map(|(name, degrees)| ShorthandInfo {
            name,
            degrees: degrees.to_vec(),
        })
        .collect::<Vec<_>>();
    serde_wasm_bindgen::to_value(&table).map_err(|err| JsValue::from_str(&err.to_string()))
}

// ---------------------------------------------------------------- Roman numerals

#[derive(Serialize)]
struct RomanInfo {
    figure: String,
    roman_numeral: String,
    degree: u8,
    alteration: i8,
    inversion: u8,
    quality: String,
    key: String,
    pitches: Vec<String>,
    pitch_names: Vec<String>,
    common_name: String,
    pitched_common_name: String,
    functionality_score: u8,
    is_neapolitan: bool,
    is_mixture: bool,
}

fn key_from_text(key: &str) -> Result<Key, JsValue> {
    Key::from_tonic(key.trim()).map_err(|err| JsValue::from_str(&format!("Key {key:?}: {err}")))
}

fn describe_numeral(numeral: &music21_rs::RomanNumeral) -> Result<RomanInfo, JsValue> {
    let chord = numeral
        .to_chord()
        .map_err(|err| JsValue::from_str(&err.to_string()))?;
    let (degree, alteration) = numeral.scale_degree_with_alteration();
    let key = numeral.key();
    Ok(RomanInfo {
        figure: numeral.figure().to_string(),
        roman_numeral: numeral.roman_numeral(),
        degree,
        alteration,
        inversion: numeral.inversion(),
        quality: format!("{:?}", numeral.implied_quality()).to_lowercase(),
        key: format!("{} {}", key.tonic_pitch_name_with_case(), key.mode()),
        pitches: chord
            .pitches()
            .iter()
            .map(Pitch::name_with_octave)
            .collect(),
        pitch_names: chord.pitch_names(),
        common_name: chord.common_name(),
        pitched_common_name: chord.pitched_common_name(),
        functionality_score: numeral.functionality_score(),
        is_neapolitan: numeral.is_neapolitan(false),
        is_mixture: numeral.is_mixture(true).unwrap_or(false),
    })
}

#[wasm_bindgen]
/// Realizes a roman numeral figure in a key: `V65/V` in `g`, say.
pub fn realize_roman(figure: &str, key: &str) -> Result<JsValue, JsValue> {
    let numeral = music21_rs::RomanNumeral::new(figure.trim(), key_from_text(key)?)
        .map_err(|err| JsValue::from_str(&err.to_string()))?;
    let info = describe_numeral(&numeral)?;
    serde_wasm_bindgen::to_value(&info).map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen]
/// Names a chord as a roman numeral in a key, the way music21's
/// `romanNumeralFromChord` names it; with no key the chord's root is the key.
pub fn roman_from_chord(chord: &str, key: &str) -> Result<JsValue, JsValue> {
    let chord = Chord::new(chord.trim()).map_err(|err| JsValue::from_str(&err.to_string()))?;
    let key = if key.trim().is_empty() {
        None
    } else {
        Some(key_from_text(key)?)
    };
    let numeral = music21_rs::roman_numeral_from_chord(&chord, key.as_ref())
        .map_err(|err| JsValue::from_str(&err.to_string()))?
        .ok_or_else(|| JsValue::from_str("the chord has no root to name"))?;
    let info = describe_numeral(&numeral)?;
    serde_wasm_bindgen::to_value(&info).map_err(|err| JsValue::from_str(&err.to_string()))
}

// ---------------------------------------------------------------- Scales

#[derive(Serialize)]
struct ScaleTypeInfo {
    id: &'static str,
    name: &'static str,
    steps: usize,
}

#[wasm_bindgen]
/// The scale types the crate realizes, by music21's class names.
pub fn scale_types() -> Result<JsValue, JsValue> {
    use music21_rs::scale::{Scale, ScaleType};
    let infos = ScaleType::ALL
        .iter()
        .map(|scale_type| ScaleTypeInfo {
            id: scale_type.music21_name(),
            name: scale_type.music21_descriptive_name(),
            steps: Scale::new(*scale_type, Pitch::from_name("C4").expect("a pitch"))
                .pitches()
                .map(|pitches| pitches.len().saturating_sub(1))
                .unwrap_or(0),
        })
        .collect::<Vec<_>>();
    serde_wasm_bindgen::to_value(&infos).map_err(|err| JsValue::from_str(&err.to_string()))
}

#[derive(Serialize)]
struct ScaleInfo {
    id: &'static str,
    name: &'static str,
    tonic: String,
    pitches: Vec<String>,
    pitch_names: Vec<String>,
    degrees: Vec<u8>,
    steps: Vec<String>,
}

fn scale_type_named(id: &str) -> Result<music21_rs::scale::ScaleType, JsValue> {
    music21_rs::scale::ScaleType::ALL
        .into_iter()
        .find(|scale_type| scale_type.music21_name() == id)
        .ok_or_else(|| JsValue::from_str(&format!("no scale type named {id}")))
}

fn describe_realized(scale: &music21_rs::scale::Scale) -> Result<ScaleInfo, JsValue> {
    let pitches = scale
        .pitches()
        .map_err(|err| JsValue::from_str(&err.to_string()))?;
    let steps = pitches
        .windows(2)
        .map(|pair| {
            music21_rs::Interval::between_pitches(&pair[0], &pair[1])
                .map(|interval| interval.directed_name())
                .unwrap_or_default()
        })
        .collect();
    Ok(ScaleInfo {
        id: scale.scale_type().music21_name(),
        name: scale.scale_type().music21_descriptive_name(),
        tonic: scale.tonic().name_with_octave(),
        pitch_names: pitches.iter().map(Pitch::name).collect(),
        pitches: pitches.iter().map(Pitch::name_with_octave).collect(),
        degrees: scale
            .named_degrees()
            .unwrap_or_default()
            .into_iter()
            .map(|degree| degree as u8)
            .collect(),
        steps,
    })
}

#[wasm_bindgen]
/// Realizes a scale type from a tonic, over one octave.
pub fn realize_scale(scale_type: &str, tonic: &str) -> Result<JsValue, JsValue> {
    let tonic =
        Pitch::from_name(tonic.trim()).map_err(|err| JsValue::from_str(&err.to_string()))?;
    let tonic = if tonic.octave().is_none() {
        Pitch::from_name(format!("{}4", tonic.name()))
            .map_err(|err| JsValue::from_str(&err.to_string()))?
    } else {
        tonic
    };
    let scale = music21_rs::scale::Scale::new(scale_type_named(scale_type)?, tonic);
    let info = describe_realized(&scale)?;
    serde_wasm_bindgen::to_value(&info).map_err(|err| JsValue::from_str(&err.to_string()))
}

#[derive(Serialize)]
struct DerivedScaleInfo {
    matched: usize,
    total: usize,
    scale: ScaleInfo,
}

#[wasm_bindgen]
/// The scales a set of pitches fits best, every type tried from every tonic
/// and ranked by how many of the pitches each holds.
pub fn derive_scales(pitches: &str, limit: usize) -> Result<JsValue, JsValue> {
    let pitches = pitches
        .split_whitespace()
        .map(Pitch::from_name)
        .collect::<Result<Vec<_>>>()
        .map_err(|err| JsValue::from_str(&err.to_string()))?;
    if pitches.is_empty() {
        return Err(JsValue::from_str("give some pitches, such as C E G B"));
    }
    let mut found = Vec::new();
    for scale_type in music21_rs::scale::ScaleType::ALL {
        let ranked = scale_type
            .derive_ranked(&pitches, Some(3))
            .map_err(|err| JsValue::from_str(&err.to_string()))?;
        for (matched, scale) in ranked {
            found.push(DerivedScaleInfo {
                matched,
                total: pitches.len(),
                scale: describe_realized(&scale)?,
            });
        }
    }
    found.sort_by(|a, b| {
        b.matched
            .cmp(&a.matched)
            .then_with(|| a.scale.pitch_names.len().cmp(&b.scale.pitch_names.len()))
    });
    found.truncate(if limit == 0 { usize::MAX } else { limit });
    serde_wasm_bindgen::to_value(&found).map_err(|err| JsValue::from_str(&err.to_string()))
}

// ---------------------------------------------------------------- Tone rows

#[derive(Serialize)]
struct RowFormInfo {
    label: String,
    pitch_classes: Vec<u8>,
    names: Vec<String>,
}

#[derive(Serialize)]
struct LinkInfo {
    number: u32,
    special_intervals: Vec<&'static str>,
}

#[derive(Serialize)]
struct HistoricalMatch {
    name: &'static str,
    composer: &'static str,
    opus: Option<&'static str>,
    title: &'static str,
}

#[derive(Serialize)]
struct ToneRowInfo {
    pitch_classes: Vec<u8>,
    names: Vec<String>,
    is_twelve_tone_row: bool,
    intervals: String,
    matrix: Vec<Vec<u8>>,
    forms: Vec<RowFormInfo>,
    is_all_interval: Option<bool>,
    link: Option<LinkInfo>,
    historical: Vec<HistoricalMatch>,
}

fn parse_row(input: &str) -> Result<music21_rs::serial::ToneRow, JsValue> {
    use music21_rs::serial::ToneRow;
    let tokens: Vec<&str> = input
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|token| !token.is_empty())
        .collect();
    if tokens.is_empty() {
        return Err(JsValue::from_str(
            "give a row: pitch classes such as 0 1 4 6, or note names",
        ));
    }
    if tokens.iter().all(|token| token.parse::<i32>().is_ok()) {
        return Ok(ToneRow::new(
            tokens.iter().map(|token| token.parse::<i32>().unwrap_or(0)),
        ));
    }
    let pitches = tokens
        .iter()
        .map(|token| Pitch::from_name(*token))
        .collect::<Result<Vec<_>>>()
        .map_err(|err| JsValue::from_str(&err.to_string()))?;
    Ok(ToneRow::from_pitches(pitches.iter()))
}

#[wasm_bindgen]
/// Describes a tone row: its matrix, its forty-eight forms, whether it is
/// all-interval or a Link chord, and which historical row it is.
pub fn tone_row(input: &str) -> Result<JsValue, JsValue> {
    use music21_rs::serial::{Transformation, TransformationConvention};
    let row = parse_row(input)?;
    let twelve = row.is_twelve_tone_row();
    let matrix = if twelve {
        row.matrix()
            .rows()
            .iter()
            .map(|form| form.pitch_classes().to_vec())
            .collect()
    } else {
        Vec::new()
    };
    let mut forms = Vec::new();
    if twelve {
        for transformation in [
            Transformation::Prime,
            Transformation::Inversion,
            Transformation::Retrograde,
            Transformation::RetrogradeInversion,
        ] {
            for index in 0..12_i32 {
                let form = row.zero_centered_transformation(transformation, index);
                forms.push(RowFormInfo {
                    label: format!(
                        "{}{index}",
                        transformation.label(TransformationConvention::ZeroCentered)
                    ),
                    pitch_classes: form.pitch_classes().to_vec(),
                    names: form.note_names(),
                });
            }
        }
    }
    let info = ToneRowInfo {
        pitch_classes: row.pitch_classes().to_vec(),
        names: row.note_names(),
        is_twelve_tone_row: twelve,
        intervals: row.intervals_as_string(),
        matrix,
        forms,
        is_all_interval: row.is_all_interval().ok(),
        link: row
            .link_classification()
            .ok()
            .flatten()
            .map(|link| LinkInfo {
                number: link.number,
                special_intervals: link.special_intervals,
            }),
        historical: row
            .find_historical()
            .into_iter()
            .map(|historical| HistoricalMatch {
                name: historical.name,
                composer: historical.composer,
                opus: historical.opus,
                title: historical.title,
            })
            .collect(),
    };
    serde_wasm_bindgen::to_value(&info).map_err(|err| JsValue::from_str(&err.to_string()))
}

#[derive(Serialize)]
struct HistoricalRowInfo {
    name: &'static str,
    composer: &'static str,
    opus: Option<&'static str>,
    title: &'static str,
    pitch_classes: Vec<u8>,
}

#[wasm_bindgen]
/// The seventy-one historical twelve-tone rows music21 carries.
pub fn historical_rows() -> Result<JsValue, JsValue> {
    let rows = music21_rs::serial::HISTORICAL_ROWS
        .iter()
        .map(|row| HistoricalRowInfo {
            name: row.name,
            composer: row.composer,
            opus: row.opus,
            title: row.title,
            pitch_classes: row.pitch_classes.to_vec(),
        })
        .collect::<Vec<_>>();
    serde_wasm_bindgen::to_value(&rows).map_err(|err| JsValue::from_str(&err.to_string()))
}
