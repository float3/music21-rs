//! music21's `graph.plot`: graphs of a stream, each a graph primitive and
//! `PlotStreamMixin` at once.
//!
//! A plot reads its stream through its axes, one value per axis for each
//! note (one per pitch for a chord), tidies the values as its axes and
//! its kind say, and hands them to the primitive it is to draw. The plots
//! of other analyses read them as music21's do -- `ActivityMatch` for
//! dynamics, `WindowedAnalysis` for keys and ambitus, the feature
//! extractors -- except `Dolan`, whose part reduction is the crate's
//! `PartReduction`.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::IntoPyObjectExt;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyList, PyModule, PyString, PyTuple};

use music21::analysis::reduction::{PartGroup, PartReduction};

use super::utilities::{GraphException, PlotStreamException};
use super::{
    ClassSpec, Property, built, given, keywords_of, kwargs, music21_name, optional, proto,
};

/// The module music21 keeps these in.
pub(crate) const MODULE: &str = "music21.graph.plot";

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

/// The axis attributes a plot may have, in order.
const AXIS_ATTRIBUTES: [&str; 3] = ["axisX", "axisY", "axisZ"];

/// The classes a plot reads from its stream by default.
const DEFAULT_CLASSES: [&str; 2] = ["Note", "Chord"];

/// The most rows of window sizes a windowed plot labels.
const MOST_WINDOW_TICKS: usize = 12;

/// A class of `graph.axis`.
fn axis_class<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    built(py, super::axis::MODULE, name)
}

/// A class of `graph.primitives`.
fn primitive<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    built(py, super::primitives::MODULE, name)
}

/// Calls a primitive's `__init__` on a plot, as music21 names the class.
fn primitive_init(
    slf: &Bound<'_, PyAny>,
    class: &str,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    primitive(slf.py(), class)?
        .getattr("__init__")?
        .call((slf,), keywords)?;
    Ok(())
}

/// Sets an attribute unless the keyword of that name was given.
fn default_unless<'py>(
    slf: &Bound<'py, PyAny>,
    keywords: Option<&Bound<'py, PyDict>>,
    name: &str,
    value: impl IntoPyObject<'py>,
) -> PyResult<()> {
    if given(keywords, name)? {
        return Ok(());
    }
    slf.setattr(name, value)
}

/// Sets `title`, and `figureSize` and `alpha` where given, unless their
/// keywords were.
fn defaults<'py>(
    slf: &Bound<'py, PyAny>,
    keywords: Option<&Bound<'py, PyDict>>,
    title: &str,
    figure_size: Option<(f64, f64)>,
    alpha: Option<f64>,
) -> PyResult<()> {
    if let Some(size) = figure_size {
        default_unless(slf, keywords, "figureSize", size)?;
    }
    default_unless(slf, keywords, "title", title)?;
    if let Some(alpha) = alpha {
        default_unless(slf, keywords, "alpha", alpha)?;
    }
    Ok(())
}

// ---------------------------------------------------------------- PlotStreamMixin

#[pyfunction]
#[pyo3(signature = (slf, streamObj=None, recurse=None, **keywords))]
fn mixin_init(
    slf: &Bound<'_, PyAny>,
    streamObj: Option<&Bound<'_, PyAny>>,
    recurse: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    let py = slf.py();
    if let Some(stream) = streamObj.filter(|stream| !stream.is_none())
        && !stream.hasattr("elements")?
    {
        return Err(PlotStreamException::new_err(format!(
            "non-stream provided as argument: {}",
            stream.str()?
        )));
    }
    slf.setattr("streamObj", streamObj)?;
    match recurse {
        Some(recurse) => slf.setattr("recurse", recurse)?,
        None => slf.setattr("recurse", true)?,
    }
    slf.setattr("classFilterList", PyList::new(py, DEFAULT_CLASSES)?)?;
    slf.setattr("matchPitchCountForChords", true)?;
    slf.setattr("data", py.None())?;

    // An axis of each class the class names.
    for item in slf
        .getattr("axesClasses")?
        .call_method0("items")?
        .try_iter()?
    {
        let (name, class): (String, Bound<'_, PyAny>) = item?.extract()?;
        if class.is_none() {
            continue;
        }
        let made = class.call1((slf, &name))?;
        slf.setattr(format!("axis{}", name.to_uppercase()).as_str(), made)?;
    }
    slf.setattr("savedKeywords", keywords_of(py, keywords)?)
}

/// `for` and the stream's repr, inside the plot's repr.
#[pyfunction]
fn mixin_repr(slf: &Bound<'_, PyAny>) -> PyResult<String> {
    match optional(slf, "streamObj")? {
        Some(stream) => Ok(format!("for {}", stream.repr()?)),
        None => Ok("for (no stream)".to_string()),
    }
}

/// The plot's `axisX`, `axisY` and `axisZ`, those it has.
#[pyfunction]
fn allAxes<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
    let all = PyList::empty(slf.py());
    for name in AXIS_ATTRIBUTES {
        if slf.hasattr(name)? {
            all.append(slf.getattr(name)?)?;
        }
    }
    Ok(all)
}

/// Extracts the data, sets the ticks and labels of the y and x axes, and
/// draws the graph unless `callProcess` is false.
#[pyfunction]
#[pyo3(signature = (slf, *, callProcess=true, **keywords))]
fn run(
    slf: &Bound<'_, PyAny>,
    callProcess: bool,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    let _ = keywords;
    slf.call_method0("setAxisKeywords")?;
    slf.call_method0("extractData")?;
    for (attribute, letter) in [("axisY", "y"), ("axisX", "x")] {
        let Some(axis) = optional(slf, attribute)? else {
            continue;
        };
        if !axis.is_truthy()? {
            continue;
        }
        slf.call_method1("setTicks", (letter, axis.call_method0("ticks")?))?;
        slf.call_method1("setAxisLabel", (letter, axis.getattr("label")?))?;
    }
    if callProcess {
        slf.call_method0("process")?;
    }
    Ok(())
}

/// Sets each axis attribute a saved keyword names by the axis's letter and
/// the attribute (`xHideUnused` sets `axisX.hideUnused`), where the axis
/// has that attribute already.
#[pyfunction]
fn setAxisKeywords(slf: &Bound<'_, PyAny>) -> PyResult<()> {
    let saved = slf.getattr("savedKeywords")?;
    for axis in allAxes(slf)?.iter() {
        if axis.is_none() {
            continue;
        }
        let letter: String = axis.getattr("axisName")?.extract()?;
        for keyword in saved.try_iter()? {
            let keyword: String = keyword?.extract()?;
            if !keyword.starts_with(&letter) || keyword.chars().count() < 3 {
                continue;
            }
            let mut rest = keyword.chars().skip(1);
            let first = rest
                .next()
                .map(|c| c.to_lowercase().to_string())
                .unwrap_or_default();
            let short = format!("{first}{}", rest.collect::<String>());
            if !axis.hasattr(short.as_str())? {
                continue;
            }
            axis.setattr(short.as_str(), saved.get_item(&keyword)?)?;
        }
    }
    Ok(())
}

/// Reads the data off each element of the stream (each of the classes in
/// `classFilterList`), tidies it, and sets each axis's bounds from it.
#[pyfunction]
fn extractData(slf: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    let axes = allAxes(slf)?;
    if axes.iter().any(|axis| axis.is_none()) {
        return Err(PlotStreamException::new_err(
            "Set all axes before calling extractData() via run()",
        ));
    }
    let stream = slf.getattr("streamObj")?;
    let mut elements = if slf.getattr("recurse")?.is_truthy()? {
        stream.call_method0("recurse")?
    } else {
        stream.call_method0("iter")?
    };
    let classes = slf.getattr("classFilterList")?;
    if classes.is_truthy()? {
        elements = elements.call_method1("getElementsByClass", (classes,))?;
    }

    let data = PyList::empty(py);
    slf.setattr("data", &data)?;
    for element in elements.try_iter()? {
        let found = slf.call_method1("processOneElement", (element?,))?;
        if !found.is_none() {
            slf.getattr("data")?.call_method1("extend", (found,))?;
        }
    }
    slf.call_method0("postProcessData")?;

    let data = slf.getattr("data")?;
    for (index, axis) in axes.iter().enumerate() {
        let column = PyList::empty(py);
        for point in data.try_iter()? {
            column.append(point?.get_item(index)?)?;
        }
        axis.call_method1("setBoundariesFromData", (column,))?;
    }
    Ok(())
}

