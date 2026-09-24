//! music21's ornaments, over the crate's `expressions`.
//!
//! `Ornament` holds the crate's `Ornament`, and each of music21's ornament
//! classes is a class of its own beneath it. What music21 keeps as Python
//! objects -- an accidental a caller handed over, an appoggiatura's interval,
//! the ornamental pitches last resolved -- is kept as those objects, so a
//! caller holding one sees what music21's caller would; the crate answers
//! every musical question from their values. The notes an ornament is played
//! as are copies of the note given, as music21's are, carrying the crate's
//! pitches and lengths. music21's other expressions stay music21's.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString, PyTuple};

use music21_rs_crate::expressions::{
    Ornament as RsOrnament, OrnamentDelay, OrnamentKind, Realization,
};
use music21_rs_crate::{
    Duration as RsDuration, IntervalDirection, KeySignature as RsKeySignature, Note as RsNote,
    Pitch as RsPitch,
};

use crate::pitch::{Pitch, accidental_from_any, pitch_from_any};

pyo3::create_exception!(
    music21_rs_facade,
    ExpressionException,
    crate::Music21Exception
);
pyo3::create_exception!(music21_rs_facade, TremoloException, crate::Music21Exception);

error_into!(expression_error, ExpressionException);

/// Which of music21's ornament families a class belongs to, which decides
/// the members it has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Plain,
    Mordent,
    Trill,
    Turn,
    Appoggiatura,
    Tremolo,
}

/// music21's `expressions.Ornament`.
#[pyclass(
    name = "Ornament",
    module = "music21.expressions",
    subclass,
    skip_from_py_object
)]
pub struct Ornament {
    pub(crate) inner: RsOrnament,
    /// A mordent's or trill's accidental, as the object it was given.
    accidental: Option<Py<PyAny>>,
    /// A turn's upper accidental, as given.
    upper_accidental: Option<Py<PyAny>>,
    /// A turn's lower accidental, as given.
    lower_accidental: Option<Py<PyAny>>,
    /// An appoggiatura's or schleifer's `size`, once it has been asked for
    /// or given.
    size: Option<Py<PyAny>>,
    /// The ornamental pitches `resolveOrnamentalPitches` last worked out.
    ornamental: Vec<Py<PyAny>>,
    connected_to_previous: bool,
}

impl Ornament {
    /// An ornament of music21's `class`, with the keywords its constructor
    /// takes.
    fn initializer(
        class: &str,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let kind = OrnamentKind::from_class_name(class)
            .ok_or_else(|| ExpressionException::new_err(format!("no ornament class {class}")))?;
        let mut made = Self {
            inner: RsOrnament::of_kind(kind),
            accidental: None,
            upper_accidental: None,
            lower_accidental: None,
            size: None,
            ornamental: Vec::new(),
            connected_to_previous: true,
        };
        if let Some(keywords) = keywords {
            if let Some(value) = keywords.get_item("accidental")? {
                if kind.has_fixed_size() {
                    return Err(ExpressionException::new_err(format!(
                        "Cannot initialize {class} with accidental"
                    )));
                }
                if matches!(made.shape(), Shape::Mordent | Shape::Trill) {
                    made.take_accidental(Some(&value))?;
                }
            }
            if made.shape() == Shape::Turn {
                if let Some(value) = keywords.get_item("upperAccidental")? {
                    made.take_upper_accidental(Some(&value))?;
                }
                if let Some(value) = keywords.get_item("lowerAccidental")? {
                    made.take_lower_accidental(Some(&value))?;
                }
                if let Some(value) = keywords.get_item("delay")? {
                    made.inner.set_delay(delay_from_any(&value)?);
                }
            }
        }
        Ok(PyClassInitializer::from(made))
    }

    fn shape(&self) -> Shape {
        let inner = &self.inner;
        if inner.is_a("GeneralMordent") {
            Shape::Mordent
        } else if inner.is_a("Trill") {
            Shape::Trill
        } else if inner.is_a("Turn") {
            Shape::Turn
        } else if inner.is_a("GeneralAppoggiatura") {
            Shape::Appoggiatura
        } else if inner.is_a("Tremolo") {
            Shape::Tremolo
        } else {
            Shape::Plain
        }
    }

    /// Refuses a member music21's class for this ornament has not got, as a
    /// missing attribute.
    fn member(slf: &Bound<'_, Self>, member: &str, shapes: &[Shape]) -> PyResult<()> {
        if shapes.contains(&slf.borrow().shape()) {
            return Ok(());
        }
        Err(pyo3::exceptions::PyAttributeError::new_err(format!(
            "'{}' object has no attribute '{member}'",
            slf.get_type().name()?
        )))
    }

    fn take_accidental(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let value = value.filter(|value| !value.is_none());
        let read = value.map(accidental_from_any).transpose()?;
        self.inner.set_accidental(read).map_err(expression_error)?;
        self.accidental = value.map(|value| value.clone().unbind());
        Ok(())
    }

    fn take_upper_accidental(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let value = value.filter(|value| !value.is_none());
        self.inner
            .set_upper_accidental(value.map(accidental_from_any).transpose()?);
        self.upper_accidental = value.map(|value| value.clone().unbind());
        Ok(())
    }

    fn take_lower_accidental(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let value = value.filter(|value| !value.is_none());
        self.inner
            .set_lower_accidental(value.map(accidental_from_any).transpose()?);
        self.lower_accidental = value.map(|value| value.clone().unbind());
        Ok(())
    }

