//! music21's `scale` module over `music21-rs`: the concrete scales, with
//! music21's names, properties and `repr`.
//!
//! music21 splits a scale in two — an `AbstractScale`, which is the pattern
//! of steps, and a `ConcreteScale`, which is that pattern standing on a
//! tonic. The crate's [`music21_rs::scale::Scale`] is the second, and its
//! `ScaleType` is the first; the twenty concrete classes music21 names are
//! built here from that one pair rather than written out twenty times.

#![allow(non_snake_case)]

use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple, PyType};

use music21_rs::scale::{
    Scale as RsScale, ScaleType as RsScaleType, SolfegVariant as RsSolfegVariant,
};
use music21_rs::{Interval as RsInterval, Pitch as RsPitch};

use crate::interval::interval_from_any;
use crate::pitch::{Pitch, message, pitch_from_any};

/// The names the `scale` facade replaces in `music21.scale`.
///
/// The twenty concrete classes are named here rather than derived from
/// `ScaleType::ALL` because this has to be a constant, and a test below
/// keeps the two lists in step.
pub const NAMES: &[&str] = &[
    "Scale",
    "ConcreteScale",
    "DiatonicScale",
    "ScaleException",
    "MajorScale",
    "MinorScale",
    "DorianScale",
    "PhrygianScale",
    "LydianScale",
    "MixolydianScale",
    "LocrianScale",
    "HypodorianScale",
    "HypophrygianScale",
    "HypolydianScale",
    "HypomixolydianScale",
    "HypolocrianScale",
    "HypoaeolianScale",
    "HarmonicMinorScale",
    "MelodicMinorScale",
    "ChromaticScale",
    "WholeToneScale",
    "OctatonicScale",
    "RagAsawari",
    "RagMarwa",
];

pyo3::create_exception!(music21_rs_facade, ScaleException, PyException);

fn scale_error(error: music21_rs::Error) -> PyErr {
    ScaleException::new_err(message(&error))
}

/// music21's `scale.Scale`: the root of the hierarchy, which says only that
/// something is a scale and whether it stands on a tonic.
#[pyclass(
    name = "Scale",
    module = "music21.scale",
    subclass,
    skip_from_py_object
)]
pub struct Scale;

#[pymethods]
impl Scale {
    #[new]
    #[pyo3(signature = (**_keywords))]
    fn new(_keywords: Option<&Bound<'_, PyDict>>) -> Self {
        Self
    }

    /// music21's `isConcrete`: whether the scale stands on a tonic. A bare
    /// `Scale` does not.
    #[getter]
    fn isConcrete(&self) -> bool {
        false
    }

    #[getter]
    fn name(&self) -> &'static str {
        "Scale"
    }

    /// music21's `extractPitchList`: the pitches out of anything that has
    /// them — a list of pitches or names, or notes, or a stream of notes.
    #[staticmethod]
    #[pyo3(signature = (other, comparisonAttribute = "nameWithOctave", removeDuplicates = true))]
    fn extractPitchList(
        other: &Bound<'_, PyAny>,
        comparisonAttribute: &str,
        removeDuplicates: bool,
    ) -> PyResult<Vec<Pitch>> {
        let mut pitches = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        for pitch in pitch_list(other)? {
            if removeDuplicates {
                let key = match comparisonAttribute {
                    "name" => pitch.name(),
                    "pitchClass" => pitch.pitch_class().number().to_string(),
                    _ => pitch.name_with_octave(),
                };
                if seen.contains(&key) {
                    continue;
                }
                seen.push(key);
            }
            pitches.push(Pitch::wrap(pitch, false));
        }
        Ok(pitches)
    }
}

/// music21's `scale.ConcreteScale`: a pattern of steps standing on a tonic.
///
/// Every one of music21's twenty named scales is this with the pattern fixed,
/// which is how they are built at registration — a Python subclass carrying
/// the crate's `ScaleType` under `scaleTypeName`, exactly as music21's own
/// subclasses carry an `AbstractScale`.
#[pyclass(
    name = "ConcreteScale",
    module = "music21.scale",
    extends = Scale,
    subclass,
    skip_from_py_object
)]
pub struct ConcreteScale {
    pub(crate) inner: RsScale,
}

