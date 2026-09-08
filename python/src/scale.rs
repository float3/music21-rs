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
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple, PyType};

use music21_rs::scale::{
    DegreeComparison as RsDegreeComparison, Scale as RsScale, ScaleType as RsScaleType,
    SolfegVariant as RsSolfegVariant,
};
use music21_rs::{Interval as RsInterval, Pitch as RsPitch};

use crate::interval::interval_from_any;
use crate::pitch::{Pitch, message, pitch_from_any};

/// The names the `scale` facade replaces in `music21.scale`.
///
/// The twenty concrete classes are named here rather than derived from
/// `ScaleType::ALL` because this has to be a constant, and a test below
/// keeps the two lists in step.
///
/// `AbstractScale` is deliberately absent. A concrete scale hands one out
/// under `abstract`, so the class is registered and reachable — but music21
/// builds its own abstract scales out of interval networks, which this crate
/// does not model at all, so replacing the class would be claiming something
/// untrue.
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
// music21 raises this one from the interval network underneath a scale, and
// names it after that module rather than after `scale` itself.
pyo3::create_exception!(music21_rs_facade, IntervalNetworkException, PyException);

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

    /// music21's `type`: what kind of scale this is. A bare `Scale` is only
    /// a scale.
    #[getter]
    fn r#type(&self) -> &'static str {
        "Scale"
    }

    /// music21's `extractPitchList`: the pitches out of anything that has
    /// them — a scale, a chord or a stream, or a list of pitches, names or
    /// notes.
    ///
    /// Duplicates go by default, judged on whichever attribute is named, and
    /// what survives is given octave 4 if it had none. Keeping the list as
    /// given is what `removeDuplicates=False` is for, and that skips the
    /// octave too, exactly as upstream.
    #[staticmethod]
    #[pyo3(signature = (other, comparisonAttribute = "nameWithOctave", removeDuplicates = true))]
    fn extractPitchList(
        other: &Bound<'_, PyAny>,
        comparisonAttribute: &str,
        removeDuplicates: bool,
    ) -> PyResult<Vec<Pitch>> {
        let read = pitch_list(other)?;
        if !removeDuplicates {
            return Ok(wrap_pitches(read));
        }
        let mut pitches = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        for pitch in read {
            let key = match comparisonAttribute {
                "name" => pitch.name(),
                // A pitch's step is the letter its name starts with.
                "step" => pitch.name().chars().take(1).collect(),
                "pitchClass" => pitch.pitch_class().number().to_string(),
                _ => pitch.name_with_octave(),
            };
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            pitches.push(pitch);
        }
        // What survives is heard in octave 4 if it had no octave of its own.
        for pitch in &mut pitches {
            if pitch.octave().is_none() {
                pitch.set_octave(Some(4));
            }
        }
        Ok(wrap_pitches(pitches))
    }
}

/// music21's `scale.AbstractScale`: a pattern of steps with no note to
/// stand on.
///
/// music21 keeps the pattern and the tonic in separate objects, and a
/// concrete scale hands its pattern out under `abstract`. Here the pattern
/// is a [`RsScaleType`], so this is a thin thing over one — but it has to be
/// an object rather than a name, because music21's own docstrings ask it how
/// many degrees it has and whether it repeats at the octave.
#[pyclass(
    name = "AbstractScale",
    module = "music21.scale",
    extends = Scale,
    subclass,
    skip_from_py_object
)]
pub struct AbstractScale {
    scale_type: Option<RsScaleType>,
}

impl AbstractScale {
    fn object(py: Python<'_>, scale_type: Option<RsScaleType>) -> PyResult<Py<PyAny>> {
        Ok(Py::new(
            py,
            PyClassInitializer::from(Scale).add_subclass(Self { scale_type }),
        )?
        .into_any())
    }

    /// The pattern named by an abstract scale of any kind — one of these, or
    /// music21's own, which says which it is in its class name.
    pub(crate) fn scale_type_of(value: &Bound<'_, PyAny>) -> Option<RsScaleType> {
        if let Ok(ours) = value.extract::<PyRef<'_, Self>>() {
            return ours.scale_type;
        }
        if let Ok(name) = value.extract::<String>() {
            return named_scale_type(&name);
        }
        // music21 names the mode separately from the class for the diatonic
        // ones: `AbstractDiatonicScale('major')`.
        if let Ok(mode) = value.getattr("mode")
            && let Ok(mode) = mode.extract::<String>()
            && let Some(scale_type) = named_scale_type(&mode)
        {
            return Some(scale_type);
        }
        let name = value.get_type().name().ok()?.extract::<String>().ok()?;
        named_scale_type(&name)
    }
}

