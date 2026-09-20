//! music21's `scale.scala`, over the crate's Scala reader.
//!
//! A Scala file is a description, a count and a line per degree, the last of
//! them the interval the scale repeats at. `ScalaData` is that file as an
//! object and `ScalaPitch` is one of its lines; both are ported, so music21's
//! own `ScalaScale` builds its pitches out of what the crate reads.
//!
//! `ScalaFile` stays music21's. It opens files, which this library leaves to
//! its caller — and leaving it alone is the harder test: music21's own file
//! reader hands its text to the `ScalaData` here.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyList;

use music21_rs_crate::FloatType;
use music21_rs_crate::tuningsystem::scala::{
    ScalaDegree as RsScalaDegree, ScalaScale as RsScalaScale,
};

/// The names the `scala` facade replaces in `music21.scale.scala`.
///
/// `ScalaFile` and `getPaths` are not among them; see the module comment.
/// `parse` and `search` answer a name out of the bundled archive, so they are
/// here only in a build carrying it.
#[cfg(feature = "scala-archive")]
pub const NAMES: &[&str] = &["ScalaPitch", "ScalaData", "parse", "search"];

/// The names the `scala` facade replaces, in a build with no archive to
/// answer a name out of.
#[cfg(not(feature = "scala-archive"))]
pub const NAMES: &[&str] = &["ScalaPitch", "ScalaData"];

/// A value that cannot be read, as Python fails to read one: music21 reads a
/// line with `float()`, which raises `ValueError`.
fn scala_error(error: music21_rs_crate::Error) -> PyErr {
    PyValueError::new_err(crate::pitch::message(&error))
}

/// The characters a value may be written with, which is music21's
/// `getNumFromStr` over `0123456789./` — and a backslash besides, since Scala
/// writes an equal-tempered step as `n\m` and the crate reads it.
fn value_of(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_digit() || matches!(c, '.' | '/' | '\\'))
        .collect::<String>()
        .trim()
        .to_string()
}

/// music21's `scale.scala.ScalaPitch`: one line of a Scala file.
#[pyclass(
    name = "ScalaPitch",
    module = "music21.scale.scala",
    subclass,
    skip_from_py_object
)]
pub struct ScalaPitch {
    src: Option<String>,
    cents: Option<FloatType>,
}

impl ScalaPitch {
    /// A line already read, which is what a parsed file is made of.
    pub(crate) fn read(degree: RsScalaDegree) -> Self {
        Self {
            src: Some(degree.written()),
            cents: Some(degree.cents()),
        }
    }
}

#[pymethods]
impl ScalaPitch {
    #[new]
    #[pyo3(signature = (sourceString = None))]
    fn new(sourceString: Option<&str>) -> Self {
        Self {
            src: sourceString.map(value_of),
            cents: None,
        }
    }

    /// The value as written, with whatever followed it dropped.
    #[getter]
    fn get_src(&self) -> Option<String> {
        self.src.clone()
    }

    #[setter]
    fn set_src(&mut self, value: Option<&str>) {
        self.src = value.map(value_of);
    }

    /// How far above the tonic the line sounds, once it has been read.
    #[getter]
    fn get_cents(&self) -> Option<FloatType> {
        self.cents
    }

    #[setter]
    fn set_cents(&mut self, value: Option<FloatType>) {
        self.cents = value;
    }

    /// music21's `parse`: reads the line and says how far above the tonic it
    /// sounds. A value carrying a point is cents and one without is a ratio.
    #[pyo3(signature = (sourceString = None))]
    fn parse(&mut self, sourceString: Option<&str>) -> PyResult<FloatType> {
        if let Some(source) = sourceString {
            self.src = Some(value_of(source));
        }
        let src = self.src.clone().unwrap_or_default();
        let degree: RsScalaDegree = src.parse().map_err(scala_error)?;
        let cents = degree.cents();
        self.cents = Some(cents);
        Ok(cents)
    }
}

