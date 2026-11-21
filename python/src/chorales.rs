//! music21's `corpus.chorales`: Bach's chorales by catalogue number, and an
//! iterator over them.
//!
//! The catalogues are the crate's `corpus::chorales`; here they are
//! music21's dictionaries of dictionaries, and `Iterator` walks a range of
//! one catalogue, parsing each chorale from the corpus as it is reached.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::{PyIndexError, PyStopIteration};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyList, PySlice, PyString};

use music21::corpus::chorales::{bwv_sort_key, chorales, corpus_chorales};

/// The module's dotted name.
const MODULE: &str = "music21.corpus.chorales";

/// Where the corpus keeps the chorales, before each BWV number.
const BWV_PREFIX: &str = "bach/bwv";

pyo3::create_exception!(music21_rs_facade, BachException, crate::Music21Exception);

fn bach_error(message: impl Into<String>) -> PyErr {
    BachException::new_err(message.into())
}

/// The module, built once.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        let exception = py.get_type::<BachException>();
        exception.setattr("__module__", MODULE)?;
        m.add("BachException", exception)?;
        m.add_class::<ChoraleList>()?;
        m.add_class::<ChoraleListRKBWV>()?;
        m.add_class::<Iterator>()?;
        m.add_function(wrap_pyfunction!(getByTitle, &m)?)?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

/// Python's `print`, which music21 uses to say what it skips.
fn say(py: Python<'_>, message: String) -> PyResult<()> {
    py.import("builtins")?.call_method1("print", (message,))?;
    Ok(())
}

// ------------------------------------------------------------ catalogues

/// The chorales of BWV 250-438 in every catalogue: music21's `ChoraleList`.
#[pyclass(name = "ChoraleList", module = "music21.corpus.chorales")]
pub struct ChoraleList {
    #[pyo3(get)]
    byTitle: Py<PyDict>,
    #[pyo3(get)]
    byBWV: Py<PyDict>,
    #[pyo3(get)]
    byKalmus: Py<PyDict>,
    #[pyo3(get)]
    byBaerenreiter: Py<PyDict>,
    #[pyo3(get)]
    byBudapest: Py<PyDict>,
    #[pyo3(get)]
    byRiemenschneider: Py<PyDict>,
}

#[pymethods]
impl ChoraleList {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        for held in self.held() {
            visit.call(held)?;
        }
        Ok(())
    }

    #[new]
    fn new(py: Python<'_>) -> PyResult<Self> {
        let made = Self {
            byTitle: PyDict::new(py).unbind(),
            byBWV: PyDict::new(py).unbind(),
            byKalmus: PyDict::new(py).unbind(),
            byBaerenreiter: PyDict::new(py).unbind(),
            byBudapest: PyDict::new(py).unbind(),
            byRiemenschneider: PyDict::new(py).unbind(),
        };
        made.prepareList(py)?;
        Ok(made)
    }

    /// Fills the dictionaries from the catalogue.
    fn prepareList(&self, py: Python<'_>) -> PyResult<()> {
        for chorale in chorales() {
            let entry = PyDict::new(py);
            entry.set_item("title", &chorale.title)?;
            entry.set_item("bwv", chorale.bwv)?;
            entry.set_item("kalmus", chorale.kalmus)?;
            entry.set_item("baerenreiter", chorale.baerenreiter)?;
            entry.set_item("budapest", chorale.budapest)?;
            entry.set_item("riemenschneider", chorale.riemenschneider)?;
            entry.set_item("notes", chorale.notes.as_deref())?;
            self.byTitle.bind(py).set_item(&chorale.title, &entry)?;
            self.byBWV.bind(py).set_item(chorale.bwv, &entry)?;
            self.byKalmus.bind(py).set_item(chorale.kalmus, &entry)?;
            self.byBaerenreiter
                .bind(py)
                .set_item(chorale.baerenreiter, &entry)?;
            self.byBudapest
                .bind(py)
                .set_item(chorale.budapest, &entry)?;
            self.byRiemenschneider
                .bind(py)
                .set_item(chorale.riemenschneider, &entry)?;
        }
        Ok(())
    }
}

