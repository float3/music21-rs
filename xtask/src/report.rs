//! The `report` command: test coverage and the ported feature set, as one
//! static page for the GitHub Pages site.
//!
//! Coverage comes from `cargo llvm-cov` over the library's own tests, with the
//! generated tables left out — `chord/tables/generated.rs` and
//! `scala_bundled.rs` are data, and counting their thousands of lines would
//! say nothing about the code.
//!
//! The feature set is read off music21 itself. `data/feature_map.toml` names
//! the music21 classes the crate ports and the Rust files each lives in; this
//! scans the submodule for every public method of those classes, converts the
//! name the way the crate does (`isMajorTriad` → `is_major_triad`, a leading
//! `get` dropped), and looks for a `pub fn` of that name. The map carries the
//! renames the convention does not give and the members deliberately left out,
//! with a reason each — and a rename that points at a function that does not
//! exist, or an exclusion of a member music21 no longer has, is an error, so
//! the map cannot go stale silently. That is the "check" in the command.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

/// Files that are generated data rather than code, kept out of the coverage
/// figures.
const COVERAGE_IGNORE: &str = r"generated\.rs|scala_bundled\.rs";

#[derive(Debug, Deserialize)]
struct FeatureMap {
    class: Vec<ClassMap>,
}

#[derive(Debug, Deserialize)]
struct ClassMap {
    /// Path of the Python file inside the `music21` submodule.
    python: String,
    /// The class whose methods are read, or absent for the module's own
    /// functions.
    #[serde(default)]
    name: Option<String>,
    /// What is enumerated: the class's methods (the default) or, for a module
    /// whose port is a table of variants, its classes.
    #[serde(default)]
    members: Members,
    /// Only classes whose names end with this are enumerated, for
    /// `members = "classes"`.
    #[serde(default)]
    suffix: Option<String>,
    /// Rust files searched for the port.
    rust: Vec<String>,
    /// A short description for the page.
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    renames: BTreeMap<String, String>,
    #[serde(default)]
    excluded: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Members {
    #[default]
    Methods,
    Classes,
}

#[derive(Debug, Serialize)]
struct Report {
    generated_from: String,
    music21_version: String,
    coverage: Option<Coverage>,
    features: Vec<ClassReport>,
}

#[derive(Debug, Serialize, Clone, Copy)]
struct Coverage {
    lines: Percent,
    functions: Percent,
    regions: Percent,
}

#[derive(Debug, Serialize, Clone, Copy)]
struct Percent {
    count: u64,
    covered: u64,
    percent: f64,
}

#[derive(Debug, Serialize)]
struct ClassReport {
    python: String,
    name: String,
    note: Option<String>,
    ported: usize,
    missing: usize,
    excluded: usize,
    members: Vec<MemberReport>,
}

#[derive(Debug, Serialize)]
struct MemberReport {
    name: String,
    status: Status,
    /// The Rust name for a ported member, the reason for an excluded one.
    detail: Option<String>,
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Status {
    Ported,
    Missing,
    Excluded,
}

pub(crate) struct Options {
    pub out: PathBuf,
    pub coverage: bool,
    pub features: bool,
}

pub(crate) fn parse_options(workspace_root: &Path, args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        out: workspace_root.join("target/reports"),
        coverage: true,
        features: true,
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => {
                let path = args.next().ok_or("--out needs a directory")?;
                options.out = workspace_root.join(path);
            }
            "--features-only" => options.coverage = false,
            "--coverage-only" => options.features = false,
            other => return Err(format!("unknown report option {other:?}")),
        }
    }
    Ok(options)
}

