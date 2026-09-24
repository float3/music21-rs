//! music21's `analysis.enharmonics`, over the crate's.
//!
//! The rule classes hold music21's four attributes as the Python values
//! they were given, since music21 reads them afresh on every score and a
//! caller may write `False` as readily as a number. The simplifier keeps
//! music21's state — the pitches, the rule object, each pitch's spellings
//! and their product — and asks the crate to score and choose.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyList, PyTuple};

use music21_rs_crate::Pitch as RsPitch;
use music21_rs_crate::analysis::enharmonics::{self as rs, EnharmonicRules};

use crate::Walkable;
use crate::pitch::{Pitch, pitch_from_any};

// music21 looks a spelling up in its base-40 table, and one it has no
// place for is a missing key.
error_into!(enharmonics_error, pyo3::exceptions::PyKeyError);

/// The names this facade replaces in `music21.analysis.enharmonics`.
pub const NAMES: &[&str] = &[
    "EnharmonicScoreRules",
    "ChordEnharmonicScoreRules",
    "EnharmonicSimplifier",
];

/// music21's `EnharmonicScoreRules`.
#[pyclass(
    name = "EnharmonicScoreRules",
    module = "music21.analysis.enharmonics",
    subclass,
    skip_from_py_object
)]
pub struct EnharmonicScoreRules {
    #[pyo3(get, set)]
    sameStaffLine: Py<PyAny>,
    #[pyo3(get, set)]
    alterationPenalty: Py<PyAny>,
    #[pyo3(get, set)]
    augDimPenalty: Py<PyAny>,
    #[pyo3(get, set)]
    mixSharpsFlatsPenalty: Py<PyAny>,
}

impl EnharmonicScoreRules {
    /// The rules with `rules`' penalties, a penalty that is off as `False`.
    fn of(py: Python<'_>, rules: EnharmonicRules) -> PyResult<Self> {
        let penalty = |penalty: Option<i32>| -> PyResult<Py<PyAny>> {
            Ok(match penalty {
                Some(penalty) => penalty.into_pyobject(py)?.into_any().unbind(),
                None => false.into_pyobject(py)?.to_owned().into_any().unbind(),
            })
        };
        Ok(Self {
            sameStaffLine: false.into_pyobject(py)?.to_owned().into_any().unbind(),
            alterationPenalty: penalty(rules.alteration_penalty())?,
            augDimPenalty: penalty(rules.aug_dim_penalty())?,
            mixSharpsFlatsPenalty: penalty(rules.mixture_penalty())?,
        })
    }
}

#[pymethods]
impl EnharmonicScoreRules {
    #[new]
    fn new(py: Python<'_>) -> PyResult<Self> {
        Self::of(py, EnharmonicRules::MELODIC)
    }
}

/// music21's `ChordEnharmonicScoreRules`.
#[pyclass(
    name = "ChordEnharmonicScoreRules",
    module = "music21.analysis.enharmonics",
    extends = EnharmonicScoreRules,
    subclass
)]
pub struct ChordEnharmonicScoreRules;

#[pymethods]
impl ChordEnharmonicScoreRules {
    #[new]
    fn new(py: Python<'_>) -> PyResult<PyClassInitializer<Self>> {
        Ok(
            PyClassInitializer::from(EnharmonicScoreRules::of(py, EnharmonicRules::CHORDAL)?)
                .add_subclass(Self),
        )
    }
}

/// The crate's rules from a rule object's penalties, each read as music21
/// reads it: anything false is off.
fn rules_of(rule_object: &Bound<'_, PyAny>) -> PyResult<EnharmonicRules> {
    let penalty = |name: &str| -> PyResult<Option<i32>> {
        let value = rule_object.getattr(name)?;
        if !value.is_truthy()? {
            return Ok(None);
        }
        value.extract().map(Some)
    };
    Ok(EnharmonicRules::new(
        penalty("alterationPenalty")?,
        penalty("augDimPenalty")?,
        penalty("mixSharpsFlatsPenalty")?,
    ))
}