    /// The crate's ornament with the accidentals read afresh off the objects
    /// a caller handed over, since a caller may have changed one since --
    /// shown or hidden it, most often.
    fn synced(&self, py: Python<'_>) -> PyResult<RsOrnament> {
        let mut inner = self.inner.clone();
        if let Some(accidental) = &self.accidental {
            inner
                .set_accidental(Some(accidental_from_any(accidental.bind(py))?))
                .map_err(expression_error)?;
        }
        if let Some(accidental) = &self.upper_accidental {
            inner.set_upper_accidental(Some(accidental_from_any(accidental.bind(py))?));
        }
        if let Some(accidental) = &self.lower_accidental {
            inner.set_lower_accidental(Some(accidental_from_any(accidental.bind(py))?));
        }
        Ok(inner)
    }

    /// Whether `self` is among `source`'s expressions, and where.
    fn place_in_expressions(slf: &Bound<'_, Self>, source: &Bound<'_, PyAny>) -> Option<usize> {
        let expressions = source.getattr("expressions").ok()?;
        expressions
            .try_iter()
            .ok()?
            .position(|expression| expression.is_ok_and(|expression| expression.is(slf)))
    }

    /// What is left of `source` once the ornament has taken its notes:
    /// `source` itself when realizing in place, a copy otherwise, lasting
    /// `length` and without this ornament among its expressions.
    fn remainder<'py>(
        slf: &Bound<'py, Self>,
        source: &Bound<'py, PyAny>,
        length: f64,
        in_place: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let place = Self::place_in_expressions(slf, source);
        let remainder = if in_place {
            source.clone()
        } else {
            deep_copy(source)?
        };
        remainder
            .getattr("duration")?
            .setattr("quarterLength", crate::duration::op_frac(py, length)?)?;
        if let Some(place) = place {
            remainder
                .getattr("expressions")?
                .call_method1("pop", (place,))?;
        }
        Ok(remainder)
    }
}

/// A copy of `source` played as `played`: its length, and its pitch where
/// the ornament moved it.
fn played_as<'py>(
    source: &Bound<'py, PyAny>,
    played: &RsNote,
    written: Option<&RsPitch>,
    clear_expressions: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py = source.py();
    let copy = deep_copy(source)?;
    let length = played.duration().map_or(1.0, RsDuration::quarter_length);
    copy.getattr("duration")?
        .setattr("quarterLength", crate::duration::op_frac(py, length)?)?;
    if let Some(written) = written
        && played.pitch() != written
    {
        let pitch = played.pitch().clone();
        let inferred = pitch.spelling_is_inferred();
        let object =
            crate::installed_new(py, "music21.pitch", "Pitch", Pitch::wrap(pitch, inferred))?;
        copy.setattr("pitch", object)?;
    }
    if clear_expressions {
        copy.setattr("expressions", PyList::empty(py))?;
    }
    Ok(copy)
}

fn deep_copy<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    value
        .py()
        .import("copy")?
        .getattr("deepcopy")?
        .call1((value,))
}

/// The key an ornament on `source` is heard in: the one given, else the one
/// in force where `source` stands, else no sharps or flats -- music21's
/// `keySig or srcObj.getContextByClass(KeySignature) or KeySignature(0)`.
fn key_for(
    source: &Bound<'_, PyAny>,
    given: Option<&Bound<'_, PyAny>>,
) -> PyResult<RsKeySignature> {
    let given = match given {
        Some(given) if !given.is_none() && given.is_truthy()? => Some(given.clone()),
        _ => None,
    };
    let found = match given {
        Some(given) => Some(given),
        None => source
            .call_method1("getContextByClass", ("KeySignature",))
            .ok()
            .filter(|found| !found.is_none()),
    };
    match found {
        None => Ok(RsKeySignature::new(0)),
        Some(found) => {
            if let Ok(facade) = found.extract::<PyRef<'_, crate::key::KeySignature>>() {
                return Ok(facade.signature.clone());
            }
            Ok(RsKeySignature::new(found.getattr("sharps")?.extract()?))
        }
    }
}

/// The pitch an ornament on `source` stands on: the last of its pitches, as
/// music21 takes, or none for an unpitched note or a rest.
fn source_pitch(source: &Bound<'_, PyAny>) -> PyResult<Option<RsPitch>> {
    let pitches = source.getattr("pitches")?;
    let count = pitches.len()?;
    if count == 0 {
        return Ok(None);
    }
    Ok(Some(pitch_from_any(&pitches.get_item(count - 1)?)?))
}

/// A turn's delay as music21 writes it: one of its `OrnamentDelay` members,
/// or a length.
fn delay_from_any(value: &Bound<'_, PyAny>) -> PyResult<OrnamentDelay> {
    if let Ok(text) = value.cast::<PyString>() {
        return match text.to_str()? {
            "noDelay" => Ok(OrnamentDelay::NoDelay),
            "defaultDelay" => Ok(OrnamentDelay::Default),
            other => Err(ExpressionException::new_err(format!(
                "Invalid delay: {other}"
            ))),
        };
    }
    let length: f64 = value.extract()?;
    if length <= 0.0 {
        return Ok(OrnamentDelay::NoDelay);
    }
    Ok(OrnamentDelay::Timed(length))
}

/// One of music21's `OrnamentDelay` members, or the text it stands for
/// where music21 is not there to ask.
fn delay_member<'py>(py: Python<'py>, member: &str, value: &str) -> PyResult<Bound<'py, PyAny>> {
    match py.import("music21.common.enums") {
        Ok(enums) => enums.getattr("OrnamentDelay")?.getattr(member),
        Err(_) => Ok(PyString::new(py, value).into_any()),
    }
}