impl ConcreteScale {
    pub(crate) fn of(scale_type: RsScaleType, tonic: RsPitch) -> Self {
        Self {
            inner: RsScale::new(scale_type, tonic),
        }
    }

    /// A Python object of the class music21 names this scale type, when one
    /// has been registered, and a plain `ConcreteScale` otherwise.
    pub(crate) fn object(py: Python<'_>, scale: RsScale) -> PyResult<Py<PyAny>> {
        let name = scale.scale_type().music21_name();
        let module = py
            .import("music21_rs_facade")
            .or_else(|_| py.import("music21_rs"))?;
        if let Ok(class) = module.getattr(name)
            && let Ok(class) = class.cast_into::<PyType>()
        {
            let built = class.call1((Pitch::wrap(scale.tonic().clone(), false),))?;
            return Ok(built.unbind());
        }
        Ok(Py::new(
            py,
            PyClassInitializer::from(Scale).add_subclass(Self { inner: scale }),
        )?
        .into_any())
    }

    /// The scale type a subclass stands for, read off the class the way
    /// music21 reads its `_abstract` off its own.
    fn scale_type_of(class: &Bound<'_, PyType>) -> PyResult<RsScaleType> {
        let Ok(name) = class.getattr("scaleTypeName") else {
            return Ok(RsScaleType::Major);
        };
        let name: String = name.extract()?;
        RsScaleType::from_music21_name(&name)
            .ok_or_else(|| ScaleException::new_err(format!("no such scale type: {name}")))
    }
}

#[pymethods]
impl ConcreteScale {
    /// music21's `ConcreteScale(tonic)`, where the pattern of steps comes
    /// from the class and only the tonic is given. No tonic means C, as it
    /// does upstream.
    #[new]
    #[pyo3(signature = (tonic = None, **_keywords))]
    fn new(
        tonic: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let tonic = match tonic.filter(|value| !value.is_none()) {
            Some(value) => pitch_from_any(value)?,
            None => RsPitch::from_name("C").map_err(scale_error)?,
        };
        Ok(PyClassInitializer::from(Scale).add_subclass(Self::of(RsScaleType::Major, tonic)))
    }

    /// The pattern of steps is only known once the class is, which is why it
    /// is settled here rather than in `__new__`: a Python subclass hands
    /// `__new__` its own arguments, and only `__init__` is told what class
    /// it is building.
    #[pyo3(signature = (tonic = None, **_keywords))]
    fn __init__(
        slf: &Bound<'_, Self>,
        tonic: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let scale_type = Self::scale_type_of(&slf.as_any().get_type())?;
        let tonic = match tonic.filter(|value| !value.is_none()) {
            Some(value) => pitch_from_any(value)?,
            None => RsPitch::from_name("C").map_err(scale_error)?,
        };
        slf.borrow_mut().inner = RsScale::new(scale_type, tonic);
        Ok(())
    }

    #[getter]
    fn isConcrete(&self) -> bool {
        true
    }

    /// music21's `name`: the tonic and the pattern, `"D major"`.
    #[getter]
    fn name(&self) -> String {
        format!(
            "{} {}",
            self.inner.tonic().name(),
            self.inner.scale_type().music21_descriptive_name()
        )
    }

    #[getter]
    fn get_tonic(&self) -> Pitch {
        Pitch::wrap(self.inner.tonic().clone(), false)
    }

    #[setter]
    fn set_tonic(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.inner = RsScale::new(self.inner.scale_type(), pitch_from_any(value)?);
        Ok(())
    }

    /// music21's `pitches`: the scale from its tonic through the octave, the
    /// closing octave included.
    #[getter]
    fn pitches(&self) -> PyResult<Vec<Pitch>> {
        Ok(self
            .inner
            .pitches()
            .map_err(scale_error)?
            .into_iter()
            .map(|pitch| Pitch::wrap(pitch, false))
            .collect())
    }

