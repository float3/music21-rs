//! music21's `pitch` module: `Pitch`, `Accidental`, `Microtone` and the
//! module-level functions, over `music21-rs`.

#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyDict;

use music21_rs::{
    Accidental as RsAccidental, AccidentalDisplayOptions, Interval, Microtone as RsMicrotone,
    Pitch as RsPitch, PitchOptions,
};

use crate::note::Note;

/// The names the pitch facade provides, for swapping into `music21.pitch`.
pub const NAMES: &[&str] = &[
    "Pitch",
    "Accidental",
    "Microtone",
    "PitchException",
    "AccidentalException",
    "MicrotoneException",
    "simplifyMultipleEnharmonics",
    "convertPitchClassToStr",
    "isValidAccidentalName",
    "standardizeAccidentalName",
];

pyo3::create_exception!(music21_rs_facade, PitchException, crate::Music21Exception);
pyo3::create_exception!(
    music21_rs_facade,
    AccidentalException,
    crate::Music21Exception
);
pyo3::create_exception!(
    music21_rs_facade,
    MicrotoneException,
    crate::Music21Exception
);

/// The crate's `Display` prefixes every message with its kind, `Pitch error:`;
/// music21's exceptions carry the message alone, and the doctests compare
/// it.
pub(crate) fn message(error: &music21_rs::Error) -> String {
    let text = error.to_string();
    match text.split_once(" error: ") {
        Some((_, rest)) => rest.to_string(),
        None => text,
    }
}

/// The exception class music21 would raise for a crate error: accidental
/// and microtone errors keep their own classes, everything else is a pitch
/// error.
pub(crate) fn pitch_error(error: music21_rs::Error) -> PyErr {
    specific_error(&error).unwrap_or_else(|| PitchException::new_err(message(&error)))
}

/// The exception class music21 names for a crate error that has one of its
/// own, whatever module the error came out of: an accidental it cannot
/// spell, a microtone it cannot read, or an argument that was never a value
/// at all.
pub(crate) fn specific_error(error: &music21_rs::Error) -> Option<PyErr> {
    match error {
        music21_rs::Error::Accidental(_) => Some(AccidentalException::new_err(message(error))),
        music21_rs::Error::Microtone(_) => Some(MicrotoneException::new_err(message(error))),
        music21_rs::Error::Value(_) => {
            Some(pyo3::exceptions::PyValueError::new_err(message(error)))
        }
        _ => None,
    }
}

error_into!(pub(crate) accidental_error, AccidentalException);

/// The octave a pitch that was never given one sounds in: music21's
/// `defaults.pitchOctave`.
///
/// It is read afresh every time rather than taken once, because it is a
/// module-level setting a caller may change — set it to three and every
/// octaveless pitch drops an octave, which is what music21's own test for it
/// checks. Four is the crate's own answer, and stands when there is no
/// music21 to ask, which is how the wheel answers on its own.
pub(crate) fn default_octave() -> i32 {
    Python::attach(|py| {
        py.import("music21.defaults")
            .and_then(|module| module.getattr("pitchOctave"))
            .and_then(|value| value.extract::<i32>())
            .unwrap_or(4)
    })
}

/// music21's `pitch.Microtone`.
#[pyclass(
    name = "Microtone",
    module = "music21.pitch",
    subclass,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct Microtone {
    inner: RsMicrotone,
}

#[pymethods]
impl Microtone {
    /// music21's `classes`: what this is, and everything it is a kind of.
    /// Its own code reads this to decide what it is looking at.
    #[getter]
    fn classes(&self) -> Vec<&'static str> {
        vec!["Microtone", "ProtoM21Object", "object"]
    }

    #[getter]
    fn classSet(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let names: Vec<&'static str> = vec!["Microtone", "ProtoM21Object", "object"];
        Ok(pyo3::types::PyFrozenSet::new(py, &names)?
            .into_any()
            .unbind())
    }

    /// music21 freezes a score by pickling it, and what this object is lives
    /// in Rust where a pickle cannot see it — so it is written out as text,
    /// and read back into a fresh one of these.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let Some(inner) = crate::unpickled::<_, RsMicrotone>(slf, state)? else {
            return Ok(());
        };
        slf.borrow_mut().inner = inner;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (centsOrString = None, harmonicShift = 1))]
    fn new(centsOrString: Option<&Bound<'_, PyAny>>, harmonicShift: i32) -> PyResult<Self> {
        let inner = match centsOrString {
            None => RsMicrotone::new(0.0),
            Some(value) => {
                if let Ok(text) = value.extract::<String>() {
                    RsMicrotone::new(text.as_str())
                } else {
                    RsMicrotone::new(value.extract::<f64>()?)
                }
            }
        }
        .map_err(|error| MicrotoneException::new_err(message(&error)))?;
        let mut inner = inner;
        inner.set_harmonic_shift(harmonicShift);
        Ok(Self { inner })
    }

    #[getter]
    fn cents<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let cents = self.inner.cents();
        if cents.fract() == 0.0 && cents.abs() < 1e15 {
            Ok((cents as i64).into_pyobject(py)?.into_any())
        } else {
            Ok(cents.into_pyobject(py)?.into_any())
        }
    }

    #[getter]
    fn alter(&self) -> f64 {
        self.inner.alter()
    }

    #[getter]
    fn harmonicShift(&self) -> i32 {
        self.inner.harmonic_shift()
    }

    #[setter]
    fn set_harmonicShift(&mut self, value: i32) {
        self.inner.set_harmonic_shift(value);
    }

    fn __repr__(&self) -> String {
        format!("<music21.pitch.Microtone {}>", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .getattr("cents")
            .ok()
            .and_then(|cents| cents.extract::<f64>().ok())
            .is_some_and(|cents| cents == self.inner.cents())
    }

    fn __hash__(&self) -> u64 {
        self.inner.cents().to_bits()
    }

    /// A copy as an object of the class it was asked on: music21 compares
    /// two of these by class before anything else, so a copy built as the
    /// bare facade would not equal the original.
    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.borrow().clone();
        crate::copy_as_same_type(slf, copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.borrow().clone();
        crate::copy_as_same_type(slf, copied)
    }
}

/// music21's `pitch.Accidental`.
#[pyclass(
    name = "Accidental",
    module = "music21.pitch",
    subclass,
    skip_from_py_object
)]
pub struct Accidental {
    pub(crate) inner: RsAccidental,
    /// The pitch this accidental belongs to, when it came out of one.
    ///
    /// music21's `p.accidental` is the pitch's own accidental, so editing it
    /// edits the pitch — its `roman` module corrects a chord's spelling by
    /// calling `set` on the accidental it read off a pitch, and expects the
    /// pitch to change. [`Accidental::write_back`] is what carries it there.
    owner: Option<Py<Pitch>>,
    /// music21's `style`, once something has asked for one: the object
    /// saying how this is drawn. It is music21's own object — the page is
    /// not something this crate models — and its mere existence is what
    /// `hasStyleInformation` answers, as music21's does.
    style: Option<Py<PyAny>>,
}

impl Clone for Accidental {
    /// A copy of an accidental is a loose one, as a copy of a pitch is —
    /// but it is drawn the way the original was, so the style comes across
    /// as music21's own deepcopy carries it.
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            inner: self.inner.clone(),
            owner: None,
            style: crate::notation::copied_style(py, self.style.as_ref()),
        })
    }
}

