//! music21's `musedata.translate`: MuseData parts and works turned into
//! streams, over the crate's MuseData reader.
//!
//! A part is read from the lines music21's `MuseDataPart` keeps in `src`,
//! a work from its parts', so any object holding them -- music21's own
//! `MuseDataPart` included -- is read by the crate. The steps music21
//! exposes for one note (`_musedataRecordListToNoteOrChord`) and for slurs
//! (`_SlurCollector`) read records' lines the same way.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use music21::musedata::{self as rs, Header, Slurs, Stage};
use music21::notation::TieType;
use music21::stream::StreamElement;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyList, PyModule, PyTuple};

use crate::subconverters::music21_name;

/// The module music21 keeps these in.
const MODULE: &str = "music21.musedata.translate";

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

// music21's `musedata.MuseDataException`, which its translate raises.
pyo3::create_exception!(
    music21_rs_facade,
    MuseDataException,
    crate::Music21Exception
);

/// A crate refusal as an exception.
fn refusal(error: &music21::Error) -> PyErr {
    MuseDataException::new_err(crate::pitch::message(error))
}

/// A part's lines, as music21's `MuseDataPart.src` holds them.
fn lines_of(part: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    part.getattr("src")?.extract()
}

/// The stage a record or part is written in, stage 2 where unknown.
fn stage_of(object: &Bound<'_, PyAny>) -> PyResult<Stage> {
    let stage = object.getattr("stage")?;
    if !stage.is_none() && stage.extract::<i64>()? == 1 {
        return Ok(Stage::One);
    }
    Ok(Stage::Two)
}

/// music21's `_musedataBeamToBeams`: the beams a beam column writes.
#[pyfunction]
#[pyo3(name = "_musedataBeamToBeams")]
fn beam_to_beams(py: Python<'_>, beamSymbol: &str) -> PyResult<Py<PyAny>> {
    crate::makenotation::beams_object(py, rs::beams_of(beamSymbol))
}

/// The tie a previous note, chord or rest leaves for the next record.
fn previous_tie(previous: Option<&Bound<'_, PyAny>>) -> PyResult<Option<Option<TieType>>> {
    let Some(previous) = previous.filter(|given| !given.is_none()) else {
        return Ok(None);
    };
    let tie = previous.getattr("tie")?;
    if tie.is_none() {
        return Ok(Some(None));
    }
    let name: String = tie.getattr("type")?.extract()?;
    Ok(Some(TieType::from_name(&name).ok()))
}

/// music21's `_musedataRecordListToNoteOrChord`: a note, or a chord of
/// several records, and the dynamics the first carries. The records'
/// divisions are their part's, reached through their measure.
#[pyfunction]
#[pyo3(name = "_musedataRecordListToNoteOrChord", signature = (records, previousElement = None))]
fn record_note<'py>(
    records: &Bound<'py, PyAny>,
    previousElement: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py = records.py();
    let records: Vec<Bound<'py, PyAny>> = records.try_iter()?.collect::<PyResult<_>>()?;
    let Some(first) = records.first() else {
        return Err(MuseDataException::new_err("a note of no records"));
    };
    let lines: Vec<String> = records
        .iter()
        .map(|record| record.getattr("src")?.extract())
        .collect::<PyResult<_>>()?;
    let measure = first.getattr("parent")?;
    if measure.is_none() {
        return Err(MuseDataException::new_err(
            "cannot access parent container of this record to obtain divisions per quarter",
        ));
    }
    let divisions: f64 = measure
        .getattr("parent")?
        .call_method0("getDivisionsPerQuarterNote")?
        .extract()?;
    let (made, dynamics, _) = rs::record_note(
        &lines,
        stage_of(first)?,
        divisions,
        previous_tie(previousElement)?,
    )
    .map_err(|error| refusal(&error))?;
    let element =
        crate::stream::element_object(py, &made)?.unwrap_or_else(|| py.None().into_bound(py));
    let marks = PyList::empty(py);
    for dynamic in dynamics {
        if let Some(object) = crate::stream::element_object(py, &StreamElement::Dynamic(dynamic))? {
            marks.append(object)?;
        }
    }
    PyTuple::new(py, [element, marks.into_any()])
}

/// music21's `_SlurCollector`: slurs built from the slur marks of records
/// read in order, up to four open at once.
#[pyclass(name = "_SlurCollector", module = "music21.musedata.translate")]
pub struct SlurCollector {
    read: Slurs,
    /// Each note or chord added, in order.
    elements: Vec<Py<PyAny>>,
    /// The slurs made so far.
    slurs: Py<PyList>,
}

