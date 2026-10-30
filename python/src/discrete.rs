//! music21's `analysis.discrete`, which asks a stream one question and
//! answers it.
//!
//! `Ambitus` is the span of a stream's pitches, the five key weightings are
//! the key it is most likely in, and `MelodicIntervalDiversity` counts the
//! melodic intervals it uses. What each reads is the crate's; what each
//! hands back is the stream's own -- a span is the very pitch objects the
//! notes hold, and a counted interval is built on the notes it was measured
//! between. `Stream.analyze` is the short way to the first two, and reads a
//! method name as music21 reads it.
//!
//! **`process` is not here for `Ambitus` or for a key weighting.** It
//! answers a solution *and a colour*, and the colour comes from music21's
//! colour dictionaries -- a keyscape's hue per key, a ramp over the span of
//! a pitch range -- which are presentation and no part of this crate. Half
//! an answer would be worse than none, so those two are left out and said
//! so here. `MelodicIntervalDiversity.process` is here, its colour being
//! the one white string music21 has not worked out yet.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString};

use music21_rs_crate::analysis::{KeyProfile, estimate_key_of_stream, melodic_line_positions};

use crate::Walkable;

/// The names this module hands out.
pub const NAMES: &[&str] = &[
    "DiscreteAnalysisException",
    "Ambitus",
    "KeyWeightKeyAnalysis",
    "KrumhanslSchmuckler",
    "AardenEssen",
    "SimpleWeights",
    "BellmanBudge",
    "TemperleyKostkaPayne",
    "MelodicIntervalDiversity",
];

pyo3::create_exception!(
    music21_rs_facade,
    DiscreteAnalysisException,
    crate::Music21Exception
);

/// music21's `analysis.discrete.Ambitus`: the lowest pitch a stream sounds
/// and the highest.
#[pyclass(
    name = "Ambitus",
    module = "music21.analysis.discrete",
    subclass,
    skip_from_py_object
)]
pub struct Ambitus {
    reference: Option<Py<PyAny>>,
}

#[pymethods]
impl Ambitus {
    #[new]
    #[pyo3(signature = (referenceStream = None))]
    fn make(referenceStream: Option<&Bound<'_, PyAny>>) -> Self {
        Self {
            reference: referenceStream
                .filter(|given| !given.is_none())
                .map(|given| given.clone().unbind()),
        }
    }

    /// The name music21's class calls itself by.
    #[classattr]
    fn name() -> &'static str {
        "Ambitus Analysis"
    }

    /// The names a method name may ask for it by: music21's `identifiers`.
    #[classattr]
    fn identifiers() -> Vec<&'static str> {
        vec!["ambitus", "span"]
    }

    #[getter]
    fn get_referenceStream(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.reference.as_ref().map(|held| held.clone_ref(py))
    }

    #[setter]
    fn set_referenceStream(&mut self, value: Option<&Bound<'_, PyAny>>) {
        self.reference = value
            .filter(|given| !given.is_none())
            .map(|given| given.clone().unbind());
    }

    /// The two pitches a stream sounds lowest and highest, nothing where it
    /// sounds none: music21's `getPitchSpan`.
    ///
    /// They are the stream's own pitch objects, and a chord symbol is
    /// passed over, as music21 passes it over. Where several pitches share
    /// a place the first met is the one answered.
    fn getPitchSpan<'py>(
        &self,
        subStream: &Bound<'py, PyAny>,
    ) -> PyResult<Option<(Bound<'py, PyAny>, Bound<'py, PyAny>)>> {
        let py = subStream.py();
        let note = PyString::new(py, "GeneralNote").into_any();
        let symbol = PyString::new(py, "ChordSymbol").into_any();
        let mut lowest: Option<(f64, Bound<'py, PyAny>)> = None;
        let mut highest: Option<(f64, Bound<'py, PyAny>)> = None;
        for element in subStream.call_method0("recurse")?.walk()? {
            let element = element?;
            if element
                .getattr("isStream")
                .and_then(|flag| flag.is_truthy())
                .unwrap_or(false)
            {
                continue;
            }
            if !crate::stream::answers_to_class(&element, &note)?
                || crate::stream::answers_to_class(&element, &symbol)?
            {
                continue;
            }
            for pitch in element.getattr("pitches")?.walk()? {
                let pitch = pitch?;
                let ps = pitch.getattr("ps")?.extract::<f64>()?;
                if lowest.as_ref().is_none_or(|(known, _)| ps < *known) {
                    lowest = Some((ps, pitch.clone()));
                }
                if highest.as_ref().is_none_or(|(known, _)| ps > *known) {
                    highest = Some((ps, pitch));
                }
            }
        }
        Ok(match (lowest, highest) {
            (Some((_, low)), Some((_, high))) => Some((low, high)),
            _ => None,
        })
    }

    /// The span as an interval, nothing where the stream sounds no pitch:
    /// music21's `getSolution`.
    ///
    /// The interval is built on the two pitches the stream holds, as
    /// music21 builds it, so its `pitchStart` is one of them.
    fn getSolution<'py>(&self, sStream: &Bound<'py, PyAny>) -> PyResult<Option<Bound<'py, PyAny>>> {
        let py = sStream.py();
        let Some((low, high)) = self.getPitchSpan(sStream)? else {
            return Ok(None);
        };
        let said = PyDict::new(py);
        said.set_item("noteStart", low)?;
        said.set_item("noteEnd", high)?;
        Ok(Some(
            crate::class_of(py, "music21.interval", "Interval")?.call((), Some(&said))?,
        ))
    }
}

