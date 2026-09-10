//! Run a real music21 project's test suite against `music21_rs`.
//!
//! The doctest harness in `python-parity` measures the crate against music21's
//! own docstrings. This measures it against somebody else's code: a library
//! written for music21, by someone who had never heard of this one, with its
//! own test suite and its own idea of what music21 does.
//!
//! `harte-library` parses Harte chord notation. It is a good subject because
//! it does exactly what a music21 library does — it *subclasses*
//! `chord.Chord` and `interval.Interval` and builds on their behaviour — and
//! because it stays inside the part of music21 the crate models: pitches,
//! intervals and chords, with no streams and no notation files.
//!
//! The test is a comparison, not a pass mark. The suite is run twice, once
//! against music21 and once with `music21_rs.install_into_music21()` in front
//! of it, and the two sets of failures have to match exactly. harte-library's
//! own suite has 212 failures of its own (its chord grammar mis-parses `113`
//! as a degree), and those must fail identically either way; anything that
//! fails only under `music21_rs` is a gap in the crate.
//!
//! ```text
//! cargo run --release -p xtask -- downstream
//! ```
//!
//! Needs `git`, and `music21`, `lark`, `numpy`, `pytest` and the `music21_rs`
//! wheel installed in the interpreter that runs it. Nothing here talks to
//! Python itself — it drives `git` and `pytest` — so unlike the other two
//! downstream commands it needs no `python` feature.

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const PROJECT: &str = "harte-library";
const REPOSITORY: &str = "https://github.com/andreamust/harte-library.git";
/// Pinned so the comparison is against a fixed suite; bump deliberately.
pub(crate) const COMMIT: &str = "dd8cfd728572ed728195e05c8f2062a58b3e9036";

/// The one piece of Python left in this: a `conftest.py` written *into
/// somebody else's pytest project*, which pytest imports before any test
/// module. The swap has to happen before that project does `from music21.chord
/// import Chord`, and there is no way to reach inside another interpreter's
/// import of a third-party suite from out here.
const CONFTEST: &str = r#""""Point this project at the Rust implementation.

