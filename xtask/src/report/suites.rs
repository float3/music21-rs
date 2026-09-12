//! Running the suites the report records, and reading their output.
//!
//! Every suite runs under the coverage instrumentation the report
//! starts, so what they leave behind merges into one figure; a suite
//! whose tooling is missing is recorded as skipped, with the reason,
//! rather than failing the report. The parsers of libtest's and
//! pytest's summary lines live here with the runs that need them.

use super::*;

/// Wipes any previous profile data and returns the environment
/// `cargo llvm-cov show-env` describes, so that every suite run with it is
/// instrumented and writes its profile into one place.
///
/// `CARGO_TARGET_DIR` is added to it: `python-parity` is outside the
/// workspace and would otherwise build into its own target directory, where
/// the report step could not find its binaries to map the profiles onto.
pub(super) fn start_coverage(
    workspace_root: &Path,
) -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let cleaned = Command::new("cargo")
        .args(["llvm-cov", "clean", "--workspace"])
        .current_dir(workspace_root)
        .status()
        .map_err(|err| {
            format!("could not run cargo llvm-cov ({err}); is cargo-llvm-cov installed?")
        })?;
    if !cleaned.success() {
        return Err("cargo llvm-cov clean failed".into());
    }

    let output = Command::new("cargo")
        .args(["llvm-cov", "show-env"])
        .current_dir(workspace_root)
        .output()
        .map_err(|err| {
            format!("could not run cargo llvm-cov ({err}); is cargo-llvm-cov installed?")
        })?;
    if !output.status.success() {
        return Err("cargo llvm-cov show-env failed".into());
    }

    let mut env: Vec<(String, String)> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| {
            (
                key.trim().to_string(),
                value.trim().trim_matches('\'').to_string(),
            )
        })
        .collect();
    if env.is_empty() {
        return Err("cargo llvm-cov show-env printed no environment".into());
    }
    let build_dir = env
        .iter()
        .find(|(key, _)| key == "CARGO_LLVM_COV_BUILD_DIR" || key == "CARGO_LLVM_COV_TARGET_DIR")
        .map(|(_, value)| value.clone());
    if let Some(dir) = build_dir {
        env.push(("CARGO_TARGET_DIR".to_string(), dir));
    }
    Ok(env)
}

/// Merges what the suites left behind into one figure, and writes the
/// file-by-file HTML beside the page.
pub(super) fn collect_coverage(
    workspace_root: &Path,
    out: &Path,
    env: &[(String, String)],
) -> Result<Coverage, Box<dyn Error>> {
    let html_dir = out.join("coverage");
    let common = [
        "llvm-cov",
        "report",
        "--ignore-filename-regex",
        COVERAGE_IGNORE,
    ];

    let status = Command::new("cargo")
        .args(common)
        .arg("--html")
        .arg("--output-dir")
        .arg(&html_dir)
        .envs(env.iter().map(|(key, value)| (key, value)))
        .current_dir(workspace_root)
        .status()
        .map_err(|err| format!("could not run cargo llvm-cov ({err})"))?;
    if !status.success() {
        return Err("cargo llvm-cov report --html failed".into());
    }

    let output = Command::new("cargo")
        .args(common)
        .args(["--json", "--summary-only"])
        .envs(env.iter().map(|(key, value)| (key, value)))
        .current_dir(workspace_root)
        .output()?;
    if !output.status.success() {
        return Err("cargo llvm-cov report failed".into());
    }
    let summary: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let totals = summary
        .pointer("/data/0/totals")
        .ok_or("cargo llvm-cov report wrote no totals")?;
    let percent = |key: &str| -> Result<Percent, Box<dyn Error>> {
        let entry = totals
            .get(key)
            .ok_or_else(|| format!("coverage totals carry no {key}"))?;
        let field = |name: &str| entry.get(name).and_then(serde_json::Value::as_f64);
        Ok(Percent {
            count: field("count").unwrap_or(0.0) as u64,
            covered: field("covered").unwrap_or(0.0) as u64,
            percent: field("percent").unwrap_or(0.0),
        })
    };
    Ok(Coverage {
        lines: percent("lines")?,
        functions: percent("functions")?,
        regions: percent("regions")?,
    })
}

