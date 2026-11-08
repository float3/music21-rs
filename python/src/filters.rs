//! music21's `stream.filters`: the tests a stream iterator puts each
//! element to, deciding whether it is one of those asked for.
//!
//! A filter is called with an element and the iterator, and answers
//! whether the element is wanted; one may also raise `StopIteration` when
//! nothing later can be. Each class here is music21's, with its attributes
//! and `derivationStr`, and may be subclassed in Python as music21's are.
//! The offset filters decide with the crate's `stream::OffsetSpan`, the
//! same rule the crate's own `getElementsByOffset` keeps.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::PyClass;
use pyo3::exceptions::{PyAttributeError, PyStopIteration, PyTypeError};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyList, PyModule, PyTuple};

use music21::stream::OffsetSpan;

use crate::prebase::ProtoM21Object;

/// The module music21 keeps these in.
const MODULE: &str = "music21.stream.filters";

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

pyo3::create_exception!(music21_rs_facade, FilterException, crate::Music21Exception);

/// music21's `common.isListLike`: a list or a tuple, and nothing else.
fn is_list_like(value: &Bound<'_, PyAny>) -> bool {
    value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>()
}

/// music21's `StreamFilter`: a filter that wants everything. Its subclasses
/// say what they want.
#[pyclass(
    name = "StreamFilter",
    module = "music21.stream.filters",
    extends = ProtoM21Object,
    subclass,
    dict
)]
pub struct StreamFilter;

#[pymethods]
impl StreamFilter {
    #[new]
    #[pyo3(signature = (*_arguments, **_keywords))]
    fn new(
        _arguments: &Bound<'_, PyTuple>,
        _keywords: Option<&Bound<'_, pyo3::types::PyDict>>,
    ) -> PyClassInitializer<Self> {
        crate::base::built(Self)
    }

    /// What a stream derived through this filter says it came from.
    #[classattr]
    fn derivationStr() -> &'static str {
        "streamFilter"
    }

    /// Readies the filter for a new pass; here, nothing to ready.
    fn reset(&self) {}

    #[pyo3(signature = (item, iterator = None))]
    fn __call__(&self, item: &Bound<'_, PyAny>, iterator: Option<&Bound<'_, PyAny>>) -> bool {
        let _ = (item, iterator);
        true
    }
}

fn under_filter<T: PyClass<BaseType = StreamFilter>>(value: T) -> PyClassInitializer<T> {
    crate::base::built(StreamFilter).add_subclass(value)
}

/// music21's `IsFilter`: the elements that are one of these very objects.
/// It counts down as it finds them, and stops the iteration once it has
/// found them all.
#[pyclass(name = "IsFilter", module = "music21.stream.filters", extends = StreamFilter, subclass)]
pub struct IsFilter {
    /// What is looked for: a tuple or list of objects.
    #[pyo3(get, set)]
    target: Py<PyAny>,
    /// How many are still to be found.
    #[pyo3(get, set)]
    numToFind: Py<PyAny>,
}

impl IsFilter {
    fn made(py: Python<'_>, target: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let target = match target {
            Some(target) if is_list_like(target) => target.clone(),
            Some(target) => PyTuple::new(py, [target])?.into_any(),
            None => PyTuple::empty(py).into_any(),
        };
        let count = target.len()?;
        Ok(Self {
            target: target.unbind(),
            numToFind: count.into_pyobject(py)?.into_any().unbind(),
        })
    }

    /// Whether `item` is one looked for, counting it off where it is.
    fn found(slf: &Bound<'_, Self>, item: &Bound<'_, PyAny>) -> PyResult<bool> {
        let py = slf.py();
        let remaining = slf.borrow().numToFind.clone_ref(py);
        if remaining.bind(py).eq(0)? {
            return Err(PyStopIteration::new_err(()));
        }
        let target = slf.borrow().target.clone_ref(py);
        if !target.bind(py).contains(item)? {
            return Ok(false);
        }
        let left = remaining.bind(py).sub(1)?;
        slf.borrow_mut().numToFind = left.unbind();
        Ok(true)
    }
}

#[pymethods]
impl IsFilter {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.target)?;
        visit.call(&self.numToFind)?;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (target = None))]
    fn new(
        py: Python<'_>,
        target: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(under_filter(Self::made(py, target)?))
    }

    #[classattr]
    fn derivationStr() -> &'static str {
        "is"
    }

    /// Starts counting again from every object looked for.
    fn reset(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let count = slf.borrow().target.bind(py).len()?;
        slf.borrow_mut().numToFind = count.into_pyobject(py)?.into_any().unbind();
        Ok(())
    }

    #[pyo3(signature = (item, iterator = None))]
    fn __call__(
        slf: &Bound<'_, Self>,
        item: &Bound<'_, PyAny>,
        iterator: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        let _ = iterator;
        Self::found(slf, item)
    }
}

