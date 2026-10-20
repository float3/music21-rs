//! music21's `alpha.analysis.aligner`: two streams aligned note by note,
//! as the cheapest edit from one to the other.
//!
//! The alignment is the crate's `alpha::analysis::aligner`, which
//! `alpha_parity` holds to music21, and every step of it is one the crate
//! carries: the costs over a pair of hashes (`insert_cost`,
//! `delete_cost`, `num_similarities`, `hashes_equal_without_reference`,
//! `substitution_cost`), the matrix before and after it is filled
//! (`setup_distance_matrix`, `fill_distance_matrix`), the three moves onto
//! a cell and the one of them taken (`moves_from_location`,
//! `op_from_location`), and the edit read back out of the matrix
//! (`change_places`). music21 documents each of those on its own -- its
//! own examples fill a matrix by hand and ask what a cell says -- so the
//! facade is a reading of music21's objects into the crate's values and
//! back, and nothing of the alignment is written here.
//!
//! **The matrix is numpy's, and a caller writes into it.** music21's own
//! examples assign `sa.distanceMatrix[i][j]` and then ask what move that
//! makes, and print the array itself, so the facade keeps music21's own
//! `numpy.zeros((n + 1, m + 1), dtype=int)` and reads it afresh for every
//! question. Without numpy the aligner refuses, with music21's own
//! sentence: it has no matrix to work in.
//!
//! **A hash is read by its keys.** music21 reads a hashed note with
//! `getattr(tup, key)` for each of its `hashItemsKeys`, which is what the
//! facade reads into the crate's `NoteHash`, so a hash music21's own
//! hasher made is read as readily as one of ours -- and the examples that
//! build a `NoteHashWithReference` by hand out of a bare `namedtuple` are
//! read the same way.
//!
//! **What each change is between stays the score's own object.** The
//! crate's `change_places` says which hash of each side a change lies
//! between, and the facade reads `reference` off that hash, so a change
//! names the very note music21's `showChanges` colours. `ChangeOps` is
//! not swapped: it is an `IntEnum` holding nothing of the crate, so its
//! four examples pass on music21's own class, and the one here -- which
//! reads its colours off `ChangeOp::color` -- is what a wheel with no
//! music21 beside it names.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

use music21_rs_crate::alpha::analysis::aligner::{
    ChangeOp, change_places, delete_cost, fill_distance_matrix, hashes_equal_without_reference,
    insert_cost, moves_from_location, num_similarities, op_from_location, setup_distance_matrix,
    substitution_cost,
};
use music21_rs_crate::alpha::analysis::hasher::{HashValue, NoteHash};

use crate::Walkable;

/// The names this module hands out.
pub const NAMES: &[&str] = &["StreamAligner"];

pyo3::create_exception!(music21_rs_facade, AlignerException, crate::Music21Exception);
pyo3::create_exception!(
    music21_rs_facade,
    AlignmentTracebackException,
    AlignerException
);

/// music21's `alpha.analysis.aligner.ChangeOps`, an `IntEnum` of the four
/// changes.
///
/// music21's own answers its own examples wherever music21 is there to
/// ask; this is what a wheel with none beside it names, and its colours
/// are the crate's.
#[pyclass(
    name = "ChangeOps",
    module = "music21.alpha.analysis.aligner",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Copy)]
pub struct ChangeOps {
    inner: ChangeOp,
}

impl ChangeOps {
    fn wrap(inner: ChangeOp) -> Self {
        Self { inner }
    }

    fn python_name(&self) -> &'static str {
        match self.inner {
            ChangeOp::Insertion => "Insertion",
            ChangeOp::Deletion => "Deletion",
            ChangeOp::Substitution => "Substitution",
            ChangeOp::NoChange => "NoChange",
        }
    }
}