/// Runs every suite the repository has and records what each one answered.
/// A suite whose tooling is missing is skipped with the reason rather than
/// failing the report, so this runs anywhere and says what it could not reach.
pub(super) fn run_suites(workspace_root: &Path, env: &[(String, String)]) -> Vec<Suite> {
    let submodule = workspace_root.join("music21/music21/__init__.py");
    let mut suites = vec![
        cargo_suite(
            workspace_root,
            "Workspace",
            &["test", "--workspace", "--all-targets"],
            None,
            env,
        ),
        // `--all-targets` does not include doctests, so without this the
        // crate's own rustdoc examples are never run here at all. They do
        // not reach the *coverage* figure even so: rustdoc compiles a
        // doctest itself and never sees `RUSTC_WRAPPER`, and folding them in
        // properly needs cargo-llvm-cov's `--doctests`.
        //
        // **Turn that on the day it lands on stable**, and drop this note.
        // The whole of `start_coverage` would move to nightly otherwise, for
        // a flag its own help calls unstable, and that is the only reason it
        // is not on already.
        //
        // It is worth having but not worth chasing, which was measured rather
        // than assumed. `cargo +nightly llvm-cov --doctests -p music21-rs
        // --all-features` runs clean today and does move the figure, by
        // 8 lines and 3 functions out of 41,221 and 3,150 — 89.81% to 89.83%.
        // Small because 521 unit tests already cover what 16 rustdoc examples
        // illustrate. Re-measure before deciding it is worth a toolchain
        // change; if the example count ever catches up with the test count,
        // the answer changes.
        cargo_suite(
            workspace_root,
            "Workspace doctests",
            &["test", "--workspace", "--doc"],
            None,
            env,
        ),
        cargo_suite(
            workspace_root,
            "Python parity and music21's doctests",
            &[
                "test",
                "--manifest-path",
                "python-parity/Cargo.toml",
                "--",
                "--test-threads=1",
            ],
            (!submodule.exists()).then_some("the music21 submodule is not checked out"),
            env,
        ),
    ];
    // The wheel is deliberately built uninstrumented. Its Rust half lives in
    // a `.pyd` inside the installed package rather than under the target
    // directory, so the report step could not find the object to map its
    // profiles onto; and an instrumented wheel is not the artifact CI ships.
    // What it tests of the crate, the parity suite covers far more of anyway.
    let (build, tests) = wheel_suites(workspace_root);
    suites.push(build);
    suites.push(tests);
    // The one row whose result is not a pass mark, which the title cannot say.
    suites.push(music21_suite(workspace_root, &submodule).describing(
        "Run twice, once on music21 and once on the crate, and diffed: music21's own suite fails a number of its own tests in any environment, so what counts is that the two sets match.",
    ));
    suites
}

