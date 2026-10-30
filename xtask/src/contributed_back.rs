//! Brings the READMEs' contributed-back list up to date with GitHub.
//!
//! Every entry that links a GitHub pull request or issue says "(open)" while
//! it is open and stops saying it once it is merged or closed. A pull request
//! the contributor has opened on music21 that the list does not have yet is
//! added, with its title as its description, among the other music21 pull
//! requests in number order. Issues are never added: an issue can be a
//! question rather than a contribution, so which ones belong is left to
//! whoever edits the list.
//!
//! Both READMEs hold the same list and get the same changes. GitHub is asked
//! through `gh`, which has to be signed in.

use std::collections::BTreeMap;
use std::error::Error;
use std::path::Path;
use std::process::Command;

use serde::Deserialize;

const READMES: [&str; 2] = ["README.md", "python/README.md"];
const MUSIC21: &str = "cuthbertLab/music21";
const CONTRIBUTOR: &str = "float3";
const HEADING: &str = "contributed upstream, mostly fixes found while porting:";
const WIDTH: usize = 76;

/// A pull request or issue an entry links.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Link {
    repo: String,
    number: u64,
    issue: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Open,
    Merged,
    Closed,
}

/// A music21 pull request the list does not have yet.
struct Missing {
    number: u64,
    title: String,
    open: bool,
}

/// Rewrites both READMEs, or with `check` only says whether they are stale.
/// The exit code is 1 when `check` finds anything to change.
pub fn run(root: &Path, check: bool) -> Result<i32, Box<dyn Error>> {
    let mut texts = Vec::new();
    for readme in READMES {
        texts.push((readme, std::fs::read_to_string(root.join(readme))?));
    }

    let mut states = BTreeMap::new();
    for (_, text) in &texts {
        let lines: Vec<&str> = text.lines().collect();
        for (start, _) in entries(&lines)? {
            if let Some(link) = link(lines[start])
                && let std::collections::btree_map::Entry::Vacant(entry) = states.entry(link)
            {
                let state = state_of(entry.key())?;
                entry.insert(state);
            }
        }
    }

    let listed: Vec<u64> = states
        .keys()
        .filter(|link| link.repo == MUSIC21 && !link.issue)
        .map(|link| link.number)
        .collect();
    let mut missing = Vec::new();
    for pr in contributor_prs()? {
        let known = listed.contains(&pr.number);
        match (known, pr.state.as_str()) {
            (true, "CLOSED") => println!(
                "{MUSIC21}#{} was closed without merging; it is still listed",
                pr.number
            ),
            (false, "OPEN" | "MERGED") => missing.push(Missing {
                number: pr.number,
                title: pr.title,
                open: pr.state == "OPEN",
            }),
            _ => {}
        }
    }
    missing.sort_by_key(|pr| pr.number);

    let mut stale = false;
    for (readme, text) in &texts {
        let lines: Vec<&str> = text.lines().collect();
        let (updated, changes) = update(&lines, &states, &missing)?;
        for change in &changes {
            println!("{readme}: {change}");
        }
        if !changes.is_empty() {
            stale = true;
            if !check {
                let mut out = updated.join("\n");
                out.push('\n');
                std::fs::write(root.join(readme), out)?;
            }
        }
    }
    if !stale {
        println!("the contributed-back list is up to date");
    }
    Ok(if check && stale { 1 } else { 0 })
}

