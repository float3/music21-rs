//! music21's `common.decorators`, as far as it is not about Python objects.

/// The warning music21's `deprecated` gives the first time a deprecated
/// function is called: when it was deprecated, when it goes, and what to do
/// instead, each with music21's wording where it is not given.
///
/// ```
/// use music21::common::decorators::deprecation_message;
///
/// assert_eq!(
///     deprecation_message("hi", None, None, None),
///     "hi was deprecated and will disappear soon. Find alternative methods."
/// );
/// assert_eq!(
///     deprecation_message("bye", Some("February 1972"), Some("September 2099"), Some("You should be okay.")),
///     "bye was deprecated on February 1972 and will disappear at or after September 2099. You should be okay."
/// );
/// ```
pub fn deprecation_message(
    name: &str,
    start_date: Option<&str>,
    remove_date: Option<&str>,
    message: Option<&str>,
) -> String {
    let start = start_date.map_or_else(String::new, |date| format!(" on {date}"));
    let remove =
        remove_date.map_or_else(|| "soon".to_string(), |date| format!("at or after {date}"));
    let message = message.unwrap_or("Find alternative methods.");
    format!("{name} was deprecated{start} and will disappear {remove}. {message}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each part is filled in on its own.
    #[test]
    fn missing_parts_take_music21s_wording() {
        assert_eq!(
            deprecation_message("A.b", Some("v11"), None, Some("")),
            "A.b was deprecated on v11 and will disappear soon. "
        );
        assert_eq!(
            deprecation_message("f", None, Some("v12"), None),
            "f was deprecated and will disappear at or after v12. Find alternative methods."
        );
    }
}
