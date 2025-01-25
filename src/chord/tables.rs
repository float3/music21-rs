use crate::exceptions::Exception;
use std::{collections::HashMap, sync::LazyLock};
#[derive(Debug, Clone)]
struct ChordTableAddress {
    cardinality: u8,
    forte_class: u8,
    inversion: i8,
    pc_original: Option<u8>,
}

impl ChordTableAddress {
    pub fn new(cardinality: u8, forte_class: u8, inversion: i8, pc_original: Option<u8>) -> Self {
        Self {
            cardinality,
            forte_class,
            inversion,
            pc_original,
        }
    }
}
// TNI structures are defined as
// [0] = tuple of pitch classes (0-11)
// [1] = 6-tuple of interval class vector (ICV)
// [2] = 8-tuple of invariance vector (Robert Morris) -- see below
// [3] = index of Z-relation (0=none)
type PitchClasses = Vec<u8>;
type IntervalClassVector = [u8; 6];
type InvarianceVector = [u8; 8];
type ZRelation = u8;
#[derive(Clone)]
struct TNIStructure {
    pub pitch_classes: PitchClasses,
    pub interval_class_vector: IntervalClassVector,
    pub invariance_vector: InvarianceVector,
    pub z_relation: ZRelation,
}
type TNITuple = (
    PitchClasses,
    IntervalClassVector,
    InvarianceVector,
    ZRelation,
);
type Pcivicv = (PitchClasses, InvarianceVector, IntervalClassVector);

#[repr(i8)]
#[derive(Eq, Hash, PartialEq)]
enum SuperBool {
    NegativeOne = -1,
    Zero = 0,
    One = 1,
}

type U8SB = (u8, SuperBool);
type U8U8SB = (u8, u8, SuperBool);