/// music21's `analysis.discrete.KeyWeightKeyAnalysis`: the key a stream is
/// most likely in, by correlating its duration-weighted pitch-class
/// distribution with a profile.
///
/// The base class weighs as Krumhansl-Schmuckler does, as music21's does,
/// and the five subclasses below are the five profiles.
#[pyclass(
    name = "KeyWeightKeyAnalysis",
    module = "music21.analysis.discrete",
    subclass,
    skip_from_py_object
)]
pub struct KeyWeightKeyAnalysis {
    profile: KeyProfile,
    reference: Option<Py<PyAny>>,
}

impl KeyWeightKeyAnalysis {
    /// The analysis weighing by one profile.
    fn of(profile: KeyProfile, referenceStream: Option<&Bound<'_, PyAny>>) -> Self {
        Self {
            profile,
            reference: referenceStream
                .filter(|given| !given.is_none())
                .map(|given| given.clone().unbind()),
        }
    }
}

#[pymethods]
impl KeyWeightKeyAnalysis {
    #[new]
    #[pyo3(signature = (referenceStream = None))]
    fn make(referenceStream: Option<&Bound<'_, PyAny>>) -> Self {
        Self::of(KeyProfile::KrumhanslSchmuckler, referenceStream)
    }

    /// The name music21's class calls itself by.
    #[classattr]
    fn name() -> &'static str {
        "KeyWeightKeyAnalysis Base Class"
    }

    /// The names a method name may ask for it by: music21's `identifiers`.
    #[classattr]
    fn identifiers() -> Vec<&'static str> {
        vec!["key.baseClass", "keyscape.baseClass"]
    }

    #[getter]
    fn get_referenceStream(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.reference.as_ref().map(|held| held.clone_ref(py))
    }

    #[setter]
    fn set_referenceStream(&mut self, value: Option<&Bound<'_, PyAny>>) {
        self.reference = value
            .filter(|given| !given.is_none())
            .map(|given| given.clone().unbind());
    }

    /// The twelve weights this analysis correlates against, from the tonic
    /// up: music21's `getWeights`, which takes `major` or `minor` in any
    /// case and refuses anything else.
    #[pyo3(signature = (weightType = "major".to_string()))]
    fn getWeights(&self, weightType: String) -> PyResult<Vec<f64>> {
        match weightType.to_lowercase().as_str() {
            "major" => Ok(self.profile.major_weights().to_vec()),
            "minor" => Ok(self.profile.minor_weights().to_vec()),
            other => Err(DiscreteAnalysisException::new_err(format!(
                "Weights must be major or minor, not {other}"
            ))),
        }
    }

    /// The key the stream is most likely in: music21's `getSolution`,
    /// carrying how well it fitted (`correlationCoefficient`) and the
    /// twenty-three keys that fitted less well
    /// (`alternateInterpretations`).
    fn getSolution<'py>(&self, sStream: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let py = sStream.py();
        let stream = crate::stream::crate_stream(sStream)?;
        let estimates = estimate_key_of_stream(self.profile, &stream).ok_or_else(|| {
            DiscreteAnalysisException::new_err("failed to get likely keys for Stream component")
        })?;
        crate::analysis::key_of(py, &estimates)
    }
}