/// music21's `scale.scala.ScalaData`: a Scala file as an object.
#[pyclass(
    name = "ScalaData",
    module = "music21.scale.scala",
    subclass,
    skip_from_py_object
)]
pub struct ScalaData {
    src: Option<String>,
    file_name: Option<String>,
    description: Option<String>,
    /// The lines as the objects music21 hands back, so that editing one edits
    /// the file this writes.
    pitch_values: Vec<Py<ScalaPitch>>,
}

impl ScalaData {
    /// The scale as the crate reads it, built from the lines as they stand —
    /// so an edit through a `ScalaPitch` is in it.
    fn scale(&self, py: Python<'_>) -> PyResult<RsScalaScale> {
        let mut cents = Vec::with_capacity(self.pitch_values.len());
        for value in &self.pitch_values {
            let read = value.borrow(py).cents.ok_or_else(|| {
                PyValueError::new_err("a scala pitch value that has not been read has no cents")
            })?;
            cents.push(read);
        }
        Ok(RsScalaScale::from_written_cents(
            self.description.clone().unwrap_or_default(),
            &cents,
        ))
    }

    /// Fills in the lines from a scale the crate has read.
    fn read_scale(&mut self, py: Python<'_>, scale: &RsScalaScale) -> PyResult<()> {
        if !scale.description().is_empty() {
            self.description = Some(scale.description().to_string());
        }
        self.pitch_values = scale
            .written_degrees()
            .into_iter()
            .map(|degree| Py::new(py, ScalaPitch::read(degree)))
            .collect::<PyResult<Vec<_>>>()?;
        Ok(())
    }

    /// A file read out of the archive, under the name it is filed as.
    #[cfg(feature = "scala-archive")]
    pub(crate) fn of(py: Python<'_>, file_name: &str, scale: &RsScalaScale) -> PyResult<Self> {
        let mut made = Self {
            src: None,
            file_name: Some(file_name.to_string()),
            description: None,
            pitch_values: Vec::new(),
        };
        made.read_scale(py, scale)?;
        Ok(made)
    }
}

#[pymethods]
impl ScalaData {
    #[new]
    #[pyo3(signature = (sourceString = None, fileName = None))]
    fn new(sourceString: Option<String>, fileName: Option<String>) -> Self {
        Self {
            src: sourceString,
            file_name: fileName,
            description: None,
            pitch_values: Vec::new(),
        }
    }