fn accidental_from_any(value: &Bound<'_, PyAny>) -> PyResult<RsAccidental> {
    if let Ok(facade) = value.extract::<PyRef<Accidental>>() {
        return Ok(facade.inner.clone());
    }
    if let Ok(text) = value.extract::<String>() {
        return RsAccidental::new(text.as_str()).map_err(accidental_error);
    }
    if let Ok(name) = value
        .getattr("name")
        .and_then(|name| name.extract::<String>())
    {
        return RsAccidental::new(name.as_str()).map_err(accidental_error);
    }
    RsAccidental::new(value.extract::<f64>()?).map_err(accidental_error)
}

impl Accidental {
    pub(crate) fn from_inner(inner: RsAccidental) -> Self {
        Self {
            inner,
            owner: None,
            style: None,
        }
    }

    /// The same accidental, knowing which pitch it came off.
    fn owned_by(inner: RsAccidental, pitch: Py<Pitch>) -> Self {
        Self {
            inner,
            owner: Some(pitch),
            style: None,
        }
    }

    /// Writes this accidental into the pitch it belongs to, if it belongs to
    /// one, so that an edit through it is an edit to the pitch.
    ///
    /// The value goes in, not a new object: music21's `p.accidental` is one
    /// object that stays put, and a caller holding it — music21's own
    /// `makeAccidentals` holds one while it decides whether to write it — has
    /// to go on holding the accidental the pitch has.
    fn write_back(&self, py: Python<'_>) -> PyResult<()> {
        let Some(owner) = &self.owner else {
            return Ok(());
        };
        let mut value = self.inner.clone();
        // music21 keeps the colour on the style, so an edit through
        // `a.style.color` is an edit to the accidental.
        if self.style.is_some() {
            value.set_color(crate::notation::style_colour(py, self.style.as_ref()));
        }
        Pitch::take_accidental_value(owner.bind(py), value)
    }
}

#[pymethods]
impl Accidental {
    /// music21's `classes`: what this is, and everything it is a kind of.
    /// Its own code reads this to decide what it is looking at.
    #[getter]
    fn classes(&self) -> Vec<&'static str> {
        vec![
            "Accidental",
            "ProtoM21Object",
            "StyleMixin",
            "SlottedObjectMixin",
            "object",
        ]
    }

    #[getter]
    fn classSet(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let names: Vec<&'static str> = vec![
            "Accidental",
            "ProtoM21Object",
            "StyleMixin",
            "SlottedObjectMixin",
            "object",
        ];
        Ok(pyo3::types::PyFrozenSet::new(py, &names)?
            .into_any()
            .unbind())
    }

    /// music21 freezes a score by pickling it, and what this object is lives
    /// in Rust where a pickle cannot see it — so it is written out as text,
    /// and read back into a fresh one of these.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let Some(inner) = crate::unpickled::<_, RsAccidental>(slf, state)? else {
            return Ok(());
        };
        slf.borrow_mut().inner = inner;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (specifier = None))]
    fn new(specifier: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let inner = match specifier {
            None => RsAccidental::natural(),
            Some(value) => accidental_from_any(value)?,
        };
        Ok(Self::from_inner(inner))
    }

    #[staticmethod]
    fn listNames() -> Vec<&'static str> {
        RsAccidental::list_names()
    }

    #[pyo3(signature = (specifier, *, allowNonStandardValue = false))]
    fn set(
        &mut self,
        py: Python<'_>,
        specifier: &Bound<'_, PyAny>,
        allowNonStandardValue: bool,
    ) -> PyResult<()> {
        if allowNonStandardValue {
            if let Ok(text) = specifier.extract::<String>() {
                self.inner
                    .set_allowing_non_standard_value(text.as_str())
                    .map_err(accidental_error)?;
            } else {
                self.inner
                    .set_allowing_non_standard_value(specifier.extract::<f64>()?)
                    .map_err(accidental_error)?;
            }
            return self.write_back(py);
        }
        self.inner = accidental_from_any(specifier)?;
        self.write_back(py)
    }

    fn isTwelveTone(&self) -> bool {
        self.inner.is_twelve_tone()
    }

    #[getter]
    fn name(&self) -> String {
        self.inner.name().to_string()
    }

    #[setter]
    fn set_name(&mut self, py: Python<'_>, value: &str) -> PyResult<()> {
        self.inner.set_name(value).map_err(accidental_error)?;
        self.write_back(py)
    }

    #[getter]
    fn alter(&self) -> f64 {
        self.inner.alter()
    }

    #[setter]
    fn set_alter(&mut self, py: Python<'_>, value: f64) -> PyResult<()> {
        self.inner.set_alter(value).map_err(accidental_error)?;
        self.write_back(py)
    }

    #[getter]
    fn modifier(&self) -> String {
        self.inner.modifier().to_string()
    }

    #[setter]
    fn set_modifier(&mut self, py: Python<'_>, value: &str) -> PyResult<()> {
        self.inner.set_modifier(value);
        self.write_back(py)
    }

    #[getter]
    fn unicode(&self) -> String {
        self.inner.unicode()
    }

    #[getter]
    fn fullName(&self) -> String {
        self.inner.full_name().to_string()
    }

    /// music21's `style`: the object saying how this is drawn, made on
    /// first asking and the same one after that. It is music21's own — the
    /// page is not something this crate models — with the colour, which it
    /// does model, written into it.
    #[getter]
    fn get_style(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        if let Some(style) = &slf.borrow().style {
            return Ok(style.clone_ref(py));
        }
        let colour = slf.borrow().inner.color().map(str::to_string);
        let style = crate::notation::new_style(slf.as_any(), colour.as_deref())?;
        slf.borrow_mut().style = Some(style.clone_ref(py));
        Ok(style)
    }

    #[setter]
    fn set_style(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let colour: Option<String> = value
            .getattr("color")
            .ok()
            .and_then(|colour| colour.extract().ok());
        slf.borrow_mut().inner.set_color(colour);
        slf.borrow().write_back(slf.py())?;
        slf.borrow_mut().style = Some(value.clone().unbind());
        Ok(())
    }

    /// music21's `hasStyleInformation`: whether a style object has been made
    /// for this yet, which is what its own code asks before making one.
    #[getter]
    fn hasStyleInformation(&self) -> bool {
        self.style.is_some()
    }

    /// music21's `inheritDisplay`: copies every display setting from another
    /// accidental.
    fn inheritDisplay(&mut self, py: Python<'_>, other: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let Some(other) = other.filter(|other| !other.is_none()) else {
            return Ok(());
        };
        let source = other.extract::<PyRef<Accidental>>()?;
        self.inner.inherit_display(&source.inner);
        self.write_back(py)
    }

    /// music21's `setAttributeIndependently`: writes `name`, `alter` or
    /// `modifier` without the other two following it. Any other attribute is
    /// an error, as upstream.
    fn setAttributeIndependently(
        &mut self,
        py: Python<'_>,
        attribute: &str,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        match attribute {
            "name" => self
                .inner
                .set_name_independently(value.extract::<String>()?),
            "alter" => self.inner.set_alter_independently(value.extract::<f64>()?),
            "modifier" => self
                .inner
                .set_modifier_independently(value.extract::<String>()?),
            other => {
                return Err(AccidentalException::new_err(format!(
                    "Cannot set attribute {other} independently of other parts."
                )));
            }
        }
        self.write_back(py)
    }

    #[getter]
    fn displayType(&self) -> &'static str {
        self.inner.display_type()
    }

    #[setter]
    fn set_displayType(&mut self, py: Python<'_>, value: &str) -> PyResult<()> {
        self.inner
            .set_display_type(value)
            .map_err(accidental_error)?;
        self.write_back(py)
    }

    /// music21's `displayStyle`: whether the accidental is written plainly,
    /// in parentheses or in brackets.
    #[getter]
    fn get_displayStyle(&self) -> &'static str {
        self.inner.display_style()
    }

    #[setter]
    fn set_displayStyle(&mut self, py: Python<'_>, value: &str) -> PyResult<()> {
        self.inner
            .set_display_style(value)
            .map_err(accidental_error)?;
        self.write_back(py)
    }

    /// music21's `displaySize`: full size, or a cue.
    #[getter]
    fn get_displaySize(&self) -> String {
        self.inner.display_size()
    }

    #[setter]
    fn set_displaySize(&mut self, py: Python<'_>, value: &str) -> PyResult<()> {
        self.inner
            .set_display_size(value)
            .map_err(accidental_error)?;
        self.write_back(py)
    }

    /// music21's `displayLocation`: beside the note, or above or below it.
    #[getter]
    fn get_displayLocation(&self) -> &'static str {
        self.inner.display_location()
    }

    #[setter]
    fn set_displayLocation(&mut self, py: Python<'_>, value: &str) -> PyResult<()> {
        self.inner
            .set_display_location(value)
            .map_err(accidental_error)?;
        self.write_back(py)
    }

    #[getter]
    fn displayStatus(&self) -> Option<bool> {
        self.inner.display_status()
    }

    #[setter]
    fn set_displayStatus(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        if value.is_none() {
            self.inner.set_display_status(None);
        } else if let Ok(status) = value.extract::<bool>() {
            self.inner.set_display_status(Some(status));
        } else {
            return Err(AccidentalException::new_err(format!(
                "Supplied display status is not supported: {}",
                value.repr()?
            )));
        }
        self.write_back(value.py())
    }

    fn __repr__(&self) -> String {
        format!("<music21.pitch.Accidental {}>", self.inner.name())
    }

    fn __str__(&self) -> String {
        self.inner.name().to_string()
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .getattr("name")
            .ok()
            .and_then(|name| name.extract::<String>().ok())
            .is_some_and(|name| name == self.inner.name())
    }

    fn __lt__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<Accidental>>()
            .is_ok_and(|other| self.inner.alter() < other.inner.alter())
    }

    fn __le__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<Accidental>>()
            .is_ok_and(|other| self.inner.alter() <= other.inner.alter())
    }

    fn __gt__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<Accidental>>()
            .is_ok_and(|other| self.inner.alter() > other.inner.alter())
    }

    fn __ge__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<Accidental>>()
            .is_ok_and(|other| self.inner.alter() >= other.inner.alter())
    }

    fn __hash__(&self) -> u64 {
        self.inner.alter().to_bits()
    }

    /// A copy as an object of the class it was asked on: music21 compares
    /// two of these by class before anything else, so a copy built as the
    /// bare facade would not equal the original.
    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.borrow().clone();
        crate::copy_as_same_type(slf, copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.borrow().clone();
        crate::copy_as_same_type(slf, copied)
    }
}

