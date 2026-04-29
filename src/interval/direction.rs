use crate::defaults::IntegerType;

#[derive(PartialEq, Copy, Clone)]
pub(crate) enum Direction {
    Descending = -1,
    Oblique = 0,
    Ascending = 1,
}

impl Direction {
    pub(crate) fn term(&self) -> String {
        match self {
            Direction::Descending => "Descending".to_string(),
            Direction::Oblique => "Oblique".to_string(),
            Direction::Ascending => "Ascending".to_string(),
        }
    }

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

    #[test]
    fn test_direction_term() {
        assert_eq!(Direction::Descending.term(), "Descending");
        assert_eq!(Direction::Oblique.term(), "Oblique");
        assert_eq!(Direction::Ascending.term(), "Ascending");
    }
}