/// The data points of one element, one per value its axes read (one per
/// pitch, for a chord), each with a copy of its format.
#[pyfunction]
fn processOneElement<'py>(
    slf: &Bound<'py, PyAny>,
    el: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let axes = allAxes(slf)?;
    let format = PyDict::new(py);
    let chord = music21_name(py, "music21.chord", "Chord")?;
    let values = if el.is_instance(&chord)? {
        slf.call_method1("extractChordDataMultiAxis", (el, &format))?
            .cast_into::<PyList>()?
    } else {
        let values = PyList::empty(py);
        for axis in axes.iter() {
            let value = axis.call_method1("extractOneElement", (el, &format))?;
            if !value.is_instance_of::<PyList>() && !value.is_none() {
                values.append(PyList::new(py, [value])?)?;
            } else {
                values.append(value)?;
            }
        }
        values
    };

    let mut arguments = vec![el.clone(), format.clone().into_any()];
    arguments.extend(values.iter());
    slf.call_method1("postProcessElement", PyTuple::new(py, arguments)?)?;
    if values.iter().any(|value| value.is_none()) {
        return Ok(py.None().into_bound(py));
    }

    let mut longest = 0;
    for value in values.iter() {
        longest = longest.max(value.len()?);
    }
    let formats = PyList::empty(py);
    for _ in 0..longest {
        formats.append(format.copy()?)?;
    }
    values.append(formats)?;
    let zipped = py
        .import("builtins")?
        .getattr("zip")?
        .call1(values.to_tuple())?;
    Ok(PyList::new(py, zipped.try_iter()?.collect::<PyResult<Vec<_>>>()?)?.into_any())
}

/// Anything each element needs, whatever the axes read; nothing here.
#[pyfunction]
#[pyo3(signature = (slf, el, formatDict, *values))]
fn postProcessElement(
    slf: &Bound<'_, PyAny>,
    el: &Bound<'_, PyAny>,
    formatDict: &Bound<'_, PyAny>,
    values: &Bound<'_, PyTuple>,
) {
    let _ = (slf, el, formatDict, values);
}

/// Each axis's tidying of the data.
#[pyfunction]
fn postProcessData(slf: &Bound<'_, PyAny>) -> PyResult<()> {
    for axis in allAxes(slf)?.iter() {
        axis.call_method0("postProcessData")?;
    }
    Ok(())
}

/// What an axis reads off a chord: off the chord itself, else off each
/// of its notes.
#[pyfunction]
fn extractChordDataOneAxis<'py>(
    ax: &Bound<'py, PyAny>,
    c: &Bound<'py, PyAny>,
    formatDict: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    let py = ax.py();
    let read = |element: &Bound<'py, PyAny>| -> PyResult<Option<Bound<'py, PyAny>>> {
        match ax.call_method1("extractOneElement", (element, formatDict)) {
            Ok(value) if value.is_none() => Ok(None),
            Ok(value) => Ok(Some(value)),
            Err(error) if error.is_instance_of::<pyo3::exceptions::PyAttributeError>(py) => {
                Err(error)
            }
            Err(error) => Err(error),
        }
    };
    let values = PyList::empty(py);
    let own = match read(c) {
        Ok(value) => value,
        Err(error) if error.is_instance_of::<pyo3::exceptions::PyAttributeError>(py) => None,
        Err(error) => return Err(error),
    };
    if let Some(value) = own {
        values.append(value)?;
        return Ok(values);
    }
    for note in c.try_iter()? {
        match read(&note?) {
            Ok(Some(value)) => values.append(value)?,
            Ok(None) => {}
            Err(error) if error.is_instance_of::<pyo3::exceptions::PyAttributeError>(py) => break,
            Err(error) => return Err(error),
        }
    }
    Ok(values)
}

/// What each axis reads off a chord, the shorter lists filled out to the
/// longest where `matchPitchCountForChords` is set.
#[pyfunction]
fn extractChordDataMultiAxis<'py>(
    slf: &Bound<'py, PyAny>,
    c: &Bound<'py, PyAny>,
    formatDict: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    let py = slf.py();
    let axes = allAxes(slf)?;
    let values = PyList::empty(py);
    for axis in axes.iter() {
        values.append(slf.call_method1("extractChordDataOneAxis", (&axis, c, formatDict))?)?;
    }

    // An axis that read nothing tries each note.
    for (axis, found) in axes.iter().zip(values.iter()) {
        if found.is_truthy()? {
            continue;
        }
        for note in c.try_iter()? {
            match axis.call_method1("extractOneElement", (note?, formatDict)) {
                Ok(value) if value.is_none() => {}
                Ok(value) => {
                    found.call_method1("append", (value,))?;
                }
                Err(error) if error.is_instance_of::<pyo3::exceptions::PyAttributeError>(py) => {}
                Err(error) => return Err(error),
            }
        }
    }
    if slf.getattr("matchPitchCountForChords")?.is_truthy()? {
        slf.call_method1("fillValueLists", (&values,))?;
    }
    Ok(values)
}

/// Pads each list to the longest, with its first value, or `nullFillValue`
/// where it has none; in place.
#[pyfunction]
#[pyo3(signature = (elementValues, nullFillValue=None))]
fn fillValueLists(
    elementValues: &Bound<'_, PyAny>,
    nullFillValue: Option<&Bound<'_, PyAny>>,
) -> PyResult<()> {
    let py = elementValues.py();
    let null = match nullFillValue {
        Some(value) => value.clone(),
        None => 0.into_bound_py_any(py)?,
    };
    let mut longest = 0;
    for values in elementValues.try_iter()? {
        longest = longest.max(values?.len()?);
    }
    for values in elementValues.try_iter()? {
        let values = values?;
        let short = longest - values.len()?;
        let fill = if values.is_truthy()? {
            values.get_item(0)?
        } else {
            null.clone()
        };
        for _ in 0..short {
            values.call_method1("append", (&fill,))?;
        }
    }
    Ok(())
}

/// The plot's graph type and the first quantity of each of its axes.
#[pyfunction]
fn plot_id(slf: &Bound<'_, PyAny>) -> PyResult<String> {
    let mut id: String = slf.getattr("graphType")?.extract()?;
    for axis in allAxes(slf)?.iter() {
        if axis.is_none() {
            continue;
        }
        let quantity: String = axis.getattr("quantities")?.get_item(0)?.extract()?;
        id.push('-');
        id.push_str(&quantity);
    }
    Ok(id)
}

// ---------------------------------------------------------------- PlotStream and the scatters

#[pyfunction]
#[pyo3(signature = (slf, streamObj=None, **keywords))]
fn plot_stream_init(
    slf: &Bound<'_, PyAny>,
    streamObj: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    primitive_init(slf, "Graph", keywords)?;
    mixin_init(slf, streamObj, None, keywords)?;
    slf.setattr(
        "axisX",
        axis_class(slf.py(), "OffsetAxis")?.call1((slf, "x"))?,
    )
}

#[pyfunction]
#[pyo3(signature = (slf, streamObj=None, **keywords))]
fn scatter_init(
    slf: &Bound<'_, PyAny>,
    streamObj: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    primitive_init(slf, "GraphScatter", keywords)?;
    mixin_init(slf, streamObj, None, keywords)
}

#[pyfunction]
#[pyo3(signature = (slf, streamObj=None, **keywords))]
fn scatter_ps_ql_init(
    slf: &Bound<'_, PyAny>,
    streamObj: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    scatter_init(slf, streamObj, keywords)?;
    slf.getattr("axisX")?.setattr("useLogScale", true)?;
    defaults(
        slf,
        keywords,
        "Pitch by Quarter Length Scatter",
        Some((6.0, 6.0)),
        None,
    )
}

#[pyfunction]
#[pyo3(signature = (slf, streamObj=None, **keywords))]
fn scatter_pc_ql_init(
    slf: &Bound<'_, PyAny>,
    streamObj: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    scatter_ps_ql_init(slf, streamObj, keywords)?;
    defaults(
        slf,
        keywords,
        "Pitch Class by Quarter Length Scatter",
        None,
        None,
    )
}

#[pyfunction]
#[pyo3(signature = (slf, streamObj=None, **keywords))]
fn scatter_pc_offset_init(
    slf: &Bound<'_, PyAny>,
    streamObj: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    scatter_init(slf, streamObj, keywords)?;
    defaults(
        slf,
        keywords,
        "Pitch Class by Offset Scatter",
        Some((10.0, 5.0)),
        Some(0.7),
    )
}

#[pyfunction]
#[pyo3(signature = (slf, streamObj=None, **keywords))]
fn scatter_dynamics_init(
    slf: &Bound<'_, PyAny>,
    streamObj: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    scatter_init(slf, streamObj, keywords)?;
    slf.getattr("axisX")?.setattr("showEnharmonic", false)?;
    default_unless(
        slf,
        keywords,
        "figureSize",
        slf.getattr("figureSizeDefault")?,
    )?;
    defaults(slf, keywords, "Dynamics by Pitch Scatter", None, Some(0.7))
}