/// music21's `IsNotFilter`: everything but these very objects. There is
/// always more to find, so it never stops the iteration.
#[pyclass(name = "IsNotFilter", module = "music21.stream.filters", extends = IsFilter, subclass)]
pub struct IsNotFilter;

#[pymethods]
impl IsNotFilter {
    #[new]
    #[pyo3(signature = (target = None))]
    fn new(
        py: Python<'_>,
        target: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let mut base = IsFilter::made(py, target)?;
        base.numToFind = f64::INFINITY.into_pyobject(py)?.into_any().unbind();
        Ok(under_filter(base).add_subclass(Self))
    }

    #[classattr]
    fn derivationStr() -> &'static str {
        "isNot"
    }

    /// Nothing to reset: the count is infinite.
    fn reset(&self) {}

    #[pyo3(signature = (item, iterator = None))]
    fn __call__(
        slf: &Bound<'_, Self>,
        item: &Bound<'_, PyAny>,
        iterator: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        let _ = iterator;
        IsFilter::found(slf.as_super(), item).map(|found| !found)
    }
}

/// music21's `IdFilter`: the element of an id, matched without regard to
/// case where the ids are text. `getElementById` uses it.
#[pyclass(name = "IdFilter", module = "music21.stream.filters", extends = StreamFilter, subclass)]
pub struct IdFilter {
    /// The id looked for, lowercased where it is text.
    #[pyo3(get, set)]
    searchId: Py<PyAny>,
}

#[pymethods]
impl IdFilter {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.searchId)?;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (searchId = None))]
    fn new(
        py: Python<'_>,
        searchId: Option<Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let given = searchId.unwrap_or_else(|| py.None().into_bound(py));
        let searchId = lowered_or_same(&given)?;
        Ok(under_filter(Self {
            searchId: searchId.unbind(),
        }))
    }

    #[classattr]
    fn derivationStr() -> &'static str {
        "getElementById"
    }

    #[pyo3(signature = (item, iterator = None))]
    fn __call__(
        &self,
        py: Python<'_>,
        item: &Bound<'_, PyAny>,
        iterator: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        let _ = iterator;
        let id = item.getattr("id")?;
        let wanted = self.searchId.bind(py);
        if id.eq(wanted)? {
            return Ok(true);
        }
        match id.call_method0("lower") {
            Ok(lower) => lower.eq(wanted),
            Err(error) if error.is_instance_of::<PyAttributeError>(py) => Ok(false),
            Err(error) => Err(error),
        }
    }
}

/// `value.lower()`, or the value itself where it has no `lower`.
fn lowered_or_same<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    match value.call_method0("lower") {
        Ok(lower) => Ok(lower),
        Err(error) if error.is_instance_of::<PyAttributeError>(value.py()) => Ok(value.clone()),
        Err(error) => Err(error),
    }
}

/// music21's `ClassFilter`: the elements of any of these classes, named
/// or given as the classes themselves. `getElementsByClass` uses it.
#[pyclass(name = "ClassFilter", module = "music21.stream.filters", extends = StreamFilter, subclass)]
pub struct ClassFilter {
    /// The classes, as a tuple or a list.
    #[pyo3(get, set)]
    classList: Py<PyAny>,
}

impl ClassFilter {
    fn made<'py>(py: Python<'py>, classList: Option<&Bound<'py, PyAny>>) -> PyResult<Self> {
        let classList = match classList {
            Some(given) if is_list_like(given) => given.clone(),
            Some(given) => PyTuple::new(py, [given])?.into_any(),
            None => PyTuple::empty(py).into_any(),
        };
        Ok(Self {
            classList: classList.unbind(),
        })
    }

    /// Whether `item` is of none of the classes.
    fn none_of(&self, py: Python<'_>, item: &Bound<'_, PyAny>) -> PyResult<bool> {
        item.getattr("classSet")?
            .call_method1("isdisjoint", (self.classList.bind(py),))?
            .is_truthy()
    }
}

