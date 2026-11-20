//! music21's `graph.axis`: what a plot reads off each element for one of
//! its axes, and the ticks and labels that axis shows.
//!
//! Values are read here from whatever elements the plot hands over,
//! through their own attributes, as music21's axes read them; the ticks
//! and labels, given what was read, are the crate's `graph::axis`.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use std::collections::HashMap;

use pyo3::IntoPyObjectExt;
use pyo3::exceptions::{PyAttributeError, PyIndexError};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBool, PyDict, PyList, PyModule, PyString, PyTuple};

use music21::graph::axis::{
    MeasureMark, MeasureSpan, Octaves, PitchLabels, Scale, Tied, dynamics_max, dynamics_ticks,
    is_strong_barline, log_label_tag, measure_ticks, numeric_ticks, octave_ticks, offset_end,
    offset_ticks, pitch_ticks, quarter_length_value,
};

use super::utilities::{GraphException, accidentalLabelToUnicode, graph_error};
use super::{ClassSpec, Property, kwargs, music21_name, optional, proto};

/// The module music21 keeps these in.
pub(crate) const MODULE: &str = "music21.graph.axis";

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

/// An `OffsetEndAxis` spacing that means "the grace-note length".
const USE_GRACE_NOTE_SPACING: i64 = -1;

/// The length a grace note stands for on a position axis: a 64th note.
const GRACE_NOTE_QL: f64 = 0.0625;

/// The space a pitch-class axis spans.
const PITCH_CLASS_MAX: i64 = 11;

/// What a pitch-class axis's ticks are named by and placed by.
const PITCH_CLASS_ATTRIBUTES: (&str, &str) = ("name", "pitchClass");

/// What a pitch-space axis's ticks are named by and placed by.
const PITCH_SPACE_ATTRIBUTES: (&str, &str) = ("nameWithOctave", "ps");

/// Pairs as a list of tuples, as music21's ticks are.
fn tick_list<'py, V, L>(py: Python<'py>, ticks: Vec<(V, L)>) -> PyResult<Bound<'py, PyList>>
where
    (V, L): IntoPyObject<'py>,
{
    let list = PyList::empty(py);
    for tick in ticks {
        list.append(tick)?;
    }
    Ok(list)
}

/// Python's `int()` of a value.
fn int_of(value: &Bound<'_, PyAny>) -> PyResult<i64> {
    value
        .py()
        .import("builtins")?
        .getattr("int")?
        .call1((value,))?
        .extract()
}

// ---------------------------------------------------------------- Axis

#[pyfunction]
#[pyo3(signature = (slf, client=None, axisName=None))]
fn axis_init(
    slf: &Bound<'_, PyAny>,
    client: Option<&Bound<'_, PyAny>>,
    axisName: Option<&Bound<'_, PyAny>>,
) -> PyResult<()> {
    let py = slf.py();
    if client.is_some_and(|client| client.is_instance_of::<PyString>()) {
        return Err(GraphException::new_err(
            "Client must be a PlotStream, Stream, or None",
        ));
    }
    slf.setattr("_client", py.None())?;
    slf.setattr("_label", py.None())?;
    slf.setattr("client", client)?;
    match axisName {
        Some(name) => slf.setattr("axisName", name)?,
        None => slf.setattr("axisName", "x")?,
    }
    slf.setattr("minValue", py.None())?;
    slf.setattr("maxValue", py.None())
}

/// The axis's name and its client's class, inside its repr.
#[pyfunction]
fn axis_repr(slf: &Bound<'_, PyAny>) -> PyResult<String> {
    let client = match optional(slf, "client")? {
        Some(client) => client.get_type().name()?.to_string(),
        None => "(no client)".to_string(),
    };
    Ok(format!(
        ": {} axis for {client}",
        slf.getattr("axisName")?.str()?
    ))
}

/// The label set, or the class's `labelDefault`.
#[pyfunction]
fn label_get<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    match optional(slf, "_label")? {
        Some(label) => Ok(label),
        None => slf.getattr("labelDefault"),
    }
}

#[pyfunction]
fn label_set(slf: &Bound<'_, PyAny>, value: &Bound<'_, PyAny>) -> PyResult<()> {
    slf.setattr("_label", value)
}

/// The plot the axis belongs to, kept as a weak reference.
#[pyfunction]
fn client_get<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let held = slf.getattr("_client")?;
    if held.get_type().is(py.import("weakref")?.getattr("ref")?) {
        return held.call0();
    }
    Ok(held)
}

#[pyfunction]
fn client_set(slf: &Bound<'_, PyAny>, referent: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    let wrapped = match py.import("weakref")?.getattr("ref")?.call1((referent,)) {
        Ok(wrapped) => wrapped,
        Err(error) if error.is_instance_of::<pyo3::exceptions::PyTypeError>(py) => referent.clone(),
        Err(error) => return Err(error),
    };
    slf.setattr("_client", wrapped)
}