/// music21's `pitch.Pitch`.
// `dict`: music21's own pitch is not slotted, and its code decorates one —
// `audioSearch` hangs the frequency it heard on the pitch it decided — so a
// pitch here takes an attribute nobody here has heard of, as that one does.
#[pyclass(
    name = "Pitch",
    module = "music21.pitch",
    dict,
    subclass,
    skip_from_py_object
)]
pub struct Pitch {
    pub(crate) inner: RsPitch,
    pub(crate) spelling_is_inferred: bool,
    /// The note this pitch belongs to, when it came out of one. music21's
    /// `chord[0].pitch` and `chord.pitches[0]` are the chord's own pitch, so
    /// an edit through this object has to reach the note and the chord;
    /// [`Pitch::write_back`] is what carries it there.
    pub(crate) owner: Option<Py<Note>>,
    /// The pitch's own `Accidental` object, once something has asked for it.
    ///
    /// music21 keeps one object and hands it back every time, so
    /// `p.accidental.displayType = 'never'` is an edit to the pitch and not
    /// to a copy thrown away on the next line. `inner` is still what says
    /// what the accidental *is*; this object mirrors it.
    accidental: Option<Py<Accidental>>,
}

impl Clone for Pitch {
    /// A copy of a pitch is a loose one: music21's `copy.copy(p)` is not the
    /// note's pitch any more, so the owner does not come along.
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            spelling_is_inferred: self.spelling_is_inferred,
            owner: None,
            accidental: None,
        }
    }
}

/// Reads a transposition argument the way `Pitch.transpose` does: an interval
/// name, a semitone count, a facade interval, or any object with music21's
/// `directedName`.
pub(crate) fn interval_from_any(value: &Bound<'_, PyAny>) -> PyResult<Interval> {
    crate::interval::interval_from_any(value).map_err(|error| {
        PitchException::new_err(format!(
            "cannot transpose by {}: {error}",
            value
                .repr()
                .map_or_else(|_| "?".to_string(), |r| r.to_string())
        ))
    })
}

/// Reads an optional sequence of pitch arguments, as the display and key
/// helpers take them.
pub(crate) fn pitch_list(value: Option<&Bound<'_, PyAny>>) -> PyResult<Vec<RsPitch>> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(Vec::new());
    };
    value
        .try_iter()?
        .map(|item| pitch_from_any(&item?))
        .collect()
}

/// The keywords music21's `Pitch.__init__` understands, which its `Note`
/// hands straight through: `note.Note(step='C', accidental='sharp', octave=2)`
/// is the pitch built from them.
const PITCH_KEYWORDS: [&str; 10] = [
    "name",
    "nameWithOctave",
    "step",
    "octave",
    "accidental",
    "microtone",
    "pitchClass",
    "midi",
    "ps",
    "fundamental",
];