#[pymethods]
impl ClassFilter {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.classList)?;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (classList = None))]
    fn new(
        py: Python<'_>,
        classList: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(under_filter(Self::made(py, classList)?))
    }

    #[classattr]
    fn derivationStr() -> &'static str {
        "getElementsByClass"
    }

    /// Equal to another filter of the very same class with the same
    /// classes.
    fn __eq__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        if !other.get_type().is(slf.get_type()) {
            return Ok(false);
        }
        let mine = slf.borrow().classList.clone_ref(slf.py());
        mine.bind(slf.py()).eq(other.getattr("classList")?)
    }

    #[pyo3(signature = (item, iterator = None))]
    fn __call__(
        &self,
        py: Python<'_>,
        item: &Bound<'_, PyAny>,
        iterator: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        let _ = iterator;
        self.none_of(py, item).map(|none| !none)
    }

    /// The one class, or the whole list.
    fn _reprInternal(&self, py: Python<'_>) -> PyResult<String> {
        let classes = self.classList.bind(py);
        if classes.len()? == 1 {
            return Ok(classes.get_item(0)?.str()?.to_string());
        }
        Ok(classes.str()?.to_string())
    }
}

/// music21's `ClassNotFilter`: the elements of none of these classes.
#[pyclass(name = "ClassNotFilter", module = "music21.stream.filters", extends = ClassFilter, subclass)]
pub struct ClassNotFilter;

#[pymethods]
impl ClassNotFilter {
    #[new]
    #[pyo3(signature = (classList = None))]
    fn new(
        py: Python<'_>,
        classList: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(under_filter(ClassFilter::made(py, classList)?).add_subclass(Self))
    }

    #[classattr]
    fn derivationStr() -> &'static str {
        "getElementsNotOfClass"
    }

    #[pyo3(signature = (item, iterator = None))]
    fn __call__(
        slf: &Bound<'_, Self>,
        item: &Bound<'_, PyAny>,
        iterator: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        let _ = iterator;
        slf.as_super().borrow().none_of(slf.py(), item)
    }
}

/// music21's `GroupFilter`: the elements in any of these groups.
#[pyclass(name = "GroupFilter", module = "music21.stream.filters", extends = StreamFilter, subclass)]
pub struct GroupFilter {
    /// The groups' names.
    #[pyo3(get, set)]
    groupFilterList: Py<PyAny>,
}