/// The client's stream, or the client where it is a stream itself.
#[pyfunction]
fn stream_get<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let Some(client) = optional(slf, "client")? else {
        return Ok(py.None().into_bound(py));
    };
    let stream = music21_name(py, "music21.stream", "Stream")?;
    if client.is_instance(&stream)? {
        return Ok(client);
    }
    client.getattr("streamObj")
}

/// What the axis reads off one element; 1 for an axis of no quantity.
#[pyfunction]
fn axis_extract(
    slf: &Bound<'_, PyAny>,
    n: &Bound<'_, PyAny>,
    formatDict: &Bound<'_, PyAny>,
) -> i64 {
    let _ = (slf, n, formatDict);
    1
}

/// Sets each bound not already set to the least or the most of `values`.
#[pyfunction]
#[pyo3(signature = (slf, values=None))]
fn axis_bounds(slf: &Bound<'_, PyAny>, values: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
    let py = slf.py();
    let Some(values) = values.filter(|values| values.is_truthy().unwrap_or(false)) else {
        return Ok(());
    };
    let builtins = py.import("builtins")?;
    if optional(slf, "minValue")?.is_none() {
        slf.setattr("minValue", builtins.call_method1("min", (values,))?)?;
    }
    if optional(slf, "maxValue")?.is_none() {
        slf.setattr("maxValue", builtins.call_method1("max", (values,))?)?;
    }
    Ok(())
}

/// About ten ticks a power of ten apart over the axis's range.
#[pyfunction]
fn axis_ticks<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
    let py = slf.py();
    let bound = |name: &str| -> PyResult<Option<f64>> {
        optional(slf, name)?
            .map(|value| value.extract())
            .transpose()
    };
    let ticks = numeric_ticks(bound("minValue")?, bound("maxValue")?).map_err(graph_error)?;
    tick_list(
        py,
        ticks
            .into_iter()
            .map(|tick| (tick, tick.to_string()))
            .collect(),
    )
}

/// Tidies the plot's data once extracted; nothing, for most axes.
#[pyfunction]
#[pyo3(signature = (slf, dataList=None))]
fn axis_post(slf: &Bound<'_, PyAny>, dataList: Option<&Bound<'_, PyAny>>) {
    let _ = (slf, dataList);
}

// ---------------------------------------------------------------- PitchAxis

#[pyfunction]
#[pyo3(signature = (slf, client=None, axisName=None))]
fn pitch_init(
    slf: &Bound<'_, PyAny>,
    client: Option<&Bound<'_, PyAny>>,
    axisName: Option<&Bound<'_, PyAny>>,
) -> PyResult<()> {
    axis_init(slf, client, axisName)?;
    slf.setattr("showOctaves", "few")?;
    slf.setattr("showEnharmonic", true)?;
    slf.setattr("blankLabelUnused", true)?;
    slf.setattr("hideUnused", true)
}

/// Ticks with their labels' accidentals as music symbols.
#[pyfunction]
fn makePitchLabelsUnicode<'py>(ticks: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
    let py = ticks.py();
    let post = PyList::empty(py);
    for tick in ticks.try_iter()? {
        let (value, label) = super::pair(&tick?)?;
        post.append((value, accidentalLabelToUnicode(&label)?))?;
    }
    Ok(post)
}

/// How the axis writes octaves, from its `showOctaves`.
fn octaves_of(slf: &Bound<'_, PyAny>) -> PyResult<Octaves> {
    let shown = slf.getattr("showOctaves")?;
    if !shown.is_truthy()? {
        return Ok(Octaves::Hidden);
    }
    if shown.eq("few")? {
        return Ok(Octaves::First);
    }
    Ok(Octaves::All)
}

/// A crate error from a Python one, for a closure the crate calls.
fn crate_error(error: PyErr) -> music21::Error {
    music21::Error::Value(error.to_string())
}

/// The ticks of a pitch axis: each pitch found named by `counter` and
/// placed by `compare`, as the axis's `showEnharmonic`, `hideUnused`,
/// `blankLabelUnused` and `showOctaves` say.
#[pyfunction]
fn _pitchTickHelper<'py>(
    slf: &Bound<'py, PyAny>,
    attributeCounter: &str,
    attributeCompare: &str,
) -> PyResult<Bound<'py, PyList>> {
    let py = slf.py();
    let pitch_class = music21_name(py, "music21.pitch", "Pitch")?;

    // How often each name was found, in the order found.
    let mut counts: Vec<(String, usize)> = Vec::new();
    if let Some(stream) = optional(slf, "stream")? {
        let counted = music21_name(py, "music21.analysis.pitchAnalysis", "pitchAttributeCount")?
            .call1((stream, attributeCounter))?;
        for item in counted.call_method0("items")?.try_iter()? {
            counts.push(item?.extract()?);
        }
    }
    let mut values: HashMap<String, f64> = HashMap::new();
    for (name, _) in &counts {
        let value = pitch_class
            .call1((name,))?
            .getattr(attributeCompare)?
            .extract()?;
        values.insert(name.clone(), value);
    }

    let labels = PitchLabels {
        octaves: octaves_of(slf)?,
        show_enharmonic: slf.getattr("showEnharmonic")?.is_truthy()?,
        blank_label_unused: slf.getattr("blankLabelUnused")?.is_truthy()?,
        hide_unused: slf.getattr("hideUnused")?.is_truthy()?,
    };
    let min = int_of(&slf.getattr("minValue")?)?;
    let max = int_of(&slf.getattr("maxValue")?)?;
    let unused = |tick: i64| -> music21::Result<String> {
        let made = pitch_class.call0().map_err(crate_error)?;
        made.setattr(attributeCompare, tick).map_err(crate_error)?;
        made.getattr(attributeCounter)
            .and_then(|label| label.extract())
            .map_err(crate_error)
    };
    let ticks = pitch_ticks(labels, min, max, &counts, |name| Ok(values[name]), unused)
        .map_err(graph_error)?;
    tick_list(py, ticks)
}

