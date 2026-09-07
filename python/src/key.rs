//! music21's `key` module: `KeySignature`, `Key` and the module functions,
//! over `music21-rs`. `Key` extends `KeySignature` as it does upstream.

#![allow(non_snake_case)]

use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use music21_rs::{
    Key as RsKey, KeySignature as RsKeySignature, convert_key_string_to_music21_key_string,
    key::{pitch_to_sharps, sharps_to_pitch},
};

use crate::pitch::{Accidental, Pitch, interval_from_any, message, pitch_from_any};

pub const NAMES: &[&str] = &[
    "KeySignature",
    "Key",
    "KeySignatureException",
    "KeyException",
    "sharpsToPitch",
    "_sharpsToPitchCache",
    "pitchToSharps",
    "convertKeyStringToMusic21KeyString",
];

pyo3::create_exception!(music21_rs_facade, KeySignatureException, PyException);
pyo3::create_exception!(music21_rs_facade, KeyException, PyException);

fn key_error(error: music21_rs::Error) -> PyErr {
    KeyException::new_err(message(&error))
}

fn signature_error(error: music21_rs::Error) -> PyErr {
    KeySignatureException::new_err(message(&error))
}

/// music21's `key.KeySignature`.
#[pyclass(
    name = "KeySignature",
    module = "music21.key",
    subclass,
    skip_from_py_object
)]
pub struct KeySignature {
    pub(crate) signature: RsKeySignature,
}

impl KeySignature {
    fn inner(&self) -> RsKeySignature {
        self.signature.clone()
    }

    fn of(sharps: i32) -> Self {
        Self {
            signature: RsKeySignature::new(sharps),
        }
    }
}

fn mode_of(value: Option<&Bound<'_, PyAny>>) -> PyResult<Option<String>> {
    match value {
        Some(value) if !value.is_none() => Ok(Some(value.extract::<String>()?.to_lowercase())),
        _ => Ok(None),
    }
}

fn tonic_name(value: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Ok(text) = value.extract::<String>() {
        return Ok(text);
    }
    if let Ok(pitch) = value.getattr("pitch") {
        return Ok(pitch_from_any(&pitch)?.name());
    }
    Ok(pitch_from_any(value)?.name())
}

#[pymethods]
impl KeySignature {
    #[new]
    #[pyo3(signature = (sharps = None, **kwargs))]
    fn new(
        sharps: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let _ = kwargs;
        let sharps = match sharps {
            None => 0,
            Some(value) if value.is_none() => 0,
            Some(value) => match value.extract::<f64>() {
                Ok(number) if number.fract() == 0.0 && value.extract::<String>().is_err() => {
                    number as i32
                }
                _ => {
                    return Err(KeySignatureException::new_err(format!(
                        "Cannot get a KeySignature from this \"number\" of sharps: {}; did you mean to use a key.Key() object instead?",
                        value.repr()?
                    )));
                }
            },
        };
        Ok(Self::of(sharps))
    }

    #[getter]
    fn sharps(&self) -> Option<i32> {
        self.signature.sharps()
    }

    #[setter]
    fn set_sharps(&mut self, value: Option<i32>) {
        self.signature.set_sharps(value.unwrap_or(0));
    }

    #[setter]
    fn set_alteredPitches(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.signature
            .set_altered_pitches(crate::pitch::pitch_list(Some(value))?);
        Ok(())
    }

    #[getter]
    fn accidentalsApplyOnlyToOctave(&self) -> bool {
        self.signature.accidentals_apply_only_to_octave()
    }

    #[setter]
    fn set_accidentalsApplyOnlyToOctave(&mut self, value: bool) {
        self.signature.set_accidentals_apply_only_to_octave(value);
    }

    #[getter]
    fn alteredPitches(&self) -> PyResult<Vec<Pitch>> {
        Ok(self
            .inner()
            .altered_pitches()
            .map_err(signature_error)?
            .into_iter()
            .map(|pitch| Pitch::wrap(pitch, false))
            .collect())
    }

    #[getter]
    fn isNonTraditional(&self) -> bool {
        self.signature.is_non_traditional()
    }

    fn accidentalByStep(&self, step: &str) -> PyResult<Option<Accidental>> {
        let letter = step
            .chars()
            .next()
            .ok_or_else(|| KeySignatureException::new_err("step cannot be empty"))?
            .to_ascii_uppercase();
        Ok(self
            .inner()
            .accidental_by_step(letter)
            .map_err(signature_error)?
            .map(Accidental::from_inner))
    }

