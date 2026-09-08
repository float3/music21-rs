//! music21's `interval` module over `music21-rs`: `Direction`, `Specifier`,
//! `GenericInterval`, `DiatonicInterval`, `ChromaticInterval`, `Interval`
//! and the module functions, with music21's names, arguments and `repr`.

#![allow(non_snake_case)]

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use music21_rs::{
    ChromaticInterval as RsChromatic, DiatonicInterval as RsDiatonic, GenericInterval as RsGeneric,
    Interval as RsInterval, IntervalDirection, KeySignature, Pitch as RsPitch,
    Specifier as RsSpecifier, convert_diatonic_number_to_step,
    convert_semitone_to_specifier_generic, convert_semitone_to_specifier_generic_microtone,
    intervals_to_diatonic, notes_to_chromatic, notes_to_generic, staff_distance_to_generic_number,
};

use crate::pitch::{Pitch, message, pitch_from_any};

/// The names the `interval` facade replaces in `music21.interval`.
pub const NAMES: &[&str] = &[
    "Direction",
    "Specifier",
    "GenericInterval",
    "DiatonicInterval",
    "ChromaticInterval",
    "Interval",
    "IntervalException",
    "convertStaffDistanceToInterval",
    "convertDiatonicNumberToStep",
    "parseSpecifier",
    "convertGeneric",
    "convertSemitoneToSpecifierGenericMicrotone",
    "convertSemitoneToSpecifierGeneric",
    "intervalToPythagoreanRatio",
    "notesToGeneric",
    "notesToChromatic",
    "intervalsToDiatonic",
    "intervalFromGenericAndChromatic",
    "getWrittenHigherNote",
    "getWrittenLowerNote",
    "getAbsoluteHigherNote",
    "getAbsoluteLowerNote",
    "transposePitch",
    "transposeNote",
    "notesToInterval",
    "add",
    "subtract",
];

pyo3::create_exception!(
    music21_rs_facade,
    IntervalException,
    crate::Music21Exception
);

fn interval_error(error: music21_rs::Error) -> PyErr {
    // An accidental this crate cannot spell is music21's `AccidentalException`
    // wherever it is raised from: its own transposition raises one on the
    // fifth sharp, and callers catch that class by name.
    if let Some(specific) = crate::pitch::specific_error(&error) {
        return specific;
    }
    IntervalException::new_err(message(&error))
}

/// Reads a pitch argument together with the facade flag that says whether
/// its spelling was inferred, so a transposed copy can carry it on.
fn pitch_and_inferred(value: &Bound<'_, PyAny>) -> PyResult<(RsPitch, bool)> {
    if let Ok(facade) = value.extract::<PyRef<Pitch>>() {
        return Ok((facade.inner.clone(), facade.spelling_is_inferred));
    }
    Ok((pitch_from_any(value)?, false))
}

/// Hands a transposed pitch back the way music21's `transposePitch` does:
/// written into the argument for `inPlace`, otherwise as a new `Pitch`.
fn deliver_pitch(
    value: &Bound<'_, PyAny>,
    transposed: RsPitch,
    inferred: bool,
    in_place: bool,
) -> PyResult<Option<Pitch>> {
    if in_place {
        if let Ok(mut facade) = value.extract::<PyRefMut<Pitch>>() {
            facade.inner = transposed;
            facade.spelling_is_inferred = inferred;
            return Ok(None);
        }
        // Not one of ours: a music21 module that bound `Pitch` at import
        // time still holds music21's own class, and music21's own
        // `Stream.transpose` hands those in. Write the answer onto it
        // through the names every pitch has.
        value.setattr("name", transposed.name())?;
        value.setattr("octave", transposed.octave())?;
        return Ok(None);
    }
    Ok(Some(Pitch::wrap(transposed, inferred)))
}

fn key_signature_from_any(value: Option<&Bound<'_, PyAny>>) -> PyResult<Option<KeySignature>> {
    match value {
        Some(value) if !value.is_none() => {
            let sharps: i32 = value.getattr("sharps")?.extract()?;
            Ok(Some(KeySignature::new(sharps)))
        }
        _ => Ok(None),
    }
}

/// music21's `interval.Direction`, an `IntEnum` with three members.
#[pyclass(
    name = "Direction",
    module = "music21.interval",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Copy)]
pub struct Direction {
    inner: IntervalDirection,
}

impl Direction {
    pub(crate) fn wrap(inner: IntervalDirection) -> Self {
        Self { inner }
    }

    fn from_int(value: i32) -> PyResult<Self> {
        let inner = match value {
            -1 => IntervalDirection::Descending,
            0 => IntervalDirection::Oblique,
            1 => IntervalDirection::Ascending,
            other => {
                return Err(PyValueError::new_err(format!(
                    "{other} is not a valid Direction"
                )));
            }
        };
        Ok(Self { inner })
    }

    fn python_name(&self) -> &'static str {
        match self.inner {
            IntervalDirection::Descending => "DESCENDING",
            IntervalDirection::Oblique => "OBLIQUE",
            IntervalDirection::Ascending => "ASCENDING",
        }
    }
}

#[pymethods]
impl Direction {
    #[new]
    fn new(value: i32) -> PyResult<Self> {
        Self::from_int(value)
    }

    #[classattr]
    fn DESCENDING() -> Self {
        Self::wrap(IntervalDirection::Descending)
    }

    #[classattr]
    fn OBLIQUE() -> Self {
        Self::wrap(IntervalDirection::Oblique)
    }

    #[classattr]
    fn ASCENDING() -> Self {
        Self::wrap(IntervalDirection::Ascending)
    }

    #[getter]
    fn name(&self) -> &'static str {
        self.python_name()
    }

    #[getter]
    fn value(&self) -> i32 {
        self.inner.as_int()
    }

    fn __int__(&self) -> i32 {
        self.inner.as_int()
    }

    fn __index__(&self) -> i32 {
        self.inner.as_int()
    }

    fn __neg__(&self) -> i32 {
        -self.inner.as_int()
    }

    fn __mul__(&self, other: i32) -> i32 {
        self.inner.as_int() * other
    }

    fn __rmul__(&self, other: i32) -> i32 {
        self.inner.as_int() * other
    }

    fn __hash__(&self) -> isize {
        self.inner.as_int() as isize
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        if let Ok(other) = other.extract::<PyRef<Direction>>() {
            return other.inner == self.inner;
        }
        other
            .extract::<i32>()
            .map(|value| value == self.inner.as_int())
            .unwrap_or(false)
    }

    fn __repr__(&self) -> String {
        format!(
            "<Direction.{}: {}>",
            self.python_name(),
            self.inner.as_int()
        )
    }

    fn __str__(&self) -> String {
        self.inner.as_int().to_string()
    }
}

/// music21's `interval.Specifier`, an `IntEnum` of the eleven qualities.
#[pyclass(
    name = "Specifier",
    module = "music21.interval",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Copy)]
pub struct Specifier {
    inner: RsSpecifier,
}

impl Specifier {
    pub(crate) fn wrap(inner: RsSpecifier) -> Self {
        Self { inner }
    }
}

/// Reads a specifier argument the way `parseSpecifier` does: a `Specifier`,
/// its number, its prefix or its spelled-out name.
pub(crate) fn specifier_from_any(value: &Bound<'_, PyAny>) -> PyResult<RsSpecifier> {
    if let Ok(facade) = value.extract::<PyRef<Specifier>>() {
        return Ok(facade.inner);
    }
    if let Ok(number) = value.extract::<i32>() {
        return RsSpecifier::from_value(number).map_err(interval_error);
    }
    if let Ok(name) = value.extract::<String>() {
        return RsSpecifier::from_name(&name).map_err(interval_error);
    }
    Err(IntervalException::new_err(format!(
        "Cannot find a match for value: {}",
        value.repr()?
    )))
}

