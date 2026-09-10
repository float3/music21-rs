//! Reading the ported feature set off music21 and the crate.
//!
//! `data/feature_map.toml` names the classes the crate ports and the
//! Rust files each lives in. This walks the submodule for every public
//! member of those classes, converts each name the way the crate does,
//! and looks for a `pub fn` of that name — with the renames and
//! exclusions the map carries, and an error for a rename or exclusion
//! that has gone stale.

use super::*;

/// The declared length of a `const NAME: [T; N]`, which is the one place a
/// table's size is written down and cannot be wrong.
pub(super) fn declared_length(rust: &str, name: &str) -> Option<usize> {
    let start = rust.find(&format!("{name}:"))? + name.len() + 1;
    let rest = &rust[start..];
    let close = rest.find(']')?;
    let (_, count) = rest[..close].rsplit_once(';')?;
    count.trim().parse().ok()
}

/// Counts what the crate has beyond music21, reading each number out of the
/// source so none of them can go stale silently.
pub(super) fn scan_beyond(
    workspace_root: &Path,
    map: &[BeyondMap],
) -> Result<Vec<BeyondReport>, Box<dyn Error>> {
    let mut reports = Vec::new();
    for entry in map {
        let mut count = None;
        if let Some(constant) = &entry.count {
            let relative = entry
                .rust
                .as_deref()
                .ok_or_else(|| format!("{}: a count needs a rust file", entry.name))?;
            let path = workspace_root.join(relative);
            let rust = fs::read_to_string(&path)
                .map_err(|err| format!("{} is unreadable: {err}", path.display()))?;
            count = Some(declared_length(&rust, constant).ok_or_else(|| {
                format!(
                    "{}: {relative} declares no `const {constant}: [_; N]`",
                    entry.name
                )
            })?);
        }
        let mut music21 = None;
        if entry.scala_archive {
            let archive = workspace_root.join("data/scala_archive.toml");
            let text = fs::read_to_string(&archive)
                .map_err(|err| format!("{} is unreadable: {err}", archive.display()))?;
            count = Some(
                text.matches(
                    "
[[scale]]",
                )
                .count()
                    + text.starts_with("[[scale]]") as usize,
            );
            music21 = count_scl_files(&workspace_root.join("music21/music21"));
        }
        reports.push(BeyondReport {
            name: entry.name.clone(),
            note: entry.note.clone(),
            count,
            unit: entry.unit.clone(),
            music21,
        });
    }
    Ok(reports)
}

/// music21's own Scala archive, counted where it ships.
pub(super) fn count_scl_files(root: &Path) -> Option<usize> {
    let mut found = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).ok()?.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "scl") {
                found += 1;
            }
        }
    }
    (found > 0).then_some(found)
}

/// Which `(rust file, name)` pairs some music21 member already accounts for.
///
/// The ported list matches a music21 member against the whole of a class's
/// Rust files at once, so which file it landed in is asked again here: a
/// member is only excused on the file it is actually declared in, or a name
/// that means two different things in two modules would be excused twice
/// over by one port.
pub(super) fn claimed_names(
    workspace_root: &Path,
    map: &FeatureMap,
) -> Result<Claims, Box<dyn Error>> {
    let mut ported = BTreeSet::new();
    let mut known = BTreeSet::new();
    for class in &map.class {
        let python_path = workspace_root.join("music21/music21").join(&class.python);
        let Ok(python) = fs::read_to_string(&python_path) else {
            continue;
        };
        let members = match class.members {
            Members::Methods => match &class.name {
                Some(name) => class_methods(&python, name),
                None => module_functions(&python),
            },
            Members::Classes => module_classes(&python, class.suffix.as_deref()),
        };
        // Every name music21 uses anywhere, whatever the crate did with it.
        // music21 reaches a member through inheritance that its own class body
        // never mentions — `addLyric` is a `Chord` member and is written in
        // `GeneralNote` — so a file-scoped claim alone would announce
        // `Chord::add_lyric` as something music21 has no counterpart for.
        for member in &members {
            known.extend(candidates(member, &class.renames, class.members));
        }
        // The class itself, so that `Chord` is not announced as something
        // music21 has no counterpart for.
        if let Some(name) = &class.name {
            known.insert(name.clone());
            known.insert(snake_case(name));
        }
        for file in &class.rust {
            let Ok(rust) = fs::read_to_string(workspace_root.join(file)) else {
                continue;
            };
            for member in &members {
                for candidate in candidates(member, &class.renames, class.members) {
                    if defines(&rust, &candidate, class.members) {
                        ported.insert((file.clone(), candidate));
                    }
                }
            }
        }
    }
    Ok(Claims { ported, known })
}

