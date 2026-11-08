//! music21's `humdrum.instruments`: the instrument each Humdrum `*I` code
//! and `*IC` class names, over the crate's Humdrum tables.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyModule};

use music21::humdrum::{INSTRUMENT_CLASSES, INSTRUMENTS};

/// The module music21 keeps these in.
const MODULE: &str = "music21.humdrum.instruments";

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

pyo3::create_exception!(
    music21_rs_facade,
    HumdrumInstrumentException,
    crate::Music21Exception
);

/// A table as the dictionary music21 keeps it in.
fn table<'py>(py: Python<'py>, rows: &[(&str, &str)]) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new(py);
    for (code, class) in rows {
        out.set_item(code, class)?;
    }
    Ok(out)
}

/// The instrument the class of that name in music21's `instrument` makes,
/// or the wheel's own where music21 is not being answered for.
fn instrument_named<'py>(py: Python<'py>, class: &str) -> PyResult<Bound<'py, PyAny>> {
    let module = py
        .import("music21.instrument")
        .or_else(|_| py.import("music21_rs"))?;
    module.getattr(class)?.call0()
}

/// The instrument a code names in a table, or music21's refusal.
fn instrument_for<'py>(
    py: Python<'py>,
    rows: &[(&str, &str)],
    code: &Bound<'py, PyAny>,
    refusal: String,
) -> PyResult<Bound<'py, PyAny>> {
    let class = code
        .extract::<String>()
        .ok()
        .and_then(|code| rows.iter().find(|(known, _)| *known == code));
    let Some((_, class)) = class else {
        return Err(HumdrumInstrumentException::new_err(refusal));
    };
    instrument_named(py, class).map_err(|error| {
        if error.is_instance_of::<pyo3::exceptions::PyAttributeError>(py) {
            HumdrumInstrumentException::new_err(refusal)
        } else {
            error
        }
    })
}

/// The instrument a Humdrum instrument class (`*ICvox`) names.
#[pyfunction]
fn fromHumdrumClass<'py>(
    py: Python<'py>,
    hdClass: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let refusal = format!(
        "Cannot get an instrument from this Humdrum class *IC{}",
        hdClass.str()?
    );
    instrument_for(py, INSTRUMENT_CLASSES, hdClass, refusal)
}

/// The instrument a Humdrum instrument code (`*Icalto`) names.
#[pyfunction]
fn fromHumdrumInstrument<'py>(
    py: Python<'py>,
    hdInst: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let refusal = format!(
        "Cannot get an instrument from this Humdrum class: *I{}",
        hdInst.str()?
    );
    instrument_for(py, INSTRUMENTS, hdInst, refusal)
}

/// music21's `humdrum.instruments`, built.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        m.add(
            "humdrumInstrumentClassToInstrument",
            table(py, INSTRUMENT_CLASSES)?,
        )?;
        m.add("humdrumInstruments", table(py, INSTRUMENTS)?)?;
        let exception = py.get_type::<HumdrumInstrumentException>();
        exception.setattr("__module__", MODULE)?;
        m.add("HumdrumInstrumentException", exception)?;
        m.add_function(wrap_pyfunction!(fromHumdrumClass, &m)?)?;
        m.add_function(wrap_pyfunction!(fromHumdrumInstrument, &m)?)?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}