// BEGIN_GENERATED_CODE
static FORTE: LazyLock<Vec<Vec<Option<TNIStructure>>>> = LazyLock::new(|| {
    vec![
        vec![],
        vec![
            None,
            Some(TNIStructure {
                pitch_classes: vec![0],
                interval_class_vector: [0, 0, 0, 0, 0, 0],
                invariance_vector: [1, 1, 1, 1, 11, 11, 11, 11],
                z_relation: 0,
            }),
        ],
        vec![
            None,
            Some(TNIStructure {
                pitch_classes: vec![0, 1],
                interval_class_vector: [1, 0, 0, 0, 0, 0],
                invariance_vector: [1, 1, 0, 0, 9, 9, 8, 8],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2],
                interval_class_vector: [0, 1, 0, 0, 0, 0],
                invariance_vector: [1, 1, 1, 1, 9, 9, 9, 9],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 3],
                interval_class_vector: [0, 0, 1, 0, 0, 0],
                invariance_vector: [1, 1, 1, 1, 9, 9, 9, 9],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 4],
                interval_class_vector: [0, 0, 0, 1, 0, 0],
                invariance_vector: [1, 1, 1, 1, 9, 9, 9, 9],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 5],
                interval_class_vector: [0, 0, 0, 0, 1, 0],
                invariance_vector: [1, 1, 0, 0, 9, 9, 8, 8],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 6],
                interval_class_vector: [0, 0, 0, 0, 0, 1],
                invariance_vector: [2, 2, 2, 2, 10, 10, 10, 10],
                z_relation: 0,
            }),
        ],
        vec![
            None,
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2],
                interval_class_vector: [2, 1, 0, 0, 0, 0],
                invariance_vector: [1, 1, 0, 0, 7, 7, 4, 4],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3],
                interval_class_vector: [1, 1, 1, 0, 0, 0],
                invariance_vector: [1, 0, 0, 0, 5, 6, 5, 5],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 4],
                interval_class_vector: [1, 0, 1, 1, 0, 0],
                invariance_vector: [1, 0, 0, 0, 5, 6, 5, 5],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 5],
                interval_class_vector: [1, 0, 0, 1, 1, 0],
                invariance_vector: [1, 0, 1, 0, 5, 6, 5, 6],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 6],
                interval_class_vector: [1, 0, 0, 0, 1, 1],
                invariance_vector: [1, 0, 0, 1, 6, 7, 7, 6],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 4],
                interval_class_vector: [0, 2, 0, 1, 0, 0],
                invariance_vector: [1, 1, 1, 1, 7, 7, 7, 7],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 5],
                interval_class_vector: [0, 1, 1, 0, 1, 0],
                invariance_vector: [1, 0, 0, 0, 5, 6, 5, 5],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 6],
                interval_class_vector: [0, 1, 0, 1, 0, 1],
                invariance_vector: [1, 0, 0, 1, 6, 7, 7, 6],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 7],
                interval_class_vector: [0, 1, 0, 0, 2, 0],
                invariance_vector: [1, 1, 0, 0, 7, 7, 4, 4],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 3, 6],
                interval_class_vector: [0, 0, 2, 0, 0, 1],
                invariance_vector: [1, 1, 1, 1, 8, 8, 8, 8],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 3, 7],
                interval_class_vector: [0, 0, 1, 1, 1, 0],
                invariance_vector: [1, 0, 0, 0, 5, 6, 5, 5],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 4, 8],
                interval_class_vector: [0, 0, 0, 3, 0, 0],
                invariance_vector: [3, 3, 3, 3, 9, 9, 9, 9],
                z_relation: 0,
            }),
        ],
        vec![
            None,
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3],
                interval_class_vector: [3, 2, 1, 0, 0, 0],
                invariance_vector: [1, 1, 0, 0, 5, 5, 1, 1],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4],
                interval_class_vector: [2, 2, 1, 1, 0, 0],
                invariance_vector: [1, 0, 0, 0, 3, 4, 1, 1],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 4],
                interval_class_vector: [2, 1, 2, 1, 0, 0],
                invariance_vector: [1, 1, 0, 0, 3, 3, 2, 2],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 5],
                interval_class_vector: [2, 1, 1, 1, 1, 0],
                invariance_vector: [1, 0, 0, 0, 1, 3, 2, 3],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 6],
                interval_class_vector: [2, 1, 0, 1, 1, 1],
                invariance_vector: [1, 0, 0, 0, 2, 4, 3, 2],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 7],
                interval_class_vector: [2, 1, 0, 0, 2, 1],
                invariance_vector: [1, 1, 1, 1, 4, 4, 4, 4],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 4, 5],
                interval_class_vector: [2, 0, 1, 2, 1, 0],
                invariance_vector: [1, 1, 0, 0, 3, 3, 3, 3],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 5, 6],
                interval_class_vector: [2, 0, 0, 1, 2, 1],
                invariance_vector: [1, 1, 1, 1, 4, 4, 4, 4],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 6, 7],
                interval_class_vector: [2, 0, 0, 0, 2, 2],
                invariance_vector: [2, 2, 2, 2, 6, 6, 6, 6],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 3, 5],
                interval_class_vector: [1, 2, 2, 0, 1, 0],
                invariance_vector: [1, 1, 1, 1, 3, 3, 3, 3],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 5],
                interval_class_vector: [1, 2, 1, 1, 1, 0],
                invariance_vector: [1, 0, 1, 0, 1, 3, 1, 3],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 3, 6],
                interval_class_vector: [1, 1, 2, 1, 0, 1],
                invariance_vector: [1, 0, 0, 0, 2, 4, 3, 2],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 6],
                interval_class_vector: [1, 1, 2, 0, 1, 1],
                invariance_vector: [1, 0, 0, 1, 2, 4, 4, 2],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 3, 7],
                interval_class_vector: [1, 1, 1, 1, 2, 0],
                invariance_vector: [1, 0, 0, 0, 1, 3, 2, 3],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 4, 6],
                interval_class_vector: [1, 1, 1, 1, 1, 1],
                invariance_vector: [1, 0, 0, 0, 0, 3, 3, 1],
                z_relation: 29,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 5, 7],
                interval_class_vector: [1, 1, 0, 1, 2, 1],
                invariance_vector: [1, 0, 0, 0, 2, 4, 3, 2],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 3, 4, 7],
                interval_class_vector: [1, 0, 2, 2, 1, 0],
                invariance_vector: [1, 1, 1, 1, 3, 3, 3, 3],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 4, 7],
                interval_class_vector: [1, 0, 2, 1, 1, 1],
                invariance_vector: [1, 0, 0, 1, 2, 4, 4, 2],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 4, 8],
                interval_class_vector: [1, 0, 1, 3, 1, 0],
                invariance_vector: [1, 0, 1, 0, 3, 5, 3, 5],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 5, 8],
                interval_class_vector: [1, 0, 1, 2, 2, 0],
                invariance_vector: [1, 1, 0, 0, 3, 3, 3, 3],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 4, 6],
                interval_class_vector: [0, 3, 0, 2, 0, 1],
                invariance_vector: [1, 1, 1, 1, 6, 6, 6, 6],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 4, 7],
                interval_class_vector: [0, 2, 1, 1, 2, 0],
                invariance_vector: [1, 0, 0, 0, 3, 4, 1, 1],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 5, 7],
                interval_class_vector: [0, 2, 1, 0, 3, 0],
                invariance_vector: [1, 1, 0, 0, 5, 5, 1, 1],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 4, 8],
                interval_class_vector: [0, 2, 0, 3, 0, 1],
                invariance_vector: [1, 1, 1, 1, 6, 6, 6, 6],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 6, 8],
                interval_class_vector: [0, 2, 0, 2, 0, 2],
                invariance_vector: [2, 2, 2, 2, 6, 6, 6, 6],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 3, 5, 8],
                interval_class_vector: [0, 1, 2, 1, 2, 0],
                invariance_vector: [1, 1, 0, 0, 3, 3, 2, 2],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 5, 8],
                interval_class_vector: [0, 1, 2, 1, 1, 1],
                invariance_vector: [1, 0, 0, 0, 2, 4, 3, 2],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 3, 6, 9],
                interval_class_vector: [0, 0, 4, 0, 0, 2],
                invariance_vector: [4, 4, 4, 4, 8, 8, 8, 8],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 7],
                interval_class_vector: [1, 1, 1, 1, 1, 1],
                invariance_vector: [1, 0, 0, 0, 0, 3, 3, 1],
                z_relation: 15,
            }),
        ],
        vec![
            None,
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4],
                interval_class_vector: [4, 3, 2, 1, 0, 0],
                invariance_vector: [1, 1, 0, 0, 3, 3, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 5],
                interval_class_vector: [3, 3, 2, 1, 1, 0],
                invariance_vector: [1, 0, 0, 0, 1, 2, 1, 1],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 5],
                interval_class_vector: [3, 2, 2, 2, 1, 0],
                invariance_vector: [1, 0, 0, 0, 1, 1, 1, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 6],
                interval_class_vector: [3, 2, 2, 1, 1, 1],
                invariance_vector: [1, 0, 0, 0, 0, 2, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 7],
                interval_class_vector: [3, 2, 1, 1, 2, 1],
                invariance_vector: [1, 0, 0, 0, 0, 1, 1, 1],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 5, 6],
                interval_class_vector: [3, 1, 1, 2, 2, 1],
                invariance_vector: [1, 0, 0, 0, 0, 1, 1, 1],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 6, 7],
                interval_class_vector: [3, 1, 0, 1, 3, 2],
                invariance_vector: [1, 0, 0, 1, 2, 3, 3, 2],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 3, 4, 6],
                interval_class_vector: [2, 3, 2, 2, 0, 1],
                invariance_vector: [1, 1, 0, 0, 2, 2, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 6],
                interval_class_vector: [2, 3, 1, 2, 1, 1],
                invariance_vector: [1, 0, 0, 0, 0, 2, 0, 1],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 4, 6],
                interval_class_vector: [2, 2, 3, 1, 1, 1],
                invariance_vector: [1, 0, 0, 0, 0, 1, 1, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 3, 4, 7],
                interval_class_vector: [2, 2, 2, 2, 2, 0],
                invariance_vector: [1, 0, 1, 0, 1, 1, 1, 1],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 5, 6],
                interval_class_vector: [2, 2, 2, 1, 2, 1],
                invariance_vector: [1, 1, 1, 1, 0, 0, 0, 0],
                z_relation: 36,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 8],
                interval_class_vector: [2, 2, 1, 3, 1, 1],
                invariance_vector: [1, 0, 0, 0, 0, 2, 0, 1],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 5, 7],
                interval_class_vector: [2, 2, 1, 1, 3, 1],
                invariance_vector: [1, 0, 0, 0, 0, 1, 1, 1],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 6, 8],
                interval_class_vector: [2, 2, 0, 2, 2, 2],
                invariance_vector: [1, 1, 1, 1, 2, 2, 2, 2],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 4, 7],
                interval_class_vector: [2, 1, 3, 2, 1, 1],
                invariance_vector: [1, 0, 0, 0, 0, 1, 1, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 4, 8],
                interval_class_vector: [2, 1, 2, 3, 2, 0],
                invariance_vector: [1, 1, 0, 0, 1, 1, 2, 2],
                z_relation: 37,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 4, 5, 7],
                interval_class_vector: [2, 1, 2, 2, 2, 1],
                invariance_vector: [1, 0, 0, 0, 0, 1, 1, 0],
                z_relation: 38,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 6, 7],
                interval_class_vector: [2, 1, 2, 1, 2, 2],
                invariance_vector: [1, 0, 0, 1, 0, 2, 2, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 7, 8],
                interval_class_vector: [2, 1, 1, 2, 3, 1],
                invariance_vector: [1, 0, 0, 0, 0, 1, 1, 1],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 4, 5, 8],
                interval_class_vector: [2, 0, 2, 4, 2, 0],
                invariance_vector: [1, 0, 1, 0, 3, 3, 3, 3],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 4, 7, 8],
                interval_class_vector: [2, 0, 2, 3, 2, 1],
                invariance_vector: [1, 1, 1, 1, 2, 2, 2, 2],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 3, 5, 7],
                interval_class_vector: [1, 3, 2, 1, 3, 0],
                invariance_vector: [1, 0, 0, 0, 1, 2, 1, 1],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 5, 7],
                interval_class_vector: [1, 3, 1, 2, 2, 1],
                invariance_vector: [1, 0, 0, 0, 0, 2, 0, 1],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 3, 5, 8],
                interval_class_vector: [1, 2, 3, 1, 2, 1],
                invariance_vector: [1, 0, 0, 0, 0, 1, 1, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 4, 5, 8],
                interval_class_vector: [1, 2, 2, 3, 1, 1],
                invariance_vector: [1, 0, 1, 0, 0, 2, 0, 2],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 5, 8],
                interval_class_vector: [1, 2, 2, 2, 3, 0],
                invariance_vector: [1, 0, 0, 0, 1, 1, 1, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 3, 6, 8],
                interval_class_vector: [1, 2, 2, 2, 1, 2],
                invariance_vector: [1, 0, 0, 1, 0, 2, 2, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 6, 8],
                interval_class_vector: [1, 2, 2, 1, 3, 1],
                invariance_vector: [1, 0, 0, 0, 0, 2, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 4, 6, 8],
                interval_class_vector: [1, 2, 1, 3, 2, 1],
                invariance_vector: [1, 0, 0, 0, 0, 2, 0, 1],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 6, 9],
                interval_class_vector: [1, 1, 4, 1, 1, 2],
                invariance_vector: [1, 0, 0, 1, 0, 3, 3, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 4, 6, 9],
                interval_class_vector: [1, 1, 3, 2, 2, 1],
                invariance_vector: [1, 0, 0, 0, 0, 1, 1, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 4, 6, 8],
                interval_class_vector: [0, 4, 0, 4, 0, 2],
                invariance_vector: [1, 1, 1, 1, 6, 6, 6, 6],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 4, 6, 9],
                interval_class_vector: [0, 3, 2, 2, 2, 1],
                invariance_vector: [1, 1, 0, 0, 2, 2, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 4, 7, 9],
                interval_class_vector: [0, 3, 2, 1, 4, 0],
                invariance_vector: [1, 1, 0, 0, 3, 3, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 7],
                interval_class_vector: [2, 2, 2, 1, 2, 1],
                invariance_vector: [1, 0, 0, 1, 0, 1, 1, 0],
                z_relation: 12,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 3, 4, 5, 8],
                interval_class_vector: [2, 1, 2, 3, 2, 0],
                invariance_vector: [1, 1, 0, 0, 1, 1, 2, 2],
                z_relation: 17,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 5, 8],
                interval_class_vector: [2, 1, 2, 2, 2, 1],
                invariance_vector: [1, 0, 0, 0, 0, 1, 1, 0],
                z_relation: 18,
            }),
        ],
        vec![
            None,
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 5],
                interval_class_vector: [5, 4, 3, 2, 1, 0],
                invariance_vector: [1, 1, 0, 0, 1, 1, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 6],
                interval_class_vector: [4, 4, 3, 2, 1, 1],
                invariance_vector: [1, 0, 0, 0, 0, 1, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 5, 6],
                interval_class_vector: [4, 3, 3, 2, 2, 1],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 36,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 5, 6],
                interval_class_vector: [4, 3, 2, 3, 2, 1],
                invariance_vector: [1, 1, 0, 0, 0, 0, 0, 0],
                z_relation: 37,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 6, 7],
                interval_class_vector: [4, 2, 2, 2, 3, 2],
                invariance_vector: [1, 0, 0, 0, 0, 1, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 5, 6, 7],
                interval_class_vector: [4, 2, 1, 2, 4, 2],
                invariance_vector: [1, 1, 0, 0, 0, 0, 1, 1],
                z_relation: 38,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 6, 7, 8],
                interval_class_vector: [4, 2, 0, 2, 4, 3],
                invariance_vector: [2, 2, 2, 2, 2, 2, 2, 2],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 3, 4, 5, 7],
                interval_class_vector: [3, 4, 3, 2, 3, 0],
                invariance_vector: [1, 1, 1, 1, 1, 1, 1, 1],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 5, 7],
                interval_class_vector: [3, 4, 2, 2, 3, 1],
                invariance_vector: [1, 0, 1, 0, 0, 1, 0, 1],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 4, 5, 7],
                interval_class_vector: [3, 3, 3, 3, 2, 1],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 39,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 5, 7],
                interval_class_vector: [3, 3, 3, 2, 3, 1],
                invariance_vector: [1, 0, 0, 0, 0, 0, 1, 0],
                z_relation: 40,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 6, 7],
                interval_class_vector: [3, 3, 2, 2, 3, 2],
                invariance_vector: [1, 0, 0, 1, 0, 0, 0, 0],
                z_relation: 41,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 4, 6, 7],
                interval_class_vector: [3, 2, 4, 2, 2, 2],
                invariance_vector: [1, 1, 0, 0, 0, 0, 0, 0],
                z_relation: 42,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 4, 5, 8],
                interval_class_vector: [3, 2, 3, 4, 3, 0],
                invariance_vector: [1, 0, 1, 0, 1, 0, 1, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 5, 8],
                interval_class_vector: [3, 2, 3, 4, 2, 1],
                invariance_vector: [1, 0, 0, 0, 0, 1, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 4, 5, 6, 8],
                interval_class_vector: [3, 2, 2, 4, 3, 1],
                invariance_vector: [1, 0, 1, 0, 0, 1, 0, 1],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 7, 8],
                interval_class_vector: [3, 2, 2, 3, 3, 2],
                invariance_vector: [1, 0, 0, 1, 0, 0, 0, 0],
                z_relation: 43,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 5, 7, 8],
                interval_class_vector: [3, 2, 2, 2, 4, 2],
                invariance_vector: [1, 0, 0, 0, 0, 1, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 4, 7, 8],
                interval_class_vector: [3, 1, 3, 4, 3, 1],
                invariance_vector: [1, 0, 0, 0, 0, 0, 1, 0],
                z_relation: 44,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 4, 5, 8, 9],
                interval_class_vector: [3, 0, 3, 6, 3, 0],
                invariance_vector: [3, 3, 3, 3, 3, 3, 3, 3],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 3, 4, 6, 8],
                interval_class_vector: [2, 4, 2, 4, 1, 2],
                invariance_vector: [1, 0, 0, 0, 0, 1, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 6, 8],
                interval_class_vector: [2, 4, 1, 4, 2, 2],
                invariance_vector: [1, 0, 1, 0, 0, 1, 0, 1],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 3, 5, 6, 8],
                interval_class_vector: [2, 3, 4, 2, 2, 2],
                invariance_vector: [1, 1, 1, 1, 0, 0, 0, 0],
                z_relation: 45,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 4, 6, 8],
                interval_class_vector: [2, 3, 3, 3, 3, 1],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 46,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 5, 6, 8],
                interval_class_vector: [2, 3, 3, 2, 4, 1],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 47,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 5, 7, 8],
                interval_class_vector: [2, 3, 2, 3, 4, 1],
                invariance_vector: [1, 1, 0, 0, 0, 0, 0, 0],
                z_relation: 48,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 4, 6, 9],
                interval_class_vector: [2, 2, 5, 2, 2, 2],
                invariance_vector: [1, 0, 0, 1, 0, 1, 1, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 5, 6, 9],
                interval_class_vector: [2, 2, 4, 3, 2, 2],
                invariance_vector: [1, 1, 1, 1, 0, 0, 0, 0],
                z_relation: 49,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 6, 8, 9],
                interval_class_vector: [2, 2, 4, 2, 3, 2],
                invariance_vector: [1, 1, 0, 0, 0, 0, 0, 0],
                z_relation: 50,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 6, 7, 9],
                interval_class_vector: [2, 2, 4, 2, 2, 3],
                invariance_vector: [2, 0, 0, 2, 0, 2, 2, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 5, 8, 9],
                interval_class_vector: [2, 2, 3, 4, 3, 1],
                invariance_vector: [1, 0, 0, 0, 0, 1, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 4, 5, 7, 9],
                interval_class_vector: [1, 4, 3, 2, 5, 0],
                invariance_vector: [1, 1, 0, 0, 1, 1, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 3, 5, 7, 9],
                interval_class_vector: [1, 4, 3, 2, 4, 1],
                invariance_vector: [1, 0, 0, 0, 0, 1, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 5, 7, 9],
                interval_class_vector: [1, 4, 2, 4, 2, 2],
                invariance_vector: [1, 0, 0, 0, 0, 1, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 4, 6, 8, 10],
                interval_class_vector: [0, 6, 0, 6, 0, 3],
                invariance_vector: [6, 6, 6, 6, 6, 6, 6, 6],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 7],
                interval_class_vector: [4, 3, 3, 2, 2, 1],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 3,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 8],
                interval_class_vector: [4, 3, 2, 3, 2, 1],
                invariance_vector: [1, 1, 0, 0, 0, 0, 0, 0],
                z_relation: 4,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 7, 8],
                interval_class_vector: [4, 2, 1, 2, 4, 2],
                invariance_vector: [1, 1, 0, 0, 0, 0, 1, 1],
                z_relation: 6,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 3, 4, 5, 8],
                interval_class_vector: [3, 3, 3, 3, 2, 1],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 10,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 5, 8],
                interval_class_vector: [3, 3, 3, 2, 3, 1],
                invariance_vector: [1, 0, 0, 0, 0, 0, 1, 0],
                z_relation: 11,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 6, 8],
                interval_class_vector: [3, 3, 2, 2, 3, 2],
                invariance_vector: [1, 0, 0, 1, 0, 0, 0, 0],
                z_relation: 12,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 6, 9],
                interval_class_vector: [3, 2, 4, 2, 2, 2],
                invariance_vector: [1, 1, 0, 0, 0, 0, 0, 0],
                z_relation: 13,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 5, 6, 8],
                interval_class_vector: [3, 2, 2, 3, 3, 2],
                invariance_vector: [1, 0, 0, 1, 0, 0, 0, 0],
                z_relation: 17,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 5, 6, 9],
                interval_class_vector: [3, 1, 3, 4, 3, 1],
                invariance_vector: [1, 0, 0, 0, 0, 0, 1, 0],
                z_relation: 19,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 3, 4, 6, 9],
                interval_class_vector: [2, 3, 4, 2, 2, 2],
                invariance_vector: [1, 1, 1, 1, 0, 0, 0, 0],
                z_relation: 23,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 6, 9],
                interval_class_vector: [2, 3, 3, 3, 3, 1],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 24,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 7, 9],
                interval_class_vector: [2, 3, 3, 2, 4, 1],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 25,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 5, 7, 9],
                interval_class_vector: [2, 3, 2, 3, 4, 1],
                invariance_vector: [1, 1, 0, 0, 0, 0, 0, 0],
                z_relation: 26,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 4, 7, 9],
                interval_class_vector: [2, 2, 4, 3, 2, 2],
                invariance_vector: [1, 1, 1, 1, 0, 0, 0, 0],
                z_relation: 28,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 4, 6, 7, 9],
                interval_class_vector: [2, 2, 4, 2, 3, 2],
                invariance_vector: [1, 1, 0, 0, 0, 0, 0, 0],
                z_relation: 29,
            }),
        ],
        vec![
            None,
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 5, 6],
                interval_class_vector: [6, 5, 4, 3, 2, 1],
                invariance_vector: [1, 1, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 5, 7],
                interval_class_vector: [5, 5, 4, 3, 3, 1],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 5, 8],
                interval_class_vector: [5, 4, 4, 4, 3, 1],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 6, 7],
                interval_class_vector: [5, 4, 4, 3, 3, 2],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 5, 6, 7],
                interval_class_vector: [5, 4, 3, 3, 4, 2],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 7, 8],
                interval_class_vector: [5, 3, 3, 4, 4, 2],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 6, 7, 8],
                interval_class_vector: [5, 3, 2, 3, 5, 3],
                invariance_vector: [1, 0, 0, 1, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 3, 4, 5, 6, 8],
                interval_class_vector: [4, 5, 4, 4, 2, 2],
                invariance_vector: [1, 1, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 6, 8],
                interval_class_vector: [4, 5, 3, 4, 3, 2],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 6, 9],
                interval_class_vector: [4, 4, 5, 3, 3, 2],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 4, 5, 6, 8],
                interval_class_vector: [4, 4, 4, 4, 4, 1],
                invariance_vector: [1, 0, 1, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 7, 9],
                interval_class_vector: [4, 4, 4, 3, 4, 2],
                invariance_vector: [1, 1, 1, 1, 0, 0, 0, 0],
                z_relation: 36,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 5, 6, 8],
                interval_class_vector: [4, 4, 3, 5, 3, 2],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 5, 7, 8],
                interval_class_vector: [4, 4, 3, 3, 5, 2],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 6, 7, 8],
                interval_class_vector: [4, 4, 2, 4, 4, 3],
                invariance_vector: [1, 1, 1, 1, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 5, 6, 9],
                interval_class_vector: [4, 3, 5, 4, 3, 2],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 5, 6, 9],
                interval_class_vector: [4, 3, 4, 5, 4, 1],
                invariance_vector: [1, 1, 0, 0, 0, 0, 0, 0],
                z_relation: 37,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 5, 8, 9],
                interval_class_vector: [4, 3, 4, 4, 4, 2],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 38,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 6, 7, 9],
                interval_class_vector: [4, 3, 4, 3, 4, 3],
                invariance_vector: [1, 0, 0, 1, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 7, 8, 9],
                interval_class_vector: [4, 3, 3, 4, 5, 2],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 5, 8, 9],
                interval_class_vector: [4, 2, 4, 6, 4, 1],
                invariance_vector: [1, 0, 1, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 5, 6, 8, 9],
                interval_class_vector: [4, 2, 4, 5, 4, 2],
                invariance_vector: [1, 1, 1, 1, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 3, 4, 5, 7, 9],
                interval_class_vector: [3, 5, 4, 3, 5, 1],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 5, 7, 9],
                interval_class_vector: [3, 5, 3, 4, 4, 2],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 3, 4, 6, 7, 9],
                interval_class_vector: [3, 4, 5, 3, 4, 2],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 4, 5, 7, 9],
                interval_class_vector: [3, 4, 4, 5, 3, 2],
                invariance_vector: [1, 0, 1, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 5, 7, 9],
                interval_class_vector: [3, 4, 4, 4, 5, 1],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 5, 6, 7, 9],
                interval_class_vector: [3, 4, 4, 4, 3, 3],
                invariance_vector: [1, 0, 0, 1, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 6, 7, 9],
                interval_class_vector: [3, 4, 4, 3, 5, 2],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 6, 8, 9],
                interval_class_vector: [3, 4, 3, 5, 4, 2],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 4, 6, 7, 9],
                interval_class_vector: [3, 3, 6, 3, 3, 3],
                invariance_vector: [1, 0, 0, 1, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 4, 6, 8, 9],
                interval_class_vector: [3, 3, 5, 4, 4, 2],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 6, 8, 10],
                interval_class_vector: [2, 6, 2, 6, 2, 3],
                invariance_vector: [1, 1, 1, 1, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 4, 6, 8, 10],
                interval_class_vector: [2, 5, 4, 4, 4, 2],
                invariance_vector: [1, 1, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 5, 6, 8, 10],
                interval_class_vector: [2, 5, 4, 3, 6, 1],
                invariance_vector: [1, 1, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 5, 6, 8],
                interval_class_vector: [4, 4, 4, 3, 4, 2],
                invariance_vector: [1, 0, 0, 1, 0, 0, 0, 0],
                z_relation: 12,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 4, 5, 7, 8],
                interval_class_vector: [4, 3, 4, 5, 4, 1],
                invariance_vector: [1, 1, 0, 0, 0, 0, 0, 0],
                z_relation: 17,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 5, 7, 8],
                interval_class_vector: [4, 3, 4, 4, 4, 2],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 18,
            }),
        ],
        vec![
            None,
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 5, 6, 7],
                interval_class_vector: [7, 6, 5, 4, 4, 2],
                invariance_vector: [1, 1, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 5, 6, 8],
                interval_class_vector: [6, 6, 5, 5, 4, 2],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 5, 6, 9],
                interval_class_vector: [6, 5, 6, 5, 4, 2],
                invariance_vector: [1, 1, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 5, 7, 8],
                interval_class_vector: [6, 5, 5, 5, 5, 2],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 6, 7, 8],
                interval_class_vector: [6, 5, 4, 5, 5, 3],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 5, 6, 7, 8],
                interval_class_vector: [6, 5, 4, 4, 6, 3],
                invariance_vector: [1, 1, 1, 1, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 5, 8, 9],
                interval_class_vector: [6, 4, 5, 6, 5, 2],
                invariance_vector: [1, 1, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 7, 8, 9],
                interval_class_vector: [6, 4, 4, 5, 6, 3],
                invariance_vector: [1, 1, 1, 1, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 6, 7, 8, 9],
                interval_class_vector: [6, 4, 4, 4, 6, 4],
                invariance_vector: [2, 2, 2, 2, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 2, 3, 4, 5, 6, 7, 9],
                interval_class_vector: [5, 6, 6, 4, 5, 2],
                invariance_vector: [1, 1, 1, 1, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 5, 7, 9],
                interval_class_vector: [5, 6, 5, 5, 5, 2],
                invariance_vector: [1, 0, 1, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 4, 5, 6, 7, 9],
                interval_class_vector: [5, 5, 6, 5, 4, 3],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 6, 7, 9],
                interval_class_vector: [5, 5, 6, 4, 5, 3],
                invariance_vector: [1, 0, 0, 1, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 5, 6, 7, 9],
                interval_class_vector: [5, 5, 5, 5, 6, 2],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 6, 8, 9],
                interval_class_vector: [5, 5, 5, 5, 5, 3],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 29,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 5, 7, 8, 9],
                interval_class_vector: [5, 5, 4, 5, 6, 3],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 4, 5, 6, 8, 9],
                interval_class_vector: [5, 4, 6, 6, 5, 2],
                invariance_vector: [1, 1, 1, 1, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 5, 6, 8, 9],
                interval_class_vector: [5, 4, 6, 5, 5, 3],
                invariance_vector: [1, 0, 0, 1, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 5, 6, 8, 9],
                interval_class_vector: [5, 4, 5, 7, 5, 2],
                invariance_vector: [1, 0, 1, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 5, 7, 8, 9],
                interval_class_vector: [5, 4, 5, 6, 6, 2],
                invariance_vector: [1, 1, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 6, 8, 10],
                interval_class_vector: [4, 7, 4, 6, 4, 3],
                invariance_vector: [1, 1, 1, 1, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 5, 6, 8, 10],
                interval_class_vector: [4, 6, 5, 5, 6, 2],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 5, 7, 8, 10],
                interval_class_vector: [4, 6, 5, 4, 7, 2],
                invariance_vector: [1, 1, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 5, 6, 8, 10],
                interval_class_vector: [4, 6, 4, 7, 4, 3],
                invariance_vector: [1, 1, 1, 1, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 6, 7, 8, 10],
                interval_class_vector: [4, 6, 4, 6, 4, 4],
                invariance_vector: [2, 2, 2, 2, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 5, 7, 9, 10],
                interval_class_vector: [4, 5, 6, 5, 6, 2],
                invariance_vector: [1, 1, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 5, 7, 8, 10],
                interval_class_vector: [4, 5, 6, 5, 5, 3],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 3, 4, 6, 7, 9, 10],
                interval_class_vector: [4, 4, 8, 4, 4, 4],
                invariance_vector: [4, 4, 4, 4, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 5, 6, 7, 9],
                interval_class_vector: [5, 5, 5, 5, 5, 3],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 15,
            }),
        ],
        vec![
            None,
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 5, 6, 7, 8],
                interval_class_vector: [8, 7, 6, 6, 6, 3],
                invariance_vector: [1, 1, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 5, 6, 7, 9],
                interval_class_vector: [7, 7, 7, 6, 6, 3],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 5, 6, 8, 9],
                interval_class_vector: [7, 6, 7, 7, 6, 3],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 5, 7, 8, 9],
                interval_class_vector: [7, 6, 6, 7, 7, 3],
                invariance_vector: [1, 0, 1, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 6, 7, 8, 9],
                interval_class_vector: [7, 6, 6, 6, 7, 4],
                invariance_vector: [1, 0, 0, 1, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 5, 6, 8, 10],
                interval_class_vector: [6, 8, 6, 7, 6, 3],
                invariance_vector: [1, 1, 1, 1, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 5, 7, 8, 10],
                interval_class_vector: [6, 7, 7, 6, 7, 3],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 6, 7, 8, 10],
                interval_class_vector: [6, 7, 6, 7, 6, 4],
                invariance_vector: [1, 0, 0, 1, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 5, 6, 7, 8, 10],
                interval_class_vector: [6, 7, 6, 6, 8, 3],
                invariance_vector: [1, 1, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 6, 7, 9, 10],
                interval_class_vector: [6, 6, 8, 6, 6, 4],
                invariance_vector: [1, 1, 1, 1, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 5, 6, 7, 9, 10],
                interval_class_vector: [6, 6, 7, 7, 7, 3],
                invariance_vector: [1, 0, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 4, 5, 6, 8, 9, 10],
                interval_class_vector: [6, 6, 6, 9, 6, 3],
                invariance_vector: [3, 3, 3, 3, 0, 0, 0, 0],
                z_relation: 0,
            }),
        ],
        vec![
            None,
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
                interval_class_vector: [9, 8, 8, 8, 8, 4],
                invariance_vector: [1, 1, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 10],
                interval_class_vector: [8, 9, 8, 8, 8, 4],
                invariance_vector: [1, 1, 1, 1, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 5, 6, 7, 9, 10],
                interval_class_vector: [8, 8, 9, 8, 8, 4],
                invariance_vector: [1, 1, 1, 1, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 5, 6, 8, 9, 10],
                interval_class_vector: [8, 8, 8, 9, 8, 4],
                invariance_vector: [1, 1, 1, 1, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 5, 7, 8, 9, 10],
                interval_class_vector: [8, 8, 8, 8, 9, 4],
                invariance_vector: [1, 1, 0, 0, 0, 0, 0, 0],
                z_relation: 0,
            }),
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 6, 7, 8, 9, 10],
                interval_class_vector: [8, 8, 8, 8, 8, 5],
                invariance_vector: [2, 2, 2, 2, 0, 0, 0, 0],
                z_relation: 0,
            }),
        ],
        vec![
            None,
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
                interval_class_vector: [10, 10, 10, 10, 10, 5],
                invariance_vector: [1, 1, 1, 1, 0, 0, 0, 0],
                z_relation: 0,
            }),
        ],
        vec![
            None,
            Some(TNIStructure {
                pitch_classes: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
                interval_class_vector: [12, 12, 12, 12, 12, 6],
                invariance_vector: [12, 12, 12, 12, 0, 0, 0, 0],
                z_relation: 0,
            }),
        ],
    ]
});

