//! music21's `tree.analysis`: `Horizontality`, one part's timespans one
//! after another, and the melodic shapes it recognises.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PySet, PyTuple};

use crate::exceptions21::TreeException;

pyo3::create_exception!(music21_rs_facade, HorizontalityException, TreeException);

/// How many timespans the shapes are read from.
const SHAPE_LENGTH: usize = 3;

/// The widest step, in semitones, a neighbour tone is short of.
const NEIGHBOUR_LIMIT: f64 = 3.0;

/// One part's timespans one after another: music21's
/// `tree.analysis.Horizontality`.
#[pyclass(name = "Horizontality", module = "music21.tree.analysis", frozen)]
pub struct Horizontality {
    timespans: Py<PyTuple>,
}

impl Horizontality {
    /// One of timespans, as music21's constructor checks them.
    pub(crate) fn of(timespans: &Bound<'_, PyAny>) -> PyResult<Self> {
        let py = timespans.py();
        if timespans.try_iter().is_err() || timespans.hasattr("keys")? {
            return Err(HorizontalityException::new_err(format!(
                "timespans must be a sequence, not {}",
                timespans.repr()?
            )));
        }
        let held = timespans.try_iter()?.collect::<PyResult<Vec<_>>>()?;
        if held.is_empty() {
            return Err(HorizontalityException::new_err(
                "there must be at least one timespan in the timespans list",
            ));
        }
        for span in &held {
            if !span.hasattr("offset")? || !span.hasattr("endTime")? {
                return Err(HorizontalityException::new_err(
                    "only Timespan objects can be added to a horizontality",
                ));
            }
        }
        Ok(Self {
            timespans: PyTuple::new(py, held)?.unbind(),
        })
    }

    /// The first pitch of each of the first three timespans, where there
    /// are three and each has a pitch.
    fn first_pitches<'py>(&self, py: Python<'py>) -> PyResult<Option<Vec<Bound<'py, PyAny>>>> {
        let spans = self.timespans.bind(py);
        if spans.len() < SHAPE_LENGTH {
            return Ok(None);
        }
        for span in spans.iter() {
            if span.getattr("pitches")?.len()? == 0 {
                return Ok(None);
            }
        }
        let mut pitches = Vec::new();
        for span in spans.iter().take(SHAPE_LENGTH) {
            pitches.push(span.getattr("pitches")?.get_item(0)?);
        }
        Ok(Some(pitches))
    }
}

#[pymethods]
impl Horizontality {
    #[new]
    #[pyo3(signature = (timespans = None))]
    fn py_new(py: Python<'_>, timespans: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        match timespans {
            Some(timespans) => Self::of(timespans),
            None => Self::of(&py.None().into_bound(py)),
        }
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.timespans)
    }

    /// The timespans, in order.
    #[getter]
    fn timespans(&self, py: Python<'_>) -> Py<PyTuple> {
        self.timespans.clone_ref(py)
    }

    fn __len__(&self, py: Python<'_>) -> usize {
        self.timespans.bind(py).len()
    }

    fn __getitem__<'py>(
        &self,
        py: Python<'py>,
        item: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.timespans.bind(py).as_any().get_item(item)
    }

    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        Ok(self.timespans.bind(py).as_any().try_iter()?.into_any())
    }

    /// Whether the first pitches of the first three rise all the way or
    /// fall all the way.
    #[getter]
    fn hasPassingTone(&self, py: Python<'_>) -> PyResult<bool> {
        let Some(pitches) = self.first_pitches(py)? else {
            return Ok(false);
        };
        let rising = pitches[0].lt(&pitches[1])? && pitches[1].lt(&pitches[2])?;
        let falling = pitches[0].gt(&pitches[1])? && pitches[1].gt(&pitches[2])?;
        Ok(rising || falling)
    }

    /// Whether the first and third of the first three are the same pitch
    /// and the second is less than three semitones from them.
    #[getter]
    fn hasNeighborTone(&self, py: Python<'_>) -> PyResult<bool> {
        let Some(pitches) = self.first_pitches(py)? else {
            return Ok(false);
        };
        if !pitches[0].eq(&pitches[2])? {
            return Ok(false);
        }
        let first: f64 = pitches[0].getattr("ps")?.extract()?;
        let second: f64 = pitches[1].getattr("ps")?.extract()?;
        Ok((second - first).abs() < NEIGHBOUR_LIMIT)
    }

    /// Whether every timespan sounds the same pitches.
    #[getter]
    fn hasNoMotion(&self, py: Python<'_>) -> PyResult<bool> {
        let sets = PySet::empty(py)?;
        for span in self.timespans.bind(py).iter() {
            let pitches = span
                .getattr("pitches")?
                .try_iter()?
                .collect::<PyResult<Vec<_>>>()?;
            sets.add(PyTuple::new(py, pitches)?)?;
        }
        Ok(sets.len() == 1)
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let mut shown = Vec::new();
        for span in self.timespans.bind(py).iter() {
            let mut names = Vec::new();
            if let Ok(pitches) = span.getattr("pitches") {
                for pitch in pitches.try_iter()? {
                    names.push(pitch?.getattr("nameWithOctave")?.extract::<String>()?);
                }
            }
            shown.push(format!("({},)", names.join(", ")));
        }
        Ok(format!("<Horizontality: {}>", shown.join(" ")))
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Horizontality>()?;
    Ok(())
}