// ---------------------------------------------------------------- PitchClassAxis

#[pyfunction]
#[pyo3(signature = (slf, client=None, axisName=None))]
fn pitch_class_init(
    slf: &Bound<'_, PyAny>,
    client: Option<&Bound<'_, PyAny>>,
    axisName: Option<&Bound<'_, PyAny>>,
) -> PyResult<()> {
    slf.setattr("showOctaves", false)?;
    pitch_init(slf, client, axisName)?;
    slf.setattr("minValue", 0)?;
    slf.setattr("maxValue", PITCH_CLASS_MAX)
}

/// An element's pitch class, where it has a pitch.
#[pyfunction]
#[pyo3(name = "extractOneElement")]
fn pitch_class_extract<'py>(
    slf: &Bound<'py, PyAny>,
    n: &Bound<'py, PyAny>,
    formatDict: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let _ = formatDict;
    if !n.hasattr("pitch")? {
        return Ok(slf.py().None().into_bound(slf.py()));
    }
    n.getattr("pitch")?.getattr("pitchClass")
}

/// A tick for each pitch class, named by the spellings found of it.
#[pyfunction]
#[pyo3(name = "ticks")]
fn pitch_class_ticks<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let (counter, compare) = PITCH_CLASS_ATTRIBUTES;
    slf.call_method1("_pitchTickHelper", (counter, compare))
}

// ---------------------------------------------------------------- PitchSpaceAxis

/// An element's pitch space number, where it has a pitch.
#[pyfunction]
#[pyo3(name = "extractOneElement")]
fn pitch_space_extract<'py>(
    slf: &Bound<'py, PyAny>,
    n: &Bound<'py, PyAny>,
    formatDict: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let _ = formatDict;
    if !n.hasattr("pitch")? {
        return Ok(slf.py().None().into_bound(slf.py()));
    }
    n.getattr("pitch")?.getattr("ps")
}

/// A tick for each pitch space number, named by the spellings found of it.
#[pyfunction]
#[pyo3(name = "ticks", signature = (slf, dataMin=None, dataMax=None))]
fn pitch_space_ticks<'py>(
    slf: &Bound<'py, PyAny>,
    dataMin: Option<&Bound<'py, PyAny>>,
    dataMax: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let _ = (dataMin, dataMax);
    let (counter, compare) = PITCH_SPACE_ATTRIBUTES;
    slf.call_method1("_pitchTickHelper", (counter, compare))
}

// ---------------------------------------------------------------- PitchSpaceOctaveAxis

#[pyfunction]
#[pyo3(signature = (slf, client=None, axisName=None))]
fn octave_init(
    slf: &Bound<'_, PyAny>,
    client: Option<&Bound<'_, PyAny>>,
    axisName: Option<&Bound<'_, PyAny>>,
) -> PyResult<()> {
    pitch_init(slf, client, axisName)?;
    slf.setattr("startNameWithOctave", "C2")
}

/// A tick at each octave of `startNameWithOctave` in the axis's range,
/// whether or not the stream has it.
#[pyfunction]
#[pyo3(name = "ticks")]
fn octave_ticks_py<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
    let start: String = slf.getattr("startNameWithOctave")?.extract()?;
    let min: f64 = slf.getattr("minValue")?.extract()?;
    let max: f64 = slf.getattr("maxValue")?.extract()?;
    let ticks = octave_ticks(&start, min, max).map_err(graph_error)?;
    tick_list(slf.py(), ticks)
}

// ---------------------------------------------------------------- PositionAxis

#[pyfunction]
#[pyo3(signature = (slf, client=None, axisName=None))]
fn position_init(
    slf: &Bound<'_, PyAny>,
    client: Option<&Bound<'_, PyAny>>,
    axisName: Option<&Bound<'_, PyAny>>,
) -> PyResult<()> {
    axis_init(slf, client, axisName)?;
    slf.setattr("graceNoteQL", GRACE_NOTE_QL)
}

// ---------------------------------------------------------------- OffsetAxis

#[pyfunction]
#[pyo3(signature = (slf, client=None, axisName=None))]
fn offset_init(
    slf: &Bound<'_, PyAny>,
    client: Option<&Bound<'_, PyAny>>,
    axisName: Option<&Bound<'_, PyAny>>,
) -> PyResult<()> {
    let py = slf.py();
    position_init(slf, client, axisName)?;
    slf.setattr("useMeasures", py.None())?;
    slf.setattr("offsetStepSize", 10)?;
    slf.setattr("mostMeasureTicksToShow", 20)?;
    slf.setattr("minMaxMeasureOnly", false)
}