static INVERSION_DEFAULT_PITCH_CLASSES: LazyLock<HashMap<(u8, u8), Vec<u8>>> =
    LazyLock::new(|| {
        let mut m = HashMap::new();
        m.insert((3, 2), vec![0, 2, 3]);
        m.insert((3, 3), vec![0, 3, 4]);
        m.insert((3, 4), vec![0, 4, 5]);
        m.insert((3, 5), vec![0, 5, 6]);
        m.insert((3, 7), vec![0, 3, 5]);
        m.insert((3, 8), vec![0, 4, 6]);
        m.insert((3, 11), vec![0, 4, 7]);
        m.insert((4, 2), vec![0, 2, 3, 4]);
        m.insert((4, 4), vec![0, 3, 4, 5]);
        m.insert((4, 5), vec![0, 4, 5, 6]);
        m.insert((4, 11), vec![0, 2, 4, 5]);
        m.insert((4, 12), vec![0, 3, 4, 6]);
        m.insert((4, 13), vec![0, 3, 5, 6]);
        m.insert((4, 14), vec![0, 4, 5, 7]);
        m.insert((4, 15), vec![0, 2, 5, 6]);
        m.insert((4, 16), vec![0, 2, 6, 7]);
        m.insert((4, 18), vec![0, 3, 6, 7]);
        m.insert((4, 19), vec![0, 3, 4, 8]);
        m.insert((4, 22), vec![0, 3, 5, 7]);
        m.insert((4, 27), vec![0, 3, 6, 8]);
        m.insert((4, 29), vec![0, 4, 6, 7]);
        m.insert((5, 2), vec![0, 2, 3, 4, 5]);
        m.insert((5, 3), vec![0, 1, 3, 4, 5]);
        m.insert((5, 4), vec![0, 3, 4, 5, 6]);
        m.insert((5, 5), vec![0, 4, 5, 6, 7]);
        m.insert((5, 6), vec![0, 1, 4, 5, 6]);
        m.insert((5, 7), vec![0, 1, 5, 6, 7]);
        m.insert((5, 9), vec![0, 2, 4, 5, 6]);
        m.insert((5, 10), vec![0, 2, 3, 5, 6]);
        m.insert((5, 11), vec![0, 3, 4, 5, 7]);
        m.insert((5, 13), vec![0, 2, 3, 4, 8]);
        m.insert((5, 14), vec![0, 2, 5, 6, 7]);
        m.insert((5, 16), vec![0, 3, 4, 6, 7]);
        m.insert((5, 18), vec![0, 2, 3, 6, 7]);
        m.insert((5, 19), vec![0, 1, 4, 6, 7]);
        m.insert((5, 20), vec![0, 1, 5, 7, 8]);
        m.insert((5, 21), vec![0, 3, 4, 7, 8]);
        m.insert((5, 23), vec![0, 2, 4, 5, 7]);
        m.insert((5, 24), vec![0, 2, 4, 6, 7]);
        m.insert((5, 25), vec![0, 3, 5, 6, 8]);
        m.insert((5, 26), vec![0, 3, 4, 6, 8]);
        m.insert((5, 27), vec![0, 3, 5, 7, 8]);
        m.insert((5, 28), vec![0, 2, 5, 6, 8]);
        m.insert((5, 29), vec![0, 2, 5, 7, 8]);
        m.insert((5, 30), vec![0, 2, 4, 7, 8]);
        m.insert((5, 31), vec![0, 2, 3, 6, 9]);
        m.insert((5, 32), vec![0, 1, 4, 7, 9]);
        m.insert((5, 36), vec![0, 3, 5, 6, 7]);
        m.insert((5, 38), vec![0, 3, 6, 7, 8]);
        m.insert((6, 2), vec![0, 2, 3, 4, 5, 6]);
        m.insert((6, 3), vec![0, 1, 3, 4, 5, 6]);
        m.insert((6, 5), vec![0, 1, 4, 5, 6, 7]);
        m.insert((6, 9), vec![0, 2, 4, 5, 6, 7]);
        m.insert((6, 10), vec![0, 2, 3, 4, 6, 7]);
        m.insert((6, 11), vec![0, 2, 3, 5, 6, 7]);
        m.insert((6, 12), vec![0, 1, 3, 5, 6, 7]);
        m.insert((6, 14), vec![0, 3, 4, 5, 7, 8]);
        m.insert((6, 15), vec![0, 3, 4, 6, 7, 8]);
        m.insert((6, 16), vec![0, 2, 3, 4, 7, 8]);
        m.insert((6, 17), vec![0, 1, 4, 6, 7, 8]);
        m.insert((6, 18), vec![0, 1, 3, 6, 7, 8]);
        m.insert((6, 19), vec![0, 1, 4, 5, 7, 8]);
        m.insert((6, 21), vec![0, 2, 4, 5, 6, 8]);
        m.insert((6, 22), vec![0, 2, 4, 6, 7, 8]);
        m.insert((6, 24), vec![0, 2, 4, 5, 7, 8]);
        m.insert((6, 25), vec![0, 2, 3, 5, 7, 8]);
        m.insert((6, 27), vec![0, 2, 3, 5, 6, 9]);
        m.insert((6, 30), vec![0, 2, 3, 6, 8, 9]);
        m.insert((6, 31), vec![0, 1, 4, 6, 8, 9]);
        m.insert((6, 33), vec![0, 2, 4, 6, 7, 9]);
        m.insert((6, 34), vec![0, 2, 4, 6, 8, 9]);
        m.insert((6, 36), vec![0, 3, 4, 5, 6, 7]);
        m.insert((6, 39), vec![0, 3, 4, 5, 6, 8]);
        m.insert((6, 40), vec![0, 3, 5, 6, 7, 8]);
        m.insert((6, 41), vec![0, 2, 5, 6, 7, 8]);
        m.insert((6, 43), vec![0, 2, 3, 6, 7, 8]);
        m.insert((6, 44), vec![0, 1, 2, 5, 8, 9]);
        m.insert((6, 46), vec![0, 2, 4, 5, 6, 9]);
        m.insert((6, 47), vec![0, 2, 3, 4, 7, 9]);
        m.insert((7, 2), vec![0, 2, 3, 4, 5, 6, 7]);
        m.insert((7, 3), vec![0, 3, 4, 5, 6, 7, 8]);
        m.insert((7, 4), vec![0, 1, 3, 4, 5, 6, 7]);
        m.insert((7, 5), vec![0, 1, 2, 4, 5, 6, 7]);
        m.insert((7, 6), vec![0, 1, 4, 5, 6, 7, 8]);
        m.insert((7, 7), vec![0, 1, 2, 5, 6, 7, 8]);
        m.insert((7, 9), vec![0, 2, 4, 5, 6, 7, 8]);
        m.insert((7, 10), vec![0, 2, 3, 4, 5, 6, 9]);
        m.insert((7, 11), vec![0, 2, 3, 4, 5, 7, 8]);
        m.insert((7, 13), vec![0, 2, 3, 4, 6, 7, 8]);
        m.insert((7, 14), vec![0, 1, 3, 5, 6, 7, 8]);
        m.insert((7, 16), vec![0, 1, 3, 4, 5, 6, 9]);
        m.insert((7, 18), vec![0, 1, 4, 6, 7, 8, 9]);
        m.insert((7, 19), vec![0, 1, 2, 3, 6, 8, 9]);
        m.insert((7, 20), vec![0, 1, 2, 5, 7, 8, 9]);
        m.insert((7, 21), vec![0, 1, 3, 4, 5, 8, 9]);
        m.insert((7, 23), vec![0, 2, 4, 5, 6, 7, 9]);
        m.insert((7, 24), vec![0, 2, 4, 6, 7, 8, 9]);
        m.insert((7, 25), vec![0, 2, 3, 5, 6, 7, 9]);
        m.insert((7, 26), vec![0, 2, 4, 5, 6, 8, 9]);
        m.insert((7, 27), vec![0, 2, 4, 5, 7, 8, 9]);
        m.insert((7, 28), vec![0, 2, 3, 4, 6, 8, 9]);
        m.insert((7, 29), vec![0, 2, 3, 5, 7, 8, 9]);
        m.insert((7, 30), vec![0, 1, 3, 5, 7, 8, 9]);
        m.insert((7, 31), vec![0, 2, 3, 5, 6, 8, 9]);
        m.insert((7, 32), vec![0, 1, 3, 5, 6, 8, 9]);
        m.insert((7, 36), vec![0, 2, 3, 5, 6, 7, 8]);
        m.insert((7, 38), vec![0, 1, 3, 4, 6, 7, 8]);
        m.insert((8, 2), vec![0, 2, 3, 4, 5, 6, 7, 8]);
        m.insert((8, 4), vec![0, 1, 3, 4, 5, 6, 7, 8]);
        m.insert((8, 5), vec![0, 1, 2, 4, 5, 6, 7, 8]);
        m.insert((8, 11), vec![0, 2, 4, 5, 6, 7, 8, 9]);
        m.insert((8, 12), vec![0, 2, 3, 4, 5, 6, 8, 9]);
        m.insert((8, 13), vec![0, 2, 3, 5, 6, 7, 8, 9]);
        m.insert((8, 14), vec![0, 2, 3, 4, 5, 7, 8, 9]);
        m.insert((8, 15), vec![0, 1, 3, 5, 6, 7, 8, 9]);
        m.insert((8, 16), vec![0, 1, 2, 4, 6, 7, 8, 9]);
        m.insert((8, 18), vec![0, 1, 3, 4, 6, 7, 8, 9]);
        m.insert((8, 19), vec![0, 1, 3, 4, 5, 7, 8, 9]);
        m.insert((8, 22), vec![0, 1, 2, 3, 5, 7, 9, 10]);
        m.insert((8, 27), vec![0, 1, 2, 4, 6, 7, 9, 10]);
        m.insert((8, 29), vec![0, 2, 3, 4, 6, 7, 8, 9]);
        m.insert((9, 2), vec![0, 2, 3, 4, 5, 6, 7, 8, 9]);
        m.insert((9, 3), vec![0, 1, 3, 4, 5, 6, 7, 8, 9]);
        m.insert((9, 4), vec![0, 1, 2, 4, 5, 6, 7, 8, 9]);
        m.insert((9, 5), vec![0, 1, 2, 3, 5, 6, 7, 8, 9]);
        m.insert((9, 7), vec![0, 1, 2, 3, 4, 5, 7, 9, 10]);
        m.insert((9, 8), vec![0, 1, 2, 3, 4, 6, 8, 9, 10]);
        m.insert((9, 11), vec![0, 1, 2, 3, 5, 6, 8, 9, 10]);
        m
    });

