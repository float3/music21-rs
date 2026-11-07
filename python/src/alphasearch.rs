//! music21's `alpha.analysis.search`: runs of notes climbing or falling
//! through a scale.
//!
//! The search is the crate's `alpha::analysis::search`, over the stream's
//! own notes read as values at the offsets the stream gives them. A run
//! found is handed back as music21 hands it back: a stream holding the
//! caller's own notes at those offsets -- this package's stream for one of
//! this package's, music21's otherwise -- beside the way it went --
//! music21's `Direction` member where music21 is there to ask, and the
//! text it stands for where not.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString};

use music21::alpha::analysis::search::{
    PitchAttribute, RunDirection, ScaleRunOptions, find_consecutive_scale,
};
use music21::{Stream as RsStream, StreamElement};

use crate::Walkable;

/// The names this module hands out.
pub const NAMES: &[&str] = &["findConsecutiveScale", "SearchModuleException"];

pyo3::create_exception!(
    music21_rs_facade,
    SearchModuleException,
    crate::Music21Exception
);

/// The pitch attribute music21's `comparisonAttribute` names.
fn comparison_of(attribute: &str) -> PyResult<PitchAttribute> {
    match attribute {
        "name" => Ok(PitchAttribute::Name),
        "nameWithOctave" => Ok(PitchAttribute::NameWithOctave),
        "pitchClass" => Ok(PitchAttribute::PitchClass),
        "step" => Ok(PitchAttribute::Step),
        other => Err(pyo3::exceptions::PyAttributeError::new_err(format!(
            "'Pitch' object has no attribute '{other}'"
        ))),
    }
}

/// Whether music21 would take this for a concrete scale.
fn is_concrete(scale: &Bound<'_, PyAny>) -> PyResult<bool> {
    if scale
        .extract::<PyRef<'_, crate::scale::ConcreteScale>>()
        .is_ok()
    {
        return Ok(true);
    }
    Ok(scale.hasattr("getPitches")?
        && scale
            .getattr("isConcrete")
            .and_then(|flag| flag.is_truthy())
            .unwrap_or(false))
}

/// The way a run went, as music21's `Direction` names it.
fn direction_object<'py>(
    py: Python<'py>,
    direction: Option<RunDirection>,
) -> PyResult<Bound<'py, PyAny>> {
    let (member, text) = match direction {
        None => return Ok(py.None().into_bound(py)),
        Some(RunDirection::Ascending) => ("ASCENDING", "ascending"),
        Some(RunDirection::Descending) => ("DESCENDING", "descending"),
    };
    match crate::real_music21(py, "music21.scale.intervalNetwork") {
        Ok(network) => network.getattr("Direction")?.getattr(member),
        Err(_) => Ok(PyString::new(py, text).into_any()),
    }
}

/// A source's notes read as the crate's stream, beside the object each was
/// read from and its offset, by the same position.
struct ReadNotes<'py> {
    stream: RsStream,
    objects: Vec<(f64, Bound<'py, PyAny>)>,
}

/// The source's notes at their offsets, in the order music21's copy of it
/// holds them, each beside the object it was read from.
fn notes_of<'py>(source: &Bound<'py, PyAny>) -> PyResult<ReadNotes<'py>> {
    let mut found = Vec::new();
    for element in source.walk()? {
        let element = element?;
        let Some(value @ StreamElement::Note(_)) = crate::stream::element_written(&element)? else {
            continue;
        };
        let offset: f64 = source
            .call_method1("elementOffset", (&element,))?
            .extract()?;
        found.push((offset, element, value));
    }
    found.sort_by(|left, right| left.0.total_cmp(&right.0));

    let mut read = RsStream::new();
    let mut objects = Vec::with_capacity(found.len());
    for (offset, element, value) in found {
        read.insert(offset, value);
        objects.push((offset, element));
    }
    Ok(ReadNotes {
        stream: read,
        objects,
    })
}

/// An empty stream of the same make as the source: this package's for one
/// of this package's, music21's otherwise.
fn empty_like<'py>(source: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = source.py();
    if source.is_instance_of::<crate::stream::Stream>() {
        return py.get_type::<crate::stream::Stream>().call0();
    }
    crate::stream::stream_class(py, "Stream")?.call0()
}

/// music21's `findConsecutiveScale`: every run of a stream's notes going
/// one way through a scale, a degree at a time, that reaches at least
/// `degreesRequired` different degrees, each as a dictionary of the stream
/// of its notes and the way it went.
#[pyfunction]
#[pyo3(signature = (
    source,
    targetScale,
    degreesRequired = 5,
    stepSize = 1,
    comparisonAttribute = "name".to_string(),
    repeatsAllowed = true,
    restsAllowed = false,
))]
fn findConsecutiveScale<'py>(
    source: &Bound<'py, PyAny>,
    targetScale: &Bound<'py, PyAny>,
    degreesRequired: usize,
    stepSize: usize,
    comparisonAttribute: String,
    #[pyo3(from_py_with = crate::truthy)] repeatsAllowed: bool,
    #[pyo3(from_py_with = crate::truthy)] restsAllowed: bool,
) -> PyResult<Bound<'py, PyList>> {
    // A rest never breaks a run either way: music21 reads only pitches.
    let _ = restsAllowed;
    let py = source.py();
    if !is_concrete(targetScale)? {
        return Err(SearchModuleException::new_err(
            "scale must be a concrete scale class",
        ));
    }
    let scale = crate::scale::crate_scale(targetScale)?;
    let mut options = ScaleRunOptions::default();
    options.degrees_required = degreesRequired;
    options.step_size = stepSize;
    options.comparison = comparison_of(&comparisonAttribute)?;
    options.repeats_allowed = repeatsAllowed;

    let ReadNotes {
        stream: read,
        objects,
    } = notes_of(source)?;
    let runs = find_consecutive_scale(&read, &scale, &options)
        .map_err(|error| crate::scale::ScaleException::new_err(crate::pitch::message(&error)))?;

    let out = PyList::empty(py);
    for run in runs {
        let stream = empty_like(source)?;
        for place in &run.elements {
            let (offset, element) = &objects[*place];
            stream.call_method1("insert", (*offset, element))?;
        }
        let found = PyDict::new(py);
        found.set_item("stream", stream)?;
        found.set_item("direction", direction_object(py, run.direction)?)?;
        out.append(found)?;
    }
    Ok(out)
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(findConsecutiveScale, m)?)?;
    Ok(())
}
