use music21_rs::Chord;
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct PitchInfo {
    index: usize,
    name: String,
    name_with_octave: String,
    octave: Option<i32>,
    pitch_space: f64,
    pitch_class: u8,
    alter: f64,
}

#[derive(Serialize)]
struct ChordAnalysis {
    input: String,
    common_name: String,
    common_names: Vec<String>,
    pitched_common_name: String,
    pitched_common_names: Vec<String>,
    pitch_classes: Vec<u8>,
    root_pitch_name: Option<String>,
    bass_pitch_name: Option<String>,
    forte_class: Option<String>,
    normal_form: Option<Vec<u8>>,
    interval_class_vector: Option<Vec<u8>>,
    inversion: Option<u8>,
    inversion_name: Option<String>,
    key_estimate: Option<String>,
    pitches: Vec<PitchInfo>,
}

#[wasm_bindgen]
pub fn analyze_chord(input: &str) -> Result<JsValue, JsValue> {
    let chord = Chord::new(input).map_err(|err| JsValue::from_str(&err.to_string()))?;
    let common_name = chord.common_name();
    let root_pitch_name = chord.root_pitch_name();
    let key_estimate = estimate_key(root_pitch_name.as_deref(), &common_name);

    let pitches = chord
        .pitches()
        .into_iter()
        .enumerate()
        .map(|(index, pitch)| {
            let pitch_space = pitch.ps();
            PitchInfo {
                index,
                name: pitch.name(),
                name_with_octave: pitch.name_with_octave(),
                octave: pitch.octave(),
                pitch_space,
                pitch_class: (pitch_space.round() as i32).rem_euclid(12) as u8,
                alter: pitch.alter(),
            }
        })
        .collect();

    serde_wasm_bindgen::to_value(&ChordAnalysis {
        input: input.to_string(),
        common_name,
        common_names: chord.common_names(),
        pitched_common_name: chord.pitched_common_name(),
        pitched_common_names: chord.pitched_common_names(),
        pitch_classes: chord.pitch_classes(),
        root_pitch_name,
        bass_pitch_name: chord.bass_pitch_name_public(),
        forte_class: chord.forte_class(),
        normal_form: chord.normal_form(),
        interval_class_vector: chord.interval_class_vector(),
        inversion: chord.inversion(),
        inversion_name: chord.inversion_name(),
        key_estimate,
        pitches,
    })
    .map_err(|err| JsValue::from_str(&err.to_string()))
}

fn estimate_key(root: Option<&str>, common_name: &str) -> Option<String> {
    let root = root?;
    if common_name == "major triad" {
        Some(format!("{root} major"))
    } else if common_name == "minor triad" {
        Some(format!("{root} minor"))
    } else {
        None
    }
}