/// The pitch and dynamic of each note, from `ActivityMatch`.
fn pitch_dynamics<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let matched = music21_name(py, "music21.analysis.correlate", "ActivityMatch")?
        .call1((slf.getattr("streamObj")?,))?;
    matched.call_method(
        "pitchToDynamic",
        (),
        Some(&kwargs!(py; "dataPoints" => true)),
    )
}

/// Each pitch against the dynamic it sounds at.
#[pyfunction]
#[pyo3(name = "extractData")]
fn scatter_dynamics_extract(slf: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    let data = PyList::empty(py);
    let xs = PyList::empty(py);
    let ys = PyList::empty(py);
    for point in pitch_dynamics(slf)?.try_iter()? {
        let (x, y) = super::pair(&point?)?;
        xs.append(&x)?;
        ys.append(&y)?;
        data.append((x, y, PyDict::new(py)))?;
    }
    slf.setattr("data", data)?;
    slf.getattr("axisX")?
        .call_method1("setBoundariesFromData", (xs,))?;
    slf.getattr("axisY")?
        .call_method1("setBoundariesFromData", (ys,))?;
    slf.call_method0("postProcessData")?;
    Ok(())
}

// ---------------------------------------------------------------- histograms

#[pyfunction]
#[pyo3(signature = (slf, streamObj=None, **keywords))]
fn histogram_init(
    slf: &Bound<'_, PyAny>,
    streamObj: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    primitive_init(slf, "GraphHistogram", keywords)?;
    mixin_init(slf, streamObj, None, keywords)?;
    default_unless(slf, keywords, "alpha", 1.0)
}

/// As a plot's `run`, with the x data and ticks numbered 1, 2, 3 ...
#[pyfunction]
#[pyo3(name = "run", signature = (slf, *, callProcess=true, **keywords))]
fn histogram_run(
    slf: &Bound<'_, PyAny>,
    callProcess: bool,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    let _ = keywords;
    slf.call_method0("setAxisKeywords")?;
    slf.call_method0("extractData")?;
    let axis_y = slf.getattr("axisY")?;
    let axis_x = slf.getattr("axisX")?;
    slf.call_method1("setTicks", ("y", axis_y.call_method0("ticks")?))?;
    let ticks = slf.call_method0("remapXTicksData")?;
    slf.call_method1("setTicks", ("x", ticks))?;
    slf.call_method1("setAxisLabel", ("y", axis_y.getattr("label")?))?;
    slf.call_method1("setAxisLabel", ("x", axis_x.getattr("label")?))?;
    if callProcess {
        slf.call_method0("process")?;
    }
    Ok(())
}

/// Renumbers the data and the x ticks 1, 2, 3 ... where unused values are
/// hidden; else a tick at every whole number of the range.
#[pyfunction]
fn remapXTicksData<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
    let py = slf.py();
    let axis = slf.getattr("axisX")?;
    let labels = PyDict::new(py);
    for tick in axis.call_method0("ticks")?.try_iter()? {
        let tick = tick?;
        labels.set_item(tick.get_item(0)?, tick.get_item(1)?)?;
    }
    let ticks = PyList::empty(py);
    let hides = match optional(&axis, "hideUnused")? {
        Some(hide) => hide.is(pyo3::types::PyBool::new(py, true)),
        None => !axis.hasattr("hideUnused")?,
    };
    let unbounded =
        optional(&axis, "minValue")?.is_none() || optional(&axis, "maxValue")?.is_none();
    if hides || unbounded {
        let data = slf.getattr("data")?;
        for index in 0..data.len()? {
            let point = data.get_item(index)?;
            let x = point.get_item(0)?;
            let rest = point.get_item(pyo3::types::PySlice::new(py, 1, isize::MAX, 1))?;
            let renumbered = PyTuple::new(py, [(index + 1).into_bound_py_any(py)?])?.add(rest)?;
            data.set_item(index, renumbered)?;
            if let Some(label) = labels.get_item(&x)? {
                ticks.append((index + 1, label))?;
            }
        }
        return Ok(ticks);
    }
    let int = py.import("builtins")?.getattr("int")?;
    let min: i64 = int.call1((axis.getattr("minValue")?,))?.extract()?;
    let max: i64 = int.call1((axis.getattr("maxValue")?,))?.extract()?;
    let blank = match optional(&axis, "blankLabelUnused")? {
        Some(blank) => blank.is_truthy()?,
        None => true,
    };
    for value in min..=max {
        let label = match labels.get_item(value)? {
            Some(label) => label,
            None if axis.hasattr("blankLabelUnused")? && !blank => {
                music21_name(py, "music21.pitch", "Pitch")?
                    .call1((value,))?
                    .getattr("name")?
            }
            None => PyString::new(py, "").into_any(),
        };
        ticks.append((value, label))?;
    }
    Ok(ticks)
}

#[pyfunction]
#[pyo3(signature = (slf, streamObj=None, **keywords))]
fn histogram_ps_init(
    slf: &Bound<'_, PyAny>,
    streamObj: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    histogram_init(slf, streamObj, keywords)?;
    slf.getattr("axisX")?.setattr("showEnharmonic", false)?;
    defaults(slf, keywords, "Pitch Histogram", Some((10.0, 6.0)), None)
}

#[pyfunction]
#[pyo3(signature = (slf, streamObj=None, **keywords))]
fn histogram_pc_init(
    slf: &Bound<'_, PyAny>,
    streamObj: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    histogram_init(slf, streamObj, keywords)?;
    slf.getattr("axisX")?.setattr("showEnharmonic", false)?;
    defaults(slf, keywords, "Pitch Class Histogram", None, None)
}

#[pyfunction]
#[pyo3(signature = (slf, streamObj=None, **keywords))]
fn histogram_ql_init(
    slf: &Bound<'_, PyAny>,
    streamObj: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    histogram_init(slf, streamObj, keywords)?;
    let axis = axis_class(slf.py(), "QuarterLengthAxis")?.call1((slf, "x"))?;
    slf.setattr("axisX", &axis)?;
    axis.setattr("useLogScale", false)?;
    defaults(slf, keywords, "Quarter Length Histogram", None, None)
}

// ---------------------------------------------------------------- weighted scatters

#[pyfunction]
#[pyo3(signature = (slf, streamObj=None, **keywords))]
fn weighted_init(
    slf: &Bound<'_, PyAny>,
    streamObj: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    primitive_init(slf, "GraphScatterWeighted", keywords)?;
    mixin_init(slf, streamObj, None, keywords)?;
    slf.getattr("axisZ")?.setattr("countAxes", ("x", "y"))
}

#[pyfunction]
#[pyo3(signature = (slf, streamObj=None, **keywords))]
fn weighted_ps_ql_init(
    slf: &Bound<'_, PyAny>,
    streamObj: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    weighted_init(slf, streamObj, keywords)?;
    defaults(
        slf,
        keywords,
        "Count of Pitch and Quarter Length",
        Some((7.0, 7.0)),
        Some(0.8),
    )
}

#[pyfunction]
#[pyo3(signature = (slf, streamObj=None, **keywords))]
fn weighted_pc_ql_init(
    slf: &Bound<'_, PyAny>,
    streamObj: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    weighted_init(slf, streamObj, keywords)?;
    defaults(
        slf,
        keywords,
        "Count of Pitch Class and Quarter Length",
        Some((7.0, 7.0)),
        Some(0.8),
    )
}

#[pyfunction]
#[pyo3(signature = (slf, streamObj=None, **keywords))]
fn weighted_dynamics_init(
    slf: &Bound<'_, PyAny>,
    streamObj: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    weighted_init(slf, streamObj, keywords)?;
    slf.getattr("axisX")?.setattr("showEnharmonic", false)?;
    defaults(
        slf,
        keywords,
        "Count of Pitch Class and Quarter Length",
        Some((10.0, 10.0)),
        Some(0.8),
    )?;
    default_unless(slf, keywords, "tickFontSize", 7)
}

/// Each pitch against the dynamic it sounds at, counted.
#[pyfunction]
#[pyo3(name = "extractData")]
fn weighted_dynamics_extract(slf: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    let points = pitch_dynamics(slf)?;
    let xs = PyList::empty(py);
    let ys = PyList::empty(py);
    let data = PyList::empty(py);
    for point in points.try_iter()? {
        let (x, y) = super::pair(&point?)?;
        xs.append(&x)?;
        ys.append(&y)?;
        data.append(PyList::new(py, [x, y, 1.into_bound_py_any(py)?])?)?;
    }
    slf.setattr("data", data)?;
    slf.getattr("axisX")?
        .call_method1("setBoundariesFromData", (xs,))?;
    slf.getattr("axisY")?
        .call_method1("setBoundariesFromData", (ys,))?;
    slf.call_method0("postProcessData")?;
    Ok(())
}