static FORTE_NUMBER_WITH_INVERSION_TO_INDEX: LazyLock<HashMap<U8U8SB, u8>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert((1, 1, SuperBool::Zero), 1);
    m.insert((2, 1, SuperBool::Zero), 1);
    m.insert((2, 2, SuperBool::Zero), 2);
    m.insert((2, 3, SuperBool::Zero), 3);
    m.insert((2, 4, SuperBool::Zero), 4);
    m.insert((2, 5, SuperBool::Zero), 5);
    m.insert((2, 6, SuperBool::Zero), 6);
    m.insert((3, 1, SuperBool::Zero), 1);
    m.insert((3, 2, SuperBool::One), 2);
    m.insert((3, 2, SuperBool::NegativeOne), 3);
    m.insert((3, 3, SuperBool::One), 4);
    m.insert((3, 3, SuperBool::NegativeOne), 5);
    m.insert((3, 4, SuperBool::One), 6);
    m.insert((3, 4, SuperBool::NegativeOne), 7);
    m.insert((3, 5, SuperBool::One), 8);
    m.insert((3, 5, SuperBool::NegativeOne), 9);
    m.insert((3, 6, SuperBool::Zero), 10);
    m.insert((3, 7, SuperBool::One), 11);
    m.insert((3, 7, SuperBool::NegativeOne), 12);
    m.insert((3, 8, SuperBool::One), 13);
    m.insert((3, 8, SuperBool::NegativeOne), 14);
    m.insert((3, 9, SuperBool::Zero), 15);
    m.insert((3, 10, SuperBool::Zero), 16);
    m.insert((3, 11, SuperBool::One), 17);
    m.insert((3, 11, SuperBool::NegativeOne), 18);
    m.insert((3, 12, SuperBool::Zero), 19);
    m.insert((4, 1, SuperBool::Zero), 1);
    m.insert((4, 2, SuperBool::One), 2);
    m.insert((4, 2, SuperBool::NegativeOne), 3);
    m.insert((4, 3, SuperBool::Zero), 4);
    m.insert((4, 4, SuperBool::One), 5);
    m.insert((4, 4, SuperBool::NegativeOne), 6);
    m.insert((4, 5, SuperBool::One), 7);
    m.insert((4, 5, SuperBool::NegativeOne), 8);
    m.insert((4, 6, SuperBool::Zero), 9);
    m.insert((4, 7, SuperBool::Zero), 10);
    m.insert((4, 8, SuperBool::Zero), 11);
    m.insert((4, 9, SuperBool::Zero), 12);
    m.insert((4, 10, SuperBool::Zero), 13);
    m.insert((4, 11, SuperBool::One), 14);
    m.insert((4, 11, SuperBool::NegativeOne), 15);
    m.insert((4, 12, SuperBool::One), 16);
    m.insert((4, 12, SuperBool::NegativeOne), 17);
    m.insert((4, 13, SuperBool::One), 18);
    m.insert((4, 13, SuperBool::NegativeOne), 19);
    m.insert((4, 14, SuperBool::One), 20);
    m.insert((4, 14, SuperBool::NegativeOne), 21);
    m.insert((4, 15, SuperBool::One), 22);
    m.insert((4, 15, SuperBool::NegativeOne), 23);
    m.insert((4, 16, SuperBool::One), 24);
    m.insert((4, 16, SuperBool::NegativeOne), 25);
    m.insert((4, 17, SuperBool::Zero), 26);
    m.insert((4, 18, SuperBool::One), 27);
    m.insert((4, 18, SuperBool::NegativeOne), 28);
    m.insert((4, 19, SuperBool::One), 29);
    m.insert((4, 19, SuperBool::NegativeOne), 30);
    m.insert((4, 20, SuperBool::Zero), 31);
    m.insert((4, 21, SuperBool::Zero), 32);
    m.insert((4, 22, SuperBool::One), 33);
    m.insert((4, 22, SuperBool::NegativeOne), 34);
    m.insert((4, 23, SuperBool::Zero), 35);
    m.insert((4, 24, SuperBool::Zero), 36);
    m.insert((4, 25, SuperBool::Zero), 37);
    m.insert((4, 26, SuperBool::Zero), 38);
    m.insert((4, 27, SuperBool::One), 39);
    m.insert((4, 27, SuperBool::NegativeOne), 40);
    m.insert((4, 28, SuperBool::Zero), 41);
    m.insert((4, 29, SuperBool::One), 42);
    m.insert((4, 29, SuperBool::NegativeOne), 43);
    m.insert((5, 1, SuperBool::Zero), 1);
    m.insert((5, 2, SuperBool::One), 2);
    m.insert((5, 2, SuperBool::NegativeOne), 3);
    m.insert((5, 3, SuperBool::One), 4);
    m.insert((5, 3, SuperBool::NegativeOne), 5);
    m.insert((5, 4, SuperBool::One), 6);
    m.insert((5, 4, SuperBool::NegativeOne), 7);
    m.insert((5, 5, SuperBool::One), 8);
    m.insert((5, 5, SuperBool::NegativeOne), 9);
    m.insert((5, 6, SuperBool::One), 10);
    m.insert((5, 6, SuperBool::NegativeOne), 11);
    m.insert((5, 7, SuperBool::One), 12);
    m.insert((5, 7, SuperBool::NegativeOne), 13);
    m.insert((5, 8, SuperBool::Zero), 14);
    m.insert((5, 9, SuperBool::One), 15);
    m.insert((5, 9, SuperBool::NegativeOne), 16);
    m.insert((5, 10, SuperBool::One), 17);
    m.insert((5, 10, SuperBool::NegativeOne), 18);
    m.insert((5, 11, SuperBool::One), 19);
    m.insert((5, 11, SuperBool::NegativeOne), 20);
    m.insert((5, 12, SuperBool::Zero), 21);
    m.insert((5, 13, SuperBool::One), 22);
    m.insert((5, 13, SuperBool::NegativeOne), 23);
    m.insert((5, 14, SuperBool::One), 24);
    m.insert((5, 14, SuperBool::NegativeOne), 25);
    m.insert((5, 15, SuperBool::Zero), 26);
    m.insert((5, 16, SuperBool::One), 27);
    m.insert((5, 16, SuperBool::NegativeOne), 28);
    m.insert((5, 17, SuperBool::Zero), 29);
    m.insert((5, 18, SuperBool::One), 30);
    m.insert((5, 18, SuperBool::NegativeOne), 31);
    m.insert((5, 19, SuperBool::One), 32);
    m.insert((5, 19, SuperBool::NegativeOne), 33);
    m.insert((5, 20, SuperBool::One), 34);
    m.insert((5, 20, SuperBool::NegativeOne), 35);
    m.insert((5, 21, SuperBool::One), 36);
    m.insert((5, 21, SuperBool::NegativeOne), 37);
    m.insert((5, 22, SuperBool::Zero), 38);
    m.insert((5, 23, SuperBool::One), 39);
    m.insert((5, 23, SuperBool::NegativeOne), 40);
    m.insert((5, 24, SuperBool::One), 41);
    m.insert((5, 24, SuperBool::NegativeOne), 42);
    m.insert((5, 25, SuperBool::One), 43);
    m.insert((5, 25, SuperBool::NegativeOne), 44);
    m.insert((5, 26, SuperBool::One), 45);
    m.insert((5, 26, SuperBool::NegativeOne), 46);
    m.insert((5, 27, SuperBool::One), 47);
    m.insert((5, 27, SuperBool::NegativeOne), 48);
    m.insert((5, 28, SuperBool::One), 49);
    m.insert((5, 28, SuperBool::NegativeOne), 50);
    m.insert((5, 29, SuperBool::One), 51);
    m.insert((5, 29, SuperBool::NegativeOne), 52);
    m.insert((5, 30, SuperBool::One), 53);
    m.insert((5, 30, SuperBool::NegativeOne), 54);
    m.insert((5, 31, SuperBool::One), 55);
    m.insert((5, 31, SuperBool::NegativeOne), 56);
    m.insert((5, 32, SuperBool::One), 57);
    m.insert((5, 32, SuperBool::NegativeOne), 58);
    m.insert((5, 33, SuperBool::Zero), 59);
    m.insert((5, 34, SuperBool::Zero), 60);
    m.insert((5, 35, SuperBool::Zero), 61);
    m.insert((5, 36, SuperBool::One), 62);
    m.insert((5, 36, SuperBool::NegativeOne), 63);
    m.insert((5, 37, SuperBool::Zero), 64);
    m.insert((5, 38, SuperBool::One), 65);
    m.insert((5, 38, SuperBool::NegativeOne), 66);
    m.insert((6, 1, SuperBool::Zero), 1);
    m.insert((6, 2, SuperBool::One), 2);
    m.insert((6, 2, SuperBool::NegativeOne), 3);
    m.insert((6, 3, SuperBool::One), 4);
    m.insert((6, 3, SuperBool::NegativeOne), 5);
    m.insert((6, 4, SuperBool::Zero), 6);
    m.insert((6, 5, SuperBool::One), 7);
    m.insert((6, 5, SuperBool::NegativeOne), 8);
    m.insert((6, 6, SuperBool::Zero), 9);
    m.insert((6, 7, SuperBool::Zero), 10);
    m.insert((6, 8, SuperBool::Zero), 11);
    m.insert((6, 9, SuperBool::One), 12);
    m.insert((6, 9, SuperBool::NegativeOne), 13);
    m.insert((6, 10, SuperBool::One), 14);
    m.insert((6, 10, SuperBool::NegativeOne), 15);
    m.insert((6, 11, SuperBool::One), 16);
    m.insert((6, 11, SuperBool::NegativeOne), 17);
    m.insert((6, 12, SuperBool::One), 18);
    m.insert((6, 12, SuperBool::NegativeOne), 19);
    m.insert((6, 13, SuperBool::Zero), 20);
    m.insert((6, 14, SuperBool::One), 21);
    m.insert((6, 14, SuperBool::NegativeOne), 22);
    m.insert((6, 15, SuperBool::One), 23);
    m.insert((6, 15, SuperBool::NegativeOne), 24);
    m.insert((6, 16, SuperBool::One), 25);
    m.insert((6, 16, SuperBool::NegativeOne), 26);
    m.insert((6, 17, SuperBool::One), 27);
    m.insert((6, 17, SuperBool::NegativeOne), 28);
    m.insert((6, 18, SuperBool::One), 29);
    m.insert((6, 18, SuperBool::NegativeOne), 30);
    m.insert((6, 19, SuperBool::One), 31);
    m.insert((6, 19, SuperBool::NegativeOne), 32);
    m.insert((6, 20, SuperBool::Zero), 33);
    m.insert((6, 21, SuperBool::One), 34);
    m.insert((6, 21, SuperBool::NegativeOne), 35);
    m.insert((6, 22, SuperBool::One), 36);
    m.insert((6, 22, SuperBool::NegativeOne), 37);
    m.insert((6, 23, SuperBool::Zero), 38);
    m.insert((6, 24, SuperBool::One), 39);
    m.insert((6, 24, SuperBool::NegativeOne), 40);
    m.insert((6, 25, SuperBool::One), 41);
    m.insert((6, 25, SuperBool::NegativeOne), 42);
    m.insert((6, 26, SuperBool::Zero), 43);
    m.insert((6, 27, SuperBool::One), 44);
    m.insert((6, 27, SuperBool::NegativeOne), 45);
    m.insert((6, 28, SuperBool::Zero), 46);
    m.insert((6, 29, SuperBool::Zero), 47);
    m.insert((6, 30, SuperBool::One), 48);
    m.insert((6, 30, SuperBool::NegativeOne), 49);
    m.insert((6, 31, SuperBool::One), 50);
    m.insert((6, 31, SuperBool::NegativeOne), 51);
    m.insert((6, 32, SuperBool::Zero), 52);
    m.insert((6, 33, SuperBool::One), 53);
    m.insert((6, 33, SuperBool::NegativeOne), 54);
    m.insert((6, 34, SuperBool::One), 55);
    m.insert((6, 34, SuperBool::NegativeOne), 56);
    m.insert((6, 35, SuperBool::Zero), 57);
    m.insert((6, 36, SuperBool::One), 58);
    m.insert((6, 36, SuperBool::NegativeOne), 59);
    m.insert((6, 37, SuperBool::Zero), 60);
    m.insert((6, 38, SuperBool::Zero), 61);
    m.insert((6, 39, SuperBool::One), 62);
    m.insert((6, 39, SuperBool::NegativeOne), 63);
    m.insert((6, 40, SuperBool::One), 64);
    m.insert((6, 40, SuperBool::NegativeOne), 65);
    m.insert((6, 41, SuperBool::One), 66);
    m.insert((6, 41, SuperBool::NegativeOne), 67);
    m.insert((6, 42, SuperBool::Zero), 68);
    m.insert((6, 43, SuperBool::One), 69);
    m.insert((6, 43, SuperBool::NegativeOne), 70);
    m.insert((6, 44, SuperBool::One), 71);
    m.insert((6, 44, SuperBool::NegativeOne), 72);
    m.insert((6, 45, SuperBool::Zero), 73);
    m.insert((6, 46, SuperBool::One), 74);
    m.insert((6, 46, SuperBool::NegativeOne), 75);
    m.insert((6, 47, SuperBool::One), 76);
    m.insert((6, 47, SuperBool::NegativeOne), 77);
    m.insert((6, 48, SuperBool::Zero), 78);
    m.insert((6, 49, SuperBool::Zero), 79);
    m.insert((6, 50, SuperBool::Zero), 80);
    m.insert((7, 1, SuperBool::Zero), 1);
    m.insert((7, 2, SuperBool::One), 2);
    m.insert((7, 2, SuperBool::NegativeOne), 3);
    m.insert((7, 3, SuperBool::One), 4);
    m.insert((7, 3, SuperBool::NegativeOne), 5);
    m.insert((7, 4, SuperBool::One), 6);
    m.insert((7, 4, SuperBool::NegativeOne), 7);
    m.insert((7, 5, SuperBool::One), 8);
    m.insert((7, 5, SuperBool::NegativeOne), 9);
    m.insert((7, 6, SuperBool::One), 10);
    m.insert((7, 6, SuperBool::NegativeOne), 11);
    m.insert((7, 7, SuperBool::One), 12);
    m.insert((7, 7, SuperBool::NegativeOne), 13);
    m.insert((7, 8, SuperBool::Zero), 14);
    m.insert((7, 9, SuperBool::One), 15);
    m.insert((7, 9, SuperBool::NegativeOne), 16);
    m.insert((7, 10, SuperBool::One), 17);
    m.insert((7, 10, SuperBool::NegativeOne), 18);
    m.insert((7, 11, SuperBool::One), 19);
    m.insert((7, 11, SuperBool::NegativeOne), 20);
    m.insert((7, 12, SuperBool::Zero), 21);
    m.insert((7, 13, SuperBool::One), 22);
    m.insert((7, 13, SuperBool::NegativeOne), 23);
    m.insert((7, 14, SuperBool::One), 24);
    m.insert((7, 14, SuperBool::NegativeOne), 25);
    m.insert((7, 15, SuperBool::Zero), 26);
    m.insert((7, 16, SuperBool::One), 27);
    m.insert((7, 16, SuperBool::NegativeOne), 28);
    m.insert((7, 17, SuperBool::Zero), 29);
    m.insert((7, 18, SuperBool::One), 30);
    m.insert((7, 18, SuperBool::NegativeOne), 31);
    m.insert((7, 19, SuperBool::One), 32);
    m.insert((7, 19, SuperBool::NegativeOne), 33);
    m.insert((7, 20, SuperBool::One), 34);
    m.insert((7, 20, SuperBool::NegativeOne), 35);
    m.insert((7, 21, SuperBool::One), 36);
    m.insert((7, 21, SuperBool::NegativeOne), 37);
    m.insert((7, 22, SuperBool::Zero), 38);
    m.insert((7, 23, SuperBool::One), 39);
    m.insert((7, 23, SuperBool::NegativeOne), 40);
    m.insert((7, 24, SuperBool::One), 41);
    m.insert((7, 24, SuperBool::NegativeOne), 42);
    m.insert((7, 25, SuperBool::One), 43);
    m.insert((7, 25, SuperBool::NegativeOne), 44);
    m.insert((7, 26, SuperBool::One), 45);
    m.insert((7, 26, SuperBool::NegativeOne), 46);
    m.insert((7, 27, SuperBool::One), 47);
    m.insert((7, 27, SuperBool::NegativeOne), 48);
    m.insert((7, 28, SuperBool::One), 49);
    m.insert((7, 28, SuperBool::NegativeOne), 50);
    m.insert((7, 29, SuperBool::One), 51);
    m.insert((7, 29, SuperBool::NegativeOne), 52);
    m.insert((7, 30, SuperBool::One), 53);
    m.insert((7, 30, SuperBool::NegativeOne), 54);
    m.insert((7, 31, SuperBool::One), 55);
    m.insert((7, 31, SuperBool::NegativeOne), 56);
    m.insert((7, 32, SuperBool::One), 57);
    m.insert((7, 32, SuperBool::NegativeOne), 58);
    m.insert((7, 33, SuperBool::Zero), 59);
    m.insert((7, 34, SuperBool::Zero), 60);
    m.insert((7, 35, SuperBool::Zero), 61);
    m.insert((7, 36, SuperBool::One), 62);
    m.insert((7, 36, SuperBool::NegativeOne), 63);
    m.insert((7, 37, SuperBool::Zero), 64);
    m.insert((7, 38, SuperBool::One), 65);
    m.insert((7, 38, SuperBool::NegativeOne), 66);
    m.insert((8, 1, SuperBool::Zero), 1);
    m.insert((8, 2, SuperBool::One), 2);
    m.insert((8, 2, SuperBool::NegativeOne), 3);
    m.insert((8, 3, SuperBool::Zero), 4);
    m.insert((8, 4, SuperBool::One), 5);
    m.insert((8, 4, SuperBool::NegativeOne), 6);
    m.insert((8, 5, SuperBool::One), 7);
    m.insert((8, 5, SuperBool::NegativeOne), 8);
    m.insert((8, 6, SuperBool::Zero), 9);
    m.insert((8, 7, SuperBool::Zero), 10);
    m.insert((8, 8, SuperBool::Zero), 11);
    m.insert((8, 9, SuperBool::Zero), 12);
    m.insert((8, 10, SuperBool::Zero), 13);
    m.insert((8, 11, SuperBool::One), 14);
    m.insert((8, 11, SuperBool::NegativeOne), 15);
    m.insert((8, 12, SuperBool::One), 16);
    m.insert((8, 12, SuperBool::NegativeOne), 17);
    m.insert((8, 13, SuperBool::One), 18);
    m.insert((8, 13, SuperBool::NegativeOne), 19);
    m.insert((8, 14, SuperBool::One), 20);
    m.insert((8, 14, SuperBool::NegativeOne), 21);
    m.insert((8, 15, SuperBool::One), 22);
    m.insert((8, 15, SuperBool::NegativeOne), 23);
    m.insert((8, 16, SuperBool::One), 24);
    m.insert((8, 16, SuperBool::NegativeOne), 25);
    m.insert((8, 17, SuperBool::Zero), 26);
    m.insert((8, 18, SuperBool::One), 27);
    m.insert((8, 18, SuperBool::NegativeOne), 28);
    m.insert((8, 19, SuperBool::One), 29);
    m.insert((8, 19, SuperBool::NegativeOne), 30);
    m.insert((8, 20, SuperBool::Zero), 31);
    m.insert((8, 21, SuperBool::Zero), 32);
    m.insert((8, 22, SuperBool::One), 33);
    m.insert((8, 22, SuperBool::NegativeOne), 34);
    m.insert((8, 23, SuperBool::Zero), 35);
    m.insert((8, 24, SuperBool::Zero), 36);
    m.insert((8, 25, SuperBool::Zero), 37);
    m.insert((8, 26, SuperBool::Zero), 38);
    m.insert((8, 27, SuperBool::One), 39);
    m.insert((8, 27, SuperBool::NegativeOne), 40);
    m.insert((8, 28, SuperBool::Zero), 41);
    m.insert((8, 29, SuperBool::One), 42);
    m.insert((8, 29, SuperBool::NegativeOne), 43);
    m.insert((9, 1, SuperBool::Zero), 1);
    m.insert((9, 2, SuperBool::One), 2);
    m.insert((9, 2, SuperBool::NegativeOne), 3);
    m.insert((9, 3, SuperBool::One), 4);
    m.insert((9, 3, SuperBool::NegativeOne), 5);
    m.insert((9, 4, SuperBool::One), 6);
    m.insert((9, 4, SuperBool::NegativeOne), 7);
    m.insert((9, 5, SuperBool::One), 8);
    m.insert((9, 5, SuperBool::NegativeOne), 9);
    m.insert((9, 6, SuperBool::Zero), 10);
    m.insert((9, 7, SuperBool::One), 11);
    m.insert((9, 7, SuperBool::NegativeOne), 12);
    m.insert((9, 8, SuperBool::One), 13);
    m.insert((9, 8, SuperBool::NegativeOne), 14);
    m.insert((9, 9, SuperBool::Zero), 15);
    m.insert((9, 10, SuperBool::Zero), 16);
    m.insert((9, 11, SuperBool::One), 17);
    m.insert((9, 11, SuperBool::NegativeOne), 18);
    m.insert((9, 12, SuperBool::Zero), 19);
    m.insert((10, 1, SuperBool::Zero), 1);
    m.insert((10, 2, SuperBool::Zero), 2);
    m.insert((10, 3, SuperBool::Zero), 3);
    m.insert((10, 4, SuperBool::Zero), 4);
    m.insert((10, 5, SuperBool::Zero), 5);
    m.insert((10, 6, SuperBool::Zero), 6);
    m.insert((11, 1, SuperBool::Zero), 1);
    m.insert((12, 1, SuperBool::Zero), 1);
    m
});

