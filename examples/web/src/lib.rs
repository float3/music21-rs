use music21_rs::{
    COMMON_TWELVE_TONE_TUNING_SYSTEMS, Chord, ExceptionResult, Fraction, KnownChordType, Pitch,
    TuningSystem,
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
    pitch_classes: Vec<u8>,
    root_pitch_name: Option<String>,
    bass_pitch_name: Option<String>,
    forte_class: Option<String>,
    normal_form: Option<Vec<u8>>,
    interval_class_vector: Option<Vec<u8>>,
    inversion: Option<u8>,
    inversion_name: Option<String>,
    key_estimate: Option<String>,
    resolution_chords: Vec<ResolutionChordInfo>,
    pitches: Vec<PitchInfo>,
}

#[derive(Serialize)]
struct KnownChordInfo {
    id: String,
    primary_common_name: String,
    common_names: Vec<String>,
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

#[wasm_bindgen]
pub fn analyze_chord(input: &str) -> Result<JsValue, JsValue> {
    let midi_numbers = parse_midi_input(input);
    let chord = if let Some(midi_numbers) = midi_numbers.as_deref() {
        Chord::from_midi_numbers(midi_numbers)
    } else {
        Chord::new(input)
    }
    .map_err(|err| JsValue::from_str(&err.to_string()))?;
    let common_name = chord.common_name();
    let root_pitch_name = chord.root_pitch_name();
    let key_estimate = estimate_key(root_pitch_name.as_deref(), &common_name);

    let pitches =
        display_pitch_infos(chord.pitches()).map_err(|err| JsValue::from_str(&err.to_string()))?;

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
        resolution_chords: suggested_resolution_chords(&chord),
        pitches,
    })
    .map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen]
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
    KnownChordInfo {
        id: format!("{}:{id_suffix}:{primary_common_name}", chord.forte_class),
        primary_common_name,
        common_names,
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

#[wasm_bindgen]
pub fn tuning_systems(root_frequency_hz: f64) -> Result<JsValue, JsValue> {
    if !root_frequency_hz.is_finite() || root_frequency_hz <= 0.0 {
        return Err(JsValue::from_str(
            "root frequency must be a positive number",
        ));
    }

    let systems = all_tuning_systems()
        .into_iter()
        .map(|(id, tuning_system)| {
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
                        ratio_label: fraction_label(fraction),
                        frequency_hz: root_frequency_hz * ratio,
                        cents_from_equal_temperament: tuning_system.cents(degree),
                    }
                })
                .collect();

            TuningSystemInfo {
                id,
                name: tuning_system.display_name(),
                description: tuning_system_description(id),
                octave_size,
                root_frequency_hz,
                degrees,
            }
        })
        .collect::<Vec<_>>();

    serde_wasm_bindgen::to_value(&systems).map_err(|err| JsValue::from_str(&err.to_string()))
}