/// The crate pitches of a spelling.
fn pitches_of(possibility: &Bound<'_, PyAny>) -> PyResult<Vec<RsPitch>> {
    possibility
        .walk()?
        .map(|pitch| pitch_from_any(&pitch?))
        .collect()
}

/// A new Python pitch of `pitch`.
fn pitch_object(py: Python<'_>, pitch: RsPitch) -> PyResult<Py<PyAny>> {
    let inferred = pitch.spelling_is_inferred();
    Ok(
        crate::installed_new(py, "music21.pitch", "Pitch", Pitch::wrap(pitch, inferred))?
            .into_any(),
    )
}

/// music21's `EnharmonicSimplifier`.
#[pyclass(
    name = "EnharmonicSimplifier",
    module = "music21.analysis.enharmonics",
    subclass,
    skip_from_py_object
)]
pub struct EnharmonicSimplifier {
    #[pyo3(get, set)]
    pitchList: Py<PyAny>,
    #[pyo3(get, set)]
    ruleObject: Py<PyAny>,
    #[pyo3(get, set)]
    allPossibleSpellings: Py<PyAny>,
    #[pyo3(get, set)]
    allSpellings: Py<PyAny>,
}

#[pymethods]
impl EnharmonicSimplifier {
    #[new]
    #[pyo3(signature = (pitchList, ruleClass = None))]
    fn new(
        py: Python<'_>,
        pitchList: &Bound<'_, PyAny>,
        ruleClass: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        // Names are read as pitches when the first is a name, as music21
        // decides it.
        let mut pitch_list = pitchList.clone().unbind();
        if pitchList
            .get_item(0)?
            .is_instance_of::<pyo3::types::PyString>()
        {
            let pitches = pitchList
                .walk()?
                .map(|name| pitch_object(py, pitch_from_any(&name?)?))
                .collect::<PyResult<Vec<_>>>()?;
            pitch_list = PyList::new(py, pitches)?.into_any().unbind();
        }

        let rule_class = match ruleClass.filter(|class| !class.is_none()) {
            Some(class) => class.clone(),
            None => {
                crate::installed_class(py, "music21.analysis.enharmonics", "EnharmonicScoreRules")
                    .unwrap_or_else(|| py.get_type::<EnharmonicScoreRules>().into_any())
            }
        };

        // music21 fills the spellings as the simplifier is built.
        let all_spellings = representations(pitch_list.bind(py))?;
        Ok(Self {
            pitchList: pitch_list,
            ruleObject: rule_class.call0()?.unbind(),
            allPossibleSpellings: py.None(),
            allSpellings: all_spellings,
        })
    }

    /// Each pitch as given, then its common enharmonics.
    fn getRepresentations(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let pitch_list = slf.borrow().pitchList.clone_ref(py);
        slf.borrow_mut().allSpellings = representations(pitch_list.bind(py))?;
        Ok(())
    }

    /// Every combination of the spellings, as tuples in the order
    /// `itertools.product` lists them.
    fn getProduct<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let rows = spelling_rows(slf)?;

