//! music21's `common.parallel`: running a function over many inputs on
//! every processor but one, through joblib, or on one where that is not
//! safe or not worth it.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::PyZeroDivisionError;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyList, PyTuple};

use music21::common::parallel::{cpus_for, update_positions};

use super::built::with_context;

/// The module music21 keeps these in.
pub(crate) const MODULE: &str = "music21.common.parallel";

/// The processors to use: one fewer than `multiprocessing` counts, from
/// three up, leaving one free.
#[pyfunction]
pub(crate) fn cpus(py: Python<'_>) -> PyResult<usize> {
    let count: usize = py
        .import("multiprocessing")?
        .call_method0("cpu_count")?
        .extract()?;
    Ok(cpus_for(count))
}

/// Whether starting processes is safe and worth it: more than one
/// processor, not inside a worker process already, and not inside a
/// pytest-xdist worker.
#[pyfunction]
pub(crate) fn safeToParallize(py: Python<'_>) -> PyResult<bool> {
    if cpus(py)? <= 1 {
        return Ok(false);
    }
    if py
        .import("multiprocessing")?
        .call_method0("parent_process")?
        .is_truthy()?
    {
        return Ok(false);
    }
    let environment = py.import("os")?.getattr("environ")?;
    Ok(!environment.contains("PYTEST_XDIST_WORKER")?)
}

/// What both runners are given besides the inputs and the function.
struct Updates<'py> {
    function: Bound<'py, PyAny>,
    every: i64,
    unpack: bool,
    sends_iterable: bool,
}

impl<'py> Updates<'py> {
    /// Reports that `done` of `total` are done: a line printed where the
    /// update function is `True`, else the function called for each of the
    /// positions `window` before `done`, with its result where there is one.
    fn report(
        &self,
        done: i64,
        total: i64,
        window: i64,
        iterable: &Bound<'py, PyAny>,
        results: &Bound<'py, PyList>,
    ) -> PyResult<()> {
        let py = iterable.py();
        if self.function.is(PyBool::new(py, true)) {
            let message = format!("Done {} tasks of {total}", done.min(total));
            py.import("builtins")?.getattr("print")?.call1((message,))?;
            return Ok(());
        }
        if !self.function.is_callable() {
            return Ok(());
        }
        for position in update_positions(done, window) {
            let result = match usize::try_from(position) {
                Ok(at) if at < results.len() => results.get_item(at)?,
                _ => py.None().into_bound(py),
            };
            if self.sends_iterable {
                self.function
                    .call1((position, total, result, iterable.get_item(position)?))?;
            } else {
                self.function.call1((position, total, result))?;
            }
        }
        Ok(())
    }

    /// One call of the function on the input at `position`.
    fn arguments(
        &self,
        iterable: &Bound<'py, PyAny>,
        position: i64,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let input = iterable.get_item(position)?;
        if self.unpack {
            return input
                .try_iter()?
                .collect::<PyResult<Vec<_>>>()
                .and_then(|all| PyTuple::new(input.py(), all));
        }
        PyTuple::new(input.py(), [input])
    }
}

/// Runs `parallelFunction` over `iterable` on one processor, in order,
/// exactly as `runParallel` would but for the processes: what
/// `runParallel` falls back to where starting them is not safe.
#[pyfunction]
#[pyo3(signature = (
    iterable,
    parallelFunction,
    *,
    updateFunction = None,
    updateMultiply = 3,
    unpackIterable = false,
    updateSendsIterable = false,
))]
pub(crate) fn runNonParallel<'py>(
    iterable: &Bound<'py, PyAny>,
    parallelFunction: &Bound<'py, PyAny>,
    updateFunction: Option<Bound<'py, PyAny>>,
    updateMultiply: i64,
    unpackIterable: bool,
    updateSendsIterable: bool,
) -> PyResult<Bound<'py, PyList>> {
    let py = iterable.py();
    let total = i64::try_from(iterable.len()?)?;
    let updates = Updates {
        function: updateFunction.unwrap_or_else(|| py.None().into_bound(py)),
        every: updateMultiply,
        unpack: unpackIterable,
        sends_iterable: updateSendsIterable,
    };
    let results = PyList::empty(py);
    let report = |done: i64| -> PyResult<()> {
        if updates.every == 0 {
            return Err(PyZeroDivisionError::new_err("integer modulo by zero"));
        }
        if done % updates.every != 0 {
            return Ok(());
        }
        updates.report(done, total, updates.every, iterable, &results)
    };
    report(0)?;
    for position in 0..total {
        let arguments = updates.arguments(iterable, position)?;
        results.append(parallelFunction.call1(arguments)?)?;
        report(position + 1)?;
    }
    Ok(results)
}

