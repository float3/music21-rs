//! music21's `common.pathTools`, as far as it needs no Python.

use std::path::Path;

/// The index of the last `..` among a path's parts, if there is one.
/// music21's `cleanpath` collapses a `..` only where the folders up to and
/// including the last one exist, so this is how far it has to look.
///
/// ```
/// use music21::common::pathtools::last_parent_dir;
///
/// assert_eq!(last_parent_dir(&["/", "a", "..", "b", "..", "c"]), Some(4));
/// assert_eq!(last_parent_dir(&["a", "b"]), None);
/// ```
pub fn last_parent_dir<S: AsRef<str>>(parts: &[S]) -> Option<usize> {
    parts.iter().rposition(|part| part.as_ref() == "..")
}

/// A path as music21's `common.cleanpath` leaves it: `~` expanded, made
/// absolute against the working directory and normalized, and environment
/// variables expanded.
///
/// A `..` is collapsed only where the directories before it exist, as the
/// system would; otherwise the path is joined to the working directory as
/// written.
///
/// ```
/// use music21::common::pathtools::clean_path;
///
/// assert_eq!(clean_path("/usr//bin/./lilypond"), "/usr/bin/lilypond");
/// assert!(clean_path("relative/file.xml").ends_with("/relative/file.xml"));
/// ```
pub fn clean_path(path: &str) -> String {
    let expanded = expand_user(path);
    let parts: Vec<&str> = expanded.split('/').collect();
    let normalizable = match last_parent_dir(&parts) {
        Some(last) => {
            let through = parts[..=last].join("/");
            let through = if through.is_empty() {
                "/".to_owned()
            } else {
                through
            };
            Path::new(&through).is_dir()
        }
        None => true,
    };
    let cleaned = if normalizable {
        absolute(&normalized(&expanded))
    } else {
        join(&working_directory(), &expanded)
    };
    expand_variables(&cleaned)
}

/// `~` and `~user` at the start expanded, as `os.path.expanduser` does on
/// a POSIX system: `~` from `HOME`.
pub(crate) fn expand_user(path: &str) -> String {
    if !path.starts_with('~') {
        return path.to_owned();
    }
    let end = path.find('/').unwrap_or(path.len());
    let home = if end == 1 {
        std::env::var("HOME").ok()
    } else {
        // Another user's home: where it would be beside this user's.
        std::env::var("HOME").ok().and_then(|home| {
            Path::new(&home)
                .parent()
                .map(|parent| parent.join(&path[1..end]).to_string_lossy().into_owned())
        })
    };
    let Some(home) = home else {
        return path.to_owned();
    };
    let home = home.trim_end_matches('/');
    let home = if home.is_empty() { "/" } else { home };
    let rest = &path[end..];
    if home == "/" && rest.starts_with('/') {
        return rest.to_owned();
    }
    format!("{home}{rest}")
}

/// A path with `.`, doubled slashes and `name/..` taken out, as
/// `os.path.normpath` takes them out on a POSIX system.
fn normalized(path: &str) -> String {
    if path.is_empty() {
        return ".".to_owned();
    }
    let leading = if path.starts_with("//") && !path.starts_with("///") {
        "//"
    } else if path.starts_with('/') {
        "/"
    } else {
        ""
    };
    let mut kept: Vec<&str> = Vec::new();
    for part in path.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        let climbs = part == "..";
        if climbs && kept.last().is_some_and(|last| *last != "..") {
            kept.pop();
            continue;
        }
        if climbs && !leading.is_empty() {
            continue;
        }
        kept.push(part);
    }
    let joined = format!("{leading}{}", kept.join("/"));
    if joined.is_empty() {
        return ".".to_owned();
    }
    joined
}

/// The working directory, as text.
fn working_directory() -> String {
    std::env::current_dir()
        .map(|directory| directory.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "/".to_owned())
}

/// `os.path.join` of two paths: the second alone where it is absolute.
fn join(first: &str, second: &str) -> String {
    if second.starts_with('/') || first.is_empty() {
        return second.to_owned();
    }
    if first.ends_with('/') {
        return format!("{first}{second}");
    }
    format!("{first}/{second}")
}

/// `os.path.abspath` of a path already normalized.
fn absolute(path: &str) -> String {
    if path.starts_with('/') {
        return path.to_owned();
    }
    normalized(&join(&working_directory(), path))
}

/// `$name` and `${name}` replaced by the variable's value, as
/// `os.path.expandvars` does on a POSIX system; one that is not set is
/// left as written.
fn expand_variables(path: &str) -> String {
    if !path.contains('$') {
        return path.to_owned();
    }
    let is_name = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut expanded = String::new();
    let mut rest = path;
    while let Some(dollar) = rest.find('$') {
        expanded.push_str(&rest[..dollar]);
        let after = &rest[dollar + 1..];
        let (name, written_length) = if let Some(braced) = after.strip_prefix('{') {
            match braced.find('}') {
                Some(close) => (&braced[..close], close + 2),
                None => ("", 0),
            }
        } else {
            let length = after.find(|c: char| !is_name(c)).unwrap_or(after.len());
            (&after[..length], length)
        };
        let value = if name.is_empty() {
            None
        } else {
            std::env::var(name).ok()
        };
        match value {
            Some(value) => expanded.push_str(&value),
            None => expanded.push_str(&rest[dollar..=dollar + written_length]),
        }
        rest = &after[written_length..];
    }
    expanded.push_str(rest);
    expanded
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only a whole part counts, not one that merely holds two dots.
    #[test]
    fn only_whole_parent_parts_count() {
        assert_eq!(last_parent_dir(&["...", "a..", "..b"]), None);
        assert_eq!(last_parent_dir(&[".."]), Some(0));
        assert_eq!(last_parent_dir::<&str>(&[]), None);
    }

    #[test]
    fn dot_dot_after_a_missing_directory_is_kept() {
        let cleaned = clean_path("tinyNotation: /../../x.xml");
        assert!(
            cleaned.ends_with("/tinyNotation: /../../x.xml"),
            "{cleaned}"
        );
    }

    #[test]
    fn normalizing_matches_posix_normpath() {
        assert_eq!(normalized("//a/b"), "//a/b");
        assert_eq!(normalized("///a/../b"), "/b");
        assert_eq!(normalized("../a/./b/"), "../a/b");
        assert_eq!(normalized("/.."), "/");
        assert_eq!(normalized("a/.."), ".");
    }

    #[test]
    fn unset_variables_are_left_as_written() {
        assert_eq!(
            expand_variables("/x/$MUSIC21_RS_SURELY_UNSET/${ALSO_UNSET_HERE}"),
            "/x/$MUSIC21_RS_SURELY_UNSET/${ALSO_UNSET_HERE}"
        );
    }
}
