//! Checks the crate's copy of music21's `harmony.CHORD_TYPES` against upstream.
//!
//! `src/chordsymbol.rs` hand-maintains a 50-entry mirror of that table. It is
//! correct today, but nothing stopped it drifting, which is the same failure
//! mode that let two mistranscribed Partch ratios ship. The expectations in
//! `data/chord_type_expectations.toml` are generated from the submodule by
//! `cargo run -p xtask --features python -- regenerate-fixtures`, so this test needs neither
//! Python nor the submodule.

use music21_rs::known_chord_symbol_types;
use serde::Deserialize;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct Expectations {
    chord_type: Vec<ChordTypeExpectation>,
    aliases: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct ChordTypeExpectation {
    kind: String,
    notation: String,
    abbreviations: Vec<String>,
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("python-parity has a parent directory")
        .to_path_buf()
}

fn expectations() -> Expectations {
    let path = repo_root().join("data/chord_type_expectations.toml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("{} is unreadable: {err}", path.display()));
    toml::from_str(&text).expect("chord type expectations parse")
}

#[test]
fn chord_types_match_music21_in_order() {
    let expected = expectations().chord_type;
    let actual = known_chord_symbol_types();

    assert_eq!(
        actual.len(),
        expected.len(),
        "crate has {} chord types, music21 has {}",
        actual.len(),
        expected.len()
    );

    let mut problems = Vec::new();
    for (index, (ours, theirs)) in actual.iter().zip(&expected).enumerate() {
        if ours.kind != theirs.kind {
            problems.push(format!(
                "index {index}: kind {:?} vs music21 {:?}",
                ours.kind, theirs.kind
            ));
            continue;
        }
        if ours.notation != theirs.notation {
            problems.push(format!(
                "{}: notation {:?} vs music21 {:?}",
                ours.kind, ours.notation, theirs.notation
            ));
        }
        // The crate keeps only one abbreviation per kind. music21 writes figures
        // with the first, so that is the one it must hold.
        let first = theirs.abbreviations.first().map(String::as_str);
        if Some(ours.abbreviation) != first {
            problems.push(format!(
                "{}: abbreviation {:?} vs music21's first {:?} (all: {:?})",
                ours.kind, ours.abbreviation, first, theirs.abbreviations
            ));
        }
    }

    assert!(
        problems.is_empty(),
        "{} chord types differ from music21:\n    {}\nregenerate with \
         cargo run -p xtask --features python -- regenerate-fixtures",
        problems.len(),
        problems.join("\n    ")
    );
}

#[test]
fn chord_kinds_are_unique() {
    let mut kinds: Vec<&str> = known_chord_symbol_types()
        .iter()
        .map(|chord_type| chord_type.kind)
        .collect();
    let total = kinds.len();
    kinds.sort_unstable();
    kinds.dedup();
    assert_eq!(kinds.len(), total, "chord kinds must be unique");
}

#[test]
fn every_alias_target_is_a_known_kind() {
    // music21's CHORD_ALIASES map input spellings onto real kinds. The crate
    // does not accept chord kinds by name yet, so it carries no aliases - but
    // every target must still exist here, or adding them later would be broken
    // from the start.
    let expectations = expectations();
    let kinds: Vec<&str> = known_chord_symbol_types()
        .iter()
        .map(|chord_type| chord_type.kind)
        .collect();

    for (alias, target) in &expectations.aliases {
        assert!(
            kinds.contains(&target.as_str()),
            "alias {alias:?} points at {target:?}, which the crate does not know"
        );
    }
}
