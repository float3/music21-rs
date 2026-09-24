//! music21's `figuredBass.rules`, over the crate's.
//!
//! The switches live in the crate's `Rules`. The three lists music21 keeps on
//! one -- the part movement limits and the realizer's own part limits and
//! parts to keep -- are held as the lists they are, since music21's realizer
//! appends to them, and read into the crate's rules whenever a realization
//! asks.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyList;

use music21_rs_crate::figuredbass::rules::Rules as RsRules;

use crate::pitch::pitch_from_any;

/// The names this facade replaces in `music21.figuredBass.rules`.
pub const NAMES: &[&str] = &["Rules"];

/// music21's `figuredBass.rules.Rules`.
#[pyclass(
    name = "Rules",
    module = "music21.figuredBass.rules",
    subclass,
    skip_from_py_object
)]
pub struct Rules {
    inner: RsRules,
    part_movement_limits: Py<PyList>,
    part_pitch_limits: Py<PyList>,
    parts_to_check: Py<PyList>,
}

impl Rules {
    /// The crate's rules, with the lists read as they stand now.
    pub(crate) fn synced(&self, py: Python<'_>) -> PyResult<RsRules> {
        let mut rules = self.inner.clone();
        rules.part_movement_limits = self
            .part_movement_limits
            .bind(py)
            .iter()
            .map(|limit| pair::<usize, i32>(&limit))
            .collect::<PyResult<_>>()?;
        rules.part_pitch_limits = self
            .part_pitch_limits
            .bind(py)
            .iter()
            .map(|limit| {
                let (part, pitch) = pair::<usize, Bound<'_, PyAny>>(&limit)?;
                Ok((part, pitch_from_any(&pitch)?))
            })
            .collect::<PyResult<_>>()?;
        rules.parts_to_check = self
            .parts_to_check
            .bind(py)
            .iter()
            .map(|part| part.extract::<usize>())
            .collect::<PyResult<_>>()?;
        Ok(rules)
    }

    fn fresh(py: Python<'_>) -> Self {
        Self {
            inner: RsRules::default(),
            part_movement_limits: PyList::empty(py).unbind(),
            part_pitch_limits: PyList::empty(py).unbind(),
            parts_to_check: PyList::empty(py).unbind(),
        }
    }
}

#[pymethods]
impl Rules {
    #[new]
    fn new(py: Python<'_>) -> Self {
        Self::fresh(py)
    }

    #[getter]
    fn get_forbidIncompletePossibilities(&self) -> bool {
        self.inner.forbid_incomplete_possibilities
    }

    #[setter]
    fn set_forbidIncompletePossibilities(&mut self, value: bool) {
        self.inner.forbid_incomplete_possibilities = value;
    }

    #[getter]
    fn get_forbidVoiceCrossing(&self) -> bool {
        self.inner.forbid_voice_crossing
    }

    #[setter]
    fn set_forbidVoiceCrossing(&mut self, value: bool) {
        self.inner.forbid_voice_crossing = value;
    }

    #[getter]
    fn get_forbidParallelFifths(&self) -> bool {
        self.inner.forbid_parallel_fifths
    }

    #[setter]
    fn set_forbidParallelFifths(&mut self, value: bool) {
        self.inner.forbid_parallel_fifths = value;
    }

    #[getter]
    fn get_forbidParallelOctaves(&self) -> bool {
        self.inner.forbid_parallel_octaves
    }

    #[setter]
    fn set_forbidParallelOctaves(&mut self, value: bool) {
        self.inner.forbid_parallel_octaves = value;
    }

    #[getter]
    fn get_forbidHiddenFifths(&self) -> bool {
        self.inner.forbid_hidden_fifths
    }

    #[setter]
    fn set_forbidHiddenFifths(&mut self, value: bool) {
        self.inner.forbid_hidden_fifths = value;
    }

    #[getter]
    fn get_forbidHiddenOctaves(&self) -> bool {
        self.inner.forbid_hidden_octaves
    }

    #[setter]
    fn set_forbidHiddenOctaves(&mut self, value: bool) {
        self.inner.forbid_hidden_octaves = value;
    }

    #[getter]
    fn get_forbidVoiceOverlap(&self) -> bool {
        self.inner.forbid_voice_overlap
    }

    #[setter]
    fn set_forbidVoiceOverlap(&mut self, value: bool) {
        self.inner.forbid_voice_overlap = value;
    }

    #[getter]
    fn get_resolveDominantSeventhProperly(&self) -> bool {
        self.inner.resolve_dominant_seventh_properly
    }

    #[setter]
    fn set_resolveDominantSeventhProperly(&mut self, value: bool) {
        self.inner.resolve_dominant_seventh_properly = value;
    }

    #[getter]
    fn get_resolveDiminishedSeventhProperly(&self) -> bool {
        self.inner.resolve_diminished_seventh_properly
    }

    #[setter]
    fn set_resolveDiminishedSeventhProperly(&mut self, value: bool) {
        self.inner.resolve_diminished_seventh_properly = value;
    }

    #[getter]
    fn get_resolveAugmentedSixthProperly(&self) -> bool {
        self.inner.resolve_augmented_sixth_properly
    }

    #[setter]
    fn set_resolveAugmentedSixthProperly(&mut self, value: bool) {
        self.inner.resolve_augmented_sixth_properly = value;
    }

