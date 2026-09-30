use crate::defaults::FloatType;

/// Something about how a mark is drawn: its colour, which side of the staff
/// it sits on, whether it is bracketed.
///
/// How a mark is drawn says nothing about what it means, so two marks that
/// differ only in this are equal: a red `4/4` is still `4/4`.
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub(crate) struct Drawn<T>(pub(crate) T);

impl<T> PartialEq for Drawn<T> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl<T> Eq for Drawn<T> {}

impl<T> std::hash::Hash for Drawn<T> {
    /// Hashes nothing, so marks equal without it hash alike.
    fn hash<H: std::hash::Hasher>(&self, _: &mut H) {}
}

/// The colour a mark is drawn in.
pub(crate) type Color = Drawn<Option<String>>;

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