impl ChoraleList {
    fn held(&self) -> [&Py<PyDict>; 6] {
        [
            &self.byTitle,
            &self.byBWV,
            &self.byKalmus,
            &self.byBaerenreiter,
            &self.byBudapest,
            &self.byRiemenschneider,
        ]
    }
}

/// Every chorale of the corpus by Riemenschneider, BWV and Kalmus number:
/// music21's `ChoraleListRKBWV`.
#[pyclass(name = "ChoraleListRKBWV", module = "music21.corpus.chorales")]
pub struct ChoraleListRKBWV {
    #[pyo3(get)]
    byTitle: Py<PyDict>,
    #[pyo3(get)]
    byBWV: Py<PyDict>,
    #[pyo3(get)]
    byKalmus: Py<PyDict>,
    #[pyo3(get)]
    byRiemenschneider: Py<PyDict>,
}

#[pymethods]
impl ChoraleListRKBWV {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.byTitle)?;
        visit.call(&self.byBWV)?;
        visit.call(&self.byKalmus)?;
        visit.call(&self.byRiemenschneider)
    }

    #[new]
    fn new(py: Python<'_>) -> PyResult<Self> {
        let made = Self {
            byTitle: PyDict::new(py).unbind(),
            byBWV: PyDict::new(py).unbind(),
            byKalmus: PyDict::new(py).unbind(),
            byRiemenschneider: PyDict::new(py).unbind(),
        };
        made.prepareList(py)?;
        Ok(made)
    }

    /// Fills the dictionaries from the catalogue.
    fn prepareList(&self, py: Python<'_>) -> PyResult<()> {
        for chorale in corpus_chorales() {
            let entry = PyDict::new(py);
            entry.set_item("title", &chorale.title)?;
            entry.set_item("bwv", &chorale.bwv)?;
            entry.set_item("kalmus", chorale.kalmus)?;
            entry.set_item("riemenschneider", chorale.riemenschneider)?;
            self.byTitle.bind(py).set_item(&chorale.title, &entry)?;
            self.byBWV.bind(py).set_item(&chorale.bwv, &entry)?;
            self.byKalmus.bind(py).set_item(chorale.kalmus, &entry)?;
            self.byRiemenschneider
                .bind(py)
                .set_item(chorale.riemenschneider, &entry)?;
        }
        Ok(())
    }
}

// --------------------------------------------------------------- iterator

/// The catalogue an iterator walks, or the titles it was given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Numbering {
    Riemenschneider,
    Bwv,
    Kalmus,
    Budapest,
    Baerenreiter,
    Title,
}

impl Numbering {
    fn named(name: &str) -> Option<Self> {
        Some(match name {
            "riemenschneider" => Self::Riemenschneider,
            "bwv" => Self::Bwv,
            "kalmus" => Self::Kalmus,
            "budapest" => Self::Budapest,
            "baerenreiter" => Self::Baerenreiter,
            "title" => Self::Title,
            _ => return None,
        })
    }

    fn name(self) -> &'static str {
        match self {
            Self::Riemenschneider => "riemenschneider",
            Self::Bwv => "bwv",
            Self::Kalmus => "kalmus",
            Self::Budapest => "budapest",
            Self::Baerenreiter => "baerenreiter",
            Self::Title => "title",
        }
    }
}

/// What the iterator answers for each chorale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Returned {
    Stream,
    Filename,
}

/// Whether the bounds are catalogue numbers or places in the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Bounds {
    Number,
    Index,
}

/// Which bound is being set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Edge {
    Current,
    Highest,
}

/// Bach's chorales of a range of one catalogue, each parsed as it is
/// reached: music21's `corpus.chorales.Iterator`.
#[pyclass(name = "Iterator", module = "music21.corpus.chorales")]
pub struct Iterator {
    currentIndex: Option<i64>,
    highestIndex: Option<i64>,
    titles: Vec<String>,
    /// The numbers walked; None for titles.
    numbers: Option<Py<PyList>>,
    numbering: Numbering,
    returned: Returned,
    bounds: Bounds,
    #[pyo3(get, set)]
    analysis: bool,
    #[pyo3(get)]
    _choraleList1: Py<ChoraleList>,
    #[pyo3(get)]
    _choraleList2: Py<ChoraleListRKBWV>,
}