#[pymethods]
impl ChangeOps {
    #[new]
    fn new(value: i64) -> PyResult<Self> {
        let inner = match value {
            0 => ChangeOp::Insertion,
            1 => ChangeOp::Deletion,
            2 => ChangeOp::Substitution,
            3 => ChangeOp::NoChange,
            other => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "{other} is not a valid ChangeOps"
                )));
            }
        };
        Ok(Self::wrap(inner))
    }

    #[classattr]
    fn Insertion() -> Self {
        Self::wrap(ChangeOp::Insertion)
    }

    #[classattr]
    fn Deletion() -> Self {
        Self::wrap(ChangeOp::Deletion)
    }

    #[classattr]
    fn Substitution() -> Self {
        Self::wrap(ChangeOp::Substitution)
    }

    #[classattr]
    fn NoChange() -> Self {
        Self::wrap(ChangeOp::NoChange)
    }

    /// What colour the change is shown in: music21's `color`.
    #[getter]
    fn color(&self) -> Option<&'static str> {
        self.inner.color()
    }

    #[getter]
    fn name(&self) -> &'static str {
        self.python_name()
    }

    #[getter]
    fn value(&self) -> i64 {
        index_of(self.inner)
    }

    fn __int__(&self) -> i64 {
        index_of(self.inner)
    }

    fn __index__(&self) -> i64 {
        index_of(self.inner)
    }

    fn __hash__(&self) -> isize {
        index_of(self.inner) as isize
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        if let Ok(other) = other.extract::<PyRef<ChangeOps>>() {
            return other.inner == self.inner;
        }
        other
            .extract::<i64>()
            .map(|value| value == index_of(self.inner))
            .unwrap_or(false)
    }

    fn __repr__(&self) -> String {
        format!("<ChangeOps.{}: {}>", self.python_name(), self.value())
    }

    fn __str__(&self) -> String {
        self.value().to_string()
    }
}

/// music21's `ChangeOps`, or this module's own where music21 cannot be
/// asked.
fn change_ops(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    if let Some(installed) =
        crate::installed_class(py, "music21.alpha.analysis.aligner", "ChangeOps")
    {
        return Ok(installed);
    }
    if let Ok(found) = py
        .import("music21.alpha.analysis.aligner")
        .and_then(|module| module.getattr("ChangeOps"))
    {
        return Ok(found);
    }
    Ok(py.get_type::<ChangeOps>().into_any())
}

/// What music21 numbers a change.
fn index_of(op: ChangeOp) -> i64 {
    match op {
        ChangeOp::Insertion => 0,
        ChangeOp::Deletion => 1,
        ChangeOp::Substitution => 2,
        ChangeOp::NoChange => 3,
    }
}

/// One of music21's four changes, as music21 holds it.
fn op_object(py: Python<'_>, op: ChangeOp) -> PyResult<Bound<'_, PyAny>> {
    change_ops(py)?.call1((index_of(op),))
}

/// One value of a hash, read as the crate holds it.
fn value_read(value: &Bound<'_, PyAny>) -> PyResult<HashValue> {
    if value.is_none() {
        return Ok(HashValue::None);
    }
    if value.is_instance_of::<pyo3::types::PyBool>() {
        return Ok(HashValue::Integer(i64::from(value.extract::<bool>()?)));
    }
    if value.is_instance_of::<pyo3::types::PyInt>() {
        return Ok(HashValue::Integer(value.extract()?));
    }
    if value.is_instance_of::<pyo3::types::PyFloat>() {
        return Ok(HashValue::Float(value.extract()?));
    }
    if value.is_instance_of::<pyo3::types::PyString>() {
        return Ok(HashValue::Text(value.extract()?));
    }
    Err(AlignerException::new_err(format!(
        "a hash saying {} cannot be read",
        value.repr()?
    )))
}

/// A hashed note read as the crate holds one: its keys, and the value it
/// says for each, which is how music21 reads one too.
fn hash_read(tup: &Bound<'_, PyAny>) -> PyResult<NoteHash> {
    let keys: Vec<String> = tup
        .getattr("hashItemsKeys")
        .map_err(|_| {
            AlignerException::new_err(
                "a hash that does not say what its values are cannot be aligned".to_string(),
            )
        })?
        .extract()?;
    let mut values = Vec::new();
    for key in &keys {
        values.push(value_read(&tup.getattr(key.as_str())?)?);
    }
    Ok(NoteHash {
        keys,
        values,
        reference: None,
    })
}

/// A whole hashed stream read as the crate holds one.
fn hashes_read(hashed: &Bound<'_, PyAny>) -> PyResult<Vec<NoteHash>> {
    let mut out = Vec::new();
    for hash in hashed.walk()? {
        out.push(hash_read(&hash?)?);
    }
    Ok(out)
}

/// music21's `alpha.analysis.aligner.StreamAligner`: two streams and the
/// cheapest edit from one to the other.
#[pyclass(
    name = "StreamAligner",
    module = "music21.alpha.analysis.aligner",
    subclass,
    skip_from_py_object
)]
pub struct StreamAligner {
    /// The stream being aligned to.
    #[pyo3(get, set)]
    targetStream: Py<PyAny>,
    /// The stream being aligned.
    #[pyo3(get, set)]
    sourceStream: Py<PyAny>,
    /// The cost of the cheapest edit onto every opening of the two.
    #[pyo3(get, set)]
    distanceMatrix: Py<PyAny>,
    /// How the notes are hashed.
    #[pyo3(get, set)]
    hasher: Py<PyAny>,
    /// Whether the two streams are lists of hashes already.
    #[pyo3(get, set)]
    preHashed: bool,
    /// The changes, in order.
    #[pyo3(get, set)]
    changes: Py<PyAny>,
    /// The share of the changes that change nothing.
    #[pyo3(get, set)]
    similarityScore: Py<PyAny>,
    /// How many hashes the target has.
    #[pyo3(get, set)]
    n: usize,
    /// How many hashes the source has.
    #[pyo3(get, set)]
    m: usize,
    /// The target's hashes.
    #[pyo3(get, set)]
    hashedTargetStream: Py<PyAny>,
    /// The source's hashes.
    #[pyo3(get, set)]
    hashedSourceStream: Py<PyAny>,
    /// How many changes of each kind there are.
    #[pyo3(get, set)]
    changesCount: Py<PyAny>,
}