/// The pitch music21 would build from a note's keywords, or nothing when
/// none of them say anything about a pitch.
///
/// `nameWithOctave` arrives as `name`, since a name carrying an octave is
/// what the crate's parser already reads.
pub(crate) fn pitch_from_keywords(
    py: Python<'_>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<Option<RsPitch>> {
    let wanted = pitch_keywords(py, keywords)?;
    if wanted.is_empty() {
        return Ok(None);
    }
    let built = py.get_type::<Pitch>().call((), Some(&wanted))?;
    Ok(Some(built.extract::<PyRef<'_, Pitch>>()?.inner.clone()))
}

/// The pitch keywords out of a note's keywords, under the names `Pitch`
/// itself takes.
fn pitch_keywords<'py>(
    py: Python<'py>,
    keywords: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyDict>> {
    let wanted = PyDict::new(py);
    let Some(keywords) = keywords else {
        return Ok(wanted);
    };
    for name in PITCH_KEYWORDS {
        if let Some(value) = keywords.get_item(name)? {
            wanted.set_item(
                if name == "nameWithOctave" {
                    "name"
                } else {
                    name
                },
                value,
            )?;
        }
    }
    Ok(wanted)
}

/// The pitch music21 builds from a positional argument *and* the keywords
/// beside it: its `Note.__init__` writes `Pitch(pitch, **keywords)`, so
/// `Note('A', octave=4)` is `A4` rather than an octave-less `A`. A `Pitch`
/// already built is kept as it stands, which is the other half of what
/// music21 writes there.
pub(crate) fn pitch_from_any_with_keywords(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<RsPitch> {
    let wanted = pitch_keywords(py, keywords)?;
    // Only a name or a number is something `Pitch` takes positionally; any
    // other object is read for its pitch as it always was.
    let positional = value.extract::<String>().is_ok() || value.extract::<f64>().is_ok();
    if wanted.is_empty() || !positional {
        return pitch_from_any(value);
    }
    let built = py.get_type::<Pitch>().call((value,), Some(&wanted))?;
    Ok(built.extract::<PyRef<'_, Pitch>>()?.inner.clone())
}

/// Reads a pitch argument: a facade, a name, or a number.
pub(crate) fn pitch_from_any(value: &Bound<'_, PyAny>) -> PyResult<RsPitch> {
    if let Ok(facade) = value.extract::<PyRef<Pitch>>() {
        return Ok(facade.inner.clone());
    }
    if let Ok(name) = value.extract::<String>() {
        return RsPitch::from_name(name).map_err(pitch_error);
    }
    if let Ok(number) = value.extract::<f64>() {
        return RsPitch::from_number(number).map_err(pitch_error);
    }
    if let Ok(name) = value
        .getattr("nameWithOctave")
        .and_then(|name| name.extract::<String>())
    {
        return RsPitch::from_name(name).map_err(pitch_error);
    }
    Err(PitchException::new_err(format!(
        "cannot read a pitch from {}",
        value.repr()?
    )))
}

impl Pitch {
    pub(crate) fn wrap(mut inner: RsPitch, spelling_is_inferred: bool) -> Self {
        // The value carries the same answer, since it is what decides
        // whether transposing respells: a caller who says the spelling was
        // chosen must be believed by everything downstream.
        inner.set_spelling_is_inferred(spelling_is_inferred);
        Self {
            inner,
            spelling_is_inferred,
            owner: None,
            accidental: None,
        }
    }

    /// Says whether the spelling was chosen or merely worked out, on both
    /// halves at once.
    fn mark_inferred(&mut self, inferred: bool) {
        self.spelling_is_inferred = inferred;
        self.inner.set_spelling_is_inferred(inferred);
    }

    /// Takes a value written through the pitch's own accidental object,
    /// without disturbing that object: it is the one music21 hands back
    /// every time, and a caller may be holding it.
    fn take_accidental_value(slf: &Bound<'_, Self>, value: RsAccidental) -> PyResult<()> {
        let rebuilt = {
            let pitch = slf.borrow();
            pitch.rebuilt(
                pitch.step_char(),
                value.name(),
                pitch.inner.octave(),
                pitch.microtone_cents(),
            )?
        };
        let mut pitch = slf.borrow_mut();
        pitch.inner = rebuilt;
        pitch.inner.set_accidental(Some(value));
        pitch.write_back()
    }

    /// Writes this pitch's value back to the note that holds it, and through
    /// the note to its chord. A pitch nobody owns writes nowhere.
    ///
    /// This is what every mutating method ends with. `Python::attach` rather
    /// than a threaded-through token so that music21's setters keep their
    /// shapes; a mutating method is only ever reached from Python, so the
    /// interpreter is already there.
    pub(crate) fn write_back(&self) -> PyResult<()> {
        let Some(owner) = &self.owner else {
            return Ok(());
        };
        Python::attach(|py| Note::adopt_pitch(py, owner, &self.inner))
    }

    /// This pitch as it sounds, which for one that was never given an octave
    /// means in music21's [`default_octave`].
    ///
    /// Everything that reads a pitch as a number — its pitch space, its MIDI
    /// number, its frequency, the note number it is written on — goes through
    /// here, because music21 reads all of them off `.octave` and `.octave`
    /// answers the default when none was given.
    fn sounding(&self) -> RsPitch {
        if !self.inner.octave_is_implicit() {
            return self.inner.clone();
        }
        let mut sounding = self.inner.clone();
        sounding.set_octave(Some(default_octave()));
        sounding
    }

    fn accidental_name(&self) -> String {
        self.inner.accidental().name().to_string()
    }

    fn microtone_cents(&self) -> f64 {
        self.inner.microtone().map_or(0.0, RsMicrotone::cents)
    }

    /// Rebuilds the pitch from its parts with one of them changed, the way
    /// music21's property setters mutate in place.
    fn rebuilt(
        &self,
        step: char,
        accidental: &str,
        octave: Option<i32>,
        cents: f64,
    ) -> PyResult<RsPitch> {
        let mut options = PitchOptions::new().step(step).accidental(accidental);
        if let Some(octave) = octave {
            options = options.octave(octave);
        }
        if cents != 0.0 {
            options = options.microtone(cents);
        }
        let mut rebuilt = options.build().map_err(pitch_error)?;
        if !self.inner.has_accidental() && accidental == "natural" {
            rebuilt.set_accidental(None);
        }
        Ok(rebuilt)
    }

    fn step_char(&self) -> char {
        self.inner.name().chars().next().unwrap_or('C')
    }
}

#[pymethods]
impl Pitch {
    /// music21 freezes a score by pickling it, and what a pitch is lives in
    /// Rust where a pickle cannot see it — so it is written out as text and
    /// read back. The accidental object a caller may be holding is not: a
    /// thawed pitch spells its own.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let Some(inner) = crate::unpickled::<_, RsPitch>(slf, state)? else {
            return Ok(());
        };
        slf.borrow_mut().inner = inner;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (name = None, *, step = None, octave = None, accidental = None, microtone = None, pitchClass = None, midi = None, ps = None, fundamental = None, **kwargs))]
    #[allow(non_snake_case, clippy::too_many_arguments)]
    fn new(
        name: Option<&Bound<'_, PyAny>>,
        step: Option<&str>,
        octave: Option<i32>,
        accidental: Option<&Bound<'_, PyAny>>,
        microtone: Option<&Bound<'_, PyAny>>,
        pitchClass: Option<&Bound<'_, PyAny>>,
        midi: Option<i32>,
        ps: Option<f64>,
        fundamental: Option<PyRef<'_, Pitch>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let _ = kwargs;
        let mut options = PitchOptions::new();
        let mut inferred = name.is_none() && step.is_none();
        if let Some(name) = name {
            if let Ok(text) = name.extract::<String>() {
                options = options.name(text);
                inferred = false;
            } else if let Ok(integer) = name.extract::<i32>() {
                options = options.name(integer);
                inferred = true;
            } else {
                options = options.name(name.extract::<f64>()?);
                inferred = true;
            }
        }
        if let Some(step) = step {
            let letter = step
                .chars()
                .next()
                .ok_or_else(|| PitchException::new_err("step cannot be empty"))?;
            options = options.step(letter);
            inferred = false;
        }
        if let Some(octave) = octave {
            options = options.octave(octave);
        }
        if let Some(accidental) = accidental
            && !accidental.is_none()
        {
            options = options.accidental(accidental_from_any(accidental)?);
        }
        if let Some(microtone) = microtone
            && !microtone.is_none()
        {
            if let Ok(facade) = microtone.extract::<PyRef<Microtone>>() {
                options = options.microtone(facade.inner.clone());
            } else if let Ok(text) = microtone.extract::<String>() {
                options = options.microtone(text);
            } else {
                options = options.microtone(microtone.extract::<f64>()?);
            }
        }
        if let Some(pitch_class) = pitchClass {
            if let Ok(text) = pitch_class.extract::<String>() {
                options = options.pitch_class(text);
            } else {
                options = options.pitch_class(pitch_class.extract::<i32>()?);
            }
            inferred = true;
        }
        if let Some(midi) = midi {
            options = options.midi(midi);
            inferred = true;
        }
        if let Some(ps) = ps {
            options = options.ps(ps);
            inferred = true;
        }
        if let Some(fundamental) = fundamental {
            options = options.fundamental(fundamental.inner.clone());
        }
        let inner = options.build().map_err(pitch_error)?;
        Ok(Self::wrap(inner, inferred))
    }

    // ---- names -----------------------------------------------------------

    #[getter]
    fn name(&self) -> String {
        self.inner.name()
    }

    #[setter]
    fn set_name(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        // music21 refuses anything but a string here, and says so as a
        // `ValueError` rather than letting Python raise its own `TypeError`.
        let value = &value.extract::<String>().map_err(|_| {
            pyo3::exceptions::PyValueError::new_err(format!(
                "Argument to name, {}, must be a string, not {}.",
                value
                    .repr()
                    .map(|repr| repr.to_string())
                    .unwrap_or_default(),
                value.get_type(),
            ))
        })?;
        let value = value.as_str();
        let octave = self.inner.octave();
        let mut options = PitchOptions::new().name(value);
        if let Some(octave) = octave
            && !value.chars().any(|c| c.is_ascii_digit())
        {
            options = options.octave(octave);
        }
        self.inner = options.build().map_err(pitch_error)?;
        self.mark_inferred(false);
        self.write_back()
    }

    #[getter]
    fn nameWithOctave(&self) -> String {
        self.inner.name_with_octave()
    }

    #[setter]
    fn set_nameWithOctave(&mut self, value: &str) -> PyResult<()> {
        if !value.chars().any(|c| c.is_ascii_digit()) {
            return Err(PitchException::new_err(format!(
                "Cannot set a nameWithOctave with '{value}'"
            )));
        }
        self.inner = RsPitch::from_name(value).map_err(pitch_error)?;
        self.mark_inferred(false);
        self.write_back()
    }

    #[getter]
    fn unicodeName(&self) -> String {
        self.inner.unicode_name()
    }

    #[getter]
    fn unicodeNameWithOctave(&self) -> String {
        self.inner.unicode_name_with_octave()
    }

    #[getter]
    fn fullName(&self) -> String {
        self.inner.full_name()
    }

    #[getter]
    fn german(&self) -> PyResult<String> {
        self.inner.german().map_err(pitch_error)
    }

    #[getter]
    fn italian(&self) -> PyResult<String> {
        self.inner.italian().map_err(pitch_error)
    }

    #[getter]
    fn french(&self) -> PyResult<String> {
        self.inner.french().map_err(pitch_error)
    }

    #[getter]
    fn spanish(&self) -> PyResult<String> {
        self.inner.spanish().map_err(pitch_error)
    }

    // ---- parts -----------------------------------------------------------

    #[getter]
    fn step(&self) -> String {
        self.step_char().to_string()
    }

    #[setter]
    fn set_step(&mut self, value: &str) -> PyResult<()> {
        let mut letters = value.chars();
        let letter = match (letters.next(), letters.next()) {
            (Some(letter), None)
                if letter.is_ascii_alphabetic()
                    && "ABCDEFG".contains(letter.to_ascii_uppercase()) =>
            {
                letter
            }
            _ => {
                return Err(PitchException::new_err(format!(
                    "Cannot make a step out of '{value}'"
                )));
            }
        };
        self.inner = self.rebuilt(
            letter.to_ascii_uppercase(),
            &self.accidental_name(),
            self.inner.octave(),
            self.microtone_cents(),
        )?;
        self.mark_inferred(false);
        self.write_back()
    }

    /// music21's `octave` is always an `int`: a pitch given no octave
    /// reports the default, 4, and `octaveIsImplicit` says which it was.
    #[getter]
    fn octave(&self) -> i32 {
        self.inner.octave().unwrap_or_else(default_octave)
    }

    /// music21's `octaveIsImplicit`: true for a pitch that was
    /// never given an octave and so stands for its pitch class in any.
    #[getter]
    fn octaveIsImplicit(&self) -> bool {
        self.inner.octave_is_implicit()
    }

    /// Setting it true takes the octave away; setting it false puts the pitch
    /// in the default octave. Setting it to what it already is does nothing,
    /// as upstream's does.
    #[setter]
    fn set_octaveIsImplicit(&mut self, value: bool) -> PyResult<()> {
        if value == self.inner.octave_is_implicit() {
            return Ok(());
        }
        let octave = if value { None } else { Some(self.octave()) };
        self.inner = self.rebuilt(
            self.step_char(),
            &self.accidental_name(),
            octave,
            self.microtone_cents(),
        )?;
        self.write_back()
    }

    /// music21's setter is `int(value)`, so an octave read out of a file as
    /// text still sets one.
    #[setter]
    fn set_octave(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let value = match value {
            Some(value) => Some(crate::as_int(value)?),
            None => None,
        };
        self.inner = self.rebuilt(
            self.step_char(),
            &self.accidental_name(),
            value,
            self.microtone_cents(),
        )?;
        self.write_back()
    }

    #[getter]
    fn implicitOctave(&self) -> i32 {
        self.octave()
    }

    /// music21's `accidental`, which is the pitch's own: editing it edits
    /// the pitch. A pitch that carries none — music21 keeps no accidental on
    /// a bare `D` — answers nothing.
    #[getter]
    fn accidental(slf: &Bound<'_, Self>) -> PyResult<Option<Py<Accidental>>> {
        let py = slf.py();
        let Some(current) = slf.borrow().inner.explicit_accidental().cloned() else {
            slf.borrow_mut().accidental = None;
            return Ok(None);
        };
        // The same object as last time, unless the pitch is spelt with a
        // different accidental now — music21 makes a new one for that.
        let held = slf.borrow().accidental.as_ref().map(|a| a.clone_ref(py));
        if let Some(held) = held
            && held.borrow(py).inner.name() == current.name()
        {
            held.borrow_mut(py).inner = current;
            return Ok(Some(held));
        }
        let created = crate::installed_new(
            py,
            "music21.pitch",
            "Accidental",
            Accidental::owned_by(current, slf.clone().unbind()),
        )?;
        slf.borrow_mut().accidental = Some(created.clone_ref(py));
        Ok(Some(created))
    }

    /// The accidental object assigned becomes the pitch's own, as music21's
    /// does: a caller who sets one and then edits it is editing the pitch.
    #[setter]
    fn set_accidental(slf: &Bound<'_, Self>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let py = slf.py();
        let given = value.filter(|value| !value.is_none());
        let accidental = match given {
            None => None,
            Some(value) => Some(accidental_from_any(value)?),
        };
        let rebuilt = {
            let pitch = slf.borrow();
            pitch.rebuilt(
                pitch.step_char(),
                accidental.as_ref().map_or("natural", RsAccidental::name),
                pitch.inner.octave(),
                pitch.microtone_cents(),
            )?
        };
        {
            let mut pitch = slf.borrow_mut();
            pitch.inner = rebuilt;
            pitch.inner.set_accidental(accidental.clone());
            pitch.accidental = None;
        }
        // An accidental object handed over is kept, and told which pitch it
        // now belongs to.
        if let Some(given) = given
            && let Ok(object) = given.extract::<Py<Accidental>>()
        {
            object.borrow_mut(py).owner = Some(slf.clone().unbind());
            slf.borrow_mut().accidental = Some(object);
        }
        slf.borrow().write_back()
    }

    /// music21 keeps the accidental in a private slot and its own code
    /// reaches for it — `musedata` writes one straight in — so the slot
    /// answers here too, as the accidental itself.
    #[getter]
    fn _accidental(slf: &Bound<'_, Self>) -> PyResult<Option<Py<Accidental>>> {
        Self::accidental(slf)
    }

    #[setter]
    fn set__accidental(slf: &Bound<'_, Self>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        Self::set_accidental(slf, value)
    }

    #[getter]
    fn alter(&self) -> f64 {
        self.inner.alter()
    }

    #[getter]
    fn microtone(&self) -> Microtone {
        Microtone {
            inner: self
                .inner
                .microtone()
                .cloned()
                .unwrap_or_else(|| RsMicrotone::new(0.0).expect("zero cents is a microtone")),
        }
    }

    #[setter]
    fn set_microtone(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let cents = match value {
            None => 0.0,
            Some(value) if value.is_none() => 0.0,
            Some(value) => {
                if let Ok(facade) = value.extract::<PyRef<Microtone>>() {
                    facade.inner.cents()
                } else if let Ok(text) = value.extract::<String>() {
                    RsMicrotone::new(text.as_str())
                        .map_err(|error| MicrotoneException::new_err(message(&error)))?
                        .cents()
                } else {
                    value.extract::<f64>()?
                }
            }
        };
        self.inner = self.rebuilt(
            self.step_char(),
            &self.accidental_name(),
            self.inner.octave(),
            cents,
        )?;
        self.write_back()
    }

    #[getter]
    fn spellingIsInferred(&self) -> bool {
        self.spelling_is_inferred
    }

    #[setter]
    fn set_spellingIsInferred(&mut self, value: bool) {
        self.mark_inferred(value);
    }

    #[getter]
    fn fundamental(&self) -> Option<Pitch> {
        self.inner
            .fundamental()
            .map(|fundamental| Pitch::wrap(fundamental.clone(), false))
    }

    // ---- numbers ---------------------------------------------------------

    #[getter]
    fn ps(&self) -> f64 {
        self.sounding().ps()
    }

    #[setter]
    fn set_ps(&mut self, value: f64) -> PyResult<()> {
        self.inner = RsPitch::from_pitch_space(value).map_err(pitch_error)?;
        self.mark_inferred(true);
        self.write_back()
    }

    #[getter]
    fn midi(&self) -> i32 {
        self.sounding().midi()
    }

    #[setter]
    fn set_midi(&mut self, value: f64) -> PyResult<()> {
        let mut midi = value.round_ties_even() as i32;
        if midi > 127 {
            midi = 108 + midi.rem_euclid(12);
            if midi < 115 {
                midi += 12;
            }
        } else if midi < 0 {
            midi = midi.rem_euclid(12);
        }
        self.inner = RsPitch::from_midi(midi).map_err(pitch_error)?;
        self.mark_inferred(true);
        self.write_back()
    }

    #[getter]
    fn pitchClass(&self) -> i32 {
        self.inner.ps().round_ties_even().rem_euclid(12.0) as i32
    }

    #[setter]
    fn set_pitchClass(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let mut options = PitchOptions::new();
        if let Ok(text) = value.extract::<String>() {
            options = options.pitch_class(text);
        } else {
            options = options.pitch_class(value.extract::<i32>()?);
        }
        if let Some(octave) = self.inner.octave() {
            options = options.octave(octave);
        }
        let cents = self.microtone_cents();
        if cents != 0.0 {
            options = options.microtone(cents);
        }
        self.inner = options.build().map_err(pitch_error)?;
        self.mark_inferred(true);
        self.write_back()
    }

    #[getter]
    fn pitchClassString(&self) -> String {
        self.inner.pitch_class_string()
    }

    #[setter]
    fn set_pitchClassString(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.set_pitchClass(value)
    }

    #[getter]
    fn diatonicNoteNum(&self) -> i32 {
        self.sounding().diatonic_note_number()
    }

    #[setter]
    fn set_diatonicNoteNum(&mut self, value: i32) -> PyResult<()> {
        let index = (value - 1).rem_euclid(7) as usize;
        let letter = ['C', 'D', 'E', 'F', 'G', 'A', 'B'][index];
        let octave = (value - 1).div_euclid(7);
        self.inner = self.rebuilt(
            letter,
            &self.accidental_name(),
            Some(octave),
            self.microtone_cents(),
        )?;
        self.write_back()
    }

    #[getter]
    fn frequency(&self) -> f64 {
        self.sounding().frequency_hz()
    }

    #[setter]
    fn set_frequency(&mut self, value: f64) -> PyResult<()> {
        self.inner = RsPitch::from_frequency(value).map_err(pitch_error)?;
        self.mark_inferred(true);
        self.write_back()
    }

    #[getter]
    fn freq440(&self) -> f64 {
        self.sounding().frequency_hz()
    }

    #[setter]
    fn set_freq440(&mut self, value: f64) -> PyResult<()> {
        self.set_frequency(value)
    }

    // ---- methods ---------------------------------------------------------

    fn isTwelveTone(&self) -> bool {
        self.inner.is_twelve_tone()
    }

    fn getCentShiftFromMidi(&self) -> i32 {
        self.inner.cent_shift_from_midi()
    }

    #[pyo3(signature = (value, *, inPlace = false))]
    fn transpose(
        mut slf: PyRefMut<'_, Self>,
        value: &Bound<'_, PyAny>,
        inPlace: bool,
    ) -> PyResult<Option<Pitch>> {
        let transposed = crate::interval::transpose_pitch_by_any(&slf.inner, value)?;
        // Transposing by a bare number of semitones says nothing about how
        // the answer should be spelled, so music21 marks the spelling of
        // what comes back as one nobody chose.
        let inferred = value.is_instance_of::<pyo3::types::PyInt>() || slf.spelling_is_inferred;
        if inPlace {
            slf.inner = transposed;
            slf.mark_inferred(inferred);
            slf.write_back()?;
            Ok(None)
        } else {
            Ok(Some(Pitch::wrap(transposed, inferred)))
        }
    }

    #[pyo3(signature = (*, inPlace = false))]
    fn getEnharmonic(mut slf: PyRefMut<'_, Self>, inPlace: bool) -> PyResult<Option<Pitch>> {
        let result = slf.inner.get_enharmonic().map_err(pitch_error)?;
        if inPlace {
            slf.inner = result;
            slf.write_back()?;
            Ok(None)
        } else {
            Ok(Some(Pitch::wrap(result, false)))
        }
    }

    #[pyo3(signature = (*, inPlace = false))]
    fn getHigherEnharmonic(mut slf: PyRefMut<'_, Self>, inPlace: bool) -> PyResult<Option<Pitch>> {
        let result = slf.inner.get_higher_enharmonic().map_err(pitch_error)?;
        if inPlace {
            slf.inner = result;
            slf.write_back()?;
            Ok(None)
        } else {
            Ok(Some(Pitch::wrap(result, false)))
        }
    }

    #[pyo3(signature = (*, inPlace = false))]
    fn getLowerEnharmonic(mut slf: PyRefMut<'_, Self>, inPlace: bool) -> PyResult<Option<Pitch>> {
        let result = slf.inner.get_lower_enharmonic().map_err(pitch_error)?;
        if inPlace {
            slf.inner = result;
            slf.write_back()?;
            Ok(None)
        } else {
            Ok(Some(Pitch::wrap(result, false)))
        }
    }

    #[pyo3(signature = (*, inPlace = false, mostCommon = false))]
    fn simplifyEnharmonic(
        mut slf: PyRefMut<'_, Self>,
        inPlace: bool,
        mostCommon: bool,
    ) -> PyResult<Option<Pitch>> {
        let result = slf
            .inner
            .simplify_enharmonic(mostCommon)
            .map_err(pitch_error)?;
        let inferred = slf.spelling_is_inferred;
        if inPlace {
            slf.inner = result;
            slf.write_back()?;
            Ok(None)
        } else {
            Ok(Some(Pitch::wrap(result, inferred)))
        }
    }

    fn isEnharmonic(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(self.inner.is_enharmonic(&pitch_from_any(other)?))
    }

    #[pyo3(signature = (alterLimit = 2))]
    fn getAllCommonEnharmonics(&self, alterLimit: i32) -> Vec<Pitch> {
        self.inner
            .all_common_enharmonics(alterLimit)
            .into_iter()
            .map(|pitch| Pitch::wrap(pitch, false))
            .collect()
    }

    fn getHarmonic(&self, number: u32) -> PyResult<Pitch> {
        Ok(Pitch::wrap(
            self.inner.harmonic(number).map_err(pitch_error)?,
            true,
        ))
    }

    fn harmonicFromFundamental(&self, fundamental: &Bound<'_, PyAny>) -> PyResult<(u32, f64)> {
        self.inner
            .harmonic_from_fundamental(&pitch_from_any(fundamental)?)
            .map_err(pitch_error)
    }

    #[pyo3(signature = (fundamental = None))]
    fn harmonicString(&self, fundamental: Option<&Bound<'_, PyAny>>) -> PyResult<String> {
        let fundamental = fundamental
            .filter(|value| !value.is_none())
            .map(pitch_from_any)
            .transpose()?;
        self.inner
            .harmonic_string(fundamental.as_ref())
            .map_err(pitch_error)
    }

    fn harmonicAndFundamentalFromPitch(&self, target: &Bound<'_, PyAny>) -> PyResult<(u32, Pitch)> {
        let (number, fundamental) = self
            .inner
            .harmonic_and_fundamental_from_pitch(&pitch_from_any(target)?)
            .map_err(pitch_error)?;
        Ok((number, Pitch::wrap(fundamental, false)))
    }

    fn harmonicAndFundamentalStringFromPitch(
        &self,
        fundamental: &Bound<'_, PyAny>,
    ) -> PyResult<String> {
        self.inner
            .harmonic_and_fundamental_string_from_pitch(&pitch_from_any(fundamental)?)
            .map_err(pitch_error)
    }

    #[pyo3(signature = (target, *, minimize = false, inPlace = false))]
    fn transposeBelowTarget(
        mut slf: PyRefMut<'_, Self>,
        target: &Bound<'_, PyAny>,
        minimize: bool,
        inPlace: bool,
    ) -> PyResult<Option<Pitch>> {
        let result = slf
            .inner
            .transpose_below_target(&pitch_from_any(target)?, minimize)
            .map_err(pitch_error)?;
        let inferred = slf.spelling_is_inferred;
        if inPlace {
            slf.inner = result;
            slf.write_back()?;
            Ok(None)
        } else {
            Ok(Some(Pitch::wrap(result, inferred)))
        }
    }

    #[pyo3(signature = (target, *, minimize = false, inPlace = false))]
    fn transposeAboveTarget(
        mut slf: PyRefMut<'_, Self>,
        target: &Bound<'_, PyAny>,
        minimize: bool,
        inPlace: bool,
    ) -> PyResult<Option<Pitch>> {
        let result = slf
            .inner
            .transpose_above_target(&pitch_from_any(target)?, minimize)
            .map_err(pitch_error)?;
        let inferred = slf.spelling_is_inferred;
        if inPlace {
            slf.inner = result;
            slf.write_back()?;
            Ok(None)
        } else {
            Ok(Some(Pitch::wrap(result, inferred)))
        }
    }

    #[pyo3(signature = (*, inPlace = false))]
    fn convertQuarterTonesToMicrotones(
        mut slf: PyRefMut<'_, Self>,
        inPlace: bool,
    ) -> PyResult<Option<Pitch>> {
        let result = slf
            .inner
            .convert_quarter_tones_to_microtones()
            .map_err(pitch_error)?;
        let inferred = slf.spelling_is_inferred;
        if inPlace {
            slf.inner = result;
            slf.write_back()?;
            Ok(None)
        } else {
            Ok(Some(Pitch::wrap(result, inferred)))
        }
    }

    #[pyo3(signature = (*, inPlace = false))]
    fn convertMicrotonesToQuarterTones(
        mut slf: PyRefMut<'_, Self>,
        inPlace: bool,
    ) -> PyResult<Option<Pitch>> {
        let result = slf
            .inner
            .convert_microtones_to_quarter_tones()
            .map_err(pitch_error)?;
        let inferred = slf.spelling_is_inferred;
        if inPlace {
            slf.inner = result;
            slf.write_back()?;
            Ok(None)
        } else {
            Ok(Some(Pitch::wrap(result, inferred)))
        }
    }

    fn informClient(&self) {}

    /// music21's `getStringHarmonic`: given a chord whose second note is
    /// written with a diamond head, the chord comes back with the harmonic
    /// those two pitches sound added on top. A chord not written as a
    /// harmonic answers `False`, as music21 does.
    ///
    /// The pitch it is called on takes no part; music21 reads both pitches
    /// off the chord.
    fn getStringHarmonic<'py>(
        &self,
        py: Python<'py>,
        chordIn: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let chord = chordIn
            .extract::<PyRef<crate::chord::Chord>>()
            .map_err(|_| PitchException::new_err("getStringHarmonic needs a chord.Chord"))?;
        match chord.getStringHarmonic(py)? {
            Some(sounded) => Ok(Bound::new(py, sounded)?.into_any()),
            None => Ok(false.into_pyobject(py)?.to_owned().into_any()),
        }
    }

    /// music21's `updateAccidentalDisplay`: decides whether this pitch's
    /// accidental should be shown, given what came before.
    #[pyo3(signature = (
        *,
        pitchPast = None,
        pitchPastMeasure = None,
        otherSimultaneousPitches = None,
        alteredPitches = None,
        cautionaryPitchClass = true,
        cautionaryAll = false,
        overrideStatus = false,
        cautionaryNotImmediateRepeat = true,
        lastNoteWasTied = false,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn updateAccidentalDisplay(
        &mut self,
        pitchPast: Option<&Bound<'_, PyAny>>,
        pitchPastMeasure: Option<&Bound<'_, PyAny>>,
        otherSimultaneousPitches: Option<&Bound<'_, PyAny>>,
        alteredPitches: Option<&Bound<'_, PyAny>>,
        cautionaryPitchClass: bool,
        cautionaryAll: bool,
        overrideStatus: bool,
        cautionaryNotImmediateRepeat: bool,
        lastNoteWasTied: bool,
    ) -> PyResult<()> {
        let past = pitch_list(pitchPast)?;
        let past_measure = pitch_list(pitchPastMeasure)?;
        let simultaneous = pitch_list(otherSimultaneousPitches)?;
        let altered = pitch_list(alteredPitches)?;
        self.inner
            .update_accidental_display(&AccidentalDisplayOptions {
                pitch_past: &past,
                pitch_past_measure: &past_measure,
                other_simultaneous_pitches: &simultaneous,
                altered_pitches: &altered,
                cautionary_pitch_class: cautionaryPitchClass,
                cautionary_all: cautionaryAll,
                override_status: overrideStatus,
                cautionary_not_immediate_repeat: cautionaryNotImmediateRepeat,
                last_note_was_tied: lastNoteWasTied,
            });
        self.write_back()
    }

    /// music21's `_nameInKeySignature`: whether one of the key signature's
    /// altered pitches has this pitch's step and accidental.
    fn _nameInKeySignature(&self, alteredPitches: &Bound<'_, PyAny>) -> PyResult<bool> {
        if self.inner.explicit_accidental().is_none() {
            return Ok(false);
        }
        let own_name = self.accidental_name();
        for altered in alteredPitches.try_iter()? {
            let altered = altered?;
            let step: String = altered.getattr("step")?.extract()?;
            let accidental = altered.getattr("accidental")?;
            if step == self.step()
                && !accidental.is_none()
                && accidental.getattr("name")?.extract::<String>()? == own_name
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// music21's `_stepInKeySignature`: whether the key signature alters
    /// this pitch's step at all.
    fn _stepInKeySignature(&self, alteredPitches: &Bound<'_, PyAny>) -> PyResult<bool> {
        for altered in alteredPitches.try_iter()? {
            let step: String = altered?.getattr("step")?.extract()?;
            if step == self.step() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// music21's `classes`: the names of everything this object is.
    ///
    /// A pitch is not a `Music21Object` and so gets no music21 half from the
    /// install, but music21's own code still asks it what it is — its
    /// `_extractPitch` tells a pitch from a note by looking here — so the
    /// answer is given directly.
    #[getter]
    fn classes(&self) -> [&'static str; 3] {
        ["Pitch", "ProtoM21Object", "object"]
    }

    #[getter]
    fn classSet(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(pyo3::types::PyFrozenSet::new(py, self.classes())?
            .into_any()
            .unbind())
    }

    /// music21's `_client`: the note this pitch belongs to, where it belongs
    /// to one. Its own `interval._extractPitch` reads it to tell a pitch
    /// from the note around it.
    #[getter]
    fn _client(&self, py: Python<'_>) -> Py<PyAny> {
        match &self.owner {
            Some(owner) => owner.clone_ref(py).into_any(),
            None => py.None(),
        }
    }

    /// music21's `Note.__init__` writes itself here, which is how a pitch
    /// finds the note around it.
    #[setter]
    fn set__client(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) {
        slf.borrow_mut().owner = value.extract::<Py<Note>>().ok();
    }

    /// music21's `groups`: the labels a caller has put on this pitch. The
    /// list is made on first asking and kept, so appending to what a caller
    /// was handed reaches the pitch.
    #[getter]
    fn get_groups(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let own = slf.as_any().getattr("__dict__")?;
        if let Ok(groups) = own.get_item("_groups") {
            return Ok(groups.unbind());
        }
        // music21's own `Groups`, which is a list that refuses a name it
        // already has — `chordify` labels the same pitch once per bar and
        // expects to see the part named once.
        let groups = match py
            .import("music21.base")
            .and_then(|base| base.getattr("Groups"))
            .and_then(|class| class.call0())
        {
            Ok(groups) => groups,
            Err(_) => pyo3::types::PyList::empty(py).into_any(),
        };
        own.set_item("_groups", &groups)?;
        Ok(groups.unbind())
    }

    #[setter]
    fn set_groups(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        slf.as_any().getattr("__dict__")?.set_item("_groups", value)
    }

    // ---- dunders ---------------------------------------------------------

    fn __repr__(&self) -> String {
        format!("<music21.pitch.Pitch {}>", self.__str__())
    }

    fn __str__(&self) -> String {
        let name = self.inner.name_with_octave();
        match self.inner.microtone() {
            Some(microtone) if microtone.cents() != 0.0 => format!("{name}{microtone}"),
            _ => name,
        }
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        let Ok(other) = other.extract::<PyRef<Pitch>>() else {
            return false;
        };
        self.inner.name_with_octave() == other.inner.name_with_octave()
            && self.inner.octave() == other.inner.octave()
            && self.inner.has_accidental() == other.inner.has_accidental()
            && self.microtone_cents() == other.microtone_cents()
    }

    fn __ne__(&self, other: &Bound<'_, PyAny>) -> bool {
        !self.__eq__(other)
    }

    fn __lt__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(self.inner.ps() < pitch_from_any(other)?.ps())
    }

    fn __le__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(self.inner.ps() < pitch_from_any(other)?.ps() || self.__eq__(other))
    }

    fn __gt__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(self.inner.ps() > pitch_from_any(other)?.ps())
    }

    fn __ge__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(self.inner.ps() > pitch_from_any(other)?.ps() || self.__eq__(other))
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.inner.name_with_octave().hash(&mut hasher);
        self.microtone_cents().to_bits().hash(&mut hasher);
        hasher.finish()
    }

    /// A copy as an object of the class it was asked on: music21 compares
    /// two of these by class before anything else, so a copy built as the
    /// bare facade would not equal the original.
    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.borrow().clone();
        crate::copy_as_same_type(slf, copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.borrow().clone();
        crate::copy_as_same_type(slf, copied)
    }
}