/// Where an element stands in the axis's stream.
#[pyfunction]
#[pyo3(name = "extractOneElement")]
fn offset_extract<'py>(
    slf: &Bound<'py, PyAny>,
    n: &Bound<'py, PyAny>,
    formatDict: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let _ = formatDict;
    n.call_method1("getOffsetInHierarchy", (slf.getattr("stream")?,))
}

/// The label set, or `'Measure Number'` where measures are used, else
/// `'Offset'`.
#[pyfunction]
fn offset_label<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    if let Some(label) = optional(slf, "_label")? {
        return Ok(label);
    }
    let use_measures = match optional(slf, "useMeasures")? {
        Some(set) => set,
        None => slf.call_method0("setUseMeasuresFromOffsetMap")?,
    };
    let label = if use_measures.is_truthy()? {
        "Measure Number"
    } else {
        "Offset"
    };
    Ok(PyString::new(py, label).into_any())
}

/// The stream's span, or nought to the most of `values` (or ten) where
/// there is no stream.
#[pyfunction]
#[pyo3(name = "setBoundariesFromData", signature = (slf, values=None))]
fn offset_bounds(slf: &Bound<'_, PyAny>, values: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
    let py = slf.py();
    let stream = slf.getattr("stream")?;
    let read = stream
        .getattr("lowestOffset")
        .and_then(|low| Ok((low, stream.getattr("highestTime")?)));
    match read {
        Ok((low, high)) => {
            slf.setattr("minValue", low)?;
            slf.setattr("maxValue", high)
        }
        Err(error) if error.is_instance_of::<PyAttributeError>(py) => {
            slf.setattr("minValue", 0)?;
            match values.filter(|values| values.is_truthy().unwrap_or(false)) {
                Some(values) => slf.setattr(
                    "maxValue",
                    py.import("builtins")?.call_method1("max", (values,))?,
                ),
                None => slf.setattr("maxValue", 10),
            }
        }
        Err(error) => Err(error),
    }
}

/// Measure ticks where the stream has measures, else ticks every
/// `offsetStepSize`.
#[pyfunction]
#[pyo3(name = "ticks")]
fn offset_ticks_py<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let offset_map = slf.call_method0("getOffsetMap")?;
    slf.call_method1("setUseMeasuresFromOffsetMap", (&offset_map,))?;
    if slf.getattr("useMeasures")?.is_truthy()? {
        return slf.call_method1(
            "_measureTicks",
            (
                slf.getattr("minValue")?,
                slf.getattr("maxValue")?,
                offset_map,
            ),
        );
    }
    let min: f64 = slf.getattr("minValue")?.extract()?;
    let max: f64 = slf.getattr("maxValue")?.extract()?;
    let step: i64 = slf.getattr("offsetStepSize")?.extract()?;
    let ticks = offset_ticks(min, max, step);
    Ok(tick_list(
        py,
        ticks
            .into_iter()
            .map(|tick| (tick, tick.to_string()))
            .collect(),
    )?
    .into_any())
}

/// The ticks of the measures starting from `dataMin` to `dataMax`.
#[pyfunction]
fn _measureTicks<'py>(
    slf: &Bound<'py, PyAny>,
    dataMin: f64,
    dataMax: f64,
    offsetMap: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    let py = slf.py();
    let mut marks = Vec::new();
    for item in offsetMap.call_method0("items")?.try_iter()? {
        let (offset, measures): (f64, Bound<'py, PyAny>) = item?.extract()?;
        let measure = measures.get_item(0)?;
        let strong = match optional(&measure, "rightBarline")? {
            Some(barline) => is_strong_barline(&barline.getattr("type")?.extract::<String>()?),
            None => false,
        };
        marks.push(MeasureMark {
            offset,
            number: measure.getattr("number")?.str()?.to_string(),
            strong,
        });
    }
    let span = if slf.getattr("minMaxMeasureOnly")?.is_truthy()? {
        MeasureSpan::Ends
    } else {
        MeasureSpan::Spaced(slf.getattr("mostMeasureTicksToShow")?.extract()?)
    };
    let ticks = measure_ticks(&marks, dataMin, dataMax, span)
        .map_err(|error| PyIndexError::new_err(error.to_string()))?;
    tick_list(py, ticks)
}

/// The measures of the stream's first part, or its own, by where each
/// starts; empty where there are none.
#[pyfunction]
fn getOffsetMap<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let Some(stream) = optional(slf, "stream")? else {
        return Ok(PyDict::new(py).into_any());
    };
    let measure = music21_name(py, "music21.stream", "Measure")?;
    let classes = PyList::new(py, [&measure])?;
    if stream.call_method0("hasPartLikeStreams")?.is_truthy()? {
        let part = stream
            .call_method1(
                "getElementsByClass",
                (music21_name(py, "music21.stream", "Stream")?,),
            )?
            .call_method0("first")?;
        return part.call_method1("measureOffsetMap", (classes,));
    }
    if stream.call_method0("hasMeasures")?.is_truthy()? {
        return stream.call_method1("measureOffsetMap", (classes,));
    }
    Ok(PyDict::new(py).into_any())
}

