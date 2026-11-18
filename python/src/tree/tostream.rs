//! music21's `tree.toStream`: a timespan tree made back into a stream of
//! chords. music21 deprecated both functions in v7 for `chordify()`, and
//! they warn as music21's do.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyModule, PySet};

use super::TimespanTree;
use crate::exceptions21::TreeException;
use crate::plumbing::built::method;
use crate::plumbing::decorators::deprecated_function;

/// The module music21 keeps these in.
const MODULE: &str = "music21.tree.toStream";

/// The release that deprecated these, and the one music21 meant to remove
/// them in.
const DEPRECATED_IN: &str = "v7";
const REMOVED_IN: &str = "v8";
const INSTEAD: &str = "use chordify() instead";

/// A chord or rest for each stretch between two time points of a tree,
/// lasting until the next.
fn chords_between<'py>(
    timespans: &Bound<'py, PyAny>,
    points: &[Bound<'py, PyAny>],
) -> PyResult<Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>)>> {
    let mut made = Vec::new();
    for pair in points.windows(2) {
        let (offset, end) = (&pair[0], &pair[1]);
        let verticality = timespans.call_method1("getVerticalityAt", (offset,))?;
        let length = end.sub(offset)?;
        if length.lt(0)? {
            return Err(TreeException::new_err(format!(
                "Something is wrong with the verticality {}, its endTime {} is less than its \
                 offset {}",
                verticality.repr()?,
                end.str()?,
                offset.str()?
            )));
        }
        made.push((
            offset.clone(),
            verticality.call_method1("makeElement", (length,))?,
        ));
    }
    Ok(made)
}

/// A score of the chords of a timespan tree, laid into the measures of
/// `templateStream` where one is given: music21's `chordified`.
#[pyfunction]
#[pyo3(signature = (timespans, templateStream = None))]
fn chordified<'py>(
    timespans: &Bound<'py, PyAny>,
    templateStream: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = timespans.py();
    if timespans.cast::<TimespanTree>().is_err() {
        return Err(TreeException::new_err("Needs a TimespanTree to run"));
    }
    let stream = py.import("music21.stream")?;
    let Some(template) = templateStream.filter(|template| !template.is_none()) else {
        let points: Vec<_> = timespans
            .call_method0("allTimePoints")?
            .try_iter()?
            .collect::<PyResult<_>>()?;
        let score = stream.getattr("Score")?.call0()?;
        for (_, element) in chords_between(timespans, &points)? {
            score.call_method1("append", (element,))?;
        }
        return Ok(score);
    };

    // The measures' offsets, and the template's end.
    let mut bounds: Vec<Bound<'py, PyAny>> = template
        .call_method0("measureOffsetMap")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    bounds.push(template.getattr("duration")?.getattr("quarterLength")?);
    let keywords = PyDict::new(py);
    keywords.set_item("fillWithRests", false)?;
    keywords.set_item("retainVoices", false)?;
    let parts = template
        .getattr("parts")
        .ok()
        .filter(|parts| parts.is_truthy().unwrap_or(false));
    let output = match parts {
        Some(parts) => parts
            .get_item(0)?
            .call_method("template", (), Some(&keywords))?,
        None => template.call_method("template", (), Some(&keywords))?,
    };

    // Split at each measure line, then a chord for each stretch.
    let split = timespans.call_method0("copy")?;
    split.call_method1("splitAt", (PyList::new(py, &bounds)?,))?;
    let points = PySet::empty(py)?;
    for point in split.call_method0("allTimePoints")?.try_iter()? {
        points.add(point?)?;
    }
    for bound in &bounds {
        points.add(bound)?;
    }
    let last = bounds[bounds.len() - 1].clone();
    let sorted = py.import("builtins")?.getattr("sorted")?.call1((points,))?;
    let mut kept = Vec::new();
    for point in sorted.try_iter()? {
        let point = point?;
        if point.le(&last)? {
            kept.push(point);
        }
    }
    let measures: Vec<Bound<'py, PyAny>> = output
        .call_method1("getElementsByClass", ("Measure",))?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    let mut measure = 0;
    let mut upcoming = 1;
    for (offset, element) in chords_between(&split, &kept)? {
        while upcoming < bounds.len() && bounds[upcoming].le(&offset)? {
            upcoming += 1;
            measure += 1;
        }
        measures[measure].call_method1("append", (element,))?;
    }
    Ok(output)
}

/// A score of each part's chords, laid into the part of `templateStream`
/// it came from: music21's `partwise`.
#[pyfunction]
fn partwise<'py>(
    tsTree: &Bound<'py, PyAny>,
    templateStream: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = tsTree.py();
    let mapping = tsTree.call_method0("toPartwiseTimespanTrees")?;
    let score = py.import("music21.stream")?.getattr("Score")?.call0()?;
    for part in templateStream.getattr("parts")?.try_iter()? {
        let part = part?;
        let Some(timespans) = mapping.get_item(&part).ok() else {
            continue;
        };
        score.call_method1("append", (chordified(&timespans, Some(&part))?,))?;
    }
    Ok(score)
}

/// music21's `tree.toStream`, built whole.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let module = PyModule::new(py, "toStream")?;
    for function in [
        wrap_pyfunction!(chordified, &module)?,
        wrap_pyfunction!(partwise, &module)?,
    ] {
        // A function with a `__dict__`, which `deprecated` marks.
        let name: String = function.getattr("__name__")?.extract()?;
        let held = method(function.into_any(), MODULE, "", &name)?;
        held.setattr("__qualname__", &name)?;
        let warned =
            deprecated_function(&held, Some(DEPRECATED_IN), Some(REMOVED_IN), Some(INSTEAD))?;
        module.add(name.as_str(), warned)?;
    }
    Ok(module)
}