    /// The Python objects this holds, shown to the cycle collector.
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        for value in &self.pitch_values {
            visit.call(value)?;
        }
        Ok(())
    }

    fn __clear__(&mut self) {
        self.pitch_values.clear();
    }

    /// music21's `parse`: reads the text this was built on.
    ///
    /// The file name is taken off the first line where that is a comment
    /// naming a `.scl` file and no name was given, as music21 does.
    fn parse(&mut self, py: Python<'_>) -> PyResult<()> {
        let source = self.src.clone().unwrap_or_default();
        if self.file_name.is_none()
            && let Some(first) = source.lines().next()
            && let Some(comment) = first.trim().strip_prefix('!')
            && comment.contains(".scl")
        {
            self.file_name = Some(comment.trim().to_string());
        }
        let scale = RsScalaScale::parse(&source).map_err(scala_error)?;
        self.read_scale(py, &scale)
    }

    #[getter]
    fn get_src(&self) -> Option<String> {
        self.src.clone()
    }

    #[setter]
    fn set_src(&mut self, value: Option<String>) {
        self.src = value;
    }

    #[getter]
    fn get_fileName(&self) -> Option<String> {
        self.file_name.clone()
    }

    #[setter]
    fn set_fileName(&mut self, value: Option<String>) {
        self.file_name = value;
    }

    #[getter]
    fn get_description(&self) -> Option<String> {
        self.description.clone()
    }

    #[setter]
    fn set_description(&mut self, value: Option<String>) {
        self.description = value;
    }

    /// How many lines of values the file carries: music21's `pitchCount`,
    /// which says nothing until something has been read.
    #[getter]
    fn get_pitchCount(&self) -> Option<usize> {
        if self.pitch_values.is_empty() {
            return None;
        }
        Some(self.pitch_values.len())
    }

    /// The lines themselves, as the same objects every time, so that writing
    /// a value into one writes it into the file.
    #[getter]
    fn pitchValues<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        PyList::new(
            py,
            self.pitch_values.iter().map(|value| value.clone_ref(py)),
        )
    }

    /// music21's `getCentsAboveTonic`: how far above the tonic each line
    /// sounds, the period last and the unison left out.
    fn getCentsAboveTonic(&self, py: Python<'_>) -> Vec<Option<FloatType>> {
        self.pitch_values
            .iter()
            .map(|value| value.borrow(py).cents)
            .collect()
    }

    /// music21's `getAdjacentCents`: the width of each step.
    fn getAdjacentCents(&self, py: Python<'_>) -> PyResult<Vec<FloatType>> {
        Ok(self.scale(py)?.adjacent_cents())
    }

    /// music21's `setAdjacentCents`: the scale written afresh from the width
    /// of each step.
    fn setAdjacentCents(&mut self, py: Python<'_>, centList: Vec<FloatType>) -> PyResult<()> {
        let scale = RsScalaScale::from_adjacent_cents(
            self.description.clone().unwrap_or_default(),
            &centList,
        );
        self.read_scale(py, &scale)
    }

    /// music21's `getIntervalSequence`: each step as an interval.
    fn getIntervalSequence(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        self.scale(py)?
            .interval_sequence()
            .map_err(scala_error)?
            .into_iter()
            .map(|interval| crate::interval::interval_object(py, interval))
            .collect()
    }

    /// music21's `setIntervalSequence`: the scale written afresh from an
    /// interval per step.
    fn setIntervalSequence(&mut self, py: Python<'_>, iList: Vec<Py<PyAny>>) -> PyResult<()> {
        let mut widths = Vec::with_capacity(iList.len());
        for interval in &iList {
            widths.push(interval.bind(py).getattr("cents")?.extract()?);
        }
        self.setAdjacentCents(py, widths)
    }

    /// music21's `getFileString`: the text of a `.scl` file for this scale.
    ///
    /// Every value is written in cents, as music21 writes them, since a line
    /// read into a `ScalaPitch` is kept as cents and nothing else.
    fn getFileString(&self, py: Python<'_>) -> PyResult<String> {
        Ok(self.scale(py)?.file_string(self.file_name.as_deref()))
    }
}

/// The archive as read once, since every name is answered out of it.
#[cfg(feature = "scala-archive")]
fn archive() -> &'static music21_rs_crate::tuningsystem::scala::ScalaArchive {
    use music21_rs_crate::tuningsystem::scala::ScalaArchive;
    static ARCHIVE: std::sync::OnceLock<ScalaArchive> = std::sync::OnceLock::new();
    ARCHIVE.get_or_init(ScalaArchive::bundled)
}

/// music21's `scale.scala.parse`: the scale a name names, out of the archive.
///
/// A name is a file name, a name written without its extension, underscores
/// or hyphens, or anything a file name contains; a name that names nothing
/// answers `None`, as music21 does.
///
/// Only a build carrying the archive has this, since a name is answered out
/// of the archive and nowhere else. Without it, read the file yourself and
/// hand its text to `ScalaData`.
#[cfg(feature = "scala-archive")]
#[pyfunction]
#[pyo3(signature = (target))]
pub fn parse(py: Python<'_>, target: &str) -> PyResult<Option<ScalaData>> {
    let Some((file_name, scale)) = archive().find(target) else {
        return Ok(None);
    };
    ScalaData::of(py, file_name, scale).map(Some)
}

/// music21's `scale.scala.search`: every file in the archive whose name
/// matches, sorted. Only a build carrying the archive has it.
#[cfg(feature = "scala-archive")]
#[pyfunction]
#[pyo3(signature = (target))]
pub fn search(target: &str) -> PyResult<Vec<String>> {
    Ok(archive()
        .search(target)
        .into_iter()
        .map(str::to_string)
        .collect())
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ScalaPitch>()?;
    m.add_class::<ScalaData>()?;
    #[cfg(feature = "scala-archive")]
    {
        m.add_function(wrap_pyfunction!(parse, m)?)?;
        m.add_function(wrap_pyfunction!(search, m)?)?;
    }
    Ok(())
}
