//! The crate writes a flat `b` (`Bb4`); music21 writes it `-` (`B-4`).
//! The parity tests compare against music21, so they respell the crate's
//! names through here rather than each keeping a copy.

/// A name the crate writes with `b` for flat, as music21 writes it with
/// `-`: `Bb4` is `B-4`, `bb` (B-flat minor) `b-`. The first letter is the
/// step; every `b` after it is a flat.
pub fn music21_name(name: &str) -> String {
    let mut letters = name.chars();
    let Some(step) = letters.next() else {
        return String::new();
    };
    format!("{step}{}", letters.as_str().replace('b', "-"))
}

#[cfg(test)]
mod tests {
    use super::music21_name;

    #[test]
    fn flats_after_the_step() {
        assert_eq!(music21_name("Bb4"), "B-4");
        assert_eq!(music21_name("bb"), "b-");
        assert_eq!(music21_name("Ebb-1"), "E---1");
        assert_eq!(music21_name("F#"), "F#");
        assert_eq!(music21_name(""), "");
    }
}