// ---------------------------------------------------------------- windowed analyses

#[pyfunction]
#[pyo3(signature = (slf, streamObj=None, **keywords))]
fn windowed_init(
    slf: &Bound<'_, PyAny>,
    streamObj: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    let py = slf.py();
    slf.setattr("processorClass", slf.getattr("processorClassDefault")?)?;
    slf.setattr("_processor", py.None())?;
    slf.setattr("graphLegend", py.None())?;
    slf.setattr("minWindow", 1)?;
    slf.setattr("maxWindow", py.None())?;
    slf.setattr("windowStep", "pow2")?;
    slf.setattr("windowType", "overlap")?;
    slf.setattr("compressLegend", true)?;
    primitive_init(slf, "GraphColorGrid", keywords)?;
    mixin_init(slf, streamObj, None, keywords)?;
    slf.setattr("axisX", axis_class(py, "OffsetAxis")?.call1((slf, "x"))?)
}

/// The analysis run in each window: an instance of `processorClass` over
/// the stream, made once.
#[pyfunction]
fn processor<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let class = slf.getattr("processorClass")?;
    if !class.is_truthy()? {
        return Ok(py.None().into_bound(py));
    }
    let held = slf.getattr("_processor")?;
    if held.is_truthy()? {
        return Ok(held);
    }
    let made = class.call1((slf.getattr("streamObj")?,))?;
    slf.setattr("_processor", &made)?;
    Ok(made)
}

/// Runs the windowed analysis, sets ticks, labels and the legend, and
/// draws the grid unless `callProcess` is false.
#[pyfunction]
#[pyo3(name = "run", signature = (slf, *, callProcess=true, **keywords))]
fn windowed_run(
    slf: &Bound<'_, PyAny>,
    callProcess: bool,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    let py = slf.py();
    let _ = keywords;
    let found = slf.getattr("processor")?;
    if slf.getattr("title")?.eq("Music21 Graph")? && found.is_truthy()? {
        let unit = found.call_method0("solutionUnitString")?.str()?;
        let title = format!("{} ({unit})", found.getattr("name")?.str()?);
        slf.setattr("title", title)?;
    }
    let (data, y_ticks) = super::pair(&slf.call_method0("extractData")?)?;
    slf.setattr("data", data)?;
    slf.call_method1("setTicks", ("y", y_ticks))?;

    // Only the first and last ticks, at the grid's ends.
    let axis = slf.getattr("axisX")?;
    axis.call_method0("setBoundariesFromData")?;
    let mut ticks = axis.call_method0("ticks")?;
    if ticks.len()? >= 2 {
        let first = ticks.get_item(0)?.get_item(1)?;
        let last = ticks.get_item(-1)?.get_item(1)?;
        ticks = PyList::new(py, [(0, first), (1, last)])?.into_any();
    }
    slf.call_method1("setTicks", ("x", ticks))?;
    slf.call_method1("setAxisLabel", ("y", "Window Size\n(Quarter Lengths)"))?;
    let label = format!("Windows ({} Span)", axis.getattr("label")?.str()?);
    slf.call_method1("setAxisLabel", ("x", label))?;
    slf.setattr("graphLegend", slf.call_method0("_getLegend")?)?;
    if callProcess {
        slf.call_method0("process")?;
    }
    Ok(())
}

/// The windowed analysis's colours, and y ticks naming each row's window
/// size: at most about twelve labelled.
#[pyfunction]
#[pyo3(name = "extractData")]
fn windowed_extract<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    let py = slf.py();
    let analysis = music21_name(py, "music21.analysis.windowed", "WindowedAnalysis")?
        .call1((slf.getattr("streamObj")?, slf.getattr("processor")?))?;
    let keywords = kwargs!(py; "windowType" => slf.getattr("windowType")?);
    let processed = analysis.call_method(
        "process",
        (
            slf.getattr("minWindow")?,
            slf.getattr("maxWindow")?,
            slf.getattr("windowStep")?,
        ),
        Some(&keywords),
    )?;
    let colors = processed.get_item(1)?;
    let meta = processed.get_item(2)?;
    let rows = meta.len()?;
    let step = if rows > MOST_WINDOW_TICKS {
        rows / MOST_WINDOW_TICKS
    } else {
        1
    };
    let y_ticks = PyList::empty(py);
    for (position, row) in (0..rows).step_by(step).enumerate() {
        let size = meta.get_item(row)?.get_item("windowSize")?.str()?;
        let at = position * 3;
        y_ticks.append(PyList::new(
            py,
            [at.into_bound_py_any(py)?, PyString::new(py, "").into_any()],
        )?)?;
        y_ticks.append(PyList::new(
            py,
            [(at + 1).into_bound_py_any(py)?, size.into_any()],
        )?)?;
        y_ticks.append(PyList::new(
            py,
            [
                (at + 2).into_bound_py_any(py)?,
                PyString::new(py, "").into_any(),
            ],
        )?)?;
    }
    PyTuple::new(py, [colors, y_ticks.into_any()])
}

/// A legend of the analysis's solutions and their colours.
#[pyfunction]
fn _getLegend<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let keywords = kwargs!(py; "doneAction" => py.None(), "title" => slf.getattr("title")?);
    let legend = primitive(py, "GraphColorGridLegend")?.call((), Some(&keywords))?;
    let keywords = kwargs!(py; "compress" => slf.getattr("compressLegend")?);
    let data = slf
        .getattr("processor")?
        .call_method("solutionLegend", (), Some(&keywords))?;
    legend.setattr("data", data)?;
    Ok(legend)
}

/// Writes the grid, then its legend beside it as `legend-` and the name.
#[pyfunction]
#[pyo3(name = "write", signature = (slf, fp=None))]
fn windowed_write(slf: &Bound<'_, PyAny>, fp: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
    let py = slf.py();
    primitive(py, "GraphColorGrid")?
        .getattr("write")?
        .call1((slf, fp))?;
    let path = match fp.filter(|fp| !fp.is_none()) {
        Some(fp) => music21_name(py, "music21.common", "cleanpath")?
            .call((fp,), Some(&kwargs!(py; "returnPathlib" => true)))?,
        None => music21_name(py, "music21.environment", "Environment")?
            .call1(("graph.plot",))?
            .call_method(
                "getTempFile",
                (".png",),
                Some(&kwargs!(py; "returnPathlib" => true)),
            )?,
    };
    let os_path = py.import("os.path")?;
    let (directory, name): (Bound<'_, PyAny>, String) =
        os_path.call_method1("split", (path,))?.extract()?;
    let legend_path = os_path.call_method1("join", (directory, format!("legend-{name}")))?;
    let legend = slf.getattr("graphLegend")?;
    legend.call_method0("process")?;
    legend.call_method1("write", (legend_path,))?;
    Ok(())
}

// ---------------------------------------------------------------- horizontal bars

#[pyfunction]
#[pyo3(signature = (slf, streamObj=None, *, colorByPart=false, **keywords))]
fn horizontal_init(
    slf: &Bound<'_, PyAny>,
    streamObj: Option<&Bound<'_, PyAny>>,
    colorByPart: bool,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    let py = slf.py();
    slf.setattr("colorByPart", colorByPart)?;
    slf.setattr("_partsToColor", PyDict::new(py))?;
    primitive_init(slf, "GraphHorizontalBar", keywords)?;
    mixin_init(slf, streamObj, None, keywords)?;
    slf.getattr("axisY")?.setattr("hideUnused", false)
}

/// Gives each part its colour first, where `colorByPart` is set.
#[pyfunction]
#[pyo3(name = "run", signature = (slf, *, callProcess=true, **keywords))]
fn horizontal_run(
    slf: &Bound<'_, PyAny>,
    callProcess: bool,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    if slf.getattr("colorByPart")?.is_truthy()? {
        slf.call_method0("assignColorsToParts")?;
    }
    run(slf, callProcess, keywords)
}

/// A colour of `colors` for each part, round again after the last;
/// answers the assignment.
#[pyfunction]
fn assignColorsToParts<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let colors = slf.getattr("colors")?;
    let count = colors.len()?;
    let assigned = slf.getattr("_partsToColor")?;
    let parts = slf
        .getattr("streamObj")?
        .get_item(music21_name(py, "music21.stream", "Part")?)?;
    for (index, part) in parts.try_iter()?.enumerate() {
        assigned.set_item(part?, colors.get_item(index % count)?)?;
    }
    Ok(assigned)
}

