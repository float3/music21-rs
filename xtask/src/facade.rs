//! What the Python wheel has of music21, as distinct from what the crate has.
//!
//! The ported table asks one question: does `music21-rs` carry this member.
//! That is the right question for the crate, which is allowed to leave things
//! out — a value type that answers the same question twice is doing
//! arithmetic, not keeping a cache, and no amount of music21's runtime
//! machinery belongs in it.
//!
//! The wheel is held to a different standard. `music21-rs-python` stands in
//! for music21's own classes in music21's own process; a member it does not
//! carry is one an existing program loses the moment it calls
//! `install_into_music21()`. So it is asked separately, and against music21's
//! own spelling: the facade names its members exactly as music21 does, which
//! is why this looks for `commonName` and not `common_name`.
//!
//! Read from the source rather than from a running interpreter, like the rest
//! of the map. The facade's own install helper knows the true answer at
//! runtime — it lists every unported member and makes it raise — but the
//! report has to say something on a machine with no wheel built.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// The facade's source, file by file.
pub struct Facade {
    files: BTreeMap<String, String>,
}

impl Facade {
    /// Reads `python/src`. An empty set is not an error: a checkout without
    /// it simply has nothing to say about the wheel.
    pub fn read(workspace_root: &Path) -> Self {
        let mut files = BTreeMap::new();
        if let Ok(entries) = fs::read_dir(workspace_root.join("python/src")) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "rs")
                    && let Ok(text) = fs::read_to_string(&path)
                    && let Some(name) = path.file_stem().and_then(|n| n.to_str())
                {
                    files.insert(name.to_string(), text);
                }
            }
        }
        Self { files }
    }

    /// Everything the facade writes for one class: its `impl` blocks, from
    /// whichever file declares it.
    ///
    /// The blocks rather than the file, because one file holds several
    /// classes — `notation.rs` carries `Beam`, `Beams`, `Tie` and `Volume` —
    /// and a whole-file search would credit one class's members to another.
    ///
    /// A subclass carries what it inherits too, since Python finds a
    /// member up the chain: `Trill` extends `Ornament`, whose `impl` holds
    /// `realize`.
    pub fn class_body(&self, class: &str) -> Option<String> {
        let mut body = String::new();
        let mut current = class.to_string();

        // Walk up the chain; the depth bound only guards against a cycle.
        for _ in 0..MAX_INHERITANCE_DEPTH {
            let (text, parent) = self.declaration(&current)?;
            body.push_str(&impl_blocks(text, &current));
            let Some(parent) = parent else {
                return Some(body);
            };
            current = parent;
        }
        Some(body)
    }

    /// The file declaring a class, and the class it extends if any.
    ///
    /// A class is declared either by hand, with `extends = Parent` in its
    /// `#[pyclass]`, or as a row of a table a macro expands:
    /// `(Turn, "Turn", Ornament, [])`.
    fn declaration(&self, class: &str) -> Option<(&str, Option<String>)> {
        for text in self.files.values() {
            if declares(text, class) {
                return Some((text, extends(text, class)));
            }
            if let Some(parent) = table_row_parent(text, class) {
                return Some((text, Some(parent)));
            }
        }
        None
    }

    /// Whether the wheel carries a class of this name at all: declared as a
    /// `#[pyclass]`, or built at import time and listed in the module's
    /// `NAMES`, which is how the twenty concrete scale classes are made.
    pub fn has_class(&self, class: &str) -> bool {
        self.files
            .values()
            .any(|text| declares(text, class) || names_list(text, class))
    }

    /// Whether some facade module carries a free function of this name.
    ///
    /// Module-level functions are searched across the whole facade rather
    /// than in one file: `duration`'s live in `note.rs`, and a mapping from
    /// music21's module to the file that ports it would be one more thing to
    /// keep true.
    pub fn has_function(&self, name: &str) -> bool {
        self.files.values().any(|text| defines_member(text, name))
    }
}

/// Whether a file declares a `#[pyclass]` of this name.
fn declares(text: &str, class: &str) -> bool {
    [
        format!("pub struct {class} "),
        format!("pub struct {class}("),
        format!("pub struct {class};"),
        format!("pub struct {class}\n"),
        format!("pub enum {class} "),
    ]
    .iter()
    .any(|pattern| text.contains(pattern.as_str()))
}

/// How far up a chain of subclasses [`Facade::class_body`] looks.
const MAX_INHERITANCE_DEPTH: usize = 8;

/// The class a hand-declared `#[pyclass]` extends, read off the attribute
/// just above its `pub struct`.
fn extends(text: &str, class: &str) -> Option<String> {
    let at = text.find(&format!("pub struct {class}"))?;
    let attribute = &text[text[..at].rfind("#[pyclass")?..at];
    let (_, rest) = attribute.split_once("extends")?;
    let parent = rest.trim_start().strip_prefix('=')?.trim_start();
    let end = parent
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(parent.len());
    Some(parent[..end].to_string()).filter(|parent| !parent.is_empty())
}

/// The parent a macro table gives a class, from a row written
/// `(Class, "Name", Parent, [...])`, on one line or split over several.
fn table_row_parent(text: &str, class: &str) -> Option<String> {
    let is_ident =
        |field: &str| !field.is_empty() && field.chars().all(|c| c.is_alphanumeric() || c == '_');

    for (at, _) in text.match_indices('(') {
        // The class, the quoted name, the parent: the rest does not matter.
        let mut fields = text[at + 1..].splitn(4, ',').map(str::trim);
        if fields.next() != Some(class) {
            continue;
        }
        if !fields.next().is_some_and(|name| name.starts_with('"')) {
            continue;
        }
        if let Some(parent) = fields.next().filter(|parent| is_ident(parent)) {
            return Some(parent.to_string());
        }
    }
    None
}

