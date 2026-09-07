mod generated;

use crate::defaults::IntegerType;
use crate::error::Error;

use generated::*;

pub(crate) type ChordTableAddress = (u8, u8, i8, Option<u8>);

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
    fn invariance_vector(&self) -> InvarianceVector;
    fn z_relation(&self) -> ZRelation;
}

impl TNITupleExt for TNIStructure {
    fn pitches(&self) -> PitchClasses {
        self.0
    }

    fn pitch_classes(&self) -> PitchClasses {
        self.pitches()
    }

    fn invariance_vector(&self) -> InvarianceVector {
        self.2
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

type Forte = [&'static [Option<TNIStructure>]; CARDINALITIES];
type CardinalityToChordMembers = [&'static [(U8SB, Pcivicv)]; CARDINALITIES];
type ForteNumberWithInversionToIndex = &'static [(U8U8SB, u8)];
type TnIndexToChordInfo = &'static [(U8U8SB, Option<&'static [&'static str]>)];
type MaximumIndexNumberWithoutInversionEquivalence = [u8; CARDINALITIES];
type MaximumIndexNumberWithInversionEquivalence = [u8; CARDINALITIES];

#[derive(Debug, Clone)]
pub(crate) struct KnownChordTableEntry {
    pub(crate) cardinality: u8,
    pub(crate) common_names: Vec<&'static str>,
    pub(crate) forte_class: String,
    pub(crate) normal_form: Vec<u8>,
    pub(crate) interval_class_vector: Vec<u8>,
}

/// Whether a Forte index stands for a set class that is its own inversion.
///
/// [`forte_index_to_inversions_available`] answers the same question with a
/// `Vec`, which the address search cannot afford: it asks once per candidate
/// index, and there are up to fifty of those per cardinality.
fn forte_index_is_inversion_equivalent(card: usize, index: u8) -> Result<bool, Error> {
    if !(1..=13).contains(&card) {
        return Err(Error::ChordTables(format!("cardinality {card} not valid")));
    }
    if index < 1 || index > MAXIMUM_INDEX_NUMBER_WITHOUT_INVERSION_EQUIVALENCE[card] {
        return Err(Error::ChordTables(format!(
            "index {index} not valid for cardinality {card}"
        )));
    }
    Ok(FORTE[card]
        .get(index as usize)
        .and_then(Option::as_ref)
        .is_some_and(|entry| entry.invariance_vector()[1] > 0))
}

fn forte_index_to_inversions_available(card: usize, index: u8) -> Result<Vec<Sign>, Error> {
    if !(1..=13).contains(&card) {
        return Err(Error::ChordTables(format!("cardinality {card} not valid")));
    }
    if index < 1 || index > MAXIMUM_INDEX_NUMBER_WITHOUT_INVERSION_EQUIVALENCE[card] {
        return Err(Error::ChordTables(format!(
            "index {index} not valid for cardinality {card}"
        )));
    }

    let mut inversions = vec![];
    if let Some(entry) = FORTE[card].get(index as usize).and_then(Option::as_ref) {
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

fn validate_address(address: (u8, u8, Option<i8>)) -> Result<(u8, u8, Sign), Error> {
    let card = address.0;
    let index = address.1;
    let inversion = address.2.and_then(Sign::from_i8);

    if !(1..=12).contains(&card) {
        return Err(Error::ChordTables(format!("cardinality {card} not valid")));
    }

    if index < 1 || index > MAXIMUM_INDEX_NUMBER_WITHOUT_INVERSION_EQUIVALENCE[card as usize] {
        return Err(Error::ChordTables(format!("index {index} not valid")));
    }

    let inversions_available = forte_index_to_inversions_available(card as usize, index)?;

    let resolved_inversion = if let Some(inv) = inversion {
        if inversions_available.contains(&inv) {
            inv
        } else {
            return Err(Error::ChordTables(format!(
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
) -> Result<ChordTableAddress, Error> {
    if ordered_pitch_classes.is_empty() {
        return Err(Error::ChordTables(
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

    // Every rotation of the set, transposed to start on zero, and its
    // inversion. A chord has at most twelve pitch classes, so all of this
    // fits on the stack: the search runs on every question a caller asks a
    // chord about its set class, and heap traffic here was most of its cost.
    let count = ordered_pitch_classes.len();
    let mut candidates = [([false; 12], [false; 12], 0_u8); 12];
    let mut transposed = [0_u8; 12];
    let mut inverted = [0_u8; 12];
    for rot in 0..count {
        let original_pc = ordered_pitch_classes[rot] % 12;
        for (offset, slot) in transposed[..count].iter_mut().enumerate() {
            let pitch_class = ordered_pitch_classes[(rot + offset) % count];
            *slot = ((IntegerType::from(pitch_class) - IntegerType::from(original_pc))
                .rem_euclid(12)) as u8;
        }
        // the inversion is each degree taken downwards, read back to front
        for offset in 0..count {
            inverted[offset] = (12 - transposed[count - 1 - offset]) % 12;
        }
        let shift = (12 - inverted[0]) % 12;
        for slot in inverted[..count].iter_mut() {
            *slot = (*slot + shift) % 12;
        }
        candidates[rot] = (
            pitch_classes_to_bools(&transposed[..count]),
            pitch_classes_to_bools(&inverted[..count]),
            original_pc,
        );
    }
    let candidates = &candidates[..count];

    for (index_candidate, data_line) in FORTE[card as usize].iter().enumerate().skip(1) {
        let Some(data_line) = data_line else {
            continue;
        };
        let data_line_pcs = data_line.pitch_classes();
        let is_own_inversion =
            || forte_index_is_inversion_equivalent(card as usize, index_candidate as u8);

        for (candidate, candidate_inversion, candidate_original_pc) in candidates {
            if data_line_pcs == *candidate {
                let inversion = if is_own_inversion()? { 0 } else { 1 };
                return Ok((
                    card,
                    index_candidate as u8,
                    inversion,
                    Some(*candidate_original_pc),
                ));
            }
            if data_line_pcs == *candidate_inversion {
                let inversion = if is_own_inversion()? { 0 } else { -1 };
                return Ok((
                    card,
                    index_candidate as u8,
                    inversion,
                    Some(*candidate_original_pc),
                ));
            }
        }
    }

    Err(Error::ChordTables(format!(
        "cannot find a chord table address for {ordered_pitch_classes:?}"
    )))
}

pub(crate) fn address_to_common_names(
    address: ChordTableAddress,
) -> Result<Option<Vec<&'static str>>, Error> {
    let (card, index, inversion) = validate_address((address.0, address.1, Some(address.2)))?;
    Ok(find_tn_index_to_chord_info(card, index, inversion).map(<[_]>::to_vec))
}

pub(crate) fn address_to_forte_name(
    address: ChordTableAddress,
    classification: &str,
) -> Result<String, Error> {
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

/// The prime form of a set class, which is the same whichever inversion the
/// address names.
pub(crate) fn prime_form_from_address(address: ChordTableAddress) -> Result<Vec<u8>, Error> {
    let (card, index, inversion) = validate_address((address.0, address.1, None))?;
    let entry = find_cardinality_member(card, index, inversion).ok_or_else(|| {
        Error::ChordTables(format!(
            "cannot resolve prime form for address ({card}, {index})"
        ))
    })?;
    Ok(bool_vec_to_pitch_classes(&entry.0))
}

/// The transposed normal form for a cardinality and Forte number, with the
/// inversion left to the table's default when `None`.
pub(crate) fn transposed_normal_form(
    cardinality: u8,
    index: u8,
    inversion: Option<i8>,
) -> Result<Vec<u8>, Error> {
    let (card, index, inversion) = validate_address((cardinality, index, inversion))?;
    let entry = find_cardinality_member(card, index, inversion).ok_or_else(|| {
        Error::ChordTables(format!(
            "cannot resolve normal form for address ({card}, {index}, {})",
            inversion.as_i8()
        ))
    })?;
    Ok(bool_vec_to_pitch_classes(&entry.0))
}

pub(crate) fn transposed_normal_form_from_address(
    address: ChordTableAddress,
) -> Result<Vec<u8>, Error> {
    let (card, index, inversion) = validate_address((address.0, address.1, Some(address.2)))?;
    let entry = find_cardinality_member(card, index, inversion).ok_or_else(|| {
        Error::ChordTables(format!(
            "cannot resolve normal form for address ({card}, {index}, {})",
            inversion.as_i8()
        ))
    })?;
    Ok(bool_vec_to_pitch_classes(&entry.0))
}

pub(crate) fn interval_class_vector_from_address(
    address: ChordTableAddress,
) -> Result<Vec<u8>, Error> {
    let (card, index, inversion) = validate_address((address.0, address.1, Some(address.2)))?;
    let entry = find_cardinality_member(card, index, inversion).ok_or_else(|| {
        Error::ChordTables(format!(
            "cannot resolve interval class vector for address ({card}, {index}, {})",
            inversion.as_i8()
        ))
    })?;
    Ok(entry.2.to_vec())
}

pub(crate) fn invariance_vector_from_address(address: ChordTableAddress) -> Result<Vec<u8>, Error> {
    let (card, index, inversion) = validate_address((address.0, address.1, Some(address.2)))?;
    let entry = FORTE[card as usize]
        .get(index as usize)
        .and_then(Option::as_ref)
        .ok_or_else(|| {
            Error::ChordTables(format!(
                "cannot resolve invariance vector for address ({card}, {index}, {})",
                inversion.as_i8()
            ))
        })?;
    Ok(entry.invariance_vector().to_vec())
}

pub(crate) fn z_relation_from_address(address: ChordTableAddress) -> Result<Option<String>, Error> {
    let (card, index, inversion) = validate_address((address.0, address.1, Some(address.2)))?;
    let entry = FORTE[card as usize]
        .get(index as usize)
        .and_then(Option::as_ref)
        .ok_or_else(|| {
            Error::ChordTables(format!(
                "cannot resolve z-relation for address ({card}, {index}, {})",
                inversion.as_i8()
            ))
        })?;
    let z_relation = entry.z_relation();
    if z_relation == 0 {
        Ok(None)
    } else {
        Ok(Some(format!("{card}-{}", z_relation)))
    }
}

pub(crate) fn known_chord_table_entries() -> Vec<KnownChordTableEntry> {
    let mut entries = TN_INDEX_TO_CHORD_INFO
        .iter()
        .filter_map(|&((cardinality, index, inversion), common_names)| {
            let common_names = common_names.unwrap_or_default().to_vec();
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

fn find_cardinality_member(card: u8, index: u8, inversion: Sign) -> Option<&'static Pcivicv> {
    CARDINALITY_TO_CHORD_MEMBERS
        .get(card as usize)?
        .iter()
        .find_map(|(key, value)| (*key == (index, inversion)).then_some(value))
}

fn find_tn_index_to_chord_info(
    card: u8,
    index: u8,
    inversion: Sign,
) -> Option<&'static [&'static str]> {
    TN_INDEX_TO_CHORD_INFO
        .iter()
        .find_map(|(key, names)| (*key == (card, index, inversion)).then_some(*names))
        .flatten()
}

// include!("./generated.rs");

#[cfg(test)]
mod tests {
    use super::{Sign, find_cardinality_member};

    #[test]
    fn cardinality_to_chord_members_include_major_triad() {
        let member = find_cardinality_member(3, 11, Sign::NegativeOne).unwrap();
        assert_eq!(
            member.0,
            [
                true, false, false, false, true, false, false, true, false, false, false, false
            ]
        );
        assert_eq!(member.2, [0, 0, 1, 1, 1, 0]);
    }
}