#[pymethods]
impl Specifier {
    #[new]
    fn new(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self::wrap(specifier_from_any(value)?))
    }

    #[classattr]
    fn PERFECT() -> Self {
        Self::wrap(RsSpecifier::Perfect)
    }

    #[classattr]
    fn MAJOR() -> Self {
        Self::wrap(RsSpecifier::Major)
    }

    #[classattr]
    fn MINOR() -> Self {
        Self::wrap(RsSpecifier::Minor)
    }

    #[classattr]
    fn AUGMENTED() -> Self {
        Self::wrap(RsSpecifier::Augmented)
    }

    #[classattr]
    fn DIMINISHED() -> Self {
        Self::wrap(RsSpecifier::Diminished)
    }

    #[classattr]
    fn DBLAUG() -> Self {
        Self::wrap(RsSpecifier::DoubleAugmented)
    }

    #[classattr]
    fn DBLDIM() -> Self {
        Self::wrap(RsSpecifier::DoubleDiminished)
    }

    #[classattr]
    fn TRPAUG() -> Self {
        Self::wrap(RsSpecifier::TripleAugmented)
    }

    #[classattr]
    fn TRPDIM() -> Self {
        Self::wrap(RsSpecifier::TripleDiminished)
    }

    #[classattr]
    fn QUADAUG() -> Self {
        Self::wrap(RsSpecifier::QuadrupleAugmented)
    }

    #[classattr]
    fn QUADDIM() -> Self {
        Self::wrap(RsSpecifier::QuadrupleDiminished)
    }

    #[getter]
    fn name(&self) -> &'static str {
        self.inner.python_name()
    }

    #[getter]
    fn value(&self) -> i32 {
        self.inner.value()
    }

    #[getter]
    fn niceName(&self) -> String {
        self.inner.nice_name()
    }

    fn inversion(&self) -> Self {
        Self::wrap(self.inner.inversion())
    }

    fn semitonesAbovePerfect(&self) -> PyResult<i32> {
        self.inner.semitones_above_perfect().map_err(interval_error)
    }

    fn semitonesAboveMajor(&self) -> PyResult<i32> {
        self.inner.semitones_above_major().map_err(interval_error)
    }

    fn __int__(&self) -> i32 {
        self.inner.value()
    }

    fn __index__(&self) -> i32 {
        self.inner.value()
    }

    fn __hash__(&self) -> isize {
        self.inner.value() as isize
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        if let Ok(other) = other.extract::<PyRef<Specifier>>() {
            return other.inner == self.inner;
        }
        other
            .extract::<i32>()
            .map(|value| value == self.inner.value())
            .unwrap_or(false)
    }

    fn __repr__(&self) -> String {
        format!("<Specifier.{}>", self.inner.python_name())
    }

    fn __str__(&self) -> &'static str {
        self.inner.prefix()
    }
}

/// music21's `interval.GenericInterval`.
#[pyclass(
    name = "GenericInterval",
    module = "music21.interval",
    subclass,
    skip_from_py_object
)]
pub struct GenericInterval {
    pub(crate) inner: RsGeneric,
    /// The complement, once it has been asked for.
    complemented: Option<Py<GenericInterval>>,
}

impl GenericInterval {
    pub(crate) fn wrap(inner: RsGeneric) -> Self {
        Self {
            inner,
            complemented: None,
        }
    }
}

impl Clone for GenericInterval {
    /// A copy has not worked its complement out yet, so that its complement
    /// is its own rather than the original's.
    fn clone(&self) -> Self {
        Self::wrap(self.inner.clone())
    }
}

/// Reads a generic interval argument: a `GenericInterval`, a number, or a
/// name such as `"Third"` or `"descending fifth"`.
fn generic_from_any(value: &Bound<'_, PyAny>) -> PyResult<RsGeneric> {
    if let Ok(facade) = value.extract::<PyRef<GenericInterval>>() {
        return Ok(facade.inner.clone());
    }
    if let Ok(number) = value.extract::<i32>() {
        return RsGeneric::new(number).map_err(interval_error);
    }
    if let Ok(name) = value.extract::<String>() {
        return RsGeneric::from_name(&name).map_err(interval_error);
    }
    Err(IntervalException::new_err(format!(
        "Cannot convert {} to an interval.",
        value.repr()?
    )))
}

#[pymethods]
impl GenericInterval {
    /// music21 freezes a score by pickling it, and what this object is lives
    /// in Rust where a pickle cannot see it — so it is written out as text,
    /// and read back into a fresh one of these.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let Some(inner) = crate::unpickled::<_, RsGeneric>(slf, state)? else {
            return Ok(());
        };
        slf.borrow_mut().inner = inner;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (value = None, **_keywords))]
    fn new(
        value: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let inner = match value {
            Some(value) => generic_from_any(value)?,
            None => RsGeneric::new(1).map_err(interval_error)?,
        };
        Ok(Self::wrap(inner))
    }

    #[getter]
    fn get_value(&self) -> i32 {
        self.inner.value()
    }

    #[setter]
    fn set_value(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.inner = generic_from_any(value)?;
        self.complemented = None;
        Ok(())
    }

    #[getter]
    fn get_directed(&self) -> i32 {
        self.inner.directed()
    }

    #[setter]
    fn set_directed(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.set_value(value)
    }

    #[getter]
    fn undirected(&self) -> i32 {
        self.inner.undirected()
    }

    #[getter]
    fn direction(&self) -> Direction {
        Direction::wrap(self.inner.direction())
    }

    #[getter]
    fn isSkip(&self) -> bool {
        self.inner.is_skip()
    }

    #[getter]
    fn isDiatonicStep(&self) -> bool {
        self.inner.is_diatonic_step()
    }

    #[getter]
    fn isStep(&self) -> bool {
        self.inner.is_step()
    }

    #[getter]
    fn isUnison(&self) -> bool {
        self.inner.is_unison()
    }

    #[getter]
    fn simpleUndirected(&self) -> i32 {
        self.inner.simple_undirected()
    }

    #[getter]
    fn semiSimpleUndirected(&self) -> i32 {
        self.inner.semi_simple_undirected()
    }

    #[getter]
    fn undirectedOctaves(&self) -> i32 {
        self.inner.undirected_octaves()
    }

    #[getter]
    fn octaves(&self) -> i32 {
        self.inner.octaves()
    }

    #[getter]
    fn simpleDirected(&self) -> i32 {
        self.inner.simple_directed()
    }

    #[getter]
    fn semiSimpleDirected(&self) -> i32 {
        self.inner.semi_simple_directed()
    }

    #[getter]
    fn perfectable(&self) -> bool {
        self.inner.is_perfectable()
    }

    #[getter]
    fn niceName(&self) -> String {
        self.inner.nice_name()
    }

    #[getter]
    fn simpleNiceName(&self) -> String {
        self.inner.simple_nice_name()
    }

    #[getter]
    fn semiSimpleNiceName(&self) -> String {
        self.inner.semi_simple_nice_name()
    }

    #[getter]
    fn staffDistance(&self) -> i32 {
        self.inner.staff_distance()
    }

    #[getter]
    fn mod7inversion(&self) -> i32 {
        self.inner.mod7_inversion()
    }

    #[getter]
    fn mod7(&self) -> i32 {
        self.inner.mod7()
    }

    /// music21's `complement`, which hands back the same object every time
    /// it is asked — `fourth.complement() is fourth.complement()` is one of
    /// its own examples. The answer only depends on the interval, and the
    /// interval can change, so the one that was worked out is thrown away
    /// when it does.
    fn complement(&mut self, py: Python<'_>) -> PyResult<Py<Self>> {
        if self.complemented.is_none() {
            self.complemented = Some(crate::installed_new(
                py,
                "music21.interval",
                "GenericInterval",
                Self::wrap(self.inner.complement()),
            )?);
        }
        Ok(self
            .complemented
            .as_ref()
            .expect("just set when missing")
            .clone_ref(py))
    }

    fn reverse(&self) -> Self {
        Self::wrap(self.inner.reverse())
    }

    #[pyo3(signature = (p, *, inPlace = false))]
    fn transposePitch(&self, p: &Bound<'_, PyAny>, inPlace: bool) -> PyResult<Option<Pitch>> {
        let (pitch, inferred) = pitch_and_inferred(p)?;
        let transposed = self.inner.transpose_pitch(&pitch).map_err(interval_error)?;
        deliver_pitch(p, transposed, inferred, inPlace)
    }

    #[pyo3(signature = (p, k = None, *, inPlace = false))]
    fn transposePitchKeyAware(
        &self,
        p: &Bound<'_, PyAny>,
        k: Option<&Bound<'_, PyAny>>,
        inPlace: bool,
    ) -> PyResult<Option<Pitch>> {
        let (pitch, inferred) = pitch_and_inferred(p)?;
        let key_signature = key_signature_from_any(k)?;
        let transposed = self
            .inner
            .transpose_pitch_key_aware(&pitch, key_signature.as_ref())
            .map_err(interval_error)?;
        deliver_pitch(p, transposed, inferred, inPlace)
    }

    fn getDiatonic(&self, specifier: &Bound<'_, PyAny>) -> PyResult<DiatonicInterval> {
        let specifier = specifier_from_any(specifier)?;
        RsDiatonic::try_new(specifier, self.inner.clone())
            .map(DiatonicInterval::wrap)
            .map_err(interval_error)
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<GenericInterval>>()
            .map(|other| other.inner.value() == self.inner.value())
            .unwrap_or(false)
    }

    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        slf.as_ptr() as isize >> 4
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        Ok(format!(
            "<music21.interval.{} {}>",
            slf.get_type().qualname()?,
            slf.borrow().inner
        ))
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.borrow().clone();
        crate::copy_as_same_type(slf, copied)
    }
}