    #[pyo3(signature = (mode = None, tonic = None))]
    fn asKey(
        &self,
        py: Python<'_>,
        mode: Option<&Bound<'_, PyAny>>,
        tonic: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let mode = mode_of(mode)?;
        let tonic = tonic
            .filter(|value| !value.is_none())
            .map(tonic_name)
            .transpose()?;
        let key = self
            .inner()
            .try_as_key(mode.as_deref(), tonic.as_deref())
            .map_err(key_error)?;
        Key::object(py, key)
    }

    #[pyo3(signature = (value, *, inPlace = false))]
    fn transpose(
        mut slf: PyRefMut<'_, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
        inPlace: bool,
    ) -> PyResult<Option<Py<PyAny>>> {
        let interval = interval_from_any(value)?;
        let transposed = slf.inner().transpose(&interval).map_err(signature_error)?;
        if inPlace {
            slf.signature = transposed;
            Ok(None)
        } else {
            Ok(Some(
                Py::new(
                    py,
                    KeySignature {
                        signature: transposed,
                    },
                )?
                .into_any(),
            ))
        }
    }

    #[pyo3(signature = (p, *, inPlace = false))]
    fn transposePitchFromC(&self, p: &Bound<'_, PyAny>, inPlace: bool) -> PyResult<Option<Pitch>> {
        let transposed = self
            .inner()
            .transpose_pitch_from_c(&pitch_from_any(p)?)
            .map_err(signature_error)?;
        if inPlace {
            if let Ok(mut facade) = p.extract::<PyRefMut<Pitch>>() {
                facade.inner = transposed;
            }
            Ok(None)
        } else {
            Ok(Some(Pitch::wrap(transposed, false)))
        }
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        if let Ok(key) = slf.extract::<PyRef<Key>>() {
            return Ok(format!("<music21.key.Key of {}>", key.__str__()));
        }
        Ok(format!(
            "<music21.key.KeySignature of {}>",
            slf.borrow().signature.description()
        ))
    }

    fn __str__(slf: &Bound<'_, Self>) -> PyResult<String> {
        if let Ok(key) = slf.extract::<PyRef<Key>>() {
            return Ok(key.__str__());
        }
        Self::__repr__(slf)
    }

    fn __eq__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        if let Ok(key) = slf.extract::<PyRef<Key>>() {
            return Ok(other
                .extract::<PyRef<Key>>()
                .is_ok_and(|other| key.same_key(&other)));
        }
        if other.extract::<PyRef<Key>>().is_ok() {
            return Ok(false);
        }
        Ok(other
            .extract::<PyRef<KeySignature>>()
            .is_ok_and(|other| other.signature.sharps() == slf.borrow().signature.sharps()))
    }

    fn __hash__(slf: &Bound<'_, Self>) -> u64 {
        (slf.as_ptr() as usize >> 4) as u64
    }

    /// A copy that is an instance of the class it was asked on, so that a
    /// key installed into music21 copies as one music21 can still hold.
    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _py: Python<'py>,
        _memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let class = slf.as_any().get_type();
        let copy = class.call_method1("__new__", (&class,))?;
        if let Ok(key) = slf.extract::<PyRef<Key>>()
            && let Ok(target) = copy.cast::<Key>()
        {
            let inner = key.inner.clone();
            let mut target = target.borrow_mut();
            target.as_super().signature = inner.key_signature();
            target.inner = inner;
            return Ok(copy);
        }
        if let Ok(target) = copy.cast::<Self>() {
            target.borrow_mut().signature = slf.borrow().signature.clone();
        }
        Ok(copy)
    }
}

/// music21's `key.Key`: a key signature with a tonic and a mode.
#[pyclass(name = "Key", module = "music21.key", extends = KeySignature, subclass)]
pub struct Key {
    pub(crate) inner: RsKey,
    /// How well this key fitted the music it was analysed from, and the
    /// keys that fitted less well, best first.
    ///
    /// music21 fills both in when a key comes out of `analyze('key')`, and
    /// reads them back for `tonalCertainty`. A key that was simply named
    /// carries no ranking, which is what makes asking one how certain it is
    /// an error.
    correlation_coefficient: f64,
    /// Held as the Python list itself, not as a copy of it: music21's own
    /// key analysis fills the list by appending to what the getter hands
    /// back, so handing back a fresh one each time would lose every
    /// alternative it found.
    alternate_interpretations: Option<Py<PyList>>,
}

impl Key {
    fn object(py: Python<'_>, inner: RsKey) -> PyResult<Py<PyAny>> {
        let init = PyClassInitializer::from(KeySignature::of(inner.sharps())).add_subclass(Key {
            inner,
            correlation_coefficient: 0.0,
            alternate_interpretations: None,
        });
        Ok(Py::new(py, init)?.into_any())
    }

