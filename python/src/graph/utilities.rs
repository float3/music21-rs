//! music21's `graph.utilities`: matplotlib's modules, found when a graph
//! is first drawn, and colours and labels as matplotlib takes them, which
//! are the crate's `graph::utilities`.

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBool, PyFloat, PyInt, PyModule, PyString};

use music21::graph::utilities::{Color, Level, accidental_label_to_unicode, color_hex};

/// The module music21 keeps these in.
pub(crate) const MODULE: &str = "music21.graph.utilities";

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

/// `ExtendedModules`, made once.
static EXTENDED: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

pyo3::create_exception!(music21_rs_facade, GraphException, crate::Music21Exception);
pyo3::create_exception!(
    music21_rs_facade,
    PlotStreamException,
    crate::Music21Exception
);

/// A crate error as music21 raises it: a `GraphException` where the crate
/// says it is a graph's, a `ValueError` otherwise.
pub(crate) fn graph_error(error: music21::Error) -> PyErr {
    match error {
        music21::Error::Graph(message) => GraphException::new_err(message),
        music21::Error::Value(message) => pyo3::exceptions::PyValueError::new_err(message),
        other => pyo3::exceptions::PyValueError::new_err(other.to_string()),
    }
}

/// The named tuple music21 hands matplotlib's modules back in.
fn extended_class(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let held = EXTENDED.get_or_try_init(py, || {
        let made = py.import("collections")?.getattr("namedtuple")?.call1((
            "ExtendedModules",
            ["matplotlib", "Axes3D", "collections", "patches", "plt"],
        ))?;
        made.setattr("__module__", MODULE)?;
        Ok::<_, PyErr>(made.unbind())
    })?;
    Ok(held.bind(py).clone())
}

/// matplotlib and the parts of it the graphs draw with, imported when
/// first asked for rather than with music21: `(matplotlib, Axes3D,
/// collections, patches, plt)`. Without matplotlib, a `GraphException`.
#[pyfunction]
pub(crate) fn getExtendedModules(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let found = py
        .import("importlib.util")?
        .call_method1("find_spec", ("matplotlib",))?;
    if found.is_none() {
        return Err(GraphException::new_err(
            "could not find matplotlib, graphing is not allowed",
        ));
    }
    let matplotlib = py.import("matplotlib")?;
    let axes3d = match py.import("mpl_toolkits.mplot3d") {
        Ok(found) => found.getattr("Axes3D")?,
        Err(_) => py.None().into_bound(py),
    };
    let collections = py.import("matplotlib.collections")?;
    let patches = py.import("matplotlib.patches")?;
    let plt = py.import("matplotlib.pyplot")?;
    extended_class(py)?.call1((matplotlib, axes3d, collections, patches, plt))
}

/// matplotlib's `pyplot`, through `getExtendedModules`.
pub(crate) fn pyplot(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    getExtendedModules(py)?.getattr("plt")
}

/// A label with its sharps or flats as music symbols, as matplotlib's
/// fonts draw them; anything not a string is given back as it is.
#[pyfunction]
pub(crate) fn accidentalLabelToUnicode<'py>(
    label: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let Ok(text) = label.cast::<PyString>() else {
        return Ok(label.clone());
    };
    let drawn = accidental_label_to_unicode(text.to_str()?);
    Ok(PyString::new(label.py(), &drawn).into_any())
}

/// A number as a channel, or none where it is not a number. A bool is not
/// one, as `common.isNum` says.
fn level_of(value: &Bound<'_, PyAny>) -> PyResult<Option<Level>> {
    if value.is_instance_of::<PyBool>() {
        return Ok(None);
    }
    if value.is_instance_of::<PyInt>() {
        return Ok(Some(Level::Whole(value.extract()?)));
    }
    if value.is_instance_of::<PyFloat>() {
        return Ok(Some(Level::Real(value.extract()?)));
    }
    Ok(None)
}

/// Any way of giving a colour as the hex code matplotlib takes: a name, a
/// matplotlib letter, a hex code, a grey as one number, or a list of
/// channels as fractions or up to 255.
#[pyfunction]
pub(crate) fn getColor(color: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Some(level) = level_of(color)? {
        return color_hex(Color::Levels(&[level])).map_err(graph_error);
    }
    if let Ok(text) = color.cast::<PyString>() {
        if text.to_str()?.is_empty() {
            return Err(pyo3::exceptions::PyIndexError::new_err(
                "string index out of range",
            ));
        }
        return color_hex(Color::Text(text.to_str()?)).map_err(graph_error);
    }
    if crate::plumbing::classtools::isListLike(color) {
        let mut levels = Vec::new();
        for item in color.try_iter()? {
            let item = item?;
            match level_of(&item)? {
                Some(level) => levels.push(level),
                None => levels.push(Level::Real(item.extract()?)),
            }
        }
        return color_hex(Color::Levels(&levels)).map_err(graph_error);
    }
    let shown = color.str()?;
    Err(GraphException::new_err(format!(
        "invalid color specification: {shown}"
    )))
}

/// music21's `graph.utilities`, built.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        m.add("ExtendedModules", extended_class(py)?)?;
        m.add_function(wrap_pyfunction!(getExtendedModules, &m)?)?;
        m.add_function(wrap_pyfunction!(accidentalLabelToUnicode, &m)?)?;
        m.add_function(wrap_pyfunction!(getColor, &m)?)?;
        for exception in [
            py.get_type::<GraphException>(),
            py.get_type::<PlotStreamException>(),
        ] {
            exception.setattr("__module__", MODULE)?;
            m.add(exception.name()?, exception)?;
        }
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}
