//! The direction of an interval, shared by the generic, diatonic and
//! chromatic halves under the name music21 gives it.

pub use super::IntervalDirection as Direction;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defaults::IntegerType;

    #[test]
    fn test_direction_int() {
        assert_eq!(Direction::Descending as IntegerType, -1);
        assert_eq!(Direction::Oblique as IntegerType, 0);
        assert_eq!(Direction::Ascending as IntegerType, 1);
    }
}
