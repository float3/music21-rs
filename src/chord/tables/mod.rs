mod generated;

use crate::exceptions::Exception;

use generated::*;
use std::{collections::HashMap, sync::LazyLock};

type ChordTableAddress = (u8, u8, i8, Option<u8>);

trait ChordTableAddressExt {
    fn cardinality(&self) -> u8;
    fn forte_class(&self) -> u8;
    fn inversion(&self) -> i8;
    fn pc_original(&self) -> Option<u8>;
}

impl ChordTableAddressExt for ChordTableAddress {
    fn cardinality(&self) -> u8 {
        self.0
    }

    fn forte_class(&self) -> u8 {
        self.1
    }

    fn inversion(&self) -> i8 {
        self.2
    }

    fn pc_original(&self) -> Option<u8> {
        self.3
    }
}

// TNI structures are defined as
// [0] = tuple of pitch classes (0-11)
// [1] = 6-tuple of interval class vector (ICV)
// [2] = 8-tuple of invariance vector (Robert Morris) -- see below
// [3] = index of Z-relation (0=none)
type PitchClasses = [bool; 12];
type IntervalClassVector = [u8; 6];
type InvarianceVector = [u8; 8];
type ZRelation = u8;

type TNIStructure = (
    PitchClasses,
    IntervalClassVector,
    InvarianceVector,
    ZRelation,
);

type Pcivicv = (PitchClasses, InvarianceVector, IntervalClassVector);
trait TNITupleExt {
    fn pitches(&self) -> PitchClasses;
    fn pitch_classes(&self) -> PitchClasses;
    fn interval_class_vector(&self) -> IntervalClassVector;
    fn icv(&self) -> IntervalClassVector;
    fn invariance_vector(&self) -> InvarianceVector;
    fn morris_invariance(&self) -> InvarianceVector;
    fn iv(&self) -> InvarianceVector;
    fn z_relation(&self) -> ZRelation;
}

impl TNITupleExt for TNIStructure {
    fn pitches(&self) -> PitchClasses {
        self.0
    }

    fn pitch_classes(&self) -> PitchClasses {
        self.pitches()
    }

    fn interval_class_vector(&self) -> IntervalClassVector {
        self.1
    }

    fn icv(&self) -> IntervalClassVector {
        self.interval_class_vector()
    }

    fn invariance_vector(&self) -> InvarianceVector {
        self.2
    }

    fn morris_invariance(&self) -> InvarianceVector {
        self.invariance_vector()
    }

    fn iv(&self) -> InvarianceVector {
        self.invariance_vector()
    }

    fn z_relation(&self) -> ZRelation {
        self.3
    }
}

#[repr(i8)]
#[derive(Eq, Hash, PartialEq)]
enum SuperBool {
    NegativeOne = -1,
    Zero = 0,
    One = 1,
}

type U8SB = (u8, SuperBool);
type U8U8SB = (u8, u8, SuperBool);

static CARDINALITY_TO_CHORD_MEMBERS: LazyLock<HashMap<u8, HashMap<U8SB, Pcivicv>>> =
    LazyLock::new(|| {
        use std::collections::HashMap;
        let mut cardinality_to_chord_members = HashMap::new();
        for cardinality in 1..=12 {
            let mut entries = HashMap::new();
            let forte_entries = &generated::FORTE[cardinality as usize];
            for (forte_after_dash, _) in forte_entries.iter().enumerate().skip(1) {
                let Some(tni) = &forte_entries[forte_after_dash] else {
                    continue;
                };
                let has_distinct = tni.interval_class_vector()[1] == 0;
                let inv_num = if has_distinct {
                    SuperBool::One
                } else {
                    SuperBool::Zero
                };
                entries.insert(
                    (forte_after_dash as u8, inv_num),
                    (
                        tni.pitch_classes(),
                        tni.invariance_vector(),
                        tni.interval_class_vector(),
                    ),
                );
                if has_distinct {
                    let inv_pitches = *INVERSION_DEFAULT_PITCH_CLASSES
                        .get(&(cardinality as u8, forte_after_dash as u8))
                        .unwrap();
                    entries.insert(
                        (forte_after_dash as u8, SuperBool::NegativeOne),
                        (
                            inv_pitches,
                            tni.invariance_vector(),
                            tni.interval_class_vector(),
                        ),
                    );
                }
            }
            cardinality_to_chord_members.insert(cardinality as u8, entries);
        }
        cardinality_to_chord_members
    });

fn forte_index_to_inversions_available(card: u8, index: u8) -> Result<Vec<SuperBool>, Exception> {
    if !(1..=12).contains(&card) {
        return Err(Exception::ChordTables(format!(
            "cardinality {} not valid",
            card
        )));
    }
    if index < 1 || index > MAXIMUM_INDEX_NUMBER_WITHOUT_INVERSION_EQUIVALENCE[card as usize] {
        return Err(Exception::ChordTables(format!(
            "index {} not valid for cardinality {}",
            index, card
        )));
    }

    let mut inversions = vec![];
    let forte_entry = &FORTE[card as usize];
    if let Some(entry) = &forte_entry[index as usize] {
        // second value stored inversion status
        if entry.invariance_vector()[1] > 0 {
            inversions.push(SuperBool::Zero);
        } else {
            inversions.push(SuperBool::NegativeOne);
            inversions.push(SuperBool::One);
        }
    }
    Ok(inversions)
}

// include!("./generated.rs");
