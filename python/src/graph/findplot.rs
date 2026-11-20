//! music21's `graph.findPlot`: the plot classes and axis classes there
//! are, and the plots a format and some quantities ask for.
//!
//! The choice is the crate's `graph::find`, made over the classes as they
//! are at the time of asking -- each plot class's graph type and the
//! quantities of its axes classes -- so it is music21's choice over the
//! same classes.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyFunction, PyList, PyModule, PyTuple};

use music21::graph::find::{
    FORMAT_SYNONYMS, PLOT_SHORTCUTS, PlotCandidate, PlotChoice, axis_matches, plots_of_format,
    plots_to_make, user_format,
};

use super::built;
use super::utilities::graph_error;

/// The module music21 keeps these in.
pub(crate) const MODULE: &str = "music21.graph.findPlot";

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

/// The values of a module's names that are classes, sorted by name, kept
/// where `keep` says.
fn classes_of<'py>(
    module: &Bound<'py, PyModule>,
    keep: impl Fn(&Bound<'py, PyAny>) -> PyResult<bool>,
) -> PyResult<Bound<'py, PyList>> {
    let py = module.py();
    let mut names: Vec<String> = module.dict().keys().extract()?;
    names.sort();
    let found = PyList::empty(py);
    for name in names {
        let value = module.getattr(name.as_str())?;
        if !value.is_callable()
            || value.is_instance_of::<PyFunction>()
            || !value.hasattr("__mro__")?
        {
            continue;
        }
        if keep(&value)? {
            found.append(value)?;
        }
    }
    Ok(found)
}

/// Every plot class: each a `PlotStreamMixin` and a `Graph`, sorted by
/// name.
#[pyfunction]
fn getPlotClasses(py: Python<'_>) -> PyResult<Bound<'_, PyList>> {
    let plot = super::module(py, super::plot::MODULE)?;
    let mixin = plot.getattr("PlotStreamMixin")?;
    let graph = built(py, super::primitives::MODULE, "Graph")?;
    classes_of(&plot, |class| {
        let mro = class.getattr("__mro__")?;
        Ok(mro.contains(&mixin)? && mro.contains(&graph)?)
    })
}

/// Every axis class, sorted by name.
#[pyfunction]
fn getAxisClasses(py: Python<'_>) -> PyResult<Bound<'_, PyList>> {
    let axis = super::module(py, super::axis::MODULE)?;
    let base = axis.getattr("Axis")?;
    let builtins = py.import("builtins")?;
    classes_of(&axis, |class| {
        builtins
            .call_method1("issubclass", (class, &base))?
            .is_truthy()
    })
}

/// The first quantity of each axis class, or every quantity where
/// `synonyms` is set.
#[pyfunction]
#[pyo3(signature = (synonyms=false, axesToCheck=None))]
fn getAxisQuantities<'py>(
    py: Python<'py>,
    synonyms: bool,
    axesToCheck: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyList>> {
    let axes = match axesToCheck.filter(|axes| !axes.is_none()) {
        Some(axes) => axes.clone(),
        None => getAxisClasses(py)?.into_any(),
    };
    let all = PyList::empty(py);
    for class in axes.try_iter()? {
        let quantities = class?.getattr("quantities")?;
        if synonyms {
            all.call_method1("extend", (quantities,))?;
        } else {
            all.append(quantities.get_item(0)?)?;
        }
    }
    Ok(all)
}

/// A user's name for a format as the format's own; unknown ones
/// lowercased, without spaces.
#[pyfunction]
fn userFormatsToFormat(userFormat: &str) -> String {
    user_format(userFormat)
}

/// A plot class as the crate chooses among them.
fn candidate(class: &Bound<'_, PyAny>) -> PyResult<PlotCandidate> {
    let mut axes = Vec::new();
    for item in class
        .getattr("axesClasses")?
        .call_method0("items")?
        .try_iter()?
    {
        let (letter, axis): (String, Bound<'_, PyAny>) = item?.extract()?;
        let quantities: Vec<String> = axis.getattr("quantities")?.extract()?;
        axes.push((letter.chars().next().unwrap_or('x'), quantities));
    }
    Ok(PlotCandidate {
        graph_type: class.getattr("graphType")?.extract()?,
        axes,
    })
}

/// The plot classes of a format, among `checkPlotClasses` or all.
#[pyfunction]
#[pyo3(signature = (graphFormat, checkPlotClasses=None))]
fn getPlotClassesFromFormat<'py>(
    py: Python<'py>,
    graphFormat: &str,
    checkPlotClasses: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyList>> {
    let classes: Vec<Bound<'py, PyAny>> = match checkPlotClasses.filter(|c| !c.is_none()) {
        Some(classes) => classes.try_iter()?.collect::<PyResult<_>>()?,
        None => getPlotClasses(py)?.iter().collect(),
    };
    let candidates = classes
        .iter()
        .map(candidate)
        .collect::<PyResult<Vec<_>>>()?;
    let all: Vec<usize> = (0..classes.len()).collect();
    let kept = plots_of_format(graphFormat, &candidates, &all);
    PyList::new(py, kept.into_iter().map(|index| &classes[index]))
}

/// The first axis class with the value among its quantities, or `None`.
#[pyfunction]
fn getAxisClassFromValue<'py>(py: Python<'py>, axisValue: &str) -> PyResult<Bound<'py, PyAny>> {
    for class in getAxisClasses(py)?.iter() {
        if axisMatchesValue(&class, axisValue)? {
            return Ok(class);
        }
    }
    Ok(py.None().into_bound(py))
}