    /// The list of alternatives, made on first asking and then kept, so
    /// that appending to what a caller was handed reaches this key.
    fn interpretations(&mut self, py: Python<'_>) -> PyResult<&Py<PyList>> {
        if self.alternate_interpretations.is_none() {
            self.alternate_interpretations = Some(PyList::empty(py).unbind());
        }
        Ok(self
            .alternate_interpretations
            .as_ref()
            .expect("just set when missing"))
    }

    fn same_key(&self, other: &Key) -> bool {
        self.inner.tonic().name() == other.inner.tonic().name()
            && self.inner.mode() == other.inner.mode()
    }
}

#[pymethods]
impl Key {
    /// music21's `correlationCoefficient`: how well this key fitted the
    /// music it was analysed from. Zero until an analysis says otherwise.
    #[getter]
    fn get_correlationCoefficient(&self) -> f64 {
        self.correlation_coefficient
    }

    #[setter]
    fn set_correlationCoefficient(&mut self, value: f64) {
        self.correlation_coefficient = value;
    }

    /// music21's `alternateInterpretations`: the keys that fitted less well
    /// than this one, best first.
    #[getter]
    fn get_alternateInterpretations(&mut self, py: Python<'_>) -> PyResult<Py<PyList>> {
        Ok(self.interpretations(py)?.clone_ref(py))
    }

    #[setter]
    fn set_alternateInterpretations(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let Some(value) = value.filter(|value| !value.is_none()) else {
            self.alternate_interpretations = None;
            return Ok(());
        };
        let list = PyList::empty(value.py());
        for item in value.try_iter()? {
            list.append(item?)?;
        }
        self.alternate_interpretations = Some(list.unbind());
        Ok(())
    }

    /// music21's `tonalCertainty`: how decisively this key won the analysis
    /// it came out of. A key that was simply named never ran one, which is
    /// why asking is an error rather than a zero.
    #[pyo3(signature = (method = "correlationCoefficient"))]
    fn tonalCertainty(&mut self, py: Python<'_>, method: &str) -> PyResult<f64> {
        if method != "correlationCoefficient" {
            return Err(PyValueError::new_err(format!("Unknown method: {method}")));
        }
        let alternatives = self.interpretations(py)?.bind(py).clone();
        if alternatives.is_empty() {
            return Err(KeySignatureException::new_err(
                "cannot process ambiguity without a list of .alternateInterpretations",
            ));
        }
        let mut scores = vec![self.correlation_coefficient];
        for key in alternatives.iter() {
            let score = key.getattr("correlationCoefficient")?.extract::<f64>()?;
            if score > 0.0 {
                scores.push(score);
            }
        }
        Ok(music21_rs::tonal_certainty_from_scores(&scores))
    }

    #[new]
    #[pyo3(signature = (tonic = None, mode = None, **kwargs))]
    fn new(
        tonic: Option<&Bound<'_, PyAny>>,
        mode: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let _ = kwargs;
        let tonic = match tonic {
            Some(value) if !value.is_none() => tonic_name(value)?,
            _ => "C".to_string(),
        };
        let mode = mode_of(mode)?;
        let inner = RsKey::from_tonic_mode(&tonic, mode.as_deref()).map_err(key_error)?;
        Ok(
            PyClassInitializer::from(KeySignature::of(inner.sharps())).add_subclass(Key {
                inner,
                correlation_coefficient: 0.0,
                alternate_interpretations: None,
            }),
        )
    }

    #[getter]
    fn tonic(&self) -> Pitch {
        Pitch::wrap(self.inner.tonic(), false)
    }

    #[getter]
    fn mode(&self) -> String {
        self.inner.mode().to_string()
    }

    #[setter]
    fn set_mode(mut slf: PyRefMut<'_, Self>, value: &str) -> PyResult<()> {
        let tonic = slf.inner.tonic().name();
        let rebuilt = RsKey::from_tonic_mode(&tonic, Some(value)).map_err(key_error)?;
        let sharps = rebuilt.sharps();
        slf.inner = rebuilt;
        slf.as_super().signature = RsKeySignature::new(sharps);
        Ok(())
    }

    #[getter]
    fn pitches(&self) -> PyResult<Vec<Pitch>> {
        Ok(self
            .inner
            .pitches()
            .map_err(key_error)?
            .into_iter()
            .map(|pitch| Pitch::wrap(pitch, false))
            .collect())
    }

    #[getter]
    fn tonicPitchNameWithCase(&self) -> String {
        self.inner.tonic_pitch_name_with_case()
    }