#[pymethods]
impl AbstractScale {
    #[new]
    #[pyo3(signature = (mode = None, **_keywords))]
    fn new(
        mode: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let scale_type = mode
            .filter(|value| !value.is_none())
            .and_then(|value| Self::scale_type_of(value));
        Ok(PyClassInitializer::from(Scale).add_subclass(Self { scale_type }))
    }

    /// music21's `getDegreeMaxUnique`: how many degrees the pattern has
    /// before it repeats.
    fn getDegreeMaxUnique(&self) -> usize {
        self.scale_type.map_or(0, RsScaleType::degree_count)
    }

    /// music21's `octaveDuplicating`: whether the pattern repeats at the
    /// octave, which every one of these does.
    #[getter]
    fn octaveDuplicating(&self) -> bool {
        true
    }

    #[getter]
    fn r#type(&self) -> String {
        match self.scale_type {
            Some(scale_type) => format!("Abstract {}", scale_type.music21_descriptive_name()),
            None => "Abstract".to_string(),
        }
    }

    #[getter]
    fn name(&self) -> String {
        self.r#type()
    }

    #[getter]
    fn mode(&self) -> Option<&'static str> {
        self.scale_type.map(RsScaleType::music21_descriptive_name)
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        Self::scale_type_of(other) == self.scale_type
    }
}

/// The scale type music21 calls by this name, whether it is written as a
/// class name (`MajorScale`, `AbstractHarmonicMinorScale`) or as a mode
/// (`major`, `harmonic minor`).
fn named_scale_type(name: &str) -> Option<RsScaleType> {
    let bare = name.strip_prefix("Abstract").unwrap_or(name);
    RsScaleType::from_music21_name(bare)
        .or_else(|| RsScaleType::from_music21_name(&format!("{bare}Scale")))
        .or_else(|| {
            RsScaleType::ALL.into_iter().find(|scale_type| {
                scale_type
                    .music21_descriptive_name()
                    .eq_ignore_ascii_case(bare)
            })
        })
}

/// The note a scale stands on.
///
/// music21 takes a pitch, a note or the name of one, and refuses anything
/// else by name — a number is not a pitch, however much it looks like a MIDI
/// value.
fn tonic_pitch(value: &Bound<'_, PyAny>) -> PyResult<RsPitch> {
    if value.extract::<f64>().is_ok() && value.extract::<String>().is_err() {
        let kind = value.get_type();
        return Err(PyValueError::new_err(format!(
            "Tonic must be a Pitch, Note, or str, not {kind}"
        )));
    }
    pitch_from_any(value)
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
    /// What music21 calls a scale of this shape when it has no pattern of
    /// its own: `Concrete` for a bare `ConcreteScale`, `diatonic` for a bare
    /// `DiatonicScale`.
    family: String,
    /// Whether the class this was built as fixes the pattern of steps.
    ///
    /// A bare `ConcreteScale` does not, which is why music21 calls its type
    /// `Concrete` while a `HarmonicMinorScale` with no tonic still knows it
    /// is a harmonic minor.
    named_pattern: bool,
    /// A name a caller wrote over the one the pattern gives.
    ///
    /// music21's `type` is a plain attribute, and `key.Key` writes its mode
    /// into it, so a caller may too.
    named_type: Option<String>,
    /// Whether a tonic was ever given.
    ///
    /// music21's `ConcreteScale` can be built without one, and then it is
    /// not concrete at all: it is a pattern of steps waiting for a note to
    /// stand on, which is what `isConcrete` and the `Abstract …` name say.
    /// Anything that needs an actual pitch raises until it has one.
    has_tonic: bool,
}

impl ConcreteScale {
    pub(crate) fn of(scale_type: RsScaleType, tonic: RsPitch) -> Self {
        Self {
            inner: RsScale::new(scale_type, tonic),
            family: "Concrete".to_string(),
            named_pattern: true,
            named_type: None,
            has_tonic: true,
        }
    }

