mod generated;

use crate::exception::Exception;

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
#[derive(Eq, Hash, PartialEq, Debug)]
enum Sign {
    NegativeOne = -1,
    Zero = 0,
    One = 1,
}

impl Sign {
    fn from_i8(i: i8) -> Option<Self> {
        match i {
            0 => Some(Sign::Zero),
            1 => Some(Sign::One),
            -1 => Some(Sign::NegativeOne),
            _ => None,
        }
    }
}

type U8SB = (u8, Sign);
type U8U8SB = (u8, u8, Sign);

const CARDINALITIES: usize = 13;

type Forte = LazyLock<[Vec<Option<TNIStructure>>; CARDINALITIES]>;
type InversionDefaultPitchClasses = LazyLock<HashMap<(u8, u8), PitchClasses>>;
type CardinalityToChordMembers = LazyLock<[HashMap<U8SB, Pcivicv>; CARDINALITIES]>;
type ForteNumberWithInversionToIndex = LazyLock<HashMap<U8U8SB, u8>>;
type TnIndexToChordInfo = LazyLock<HashMap<U8U8SB, Option<Vec<&'static str>>>>;
type MaximumIndexNumberWithoutInversionEquivalence = LazyLock<Vec<u8>>;
type MaximumIndexNumberWithInversionEquivalence = LazyLock<Vec<u8>>;

static CARDINALITY_TO_CHORD_MEMBERS: CardinalityToChordMembers = LazyLock::new(|| {
    use std::collections::HashMap;
    let mut cardinality_to_chord_members: [HashMap<U8SB, Pcivicv>; 13] = Default::default();
    cardinality_to_chord_members[0] = HashMap::new();
    for cardinality in 1..=12 {
        let mut entries: HashMap<U8SB, Pcivicv> = HashMap::new();
        let forte_entries = &generated::FORTE[cardinality as usize];
        for (forte_after_dash, _) in forte_entries.iter().enumerate().skip(1) {
            let Some(tni) = &forte_entries[forte_after_dash] else {
                continue;
            };

            let has_distinct_inversion = tni.invariance_vector()[1] == 0;

            let inv_num = if has_distinct_inversion {
                Sign::One
            } else {
                Sign::Zero
            };

            let key = (forte_after_dash as u8, inv_num);
            let value = (
                tni.pitch_classes(),
                tni.invariance_vector(),
                tni.interval_class_vector(),
            );

            if key == (1, Sign::Zero) {
                println!("{:?}", value);
            }

            entries.insert(key, value);

            if has_distinct_inversion {
                let inv_pitches = match INVERSION_DEFAULT_PITCH_CLASSES
                    .get(&(cardinality as u8, forte_after_dash as u8))
                {
                    Some(pitches) => *pitches,
                    None => continue,
                };
                entries.insert(
                    (forte_after_dash as u8, Sign::NegativeOne),
                    (
                        inv_pitches,
                        tni.invariance_vector(),
                        tni.interval_class_vector(),
                    ),
                );
            }
        }
        cardinality_to_chord_members[cardinality as usize] = entries;
    }
    cardinality_to_chord_members
});

fn forte_index_to_inversions_available(card: usize, index: u8) -> Result<Vec<Sign>, Exception> {
    if !(1..=13).contains(&card) {
        return Err(Exception::ChordTables(format!(
            "cardinality {} not valid",
            card
        )));
    }
    if index < 1 || index > MAXIMUM_INDEX_NUMBER_WITHOUT_INVERSION_EQUIVALENCE[card] {
        return Err(Exception::ChordTables(format!(
            "index {} not valid for cardinality {}",
            index, card
        )));
    }

    let mut inversions = vec![];
    if let Some(entry) = &(&FORTE[card])[index as usize] {
        // second value stored inversion status
        if entry.invariance_vector()[1] > 0 {
            inversions.push(Sign::Zero);
        } else {
            inversions.push(Sign::NegativeOne);
            inversions.push(Sign::One);
        }
    }
    Ok(inversions)
}

fn _validate_address() {
    //todo
}

// include!("./generated.rs");

#[cfg(test)]
mod tests {
    use super::{Pcivicv, TNIStructure, U8SB};

    use crate::chord::tables::{
        Sign, CARDINALITY_TO_CHORD_MEMBERS, CARDINALITY_TO_CHORD_MEMBERS_GENERATED, FORTE,
    };

    use pyo3::{
        types::{PyAnyMethods, PyDict, PyDictMethods, PyTuple},
        Bound, PyResult, Python,
    };

    use std::collections::hash_map::Keys;

    use utils::{init_py, prepare, Tables};

    #[test]
    fn cardinality_to_chord_members_equality() {
        (0..13).for_each(|i| {
            check_equality(i, CARDINALITY_TO_CHORD_MEMBERS[i].keys());
            check_equality(i, CARDINALITY_TO_CHORD_MEMBERS_GENERATED[i].keys());

            println!("{} passed", i);
        });
    }