/// The five profiles as the classes music21 names them, each the analysis
/// above weighing its own way.
macro_rules! weightings {
    ($(($class:ident, $spelled:literal, $name:literal, $profile:ident)),* $(,)?) => {
        $(
            #[doc = concat!("music21's `analysis.discrete.", $spelled, "`.")]
            #[pyclass(
                name = $spelled,
                module = "music21.analysis.discrete",
                extends = KeyWeightKeyAnalysis,
                subclass
            )]
            pub struct $class;

            #[pymethods]
            impl $class {
                #[new]
                #[pyo3(signature = (referenceStream = None))]
                fn make(
                    referenceStream: Option<&Bound<'_, PyAny>>,
                ) -> PyClassInitializer<Self> {
                    PyClassInitializer::from(KeyWeightKeyAnalysis::of(
                        KeyProfile::$profile,
                        referenceStream,
                    ))
                    .add_subclass($class)
                }

                /// The name music21's class calls itself by.
                #[classattr]
                fn name() -> String {
                    $name.to_string()
                }

                /// The names a method name may ask for it by.
                #[classattr]
                fn identifiers() -> Vec<String> {
                    KeyProfile::$profile
                        .identifiers()
                        .iter()
                        .map(|each| (*each).to_string())
                        .collect()
                }
            }
        )*
    };
}

weightings![
    (
        KrumhanslSchmuckler,
        "KrumhanslSchmuckler",
        "Krumhansl Schmuckler/Kessler Key Analysis",
        KrumhanslSchmuckler
    ),
    (
        AardenEssen,
        "AardenEssen",
        "Aarden Essen Key Analysis",
        AardenEssen
    ),
    (
        SimpleWeights,
        "SimpleWeights",
        "Simple Weight Key Analysis",
        SimpleWeights
    ),
    (
        BellmanBudge,
        "BellmanBudge",
        "Bellman Budge Key Analysis",
        BellmanBudge
    ),
    (
        TemperleyKostkaPayne,
        "TemperleyKostkaPayne",
        "Temperley Kostka Payne Key Analysis",
        TemperleyKostkaPayne
    ),
];

/// music21's `analysis.discrete.MelodicIntervalDiversity`: how many
/// different melodic intervals a stream uses.
#[pyclass(
    name = "MelodicIntervalDiversity",
    module = "music21.analysis.discrete",
    subclass,
    skip_from_py_object
)]
pub struct MelodicIntervalDiversity {
    reference: Option<Py<PyAny>>,
}

#[pymethods]
impl MelodicIntervalDiversity {
    #[new]
    #[pyo3(signature = (referenceStream = None))]
    fn make(referenceStream: Option<&Bound<'_, PyAny>>) -> Self {
        Self {
            reference: referenceStream
                .filter(|given| !given.is_none())
                .map(|given| given.clone().unbind()),
        }
    }

