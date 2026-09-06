//! music21's `key` module: `KeySignature`, `Key` and the module functions,
//! over `music21-rs`. `Key` extends `KeySignature` as it does upstream.

use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use music21_rs::{
    Key as RsKey, KeySignature as RsKeySignature, convert_key_string_to_music21_key_string,
    key::{pitch_to_sharps, sharps_to_pitch},
};

use crate::pitch::{Accidental, Pitch, interval_from_any, message, pitch_from_any};

pyo3::create_exception!(music21_rs_facade, KeySignatureException, PyException);
pyo3::create_exception!(music21_rs_facade, KeyException, PyException);

fn key_error(error: music21_rs::Error) -> PyErr {
    KeyException::new_err(message(&error))
}

fn signature_error(error: music21_rs::Error) -> PyErr {
    KeySignatureException::new_err(message(&error))
}

fn description(sharps: i32) -> String {
    match sharps {
        0 => "no sharps or flats".to_string(),
        1 => "1 sharp".to_string(),
        -1 => "1 flat".to_string(),
        n if n > 0 => format!("{n} sharps"),
        n => format!("{} flats", -n),
    }
}

/// music21's `key.KeySignature`.
#[pyclass(
    name = "KeySignature",
    module = "music21.key",
    subclass,
    skip_from_py_object
)]
pub struct KeySignature {
    sharps: i32,
}

impl KeySignature {
    fn inner(&self) -> RsKeySignature {
        RsKeySignature::new(self.sharps)
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
        Ok(Self { sharps })
    }

    #[getter]
    fn sharps(&self) -> i32 {
        self.sharps
    }

    #[setter]
    fn set_sharps(&mut self, value: Option<i32>) {
        self.sharps = value.unwrap_or(0);
    }

    #[getter]
    #[allow(non_snake_case)]
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
    #[allow(non_snake_case)]
    fn isNonTraditional(&self) -> bool {
        false
    }

    #[allow(non_snake_case)]
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
    #[allow(non_snake_case)]
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
    #[allow(non_snake_case)]
    fn transpose(
        mut slf: PyRefMut<'_, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
        inPlace: bool,
    ) -> PyResult<Option<Py<PyAny>>> {
        let interval = interval_from_any(value)?;
        let transposed = slf.inner().transpose(&interval).map_err(signature_error)?;
        if inPlace {
            slf.sharps = transposed.sharps();
            Ok(None)
        } else {
            Ok(Some(
                Py::new(
                    py,
                    KeySignature {
                        sharps: transposed.sharps(),
                    },
                )?
                .into_any(),
            ))
        }
    }

    #[pyo3(signature = (p, *, inPlace = false))]
    #[allow(non_snake_case)]
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
            description(slf.borrow().sharps)
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
            .is_ok_and(|other| other.sharps == slf.borrow().sharps))
    }

    fn __hash__(slf: &Bound<'_, Self>) -> u64 {
        (slf.as_ptr() as usize >> 4) as u64
    }

    fn __deepcopy__(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        _memo: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        if let Ok(key) = slf.extract::<PyRef<Key>>() {
            return Key::object(py, key.inner.clone());
        }
        Ok(Py::new(
            py,
            KeySignature {
                sharps: slf.borrow().sharps,
            },
        )?
        .into_any())
    }
}

/// music21's `key.Key`: a key signature with a tonic and a mode.
#[pyclass(name = "Key", module = "music21.key", extends = KeySignature)]
pub struct Key {
    pub(crate) inner: RsKey,
}

impl Key {
    fn object(py: Python<'_>, inner: RsKey) -> PyResult<Py<PyAny>> {
        let init = PyClassInitializer::from(KeySignature {
            sharps: inner.sharps(),
        })
        .add_subclass(Key { inner });
        Ok(Py::new(py, init)?.into_any())
    }

    fn same_key(&self, other: &Key) -> bool {
        self.inner.tonic().name() == other.inner.tonic().name()
            && self.inner.mode() == other.inner.mode()
    }
}

#[pymethods]
impl Key {
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
        Ok(PyClassInitializer::from(KeySignature {
            sharps: inner.sharps(),
        })
        .add_subclass(Key { inner }))
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
        slf.as_super().sharps = sharps;
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
    #[allow(non_snake_case)]
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

    #[allow(non_snake_case)]
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
    #[allow(non_snake_case)]
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
    #[allow(non_snake_case)]
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
            slf.as_super().sharps = sharps;
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

/// music21's `key.sharpsToPitch`.
#[pyfunction]
#[pyo3(name = "sharpsToPitch")]
#[allow(non_snake_case)]
fn sharps_to_pitch_facade(sharpCount: i32) -> PyResult<Pitch> {
    Ok(Pitch::wrap(
        sharps_to_pitch(sharpCount).map_err(key_error)?,
        false,
    ))
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
#[allow(non_snake_case)]
fn convert_key_string(textString: &str) -> String {
    convert_key_string_to_music21_key_string(textString)
}

/// Adds the key facades to the module.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add_class::<KeySignature>()?;
    m.add_class::<Key>()?;
    m.add_function(wrap_pyfunction!(sharps_to_pitch_facade, m)?)?;
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
