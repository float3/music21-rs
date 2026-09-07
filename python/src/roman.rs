//! music21's `roman.RomanNumeral` over `music21-rs`, with music21's names,
//! properties and `repr`.
//!
//! The crate reads a figure — the numeral, its alteration, its inversion and
//! any secondary key — and works out the chord it stands for. What this adds
//! is music21's surface over that, and its way of naming a degree: a roman
//! numeral can be asked for by figure (`'V7'`) or by scale degree (`5`), and
//! a scale can be asked for the numeral on one of its degrees.

#![allow(non_snake_case)]

use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;

use music21_rs::{
    Chord as RsChord, Interval as RsInterval, Key as RsKey, Minor67Default as RsMinor67Default,
    RomanNumeral as RsRomanNumeral,
};

use crate::chord::Chord;
use crate::pitch::message;

/// The names the `roman` facade replaces in `music21.roman`.
pub const NAMES: &[&str] = &["RomanNumeral", "RomanNumeralException"];

pyo3::create_exception!(music21_rs_facade, RomanNumeralException, PyException);

fn roman_error(error: music21_rs::Error) -> PyErr {
    RomanNumeralException::new_err(message(&error))
}

/// The figures music21 writes for the seven degrees, upper case in a major
/// key and lower where the triad on that degree is minor or diminished.
const NUMERALS: [&str; 7] = ["I", "II", "III", "IV", "V", "VI", "VII"];

/// music21's `roman.RomanNumeral`, which *is* a chord.
///
/// Upstream it inherits from `Harmony` and so from `Chord`, and every chord
/// question — its pitches, its root, whether it is a seventh — is asked of
/// it directly. So it extends the chord facade here too, and the figure is
/// what this class adds on top.
#[pyclass(
    name = "RomanNumeral",
    module = "music21.roman",
    extends = Chord,
    subclass,
    skip_from_py_object
)]
pub struct RomanNumeral {
    pub(crate) inner: RsRomanNumeral,
    /// The octave the key's tonic sounded in, when the numeral was built on
    /// a scale that had one. music21 spells its chord from there, so a
    /// numeral on `A-4` roots on `A-4` and not on a bare `A-`.
    octave: Option<i32>,
    /// Whether the numeral was given a key at all. One that was not still
    /// reads in C major — music21 calls that the implied scale — but it
    /// reports no key, and says only its figure when asked for both.
    implied_key: bool,
    /// music21's `functionalityScore` when a caller has set one, which wins
    /// over the score the figure is looked up under.
    score: Option<u8>,
}

impl RomanNumeral {
    pub(crate) fn wrap(inner: RsRomanNumeral, octave: Option<i32>) -> Self {
        Self {
            inner,
            octave,
            implied_key: false,
            score: None,
        }
    }

    /// Replaces the figure this numeral reads, and the chord it stands on.
    fn rebuild(slf: &Bound<'_, Self>, py: Python<'_>, rebuilt: RsRomanNumeral) -> PyResult<()> {
        let octave = slf.borrow().octave;
        let mut numeral = Self::wrap(rebuilt, octave);
        numeral.implied_key = slf.borrow().implied_key;
        let chord = numeral.chord()?;
        slf.as_super().borrow_mut().replace_value(py, chord)?;
        slf.borrow_mut().inner = numeral.inner;
        Ok(())
    }

    /// The numeral, standing on the chord it names.
    pub(crate) fn initializer(py: Python<'_>, numeral: Self) -> PyResult<PyClassInitializer<Self>> {
        let chord = Chord::from_inner(py, numeral.chord()?)?;
        Ok(PyClassInitializer::from(chord).add_subclass(numeral))
    }