impl StreamAligner {
    /// The matrix as the crate reads one, out of whatever numpy holds.
    fn matrix_read(&self, py: Python<'_>) -> PyResult<Vec<Vec<i64>>> {
        let matrix = self.distanceMatrix.bind(py);
        if matrix.is_none() {
            return Err(AlignerException::new_err(
                "Cannot read a distance matrix that has not been set up.".to_string(),
            ));
        }
        matrix.call_method0("tolist")?.extract()
    }

    /// The two streams' hashes, as the crate reads them.
    fn hashed(&self, py: Python<'_>) -> PyResult<(Vec<NoteHash>, Vec<NoteHash>)> {
        Ok((
            hashes_read(self.hashedTargetStream.bind(py))?,
            hashes_read(self.hashedSourceStream.bind(py))?,
        ))
    }

    /// The parts a copy or a pickle carries.
    fn state<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyDict>> {
        let py = slf.py();
        let me = slf.borrow();
        let state = PyDict::new(py);
        state.set_item("targetStream", me.targetStream.bind(py))?;
        state.set_item("sourceStream", me.sourceStream.bind(py))?;
        state.set_item("distanceMatrix", me.distanceMatrix.bind(py))?;
        state.set_item("hasher", me.hasher.bind(py))?;
        state.set_item("preHashed", me.preHashed)?;
        state.set_item("changes", me.changes.bind(py))?;
        state.set_item("similarityScore", me.similarityScore.bind(py))?;
        state.set_item("n", me.n)?;
        state.set_item("m", me.m)?;
        state.set_item("hashedTargetStream", me.hashedTargetStream.bind(py))?;
        state.set_item("hashedSourceStream", me.hashedSourceStream.bind(py))?;
        state.set_item("changesCount", me.changesCount.bind(py))?;
        Ok(state)
    }

    /// A copy of the aligner, everything it holds copied too where a deep
    /// copy asks for it.
    fn copied(
        slf: &Bound<'_, Self>,
        memo: Option<Option<&Bound<'_, PyAny>>>,
    ) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let carry = |held: &Py<PyAny>| -> PyResult<Py<PyAny>> {
            let Some(memo) = memo else {
                return Ok(held.clone_ref(py));
            };
            Ok(py
                .import("copy")?
                .call_method1("deepcopy", (held.bind(py), memo))?
                .unbind())
        };
        let value = {
            let me = slf.borrow();
            Self {
                targetStream: carry(&me.targetStream)?,
                sourceStream: carry(&me.sourceStream)?,
                distanceMatrix: carry(&me.distanceMatrix)?,
                hasher: carry(&me.hasher)?,
                preHashed: me.preHashed,
                changes: carry(&me.changes)?,
                similarityScore: carry(&me.similarityScore)?,
                n: me.n,
                m: me.m,
                hashedTargetStream: carry(&me.hashedTargetStream)?,
                hashedSourceStream: carry(&me.hashedSourceStream)?,
                changesCount: carry(&me.changesCount)?,
            }
        };
        Ok(crate::copy_as_same_type(slf, value)?.unbind())
    }
}