#[pymethods]
impl SlurCollector {
    /// The marks that start slurs 1 to 4.
    #[classattr]
    fn starts() -> &'static str {
        "([{z"
    }

    /// The marks that stop them.
    #[classattr]
    fn stops() -> &'static str {
        ")]}x"
    }

    #[new]
    fn new(py: Python<'_>) -> Self {
        Self {
            read: Slurs::default(),
            elements: Vec::new(),
            slurs: PyList::empty(py).unbind(),
        }
    }

    /// The slurs made so far.
    #[getter]
    fn slurs(&self, py: Python<'_>) -> Py<PyList> {
        self.slurs.clone_ref(py)
    }

    /// The open slurs, by number from nought, each as the note it starts on.
    #[getter]
    fn openSlurs<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let open = PyDict::new(py);
        for (number, start) in self.read.open().iter().enumerate() {
            if let Some(start) = start {
                open.set_item(number, self.elements[*start].bind(py))?;
            }
        }
        Ok(open)
    }

    /// music21's `add`: the slurs `record` marks stopped, then started, on
    /// `element`.
    fn add(
        &mut self,
        py: Python<'_>,
        record: &Bound<'_, PyAny>,
        element: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let marks: String = record.call_method0("getSlurSymbols")?.extract()?;
        self.elements.push(element.clone().unbind());
        let slur = music21_name(py, "music21.spanner", "Slur")?;
        for (start, stop) in self.read.add(&marks) {
            let made = slur.call1((self.elements[start].bind(py), self.elements[stop].bind(py)))?;
            self.slurs.bind(py).append(made)?;
        }
        Ok(())
    }
}

/// music21's `_processPending`: the pending records made a note or chord,
/// put into the voice or measure with its dynamics, slurs collected. Hands
/// back what was made.
#[pyfunction]
#[pyo3(name = "_processPending")]
fn process_pending<'py>(
    pendingRecords: &Bound<'py, PyAny>,
    eLast: Option<&Bound<'py, PyAny>>,
    m: &Bound<'py, PyAny>,
    vActive: Option<&Bound<'py, PyAny>>,
    slurCollector: &Bound<'py, SlurCollector>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = m.py();
    let made = record_note(pendingRecords, eLast)?;
    let element = made.get_item(0)?;
    slurCollector
        .borrow_mut()
        .add(py, &pendingRecords.get_item(0)?, &element)?;
    let into = match vActive.filter(|given| !given.is_none()) {
        Some(voice) => voice.clone(),
        None => m.clone(),
    };
    into.call_method1("coreAppend", (&element,))?;
    for dynamic in made.get_item(1)?.try_iter()? {
        let offset = element.call_method1("getOffsetBySite", (&into,))?;
        into.call_method1("coreInsert", (offset, dynamic?))?;
    }
    Ok(element)
}

/// The parts of a crate score, in order.
fn parts_of<'py>(py: Python<'py>, score: &music21::Stream) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let made = crate::stream::from_crate(py, score)?;
    let mut parts = Vec::new();
    for part in made.getattr("parts")?.try_iter()? {
        parts.push(part?);
    }
    Ok(parts)
}

/// A new score of music21's, or the one handed in.
fn score_or<'py>(
    py: Python<'py>,
    inputM21: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    match inputM21.filter(|given| !given.is_none()) {
        Some(given) => Ok(given.clone()),
        None => music21_name(py, "music21.stream", "Score")?.call0(),
    }
}

/// music21's `musedataPartToStreamPart`: a part read and put at the start
/// of `inputM21`, or of a new score, which is handed back.
#[pyfunction]
#[pyo3(signature = (museDataPart, inputM21 = None))]
fn musedataPartToStreamPart<'py>(
    museDataPart: &Bound<'py, PyAny>,
    inputM21: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = museDataPart.py();
    let score = rs::from_musedata_parts(&[lines_of(museDataPart)?], Header::Skipped)
        .map_err(|error| refusal(&error))?;
    let into = score_or(py, inputM21)?;
    for part in parts_of(py, &score)? {
        into.call_method1("insert", (0, part))?;
    }
    Ok(into)
}

/// music21's `museDataWorkToStreamScore`: each of a work's parts read into
/// one score, with the work's title, number and movement.
#[pyfunction]
#[pyo3(signature = (museDataWork, inputM21 = None))]
fn museDataWorkToStreamScore<'py>(
    museDataWork: &Bound<'py, PyAny>,
    inputM21: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = museDataWork.py();
    let mut parts = Vec::new();
    for part in museDataWork.call_method0("getParts")?.try_iter()? {
        parts.push(lines_of(&part?)?);
    }
    let score = rs::from_musedata_parts(&parts, Header::Read).map_err(|error| refusal(&error))?;
    let made = crate::stream::from_crate(py, &score)?;
    let Some(into) = inputM21.filter(|given| !given.is_none()) else {
        return Ok(made);
    };
    for element in made.getattr("elements")?.try_iter()? {
        let element = element?;
        let offset = made.call_method1("elementOffset", (&element,))?;
        into.call_method1("insert", (offset, element))?;
    }
    Ok(into.clone())
}

/// The module music21 calls `musedata.translate`.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        m.add_function(wrap_pyfunction!(beam_to_beams, &m)?)?;
        m.add_function(wrap_pyfunction!(record_note, &m)?)?;
        m.add_function(wrap_pyfunction!(process_pending, &m)?)?;
        m.add_function(wrap_pyfunction!(musedataPartToStreamPart, &m)?)?;
        m.add_function(wrap_pyfunction!(museDataWorkToStreamScore, &m)?)?;
        m.add_class::<SlurCollector>()?;
        py.get_type::<MuseDataException>()
            .setattr("__module__", "music21.musedata")?;
        m.add(
            "environLocal",
            crate::environment::module(py)?
                .getattr("Environment")?
                .call1(("musedata.translate",))?,
        )?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}