/// Whether an interval moves nowhere, as music21's `isUnison` reads one: an
/// interval or diatonic interval named `P1` (and, for an interval, of no
/// semitones), a chromatic interval of none, and never anything else.
fn is_unison(interval: &Bound<'_, PyAny>) -> PyResult<bool> {
    if interval
        .extract::<PyRef<'_, crate::interval::Interval>>()
        .is_ok()
    {
        return Ok(interval.getattr("name")?.extract::<String>()? == "P1"
            && interval
                .getattr("chromatic")?
                .getattr("semitones")?
                .extract::<f64>()?
                == 0.0);
    }
    if interval
        .extract::<PyRef<'_, crate::interval::DiatonicInterval>>()
        .is_ok()
    {
        return Ok(interval.getattr("name")?.extract::<String>()? == "P1");
    }
    if interval
        .extract::<PyRef<'_, crate::interval::ChromaticInterval>>()
        .is_ok()
    {
        return Ok(interval.getattr("semitones")?.extract::<f64>()? == 0.0);
    }
    Ok(false)
}

fn direction_word(direction: Option<IntervalDirection>) -> &'static str {
    match direction {
        Some(IntervalDirection::Ascending) => "up",
        Some(IntervalDirection::Descending) => "down",
        _ => "",
    }
}

#[pymethods]
impl Ornament {
    #[new]
    #[pyo3(signature = (**keywords))]
    fn new(keywords: Option<&Bound<'_, PyDict>>) -> PyResult<PyClassInitializer<Self>> {
        Self::initializer("Ornament", keywords)
    }

    /// music21's `name`: the class as lower-case words, with an ornament's
    /// accidentals and a turn's delay beside it.
    #[getter]
    fn get_name(&self, py: Python<'_>) -> PyResult<String> {
        Ok(self.synced(py)?.name())
    }