#[pymethods]
impl GroupFilter {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.groupFilterList)?;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (groupFilterList = None))]
    fn new(
        py: Python<'_>,
        groupFilterList: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let groups = match groupFilterList {
            Some(given) if is_list_like(given) => given.clone(),
            Some(given) => PyList::new(py, [given])?.into_any(),
            None => PyTuple::empty(py).into_any(),
        };
        Ok(under_filter(Self {
            groupFilterList: groups.unbind(),
        }))
    }

    #[classattr]
    fn derivationStr() -> &'static str {
        "getElementsByGroup"
    }

    #[pyo3(signature = (item, iterator = None))]
    fn __call__(
        &self,
        py: Python<'_>,
        item: &Bound<'_, PyAny>,
        iterator: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        let _ = iterator;
        let groups = item.getattr("groups")?;
        for name in self.groupFilterList.bind(py).try_iter()? {
            if groups.contains(name?)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// music21's `OffsetFilter`: the elements standing in a span of offsets,
/// with music21's keywords for what its edges take. See music21's
/// `getElementsByOffset`.
#[pyclass(name = "OffsetFilter", module = "music21.stream.filters", extends = StreamFilter, subclass)]
pub struct OffsetFilter {
    #[pyo3(get, set)]
    offsetStart: Py<PyAny>,
    #[pyo3(get, set)]
    offsetEnd: Py<PyAny>,
    /// Whether the span has no length, so that every element of no
    /// length standing at it is taken.
    #[pyo3(get, set)]
    zeroLengthSearch: bool,
    #[pyo3(get, set)]
    mustFinishInSpan: bool,
    #[pyo3(get, set)]
    mustBeginInSpan: bool,
    #[pyo3(get, set)]
    includeEndBoundary: bool,
    #[pyo3(get, set)]
    includeElementsThatEndAtStart: bool,
    #[pyo3(get, set)]
    stopAfterEnd: bool,
}

/// A number as a float, as an offset is compared.
fn float_of(value: &Bound<'_, PyAny>) -> PyResult<f64> {
    value.call_method0("__float__")?.extract()
}

/// music21's `opFrac` of a number given in any form.
fn op_frac<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    crate::duration::op_frac(value.py(), float_of(value)?)
}

/// How long an element lasts: its duration's quarter length, and nothing
/// for one of the wheel's marks that keeps no duration, as music21's key
/// and time signatures last nothing.
fn quarter_length(e: &Bound<'_, PyAny>) -> PyResult<f64> {
    match e.getattr("duration") {
        Ok(duration) => float_of(&duration.getattr("quarterLength")?),
        Err(error) if error.is_instance_of::<PyAttributeError>(e.py()) => Ok(0.0),
        Err(error) => Err(error),
    }
}

impl OffsetFilter {
    /// The crate's span with this filter's edges.
    fn span(&self, py: Python<'_>) -> PyResult<OffsetSpan> {
        let start = float_of(self.offsetStart.bind(py))?;
        let end = float_of(self.offsetEnd.bind(py))?;
        let mut span = if self.zeroLengthSearch {
            OffsetSpan::at(start)
        } else {
            OffsetSpan::between(start, end)
        };
        span.include_end_boundary = self.includeEndBoundary;
        span.must_finish_in_span = self.mustFinishInSpan;
        span.must_begin_in_span = self.mustBeginInSpan;
        span.include_elements_that_end_at_start = self.includeElementsThatEndAtStart;
        Ok(span)
    }

    /// music21's `isElementOffsetInRange`, for an element standing at
    /// `offset`.
    fn in_range(
        &self,
        e: &Bound<'_, PyAny>,
        offset: &Bound<'_, PyAny>,
        stop_after_end: bool,
    ) -> PyResult<bool> {
        let py = e.py();
        if offset.gt(self.offsetEnd.bind(py))? {
            if stop_after_end {
                return Err(PyStopIteration::new_err(()));
            }
            return Ok(false);
        }
        let length = quarter_length(e)?;
        Ok(self.span(py)?.takes_at(float_of(offset)?, length))
    }
}

#[pymethods]
impl OffsetFilter {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.offsetStart)?;
        visit.call(&self.offsetEnd)?;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (
        offsetStart = None,
        offsetEnd = None,
        *,
        includeEndBoundary = true,
        mustFinishInSpan = false,
        mustBeginInSpan = true,
        includeElementsThatEndAtStart = true,
        stopAfterEnd = true
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        offsetStart: Option<&Bound<'_, PyAny>>,
        offsetEnd: Option<&Bound<'_, PyAny>>,
        includeEndBoundary: bool,
        mustFinishInSpan: bool,
        mustBeginInSpan: bool,
        includeElementsThatEndAtStart: bool,
        stopAfterEnd: bool,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(under_filter(Self::made(
            py,
            offsetStart,
            offsetEnd,
            [
                includeEndBoundary,
                mustFinishInSpan,
                mustBeginInSpan,
                includeElementsThatEndAtStart,
                stopAfterEnd,
            ],
        )?))
    }

    #[classattr]
    fn derivationStr() -> &'static str {
        "getElementsByOffset"
    }

    /// The offset, or the span from one to the other.
    fn _reprInternal(&self, py: Python<'_>) -> PyResult<String> {
        let start = self.offsetStart.bind(py).str()?.to_string();
        if self.zeroLengthSearch {
            return Ok(start);
        }
        Ok(format!("{start}-{}", self.offsetEnd.bind(py).str()?))
    }

    #[pyo3(signature = (e, iterator = None))]
    fn __call__(
        &self,
        e: &Bound<'_, PyAny>,
        iterator: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        let Some(iterator) = iterator.filter(|iterator| !iterator.is_none()) else {
            return self.in_range(e, &e.getattr("offset")?, false);
        };
        let source = iterator.getattr("srcStream")?;
        if source.is(e) {
            return Ok(false);
        }
        let offset = source.call_method1("elementOffset", (e,))?;
        // Never stop early on an unsorted stream: a later element may
        // stand earlier. The wheel's streams keep what they hold in
        // order, and say nothing of it.
        let sorted = match source.getattr("isSorted") {
            Ok(sorted) => sorted.is_truthy()?,
            Err(error) if error.is_instance_of::<PyAttributeError>(e.py()) => true,
            Err(error) => return Err(error),
        };
        let stop = self.stopAfterEnd && sorted;
        self.in_range(e, &offset, stop)
    }

    /// Whether an element standing at `offset` is in the span, raising
    /// `StopIteration` where it starts past the end and `stopAfterEnd`
    /// says nothing later can be in it.
    #[pyo3(signature = (e, offset, *, stopAfterEnd = false))]
    fn isElementOffsetInRange(
        &self,
        e: &Bound<'_, PyAny>,
        offset: &Bound<'_, PyAny>,
        stopAfterEnd: bool,
    ) -> PyResult<bool> {
        self.in_range(e, offset, stopAfterEnd)
    }
}

