#[derive(Debug, Clone, PartialEq)]
pub(crate) enum OffsetSpecial {
    AtEnd,
    LowestOffset,
    HighestOffset,
}

impl OffsetSpecial {
    pub const fn as_str(&self) -> &str {
        match self {
            OffsetSpecial::AtEnd => "atEnd",
            OffsetSpecial::LowestOffset => "lowestOffset",
            OffsetSpecial::HighestOffset => "highestOffset",
        }
    }
}