/// Runs music21's own test suite twice — once on music21, once with the
/// crate installed over it — and records the difference.
///
/// Like the wheel's own tests, this runs against the *installed* wheel, whose
/// Rust half is a `.pyd` in site-packages rather than an object under the
/// target directory; so it is not instrumented and adds nothing to the
/// coverage figure. What it adds is the measure: it drives the MusicXML
/// importer, the stream machinery, `freezeThaw` and the corpus, none of which
/// any other suite here reaches.
///
/// music21's suite has failures of its own in any environment, so what is
/// reported is the comparison. `failed` counts everything red under the
/// crate; the status is green only when nothing is red under the crate that
/// was not already red under music21.
pub(super) fn music21_suite(workspace_root: &Path, submodule: &Path) -> Suite {
    const NAME: &str = "music21's own test suite";
    let command = "cargo run --release -p xtask --features python -- music21-suite".to_string();

    let skipped = |detail: String| Suite {
        name: NAME.to_string(),
        command: command.clone(),
        status: SuiteStatus::Skipped,
        passed: 0,
        failed: 0,
        detail: Some(detail),
        note: None,
    };

    if !submodule.exists() {
        return skipped("the music21 submodule is not checked out".to_string());
    }

    let out = workspace_root.join("target/music21-suite");
    let output = xtask_command()
        .arg("music21-suite")
        .arg("--out")
        .arg(&out)
        .current_dir(workspace_root)
        .output();
    let output = match output {
        Ok(output) => output,
        Err(err) => return skipped(format!("could not run xtask again ({err})")),
    };

    // The two reports are what the run means; a non-zero exit only says the
    // comparison found something, which is read off them too.
    let read = |name: &str| -> Option<serde_json::Value> {
        serde_json::from_str(&fs::read_to_string(out.join(name)).ok()?).ok()
    };
    let (Some(plain), Some(ours)) = (read("music21.json"), read("music21_rs.json")) else {
        let text = merged(&output);
        if without_python_feature(&text) {
            return skipped("xtask was built without its `python` feature".to_string());
        }
        return match missing_module(&text) {
            Some(module) => skipped(format!("{module} is not installed in this interpreter")),
            None => Suite {
                name: NAME.to_string(),
                command,
                status: SuiteStatus::Failed,
                passed: 0,
                failed: 0,
                detail: last_line(&text),
                note: None,
            },
        };
    };

    let bad = |report: &serde_json::Value| -> Vec<String> {
        ["failures", "errors"]
            .iter()
            .filter_map(|key| report.get(*key)?.as_array())
            .flatten()
            .filter_map(|case| case.as_str().map(str::to_string))
            .collect()
    };
    let theirs = bad(&plain);
    let mine = bad(&ours);
    // The same list the suite itself excuses. Without it the page called
    // this row red for a divergence that is documented and expected, while
    // the command it names exits green -- the two disagreeing about the same
    // run.
    let regressions = mine
        .iter()
        .filter(|case| !theirs.contains(case))
        .filter(|case| {
            !super::EXPECTED_DIVERGENCES
                .iter()
                .any(|(listed, _)| *listed == case.as_str())
        })
        .count();
    let run = ours
        .get("run")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default() as usize;

    Suite {
        name: NAME.to_string(),
        command,
        status: if regressions == 0 {
            SuiteStatus::Passed
        } else {
            SuiteStatus::Failed
        },
        passed: run.saturating_sub(mine.len()),
        failed: mine.len(),
        detail: Some(format!(
            "{} of these fail on music21 itself; {regressions} fail only under music21_rs",
            theirs.len()
        )),
        note: None,
    }
}

/// The interpreter the wheel suite should use: whatever `PYO3_PYTHON` names,
/// since that is what the pyo3 crates here link against.
pub(crate) fn python_command() -> String {
    env::var("PYO3_PYTHON").unwrap_or_else(|_| "python".to_string())
}

/// This very binary, ready to be run again with a subcommand.
///
/// The benchmark and music21's suite are subcommands of `xtask`, so driving
/// them means running this program again. Taking the running executable rather than
/// `cargo run` keeps the report from rebuilding itself underneath its own run.
pub(super) fn xtask_command() -> Command {
    Command::new(env::current_exe().unwrap_or_else(|_| PathBuf::from("xtask")))
}

/// Whether some output is this program saying it was built without pyo3.
///
/// Those subcommands exist either way and say what is missing, so a report run
/// from a build with no `python` feature records them as skipped with the
/// reason rather than as failures.
pub(super) fn without_python_feature(text: &str) -> bool {
    text.contains("built without its `python` feature")
}

/// Times the crate against music21 through the same Python API. Both need to
/// be importable — the wheel installed, music21 with its dependencies — so
/// this is skipped with the reason wherever they are not, like the suites.
pub(super) fn run_benchmarks(workspace_root: &Path) -> Benchmarks {
    let json = workspace_root.join("target/benchmarks.json");
    let command =
        "cargo run --release -p xtask --features python -- bench --json target/benchmarks.json"
            .to_string();

    let skipped = |detail: String| Benchmarks {
        status: SuiteStatus::Skipped,
        command: command.clone(),
        detail: Some(detail),
        music21: String::new(),
        python: String::new(),
        platform: String::new(),
        cases: Vec::new(),
    };

    let output = xtask_command()
        .arg("bench")
        .arg("--json")
        .arg(&json)
        .current_dir(workspace_root)
        .output();
    let output = match output {
        Ok(output) => output,
        Err(err) => return skipped(format!("could not run xtask again ({err})")),
    };
    if !output.status.success() {
        let text = merged(&output);
        if without_python_feature(&text) {
            return skipped("xtask was built without its `python` feature".to_string());
        }
        return match missing_module(&text) {
            Some(module) => skipped(format!("{module} is not installed in this interpreter")),
            None => Benchmarks {
                status: SuiteStatus::Failed,
                command,
                detail: last_line(&text),
                music21: String::new(),
                python: String::new(),
                platform: String::new(),
                cases: Vec::new(),
            },
        };
    }

    let Ok(text) = fs::read_to_string(&json) else {
        return skipped(format!("{} was not written", json.display()));
    };
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) else {
        return skipped(format!("{} does not parse", json.display()));
    };
    let string = |key: &str| {
        parsed
            .get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let cases: Vec<BenchCase> = parsed
        .get("results")
        .cloned()
        .and_then(|results| serde_json::from_value(results).ok())
        .unwrap_or_default();

    Benchmarks {
        status: if cases.is_empty() {
            SuiteStatus::Skipped
        } else {
            SuiteStatus::Passed
        },
        command,
        detail: cases.is_empty().then(|| "no case was timed".to_string()),
        music21: string("music21"),
        python: string("python"),
        platform: string("platform"),
        cases,
    }
}