    /// Builds one from music21's two arguments: a figure or a scale degree,
    /// and a key or a scale.
    pub(crate) fn build(
        figure: Option<&Bound<'_, PyAny>>,
        keyOrScale: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let given = keyOrScale.filter(|value| !value.is_none()).is_some();
        let (key, octave) = key_and_octave(keyOrScale)?;
        let figure = match figure.filter(|value| !value.is_none()) {
            None => "I".to_string(),
            Some(value) => match value.extract::<usize>() {
                Ok(degree) => figure_for_degree(given.then_some(&key), degree)?,
                Err(_) => value.extract::<String>()?,
            },
        };
        let inner = RsRomanNumeral::with_options(
            figure,
            key,
            minor_reading(keywords, "sixthMinor")?,
            minor_reading(keywords, "seventhMinor")?,
            case_matters(keywords)?,
        )
        .map_err(roman_error)?;
        let mut numeral = Self::wrap(inner, octave);
        numeral.implied_key = !given;
        Ok(numeral)
    }

    /// The chord this numeral stands for, sounding where its key does.
    ///
    /// The crate spells it from the key's own octave, so the octaves come
    /// back with the pitches; a numeral built on a scale that stood in some
    /// other octave is moved there afterwards.
    fn chord(&self) -> PyResult<RsChord> {
        let chord = self.inner.to_chord().map_err(roman_error)?;
        let Some(octave) = self.octave else {
            return Ok(chord);
        };
        let tonic_octave = self.inner.key().tonic().octave().unwrap_or(4);
        let shift = octave - tonic_octave;
        if shift == 0 {
            return Ok(chord);
        }
        let moved = chord
            .pitches()
            .into_iter()
            .map(|pitch| {
                let mut moved = pitch;
                moved.set_octave(Some(moved.octave().unwrap_or(4) + shift));
                moved
            })
            .collect::<Vec<_>>();
        let mut moved = RsChord::new(moved.as_slice()).map_err(roman_error)?;
        if let Some(root) = chord.root() {
            let mut root = root.clone();
            root.set_octave(Some(root.octave().unwrap_or(4) + shift));
            moved.set_root(Some(root));
        }
        Ok(moved)
    }
}

/// The key a numeral is written in, and the octave its scale stood in.
fn key_and_octave(value: Option<&Bound<'_, PyAny>>) -> PyResult<(RsKey, Option<i32>)> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok((
            RsKey::from_tonic_mode("C", Some("major")).map_err(roman_error)?,
            None,
        ));
    };
    if let Ok(name) = value.extract::<String>() {
        return Ok((RsKey::from_tonic(&name).map_err(roman_error)?, None));
    }
    if let Ok(key) = value.extract::<PyRef<'_, crate::key::Key>>() {
        return Ok((key.inner.clone(), None));
    }
    // Anything else that has a tonic and a mode: a scale of ours, or one of
    // music21's own.
    let tonic = value.getattr("tonic")?;
    let name: String = tonic.getattr("name")?.extract()?;
    let octave: Option<i32> = tonic.getattr("octave").ok().and_then(|o| o.extract().ok());
    let mode: Option<String> = value
        .getattr("mode")
        .ok()
        .and_then(|mode| mode.extract().ok())
        .or_else(|| {
            value
                .getattr("type")
                .ok()
                .and_then(|kind| kind.extract().ok())
        });
    // A numeral can be read against any scale, and most scales are not a
    // mode a key signature can express; music21 reads those in the major of
    // the same tonic, which is what a numeral on a degree means there.
    let key = RsKey::from_tonic_mode(&name, mode.as_deref())
        .or_else(|_| RsKey::from_tonic_mode(&name, Some("major")))
        .map_err(roman_error)?;
    Ok((key, octave))
}

/// The reading, as music21's own `Minor67Default` member.
fn minor_default(py: Python<'_>, reading: RsMinor67Default) -> PyResult<Py<PyAny>> {
    let Ok(roman) = py.import("music21.roman") else {
        return Ok(py.None());
    };
    let name = match reading {
        RsMinor67Default::Quality => "QUALITY",
        RsMinor67Default::Flat => "FLAT",
        RsMinor67Default::Sharp => "SHARP",
        RsMinor67Default::Cautionary => "CAUTIONARY",
    };
    Ok(roman.getattr("Minor67Default")?.getattr(name)?.unbind())
}

