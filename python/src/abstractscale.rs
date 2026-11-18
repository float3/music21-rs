//! music21's subclasses of `AbstractScale`: each builds the interval
//! network of one pattern of steps in its constructor, from the crate's
//! `scale::intervalnetwork`.
//!
//! A pattern the crate has by name (the diatonic modes, the minors, the
//! rags, the octatonic in its second mode) also carries that name, so a
//! concrete scale given one walks the crate's own scale; the rest are
//! realized through their networks.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString, PyTuple};

use music21::Interval as RsInterval;
use music21::scale::intervalnetwork::{
    IntervalNetwork as RsNetwork, Octaves, close_on_tonic, cyclical, octave_repeating,
    weighted_hexatonic_blues,
};
use music21::scale::{ScaleType as RsScaleType, Simplification};

use crate::Walkable;
use crate::interval::{Interval, interval_from_any};
use crate::scale::{AbstractScale, ConcreteScale, ScaleException};

/// The names this module hands out, all under `music21.scale`.
pub const NAMES: &[&str] = &[
    "AbstractDiatonicScale",
    "AbstractOctatonicScale",
    "AbstractHarmonicMinorScale",
    "AbstractMelodicMinorScale",
    "AbstractCyclicalScale",
    "AbstractOctaveRepeatingScale",
    "AbstractRagAsawari",
    "AbstractRagMarwa",
    "AbstractWeightedHexatonicBlues",
    "WeightedHexatonicBlues",
];

/// The steps of the major scale, which every diatonic mode rotates.
const DIATONIC_STEPS: [&str; 7] = ["M2", "M2", "m2", "M2", "M2", "M2", "m2"];

/// The octatonic steps from a whole step, music21's second mode.
const OCTATONIC_STEPS: [&str; 8] = ["M2", "m2", "M2", "m2", "M2", "m2", "M2", "m2"];

/// The harmonic minor's steps, closing on the leading tone.
const HARMONIC_MINOR_STEPS: [&str; 7] = ["M2", "m2", "M2", "M2", "m2", "A2", "m2"];

/// The degree music21 calls the dominant of every one of these.
const DOMINANT: i64 = 5;

/// One diatonic mode as music21's `AbstractDiatonicScale.buildNetwork`
/// lays it out: the names it answers to, how far the major steps are
/// rotated, and its tonic, dominant and relative degrees.
struct DiatonicMode {
    names: &'static [&'static str],
    rotation: usize,
    tonic: i64,
    dominant: i64,
    relative_major: i64,
    relative_minor: i64,
}

const fn mode(
    names: &'static [&'static str],
    rotation: usize,
    (tonic, dominant): (i64, i64),
    (relative_major, relative_minor): (i64, i64),
) -> DiatonicMode {
    DiatonicMode {
        names,
        rotation,
        tonic,
        dominant,
        relative_major,
        relative_minor,
    }
}

/// The authentic modes stand on their first degree, the plagal ones on
/// their fourth.
const AUTHENTIC: (i64, i64) = (1, 5);

const DIATONIC_MODES: [DiatonicMode; 13] = [
    mode(&["major", "ionian"], 0, AUTHENTIC, (1, 6)),
    mode(&["dorian"], 1, AUTHENTIC, (7, 5)),
    mode(&["phrygian"], 2, AUTHENTIC, (6, 4)),
    mode(&["lydian"], 3, AUTHENTIC, (5, 3)),
    mode(&["mixolydian"], 4, AUTHENTIC, (4, 2)),
    mode(&["aeolian", "minor"], 5, AUTHENTIC, (3, 1)),
    mode(&["locrian"], 6, AUTHENTIC, (2, 7)),
    mode(&["hypodorian"], 5, (4, 6), (3, 1)),
    mode(&["hypophrygian"], 6, (4, 7), (2, 7)),
    mode(&["hypolydian"], 0, (4, 6), (1, 6)),
    mode(&["hypomixolydian"], 1, (4, 7), (7, 5)),
    mode(&["hypoaeolian"], 2, (4, 6), (6, 4)),
    mode(&["hypolocrian"], 3, (4, 6), (5, 3)),
];

fn scale_error(error: music21::Error) -> PyErr {
    ScaleException::new_err(crate::pitch::message(&error))
}

