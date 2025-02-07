pub(crate) fn get_num_from_str(usr_str: &str, numbers: &str) -> (String, String) {
    let mut found = String::new();
    let mut remain = String::new();
    for ch in usr_str.chars() {
        if numbers.contains(ch) {
            found.push(ch);
        } else {
            remain.push(ch);
        }
    }
    (found, remain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        let (nums, rest) = get_num_from_str("23a", "0123456789");
        assert_eq!(nums, "23");
        assert_eq!(rest, "a");
    }

    #[test]
    fn test_multiple_numbers() {
        let (nums, rest) = get_num_from_str("23a954Hello", "0123456789");
        assert_eq!(nums, "23954");
        assert_eq!(rest, "aHello");
    }

    #[test]
    fn test_empty_string() {
        let (nums, rest) = get_num_from_str("", "0123456789");
        assert_eq!(nums, "");
        assert_eq!(rest, "");
    }

    #[test]
    fn test_no_numbers() {
        let (nums, rest) = get_num_from_str("Hello", "0123456789");
        assert_eq!(nums, "");
        assert_eq!(rest, "Hello");
    }
}