/// The reading a keyword names, defaulting to reading it off the quality.
/// music21's `caseMatters` keyword: whether an upper-case numeral means a
/// major chord. The older figured-bass reading lets the key say instead.
fn case_matters(keywords: Option<&Bound<'_, PyDict>>) -> PyResult<bool> {
    let Some(keywords) = keywords else {
        return Ok(true);
    };
    match keywords.get_item("caseMatters")? {
        Some(value) if !value.is_none() => value.extract(),
        _ => Ok(true),
    }
}

fn minor_reading(keywords: Option<&Bound<'_, PyDict>>, name: &str) -> PyResult<RsMinor67Default> {
    let Some(keywords) = keywords else {
        return Ok(RsMinor67Default::Quality);
    };
    let Some(value) = keywords.get_item(name)? else {
        return Ok(RsMinor67Default::Quality);
    };
    if value.is_none() {
        return Ok(RsMinor67Default::Quality);
    }
    // music21 passes its own enum member; its name is what says which.
    let named = value
        .getattr("name")
        .and_then(|name| name.extract::<String>())
        .or_else(|_| value.extract::<String>())?;
    Ok(match named.to_ascii_uppercase().as_str() {
        "FLAT" => RsMinor67Default::Flat,
        "SHARP" => RsMinor67Default::Sharp,
        "CAUTIONARY" => RsMinor67Default::Cautionary,
        _ => RsMinor67Default::Quality,
    })
}

/// The numeral written on a scale degree, keeping the case the key implies.
fn numeral_for_degree(key: &RsKey, degree: usize) -> PyResult<String> {
    figure_for_degree(Some(key), degree).map(|figure| {
        figure
            .chars()
            .take_while(|letter| matches!(letter, 'I' | 'V' | 'i' | 'v'))
            .collect()
    })
}

/// The same figure written on a different numeral: everything before the
/// roman letters is dropped and everything after them is kept.
fn replace_numeral(figure: &str, numeral: &str) -> String {
    let rest: String = figure
        .chars()
        .skip_while(|letter| !matches!(letter, 'I' | 'V' | 'i' | 'v'))
        .skip_while(|letter| matches!(letter, 'I' | 'V' | 'i' | 'v'))
        .collect();
    format!("{numeral}{rest}")
}

/// The figure music21 writes for a scale degree given as a number.
///
/// It is the numeral alone, lower-cased on the degrees whose ordinary triad
/// is minor or diminished in that mode — and left upper-case throughout when
/// no key was given, since then there is no mode to read it against.
fn figure_for_degree(key: Option<&RsKey>, degree: usize) -> PyResult<String> {
    if degree == 0 || degree > 7 {
        return Err(RomanNumeralException::new_err(format!(
            "cannot make a roman numeral on degree {degree}"
        )));
    }
    let numeral = NUMERALS[degree - 1];
    let lower = match key.map(RsKey::mode) {
        Some("minor") => matches!(degree, 1 | 2 | 4 | 5),
        Some(_) => matches!(degree, 2 | 3 | 6 | 7),
        None => false,
    };
    Ok(if lower {
        numeral.to_ascii_lowercase()
    } else {
        numeral.to_string()
    })
}

#[pymethods]
impl RomanNumeral {
    #[new]
    #[pyo3(signature = (figure = None, keyOrScale = None, **_keywords))]
    fn new(
        py: Python<'_>,
        figure: Option<&Bound<'_, PyAny>>,
        keyOrScale: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Self::initializer(py, Self::build(figure, keyOrScale, _keywords)?)
    }