#[pymethods]
impl Iterator {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.numbers)?;
        visit.call(&self._choraleList1)?;
        visit.call(&self._choraleList2)
    }

    #[new]
    #[pyo3(signature = (
        currentNumber = None,
        highestNumber = None,
        *,
        numberingSystem = "riemenschneider",
        returnType = "stream",
        iterationType = "number",
        analysis = false,
        numberList = None,
        titleList = None,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        currentNumber: Option<Bound<'_, PyAny>>,
        highestNumber: Option<Bound<'_, PyAny>>,
        numberingSystem: &str,
        returnType: &str,
        iterationType: &str,
        analysis: bool,
        numberList: Option<Bound<'_, PyAny>>,
        titleList: Option<Bound<'_, PyAny>>,
    ) -> PyResult<Py<Self>> {
        let made = Py::new(
            py,
            Self {
                currentIndex: None,
                highestIndex: None,
                titles: Vec::new(),
                numbers: None,
                numbering: Numbering::Riemenschneider,
                returned: Returned::Stream,
                bounds: Bounds::Number,
                analysis,
                _choraleList1: Py::new(py, ChoraleList::new(py)?)?,
                _choraleList2: Py::new(py, ChoraleListRKBWV::new(py)?)?,
            },
        )?;

        // In music21's order: the catalogue first, the bounds last.
        let me = made.bind(py);
        me.setattr("numberingSystem", numberingSystem)?;
        me.setattr("returnType", returnType)?;
        me.setattr("iterationType", iterationType)?;
        if let Some(numbers) = numberList.filter(|given| !given.is_none()) {
            me.setattr("numberList", numbers)?;
        }
        if let Some(titles) = titleList.filter(|given| !given.is_none()) {
            me.setattr("titleList", titles)?;
        }
        me.setattr("currentNumber", currentNumber)?;
        me.setattr("highestNumber", highestNumber)?;
        Ok(made)
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __len__(&self, py: Python<'_>) -> usize {
        match &self.numbers {
            _ if self.numbering == Numbering::Title => self.titles.len(),
            Some(numbers) => numbers.bind(py).len(),
            None => 0,
        }
    }

    fn __next__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let (current, highest) = {
            let me = slf.borrow();
            (me.currentIndex.unwrap_or(0), me.highestIndex.unwrap_or(-1))
        };
        if current > highest {
            return Err(PyStopIteration::new_err(()));
        }
        let chorale = Self::_returnChorale(slf, None)?;
        slf.borrow_mut().currentIndex = Some(current + 1);
        Ok(chorale)
    }

    fn __deepcopy__(&self, py: Python<'_>, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        let numbers = match &self.numbers {
            Some(numbers) => Some(PyList::new(py, numbers.bind(py).iter())?.unbind()),
            None => None,
        };
        Ok(Self {
            currentIndex: self.currentIndex,
            highestIndex: self.highestIndex,
            titles: self.titles.clone(),
            numbers,
            numbering: self.numbering,
            returned: self.returned,
            bounds: self.bounds,
            analysis: self.analysis,
            _choraleList1: self._choraleList1.clone_ref(py),
            _choraleList2: self._choraleList2.clone_ref(py),
        })
    }

    /// A slice is a copy over that range; a key is a chorale, by its place
    /// or, iterating by number, by its number.
    fn __getitem__<'py>(
        slf: &Bound<'py, Self>,
        key: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        if let Ok(slice) = key.cast::<PySlice>() {
            let copied = py.import("copy")?.call_method1("deepcopy", (slf,))?;
            copied.setattr("currentNumber", slice.getattr("start")?)?;
            copied.setattr("highestNumber", slice.getattr("stop")?)?;
            return Ok(copied);
        }
        let (numbering, bounds, length) = {
            let me = slf.borrow();
            (me.numbering, me.bounds, me.__len__(py))
        };
        if numbering == Numbering::Title {
            if !in_range(key, length)? {
                return Err(PyIndexError::new_err(format!(
                    "{key} is not in the range of the titleList."
                )));
            }
            return Self::_returnChorale(slf, Some(key.extract()?));
        }
        if bounds == Bounds::Index || numbering == Numbering::Bwv {
            if !in_range(key, length)? {
                return Err(PyIndexError::new_err(format!(
                    "{key} is not in the range of the numberList."
                )));
            }
            return Self::_returnChorale(slf, Some(key.extract()?));
        }
        if !slf.borrow().number_list(py).contains(key)? {
            return Err(PyIndexError::new_err(format!(
                "{key} is not in the numberList"
            )));
        }
        Self::_returnChorale(slf, Some(key.extract()?))
    }

    /// The chorale at a place in the list, the current one where none is
    /// given: parsed, or its corpus name.
    #[pyo3(signature = (choraleIndex = None))]
    fn _returnChorale<'py>(
        slf: &Bound<'py, Self>,
        choraleIndex: Option<i64>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let (filename, title) = slf.borrow().chosen(py, choraleIndex)?;
        let me = slf.borrow();
        if me.returned == Returned::Filename {
            return Ok(PyString::new(py, &filename).into_any());
        }

        let corpus = crate::corpus::module(py)?;
        let chorale = corpus.call_method1("parse", (&filename,))?;
        let current = me.currentIndex.unwrap_or(0);
        if me.numbering == Numbering::Riemenschneider && me.analysis {
            // The analysis, where there is one, as a part; none is no error.
            let name = format!(
                "bach/choraleAnalyses/riemenschneider{:03}.rntxt",
                current + 1
            );
            if let Ok(analysis) = corpus.call_method1("parse", (name,))
                && !analysis.is_none()
            {
                let _ = analysis
                    .getattr("parts")
                    .and_then(|parts| parts.get_item(0))
                    .and_then(|part| chorale.call_method1("insert", (0, part)));
            }
        }

        // The German title, where the parser gives the BWV number.
        if chorale.getattr("metadata")?.is_none() {
            let metadata = py
                .import("music21.metadata")?
                .getattr("Metadata")?
                .call0()?;
            chorale.setattr("metadata", metadata)?;
        }
        let metadata = chorale.getattr("metadata")?;
        metadata.setattr("title", title)?;
        metadata.setattr("number", current + 1)?;
        Ok(chorale)
    }

    /// A BWV number as a number to sort by.
    #[staticmethod]
    fn _bwvSort(bwv: &str) -> f64 {
        bwv_sort_key(bwv)
    }

    /// Makes the list of numbers walked from the catalogue, and sets the
    /// bounds to its first and last.
    fn _initializeNumberList(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let numbering = slf.borrow().numbering;
        if numbering == Numbering::Title {
            let last = slf.borrow().titles.len() as i64 - 1;
            slf.borrow_mut().numbers = None;
            slf.setattr("currentNumber", 0)?;
            slf.setattr("highestNumber", last.max(0))?;
            return Ok(());
        }

        let numbers = slf.borrow().catalogue_numbers(py)?;
        slf.borrow_mut().numbers = Some(numbers.clone().unbind());
        if slf.borrow().bounds == Bounds::Number {
            slf.setattr("currentNumber", numbers.get_item(0)?)?;
            slf.setattr("highestNumber", numbers.get_item(numbers.len() - 1)?)?;
        } else {
            slf.setattr("currentNumber", 0)?;
            slf.setattr("highestNumber", numbers.len() as i64 - 1)?;
        }
        Ok(())
    }

    /// The numbers walked, None for titles.
    #[getter]
    fn _numberList<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyList>> {
        self.numbers
            .as_ref()
            .map(|numbers| numbers.bind(py).clone())
    }

    /// The catalogue walked: `bwv`, `kalmus`, `baerenreiter`, `budapest`,
    /// `riemenschneider`, or `title` for the chorales of `titleList`.
    #[getter]
    fn get_numberingSystem(&self) -> &'static str {
        self.numbering.name()
    }

    #[setter]
    fn set_numberingSystem(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let named = value
            .extract::<String>()
            .ok()
            .and_then(|name| Numbering::named(&name));
        let Some(numbering) = named else {
            return Err(bach_error(format!(
                "{value} is not a valid numbering system for Bach Chorales."
            )));
        };
        slf.borrow_mut().numbering = numbering;
        if numbering == Numbering::Title {
            return slf.setattr("titleList", PyList::empty(slf.py()));
        }
        Self::_initializeNumberList(slf)
    }

    /// The titles walked when the numbering system is `title`.
    #[getter]
    fn get_titleList<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        PyList::new(py, &self.titles)
    }

    #[setter]
    fn set_titleList(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        if value.is_instance_of::<PyString>() || value.try_iter().is_err() {
            return Err(bach_error(format!("{} must be a list.", value.repr()?)));
        }
        let known = slf
            .borrow()
            ._choraleList2
            .bind(py)
            .borrow()
            .byTitle
            .clone_ref(py);
        let mut titles = Vec::new();
        for title in value.try_iter()? {
            let title = title?;
            if known.bind(py).contains(&title)? {
                titles.push(title.extract::<String>()?);
            } else {
                say(
                    py,
                    format!(
                        "{} will be skipped because it is not a recognized title.",
                        title.repr()?
                    ),
                )?;
            }
        }
        slf.borrow_mut().titles = titles;
        Self::_initializeNumberList(slf)
    }

    /// The catalogue numbers walked, sorted; a list given keeps only those
    /// in the catalogue.
    #[getter]
    fn get_numberList<'py>(&self, py: Python<'py>) -> Bound<'py, PyList> {
        self.number_list(py)
    }

    #[setter]
    fn set_numberList(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        if !value.is_instance_of::<PyList>() {
            return Err(bach_error(format!("{} must be a list.", value.repr()?)));
        }
        let numbering = slf.borrow().numbering;
        if numbering == Numbering::Title {
            slf.borrow_mut().numbers = None;
            return Err(bach_error(
                "Cannot set numberList when .numberingSystem == 'title'",
            ));
        }

        // The numbers given that the catalogue has, sorted.
        let catalogue = slf.borrow().catalogue(py)?;
        let kept = PyList::empty(py);
        let sorted = py.import("builtins")?.call_method1("sorted", (value,))?;
        for number in sorted.try_iter()? {
            let number = number?;
            let zero_kalmus = numbering == Numbering::Kalmus && number.eq(0)?;
            if catalogue.contains(&number)? && !zero_kalmus {
                kept.append(number)?;
            } else {
                say(
                    py,
                    format!(
                        "{} will be skipped because it is not in the numberingSystem {}",
                        number.str()?,
                        numbering.name()
                    ),
                )?;
            }
        }
        slf.borrow_mut().numbers = Some(kept.clone().unbind());
        if slf.borrow().bounds == Bounds::Number {
            slf.setattr("currentNumber", kept.get_item(0)?)?;
            slf.setattr(
                "highestNumber",
                kept.get_item(kept.len().saturating_sub(1))?,
            )?;
        } else {
            slf.setattr("currentNumber", 0)?;
            slf.setattr("highestNumber", kept.len() as i64 - 1)?;
        }
        Ok(())
    }

    /// The number of the next chorale, or its place when iterating by
    /// place or by title.
    #[getter]
    fn get_currentNumber<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.number_at(py, self.currentIndex)
    }

    #[setter]
    fn set_currentNumber(slf: &Bound<'_, Self>, value: Option<Bound<'_, PyAny>>) -> PyResult<()> {
        slf.borrow_mut().set_bound(slf.py(), Edge::Current, value)
    }

    /// The number of the last chorale walked, or its place when iterating
    /// by place or by title.
    #[getter]
    fn get_highestNumber<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.number_at(py, self.highestIndex)
    }

    #[setter]
    fn set_highestNumber(slf: &Bound<'_, Self>, value: Option<Bound<'_, PyAny>>) -> PyResult<()> {
        slf.borrow_mut().set_bound(slf.py(), Edge::Highest, value)
    }

    /// `stream`, to parse each chorale, or `filename`, to answer its name
    /// in the corpus.
    #[getter]
    fn get_returnType(&self) -> &'static str {
        match self.returned {
            Returned::Stream => "stream",
            Returned::Filename => "filename",
        }
    }

    #[setter]
    fn set_returnType(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.returned = match value.extract::<String>().unwrap_or_default().as_str() {
            "stream" => Returned::Stream,
            "filename" => Returned::Filename,
            _ => {
                return Err(bach_error(format!(
                    "{value} is not a proper returnType for this iterator. Only 'stream' and 'filename' are acceptable."
                )));
            }
        };
        Ok(())
    }

    /// `number`, for bounds that are catalogue numbers, or `index`, for
    /// places in the list.
    #[getter]
    fn get_iterationType(&self) -> &'static str {
        match self.bounds {
            Bounds::Number => "number",
            Bounds::Index => "index",
        }
    }

    #[setter]
    fn set_iterationType(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let bounds = match value.extract::<String>().unwrap_or_default().as_str() {
            "number" => Bounds::Number,
            "index" => Bounds::Index,
            _ => {
                return Err(bach_error(format!(
                    "{value} is not a proper iterationType for this iterator. Only 'number' and 'index' are acceptable."
                )));
            }
        };
        slf.borrow_mut().bounds = bounds;
        Self::_initializeNumberList(slf)
    }
}