/// The colour of the element's part, where `colorByPart` is set.
#[pyfunction]
#[pyo3(name = "postProcessElement", signature = (slf, el, formatDict, *values))]
fn horizontal_element(
    slf: &Bound<'_, PyAny>,
    el: &Bound<'_, PyAny>,
    formatDict: &Bound<'_, PyAny>,
    values: &Bound<'_, PyTuple>,
) -> PyResult<()> {
    let py = slf.py();
    let _ = values;
    if !slf.getattr("colorByPart")?.is_truthy()? {
        return Ok(());
    }
    let part = el.call_method1(
        "getContextByClass",
        (music21_name(py, "music21.stream", "Part")?,),
    )?;
    if part.is_none() {
        return Ok(());
    }
    let first = slf.getattr("colors")?.get_item(0)?;
    let color = slf
        .getattr("_partsToColor")?
        .call_method1("get", (part, first))?;
    formatDict.set_item("color", color)
}

/// Gathers the bars of each pitch into a row, with a row for every tick
/// of the pitch axis, sorted by start.
#[pyfunction]
#[pyo3(name = "postProcessData")]
fn horizontal_post(slf: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    postProcessData(slf)?;
    let data = slf.getattr("data")?;
    let pitches = PyList::empty(py);
    for point in data.try_iter()? {
        pitches.append(point?.get_item(1)?)?;
    }
    let axis = slf.getattr("axisY")?;
    axis.call_method1("setBoundariesFromData", (pitches,))?;
    let y_ticks = axis.call_method0("ticks")?;

    let spans = PyDict::new(py);
    let formats = PyDict::new(py);
    for point in data.try_iter()? {
        let [position, pitch, format] = super::unpacked::<3>(&point?)?;
        if !spans.contains(&pitch)? {
            spans.set_item(&pitch, PyList::empty(py))?;
            formats.set_item(&pitch, PyDict::new(py))?;
        }
        let mut items: Vec<Bound<'_, PyAny>> = position.try_iter()?.collect::<PyResult<_>>()?;
        items.push(format);
        spans
            .get_item(&pitch)?
            .unwrap()
            .call_method1("append", (PyTuple::new(py, items)?,))?;
    }
    // Each row by start, then length; the formats are not compared.
    let by_start = pyo3::types::PyCFunction::new_closure(
        py,
        None,
        None,
        |args: &Bound<'_, PyTuple>, _: Option<&Bound<'_, PyDict>>| -> PyResult<Py<PyAny>> {
            let point = args.get_item(0)?;
            let key = PyTuple::new(args.py(), [point.get_item(0)?, point.get_item(1)?])?;
            Ok(key.into_any().unbind())
        },
    )?;
    for row in spans.values().iter() {
        row.call_method("sort", (), Some(&kwargs!(py; "key" => &by_start)))?;
    }

    let rows = PyList::empty(py);
    for tick in y_ticks.try_iter()? {
        let (value, label) = super::pair(&tick?)?;
        match spans.get_item(&value)? {
            Some(row) => rows.append(PyList::new(
                py,
                [label, row, formats.get_item(&value)?.unwrap()],
            )?)?,
            None => rows.append(PyList::new(
                py,
                [
                    label,
                    PyList::empty(py).into_any(),
                    PyDict::new(py).into_any(),
                ],
            )?)?,
        }
    }
    slf.setattr("data", rows)
}

#[pyfunction]
#[pyo3(signature = (slf, streamObj=None, *, colorByPart=false, **keywords))]
fn horizontal_pc_init(
    slf: &Bound<'_, PyAny>,
    streamObj: Option<&Bound<'_, PyAny>>,
    colorByPart: bool,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    horizontal_init(slf, streamObj, colorByPart, keywords)?;
    let axis = axis_class(slf.py(), "PitchClassAxis")?.call1((slf, "y"))?;
    slf.setattr("axisY", &axis)?;
    axis.setattr("hideUnused", false)?;
    defaults(
        slf,
        keywords,
        "Note Quarter Length and Offset by Pitch Class",
        Some((10.0, 4.0)),
        None,
    )
}

#[pyfunction]
#[pyo3(signature = (slf, streamObj=None, *, colorByPart=false, **keywords))]
fn horizontal_ps_init(
    slf: &Bound<'_, PyAny>,
    streamObj: Option<&Bound<'_, PyAny>>,
    colorByPart: bool,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    horizontal_init(slf, streamObj, colorByPart, keywords)?;
    defaults(
        slf,
        keywords,
        "Note Quarter Length by Pitch",
        Some((10.0, 6.0)),
        None,
    )
}

#[pyfunction]
#[pyo3(signature = (slf, streamObj=None, **keywords))]
fn weighted_bars_init(
    slf: &Bound<'_, PyAny>,
    streamObj: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    let py = slf.py();
    slf.setattr("fillByMeasure", false)?;
    slf.setattr("segmentByTarget", true)?;
    slf.setattr("normalizeByPart", false)?;
    slf.setattr("partGroups", py.None())?;
    primitive_init(slf, "GraphHorizontalBarWeighted", keywords)?;
    mixin_init(slf, streamObj, None, keywords)
}

/// A part group of music21's: a dict with a name, a colour and what a
/// part's id must hold.
fn part_group(group: &Bound<'_, PyAny>) -> PyResult<PartGroup> {
    let matches = match group.call_method1("get", ("match",))? {
        found if found.is_none() => None,
        found => Some(found.extract()?),
    };
    Ok(PartGroup {
        name: group.get_item("name")?.extract()?,
        color: group.get_item("color")?.extract()?,
        matches,
    })
}

/// How loudly each part, or group of parts, plays, and when.
#[pyfunction]
#[pyo3(name = "extractData")]
fn weighted_bars_extract(slf: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    let stream = slf.getattr("streamObj")?;
    if !stream.is_instance(&music21_name(py, "music21.stream", "Score")?)? {
        return Err(GraphException::new_err("provided Stream must be Score"));
    }
    let part_groups = match optional(slf, "partGroups")? {
        Some(groups) => Some(
            groups
                .try_iter()?
                .map(|group| part_group(&group?))
                .collect::<PyResult<Vec<_>>>()?,
        ),
        None => None,
    };
    let reduction = PartReduction {
        part_groups,
        fill_by_measure: slf.getattr("fillByMeasure")?.is_truthy()?,
        segment_by_target: slf.getattr("segmentByTarget")?.is_truthy()?,
        normalize_by_part: slf.getattr("normalizeByPart")?.is_truthy()?,
        ..PartReduction::default()
    };
    let score = crate::stream::crate_stream(&stream)?;
    let found = reduction
        .weighted_spans(&score)
        .map_err(|error| GraphException::new_err(crate::pitch::message(&error)))?;

    let data = PyList::empty(py);
    let mut offsets: Vec<f64> = Vec::new();
    for activity in found {
        let spans = PyList::empty(py);
        for span in activity.spans {
            for edge in [span.start, span.start + span.span] {
                if !offsets.contains(&edge) {
                    offsets.push(edge);
                }
            }
            let item = PyList::new(
                py,
                [
                    span.start.into_bound_py_any(py)?,
                    span.span.into_bound_py_any(py)?,
                    span.weight.into_bound_py_any(py)?,
                    PyString::new(py, &span.color).into_any(),
                ],
            )?;
            spans.append(item)?;
        }
        data.append((activity.id, spans))?;
    }
    let axis = slf.getattr("axisX")?;
    let builtins = py.import("builtins")?;
    axis.setattr(
        "minValue",
        builtins.call_method1("min", (offsets.clone(),))?,
    )?;
    axis.setattr("maxValue", builtins.call_method1("max", (offsets,))?)?;
    slf.setattr("data", data)
}

#[pyfunction]
#[pyo3(signature = (slf, streamObj=None, **keywords))]
fn dolan_init(
    slf: &Bound<'_, PyAny>,
    streamObj: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    weighted_bars_init(slf, streamObj, keywords)?;
    if streamObj.is_some_and(|stream| !stream.is_none()) {
        slf.call_method0("_getPartGroups")?;
    }
    default_unless(slf, keywords, "figureSize", (10, 4))?;
    if !given(keywords, "title")? {
        slf.setattr("title", "Instrumentation")?;
        if let Some(stream) = optional(slf, "streamObj")?
            && stream.is_truthy()?
            && let Some(metadata) = optional(&stream, "metadata")?
            && let Some(title) = optional(&metadata, "title")?
        {
            slf.setattr("title", title)?;
        }
    }
    default_unless(slf, keywords, "hideYGrid", true)
}

/// A part group of `Dolan`'s defaults.
struct DefaultGroup {
    name: &'static str,
    color: &'static str,
    matches: Option<&'static [&'static str]>,
}

const fn group(
    name: &'static str,
    color: &'static str,
    matches: Option<&'static [&'static str]>,
) -> DefaultGroup {
    DefaultGroup {
        name,
        color,
        matches,
    }
}

/// The groups of a choir: four parts, one of them `Soprano`.
const CHOIR: [DefaultGroup; 4] = [
    group("Soprano", "purple", Some(&["soprano", "0"])),
    group("Alto", "orange", Some(&["alto", "1"])),
    group("Tenor", "lightgreen", Some(&["tenor"])),
    group("Bass", "mediumblue", Some(&["bass"])),
];

