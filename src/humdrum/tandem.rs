//! A `*` instruction of a Humdrum spine the reader has no object for:
//! music21's `MiscTandem`.

/// An instruction kept in the score where it was written: music21's
/// `MiscTandem`.
///
/// Its priority is the line it was written on, as music21 gives it.
///
/// ```
/// use music21_rs::humdrum::MiscTandem;
///
/// let tandem = MiscTandem::new("*staff1", 2);
/// assert_eq!(tandem.tandem(), "*staff1");
/// assert_eq!(tandem.priority(), 2);
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct MiscTandem {
    tandem: String,
    priority: i32,
}

impl MiscTandem {
    /// An instruction with the priority music21 sorts it by.
    pub fn new(tandem: impl Into<String>, priority: i32) -> Self {
        Self {
            tandem: tandem.into(),
            priority,
        }
    }

    /// The instruction as written: music21's `tandem`.
    pub fn tandem(&self) -> &str {
        &self.tandem
    }

    /// music21's `priority`, which sorts it among what stands at its offset.
    pub fn priority(&self) -> i32 {
        self.priority
    }
}