/// music21's `interval.DiatonicInterval`.
#[pyclass(
    name = "DiatonicInterval",
    module = "music21.interval",
    subclass,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct DiatonicInterval {
    pub(crate) inner: RsDiatonic,
}

impl DiatonicInterval {
    pub(crate) fn wrap(inner: RsDiatonic) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl DiatonicInterval {
    /// music21 freezes a score by pickling it, and what this object is lives
    /// in Rust where a pickle cannot see it — so it is written out as text,
    /// and read back into a fresh one of these.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let Some(inner) = crate::unpickled::<_, RsDiatonic>(slf, state)? else {
            return Ok(());
        };
        slf.borrow_mut().inner = inner;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (specifier = None, generic = None, **_keywords))]
    fn new(
        specifier: Option<&Bound<'_, PyAny>>,
        generic: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let generic = match generic {
            Some(generic) => generic_from_any(generic)?,
            None => RsGeneric::new(1).map_err(interval_error)?,
        };
        let specifier = match specifier {
            Some(specifier) => specifier_from_any(specifier)?,
            None => RsSpecifier::Perfect,
        };
        RsDiatonic::try_new(specifier, generic)
            .map(Self::wrap)
            .map_err(interval_error)
    }

    #[getter]
    fn generic(&self) -> GenericInterval {
        GenericInterval::wrap(self.inner.generic().clone())
    }

    #[getter]
    fn specifier(&self) -> Specifier {
        Specifier::wrap(self.inner.specifier())
    }

    #[getter]
    fn name(&self) -> String {
        self.inner.name()
    }

    #[getter]
    fn niceName(&self) -> String {
        self.inner.nice_name()
    }

    #[getter]
    fn specificName(&self) -> String {
        self.inner.specific_name()
    }

    #[getter]
    fn simpleName(&self) -> String {
        self.inner.simple_name()
    }

    #[getter]
    fn simpleNiceName(&self) -> String {
        self.inner.simple_nice_name()
    }

    #[getter]
    fn semiSimpleName(&self) -> String {
        self.inner.semi_simple_name()
    }

    #[getter]
    fn semiSimpleNiceName(&self) -> String {
        self.inner.semi_simple_nice_name()
    }

    #[getter]
    fn direction(&self) -> Direction {
        Direction::wrap(self.inner.direction())
    }

    #[getter]
    fn directedName(&self) -> String {
        self.inner.directed_name()
    }

    #[getter]
    fn directedNiceName(&self) -> String {
        self.inner.directed_nice_name()
    }

    #[getter]
    fn directedSimpleName(&self) -> String {
        self.inner.directed_simple_name()
    }

    #[getter]
    fn directedSimpleNiceName(&self) -> String {
        self.inner.directed_simple_nice_name()
    }

    #[getter]
    fn directedSemiSimpleName(&self) -> String {
        self.inner.directed_semi_simple_name()
    }

    #[getter]
    fn directedSemiSimpleNiceName(&self) -> String {
        self.inner.directed_semi_simple_nice_name()
    }

    #[getter]
    fn isStep(&self) -> bool {
        self.inner.is_step()
    }

    #[getter]
    fn isDiatonicStep(&self) -> bool {
        self.inner.is_diatonic_step()
    }

    #[getter]
    fn isSkip(&self) -> bool {
        self.inner.is_skip()
    }

    #[getter]
    fn perfectable(&self) -> bool {
        self.inner.is_perfectable()
    }

    #[getter]
    fn mod7inversion(&self) -> String {
        self.inner.mod7_inversion()
    }

    #[getter]
    fn mod7(&self) -> String {
        self.inner.mod7()
    }

    #[getter]
    fn specifierAbbreviation(&self) -> &'static str {
        self.inner.specifier().prefix()
    }

    #[getter]
    fn cents(&self) -> PyResult<f64> {
        self.inner.cents().map_err(interval_error)
    }

    fn reverse(&self) -> Self {
        Self::wrap(self.inner.reverse())
    }

    fn getChromatic(&self) -> PyResult<ChromaticInterval> {
        self.inner
            .get_chromatic()
            .map(ChromaticInterval::wrap)
            .map_err(interval_error)
    }

    #[pyo3(signature = (p, *, inPlace = false))]
    fn transposePitch(&self, p: &Bound<'_, PyAny>, inPlace: bool) -> PyResult<Option<Pitch>> {
        let (pitch, inferred) = pitch_and_inferred(p)?;
        let transposed = self.inner.transpose_pitch(&pitch).map_err(interval_error)?;
        deliver_pitch(p, transposed, inferred, inPlace)
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<DiatonicInterval>>()
            .map(|other| other.inner == self.inner)
            .unwrap_or(false)
    }

    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        slf.as_ptr() as isize >> 4
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        Ok(format!(
            "<music21.interval.{} {}>",
            slf.get_type().qualname()?,
            slf.borrow().inner.name()
        ))
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.borrow().clone();
        crate::copy_as_same_type(slf, copied)
    }
}