/// The groups of a string quartet: four parts, one of them `Viola`.
const QUARTET: [DefaultGroup; 4] = [
    group(
        "1st Violin",
        "purple",
        Some(&["1st violin", "0", "violin 1", "violin i"]),
    ),
    group(
        "2nd Violin",
        "orange",
        Some(&["2nd violin", "1", "violin 2", "violin ii"]),
    ),
    group("Viola", "lightgreen", Some(&["viola"])),
    group(
        "Cello",
        "mediumblue",
        Some(&["cello", "violoncello", "'cello"]),
    ),
];

/// The groups of an orchestra: more than ten parts.
const ORCHESTRA: [DefaultGroup; 12] = [
    group("Flute", "#C154C1", Some(&["flauto", r"flute \d"])),
    group("Oboe", "blue", Some(&["oboe", r"oboe \d"])),
    group(
        "Clarinet",
        "mediumblue",
        Some(&["clarinetto", r"clarinet in \w* \d"]),
    ),
    group("Bassoon", "purple", Some(&["fagotto", r"bassoon \d"])),
    group("Horns", "orange", Some(&["corno", r"horn in \w* \d"])),
    group(
        "Trumpet",
        "red",
        Some(&["tromba", r"trumpet \d", r"trumpet in \w* \d"]),
    ),
    group("Trombone", "red", Some(&[r"trombone \d"])),
    group("Timpani", "#5C3317", None),
    group("Violin I", "lightgreen", Some(&["violino i", "violin i"])),
    group("Violin II", "green", Some(&["violino ii", "violin ii"])),
    group("Viola", "forestgreen", None),
    group(
        "Violoncello & CB",
        "dark green",
        Some(&["violoncello", "contrabasso"]),
    ),
];

/// Orchestras have more parts than this.
const ORCHESTRA_PARTS: usize = 10;

/// Part groups chosen from the score's instruments where none are set: a
/// choir's, a string quartet's or an orchestra's.
#[pyfunction]
fn _getPartGroups(slf: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    if slf.getattr("partGroups")?.is_truthy()? {
        return Ok(());
    }
    let Some(stream) = optional(slf, "streamObj")? else {
        return Ok(());
    };
    let instruments = stream.call_method0("flatten")?.call_method1(
        "getElementsByClass",
        (music21_name(py, "music21.instrument", "Instrument")?,),
    )?;
    if !instruments.is_truthy()? {
        return Ok(());
    }
    let count = instruments.len()?;
    let has = |id: &str| -> PyResult<bool> {
        Ok(!stream.call_method1("getElementById", (id,))?.is_none())
    };
    let chosen: &[DefaultGroup] = if count == 4 && has("Soprano")? {
        &CHOIR
    } else if count == 4 && has("Viola")? {
        &QUARTET
    } else if count > ORCHESTRA_PARTS {
        &ORCHESTRA
    } else {
        return Ok(());
    };
    let groups = PyList::empty(py);
    for made in chosen {
        let matches = match made.matches {
            Some(matches) => PyList::new(py, matches)?.into_any(),
            None => py.None().into_bound(py),
        };
        groups
            .append(kwargs!(py; "name" => made.name, "color" => made.color, "match" => matches))?;
    }
    slf.setattr("partGroups", groups)
}

// ---------------------------------------------------------------- 3D bars

#[pyfunction]
#[pyo3(signature = (slf, streamObj=None, **keywords))]
fn bars3d_init(
    slf: &Bound<'_, PyAny>,
    streamObj: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    primitive_init(slf, "Graph3DBars", keywords)?;
    mixin_init(slf, streamObj, None, keywords)?;
    slf.getattr("axisZ")?.setattr("countAxes", ("x", "y"))
}

#[pyfunction]
#[pyo3(signature = (slf, streamObj=None, **keywords))]
fn bars3d_ps_init(
    slf: &Bound<'_, PyAny>,
    streamObj: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    bars3d_init(slf, streamObj, keywords)?;
    defaults(
        slf,
        keywords,
        "Pitch by Quarter Length Count",
        Some((6.0, 6.0)),
        None,
    )
}

// ---------------------------------------------------------------- several streams

#[pyfunction]
#[pyo3(signature = (slf, streamList, labelList=None, **keywords))]
fn multi_init(
    slf: &Bound<'_, PyAny>,
    streamList: &Bound<'_, PyAny>,
    labelList: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    let py = slf.py();
    primitive_init(slf, "GraphGroupedVerticalBar", keywords)?;
    mixin_init(slf, None, None, None)?;
    let labels = match labelList.filter(|labels| !labels.is_none()) {
        Some(labels) => labels.clone(),
        None => PyList::empty(py).into_any(),
    };
    slf.setattr("streamList", py.None())?;
    let paths = slf.call_method1("parseStreams", (streamList,))?;
    if !labels.is_truthy()? && paths.len()? == streamList.len()? {
        slf.setattr("labelList", paths)?;
    } else {
        slf.setattr("labelList", labels)?;
    }
    slf.setattr("data", py.None())
}

/// Each stream given, parsed where it is a path or a corpus name; answers
/// the file names of those that were.
#[pyfunction]
fn parseStreams<'py>(
    slf: &Bound<'py, PyAny>,
    streamList: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    let py = slf.py();
    let streams = PyList::empty(py);
    let found = PyList::empty(py);
    let path_class = py.import("pathlib")?.getattr("Path")?;
    let os_path = py.import("os.path")?;
    for given in streamList.try_iter()? {
        let mut stream = given?;
        let exists = if stream.is_instance_of::<PyString>() {
            found.append(os_path.call_method1("basename", (&stream,))?)?;
            Some(os_path.call_method1("exists", (&stream,))?.is_truthy()?)
        } else if stream.is_instance(&path_class)? {
            found.append(stream.getattr("name")?)?;
            Some(stream.call_method0("exists")?.is_truthy()?)
        } else {
            None
        };
        if let Some(exists) = exists {
            let reader = if exists {
                music21_name(py, "music21.converter", "parse")?
            } else {
                music21_name(py, "music21.corpus", "parse")?
            };
            stream = reader.call1((stream,))?;
        }
        streams.append(stream)?;
    }
    slf.setattr("streamList", streams)?;
    Ok(found)
}

#[pyfunction]
#[pyo3(signature = (slf, streamList, featureExtractors, labelList=None, **keywords))]
fn features_init(
    slf: &Bound<'_, PyAny>,
    streamList: &Bound<'_, PyAny>,
    featureExtractors: &Bound<'_, PyAny>,
    labelList: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    let py = slf.py();
    let labels = match labelList.filter(|labels| !labels.is_none()) {
        Some(labels) => labels.clone(),
        None => PyList::empty(py).into_any(),
    };
    multi_init(slf, streamList, Some(&labels), keywords)?;
    slf.setattr("featureExtractors", featureExtractors)?;
    slf.setattr("xTickLabelRotation", 90)?;
    slf.setattr("xTickLabelHorizontalAlignment", "left")?;
    slf.setattr("xTickLabelVerticalAlignment", "top")?;
    default_unless(slf, keywords, "figureSize", (10, 6))?;
    default_unless(slf, keywords, "title", py.None())
}

/// Extracts the features, sets the ticks, and draws the bars unless
/// `callProcess` is false.
#[pyfunction]
#[pyo3(name = "run", signature = (slf, *, callProcess=true, **keywords))]
fn features_run(
    slf: &Bound<'_, PyAny>,
    callProcess: bool,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    let _ = keywords;
    slf.call_method0("setAxisKeywords")?;
    let [data, x_ticks, y_ticks] = super::unpacked::<3>(&slf.call_method0("extractData")?)?;
    slf.setattr("data", data)?;
    slf.setattr("grid", false)?;
    slf.call_method1("setTicks", ("x", x_ticks))?;
    slf.call_method1("setTicks", ("y", y_ticks))?;
    if callProcess {
        slf.call_method0("process")?;
    }
    Ok(())
}

