//! The crate's public surface, and what of it music21 has no counterpart for.
//!
//! The rest of the report measures music21-rs against music21: every public
//! member of the classes it ports, matched to a `pub fn`. This is the same
//! question asked the other way round — every public member the crate has,
//! less the ones some music21 member already claims. What is left is the part
//! of the crate music21 has no name for.
//!
//! Derived rather than written down, so it cannot go stale. That costs
//! precision in one direction: a member is "beyond music21" here whenever the
//! feature map does not claim it, which includes a genuine port the map has
//! not been told about. The map failing on a stale rename is what keeps that
//! honest from the other side.
//!
//! What counts as public is the module tree walked from `src/lib.rs` through
//! `pub mod` alone. A `pub fn` inside a private support module — `common`,
//! `defaults`, `display` — is not public API and is not here.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Generated files, which are data. Their thousands of items would say
/// nothing about what the crate does that music21 does not.
const GENERATED: [&str; 3] = [
    "generated.rs",
    "scala_bundled.rs",
    "temperaments_generated.rs",
];

/// What kind of thing a member is, which is worth showing because the list
/// mixes types and functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Function,
    Method,
    Struct,
    Enum,
    Trait,
    Type,
    Constant,
}

impl Kind {
    /// How the page names it.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Function => "fn",
            Kind::Method => "method",
            Kind::Struct => "struct",
            Kind::Enum => "enum",
            Kind::Trait => "trait",
            Kind::Type => "type",
            Kind::Constant => "const",
        }
    }
}

/// One public member of the crate.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Member {
    /// `Chord::resolution_suggestions`, or a bare name for a free item.
    pub name: String,
    /// The bare identifier, which is what a music21 member would claim.
    pub bare: String,
    pub kind: Kind,
}

/// One public module, and what it has that music21 does not.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeyondModule {
    /// `tuningsystem`, `chord`, `tuningsystem::scala`.
    pub module: String,
    pub members: Vec<Member>,
}

/// Every public member of the crate that no music21 member claims, by module.
///
/// `claimed` is the set of `(rust file, name)` pairs the feature map matched.
/// `known` is every name music21 uses anywhere, matched or not: a member whose
/// name music21 also has is not something music21 lacks, even where the crate
/// hangs it off a different type than music21 does.
pub fn beyond_music21(
    workspace_root: &Path,
    claimed: &BTreeSet<(String, String)>,
    known: &BTreeSet<String>,
) -> Vec<BeyondModule> {
    let mut out = Vec::new();
    for (module, path) in public_modules(workspace_root) {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let relative = relative_to(workspace_root, &path);
        let mut members: Vec<Member> = public_members(&text)
            .into_iter()
            .filter(|member| {
                !known.contains(&member.bare)
                    && !claimed.contains(&(relative.clone(), member.bare.clone()))
            })
            .collect();
        if members.is_empty() {
            continue;
        }
        members.sort();
        members.dedup();
        out.push(BeyondModule { module, members });
    }
    out.sort_by(|a, b| {
        b.members
            .len()
            .cmp(&a.members.len())
            .then(a.module.cmp(&b.module))
    });
    out
}

/// Every module reachable from `src/lib.rs` through `pub mod` alone, as
/// `(module path, file)`.
///
/// A `mod` without `pub` is a crate-private support module, and nothing under
/// it is public API however it is declared inside.
pub fn public_modules(workspace_root: &Path) -> Vec<(String, PathBuf)> {
    let src = workspace_root.join("src");
    let mut found = Vec::new();
    let mut queue = vec![(String::new(), src.join("lib.rs"))];
    while let Some((prefix, path)) = queue.pop() {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        if !prefix.is_empty() {
            found.push((prefix.clone(), path.clone()));
        }
        let directory = module_directory(&path);
        for child in declared_public_modules(&text) {
            let name = if prefix.is_empty() {
                child.clone()
            } else {
                format!("{prefix}::{child}")
            };
            let file = directory.join(format!("{child}.rs"));
            let folder = directory.join(&child).join("mod.rs");
            let file = if file.is_file() {
                file
            } else if folder.is_file() {
                folder
            } else {
                continue;
            };
            if GENERATED.iter().any(|generated| file.ends_with(generated)) {
                continue;
            }
            queue.push((name, file));
        }
    }
    found.sort();
    found.dedup();
    found
}