/// Interval names as the crate's intervals.
fn named(names: &[&str]) -> PyResult<Vec<RsInterval>> {
    names
        .iter()
        .map(|name| RsInterval::from_name(name).map_err(scale_error))
        .collect()
}

/// A network of these steps walked both ways, repeating at the octave and
/// spelled as `simplification` says.
fn bidirected(steps: &[RsInterval], simplification: Simplification) -> RsNetwork {
    let mut network = RsNetwork::default();
    network.fill_bidirected(steps);
    network.octaves = Octaves::Duplicating;
    network.simplification = simplification;
    network
}

/// music21's `_toIntervals`: an interval or its name as a list of one, a
/// list of either as intervals, and nothing as no intervals.
fn to_intervals(value: Option<&Bound<'_, PyAny>>) -> PyResult<Vec<RsInterval>> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(Vec::new());
    };
    if value.is_instance_of::<PyString>() || value.extract::<PyRef<'_, Interval>>().is_ok() {
        return Ok(vec![interval_from_any(value)?]);
    }
    value
        .walk()?
        .map(|item| interval_from_any(&item?))
        .collect()
}

/// music21's `_intervalListOrMode`: `intervalList`, or `mode` where that
/// was given instead under its old name, with a warning.
fn list_or_mode<'py>(
    class: &str,
    intervalList: Option<Bound<'py, PyAny>>,
    mode: Option<Bound<'py, PyAny>>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let Some(mode) = mode.filter(|mode| !mode.is_none()) else {
        return Ok(intervalList);
    };
    if intervalList.as_ref().is_some_and(|given| !given.is_none()) {
        return Err(PyTypeError::new_err(format!(
            "{class} received both 'intervalList' and its former name 'mode'; \
             pass only 'intervalList'."
        )));
    }
    crate::warn_deprecated(
        mode.py(),
        &format!(
            "{class}'s 'mode' keyword is deprecated and will be removed in v12; \
             use 'intervalList' instead."
        ),
    )?;
    Ok(Some(mode))
}

/// The intervals as Python objects.
fn interval_objects(steps: &[RsInterval]) -> Vec<Interval> {
    steps.iter().cloned().map(Interval::wrap).collect()
}

/// music21's `AbstractDiatonicScale`: a diatonic pattern in one of its
/// modes, named in any case.
#[pyclass(name = "AbstractDiatonicScale", module = "music21.scale", extends = AbstractScale, subclass)]
pub struct AbstractDiatonicScale {
    #[pyo3(get, set)]
    mode: Option<Py<PyAny>>,
    #[pyo3(get, set)]
    dominantDegree: i64,
    #[pyo3(get, set)]
    relativeMajorDegree: i64,
    #[pyo3(get, set)]
    relativeMinorDegree: i64,
}