    /// The scale as it sounds in the direction asked for — a melodic minor
    /// lets its raised degrees fall coming down.
    fn heard(&self, direction: Option<&Bound<'_, PyAny>>) -> PyResult<RsScale> {
        let scale = self.realized()?;
        Ok(if is_descending(direction) {
            scale.descending()
        } else {
            scale.clone()
        })
    }

    /// The scale as something that can actually be realized, or music21's
    /// complaint that it has no note to stand on.
    fn realized(&self) -> PyResult<&RsScale> {
        if !self.has_tonic {
            return Err(IntervalNetworkException::new_err(
                "pitchReference cannot be None",
            ));
        }
        Ok(&self.inner)
    }

    /// The same, but a scale nobody gave a tonic to sounds from middle C.
    ///
    /// music21 asks a scale with no tonic for its notes and gets them, since
    /// the pattern is a pattern whatever it stands on; only the questions
    /// that name a degree refuse.
    fn realized_anywhere(&self) -> &RsScale {
        &self.inner
    }

    /// A Python object of the class music21 names this scale type, when one
    /// has been registered, and a plain `ConcreteScale` otherwise.
    pub(crate) fn object(py: Python<'_>, scale: RsScale) -> PyResult<Py<PyAny>> {
        // A scale given by its notes has no class to be built as.
        if scale.is_custom() {
            let mut built = Self {
                inner: scale,
                family: "Concrete".to_string(),
                named_pattern: false,
                named_type: None,
                has_tonic: true,
            };
            built.named_pattern = false;
            return Ok(
                Py::new(py, PyClassInitializer::from(Scale).add_subclass(built))?.into_any(),
            );
        }
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
            PyClassInitializer::from(Scale).add_subclass(Self {
                inner: scale,
                family: "Concrete".to_string(),
                named_pattern: true,
                named_type: None,
                has_tonic: true,
            }),
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
    /// A scale is written out as text and read back.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<(Py<PyAny>, (), Py<PyAny>)> {
        let me = slf.borrow();
        let written = (
            me.inner.clone(),
            me.family.clone(),
            me.named_pattern,
            me.has_tonic,
        );
        drop(me);
        crate::pickled(slf, &written)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        type State = (RsScale, String, bool, bool);
        let Some((inner, family, named_pattern, has_tonic)) =
            crate::unpickled::<_, State>(slf, state)?
        else {
            return Ok(());
        };
        let mut me = slf.borrow_mut();
        me.inner = inner;
        me.family = family;
        me.named_pattern = named_pattern;
        me.has_tonic = has_tonic;
        Ok(())
    }

    /// music21's `ConcreteScale(tonic)`, where the pattern of steps comes
    /// from the class and only the tonic is given. No tonic means C, as it
    /// does upstream.
    #[new]
    #[pyo3(signature = (tonic = None, **keywords))]
    fn new(
        tonic: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let mut built = Self::of(
            RsScaleType::Major,
            RsPitch::from_name("C").map_err(scale_error)?,
        );
        built.named_pattern = false;
        match tonic.filter(|value| !value.is_none()) {
            Some(value) => {
                built.inner = RsScale::new(RsScaleType::Major, tonic_pitch(value)?);
            }
            None => built.has_tonic = false,
        }
        // music21 also takes the notes themselves, which is a scale nobody
        // has a name for.
        if let Some(keywords) = keywords
            && let Some(pitches) = keywords.get_item("pitches")?
            && !pitches.is_none()
        {
            built.inner = RsScale::from_pitches(&pitch_list(&pitches)?).map_err(scale_error)?;
            built.has_tonic = true;
        }
        Ok(PyClassInitializer::from(Scale).add_subclass(built))
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
        let class = slf.as_any().get_type();
        let named = class.hasattr("scaleTypeName")?;
        let family: String = match class.getattr("scaleFamilyName") {
            Ok(family) => family.extract()?,
            Err(_) => "Concrete".to_string(),
        };
        // A scale given by its notes keeps them; only the tonic is settled
        // here, and it already has one.
        if slf.borrow().inner.is_custom() {
            return Ok(());
        }
        let scale_type = Self::scale_type_of(&class)?;
        let given = tonic.filter(|value| !value.is_none());
        let tonic = match given {
            Some(value) => tonic_pitch(value)?,
            None => RsPitch::from_name("C").map_err(scale_error)?,
        };
        let mut me = slf.borrow_mut();
        me.inner = RsScale::new(scale_type, tonic);
        me.family = family;
        me.named_pattern = named;
        me.has_tonic = given.is_some();
        Ok(())
    }