/// Where a module's children live: beside `foo.rs`, or inside `foo/`.
fn module_directory(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    match path.file_name().and_then(|name| name.to_str()) {
        Some("mod.rs") | Some("lib.rs") => parent,
        Some(name) => parent.join(name.trim_end_matches(".rs")),
        None => parent,
    }
}

fn relative_to(workspace_root: &Path, path: &Path) -> String {
    path.strip_prefix(workspace_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// The `pub mod name;` declarations of one file.
pub fn declared_public_modules(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in body(text).lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("pub mod ") else {
            continue;
        };
        // `pub mod foo;` only. An inline `pub mod foo {` is scanned as part
        // of the file it is written in, so it needs no file of its own.
        if let Some(name) = rest.strip_suffix(';') {
            out.push(name.trim().to_string());
        }
    }
    out
}

/// Every public item one file declares.
pub fn public_members(text: &str) -> Vec<Member> {
    let mut out = Vec::new();
    let mut owner: Option<String> = None;
    let mut depth: i32 = 0;
    let mut owner_depth: i32 = 0;

    for line in body(text).lines() {
        let trimmed = line.trim();
        // An inherent `impl Chord {` says whose the methods below it are; a
        // trait impl has no `pub` members, so it never contributes.
        if trimmed.starts_with("impl ") || trimmed.starts_with("impl<") {
            if let Some(name) = inherent_impl_target(trimmed) {
                owner = Some(name);
                owner_depth = depth;
            } else {
                owner = None;
            }
        }
        if let Some(member) = declared_item(trimmed, owner.as_deref()) {
            out.push(member);
        }
        depth += trimmed.matches('{').count() as i32;
        depth -= trimmed.matches('}').count() as i32;
        if owner.is_some() && depth <= owner_depth {
            owner = None;
        }
    }
    out
}

/// The type an inherent `impl` block is for, or nothing for a trait impl.
///
/// `impl Chord {` and `impl<'a> Iterator for Thing {` are told apart by the
/// `for`: only the first has members of its own to declare.
fn inherent_impl_target(line: &str) -> Option<String> {
    let rest = line.strip_prefix("impl")?;
    let rest = match rest.strip_prefix('<') {
        // Skip the generic parameter list, which may itself hold `>`.
        Some(generics) => {
            let mut depth = 1;
            let mut end = None;
            for (index, c) in generics.char_indices() {
                match c {
                    '<' => depth += 1,
                    '>' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(index);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            &generics[end? + 1..]
        }
        None => rest,
    };
    let rest = rest.trim();
    let head: &str = rest.split_whitespace().next()?;
    if rest.split_whitespace().any(|word| word == "for") {
        return None;
    }
    let name = head
        .trim_end_matches('{')
        .split('<')
        .next()
        .unwrap_or(head)
        .trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// The public item a line declares, if it declares one.
fn declared_item(line: &str, owner: Option<&str>) -> Option<Member> {
    // `pub(crate)` and `pub(super)` are not public API.
    let rest = line.strip_prefix("pub ")?;
    // Longest first, so `const fn` is not read as a `const`.
    const DECLARATIONS: [(&str, Kind); 9] = [
        ("const fn ", Kind::Function),
        ("unsafe fn ", Kind::Function),
        ("fn ", Kind::Function),
        ("struct ", Kind::Struct),
        ("enum ", Kind::Enum),
        ("trait ", Kind::Trait),
        ("type ", Kind::Type),
        ("const ", Kind::Constant),
        ("static ", Kind::Constant),
    ];
    let (kind, rest) = DECLARATIONS
        .iter()
        .find_map(|(prefix, kind)| Some((*kind, rest.strip_prefix(prefix)?)))?;
    let bare: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if bare.is_empty() {
        return None;
    }
    let (kind, name) = match (kind, owner) {
        (Kind::Function, Some(owner)) => (Kind::Method, format!("{owner}::{bare}")),
        _ => (kind, bare.clone()),
    };
    Some(Member { name, bare, kind })
}

/// A file without its tests.
///
/// Every test in this repository is a `#[cfg(test)] mod tests` block at the
/// bottom of its own file, so cutting there is enough and needs no brace
/// counting. A file with no such block is returned whole.
fn body(text: &str) -> &str {
    match text.find("\nmod tests {") {
        Some(index) => &text[..index],
        None => text,
    }
}

/// How many members each module has, for the summary line.
pub fn totals(modules: &[BeyondModule]) -> (usize, BTreeMap<Kind, usize>) {
    let mut by_kind = BTreeMap::new();
    let mut total = 0;
    for module in modules {
        for member in &module.members {
            *by_kind.entry(member.kind).or_insert(0) += 1;
            total += 1;
        }
    }
    (total, by_kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_public_modules_are_walked() {
        let text = "pub mod chord;\nmod defaults;\npub mod pitch;\npub mod inline { }\n";
        assert_eq!(declared_public_modules(text), ["chord", "pitch"]);
    }

    #[test]
    fn an_inherent_impl_owns_the_methods_below_it() {
        let text = "\
impl Chord {
    pub fn resolution_suggestions(&self) -> u8 { 0 }
}
pub fn free_standing() {}
";
        let found = public_members(text);
        assert_eq!(found[0].name, "Chord::resolution_suggestions");
        assert_eq!(found[0].bare, "resolution_suggestions");
        assert_eq!(found[0].kind, Kind::Method);
        assert_eq!(found[1].name, "free_standing");
        assert_eq!(found[1].kind, Kind::Function);
    }

    #[test]
    fn a_trait_impl_declares_nothing_of_its_own() {
        assert_eq!(inherent_impl_target("impl Chord {"), Some("Chord".into()));
        assert_eq!(
            inherent_impl_target("impl<'a> Scale<'a> {"),
            Some("Scale".into())
        );
        assert!(inherent_impl_target("impl Display for Chord {").is_none());
        assert!(inherent_impl_target("impl<T: Into<u8>> From<T> for Chord {").is_none());
    }

    #[test]
    fn crate_private_items_are_not_public_api() {
        assert!(declared_item("pub(crate) fn helper()", None).is_none());
        assert!(declared_item("pub(super) struct Inner;", None).is_none());
        assert!(declared_item("fn private()", None).is_none());
        assert!(declared_item("pub use chord::Chord;", None).is_none());
    }

    #[test]
    fn every_kind_of_item_is_recognised() {
        let kinds = |line: &str| declared_item(line, None).map(|member| member.kind);
        assert_eq!(kinds("pub struct Monzo {"), Some(Kind::Struct));
        assert_eq!(kinds("pub enum Kind {"), Some(Kind::Enum));
        assert_eq!(kinds("pub trait IntoNotes {"), Some(Kind::Trait));
        assert_eq!(kinds("pub type Result<T> = ..."), Some(Kind::Type));
        assert_eq!(
            kinds("pub const PRIMES: [u32; 4] = ["),
            Some(Kind::Constant)
        );
        assert_eq!(
            kinds("pub const fn one() -> u8 { 1 }"),
            Some(Kind::Function)
        );
    }

    #[test]
    fn the_tests_at_the_bottom_are_not_part_of_the_surface() {
        let text = "pub fn real() {}\nmod tests {\n    pub fn not_real() {}\n}\n";
        let found = public_members(text);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "real");
    }

    #[test]
    fn an_impl_stops_owning_once_its_block_closes() {
        let text = "\
impl Chord {
    pub fn inside(&self) {}
}

pub fn outside() {}
";
        let found = public_members(text);
        assert_eq!(found[0].name, "Chord::inside");
        assert_eq!(found[1].name, "outside");
    }
}