/// Runs `parallelFunction` over `iterable` on every processor but one,
/// through joblib, reporting to `updateFunction` after each batch of
/// `cpus() * updateMultiply` inputs; with no update function, in one batch.
///
/// The update function is given the position, the number of inputs and the
/// result there, and the input too with `updateSendsIterable`; `True` prints
/// a line instead. With `unpackIterable` each input is a sequence of the
/// function's arguments. The function, the inputs and the results must all
/// pickle. Where `safeToParallize` says no, this is `runNonParallel`.
#[pyfunction]
#[pyo3(signature = (
    iterable,
    parallelFunction,
    *,
    updateFunction = None,
    updateMultiply = 3,
    unpackIterable = false,
    updateSendsIterable = false,
))]
pub(crate) fn runParallel<'py>(
    iterable: &Bound<'py, PyAny>,
    parallelFunction: &Bound<'py, PyAny>,
    updateFunction: Option<Bound<'py, PyAny>>,
    updateMultiply: i64,
    unpackIterable: bool,
    updateSendsIterable: bool,
) -> PyResult<Bound<'py, PyList>> {
    let py = iterable.py();
    if !safeToParallize(py)? {
        return runNonParallel(
            iterable,
            parallelFunction,
            updateFunction,
            updateMultiply,
            unpackIterable,
            updateSendsIterable,
        );
    }
    let workers = i64::try_from(cpus(py)?)?;
    let total = i64::try_from(iterable.len()?)?;
    let every = if updateFunction.is_none() {
        total
    } else {
        updateMultiply
    };
    let updates = Updates {
        function: updateFunction.unwrap_or_else(|| py.None().into_bound(py)),
        every,
        unpack: unpackIterable,
        sends_iterable: updateSendsIterable,
    };
    let results = PyList::empty(py);
    let window = every * workers;
    updates.report(0, total, window, iterable, &results)?;
    let joblib = py.import("joblib")?;
    let keywords = PyDict::new(py);
    keywords.set_item("n_jobs", workers)?;
    let runner = joblib.getattr("Parallel")?.call((), Some(&keywords))?;
    with_context(&runner, |para| {
        let delayed = joblib.getattr("delayed")?.call1((parallelFunction,))?;
        let mut run = 0;
        while run < total {
            let end = (run + window).min(total);
            let mut calls = Vec::new();
            for position in run..end {
                calls.push(delayed.call1(updates.arguments(iterable, position)?)?);
            }
            let batch = para.call1((PyList::new(py, calls)?,))?;
            run = end;
            results.call_method1("extend", (batch,))?;
            updates.report(run, total, window, iterable, &results)?;
        }
        Ok(())
    })?;
    Ok(results)
}

/// music21's `common.parallel`, built.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let m = PyModule::new(py, MODULE)?;
    m.add_function(wrap_pyfunction!(cpus, &m)?)?;
    m.add_function(wrap_pyfunction!(runParallel, &m)?)?;
    m.add_function(wrap_pyfunction!(runNonParallel, &m)?)?;
    m.add_function(wrap_pyfunction!(safeToParallize, &m)?)?;
    m.add(
        "__all__",
        ["cpus", "runParallel", "runNonParallel", "safeToParallize"],
    )?;
    Ok(m)
}