    /// How long each ornamental note lasts, before any scaling to fit.
    #[getter]
    fn get_quarterLength<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::duration::op_frac(py, self.inner.quarter_length())
    }

    #[setter]
    fn set_quarterLength(&mut self, value: f64) {
        self.inner.set_quarter_length(value);
    }

    /// Whether the ornament shrinks to fit a note too short for it.
    #[getter]
    fn get_autoScale(&self) -> bool {
        self.inner.auto_scale()
    }

    #[setter]
    fn set_autoScale(&mut self, value: bool) {
        self.inner.set_auto_scale(value);
    }

    #[getter]
    fn get_connectedToPrevious(&self) -> bool {
        self.connected_to_previous
    }

    #[setter]
    fn set_connectedToPrevious(&mut self, value: bool) {
        self.connected_to_previous = value;
    }

    #[getter]
    fn get_placement(&self) -> Option<&str> {
        self.inner.placement()
    }

    #[setter]
    fn set_placement(&mut self, value: Option<String>) {
        self.inner.set_placement(value);
    }

    /// Which part of a note split across a tie keeps the ornament.
    #[getter]
    fn get_tieAttach(&self) -> &str {
        self.inner.tie_attach()
    }

    #[setter]
    fn set_tieAttach(&mut self, value: String) {
        self.inner.set_tie_attach(value);
    }

    /// Which way a mordent, trill or appoggiatura goes: `'up'`, `'down'`, or
    /// nothing for a general class that has not said.
    #[getter]
    fn get_direction(slf: &Bound<'_, Self>) -> PyResult<&'static str> {
        Self::member(
            slf,
            "direction",
            &[Shape::Mordent, Shape::Trill, Shape::Appoggiatura],
        )?;
        Ok(direction_word(slf.borrow().inner.direction()))
    }

    /// A mordent's or trill's accidental; always nothing for a half-step or
    /// whole-step one.
    #[getter]
    fn get_accidental(slf: &Bound<'_, Self>) -> PyResult<Option<Py<PyAny>>> {
        Self::member(slf, "accidental", &[Shape::Mordent, Shape::Trill])?;
        let py = slf.py();
        Ok(slf
            .borrow()
            .accidental
            .as_ref()
            .map(|accidental| accidental.clone_ref(py)))
    }

    #[setter]
    fn set_accidental(slf: &Bound<'_, Self>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        Self::member(slf, "accidental", &[Shape::Mordent, Shape::Trill])?;
        slf.borrow_mut().take_accidental(value)
    }

    #[getter]
    fn get_upperAccidental(slf: &Bound<'_, Self>) -> PyResult<Option<Py<PyAny>>> {
        Self::member(slf, "upperAccidental", &[Shape::Turn])?;
        let py = slf.py();
        Ok(slf
            .borrow()
            .upper_accidental
            .as_ref()
            .map(|accidental| accidental.clone_ref(py)))
    }

    #[setter]
    fn set_upperAccidental(
        slf: &Bound<'_, Self>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        Self::member(slf, "upperAccidental", &[Shape::Turn])?;
        slf.borrow_mut().take_upper_accidental(value)
    }

    #[getter]
    fn get_lowerAccidental(slf: &Bound<'_, Self>) -> PyResult<Option<Py<PyAny>>> {
        Self::member(slf, "lowerAccidental", &[Shape::Turn])?;
        let py = slf.py();
        Ok(slf
            .borrow()
            .lower_accidental
            .as_ref()
            .map(|accidental| accidental.clone_ref(py)))
    }

    #[setter]
    fn set_lowerAccidental(
        slf: &Bound<'_, Self>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        Self::member(slf, "lowerAccidental", &[Shape::Turn])?;
        slf.borrow_mut().take_lower_accidental(value)
    }

    /// A turn's delay: music21's `OrnamentDelay.NO_DELAY` or `DEFAULT_DELAY`,
    /// or how long the note sounds before the turn begins.
    #[getter]
    fn get_delay<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::member(slf, "delay", &[Shape::Turn])?;
        let py = slf.py();
        let delay = slf.borrow().inner.delay();
        match delay {
            OrnamentDelay::NoDelay => delay_member(py, "NO_DELAY", "noDelay"),
            OrnamentDelay::Default => delay_member(py, "DEFAULT_DELAY", "defaultDelay"),
            OrnamentDelay::Timed(length) => crate::duration::op_frac(py, length),
        }
    }

    #[setter]
    fn set_delay(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        Self::member(slf, "delay", &[Shape::Turn])?;
        let delay = delay_from_any(value)?;
        slf.borrow_mut().inner.set_delay(delay);
        Ok(())
    }

    #[getter]
    fn get_isDelayed(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Self::member(slf, "isDelayed", &[Shape::Turn])?;
        Ok(slf.borrow().inner.is_delayed())
    }

    /// Whether a trill ends on two grace notes turning back to the main one.
    #[getter]
    fn get_nachschlag(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Self::member(slf, "nachschlag", &[Shape::Trill])?;
        Ok(slf.borrow().inner.nachschlag())
    }

    #[setter]
    fn set_nachschlag(slf: &Bound<'_, Self>, value: bool) -> PyResult<()> {
        Self::member(slf, "nachschlag", &[Shape::Trill])?;
        slf.borrow_mut().inner.set_nachschlag(value);
        Ok(())
    }

    /// How many strokes cross a tremolo's stem.
    #[getter]
    fn get_numberOfMarks(slf: &Bound<'_, Self>) -> PyResult<u8> {
        Self::member(slf, "numberOfMarks", &[Shape::Tremolo])?;
        Ok(slf.borrow().inner.number_of_marks())
    }

    #[setter]
    fn set_numberOfMarks(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        Self::member(slf, "numberOfMarks", &[Shape::Tremolo])?;
        let refused = || TremoloException::new_err("Number of marks must be a number from 0 to 8");
        let py = slf.py();
        let whole = match py.import("builtins")?.getattr("int")?.call1((value,)) {
            Ok(whole) => whole,
            Err(error) if error.is_instance_of::<pyo3::exceptions::PyValueError>(py) => {
                return Err(refused());
            }
            Err(error) => return Err(error),
        };
        let marks = whole
            .extract::<i64>()
            .ok()
            .and_then(|marks| u8::try_from(marks).ok())
            .ok_or_else(refused)?;
        slf.borrow_mut()
            .inner
            .set_number_of_marks(marks)
            .map_err(|_| refused())
    }

    #[getter]
    fn get_measured(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Self::member(slf, "measured", &[Shape::Tremolo])?;
        Ok(slf.borrow().inner.measured())
    }

    #[setter]
    fn set_measured(slf: &Bound<'_, Self>, value: bool) -> PyResult<()> {
        Self::member(slf, "measured", &[Shape::Tremolo])?;
        slf.borrow_mut().inner.set_measured(value);
        Ok(())
    }

    /// An appoggiatura's interval, or a schleifer's generic second.
    #[getter]
    fn get_size(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let shape = slf.borrow().shape();
        let schleifer = slf.borrow().inner.kind() == OrnamentKind::Schleifer;
        if shape != Shape::Appoggiatura && !schleifer {
            Self::member(slf, "size", &[Shape::Appoggiatura])?;
        }
        if let Some(size) = &slf.borrow().size {
            return Ok(size.clone_ref(py));
        }
        let made = if schleifer {
            let second = music21_rs_crate::GenericInterval::new(2)
                .map_err(crate::interval::interval_error)?;
            crate::installed_new(
                py,
                "music21.interval",
                "GenericInterval",
                crate::interval::GenericInterval::wrap(second),
            )?
            .into_any()
        } else {
            match slf.borrow().inner.appoggiatura_size() {
                Some(size) => crate::interval::interval_object(py, size.clone())?,
                None => PyString::new(py, "").into_any().unbind(),
            }
        };
        slf.borrow_mut().size = Some(made.clone_ref(py));
        Ok(made)
    }

    #[setter]
    fn set_size(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let shape = slf.borrow().shape();
        if shape == Shape::Appoggiatura {
            let read =
                if value.is_none() || value.extract::<String>().is_ok_and(|text| text.is_empty()) {
                    None
                } else {
                    Some(crate::interval::interval_from_any(value)?)
                };
            slf.borrow_mut().inner.set_appoggiatura_size(read);
        }
        slf.borrow_mut().size = Some(value.clone().unbind());
        Ok(())
    }

    /// The interval from `srcObj` to a mordent's or trill's ornamental note,
    /// or to a turn's upper or lower one, in `keySig` or the key in force
    /// where the note stands. A note with no pitch gets a unison.
    #[pyo3(signature = (srcObj, which = None, *, keySig = None))]
    fn getSize(
        slf: &Bound<'_, Self>,
        srcObj: &Bound<'_, PyAny>,
        which: Option<&str>,
        keySig: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        Self::member(slf, "getSize", &[Shape::Mordent, Shape::Trill, Shape::Turn])?;
        let py = slf.py();
        let inner = slf.borrow().synced(py)?;
        let shape = slf.borrow().shape();
        let unison = || -> PyResult<Py<PyAny>> {
            crate::interval::interval_object(
                py,
                music21_rs_crate::Interval::from_name("P1")
                    .map_err(crate::interval::interval_error)?,
            )
        };
        let size = if shape == Shape::Turn {
            let which = match which {
                Some("upper") => music21_rs_crate::expressions::TurnNote::Upper,
                Some("lower") => music21_rs_crate::expressions::TurnNote::Lower,
                _ => {
                    return Err(ExpressionException::new_err(
                        "Turn.getSize requires 'which' parameter be set to 'upper' or 'lower'",
                    ));
                }
            };
            let Some(source) = source_pitch(srcObj)? else {
                return unison();
            };
            let key = key_for(srcObj, keySig)?;
            inner
                .turn_size(&source, which, &key)
                .map_err(expression_error)?
        } else {
            if inner.direction().is_none() {
                let what = if shape == Shape::Mordent {
                    "mordent"
                } else {
                    "trill"
                };
                return Err(ExpressionException::new_err(format!(
                    "Cannot compute {what} size if I do not know its direction"
                )));
            }
            let Some(source) = source_pitch(srcObj)? else {
                return unison();
            };
            let key = key_for(srcObj, keySig)?;
            inner.size(&source, &key).map_err(expression_error)?
        };
        crate::interval::interval_object(py, size)
    }

    /// Works out the ornamental pitches for an ornament on `srcObj`, in
    /// `keySig` or the key in force where it stands, and keeps them for
    /// `ornamentalPitches`. Does nothing for a note with no pitch, or for an
    /// ornament with no ornamental pitches of its own.
    #[pyo3(signature = (srcObj, *, keySig = None))]
    fn resolveOrnamentalPitches(
        slf: &Bound<'_, Self>,
        srcObj: &Bound<'_, PyAny>,
        keySig: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let py = slf.py();
        let shape = slf.borrow().shape();
        if !matches!(shape, Shape::Mordent | Shape::Trill | Shape::Turn) {
            return Ok(());
        }
        let Some(source) = source_pitch(srcObj)? else {
            return Ok(());
        };
        let inner = slf.borrow().synced(py)?;
        if inner.direction().is_none() && shape != Shape::Turn {
            // music21 asks for the size first, which refuses.
            Self::getSize(slf, srcObj, None, keySig)?;
        }
        let key = key_for(srcObj, keySig)?;
        let pitches = inner
            .ornamental_pitches(&source, &key)
            .map_err(expression_error)?;
        let mut objects = Vec::with_capacity(pitches.len());
        for pitch in pitches {
            let inferred = pitch.spelling_is_inferred();
            objects.push(
                crate::installed_new(py, "music21.pitch", "Pitch", Pitch::wrap(pitch, inferred))?
                    .into_any(),
            );
        }
        slf.borrow_mut().ornamental = objects;
        Ok(())
    }

    /// The ornamental pitches `resolveOrnamentalPitches` last worked out.
    #[getter]
    fn get_ornamentalPitches<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyTuple>> {
        let py = slf.py();
        let me = slf.borrow();
        PyTuple::new(py, me.ornamental.iter().map(|pitch| pitch.clone_ref(py)))
    }

    /// A mordent's or trill's one ornamental pitch, once resolved.
    #[getter]
    fn get_ornamentalPitch(slf: &Bound<'_, Self>) -> PyResult<Option<Py<PyAny>>> {
        Self::member(slf, "ornamentalPitch", &[Shape::Mordent, Shape::Trill])?;
        let py = slf.py();
        Ok(slf
            .borrow()
            .ornamental
            .first()
            .map(|pitch| pitch.clone_ref(py)))
    }

    /// A turn's upper ornamental pitch, once resolved.
    #[getter]
    fn get_upperOrnamentalPitch(slf: &Bound<'_, Self>) -> PyResult<Option<Py<PyAny>>> {
        Self::member(slf, "upperOrnamentalPitch", &[Shape::Turn])?;
        let py = slf.py();
        Ok(slf
            .borrow()
            .ornamental
            .first()
            .map(|pitch| pitch.clone_ref(py)))
    }

    /// A turn's lower ornamental pitch, once resolved.
    #[getter]
    fn get_lowerOrnamentalPitch(slf: &Bound<'_, Self>) -> PyResult<Option<Py<PyAny>>> {
        Self::member(slf, "lowerOrnamentalPitch", &[Shape::Turn])?;
        let py = slf.py();
        Ok(slf
            .borrow()
            .ornamental
            .get(1)
            .map(|pitch| pitch.clone_ref(py)))
    }

    /// Decides whether each ornamental pitch shows its accidental, as a
    /// pitch's `updateAccidentalDisplay` does, but never tied. An accidental
    /// given the ornament with a display status of its own says so outright.
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
    ))]
    #[allow(clippy::too_many_arguments)]
    fn updateAccidentalDisplay(
        slf: &Bound<'_, Self>,
        pitchPast: Option<&Bound<'_, PyAny>>,
        pitchPastMeasure: Option<&Bound<'_, PyAny>>,
        otherSimultaneousPitches: Option<&Bound<'_, PyAny>>,
        alteredPitches: Option<&Bound<'_, PyAny>>,
        cautionaryPitchClass: bool,
        cautionaryAll: bool,
        overrideStatus: bool,
        cautionaryNotImmediateRepeat: bool,
    ) -> PyResult<()> {
        let _ = otherSimultaneousPitches;
        let py = slf.py();
        let trill = slf.borrow().shape() == Shape::Trill;
        let pairs: Vec<(Py<PyAny>, Option<Py<PyAny>>)> = {
            let me = slf.borrow();
            let governing = match me.shape() {
                Shape::Mordent | Shape::Trill => vec![me.accidental.as_ref()],
                Shape::Turn => vec![me.upper_accidental.as_ref(), me.lower_accidental.as_ref()],
                _ => return Ok(()),
            };
            me.ornamental
                .iter()
                .zip(governing)
                .map(|(pitch, accidental)| {
                    (
                        pitch.clone_ref(py),
                        accidental.map(|accidental| accidental.clone_ref(py)),
                    )
                })
                .collect()
        };
        for (pitch, accidental) in pairs {
            let pitch = pitch.bind(py);
            // A trill takes whatever its accidental says, decided or not; a
            // mordent and a turn only an accidental whose showing is decided.
            let status = match &accidental {
                Some(accidental) => Some(accidental.bind(py).getattr("displayStatus")?),
                None => None,
            }
            .filter(|status| trill || !status.is_none());
            if let Some(status) = status {
                if pitch.getattr("accidental")?.is_none() {
                    let natural = crate::installed_new(
                        py,
                        "music21.pitch",
                        "Accidental",
                        crate::pitch::Accidental::from_inner(
                            music21_rs_crate::Accidental::new("natural")
                                .map_err(crate::pitch::pitch_error)?,
                        ),
                    )?;
                    pitch.setattr("accidental", natural)?;
                }
                pitch
                    .getattr("accidental")?
                    .setattr("displayStatus", status)?;
                continue;
            }
            let keywords = PyDict::new(py);
            keywords.set_item("pitchPast", pitchPast)?;
            keywords.set_item("pitchPastMeasure", pitchPastMeasure)?;
            keywords.set_item("alteredPitches", alteredPitches)?;
            keywords.set_item("cautionaryPitchClass", cautionaryPitchClass)?;
            keywords.set_item("cautionaryAll", cautionaryAll)?;
            keywords.set_item("overrideStatus", overrideStatus)?;
            keywords.set_item("cautionaryNotImmediateRepeat", cautionaryNotImmediateRepeat)?;
            keywords.set_item("lastNoteWasTied", false)?;
            pitch.call_method("updateAccidentalDisplay", (), Some(&keywords))?;
        }
        Ok(())
    }

    /// Adds two copies of `srcObj` to `fillObjects`, each lasting `useQL`
    /// (the ornament's own length by default), the second moved by
    /// `transposeInterval`: how a mordent or trill builds its notes.
    #[pyo3(signature = (srcObj, fillObjects, transposeInterval, *, useQL = None))]
    fn fillListOfRealizedNotes(
        slf: &Bound<'_, Self>,
        srcObj: &Bound<'_, PyAny>,
        fillObjects: &Bound<'_, PyAny>,
        transposeInterval: &Bound<'_, PyAny>,
        useQL: Option<f64>,
    ) -> PyResult<()> {
        let py = slf.py();
        let transposed = !is_unison(transposeInterval)?;
        if transposed && !srcObj.hasattr("transpose")? {
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "Expected note; got {}",
                srcObj.get_type().repr()?
            )));
        }
        let length = useQL.unwrap_or_else(|| slf.borrow().inner.quarter_length());
        let length = crate::duration::op_frac(py, length)?;
        let first = deep_copy(srcObj)?;
        first
            .getattr("duration")?
            .setattr("quarterLength", &length)?;
        let second = deep_copy(srcObj)?;
        second
            .getattr("duration")?
            .setattr("quarterLength", &length)?;
        if transposed {
            let keywords = PyDict::new(py);
            keywords.set_item("inPlace", true)?;
            second.call_method("transpose", (transposeInterval,), Some(&keywords))?;
        }
        fillObjects.call_method1("append", (first,))?;
        fillObjects.call_method1("append", (second,))?;
        Ok(())
    }

    /// The notes `srcObj` is played as under this ornament: the notes
    /// before what is left of it, what is left (or nothing, where the
    /// ornament takes the whole note), and the notes after. Each is a copy
    /// of `srcObj`, except that what is left is `srcObj` itself when
    /// realizing in place.
    #[pyo3(signature = (srcObj, *, keySig = None, inPlace = false))]
    fn realize<'py>(
        slf: &Bound<'py, Self>,
        srcObj: &Bound<'py, PyAny>,
        keySig: Option<&Bound<'py, PyAny>>,
        inPlace: bool,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let py = slf.py();
        let shape = slf.borrow().shape();
        let inner = slf.borrow().synced(py)?;
        if shape == Shape::Plain {
            let main = if inPlace {
                srcObj.clone()
            } else {
                deep_copy(srcObj)?
            };
            return PyTuple::new(
                py,
                [
                    PyList::empty(py).into_any(),
                    main,
                    PyList::empty(py).into_any(),
                ],
            );
        }
        let written = source_pitch(srcObj)?;
        let length: f64 = srcObj
            .getattr("duration")?
            .getattr("quarterLength")?
            .extract()?;
        // An unpitched note is played on no pitch at all; the crate is
        // asked about a stand-in and only its lengths are read.
        let mut value = RsNote::from_pitch(
            written
                .clone()
                .map_or_else(|| RsPitch::from_name("C4"), Ok)
                .map_err(crate::pitch::pitch_error)?,
        );
        value.set_duration(RsDuration::new(length).map_err(expression_error)?);
        let key = if matches!(shape, Shape::Mordent | Shape::Trill | Shape::Turn) {
            key_for(srcObj, keySig)?
        } else {
            RsKeySignature::new(0)
        };
        let played: Realization = inner.realize(&value, &key).map_err(|error| {
            if shape == Shape::Tremolo {
                TremoloException::new_err(crate::pitch::message(&error))
            } else {
                expression_error(error)
            }
        })?;
        let written = written.as_ref();
        let copies = |notes: &[RsNote], clear: bool| -> PyResult<Bound<'py, PyList>> {
            let list = PyList::empty(py);
            for note in notes {
                list.append(played_as(srcObj, note, written, clear)?)?;
            }
            Ok(list)
        };
        let main_length = played
            .main
            .as_ref()
            .map(|main| main.duration().map_or(1.0, RsDuration::quarter_length));
        let (before, main, after) = match shape {
            Shape::Mordent | Shape::Appoggiatura => {
                let before = copies(&played.before, false)?;
                let main = Self::remainder(slf, srcObj, main_length.unwrap_or(0.0), inPlace)?;
                (before, Some(main), PyList::empty(py))
            }
            Shape::Trill => {
                let before = copies(&played.before, false)?;
                if inPlace && let Some(place) = Self::place_in_expressions(slf, srcObj) {
                    srcObj
                        .getattr("expressions")?
                        .call_method1("pop", (place,))?;
                }
                let after = copies(&played.after, true)?;
                (before, None, after)
            }
            Shape::Turn => {
                let after = copies(&played.after, true)?;
                let main = match main_length {
                    Some(length) => Some(Self::remainder(slf, srcObj, length, inPlace)?),
                    None => None,
                };
                (PyList::empty(py), main, after)
            }
            Shape::Tremolo => {
                // music21 splits the note, which ties the pieces and keeps
                // what a split keeps; the crate says how long each is.
                let mut remaining = if inPlace {
                    srcObj.clone()
                } else {
                    deep_copy(srcObj)?
                };
                if let Some(place) = Self::place_in_expressions(slf, &remaining) {
                    remaining
                        .getattr("expressions")?
                        .call_method1("pop", (place,))?;
                }
                let pieces = PyList::empty(py);
                let lengths: Vec<f64> = played
                    .before
                    .iter()
                    .map(|note| note.duration().map_or(1.0, RsDuration::quarter_length))
                    .collect();
                let keywords = PyDict::new(py);
                keywords.set_item("retainOrigin", false)?;
                for piece in lengths.iter().take(lengths.len().saturating_sub(1)) {
                    let split = remaining.call_method(
                        "splitAtQuarterLength",
                        (crate::duration::op_frac(py, *piece)?,),
                        Some(&keywords),
                    )?;
                    pieces.append(split.get_item(0)?)?;
                    remaining = split.get_item(1)?;
                }
                pieces.append(remaining)?;
                (pieces, None, PyList::empty(py))
            }
            Shape::Plain => unreachable!("answered above"),
        };
        let main = main.unwrap_or_else(|| py.None().into_bound(py));
        PyTuple::new(py, [before.into_any(), main, after.into_any()])
    }

    fn _reprInternal(&self) -> &'static str {
        ""
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        Ok(format!("<music21.expressions.{}>", slf.get_type().name()?))
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let copier = py.import("copy")?.getattr("deepcopy")?;
        let copy_of = |value: &Py<PyAny>| -> PyResult<Py<PyAny>> {
            Ok(match memo {
                Some(memo) => copier.call1((value.bind(py), memo))?.unbind(),
                None => copier.call1((value.bind(py),))?.unbind(),
            })
        };
        let copied = slf.get_type().call0()?;
        {
            let me = slf.borrow();
            let accidental = me.accidental.as_ref().map(&copy_of).transpose()?;
            let upper = me.upper_accidental.as_ref().map(&copy_of).transpose()?;
            let lower = me.lower_accidental.as_ref().map(&copy_of).transpose()?;
            let size = me.size.as_ref().map(&copy_of).transpose()?;
            let ornamental = me
                .ornamental
                .iter()
                .map(&copy_of)
                .collect::<PyResult<Vec<_>>>()?;
            let mut copy = copied.extract::<PyRefMut<'_, Self>>()?;
            copy.inner = me.inner.clone();
            copy.accidental = accidental;
            copy.upper_accidental = upper;
            copy.lower_accidental = lower;
            copy.size = size;
            copy.ornamental = ornamental;
            copy.connected_to_previous = me.connected_to_previous;
        }
        Ok(copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::__deepcopy__(slf, None)
    }

    /// music21 freezes a score by pickling it, and what this object is lives
    /// in Rust where a pickle cannot see it.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        let py = slf.py();
        let extra = PyDict::new(py);
        {
            let me = slf.borrow();
            extra.set_item("accidental", me.accidental.as_ref())?;
            extra.set_item("upperAccidental", me.upper_accidental.as_ref())?;
            extra.set_item("lowerAccidental", me.lower_accidental.as_ref())?;
            extra.set_item("size", me.size.as_ref())?;
            extra.set_item(
                "ornamentalPitches",
                PyList::new(py, me.ornamental.iter().map(|pitch| pitch.clone_ref(py)))?,
            )?;
            extra.set_item("connectedToPrevious", me.connected_to_previous)?;
        }
        crate::pickled_extra(slf, &slf.borrow().inner, Some(&extra))
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let (inner, extra) = crate::unpickled_extra::<_, RsOrnament>(slf, state)?;
        let mut me = slf.borrow_mut();
        if let Some(inner) = inner {
            me.inner = inner;
        }
        if let Some(extra) = extra {
            let extra = extra.bind(py);
            let held = |name: &str| -> Option<Py<PyAny>> {
                extra
                    .get_item(name)
                    .ok()
                    .filter(|value| !value.is_none())
                    .map(Bound::unbind)
            };
            me.accidental = held("accidental");
            me.upper_accidental = held("upperAccidental");
            me.lower_accidental = held("lowerAccidental");
            me.size = held("size");
            if let Ok(pitches) = extra.get_item("ornamentalPitches") {
                me.ornamental = pitches
                    .try_iter()?
                    .map(|pitch| pitch.map(Bound::unbind))
                    .collect::<PyResult<_>>()?;
            }
            if let Ok(connected) = extra.get_item("connectedToPrevious") {
                me.connected_to_previous = connected.extract()?;
            }
        }
        Ok(())
    }
}