/// `useMeasures`, set from whether there are measures where it is unset.
#[pyfunction]
#[pyo3(signature = (slf, offsetMap=None))]
fn setUseMeasuresFromOffsetMap<'py>(
    slf: &Bound<'py, PyAny>,
    offsetMap: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    if let Some(set) = optional(slf, "useMeasures")? {
        return Ok(set);
    }
    let offset_map = match offsetMap.filter(|map| !map.is_none()) {
        Some(map) => map.clone(),
        None => slf.call_method0("getOffsetMap")?,
    };
    slf.setattr("useMeasures", offset_map.is_truthy()?)?;
    slf.getattr("useMeasures")
}

// ---------------------------------------------------------------- QuarterLengthAxis

#[pyfunction]
#[pyo3(signature = (slf, client=None, axisName=None))]
fn ql_init(
    slf: &Bound<'_, PyAny>,
    client: Option<&Bound<'_, PyAny>>,
    axisName: Option<&Bound<'_, PyAny>>,
) -> PyResult<()> {
    position_init(slf, client, axisName)?;
    slf.setattr("useLogScale", true)?;
    slf.setattr("useDurationNames", false)
}

/// An element's length where it stands on the axis.
#[pyfunction]
#[pyo3(name = "extractOneElement")]
fn ql_extract<'py>(
    slf: &Bound<'py, PyAny>,
    n: &Bound<'py, PyAny>,
    formatDict: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let _ = formatDict;
    let length = n.getattr("duration")?.getattr("quarterLength")?;
    slf.call_method1("dataFromQL", (length,))
}

/// Where a length stands: its log, base two, where `useLogScale` is set.
#[pyfunction]
fn dataFromQL(slf: &Bound<'_, PyAny>, ql: f64) -> PyResult<f64> {
    let scale = if slf.getattr("useLogScale")?.is_truthy()? {
        Scale::Log2
    } else {
        Scale::Linear
    };
    let grace: f64 = slf.getattr("graceNoteQL")?.extract()?;
    quarter_length_value(ql, scale, grace).map_err(graph_error)
}

/// A tick for each length the stream's notes and chords have, labelled
/// by the length, or by its name where `useDurationNames` is set.
#[pyfunction]
#[pyo3(name = "ticks")]
fn ql_ticks<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
    let py = slf.py();
    let stream = slf.getattr("stream")?;
    if !stream.is_truthy()? {
        return Ok(PyList::empty(py));
    }
    let client = slf.getattr("client")?;
    let source = if client.getattr("recurse")?.is_truthy()? {
        stream.call_method0("recurse")?
    } else {
        stream.call_method0("iter")?
    };
    let source =
        source.call_method1("getElementsByClass", (client.getattr("classFilterList")?,))?;
    let mapping = music21_name(py, "music21.analysis.elements", "attributeCount")?
        .call1((source, "quarterLength"))?;

    let builtins = py.import("builtins")?;
    let names = slf.getattr("useDurationNames")?.is_truthy()?;
    let ticks = PyList::empty(py);
    for length in builtins.call_method1("sorted", (mapping,))?.try_iter()? {
        let length = length?;
        let x = slf.call_method1("dataFromQL", (&length,))?;
        let label = if names {
            music21_name(py, "music21.duration", "Duration")?
                .call1((&length,))?
                .getattr("fullName")?
        } else {
            builtins
                .call_method1("round", (&length, 2))?
                .str()?
                .into_any()
        };
        ticks.append((x, label))?;
    }
    Ok(ticks)
}

/// What the label gains for the scale: `' ($log_2$)'`, or the base given
/// to `useLogScale`, or nothing where it is off.
#[pyfunction]
fn labelLogTag(slf: &Bound<'_, PyAny>) -> PyResult<String> {
    let scale = slf.getattr("useLogScale")?;
    if scale.is_instance_of::<PyBool>() {
        let base = scale.is_truthy()?.then_some(2);
        return Ok(log_label_tag(base));
    }
    Ok(log_label_tag(Some(scale.extract()?)))
}

#[pyfunction]
fn ql_label<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    label_get(slf)?.add(labelLogTag(slf)?)
}

#[pyfunction]
fn ql_label_set(slf: &Bound<'_, PyAny>, value: &Bound<'_, PyAny>) -> PyResult<()> {
    let _ = (slf, value);
    Err(PyAttributeError::new_err(
        "'super' object has no attribute 'label'",
    ))
}

/// A length's log, base two; a grace note's length is `graceNoteQL`.
#[pyfunction]
fn remapQuarterLength(slf: &Bound<'_, PyAny>, x: f64) -> PyResult<f64> {
    let grace: f64 = slf.getattr("graceNoteQL")?.extract()?;
    quarter_length_value(x, Scale::Log2, grace).map_err(graph_error)
}

