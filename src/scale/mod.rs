use std::{cmp::Ordering, collections::HashMap};

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

pub(crate) fn accidental_modifier_from_alter(alter: IntegerType) -> String {
    match alter.cmp(&0) {
        Ordering::Greater => "#".repeat(alter as usize),
        Ordering::Less => "-".repeat((-alter) as usize),
        Ordering::Equal => String::new(),
    }
}
