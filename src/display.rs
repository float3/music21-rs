use crate::defaults::FloatType;

#[derive(Clone, Debug)]
pub(crate) enum DisplayType {
    Normal,
    Always,
    Never,
    UnlessRepeated,
    EvenTied,
    IfAbsolutelyNecessary,
}

#[derive(Clone, Debug)]
pub(crate) enum DisplayStyle {
    Normal,
    Parentheses,
    Bracket,
    Both,
}

#[derive(Clone, Debug)]
pub(crate) enum DisplaySize {
    Full,
    Cue,
    Large,
    Percentage(FloatType),
}

#[derive(Clone, Debug)]
pub(crate) enum DisplayLocation {
    Normal,
    Above,
    Ficta,
    Below,
}
