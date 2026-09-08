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
    pub fn class_body(&self, class: &str) -> Option<String> {
        let text = self.files.values().find(|text| declares(text, class))?;
        let body = impl_blocks(text, class);
        Some(body)
    }

    /// Whether the wheel carries a class of this name at all.
    pub fn has_class(&self, class: &str) -> bool {
        self.files.values().any(|text| declares(text, class))
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
/// looked for as written. A getter renamed with `#[getter(name)]` and one
/// declared with `#[pyo3(name = "...")]` both count, since either is what
/// Python sees.
pub fn defines_member(body: &str, name: &str) -> bool {
    [
        format!("fn {name}("),
        format!("fn {name}<"),
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
}

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
    fn a_renamed_getter_is_the_name_python_sees() {
        let body = impl_blocks(SOURCE, "Chord");
        assert!(defines_member(&body, "_chordAttached"));
        assert!(!defines_member(&body, "notThere"));
    }
}