pub(crate) fn report(workspace_root: &Path, options: &Options) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(&options.out)?;

    let coverage = if options.coverage {
        Some(measure_coverage(workspace_root, &options.out)?)
    } else {
        None
    };

    let features = if options.features {
        scan_features(workspace_root)?
    } else {
        Vec::new()
    };

    let report = Report {
        generated_from: git_head(workspace_root),
        music21_version: submodule_version(workspace_root)?,
        coverage,
        features,
    };

    fs::write(
        options.out.join("report.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    fs::write(options.out.join("index.html"), render_html(&report))?;

    println!("wrote {}", options.out.join("index.html").display());
    if let Some(coverage) = &report.coverage {
        println!(
            "  coverage: {:.2}% of lines, {:.2}% of functions, {:.2}% of regions",
            coverage.lines.percent, coverage.functions.percent, coverage.regions.percent
        );
    }
    for class in &report.features {
        let counted = class.ported + class.missing;
        println!(
            "  {}: {} of {} ported, {} excluded",
            class.name, class.ported, counted, class.excluded
        );
    }
    Ok(())
}

fn git_head(workspace_root: &Path) -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(workspace_root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn submodule_version(workspace_root: &Path) -> Result<String, Box<dyn Error>> {
    let path = workspace_root.join("music21/music21/_version.py");
    let text = fs::read_to_string(&path).map_err(|err| {
        format!(
            "{} is unreadable ({err}); run `git submodule update --init --recursive`",
            path.display()
        )
    })?;
    text.lines()
        .find_map(|line| {
            let rest = line.trim().strip_prefix("__version__")?;
            let rest = rest.trim().strip_prefix('=')?.trim();
            Some(rest.trim_matches(|c| c == '\'' || c == '"').to_string())
        })
        .ok_or_else(|| format!("no __version__ in {}", path.display()).into())
}

/// Runs the library's tests under instrumentation and writes the HTML.
///
/// Cleans first: `cargo llvm-cov report` merges every profile and object it
/// finds under `target`, and stale ones — from a checkout that has since moved,
/// or from a package built with coverage on for some other reason — would be
/// counted as never executed and drag the total down.
fn measure_coverage(workspace_root: &Path, out: &Path) -> Result<Coverage, Box<dyn Error>> {
    let html_dir = out.join("coverage");
    let common = [
        "-p",
        "music21-rs",
        "--all-features",
        "--ignore-filename-regex",
        COVERAGE_IGNORE,
    ];

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

    let status = Command::new("cargo")
        .arg("llvm-cov")
        .args(common)
        .arg("--html")
        .arg("--output-dir")
        .arg(&html_dir)
        .current_dir(workspace_root)
        .status()
        .map_err(|err| {
            format!("could not run cargo llvm-cov ({err}); is cargo-llvm-cov installed?")
        })?;
    if !status.success() {
        return Err("cargo llvm-cov failed".into());
    }

    let output = Command::new("cargo")
        .args(["llvm-cov", "report", "--json", "--summary-only"])
        .args(["--ignore-filename-regex", COVERAGE_IGNORE])
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

fn scan_features(workspace_root: &Path) -> Result<Vec<ClassReport>, Box<dyn Error>> {
    let map_path = workspace_root.join("data/feature_map.toml");
    let map: FeatureMap = toml::from_str(&fs::read_to_string(&map_path)?)
        .map_err(|err| format!("{} does not parse: {err}", map_path.display()))?;

    let mut classes = Vec::new();
    let mut problems = Vec::new();

    for class in &map.class {
        let python_path = workspace_root.join("music21/music21").join(&class.python);
        let python = fs::read_to_string(&python_path)
            .map_err(|err| format!("{} is unreadable: {err}", python_path.display()))?;
        let label = match (&class.name, class.members) {
            (Some(name), _) => name.clone(),
            (None, Members::Methods) => format!("{} functions", class.python),
            (None, Members::Classes) => format!("{} classes", class.python),
        };

        let members = match class.members {
            Members::Methods => match &class.name {
                Some(name) => class_methods(&python, name),
                None => module_functions(&python),
            },
            Members::Classes => module_classes(&python, class.suffix.as_deref()),
        };
        if members.is_empty() {
            problems.push(format!("{label}: nothing found in {}", class.python));
        }

        let mut rust = String::new();
        for file in &class.rust {
            let path = workspace_root.join(file);
            let text = fs::read_to_string(&path)
                .map_err(|err| format!("{} is unreadable: {err}", path.display()))?;
            rust.push_str(&text);
            rust.push('\n');
        }

        for excluded in class.excluded.keys() {
            if !members.contains(excluded) {
                problems.push(format!(
                    "{label}: `{excluded}` is excluded but music21 has no such member"
                ));
            }
        }
        for (from, to) in &class.renames {
            if !members.contains(from) {
                problems.push(format!(
                    "{label}: `{from}` is renamed but music21 has no such member"
                ));
            }
            if !defines(&rust, to, class.members) {
                problems.push(format!(
                    "{label}: `{from}` is renamed to `{to}`, which none of {} defines",
                    class.rust.join(", ")
                ));
            }
        }

        let mut reports = Vec::new();
        for member in &members {
            let report = if let Some(reason) = class.excluded.get(member) {
                MemberReport {
                    name: member.clone(),
                    status: Status::Excluded,
                    detail: Some(reason.clone()),
                }
            } else if let Some(found) = candidates(member, &class.renames, class.members)
                .into_iter()
                .find(|candidate| defines(&rust, candidate, class.members))
            {
                MemberReport {
                    name: member.clone(),
                    status: Status::Ported,
                    detail: Some(found),
                }
            } else {
                MemberReport {
                    name: member.clone(),
                    status: Status::Missing,
                    detail: None,
                }
            };
            reports.push(report);
        }

        let count = |status: Status| reports.iter().filter(|r| r.status == status).count();
        classes.push(ClassReport {
            python: class.python.clone(),
            name: label,
            note: class.note.clone(),
            ported: count(Status::Ported),
            missing: count(Status::Missing),
            excluded: count(Status::Excluded),
            members: reports,
        });
    }

    if !problems.is_empty() {
        return Err(format!(
            "{} is out of date:\n  {}",
            map_path.display(),
            problems.join("\n  ")
        )
        .into());
    }
    Ok(classes)
}

/// The public methods and properties of one class, in source order, each once.
fn class_methods(python: &str, class: &str) -> Vec<String> {
    let header = format!("class {class}(");
    let bare = format!("class {class}:");
    let mut inside = false;
    let mut names = Vec::new();
    for line in python.lines() {
        if line.starts_with("class ") {
            inside = line.starts_with(&header) || line.starts_with(&bare);
            continue;
        }
        if !inside {
            continue;
        }
        if let Some(name) = method_name(line, "    def ") {
            push_unique(&mut names, name);
        }
    }
    names
}

/// The public functions defined at the top of a module.
fn module_functions(python: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in python.lines() {
        if let Some(name) = method_name(line, "def ") {
            push_unique(&mut names, name);
        }
    }
    names
}

/// The public classes of a module, optionally only those ending in `suffix`,
/// and never the test cases or the exceptions.
fn module_classes(python: &str, suffix: Option<&str>) -> Vec<String> {
    let mut names = Vec::new();
    for line in python.lines() {
        let Some(rest) = line.strip_prefix("class ") else {
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty()
            || name.starts_with('_')
            || name.starts_with("Test")
            || name.ends_with("Exception")
            || suffix.is_some_and(|suffix| !name.ends_with(suffix))
        {
            continue;
        }
        push_unique(&mut names, name);
    }
    names
}

fn method_name(line: &str, prefix: &str) -> Option<String> {
    let rest = line.strip_prefix(prefix)?;
    let name: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() || name.starts_with('_') {
        return None;
    }
    Some(name)
}

fn push_unique(names: &mut Vec<String>, name: String) {
    if !names.contains(&name) {
        names.push(name);
    }
}

/// The Rust names a music21 member might have been given, most specific
/// first.
fn candidates(member: &str, renames: &BTreeMap<String, String>, members: Members) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(renamed) = renames.get(member) {
        out.push(renamed.clone());
    }
    match members {
        Members::Methods => {
            let snake = snake_case(member);
            out.push(snake.clone());
            if let Some(rest) = snake.strip_prefix("get_") {
                out.push(rest.to_string());
            }
        }
        Members::Classes => out.push(member.to_string()),
    }
    out
}

/// `isMajorTriad` → `is_major_triad`, `forteClassTnI` → `forte_class_tn_i`.
fn snake_case(name: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = name.chars().collect();
    for (index, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let previous_lower = index > 0 && chars[index - 1].is_ascii_lowercase();
            let previous_digit = index > 0 && chars[index - 1].is_ascii_digit();
            let acronym_ends = index > 0
                && chars[index - 1].is_ascii_uppercase()
                && chars.get(index + 1).is_some_and(char::is_ascii_lowercase);
            if previous_lower || previous_digit || acronym_ends {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Whether the Rust source defines the name: a `pub fn` for a method, or the
/// bare identifier — an enum variant or a type — for a class.
fn defines(rust: &str, name: &str, members: Members) -> bool {
    match members {
        Members::Methods => {
            let plain = format!("pub fn {name}(");
            let plain_generic = format!("pub fn {name}<");
            let constant = format!("pub const fn {name}(");
            rust.contains(&plain) || rust.contains(&plain_generic) || rust.contains(&constant)
        }
        Members::Classes => rust
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .any(|word| word == name),
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn render_html(report: &Report) -> String {
    let mut html = String::new();
    let _ = write!(
        html,
        r#"<!doctype html>
<html lang="en">
    <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>music21-rs Reports</title>
        <link rel="stylesheet" href="../theme.css" />
        <style>
            .summary {{
                display: grid;
                grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
                gap: 18px;
                margin-bottom: 24px;
            }}
            .summary .panel {{
                padding: 18px;
            }}
            .summary .number {{
                display: block;
                font-size: 32px;
                font-weight: 800;
            }}
            .summary .label {{
                color: var(--muted);
            }}
            .bar {{
                height: 8px;
                border-radius: 4px;
                background: var(--line);
                overflow: hidden;
                margin-top: 10px;
            }}
            .bar span {{
                display: block;
                height: 100%;
                background: var(--accent-strong);
            }}
            details {{
                margin-bottom: 14px;
            }}
            summary {{
                cursor: pointer;
                font-weight: 700;
            }}
            summary .count {{
                color: var(--muted);
                font-weight: 400;
                margin-left: 8px;
            }}
            .status-ported {{
                color: var(--accent-strong);
            }}
            .status-missing {{
                color: var(--muted);
            }}
            .status-excluded {{
                color: var(--muted);
                font-style: italic;
            }}
            td code {{
                font-size: 13px;
            }}
            .meta {{
                color: var(--muted);
                margin-bottom: 24px;
            }}
        </style>
    </head>
    <body>
        <main class="shell">
            <header>
                <div class="title-row">
                    <a class="home-link" href="../">music21-rs</a>
                    <h1>Reports</h1>
                </div>
                <div class="top-links">
                    <a href="../docs/music21_rs/index.html">Rust docs</a>
                </div>
            </header>
            <p class="meta">Generated from commit <code>{head}</code> against music21 {version}.</p>
"#,
        head = escape(&report.generated_from),
        version = escape(&report.music21_version),
    );

    if let Some(coverage) = &report.coverage {
        let _ = write!(
            html,
            "            <h2>Test coverage</h2>\n            <div class=\"summary\">\n"
        );
        for (label, percent) in [
            ("lines", coverage.lines),
            ("functions", coverage.functions),
            ("regions", coverage.regions),
        ] {
            let _ = write!(
                html,
                r#"                <div class="panel">
                    <span class="number">{:.1}%</span>
                    <span class="label">of {label}, {} of {}</span>
                    <div class="bar"><span style="width: {:.1}%"></span></div>
                </div>
"#,
                percent.percent, percent.covered, percent.count, percent.percent
            );
        }
        let _ = write!(
            html,
            "            </div>\n            <p>The library's own unit tests, measured by <code>cargo llvm-cov</code>, with the generated chord and Scala tables left out. <a href=\"./coverage/html/index.html\">File by file</a>.</p>\n"
        );
    }

    if !report.features.is_empty() {
        let ported: usize = report.features.iter().map(|c| c.ported).sum();
        let missing: usize = report.features.iter().map(|c| c.missing).sum();
        let excluded: usize = report.features.iter().map(|c| c.excluded).sum();
        let total = ported + missing;
        let percent = if total == 0 {
            0.0
        } else {
            100.0 * ported as f64 / total as f64
        };
        let _ = write!(
            html,
            r#"            <h2>Ported from music21</h2>
            <div class="summary">
                <div class="panel">
                    <span class="number">{percent:.1}%</span>
                    <span class="label">of the members in scope, {ported} of {total}</span>
                    <div class="bar"><span style="width: {percent:.1}%"></span></div>
                </div>
                <div class="panel">
                    <span class="number">{missing}</span>
                    <span class="label">not ported yet</span>
                </div>
                <div class="panel">
                    <span class="number">{excluded}</span>
                    <span class="label">left out on purpose</span>
                </div>
            </div>
            <p>Every public method of the music21 classes the crate ports, read from the submodule, against the <code>pub fn</code>s of the Rust files that port them. Members left out on purpose say why; the mapping lives in <code>data/feature_map.toml</code>.</p>
"#
        );
        for class in &report.features {
            let counted = class.ported + class.missing;
            let class_percent = if counted == 0 {
                0.0
            } else {
                100.0 * class.ported as f64 / counted as f64
            };
            let _ = write!(
                html,
                r#"            <details>
                <summary>{name}<span class="count">{ported} of {counted} ported ({class_percent:.0}%), {excluded} excluded</span></summary>
                <div class="bar"><span style="width: {class_percent:.1}%"></span></div>
                <p class="meta"><code>{python}</code>{note}</p>
                <table>
                    <thead><tr><th>music21</th><th>status</th><th>music21-rs</th></tr></thead>
                    <tbody>
"#,
                name = escape(&class.name),
                ported = class.ported,
                excluded = class.excluded,
                python = escape(&class.python),
                note = class
                    .note
                    .as_deref()
                    .map(|note| format!(" — {}", escape(note)))
                    .unwrap_or_default(),
            );
            for member in &class.members {
                let (status, detail) = match member.status {
                    Status::Ported => (
                        "ported",
                        format!(
                            "<code>{}</code>",
                            escape(member.detail.as_deref().unwrap_or(""))
                        ),
                    ),
                    Status::Missing => ("missing", String::new()),
                    Status::Excluded => {
                        ("excluded", escape(member.detail.as_deref().unwrap_or("")))
                    }
                };
                let _ = writeln!(
                    html,
                    "                        <tr><td><code>{}</code></td><td class=\"status-{status}\">{status}</td><td>{detail}</td></tr>\n",
                    escape(&member.name)
                );
            }
            let _ = write!(
                html,
                "                    </tbody>\n                </table>\n            </details>\n"
            );
        }
    }

    html.push_str("        </main>\n        <script type=\"module\" src=\"../theme.js\"></script>\n    </body>\n</html>\n");
    html
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case_follows_the_crate_convention() {
        assert_eq!(snake_case("isMajorTriad"), "is_major_triad");
        assert_eq!(snake_case("pitchClasses"), "pitch_classes");
        assert_eq!(snake_case("forteClassTnI"), "forte_class_tn_i");
        assert_eq!(snake_case("hasZRelation"), "has_z_relation");
        assert_eq!(snake_case("areZRelations"), "are_z_relations");
        assert_eq!(
            snake_case("correctRNAlterationForMinor"),
            "correct_rn_alteration_for_minor"
        );
        assert_eq!(snake_case("root"), "root");
        assert_eq!(snake_case("mod7inversion"), "mod7inversion");
        assert_eq!(
            snake_case("getHistoricalRowByName"),
            "get_historical_row_by_name"
        );
    }

    #[test]
    fn a_leading_get_is_optional() {
        let none = BTreeMap::new();
        assert_eq!(
            candidates("getChordStep", &none, Members::Methods),
            ["get_chord_step", "chord_step"]
        );
        let mut renames = BTreeMap::new();
        renames.insert("forteClassTnI".to_string(), "forte_class_tni".to_string());
        assert_eq!(
            candidates("forteClassTnI", &renames, Members::Methods),
            ["forte_class_tni", "forte_class_tn_i"]
        );
    }

    #[test]
    fn methods_are_read_per_class_without_privates_or_duplicates() {
        let python = "class A(Base):\n    def one(self):\n        pass\n    @property\n    def two(self):\n        pass\n    @two.setter\n    def two(self, v):\n        pass\n    def _hidden(self):\n        pass\nclass B:\n    def three(self):\n        pass\ndef top():\n    pass\n";
        assert_eq!(class_methods(python, "A"), ["one", "two"]);
        assert_eq!(class_methods(python, "B"), ["three"]);
        assert_eq!(module_functions(python), ["top"]);
        assert_eq!(module_classes(python, None), ["A", "B"]);
        assert_eq!(module_classes(python, Some("B")), ["B"]);
    }

    #[test]
    fn definitions_are_found_by_kind() {
        let rust = "impl X {\n    pub fn is_triad(&self) -> bool {}\n    pub const fn len() -> usize {}\n    fn private(&self) {}\n}\npub enum ScaleType { Major, HarmonicMinor }\n";
        assert!(defines(rust, "is_triad", Members::Methods));
        assert!(defines(rust, "len", Members::Methods));
        assert!(!defines(rust, "private", Members::Methods));
        assert!(!defines(rust, "is_tria", Members::Methods));
        assert!(defines(rust, "HarmonicMinor", Members::Classes));
        assert!(!defines(rust, "Minor", Members::Classes));
    }
}