/// Each stream's value of each feature (the mean of a feature of several
/// values), labelled, with an x tick at the middle of each group.
#[pyfunction]
#[pyo3(name = "extractData")]
fn features_extract<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    let py = slf.py();
    let streams = slf.getattr("streamList")?;
    let given_labels = slf.getattr("labelList")?;
    let labels: Vec<Bound<'py, PyAny>> = if given_labels.len()? != streams.len()? {
        (1..=streams.len()?)
            .map(|n| n.into_bound_py_any(py))
            .collect::<PyResult<_>>()?
    } else {
        given_labels.try_iter()?.collect::<PyResult<_>>()?
    };

    // An instance of each extractor named or given.
    let mut extractors = Vec::new();
    for extractor in slf.getattr("featureExtractors")?.try_iter()? {
        let extractor = extractor?;
        if extractor.is_instance_of::<PyString>() {
            let found =
                music21_name(py, "music21.features", "extractorsById")?.call1((extractor,))?;
            for class in found.try_iter()? {
                extractors.push(class?.call0()?);
            }
        } else {
            extractors.push(extractor.call0()?);
        }
    }

    let data_instance = music21_name(py, "music21.features", "DataInstance")?;
    let ordered = py.import("collections")?.getattr("OrderedDict")?;
    let data = PyList::empty(py);
    for (index, stream) in streams.try_iter()?.enumerate() {
        let instance = data_instance.call1((stream?,))?;
        let row = ordered.call0()?;
        for extractor in &extractors {
            extractor.setattr("data", &instance)?;
            let vector = extractor.call_method0("extract")?.getattr("vector")?;
            let value = if vector.len()? == 1 {
                vector.get_item(0)?
            } else {
                py.import("builtins")?
                    .call_method1("sum", (&vector,))?
                    .div(vector.len()?)?
            };
            row.set_item(extractor.getattr("name")?, value)?;
        }
        data.append(PyList::new(py, [labels[index].clone(), row])?)?;
    }
    let x_ticks = PyList::empty(py);
    for (x, label) in labels.iter().enumerate() {
        x_ticks.append(PyList::new(
            py,
            [
                (x as f64 + 0.5).into_bound_py_any(py)?,
                label.str()?.into_any(),
            ],
        )?)?;
    }
    PyTuple::new(
        py,
        [
            data.into_any(),
            x_ticks.into_any(),
            PyList::empty(py).into_any(),
        ],
    )
}

// ---------------------------------------------------------------- the module

/// A Rust function, to be a method.
macro_rules! f {
    ($m:expr, $function:path) => {
        wrap_pyfunction!($function, $m)?.into_any()
    };
}

/// `{'x': ..., 'y': ...}` of `graph.axis` classes by name.
fn axes<'py>(py: Python<'py>, named: &[(&str, &str)]) -> PyResult<Bound<'py, PyAny>> {
    let made = PyDict::new(py);
    for (letter, name) in named {
        made.set_item(letter, axis_class(py, name)?)?;
    }
    Ok(made.into_any())
}

/// A plot class, built in `m`.
fn plot_class<'py>(
    m: &Bound<'py, PyModule>,
    name: &'static str,
    doc: &'static str,
    bases: Vec<Bound<'py, PyAny>>,
    attributes: Vec<(&'static str, Bound<'py, PyAny>)>,
    methods: Vec<(&'static str, Bound<'py, PyAny>)>,
) -> PyResult<Bound<'py, PyAny>> {
    let mut spec = ClassSpec::new(name, doc, bases);
    spec.attributes = attributes;
    spec.methods = methods;
    let class = spec.build(MODULE)?;
    m.add(name, &class)?;
    Ok(class)
}

