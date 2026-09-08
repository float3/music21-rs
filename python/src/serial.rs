//! music21's `serial` module: `ToneRow`, `TwelveToneRow`,
//! `HistoricalTwelveToneRow`, `TwelveToneMatrix` and the module functions,
//! over `music21-rs`.
//!
//! music21's rows are streams of notes; the crate's is a list of pitch
//! classes. The facade keeps the three-class hierarchy, because doctests
//! print `type(row)` and `repr(row)`, and yields note-shaped objects when a
//! row is iterated, because they read `.pitch` and `.name` off the elements.

use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

use music21_rs::{
    HISTORICAL_ROWS, ToneRow as RsToneRow, Transformation, TransformationConvention,
    TwelveToneMatrix as RsMatrix, historical_row_by_name, row_to_matrix,
};

use crate::pitch::{Pitch, pitch_from_any};

/// The names the serial facade provides, for swapping into `music21.serial`.
pub const NAMES: &[&str] = &[
    "ToneRow",
    "TwelveToneRow",
    "HistoricalTwelveToneRow",
    "TwelveToneMatrix",
    "SerialException",
    "pcToToneRow",
    "rowToMatrix",
    "getHistoricalRowByName",
    "historicalDict",
];

pyo3::create_exception!(music21_rs_facade, SerialException, PyException);

fn serial_error(error: music21_rs::Error) -> PyErr {
    SerialException::new_err(crate::pitch::message(&error))
}

fn transformation(name: &str) -> PyResult<Transformation> {
    Transformation::from_name(name).map_err(serial_error)
}

fn convention(name: &str) -> PyResult<TransformationConvention> {
    TransformationConvention::from_name(name).map_err(serial_error)
}

fn labelled(
    py: Python<'_>,
    found: &[(Transformation, u8)],
    convention: TransformationConvention,
) -> PyResult<Py<PyList>> {
    let list = PyList::empty(py);
    for (transformation, index) in found {
        list.append((transformation.label(convention), *index))?;
    }
    Ok(list.unbind())
}

/// One pitch class read out of a row argument: an integer, a facade pitch, a
/// note-shaped object with a `pitch`, or a pitch-class letter such as `A`.
fn pitch_class_of(item: &Bound<'_, PyAny>) -> PyResult<i32> {
    if let Ok(value) = item.extract::<i32>() {
        return Ok(value);
    }
    if let Ok(text) = item.extract::<String>() {
        return match text.as_str() {
            "A" | "a" => Ok(10),
            "B" | "b" => Ok(11),
            other => other.parse::<i32>().map_err(|_| {
                SerialException::new_err(format!("cannot read a pitch class from {other:?}"))
            }),
        };
    }
    if let Ok(pitch) = item.getattr("pitch") {
        return Ok(pitch_class_of_pitch(&pitch_from_any(&pitch)?));
    }
    Ok(pitch_class_of_pitch(&pitch_from_any(item)?))
}

fn pitch_class_of_pitch(pitch: &music21_rs::Pitch) -> i32 {
    (pitch.ps().round_ties_even() as i32).rem_euclid(12)
}

fn row_from_any(row: Option<&Bound<'_, PyAny>>) -> PyResult<RsToneRow> {
    let Some(row) = row.filter(|value| !value.is_none()) else {
        return Ok(RsToneRow::default());
    };
    let mut classes = Vec::new();
    for item in row.try_iter()? {
        classes.push(pitch_class_of(&item?)?);
    }
    Ok(RsToneRow::new(classes))
}

/// A note-shaped element of a row: what iterating a music21 `ToneRow`
/// yields. Only what the doctests read.
#[pyclass(name = "Note", module = "music21.note", skip_from_py_object)]
pub struct RowNote {
    #[pyo3(get)]
    pitch: Pitch,
}

#[pymethods]
impl RowNote {
    #[getter]
    fn name(&self) -> String {
        self.pitch.inner.name()
    }