    fn check_equality(i: usize, keys: Keys<'_, U8SB, Pcivicv>) {
        keys.for_each(|key| {
            assert_eq!(
                format!("{:?}", CARDINALITY_TO_CHORD_MEMBERS[i].get(key)),
                format!("{:?}", CARDINALITY_TO_CHORD_MEMBERS_GENERATED[i].get(key))
            );
        });
    }

    fn match_python(tuple: &Pcivicv) -> String {
        let true_indices: Vec<String> = tuple
            .0
            .iter()
            .enumerate()
            .filter(|&(_, &b)| b)
            .map(|(i, _)| i.to_string())
            .collect();

        let first = if true_indices.len() == 1 {
            format!("{},", true_indices.join(""))
        } else {
            true_indices.join(", ")
        };

        let second = tuple
            .1
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        let third = tuple
            .2
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        format!("(({}), ({}), ({}))", first, second, third)
    }

    #[test]
    fn python_cardinality_to_chord_members_equality_test() {
        prepare().unwrap();

        Python::with_gil(|py| -> PyResult<()> {
            init_py(py)?;

            let tables: Tables = py.import("music21.chord.tables")?;

            let cardinality_to_chord_members = tables.getattr("cardinalityToChordMembers")?;
            let cardinality_to_chord_members: &Bound<'_, PyDict> =
                cardinality_to_chord_members.downcast_exact()?;

            cardinality_to_chord_members.keys().into_iter().for_each(
                |outer_key: Bound<'_, pyo3::PyAny>| {
                    let outer_key_rust: usize = outer_key.extract().unwrap();
                    println!("outer_key: {:?}", outer_key);
                    let inner_dict: Bound<'_, pyo3::PyAny> = cardinality_to_chord_members
                        .get_item(outer_key)
                        .unwrap()
                        .unwrap();
                    let inner_dict: &Bound<'_, PyDict> = inner_dict.downcast_exact().unwrap();

                    inner_dict.keys().into_iter().for_each(|inner_key| {
                        let inner_key: &Bound<'_, PyTuple> = inner_key.downcast_exact().unwrap();

                        let (first, second): (u8, i8) = inner_key.extract().unwrap();
                        let key: U8SB = (first, Sign::from_i8(second).unwrap());

                        println!("python key: {:?}", inner_key);
                        println!("rust key: {:?}", key);
                        assert_eq!(
                            format!("{:?}", inner_dict.get_item(inner_key).unwrap().unwrap()),
                            match_python(
                                CARDINALITY_TO_CHORD_MEMBERS_GENERATED[outer_key_rust]
                                    .get(&key)
                                    .unwrap()
                            )
                        );
                        println!("{:?} passed", &key);
                    });
                    println!("{} passed", outer_key_rust);
                },
            );

            Ok(())
        })
        .unwrap();
    }

    fn match_python2(v: &[Option<TNIStructure>]) -> String {
        let elems: Vec<String> = v
            .iter()
            .map(|opt| {
                match opt {
                    None => "None".to_string(),
                    Some(t) => {
                        // Collect indices where bool is true.
                        let true_indices: Vec<String> =
                            t.0.iter()
                                .enumerate()
                                .filter(|&(_, &b)| b)
                                .map(|(i, _)| i.to_string())
                                .collect();
                        let first = if true_indices.len() == 1 {
                            format!("{},", true_indices.join(""))
                        } else {
                            true_indices.join(", ")
                        };
                        let second =
                            t.1.iter()
                                .map(|i| i.to_string())
                                .collect::<Vec<_>>()
                                .join(", ");
                        let third =
                            t.2.iter()
                                .map(|i| i.to_string())
                                .collect::<Vec<_>>()
                                .join(", ");
                        let fourth = t.3.to_string();
                        format!("(({}), ({}), ({}), {})", first, second, third, fourth)
                    }
                }
            })
            .collect();
        format!("({})", elems.join(", "))
    }

    #[test]
    fn test() {
        for i in 1..12 {
            println!("{}", i);
        }
    }
    #[test]
    fn python_forte_equality_test() {
        prepare().unwrap();

        Python::with_gil(|py| -> PyResult<()> {
            init_py(py)?;

            let operator = py.import("operator")?;

            let tables: Tables = py.import("music21.chord.tables")?;

            let forte = tables.getattr("FORTE")?;
            let forte: &Bound<'_, PyTuple> = forte.downcast_exact()?;

            for i in 0..13 {
                let item = operator.call_method1("getitem", (forte, i))?;

                let tuple: Result<&Bound<'_, PyTuple>, pyo3::DowncastError<'_, '_>> =
                    item.downcast_exact();

                match tuple {
                    Ok(t) => {
                        assert_eq!(format!("{:?}", t), format!("{}", match_python2(&FORTE[i])));
                        println!("{:?}", t);
                        println!("{}", match_python2(&FORTE[i]));
                    }
                    Err(_) => {
                        assert!(FORTE[i].is_empty());
                        continue;
                    }
                }
            }

            Ok(())
        })
        .unwrap()
    }
}
