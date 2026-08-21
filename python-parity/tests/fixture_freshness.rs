//! Stops the committed music21 fixtures drifting from the submodule.
//!
//! Every fixture under `data/` that is generated from music21 records the
//! `music21_version` it was generated against. The library-side parity tests
//! deliberately need neither Python nor the submodule, which is what makes them
//! fast — but it also means they would keep passing forever against a fixture
//! generated from a music21 that no longer matches the pinned submodule.
//!
//! This test closes that hole. It needs the submodule (but not Python), and
//! fails when the submodule is bumped without regenerating the fixtures.

use std::path::{Path, PathBuf};

/// Fixtures generated from music21, all of which must carry a version stamp.
const VERSIONED_FIXTURES: [&str; 5] = [
    "data/scale_expectations.toml",
    "data/chord_type_expectations.toml",
    "data/meter_expectations.toml",
    "data/table_expectations.toml",
    // Not an expectation fixture but generated from the submodule all the same,
    // so a submodule bump has to refresh it too.
    "data/scala_archive.toml",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("python-parity has a parent directory")
        .to_path_buf()
}

/// Reads `__version__` out of the submodule without importing Python.
fn submodule_version() -> String {
    let path = repo_root().join("music21/music21/_version.py");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "{} is unreadable ({err}); run `git submodule update --init --recursive`",
            path.display()
        )
    });

    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("__version__") else {
            continue;
        };
        let Some((_, value)) = rest.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches(['\'', '"'].as_slice());
        if !value.is_empty() {
            return value.to_string();
        }
    }

    panic!("no __version__ assignment found in {}", path.display());
}

/// Reads the `music21_version = "..."` stamp out of a fixture.
fn fixture_version(relative: &str) -> String {
    let path = repo_root().join(relative);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("{} is unreadable: {err}", path.display()));

    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("music21_version") else {
            continue;
        };
        let Some((_, value)) = rest.split_once('=') else {
            continue;
        };
        return value.trim().trim_matches('"').to_string();
    }

    panic!(
        "{relative} has no `music21_version` stamp; regenerate it with \
         cargo run -p xtask --features python -- regenerate-fixtures"
    );
}

#[test]
fn every_fixture_was_generated_from_the_pinned_submodule() {
    let expected = submodule_version();
    let mut stale = Vec::new();

    for fixture in VERSIONED_FIXTURES {
        let actual = fixture_version(fixture);
        if actual != expected {
            stale.push(format!("{fixture}: generated from {actual}, submodule is {expected}"));
        }
    }

    assert!(
        stale.is_empty(),
        "{} fixture(s) are stale; regenerate with \
         `cargo run -p xtask --features python -- regenerate-fixtures`:\n    {}",
        stale.len(),
        stale.join("\n    ")
    );
}

#[test]
fn every_generated_fixture_is_listed_here() {
    // A new fixture that forgets to register itself would never be checked for
    // staleness, so the directory listing is the source of truth.
    let data = repo_root().join("data");
    let mut unlisted = Vec::new();

    for entry in std::fs::read_dir(&data).expect("data directory is readable") {
        let path = entry.expect("directory entry is readable").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("fixture is readable");
        if !text.contains("music21_version") {
            // chord_tables.toml and tuning_tables.toml are pipeline *inputs*
            // verified by `xtask verify-tables` / `verify-tuning-tables`, not
            // music21 expectation fixtures, so they carry no stamp.
            continue;
        }
        let name = format!(
            "data/{}",
            path.file_name()
                .and_then(|name| name.to_str())
                .expect("fixture has a UTF-8 name")
        );
        if !VERSIONED_FIXTURES.contains(&name.as_str()) {
            unlisted.push(name);
        }
    }

    assert!(
        unlisted.is_empty(),
        "these fixtures carry a music21_version but are not in VERSIONED_FIXTURES: {unlisted:?}"
    );
}