    fn __repr__(&self) -> String {
        format!("<music21.note.Note {}>", self.pitch.inner.name())
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

/// music21's `serial.ToneRow`.
#[pyclass(
    name = "ToneRow",
    module = "music21.serial",
    subclass,
    skip_from_py_object
)]
pub struct ToneRow {
    pub(crate) inner: RsToneRow,
    id: Option<String>,
}

impl ToneRow {
    fn of(inner: RsToneRow) -> Self {
        Self { inner, id: None }
    }

    /// A row object of the class `pcToToneRow` picks: `TwelveToneRow` for
    /// twelve pitch classes, `ToneRow` otherwise.
    fn object(py: Python<'_>, inner: RsToneRow) -> PyResult<Py<PyAny>> {
        if inner.len() == 12 {
            let init = PyClassInitializer::from(ToneRow::of(inner)).add_subclass(TwelveToneRow);
            Ok(Py::new(py, init)?.into_any())
        } else {
            Ok(Py::new(py, ToneRow::of(inner))?.into_any())
        }
    }

    fn twelve(py: Python<'_>, inner: RsToneRow, id: Option<String>) -> PyResult<Py<PyAny>> {
        let init = PyClassInitializer::from(ToneRow { inner, id }).add_subclass(TwelveToneRow);
        Ok(Py::new(py, init)?.into_any())
    }
}

fn other_row(value: &Bound<'_, PyAny>) -> PyResult<RsToneRow> {
    if let Ok(row) = value.extract::<PyRef<ToneRow>>() {
        return Ok(row.inner.clone());
    }
    row_from_any(Some(value))
}