    /// The figure is read again here rather than only in `__new__`.
    ///
    /// A Python subclass hands `__new__` its own arguments and only then
    /// calls `super().__init__` with music21's, so a class that builds in
    /// `__new__` alone cannot be subclassed — and music21's own
    /// `RomanNumeral` is subclassed by `romanText` and by downstream code.
    #[pyo3(signature = (figure = None, keyOrScale = None, **_keywords))]
    fn __init__(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        figure: Option<&Bound<'_, PyAny>>,
        keyOrScale: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let built = Self::build(figure, keyOrScale, _keywords)?;
        let chord = built.chord()?;
        slf.as_super().borrow_mut().replace_value(py, chord)?;
        let mut me = slf.borrow_mut();
        me.inner = built.inner;
        me.octave = built.octave;
        Ok(())
    }

    /// music21's `figure`. Setting it rebuilds the numeral, and with it the
    /// chord it stands on: a roman numeral is its figure read in a key, so
    /// changing either changes everything downstream of it.
    #[getter]
    fn get_figure(&self) -> String {
        self.inner.figure().to_string()
    }

    #[setter]
    fn set_figure(slf: &Bound<'_, Self>, py: Python<'_>, value: &str) -> PyResult<()> {
        let key = slf.borrow().inner.key().clone();
        let rebuilt = RsRomanNumeral::new(value.to_string(), key).map_err(roman_error)?;
        Self::rebuild(slf, py, rebuilt)
    }

    /// music21's `romanNumeral`: the numeral with whatever alters it in
    /// front, but without the figures that say the inversion or the added
    /// notes — `bVII65/V` reads as `bVII`.
    #[getter]
    fn get_romanNumeral(&self) -> String {
        self.inner.roman_numeral()
    }

    /// music21 refuses this one: a numeral is what its figure says, so the
    /// figure is what a caller sets.
    #[setter]
    fn set_romanNumeral(&self, _value: &Bound<'_, PyAny>) -> PyResult<()> {
        Err(PyValueError::new_err(
            "Cannot set romanNumeral property of RomanNumeral objects",
        ))
    }

    /// music21's `romanNumeralAlone`: the numeral with nothing in front of
    /// it at all.
    #[getter]
    fn romanNumeralAlone(&self) -> String {
        self.inner.roman_numeral_alone()
    }

    /// music21's `frontAlterationString`: the flats or sharps written before
    /// the numeral, as they were written — which is not always what the
    /// numeral reports, since `vi` in a minor key roots on a raised sixth.
    #[getter]
    fn frontAlterationString(&self) -> String {
        let alteration = self.inner.written_accidental();
        let mark = if alteration < 0 { "b" } else { "#" };
        mark.repeat(alteration.unsigned_abs() as usize)
    }

    /// music21's `frontAlterationAccidental`: that alteration as an
    /// accidental, and nothing when the numeral is unaltered.
    #[getter]
    fn frontAlterationAccidental(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let alteration = self.inner.accidental();
        if alteration == 0 {
            return Ok(py.None());
        }
        Ok(crate::pitch::Accidental::from_inner(
            music21_rs::pitch::Accidental::new(f64::from(alteration)).map_err(roman_error)?,
        )
        .into_pyobject(py)?
        .into_any()
        .unbind())
    }

    /// music21's `figureAndKey`: the figure and the key it is read in, or
    /// the figure alone when the key was only implied.
    #[getter]
    fn figureAndKey(&self) -> String {
        if self.implied_key {
            return self.inner.figure().to_string();
        }
        self.inner.figure_and_key()
    }

    /// music21's `useImpliedScale`: whether the numeral was left to guess
    /// the key it is read in.
    #[getter]
    fn useImpliedScale(&self) -> bool {
        self.implied_key
    }

    /// music21's `impliedScale`: the scale a numeral with no key reads in.
    #[getter]
    fn impliedScale(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        if !self.implied_key {
            return Ok(py.None());
        }
        crate::key::Key::object(py, self.inner.key().clone())
    }

