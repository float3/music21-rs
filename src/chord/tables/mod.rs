//! Look-up over music21's Forte tables of set classes.
//!
//! These are the tables every set-class question a [`Chord`] answers is read
//! out of — its prime form, its Forte name, its interval vector — and
//! music21 exposes them as `chord.tables` so that a caller holding an
//! address can read the same values with no chord to ask. A set class is
//! named by its cardinality and its Forte class number, with an inversion of
//! `1` or `-1` for the two halves of an inversionally related pair and `0`
//! for a class that is its own inversion; the inversion may be left out, and
//! the table then says which it is.
//!
//! The tables are generated from music21's own `chord/tables.py`; see
//! `xtask verify-tables`.
//!
//! [`Chord`]: crate::Chord

mod generated;

use crate::chord::ChordTableAddress;
use crate::defaults::IntegerType;
use crate::error::Error;

use generated::*;

/// An address as the table search hands one back, before it is read into a
/// [`ChordTableAddress`]: cardinality, Forte class, inversion, and the pitch
/// class the prime form was transposed away from.
pub(crate) type RawAddress = (u8, u8, i8, Option<u8>);

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
    if !(1..=12).contains(&card) {
        return Err(Error::ChordTables(format!("cardinality {card} not valid")));
    }
    if index < 1 || index > MAXIMUM_INDEX_NUMBER_WITHOUT_INVERSION_EQUIVALENCE[card] {
        return Err(Error::ChordTables(format!("index {index} not valid")));
    }
    Ok(FORTE[card]
        .get(index as usize)
        .and_then(Option::as_ref)
        .is_some_and(|entry| entry.invariance_vector()[1] > 0))
}