/// music21's `interval.ChromaticInterval`.
#[pyclass(
    name = "ChromaticInterval",
    module = "music21.interval",
    subclass,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct ChromaticInterval {
    pub(crate) inner: RsChromatic,
}

impl ChromaticInterval {
    pub(crate) fn wrap(inner: RsChromatic) -> Self {
        Self { inner }
    }
}

fn semitones_from_any(value: &Bound<'_, PyAny>) -> PyResult<f64> {
    if let Ok(semitones) = value.extract::<i32>() {
        return Ok(f64::from(semitones));
    }
    value.extract()
}

/// A semitone count as music21 hands it back: an `int` when it is whole, a
/// `float` when it carries a microtone.
fn semitone_object(py: Python<'_>, semitones: f64) -> PyResult<Py<PyAny>> {
    if semitones.fract() == 0.0 {
        Ok((semitones as i64).into_pyobject(py)?.into_any().unbind())
    } else {
        Ok(semitones.into_pyobject(py)?.into_any().unbind())
    }
}

#[pymethods]
impl ChromaticInterval {
    /// music21 freezes a score by pickling it, and what this object is lives
    /// in Rust where a pickle cannot see it — so it is written out as text,
    /// and read back into a fresh one of these.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let Some(inner) = crate::unpickled::<_, RsChromatic>(slf, state)? else {
            return Ok(());
        };
        slf.borrow_mut().inner = inner;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (semitones = None, **_keywords))]
    fn new(
        semitones: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let semitones = match semitones {
            Some(value) => semitones_from_any(value)?,
            None => 0.0,
        };
        Ok(Self::wrap(RsChromatic::new(semitones)))
    }

    #[getter]
    fn get_semitones(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        semitone_object(py, self.inner.semitones())
    }

    #[setter]
    fn set_semitones(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.inner = RsChromatic::new(semitones_from_any(value)?);
        Ok(())
    }

    #[getter]
    fn cents(&self) -> f64 {
        self.inner.cents()
    }

    #[getter]
    fn directed(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        semitone_object(py, self.inner.directed())
    }

    #[getter]
    fn undirected(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        semitone_object(py, self.inner.undirected())
    }

    #[getter]
    fn direction(&self) -> Direction {
        Direction::wrap(self.inner.direction())
    }

    #[getter]
    fn mod12(&self) -> i32 {
        self.inner.mod12()
    }

    #[getter]
    fn simpleUndirected(&self) -> i32 {
        self.inner.simple_undirected()
    }

    #[getter]
    fn simpleDirected(&self) -> i32 {
        self.inner.simple_directed()
    }

    #[getter]
    fn intervalClass(&self) -> i32 {
        self.inner.interval_class()
    }

    #[getter]
    fn isChromaticStep(&self) -> bool {
        self.inner.is_chromatic_step()
    }

    #[getter]
    fn isStep(&self) -> bool {
        self.inner.is_step()
    }

    fn reverse(&self) -> Self {
        Self::wrap(self.inner.reverse())
    }

    fn getDiatonic(&self) -> DiatonicInterval {
        DiatonicInterval::wrap(self.inner.get_diatonic())
    }

    #[pyo3(signature = (p, *, inPlace = false))]
    fn transposePitch(&self, p: &Bound<'_, PyAny>, inPlace: bool) -> PyResult<Option<Pitch>> {
        let (pitch, _) = pitch_and_inferred(p)?;
        let transposed = self.inner.transpose_pitch(&pitch).map_err(interval_error)?;
        deliver_pitch(p, transposed, true, inPlace)
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<ChromaticInterval>>()
            .map(|other| other.inner == self.inner)
            .unwrap_or(false)
    }

    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        slf.as_ptr() as isize >> 4
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        Ok(format!(
            "<music21.interval.{} {}>",
            slf.get_type().qualname()?,
            slf.borrow().inner.directed()
        ))
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.borrow().clone();
        crate::copy_as_same_type(slf, copied)
    }
}

/// music21's `interval.Interval`.
#[pyclass(
    name = "Interval",
    module = "music21.interval",
    subclass,
    skip_from_py_object
)]
pub struct Interval {
    pub(crate) inner: RsInterval,
    pitch_start: Option<Py<PyAny>>,
    pitch_end: Option<Py<PyAny>>,
    interval_type: String,
}

impl Interval {
    pub(crate) fn wrap(inner: RsInterval) -> Self {
        Self {
            inner,
            pitch_start: None,
            pitch_end: None,
            interval_type: String::new(),
        }
    }

    /// The same, spanning two objects the caller already holds, so that
    /// `interval.noteStart is n1`.
    pub(crate) fn between(
        inner: RsInterval,
        start: Py<PyAny>,
        end: Py<PyAny>,
        interval_type: &str,
    ) -> Self {
        Self {
            inner,
            pitch_start: Some(start),
            pitch_end: Some(end),
            interval_type: interval_type.to_string(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        arg0: Option<&Bound<'_, PyAny>>,
        arg1: Option<&Bound<'_, PyAny>>,
        diatonic: Option<PyRef<'_, DiatonicInterval>>,
        chromatic: Option<PyRef<'_, ChromaticInterval>>,
        pitchStart: Option<&Bound<'_, PyAny>>,
        pitchEnd: Option<&Bound<'_, PyAny>>,
        noteStart: Option<&Bound<'_, PyAny>>,
        noteEnd: Option<&Bound<'_, PyAny>>,
        name: Option<String>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let mut name = name;
        let mut chromatic = chromatic.map(|value| value.inner.clone());
        let mut diatonic = diatonic.map(|value| value.inner.clone());
        let mut pitch_start = optional_pitch_object(pitchStart)?;
        let mut pitch_end = optional_pitch_object(pitchEnd)?;
        let note_start = optional_pitch_object(noteStart)?;
        let note_end = optional_pitch_object(noteEnd)?;

        if let Some(arg1) = arg1.filter(|value| !value.is_none()) {
            if arg0.is_none_or(|value| value.is_none()) {
                return Err(PyValueError::new_err(
                    "Cannot supply a second value without a first.",
                ));
            }
            pitch_end = Some(extract_pitch_object(arg1)?);
        }
        if let Some(arg0) = arg0.filter(|value| !value.is_none()) {
            if let Ok(text) = arg0.extract::<String>() {
                name = Some(text);
            } else if arg0.extract::<i32>().is_ok() || arg0.extract::<f64>().is_ok() {
                chromatic = Some(RsChromatic::new(semitones_from_any(arg0)?));
            } else {
                pitch_start = Some(extract_pitch_object(arg0)?);
            }
        }
        if (note_start.is_some() && pitch_start.is_some())
            || (note_end.is_some() && pitch_end.is_some())
        {
            return Err(PyValueError::new_err(
                "Cannot instantiate an interval with both notes and pitches.",
            ));
        }
        if note_start.is_some() {
            pitch_start = note_start;
        }
        if note_end.is_some() {
            pitch_end = note_end;
        }
        if pitch_start.is_some() != pitch_end.is_some() {
            return Err(PyValueError::new_err(
                "either both the starting and the ending pitch (or note) must be \
                 given or neither can be given.  You cannot have one without the other.",
            ));
        }

        let mut implicit_diatonic = false;
        if let (Some(start), Some(end)) = (&pitch_start, &pitch_end) {
            let between =
                RsInterval::between_pitches(&pitch_from_any(start)?, &pitch_from_any(end)?)
                    .map_err(interval_error)?;
            if (chromatic.is_some() || diatonic.is_some())
                && chromatic.as_ref() != Some(between.chromatic())
            {
                return Err(PyValueError::new_err(
                    "Do not pass in pitches/notes and diatonic/chromatic \
                     interval objects, unless they represent the same interval.",
                ));
            }
            chromatic = Some(between.chromatic().clone());
            diatonic = Some(between.diatonic().clone());
        }
        if let Some(name) = name {
            let named = RsInterval::from_name(name).map_err(interval_error)?;
            if (chromatic.is_some() || diatonic.is_some())
                && (chromatic.as_ref() != Some(named.chromatic())
                    || diatonic.as_ref() != Some(named.diatonic()))
            {
                return Err(PyValueError::new_err(
                    "Do not pass in a name and pitches/notes or diatonic/chromatic \
                     interval objects, unless they represent the same interval.",
                ));
            }
            implicit_diatonic = named.is_implicit_diatonic();
            chromatic = Some(named.chromatic().clone());
            diatonic = Some(named.diatonic().clone());
        }

        let inner = match (diatonic, chromatic) {
            (Some(diatonic), Some(chromatic)) => {
                let mut interval = RsInterval::from_diatonic_and_chromatic(diatonic, chromatic)
                    .map_err(interval_error)?;
                if implicit_diatonic {
                    interval = RsInterval::from_chromatic(interval.chromatic().clone())
                        .map_err(interval_error)?;
                }
                interval
            }
            (None, Some(chromatic)) => {
                RsInterval::from_chromatic(chromatic).map_err(interval_error)?
            }
            (Some(diatonic), None) => {
                RsInterval::from_diatonic(diatonic).map_err(interval_error)?
            }
            (None, None) => PERFECT_UNISON.clone(),
        };
        Ok(Self {
            inner,
            pitch_start: pitch_start.map(Bound::unbind),
            pitch_end: pitch_end.map(Bound::unbind),
            interval_type: String::new(),
        })
    }

    fn with_pitches(inner: RsInterval, start: &Bound<'_, PyAny>, end: &Bound<'_, PyAny>) -> Self {
        Self {
            inner,
            pitch_start: Some(start.clone().unbind()),
            pitch_end: Some(end.clone().unbind()),
            interval_type: String::new(),
        }
    }
}

/// The interval every `Interval` starts as, before `__init__` fills it in.
/// Parsed once: `__new__` runs on every construction, and parsing a name is
/// a thousand times the cost of cloning one.
static PERFECT_UNISON: std::sync::LazyLock<RsInterval> =
    std::sync::LazyLock::new(|| RsInterval::from_name("P1").expect("a perfect unison"));

/// The pitch music21's `_extractPitch` reads off a note or pitch argument:
/// the note's `pitch` when it has one, otherwise the object itself.
fn extract_pitch_object<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if value.extract::<PyRef<Pitch>>().is_ok() {
        return Ok(value.clone());
    }
    match value.getattr("pitch") {
        Ok(pitch) => Ok(pitch),
        Err(_) => Ok(value.clone()),
    }
}

fn optional_pitch_object<'py>(
    value: Option<&Bound<'py, PyAny>>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    match value {
        Some(value) if !value.is_none() => Ok(Some(extract_pitch_object(value)?)),
        _ => Ok(None),
    }
}