// ---------------------------------------------------------------- OffsetEndAxis

#[pyfunction]
#[pyo3(signature = (slf, client=None, axisName=None, noteSpacing=None))]
fn offset_end_init(
    slf: &Bound<'_, PyAny>,
    client: Option<&Bound<'_, PyAny>>,
    axisName: Option<&Bound<'_, PyAny>>,
    noteSpacing: Option<&Bound<'_, PyAny>>,
) -> PyResult<()> {
    offset_init(slf, client, axisName)?;
    let spacing = match noteSpacing {
        Some(spacing) => spacing.clone(),
        None => USE_GRACE_NOTE_SPACING.into_bound_py_any(slf.py())?,
    };
    if spacing.eq(USE_GRACE_NOTE_SPACING)? {
        return slf.setattr("noteSpacing", slf.getattr("graceNoteQL")?);
    }
    slf.setattr("noteSpacing", spacing)
}

/// Where an element starts and how long its bar is drawn: a little short
/// where it is not tied on, so untied notes stand apart.
#[pyfunction]
#[pyo3(name = "extractOneElement")]
fn offset_end_extract(
    slf: &Bound<'_, PyAny>,
    n: &Bound<'_, PyAny>,
    formatDict: &Bound<'_, PyAny>,
) -> PyResult<(f64, f64)> {
    let _ = formatDict;
    let offset: f64 = n
        .call_method1("getOffsetInHierarchy", (slf.getattr("stream")?,))?
        .extract()?;
    let length: f64 = n.getattr("duration")?.getattr("quarterLength")?.extract()?;
    let tied = match optional(n, "tie")? {
        Some(tie)
            if ["start", "continue"]
                .contains(&tie.getattr("type")?.extract::<String>()?.as_str()) =>
        {
            Tied::Onward
        }
        _ => Tied::Not,
    };
    let spacing: f64 = slf.getattr("noteSpacing")?.extract()?;
    Ok(offset_end(offset, length, spacing, tied))
}

// ---------------------------------------------------------------- DynamicsAxis

/// Every dynamic where no values are given, else their span, as whole
/// numbers.
#[pyfunction]
#[pyo3(name = "setBoundariesFromData", signature = (slf, values=None))]
fn dynamics_bounds(slf: &Bound<'_, PyAny>, values: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
    let Some(values) = values.filter(|values| !values.is_none()) else {
        slf.setattr("minValue", 0)?;
        return slf.setattr("maxValue", dynamics_max());
    };
    axis_bounds(slf, Some(values))?;
    slf.setattr("minValue", int_of(&slf.getattr("minValue")?)?)?;
    slf.setattr("maxValue", int_of(&slf.getattr("maxValue")?)?)
}

/// A tick for each dynamic in the axis's range, written in TeX italics.
#[pyfunction]
#[pyo3(name = "ticks")]
fn dynamics_ticks_py<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
    if optional(slf, "minValue")?.is_none() {
        slf.call_method0("setBoundariesFromData")?;
    }
    let min: i64 = slf.getattr("minValue")?.extract()?;
    let max: i64 = slf.getattr("maxValue")?.extract()?;
    let ticks =
        dynamics_ticks(min, max).map_err(|error| PyIndexError::new_err(error.to_string()))?;
    tick_list(slf.py(), ticks)
}

// ---------------------------------------------------------------- CountingAxis

#[pyfunction]
#[pyo3(signature = (slf, client=None, axisName=None))]
fn counting_init(
    slf: &Bound<'_, PyAny>,
    client: Option<&Bound<'_, PyAny>>,
    axisName: Option<&Bound<'_, PyAny>>,
) -> PyResult<()> {
    let py = slf.py();
    let name = match axisName {
        Some(name) => name.clone(),
        None => PyString::new(py, "y").into_any(),
    };
    axis_init(slf, client, Some(&name))?;
    slf.setattr("countAxes", "x")
}

