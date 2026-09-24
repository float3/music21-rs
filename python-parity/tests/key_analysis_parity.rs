//! Checks the crate's discrete analyses against music21's `analysis.discrete`.
//!
//! `data/key_analysis_expectations.toml` records, for a few pieces, the
//! notes music21 read, every key it ranked under each of the five profiles
//! with its coefficient, the pitch span and the melodic intervals of each
//! line, written by
//! `cargo run --release -p xtask --features python -- regenerate-fixtures`.
//! This reads the notes back and asks the crate. The ranking and each key's
//! spelling must agree exactly; a coefficient to within a few units in the
//! last place, since music21 squares through the platform's `pow`, which on
//! Windows is not exact, so its own last digit differs by platform.

use std::collections::BTreeMap;
use std::path::Path;

use music21_rs::Pitch;
use music21_rs::analysis::{
    KeyProfile, estimate_key_from_distribution, melodic_interval_counts, pitch_class_distribution,
    pitch_span,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Expectations {
    piece: Vec<Piece>,
}

#[derive(Debug, Deserialize)]
struct Piece {
    name: String,
    notes: Vec<String>,
    span: Option<[String; 2]>,
    lines: Vec<String>,
    intervals: Vec<String>,
    #[serde(flatten)]
    rankings: BTreeMap<String, toml::Value>,
}

fn expectations() -> Expectations {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("python-parity has a parent directory")
        .join("data/key_analysis_expectations.toml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("{} is unreadable: {err}", path.display()));
    toml::from_str(&text).expect("key analysis expectations parse")
}

fn pitch(name: &str) -> Pitch {
    Pitch::from_name(name).unwrap_or_else(|err| panic!("{name}: {err}"))
}

#[test]
fn every_piece_is_analysed_as_music21_analyses_it() {
    let expected = expectations();
    assert!(expected.piece.len() >= 5, "the pieces");
    for piece in &expected.piece {
        let label = &piece.name;
        let notes: Vec<(Vec<Pitch>, f64)> = piece
            .notes
            .iter()
            .map(|written| {
                let mut words: Vec<&str> = written.split(' ').collect();
                let length: f64 = words.pop().expect("a length").parse().expect("a number");
                (words.into_iter().map(pitch).collect(), length)
            })
            .collect();
        let distribution = pitch_class_distribution(
            notes
                .iter()
                .map(|(pitches, length)| (pitches.as_slice(), *length)),
        )
        .expect("the piece has notes");

        for profile in KeyProfile::ALL {
            let name = profile.music21_class_name();
            let theirs: Vec<(String, f64)> = piece.rankings[name]
                .as_array()
                .expect("a ranking")
                .iter()
                .map(|key| {
                    let written = key.as_str().expect("a key");
                    let (named, score) = written.rsplit_once(' ').expect("a key and its score");
                    (named.to_string(), score.parse().expect("a score"))
                })
                .collect();
            let ours: Vec<(String, f64)> = estimate_key_from_distribution(profile, &distribution)
                .iter()
                .map(|estimate| {
                    (
                        format!(
                            "{} {}",
                            estimate.key().tonic().name(),
                            estimate.key().mode()
                        ),
                        estimate.score(),
                    )
                })
                .collect();
            let names = |ranking: &[(String, f64)]| -> Vec<String> {
                ranking.iter().map(|(named, _)| named.clone()).collect()
            };
            assert_eq!(names(&ours), names(&theirs), "{label}: {name}");
            for ((named, ours), (_, theirs)) in ours.iter().zip(&theirs) {
                assert!(
                    (ours - theirs).abs() <= 4.0 * f64::EPSILON * theirs.abs().max(1.0),
                    "{label}: {name}: {named} scores {ours} where music21 gives {theirs}"
                );
            }
        }

        let all: Vec<Pitch> = notes
            .iter()
            .flat_map(|(pitches, _)| pitches.clone())
            .collect();
        let span =
            pitch_span(&all).map(|(low, high)| [low.name_with_octave(), high.name_with_octave()]);
        assert_eq!(span, piece.span, "{label}: span");

        let lines: Vec<Vec<Pitch>> = piece
            .lines
            .iter()
            .map(|line| {
                line.split(' ')
                    .filter(|name| !name.is_empty())
                    .map(pitch)
                    .collect()
            })
            .collect();
        let counted: Vec<String> = melodic_interval_counts(&lines, true, true)
            .expect("the lines' intervals")
            .iter()
            .map(|(interval, count)| format!("{} {count}", interval.directed_name()))
            .collect();
        assert_eq!(counted, piece.intervals, "{label}: intervals");
    }
}