/// Reads an interval argument the way music21's transposition helpers do:
/// an `Interval`, one of its halves, a name, a semitone count, or any object
/// carrying music21's `directedName`.
pub(crate) fn interval_from_any(value: &Bound<'_, PyAny>) -> PyResult<RsInterval> {
    if let Ok(facade) = value.extract::<PyRef<Interval>>() {
        return Ok(facade.inner.clone());
    }
    if let Ok(facade) = value.extract::<PyRef<DiatonicInterval>>() {
        return RsInterval::from_diatonic(facade.inner.clone()).map_err(interval_error);
    }
    if let Ok(facade) = value.extract::<PyRef<ChromaticInterval>>() {
        return RsInterval::from_chromatic(facade.inner.clone()).map_err(interval_error);
    }
    if let Ok(name) = value.extract::<String>() {
        return RsInterval::from_name(name).map_err(interval_error);
    }
    if let Ok(semitones) = value.extract::<i32>() {
        return RsInterval::from_semitones(semitones).map_err(interval_error);
    }
    // A number of semitones written with a decimal point, which music21
    // reads as a microtonal interval: its own analysis transposes by one.
    if let Ok(semitones) = value.extract::<f64>() {
        return RsInterval::from_chromatic(RsChromatic::new(semitones)).map_err(interval_error);
    }
    if let Ok(name) = value
        .getattr("directedName")
        .and_then(|name| name.extract::<String>())
    {
        return RsInterval::from_name(name).map_err(interval_error);
    }
    Err(IntervalException::new_err(format!(
        "cannot read an interval from {}",
        value.repr()?
    )))
}

/// Transposes a pitch by whichever interval object music21's
/// `Pitch.transpose` accepts. A `GenericInterval` keeps the accidental, a
/// `ChromaticInterval` respells from pitch space, and anything else goes
/// through a full `Interval`.
pub(crate) fn transpose_pitch_by_any(
    pitch: &RsPitch,
    value: &Bound<'_, PyAny>,
) -> PyResult<RsPitch> {
    if let Ok(generic) = value.extract::<PyRef<GenericInterval>>() {
        return generic.inner.transpose_pitch(pitch).map_err(interval_error);
    }
    if let Ok(chromatic) = value.extract::<PyRef<ChromaticInterval>>() {
        return chromatic
            .inner
            .transpose_pitch(pitch)
            .map_err(interval_error);
    }
    let interval = interval_from_any(value)?;
    pitch.transpose(&interval).map_err(interval_error)
}