    #[getter]
    fn get_doubledRootInDim7(&self) -> bool {
        self.inner.doubled_root_in_dim7
    }

    #[setter]
    fn set_doubledRootInDim7(&mut self, value: bool) {
        self.inner.doubled_root_in_dim7 = value;
    }

    #[getter]
    fn get_applySinglePossibRulesToResolution(&self) -> bool {
        self.inner.apply_single_possib_rules_to_resolution
    }

    #[setter]
    fn set_applySinglePossibRulesToResolution(&mut self, value: bool) {
        self.inner.apply_single_possib_rules_to_resolution = value;
    }

    #[getter]
    fn get_applyConsecutivePossibRulesToResolution(&self) -> bool {
        self.inner.apply_consecutive_possib_rules_to_resolution
    }

    #[setter]
    fn set_applyConsecutivePossibRulesToResolution(&mut self, value: bool) {
        self.inner.apply_consecutive_possib_rules_to_resolution = value;
    }

    #[getter]
    fn get_restrictDoublingsInItalianA6Resolution(&self) -> bool {
        self.inner.restrict_doublings_in_italian_a6_resolution
    }

    #[setter]
    fn set_restrictDoublingsInItalianA6Resolution(&mut self, value: bool) {
        self.inner.restrict_doublings_in_italian_a6_resolution = value;
    }

    #[getter]
    fn get__upperPartsRemainSame(&self) -> bool {
        self.inner.upper_parts_remain_same
    }

    #[setter]
    fn set__upperPartsRemainSame(&mut self, value: bool) {
        self.inner.upper_parts_remain_same = value;
    }

    /// How far apart the upper parts may lie, in semitones, or `None` for
    /// anywhere.
    #[getter]
    fn get_upperPartsMaxSemitoneSeparation(&self) -> Option<i32> {
        self.inner.upper_parts_max_semitone_separation
    }

    #[setter]
    fn set_upperPartsMaxSemitoneSeparation(&mut self, value: Option<i32>) {
        self.inner.upper_parts_max_semitone_separation = value;
    }

    /// `(partNumber, maxSeparation)` pairs, the list itself.
    #[getter]
    fn get_partMovementLimits(&self, py: Python<'_>) -> Py<PyList> {
        self.part_movement_limits.clone_ref(py)
    }

    #[setter]
    fn set_partMovementLimits(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.part_movement_limits = listed(value)?;
        Ok(())
    }

    #[getter]
    fn get__partPitchLimits(&self, py: Python<'_>) -> Py<PyList> {
        self.part_pitch_limits.clone_ref(py)
    }

    #[setter]
    fn set__partPitchLimits(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.part_pitch_limits = listed(value)?;
        Ok(())
    }

    #[getter]
    fn get__partsToCheck(&self, py: Python<'_>) -> Py<PyList> {
        self.parts_to_check.clone_ref(py)
    }

    #[setter]
    fn set__partsToCheck(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.parts_to_check = listed(value)?;
        Ok(())
    }

    fn _reprInternal(&self) -> &'static str {
        ""
    }

    fn __repr__(&self) -> &'static str {
        "<music21.figuredBass.rules.Rules>"
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let copier = py.import("copy")?.getattr("deepcopy")?;
        let copy_list = |list: &Py<PyList>| -> PyResult<Py<PyList>> {
            let copied = match memo {
                Some(memo) => copier.call1((list.bind(py), memo))?,
                None => copier.call1((list.bind(py),))?,
            };
            Ok(copied.cast_into::<PyList>()?.unbind())
        };
        let copied = slf.get_type().call0()?;
        {
            let me = slf.borrow();
            let movement = copy_list(&me.part_movement_limits)?;
            let pitch = copy_list(&me.part_pitch_limits)?;
            let check = copy_list(&me.parts_to_check)?;
            let mut copy = copied.extract::<PyRefMut<'_, Self>>()?;
            copy.inner = me.inner.clone();
            copy.part_movement_limits = movement;
            copy.part_pitch_limits = pitch;
            copy.parts_to_check = check;
        }
        Ok(copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::__deepcopy__(slf, None)
    }
}

/// The two items of a pair music21 unpacks, written as a tuple or a list.
pub(crate) fn pair<'py, A, B>(value: &Bound<'py, PyAny>) -> PyResult<(A, B)>
where
    A: for<'a> FromPyObject<'a, 'py>,
    B: for<'a> FromPyObject<'a, 'py>,
{
    let items: Vec<Bound<'py, PyAny>> = value.try_iter()?.collect::<PyResult<_>>()?;
    let [first, second]: [Bound<'py, PyAny>; 2] = items
        .try_into()
        .map_err(|_| pyo3::exceptions::PyValueError::new_err("expected a pair of two values"))?;
    Ok((
        first.extract().map_err(Into::into)?,
        second.extract().map_err(Into::into)?,
    ))
}

/// A value music21 assigns as a list, kept as one.
fn listed(value: &Bound<'_, PyAny>) -> PyResult<Py<PyList>> {
    match value.cast::<PyList>() {
        Ok(list) => Ok(list.clone().unbind()),
        Err(_) => {
            Ok(PyList::new(value.py(), value.try_iter()?.collect::<PyResult<Vec<_>>>()?)?.unbind())
        }
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Rules>()
}