    /// music21's `isConcrete`: whether the scale has a note to stand on.
    #[getter]
    fn isConcrete(&self) -> bool {
        self.has_tonic
    }

    /// music21's `type`: what kind of scale this is, which for a concrete
    /// one is the pattern it stands on and for a vague one is `"Concrete"`.
    #[getter]
    fn r#type(&self) -> String {
        if let Some(named) = &self.named_type {
            return named.clone();
        }
        if !self.named_pattern || self.inner.is_custom() {
            return self.family.clone();
        }
        self.inner
            .scale_type()
            .music21_descriptive_name()
            .to_string()
    }

    /// music21's `type` is a plain attribute, so a caller may name a scale
    /// whatever it likes and the name comes back with it.
    #[setter]
    fn set_type(&mut self, value: &str) {
        self.named_type = Some(value.to_string());
    }

    /// music21's `name`: the tonic and the pattern, `"D major"`, and
    /// `"Abstract Concrete"` while there is no tonic to name.
    #[getter]
    fn name(&self) -> String {
        if !self.has_tonic {
            return format!("Abstract {}", self.r#type());
        }
        format!("{} {}", self.inner.tonic().name(), self.r#type())
    }

    #[getter]
    fn get_tonic(&self) -> Option<Pitch> {
        self.has_tonic
            .then(|| Pitch::wrap(self.inner.tonic().clone(), false))
    }

    #[setter]
    fn set_tonic(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        match value.filter(|value| !value.is_none()) {
            Some(value) => {
                self.inner = RsScale::new(self.inner.scale_type(), pitch_from_any(value)?);
                self.has_tonic = true;
            }
            None => self.has_tonic = false,
        }
        Ok(())
    }

    /// music21's `pitches`: the scale from its tonic through the octave, the
    /// closing octave included.
    #[getter]
    fn pitches(&self) -> PyResult<Vec<Pitch>> {
        Ok(self
            .realized_anywhere()
            .pitches()
            .map_err(scale_error)?
            .into_iter()
            .map(|pitch| Pitch::wrap(pitch, false))
            .collect())
    }

    /// music21's `getPitches`: the scale over a range of pitches, or over
    /// the octave above the tonic when no range is given.
    #[pyo3(signature = (minPitch = None, maxPitch = None, direction = None, **_keywords))]
    fn getPitches(
        &self,
        minPitch: Option<&Bound<'_, PyAny>>,
        maxPitch: Option<&Bound<'_, PyAny>>,
        direction: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<Pitch>> {
        let descending = is_descending(direction);
        let (Some(low), Some(high)) = (
            minPitch.filter(|value| !value.is_none()),
            maxPitch.filter(|value| !value.is_none()),
        ) else {
            if descending {
                return Ok(wrap_pitches(
                    self.realized()?.pitches_descending().map_err(scale_error)?,
                ));
            }
            return self.pitches();
        };
        let (low, high) = (pitch_from_any(low)?, pitch_from_any(high)?);
        let scale = self.realized()?;
        Ok(wrap_pitches(if descending {
            scale
                .pitches_between_descending(&low, &high)
                .map_err(scale_error)?
        } else {
            scale.pitches_between(&low, &high).map_err(scale_error)?
        }))
    }

    /// music21's `getTonic`: the note the scale comes to rest on, as it
    /// sounds — so a scale built on a bare `C` answers `C4`, and a plagal
    /// mode answers its final rather than the bottom of its range.
    fn getTonic(&self) -> PyResult<Pitch> {
        self.realized()?
            .final_pitch()
            .map(|pitch| Pitch::wrap(pitch, false))
            .map_err(scale_error)
    }

    /// music21's `getDominant`: the reciting tone.
    fn getDominant(&self) -> PyResult<Pitch> {
        self.realized()?
            .dominant()
            .map(|pitch| Pitch::wrap(pitch, false))
            .map_err(scale_error)
    }

    /// music21's `getLeadingTone`: the seventh degree raised or lowered to
    /// sit a semitone below the final, which in a minor scale is not the
    /// seventh degree the scale itself has.
    fn getLeadingTone(&self) -> PyResult<Pitch> {
        self.realized()?
            .leading_tone()
            .map(|pitch| Pitch::wrap(pitch, false))
            .map_err(scale_error)
    }

    /// music21's `abstract`: the pattern of steps this scale stands on,
    /// which here is the scale type it was built with.
    ///
    /// Setting it changes the pattern and keeps the tonic, which is how
    /// music21 turns a scale of one kind into a scale of another.
    #[getter]
    fn get_abstract(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        AbstractScale::object(py, self.named_pattern.then(|| self.inner.scale_type()))
    }

    #[setter]
    fn set_abstract(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let scale_type = AbstractScale::scale_type_of(value).ok_or_else(|| {
            ScaleException::new_err(format!(
                "cannot read a scale pattern from {}",
                value
                    .str()
                    .map_or_else(|_| "?".to_string(), |v| v.to_string())
            ))
        })?;
        self.inner = RsScale::new(scale_type, self.inner.tonic().clone());
        self.named_pattern = true;
        Ok(())
    }

    /// music21's `octaveDuplicating`: whether the pattern repeats at the
    /// octave, which every one of these does.
    #[getter]
    fn octaveDuplicating(&self) -> bool {
        true
    }

    /// music21's `deriveRanked`: the scales of this pattern containing the
    /// most of the given pitches, best first, each with how many it matched.
    ///
    /// Only the first four by default, as upstream, and matched by sounding
    /// note unless asked for written ones.
    #[pyo3(signature = (other, *, resultsReturned = 4, comparisonAttribute = "pitchClass", removeDuplicates = false, **_keywords))]
    fn deriveRanked(
        slf: &Bound<'_, Self>,
        other: &Bound<'_, PyAny>,
        resultsReturned: Option<usize>,
        comparisonAttribute: &str,
        removeDuplicates: bool,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<(usize, Py<PyAny>)>> {
        let comparison = comparison_of(comparisonAttribute);
        let mut pitches = pitch_list(other)?;
        if removeDuplicates {
            pitches = without_duplicates(pitches, comparison);
        }
        let ranked = slf
            .borrow()
            .inner
            .derive_ranked_by(&pitches, resultsReturned, comparison)
            .map_err(scale_error)?;
        ranked
            .into_iter()
            .map(|(matched, scale)| Ok((matched, Self::object(slf.py(), scale)?)))
            .collect()
    }

    /// music21's `derive`: the best-ranked of those.
    #[pyo3(signature = (other, *, comparisonAttribute = "pitchClass", **_keywords))]
    fn derive(
        slf: &Bound<'_, Self>,
        other: &Bound<'_, PyAny>,
        comparisonAttribute: &str,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        let pitches = pitch_list(other)?;
        let mut ranked = slf
            .borrow()
            .inner
            .derive_ranked_by(&pitches, Some(1), comparison_of(comparisonAttribute))
            .map_err(scale_error)?;
        let derived = ranked
            .pop()
            .map(|(_, scale)| scale)
            .ok_or_else(|| ScaleException::new_err("no candidate tonics"))?;
        Self::object(slf.py(), derived)
    }

    /// music21's `deriveAll`: every scale of this pattern that contains all
    /// of the given pitches.
    #[pyo3(signature = (other, *, comparisonAttribute = "pitchClass", **_keywords))]
    fn deriveAll(
        slf: &Bound<'_, Self>,
        other: &Bound<'_, PyAny>,
        comparisonAttribute: &str,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let pitches = pitch_list(other)?;
        let wanted = pitches.len();
        let ranked = slf
            .borrow()
            .inner
            .derive_ranked_by(&pitches, None, comparison_of(comparisonAttribute))
            .map_err(scale_error)?;
        ranked
            .into_iter()
            .filter(|(matched, _)| *matched == wanted)
            .map(|(_, scale)| Self::object(slf.py(), scale))
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
        self.realized()?
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
        let scale = self.realized()?;
        let (Some(low), Some(high)) = (
            minPitch.filter(|value| !value.is_none()),
            maxPitch.filter(|value| !value.is_none()),
        ) else {
            return Ok(wrap_pitches(
                scale
                    .pitches_from_scale_degrees(&degreeTargets)
                    .map_err(scale_error)?,
            ));
        };
        Ok(wrap_pitches(
            scale
                .pitches_from_scale_degrees_between(
                    &degreeTargets,
                    &pitch_from_any(low)?,
                    &pitch_from_any(high)?,
                )
                .map_err(scale_error)?,
        ))
    }

    /// music21's `romanNumeral`: the roman numeral on a degree of this
    /// scale, which is the plain triad on it.
    fn romanNumeral(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        degree: &Bound<'_, PyAny>,
    ) -> PyResult<Py<crate::roman::RomanNumeral>> {
        let numeral = crate::roman::RomanNumeral::build(Some(degree), Some(slf.as_any()), None)?;
        crate::roman::RomanNumeral::object(py, numeral)
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
        self.realized()?
            .is_next(
                &pitch_from_any(other)?,
                &pitch_from_any(pitchOrigin)?,
                stepSize,
            )
            .map_err(scale_error)
    }

    /// music21's `pitchFromDegree`: the pitch at a scale degree, counting the
    /// tonic as one.
    ///
    /// The range and the direction are music21's, and are ignored here: the
    /// scale is realized from its tonic either way, and the crate has no
    /// bounded realization to narrow.
    #[pyo3(signature = (
        degree,
        minPitch = None,
        maxPitch = None,
        direction = None,
        equateTermini = true,
    ))]
    fn pitchFromDegree(
        &self,
        degree: usize,
        minPitch: Option<&Bound<'_, PyAny>>,
        maxPitch: Option<&Bound<'_, PyAny>>,
        direction: Option<&Bound<'_, PyAny>>,
        equateTermini: bool,
    ) -> PyResult<Pitch> {
        let _ = (minPitch, maxPitch, direction, equateTermini);
        self.realized()?
            .pitch_at_degree(degree)
            .map(|pitch| Pitch::wrap(pitch, false))
            .map_err(scale_error)
    }

    /// music21's `getChord`: the scale's own notes, sounded together.
    #[pyo3(signature = (minPitch = None, maxPitch = None, direction = None, **_keywords))]
    fn getChord(
        &self,
        py: Python<'_>,
        minPitch: Option<&Bound<'_, PyAny>>,
        maxPitch: Option<&Bound<'_, PyAny>>,
        direction: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        let pitches = match (minPitch, maxPitch) {
            (Some(low), Some(high)) if !low.is_none() && !high.is_none() => self
                .realized()?
                .pitches_between(&pitch_from_any(low)?, &pitch_from_any(high)?)
                .map_err(scale_error)?,
            _ => self.realized()?.pitches().map_err(scale_error)?,
        };
        let _ = direction;
        let chord = music21_rs::Chord::new(pitches.as_slice()).map_err(scale_error)?;
        let facade = crate::chord::Chord::from_inner(py, chord)?;
        Ok(crate::installed_new(py, "music21.chord", "Chord", facade)?.into_any())
    }

    /// music21's `getScaleDegreeFromPitch`: which degree a pitch is, or
    /// nothing when it is not in the scale.
    #[pyo3(signature = (pitchTarget, comparisonAttribute = "name", direction = None, **_keywords))]
    fn getScaleDegreeFromPitch(
        &self,
        pitchTarget: &Bound<'_, PyAny>,
        comparisonAttribute: &str,
        direction: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Option<usize>> {
        let pitch = pitch_from_any(pitchTarget)?;
        self.heard(direction)?
            .degree_of_by(&pitch, comparison_of(comparisonAttribute))
            .map_err(scale_error)
    }

    /// music21's `getScaleDegreeAndAccidentalFromPitch`: the degree and how
    /// far the pitch is altered from it.
    #[pyo3(signature = (pitchTarget, direction = None, **_keywords))]
    fn getScaleDegreeAndAccidentalFromPitch(
        &self,
        py: Python<'_>,
        pitchTarget: &Bound<'_, PyAny>,
        direction: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        let pitch = pitch_from_any(pitchTarget)?;
        let (degree, accidental) = self
            .heard(direction)?
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
        let descending = is_descending(direction);
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
            .realized()?
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
    #[pyo3(name = "match", signature = (other, comparisonAttribute = "name", **_keywords))]
    fn match_pitches(
        &self,
        py: Python<'_>,
        other: &Bound<'_, PyAny>,
        comparisonAttribute: &str,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyDict>> {
        let comparison = comparison_of(comparisonAttribute);
        let pitches = without_duplicates(pitch_list(other)?, comparison);
        let (matched, unmatched) = self
            .realized()?
            .match_pitches_by(&pitches, comparison)
            .map_err(scale_error)?;
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
        self.realized()?
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

    /// Two scales are the same when they are the same pattern on the same
    /// tonic — and a scale with no tonic is compared on its pattern alone,
    /// which is music21's implicit abstract comparison.
    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other.extract::<PyRef<'_, Self>>().is_ok_and(|other| {
            if other.inner.scale_type() != self.inner.scale_type() {
                return false;
            }
            if !other.has_tonic || !self.has_tonic {
                return true;
            }
            other.inner.tonic().name_with_octave() == self.inner.tonic().name_with_octave()
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
    /// What music21 calls a diatonic scale that has no pattern of its own.
    #[classattr]
    #[allow(non_upper_case_globals)]
    const scaleFamilyName: &'static str = "diatonic";

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
            .realized()?
            .relative_major()
            .map_err(scale_error)?;
        ConcreteScale::object(slf.py(), relative)
    }

    /// music21's `getRelativeMinor`.
    fn getRelativeMinor(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let relative = slf
            .as_super()
            .borrow()
            .realized()?
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

/// The pitches with duplicates removed, judged on whichever attribute is
/// named — music21 runs every list of targets through this before matching
/// or deriving from it.
fn without_duplicates(pitches: Vec<RsPitch>, comparison: RsDegreeComparison) -> Vec<RsPitch> {
    let mut seen: Vec<String> = Vec::new();
    let mut kept = Vec::new();
    for pitch in pitches {
        let key = match comparison {
            RsDegreeComparison::Name => pitch.name(),
            RsDegreeComparison::Step => pitch.name().chars().take(1).collect(),
            RsDegreeComparison::PitchClass => pitch.pitch_class().number().to_string(),
        };
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        kept.push(pitch);
    }
    kept
}

/// music21's `comparisonAttribute`: whether a pitch matches a degree by
/// sounding note or by written one.
pub(crate) fn comparison_of(attribute: &str) -> RsDegreeComparison {
    match attribute {
        "step" => RsDegreeComparison::Step,
        "name" | "nameWithOctave" => RsDegreeComparison::Name,
        _ => RsDegreeComparison::PitchClass,
    }
}

/// Whether a `direction` argument says downwards. music21 passes its own
/// `Direction` enum, a name, or a signed number.
fn is_descending(direction: Option<&Bound<'_, PyAny>>) -> bool {
    direction.is_some_and(|direction| {
        direction
            .str()
            .map(|name| name.to_string_lossy().to_lowercase().contains("descending"))
            .unwrap_or(false)
            || direction.extract::<i32>().is_ok_and(|value| value < 0)
    })
}

/// Reads pitches out of whatever holds them.
///
/// Anything with a `pitches` list — another scale, a chord, a stream — is
/// asked for that first, which is how music21 lets one scale be matched
/// against another; then a sequence of pitches, names or notes; then a
/// single thing with a pitch.
fn pitch_list(value: &Bound<'_, PyAny>) -> PyResult<Vec<RsPitch>> {
    if let Ok(pitches) = value.getattr("pitches")
        && !pitches.is_none()
    {
        return pitches
            .try_iter()?
            .map(|pitch| pitch_from_any(&pitch?))
            .collect();
    }
    let Ok(items) = value.try_iter() else {
        return Ok(vec![pitch_from_any(&value.getattr("pitch")?)?]);
    };
    let mut pitches = Vec::new();
    for item in items {
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
    m.add_class::<AbstractScale>()?;
    m.add_class::<ConcreteScale>()?;
    m.add_class::<DiatonicScale>()?;
    let exception = py.get_type::<ScaleException>();
    exception.setattr("__module__", "music21.scale")?;
    m.add("ScaleException", exception)?;
    let network = py.get_type::<IntervalNetworkException>();
    network.setattr("__module__", "music21.scale.intervalNetwork")?;
    m.add("IntervalNetworkException", network)?;

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