impl AbstractDiatonicScale {
    /// The mode named, or music21's complaint.
    fn mode_named(
        mode: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<(&'static DiatonicMode, Option<String>)> {
        let Some(given) = mode.filter(|mode| !mode.is_none()) else {
            return Ok((&DIATONIC_MODES[0], None));
        };
        let refused = || -> PyResult<PyErr> {
            Ok(ScaleException::new_err(format!(
                "Cannot create a scale of the following mode: {}",
                given.repr()?
            )))
        };
        let Ok(name) = given.extract::<String>() else {
            return Err(refused()?);
        };
        let lower = name.to_lowercase();
        match DIATONIC_MODES
            .iter()
            .find(|mode| mode.names.contains(&lower.as_str()))
        {
            Some(found) => Ok((found, Some(lower))),
            None => Err(refused()?),
        }
    }

    /// The network of a mode, and the crate's name for its pattern.
    fn network_of(mode: &DiatonicMode) -> PyResult<(RsNetwork, Option<RsScaleType>)> {
        let mut steps = named(&DIATONIC_STEPS)?;
        steps.rotate_left(mode.rotation);
        let scale_type = RsScaleType::from_music21_name(&capitalized_scale(mode.names[0]));
        Ok((bidirected(&steps, Simplification::Exact), scale_type))
    }
}

/// `dorian` as the crate names its pattern: `DorianScale`.
fn capitalized_scale(mode: &str) -> String {
    let mut letters = mode.chars();
    let first = letters.next().map(|letter| letter.to_ascii_uppercase());
    format!("{}{}Scale", first.unwrap_or_default(), letters.as_str())
}

#[pymethods]
impl AbstractDiatonicScale {
    /// What it holds, shown to Python's collector.
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.mode)
    }

    fn __clear__(&mut self) {
        self.mode = None;
    }
    #[new]
    #[pyo3(signature = (mode = None, **_keywords))]
    fn new(
        py: Python<'_>,
        mode: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let (found, name) = Self::mode_named(mode)?;
        let (network, scale_type) = Self::network_of(found)?;
        let mut base = AbstractScale::standing(py, network, scale_type)?;
        base.set_tonic_degree(found.tonic);
        Ok(base.initializer().add_subclass(Self {
            mode: name.map(|name| PyString::new(py, &name).into_any().unbind()),
            dominantDegree: found.dominant,
            relativeMajorDegree: found.relative_major,
            relativeMinorDegree: found.relative_minor,
        }))
    }

    /// music21's `buildNetwork`: the network of a mode, named in any case.
    #[pyo3(signature = (mode = None))]
    fn buildNetwork(slf: &Bound<'_, Self>, mode: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let py = slf.py();
        let (found, name) = Self::mode_named(mode)?;
        let (network, scale_type) = Self::network_of(found)?;
        {
            let mut base = slf.as_super().borrow_mut();
            base.stand(py, network, scale_type)?;
            base.set_tonic_degree(found.tonic);
        }
        let mut me = slf.borrow_mut();
        me.mode = name.map(|name| PyString::new(py, &name).into_any().unbind());
        me.dominantDegree = found.dominant;
        me.relativeMajorDegree = found.relative_major;
        me.relativeMinorDegree = found.relative_minor;
        Ok(())
    }

    #[getter]
    fn r#type(&self) -> &'static str {
        "Abstract diatonic"
    }
}

/// music21's `AbstractOctatonicScale`: mode `2` (or `'M2'`, or none)
/// starts with a whole step, mode `1` (or `'m2'`) with a half step.
#[pyclass(name = "AbstractOctatonicScale", module = "music21.scale", extends = AbstractScale, subclass)]
pub struct AbstractOctatonicScale {
    #[pyo3(get, set)]
    mode: Option<Py<PyAny>>,
}

impl AbstractOctatonicScale {
    /// The network of a mode, whether it starts with the half step, and
    /// the crate's name for the pattern, which it has for the other.
    fn network_of(mode: Option<&Bound<'_, PyAny>>) -> PyResult<(RsNetwork, Option<RsScaleType>)> {
        let mut steps = named(&OCTATONIC_STEPS)?;
        let given = mode.filter(|mode| !mode.is_none());
        let is = |number: i64, name: &str| {
            given.is_some_and(|mode| {
                mode.extract::<i64>().is_ok_and(|said| said == number)
                    || mode.extract::<String>().is_ok_and(|said| said == name)
            })
        };
        let scale_type = if given.is_none() || is(2, "M2") {
            Some(RsScaleType::Octatonic)
        } else if is(1, "m2") {
            steps.rotate_left(1);
            None
        } else {
            let shown = given.map_or(Ok(String::new()), |mode| {
                mode.str().map(|said| said.to_string())
            })?;
            return Err(ScaleException::new_err(format!(
                "cannot create a scale of the following mode: {shown}"
            )));
        };
        Ok((
            bidirected(&steps, Simplification::MaxAccidental),
            scale_type,
        ))
    }
}