    /// The name music21's class calls itself by.
    #[classattr]
    fn name() -> &'static str {
        "Interval Diversity"
    }

    /// The names a method name may ask for it by: music21's `identifiers`.
    #[classattr]
    fn identifiers() -> Vec<&'static str> {
        vec!["interval.diversity", "intervals"]
    }

    #[getter]
    fn get_referenceStream(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.reference.as_ref().map(|held| held.clone_ref(py))
    }

    #[setter]
    fn set_referenceStream(&mut self, value: Option<&Bound<'_, PyAny>>) {
        self.reference = value
            .filter(|given| !given.is_none())
            .map(|given| given.clone().unbind());
    }

    /// The one colour music21 draws a count in, having not worked out a
    /// ramp for it: music21's `solutionToColor`.
    fn solutionToColor(&self, _solution: &Bound<'_, PyAny>) -> &'static str {
        "#ffffff"
    }

    /// Every melodic interval the stream uses, against the interval itself
    /// and how often it comes: music21's `countMelodicIntervals`.
    ///
    /// Each part-like stream is read on its own, else the stream as one
    /// line; a note continuing a tie is heard once, and chords and rests
    /// are passed over, which is the crate's `melodic_line_positions`.
    /// An interval is built on the two notes the stream holds, as music21
    /// builds it, so `found['M3'][0].noteStart` is one of them. A `found`
    /// given is counted into and handed back.
    #[pyo3(signature = (sStream, found = None, ignoreDirection = true, ignoreUnison = true))]
    fn countMelodicIntervals<'py>(
        &self,
        sStream: &Bound<'py, PyAny>,
        found: Option<&Bound<'py, PyDict>>,
        #[pyo3(from_py_with = crate::truthy)] ignoreDirection: bool,
        #[pyo3(from_py_with = crate::truthy)] ignoreUnison: bool,
    ) -> PyResult<Bound<'py, PyDict>> {
        let py = sStream.py();
        let counts = match found {
            Some(given) => given.clone(),
            None => PyDict::new(py),
        };
        let class = crate::class_of(py, "music21.interval", "Interval")?;
        let (stream, leaves) = crate::stream::read_with_leaves(sStream)?;
        for line in melodic_line_positions(&stream) {
            for pair in line.windows(2) {
                let (Some(first), Some(second)) = (leaves.get(pair[0]), leaves.get(pair[1])) else {
                    continue;
                };
                let mut interval = class.call1((first.bind(py), second.bind(py)))?;
                let semitones = interval
                    .getattr("chromatic")?
                    .getattr("semitones")?
                    .extract::<f64>()?;
                if ignoreUnison && semitones == 0.0 {
                    continue;
                }
                if ignoreDirection && semitones < 0.0 {
                    interval = interval.call_method0("reverse")?;
                }
                // music21 keys by the directed name, so a descending
                // interval counts on its own where direction counts.
                let name = interval.getattr("directedName")?;
                match counts.get_item(&name)? {
                    Some(known) => {
                        let count = known.get_item(1)?.extract::<usize>()?;
                        known.set_item(1, count + 1)?;
                    }
                    None => {
                        // music21 keeps the interval beside its count, as a
                        // list, and counts into that list.
                        let made = PyList::empty(py);
                        made.append(interval)?;
                        made.append(1usize)?;
                        counts.set_item(&name, made)?;
                    }
                }
            }
        }
        Ok(counts)
    }

    /// How many different intervals the stream uses, and the colour they
    /// are drawn in: music21's `process`.
    ///
    /// music21's own passes `ignoreDirection` where `found` goes, so its
    /// `process` raises for any line of two notes; this does what its
    /// docstring says.
    #[pyo3(signature = (sStream, ignoreDirection = true))]
    fn process<'py>(
        &self,
        sStream: &Bound<'py, PyAny>,
        #[pyo3(from_py_with = crate::truthy)] ignoreDirection: bool,
    ) -> PyResult<(usize, &'static str)> {
        let count = self
            .countMelodicIntervals(sStream, None, ignoreDirection, true)?
            .len();
        Ok((count, "#ffffff"))
    }

    /// How many different intervals the stream uses: music21's
    /// `getSolution`.
    fn getSolution(&self, sStream: &Bound<'_, PyAny>) -> PyResult<usize> {
        Ok(self.process(sStream, true)?.0)
    }
}

/// Adds the discrete-analysis classes to a module.
///
/// The exception is not among them: building one builds music21's own
/// `Music21Exception` and so imports music21, which this wheel is not to
/// do for a name nobody has asked for. `lib.rs`'s table hands it out on
/// first asking.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Ambitus>()?;
    m.add_class::<KeyWeightKeyAnalysis>()?;
    m.add_class::<KrumhanslSchmuckler>()?;
    m.add_class::<AardenEssen>()?;
    m.add_class::<SimpleWeights>()?;
    m.add_class::<BellmanBudge>()?;
    m.add_class::<TemperleyKostkaPayne>()?;
    m.add_class::<MelodicIntervalDiversity>()?;
    Ok(())
}