    /// music21's `secondaryRomanNumeralKey`: the key a secondary numeral
    /// establishes, so the `V` of `V/V` in G major is read in D major.
    #[getter]
    fn secondaryRomanNumeralKey(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        if self.inner.secondary().is_none() {
            return Ok(py.None());
        }
        crate::key::Key::object(py, self.inner.effective_key_of().map_err(roman_error)?)
    }

    /// music21's `caseMatters`: whether an upper-case numeral means major
    /// and a lower-case one minor.
    #[getter]
    fn caseMatters(&self) -> bool {
        self.inner.case_matters()
    }

    /// music21's `bracketedAlterations`: the alterations written in square
    /// brackets, as the mark each was written with and the chord step it
    /// moves.
    #[getter]
    fn bracketedAlterations(&self) -> Vec<(String, u8)> {
        self.inner
            .bracketed_alterations()
            .iter()
            .map(|(alter, step)| {
                let mark = if *alter < 0 { "b" } else { "#" };
                (mark.repeat(alter.unsigned_abs() as usize), *step)
            })
            .collect()
    }

    #[getter]
    fn get_scaleDegree(&self) -> u8 {
        self.inner.degree()
    }

    /// Setting the degree rewrites the figure on that degree, keeping
    /// everything else the figure said.
    #[setter]
    fn set_scaleDegree(slf: &Bound<'_, Self>, py: Python<'_>, value: usize) -> PyResult<()> {
        let (figure, key) = {
            let me = slf.borrow();
            (me.inner.figure().to_string(), me.inner.key().clone())
        };
        let numeral = numeral_for_degree(&key, value)?;
        let rewritten = replace_numeral(&figure, &numeral);
        let rebuilt = RsRomanNumeral::new(rewritten, key).map_err(roman_error)?;
        Self::rebuild(slf, py, rebuilt)
    }

    /// music21's `scaleDegreeWithAlteration`: the degree, and how it is
    /// altered from the key.
    #[getter]
    fn scaleDegreeWithAlteration(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let (degree, alteration) = self.inner.scale_degree_with_alteration();
        let alteration = if alteration == 0 {
            py.None().into_bound(py)
        } else {
            crate::pitch::Accidental::from_inner(
                music21_rs::pitch::Accidental::new(f64::from(alteration)).map_err(roman_error)?,
            )
            .into_pyobject(py)?
            .into_any()
        };
        Ok((degree, alteration).into_pyobject(py)?.into_any().unbind())
    }

    /// music21's `key`. Setting it reads the same figure in the new key.
    #[getter]
    fn get_key(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        if self.implied_key {
            return Ok(py.None());
        }
        crate::key::Key::object(py, self.inner.key().clone())
    }

    #[setter]
    fn set_key(slf: &Bound<'_, Self>, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let (key, octave) = key_and_octave(Some(value))?;
        let figure = slf.borrow().inner.figure().to_string();
        let rebuilt = RsRomanNumeral::new(figure, key).map_err(roman_error)?;
        {
            let mut me = slf.borrow_mut();
            me.octave = octave.or(me.octave);
            me.implied_key = value.is_none();
        }
        Self::rebuild(slf, py, rebuilt)
    }

    /// music21's `sixthMinor` and `seventhMinor`: how a `vi` or a `vii` in a
    /// minor key decides between the natural and the raised degree.
    ///
    /// This crate keeps the accidental as written and never rewrites it,
    /// which is the divergence its own notes record, so both answer
    /// music21's `QUALITY` — decide by the quality the figure names.
    #[getter]
    fn sixthMinor(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        minor_default(py, self.inner.sixth_minor())
    }

    #[getter]
    fn seventhMinor(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        minor_default(py, self.inner.seventh_minor())
    }

