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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direction_int() {
        assert_eq!(Direction::Descending as i32, -1);
        assert_eq!(Direction::Oblique as i32, 0);
        assert_eq!(Direction::Ascending as i32, 1);
    }

    #[test]
    fn test_direction_term() {
        assert_eq!(Direction::Descending.term(), "Descending");
        assert_eq!(Direction::Oblique.term(), "Oblique");
        assert_eq!(Direction::Ascending.term(), "Ascending");
    }
}