/// The client's data with each value of the counted axes once, its count
/// on this axis and the formats of all its points merged; answers it.
#[pyfunction]
#[pyo3(name = "postProcessData")]
fn counting_post<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let Some(client) = optional(slf, "client")? else {
        return Ok(PyList::empty(py).into_any());
    };
    let mut count_axes = slf.getattr("countAxes")?;
    if !crate::plumbing::classtools::isIterable(&count_axes)? {
        count_axes = PyTuple::new(py, [count_axes])?.into_any();
    }
    let data_map = slf.getattr("axisDataMap")?;
    let mut indices: Vec<Bound<'py, PyAny>> = Vec::new();
    for name in count_axes.try_iter()? {
        indices.push(data_map.get_item(name?)?);
    }
    let this_index: usize = data_map.get_item(slf.getattr("axisName")?)?.extract()?;
    let selector = py
        .import("operator")?
        .getattr("itemgetter")?
        .call1(PyTuple::new(py, &indices)?)?;
    let data = client.getattr("data")?;

    // The formats of every point of a value, merged.
    let relevant = PyList::empty(py);
    let formats = PyDict::new(py);
    for point in data.try_iter()? {
        let point = point?;
        let key = selector.call1((&point,))?;
        relevant.append(&key)?;
        let format = point.get_item(-1)?;
        if !format.is_instance_of::<PyDict>() {
            continue;
        }
        match formats.get_item(&key)? {
            Some(seen) => {
                seen.call_method1("update", (format,))?;
            }
            None => formats.set_item(&key, format)?,
        }
    }

    let counter = py
        .import("collections")?
        .getattr("Counter")?
        .call1((relevant,))?;
    let rows = PyList::empty(py);
    for key in counter.try_iter()? {
        let key = key?;
        let inner: Vec<Bound<'py, PyAny>> = (0..=indices.len())
            .map(|_| py.None().into_bound(py))
            .collect();
        let inner = PyList::new(py, inner)?;
        if indices.len() > 1 {
            for index in &indices {
                inner.set_item(index.extract()?, key.get_item(index)?)?;
            }
        } else {
            inner.set_item(indices[0].extract()?, &key)?;
        }
        inner.set_item(this_index, counter.get_item(&key)?)?;
        let format = match formats.get_item(&key)? {
            Some(format) => format,
            None => PyDict::new(py).into_any(),
        };
        inner.append(format)?;
        rows.append(inner.to_tuple())?;
    }
    rows.sort()?;
    client.setattr("data", &rows)?;
    client.getattr("data")
}

// ---------------------------------------------------------------- the module

/// A Rust function, to be a method.
macro_rules! f {
    ($m:expr, $function:path) => {
        wrap_pyfunction!($function, $m)?.into_any()
    };
}

/// An axis class: its label, quantities, and methods over its base's.
struct AxisClass<'py> {
    name: &'static str,
    doc: &'static str,
    label: Option<&'static str>,
    quantities: &'static [&'static str],
    methods: Vec<(&'static str, Bound<'py, PyAny>)>,
    properties: Vec<Property<'py>>,
}

fn axis_class<'py>(
    m: &Bound<'py, PyModule>,
    base: &Bound<'py, PyAny>,
    made: AxisClass<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = m.py();
    let mut spec = ClassSpec::new(made.name, made.doc, vec![base.clone()]);
    if let Some(label) = made.label {
        spec.attributes
            .push(("labelDefault", PyString::new(py, label).into_any()));
    }
    spec.attributes
        .push(("quantities", super::strings(py, made.quantities)?));
    spec.methods = made.methods;
    spec.properties = made.properties;
    let class = spec.build(MODULE)?;
    m.add(made.name, &class)?;
    Ok(class)
}