Written by music21-rs's `xtask downstream`. The swap has to happen before the
project does `from music21.chord import Chord`, and a rootdir conftest is
imported before any test module is.
"""

import os

if os.environ.get("MUSIC21_RS") == "1":
    import music21_rs

    music21_rs.install_into_music21()
"#;

/// A suite that ran this small cannot have been the real one.
const FEWEST_CREDIBLE_TESTS: usize = 1000;

/// Runs the comparison, answering the process exit code.
pub fn run(workspace_root: &Path, directory: Option<PathBuf>) -> Result<i32, Box<dyn Error>> {
    let directory = directory.unwrap_or_else(|| workspace_root.join("target/downstream"));
    let worktree = checkout(&directory)?;

    println!("\nrunning {PROJECT}'s suite twice");
    let stock = failures(&worktree, false)?;
    let ours = failures(&worktree, true)?;

    let only_ours: Vec<&String> = ours.difference(&stock).collect();
    let only_stock: Vec<&String> = stock.difference(&ours).collect();
    if only_ours.is_empty() && only_stock.is_empty() {
        println!(
            "\n{PROJECT} behaves identically on music21_rs: {} of its own failures either way",
            stock.len()
        );
        return Ok(0);
    }
    if !only_ours.is_empty() {
        println!("\n{} tests fail only against music21_rs:", only_ours.len());
        for name in &only_ours {
            println!("    {name}");
        }
    }
    if !only_stock.is_empty() {
        println!("\n{} tests fail only against music21:", only_stock.len());
        for name in &only_stock {
            println!("    {name}");
        }
    }
    Ok(1)
}

/// Clones the project at its pinned commit, or reuses what is there.
/// Clones harte-library at the pinned commit into `into`, or brings an
/// earlier clone there up to it, and answers the worktree.
pub(crate) fn checkout(into: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let worktree = into.join(PROJECT);
    if !worktree.join(".git").is_dir() {
        fs::create_dir_all(&worktree)?;
        git(&["init", "--quiet", &worktree.to_string_lossy()])?;
        git(&[
            "-C",
            &worktree.to_string_lossy(),
            "remote",
            "add",
            "origin",
            REPOSITORY,
        ])?;
    }
    git(&[
        "-C",
        &worktree.to_string_lossy(),
        "fetch",
        "--quiet",
        "--depth",
        "1",
        "origin",
        COMMIT,
    ])?;
    git(&[
        "-C",
        &worktree.to_string_lossy(),
        "checkout",
        "--quiet",
        COMMIT,
    ])?;
    fs::write(worktree.join("conftest.py"), CONFTEST)?;
    Ok(worktree)
}

fn git(args: &[&str]) -> Result<(), Box<dyn Error>> {
    println!("$ git {}", args.join(" "));
    let status = Command::new("git")
        .args(args)
        .status()
        .map_err(|err| format!("could not run git ({err})"))?;
    if !status.success() {
        return Err(format!("git {} failed", args.join(" ")).into());
    }
    Ok(())
}

/// One run of the project's suite, answering the names of what failed.
fn failures(worktree: &Path, swapped: bool) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let python = crate::report::python_command();
    println!(
        "$ {python} -m pytest test -q --no-header -p no:cacheprovider  (MUSIC21_RS={})",
        u8::from(swapped)
    );
    let output = Command::new(&python)
        .args([
            "-m",
            "pytest",
            "test",
            "-q",
            "--no-header",
            "-p",
            "no:cacheprovider",
        ])
        .current_dir(worktree)
        .env("MUSIC21_RS", if swapped { "1" } else { "0" })
        .output()
        .map_err(|err| format!("could not run {python} ({err})"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let tail = stdout.trim_end().lines().last().unwrap_or("(no output)");
    println!(
        "  {}: {tail}",
        if swapped { "music21_rs" } else { "music21   " }
    );

    // pytest answers 0 when everything passed and 1 when tests failed;
    // anything else means the suite never ran.
    let code = output.status.code().unwrap_or(-1);
    if code != 0 && code != 1 {
        eprintln!("{}", last_bytes(&stdout, 4000));
        eprintln!(
            "{}",
            last_bytes(&String::from_utf8_lossy(&output.stderr), 4000)
        );
        return Err(format!("{PROJECT}'s suite did not run (pytest exit {code})").into());
    }
    let ran = tests_reported(tail);
    if ran < FEWEST_CREDIBLE_TESTS {
        eprintln!("{}", last_bytes(&stdout, 4000));
        return Err(format!("{PROJECT} ran only {ran} tests; its suite has thousands").into());
    }
    Ok(failed_names(&stdout))
}

/// The names on pytest's `FAILED <test>` lines.
fn failed_names(stdout: &str) -> BTreeSet<String> {
    stdout
        .lines()
        .filter_map(|line| line.strip_prefix("FAILED "))
        .filter_map(|rest| rest.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

/// How many tests pytest's summary line accounts for.
///
/// The line reads `212 failed, 7904 passed in 45.67s`, so every `<count>
/// passed` and `<count> failed` pair in it is added up. Skipped and errored
/// counts are deliberately left out, exactly as the Python this replaces did:
/// the number is only there to notice a suite that did not really run.
fn tests_reported(summary: &str) -> usize {
    let words: Vec<&str> = summary.split_whitespace().collect();
    words
        .windows(2)
        .filter(|pair| matches!(pair[1].trim_end_matches(','), "passed" | "failed"))
        .filter_map(|pair| pair[0].trim_start_matches('\u{1b}').parse::<usize>().ok())
        .sum()
}

/// The last `limit` bytes of some output, on a character boundary.
fn last_bytes(text: &str, limit: usize) -> &str {
    if text.len() <= limit {
        return text;
    }
    let mut start = text.len() - limit;
    while start < text.len() && !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_lines_give_the_test_names() {
        let stdout = "\
FAILED test/test_harte.py::test_one - AssertionError
some other line
FAILED test/test_harte.py::test_two
FAILEDnot-a-line
";
        let names = failed_names(stdout);
        assert_eq!(names.len(), 2);
        assert!(names.contains("test/test_harte.py::test_one"));
        assert!(names.contains("test/test_harte.py::test_two"));
    }

    #[test]
    fn the_summary_line_says_how_many_ran() {
        assert_eq!(tests_reported("212 failed, 7904 passed in 45.67s"), 8116);
        assert_eq!(tests_reported("8116 passed in 40.00s"), 8116);
        // Skipped and errored counts are not tests that ran either way.
        assert_eq!(tests_reported("1 passed, 3 skipped in 0.10s"), 1);
        assert_eq!(tests_reported("(no output)"), 0);
    }

    #[test]
    fn the_tail_of_a_long_output_stays_valid_text() {
        // Ten single-byte characters and one two-byte one.
        let text = "a".repeat(10) + "é";
        assert_eq!(last_bytes(&text, 2), "é");
        assert_eq!(last_bytes(&text, 3), "aé");
        // A cut that would land inside a character moves past it.
        assert_eq!(last_bytes(&text, 1), "");
        assert_eq!(last_bytes("short", 4000), "short");
    }
}
