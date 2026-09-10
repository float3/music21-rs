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
const VERSIONED_FIXTURES: [&str; 10] = [
    "data/scale_expectations.toml",
    "data/harte_expectations.toml",
    "data/chord_type_expectations.toml",
    "data/meter_expectations.toml",
    "data/table_expectations.toml",
    "data/serial_expectations.toml",
    // Not an expectation either: the doctest totals the report shows for a
    // module the harness does not cover yet.
    "data/doctest_totals.toml",
    // Not an expectation fixture but generated from the submodule all the same,
    // so a submodule bump has to refresh it too.
    "data/scala_archive.toml",
    // Pipeline inputs rather than expectations, but still read out of the
    // submodule -- the first from `chord/tables.py`, the second from the Scala
    // archive. `verify-tables` and `verify-tuning-tables` only check each
    // against the Rust emitted from it, so drift from a bumped submodule is
    // invisible to them and has to be caught here.
    "data/chord_tables.toml",
    "data/tuning_tables.toml",
];

/// The one generated file that also carries scales from the `hexatone`
/// submodule, and so records its commit as well.
const HEXATONE_FIXTURE: &str = "data/scala_archive.toml";

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

/// The commit a submodule is checked out at.
///
/// The version is what a human reads, but one version spans many commits —
/// every one of the sixty-four that took music21 from 11.0.0b8 to 11.0.0b9
/// called itself one or the other — so only the commit pins exactly what a
/// generated file was built from.
fn submodule_commit(name: &str) -> String {
    let dir = repo_root().join(name);
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(&dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap_or_else(|err| panic!("could not run git in {}: {err}", dir.display()));
    assert!(
        output.status.success(),
        "git rev-parse HEAD failed in {}: {}; run `git submodule update --init --recursive`",
        dir.display(),
        String::from_utf8_lossy(&output.stderr).trim()
    );
    String::from_utf8(output.stdout)
        .expect("git prints a UTF-8 commit")
        .trim()
        .to_string()
}

/// Reads a `<key> = "..."` stamp out of a generated file.
fn fixture_stamp(relative: &str, key: &str) -> String {
    let path = repo_root().join(relative);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("{} is unreadable: {err}", path.display()));

    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix(key) else {
            continue;
        };
        let Some((_, value)) = rest.split_once('=') else {
            continue;
        };
        return value.trim().trim_matches('"').to_string();
    }

    panic!(
        "{relative} has no `{key}` stamp; regenerate it with \
         `cargo run --release -p xtask --features python -- regenerate-all`"
    );
}

#[test]
fn every_fixture_was_generated_from_the_pinned_submodule() {
    let expected_version = submodule_version();
    let expected_commit = submodule_commit("music21");
    let mut stale = Vec::new();

    for fixture in VERSIONED_FIXTURES {
        let actual = fixture_stamp(fixture, "music21_version");
        if actual != expected_version {
            stale.push(format!(
                "{fixture}: generated from music21 {actual}, submodule is {expected_version}"
            ));
        }
        let actual = fixture_stamp(fixture, "music21_commit");
        if actual != expected_commit {
            stale.push(format!(
                "{fixture}: generated at music21 {actual}, submodule is at {expected_commit}"
            ));
        }
    }

    // The bundled Scala archive is the one generated file merging two
    // submodules, so it is the only one with a second commit to check.
    let expected_hexatone = submodule_commit("hexatone");
    let actual = fixture_stamp(HEXATONE_FIXTURE, "hexatone_commit");
    if actual != expected_hexatone {
        stale.push(format!(
            "{HEXATONE_FIXTURE}: generated at hexatone {actual}, \
             submodule is at {expected_hexatone}"
        ));
    }

    assert!(
        stale.is_empty(),
        "{} stamp(s) are stale; regenerate with \
         `cargo run --release -p xtask --features python -- regenerate-all`:\n    {}",
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
            // `feature_map.toml` is hand-maintained rather than generated, so
            // there is no version for it to be stale against. Every other TOML
            // under `data/` comes out of the submodule and carries a stamp.
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