/// music21's `graph.axis`, built.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        let m = &m;
        m.add("USE_GRACE_NOTE_SPACING", USE_GRACE_NOTE_SPACING)?;

        let mut spec = ClassSpec::new(
            "Axis",
            "What one axis of a plot shows: what it reads off each element, its \
             bounds, ticks and label. Its client is the plot, or `None`.",
            vec![proto(py)],
        );
        spec.attributes = vec![
            ("labelDefault", PyString::new(py, "an axis").into_any()),
            (
                "axisDataMap",
                kwargs!(py; "x" => 0, "y" => 1, "z" => 2).into_any(),
            ),
            (
                "quantities",
                super::strings(py, &["generic", "one", "nothing", "blank"])?,
            ),
        ];
        spec.methods = vec![
            ("__init__", f!(m, axis_init)),
            ("__getstate__", f!(m, super::state_of)),
            ("_reprInternal", f!(m, axis_repr)),
            ("extractOneElement", f!(m, axis_extract)),
            ("setBoundariesFromData", f!(m, axis_bounds)),
            ("ticks", f!(m, axis_ticks)),
            ("postProcessData", f!(m, axis_post)),
        ];
        spec.properties = vec![
            Property {
                name: "label",
                getter: f!(m, label_get),
                setter: Some(f!(m, label_set)),
            },
            Property {
                name: "client",
                getter: f!(m, client_get),
                setter: Some(f!(m, client_set)),
            },
            Property {
                name: "stream",
                getter: f!(m, stream_get),
                setter: None,
            },
        ];
        let axis = spec.build(MODULE)?;
        m.add("Axis", &axis)?;

        let mut pitch_spec = ClassSpec::new("PitchAxis", "An axis of pitches.", vec![axis.clone()]);
        pitch_spec.attributes = vec![
            ("labelDefault", PyString::new(py, "Pitch").into_any()),
            ("quantities", super::strings(py, &["pitchGeneric"])?),
        ];
        pitch_spec.methods = vec![
            ("__init__", f!(m, pitch_init)),
            ("_pitchTickHelper", f!(m, _pitchTickHelper)),
        ];
        pitch_spec.statics = vec![("makePitchLabelsUnicode", f!(m, makePitchLabelsUnicode))];
        let pitch = pitch_spec.build(MODULE)?;
        m.add("PitchAxis", &pitch)?;

        axis_class(
            m,
            &pitch,
            AxisClass {
                name: "PitchClassAxis",
                doc: "An axis of pitch classes, from 0 to 11 unless set otherwise.",
                label: Some("Pitch Class"),
                quantities: &["pitchClass", "pitchclass", "pc"],
                methods: vec![
                    ("__init__", f!(m, pitch_class_init)),
                    ("extractOneElement", f!(m, pitch_class_extract)),
                    ("ticks", f!(m, pitch_class_ticks)),
                ],
                properties: vec![],
            },
        )?;
        let space = axis_class(
            m,
            &pitch,
            AxisClass {
                name: "PitchSpaceAxis",
                doc: "An axis of pitch space numbers, as MIDI numbers are.",
                label: Some("Pitch"),
                quantities: &["pitchSpace", "pitch", "pitchspace", "ps"],
                methods: vec![
                    ("extractOneElement", f!(m, pitch_space_extract)),
                    ("ticks", f!(m, pitch_space_ticks)),
                ],
                properties: vec![],
            },
        )?;
        axis_class(
            m,
            &space,
            AxisClass {
                name: "PitchSpaceOctaveAxis",
                doc: "A pitch space axis ticked at each octave of `startNameWithOctave`.",
                label: Some("Octave"),
                quantities: &["octave", "octaves"],
                methods: vec![
                    ("__init__", f!(m, octave_init)),
                    ("ticks", f!(m, octave_ticks_py)),
                ],
                properties: vec![],
            },
        )?;
        let position = axis_class(
            m,
            &axis,
            AxisClass {
                name: "PositionAxis",
                doc: "An axis of positions; a grace note stands for `graceNoteQL`.",
                label: Some("Position"),
                quantities: &["position", "positions"],
                methods: vec![("__init__", f!(m, position_init))],
                properties: vec![],
            },
        )?;
        let offset = axis_class(
            m,
            &position,
            AxisClass {
                name: "OffsetAxis",
                doc: "An axis of offsets, ticked by measure number where the stream has \
                  measures.",
                label: Some("Offset"),
                quantities: &["offset", "measure", "offsets", "measures", "time"],
                methods: vec![
                    ("__init__", f!(m, offset_init)),
                    ("extractOneElement", f!(m, offset_extract)),
                    ("setBoundariesFromData", f!(m, offset_bounds)),
                    ("ticks", f!(m, offset_ticks_py)),
                    ("_measureTicks", f!(m, _measureTicks)),
                    ("getOffsetMap", f!(m, getOffsetMap)),
                    (
                        "setUseMeasuresFromOffsetMap",
                        f!(m, setUseMeasuresFromOffsetMap),
                    ),
                ],
                properties: vec![Property {
                    name: "label",
                    getter: f!(m, offset_label),
                    setter: Some(f!(m, label_set)),
                }],
            },
        )?;
        axis_class(
            m,
            &position,
            AxisClass {
                name: "QuarterLengthAxis",
                doc: "An axis of quarter lengths, by their log, base two, unless \
                  `useLogScale` is off.",
                label: Some("Quarter Length"),
                quantities: &[
                    "quarterLength",
                    "ql",
                    "quarterlengths",
                    "durations",
                    "duration",
                ],
                methods: vec![
                    ("__init__", f!(m, ql_init)),
                    ("extractOneElement", f!(m, ql_extract)),
                    ("dataFromQL", f!(m, dataFromQL)),
                    ("ticks", f!(m, ql_ticks)),
                    ("labelLogTag", f!(m, labelLogTag)),
                    ("remapQuarterLength", f!(m, remapQuarterLength)),
                ],
                properties: vec![Property {
                    name: "label",
                    getter: f!(m, ql_label),
                    setter: Some(f!(m, ql_label_set)),
                }],
            },
        )?;
        axis_class(
            m,
            &offset,
            AxisClass {
                name: "OffsetEndAxis",
                doc: "An offset axis giving each element's start and how long it is drawn.",
                label: None,
                quantities: &["offsetEnd", "timespans", "timespan"],
                methods: vec![
                    ("__init__", f!(m, offset_end_init)),
                    ("extractOneElement", f!(m, offset_end_extract)),
                ],
                properties: vec![],
            },
        )?;
        axis_class(
            m,
            &axis,
            AxisClass {
                name: "DynamicsAxis",
                doc: "An axis of dynamics, by their place in `dynamics.shortNames`.",
                label: Some("Dynamic"),
                quantities: &["dynamic", "dynamics", "volume"],
                methods: vec![
                    ("setBoundariesFromData", f!(m, dynamics_bounds)),
                    ("ticks", f!(m, dynamics_ticks_py)),
                ],
                properties: vec![],
            },
        )?;
        axis_class(
            m,
            &axis,
            AxisClass {
                name: "CountingAxis",
                doc: "An axis counting the values of another axis, or axes: \
                  `countAxes`. Histograms and weighted scatter plots count with it.",
                label: Some("Count"),
                quantities: &["count", "quantity", "frequency", "counting"],
                methods: vec![
                    ("__init__", f!(m, counting_init)),
                    ("postProcessData", f!(m, counting_post)),
                ],
                properties: vec![],
            },
        )?;
        Ok::<_, PyErr>(m.clone().unbind())
    })?;
    Ok(held.bind(py).clone())
}
