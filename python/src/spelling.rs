//! The wheel's side of the crate's flat spelling: the crate writes a flat
//! `b` (`Bb4`), music21 writes it `-` (`B-4`), and everything the wheel
//! hands Python is spelled music21's way.
//!
//! Reading needs no help: the crate reads `-` as well as `b`.

use music21_rs_crate::chordsymbol::ChordSymbolFigure;
use music21_rs_crate::key::KeySignature;

/// A modifier as music21 writes it: `bb` is `--`, `b`` `-``.
pub(crate) fn music21_modifier(modifier: &str) -> String {
    modifier.replace('b', "-")
}

/// A pitch name as music21 writes it: `Bb4` is `B-4`, `bb` (B-flat minor)
/// `b-`. The first letter is the step; every `b` after it is a flat.
pub(crate) fn music21_name(name: &str) -> String {
    let mut letters = name.chars();
    let Some(step) = letters.next() else {
        return String::new();
    };
    format!("{step}{}", music21_modifier(letters.as_str()))
}

/// A chord-symbol figure with its pitch names spelled music21's way, so
/// that it writes `B-7/A-` rather than `Bb7/Ab`.
pub(crate) fn music21_figure(figure: ChordSymbolFigure) -> ChordSymbolFigure {
    let names = |names: Vec<String>| names.iter().map(|name| music21_name(name)).collect();
    ChordSymbolFigure {
        root: music21_name(&figure.root),
        bass: figure.bass.as_deref().map(music21_name),
        additions: names(figure.additions),
        omissions: names(figure.omissions),
        ..figure
    }
}

/// A numeral's `figureAndKey` with its key spelled music21's way:
/// `bVI in bb minor` is `bVI in b- minor`. The figure's own `b`s are left.
pub(crate) fn music21_figure_and_key(text: &str) -> String {
    const IN: &str = " in ";

    let Some((figure, key)) = text.rsplit_once(IN) else {
        return text.to_string();
    };
    let (tonic, mode) = key.split_once(' ').unwrap_or((key, ""));
    format!("{figure}{IN}{} {mode}", music21_name(tonic))
        .trim_end()
        .to_string()
}

/// A key signature's description with its pitches spelled music21's way:
/// `pitches: [E-, G#4]`, where the crate says `[Eb, G#4]`.
pub(crate) fn music21_description(signature: &KeySignature) -> String {
    if signature.sharps().is_some() {
        return signature.description();
    }
    let names: Vec<String> = signature
        .altered_pitches()
        .unwrap_or_default()
        .iter()
        .map(|pitch| music21_name(&pitch.to_string()))
        .collect();
    format!("pitches: [{}]", names.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_take_music21_flats() {
        assert_eq!(music21_name("Bb4"), "B-4");
        assert_eq!(music21_name("bb"), "b-");
        assert_eq!(music21_name("Ebb"), "E--");
        assert_eq!(music21_name("C#4"), "C#4");
        assert_eq!(music21_modifier("b`"), "-`");
    }

    #[test]
    fn a_numeral_keeps_its_figure_and_respells_its_key() {
        assert_eq!(
            music21_figure_and_key("bbVI in bb minor"),
            "bbVI in b- minor"
        );
        assert_eq!(music21_figure_and_key("I6 in Bb major"), "I6 in B- major");
        assert_eq!(music21_figure_and_key("V"), "V");
    }
}
