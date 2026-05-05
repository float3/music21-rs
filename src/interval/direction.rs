use crate::defaults::IntegerType;

#[derive(PartialEq, Copy, Clone)]
pub(crate) enum Direction {
    Descending = -1,
    Oblique = 0,
    Ascending = 1,
}

impl Direction {
    pub(crate) fn as_int(&self) -> IntegerType {
        *self as IntegerType
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direction_int() {
        assert_eq!(Direction::Descending as IntegerType, -1);
        assert_eq!(Direction::Oblique as IntegerType, 0);
        assert_eq!(Direction::Ascending as IntegerType, 1);
    }
}