#[pymethods]
impl AbstractOctatonicScale {
    /// What it holds, shown to Python's collector.
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.mode)
    }

    fn __clear__(&mut self) {
        self.mode = None;
    }
    #[new]
    #[pyo3(signature = (mode = None, **_keywords))]
    fn new(
        py: Python<'_>,
        mode: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let (network, scale_type) = Self::network_of(mode)?;
        let numbered = mode.and_then(|mode| mode.extract::<i64>().ok());
        let mut base = AbstractScale::standing(py, network, scale_type)?;
        base.set_numbered_mode(numbered);
        Ok(base.initializer().add_subclass(Self {
            mode: mode.map(|mode| mode.clone().unbind()),
        }))
    }

    /// music21's `buildNetwork`: the network of a mode.
    #[pyo3(signature = (mode = None))]
    fn buildNetwork(slf: &Bound<'_, Self>, mode: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let (network, scale_type) = Self::network_of(mode)?;
        {
            let mut base = slf.as_super().borrow_mut();
            base.stand(slf.py(), network, scale_type)?;
            base.set_numbered_mode(mode.and_then(|mode| mode.extract::<i64>().ok()));
        }
        slf.borrow_mut().mode = mode.map(|mode| mode.clone().unbind());
        Ok(())
    }

    #[getter]
    fn r#type(&self) -> &'static str {
        "Abstract Octatonic"
    }
}

/// One of music21's abstract scales whose network is fixed, which takes
/// no mode: the two minors, the rags and the weighted blues.
macro_rules! fixed_pattern {
    ($class:ident, $name:literal, $kind:literal, $network:expr, $scale_type:expr) => {
        #[doc = concat!("music21's `", $name, "`.")]
        #[pyclass(name = $name, module = "music21.scale", extends = AbstractScale, subclass)]
        pub struct $class {
            #[pyo3(get, set)]
            dominantDegree: i64,
        }

        impl $class {
            fn network() -> PyResult<RsNetwork> {
                $network
            }
        }

        #[pymethods]
        impl $class {
            #[new]
            #[pyo3(signature = (*_arguments, **_keywords))]
            fn new(
                py: Python<'_>,
                _arguments: &Bound<'_, PyTuple>,
                _keywords: Option<&Bound<'_, PyDict>>,
            ) -> PyResult<PyClassInitializer<Self>> {
                let base = AbstractScale::standing(py, Self::network()?, $scale_type)?;
                Ok(base.initializer().add_subclass(Self {
                    dominantDegree: DOMINANT,
                }))
            }

            /// music21's `buildNetwork`, which takes no mode here.
            #[pyo3(signature = (mode = None))]
            fn buildNetwork(
                slf: &Bound<'_, Self>,
                mode: Option<&Bound<'_, PyAny>>,
            ) -> PyResult<()> {
                let _ = mode;
                let mut base = slf.as_super().borrow_mut();
                base.stand(slf.py(), Self::network()?, $scale_type)?;
                base.set_tonic_degree(1);
                Ok(())
            }

            #[getter]
            fn r#type(&self) -> &'static str {
                $kind
            }

            /// These take no mode.
            #[getter]
            fn mode(&self) -> Option<i64> {
                None
            }
        }
    };
}

fixed_pattern!(
    AbstractHarmonicMinorScale,
    "AbstractHarmonicMinorScale",
    "Abstract Harmonic Minor",
    Ok(bidirected(
        &named(&HARMONIC_MINOR_STEPS)?,
        Simplification::Exact
    )),
    Some(RsScaleType::HarmonicMinor)
);

fixed_pattern!(
    AbstractMelodicMinorScale,
    "AbstractMelodicMinorScale",
    "Abstract Melodic Minor",
    {
        let mut network = RsNetwork::default();
        network.fill_melodic_minor().map_err(scale_error)?;
        network.simplification = Simplification::Exact;
        Ok(network)
    },
    Some(RsScaleType::MelodicMinor)
);

fixed_pattern!(
    AbstractRagAsawari,
    "AbstractRagAsawari",
    "Abstract Rag Asawari",
    RsScaleType::RagAsawari.network().map_err(scale_error),
    Some(RsScaleType::RagAsawari)
);

fixed_pattern!(
    AbstractRagMarwa,
    "AbstractRagMarwa",
    "Abstract Rag Marwa",
    RsScaleType::RagMarwa.network().map_err(scale_error),
    Some(RsScaleType::RagMarwa)
);

fixed_pattern!(
    AbstractWeightedHexatonicBlues,
    "AbstractWeightedHexatonicBlues",
    "Abstract Weighted Hexatonic Blues",
    weighted_hexatonic_blues().map_err(scale_error),
    None
);

