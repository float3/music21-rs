//! music21's `common.pathTools`, as far as it is not about the file system.

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
}