impl OffsetFilter {
    fn made(
        py: Python<'_>,
        start: Option<&Bound<'_, PyAny>>,
        end: Option<&Bound<'_, PyAny>>,
        flags: [bool; 5],
    ) -> PyResult<Self> {
        let nought = 0.0_f64.into_pyobject(py)?.into_any();
        let start_given = start.filter(|start| !start.is_none()).unwrap_or(&nought);
        let offsetStart = op_frac(start_given)?;
        let (offsetEnd, zeroLengthSearch) = match end.filter(|end| !end.is_none()) {
            None => (offsetStart.clone(), true),
            Some(end) => (op_frac(end)?, !end.gt(start_given)?),
        };
        let [include_end, must_finish, must_begin, end_at_start, stop] = flags;
        Ok(Self {
            offsetStart: offsetStart.unbind(),
            offsetEnd: offsetEnd.unbind(),
            zeroLengthSearch,
            mustFinishInSpan: must_finish,
            mustBeginInSpan: must_begin,
            includeEndBoundary: include_end,
            includeElementsThatEndAtStart: end_at_start,
            stopAfterEnd: stop,
        })
    }
}

/// music21's `OffsetHierarchyFilter`: the elements standing in a span of
/// offsets measured from the start of the outermost stream a recursive
/// iterator walks.
#[pyclass(
    name = "OffsetHierarchyFilter",
    module = "music21.stream.filters",
    extends = OffsetFilter,
    subclass
)]
pub struct OffsetHierarchyFilter;

#[pymethods]
impl OffsetHierarchyFilter {
    #[new]
    #[pyo3(signature = (
        offsetStart = None,
        offsetEnd = None,
        *,
        includeEndBoundary = true,
        mustFinishInSpan = false,
        mustBeginInSpan = true,
        includeElementsThatEndAtStart = true,
        stopAfterEnd = true
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        offsetStart: Option<&Bound<'_, PyAny>>,
        offsetEnd: Option<&Bound<'_, PyAny>>,
        includeEndBoundary: bool,
        mustFinishInSpan: bool,
        mustBeginInSpan: bool,
        includeElementsThatEndAtStart: bool,
        stopAfterEnd: bool,
    ) -> PyResult<PyClassInitializer<Self>> {
        let base = OffsetFilter::made(
            py,
            offsetStart,
            offsetEnd,
            [
                includeEndBoundary,
                mustFinishInSpan,
                mustBeginInSpan,
                includeElementsThatEndAtStart,
                stopAfterEnd,
            ],
        )?;
        Ok(under_filter(base).add_subclass(Self))
    }

    #[classattr]
    fn derivationStr() -> &'static str {
        "getElementsByOffsetInHierarchy"
    }

    #[pyo3(signature = (e, iterator = None))]
    fn __call__(
        slf: &Bound<'_, Self>,
        e: &Bound<'_, PyAny>,
        iterator: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        let Some(iterator) = iterator.filter(|iterator| !iterator.is_none()) else {
            return Err(PyTypeError::new_err(
                "Cannot call OffsetHierarchyFilter without an iterator",
            ));
        };
        let source = iterator.getattr("srcStream")?;
        if source.is(e) {
            return Ok(false);
        }
        if !iterator.hasattr("iteratorStartOffsetInHierarchy")? {
            return Err(FilterException::new_err(
                "Can only run OffsetHierarchyFilter on a RecursiveIterator",
            ));
        }
        let offset = source
            .call_method1("elementOffset", (e,))?
            .add(iterator.getattr("iteratorStartOffsetInHierarchy")?)?;
        let offset = op_frac(&offset)?;
        slf.as_super().borrow().in_range(e, &offset, false)
    }
}

/// music21's `stream.filters`, built.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        let exception = py.get_type::<FilterException>();
        exception.setattr("__module__", MODULE)?;
        m.add("FilterException", exception)?;
        m.add_class::<StreamFilter>()?;
        m.add_class::<IsFilter>()?;
        m.add_class::<IsNotFilter>()?;
        m.add_class::<IdFilter>()?;
        m.add_class::<ClassFilter>()?;
        m.add_class::<ClassNotFilter>()?;
        m.add_class::<GroupFilter>()?;
        m.add_class::<OffsetFilter>()?;
        m.add_class::<OffsetHierarchyFilter>()?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}