fn forte_index_to_inversions_available(card: usize, index: u8) -> Result<Vec<Sign>, Error> {
    if !(1..=12).contains(&card) {
        return Err(Error::ChordTables(format!("cardinality {card} not valid")));
    }
    if index < 1 || index > MAXIMUM_INDEX_NUMBER_WITHOUT_INVERSION_EQUIVALENCE[card] {
        return Err(Error::ChordTables(format!("index {index} not valid")));
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
    // An inversion is one of three things, and anything else is the caller's
    // mistake rather than an inversion left out — reading `-30` as "say
    // nothing" would answer for a set class nobody asked about.
    let inversion = match address.2 {
        None => None,
        Some(given) => Some(
            Sign::from_i8(given)
                .ok_or_else(|| Error::ChordTables(format!("inversion {given} not valid")))?,
        ),
    };

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

pub(crate) fn seek_chord_tables_address(ordered_pitch_classes: &[u8]) -> Result<RawAddress, Error> {
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
    address: RawAddress,
) -> Result<Option<Vec<&'static str>>, Error> {
    let (card, index, inversion) = validate_address((address.0, address.1, Some(address.2)))?;
    Ok(find_tn_index_to_chord_info(card, index, inversion).map(<[_]>::to_vec))
}

pub(crate) fn address_to_forte_name(
    address: RawAddress,
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
pub(crate) fn prime_form_from_address(address: RawAddress) -> Result<Vec<u8>, Error> {
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

pub(crate) fn transposed_normal_form_from_address(address: RawAddress) -> Result<Vec<u8>, Error> {
    let (card, index, inversion) = validate_address((address.0, address.1, Some(address.2)))?;
    let entry = find_cardinality_member(card, index, inversion).ok_or_else(|| {
        Error::ChordTables(format!(
            "cannot resolve normal form for address ({card}, {index}, {})",
            inversion.as_i8()
        ))
    })?;
    Ok(bool_vec_to_pitch_classes(&entry.0))
}

pub(crate) fn interval_class_vector_from_address(address: RawAddress) -> Result<Vec<u8>, Error> {
    let (card, index, inversion) = validate_address((address.0, address.1, Some(address.2)))?;
    let entry = find_cardinality_member(card, index, inversion).ok_or_else(|| {
        Error::ChordTables(format!(
            "cannot resolve interval class vector for address ({card}, {index}, {})",
            inversion.as_i8()
        ))
    })?;
    Ok(entry.2.to_vec())
}

pub(crate) fn invariance_vector_from_address(address: RawAddress) -> Result<Vec<u8>, Error> {
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

pub(crate) fn z_relation_from_address(address: RawAddress) -> Result<Option<String>, Error> {
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

// ---------------------------------------------------------------------------
// The tables as music21 exposes them: a set class named by its cardinality
// and Forte class number, asked about without a chord in hand.
// ---------------------------------------------------------------------------

/// The inversions a set class comes in: `[0]` for one that is its own
/// inversion, and `[-1, 1]` for the two halves of an inversionally related
/// pair.
///
/// music21's `forteIndexToInversionsAvailable`.
///
/// # Errors
///
/// When the cardinality or the class number names no set class.
pub fn inversions_available(cardinality: u8, forte_class: u8) -> Result<Vec<i8>, Error> {
    Ok(
        forte_index_to_inversions_available(cardinality as usize, forte_class)?
            .into_iter()
            .map(|sign| sign.as_i8())
            .collect(),
    )
}

/// Reads an address, filling in the inversion where one was not given:
/// music21's `_validateAddress`.
///
/// A class that is its own inversion answers nought; one that is not answers
/// the first of its pair.
///
/// # Errors
///
/// When the cardinality, the class number or the inversion names no set
/// class.
pub fn read_address(
    cardinality: u8,
    forte_class: u8,
    inversion: Option<i8>,
) -> Result<(u8, u8, i8), Error> {
    let (cardinality, forte_class, inversion) =
        validate_address((cardinality, forte_class, inversion))?;
    Ok((cardinality, forte_class, inversion.as_i8()))
}

/// The set class's normal form, transposed to start on zero: music21's
/// `addressToTransposedNormalForm`.
///
/// # Errors
///
/// When the address names no set class.
pub fn normal_form(
    cardinality: u8,
    forte_class: u8,
    inversion: Option<i8>,
) -> Result<Vec<u8>, Error> {
    transposed_normal_form(cardinality, forte_class, inversion)
}

/// The set class's prime form, which is the same whichever inversion the
/// address names: music21's `addressToPrimeForm`.
///
/// # Errors
///
/// When the address names no set class.
pub fn prime_form(cardinality: u8, forte_class: u8) -> Result<Vec<u8>, Error> {
    transposed_normal_form(cardinality, forte_class, None)
}

/// The set class's interval class vector, six counts of how many intervals
/// of each class it holds: music21's `addressToIntervalVector`.
///
/// # Errors
///
/// When the address names no set class.
pub fn interval_class_vector(
    cardinality: u8,
    forte_class: u8,
    inversion: Option<i8>,
) -> Result<Vec<u8>, Error> {
    let (cardinality, forte_class, inversion) = read_address(cardinality, forte_class, inversion)?;
    interval_class_vector_from_address((cardinality, forte_class, inversion, None))
}

/// Every set class with this interval vector, as cardinality and Forte class
/// number pairs: music21's `intervalVectorToAddress`.
///
/// More than one comes back for a Z-related pair — the all-interval
/// tetrachord is both 4-15 and 4-29 — and none at all for a vector no set
/// class has. The inversion is not part of the answer, since a vector says
/// nothing about which half of a pair is meant.
///
/// # Errors
///
/// When the vector is not six counts long.
pub fn set_classes_with_interval_vector(vector: &[u8]) -> Result<Vec<(u8, u8)>, Error> {
    if vector.len() != 6 {
        return Err(Error::ChordTables(
            "Vector must have exactly six entries".to_string(),
        ));
    }
    let mut found = Vec::new();
    for (cardinality, classes) in FORTE.iter().enumerate().skip(1) {
        for (forte_class, entry) in classes.iter().enumerate() {
            let Some(entry) = entry else {
                continue;
            };
            if entry.1 == vector {
                found.push((cardinality as u8, forte_class as u8));
            }
        }
    }
    Ok(found)
}

/// The set class Z-related to this one — the other with the same interval
/// vector — where there is one: music21's `addressToZAddress`.
///
/// The inversion comes back read the way [`read_address`] reads one.
///
/// # Errors
///
/// When the address names no set class.
pub fn z_related_class(cardinality: u8, forte_class: u8) -> Result<Option<(u8, u8, i8)>, Error> {
    let (cardinality, forte_class, _) = read_address(cardinality, forte_class, None)?;
    let related = FORTE[cardinality as usize]
        .get(forte_class as usize)
        .and_then(Option::as_ref)
        .map_or(0, TNITupleExt::z_relation);
    if related == 0 {
        return Ok(None);
    }
    Some(read_address(cardinality, related, None)).transpose()
}

/// What the set class is commonly called, where it is called anything:
/// music21's `addressToCommonNames`.
///
/// More than one name comes back where more than one is in use, and nothing
/// where the class has no common name. The names take no account of spelling
/// or inversion, so a minor triad answers "major triad"; [`Chord::common_name`]
/// is the one that reads the chord as it is written.
///
/// # Errors
///
/// When the address names no set class.
///
/// [`Chord::common_name`]: crate::Chord::common_name
pub fn common_names(
    cardinality: u8,
    forte_class: u8,
    inversion: Option<i8>,
) -> Result<Option<Vec<&'static str>>, Error> {
    let (cardinality, forte_class, inversion) = read_address(cardinality, forte_class, inversion)?;
    address_to_common_names((cardinality, forte_class, inversion, None))
}

/// The set class's Forte name, such as `8-15B`: music21's
/// `addressToForteName`.
///
/// `inversional_equivalence` is music21's `'tni'` classification, which
/// leaves off the `A` and `B` that tell the two halves of a pair apart.
///
/// # Errors
///
/// When the address names no set class.
pub fn forte_name(
    cardinality: u8,
    forte_class: u8,
    inversion: Option<i8>,
    inversional_equivalence: bool,
) -> Result<String, Error> {
    let (cardinality, forte_class, inversion) = read_address(cardinality, forte_class, inversion)?;
    address_to_forte_name(
        (cardinality, forte_class, inversion, None),
        if inversional_equivalence { "tni" } else { "tn" },
    )
}

/// Where a set of pitch classes sits in the tables: music21's
/// `seekChordTablesAddress`.
///
/// The pitch classes are the chord's own, in order and without repeats —
/// [`Chord::pitch_classes`] is what a chord hands over.
///
/// # Errors
///
/// When there are no pitch classes at all, or more than the tables hold.
///
/// [`Chord::pitch_classes`]: crate::Chord::pitch_classes
pub fn address_of_pitch_classes(pitch_classes: &[u8]) -> Result<ChordTableAddress, Error> {
    let (cardinality, forte_class, inversion, original) = seek_chord_tables_address(pitch_classes)?;
    Ok(ChordTableAddress {
        cardinality,
        forte_class,
        inversion,
        pitch_class_original: original.unwrap_or(0),
    })
}

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

    /// music21's own answers, out of the docstrings of `chord/tables.py`.
    #[test]
    fn the_tables_answer_what_music21_answers() {
        use super::*;

        assert_eq!(inversions_available(3, 1).unwrap(), [0]);
        assert_eq!(inversions_available(3, 2).unwrap(), [-1, 1]);
        assert_eq!(inversions_available(3, 12).unwrap(), [0]);
        assert!(inversions_available(20, 1).is_err());
        assert!(inversions_available(8, 200).is_err());

        // An inversion left out is read off the table: nought where the
        // class is its own inversion, and the first of the pair otherwise.
        assert_eq!(read_address(3, 1, Some(0)).unwrap(), (3, 1, 0));
        assert_eq!(read_address(2, 3, None).unwrap(), (2, 3, 0));
        assert_eq!(read_address(3, 12, None).unwrap(), (3, 12, 0));
        assert!(read_address(8, 3, Some(-30)).is_err());

        assert_eq!(normal_form(3, 1, Some(0)).unwrap(), [0, 1, 2]);
        assert_eq!(normal_form(3, 11, Some(-1)).unwrap(), [0, 4, 7]);
        assert_eq!(normal_form(3, 11, Some(1)).unwrap(), [0, 3, 7]);
        assert_eq!(normal_form(3, 11, None).unwrap(), [0, 3, 7]);

        // The prime form is the same whichever half of the pair is named.
        assert_eq!(prime_form(3, 11).unwrap(), [0, 3, 7]);
        assert_eq!(prime_form(3, 1).unwrap(), [0, 1, 2]);

        assert_eq!(
            interval_class_vector(3, 1, Some(0)).unwrap(),
            [2, 1, 0, 0, 0, 0]
        );
        assert_eq!(
            interval_class_vector(3, 11, Some(-1)).unwrap(),
            [0, 0, 1, 1, 1, 0]
        );
        assert_eq!(
            interval_class_vector(4, 29, None).unwrap(),
            [1, 1, 1, 1, 1, 1]
        );

        assert_eq!(
            set_classes_with_interval_vector(&[7, 6, 5, 4, 4, 2]).unwrap(),
            [(8, 1)]
        );
        assert_eq!(
            set_classes_with_interval_vector(&[2, 2, 3, 1, 1, 1]).unwrap(),
            [(5, 10)]
        );
        assert!(
            set_classes_with_interval_vector(&[2, 2, 3, 1, 1, 99])
                .unwrap()
                .is_empty()
        );
        // The all-interval tetrachord is a Z-related pair, so both come back.
        assert_eq!(
            set_classes_with_interval_vector(&[1, 1, 1, 1, 1, 1]).unwrap(),
            [(4, 15), (4, 29)]
        );
        assert!(set_classes_with_interval_vector(&[0, 2, 4]).is_err());

        assert_eq!(z_related_class(5, 12).unwrap(), Some((5, 36, 1)));
        assert_eq!(z_related_class(5, 36).unwrap(), Some((5, 12, 0)));
        assert_eq!(z_related_class(3, 11).unwrap(), None);
        assert_eq!(z_related_class(8, 29).unwrap(), Some((8, 15, 1)));

        assert_eq!(
            common_names(3, 1, Some(0)).unwrap(),
            Some(vec!["chromatic trimirror"])
        );
        assert_eq!(
            common_names(3, 11, Some(-1)).unwrap(),
            Some(vec!["major triad"])
        );
        assert_eq!(
            common_names(7, 33, None).unwrap(),
            Some(vec!["Neapolitan-major mode", "leading-whole-tone mode"])
        );

        assert_eq!(forte_name(8, 15, Some(-1), false).unwrap(), "8-15B");
        assert_eq!(forte_name(8, 15, None, false).unwrap(), "8-15A");
        assert_eq!(forte_name(3, 12, None, false).unwrap(), "3-12");
        assert_eq!(forte_name(8, 15, None, true).unwrap(), "8-15");
        assert_eq!(forte_name(5, 37, None, false).unwrap(), "5-37");

        let address = address_of_pitch_classes(&[0]).unwrap();
        assert_eq!(address.cardinality, 1);
        assert_eq!(address.forte_class, 1);
        assert_eq!(address.inversion, 0);
        assert_eq!(address.pitch_class_original, 0);
        assert!(address_of_pitch_classes(&[]).is_err());
    }
}