/// music21's `AbstractCyclicalScale`: any steps, repeating at the octave
/// only where whole semitones divide it.
#[pyclass(name = "AbstractCyclicalScale", module = "music21.scale", extends = AbstractScale, subclass)]
pub struct AbstractCyclicalScale {
    steps: Vec<RsInterval>,
}

#[pymethods]
impl AbstractCyclicalScale {
    #[new]
    #[pyo3(signature = (intervalList = None, *, mode = None, **_keywords))]
    fn new(
        py: Python<'_>,
        intervalList: Option<Bound<'_, PyAny>>,
        mode: Option<Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let given = list_or_mode("AbstractCyclicalScale", intervalList, mode)?;
        let steps = to_intervals(given.as_ref())?;
        let network = cyclical(&steps).map_err(scale_error)?;
        Ok(AbstractScale::standing(py, network, None)?
            .initializer()
            .add_subclass(Self { steps }))
    }

    /// music21's `buildNetwork`: the network of these steps, kept as
    /// `intervalList`.
    #[pyo3(signature = (mode = None))]
    fn buildNetwork(slf: &Bound<'_, Self>, mode: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let steps = to_intervals(mode)?;
        let network = cyclical(&steps).map_err(scale_error)?;
        slf.as_super().borrow_mut().stand(slf.py(), network, None)?;
        slf.borrow_mut().steps = steps;
        Ok(())
    }

    /// music21's `intervalList`: the steps as given.
    #[getter]
    fn intervalList(&self) -> Vec<Interval> {
        interval_objects(&self.steps)
    }

    #[getter]
    fn r#type(&self) -> &'static str {
        "Abstract Cyclical"
    }

    #[getter]
    fn mode(&self) -> Option<i64> {
        None
    }
}

/// music21's `AbstractOctaveRepeatingScale`: steps completed to the
/// octave by the interval left over.
#[pyclass(name = "AbstractOctaveRepeatingScale", module = "music21.scale", extends = AbstractScale, subclass)]
pub struct AbstractOctaveRepeatingScale {
    steps: Vec<RsInterval>,
}

impl AbstractOctaveRepeatingScale {
    /// The completed steps and their network; no steps is an octave.
    fn built(given: Option<&Bound<'_, PyAny>>) -> PyResult<(Vec<RsInterval>, RsNetwork)> {
        octave_repeating(&to_intervals(given)?).map_err(scale_error)
    }
}

#[pymethods]
impl AbstractOctaveRepeatingScale {
    #[new]
    #[pyo3(signature = (intervalList = None, *, mode = None, **_keywords))]
    fn new(
        py: Python<'_>,
        intervalList: Option<Bound<'_, PyAny>>,
        mode: Option<Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let given = list_or_mode("AbstractOctaveRepeatingScale", intervalList, mode)?;
        let given = match given.filter(|given| !given.is_none()) {
            Some(given) => given,
            None => PyString::new(py, "P8").into_any(),
        };
        let (steps, network) = Self::built(Some(&given))?;
        Ok(AbstractScale::standing(py, network, None)?
            .initializer()
            .add_subclass(Self { steps }))
    }

    /// music21's `buildNetwork`: these steps completed to the octave.
    #[pyo3(signature = (mode = None))]
    fn buildNetwork(slf: &Bound<'_, Self>, mode: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let (steps, network) = Self::built(mode)?;
        slf.as_super().borrow_mut().stand(slf.py(), network, None)?;
        slf.borrow_mut().steps = steps;
        Ok(())
    }

    /// music21's `intervalList`: the steps with the one completing the
    /// octave.
    #[getter]
    fn intervalList(&self) -> Vec<Interval> {
        interval_objects(&self.steps)
    }

    #[getter]
    fn r#type(&self) -> &'static str {
        "Abstract Octave Repeating"
    }

    #[getter]
    fn mode(&self) -> Option<i64> {
        None
    }
}

/// music21's `WeightedHexatonicBlues`: the minor pentatonic that takes the
/// blue note between its fourth and fifth degrees at random.
#[pyclass(name = "WeightedHexatonicBlues", module = "music21.scale", extends = ConcreteScale, subclass)]
pub struct WeightedHexatonicBlues;

