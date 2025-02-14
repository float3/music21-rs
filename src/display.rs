use crate::defaults::FloatType;

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) enum DisplayType {
    Normal,
    Always,
    Never,
    UnlessRepeated,
    EvenTied,
    IfAbsolutelyNecessary,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) enum DisplayStyle {
    Normal,
    Parentheses,
    Bracket,
    Both,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) enum DisplaySize {
    Full,
    Cue,
    Large,
    Percentage(FloatType),
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) enum DisplayLocation {
    Normal,
    Above,
    Ficta,
    Below,
}