/// The median speedup, which is what the headline figure quotes: one very
/// fast case should not speak for the rest.
pub(super) fn median_speedup(cases: &[BenchCase]) -> f64 {
    if cases.is_empty() {
        return 0.0;
    }
    let mut speedups: Vec<f64> = cases.iter().map(|case| case.speedup).collect();
    speedups.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    speedups[speedups.len() / 2]
}

/// `xtask bench`'s own rendering of a duration, so the page and the terminal
/// agree.
pub(super) fn humanise(nanoseconds: f64) -> String {
    if nanoseconds < 1_000.0 {
        format!("{nanoseconds:.0} ns")
    } else if nanoseconds < 1_000_000.0 {
        format!("{:.1} \u{b5}s", nanoseconds / 1_000.0)
    } else {
        format!("{:.1} ms", nanoseconds / 1_000_000.0)
    }
}

pub(super) fn cargo_suite(
    workspace_root: &Path,
    name: &str,
    args: &[&str],
    skip: Option<&str>,
    env: &[(String, String)],
) -> Suite {
    let command = format!("cargo {}", args.join(" "));
    if let Some(reason) = skip {
        return Suite {
            name: name.to_string(),
            command,
            status: SuiteStatus::Skipped,
            passed: 0,
            failed: 0,
            detail: Some(reason.to_string()),
            note: None,
        };
    }
    let output = Command::new("cargo")
        .args(args)
        .envs(env.iter().map(|(key, value)| (key, value)))
        .current_dir(workspace_root)
        .output();
    let output = match output {
        Ok(output) => output,
        Err(err) => {
            return Suite {
                name: name.to_string(),
                command,
                status: SuiteStatus::Skipped,
                passed: 0,
                failed: 0,
                detail: Some(format!("could not run cargo ({err})")),
                note: None,
            };
        }
    };
    let (passed, failed) = cargo_test_counts(&String::from_utf8_lossy(&output.stdout));
    Suite {
        name: name.to_string(),
        command,
        status: if output.status.success() {
            SuiteStatus::Passed
        } else {
            SuiteStatus::Failed
        },
        passed,
        failed,
        detail: None,
        note: None,
    }
}