#[pymethods]
impl ToneRow {
    /// music21 freezes a score by pickling it, and what this object is lives
    /// in Rust where a pickle cannot see it — so it is written out as text,
    /// and read back into a fresh one of these.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let Some(inner) = crate::unpickled::<_, RsToneRow>(slf, state)? else {
            return Ok(());
        };
        slf.borrow_mut().inner = inner;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (row = None, **kwargs))]
    fn new(row: Option<&Bound<'_, PyAny>>, kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let _ = kwargs;
        Ok(Self::of(row_from_any(row)?))
    }

    #[getter]
    fn row(&self) -> Vec<u32> {
        self.inner
            .pitch_classes()
            .iter()
            .map(|&pc| u32::from(pc))
            .collect()
    }

    #[getter]
    fn id(slf: &Bound<'_, Self>) -> String {
        match &slf.borrow().id {
            Some(id) => id.clone(),
            None => format!("0x{:x}", slf.as_ptr() as usize),
        }
    }

    #[setter]
    fn set_id(&mut self, value: Option<String>) {
        self.id = value;
    }

    #[allow(non_snake_case)]
    fn pitchClasses(&self) -> Vec<u32> {
        self.inner
            .pitch_classes()
            .iter()
            .map(|&pc| u32::from(pc))
            .collect()
    }

    #[allow(non_snake_case)]
    fn noteNames(&self) -> Vec<String> {
        self.inner.note_names()
    }

    #[allow(non_snake_case)]
    fn isTwelveToneRow(&self) -> bool {
        self.inner.is_twelve_tone_row()
    }

    #[allow(non_snake_case)]
    fn makeTwelveToneRow(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        ToneRow::twelve(py, self.inner.clone(), None)
    }

    #[allow(non_snake_case)]
    fn isSameRow(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(self.inner.is_same_row(&other_row(other)?))
    }

    #[allow(non_snake_case)]
    fn getIntervalsAsString(&self) -> String {
        self.inner.intervals_as_string()
    }

    #[allow(non_snake_case)]
    fn zeroCenteredTransformation(
        &self,
        py: Python<'_>,
        transformationType: &str,
        index: i32,
    ) -> PyResult<Py<PyAny>> {
        let kind = transformation(transformationType)?;
        ToneRow::object(py, self.inner.zero_centered_transformation(kind, index))
    }

    #[allow(non_snake_case)]
    fn originalCenteredTransformation(
        &self,
        py: Python<'_>,
        transformationType: &str,
        index: i32,
    ) -> PyResult<Py<PyAny>> {
        let kind = transformation(transformationType)?;
        ToneRow::object(py, self.inner.original_centered_transformation(kind, index))
    }

    #[allow(non_snake_case)]
    fn findZeroCenteredTransformations(
        &self,
        py: Python<'_>,
        otherRow: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let other = other_row(otherRow)?;
        if self.inner.len() != other.len() {
            return Ok(false.into_pyobject(py)?.to_owned().into_any().unbind());
        }
        let found = self.inner.find_zero_centered_transformations(&other);
        Ok(labelled(py, &found, TransformationConvention::ZeroCentered)?.into_any())
    }

    #[allow(non_snake_case)]
    fn findOriginalCenteredTransformations(
        &self,
        py: Python<'_>,
        otherRow: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyList>> {
        let found = self
            .inner
            .find_original_centered_transformations(&other_row(otherRow)?);
        labelled(py, &found, TransformationConvention::OriginalCentered)
    }

    /// music21's `matrix`, which keeps the row it was made from: its own
    /// `repr` names that row, and a historical row names itself.
    fn matrix(slf: &Bound<'_, Self>) -> PyResult<TwelveToneMatrix> {
        Ok(TwelveToneMatrix {
            inner: slf.borrow().inner.matrix(),
            source: Some(slf.clone().unbind().into_any()),
        })
    }

    #[allow(non_snake_case)]
    fn findHistorical(&self) -> Vec<&'static str> {
        self.inner
            .find_historical()
            .into_iter()
            .map(|row| row.name)
            .collect()
    }

    #[allow(non_snake_case)]
    fn findTransformedHistorical(&self, py: Python<'_>, convention: &str) -> PyResult<Py<PyList>> {
        let kind = self::convention(convention)?;
        let list = PyList::empty(py);
        for (row, found) in self.inner.find_transformed_historical(kind) {
            list.append((row.name, labelled(py, &found, kind)?))?;
        }
        Ok(list.unbind())
    }

    #[allow(non_snake_case)]
    fn isAllInterval(&self) -> PyResult<bool> {
        self.inner.is_all_interval().map_err(serial_error)
    }

    #[allow(non_snake_case)]
    fn getLinkClassification(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        let classification = self.inner.link_classification().map_err(serial_error)?;
        let (number, specials): (Option<u32>, Vec<&str>) = match classification {
            Some(link) => (Some(link.number), link.special_intervals),
            None => (None, Vec::new()),
        };
        Ok(PyTuple::new(
            py,
            [
                number.into_pyobject(py)?.into_any(),
                PyList::new(py, specials)?.into_any(),
            ],
        )?
        .unbind())
    }

    #[allow(non_snake_case)]
    fn isLinkChord(&self) -> PyResult<bool> {
        self.inner.is_link_chord().map_err(serial_error)
    }

    #[allow(non_snake_case)]
    #[pyo3(signature = (transType1, index1, transType2, index2, unused_convention = None))]
    fn areCombinatorial(
        &self,
        transType1: &str,
        index1: i32,
        transType2: &str,
        index2: i32,
        unused_convention: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        let _ = unused_convention;
        self.inner
            .are_combinatorial(
                transformation(transType1)?,
                index1,
                transformation(transType2)?,
                index2,
            )
            .map_err(serial_error)
    }

    fn append(&mut self, element: &Bound<'_, PyAny>) -> PyResult<()> {
        let mut classes: Vec<i32> = self
            .inner
            .pitch_classes()
            .iter()
            .map(|&pc| i32::from(pc))
            .collect();
        classes.push(pitch_class_of(element)?);
        self.inner = RsToneRow::new(classes);
        Ok(())
    }

    #[pyo3(signature = (fmt = None))]
    fn show(&self, py: Python<'_>, fmt: Option<&str>) -> PyResult<()> {
        let _ = fmt;
        let print = py.import("builtins")?.getattr("print")?;
        for (offset, pitch) in self.inner.pitches().into_iter().enumerate() {
            print.call1((format!(
                "{{{offset}.0}} <music21.note.Note {}>",
                pitch.name()
            ),))?;
        }
        Ok(())
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __getitem__(&self, index: isize) -> PyResult<RowNote> {
        let pitches = self.inner.pitches();
        let resolved = if index < 0 {
            pitches.len() as isize + index
        } else {
            index
        };
        let pitch = usize::try_from(resolved)
            .ok()
            .and_then(|i| pitches.get(i).cloned())
            .ok_or_else(|| pyo3::exceptions::PyIndexError::new_err("row index out of range"))?;
        Ok(RowNote {
            pitch: Pitch::wrap(pitch, true),
        })
    }

    fn __iter__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let notes: Vec<RowNote> = self
            .inner
            .pitches()
            .into_iter()
            .map(|pitch| RowNote {
                pitch: Pitch::wrap(pitch, true),
            })
            .collect();
        let list = PyList::new(py, notes)?;
        Ok(list.try_iter()?.unbind().into_any())
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let qualname = slf.get_type().qualname()?;
        let borrowed = slf.borrow();
        let detail = match &borrowed.id {
            Some(id) => id.clone(),
            None => borrowed.inner.to_string(),
        };
        Ok(format!("<music21.serial.{qualname} {detail}>"))
    }
}

