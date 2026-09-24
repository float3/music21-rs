//! Where the score editor draws its harmony labels: under the bass each
//! harmony stands on, and only where the label changes.

use super::{EPSILON, SliceInfo};
use serde::Serialize;
use std::collections::BTreeMap;

/// A bass moving for less than this many quarters under notes that stay is
/// passing on to the next harmony, not a harmony of its own.
const PASSING_BASS: f64 = 1.0;
/// Labels closer together than this many quarters would be drawn over each
/// other, which onsets a fraction apart otherwise do.
const LABEL_SPACING: f64 = 0.5;
/// How many notes coming in together make a chord a waiting bass takes.
const CHORD_ENTRY: usize = 2;

/// What a label says about a slice.
#[derive(Clone, Copy)]
enum Kind {
    /// The Roman numeral exactly as music21 writes it.
    Music21,
    /// The Roman numeral as a harmony textbook writes it.
    Textbook,
    Symbol,
    /// The pitched common name, of a chord of three pitch classes or more.
    Name,
}

impl Kind {
    fn of(self, slice: &SliceInfo) -> Option<&str> {
        let label = match self {
            Self::Music21 => slice.numeral.as_deref(),
            Self::Textbook => slice.textbook_numeral.as_deref(),
            Self::Symbol => slice.chord_symbol.as_deref(),
            Self::Name if slice.pitch_classes.len() >= 3 => {
                Some(slice.pitched_common_name.as_str())
            }
            Self::Name => None,
        };
        label.filter(|text| !text.is_empty())
    }
}

/// Each kind's labels as `(text offset, label)`.
#[derive(Serialize)]
pub(super) struct Labels {
    music21: Vec<(usize, String)>,
    textbook: Vec<(usize, String)>,
    symbol: Vec<(usize, String)>,
    name: Vec<(usize, String)>,
}

pub(super) fn placed(slices: &[SliceInfo]) -> Labels {
    Labels {
        music21: place(slices, Kind::Music21),
        textbook: place(slices, Kind::Textbook),
        symbol: place(slices, Kind::Symbol),
        name: place(slices, Kind::Name),
    }
}

/// The bass a label goes under: where it is written, when it starts, and
/// whether it still waits for a harmony over it.
struct Bass {
    anchor: usize,
    offset: f64,
    free: bool,
}

fn upper(slice: &SliceInfo) -> &[String] {
    slice.pitches.get(1..).unwrap_or_default()
}

/// A harmony is labelled under the bass it stands on, and only when the
/// label changes. A bass struck alone and then held under the chord, as a
/// guitar plays it, takes the label of the chord that comes in over it.
fn place(slices: &[SliceInfo], kind: Kind) -> Vec<(usize, String)> {
    let mut labels = BTreeMap::new();
    let mut previous: Option<&str> = None;
    let mut labelled = f64::NEG_INFINITY;
    let mut bass: Option<Bass> = None;
    let mut before: Option<&SliceInfo> = None;
    for slice in slices {
        let label = kind.of(slice);
        let passing = slice.bass_attack.is_some()
            && previous.is_some()
            && slice.duration < PASSING_BASS
            && before.is_some_and(|before| upper(before) == upper(slice));
        if passing {
            bass = None;
            continue;
        }

        // Only a bass sounding with no harmony over it yet waits for one,
        // and what it waits for is a chord, not a melody note.
        let entering = slice
            .pitches
            .iter()
            .filter(|pitch| !before.is_some_and(|before| before.pitches.contains(pitch)))
            .count();
        before = Some(slice);
        if let Some([anchor, _]) = slice.bass_attack {
            bass = Some(Bass {
                anchor,
                offset: slice.offset,
                free: label.is_none(),
            });
        }
        let waiting = slice.bass_attack.is_none()
            && bass.as_ref().is_some_and(|bass| bass.free)
            && label.is_some()
            && entering >= CHORD_ENTRY;
        if slice.bass_attack.is_none() && !waiting {
            continue;
        }

        if let (Some(under), Some(text)) = (bass.as_mut(), label) {
            let spaced = under.offset - labelled >= LABEL_SPACING - EPSILON;
            if previous != Some(text) && spaced {
                labels.insert(under.anchor, text.to_string());
                labelled = under.offset;
            }
            under.free = false;
        }
        previous = label;
    }
    labels.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slice(
        offset: f64,
        pitches: &[&str],
        symbol: Option<&str>,
        anchor: Option<usize>,
    ) -> SliceInfo {
        SliceInfo {
            offset,
            duration: 1.0,
            measure: 1,
            beat: 1.0 + offset,
            pitches: pitches.iter().map(|pitch| pitch.to_string()).collect(),
            pitch_classes: Vec::new(),
            common_name: String::new(),
            pitched_common_name: String::new(),
            chord_symbol: symbol.map(str::to_string),
            numeral: None,
            textbook_numeral: None,
            inversion: None,
            root: None,
            bass: None,
            consonant: true,
            changed: true,
            attacks: Vec::new(),
            sounding: Vec::new(),
            bass_attack: anchor.map(|at| [at, at + 1]),
        }
    }

    #[test]
    fn a_label_goes_under_the_bass_and_only_where_it_changes() {
        let slices = [
            slice(0.0, &["C3", "E4", "G4"], Some("C"), Some(10)),
            slice(1.0, &["C3", "E4", "G4"], Some("C"), Some(20)),
            slice(2.0, &["G2", "D4", "B4"], Some("G"), Some(30)),
        ];
        let labels = place(&slices, Kind::Symbol);
        assert_eq!(labels, vec![(10, "C".to_string()), (30, "G".to_string())]);
    }

    #[test]
    fn a_bass_struck_alone_takes_the_chord_that_comes_in_over_it() {
        let slices = [
            slice(0.0, &["C3"], None, Some(10)),
            slice(1.0, &["C3", "E4", "G4"], Some("C"), None),
        ];
        assert_eq!(place(&slices, Kind::Symbol), vec![(10, "C".to_string())]);
    }
}
