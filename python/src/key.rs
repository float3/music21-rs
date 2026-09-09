//! music21's `key` module: `KeySignature`, `Key` and the module functions,
//! over `music21-rs`. `Key` extends `KeySignature` as it does upstream.

#![allow(non_snake_case)]

use pyo3::exceptions::{PyKeyError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

use music21_rs::scale::{
    Scale as RsScale, ScaleType as RsScaleType, SolfegVariant as RsSolfegVariant,
};

use crate::scale::ConcreteScale;

use music21_rs::{
    Key as RsKey, KeySignature as RsKeySignature, Pitch as RsPitch,
    convert_key_string_to_music21_key_string,
    key::{pitch_to_sharps, sharps_to_pitch},
};

use crate::pitch::{Accidental, Pitch, interval_from_any, pitch_from_any};

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

pyo3::create_exception!(
    music21_rs_facade,
    KeySignatureException,
    crate::Music21Exception
);
pyo3::create_exception!(music21_rs_facade, KeyException, crate::Music21Exception);

error_into!(key_error, KeyException);

error_into!(signature_error, KeySignatureException);

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

/// The sharp count of the major key on this tonic, which is the half of the
/// mode lookup that music21 does before its table miss raises.
fn unsolved_mode_lookup(tonic: &str) -> Option<i32> {
    pitch_to_sharps(&RsPitch::from_name(tonic).ok()?, None).ok()
}

/// music21's deprecation warning for `sharps=None`: turning
/// `isNonTraditional` on is how a non-traditional signature is asked for.
fn warn_sharps_none(py: Python<'_>) -> PyResult<()> {
    let Ok(category) = py
        .import("music21.exceptions21")
        .and_then(|module| module.getattr("Music21DeprecationWarning"))
    else {
        return Ok(());
    };
    py.import("warnings")?.call_method1(
        "warn",
        (
            "sharps=None is deprecated: set isNonTraditional to True instead.",
            category,
            2,
        ),
    )?;
    Ok(())
}

/// music21's warning that a tonic given beside a mode says nothing: the mode
/// alone decides the key, so the tonic is dropped.
fn warn_ignored_tonic(py: Python<'_>, tonic: &str) -> PyResult<()> {
    let Ok(module) = py.import("music21.key") else {
        return Ok(());
    };
    let Ok(category) = module.getattr("KeyWarning") else {
        return Ok(());
    };
    py.import("warnings")?.call_method1(
        "warn",
        (format!("ignoring provided tonic: {tonic}"), category, 2),
    )?;
    Ok(())
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
    /// A key signature is written out as text and read back.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().signature)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let Some(signature) = crate::unpickled::<_, RsKeySignature>(slf, state)? else {
            return Ok(());
        };
        slf.borrow_mut().signature = signature;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (sharps = crate::Given(None), **kwargs))]
    fn new(
        py: Python<'_>,
        sharps: crate::Given<'_>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let _ = kwargs;
        // `sharps=None` asks for a non-traditional signature, with music21's
        // deprecation warning, rather than for a count of nought.
        if sharps.written_as_none() {
            warn_sharps_none(py)?;
            let mut signature = Self::of(0);
            signature.signature.set_non_traditional(true);
            return Ok(signature);
        }
        let sharps = match sharps.value() {
            None => 0,
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

    /// music21's `sharps` is always an `int`: a non-traditional signature
    /// reports nought and says so through `isNonTraditional`.
    #[getter]
    fn sharps(&self) -> i32 {
        self.signature.sharps().unwrap_or(0)
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

    /// Turning it on drops the sharp count and lets `alteredPitches` be
    /// assigned.
    #[setter]
    fn set_isNonTraditional(&mut self, value: bool) {
        self.signature.set_non_traditional(value);
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
        // A mode says which key this signature is; a tonic asks for the mode
        // to be solved from it. Given both, the mode wins and the tonic is
        // ignored — which music21 says out loud rather than silently.
        if let (Some(_), Some(tonic)) = (&mode, &tonic) {
            warn_ignored_tonic(py, tonic)?;
        }
        let key = self
            .inner()
            .try_as_key(mode.as_deref(), tonic.as_deref())
            .map_err(|error| {
                let raised = key_error(error);
                // music21 solves the mode by looking the sharp difference up
                // in a table, and lets the `KeyError` that a miss raises
                // stand as what caused the failure. Its own tests catch the
                // pair, so the cause is carried across here too.
                if mode.is_none()
                    && let Some(missing) = tonic
                        .as_deref()
                        .filter(|_| self.signature.sharps().is_some())
                        .and_then(unsolved_mode_lookup)
                        .map(|major| self.signature.sharps().unwrap_or(0) - major)
                {
                    raised.set_cause(py, Some(PyKeyError::new_err(missing)));
                }
                raised
            })?;
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

    /// music21's `getScale`: the major or minor scale this signature stands
    /// for. Only those two, as upstream — a signature says nothing about the
    /// other modes.
    #[pyo3(signature = (mode = "major"))]
    fn getScale(&self, py: Python<'_>, mode: Option<&str>) -> PyResult<Py<PyAny>> {
        let scale_type = match mode {
            None | Some("major") => RsScaleType::Major,
            Some("minor") => RsScaleType::Minor,
            Some(other) => {
                return Err(KeySignatureException::new_err(format!(
                    "No mapping to a scale exists for this mode yet: {other}"
                )));
            }
        };
        let key = self
            .signature
            .try_as_key(mode, None)
            .map_err(signature_error)?;
        ConcreteScale::object(py, RsScale::new(scale_type, key.tonic()))
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
    /// music21's `abstract`: the pattern of steps this key reads its degrees
    /// by, when a caller has replaced the one its mode implies.
    ///
    /// The seventh degree of a minor key is a whole tone below the tonic in
    /// the natural form and a semitone below it in the harmonic form, and
    /// music21 lets a caller say which by assigning an `AbstractScale` here.
    /// The object is kept as it was given; which pattern it stands for is
    /// read off its class name.
    abstract_scale: Option<Py<PyAny>>,
    /// The `Pitch` object the key stands on, once something has asked for it
    /// or handed one over. music21 keeps what it was given and hands the
    /// same object back, so `k.tonic = b` leaves `k.tonic is b` true.
    tonic_object: Option<Py<Pitch>>,
    /// A type name a caller wrote over the mode's.
    ///
    /// music21's `type` is a plain attribute — a key writes its own mode
    /// into it — so anything may be written there and comes back.
    named_type: Option<String>,
}

impl Key {
    pub(crate) fn object(py: Python<'_>, inner: RsKey) -> PyResult<Py<PyAny>> {
        let signature = KeySignature::of(inner.sharps());
        let key = Key {
            inner,
            correlation_coefficient: 0.0,
            alternate_interpretations: None,
            abstract_scale: None,
            tonic_object: None,
            named_type: None,
        };
        // Where the class has been installed over music21's, the object has
        // to be one of those: music21 will hold nothing else, and a pickle
        // looking the class up by name finds that one.
        if let Some(class) = crate::installed_class(py, "music21.key", "Key") {
            let object = crate::blank_installed(&class)?;
            {
                let cell = object.cast::<Key>()?;
                let mut me = cell.borrow_mut();
                *me = key;
                *me.into_super() = signature;
            }
            return Ok(object.unbind());
        }
        let init = PyClassInitializer::from(signature).add_subclass(key);
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

    /// The pattern of steps the assigned abstract scale stands for, read off
    /// its class name — music21 names them `AbstractHarmonicMinorScale` and
    /// so on, for the concrete `HarmonicMinorScale` they belong to.
    fn abstract_scale_type(&self, py: Python<'_>) -> Option<RsScaleType> {
        let name: String = self
            .abstract_scale
            .as_ref()?
            .bind(py)
            .get_type()
            .name()
            .ok()?
            .extract()
            .ok()?;
        RsScaleType::from_music21_name(name.strip_prefix("Abstract")?)
    }

    fn same_key(&self, other: &Key) -> bool {
        self.inner.tonic().name() == other.inner.tonic().name()
            && self.inner.mode() == other.inner.mode()
    }
}

#[pymethods]
impl Key {
    /// A key is written out as text and read back, and its signature is made
    /// again from what it says.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let Some(inner) = crate::unpickled::<_, RsKey>(slf, state)? else {
            return Ok(());
        };
        let sharps = inner.sharps();
        let mut me = slf.borrow_mut();
        me.inner = inner;
        me.as_super().signature = RsKeySignature::new(sharps);
        Ok(())
    }

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
                abstract_scale: None,
                tonic_object: None,
                named_type: None,
            }),
        )
    }

    #[getter]
    fn tonic(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<Pitch>> {
        let wanted = slf.borrow().inner.tonic();
        if let Some(object) = &slf.borrow().tonic_object
            && object.borrow(py).inner == wanted
        {
            return Ok(object.clone_ref(py));
        }
        let object =
            crate::installed_new(py, "music21.pitch", "Pitch", Pitch::wrap(wanted, false))?;
        slf.borrow_mut().tonic_object = Some(object.clone_ref(py));
        Ok(object)
    }

    /// Setting it keeps the pitch object given, which is what music21 does:
    /// a key built on a pitch a caller holds reports that very pitch.
    #[setter]
    fn set_tonic(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let name = tonic_name(value)?;
        let mode = slf.borrow().inner.mode().to_string();
        let inner = RsKey::from_tonic_mode(&name, Some(mode.as_str())).map_err(key_error)?;
        let sharps = inner.sharps();
        {
            let mut me = slf.borrow_mut();
            me.inner = inner;
            me.tonic_object = value.extract::<Py<Pitch>>().ok();
        }
        slf.as_super().borrow_mut().signature = RsKeySignature::new(sharps);
        Ok(())
    }

    #[getter]
    fn mode(&self) -> String {
        self.inner.mode().to_string()
    }

    /// music21's `type`, which a key inherits from `ConcreteScale` and
    /// writes its own mode into: a key *is* a scale of that mode.
    #[getter]
    fn get_type(&self) -> String {
        match &self.named_type {
            Some(named) => named.clone(),
            None => self.inner.mode().to_string(),
        }
    }

    #[setter]
    fn set_type(&mut self, value: &str) {
        self.named_type = Some(value.to_string());
    }

    /// music21's `name`, which for a scale is its tonic and its type:
    /// `E- major`.
    #[getter]
    fn name(&self) -> String {
        format!("{} {}", self.inner.tonic().name(), self.get_type())
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

    /// music21's `getScaleDegreeFromPitch`: which degree of this key a pitch
    /// is, or nothing when it is not in it.
    ///
    /// A key *is* a scale upstream — music21's `Key` inherits from
    /// `DiatonicScale` — so the questions a scale answers, it answers too.
    #[pyo3(signature = (pitchTarget, comparisonAttribute = "name", **_keywords))]
    fn getScaleDegreeFromPitch(
        &self,
        pitchTarget: &Bound<'_, PyAny>,
        comparisonAttribute: &str,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Option<usize>> {
        let scale = self.inner.as_scale().map_err(key_error)?;
        let pitch = pitch_from_any(pitchTarget)?;
        scale
            .degree_of_by(&pitch, crate::scale::comparison_of(comparisonAttribute))
            .map_err(key_error)
    }

    /// music21's `getScaleDegreeAndAccidentalFromPitch`: the degree, and how
    /// far the pitch is altered from it.
    #[pyo3(signature = (pitchTarget, **_keywords))]
    fn getScaleDegreeAndAccidentalFromPitch(
        &self,
        py: Python<'_>,
        pitchTarget: &Bound<'_, PyAny>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        let scale = self.inner.as_scale().map_err(key_error)?;
        let (degree, accidental) = scale
            .degree_and_accidental_of(&pitch_from_any(pitchTarget)?)
            .map_err(key_error)?;
        let accidental = match accidental {
            Some(accidental) => crate::pitch::Accidental::from_inner(accidental)
                .into_pyobject(py)?
                .into_any(),
            None => py.None().into_bound(py),
        };
        Ok(
            PyTuple::new(py, [degree.into_pyobject(py)?.into_any(), accidental])?
                .into_any()
                .unbind(),
        )
    }

    /// music21's `solfeg`: the syllable for a pitch's degree in this key.
    #[pyo3(signature = (pitchTarget = None, *, variant = "music21", chromatic = true, **_keywords))]
    fn solfeg(
        &self,
        pitchTarget: Option<&Bound<'_, PyAny>>,
        variant: &str,
        chromatic: bool,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<String> {
        let scale = self.inner.as_scale().map_err(key_error)?;
        let pitch = match pitchTarget.filter(|value| !value.is_none()) {
            Some(value) => pitch_from_any(value)?,
            None => self.inner.tonic(),
        };
        let variant = match variant {
            "humdrum" => RsSolfegVariant::Humdrum,
            _ => RsSolfegVariant::Music21,
        };
        scale.solfeg(&pitch, variant, chromatic).map_err(key_error)
    }

    /// music21's `abstract`: the pattern of steps the key reads its degrees
    /// by, when one has been put here in place of the mode's own.
    #[getter]
    fn get_abstract(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.abstract_scale
            .as_ref()
            .map(|scale| scale.clone_ref(py))
    }

    /// music21 keeps the pattern in a private slot and its own `__init__`
    /// reaches for it, so the slot answers here too.
    #[getter]
    fn _abstract(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.get_abstract(py)
    }

    #[setter]
    fn set__abstract(&mut self, value: Option<&Bound<'_, PyAny>>) {
        self.set_abstract(value);
    }

    #[setter]
    fn set_abstract(&mut self, value: Option<&Bound<'_, PyAny>>) {
        self.abstract_scale = value
            .filter(|value| !value.is_none())
            .map(|value| value.clone().unbind());
    }

    fn deriveByDegree(
        &self,
        py: Python<'_>,
        degree: usize,
        pitchRef: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let pitch = pitch_from_any(pitchRef)?;
        let derived = match self.abstract_scale_type(py) {
            Some(scale_type) => self
                .inner
                .derive_by_degree_of(scale_type, degree, &pitch)
                .map_err(key_error)?,
            None => self
                .inner
                .derive_by_degree(degree, &pitch)
                .map_err(key_error)?,
        };
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
        // A degree past the octave is the same degree again, in the octave
        // the key stands in: music21 asks its network for the node at that
        // degree, and the nodes go round. Its own roman-numeral code counts
        // a thirteenth as degree seventeen and expects the third back.
        let wrapped = degree.checked_sub(1).map_or(degree, |below| below % 7 + 1);
        Ok(Pitch::wrap(
            self.inner.pitch_from_degree(wrapped).map_err(key_error)?,
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
    // Where music21 keeps it once these are installed, since that is the
    // dictionary its own doctest reads back; the facade module's otherwise.
    if let Ok(key) = py.import("music21.key")
        && let Ok(cache) = key.getattr("_sharpsToPitchCache")
        && let Ok(cache) = cache.cast_into::<PyDict>()
    {
        return Ok(cache);
    }
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