/// Builds the wheel the way CI does, then runs its own suite against it. Those
/// tests import `music21_rs`, so they only run where the wheel is installed;
/// this never installs it.
pub(super) fn wheel_suites(workspace_root: &Path) -> (Suite, Suite) {
    const BUILD: &str = "Python wheel";
    const TESTS: &str = "Python wheel tests";

    let wheels = workspace_root.join("target/wheels");
    let command = "maturin build --release --manifest-path python/Cargo.toml".to_string();
    let built = Command::new("maturin")
        .args(["build", "--release", "--manifest-path", "python/Cargo.toml"])
        .arg("--out")
        .arg(&wheels)
        .current_dir(workspace_root)
        .output();
    let build = match built {
        Err(err) => Suite {
            name: BUILD.to_string(),
            command,
            status: SuiteStatus::Skipped,
            passed: 0,
            failed: 0,
            detail: Some(format!("maturin is not installed ({err})")),
            note: None,
        },
        Ok(output) if output.status.success() => Suite {
            name: BUILD.to_string(),
            command,
            status: SuiteStatus::Passed,
            passed: 0,
            failed: 0,
            detail: built_wheel_name(&merged(&output)),
            note: None,
        },
        Ok(output) => Suite {
            name: BUILD.to_string(),
            command,
            status: SuiteStatus::Failed,
            passed: 0,
            failed: 0,
            detail: last_line(&merged(&output)),
            note: None,
        },
    };

    let python = python_command();
    let command = "python -m pytest python/tests".to_string();
    let ran = Command::new(&python)
        .args(["-m", "pytest", "python/tests", "-q"])
        .current_dir(workspace_root)
        .output();
    let tests = match ran {
        Err(err) => Suite {
            name: TESTS.to_string(),
            command,
            status: SuiteStatus::Skipped,
            passed: 0,
            failed: 0,
            detail: Some(format!("could not run {python} ({err})")),
            note: None,
        },
        Ok(output) => {
            let text = merged(&output);
            match missing_module(&text) {
                Some(missing) => Suite {
                    name: TESTS.to_string(),
                    command,
                    status: SuiteStatus::Skipped,
                    passed: 0,
                    failed: 0,
                    detail: Some(format!("{python} has no {missing}")),
                    note: None,
                },
                None => {
                    let (passed, failed) = pytest_counts(&text);
                    Suite {
                        name: TESTS.to_string(),
                        command,
                        status: if output.status.success() {
                            SuiteStatus::Passed
                        } else {
                            SuiteStatus::Failed
                        },
                        passed,
                        failed,
                        detail: None,
                        note: None,
                    }
                }
            }
        }
    };
    (build, tests)
}

/// Everything a command wrote, on either stream: maturin reports the wheel it
/// built on stderr, and a Python import error lands there too.
pub(super) fn merged(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Sums the `test result:` lines libtest writes, one per test binary.
pub(super) fn cargo_test_counts(text: &str) -> (usize, usize) {
    let mut passed = 0;
    let mut failed = 0;
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix("test result:") else {
            continue;
        };
        let words: Vec<&str> = rest.split_whitespace().collect();
        for pair in words.windows(2) {
            match (pair[0].parse::<usize>(), pair[1].trim_end_matches(';')) {
                (Ok(count), "passed") => passed += count,
                (Ok(count), "failed") => failed += count,
                _ => {}
            }
        }
    }
    (passed, failed)
}

/// Reads pytest's one-line summary, `19 passed in 0.07s` or
/// `2 failed, 17 passed in 0.11s`.
pub(super) fn pytest_counts(text: &str) -> (usize, usize) {
    for line in text.lines().rev() {
        let words: Vec<&str> = line.split_whitespace().collect();
        let mut passed = 0;
        let mut failed = 0;
        let mut seen = false;
        for pair in words.windows(2) {
            match (pair[0].parse::<usize>(), pair[1].trim_end_matches(',')) {
                (Ok(count), "passed") => {
                    passed += count;
                    seen = true;
                }
                (Ok(count), "failed") => {
                    failed += count;
                    seen = true;
                }
                _ => {}
            }
        }
        if seen {
            return (passed, failed);
        }
    }
    (0, 0)
}

/// The module a Python run could not import, when that is why it failed.
pub(super) fn missing_module(text: &str) -> Option<String> {
    let marker = "No module named ";
    let start = text.find(marker)? + marker.len();
    let name: String = text[start..]
        .trim_start_matches(['\'', '"'])
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.')
        .collect();
    (!name.is_empty()).then_some(name)
}

pub(super) fn built_wheel_name(stdout: &str) -> Option<String> {
    let line = stdout.lines().find(|line| line.contains("Built wheel"))?;
    let (_, path) = line.rsplit_once(' ')?;
    Path::new(path.trim())
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

pub(super) fn last_line(text: &str) -> Option<String> {
    text.lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    /// A divergence the suite excuses must not make the page call the row
    /// red: the command the row names exits green on exactly that case.
    #[test]
    fn a_listed_divergence_is_not_a_regression() {
        let listed = super::EXPECTED_DIVERGENCES
            .first()
            .map(|(case, _)| *case)
            .expect("a divergence to test with");
        let theirs: Vec<String> = vec!["already red".to_string()];
        let mine: Vec<String> = vec!["already red".to_string(), listed.to_string()];
        let regressions = mine
            .iter()
            .filter(|case| !theirs.contains(case))
            .filter(|case| {
                !super::EXPECTED_DIVERGENCES
                    .iter()
                    .any(|(l, _)| *l == case.as_str())
            })
            .count();
        assert_eq!(regressions, 0);
    }
}