#[pymethods]
impl Interval {
    /// music21 freezes a score by pickling it, and what this object is lives
    /// in Rust where a pickle cannot see it — so it is written out as text,
    /// and read back into a fresh one of these.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let Some(inner) = crate::unpickled::<_, RsInterval>(slf, state)? else {
            return Ok(());
        };
        slf.borrow_mut().inner = inner;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (
        arg0 = None,
        arg1 = None,
        /,
        *,
        diatonic = None,
        chromatic = None,
        pitchStart = None,
        pitchEnd = None,
        noteStart = None,
        noteEnd = None,
        name = None,
        **_keywords
    ))]
    #[allow(clippy::too_many_arguments)]
    /// music21 builds its objects in `__init__`; pyo3 builds them in
    /// `__new__`. A Python subclass of a music21 class constructs the base
    /// with *its own* arguments and then calls `super().__init__` with
    /// music21's — `Harte('C:maj')` reaches `Chord.__init__(pitches)` — so
    /// the work belongs in `__init__`, which is where music21 does it and
    /// where a subclass can reach it. `__new__` only has to hand back an
    /// object for `__init__` to fill in; doing the work here as well would
    /// do it twice for every direct caller.
    fn new(
        arg0: Option<&Bound<'_, PyAny>>,
        arg1: Option<&Bound<'_, PyAny>>,
        diatonic: Option<PyRef<'_, DiatonicInterval>>,
        chromatic: Option<PyRef<'_, ChromaticInterval>>,
        pitchStart: Option<&Bound<'_, PyAny>>,
        pitchEnd: Option<&Bound<'_, PyAny>>,
        noteStart: Option<&Bound<'_, PyAny>>,
        noteEnd: Option<&Bound<'_, PyAny>>,
        name: Option<String>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> Self {
        let _ = (
            arg0, arg1, diatonic, chromatic, pitchStart, pitchEnd, noteStart, noteEnd, name,
        );
        let _ = _keywords;
        Self::wrap(PERFECT_UNISON.clone())
    }

    /// music21's construction proper, which a direct caller reaches through
    /// `__new__` having already run it and a subclass reaches through
    /// `super().__init__`.
    #[pyo3(signature = (
        arg0 = None,
        arg1 = None,
        /,
        *,
        diatonic = None,
        chromatic = None,
        pitchStart = None,
        pitchEnd = None,
        noteStart = None,
        noteEnd = None,
        name = None,
        **_keywords
    ))]
    #[allow(clippy::too_many_arguments)]
    fn __init__(
        slf: &Bound<'_, Self>,
        arg0: Option<&Bound<'_, PyAny>>,
        arg1: Option<&Bound<'_, PyAny>>,
        diatonic: Option<PyRef<'_, DiatonicInterval>>,
        chromatic: Option<PyRef<'_, ChromaticInterval>>,
        pitchStart: Option<&Bound<'_, PyAny>>,
        pitchEnd: Option<&Bound<'_, PyAny>>,
        noteStart: Option<&Bound<'_, PyAny>>,
        noteEnd: Option<&Bound<'_, PyAny>>,
        name: Option<String>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        *slf.borrow_mut() = Self::build(
            arg0, arg1, diatonic, chromatic, pitchStart, pitchEnd, noteStart, noteEnd, name,
            _keywords,
        )?;
        Ok(())
    }

    #[getter]
    fn generic(&self) -> GenericInterval {
        GenericInterval::wrap(self.inner.generic().clone())
    }

    #[getter]
    fn diatonic(&self) -> DiatonicInterval {
        DiatonicInterval::wrap(self.inner.diatonic().clone())
    }

    #[getter]
    fn chromatic(&self) -> ChromaticInterval {
        ChromaticInterval::wrap(self.inner.chromatic().clone())
    }

    #[getter]
    fn implicitDiatonic(&self) -> bool {
        self.inner.is_implicit_diatonic()
    }

    #[getter]
    fn get_intervalType(&self) -> String {
        self.interval_type.clone()
    }

    #[setter]
    fn set_intervalType(&mut self, value: String) {
        self.interval_type = value;
    }

    #[getter]
    fn name(&self) -> String {
        self.inner.short_name()
    }

    #[getter]
    fn niceName(&self) -> String {
        self.inner.name()
    }

    #[getter]
    fn simpleName(&self) -> String {
        self.inner.simple_name()
    }

    #[getter]
    fn simpleNiceName(&self) -> String {
        self.inner.simple_nice_name()
    }

    #[getter]
    fn semiSimpleName(&self) -> String {
        self.inner.semi_simple_name()
    }

    #[getter]
    fn semiSimpleNiceName(&self) -> String {
        self.inner.semi_simple_nice_name()
    }

    #[getter]
    fn directedName(&self) -> String {
        self.inner.directed_name()
    }

    #[getter]
    fn directedNiceName(&self) -> String {
        self.inner.directed_nice_name()
    }

    #[getter]
    fn directedSimpleName(&self) -> String {
        self.inner.directed_simple_name()
    }

    #[getter]
    fn directedSimpleNiceName(&self) -> String {
        self.inner.directed_simple_nice_name()
    }

    #[getter]
    fn semitones(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        semitone_object(py, self.inner.semitones())
    }

    #[getter]
    fn direction(&self) -> Direction {
        Direction::wrap(self.inner.direction())
    }

    #[getter]
    fn specifier(&self) -> Specifier {
        Specifier::wrap(self.inner.specifier())
    }

    #[getter]
    fn specificName(&self) -> String {
        self.inner.specific_name()
    }

    #[getter]
    fn isDiatonicStep(&self) -> bool {
        self.inner.is_diatonic_step()
    }

    #[getter]
    fn isChromaticStep(&self) -> bool {
        self.inner.is_chromatic_step()
    }

    #[getter]
    fn isStep(&self) -> bool {
        self.inner.is_step()
    }

    #[getter]
    fn isSkip(&self) -> bool {
        self.inner.is_skip()
    }

    fn isConsonant(&self) -> bool {
        self.inner.is_consonant()
    }

    #[getter]
    fn complement(&self) -> PyResult<Self> {
        self.inner
            .complement()
            .map(Self::wrap)
            .map_err(interval_error)
    }

    #[getter]
    fn intervalClass(&self) -> i32 {
        self.inner.interval_class()
    }

    #[getter]
    fn cents(&self) -> f64 {
        self.inner.cents()
    }

    #[pyo3(signature = (p, *, reverse = false, maxAccidental = 4, inPlace = false))]
    fn transposePitch(
        &self,
        p: &Bound<'_, PyAny>,
        reverse: bool,
        maxAccidental: Option<i32>,
        inPlace: bool,
    ) -> PyResult<Option<Pitch>> {
        let (pitch, inferred) = pitch_and_inferred(p)?;
        let transposed = self
            .inner
            .transpose_pitch_with_options(&pitch, reverse, maxAccidental)
            .map_err(interval_error)?;
        let inferred = if self.inner.is_implicit_diatonic() {
            true
        } else {
            inferred
        };
        deliver_pitch(p, transposed, inferred, inPlace)
    }

    fn reverse(&self, py: Python<'_>) -> PyResult<Self> {
        if let (Some(start), Some(end)) = (&self.pitch_start, &self.pitch_end) {
            let (start, end) = (start.bind(py), end.bind(py));
            let inner = RsInterval::between_pitches(&pitch_from_any(end)?, &pitch_from_any(start)?)
                .map_err(interval_error)?;
            return Ok(Self::with_pitches(inner, end, start));
        }
        RsInterval::from_diatonic_and_chromatic(
            self.inner.diatonic().reverse(),
            self.inner.chromatic().reverse(),
        )
        .map(Self::wrap)
        .map_err(interval_error)
    }

    #[getter]
    fn get_pitchStart(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.pitch_start.as_ref().map(|pitch| pitch.clone_ref(py))
    }

    #[setter]
    fn set_pitchStart(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        match optional_pitch_object(value)? {
            None => {
                self.pitch_start = None;
                self.pitch_end = None;
            }
            Some(start) => {
                let (pitch, inferred) = pitch_and_inferred(&start)?;
                let end = self
                    .inner
                    .transpose_pitch_with_options(&pitch, false, Some(4))
                    .map_err(interval_error)?;
                let end = Bound::new(start.py(), Pitch::wrap(end, inferred))?;
                self.pitch_start = Some(start.unbind());
                self.pitch_end = Some(end.into_any().unbind());
            }
        }
        Ok(())
    }

    #[getter]
    fn get_pitchEnd(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.pitch_end.as_ref().map(|pitch| pitch.clone_ref(py))
    }

    #[setter]
    fn set_pitchEnd(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        match optional_pitch_object(value)? {
            None => {
                self.pitch_start = None;
                self.pitch_end = None;
            }
            Some(end) => {
                let (pitch, inferred) = pitch_and_inferred(&end)?;
                let start = self
                    .inner
                    .transpose_pitch_with_options(&pitch, true, Some(4))
                    .map_err(interval_error)?;
                let start = Bound::new(end.py(), Pitch::wrap(start, inferred))?;
                self.pitch_start = Some(start.into_any().unbind());
                self.pitch_end = Some(end.unbind());
            }
        }
        Ok(())
    }

    #[getter]
    fn get_noteStart(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.pitch_start
            .as_ref()
            .map(|pitch| note_around(pitch.bind(py)))
            .transpose()
    }

    /// music21's `noteStart` setter, which is its `pitchStart` setter given
    /// the note's pitch.
    #[setter]
    fn set_noteStart(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let pitch = match value.filter(|value| !value.is_none()) {
            Some(note) => Some(note.getattr("pitch")?),
            None => None,
        };
        self.set_pitchStart(pitch.as_ref())
    }

    #[setter]
    fn set_noteEnd(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let pitch = match value.filter(|value| !value.is_none()) {
            Some(note) => Some(note.getattr("pitch")?),
            None => None,
        };
        self.set_pitchEnd(pitch.as_ref())
    }

    #[getter]
    fn get_noteEnd(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.pitch_end
            .as_ref()
            .map(|pitch| note_around(pitch.bind(py)))
            .transpose()
    }

    /// music21's `transposeNote`: a copy of the note with its pitch moved.
    fn transposeNote<'py>(&self, note1: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        transpose_note_by(note1, |pitch| {
            self.inner
                .transpose_pitch_with_options(pitch, false, Some(4))
                .map_err(interval_error)
        })
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<Interval>>()
            .map(|other| {
                other.inner.diatonic() == self.inner.diatonic()
                    && other.inner.chromatic() == self.inner.chromatic()
            })
            .unwrap_or(false)
    }

    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        slf.as_ptr() as isize >> 4
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        Ok(format!(
            "<music21.interval.{} {}>",
            slf.get_type().qualname()?,
            slf.borrow().inner
        ))
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        py: Python<'py>,
        _memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let me = slf.borrow();
        let copied = Self {
            inner: me.inner.clone(),
            pitch_start: me.pitch_start.as_ref().map(|p| p.clone_ref(py)),
            pitch_end: me.pitch_end.as_ref().map(|p| p.clone_ref(py)),
            interval_type: me.interval_type.clone(),
        };
        drop(me);
        crate::copy_as_same_type(slf, copied)
    }
}