/// music21's `graph.plot`, built.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        let m = &m;

        let mut spec = ClassSpec::new(
            "PlotStreamMixin",
            "What a graph needs to plot a stream: the stream, an axis object for \
             each of `axesClasses`, and the reading of the stream through them.",
            vec![proto(py)],
        );
        spec.attributes = vec![("axesClasses", axes(py, &[("x", "Axis"), ("y", "Axis")])?)];
        spec.methods = vec![
            ("__init__", f!(m, mixin_init)),
            ("__getstate__", f!(m, super::state_of)),
            ("_reprInternal", f!(m, mixin_repr)),
            ("run", f!(m, run)),
            ("setAxisKeywords", f!(m, setAxisKeywords)),
            ("extractData", f!(m, extractData)),
            ("processOneElement", f!(m, processOneElement)),
            ("postProcessElement", f!(m, postProcessElement)),
            ("postProcessData", f!(m, postProcessData)),
            (
                "extractChordDataMultiAxis",
                f!(m, extractChordDataMultiAxis),
            ),
        ];
        spec.statics = vec![
            ("extractChordDataOneAxis", f!(m, extractChordDataOneAxis)),
            ("fillValueLists", f!(m, fillValueLists)),
        ];
        spec.properties = vec![
            Property {
                name: "allAxes",
                getter: f!(m, allAxes),
                setter: None,
            },
            Property {
                name: "id",
                getter: f!(m, plot_id),
                setter: None,
            },
        ];
        let mixin = spec.build(MODULE)?;
        m.add("PlotStreamMixin", &mixin)?;

        let with = |name: &str| -> PyResult<Vec<Bound<'_, PyAny>>> {
            Ok(vec![primitive(py, name)?, mixin.clone()])
        };
        let one = |name: &str| -> PyResult<Vec<Bound<'_, PyAny>>> { Ok(vec![m.getattr(name)?]) };

        plot_class(
            m,
            "PlotStream",
            "A plot of a stream, against offset.",
            with("Graph")?,
            vec![],
            vec![("__init__", f!(m, plot_stream_init))],
        )?;

        // Scatter plots.
        plot_class(
            m,
            "Scatter",
            "The base of scatter plots of a stream.",
            with("GraphScatter")?,
            vec![],
            vec![("__init__", f!(m, scatter_init))],
        )?;
        plot_class(
            m,
            "ScatterPitchSpaceQuarterLength",
            "A scatter plot of pitch space against quarter length.",
            one("Scatter")?,
            vec![(
                "axesClasses",
                axes(py, &[("x", "QuarterLengthAxis"), ("y", "PitchSpaceAxis")])?,
            )],
            vec![("__init__", f!(m, scatter_ps_ql_init))],
        )?;
        plot_class(
            m,
            "ScatterPitchClassQuarterLength",
            "A scatter plot of pitch class against quarter length.",
            one("ScatterPitchSpaceQuarterLength")?,
            vec![(
                "axesClasses",
                axes(py, &[("x", "QuarterLengthAxis"), ("y", "PitchClassAxis")])?,
            )],
            vec![("__init__", f!(m, scatter_pc_ql_init))],
        )?;
        plot_class(
            m,
            "ScatterPitchClassOffset",
            "A scatter plot of pitch class against offset.",
            one("Scatter")?,
            vec![(
                "axesClasses",
                axes(py, &[("x", "OffsetAxis"), ("y", "PitchClassAxis")])?,
            )],
            vec![("__init__", f!(m, scatter_pc_offset_init))],
        )?;
        plot_class(
            m,
            "ScatterPitchSpaceDynamicSymbol",
            "A scatter plot of the dynamics each pitch sounds at.",
            one("Scatter")?,
            vec![
                ("figureSizeDefault", (12, 6).into_bound_py_any(py)?),
                (
                    "axesClasses",
                    axes(py, &[("x", "PitchSpaceAxis"), ("y", "DynamicsAxis")])?,
                ),
            ],
            vec![
                ("__init__", f!(m, scatter_dynamics_init)),
                ("extractData", f!(m, scatter_dynamics_extract)),
            ],
        )?;

        // Histograms.
        let counted = |x: &str| axes(py, &[("x", x), ("y", "CountingAxis")]);
        plot_class(
            m,
            "Histogram",
            "The base of histograms: one axis against its count.",
            with("GraphHistogram")?,
            vec![("axesClasses", counted("Axis")?)],
            vec![
                ("__init__", f!(m, histogram_init)),
                ("run", f!(m, histogram_run)),
                ("remapXTicksData", f!(m, remapXTicksData)),
            ],
        )?;
        for (name, doc, x, init) in [
            (
                "HistogramPitchSpace",
                "A histogram of pitch space.",
                "PitchSpaceAxis",
                f!(m, histogram_ps_init),
            ),
            (
                "HistogramPitchClass",
                "A histogram of pitch class.",
                "PitchClassAxis",
                f!(m, histogram_pc_init),
            ),
            (
                "HistogramQuarterLength",
                "A histogram of quarter length.",
                "QuarterLengthAxis",
                f!(m, histogram_ql_init),
            ),
        ] {
            plot_class(
                m,
                name,
                doc,
                one("Histogram")?,
                vec![("axesClasses", counted(x)?)],
                vec![("__init__", init)],
            )?;
        }

        // Weighted scatter plots.
        let weighted = |x: &str, y: &str| axes(py, &[("x", x), ("y", y), ("z", "CountingAxis")]);
        plot_class(
            m,
            "ScatterWeighted",
            "The base of weighted scatter plots: two axes, each point sized by its \
             count, kept as the z axis.",
            with("GraphScatterWeighted")?,
            vec![("axesClasses", weighted("Axis", "Axis")?)],
            vec![("__init__", f!(m, weighted_init))],
        )?;
        plot_class(
            m,
            "ScatterWeightedPitchSpaceQuarterLength",
            "A weighted scatter plot of pitch space against quarter length.",
            one("ScatterWeighted")?,
            vec![(
                "axesClasses",
                weighted("QuarterLengthAxis", "PitchSpaceAxis")?,
            )],
            vec![("__init__", f!(m, weighted_ps_ql_init))],
        )?;
        plot_class(
            m,
            "ScatterWeightedPitchClassQuarterLength",
            "A weighted scatter plot of pitch class against quarter length.",
            one("ScatterWeighted")?,
            vec![(
                "axesClasses",
                weighted("QuarterLengthAxis", "PitchClassAxis")?,
            )],
            vec![("__init__", f!(m, weighted_pc_ql_init))],
        )?;
        plot_class(
            m,
            "ScatterWeightedPitchSpaceDynamicSymbol",
            "A weighted scatter plot of the dynamics each pitch sounds at.",
            one("ScatterWeighted")?,
            vec![("axesClasses", weighted("PitchSpaceAxis", "DynamicsAxis")?)],
            vec![
                ("__init__", f!(m, weighted_dynamics_init)),
                ("extractData", f!(m, weighted_dynamics_extract)),
            ],
        )?;

        // Windowed analyses.
        let grid_configurables: Vec<String> = primitive(py, "GraphColorGrid")?
            .getattr("keywordConfigurables")?
            .extract()?;
        let mut windowed_configurables: Vec<&str> =
            grid_configurables.iter().map(String::as_str).collect();
        windowed_configurables.extend([
            "minWindow",
            "maxWindow",
            "windowStep",
            "windowType",
            "compressLegend",
            "processorClass",
            "graphLegend",
        ]);
        let extension = crate::provide::extension(py)?;
        let windowed = plot_class(
            m,
            "WindowedAnalysis",
            "The base of plots of windowed analyses, such as of key or ambitus: a \
             grid of colours, a row per window size.",
            with("GraphColorGrid")?,
            vec![
                ("format", PyString::new(py, "colorGrid").into_any()),
                (
                    "keywordConfigurables",
                    super::strings(py, &windowed_configurables)?,
                ),
                ("axesClasses", axes(py, &[("x", "OffsetAxis")])?),
                (
                    "processorClassDefault",
                    extension.getattr("KrumhanslSchmuckler")?,
                ),
            ],
            vec![
                ("__init__", f!(m, windowed_init)),
                ("run", f!(m, windowed_run)),
                ("extractData", f!(m, windowed_extract)),
                ("_getLegend", f!(m, _getLegend)),
                ("write", f!(m, windowed_write)),
            ],
        )?;
        let getter = crate::plumbing::built::method(
            f!(m, processor),
            MODULE,
            "WindowedAnalysis",
            "processor",
        )?;
        windowed.setattr(
            "processor",
            py.import("builtins")?
                .getattr("property")?
                .call1((getter,))?,
        )?;
        plot_class(
            m,
            "WindowedKey",
            "A windowed Krumhansl-Schmuckler key analysis; set `processorClass` for \
             another weighting.",
            one("WindowedAnalysis")?,
            vec![(
                "processorClassDefault",
                extension.getattr("KrumhanslSchmuckler")?,
            )],
            vec![],
        )?;
        plot_class(
            m,
            "WindowedAmbitus",
            "A windowed analysis of pitch span.",
            one("WindowedAnalysis")?,
            vec![("processorClassDefault", extension.getattr("Ambitus")?)],
            vec![],
        )?;

        // Horizontal bars.
        plot_class(
            m,
            "HorizontalBar",
            "Each note as a bar over its time, in a row for its pitch; with \
             `colorByPart`, each part in its own colour.",
            with("GraphHorizontalBar")?,
            vec![(
                "axesClasses",
                axes(py, &[("x", "OffsetEndAxis"), ("y", "PitchSpaceAxis")])?,
            )],
            vec![
                ("__init__", f!(m, horizontal_init)),
                ("run", f!(m, horizontal_run)),
                ("assignColorsToParts", f!(m, assignColorsToParts)),
                ("postProcessElement", f!(m, horizontal_element)),
                ("postProcessData", f!(m, horizontal_post)),
            ],
        )?;
        plot_class(
            m,
            "HorizontalBarPitchClassOffset",
            "Each note as a bar over its time, in a row for its pitch class.",
            one("HorizontalBar")?,
            vec![(
                "axesClasses",
                axes(py, &[("x", "OffsetEndAxis"), ("y", "PitchClassAxis")])?,
            )],
            vec![("__init__", f!(m, horizontal_pc_init))],
        )?;
        plot_class(
            m,
            "HorizontalBarPitchSpaceOffset",
            "A piano roll: each note as a bar over its time, in a row for its pitch.",
            one("HorizontalBar")?,
            vec![],
            vec![("__init__", f!(m, horizontal_ps_init))],
        )?;
        let weighted_configurables: Vec<String> = primitive(py, "GraphHorizontalBarWeighted")?
            .getattr("keywordConfigurables")?
            .extract()?;
        let mut bar_configurables: Vec<&str> =
            weighted_configurables.iter().map(String::as_str).collect();
        bar_configurables.extend([
            "fillByMeasure",
            "normalizeByPart",
            "partGroups",
            "segmentByTarget",
        ]);
        plot_class(
            m,
            "HorizontalBarWeighted",
            "The base of plots of a score's parts as bars of varying height.",
            with("GraphHorizontalBarWeighted")?,
            vec![
                ("axesClasses", axes(py, &[("x", "OffsetAxis")])?),
                (
                    "keywordConfigurables",
                    super::strings(py, &bar_configurables)?,
                ),
            ],
            vec![
                ("__init__", f!(m, weighted_bars_init)),
                ("extractData", f!(m, weighted_bars_extract)),
            ],
        )?;
        plot_class(
            m,
            "Dolan",
            "How loudly each part, or group of parts, plays over time, its dynamics \
             lasting to the next; part groups default from the instruments.",
            one("HorizontalBarWeighted")?,
            vec![],
            vec![
                ("__init__", f!(m, dolan_init)),
                ("_getPartGroups", f!(m, _getPartGroups)),
            ],
        )?;

        // 3D bars.
        plot_class(
            m,
            "Plot3DBars",
            "The base of 3D bar plots: two axes, each bar as tall as its count.",
            with("Graph3DBars")?,
            vec![(
                "axesClasses",
                weighted("QuarterLengthAxis", "PitchClassAxis")?,
            )],
            vec![("__init__", f!(m, bars3d_init))],
        )?;
        plot_class(
            m,
            "Plot3DBarsPitchSpaceQuarterLength",
            "3D bars of the count of each pitch space and quarter length.",
            one("Plot3DBars")?,
            vec![(
                "axesClasses",
                weighted("QuarterLengthAxis", "PitchSpaceAxis")?,
            )],
            vec![("__init__", f!(m, bars3d_ps_init))],
        )?;

        // Several streams.
        plot_class(
            m,
            "MultiStream",
            "The base of plots of several streams, each given as a stream, a path \
             or a corpus name, labelled by `labelList` or by their file names.",
            with("GraphGroupedVerticalBar")?,
            vec![("axesClasses", PyDict::new(py).into_any())],
            vec![
                ("__init__", f!(m, multi_init)),
                ("parseStreams", f!(m, parseStreams)),
            ],
        )?;
        plot_class(
            m,
            "Features",
            "The values of some feature extractors, given as classes or ids, for \
             each stream.",
            one("MultiStream")?,
            vec![("format", PyString::new(py, "features").into_any())],
            vec![
                ("__init__", f!(m, features_init)),
                ("run", f!(m, features_run)),
                ("extractData", f!(m, features_extract)),
            ],
        )?;

        let order: Vec<Bound<'_, PyAny>> = [
            "HistogramPitchSpace",
            "HistogramPitchClass",
            "HistogramQuarterLength",
            "WindowedKey",
            "WindowedAmbitus",
            "ScatterPitchSpaceQuarterLength",
            "ScatterPitchClassQuarterLength",
            "ScatterPitchClassOffset",
            "ScatterPitchSpaceDynamicSymbol",
            "HorizontalBarPitchSpaceOffset",
            "HorizontalBarPitchClassOffset",
            "Dolan",
            "ScatterWeightedPitchSpaceQuarterLength",
            "ScatterWeightedPitchClassQuarterLength",
            "ScatterWeightedPitchSpaceDynamicSymbol",
            "Plot3DBarsPitchSpaceQuarterLength",
        ]
        .iter()
        .map(|name| m.getattr(*name))
        .collect::<PyResult<_>>()?;
        m.add("_DOC_ORDER", PyList::new(py, order)?)?;
        Ok::<_, PyErr>(m.clone().unbind())
    })?;
    Ok(held.bind(py).clone())
}