/// music21's `pitch.simplifyMultipleEnharmonics`, over facades, names or
/// numbers, with a key or key signature as context.
#[pyfunction]
#[pyo3(name = "simplifyMultipleEnharmonics", signature = (pitches, *, criterion = None, keyContext = None))]
#[allow(non_snake_case)]
pub(crate) fn simplify_multiple_enharmonics(
    pitches: &Bound<'_, PyAny>,
    criterion: Option<&Bound<'_, PyAny>>,
    keyContext: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Pitch>> {
    if criterion.is_some_and(|value| !value.is_none()) {
        return Err(PitchException::new_err(
            "a custom criterion is not supported by the music21-rs facade",
        ));
    }
    let mut inputs = Vec::new();
    for item in pitches.try_iter()? {
        inputs.push(pitch_from_any(&item?)?);
    }
    let signature = match keyContext.filter(|value| !value.is_none()) {
        Some(context) => Some(music21_rs::KeySignature::new(
            context.getattr("sharps")?.extract::<i32>()?,
        )),
        None => None,
    };
    let simplified = music21_rs::pitch::simplify_multiple_enharmonics(&inputs, None, signature)
        .map_err(pitch_error)?;
    Ok(simplified
        .into_iter()
        .map(|pitch| Pitch::wrap(pitch, false))
        .collect())
}

/// music21's `pitch.convertPitchClassToStr`.
#[pyfunction]
#[pyo3(name = "convertPitchClassToStr")]
pub(crate) fn convert_pitch_class_to_str(pitch_class: i32) -> String {
    music21_rs::convert_pitch_class_to_str(pitch_class)
}

/// music21's `pitch.isValidAccidentalName`.
#[pyfunction]
#[pyo3(name = "isValidAccidentalName")]
pub(crate) fn is_valid_accidental_name(name: &str) -> bool {
    RsAccidental::is_valid_name(name)
}

/// music21's `pitch.standardizeAccidentalName`.
#[pyfunction]
#[pyo3(name = "standardizeAccidentalName")]
pub(crate) fn standardize_accidental_name(name: &str) -> PyResult<String> {
    RsAccidental::standardize_name(name).map_err(accidental_error)
}

/// Adds the pitch facades to the module.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Pitch>()?;
    m.add_class::<Accidental>()?;
    m.add_class::<Microtone>()?;
    m.add_function(wrap_pyfunction!(simplify_multiple_enharmonics, m)?)?;
    m.add_function(wrap_pyfunction!(convert_pitch_class_to_str, m)?)?;
    m.add_function(wrap_pyfunction!(is_valid_accidental_name, m)?)?;
    m.add_function(wrap_pyfunction!(standardize_accidental_name, m)?)?;
    for (name, exception) in [
        ("PitchException", m.py().get_type::<PitchException>()),
        (
            "AccidentalException",
            m.py().get_type::<AccidentalException>(),
        ),
        (
            "MicrotoneException",
            m.py().get_type::<MicrotoneException>(),
        ),
    ] {
        exception.setattr("__module__", "music21.pitch")?;
        m.add(name, exception)?;
    }
    Ok(())
}
