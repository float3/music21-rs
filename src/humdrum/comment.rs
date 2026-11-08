//! A `!!` line of a Humdrum file, music21's `GlobalComment`, and a `!`
//! comment in one spine, its `SpineComment`.

/// A comment on the whole table, kept in the score where it was written:
/// music21's `GlobalComment`.
///
/// Its priority is the one music21 gives it, which puts it before the music
/// standing at its offset.
///
/// ```
/// use music21::humdrum::GlobalComment;
///
/// let comment = GlobalComment::new("transcribed by hand", -37);
/// assert_eq!(comment.comment(), "transcribed by hand");
/// assert_eq!(comment.priority(), -37);
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct GlobalComment {
    comment: String,
    priority: i32,
}

impl GlobalComment {
    /// A comment with the priority music21 sorts it by.
    pub fn new(comment: impl Into<String>, priority: i32) -> Self {
        Self {
            comment: comment.into(),
            priority,
        }
    }

    /// What it says: music21's `comment`.
    pub fn comment(&self) -> &str {
        &self.comment
    }

    /// music21's `priority`, which sorts it among what stands at its offset.
    pub fn priority(&self) -> i32 {
        self.priority
    }
}

/// A `!` comment in one spine, kept where it was written: music21's
/// `SpineComment`. Its priority, the line it was written on, is the
/// event's.
///
/// ```
/// use music21::humdrum::SpineComment;
///
/// assert_eq!(SpineComment::new("Cantus").comment(), "Cantus");
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct SpineComment {
    comment: String,
}

impl SpineComment {
    /// A comment saying `comment`.
    pub fn new(comment: impl Into<String>) -> Self {
        Self {
            comment: comment.into(),
        }
    }

    /// What it says: music21's `comment`.
    pub fn comment(&self) -> &str {
        &self.comment
    }
}