    /// music21's `frontAlterationTransposeInterval`: the alteration in front
    /// of the numeral, as the interval it moves the root by.
    #[getter]
    fn frontAlterationTransposeInterval(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let alteration = self.inner.accidental();
        if alteration == 0 {
            return Ok(py.None());
        }
        let interval = RsInterval::from_semitones(i32::from(alteration)).map_err(roman_error)?;
        Ok(crate::interval::Interval::wrap(interval)
            .into_pyobject(py)?
            .into_any()
            .unbind())
    }

    /// music21's `inversion`, which for a roman numeral is the one its
    /// figure names rather than one read back off the pitches.
    fn inversion(&self) -> u8 {
        self.inner.inversion()
    }

    /// music21's `secondaryRomanNumeral`: the numeral after the slash, as a
    /// numeral of its own read in this one's key.
    #[getter]
    fn secondaryRomanNumeral(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let Some(secondary) = self.inner.secondary() else {
            return Ok(py.None());
        };
        let inner = RsRomanNumeral::new(secondary.to_string(), self.inner.key().clone())
            .map_err(roman_error)?;
        Ok(Py::new(py, Self::initializer(py, Self::wrap(inner, self.octave))?)?.into_any())
    }

    /// music21's `functionalityScore`: how strongly the figure pulls, on
    /// music21's own hundred-point scale. A caller may set one, and what
    /// they set stands.
    #[getter]
    fn get_functionalityScore(&self) -> u8 {
        self.score
            .unwrap_or_else(|| self.inner.functionality_score())
    }

    #[setter]
    fn set_functionalityScore(&mut self, value: u8) {
        self.score = Some(value);
    }

    /// music21's `isNeapolitan`.
    #[pyo3(signature = (require1stInversion = true))]
    fn isNeapolitan(&self, require1stInversion: bool) -> bool {
        self.inner.is_neapolitan(require1stInversion)
    }

    /// music21's `isMixture`: whether the figure borrows from the parallel
    /// key.
    #[pyo3(signature = (evaluateSecondaryNumeral = false))]
    fn isMixture(&self, evaluateSecondaryNumeral: bool) -> PyResult<bool> {
        self.inner
            .is_mixture(evaluateSecondaryNumeral)
            .map_err(roman_error)
    }

    /// music21's `transpose`: the same figure in a key moved by an interval.
    #[pyo3(signature = (value, *, inPlace = false))]
    fn transpose(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
        inPlace: bool,
    ) -> PyResult<Option<Py<Self>>> {
        let interval = crate::interval::interval_from_any(value)?;
        let (moved, octave) = {
            let me = slf.borrow();
            (
                me.inner.transpose(&interval).map_err(roman_error)?,
                me.octave,
            )
        };
        if inPlace {
            let numeral = Self::wrap(moved, octave);
            let chord = numeral.chord()?;
            slf.as_super().borrow_mut().replace_value(py, chord)?;
            slf.borrow_mut().inner = numeral.inner;
            return Ok(None);
        }
        Ok(Some(Py::new(
            py,
            Self::initializer(py, Self::wrap(moved, octave))?,
        )?))
    }

    fn __repr__(&self) -> String {
        format!("<music21.roman.RomanNumeral {}>", self.figureAndKey())
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other.extract::<PyRef<'_, Self>>().is_ok_and(|other| {
            other.inner.figure() == self.inner.figure()
                && other.inner.key().tonic().name() == self.inner.key().tonic().name()
                && other.inner.key().mode() == self.inner.key().mode()
        })
    }

    fn __deepcopy__(&self, py: Python<'_>, _memo: &Bound<'_, PyAny>) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::initializer(py, Self::wrap(self.inner.clone(), self.octave))?,
        )
    }

    fn __copy__(&self, py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::initializer(py, Self::wrap(self.inner.clone(), self.octave))?,
        )
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add_class::<RomanNumeral>()?;
    let exception = py.get_type::<RomanNumeralException>();
    exception.setattr("__module__", "music21.roman")?;
    m.add("RomanNumeralException", exception)?;
    Ok(())
}
