/// default for numbers is "0123456789"
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

/// music21's `common.camelCaseToHyphen`: a camel-cased name as words joined
/// by `replacement`, lower case. A capital that starts a run of lower-case
/// letters is set apart first, then any capital following a lower-case
/// letter or a digit, which is the pair of substitutions music21 makes.
pub(crate) fn camel_case_to_hyphen(text: &str, replacement: char) -> String {
    let characters: Vec<char> = text.chars().collect();
    let mut first = String::new();
    let mut index = 0;
    while index < characters.len() {
        let starts_word = index + 2 < characters.len() + 1
            && characters
                .get(index + 1)
                .is_some_and(char::is_ascii_uppercase)
            && characters
                .get(index + 2)
                .is_some_and(char::is_ascii_lowercase);
        if starts_word {
            first.push(characters[index]);
            first.push(replacement);
            first.push(characters[index + 1]);
            index += 2;
            while let Some(letter) = characters
                .get(index)
                .filter(|letter| letter.is_ascii_lowercase())
            {
                first.push(*letter);
                index += 1;
            }
        } else {
            first.push(characters[index]);
            index += 1;
        }
    }
    let characters: Vec<char> = first.chars().collect();
    let mut second = String::new();
    let mut index = 0;
    while index < characters.len() {
        let joins = (characters[index].is_ascii_lowercase() || characters[index].is_ascii_digit())
            && characters
                .get(index + 1)
                .is_some_and(char::is_ascii_uppercase);
        second.push(characters[index]);
        if joins {
            second.push(replacement);
            second.push(characters[index + 1]);
            index += 2;
        } else {
            index += 1;
        }
    }
    second.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// music21's own examples for `camelCaseToHyphen`.
    #[test]
    fn camel_case_is_split_as_music21_splits_it() {
        assert_eq!(camel_case_to_hyphen("movementName", '-'), "movement-name");
        assert_eq!(
            camel_case_to_hyphen("movementNameName", '_'),
            "movement_name_name"
        );
        assert_eq!(camel_case_to_hyphen("fileName", '-'), "file-name");
        assert_eq!(camel_case_to_hyphen("fileNameABC", '-'), "file-name-abc");
        assert_eq!(camel_case_to_hyphen("Movement", '-'), "movement");
        assert_eq!(camel_case_to_hyphen("SnapPizzicato", ' '), "snap pizzicato");
        assert_eq!(camel_case_to_hyphen("OrganHeel", ' '), "organ heel");
        assert_eq!(camel_case_to_hyphen("ABCd", '-'), "ab-cd");
        assert_eq!(camel_case_to_hyphen("AbCdEf", '-'), "ab-cd-ef");
        assert_eq!(camel_case_to_hyphen("x2Y", '-'), "x2-y");
    }

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
