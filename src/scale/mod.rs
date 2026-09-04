use std::{cmp::Ordering, collections::HashMap};

use crate::{defaults::IntegerType, stepname::StepName};

pub(crate) mod concretescale;
/// Diatonic scale construction and harmonization helpers.
pub mod diatonicscale;
pub mod hexatonicblues;
/// The named scales music21 exposes, realized from a tonic.
pub mod scaletype;
pub mod stepscale;

pub use diatonicscale::DiatonicScale;
pub use hexatonicblues::{BluesForm, WeightedHexatonicBlues};
pub use scaletype::{Scale, ScaleType};
pub use stepscale::StepScale;

pub(crate) const FIFTHS_ORDER_SHARP: [StepName; 7] = [
    StepName::F,
    StepName::C,
    StepName::G,
    StepName::D,
    StepName::A,
    StepName::E,
    StepName::B,
];
pub(crate) const FIFTHS_ORDER_FLAT: [StepName; 7] = [
    StepName::B,
    StepName::E,
    StepName::A,
    StepName::D,
    StepName::G,
    StepName::C,
    StepName::F,
];

pub(crate) fn altered_steps_from_sharps(sharps: IntegerType) -> HashMap<StepName, IntegerType> {
    let mut map = HashMap::new();
    match sharps.cmp(&0) {
        Ordering::Greater => {
            for step in FIFTHS_ORDER_SHARP.iter().take(sharps as usize) {
                *map.entry(*step).or_insert(0) += 1;
            }
        }
        Ordering::Less => {
            for step in FIFTHS_ORDER_FLAT.iter().take((-sharps) as usize) {
                *map.entry(*step).or_insert(0) -= 1;
            }
        }
        Ordering::Equal => {}
    }
    map
}
