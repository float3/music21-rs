//! Marks that help braille transcription and are not themselves written:
//! music21's `braille.objects`.

/// Where a braille segment divides: music21's `BrailleSegmentDivision` and
/// `BrailleOptionalSegmentDivision`. Standing in a measure, it ends the
/// segment there, in the middle of the measure if need be.
///
/// ```
/// use music21_rs::braille::SegmentDivision;
///
/// assert_eq!(SegmentDivision::Required.class_name(), "BrailleSegmentDivision");
/// assert_eq!(
///     SegmentDivision::Optional.class_name(),
///     "BrailleOptionalSegmentDivision"
/// );
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SegmentDivision {
    /// The segment divides here, unless it holds nothing yet.
    Required,
    /// The segment divides here only once it holds more than 48 elements.
    Optional,
}

impl SegmentDivision {
    /// music21's class name for it.
    pub fn class_name(self) -> &'static str {
        match self {
            Self::Required => "BrailleSegmentDivision",
            Self::Optional => "BrailleOptionalSegmentDivision",
        }
    }
}