impl Iterator {
    fn number_list<'py>(&self, py: Python<'py>) -> Bound<'py, PyList> {
        match &self.numbers {
            Some(numbers) => numbers.bind(py).clone(),
            None => PyList::empty(py),
        }
    }

    /// The dictionary of the catalogue walked.
    fn catalogue<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let wikipedia = self._choraleList1.bind(py).borrow();
        let greentree = self._choraleList2.bind(py).borrow();
        let found = match self.numbering {
            Numbering::Riemenschneider => &greentree.byRiemenschneider,
            Numbering::Kalmus => &greentree.byKalmus,
            Numbering::Bwv => &greentree.byBWV,
            Numbering::Title => &greentree.byTitle,
            Numbering::Budapest => &wikipedia.byBudapest,
            Numbering::Baerenreiter => &wikipedia.byBaerenreiter,
        };
        Ok(found.bind(py).clone())
    }

    /// Every number of the catalogue walked, sorted: BWV numbers as BWV
    /// numbers sort, and no Kalmus 0, which marks a chorale with none.
    fn catalogue_numbers<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let catalogue = self.catalogue(py)?;
        if self.numbering == Numbering::Bwv {
            let mut keys: Vec<String> = catalogue.keys().extract()?;
            keys.sort_by(|a, b| bwv_sort_key(a).total_cmp(&bwv_sort_key(b)));
            return PyList::new(py, keys);
        }
        let mut keys: Vec<i64> = catalogue.keys().extract()?;
        keys.sort_unstable();
        if self.numbering == Numbering::Kalmus {
            keys.retain(|key| *key != 0);
        }
        PyList::new(py, keys)
    }

    /// A bound as the number it is read as.
    fn number_at<'py>(&self, py: Python<'py>, index: Option<i64>) -> PyResult<Bound<'py, PyAny>> {
        let Some(index) = index else {
            return Ok(py.None().into_bound(py));
        };
        if self.bounds == Bounds::Index || self.numbering == Numbering::Title {
            return Ok(index.into_pyobject(py)?.into_any());
        }
        self.number_list(py).as_any().get_item(index)
    }

    /// Sets the current or highest bound, by number or by place as the
    /// iterator reads them, refusing one past the other.
    fn set_bound(
        &mut self,
        py: Python<'_>,
        which: Edge,
        value: Option<Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let value = value.filter(|value| !value.is_none());

        // Titles are walked by place, an empty list from 0.
        if self.numbering == Numbering::Title {
            if self.titles.is_empty() {
                self.store(which, 0);
                return Ok(());
            }
            let shown = match &value {
                Some(value) => value.str()?.to_string(),
                None => "None".to_string(),
            };
            let Some(value) =
                value.filter(|value| in_range(value, self.titles.len()).unwrap_or(false))
            else {
                return Err(bach_error(format!(
                    "{shown} is not an index in the range of the titleList"
                )));
            };
            let index: i64 = value.extract()?;
            return self.check_and_store(py, which, index);
        }

        // No bound is the first or the last.
        let numbers = self.number_list(py);
        let Some(value) = value else {
            let index = match which {
                Edge::Current => 0,
                Edge::Highest => numbers.len() as i64 - 1,
            };
            self.store(which, index);
            return Ok(());
        };
        let index = match self.bounds {
            Bounds::Number if numbers.contains(&value)? => numbers.index(&value)? as i64,
            Bounds::Index if in_range(&value, numbers.len())? => value.extract()?,
            _ => {
                return Err(bach_error(format!(
                    "{} does not correspond to a chorale in the {} numbering system",
                    value.str()?,
                    self.numbering.name()
                )));
            }
        };
        self.check_and_store(py, which, index)
    }

    fn check_and_store(&mut self, py: Python<'_>, which: Edge, index: i64) -> PyResult<()> {
        let titles = self.numbering == Numbering::Title;
        match which {
            Edge::Current if self.highestIndex.is_some_and(|highest| index > highest) => {
                let label = if titles {
                    "highestNumber"
                } else {
                    "HighestNumber"
                };
                let shown = if titles {
                    index.to_string()
                } else {
                    self.number_list(py)
                        .get_item(index as usize)?
                        .str()?
                        .to_string()
                };
                Err(bach_error(format!(
                    "{shown} is greater than the {label} {}",
                    self.number_at(py, self.highestIndex)?.str()?
                )))
            }
            Edge::Highest if self.currentIndex.is_some_and(|current| index < current) => {
                let label = if titles {
                    "currentNumber"
                } else {
                    "CurrentNumber"
                };
                let shown = if titles {
                    index.to_string()
                } else {
                    self.number_list(py)
                        .get_item(index as usize)?
                        .str()?
                        .to_string()
                };
                Err(bach_error(format!(
                    "{shown} is less than the {label} {}",
                    self.number_at(py, self.currentIndex)?.str()?
                )))
            }
            _ => {
                self.store(which, index);
                Ok(())
            }
        }
    }

    fn store(&mut self, which: Edge, index: i64) {
        match which {
            Edge::Current => self.currentIndex = Some(index),
            Edge::Highest => self.highestIndex = Some(index),
        }
    }

    /// The corpus name and the title of the chorale at a place.
    fn chosen(&self, py: Python<'_>, index: Option<i64>) -> PyResult<(String, String)> {
        let index = index.or(self.currentIndex).unwrap_or(0);
        if self.numbering == Numbering::Title {
            if self.titles.is_empty() {
                return Err(bach_error(
                    "Cannot parse Chorales because no titles to parse.",
                ));
            }
            let title = PyList::new(py, &self.titles)?
                .as_any()
                .get_item(index)?
                .extract::<String>()?;
            let entry = self.catalogue(py)?.as_any().get_item(&title)?;
            return Ok((
                format!("{BWV_PREFIX}{}", entry.get_item("bwv")?.str()?),
                title,
            ));
        }
        let number = self.number_list(py).as_any().get_item(index)?;
        if self.numbering == Numbering::Bwv {
            let number = number.str()?.to_string();
            return Ok((format!("{BWV_PREFIX}{number}"), number));
        }
        let entry = self.catalogue(py)?.as_any().get_item(&number)?;
        Ok((
            format!("{BWV_PREFIX}{}", entry.get_item("bwv")?.str()?),
            entry.get_item("title")?.extract()?,
        ))
    }
}