/// The note a pitch belongs to, the way the `noteStart` and `noteEnd`
/// properties find it upstream: the note that owns it if it has one, and a
/// note wrapped round it if it does not.
fn note_around(pitch: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let py = pitch.py();
    if let Ok(client) = pitch.getattr("_client")
        && !client.is_none()
    {
        return Ok(client.unbind());
    }
    let note = py.import("music21.note")?.getattr("Note")?.call0()?;
    note.setattr("pitch", pitch)?;
    Ok(note.unbind())
}

/// A deep copy of a music21 note with its pitch replaced by the transposed
/// one: the shape of every `transposeNote` upstream.
fn transpose_note_by<'py>(
    note1: &Bound<'py, PyAny>,
    transpose: impl Fn(&RsPitch) -> PyResult<RsPitch>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = note1.py();
    let pitch_object = note1.getattr("pitch")?;
    let (pitch, inferred) = pitch_and_inferred(&pitch_object)?;
    let transposed = Pitch::wrap(transpose(&pitch)?, inferred);
    let copy = py.import("copy")?.getattr("deepcopy")?.call1((note1,))?;
    copy.setattr("pitch", Bound::new(py, transposed)?)?;
    Ok(copy)
}

#[pyfunction]
#[pyo3(name = "convertStaffDistanceToInterval")]
fn convert_staff_distance_to_interval(staffDist: i32) -> i32 {
    staff_distance_to_generic_number(staffDist)
}

#[pyfunction]
#[pyo3(name = "convertDiatonicNumberToStep")]
fn convert_diatonic_number_to_step_py(dn: i32) -> (String, i32) {
    let (step, octave) = convert_diatonic_number_to_step(dn);
    (step.to_string(), octave)
}

#[pyfunction]
#[pyo3(name = "parseSpecifier")]
fn parse_specifier(value: &Bound<'_, PyAny>) -> PyResult<Specifier> {
    if value.is_none() {
        return Err(PyValueError::new_err(
            "Value None must be int, str, or Specifier",
        ));
    }
    Ok(Specifier::wrap(specifier_from_any(value)?))
}

#[pyfunction]
#[pyo3(name = "convertGeneric")]
fn convert_generic(value: &Bound<'_, PyAny>) -> PyResult<i32> {
    if value.is_none() {
        return Err(IntervalException::new_err(
            "Cannot get a direction from None.",
        ));
    }
    Ok(generic_from_any(value)?.value())
}

#[pyfunction]
#[pyo3(name = "convertSemitoneToSpecifierGenericMicrotone")]
fn convert_semitone_to_specifier_generic_microtone_py(count: f64) -> (Specifier, i32, f64) {
    let (specifier, generic, cents) = convert_semitone_to_specifier_generic_microtone(count);
    (Specifier::wrap(specifier), generic, cents)
}

#[pyfunction]
#[pyo3(name = "convertSemitoneToSpecifierGeneric")]
fn convert_semitone_to_specifier_generic_py(
    count: &Bound<'_, PyAny>,
) -> PyResult<(Specifier, i32)> {
    let (specifier, generic) = convert_semitone_to_specifier_generic(semitones_from_any(count)?);
    Ok((Specifier::wrap(specifier), generic))
}

#[pyfunction]
#[pyo3(name = "intervalToPythagoreanRatio")]
fn interval_to_pythagorean_ratio<'py>(
    py: Python<'py>,
    intervalObj: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let interval = interval_from_any(intervalObj)?;
    let no_ratio = || {
        IntervalException::new_err(format!(
            "Could not find a pythagorean ratio for <music21.interval.Interval {interval}>."
        ))
    };
    let ratio = interval.pythagorean_ratio().map_err(|_| no_ratio())?;
    let numer = *ratio.numer().ok_or_else(no_ratio)?;
    let denom = *ratio.denom().ok_or_else(no_ratio)?;
    let fractions = py.import("fractions")?;
    fractions.getattr("Fraction")?.call1((numer, denom))
}

#[pyfunction]
#[pyo3(name = "notesToGeneric")]
fn notes_to_generic_py(n1: &Bound<'_, PyAny>, n2: &Bound<'_, PyAny>) -> PyResult<GenericInterval> {
    notes_to_generic(&pitch_from_any(n1)?, &pitch_from_any(n2)?)
        .map(GenericInterval::wrap)
        .map_err(interval_error)
}

#[pyfunction]
#[pyo3(name = "notesToChromatic")]
fn notes_to_chromatic_py(
    n1: &Bound<'_, PyAny>,
    n2: &Bound<'_, PyAny>,
) -> PyResult<ChromaticInterval> {
    Ok(ChromaticInterval::wrap(notes_to_chromatic(
        &pitch_from_any(n1)?,
        &pitch_from_any(n2)?,
    )))
}

#[pyfunction]
#[pyo3(name = "intervalsToDiatonic")]
fn intervals_to_diatonic_py(
    gInt: PyRef<'_, GenericInterval>,
    cInt: PyRef<'_, ChromaticInterval>,
) -> PyResult<DiatonicInterval> {
    intervals_to_diatonic(&gInt.inner, &cInt.inner)
        .map(DiatonicInterval::wrap)
        .map_err(interval_error)
}

#[pyfunction]
#[pyo3(name = "intervalFromGenericAndChromatic")]
fn interval_from_generic_and_chromatic(
    py: Python<'_>,
    gInt: &Bound<'_, PyAny>,
    cInt: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let generic = generic_from_any(gInt)?;
    let chromatic = match cInt.extract::<PyRef<ChromaticInterval>>() {
        Ok(facade) => facade.inner.clone(),
        Err(_) => RsChromatic::new(semitones_from_any(cInt)?),
    };
    let diatonic = intervals_to_diatonic(&generic, &chromatic).map_err(interval_error)?;
    let inner =
        RsInterval::from_diatonic_and_chromatic(diatonic, chromatic).map_err(interval_error)?;
    interval_object(py, inner)
}

