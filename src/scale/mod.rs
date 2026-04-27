use std::collections::HashMap;

use crate::{defaults::IntegerType, stepname::StepName};

pub(crate) mod concretescale;
pub(crate) mod diatonicscale;

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
    if sharps > 0 {
        for step in FIFTHS_ORDER_SHARP.iter().take(sharps as usize) {
            *map.entry(*step).or_insert(0) += 1;
        }
    } else if sharps < 0 {
        for step in FIFTHS_ORDER_FLAT.iter().take((-sharps) as usize) {
            *map.entry(*step).or_insert(0) -= 1;
        }
    }
    map
}

pub(crate) fn accidental_modifier_from_alter(alter: IntegerType) -> String {
    if alter > 0 {
        "#".repeat(alter as usize)
    } else if alter < 0 {
        "-".repeat((-alter) as usize)
    } else {
        String::new()
    }
}