macro_rules! ornament_kinds {
    ($(($class:ident, $name:literal, $parent:ident, [$($chain:ident),*])),* $(,)?) => {
        $(
            #[doc = concat!("music21's `expressions.", $name, "`.")]
            #[pyclass(name = $name, module = "music21.expressions", extends = $parent, subclass)]
            pub struct $class;

            #[pymethods]
            impl $class {
                #[new]
                #[pyo3(signature = (**keywords))]
                fn new(keywords: Option<&Bound<'_, PyDict>>) -> PyResult<PyClassInitializer<Self>> {
                    Ok(Ornament::initializer($name, keywords)?
                        $(.add_subclass($chain))*
                        .add_subclass($class))
                }
            }
        )*

        fn register_kinds(m: &Bound<'_, PyModule>) -> PyResult<()> {
            $(m.add_class::<$class>()?;)*
            Ok(())
        }

        /// The names this facade replaces in `music21.expressions`.
        pub const NAMES: &[&str] = &[
            "Ornament",
            "ExpressionException",
            "TremoloException",
            "Trill",
            $($name),*
        ];
    };
}

ornament_kinds!(
    (GeneralAppoggiatura, "GeneralAppoggiatura", Ornament, []),
    (GeneralMordent, "GeneralMordent", Ornament, []),
    (Schleifer, "Schleifer", Ornament, []),
    (Tremolo, "Tremolo", Ornament, []),
    (Turn, "Turn", Ornament, []),
    (
        Appoggiatura,
        "Appoggiatura",
        GeneralAppoggiatura,
        [GeneralAppoggiatura]
    ),
    (
        InvertedAppoggiatura,
        "InvertedAppoggiatura",
        GeneralAppoggiatura,
        [GeneralAppoggiatura]
    ),
    (Mordent, "Mordent", GeneralMordent, [GeneralMordent]),
    (
        InvertedMordent,
        "InvertedMordent",
        GeneralMordent,
        [GeneralMordent]
    ),
    (HalfStepTrill, "HalfStepTrill", Trill, [Trill]),
    (WholeStepTrill, "WholeStepTrill", Trill, [Trill]),
    (InvertedTrill, "InvertedTrill", Trill, [Trill]),
    (Shake, "Shake", Trill, [Trill]),
    (InvertedTurn, "InvertedTurn", Turn, [Turn]),
    (
        HalfStepAppoggiatura,
        "HalfStepAppoggiatura",
        Appoggiatura,
        [GeneralAppoggiatura, Appoggiatura]
    ),
    (
        WholeStepAppoggiatura,
        "WholeStepAppoggiatura",
        Appoggiatura,
        [GeneralAppoggiatura, Appoggiatura]
    ),
    (
        HalfStepInvertedAppoggiatura,
        "HalfStepInvertedAppoggiatura",
        InvertedAppoggiatura,
        [GeneralAppoggiatura, InvertedAppoggiatura]
    ),
    (
        WholeStepInvertedAppoggiatura,
        "WholeStepInvertedAppoggiatura",
        InvertedAppoggiatura,
        [GeneralAppoggiatura, InvertedAppoggiatura]
    ),
    (
        HalfStepMordent,
        "HalfStepMordent",
        Mordent,
        [GeneralMordent, Mordent]
    ),
    (
        WholeStepMordent,
        "WholeStepMordent",
        Mordent,
        [GeneralMordent, Mordent]
    ),
    (
        HalfStepInvertedMordent,
        "HalfStepInvertedMordent",
        InvertedMordent,
        [GeneralMordent, InvertedMordent]
    ),
    (
        WholeStepInvertedMordent,
        "WholeStepInvertedMordent",
        InvertedMordent,
        [GeneralMordent, InvertedMordent]
    ),
);