/// music21's `serial.TwelveToneRow`: a `ToneRow` that can print a matrix.
#[pyclass(name = "TwelveToneRow", module = "music21.serial", extends = ToneRow, subclass)]
pub struct TwelveToneRow;

#[pymethods]
impl TwelveToneRow {
    #[new]
    #[pyo3(signature = (row = None, **kwargs))]
    fn new(
        row: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let _ = kwargs;
        Ok(PyClassInitializer::from(ToneRow::of(row_from_any(row)?)).add_subclass(TwelveToneRow))
    }
}

/// music21's `serial.HistoricalTwelveToneRow`.
#[pyclass(name = "HistoricalTwelveToneRow", module = "music21.serial", extends = TwelveToneRow)]
pub struct HistoricalTwelveToneRow {
    #[pyo3(get, set)]
    composer: Option<String>,
    #[pyo3(get, set)]
    opus: Option<String>,
    #[pyo3(get, set)]
    title: Option<String>,
}

#[pymethods]
impl HistoricalTwelveToneRow {
    #[new]
    #[pyo3(signature = (composer = None, opus = None, title = None, row = None, **kwargs))]
    fn new(
        composer: Option<String>,
        opus: Option<String>,
        title: Option<String>,
        row: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let _ = kwargs;
        Ok(PyClassInitializer::from(ToneRow::of(row_from_any(row)?))
            .add_subclass(TwelveToneRow)
            .add_subclass(Self {
                composer,
                opus,
                title,
            }))
    }

    fn __repr__(&self) -> String {
        let show = |value: &Option<String>| value.clone().unwrap_or_else(|| "None".to_string());
        format!(
            "<music21.serial.HistoricalTwelveToneRow {} {} {}>",
            show(&self.composer),
            show(&self.opus),
            show(&self.title)
        )
    }
}

/// music21's `serial.TwelveToneMatrix`.
#[pyclass(
    name = "TwelveToneMatrix",
    module = "music21.serial",
    subclass,
    skip_from_py_object
)]
pub struct TwelveToneMatrix {
    inner: RsMatrix,
    /// The row this matrix was made from, which its `repr` names.
    source: Option<Py<PyAny>>,
}

