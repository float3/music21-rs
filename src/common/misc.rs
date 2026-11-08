//! music21's `common.misc`: what fits nowhere else in `common`.

/// The message music21 gives when optional packages are missing, naming
/// them and where to read how to install them: music21's
/// `getMissingImportStr`. Nothing is missing, nothing is said.
///
/// The lines after the first keep the indent music21's source gives them.
///
/// ```
/// use music21::common::misc::missing_import_message;
///
/// let message = missing_import_message(&["matplotlib"]).unwrap();
/// assert!(message.starts_with("Certain music21 functions might need the optional package matplotlib;"));
/// assert_eq!(missing_import_message(&[]), None);
/// ```
pub fn missing_import_message(names: &[&str]) -> Option<String> {
    const WHERE: &str = "https://www.music21.org/music21docs/installing/installAdditional.html";
    match names {
        [] => None,
        [name] => Some(format!(
            "Certain music21 functions might need the optional package {name};\n{}if you run \
             into errors, install it by following the instructions at\n{}{WHERE}",
            " ".repeat(18),
            " ".repeat(18)
        )),
        _ => Some(format!(
            "Certain music21 functions might need these optional packages: {};\n{}if you run \
             into errors, install them by following the instructions at\n{}{WHERE}",
            names.join(", "),
            " ".repeat(19),
            " ".repeat(19)
        )),
    }
}

/// The platform music21 says it runs on: `win`, `darwin` or `nix`, which
/// is every other Unix: music21's `getPlatform`.
///
/// ```
/// use music21::common::misc::platform_name;
///
/// assert!(["win", "darwin", "nix", "other"].contains(&platform_name()));
/// ```
pub fn platform_name() -> &'static str {
    if cfg!(windows) {
        return "win";
    }
    if cfg!(target_os = "macos") {
        return "darwin";
    }
    if cfg!(unix) {
        return "nix";
    }
    "other"
}

/// A note name with each flat written `b` written `-` instead, as music21
/// spells it: music21's `cleanedFlatNotation`. Only a `b` after a note
/// letter is a flat.
///
/// ```
/// use music21::common::misc::cleaned_flat_notation;
///
/// assert_eq!(cleaned_flat_notation("Cb"), "C-");
/// assert_eq!(cleaned_flat_notation("Ebb E bb"), "E-b E b-");
/// ```
pub fn cleaned_flat_notation(text: &str) -> String {
    let characters: Vec<char> = text.chars().collect();
    let mut cleaned = String::with_capacity(text.len());
    let mut index = 0;
    while index < characters.len() {
        let letter = characters[index];
        cleaned.push(letter);
        let flat =
            matches!(letter, 'A'..='G' | 'a'..='g') && characters.get(index + 1) == Some(&'b');
        if flat {
            cleaned.push('-');
            index += 2;
        } else {
            index += 1;
        }
    }
    cleaned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn several_missing_packages_are_listed_together() {
        let message = missing_import_message(&["matplotlib", "numpy"]).unwrap();
        let lines: Vec<&str> = message.lines().collect();
        assert_eq!(
            lines[0],
            "Certain music21 functions might need these optional packages: matplotlib, numpy;"
        );
        assert_eq!(
            lines[1],
            "                   if you run into errors, install them by following the \
             instructions at"
        );
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn a_flat_is_only_a_b_after_a_note_letter() {
        assert_eq!(cleaned_flat_notation("Bbb"), "B-b");
        assert_eq!(cleaned_flat_notation("Hb abc"), "Hb a-c");
    }
}
