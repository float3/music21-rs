#[derive(Eq, Hash, PartialEq, Clone, Debug)]
pub(crate) enum IntervalString {
    Up = 2,
    Down = -2,
}

impl IntervalString {
    pub(crate) fn string(self) -> String {
        let x = match self {
            IntervalString::Up => "d2",
            IntervalString::Down => "-d2",
        };
        x.to_string()
    }
}