/// Whether a value is in `range(length)`, as Python reads it.
fn in_range(value: &Bound<'_, PyAny>, length: usize) -> PyResult<bool> {
    let py = value.py();
    py.import("builtins")?
        .getattr("range")?
        .call1((length,))?
        .contains(value)
}

/// The chorale whose title holds `title`, ignoring case, with its German
/// title, or None. Titles are looked through as `ChoraleList.byTitle`
/// holds them: a title listed twice answers its last chorale.
#[pyfunction]
fn getByTitle<'py>(py: Python<'py>, title: &str) -> PyResult<Bound<'py, PyAny>> {
    let wanted = title.to_lowercase();
    let listed = ChoraleList::new(py)?;
    let mut found = None;
    for (known, entry) in listed.byTitle.bind(py).iter() {
        let known: String = known.extract()?;
        if known.to_lowercase().contains(&wanted) {
            found = Some((known, entry.get_item("bwv")?.str()?.to_string()));
            break;
        }
    }
    let Some((found_title, bwv)) = found else {
        return Ok(py.None().into_bound(py));
    };
    let chorale =
        crate::corpus::module(py)?.call_method1("parse", (format!("{BWV_PREFIX}{bwv}"),))?;
    if chorale.getattr("metadata")?.is_none() {
        let metadata = py
            .import("music21.metadata")?
            .getattr("Metadata")?
            .call0()?;
        chorale.setattr("metadata", metadata)?;
    }
    chorale.getattr("metadata")?.setattr("title", found_title)?;
    Ok(chorale)
}