#[pymethods]
impl StreamAligner {
    /// music21's
    /// `StreamAligner(targetStream, sourceStream, hasher_func, preHashed)`.
    #[new]
    #[pyo3(signature = (targetStream = None, sourceStream = None, hasher_func = None, preHashed = false))]
    fn new(
        py: Python<'_>,
        targetStream: Option<Py<PyAny>>,
        sourceStream: Option<Py<PyAny>>,
        hasher_func: Option<Py<PyAny>>,
        preHashed: bool,
    ) -> PyResult<Self> {
        let hasher = match hasher_func {
            Some(given) if !given.bind(py).is_none() => given,
            _ => default_hasher(py)?.unbind(),
        };
        Ok(Self {
            targetStream: targetStream.unwrap_or_else(|| py.None()),
            sourceStream: sourceStream.unwrap_or_else(|| py.None()),
            distanceMatrix: py.None(),
            hasher,
            preHashed,
            changes: PyList::empty(py).into_any().unbind(),
            similarityScore: 0_i64.into_pyobject(py)?.into_any().unbind(),
            n: 0,
            m: 0,
            hashedTargetStream: py.None(),
            hashedSourceStream: py.None(),
            changesCount: py.None(),
        })
    }

    /// A hasher that hashes no offset and says what each hash was made
    /// from: music21's `getDefaultHasher`.
    fn getDefaultHasher(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(default_hasher(py)?.unbind())
    }

    /// The whole alignment: music21's `align`, which is its four steps in
    /// order.
    fn align(slf: &Bound<'_, Self>) -> PyResult<()> {
        slf.call_method0("makeHashedStreams")?;
        slf.call_method0("setupDistanceMatrix")?;
        slf.call_method0("populateDistanceMatrix")?;
        slf.call_method0("calculateChangesList")?;
        Ok(())
    }

    /// The two streams hashed, unless they are hashes already: music21's
    /// `makeHashedStreams`.
    fn makeHashedStreams(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let (target, source, hasher, pre) = {
            let me = slf.borrow();
            (
                me.targetStream.clone_ref(py),
                me.sourceStream.clone_ref(py),
                me.hasher.clone_ref(py),
                me.preHashed,
            )
        };
        let (hashed_target, hashed_source) = if pre {
            (target, source)
        } else {
            let hasher = hasher.bind(py);
            (
                hasher.call_method1("hashStream", (target,))?.unbind(),
                hasher.call_method1("hashStream", (source,))?.unbind(),
            )
        };
        let mut me = slf.borrow_mut();
        me.hashedTargetStream = hashed_target;
        me.hashedSourceStream = hashed_source;
        Ok(())
    }

    /// A matrix of the right size, every cost nought: music21's
    /// `setupDistanceMatrix`.
    fn setupDistanceMatrix(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        if slf.borrow().hashedTargetStream.bind(py).is_none() {
            slf.call_method0("makeHashedStreams")?;
        }
        let (n, m) = {
            let me = slf.borrow();
            (
                me.hashedTargetStream.bind(py).len()?,
                me.hashedSourceStream.bind(py).len()?,
            )
        };
        setup_distance_matrix(n, m).map_err(aligner_error)?;
        let Ok(numpy) = py.import("numpy") else {
            return Err(AlignerException::new_err(
                "Cannot run Aligner without numpy.".to_string(),
            ));
        };
        let shape = PyTuple::new(py, [n + 1, m + 1])?;
        let keywords = PyDict::new(py);
        keywords.set_item("dtype", numpy.getattr("int_")?)?;
        let matrix = numpy.call_method("zeros", (shape,), Some(&keywords))?;
        let mut me = slf.borrow_mut();
        me.n = n;
        me.m = m;
        me.distanceMatrix = matrix.unbind();
        Ok(())
    }

    /// Every cost of the matrix worked out: music21's
    /// `populateDistanceMatrix`.
    fn populateDistanceMatrix(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let (mut matrix, target, source, held) = {
            let me = slf.borrow();
            let (target, source) = me.hashed(py)?;
            (
                me.matrix_read(py)?,
                target,
                source,
                me.distanceMatrix.clone_ref(py),
            )
        };
        fill_distance_matrix(&mut matrix, &target, &source).map_err(aligner_error)?;
        let held = held.bind(py);
        for (i, row) in matrix.iter().enumerate() {
            for (j, cost) in row.iter().enumerate() {
                held.set_item(PyTuple::new(py, [i, j])?, cost)?;
            }
        }
        Ok(())
    }