fn gh(args: &[&str]) -> Result<String, Box<dyn Error>> {
    let output = Command::new("gh").args(args).output()?;
    if !output.status.success() {
        return Err(format!(
            "gh {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

fn state_of(link: &Link) -> Result<State, Box<dyn Error>> {
    #[derive(Deserialize)]
    struct Found {
        state: String,
        merged_at: Option<String>,
    }
    let kind = if link.issue { "issues" } else { "pulls" };
    let found: Found = serde_json::from_str(&gh(&[
        "api",
        &format!("repos/{}/{kind}/{}", link.repo, link.number),
    ])?)?;
    Ok(match (found.state.as_str(), found.merged_at) {
        ("open", _) => State::Open,
        (_, Some(_)) => State::Merged,
        _ => State::Closed,
    })
}

#[derive(Deserialize)]
struct Pr {
    number: u64,
    title: String,
    state: String,
}

fn contributor_prs() -> Result<Vec<Pr>, Box<dyn Error>> {
    Ok(serde_json::from_str(&gh(&[
        "pr",
        "list",
        "-R",
        MUSIC21,
        "--author",
        CONTRIBUTOR,
        "--state",
        "all",
        "--limit",
        "1000",
        "--json",
        "number,title,state",
    ])?)?)
}

/// The list's entries, each as the index of its first line and the index
/// after its last.
fn entries(lines: &[&str]) -> Result<Vec<(usize, usize)>, Box<dyn Error>> {
    let heading = lines
        .iter()
        .position(|line| *line == HEADING)
        .ok_or("the README has no contributed-back list")?;
    let mut spans = Vec::new();
    let mut index = heading + 2;
    while lines.get(index).is_some_and(|line| line.starts_with("- ")) {
        let mut end = index + 1;
        while lines.get(end).is_some_and(|line| line.starts_with("  ")) {
            end += 1;
        }
        spans.push((index, end));
        index = end;
    }
    if spans.is_empty() {
        return Err("the contributed-back list has no entries".into());
    }
    Ok(spans)
}

/// The pull request or issue an entry's first line links, as in
/// `- [owner/repo#12](https://github.com/owner/repo/pull/12):` or
/// `- issue [owner/repo#12](https://github.com/owner/repo/issues/12):`.
fn link(line: &str) -> Option<Link> {
    let (issue, rest) = match line.strip_prefix("- issue [") {
        Some(rest) => (true, rest),
        None => (false, line.strip_prefix("- [")?),
    };
    let (name, rest) = rest.split_once("](https://github.com/")?;
    let (repo, number) = name.split_once('#')?;
    let number: u64 = number.parse().ok()?;
    let kind = if issue { "issues" } else { "pull" };
    (rest == format!("{repo}/{kind}/{number}):")).then(|| Link {
        repo: repo.to_string(),
        number,
        issue,
    })
}

/// The description with " (open)" before its final full stop exactly when
/// `open`.
fn described(text: &str, open: bool) -> String {
    let body = text.strip_suffix('.').unwrap_or(text);
    let body = body.strip_suffix("(open)").unwrap_or(body).trim_end();
    if open {
        format!("{body} (open).")
    } else {
        format!("{body}.")
    }
}

/// The text as indented lines of at most `WIDTH` characters where its words
/// allow.
fn wrapped(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::from(" ");
    for word in text.split_whitespace() {
        if line.len() > 2 && line.len() + 1 + word.len() > WIDTH {
            lines.push(line);
            line = String::from(" ");
        }
        line.push(' ');
        line.push_str(word);
    }
    lines.push(line);
    lines
}

/// The README's lines with each linked entry's state set and the missing
/// pull requests added, and a line saying what changed for each change.
fn update(
    lines: &[&str],
    states: &BTreeMap<Link, State>,
    missing: &[Missing],
) -> Result<(Vec<String>, Vec<String>), Box<dyn Error>> {
    let spans = entries(lines)?;
    let mut out: Vec<String> = lines[..spans[0].0]
        .iter()
        .map(|line| line.to_string())
        .collect();
    let mut changes = Vec::new();
    // Each music21 pull request already in `out`, with the index of its
    // first line there.
    let mut music21_prs: Vec<(u64, usize)> = Vec::new();
    for &(start, end) in &spans {
        let mut body: Vec<String> = lines[start + 1..end]
            .iter()
            .map(|line| line.to_string())
            .collect();
        if let Some(link) = link(lines[start]) {
            let state = states
                .get(&link)
                .copied()
                .ok_or_else(|| format!("no state for {}#{}", link.repo, link.number))?;
            let text = body
                .iter()
                .map(|line| line.trim())
                .collect::<Vec<_>>()
                .join(" ");
            let new = described(&text, state == State::Open);
            if new != text {
                body = wrapped(&new);
                changes.push(format!(
                    "{}#{} is {}",
                    link.repo,
                    link.number,
                    format!("{state:?}").to_lowercase()
                ));
            }
            if link.repo == MUSIC21 && !link.issue {
                music21_prs.push((link.number, out.len()));
            }
        }
        out.push(lines[start].to_string());
        out.extend(body);
    }
    let tail = spans[spans.len() - 1].1;
    for pr in missing {
        let at = match music21_prs
            .iter()
            .filter(|(number, _)| *number < pr.number)
            .max_by_key(|(number, _)| *number)
        {
            Some(&(_, first)) => {
                let mut after = first + 1;
                while out.get(after).is_some_and(|line| line.starts_with("  ")) {
                    after += 1;
                }
                after
            }
            None => music21_prs
                .iter()
                .map(|&(_, first)| first)
                .min()
                .unwrap_or(spans[0].0),
        };
        let title = pr.title.trim().trim_end_matches('.');
        let mut block = vec![format!(
            "- [{MUSIC21}#{0}](https://github.com/{MUSIC21}/pull/{0}):",
            pr.number
        )];
        block.extend(wrapped(&described(&format!("{title}."), pr.open)));
        for (_, first) in &mut music21_prs {
            if *first >= at {
                *first += block.len();
            }
        }
        music21_prs.push((pr.number, at));
        out.splice(at..at, block);
        changes.push(format!("{MUSIC21}#{} added", pr.number));
    }
    out.extend(lines[tail..].iter().map(|line| line.to_string()));
    Ok((out, changes))
}

#[cfg(test)]
mod tests {
    use super::*;

    const README: &str = "\
intro

contributed upstream, mostly fixes found while porting:

- [cuthbertLab/music21#10](https://github.com/cuthbertLab/music21/pull/10):
  a first fix (open).
- [cuthbertLab/music21#30](https://github.com/cuthbertLab/music21/pull/30):
  a third fix.
- issue [cuthbertLab/music21#40](https://github.com/cuthbertLab/music21/issues/40):
  a reported bug (open).
- corrections elsewhere.

after
";

    fn link(repo: &str, number: u64, issue: bool) -> Link {
        Link {
            repo: repo.to_string(),
            number,
            issue,
        }
    }

    fn states(pr10: State, pr30: State, issue40: State) -> BTreeMap<Link, State> {
        BTreeMap::from([
            (link(MUSIC21, 10, false), pr10),
            (link(MUSIC21, 30, false), pr30),
            (link(MUSIC21, 40, true), issue40),
        ])
    }

    #[test]
    fn an_up_to_date_list_is_left_as_it_is() {
        let lines: Vec<&str> = README.lines().collect();
        let (out, changes) = update(
            &lines,
            &states(State::Open, State::Merged, State::Open),
            &[],
        )
        .unwrap();
        assert!(changes.is_empty());
        assert_eq!(out, lines);
    }

    #[test]
    fn open_follows_each_entry_s_state() {
        let lines: Vec<&str> = README.lines().collect();
        let (out, changes) = update(
            &lines,
            &states(State::Merged, State::Open, State::Closed),
            &[],
        )
        .unwrap();
        assert_eq!(changes.len(), 3);
        let text = out.join("\n");
        assert!(text.contains("  a first fix.\n"));
        assert!(text.contains("  a third fix (open).\n"));
        assert!(text.contains("  a reported bug.\n"));
    }

    #[test]
    fn a_missing_pull_request_goes_in_number_order() {
        let lines: Vec<&str> = README.lines().collect();
        let missing = [
            Missing {
                number: 20,
                title: "Fix the second thing".to_string(),
                open: true,
            },
            Missing {
                number: 50,
                title: "Fix the fifth thing.".to_string(),
                open: false,
            },
        ];
        let (out, changes) = update(
            &lines,
            &states(State::Open, State::Merged, State::Open),
            &missing,
        )
        .unwrap();
        assert_eq!(changes.len(), 2);
        let order: Vec<&str> = out
            .iter()
            .filter(|line| line.starts_with("- "))
            .map(String::as_str)
            .collect();
        assert_eq!(
            order,
            [
                "- [cuthbertLab/music21#10](https://github.com/cuthbertLab/music21/pull/10):",
                "- [cuthbertLab/music21#20](https://github.com/cuthbertLab/music21/pull/20):",
                "- [cuthbertLab/music21#30](https://github.com/cuthbertLab/music21/pull/30):",
                "- [cuthbertLab/music21#50](https://github.com/cuthbertLab/music21/pull/50):",
                "- issue [cuthbertLab/music21#40](https://github.com/cuthbertLab/music21/issues/40):",
                "- corrections elsewhere.",
            ]
        );
        let text = out.join("\n");
        assert!(text.contains("  Fix the second thing (open).\n"));
        assert!(text.contains("  Fix the fifth thing.\n"));
    }

    #[test]
    fn a_long_description_wraps_within_the_width() {
        let words = "word ".repeat(40);
        let lines = wrapped(words.trim());
        assert!(lines.len() > 1);
        assert!(
            lines
                .iter()
                .all(|line| line.len() <= WIDTH && line.starts_with("  "))
        );
    }
}