fn display_pitch_infos(pitches: Vec<Pitch>) -> ExceptionResult<Vec<PitchInfo>> {
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

fn all_tuning_systems() -> [(&'static str, TuningSystem); 17] {
    [
        (
            "EqualTemperament",
            TuningSystem::EqualTemperament { octave_size: 12 },
        ),
        (
            "RecursiveEqualTemperament",
            TuningSystem::RecursiveEqualTemperament { octave_size: 12 },
        ),
        ("WholeTone", TuningSystem::WholeTone),
        ("QuarterTone", TuningSystem::QuarterTone),
        ("JustIntonation", TuningSystem::JustIntonation),
        ("JustIntonation24", TuningSystem::JustIntonation24),
        ("PythagoreanTuning", TuningSystem::PythagoreanTuning),
        ("FiveLimit", TuningSystem::FiveLimit),
        ("ElevenLimit", TuningSystem::ElevenLimit),
        ("FortyThreeTone", TuningSystem::FortyThreeTone),
        ("StepMethod", TuningSystem::StepMethod),
        ("Javanese", TuningSystem::Javanese),
        ("Thai", TuningSystem::Thai),
        ("Indian", TuningSystem::Indian),
        ("IndianAlt", TuningSystem::IndianAlt),
        ("Indian22", TuningSystem::Indian22),
        ("IndianFull", TuningSystem::IndianFull),
    ]
}

fn fraction_label(fraction: Fraction) -> String {
    if fraction.base() == 0 {
        if fraction.denominator() == 1 {
            fraction.numerator().to_string()
        } else {
            format!("{}/{}", fraction.numerator(), fraction.denominator())
        }
    } else if fraction.numerator() == 0 {
        "1".to_string()
    } else {
        format!(
            "{}^({}/{})",
            fraction.base(),
            fraction.numerator(),
            fraction.denominator()
        )
    }
}

fn tuning_system_description(id: &str) -> &'static str {
    match id {
        "EqualTemperament" => "Twelve equal divisions of the octave.",
        "RecursiveEqualTemperament" => "Equal temperament calculated recursively.",
        "WholeTone" => "Six equal whole-tone steps per octave.",
        "QuarterTone" => "Twenty-four equal quarter-tone steps per octave.",
        "JustIntonation" => "A twelve-tone just-intonation ratio table.",
        "JustIntonation24" => "A twenty-four-tone just-intonation ratio table.",
        "PythagoreanTuning" => "A twelve-tone tuning table built from pure fifths.",
        "FiveLimit" => "A twelve-tone table using five-limit just ratios.",
        "ElevenLimit" => "A twenty-nine-tone table using eleven-limit ratios.",
        "FortyThreeTone" => "A forty-three-tone ratio table.",
        "StepMethod" => "A twelve-tone equal-temperament step method.",
        "Javanese" => "A five-tone Javanese equal-temperament approximation.",
        "Thai" => "A seven-tone Thai equal-temperament approximation.",
        "Indian" => "A seven-tone Indian scale ratio table.",
        "IndianAlt" => "An alternate seven-tone Indian scale ratio table.",
        "Indian22" => "A twenty-two-tone Indian scale ratio table.",
        "IndianFull" => "The full twenty-two-tone Indian scale table.",
        _ => "A music21-rs tuning system.",
    }
}

fn display_pitch_for_sequence(
    pitch: Pitch,
    last_pitch_space: &mut Option<i32>,
) -> ExceptionResult<Pitch> {
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
    Pitch::from_name(format!("{}{}", pitch.name(), (pitch_space / 12) - 1))
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

fn suggested_resolution_chords(chord: &Chord) -> Vec<ResolutionChordInfo> {
    let names = common_names_with_primary(chord);
    let is_dominant_function = is_dominant_function_sonority(chord, &names);
    let is_leading_tone_function = is_leading_tone_function_sonority(chord, &names);
    let is_augmented_sixth = is_augmented_sixth_spelling(chord);

    let mut suggestions = Vec::new();
    let mut seen = BTreeSet::new();

    if !is_augmented_sixth && let Some(root_pc) = root_pitch_class(chord) {
        if is_dominant_function {
            add_target_key_resolutions(
                chord,
                (root_pc + 5) % 12,
                "dominant resolution",
                &mut suggestions,
                &mut seen,
            );
        }

        if is_leading_tone_function {
            add_target_key_resolutions(
                chord,
                (root_pc + 1) % 12,
                "leading-tone resolution",
                &mut suggestions,
                &mut seen,
            );
        }
    }

    if is_augmented_sixth {
        add_augmented_sixth_resolutions(chord, &mut suggestions, &mut seen);
    }

    suggestions
}

fn common_names_with_primary(chord: &Chord) -> Vec<String> {
    let mut names = vec![chord.common_name()];
    names.extend(chord.common_names());
    names.sort();
    names.dedup();
    names
}

fn add_target_key_resolutions(
    chord: &Chord,
    target_pc: u8,
    label: &str,
    suggestions: &mut Vec<ResolutionChordInfo>,
    seen: &mut BTreeSet<(String, String)>,
) {
    let tonic = pitch_class_name(target_pc);
    for mode in ["major", "minor"] {
        let context = format!("{label} to {} {mode}", display_pitch_name(tonic));
        add_resolutions_for_key(chord, tonic, mode, &context, suggestions, seen, |_| true);
    }
}

fn add_augmented_sixth_resolutions(
    chord: &Chord,
    suggestions: &mut Vec<ResolutionChordInfo>,
    seen: &mut BTreeSet<(String, String)>,
) {
    for tonic in CANDIDATE_TONICS {
        for mode in ["major", "minor"] {
            if !is_augmented_sixth_in_key(chord, tonic, mode) {
                continue;
            }

            let tonic_pc = pitch_class_from_name(tonic).unwrap_or(0);
            let dominant_pc = (tonic_pc + 7) % 12;
            let context = format!(
                "augmented-sixth resolution in {} {mode}",
                display_pitch_name(tonic)
            );
            add_resolutions_for_key(
                chord,
                tonic,
                mode,
                &context,
                suggestions,
                seen,
                |resolution| root_pitch_class(resolution) == Some(dominant_pc),
            );
        }
    }
}

fn add_resolutions_for_key(
    chord: &Chord,
    tonic: &str,
    mode: &str,
    context: &str,
    suggestions: &mut Vec<ResolutionChordInfo>,
    seen: &mut BTreeSet<(String, String)>,
    keep: impl Fn(&Chord) -> bool,
) {
    let Ok(resolutions) = chord.resolution_chords(tonic, Some(mode)) else {
        return;
    };

    for resolution in resolutions {
        if !keep(&resolution) {
            continue;
        }

        let pitched_common_name = resolution.pitched_common_name();
        let key = (pitched_common_name.clone(), context.to_string());
        if !seen.insert(key) {
            continue;
        }

        suggestions.push(ResolutionChordInfo {
            pitched_common_name,
            key_context: context.to_string(),
            pitch_names: resolution
                .pitches()
                .iter()
                .map(|pitch| pitch.name_with_octave())
                .collect(),
            pitch_classes: resolution.pitch_classes(),
        });
    }
}

fn root_pitch_class(chord: &Chord) -> Option<u8> {
    chord
        .root_pitch_name()
        .as_deref()
        .and_then(pitch_class_from_name)
}

fn is_dominant_function_sonority(chord: &Chord, names: &[String]) -> bool {
    let has_explicit_dominant_name = names.iter().any(|name| {
        matches!(
            name.as_str(),
            "dominant seventh chord"
                | "major minor seventh chord"
                | "incomplete dominant-seventh chord"
        )
    });
    let has_dominant_family_name = names
        .iter()
        .any(|name| name.contains("dominant") || name == "major-minor");

    has_explicit_dominant_name
        || (has_dominant_family_name
            && root_pitch_class(chord)
                .is_some_and(|root_pc| has_intervals_above_root(chord, root_pc, &[4, 10])))
}

fn is_leading_tone_function_sonority(chord: &Chord, names: &[String]) -> bool {
    let has_explicit_leading_tone_name = names.iter().any(|name| {
        matches!(
            name.as_str(),
            "diminished triad"
                | "diminished seventh chord"
                | "half-diminished seventh chord"
                | "incomplete half-diminished seventh chord"
        )
    });
    let has_diminished_family_name = names.iter().any(|name| name.contains("diminished"));

    has_explicit_leading_tone_name
        || (has_diminished_family_name
            && root_pitch_class(chord)
                .is_some_and(|root_pc| has_intervals_above_root(chord, root_pc, &[3, 6])))
}

fn has_intervals_above_root(chord: &Chord, root_pc: u8, intervals: &[u8]) -> bool {
    let pitch_classes = chord.pitch_classes().into_iter().collect::<BTreeSet<_>>();
    intervals
        .iter()
        .all(|interval| pitch_classes.contains(&((root_pc + interval) % 12)))
}

fn is_augmented_sixth_spelling(chord: &Chord) -> bool {
    let pitches = chord.pitches();
    for (index, lower) in pitches.iter().enumerate() {
        for upper in pitches.iter().skip(index + 1) {
            if directed_augmented_sixth(lower, upper) || directed_augmented_sixth(upper, lower) {
                return true;
            }
        }
    }
    false
}

fn is_augmented_sixth_in_key(chord: &Chord, tonic: &str, mode: &str) -> bool {
    if !is_augmented_sixth_spelling(chord) {
        return false;
    }

    let Some(tonic_pc) = pitch_class_from_name(tonic) else {
        return false;
    };
    let chord_pcs = chord.pitch_classes().into_iter().collect::<BTreeSet<_>>();
    if chord_pcs.len() < 3 || chord_pcs.len() > 4 {
        return false;
    }

    let scale = if mode == "minor" {
        [0, 2, 3, 5, 7, 8, 10]
    } else {
        [0, 2, 4, 5, 7, 9, 11]
    };
    let fourth_pc = (tonic_pc + scale[3]) % 12;
    let sixth_pc = (tonic_pc + scale[5]) % 12;
    let lowered_sixth_pc = if scale[5] == 9 {
        (sixth_pc + 11) % 12
    } else {
        sixth_pc
    };
    let raised_fourth_pc = (fourth_pc + 1) % 12;

    chord_pcs.contains(&tonic_pc)
        && chord_pcs.contains(&lowered_sixth_pc)
        && chord_pcs.contains(&raised_fourth_pc)
}

fn directed_augmented_sixth(lower: &Pitch, upper: &Pitch) -> bool {
    let Some(lower_step) = step_index(&lower.name()) else {
        return false;
    };
    let Some(upper_step) = step_index(&upper.name()) else {
        return false;
    };

    let generic_interval = (upper_step - lower_step).rem_euclid(7) + 1;
    let semitones = ((upper.ps().round() as i32) - (lower.ps().round() as i32)).rem_euclid(12);
    generic_interval == 6 && semitones == 10
}

fn step_index(name: &str) -> Option<i32> {
    match name.chars().next()? {
        'C' => Some(0),
        'D' => Some(1),
        'E' => Some(2),
        'F' => Some(3),
        'G' => Some(4),
        'A' => Some(5),
        'B' => Some(6),
        _ => None,
    }
}

fn pitch_class_from_name(name: &str) -> Option<u8> {
    Pitch::from_name(name)
        .ok()
        .map(|pitch| (pitch.ps().round() as i32).rem_euclid(12) as u8)
}

fn pitch_class_name(pc: u8) -> &'static str {
    CANDIDATE_TONICS[pc as usize]
}

fn display_pitch_name(name: &str) -> String {
    name.replace('-', "b")
}

const CANDIDATE_TONICS: [&str; 12] = [
    "C", "D-", "D", "E-", "E", "F", "F#", "G", "A-", "A", "B-", "B",
];

#[cfg(test)]
mod tests {
    use super::parse_midi_input;

    #[test]
    fn parse_midi_input_accepts_plain_prefixed_and_csv_values() {
        assert_eq!(parse_midi_input("60 64 67"), Some(vec![60, 64, 67]));
        assert_eq!(parse_midi_input("midi: 60,64,67"), Some(vec![60, 64, 67]));
        assert_eq!(parse_midi_input("MIDI 60 64 67"), Some(vec![60, 64, 67]));
        assert_eq!(parse_midi_input("C E G"), None);
    }
}