/// Whether a file's `NAMES` list carries the class.
fn names_list(text: &str, class: &str) -> bool {
    let Some(start) = text.find("pub const NAMES") else {
        return false;
    };
    let rest = &text[start..];
    let end = rest.find("];").unwrap_or(rest.len());
    rest[..end].contains(&format!("\"{class}\""))
}

/// Every `impl <class>` block in a file, concatenated.
fn impl_blocks(text: &str, class: &str) -> String {
    let mut out = String::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        let opens = trimmed.starts_with(&format!("impl {class} "))
            || trimmed.starts_with(&format!("impl {class}{{"))
            || trimmed == format!("impl {class} {{");
        if !opens {
            continue;
        }
        let mut depth = line.matches('{').count() as i32 - line.matches('}').count() as i32;
        for body in lines.by_ref() {
            out.push_str(body);
            out.push('\n');
            depth += body.matches('{').count() as i32;
            depth -= body.matches('}').count() as i32;
            if depth <= 0 {
                break;
            }
        }
    }
    out
}

/// Whether a body defines a member under music21's own name.
///
/// The facade spells its members exactly as music21 does, so the name is
/// looked for as written. A getter renamed with `#[getter(name)]`, one
/// declared with `#[pyo3(name = "...")]` and one written as `fn get_name`
/// or `fn set_name`, which pyo3 strips the prefix off, all count, since
/// each is what Python sees.
pub fn defines_member(body: &str, name: &str) -> bool {
    [
        format!("fn {name}("),
        format!("fn {name}<"),
        format!("fn get_{name}("),
        format!("fn get_{name}<"),
        format!("fn set_{name}("),
        format!("getter({name})"),
        format!("setter({name})"),
        format!("name = \"{name}\""),
    ]
    .iter()
    .any(|pattern| body.contains(pattern.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "\
#[pyclass]
pub struct Chord {
    inner: RsChord,
}

#[pymethods]
impl Chord {
    #[getter]
    fn commonName(&self) -> String {
        String::new()
    }

    #[getter(_chordAttached)]
    fn get_chordAttached(&self) {}

    #[getter]
    fn get_pitches(&self) {}
}

pub const NAMES: &[&str] = &[
    \"Chord\",
    \"MajorScale\",
];

#[pymethods]
impl Beams {
    fn fill(&self) {}
}
";

    #[test]
    fn a_class_is_found_by_its_declaration() {
        assert!(declares(SOURCE, "Chord"));
        assert!(!declares(SOURCE, "Beams"));
    }

    #[test]
    fn only_the_named_class_s_own_members_count() {
        let body = impl_blocks(SOURCE, "Chord");
        assert!(defines_member(&body, "commonName"));
        // `fill` belongs to Beams, and must not be credited to Chord.
        assert!(!defines_member(&body, "fill"));
    }

    #[test]
    fn a_prefixed_getter_is_the_name_python_sees() {
        let body = impl_blocks(SOURCE, "Chord");
        assert!(defines_member(&body, "pitches"));
    }

    #[test]
    fn a_class_built_at_import_time_is_found_in_the_names_list() {
        assert!(names_list(SOURCE, "MajorScale"));
        assert!(!names_list(SOURCE, "MinorScale"));
    }

    const INHERITING: &str = "\
#[pyclass(name = \"Ornament\", subclass)]
pub struct Ornament {
    name: String,
}

#[pymethods]
impl Ornament {
    fn realize(&self) {}
}

#[pyclass(name = \"Trill\", module = \"music21.expressions\", extends = Ornament)]
pub struct Trill;

#[pymethods]
impl Trill {
    fn own(&self) {}
}

ornament_kinds![
    (Turn, \"Turn\", Ornament, []),
    (InvertedTurn, \"InvertedTurn\", Turn, [Turn]),
    (
        DelayedTurn,
        \"DelayedTurn\",
        Turn,
        [Turn]
    ),
];
";

    fn inheriting() -> Facade {
        let files = BTreeMap::from([("expressions".to_string(), INHERITING.to_string())]);
        Facade { files }
    }

    #[test]
    fn a_subclass_carries_what_it_extends() {
        let body = inheriting().class_body("Trill").unwrap();
        assert!(defines_member(&body, "own"));
        assert!(defines_member(&body, "realize"));
    }

    #[test]
    fn a_parent_does_not_carry_its_subclass_s_members() {
        let body = inheriting().class_body("Ornament").unwrap();
        assert!(!defines_member(&body, "own"));
    }

    #[test]
    fn a_class_built_from_a_macro_table_carries_its_ancestors() {
        let facade = inheriting();
        assert!(defines_member(
            &facade.class_body("Turn").unwrap(),
            "realize"
        ));
        assert!(defines_member(
            &facade.class_body("InvertedTurn").unwrap(),
            "realize"
        ));
    }

    #[test]
    fn a_table_row_rustfmt_split_over_lines_is_read() {
        let body = inheriting().class_body("DelayedTurn").unwrap();
        assert!(defines_member(&body, "realize"));
    }

    #[test]
    fn a_renamed_getter_is_the_name_python_sees() {
        let body = impl_blocks(SOURCE, "Chord");
        assert!(defines_member(&body, "_chordAttached"));
        assert!(!defines_member(&body, "notThere"));
    }
}