static TN_INDEX_TO_CHORD_INFO: LazyLock<HashMap<U8U8SB, Option<Vec<&'static str>>>> = LazyLock::new(
    || {
        let mut m = HashMap::new();
        m.insert(
            (1, 1, SuperBool::Zero),
            Some(vec!["unison", "monad", "singleton"]),
        );
        m.insert(
            (2, 1, SuperBool::Zero),
            Some(vec![
                "interval class 1",
                "minor second",
                "m2",
                "half step",
                "semitone",
            ]),
        );
        m.insert(
            (2, 2, SuperBool::Zero),
            Some(vec![
                "interval class 2",
                "major second",
                "M2",
                "whole step",
                "whole tone",
            ]),
        );
        m.insert(
            (2, 3, SuperBool::Zero),
            Some(vec!["interval class 3", "minor third", "m3"]),
        );
        m.insert(
            (2, 4, SuperBool::Zero),
            Some(vec!["interval class 4", "major third", "M3"]),
        );
        m.insert(
            (2, 5, SuperBool::Zero),
            Some(vec!["interval class 5", "perfect fourth", "P4"]),
        );
        m.insert(
            (2, 6, SuperBool::Zero),
            Some(vec!["tritone", "diminished fifth", "augmented fourth"]),
        );
        m.insert((3, 1, SuperBool::Zero), Some(vec!["chromatic trimirror"]));
        m.insert((3, 2, SuperBool::One), Some(vec!["phrygian trichord"]));
        m.insert((3, 2, SuperBool::NegativeOne), Some(vec!["minor trichord"]));
        m.insert((3, 3, SuperBool::One), Some(vec!["major-minor trichord"]));
        m.insert(
            (3, 3, SuperBool::NegativeOne),
            Some(vec!["major-minor trichord"]),
        );
        m.insert(
            (3, 4, SuperBool::One),
            Some(vec!["incomplete major-seventh chord"]),
        );
        m.insert(
            (3, 4, SuperBool::NegativeOne),
            Some(vec!["incomplete major-seventh chord"]),
        );
        m.insert((3, 5, SuperBool::One), Some(vec!["tritone-fourth"]));
        m.insert((3, 5, SuperBool::NegativeOne), Some(vec!["tritone-fourth"]));
        m.insert((3, 6, SuperBool::Zero), Some(vec!["whole-tone trichord"]));
        m.insert(
            (3, 7, SuperBool::One),
            Some(vec!["incomplete minor-seventh chord"]),
        );
        m.insert(
            (3, 7, SuperBool::NegativeOne),
            Some(vec!["incomplete dominant-seventh chord"]),
        );
        m.insert(
            (3, 8, SuperBool::One),
            Some(vec![
                "incomplete dominant-seventh chord",
                "Italian augmented sixth chord",
            ]),
        );
        m.insert(
            (3, 8, SuperBool::NegativeOne),
            Some(vec!["incomplete half-diminished seventh chord"]),
        );
        m.insert((3, 9, SuperBool::Zero), Some(vec!["quartal trichord"]));
        m.insert((3, 10, SuperBool::Zero), Some(vec!["diminished triad"]));
        m.insert((3, 11, SuperBool::One), Some(vec!["minor triad"]));
        m.insert((3, 11, SuperBool::NegativeOne), Some(vec!["major triad"]));
        m.insert(
            (3, 12, SuperBool::Zero),
            Some(vec!["augmented triad", "equal 3-part octave division"]),
        );
        m.insert(
            (4, 1, SuperBool::Zero),
            Some(vec!["chromatic tetramirror", "BACH"]),
        );
        m.insert(
            (4, 2, SuperBool::One),
            Some(vec!["major-second tetracluster"]),
        );
        m.insert(
            (4, 2, SuperBool::NegativeOne),
            Some(vec!["major-second tetracluster"]),
        );
        m.insert(
            (4, 3, SuperBool::Zero),
            Some(vec!["alternating tetramirror"]),
        );
        m.insert(
            (4, 4, SuperBool::One),
            Some(vec!["minor third tetracluster"]),
        );
        m.insert(
            (4, 4, SuperBool::NegativeOne),
            Some(vec!["minor third tetracluster"]),
        );
        m.insert(
            (4, 5, SuperBool::One),
            Some(vec!["major third tetracluster"]),
        );
        m.insert(
            (4, 5, SuperBool::NegativeOne),
            Some(vec!["major third tetracluster"]),
        );
        m.insert(
            (4, 6, SuperBool::Zero),
            Some(vec!["perfect fourth tetramirror"]),
        );
        m.insert((4, 7, SuperBool::Zero), Some(vec!["Arabian tetramirror"]));
        m.insert(
            (4, 8, SuperBool::Zero),
            Some(vec!["double-fourth tetramirror"]),
        );
        m.insert(
            (4, 9, SuperBool::Zero),
            Some(vec!["double tritone tetramirror"]),
        );
        m.insert((4, 10, SuperBool::Zero), Some(vec!["minor tetramirror"]));
        m.insert((4, 11, SuperBool::One), Some(vec!["phrygian tetrachord"]));
        m.insert(
            (4, 11, SuperBool::NegativeOne),
            Some(vec!["lydian tetrachord", "major tetrachord"]),
        );
        m.insert(
            (4, 12, SuperBool::One),
            Some(vec!["harmonic minor tetrachord"]),
        );
        m.insert(
            (4, 12, SuperBool::NegativeOne),
            Some(vec!["major-third diminished tetrachord"]),
        );
        m.insert(
            (4, 13, SuperBool::One),
            Some(vec!["minor-second diminished tetrachord"]),
        );
        m.insert(
            (4, 13, SuperBool::NegativeOne),
            Some(vec!["perfect-fourth diminished tetrachord"]),
        );
        m.insert(
            (4, 14, SuperBool::One),
            Some(vec!["major-second minor tetrachord"]),
        );
        m.insert(
            (4, 14, SuperBool::NegativeOne),
            Some(vec!["perfect-fourth major tetrachord"]),
        );
        m.insert(
            (4, 15, SuperBool::One),
            Some(vec!["all-interval tetrachord"]),
        );
        m.insert(
            (4, 15, SuperBool::NegativeOne),
            Some(vec!["all-interval tetrachord"]),
        );
        m.insert(
            (4, 16, SuperBool::One),
            Some(vec!["minor-second quartal tetrachord"]),
        );
        m.insert(
            (4, 16, SuperBool::NegativeOne),
            Some(vec!["tritone quartal tetrachord"]),
        );
        m.insert(
            (4, 17, SuperBool::Zero),
            Some(vec!["major-minor tetramirror"]),
        );
        m.insert(
            (4, 18, SuperBool::One),
            Some(vec!["major-diminished tetrachord"]),
        );
        m.insert(
            (4, 18, SuperBool::NegativeOne),
            Some(vec!["minor-diminished tetrachord"]),
        );
        m.insert(
            (4, 19, SuperBool::One),
            Some(vec!["minor-augmented tetrachord"]),
        );
        m.insert(
            (4, 19, SuperBool::NegativeOne),
            Some(vec!["augmented major tetrachord"]),
        );
        m.insert((4, 20, SuperBool::Zero), Some(vec!["major seventh chord"]));
        m.insert(
            (4, 21, SuperBool::Zero),
            Some(vec!["whole-tone tetramirror"]),
        );
        m.insert(
            (4, 22, SuperBool::One),
            Some(vec!["major-second major tetrachord"]),
        );
        m.insert(
            (4, 22, SuperBool::NegativeOne),
            Some(vec!["perfect-fourth minor tetrachord"]),
        );
        m.insert((4, 23, SuperBool::Zero), Some(vec!["quartal tetramirror"]));
        m.insert(
            (4, 24, SuperBool::Zero),
            Some(vec!["augmented seventh chord"]),
        );
        m.insert(
            (4, 25, SuperBool::Zero),
            Some(vec![
                "Messiaen's truncated mode 6",
                "French augmented sixth chord",
            ]),
        );
        m.insert((4, 26, SuperBool::Zero), Some(vec!["minor seventh chord"]));
        m.insert(
            (4, 27, SuperBool::One),
            Some(vec!["half-diminished seventh chord"]),
        );
        m.insert(
            (4, 27, SuperBool::NegativeOne),
            Some(vec![
                "dominant seventh chord",
                "major minor seventh chord",
                "German augmented sixth chord",
                "Swiss augmented sixth chord",
            ]),
        );
        m.insert(
            (4, 28, SuperBool::Zero),
            Some(vec![
                "diminished seventh chord",
                "equal 4-part octave division",
            ]),
        );
        m.insert(
            (4, 29, SuperBool::One),
            Some(vec!["all-interval tetrachord"]),
        );
        m.insert(
            (4, 29, SuperBool::NegativeOne),
            Some(vec!["all-interval tetrachord"]),
        );
        m.insert((5, 1, SuperBool::Zero), Some(vec!["chromatic pentamirror"]));
        m.insert(
            (5, 2, SuperBool::One),
            Some(vec!["major-second pentacluster"]),
        );
        m.insert(
            (5, 2, SuperBool::NegativeOne),
            Some(vec!["major-second pentacluster"]),
        );
        m.insert(
            (5, 3, SuperBool::One),
            Some(vec!["minor-second major pentachord"]),
        );
        m.insert(
            (5, 3, SuperBool::NegativeOne),
            Some(vec!["Spanish pentacluster"]),
        );
        m.insert((5, 4, SuperBool::One), Some(vec!["blues pentacluster"]));
        m.insert(
            (5, 4, SuperBool::NegativeOne),
            Some(vec!["minor-third pentacluster"]),
        );
        m.insert(
            (5, 5, SuperBool::One),
            Some(vec!["major-third pentacluster"]),
        );
        m.insert(
            (5, 5, SuperBool::NegativeOne),
            Some(vec!["major-third pentacluster"]),
        );
        m.insert(
            (5, 6, SuperBool::One),
            Some(vec!["Asian pentacluster", "quasi raga Megharanji"]),
        );
        m.insert(
            (5, 6, SuperBool::NegativeOne),
            Some(vec!["Asian pentacluster"]),
        );
        m.insert(
            (5, 7, SuperBool::One),
            Some(vec!["double pentacluster", "quasi raga Nabhomani"]),
        );
        m.insert(
            (5, 7, SuperBool::NegativeOne),
            Some(vec!["double pentacluster"]),
        );
        m.insert(
            (5, 8, SuperBool::Zero),
            Some(vec!["tritone-symmetric pentamirror"]),
        );
        m.insert(
            (5, 9, SuperBool::One),
            Some(vec!["tritone-expanding pentachord"]),
        );
        m.insert(
            (5, 9, SuperBool::NegativeOne),
            Some(vec!["tritone-contracting pentachord"]),
        );
        m.insert(
            (5, 10, SuperBool::One),
            Some(vec!["alternating pentachord"]),
        );
        m.insert(
            (5, 10, SuperBool::NegativeOne),
            Some(vec!["alternating pentachord"]),
        );
        m.insert(
            (5, 11, SuperBool::One),
            Some(vec!["center-cluster pentachord"]),
        );
        m.insert(
            (5, 11, SuperBool::NegativeOne),
            Some(vec!["center-cluster pentachord"]),
        );
        m.insert((5, 12, SuperBool::Zero), Some(vec!["locrian pentachord"]));
        m.insert(
            (5, 13, SuperBool::One),
            Some(vec!["augmented pentacluster"]),
        );
        m.insert(
            (5, 13, SuperBool::NegativeOne),
            Some(vec!["augmented pentacluster"]),
        );
        m.insert(
            (5, 14, SuperBool::One),
            Some(vec!["double-seconds triple-fourth pentachord"]),
        );
        m.insert(
            (5, 14, SuperBool::NegativeOne),
            Some(vec!["double-seconds triple-fourth pentachord"]),
        );
        m.insert(
            (5, 15, SuperBool::Zero),
            Some(vec!["asymmetric pentamirror"]),
        );
        m.insert(
            (5, 16, SuperBool::One),
            Some(vec!["major-minor-diminished pentachord"]),
        );
        m.insert(
            (5, 16, SuperBool::NegativeOne),
            Some(vec!["major-minor diminished pentachord"]),
        );
        m.insert(
            (5, 17, SuperBool::Zero),
            Some(vec!["minor-major ninth chord"]),
        );
        m.insert(
            (5, 18, SuperBool::One),
            Some(vec!["Roma (Gypsy) pentachord"]),
        );
        m.insert(
            (5, 18, SuperBool::NegativeOne),
            Some(vec!["Roma (Gypsy) pentachord"]),
        );
        m.insert((5, 19, SuperBool::One), Some(vec!["Javanese pentachord"]));
        m.insert(
            (5, 19, SuperBool::NegativeOne),
            Some(vec!["Balinese pentachord"]),
        );
        m.insert(
            (5, 20, SuperBool::One),
            Some(vec![
                "Balinese Pelog pentatonic",
                "quasi raga Bhupala",
                "quasi raga Bibhas",
            ]),
        );
        m.insert(
            (5, 20, SuperBool::NegativeOne),
            Some(vec![
                "Hirajoshi pentatonic",
                "Iwato",
                "Sakura",
                "quasi raga Saveri",
            ]),
        );
        m.insert(
            (5, 21, SuperBool::One),
            Some(vec![
                "major-augmented ninth chord",
                "Syrian pentatonic",
                "quasi raga Megharanji",
            ]),
        );
        m.insert(
            (5, 21, SuperBool::NegativeOne),
            Some(vec!["Lebanese pentachord", "augmented-minor chord"]),
        );
        m.insert(
            (5, 22, SuperBool::Zero),
            Some(vec!["Persian pentamirror", "quasi raga Ramkali"]),
        );
        m.insert(
            (5, 23, SuperBool::One),
            Some(vec!["dorian pentachord", "minor pentachord"]),
        );
        m.insert(
            (5, 23, SuperBool::NegativeOne),
            Some(vec!["major pentachord"]),
        );
        m.insert((5, 24, SuperBool::One), Some(vec!["phrygian pentachord"]));
        m.insert(
            (5, 24, SuperBool::NegativeOne),
            Some(vec!["lydian pentachord"]),
        );
        m.insert(
            (5, 25, SuperBool::One),
            Some(vec!["diminished-major ninth chord"]),
        );
        m.insert(
            (5, 25, SuperBool::NegativeOne),
            Some(vec!["minor-diminished ninth chord"]),
        );
        m.insert(
            (5, 26, SuperBool::One),
            Some(vec!["diminished-augmented ninth chord"]),
        );
        m.insert(
            (5, 26, SuperBool::NegativeOne),
            Some(vec!["augmented-diminished ninth chord"]),
        );
        m.insert((5, 27, SuperBool::One), Some(vec!["major-ninth chord"]));
        m.insert(
            (5, 27, SuperBool::NegativeOne),
            Some(vec!["minor-ninth chord"]),
        );
        m.insert(
            (5, 28, SuperBool::One),
            Some(vec!["augmented-sixth pentachord"]),
        );
        m.insert(
            (5, 28, SuperBool::NegativeOne),
            Some(vec!["Javanese pentatonic", "augmented-sixth pentachord"]),
        );
        m.insert((5, 29, SuperBool::One), Some(vec!["Kumoi pentachord"]));
        m.insert(
            (5, 29, SuperBool::NegativeOne),
            Some(vec!["Kumoi pentachord"]),
        );
        m.insert((5, 30, SuperBool::One), Some(vec!["enigmatic pentachord"]));
        m.insert(
            (5, 30, SuperBool::NegativeOne),
            Some(vec!["enigmatic pentachord", "altered pentatonic"]),
        );
        m.insert(
            (5, 31, SuperBool::One),
            Some(vec!["diminished minor-ninth chord"]),
        );
        m.insert(
            (5, 31, SuperBool::NegativeOne),
            Some(vec!["flat-ninth pentachord", "quasi raga Ranjaniraga"]),
        );
        m.insert((5, 32, SuperBool::One), Some(vec!["Neapolitan pentachord"]));
        m.insert(
            (5, 32, SuperBool::NegativeOne),
            Some(vec!["Neapolitan pentachord"]),
        );
        m.insert(
            (5, 33, SuperBool::Zero),
            Some(vec!["whole-tone pentachord"]),
        );
        m.insert(
            (5, 34, SuperBool::Zero),
            Some(vec![
                "dominant-ninth",
                "major-minor",
                "Prometheus pentamirror",
                "dominant pentatonic",
            ]),
        );
        m.insert(
            (5, 35, SuperBool::Zero),
            Some(vec![
                "major pentatonic",
                "black-key scale",
                "blues pentatonic",
                "slendro",
                "quartal pentamirror",
            ]),
        );
        m.insert(
            (5, 36, SuperBool::One),
            Some(vec!["major-seventh pentacluster"]),
        );
        m.insert(
            (5, 36, SuperBool::NegativeOne),
            Some(vec!["minor-seventh pentacluster"]),
        );
        m.insert(
            (5, 37, SuperBool::Zero),
            Some(vec!["center-cluster pentamirror"]),
        );
        m.insert(
            (5, 38, SuperBool::One),
            Some(vec!["diminished pentacluster"]),
        );
        m.insert(
            (5, 38, SuperBool::NegativeOne),
            Some(vec!["diminished pentacluster"]),
        );
        m.insert(
            (6, 1, SuperBool::Zero),
            Some(vec![
                "A all combinatorial (P6, I11, RI5, RI11)",
                "chromatic hexamirror",
                "first-order all-combinatorial",
            ]),
        );
        m.insert((6, 2, SuperBool::One), Some(vec!["combinatorial I (I11)"]));
        m.insert(
            (6, 2, SuperBool::NegativeOne),
            Some(vec!["combinatorial I (I1)"]),
        );
        m.insert((6, 3, SuperBool::One), None);
        m.insert((6, 3, SuperBool::NegativeOne), None);
        m.insert(
            (6, 4, SuperBool::Zero),
            Some(vec!["combinatorial RI (RI6)"]),
        );
        m.insert((6, 5, SuperBool::One), Some(vec!["combinatorial I (I11)"]));
        m.insert(
            (6, 5, SuperBool::NegativeOne),
            Some(vec!["combinatorial I (I3)"]),
        );
        m.insert(
            (6, 6, SuperBool::Zero),
            Some(vec!["double cluster hexamirror"]),
        );
        m.insert(
            (6, 7, SuperBool::Zero),
            Some(vec![
                "B all combinatorial (P3, P9, I5, R6, R12, R8)",
                "Messiaen's mode 5",
                "second-order all combinatorial",
            ]),
        );
        m.insert(
            (6, 8, SuperBool::Zero),
            Some(vec!["D all combinatorial (P6, I1, RI7)"]),
        );
        m.insert((6, 9, SuperBool::One), Some(vec!["combinatorial I (I11)"]));
        m.insert(
            (6, 9, SuperBool::NegativeOne),
            Some(vec!["combinatorial I (I3)"]),
        );
        m.insert((6, 10, SuperBool::One), None);
        m.insert((6, 10, SuperBool::NegativeOne), None);
        m.insert((6, 11, SuperBool::One), None);
        m.insert((6, 11, SuperBool::NegativeOne), None);
        m.insert((6, 12, SuperBool::One), None);
        m.insert((6, 12, SuperBool::NegativeOne), None);
        m.insert(
            (6, 13, SuperBool::Zero),
            Some(vec!["alternating hexamirror", "combinatorial I (I7)"]),
        );
        m.insert((6, 14, SuperBool::One), Some(vec!["combinatorial P (P6)"]));
        m.insert(
            (6, 14, SuperBool::NegativeOne),
            Some(vec!["combinatorial P (P6)"]),
        );
        m.insert((6, 15, SuperBool::One), Some(vec!["combinatorial I (I11)"]));
        m.insert(
            (6, 15, SuperBool::NegativeOne),
            Some(vec!["combinatorial I (I5)"]),
        );
        m.insert((6, 16, SuperBool::One), Some(vec!["combinatorial I (I3)"]));
        m.insert(
            (6, 16, SuperBool::NegativeOne),
            Some(vec!["combinatorial I (I1)", "quasi raga Megha"]),
        );
        m.insert(
            (6, 17, SuperBool::One),
            Some(vec!["all tri-chord hexachord"]),
        );
        m.insert(
            (6, 17, SuperBool::NegativeOne),
            Some(vec!["all tri-chord hexachord (inverted form)"]),
        );
        m.insert((6, 18, SuperBool::One), Some(vec!["combinatorial I (I11)"]));
        m.insert(
            (6, 18, SuperBool::NegativeOne),
            Some(vec!["combinatorial I (I5)"]),
        );
        m.insert((6, 19, SuperBool::One), None);
        m.insert((6, 19, SuperBool::NegativeOne), None);
        m.insert(
            (6, 20, SuperBool::Zero),
            Some(vec![
                "E all combinatorial (P2, P6, P10, I3, I7, R4, R8, RI1, RI5, RI9)",
                "Messiaen's truncated mode 3",
                "Genus tertium",
                "third-order all combinatorial",
            ]),
        );
        m.insert((6, 21, SuperBool::One), Some(vec!["combinatorial I (I1)"]));
        m.insert(
            (6, 21, SuperBool::NegativeOne),
            Some(vec!["combinatorial I (I3)"]),
        );
        m.insert((6, 22, SuperBool::One), Some(vec!["combinatorial I (I11)"]));
        m.insert(
            (6, 22, SuperBool::NegativeOne),
            Some(vec!["combinatorial I (I5)"]),
        );
        m.insert(
            (6, 23, SuperBool::Zero),
            Some(vec!["combinatorial RI (RI8)", "super-locrian hexamirror"]),
        );
        m.insert((6, 24, SuperBool::One), None);
        m.insert(
            (6, 24, SuperBool::NegativeOne),
            Some(vec!["melodic-minor hexachord"]),
        );
        m.insert((6, 25, SuperBool::One), Some(vec!["locrian hexachord"]));
        m.insert(
            (6, 25, SuperBool::NegativeOne),
            Some(vec!["minor hexachord"]),
        );
        m.insert(
            (6, 26, SuperBool::Zero),
            Some(vec!["phrygian hexamirror", "combinatorial RI (RI8)"]),
        );
        m.insert((6, 27, SuperBool::One), Some(vec!["combinatorial I (I11)"]));
        m.insert(
            (6, 27, SuperBool::NegativeOne),
            Some(vec!["combinatorial I (I1)", "pyramid hexachord"]),
        );
        m.insert(
            (6, 28, SuperBool::Zero),
            Some(vec!["double-phrygian hexachord", "combinatorial RI (RI6)"]),
        );
        m.insert(
            (6, 29, SuperBool::Zero),
            Some(vec!["combinatorial RI (RI9)"]),
        );
        m.insert(
            (6, 30, SuperBool::One),
            Some(vec![
                "Messiaen's truncated mode 2",
                "minor-bitonal hexachord",
                "combinatorial R (R6)",
                "combinatorial I (I1, I7)",
            ]),
        );
        m.insert(
            (6, 30, SuperBool::NegativeOne),
            Some(vec![
                "Stravinsky's Petrushka-chord",
                "Messiaen's truncated mode 2",
                "major-bitonal hexachord",
                "combinatorial R (R6)",
                "combinatorial I (I1, I7)",
            ]),
        );
        m.insert((6, 31, SuperBool::One), Some(vec!["combinatorial I (I7)"]));
        m.insert(
            (6, 31, SuperBool::NegativeOne),
            Some(vec!["combinatorial I (I11)"]),
        );
        m.insert(
            (6, 32, SuperBool::Zero),
            Some(vec![
                "Guidonian hexachord",
                "C all combinatorial (P6, I3, RI9)",
                "major hexamirror",
                "quartal hexamirror",
                "first-order all combinatorial",
            ]),
        );
        m.insert(
            (6, 33, SuperBool::One),
            Some(vec!["dorian hexachord", "combinatorial I (I6)"]),
        );
        m.insert(
            (6, 33, SuperBool::NegativeOne),
            Some(vec![
                "dominant-eleventh",
                "lydian hexachord",
                "combinatorial I (I1)",
            ]),
        );
        m.insert(
            (6, 34, SuperBool::One),
            Some(vec![
                "Scriabin's Mystic-chord",
                "Prometheus hexachord",
                "combinatorial I (I11)",
            ]),
        );
        m.insert(
            (6, 34, SuperBool::NegativeOne),
            Some(vec![
                "augmented-eleventh",
                "harmonic hexachord",
                "combinatorial I (I7)",
            ]),
        );
        m.insert((6, 35, SuperBool::Zero), Some(vec!["whole tone scale", "6 equal part division", "F all-combinatorial (P1, P3, P5, P7, P9, P11, I1, I3, I5, I7, I9, I11, R2, R4, R6, R8, R10, RI2, RI4, RI6, RI8, RI10)", "Messiaen's mode 1", "sixth-order all combinatorial"]));
        m.insert((6, 36, SuperBool::One), None);
        m.insert((6, 36, SuperBool::NegativeOne), None);
        m.insert(
            (6, 37, SuperBool::Zero),
            Some(vec!["combinatorial RI (RI4)"]),
        );
        m.insert(
            (6, 38, SuperBool::Zero),
            Some(vec!["combinatorial RI (RI3)"]),
        );
        m.insert((6, 39, SuperBool::One), None);
        m.insert((6, 39, SuperBool::NegativeOne), None);
        m.insert((6, 40, SuperBool::One), None);
        m.insert((6, 40, SuperBool::NegativeOne), None);
        m.insert((6, 41, SuperBool::One), None);
        m.insert((6, 41, SuperBool::NegativeOne), None);
        m.insert(
            (6, 42, SuperBool::Zero),
            Some(vec!["combinatorial RI (RI3)"]),
        );
        m.insert(
            (6, 43, SuperBool::One),
            Some(vec!["complement of all tri-chord hexachord"]),
        );
        m.insert(
            (6, 43, SuperBool::NegativeOne),
            Some(vec![
                "complement of all-tri-chord hexachord (inverted form)",
            ]),
        );
        m.insert(
            (6, 44, SuperBool::One),
            Some(vec!["Schoenberg Anagram hexachord"]),
        );
        m.insert(
            (6, 44, SuperBool::NegativeOne),
            Some(vec!["quasi raga Bauli"]),
        );
        m.insert(
            (6, 45, SuperBool::Zero),
            Some(vec!["combinatorial RI (RI6)"]),
        );
        m.insert((6, 46, SuperBool::One), None);
        m.insert((6, 46, SuperBool::NegativeOne), None);
        m.insert((6, 47, SuperBool::One), None);
        m.insert((6, 47, SuperBool::NegativeOne), Some(vec!["blues scale"]));
        m.insert(
            (6, 48, SuperBool::Zero),
            Some(vec!["combinatorial RI (RI2)"]),
        );
        m.insert(
            (6, 49, SuperBool::Zero),
            Some(vec!["combinatorial RI (RI4)", "Prometheus Neapolitan mode"]),
        );
        m.insert(
            (6, 50, SuperBool::Zero),
            Some(vec!["combinatorial RI (RI1)"]),
        );
        m.insert((7, 1, SuperBool::Zero), Some(vec!["chromatic heptamirror"]));
        m.insert((7, 2, SuperBool::One), None);
        m.insert((7, 2, SuperBool::NegativeOne), None);
        m.insert((7, 3, SuperBool::One), None);
        m.insert((7, 3, SuperBool::NegativeOne), None);
        m.insert((7, 4, SuperBool::One), None);
        m.insert((7, 4, SuperBool::NegativeOne), None);
        m.insert((7, 5, SuperBool::One), None);
        m.insert((7, 5, SuperBool::NegativeOne), None);
        m.insert((7, 6, SuperBool::One), None);
        m.insert((7, 6, SuperBool::NegativeOne), None);
        m.insert((7, 7, SuperBool::One), None);
        m.insert((7, 7, SuperBool::NegativeOne), None);
        m.insert((7, 8, SuperBool::Zero), None);
        m.insert((7, 9, SuperBool::One), None);
        m.insert((7, 9, SuperBool::NegativeOne), None);
        m.insert((7, 10, SuperBool::One), None);
        m.insert((7, 10, SuperBool::NegativeOne), None);
        m.insert((7, 11, SuperBool::One), None);
        m.insert((7, 11, SuperBool::NegativeOne), None);
        m.insert((7, 12, SuperBool::Zero), None);
        m.insert((7, 13, SuperBool::One), None);
        m.insert((7, 13, SuperBool::NegativeOne), None);
        m.insert((7, 14, SuperBool::One), None);
        m.insert((7, 14, SuperBool::NegativeOne), None);
        m.insert((7, 15, SuperBool::Zero), None);
        m.insert((7, 16, SuperBool::One), Some(vec!["Debussy's heptatonic"]));
        m.insert((7, 16, SuperBool::NegativeOne), None);
        m.insert((7, 17, SuperBool::Zero), None);
        m.insert((7, 18, SuperBool::One), None);
        m.insert((7, 18, SuperBool::NegativeOne), None);
        m.insert((7, 19, SuperBool::One), None);
        m.insert((7, 19, SuperBool::NegativeOne), None);
        m.insert(
            (7, 20, SuperBool::One),
            Some(vec!["chromatic phrygian inverse"]),
        );
        m.insert(
            (7, 20, SuperBool::NegativeOne),
            Some(vec![
                "Greek chromatic",
                "chromatic mixolydian",
                "chromatic dorian",
                "quasi raga Pantuvarali",
                "mela Kanakangi",
            ]),
        );
        m.insert((7, 21, SuperBool::One), None);
        m.insert(
            (7, 21, SuperBool::NegativeOne),
            Some(vec!["Roma (Gypsy) hepatonic"]),
        );
        m.insert(
            (7, 22, SuperBool::Zero),
            Some(vec![
                "double harmonic scale",
                "major Roma (Gypsy)",
                "Hungarian minor",
                "double harmonic scale",
                "quasi raga Mayamdavagaula",
            ]),
        );
        m.insert((7, 23, SuperBool::One), None);
        m.insert(
            (7, 23, SuperBool::NegativeOne),
            Some(vec!["tritone major heptachord"]),
        );
        m.insert((7, 24, SuperBool::One), None);
        m.insert(
            (7, 24, SuperBool::NegativeOne),
            Some(vec!["mystic heptachord", "Enigmatic heptatonic"]),
        );
        m.insert((7, 25, SuperBool::One), None);
        m.insert((7, 25, SuperBool::NegativeOne), None);
        m.insert((7, 26, SuperBool::One), None);
        m.insert((7, 26, SuperBool::NegativeOne), None);
        m.insert((7, 27, SuperBool::One), None);
        m.insert(
            (7, 27, SuperBool::NegativeOne),
            Some(vec!["modified blues"]),
        );
        m.insert((7, 28, SuperBool::One), None);
        m.insert((7, 28, SuperBool::NegativeOne), None);
        m.insert((7, 29, SuperBool::One), None);
        m.insert((7, 29, SuperBool::NegativeOne), None);
        m.insert((7, 30, SuperBool::One), Some(vec!["Neapolitan-minor mode"]));
        m.insert((7, 30, SuperBool::NegativeOne), None);
        m.insert(
            (7, 31, SuperBool::One),
            Some(vec!["alternating heptachord", "Hungarian major mode"]),
        );
        m.insert(
            (7, 31, SuperBool::NegativeOne),
            Some(vec!["diminished scale", "alternating heptachord"]),
        );
        m.insert(
            (7, 32, SuperBool::One),
            Some(vec![
                "harmonic minor scale",
                "harmonic minor collection",
                "Spanish Roma (Gypsy)",
                "mela Kiravani",
            ]),
        );
        m.insert(
            (7, 32, SuperBool::NegativeOne),
            Some(vec![
                "harmonic major scale (inverted)",
                "harmonic minor collection (inverted)",
                "mela Cakravana",
                "quasi raga Ahir Bhairav",
            ]),
        );
        m.insert(
            (7, 33, SuperBool::Zero),
            Some(vec!["Neapolitan-major mode", "leading-whole-tone mode"]),
        );
        m.insert(
            (7, 34, SuperBool::Zero),
            Some(vec![
                "melodic minor ascending scale",
                "jazz minor",
                "augmented thirteenth heptamirror",
                "harmonic/super-locrian",
            ]),
        );
        m.insert(
            (7, 35, SuperBool::Zero),
            Some(vec![
                "major scale",
                "major diatonic heptachord",
                "natural minor scale",
                "dominant thirteenth",
                "locrian",
                "phrygian",
                "major inverse",
            ]),
        );
        m.insert((7, 36, SuperBool::One), None);
        m.insert((7, 36, SuperBool::NegativeOne), None);
        m.insert((7, 37, SuperBool::Zero), None);
        m.insert((7, 38, SuperBool::One), None);
        m.insert((7, 38, SuperBool::NegativeOne), None);
        m.insert((8, 1, SuperBool::Zero), Some(vec!["chromatic octamirror"]));
        m.insert((8, 2, SuperBool::One), None);
        m.insert((8, 2, SuperBool::NegativeOne), None);
        m.insert((8, 3, SuperBool::Zero), None);
        m.insert((8, 4, SuperBool::One), None);
        m.insert((8, 4, SuperBool::NegativeOne), None);
        m.insert((8, 5, SuperBool::One), None);
        m.insert((8, 5, SuperBool::NegativeOne), None);
        m.insert((8, 6, SuperBool::Zero), None);
        m.insert((8, 7, SuperBool::Zero), None);
        m.insert((8, 8, SuperBool::Zero), None);
        m.insert((8, 9, SuperBool::Zero), Some(vec!["Messiaen's mode 4"]));
        m.insert((8, 10, SuperBool::Zero), None);
        m.insert((8, 11, SuperBool::One), None);
        m.insert(
            (8, 11, SuperBool::NegativeOne),
            Some(vec!["blues octatonic"]),
        );
        m.insert((8, 12, SuperBool::One), None);
        m.insert((8, 12, SuperBool::NegativeOne), None);
        m.insert((8, 13, SuperBool::One), Some(vec!["blues octatonic"]));
        m.insert((8, 13, SuperBool::NegativeOne), None);
        m.insert((8, 14, SuperBool::One), None);
        m.insert((8, 14, SuperBool::NegativeOne), None);
        m.insert((8, 15, SuperBool::One), None);
        m.insert((8, 15, SuperBool::NegativeOne), None);
        m.insert((8, 16, SuperBool::One), None);
        m.insert(
            (8, 16, SuperBool::NegativeOne),
            Some(vec!["enigmatic octachord"]),
        );
        m.insert((8, 17, SuperBool::Zero), None);
        m.insert((8, 18, SuperBool::One), None);
        m.insert((8, 18, SuperBool::NegativeOne), None);
        m.insert((8, 19, SuperBool::One), None);
        m.insert((8, 19, SuperBool::NegativeOne), None);
        m.insert((8, 20, SuperBool::Zero), None);
        m.insert((8, 21, SuperBool::Zero), None);
        m.insert((8, 22, SuperBool::One), None);
        m.insert(
            (8, 22, SuperBool::NegativeOne),
            Some(vec!["Spanish octatonic scale"]),
        );
        m.insert(
            (8, 23, SuperBool::Zero),
            Some(vec!["Greek", "quartal octachord", "diatonic octad"]),
        );
        m.insert((8, 24, SuperBool::Zero), None);
        m.insert((8, 25, SuperBool::Zero), Some(vec!["Messiaen's mode 6"]));
        m.insert(
            (8, 26, SuperBool::Zero),
            Some(vec!["Spanish phrygian", "blues"]),
        );
        m.insert((8, 27, SuperBool::One), None);
        m.insert((8, 27, SuperBool::NegativeOne), None);
        m.insert(
            (8, 28, SuperBool::Zero),
            Some(vec![
                "octatonic scale",
                "Messiaen's mode 2",
                "alternating octatonic scale",
                "diminished scale",
            ]),
        );
        m.insert((8, 29, SuperBool::One), None);
        m.insert((8, 29, SuperBool::NegativeOne), None);
        m.insert((9, 1, SuperBool::Zero), Some(vec!["chromatic nonamirror"]));
        m.insert((9, 2, SuperBool::One), None);
        m.insert((9, 2, SuperBool::NegativeOne), None);
        m.insert((9, 3, SuperBool::One), None);
        m.insert((9, 3, SuperBool::NegativeOne), None);
        m.insert((9, 4, SuperBool::One), None);
        m.insert((9, 4, SuperBool::NegativeOne), None);
        m.insert((9, 5, SuperBool::One), None);
        m.insert((9, 5, SuperBool::NegativeOne), None);
        m.insert((9, 6, SuperBool::Zero), None);
        m.insert((9, 7, SuperBool::One), Some(vec!["nonatonic blues"]));
        m.insert((9, 7, SuperBool::NegativeOne), None);
        m.insert((9, 8, SuperBool::One), None);
        m.insert((9, 8, SuperBool::NegativeOne), None);
        m.insert((9, 9, SuperBool::Zero), None);
        m.insert((9, 10, SuperBool::Zero), None);
        m.insert((9, 11, SuperBool::One), None);
        m.insert(
            (9, 11, SuperBool::NegativeOne),
            Some(vec!["diminishing nonachord"]),
        );
        m.insert(
            (9, 12, SuperBool::Zero),
            Some(vec!["Messiaen's mode 3", "Tsjerepnin"]),
        );
        m.insert((10, 1, SuperBool::Zero), Some(vec!["chromatic decamirror"]));
        m.insert((10, 2, SuperBool::Zero), None);
        m.insert((10, 3, SuperBool::Zero), None);
        m.insert((10, 4, SuperBool::Zero), None);
        m.insert((10, 5, SuperBool::Zero), Some(vec!["major-minor mixed"]));
        m.insert((10, 6, SuperBool::Zero), Some(vec!["Messiaen's mode 7"]));
        m.insert(
            (11, 1, SuperBool::Zero),
            Some(vec!["chromatic undecamirror"]),
        );
        m.insert(
            (12, 1, SuperBool::Zero),
            Some(vec![
                "aggregate",
                "dodecachord",
                "twelve-tone chromatic",
                "chromatic scale",
                "dodecamirror",
            ]),
        );
        m
    },
);

