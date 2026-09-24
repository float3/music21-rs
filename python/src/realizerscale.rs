//! music21's `figuredBass.realizerScale`, over the crate's.
//!
//! `FiguredBassScale` holds the crate's scale, and hands out the scale and
//! key signature music21 keeps on it as objects of the classes standing for
//! music21's, the same ones every time.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyList;

use music21_rs_crate::figuredbass::Notation as RsNotation;
use music21_rs_crate::figuredbass::scale::{
    FiguredBassMode, FiguredBassScale as RsFiguredBassScale,
};
use music21_rs_crate::{Pitch as RsPitch, Scale as RsScale};

use crate::pitch::{Pitch, pitch_from_any};

pyo3::create_exception!(
    music21_rs_facade,
    FiguredBassScaleException,
    crate::Music21Exception
);

error_into!(scale_error, FiguredBassScaleException);

/// The names this facade replaces in `music21.figuredBass.realizerScale`.
pub const NAMES: &[&str] = &["FiguredBassScale", "FiguredBassScaleException"];

/// A column of figures as music21's `Notation` reads it.
pub(crate) fn notation_from(written: &str) -> PyResult<RsNotation> {
    RsNotation::parse(written).map_err(crate::figuredbass::notation_error)
}

/// music21's `figuredBass.realizerScale.FiguredBassScale`: the key a figured
/// bass is counted in.
#[pyclass(
    name = "FiguredBassScale",
    module = "music21.figuredBass.realizerScale",
    subclass,
    skip_from_py_object
)]
pub struct FiguredBassScale {
    pub(crate) inner: RsFiguredBassScale,
    realizer_scale: Option<Py<PyAny>>,
    key_sig: Option<Py<PyAny>>,
}

impl FiguredBassScale {
    pub(crate) fn wrap(inner: RsFiguredBassScale) -> Self {
        Self {
            inner,
            realizer_scale: None,
            key_sig: None,
        }
    }

    /// Pitches as the objects music21 would hand back.
    fn objects<'py>(py: Python<'py>, pitches: Vec<RsPitch>) -> PyResult<Bound<'py, PyList>> {
        let list = PyList::empty(py);
        for pitch in pitches {
            list.append(crate::installed_new(
                py,
                "music21.pitch",
                "Pitch",
                Pitch::wrap(pitch, false),
            )?)?;
        }
        Ok(list)
    }
}

#[pymethods]
impl FiguredBassScale {
    #[new]
    #[pyo3(signature = (scaleTonic = None, scaleMode = "major"))]
    fn new(scaleTonic: Option<&Bound<'_, PyAny>>, scaleMode: &str) -> PyResult<Self> {
        let tonic = match scaleTonic {
            Some(tonic) => pitch_from_any(tonic)?,
            None => RsPitch::from_name("C").map_err(crate::pitch::pitch_error)?,
        };
        let mode = FiguredBassMode::from_name(scaleMode).map_err(scale_error)?;
        let inner = RsFiguredBassScale::new(tonic, mode).map_err(scale_error)?;
        Ok(Self::wrap(inner))
    }

    /// The scale on the tonic in the mode: music21's `realizerScale`.
    #[getter]
    fn get_realizerScale(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        if let Some(scale) = &slf.borrow().realizer_scale {
            return Ok(scale.clone_ref(py));
        }
        let scale: RsScale = slf.borrow().inner.scale().clone();
        let made = crate::scale::ConcreteScale::object(py, scale)?;
        slf.borrow_mut().realizer_scale = Some(made.clone_ref(py));
        Ok(made)
    }

    #[setter]
    fn set_realizerScale(&mut self, value: Py<PyAny>) {
        self.realizer_scale = Some(value);
    }

    /// The key signature the tonic and mode are written with: music21's
    /// `keySig`.
    #[getter]
    fn get_keySig(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        if let Some(signature) = &slf.borrow().key_sig {
            return Ok(signature.clone_ref(py));
        }
        let signature = slf.borrow().inner.key_signature().clone();
        let made = crate::installed_new(
            py,
            "music21.key",
            "KeySignature",
            crate::key::KeySignature { signature },
        )?
        .into_any();
        slf.borrow_mut().key_sig = Some(made.clone_ref(py));
        Ok(made)
    }

    #[setter]
    fn set_keySig(&mut self, value: Py<PyAny>) {
        self.key_sig = Some(value);
    }

    /// The names of the notes a bass and its figures stand for, the bass
    /// first.
    #[pyo3(signature = (bassPitch, notationString = ""))]
    fn getPitchNames(
        &self,
        bassPitch: &Bound<'_, PyAny>,
        notationString: &str,
    ) -> PyResult<Vec<String>> {
        self.inner
            .pitch_names(&pitch_from_any(bassPitch)?, &notation_from(notationString)?)
            .map_err(scale_error)
    }

    /// Every note a bass and its figures stand for from the bass up to the
    /// octave above it, not including that.
    #[pyo3(signature = (bassPitch, notationString = ""))]
    fn getSamplePitches<'py>(
        &self,
        py: Python<'py>,
        bassPitch: &Bound<'py, PyAny>,
        notationString: &str,
    ) -> PyResult<Bound<'py, PyList>> {
        let pitches = self
            .inner
            .sample_pitches(&pitch_from_any(bassPitch)?, &notation_from(notationString)?)
            .map_err(scale_error)?;
        Self::objects(py, pitches)
    }

    /// Every note a bass and its figures stand for from the bass up to
    /// `maxPitch`, `B5` unless given, lowest first.
    #[pyo3(signature = (bassPitch, notationString = "", maxPitch = None))]
    fn getPitches<'py>(
        &self,
        py: Python<'py>,
        bassPitch: &Bound<'py, PyAny>,
        notationString: &str,
        maxPitch: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyList>> {
        let top = maxPitch
            .filter(|top| !top.is_none())
            .map(pitch_from_any)
            .transpose()?;
        let pitches = self
            .inner
            .pitches(
                &pitch_from_any(bassPitch)?,
                &notation_from(notationString)?,
                top.as_ref(),
            )
            .map_err(scale_error)?;
        Self::objects(py, pitches)
    }

    fn _reprInternal(slf: &Bound<'_, Self>) -> PyResult<String> {
        Self::get_realizerScale(slf)?
            .bind(slf.py())
            .repr()?
            .extract()
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<FiguredBassScale>()
}