        let mut product = Vec::new();
        for choice in index_product(&rows) {
            let spelling = choice
                .iter()
                .zip(&rows)
                .map(|(index, row)| row[*index].clone_ref(py));
            product.push(PyTuple::new(py, spelling)?);
        }
        let product = PyList::new(py, product)?;
        slf.borrow_mut().allPossibleSpellings = product.clone().into_any().unbind();
        Ok(product)
    }

    /// The spelling that scores lowest, the first of equals, as a tuple of
    /// the very pitch objects the spellings hold.
    fn bestPitches<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyTuple>> {
        let py = slf.py();
        Self::getProduct(slf)?;
        let rows = spelling_rows(slf)?;

        let options = rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|pitch| pitch_from_any(pitch.bind(py)))
                    .collect()
            })
            .collect::<PyResult<Vec<Vec<_>>>>()?;
        let rules = rules_of(slf.borrow().ruleObject.bind(py))?;
        let choice = rs::best_choice(&options, rules).map_err(enharmonics_error)?;

        let best = choice
            .iter()
            .zip(&rows)
            .map(|(index, row)| row[*index].clone_ref(py));
        PyTuple::new(py, best)
    }

    fn getAlterationScore(slf: &Bound<'_, Self>, possibility: &Bound<'_, PyAny>) -> PyResult<i32> {
        let rules = rules_of(slf.borrow().ruleObject.bind(slf.py()))?;
        Ok(rules.alteration_score(&pitches_of(possibility)?))
    }

    fn getMixSharpFlatsScore(
        slf: &Bound<'_, Self>,
        possibility: &Bound<'_, PyAny>,
    ) -> PyResult<i32> {
        let rules = rules_of(slf.borrow().ruleObject.bind(slf.py()))?;
        Ok(rules.mixture_score(&pitches_of(possibility)?))
    }

    fn getAugDimScore(slf: &Bound<'_, Self>, possibility: &Bound<'_, PyAny>) -> PyResult<i32> {
        let rules = rules_of(slf.borrow().ruleObject.bind(slf.py()))?;
        rules
            .aug_dim_score(&pitches_of(possibility)?)
            .map_err(enharmonics_error)
    }
}

/// Each pitch in `pitch_list` as given, then its common enharmonics as new
/// pitches: music21's `allSpellings`.
fn representations(pitch_list: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let py = pitch_list.py();
    let given: Vec<Bound<'_, PyAny>> = pitch_list.walk()?.collect::<PyResult<_>>()?;
    let pitches = given
        .iter()
        .map(pitch_from_any)
        .collect::<PyResult<Vec<_>>>()?;

    let mut all = Vec::with_capacity(given.len());
    for (object, options) in given.into_iter().zip(rs::spelling_options(&pitches)) {
        // The first option is the pitch as given, and music21 keeps the
        // very object.
        let mut row = vec![object.unbind()];
        for option in options.into_iter().skip(1) {
            row.push(pitch_object(py, option)?);
        }
        all.push(PyList::new(py, row)?);
    }
    Ok(PyList::new(py, all)?.into_any().unbind())
}

/// Every choice of one index per row, the last row turning fastest, as
/// `itertools.product` lists them. No rows make one empty choice; an empty
/// row makes none.
fn index_product<T>(rows: &[Vec<T>]) -> Vec<Vec<usize>> {
    if rows.iter().any(Vec::is_empty) {
        return Vec::new();
    }

    let mut choices = Vec::new();
    let mut choice = vec![0; rows.len()];
    loop {
        choices.push(choice.clone());

        // Turn the odometer: bump the last position that can move and
        // reset everything after it.
        let Some(position) = (0..rows.len())
            .rev()
            .find(|p| choice[*p] + 1 < rows[*p].len())
        else {
            return choices;
        };
        choice[position] += 1;
        choice[position + 1..].fill(0);
    }
}

/// The spellings each pitch may take, as the objects the simplifier holds.
fn spelling_rows(slf: &Bound<'_, EnharmonicSimplifier>) -> PyResult<Vec<Vec<Py<PyAny>>>> {
    let py = slf.py();
    let all = slf.borrow().allSpellings.clone_ref(py);
    all.bind(py)
        .walk()?
        .map(|row| row?.walk()?.map(|pitch| pitch.map(Bound::unbind)).collect())
        .collect()
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<EnharmonicScoreRules>()?;
    m.add_class::<ChordEnharmonicScoreRules>()?;
    m.add_class::<EnharmonicSimplifier>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::index_product;

    #[test]
    fn choices_come_in_itertools_product_order() {
        let rows = vec![vec!['a', 'b'], vec!['x', 'y', 'z']];
        let product = index_product(&rows);
        assert_eq!(product.len(), 6);
        assert_eq!(product[0], [0, 0]);
        assert_eq!(product[1], [0, 1]);
        assert_eq!(product[3], [1, 0]);
    }

    #[test]
    fn an_empty_row_makes_no_choice_and_no_rows_one() {
        assert!(index_product(&[vec![1], Vec::new()]).is_empty());
        assert_eq!(index_product::<u8>(&[]), [Vec::<usize>::new()]);
    }
}