    /// What the three moves onto a cell cost, each nothing where that cell
    /// is off the matrix: music21's `getPossibleMovesFromLocation`.
    fn getPossibleMovesFromLocation<'py>(
        slf: &Bound<'py, Self>,
        i: usize,
        j: usize,
    ) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let matrix = slf.borrow().matrix_read(py)?;
        PyList::new(py, moves_from_location(&matrix, i, j))
    }

    /// Which change leads onto a cell: music21's `getOpFromLocation`.
    fn getOpFromLocation(slf: &Bound<'_, Self>, i: usize, j: usize) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let matrix = slf.borrow().matrix_read(py)?;
        let op = op_from_location(&matrix, i, j).map_err(aligner_error)?;
        Ok(op_object(py, op)?.unbind())
    }

    /// What it costs to put a note in: music21's `insertCost`.
    fn insertCost(&self, tup: &Bound<'_, PyAny>) -> PyResult<i64> {
        Ok(insert_cost(&hash_read(tup)?))
    }

    /// What it costs to take a note out: music21's `deleteCost`.
    fn deleteCost(&self, tup: &Bound<'_, PyAny>) -> PyResult<i64> {
        Ok(delete_cost(&hash_read(tup)?))
    }

    /// What it costs to turn one note into another: music21's
    /// `substitutionCost`.
    fn substitutionCost(
        &self,
        targetTup: &Bound<'_, PyAny>,
        sourceTup: &Bound<'_, PyAny>,
    ) -> PyResult<i64> {
        Ok(substitution_cost(
            &hash_read(targetTup)?,
            &hash_read(sourceTup)?,
        ))
    }

    /// How many of two hashes' values say the same: music21's
    /// `calculateNumSimilarities`.
    fn calculateNumSimilarities(
        &self,
        targetTup: &Bound<'_, PyAny>,
        sourceTup: &Bound<'_, PyAny>,
    ) -> PyResult<usize> {
        Ok(num_similarities(
            &hash_read(targetTup)?,
            &hash_read(sourceTup)?,
        ))
    }

    /// Whether two hashes say the same of a note, whatever each was made
    /// from: music21's `tupleEqualityWithoutReference`.
    fn tupleEqualityWithoutReference(
        &self,
        tup1: &Bound<'_, PyAny>,
        tup2: &Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        Ok(hashes_equal_without_reference(
            &hash_read(tup1)?,
            &hash_read(tup2)?,
        ))
    }

    /// The edit read back out of the matrix: music21's
    /// `calculateChangesList`, each change between one note of each side.
    fn calculateChangesList(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let (matrix, n, m, hashed_target, hashed_source, changes) = {
            let me = slf.borrow();
            (
                me.matrix_read(py)?,
                me.n,
                me.m,
                me.hashedTargetStream.clone_ref(py),
                me.hashedSourceStream.clone_ref(py),
                me.changes.clone_ref(py),
            )
        };
        let places = change_places(&matrix, n, m).map_err(aligner_error)?;
        let hashed_target = hashed_target.bind(py);
        let hashed_source = hashed_source.bind(py);
        let made = PyList::empty(py);
        for place in &places {
            let target = hashed_target.get_item(place.target)?.getattr("reference")?;
            let source = hashed_source.get_item(place.source)?.getattr("reference")?;
            made.append(PyTuple::new(
                py,
                [target, source, op_object(py, place.op)?],
            )?)?;
        }
        // music21 puts each change it reads at the head of the list it
        // already had, so a second reading goes in front of the first.
        made.call_method1("extend", (changes.bind(py),))?;
        let ops = PyList::empty(py);
        for change in made.try_iter()? {
            ops.append(change?.get_item(2)?)?;
        }
        let counted = py
            .import("collections")?
            .getattr("Counter")?
            .call1((ops,))?;
        let unchanged = counted
            .get_item(op_object(py, ChangeOp::NoChange)?)?
            .extract::<f64>()?;
        let score = unchanged / made.len() as f64;
        let mut me = slf.borrow_mut();
        me.changes = made.into_any().unbind();
        me.changesCount = counted.unbind();
        me.similarityScore = score.into_pyobject(py)?.into_any().unbind();
        Ok(())
    }

    /// Each changed note coloured and numbered on both streams, and the
    /// two named: music21's `showChanges`.
    #[pyo3(signature = (show = false))]
    fn showChanges(slf: &Bound<'_, Self>, show: bool) -> PyResult<()> {
        let py = slf.py();
        let (changes, target, source) = {
            let me = slf.borrow();
            (
                me.changes.clone_ref(py),
                me.targetStream.clone_ref(py),
                me.sourceStream.clone_ref(py),
            )
        };
        for (at, change) in changes.bind(py).try_iter()?.enumerate() {
            let change = change?;
            let color = change.get_item(2)?.getattr("color")?;
            if color.is_none() {
                continue;
            }
            for which in [0_usize, 1] {
                let note = change.get_item(which)?;
                note.getattr("style")?.setattr("color", &color)?;
                note.call_method1("addLyric", (at,))?;
            }
        }
        let class = metadata_class(py)?;
        let target = target.bind(py);
        let source = source.bind(py);
        target.setattr("metadata", class.call0()?)?;
        source.setattr("metadata", class.call0()?)?;
        // music21 names the source after the *target's* id as well.
        let named = format!("{}", target.getattr("id")?);
        for (stream, name) in [
            (target, format!("Target {named}")),
            (source, format!("Source {named}")),
        ] {
            let metadata = stream.getattr("metadata")?;
            metadata.setattr("title", &name)?;
            metadata.setattr("movementName", &name)?;
        }
        if show {
            target.call_method0("show")?;
            source.call_method0("show")?;
        }
        Ok(())
    }

    /// A copy holding what this one holds.
    fn __copy__(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        Self::copied(slf, None)
    }

    /// The same with everything held copied too, through the copier's
    /// memo.
    #[pyo3(signature = (memo = None))]
    fn __deepcopy__(slf: &Bound<'_, Self>, memo: Option<&Bound<'_, PyAny>>) -> PyResult<Py<PyAny>> {
        Self::copied(slf, Some(memo))
    }

    /// What a pickle writes: the class and the parts, as every facade
    /// here pickles itself rather than leaving it to music21.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let made = PyTuple::new(
            py,
            [
                slf.get_type().into_any(),
                PyTuple::empty(py).into_any(),
                Self::state(slf)?.into_any(),
            ],
        )?;
        Ok(made.into_any().unbind())
    }

    /// The parts read back, music21's own dictionary state taken as
    /// readily as the one written here.
    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let read = |name: &str| state.get_item(name).ok();
        let mut me = slf.borrow_mut();
        if let Some(value) = read("targetStream") {
            me.targetStream = value.unbind();
        }
        if let Some(value) = read("sourceStream") {
            me.sourceStream = value.unbind();
        }
        if let Some(value) = read("distanceMatrix") {
            me.distanceMatrix = value.unbind();
        }
        if let Some(value) = read("hasher") {
            me.hasher = value.unbind();
        }
        if let Some(value) = read("changes") {
            me.changes = value.unbind();
        }
        if let Some(value) = read("similarityScore") {
            me.similarityScore = value.unbind();
        }
        if let Some(value) = read("hashedTargetStream") {
            me.hashedTargetStream = value.unbind();
        }
        if let Some(value) = read("hashedSourceStream") {
            me.hashedSourceStream = value.unbind();
        }
        if let Some(value) = read("changesCount") {
            me.changesCount = value.unbind();
        }
        if let Some(value) = read("preHashed") {
            me.preHashed = value.is_truthy()?;
        }
        if let Some(value) = read("n") {
            me.n = value.extract()?;
        }
        if let Some(value) = read("m") {
            me.m = value.extract()?;
        }
        Ok(())
    }
}

/// music21's `Metadata`, or whichever class stands in for it.
fn metadata_class(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    if let Some(installed) = crate::installed_class(py, "music21.metadata", "Metadata") {
        return Ok(installed);
    }
    if let Ok(found) = py
        .import("music21.metadata")
        .and_then(|module| module.getattr("Metadata"))
    {
        return Ok(found);
    }
    Ok(py.get_type::<crate::metadata::Metadata>().into_any())
}

/// A hasher of music21's default settings for an alignment.
fn default_hasher(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let class = crate::installed_class(py, "music21.alpha.analysis.hasher", "Hasher")
        .unwrap_or_else(|| py.get_type::<crate::hasher::Hasher>().into_any());
    let made = class.call0()?;
    made.setattr("hashOffset", false)?;
    made.setattr("includeReference", true)?;
    Ok(made)
}

error_into!(pub(crate) aligner_error, AlignerException);

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ChangeOps>()?;
    m.add_class::<StreamAligner>()?;
    Ok(())
}