#[pymethods]
impl TwelveToneMatrix {
    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    /// music21's `repr`, which names the matrix's first row.
    ///
    /// That row is built as the class of the row the matrix came from, with
    /// its attributes merged in, so a matrix of a historical row names the
    /// piece — a historical row says its title rather than its id — while
    /// any other names itself `row-1`.
    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        if let Some(source) = &self.source
            && source.bind(py).is_instance_of::<HistoricalTwelveToneRow>()
        {
            return Ok(format!(
                "<music21.serial.TwelveToneMatrix for [{}]>",
                source.bind(py).repr()?
            ));
        }
        match self.inner.rows().first() {
            Some(first) => {
                let row = ToneRow::twelve(py, first.clone(), Some("row-1".to_string()))?;
                Ok(format!(
                    "<music21.serial.TwelveToneMatrix for [{}]>",
                    row.bind(py).repr()?
                ))
            }
            None => Ok("<music21.serial.TwelveToneMatrix>".to_string()),
        }
    }

    fn __len__(&self) -> usize {
        self.inner.rows().len()
    }

    fn __getitem__(&self, py: Python<'_>, index: isize) -> PyResult<Py<PyAny>> {
        let rows = self.inner.rows();
        let resolved = if index < 0 {
            rows.len() as isize + index
        } else {
            index
        };
        let row = usize::try_from(resolved)
            .ok()
            .and_then(|i| rows.get(i).cloned())
            .ok_or_else(|| pyo3::exceptions::PyIndexError::new_err("matrix index out of range"))?;
        ToneRow::twelve(py, row, Some(format!("row-{}", resolved + 1)))
    }

    fn __iter__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let list = PyList::empty(py);
        for (index, row) in self.inner.rows().iter().enumerate() {
            list.append(ToneRow::twelve(
                py,
                row.clone(),
                Some(format!("row-{}", index + 1)),
            )?)?;
        }
        Ok(list.try_iter()?.unbind().into_any())
    }
}

/// music21's `serial.pcToToneRow`.
#[pyfunction]
#[pyo3(name = "pcToToneRow")]
#[allow(non_snake_case)]
fn pc_to_tone_row(py: Python<'_>, pcSet: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    ToneRow::object(py, row_from_any(Some(pcSet))?)
}

/// music21's `serial.rowToMatrix`.
#[pyfunction]
#[pyo3(name = "rowToMatrix")]
fn row_to_matrix_text(p: Vec<i32>) -> String {
    row_to_matrix(&p)
}

/// music21's `serial.getHistoricalRowByName`.
#[pyfunction]
#[pyo3(name = "getHistoricalRowByName")]
#[allow(non_snake_case)]
fn get_historical_row_by_name(py: Python<'_>, rowName: &str) -> PyResult<Py<PyAny>> {
    let row = historical_row_by_name(rowName).map_err(serial_error)?;
    let init = PyClassInitializer::from(ToneRow::of(row.row()))
        .add_subclass(TwelveToneRow)
        .add_subclass(HistoricalTwelveToneRow {
            composer: Some(row.composer.to_string()),
            opus: row.opus.map(str::to_string),
            title: Some(row.title.to_string()),
        });
    Ok(Py::new(py, init)?.into_any())
}

/// Adds the serial facades to the module.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add_class::<ToneRow>()?;
    m.add_class::<TwelveToneRow>()?;
    m.add_class::<HistoricalTwelveToneRow>()?;
    m.add_class::<TwelveToneMatrix>()?;
    m.add_class::<RowNote>()?;
    m.add_function(wrap_pyfunction!(pc_to_tone_row, m)?)?;
    m.add_function(wrap_pyfunction!(row_to_matrix_text, m)?)?;
    m.add_function(wrap_pyfunction!(get_historical_row_by_name, m)?)?;
    let exception = py.get_type::<SerialException>();
    exception.setattr("__module__", "music21.serial")?;
    m.add("SerialException", exception)?;
    let historical = PyDict::new(py);
    for row in &HISTORICAL_ROWS {
        historical.set_item(
            row.name,
            (
                row.composer,
                row.opus,
                row.title,
                row.pitch_classes
                    .iter()
                    .map(|&pc| u32::from(pc))
                    .collect::<Vec<_>>(),
            ),
        )?;
    }
    m.add("historicalDict", historical)?;
    Ok(())
}