#[pymethods]
impl WeightedHexatonicBlues {
    #[new]
    #[pyo3(signature = (tonic = None, *arguments, **keywords))]
    fn new(
        tonic: Option<&Bound<'_, PyAny>>,
        arguments: &Bound<'_, PyTuple>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(ConcreteScale::initializer(tonic, arguments, keywords)?.add_subclass(Self))
    }

    /// Settles the tonic as any concrete scale does, then stands it on the
    /// blues network.
    #[pyo3(signature = (tonic = None, **_keywords))]
    fn __init__(
        slf: &Bound<'_, Self>,
        tonic: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let py = slf.py();
        let concrete = slf.as_super();
        ConcreteScale::settle(concrete, tonic)?;
        let pattern = py.get_type::<AbstractWeightedHexatonicBlues>().call0()?;
        ConcreteScale::stand_on(concrete, &pattern, "Weighted Hexatonic Blues");
        Ok(())
    }
}

/// music21's `_closeOnTonic`: a list of steps spanning twelve semitones
/// with the last respelled so that they reach the octave exactly; the
/// same list where they already do.
#[pyfunction]
#[pyo3(name = "_closeOnTonic")]
fn close_on_tonic_py<'py>(mode: &Bound<'py, PyList>) -> PyResult<Bound<'py, PyList>> {
    let py = mode.py();
    let steps = mode
        .iter()
        .map(|step| interval_from_any(&step))
        .collect::<PyResult<Vec<_>>>()?;
    let closed = close_on_tonic(&steps).map_err(scale_error)?;
    if closed == steps {
        return Ok(mode.clone());
    }
    let Some(last) = closed.last() else {
        return Ok(mode.clone());
    };
    let kept = mode.get_slice(0, mode.len().saturating_sub(1));
    kept.append(crate::interval::interval_object(py, last.clone())?)?;
    Ok(kept)
}

/// music21's `_toIntervals`: intervals kept as they are and names made
/// intervals; one of either is a list of one, and `None` an empty list.
#[pyfunction]
#[pyo3(name = "_toIntervals")]
fn to_intervals_py<'py>(
    py: Python<'py>,
    intervals: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyList>> {
    let made = PyList::empty(py);
    let Some(intervals) = intervals.filter(|given| !given.is_none()) else {
        return Ok(made);
    };
    let single = intervals.is_instance_of::<PyString>()
        || intervals.extract::<PyRef<'_, Interval>>().is_ok();
    let given: Vec<Bound<'py, PyAny>> = if single {
        vec![intervals.clone()]
    } else {
        intervals.walk()?.collect::<PyResult<_>>()?
    };
    for one in given {
        if one.extract::<PyRef<'_, Interval>>().is_ok() {
            made.append(one)?;
        } else {
            made.append(crate::interval::interval_object(
                py,
                interval_from_any(&one)?,
            )?)?;
        }
    }
    Ok(made)
}

/// music21's `_intervalListOrMode`, for a scale of any class.
#[pyfunction]
#[pyo3(name = "_intervalListOrMode")]
fn interval_list_or_mode<'py>(
    abstractScale: &Bound<'py, PyAny>,
    intervalList: Option<Bound<'py, PyAny>>,
    mode: Option<Bound<'py, PyAny>>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let class = abstractScale.get_type().name()?.to_string();
    list_or_mode(&class, intervalList, mode)
}

/// Registers the classes on the wheel's module.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<AbstractDiatonicScale>()?;
    m.add_class::<AbstractOctatonicScale>()?;
    m.add_class::<AbstractHarmonicMinorScale>()?;
    m.add_class::<AbstractMelodicMinorScale>()?;
    m.add_class::<AbstractCyclicalScale>()?;
    m.add_class::<AbstractOctaveRepeatingScale>()?;
    m.add_class::<AbstractRagAsawari>()?;
    m.add_class::<AbstractRagMarwa>()?;
    m.add_class::<AbstractWeightedHexatonicBlues>()?;
    m.add_class::<WeightedHexatonicBlues>()?;
    m.add_function(wrap_pyfunction!(close_on_tonic_py, m)?)?;
    m.add_function(wrap_pyfunction!(to_intervals_py, m)?)?;
    m.add_function(wrap_pyfunction!(interval_list_or_mode, m)?)?;
    Ok(())
}