/// An interval as the class music21 now has under that name, so a stream
/// can hold it and its own code can write an identifier on it.
fn interval_object(py: Python<'_>, inner: RsInterval) -> PyResult<Py<PyAny>> {
    Ok(crate::installed_new(py, "music21.interval", "Interval", Interval::wrap(inner))?.into_any())
}

fn pick<'py>(
    note1: &Bound<'py, PyAny>,
    note2: &Bound<'py, PyAny>,
    choose_first: impl Fn(&RsPitch, &RsPitch) -> bool,
) -> PyResult<Bound<'py, PyAny>> {
    if note1.hasattr("pitch")? != note2.hasattr("pitch")? {
        return Err(PyValueError::new_err(format!(
            "note1 {} and note2 {} must both be notes or pitches",
            note1.repr()?,
            note2.repr()?
        )));
    }
    let p1 = pitch_from_any(note1)?;
    let p2 = pitch_from_any(note2)?;
    if choose_first(&p1, &p2) {
        Ok(note1.clone())
    } else {
        Ok(note2.clone())
    }
}

#[pyfunction]
#[pyo3(name = "getWrittenHigherNote")]
fn get_written_higher_note<'py>(
    note1: &Bound<'py, PyAny>,
    note2: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    // Higher on the staff; where they stand on the same line, the one that
    // sounds higher, and the first of two that sound the same.
    pick(note1, note2, |p1, p2| {
        match p1.diatonic_note_number().cmp(&p2.diatonic_note_number()) {
            std::cmp::Ordering::Equal => p1.ps() >= p2.ps(),
            ordering => ordering.is_gt(),
        }
    })
}

#[pyfunction]
#[pyo3(name = "getWrittenLowerNote")]
fn get_written_lower_note<'py>(
    note1: &Bound<'py, PyAny>,
    note2: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    // Lower on the staff; where they stand on the same line, the one that
    // sounds lower, and the first of two that sound the same.
    pick(note1, note2, |p1, p2| {
        match p1.diatonic_note_number().cmp(&p2.diatonic_note_number()) {
            std::cmp::Ordering::Equal => p1.ps() <= p2.ps(),
            ordering => ordering.is_lt(),
        }
    })
}

#[pyfunction]
#[pyo3(name = "getAbsoluteHigherNote")]
fn get_absolute_higher_note<'py>(
    note1: &Bound<'py, PyAny>,
    note2: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    // Sounding higher, and the first of two that sound the same.
    pick(note1, note2, |p1, p2| p1.ps() >= p2.ps())
}

#[pyfunction]
#[pyo3(name = "getAbsoluteLowerNote")]
fn get_absolute_lower_note<'py>(
    note1: &Bound<'py, PyAny>,
    note2: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    // Sounding lower, and the first of two that sound the same.
    pick(note1, note2, |p1, p2| p1.ps() <= p2.ps())
}

#[pyfunction]
#[pyo3(name = "transposePitch", signature = (pitch1, interval1, *, inPlace = false))]
fn transpose_pitch(
    pitch1: &Bound<'_, PyAny>,
    interval1: &Bound<'_, PyAny>,
    inPlace: bool,
) -> PyResult<Option<Pitch>> {
    let (pitch, inferred) = pitch_and_inferred(pitch1)?;
    let transposed = transpose_pitch_by_any(&pitch, interval1)?;
    deliver_pitch(pitch1, transposed, inferred, inPlace)
}

#[pyfunction]
#[pyo3(name = "transposeNote")]
fn transpose_note<'py>(
    note1: &Bound<'py, PyAny>,
    intervalString: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    transpose_note_by(note1, |pitch| transpose_pitch_by_any(pitch, intervalString))
}

#[pyfunction]
#[pyo3(name = "notesToInterval")]
fn notes_to_interval(
    py: Python<'_>,
    n1: &Bound<'_, PyAny>,
    n2: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let start = extract_pitch_object(n1)?;
    let end = extract_pitch_object(n2)?;
    let inner = RsInterval::between_pitches(&pitch_from_any(&start)?, &pitch_from_any(&end)?)
        .map_err(interval_error)?;
    Ok(crate::installed_new(
        py,
        "music21.interval",
        "Interval",
        Interval::with_pitches(inner, &start, &end),
    )?
    .into_any())
}

fn intervals_from_list(intervalList: &Bound<'_, PyAny>) -> PyResult<Vec<RsInterval>> {
    intervalList
        .try_iter()?
        .map(|item| interval_from_any(&item?))
        .collect()
}

#[pyfunction]
#[pyo3(name = "add")]
fn add(py: Python<'_>, intervalList: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let intervals = intervals_from_list(intervalList)?;
    if intervals.is_empty() {
        return Err(IntervalException::new_err(
            "Cannot add an empty set of intervals",
        ));
    }
    interval_object(py, RsInterval::sum(&intervals).map_err(interval_error)?)
}

#[pyfunction]
#[pyo3(name = "subtract")]
fn subtract(py: Python<'_>, intervalList: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let intervals = intervals_from_list(intervalList)?;
    if intervals.is_empty() {
        return Err(IntervalException::new_err(
            "Cannot subtract an empty set of intervals",
        ));
    }
    interval_object(
        py,
        RsInterval::difference(&intervals).map_err(interval_error)?,
    )
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add_class::<Direction>()?;
    m.add_class::<Specifier>()?;
    m.add_class::<GenericInterval>()?;
    m.add_class::<DiatonicInterval>()?;
    m.add_class::<ChromaticInterval>()?;
    m.add_class::<Interval>()?;
    let exception = py.get_type::<IntervalException>();
    exception.setattr("__module__", "music21.interval")?;
    m.add("IntervalException", exception)?;
    m.add_function(wrap_pyfunction!(convert_staff_distance_to_interval, m)?)?;
    m.add_function(wrap_pyfunction!(convert_diatonic_number_to_step_py, m)?)?;
    m.add_function(wrap_pyfunction!(parse_specifier, m)?)?;
    m.add_function(wrap_pyfunction!(convert_generic, m)?)?;
    m.add_function(wrap_pyfunction!(
        convert_semitone_to_specifier_generic_microtone_py,
        m
    )?)?;
    m.add_function(wrap_pyfunction!(
        convert_semitone_to_specifier_generic_py,
        m
    )?)?;
    m.add_function(wrap_pyfunction!(interval_to_pythagorean_ratio, m)?)?;
    m.add_function(wrap_pyfunction!(notes_to_generic_py, m)?)?;
    m.add_function(wrap_pyfunction!(notes_to_chromatic_py, m)?)?;
    m.add_function(wrap_pyfunction!(intervals_to_diatonic_py, m)?)?;
    m.add_function(wrap_pyfunction!(interval_from_generic_and_chromatic, m)?)?;
    m.add_function(wrap_pyfunction!(get_written_higher_note, m)?)?;
    m.add_function(wrap_pyfunction!(get_written_lower_note, m)?)?;
    m.add_function(wrap_pyfunction!(get_absolute_higher_note, m)?)?;
    m.add_function(wrap_pyfunction!(get_absolute_lower_note, m)?)?;
    m.add_function(wrap_pyfunction!(transpose_pitch, m)?)?;
    m.add_function(wrap_pyfunction!(transpose_note, m)?)?;
    m.add_function(wrap_pyfunction!(notes_to_interval, m)?)?;
    m.add_function(wrap_pyfunction!(add, m)?)?;
    m.add_function(wrap_pyfunction!(subtract, m)?)?;
    Ok(())
}
