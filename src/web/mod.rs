use std::collections::BTreeSet;

use crate::{
    chord::{Chord, ChordAnalysis},
    key::{Key, keysignature::KeySignature},
    pitch::Pitch,
    polyrhythm::{Polyrhythm, PolyrhythmEvent},
};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WebsiteChordAnalysis {
    pub input: String,
    pub analysis: ChordAnalysis,
    pub common_names: Vec<String>,
    pub pitch_classes: Vec<u8>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WebsiteKeyAnalysis {
    pub tonic: String,
    pub mode: String,
    pub sharps: i32,
    pub scale_pitches: Vec<String>,
    pub harmonized_triads: Vec<String>,
    pub harmonized_sevenths: Vec<String>,
    pub relative_tonic: String,
    pub relative_mode: String,
    pub parallel_tonic: String,
    pub parallel_mode: String,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProgressionChordAnalysis {
    pub input: String,
    pub pitched_common_name: String,
    pub root: Option<String>,
    pub common_name: String,
    pub quality: String,
    pub degree: Option<u8>,
    pub roman_numeral: Option<String>,
    pub diatonic: bool,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WebsiteProgressionAnalysis {
    pub tonic: String,
    pub mode: String,
    pub chords: Vec<ProgressionChordAnalysis>,
    pub diatonic_count: usize,
    pub non_diatonic_count: usize,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScaleSuggestion {
    pub tonic: String,
    pub mode: String,
    pub sharps: i32,
    pub scale_pitches: Vec<String>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChordScaleSuggestions {
    pub input: String,
    pub chord: WebsiteChordAnalysis,
    pub suggestions: Vec<ScaleSuggestion>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WebsitePolyrhythmAnalysis {
    pub base: u32,
    pub tempo: u32,
    pub components: Vec<u32>,
    pub cycle: u32,
    pub tick_duration_seconds: f64,
    pub beat_timings_seconds: Vec<Vec<f64>>,
    pub events: Vec<PolyrhythmEvent>,
    pub coincidence_ticks: Vec<u32>,
    pub derived_chord_name: String,
}

pub fn analyze_chord(input: &str) -> Result<WebsiteChordAnalysis, String> {
    let chord = Chord::new(Some(input)).map_err(|e| e.to_string())?;
    Ok(WebsiteChordAnalysis {
        input: input.to_string(),
        analysis: chord.analysis(),
        common_names: chord.common_names(),
        pitch_classes: chord.pitch_classes(),
    })
}

pub fn analyze_key(tonic: &str, mode: Option<&str>) -> Result<WebsiteKeyAnalysis, String> {
    let key = Key::from_tonic_mode(tonic, mode).map_err(|e| e.to_string())?;
    let relative = key.relative().map_err(|e| e.to_string())?;
    let parallel = key.parallel().map_err(|e| e.to_string())?;

    let scale_pitches = key
        .pitches()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|pitch| readable_name_with_octave(&pitch))
        .collect::<Vec<_>>();

    let harmonized_triads = key
        .harmonized_triads()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|chord| chord.pitched_common_name())
        .collect::<Vec<_>>();

    let harmonized_sevenths = key
        .harmonized_sevenths()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|chord| chord.pitched_common_name())
        .collect::<Vec<_>>();

    Ok(WebsiteKeyAnalysis {
        tonic: readable_name(&key.tonic().name()),
        mode: key.mode().to_string(),
        sharps: key.sharps(),
        scale_pitches,
        harmonized_triads,
        harmonized_sevenths,
        relative_tonic: readable_name(&relative.tonic().name()),
        relative_mode: relative.mode().to_string(),
        parallel_tonic: readable_name(&parallel.tonic().name()),
        parallel_mode: parallel.mode().to_string(),
    })
}

pub fn analyze_progression(
    chords: &[&str],
    tonic: &str,
    mode: Option<&str>,
) -> Result<WebsiteProgressionAnalysis, String> {
    let key = Key::from_tonic_mode(tonic, mode).map_err(|e| e.to_string())?;
    let scale_degree_roots = (1..=7)
        .map(|degree| {
            key.pitch_from_degree(degree)
                .map(|pitch| normalize_note_name(&pitch.name()))
                .map_err(|e| e.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;

    let analyzed = chords
        .iter()
        .map(|input| {
            let chord = Chord::new(Some(*input)).map_err(|e| e.to_string())?;
            let common_name = chord.common_name();
            let quality = quality_from_common_name(&common_name).to_string();
            let root = chord.root_pitch_name();
            let degree = root.as_ref().and_then(|root_name| {
                let normalized = normalize_note_name(root_name);
                scale_degree_roots
                    .iter()
                    .position(|scale_note| *scale_note == normalized)
                    .map(|idx| (idx + 1) as u8)
            });
            let roman_numeral =
                degree.map(|value| roman_numeral_for(value, &quality, &common_name));

            Ok(ProgressionChordAnalysis {
                input: (*input).to_string(),
                pitched_common_name: chord.pitched_common_name(),
                root,
                common_name,
                quality,
                degree,
                diatonic: degree.is_some(),
                roman_numeral,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let diatonic_count = analyzed.iter().filter(|item| item.diatonic).count();
    let non_diatonic_count = analyzed.len().saturating_sub(diatonic_count);

    Ok(WebsiteProgressionAnalysis {
        tonic: readable_name(&key.tonic().name()),
        mode: key.mode().to_string(),
        chords: analyzed,
        diatonic_count,
        non_diatonic_count,
    })
}

pub fn suggest_scales_for_chord(input: &str) -> Result<ChordScaleSuggestions, String> {
    let chord = Chord::new(Some(input)).map_err(|e| e.to_string())?;
    let chord_analysis = WebsiteChordAnalysis {
        input: input.to_string(),
        analysis: chord.analysis(),
        common_names: chord.common_names(),
        pitch_classes: chord.pitch_classes(),
    };
    let chord_pcs = chord.pitch_classes().into_iter().collect::<BTreeSet<_>>();

    let modes = [
        "major",
        "minor",
        "dorian",
        "phrygian",
        "lydian",
        "mixolydian",
        "locrian",
    ];

    let mut suggestions = Vec::new();
    let mut seen = BTreeSet::new();

    for sharps in -7..=7 {
        for mode in modes {
            let key = match KeySignature::new(sharps).try_as_key(Some(mode), None) {
                Ok(key) => key,
                Err(_) => continue,
            };
            let key_id = format!("{}:{mode}", readable_name(&key.tonic().name()));
            if !seen.insert(key_id) {
                continue;
            }

            let key_pitches = match key.pitches() {
                Ok(pitches) => pitches,
                Err(_) => continue,
            };

            let key_pcs = key_pitches
                .iter()
                .take(7)
                .map(pitch_to_pc)
                .collect::<BTreeSet<_>>();

            if chord_pcs.is_subset(&key_pcs) {
                suggestions.push(ScaleSuggestion {
                    tonic: readable_name(&key.tonic().name()),
                    mode: mode.to_string(),
                    sharps,
                    scale_pitches: key_pitches
                        .into_iter()
                        .map(|pitch| readable_name_with_octave(&pitch))
                        .collect(),
                });
            }
        }
    }

    suggestions.sort_by_key(|entry| (entry.sharps.abs(), entry.mode.clone(), entry.tonic.clone()));

    Ok(ChordScaleSuggestions {
        input: input.to_string(),
        chord: chord_analysis,
        suggestions,
    })
}

pub fn analyze_polyrhythm(
    base: u32,
    tempo: u32,
    components: &[u32],
    base_pitch: &str,
) -> Result<WebsitePolyrhythmAnalysis, String> {
    let polyrhythm =
        Polyrhythm::new_with_tempo(base, tempo, components).map_err(|e| e.to_string())?;
    let events = polyrhythm.events_one_cycle().map_err(|e| e.to_string())?;
    let beat_timings_seconds = polyrhythm.beat_timings().map_err(|e| e.to_string())?;
    let tick_duration_seconds = polyrhythm.tick_duration().map_err(|e| e.to_string())?;
    let derived_chord_name = polyrhythm
        .as_chord(base_pitch)
        .map_err(|e| e.to_string())?
        .pitched_common_name();

    Ok(WebsitePolyrhythmAnalysis {
        base,
        tempo,
        components: components.to_vec(),
        cycle: polyrhythm.cycle_duration(),
        tick_duration_seconds,
        beat_timings_seconds,
        events,
        coincidence_ticks: polyrhythm.coincidence_ticks_one_cycle(2),
        derived_chord_name,
    })
}

fn normalize_note_name(name: &str) -> String {
    let trimmed = name.trim();
    let without_octave = trimmed
        .chars()
        .take_while(|ch| !ch.is_ascii_digit())
        .collect::<String>();
    let normalized = without_octave.replace('-', "b");
    if normalized.is_empty() {
        return normalized;
    }
    let mut chars = normalized.chars();
    let mut output = String::new();
    if let Some(first) = chars.next() {
        output.push(first.to_ascii_uppercase());
    }
    output.extend(chars);
    output
}

fn readable_name(name: &str) -> String {
    name.replace('-', "b")
}

fn readable_name_with_octave(pitch: &Pitch) -> String {
    readable_name(&pitch.name_with_octave())
}

fn pitch_to_pc(pitch: &Pitch) -> u8 {
    ((pitch.ps().round() as i32).rem_euclid(12)) as u8
}

fn quality_from_common_name(common_name: &str) -> &'static str {
    if common_name.contains("half-diminished") {
        return "half-diminished";
    }
    if common_name.contains("diminished") {
        return "diminished";
    }
    if common_name.contains("augmented") {
        return "augmented";
    }
    if common_name.contains("minor") {
        return "minor";
    }
    if common_name.contains("dominant") {
        return "dominant";
    }
    if common_name.contains("major") {
        return "major";
    }
    "other"
}

fn roman_numeral_for(degree: u8, quality: &str, common_name: &str) -> String {
    let base = match degree {
        1 => "I",
        2 => "II",
        3 => "III",
        4 => "IV",
        5 => "V",
        6 => "VI",
        7 => "VII",
        _ => "?",
    };

    let mut numeral = if matches!(quality, "minor" | "diminished" | "half-diminished") {
        base.to_lowercase()
    } else {
        base.to_string()
    };

    if matches!(quality, "diminished" | "half-diminished") {
        numeral.push('o');
    }
    if common_name.contains("ninth") {
        numeral.push('9');
    } else if common_name.contains("seventh") {
        numeral.push('7');
    }

    numeral
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_analyze_chord() {
        let analysis = analyze_chord("C E G").unwrap();
        assert_eq!(analysis.analysis.pitched_common_name, "C-major triad");
        assert_eq!(analysis.pitch_classes, vec![0, 4, 7]);
    }

    #[test]
    fn web_analyze_key() {
        let analysis = analyze_key("D", Some("major")).unwrap();
        assert_eq!(analysis.tonic, "D");
        assert_eq!(analysis.mode, "major");
        assert_eq!(analysis.sharps, 2);
        assert_eq!(analysis.scale_pitches[0], "D4");
        assert_eq!(analysis.harmonized_triads[0], "D-major triad");
    }

    #[test]
    fn web_analyze_progression() {
        let analysis = analyze_progression(
            &["C E G", "D F A", "G B D F", "F# A C#"],
            "C",
            Some("major"),
        )
        .unwrap();

        assert_eq!(analysis.mode, "major");
        assert_eq!(analysis.chords[0].roman_numeral.as_deref(), Some("I"));
        assert_eq!(analysis.chords[1].roman_numeral.as_deref(), Some("ii"));
        assert_eq!(analysis.chords[2].roman_numeral.as_deref(), Some("V7"));
        assert!(!analysis.chords[3].diatonic);
        assert_eq!(analysis.non_diatonic_count, 1);
    }

    #[test]
    fn web_suggest_scales_for_chord() {
        let suggestions = suggest_scales_for_chord("C E G").unwrap();
        assert!(!suggestions.suggestions.is_empty());
        assert!(
            suggestions
                .suggestions
                .iter()
                .any(|entry| entry.tonic == "C" && entry.mode == "major")
        );
    }

    #[test]
    fn web_analyze_polyrhythm() {
        let analysis = analyze_polyrhythm(4, 120, &[2, 3], "C4").unwrap();
        assert_eq!(analysis.cycle, 6);
        assert_eq!(analysis.coincidence_ticks, vec![0]);
        assert_eq!(analysis.events.len(), 6);
        assert!(!analysis.derived_chord_name.is_empty());
    }
}