/// Whether a value, ignoring case, is one of an axis class's quantities.
#[pyfunction]
fn axisMatchesValue(axisClass: &Bound<'_, PyAny>, axisValue: &str) -> PyResult<bool> {
    let quantities: Vec<String> = axisClass.getattr("quantities")?.extract()?;
    Ok(axis_matches(&quantities, axisValue))
}

/// The plot classes to make for a format and up to three quantities, a
/// piano roll where nothing is asked; where no class plots what is asked,
/// the nearest class with a dict of the axis class for each axis.
#[pyfunction]
#[pyo3(signature = (graphFormat=None, xValue=None, yValue=None, zValue=None))]
fn getPlotsToMake<'py>(
    py: Python<'py>,
    graphFormat: Option<&str>,
    xValue: Option<&str>,
    yValue: Option<&str>,
    zValue: Option<&str>,
) -> PyResult<Bound<'py, PyList>> {
    let mut classes: Vec<Bound<'py, PyAny>> = getPlotClasses(py)?.iter().collect();

    // A shortcut's class, among the candidates.
    let plot = super::module(py, super::plot::MODULE)?;
    let named = if graphFormat.is_none() && [xValue, yValue, zValue].iter().all(Option::is_none) {
        Some("pianoroll")
    } else {
        graphFormat
    };
    let mut shortcut = None;
    if let Some((_, class_name)) =
        named.and_then(|format| PLOT_SHORTCUTS.iter().find(|(name, _)| *name == format))
    {
        let class = plot.getattr(*class_name)?;
        let found = classes.iter().position(|known| known.is(&class));
        shortcut = Some(match found {
            Some(index) => index,
            None => {
                classes.push(class);
                classes.len() - 1
            }
        });
    }
    let candidates = classes
        .iter()
        .map(candidate)
        .collect::<PyResult<Vec<_>>>()?;
    let axis_classes: Vec<Bound<'py, PyAny>> = getAxisClasses(py)?.iter().collect();
    let axis_quantities = axis_classes
        .iter()
        .map(|class| class.getattr("quantities")?.extract())
        .collect::<PyResult<Vec<Vec<String>>>>()?;

    let choice = plots_to_make(
        graphFormat,
        [xValue, yValue, zValue],
        shortcut,
        &candidates,
        &axis_quantities,
    )
    .map_err(|error| match error {
        music21::Error::Value(message) if message.contains("NoneType") => {
            pyo3::exceptions::PyAttributeError::new_err(message)
        }
        music21::Error::Value(message) => pyo3::exceptions::PyIndexError::new_err(message),
        other => graph_error(other),
    })?;
    match choice {
        PlotChoice::Plots(indices) => {
            PyList::new(py, indices.into_iter().map(|index| &classes[index]))
        }
        PlotChoice::WithAxes { plot: chosen, axes } => {
            let ordered = py.import("collections")?.getattr("OrderedDict")?.call0()?;
            for (letter, axis) in axes {
                ordered.set_item(letter.to_string(), &axis_classes[axis])?;
            }
            let pair = PyTuple::new(py, [classes[chosen].clone(), ordered])?;
            PyList::new(py, [pair])
        }
    }
}

/// `{name: class}` of the shortcut formats.
fn shortcuts(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let plot = super::module(py, super::plot::MODULE)?;
    let made = PyDict::new(py);
    for (name, class) in PLOT_SHORTCUTS {
        made.set_item(name, plot.getattr(class)?)?;
    }
    Ok(made)
}

/// music21's `graph.findPlot`, built.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        m.add("PLOTCLASS_SHORTCUTS", shortcuts(py)?)?;
        let synonyms = PyList::empty(py);
        for names in FORMAT_SYNONYMS {
            synonyms.append(PyTuple::new(py, names)?)?;
        }
        m.add("FORMAT_SYNONYMS", synonyms)?;
        let formats: Vec<&str> = FORMAT_SYNONYMS.iter().map(|names| names[0]).collect();
        m.add("FORMATS", formats)?;
        m.add_function(wrap_pyfunction!(getPlotClasses, &m)?)?;
        m.add_function(wrap_pyfunction!(getAxisClasses, &m)?)?;
        m.add_function(wrap_pyfunction!(getAxisQuantities, &m)?)?;
        m.add_function(wrap_pyfunction!(userFormatsToFormat, &m)?)?;
        m.add_function(wrap_pyfunction!(getPlotClassesFromFormat, &m)?)?;
        m.add_function(wrap_pyfunction!(getAxisClassFromValue, &m)?)?;
        m.add_function(wrap_pyfunction!(axisMatchesValue, &m)?)?;
        m.add_function(wrap_pyfunction!(getPlotsToMake, &m)?)?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}