static MAXIMUM_INDEX_NUMBER_WITHOUT_INVERSION_EQUIVALENCE: LazyLock<Vec<u8>> =
    LazyLock::new(|| vec![1, 1, 6, 19, 43, 66, 80, 66, 43, 19, 6, 1, 1]);

static MAXIMUM_INDEX_NUMBER_WITH_INVERSION_EQUIVALENCE: LazyLock<Vec<u8>> =
    LazyLock::new(|| vec![1, 1, 6, 12, 29, 38, 50, 38, 29, 12, 6, 1, 1]);

// END_GENERATED_CODE

/*
# to access the data for a single form, use:
# forte   [size(tetra)] = 4
#         [number(forte)] = 3
#         [data(0=pitches, 1=ICV, 2=invariance vector (morris), 3 = Z-relation)]
#         [element in list]
# ------------------------------------------------------------------------------
def _makeCardinalityToChordMembers():
    _cardinalityToChordMembers = {}

    for cardinality in range(1, 13):
        forte_cardinality_lookup = FORTE[cardinality]
        this_cardinality_entries = {}
        for forte_after_dash in range(1, len(forte_cardinality_lookup)):
            forte_entry = forte_cardinality_lookup[forte_after_dash]
            has_distinct_inversion = (forte_entry[2][1] == 0)
            inversion_number = 1 if has_distinct_inversion else 0
            key = (forte_after_dash, inversion_number)
            pitches = forte_entry[0]
            morris_invariance = forte_entry[2]
            icv = forte_entry[1]
            value = (pitches, morris_invariance, icv)
            this_cardinality_entries[key] = value
            if not has_distinct_inversion:
                continue
            # now do the inversion of the pitch
            key = (forte_after_dash, -1)
            inversion_pitches = inversionDefaultPitchClasses[
                (cardinality, forte_after_dash)
            ]
            value = (inversion_pitches, morris_invariance, icv)
            this_cardinality_entries[key] = value

        _cardinalityToChordMembers[cardinality] = this_cardinality_entries
    return _cardinalityToChordMembers


cardinalityToChordMembers = _makeCardinalityToChordMembers()
del _makeCardinalityToChordMembers
 */

