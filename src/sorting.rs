//! music21's `sorting`: the rules of its `SortTuple`, the six numbers that
//! decide which of two things comes first in a stream.
//!
//! The tuple itself is a Python class; what is here is what it decides
//! that is not plain tuple order: how one compares against a bare offset,
//! how two combine, and how one is written short.

/// The tuple's fields, in music21's order, which is the order they are
/// compared in.
pub const SORT_TUPLE_FIELDS: [&str; 6] = [
    "atEnd",
    "offset",
    "priority",
    "classSortOrder",
    "isNotGrace",
    "insertIndex",
];

/// A comparison of a sort tuple against a bare offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comparison {
    /// `<`
    Less,
    /// `<=`
    LessEqual,
    /// `==`
    Equal,
    /// `!=`
    NotEqual,
    /// `>`
    Greater,
    /// `>=`
    GreaterEqual,
}

/// What a tuple stored at the end of its stream answers when compared
/// against a bare offset: it stands after every offset but infinity, and
/// equals infinity alone.
///
/// ```
/// use music21::sorting::{Comparison, at_end_compared};
///
/// assert!(at_end_compared(Comparison::Greater, false));
/// assert!(at_end_compared(Comparison::Equal, true));
/// assert!(!at_end_compared(Comparison::Less, true));
/// ```
pub fn at_end_compared(comparison: Comparison, other_is_infinity: bool) -> bool {
    let equal = other_is_infinity;
    let greater = !other_is_infinity;
    match comparison {
        Comparison::Less => false,
        Comparison::LessEqual | Comparison::Equal => equal,
        Comparison::NotEqual => !equal,
        Comparison::Greater => greater,
        Comparison::GreaterEqual => greater || equal,
    }
}

/// Whether a field combines with another tuple's by the larger of the two
/// in `add` and the smaller in `sub`, as `atEnd` and `isNotGrace` do,
/// rather than by adding or subtracting.
///
/// ```
/// use music21::sorting::is_flag_field;
///
/// assert!(is_flag_field("atEnd"));
/// assert!(!is_flag_field("offset"));
/// ```
pub fn is_flag_field(field: &str) -> bool {
    matches!(field, "atEnd" | "isNotGrace")
}

/// A tuple written short, each part already written as Python writes it:
/// `1.0 <0.20.323>`, or `End <4.7.[Grace].200>` for a grace note stored at
/// the end, whose offset is left out.
///
/// ```
/// use music21::sorting::short_repr;
///
/// assert_eq!(short_repr(false, "1.0", "0", "20", true, "323"), "1.0 <0.20.323>");
/// assert_eq!(short_repr(true, "1.0", "4", "7", false, "200"), "End <4.7.[Grace].200>");
/// ```
pub fn short_repr(
    at_end: bool,
    offset: &str,
    priority: &str,
    class_sort_order: &str,
    not_grace: bool,
    insert_index: &str,
) -> String {
    let start = if at_end { "End" } else { offset };
    let grace = if not_grace { "" } else { ".[Grace]" };
    format!("{start} <{priority}.{class_sort_order}{grace}.{insert_index}>")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `<=` and `>=` at the end are `<` or `==` and `>` or `==`.
    #[test]
    fn at_end_or_equal_comparisons() {
        assert!(at_end_compared(Comparison::LessEqual, true));
        assert!(!at_end_compared(Comparison::LessEqual, false));
        assert!(at_end_compared(Comparison::GreaterEqual, true));
        assert!(at_end_compared(Comparison::GreaterEqual, false));
        assert!(at_end_compared(Comparison::NotEqual, false));
        assert!(!at_end_compared(Comparison::NotEqual, true));
    }

    /// A grace note not at the end keeps its offset.
    #[test]
    fn grace_short_repr_keeps_offset() {
        assert_eq!(
            short_repr(false, "2.5", "-inf", "0", false, "0"),
            "2.5 <-inf.0.[Grace].0>"
        );
    }
}
