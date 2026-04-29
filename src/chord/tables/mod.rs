mod generated;

use crate::exception::Exception;

use generated::*;
use std::{collections::HashMap, sync::LazyLock};

pub(crate) type ChordTableAddress = (u8, u8, i8, Option<u8>);

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
#[derive(Copy, Clone, Eq, Hash, PartialEq, Debug)]
enum Sign {
    NegativeOne = -1,
    Zero = 0,
    One = 1,
}

impl Sign {
    pub(crate) fn from_i8(i: i8) -> Option<Self> {
        match i {
            0 => Some(Sign::Zero),
            1 => Some(Sign::One),
            -1 => Some(Sign::NegativeOne),
            _ => None,
        }
    }

    fn as_i8(&self) -> i8 {
        *self as i8
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

#[derive(Debug, Clone)]
pub(crate) struct KnownChordTableEntry {
    pub(crate) cardinality: u8,
    pub(crate) common_names: Vec<&'static str>,
    pub(crate) forte_class: String,
    pub(crate) normal_form: Vec<u8>,
    pub(crate) interval_class_vector: Vec<u8>,
}

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
            "cardinality {card} not valid"
        )));
    }
    if index < 1 || index > MAXIMUM_INDEX_NUMBER_WITHOUT_INVERSION_EQUIVALENCE[card] {
        return Err(Exception::ChordTables(format!(
            "index {index} not valid for cardinality {card}"
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

fn validate_address(address: (u8, u8, Option<i8>)) -> Result<(u8, u8, Sign), Exception> {
    let card = address.0;
    let index = address.1;
    let inversion = address.2.and_then(Sign::from_i8);

    if !(1..=12).contains(&card) {
        return Err(Exception::ChordTables(format!(
            "cardinality {card} not valid"
        )));
    }

    if index < 1 || index > MAXIMUM_INDEX_NUMBER_WITHOUT_INVERSION_EQUIVALENCE[card as usize] {
        return Err(Exception::ChordTables(format!("index {index} not valid")));
    }

    let inversions_available = forte_index_to_inversions_available(card as usize, index)?;

    let resolved_inversion = if let Some(inv) = inversion {
        if inversions_available.contains(&inv) {
            inv
        } else {
            return Err(Exception::ChordTables(format!(
                "inversion {} not valid",
                inv.as_i8()
            )));
        }
    } else if inversions_available.contains(&Sign::Zero) {
        Sign::Zero
    } else {
        Sign::One
    };

    Ok((card, index, resolved_inversion))
}

fn bool_vec_to_pitch_classes(v: &[bool]) -> Vec<u8> {
    v.iter()
        .enumerate()
        .filter_map(
            |(idx, present)| {
                if *present { Some(idx as u8) } else { None }
            },
        )
        .collect()
}

fn pitch_classes_to_bools(pcs: &[u8]) -> [bool; 12] {
    let mut out = [false; 12];
    for pc in pcs {
        out[*pc as usize % 12] = true;
    }
    out
}

pub(crate) fn seek_chord_tables_address(
    ordered_pitch_classes: &[u8],
) -> Result<ChordTableAddress, Exception> {
    if ordered_pitch_classes.is_empty() {
        return Err(Exception::ChordTables(
            "cannot access chord tables address for Chord with 0 pitches".to_string(),
        ));
    }

    let card = ordered_pitch_classes.len() as u8;
    if card == 1 {
        return Ok((1, 1, 0, Some(ordered_pitch_classes[0] % 12)));
    }
    if card == 12 {
        return Ok((12, 1, 0, Some(0)));
    }

    let mut candidates: Vec<([bool; 12], [bool; 12], u8)> = Vec::new();
    for rot in 0..ordered_pitch_classes.len() {
        let mut test_set: Vec<u8> = ordered_pitch_classes[rot..].to_vec();
        test_set.extend_from_slice(&ordered_pitch_classes[..rot]);

        let test_set_original_pc = test_set[0] % 12;
        let test_set_transposed: Vec<u8> = test_set
            .iter()
            .map(|x| ((*x as i32 - test_set_original_pc as i32).rem_euclid(12)) as u8)
            .collect();

        let mut test_set_invert: Vec<u8> =
            test_set_transposed.iter().map(|x| (12 - *x) % 12).collect();
        test_set_invert.reverse();
        let shift = (12 - test_set_invert[0]) % 12;
        test_set_invert = test_set_invert.iter().map(|x| (x + shift) % 12).collect();

        candidates.push((
            pitch_classes_to_bools(&test_set_transposed),
            pitch_classes_to_bools(&test_set_invert),
            test_set_original_pc,
        ));
    }

    for index_candidate in 1..FORTE[card as usize].len() {
        let Some(data_line) = &FORTE[card as usize][index_candidate] else {
            continue;
        };
        let data_line_pcs = data_line.pitch_classes();
        let inversions_available =
            forte_index_to_inversions_available(card as usize, index_candidate as u8)?;

        for (candidate, candidate_inversion, candidate_original_pc) in &candidates {
            if data_line_pcs == *candidate {
                let inversion = if inversions_available.contains(&Sign::Zero) {
                    0
                } else {
                    1
                };
                return Ok((
                    card,
                    index_candidate as u8,
                    inversion,
                    Some(*candidate_original_pc),
                ));
            }
            if data_line_pcs == *candidate_inversion {
                let inversion = if inversions_available.contains(&Sign::Zero) {
                    0
                } else {
                    -1
                };
                return Ok((
                    card,
                    index_candidate as u8,
                    inversion,
                    Some(*candidate_original_pc),
                ));
            }
        }
    }

    Err(Exception::ChordTables(format!(
        "cannot find a chord table address for {ordered_pitch_classes:?}"
    )))
}

pub(crate) fn address_to_common_names(
    address: ChordTableAddress,
) -> Result<Option<Vec<&'static str>>, Exception> {
    let (card, index, inversion) = validate_address((address.0, address.1, Some(address.2)))?;
    Ok(TN_INDEX_TO_CHORD_INFO
        .get(&(card, index, inversion))
        .cloned()
        .flatten())
}

pub(crate) fn address_to_forte_name(
    address: ChordTableAddress,
    classification: &str,
) -> Result<String, Exception> {
    let (card, index, inversion) = validate_address((address.0, address.1, Some(address.2)))?;
    let inversion_suffix = match classification.to_ascii_lowercase().as_str() {
        "tn" => match inversion {
            Sign::NegativeOne => "B",
            Sign::One => "A",
            Sign::Zero => "",
        },
        _ => "",
    };
    Ok(format!("{card}-{index}{inversion_suffix}"))
}

pub(crate) fn transposed_normal_form_from_address(
    address: ChordTableAddress,
) -> Result<Vec<u8>, Exception> {
    let (card, index, inversion) = validate_address((address.0, address.1, Some(address.2)))?;
    let entry = CARDINALITY_TO_CHORD_MEMBERS
        .get(card as usize)
        .and_then(|bucket| bucket.get(&(index, inversion)))
        .ok_or_else(|| {
            Exception::ChordTables(format!(
                "cannot resolve normal form for address ({card}, {index}, {})",
                inversion.as_i8()
            ))
        })?;
    Ok(bool_vec_to_pitch_classes(&entry.0))
}

pub(crate) fn interval_class_vector_from_address(
    address: ChordTableAddress,
) -> Result<Vec<u8>, Exception> {
    let (card, index, inversion) = validate_address((address.0, address.1, Some(address.2)))?;
    let entry = CARDINALITY_TO_CHORD_MEMBERS
        .get(card as usize)
        .and_then(|bucket| bucket.get(&(index, inversion)))
        .ok_or_else(|| {
            Exception::ChordTables(format!(
                "cannot resolve interval class vector for address ({card}, {index}, {})",
                inversion.as_i8()
            ))
        })?;
    Ok(entry.2.to_vec())
}

pub(crate) fn known_chord_table_entries() -> Vec<KnownChordTableEntry> {
    let mut entries = TN_INDEX_TO_CHORD_INFO
        .iter()
        .filter_map(|(&(cardinality, index, inversion), common_names)| {
            let common_names = common_names.clone().unwrap_or_default();
            let address = (cardinality, index, inversion.as_i8(), None);
            Some((
                (cardinality, index, inversion.as_i8()),
                KnownChordTableEntry {
                    cardinality,
                    common_names,
                    forte_class: address_to_forte_name(address, "tn").ok()?,
                    normal_form: transposed_normal_form_from_address(address).ok()?,
                    interval_class_vector: interval_class_vector_from_address(address).ok()?,
                },
            ))
        })
        .collect::<Vec<_>>();

    entries.sort_by_key(|(sort_key, _)| *sort_key);
    entries.into_iter().map(|(_, entry)| entry).collect()
}

// include!("./generated.rs");

#[cfg(test)]
mod tests {
    use super::{Pcivicv, U8SB};
    use crate::chord::tables::{
        CARDINALITY_TO_CHORD_MEMBERS, CARDINALITY_TO_CHORD_MEMBERS_GENERATED,
    };
    use std::collections::hash_map::Keys;

    #[cfg(feature = "python")]
    mod utils {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/shared.rs"));
    }

    #[cfg(feature = "python")]
    use super::TNIStructure;
    #[cfg(feature = "python")]
    use crate::chord::tables::{FORTE, Sign};
    #[cfg(feature = "python")]
    use pyo3::{
        Bound, PyResult, Python,
        types::{PyAnyMethods, PyDict, PyDictMethods, PyTuple},
    };
    #[cfg(feature = "python")]
    use utils::{get_tables, init_py_with_dummies, prepare};

    #[test]
    fn cardinality_to_chord_members_equality() {
        (0..13).for_each(|i| {
            check_equality(i, CARDINALITY_TO_CHORD_MEMBERS[i].keys());
            check_equality(i, CARDINALITY_TO_CHORD_MEMBERS_GENERATED[i].keys());

            println!("{i} passed");
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

    #[cfg(feature = "python")]
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

        format!("(({first}), ({second}), ({third}))")
    }

    #[test]
    #[cfg(feature = "python")]
    fn python_cardinality_to_chord_members_equality_test() {
        prepare().unwrap();

        Python::attach(|py| -> PyResult<()> {
            init_py_with_dummies(py)?;

            let tables = get_tables(py)?;

            let cardinality_to_chord_members = tables.getattr("cardinalityToChordMembers")?;
            let cardinality_to_chord_members: &Bound<'_, PyDict> =
                cardinality_to_chord_members.cast_exact()?;

            cardinality_to_chord_members.keys().into_iter().for_each(
                |outer_key: Bound<'_, pyo3::PyAny>| {
                    let outer_key_rust: usize = outer_key.extract().unwrap();
                    println!("outer_key: {outer_key:?}");
                    let inner_dict: Bound<'_, pyo3::PyAny> = cardinality_to_chord_members
                        .get_item(outer_key)
                        .unwrap()
                        .unwrap();
                    let inner_dict: &Bound<'_, PyDict> = inner_dict.cast_exact().unwrap();

                    inner_dict.keys().into_iter().for_each(|inner_key| {
                        let inner_key: &Bound<'_, PyTuple> = inner_key.cast_exact().unwrap();

                        let (first, second): (u8, i8) = inner_key.extract().unwrap();
                        let key: U8SB = (first, Sign::from_i8(second).unwrap());

                        println!("python key: {inner_key:?}");
                        println!("rust key: {key:?}");
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
                    println!("{outer_key_rust} passed");
                },
            );

            Ok(())
        })
        .unwrap();
    }

    #[cfg(feature = "python")]
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
                        format!("(({first}), ({second}), ({third}), {fourth})")
                    }
                }
            })
            .collect();
        format!("({})", elems.join(", "))
    }

    #[test]
    fn test() {
        for i in 1..12 {
            println!("{i}");
        }
    }
    #[test]
    #[cfg(feature = "python")]
    fn python_forte_equality_test() {
        prepare().unwrap();

        Python::attach(|py| -> PyResult<()> {
            init_py_with_dummies(py)?;

            let operator = py.import("operator")?;

            let tables = get_tables(py)?;

            let forte = tables.getattr("FORTE")?;
            let forte: &Bound<'_, PyTuple> = forte.cast_exact()?;

            for i in 0..13 {
                let item = operator.call_method1("getitem", (forte, i))?;

                let tuple: Result<&Bound<'_, PyTuple>, _> = item.cast_exact();

                match tuple {
                    Ok(t) => {
                        assert_eq!(format!("{t:?}"), format!("{}", match_python2(&FORTE[i])));
                        println!("{t:?}");
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