// macro_rules! generate_cardinality_to_chord_members {
//     () => {{
//         use std::collections::HashMap;

//         let mut cardinality_to_chord_members = HashMap::new();

//         for cardinality in 1..=12 {
//             let mut entries = HashMap::new();
//             let forte_entries = &FORTE[cardinality as usize];

//             for forte_after_dash in 1..forte_entries.len() {
//                 let Some(tni) = &forte_entries[forte_after_dash] else {
//                     continue;
//                 };

//                 let has_distinct = tni.interval_class_vector[1] == 0;
//                 let inv_num = if has_distinct { 1 } else { 0 };

//                 entries.insert(
//                     (forte_after_dash as u8, inv_num),
//                     (
//                         tni.pitch_classes.clone(),
//                         tni.invariance_vector.clone(),
//                         tni.interval_class_vector.clone(),
//                     ),
//                 );

//                 if has_distinct {
//                     let inv_pitches = INVERSION_DEFAULT_PITCH_CLASSES
//                         .get(&(cardinality as u8, forte_after_dash as u8))
//                         .unwrap()
//                         .clone();

//                     entries.insert(
//                         (forte_after_dash as u8, -1),
//                         (
//                             inv_pitches,
//                             tni.invariance_vector.clone(),
//                             tni.interval_class_vector.clone(),
//                         ),
//                     );
//                 }
//             }

