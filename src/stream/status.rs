//! What music21's `stream.streamStatus` decides that is not about Python
//! objects: whether a stream's tuplet brackets have been made.

/// Whether tuplet brackets have been made, from the notes with tuplets, in
/// order, each saying whether its first tuplet has a bracket type: `None`
/// where there are no such notes, `Some(true)` once one has a type, and
/// `Some(false)` where none does. music21's `haveTupletBracketsBeenMade`.
///
/// ```
/// use music21::stream::status::tuplet_brackets_made;
///
/// assert_eq!(tuplet_brackets_made([]), None);
/// assert_eq!(tuplet_brackets_made([false, false]), Some(false));
/// assert_eq!(tuplet_brackets_made([false, true]), Some(true));
/// ```
pub fn tuplet_brackets_made(first_typed: impl IntoIterator<Item = bool>) -> Option<bool> {
    let mut found = None;
    for typed in first_typed {
        if typed {
            return Some(true);
        }
        found = Some(false);
    }
    found
}