/// music21's `expressions.Trill`: the one ornament that carries itself onto
/// the notes it is split into, which music21 finds by asking whether an
/// ornament has `splitClient` at all.
#[pyclass(name = "Trill", module = "music21.expressions", extends = Ornament, subclass)]
pub struct Trill;

#[pymethods]
impl Trill {
    #[new]
    #[pyo3(signature = (**keywords))]
    fn new(keywords: Option<&Bound<'_, PyDict>>) -> PyResult<PyClassInitializer<Self>> {
        Ok(Ornament::initializer("Trill", keywords)?.add_subclass(Trill))
    }

    /// Carries the trill onto the notes a trilled note is split into,
    /// joining them with a trill extension: music21's hook for
    /// `splitAtQuarterLength`.
    fn splitClient<'py>(
        slf: &Bound<'py, Self>,
        noteList: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let spanners = PyList::empty(py);
        let count = noteList.len()?;
        if count == 0 {
            return Ok(spanners);
        }
        let first = noteList.get_item(0)?;
        first
            .getattr("expressions")?
            .call_method1("append", (slf,))?;
        if count > 1
            && first
                .call_method1("getSpannerSites", ("TrillExtension",))?
                .len()?
                == 0
        {
            let extension = py
                .import("music21.expressions")?
                .getattr("TrillExtension")?
                .call1((noteList,))?;
            spanners.append(extension)?;
        }
        Ok(spanners)
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Ornament>()?;
    m.add_class::<Trill>()?;
    register_kinds(m)
}