//             cardinality_to_chord_members.insert(cardinality as u8, entries);
//         }

//         cardinality_to_chord_members
//     }};
// }

static CARDINALITY_TO_CHORD_MEMBERS: LazyLock<HashMap<u8, HashMap<U8SB, Pcivicv>>> =
    LazyLock::new(|| {
        use std::collections::HashMap;
        let mut cardinality_to_chord_members = HashMap::new();
        for cardinality in 1..=12 {
            let mut entries = HashMap::new();
            let forte_entries = &FORTE[cardinality as usize];
            for (forte_after_dash, _) in forte_entries.iter().enumerate().skip(1) {
                let Some(tni) = &forte_entries[forte_after_dash] else {
                    continue;
                };
                let has_distinct = tni.interval_class_vector[1] == 0;
                let inv_num = if has_distinct {
                    SuperBool::One
                } else {
                    SuperBool::Zero
                };
                entries.insert(
                    (forte_after_dash as u8, inv_num),
                    (
                        tni.pitch_classes.clone(),
                        tni.invariance_vector,
                        tni.interval_class_vector,
                    ),
                );
                if has_distinct {
                    let inv_pitches = INVERSION_DEFAULT_PITCH_CLASSES
                        .get(&(cardinality as u8, forte_after_dash as u8))
                        .unwrap()
                        .clone();
                    entries.insert(
                        (forte_after_dash as u8, SuperBool::NegativeOne),
                        (
                            inv_pitches,
                            tni.invariance_vector,
                            tni.interval_class_vector,
                        ),
                    );
                }
            }
            cardinality_to_chord_members.insert(cardinality as u8, entries);
        }
        cardinality_to_chord_members
    });

fn forte_index_to_inversions_available(card: u8, index: u8) -> Result<Vec<i8>, Exception> {
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
        if entry.invariance_vector[1] > 0 {
            inversions.push(0);
        } else {
            inversions.push(-1);
            inversions.push(1);
        }
    }
    Ok(inversions)
}