    /// music21's `getPitches`: the scale over a range of pitches, or over
    /// the octave above the tonic when no range is given.
    #[pyo3(signature = (minPitch = None, maxPitch = None, **_keywords))]
    fn getPitches(
        &self,
        minPitch: Option<&Bound<'_, PyAny>>,
        maxPitch: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<Pitch>> {
        let (Some(low), Some(high)) = (
            minPitch.filter(|value| !value.is_none()),
            maxPitch.filter(|value| !value.is_none()),
        ) else {
            return self.pitches();
        };
        Ok(wrap_pitches(
            self.inner
                .pitches_between(&pitch_from_any(low)?, &pitch_from_any(high)?)
                .map_err(scale_error)?,
        ))
    }

    /// music21's `getTonic`: the note the scale comes to rest on, as it
    /// sounds — so a scale built on a bare `C` answers `C4`, and a plagal
    /// mode answers its final rather than the bottom of its range.
    fn getTonic(&self) -> PyResult<Pitch> {
        self.inner
            .final_pitch()
            .map(|pitch| Pitch::wrap(pitch, false))
            .map_err(scale_error)
    }

    /// music21's `getDominant`: the reciting tone.
    fn getDominant(&self) -> PyResult<Pitch> {
        self.inner
            .dominant()
            .map(|pitch| Pitch::wrap(pitch, false))
            .map_err(scale_error)
    }

    /// music21's `getLeadingTone`: the seventh degree raised or lowered to
    /// sit a semitone below the final, which in a minor scale is not the
    /// seventh degree the scale itself has.
    fn getLeadingTone(&self) -> PyResult<Pitch> {
        self.inner
            .leading_tone()
            .map(|pitch| Pitch::wrap(pitch, false))
            .map_err(scale_error)
    }

    /// music21's `abstract`: the pattern of steps this scale stands on,
    /// which here is the scale type it was built with.
    #[getter]
    fn get_abstract(&self) -> String {
        self.inner.scale_type().music21_name().to_string()
    }

    /// music21's `deriveRanked`: the scales of this pattern containing the
    /// most of the given pitches, best first, each with how many it matched.
    #[pyo3(signature = (other, **_keywords))]
    fn deriveRanked(
        slf: &Bound<'_, Self>,
        other: &Bound<'_, PyAny>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<(usize, Py<PyAny>)>> {
        let pitches = pitch_list(other)?;
        let ranked = slf
            .borrow()
            .inner
            .scale_type()
            .derive_ranked(&pitches, None)
            .map_err(scale_error)?;
        ranked
            .into_iter()
            .map(|(matched, scale)| Ok((matched, Self::object(slf.py(), scale)?)))
            .collect()
    }

    /// music21's `derive`: the best-ranked of those.
    #[pyo3(signature = (other, **_keywords))]
    fn derive(
        slf: &Bound<'_, Self>,
        other: &Bound<'_, PyAny>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        let pitches = pitch_list(other)?;
        let derived = slf
            .borrow()
            .inner
            .scale_type()
            .derive(&pitches)
            .map_err(scale_error)?;
        Self::object(slf.py(), derived)
    }

    /// music21's `deriveAll`: every scale of this pattern that contains all
    /// of the given pitches.
    #[pyo3(signature = (other, **_keywords))]
    fn deriveAll(
        slf: &Bound<'_, Self>,
        other: &Bound<'_, PyAny>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let pitches = pitch_list(other)?;
        let derived = slf
            .borrow()
            .inner
            .scale_type()
            .derive_all(&pitches)
            .map_err(scale_error)?;
        derived
            .into_iter()
            .map(|scale| Self::object(slf.py(), scale))
            .collect()
    }

    /// music21's `solfeg`: the syllable for a pitch's degree in this scale.
    #[pyo3(signature = (pitchTarget = None, *, variant = "music21", chromatic = true, **_keywords))]
    fn solfeg(
        &self,
        pitchTarget: Option<&Bound<'_, PyAny>>,
        variant: &str,
        chromatic: bool,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<String> {
        let pitch = match pitchTarget.filter(|value| !value.is_none()) {
            Some(value) => pitch_from_any(value)?,
            None => self.inner.tonic().clone(),
        };
        let variant = match variant {
            "humdrum" => RsSolfegVariant::Humdrum,
            _ => RsSolfegVariant::Music21,
        };
        self.inner
            .solfeg(&pitch, variant, chromatic)
            .map_err(scale_error)
    }

    /// music21's `pitchesFromScaleDegrees`.
    #[pyo3(signature = (degreeTargets, minPitch = None, maxPitch = None, **_keywords))]
    fn pitchesFromScaleDegrees(
        &self,
        degreeTargets: Vec<usize>,
        minPitch: Option<&Bound<'_, PyAny>>,
        maxPitch: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<Pitch>> {
        let _ = (minPitch, maxPitch);
        Ok(wrap_pitches(
            self.inner
                .pitches_from_scale_degrees(&degreeTargets)
                .map_err(scale_error)?,
        ))
    }

    /// music21's `isNext`: whether one pitch is so many degrees above another
    /// in this scale.
    #[pyo3(signature = (other, pitchOrigin, direction = None, stepSize = 1, **_keywords))]
    fn isNext(
        &self,
        other: &Bound<'_, PyAny>,
        pitchOrigin: &Bound<'_, PyAny>,
        direction: Option<&Bound<'_, PyAny>>,
        stepSize: usize,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<bool> {
        let _ = direction;
        self.inner
            .is_next(
                &pitch_from_any(other)?,
                &pitch_from_any(pitchOrigin)?,
                stepSize,
            )
            .map_err(scale_error)
    }

    /// music21's `pitchFromDegree`: the pitch at a scale degree, counting the
    /// tonic as one.
    fn pitchFromDegree(&self, degree: usize) -> PyResult<Pitch> {
        self.inner
            .pitch_at_degree(degree)
            .map(|pitch| Pitch::wrap(pitch, false))
            .map_err(scale_error)
    }

    /// music21's `getScaleDegreeFromPitch`: which degree a pitch is, or
    /// nothing when it is not in the scale.
    #[pyo3(signature = (pitchTarget, comparisonAttribute = "name", **_keywords))]
    fn getScaleDegreeFromPitch(
        &self,
        pitchTarget: &Bound<'_, PyAny>,
        comparisonAttribute: &str,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Option<usize>> {
        let pitch = pitch_from_any(pitchTarget)?;
        let degree = match comparisonAttribute {
            "pitchClass" => self.inner.degree_of_pitch_class(&pitch),
            _ => self.inner.degree_of(&pitch),
        };
        degree.map_err(scale_error)
    }

    /// music21's `getScaleDegreeAndAccidentalFromPitch`: the degree and how
    /// far the pitch is altered from it.
    #[pyo3(signature = (pitchTarget, **_keywords))]
    fn getScaleDegreeAndAccidentalFromPitch(
        &self,
        py: Python<'_>,
        pitchTarget: &Bound<'_, PyAny>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        let pitch = pitch_from_any(pitchTarget)?;
        let (degree, accidental) = self
            .inner
            .degree_and_accidental_of(&pitch)
            .map_err(scale_error)?;
        let accidental = accidental.map(crate::pitch::Accidental::from_inner);
        Ok(PyTuple::new(py, [degree.into_pyobject(py)?.into_any()])?
            .as_any()
            .add(PyTuple::new(
                py,
                [match accidental {
                    Some(accidental) => accidental.into_pyobject(py)?.into_any(),
                    None => py.None().into_bound(py),
                }],
            )?)?
            .unbind())
    }

    /// music21's `nextPitch`: the pitch so many steps along the scale.
    #[pyo3(signature = (pitchOrigin = None, direction = None, stepSize = 1, **_keywords))]
    fn nextPitch(
        &self,
        pitchOrigin: Option<&Bound<'_, PyAny>>,
        direction: Option<&Bound<'_, PyAny>>,
        stepSize: usize,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Pitch> {
        let origin = match pitchOrigin.filter(|value| !value.is_none()) {
            Some(value) => pitch_from_any(value)?,
            None => self.inner.tonic().clone(),
        };
        let descending = direction.is_some_and(|direction| {
            direction
                .str()
                .map(|name| name.to_string_lossy().to_lowercase().contains("descending"))
                .unwrap_or(false)
                || direction.extract::<i32>().is_ok_and(|value| value < 0)
        });
        let moved = if descending {
            self.inner.next_pitch_below(&origin, stepSize)
        } else {
            self.inner.next_pitch_above(&origin, stepSize)
        };
        moved
            .map(|pitch| Pitch::wrap(pitch, false))
            .map_err(scale_error)
    }

    /// music21's `transpose`: the same pattern on a tonic moved by an
    /// interval.
    #[pyo3(signature = (value, *, inPlace = false))]
    fn transpose(
        slf: &Bound<'_, Self>,
        value: &Bound<'_, PyAny>,
        inPlace: bool,
    ) -> PyResult<Option<Py<PyAny>>> {
        let interval: RsInterval = interval_from_any(value)?;
        let moved = slf
            .borrow()
            .inner
            .transpose(&interval)
            .map_err(scale_error)?;
        if inPlace {
            slf.borrow_mut().inner = moved;
            return Ok(None);
        }
        Ok(Some(Self::object(slf.py(), moved)?))
    }

    /// music21's `deriveByDegree`: the scale of this pattern that has the
    /// given pitch at the given degree.
    fn deriveByDegree(
        slf: &Bound<'_, Self>,
        degree: usize,
        pitchRef: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let pitch = pitch_from_any(pitchRef)?;
        let derived = slf
            .borrow()
            .inner
            .derive_by_degree(degree, &pitch)
            .map_err(scale_error)?;
        Self::object(slf.py(), derived)
    }

    /// music21's `match`: which of the given pitches are in the scale and
    /// which are not.
    #[pyo3(name = "match", signature = (other, **_keywords))]
    fn match_pitches(
        &self,
        py: Python<'_>,
        other: &Bound<'_, PyAny>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyDict>> {
        let pitches = pitch_list(other)?;
        let (matched, unmatched) = self.inner.match_pitches(&pitches).map_err(scale_error)?;
        let answer = PyDict::new(py);
        answer.set_item("matched", wrap_pitches(matched))?;
        answer.set_item("notMatched", wrap_pitches(unmatched))?;
        Ok(answer.unbind())
    }

    /// music21's `findMissing`: the scale's pitches that are not among the
    /// ones given.
    #[pyo3(signature = (other, **_keywords))]
    fn findMissing(
        &self,
        other: &Bound<'_, PyAny>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<Pitch>> {
        let pitches = pitch_list(other)?;
        Ok(wrap_pitches(
            self.inner.find_missing(&pitches).map_err(scale_error)?,
        ))
    }

    /// music21's `intervalBetweenDegrees`.
    fn intervalBetweenDegrees(
        &self,
        degreeStart: usize,
        degreeEnd: usize,
    ) -> PyResult<crate::interval::Interval> {
        self.inner
            .interval_between_degrees(degreeStart, degreeEnd)
            .map(crate::interval::Interval::wrap)
            .map_err(scale_error)
    }

    /// music21's `getDegreeMaxUnique`: how many degrees the scale has before
    /// it repeats.
    fn getDegreeMaxUnique(&self) -> usize {
        self.inner.degree_count()
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        Ok(format!(
            "<music21.scale.{} {}>",
            slf.as_any().get_type().qualname()?,
            slf.borrow().name()
        ))
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other.extract::<PyRef<'_, Self>>().is_ok_and(|other| {
            other.inner.scale_type() == self.inner.scale_type()
                && other.inner.tonic().name_with_octave() == self.inner.tonic().name_with_octave()
        })
    }
}

/// music21's `scale.DiatonicScale`: the seven-note scales, which is what
/// every mode and both minors are.
#[pyclass(
    name = "DiatonicScale",
    module = "music21.scale",
    extends = ConcreteScale,
    subclass,
    skip_from_py_object
)]
pub struct DiatonicScale;

#[pymethods]
impl DiatonicScale {
    #[new]
    #[pyo3(signature = (tonic = None, **keywords))]
    fn new(
        tonic: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(ConcreteScale::new(tonic, keywords)?.add_subclass(Self))
    }

    /// music21's `getRelativeMajor`: the major scale written with the same
    /// key signature, so D dorian answers C major.
    fn getRelativeMajor(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let relative = slf
            .as_super()
            .borrow()
            .inner
            .relative_major()
            .map_err(scale_error)?;
        ConcreteScale::object(slf.py(), relative)
    }

    /// music21's `getRelativeMinor`.
    fn getRelativeMinor(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let relative = slf
            .as_super()
            .borrow()
            .inner
            .relative_minor()
            .map_err(scale_error)?;
        ConcreteScale::object(slf.py(), relative)
    }

    /// music21's `getParallelMajor`: the major scale on the same tonic.
    fn getParallelMajor(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let parallel = slf.as_super().borrow().inner.parallel_major();
        ConcreteScale::object(slf.py(), parallel)
    }

    /// music21's `getParallelMinor`.
    fn getParallelMinor(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let parallel = slf.as_super().borrow().inner.parallel_minor();
        ConcreteScale::object(slf.py(), parallel)
    }
}

/// Reads a sequence of pitches, or a stream of notes, as pitches.
fn pitch_list(value: &Bound<'_, PyAny>) -> PyResult<Vec<RsPitch>> {
    let mut pitches = Vec::new();
    for item in value.try_iter()? {
        let item = item?;
        match item.getattr("pitch") {
            Ok(pitch) if !pitch.is_none() => pitches.push(pitch_from_any(&pitch)?),
            _ => pitches.push(pitch_from_any(&item)?),
        }
    }
    Ok(pitches)
}

fn wrap_pitches(pitches: Vec<RsPitch>) -> Vec<Pitch> {
    pitches
        .into_iter()
        .map(|pitch| Pitch::wrap(pitch, false))
        .collect()
}

/// The Python source that builds music21's twenty concrete scale classes.
///
/// Each is `ConcreteScale` with the pattern of steps fixed, which is what
/// music21's own subclasses are — they differ only in the `AbstractScale`
/// they carry. Writing them as Rust types would be twenty near-identical
/// `#[pyclass]`es for no gain.
const SUBCLASSES: &str = r#"
def build(base, diatonic_base, names):
    built = {}
    for class_name, is_diatonic in names:
        parent = diatonic_base if is_diatonic else base
        built[class_name] = type(class_name, (parent,), {
            '__module__': 'music21.scale',
            '__qualname__': class_name,
            'scaleTypeName': class_name,
        })
    return built
"#;

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add_class::<Scale>()?;
    m.add_class::<ConcreteScale>()?;
    m.add_class::<DiatonicScale>()?;
    let exception = py.get_type::<ScaleException>();
    exception.setattr("__module__", "music21.scale")?;
    m.add("ScaleException", exception)?;

    let builder = PyModule::from_code(
        py,
        &std::ffi::CString::new(SUBCLASSES)?,
        c"music21_rs_scale_classes.py",
        c"music21_rs_scale_classes",
    )?;
    let names = PyTuple::new(
        py,
        RsScaleType::ALL
            .into_iter()
            .map(|scale_type| (scale_type.music21_name(), scale_type.degree_count() == 7)),
    )?;
    let built = builder
        .getattr("build")?
        .call1((
            m.getattr("ConcreteScale")?,
            m.getattr("DiatonicScale")?,
            names,
        ))?
        .cast_into::<PyDict>()?;
    for (name, class) in built.iter() {
        m.add(name.extract::<String>()?.as_str(), class)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scale_type_is_named_in_the_swap_list() {
        for scale_type in RsScaleType::ALL {
            let name = scale_type.music21_name();
            assert!(
                NAMES.contains(&name),
                "{name} is a scale type the facade builds but does not install"
            );
        }
    }
}
