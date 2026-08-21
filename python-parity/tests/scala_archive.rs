//! Parses music21's whole Scala archive with [`ScalaScale`].
//!
//! This lives here rather than in the library's own tests because it needs the
//! `music21` submodule, and `cargo test` on the library must work without it.
//! It needs no Python, unlike the parity suite next to it.

use music21_rs::{ScalaArchive, ScalaScale};

use std::path::{Path, PathBuf};

/// Files in the archive that are not valid Scala and are expected to fail.
///
/// Empty, and that is the point: every file in the pinned archive now parses.
///
/// `sparschuh-stanhope.scl` was listed here for writing a degree as `697//441`
/// with a doubled slash. Upstream fixed it in cuthbertLab/music21#2003, which
/// this submodule pin now includes, so the entry is gone.
///
/// `xxx.scl` was listed for declaring zero degrees. That is legal Scala —
/// music21 reads it as a scale with no pitches — and rejecting it was a bug in
/// this crate, since fixed.
const KNOWN_BAD: [&str; 0] = [];

fn archive_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("python-parity has a parent directory")
        .join("music21/music21/scale/scala/scl")
}

#[test]
fn parses_the_whole_scala_archive() {
    let dir = archive_dir();
    if !dir.is_dir() {
        panic!(
            "{} is missing; run `git submodule update --init --recursive`",
            dir.display()
        );
    }

    let mut parsed = 0usize;
    let mut empty = 0usize;
    let mut unexpected_failures = Vec::new();
    let mut unexpected_successes = Vec::new();

    for entry in std::fs::read_dir(&dir).expect("archive directory is readable") {
        let path = entry.expect("directory entry is readable").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("scl") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("scl file has a UTF-8 name")
            .to_string();

        // Much of the archive predates UTF-8, so read bytes rather than a
        // String and let the parser handle the conversion.
        let bytes = std::fs::read(&path).expect("scl file is readable");
        let expected_bad = KNOWN_BAD.contains(&name.as_str());

        match (ScalaScale::parse(&String::from_utf8_lossy(&bytes)), expected_bad) {
            (Ok(scale), false) => {
                // A file may legally declare zero degrees; `xxx.scl` does, and
                // music21 reads it the same way. Such a scale has no unison to
                // check, so only non-empty ones get the degree assertions.
                if let Some(first) = scale.degrees().first() {
                    assert_eq!(first.ratio(), 1.0, "{name} does not start at the unison");
                } else {
                    empty += 1;
                }
                assert!(
                    scale.period().ratio() > 0.0,
                    "{name} has a non-positive period"
                );
                parsed += 1;
            }
            (Err(error), false) => unexpected_failures.push(format!("{name}: {error}")),
            (Ok(_), true) => unexpected_successes.push(name),
            (Err(_), true) => {}
        }
    }

    assert!(
        unexpected_failures.is_empty(),
        "{} archive files failed to parse:\n  {}",
        unexpected_failures.len(),
        unexpected_failures.join("\n  ")
    );
    assert!(
        unexpected_successes.is_empty(),
        "files listed in KNOWN_BAD now parse and should be removed from it: {unexpected_successes:?}"
    );
    // Exactly one archive file declares zero degrees. If that changes, the
    // "empty is legal" carve-out above is covering more than it should.
    assert_eq!(empty, 1, "expected only xxx.scl to declare zero degrees");
    // Guards against the walk silently finding nothing if the layout moves.
    assert!(
        parsed > 3_000,
        "only {parsed} scala files parsed; expected the full archive"
    );
}

#[test]
fn reads_partch_43_as_the_committed_tuning_table_does() {
    let path = archive_dir().join("partch_43.scl");
    let bytes = std::fs::read(&path).expect("partch_43.scl is readable");
    let scale = ScalaScale::parse_bytes(&bytes).expect("partch_43.scl parses");

    // The same shape `xtask regenerate-tuning-tables` writes into
    // data/tuning_tables.toml: 43 degrees starting at 1/1, octave held apart.
    assert_eq!(scale.len(), 43);
    assert_eq!(scale.period().ratio(), 2.0);

    let ratios: Vec<(u32, u32)> = scale
        .degrees()
        .iter()
        .map(|degree| {
            let fraction = degree
                .as_fraction()
                .expect("partch_43 is written entirely in exact ratios");
            (fraction.numerator(), fraction.denominator())
        })
        .collect();

    assert_eq!(ratios[0], (1, 1));
    // The two degrees that were once mistranscribed in the committed table.
    assert_eq!(ratios[26], (32, 21));
    assert_eq!(ratios[35], (16, 9));

    // Partch's scale is strictly ascending across the whole octave.
    for pair in scale.degrees().windows(2) {
        assert!(
            pair[1].ratio() > pair[0].ratio(),
            "partch_43 is not strictly ascending at {} -> {}",
            pair[0],
            pair[1]
        );
    }
}

/// Search results verified against `music21.scale.scala.search` on this same
/// archive snapshot.
const SEARCH_EXPECTATIONS: [(&str, usize); 12] = [
    ("mbira", 8),
    ("slendro", 54),
    ("partch", 13),
    ("pyth", 16),
    ("indian", 39),
    ("ji", 58),
    ("bohlen", 35),
    ("mbira banda", 2),
    ("partch_43.scl", 1),
    ("xenakis", 3),
    ("harrison", 19),
    ("GAMELAN", 1),
];

#[test]
fn archive_search_matches_music21_on_the_real_corpus() {
    let dir = archive_dir();
    if !dir.is_dir() {
        panic!(
            "{} is missing; run `git submodule update --init --recursive`",
            dir.display()
        );
    }

    let mut archive = ScalaArchive::new();
    for entry in std::fs::read_dir(&dir).expect("archive directory is readable") {
        let path = entry.expect("directory entry is readable").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("scl") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("scl file has a UTF-8 name")
            .to_string();
        if KNOWN_BAD.contains(&name.as_str()) {
            continue;
        }
        let bytes = std::fs::read(&path).expect("scl file is readable");
        archive
            .insert(name, &bytes)
            .expect("archive file parses");
    }

    assert!(archive.len() > 3_000, "only {} scales indexed", archive.len());

    for (target, expected) in SEARCH_EXPECTATIONS {
        let hits = archive.search(target);
        assert_eq!(
            hits.len(),
            expected,
            "search({target:?}) returned {:?}",
            &hits[..hits.len().min(6)]
        );
    }

    // The exact-name case must return that file, not merely something like it.
    assert_eq!(archive.search("partch_43.scl"), ["partch_43.scl"]);
    assert!(archive.search("zzz-nothing-matches-this").is_empty());
}