    #[getter]
    fn relative(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Key::object(py, self.inner.relative().map_err(key_error)?)
    }

    #[getter]
    fn parallel(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Key::object(py, self.inner.parallel().map_err(key_error)?)
    }

    fn deriveByDegree(
        &self,
        py: Python<'_>,
        degree: usize,
        pitchRef: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let derived = self
            .inner
            .derive_by_degree(degree, &pitch_from_any(pitchRef)?)
            .map_err(key_error)?;
        Key::object(py, derived)
    }

    #[pyo3(signature = (degree, *args, **kwargs))]
    fn pitchFromDegree(
        &self,
        degree: usize,
        args: &Bound<'_, pyo3::types::PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Pitch> {
        let _ = (args, kwargs);
        Ok(Pitch::wrap(
            self.inner.pitch_from_degree(degree).map_err(key_error)?,
            false,
        ))
    }

    #[pyo3(signature = (value, *, inPlace = false))]
    fn transpose(
        mut slf: PyRefMut<'_, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
        inPlace: bool,
    ) -> PyResult<Option<Py<PyAny>>> {
        let interval = interval_from_any(value)?;
        let transposed = slf.inner.transpose(&interval).map_err(key_error)?;
        if inPlace {
            let sharps = transposed.sharps();
            slf.inner = transposed;
            slf.as_super().signature = RsKeySignature::new(sharps);
            Ok(None)
        } else {
            Ok(Some(Key::object(py, transposed)?))
        }
    }

    fn __str__(&self) -> String {
        format!(
            "{} {}",
            self.inner.tonic_pitch_name_with_case(),
            self.inner.mode()
        )
    }
}

/// music21's `key.sharpsToPitch`, memoized in `_sharpsToPitchCache` as
/// music21 memoizes it — the cache is documented behaviour of the function,
/// and its own doctest reads the dictionary back.
#[pyfunction]
#[pyo3(name = "sharpsToPitch", signature = (sharpCount = None))]
fn sharps_to_pitch_facade<'py>(
    py: Python<'py>,
    sharpCount: Option<i32>,
) -> PyResult<Bound<'py, PyAny>> {
    // music21 reads a missing count as C major rather than as an error.
    let sharp_count = sharpCount.unwrap_or(0);
    let cache = sharps_to_pitch_cache(py)?;
    if let Some(cached) = cache.get_item(sharp_count)? {
        return Ok(cached);
    }
    let pitch = Pitch::wrap(sharps_to_pitch(sharp_count).map_err(key_error)?, false)
        .into_pyobject(py)?
        .into_any();
    cache.set_item(sharp_count, &pitch)?;
    Ok(pitch)
}

/// The dictionary `sharpsToPitch` remembers its answers in, made once and
/// then kept on the facade module so that it is the same object music21's
/// `key._sharpsToPitchCache` names once the facade is installed.
fn sharps_to_pitch_cache<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
    let facade = py
        .import("music21_rs_facade")
        .or_else(|_| py.import("music21_rs"))?;
    Ok(facade
        .getattr("_sharpsToPitchCache")?
        .cast_into::<PyDict>()?)
}

/// music21's `key.pitchToSharps`.
#[pyfunction]
#[pyo3(name = "pitchToSharps", signature = (value, mode = None))]
fn pitch_to_sharps_facade(
    value: &Bound<'_, PyAny>,
    mode: Option<&Bound<'_, PyAny>>,
) -> PyResult<i32> {
    let pitch = pitch_from_any(value)?;
    let mode = mode_of(mode)?;
    pitch_to_sharps(&pitch, mode.as_deref()).map_err(key_error)
}

/// music21's `key.convertKeyStringToMusic21KeyString`.
#[pyfunction]
#[pyo3(name = "convertKeyStringToMusic21KeyString")]
fn convert_key_string(textString: &str) -> String {
    convert_key_string_to_music21_key_string(textString)
}

/// Adds the key facades to the module.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add_class::<KeySignature>()?;
    m.add_class::<Key>()?;
    m.add_function(wrap_pyfunction!(sharps_to_pitch_facade, m)?)?;
    m.add("_sharpsToPitchCache", PyDict::new(py))?;
    m.add_function(wrap_pyfunction!(pitch_to_sharps_facade, m)?)?;
    m.add_function(wrap_pyfunction!(convert_key_string, m)?)?;
    for (name, exception) in [
        (
            "KeySignatureException",
            py.get_type::<KeySignatureException>(),
        ),
        ("KeyException", py.get_type::<KeyException>()),
    ] {
        exception.setattr("__module__", "music21.key")?;
        m.add(name, exception)?;
    }
    Ok(())
}