pub(super) fn scan_features(
    workspace_root: &Path,
    map: &FeatureMap,
) -> Result<Vec<ClassReport>, Box<dyn Error>> {
    let mut classes = Vec::new();
    let mut problems = Vec::new();
    let facade = Facade::read(workspace_root);

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

        // What the wheel carries for this class, under music21's own names.
        // A class the facade does not stand in for at all has nothing here,
        // and every member of it counts as missing from the wheel.
        let wheel = match (&class.name, class.members) {
            (Some(name), Members::Methods) => {
                facade.class_body(class.facade.as_deref().unwrap_or(name))
            }
            _ => None,
        };

        let mut reports = Vec::new();
        for member in &members {
            let in_wheel = match (&wheel, class.members) {
                (Some(body), _) => facade::defines_member(body, member),
                // A module's own functions live wherever the facade put them,
                // and a class is either there or it is not.
                (None, Members::Methods) if class.name.is_none() => facade.has_function(member),
                (None, Members::Classes) => facade.has_class(member),
                (None, _) => false,
            };
            let report = if let Some(reason) = class.excluded.get(member) {
                // Left out on purpose is still left out. The reason is worth
                // showing; carving it out of the total is not.
                MemberReport {
                    name: member.clone(),
                    status: Status::Missing,
                    detail: Some(reason.clone()),
                    in_wheel,
                }
            } else if let Some(found) = candidates(member, &class.renames, class.members)
                .into_iter()
                .find(|candidate| defines(&rust, candidate, class.members))
            {
                MemberReport {
                    name: member.clone(),
                    status: Status::Ported,
                    detail: Some(found),
                    in_wheel,
                }
            } else {
                MemberReport {
                    name: member.clone(),
                    status: Status::Missing,
                    detail: None,
                    in_wheel,
                }
            };
            reports.push(report);
        }

        let count = |status: Status| reports.iter().filter(|r| r.status == status).count();
        let in_wheel = reports.iter().filter(|r| r.in_wheel).count();
        classes.push(ClassReport {
            python: class.python.clone(),
            name: label,
            note: class.note.clone(),
            ported: count(Status::Ported),
            missing: count(Status::Missing),
            in_wheel,
            members: reports,
        });
    }

    if !problems.is_empty() {
        return Err(format!(
            "data/feature_map.toml is out of date:\n  {}",
            problems.join("\n  ")
        )
        .into());
    }
    Ok(classes)
}

/// The public methods and properties of one class, in source order, each once.
pub(super) fn class_methods(python: &str, class: &str) -> Vec<String> {
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
pub(super) fn module_functions(python: &str) -> Vec<String> {
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
pub(super) fn module_classes(python: &str, suffix: Option<&str>) -> Vec<String> {
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

pub(super) fn method_name(line: &str, prefix: &str) -> Option<String> {
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

pub(super) fn push_unique(names: &mut Vec<String>, name: String) {
    if !names.contains(&name) {
        names.push(name);
    }
}

/// The Rust names a music21 member might have been given, most specific
/// first.
pub(super) fn candidates(
    member: &str,
    renames: &BTreeMap<String, String>,
    members: Members,
) -> Vec<String> {
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
pub(super) fn snake_case(name: &str) -> String {
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
pub(super) fn defines(rust: &str, name: &str, members: Members) -> bool {
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
