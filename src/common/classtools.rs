//! music21's `common.classTools`, as far as it is not about Python objects.

/// The class name in the text Python writes for a class, as music21's
/// `classToClassStr` cuts it out: whatever follows the last dot, less the
/// closing quote and bracket. A class with no module in its name keeps
/// the rest of the text, as music21's does.
///
/// ```
/// use music21::common::classtools::class_to_class_str;
///
/// assert_eq!(class_to_class_str("<class 'music21.note.Note'>"), "Note");
/// assert_eq!(class_to_class_str("<class 'int'>"), "<class 'int");
/// ```
pub fn class_to_class_str(text: &str) -> String {
    let last = text.rsplit('.').next().unwrap_or(text);
    let kept = last.chars().count().saturating_sub(2);
    last.chars().take(kept).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Python slices characters, not bytes, and an empty slice is empty.
    #[test]
    fn class_names_are_cut_by_characters() {
        assert_eq!(class_to_class_str("<class 'ß.Größe'>"), "Größe");
        assert_eq!(class_to_class_str("'>"), "");
        assert_eq!(class_to_class_str(">"), "");
        assert_eq!(class_to_class_str("a.b.c'>"), "c");
    }
}
