//! The members of music21's `stream.enums`, by music21's names and values.
//!
//! The enums themselves are Python classes; what is here is their members,
//! so that anything building them builds music21's. `StaffType` is the
//! crate's [`StaffType`](crate::layout::StaffType) by its MusicXML names.

use crate::common::enums::Member;
use crate::common::enums::MemberValue::Text;

/// `StaffType`: what a staff is for, each member's value the name
/// MusicXML's `<staff-type>` gives it.
pub const STAFF_TYPE: &[Member] = &[
    ("REGULAR", Text("regular")),
    ("OSSIA", Text("ossia")),
    ("CUE", Text("cue")),
    ("EDITORIAL", Text("editorial")),
    ("ALTERNATE", Text("alternate")),
    ("OTHER", Text("other")),
];

/// `GivenElementsBehavior`: how a stream built with elements places them.
pub const GIVEN_ELEMENTS_BEHAVIOR: &[Member] = &[
    ("APPEND", Text("append")),
    ("OFFSETS", Text("offsets")),
    ("INSERT", Text("insert")),
];

/// `RecursionType`: the order a context search looks through a stream in.
pub const RECURSION_TYPE: &[Member] = &[
    ("ELEMENTS_FIRST", Text("elementsFirst")),
    ("FLATTEN", Text("flatten")),
    ("ELEMENTS_ONLY", Text("elementsOnly")),
];

/// `ShowNumber`: whether a measure's number is shown.
pub const SHOW_NUMBER: &[Member] = &[
    ("DEFAULT", Text("default")),
    ("ALWAYS", Text("always")),
    ("NEVER", Text("never")),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::StaffType;

    /// Each `StaffType` member's value names the crate's staff type of the
    /// same name, and back.
    #[test]
    fn staff_type_members_are_the_crates_staff_types() {
        for (name, value) in STAFF_TYPE {
            let Text(value) = value else {
                panic!("{name} holds no text");
            };
            let found = StaffType::from_name(value).expect("a staff type");
            assert_eq!(found.as_str(), *value);
            assert_eq!(value.to_uppercase(), *name);
        }
    }
}
