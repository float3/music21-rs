//! music21's `roman.RomanNumeral` over `music21-rs`, with music21's names,
//! properties and `repr`.
//!
//! The crate reads a figure — the numeral, its alteration, its inversion and
//! any secondary key — and works out the chord it stands for. What this adds
//! is music21's surface over that, and its way of naming a degree: a roman
//! numeral can be asked for by figure (`'V7'`) or by scale degree (`5`), and
//! a scale can be asked for the numeral on one of its degrees.

#![allow(non_snake_case)]

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use music21_rs::roman as rs_roman;
use music21_rs::scale::Scale as RsScale;
use music21_rs::{
    Chord as RsChord, ImpliedQuality as RsImpliedQuality, Interval as RsInterval, Key as RsKey,
    Minor67Default as RsMinor67Default, RomanNumeral as RsRomanNumeral,
};

use crate::chord::Chord;
use crate::pitch::message;

/// The names the `roman` facade replaces in `music21.roman`.
pub const NAMES: &[&str] = &["RomanNumeral", "RomanNumeralException"];

pyo3::create_exception!(
    music21_rs_facade,
    RomanNumeralException,
    crate::Music21Exception
);

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
    /// The key or scale object the numeral was given, kept so that `key` is
    /// the very object the caller handed over.
    ///
    /// music21 stores what it was given and hands it back, so a numeral built
    /// on a scale reports that scale — which has no mode, and is why such a
    /// numeral is never modal mixture — and an edit to the key's mode is one
    /// the numeral sees.
    key_object: Option<Py<PyAny>>,
    /// Whether the numeral was told not to work its notes out.
    ///
    /// music21's `updatePitches=False` is how a caller says it wants the
    /// figure read and nothing else, which is faster when only the figure is
    /// wanted. The chord is then empty until something asks for it again.
    silent: bool,
    /// A numeral built with no figure at all.
    ///
    /// music21 starts one of these and then walks it through its parsing
    /// steps, so a blank numeral has no figure, no notes and no key, and
    /// every field a parsing step writes starts empty.
    blank: bool,
    /// What the parsing steps have written, which stands over what the
    /// figure says.
    state: ParseState,
    /// music21's `pivotChord`: the same chord read again in the key the
    /// music turns to. Nothing here works it out — it is written by
    /// music21's own `romanText` reader, which is what carries a pivot.
    pivot: Option<Py<PyAny>>,
    /// music21's `followsKeyChange`, which its `romanText` reader sets on
    /// the first numeral after a key is declared.
    follows_key_change: bool,
    /// music21's `writeAsChord`: whether the numeral is written out as the
    /// notes it stands for rather than as a figure. A `RomanNumeral` starts
    /// as `True`, and its MusicXML exporter writes a `<numeral>` tag instead
    /// when a caller turns it off.
    write_as_chord: bool,
}

/// The fields music21's parsing steps write on a numeral as they read its
/// figure, each empty until a step writes it.
///
/// The crate answers all of these from the figure, so nothing here is needed
/// to read a numeral. They exist because music21 exposes the steps
/// themselves, and a caller — or music21's own `romanText` parser — may run
/// one and then ask what it wrote.
#[derive(Default)]
struct ParseState {
    degree: Option<u8>,
    implied_quality: Option<RsImpliedQuality>,
    /// The alteration in front of the numeral, in semitones. Zero is an
    /// alteration a step deliberately cleared.
    alteration: Option<i8>,
    numeral_alone: Option<String>,
    bracketed: Option<Vec<(i8, u8)>>,
    omitted: Option<Vec<u8>>,
    added: Option<Vec<(i8, u8)>>,
    /// `Some(None)` is a secondary numeral a step deliberately cleared.
    secondary: Option<Option<Py<PyAny>>>,
    secondary_key: Option<Option<Py<PyAny>>>,
}

impl RomanNumeral {
    pub(crate) fn wrap(inner: RsRomanNumeral, octave: Option<i32>) -> Self {
        Self {
            inner,
            octave,
            implied_key: false,
            score: None,
            key_object: None,
            silent: false,
            blank: false,
            state: ParseState::default(),
            pivot: None,
            follows_key_change: false,
            write_as_chord: true,
        }
    }

    /// Replaces the figure this numeral reads, and the chord it stands on.
    fn rebuild(slf: &Bound<'_, Self>, py: Python<'_>, rebuilt: RsRomanNumeral) -> PyResult<()> {
        let octave = slf.borrow().octave;
        let mut numeral = Self::wrap(rebuilt, octave);
        numeral.implied_key = slf.borrow().implied_key;
        slf.borrow_mut().blank = false;
        let chord = numeral.chord()?;
        slf.as_super().borrow_mut().replace_value(py, chord)?;
        slf.borrow_mut().inner = numeral.inner;
        Ok(())
    }

    /// The same numeral again, as an object of the class this one is.
    fn copied<'py>(slf: &Bound<'py, Self>, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let mut numeral = {
            let me = slf.borrow();
            let mut numeral = Self::wrap(me.inner.clone(), me.octave);
            // Everything the numeral was told rather than read off its
            // figure comes across: a copy of a pivot chord is still a pivot,
            // and music21's own `romanText` reader copies a measure that
            // holds one.
            numeral.implied_key = me.implied_key;
            numeral.score = me.score;
            numeral.silent = me.silent;
            numeral.blank = me.blank;
            numeral.follows_key_change = me.follows_key_change;
            numeral.write_as_chord = me.write_as_chord;
            numeral.key_object = me.key_object.as_ref().map(|key| key.clone_ref(py));
            numeral.pivot = me.pivot.as_ref().map(|pivot| pivot.clone_ref(py));
            numeral
        };
        // The chord half is copied rather than built afresh from the
        // figure: a numeral in a score has been timed, and one whose copy
        // lasted a quarter again would shorten every measure it was copied
        // into — which is what expanding a repeat does.
        let chord = slf.as_super().borrow_mut().copied_value(py)?;
        let class = slf.as_any().get_type();
        let copy = crate::blank_installed(class.as_any())?;
        {
            let cell = copy.cast::<Self>()?;
            let mut me = cell.borrow_mut();
            std::mem::swap(&mut *me, &mut numeral);
            *me.into_super() = chord;
        }
        Ok(copy)
    }

    /// The numeral, standing on the chord it names.
    pub(crate) fn initializer(py: Python<'_>, numeral: Self) -> PyResult<PyClassInitializer<Self>> {
        let chord = Chord::from_inner(py, numeral.chord()?)?;
        Ok(PyClassInitializer::from(chord).add_subclass(numeral))
    }

    /// The same as an object, and as the class music21 now has under that
    /// name — a numeral built as the bare facade is one no stream can hold.
    pub(crate) fn object(py: Python<'_>, numeral: Self) -> PyResult<Py<Self>> {
        let Some(class) = crate::installed_class(py, "music21.roman", "RomanNumeral") else {
            return Py::new(py, Self::initializer(py, numeral)?);
        };
        let chord = Chord::from_inner(py, numeral.chord()?)?;
        let object = crate::blank_installed(&class)?;
        let cell = object.cast::<Self>()?;
        {
            let mut me = cell.borrow_mut();
            *me = numeral;
            *me.into_super() = chord;
        }
        Ok(cell.clone().unbind())
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
        let blank = figure.filter(|value| !value.is_none()).is_none();
        let figure = match figure.filter(|value| !value.is_none()) {
            None => "I".to_string(),
            Some(value) => match value.extract::<usize>() {
                Ok(degree) => figure_for_degree(given.then_some(&key), degree)?,
                Err(_) => value.extract::<String>()?,
            },
        };
        let inner = RsRomanNumeral::over_scale(
            figure,
            key,
            collection(keyOrScale),
            minor_reading(keywords, "sixthMinor")?,
            minor_reading(keywords, "seventhMinor")?,
            case_matters(keywords)?,
        )
        .map_err(roman_error)?;
        let mut numeral = Self::wrap(inner, octave);
        numeral.implied_key = !given;
        numeral.blank = blank;
        numeral.silent = !update_pitches(keywords)?;
        numeral.key_object = keyOrScale
            .filter(|value| !value.is_none() && value.extract::<String>().is_err())
            .map(|value| value.clone().unbind());
        Ok(numeral)
    }

    /// The alteration in front of the numeral, in semitones: what a parsing
    /// step wrote if one did, and otherwise what the figure was written with.
    fn front_alteration(&self) -> i8 {
        if let Some(alteration) = self.state.alteration {
            return alteration;
        }
        if self.blank {
            return 0;
        }
        self.inner.written_accidental()
    }

    /// The quality the numeral states.
    fn implied_quality(&self) -> RsImpliedQuality {
        if let Some(quality) = self.state.implied_quality {
            return quality;
        }
        if self.blank {
            return RsImpliedQuality::Unstated;
        }
        self.inner.implied_quality()
    }

    /// The alterations written in square brackets.
    fn bracketed(&self) -> Vec<(i8, u8)> {
        if let Some(bracketed) = &self.state.bracketed {
            return bracketed.clone();
        }
        if self.blank {
            return Vec::new();
        }
        self.inner.bracketed_alterations().to_vec()
    }

    /// The notes the figure puts in beside the chord.
    fn added(&self) -> Vec<(i8, u8)> {
        if let Some(added) = &self.state.added {
            return added.clone();
        }
        if self.blank {
            return Vec::new();
        }
        self.inner.added_steps().to_vec()
    }

    /// The chord this numeral stands for, sounding where its key does.
    ///
    /// The crate spells it from the key's own octave, so the octaves come
    /// back with the pitches; a numeral built on a scale that stood in some
    /// other octave is moved there afterwards.
    fn chord(&self) -> PyResult<RsChord> {
        if self.blank || self.silent {
            return RsChord::new::<&[music21_rs::Pitch]>(&[]).map_err(roman_error);
        }
        let chord = self.inner.to_chord().map_err(roman_error)?;
        // A numeral read over a scale is already spelled where that scale
        // stands, so there is nowhere to move it to.
        let Some(octave) = self.octave.filter(|_| self.inner.scale().is_none()) else {
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

/// The collection a numeral counts its degrees against, when the caller
/// gave a scale rather than a key.
///
/// A key is left out: seven degrees spelled by a key signature is what every
/// other path already does, and reading those off a realized scale would put
/// a second answer in play. Only a scale music21 would not call diatonic
/// comes through here.
fn collection(value: Option<&Bound<'_, PyAny>>) -> Option<RsScale> {
    let scale = value?
        .extract::<PyRef<'_, crate::scale::ConcreteScale>>()
        .ok()?;
    (scale.inner.degree_count() != 7).then(|| scale.inner.clone())
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
/// music21's `updatePitches` keyword: whether the numeral works its notes
/// out at all, or only reads its figure.
fn update_pitches(keywords: Option<&Bound<'_, PyDict>>) -> PyResult<bool> {
    let Some(keywords) = keywords else {
        return Ok(true);
    };
    match keywords.get_item("updatePitches")? {
        Some(value) if !value.is_none() => value.extract(),
        _ => Ok(true),
    }
}

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

/// The key an applied numeral establishes: the key its own chord is the
/// tonic of.
fn established_key(applied: &RsRomanNumeral) -> PyResult<RsKey> {
    let chord = applied.to_chord().map_err(roman_error)?;
    let root = chord
        .root()
        .ok_or_else(|| RomanNumeralException::new_err("applied numeral has no root"))?;
    let mode = match applied.implied_quality() {
        RsImpliedQuality::Minor => "minor",
        RsImpliedQuality::Major => "major",
        _ if chord.semitones_from_chord_step(3) == Some(3) => "minor",
        _ => "major",
    };
    RsKey::from_tonic_mode(&root.name(), Some(mode)).map_err(roman_error)
}

/// The same key with a minor third, which is where every augmented sixth is
/// read: music21 hands the figure on in the parallel minor.
fn parallel_minor(py: Python<'_>, scale: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let (key, _) = key_and_octave(Some(scale))?;
    if key.mode() == "minor" {
        return Ok(scale.clone().unbind());
    }
    let minor = RsKey::from_tonic_mode(&key.tonic().name(), Some("minor")).map_err(roman_error)?;
    crate::key::Key::object(py, minor)
}

/// The sharps or flats an alteration is written with.
fn alteration_mark(alter: i8) -> String {
    let mark = if alter < 0 { "b" } else { "#" };
    mark.repeat(alter.unsigned_abs() as usize)
}

/// The same for an added note, where music21 writes a flat as `-`.
fn added_mark(alter: i8) -> String {
    let mark = if alter < 0 { "-" } else { "#" };
    mark.repeat(alter.unsigned_abs() as usize)
}

/// How many semitones a written run of accidentals moves a note.
fn mark_alteration(mark: &str) -> i8 {
    mark.chars()
        .map(|letter| match letter {
            '#' => 1,
            'b' | '-' => -1,
            _ => 0,
        })
        .sum()
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
    /// A numeral is written out as text and read back, and the chord it
    /// stands on is worked out again from the figure.
    /// A numeral is written out as its figure and what it was told, and the
    /// chord it stands on goes with it: a score is frozen to a file and read
    /// back, and a numeral that came back a quarter long would shorten every
    /// bar it was read into.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        let py = slf.py();
        let extra = PyDict::new(py);
        {
            let me = slf.borrow();
            extra.set_item("_scale", me.key_object.as_ref())?;
            extra.set_item("pivotChord", me.pivot.as_ref())?;
        }
        let me = slf.borrow();
        let written = (
            me.inner.clone(),
            me.octave,
            me.implied_key,
            me.score,
            me.silent,
            me.blank,
            me.follows_key_change,
            me.write_as_chord,
            slf.as_super().borrow().inner.clone(),
        );
        drop(me);
        crate::pickled_extra(slf, &written, Some(&extra))
    }

    fn __setstate__(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        state: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        type State = (
            RsRomanNumeral,
            Option<i32>,
            bool,
            Option<u8>,
            bool,
            bool,
            bool,
            bool,
            RsChord,
        );
        let (written, extra) = crate::unpickled_extra::<_, State>(slf, state)?;
        let Some((
            inner,
            octave,
            implied_key,
            score,
            silent,
            blank,
            follows_key_change,
            write_as_chord,
            chord,
        )) = written
        else {
            return Ok(());
        };
        slf.as_super().borrow_mut().replace_value(py, chord)?;
        {
            let mut me = slf.borrow_mut();
            me.inner = inner;
            me.octave = octave;
            me.implied_key = implied_key;
            me.score = score;
            me.silent = silent;
            me.blank = blank;
            me.follows_key_change = follows_key_change;
            me.write_as_chord = write_as_chord;
        }
        if let Some(extra) = extra {
            let extra = extra.bind(py);
            let taken = |name: &str| -> Option<Py<PyAny>> {
                extra
                    .get_item(name)
                    .ok()
                    .filter(|value| !value.is_none())
                    .map(pyo3::Bound::unbind)
            };
            let key_object = taken("_scale");
            let pivot = taken("pivotChord");
            let mut me = slf.borrow_mut();
            me.key_object = key_object;
            me.pivot = pivot;
        }
        Ok(())
    }

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
        me.implied_key = built.implied_key;
        me.blank = built.blank;
        me.silent = built.silent;
        me.key_object = built.key_object;
        drop(me);
        // A numeral may be timed as it is built, the way every other thing
        // a stream holds may be: `RomanNumeral('I', k, quarterLength=4.0)`
        // lasts a bar, and the measures a roman-text score is read into are
        // as long as the numerals in them say.
        if let Some(duration) = crate::chord::duration_from_keywords(py, _keywords)? {
            slf.as_any().setattr("duration", duration.bind(py))?;
        }
        Ok(())
    }

    /// music21's `figure`. Setting it rebuilds the numeral, and with it the
    /// chord it stands on: a roman numeral is its figure read in a key, so
    /// changing either changes everything downstream of it.
    #[getter]
    fn get_figure(&self) -> String {
        if self.blank {
            return String::new();
        }
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
    fn get_romanNumeralAlone(&self) -> String {
        if let Some(alone) = &self.state.numeral_alone {
            return alone.clone();
        }
        if self.blank {
            return String::new();
        }
        self.inner.roman_numeral_alone()
    }

    #[setter]
    fn set_romanNumeralAlone(&mut self, value: String) {
        self.state.numeral_alone = Some(value);
    }

    /// music21's `frontAlterationString`: the flats or sharps written before
    /// the numeral, as they were written — which is not always what the
    /// numeral reports, since `vi` in a minor key roots on a raised sixth.
    #[getter]
    fn frontAlterationString(&self) -> String {
        let alteration = self.front_alteration();
        let mark = if alteration < 0 { "b" } else { "#" };
        mark.repeat(alteration.unsigned_abs() as usize)
    }

    /// music21's `frontAlterationAccidental`: that alteration as an
    /// accidental, and nothing when the numeral is unaltered.
    #[getter]
    fn get_frontAlterationAccidental(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let alteration = self.front_alteration();
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
            return self.get_figure();
        }
        self.inner.figure_and_key()
    }

    /// music21's `useImpliedScale`: whether the numeral was left to guess
    /// the key it is read in.
    #[getter]
    fn useImpliedScale(&self) -> bool {
        self.implied_key
    }

    /// music21's `impliedScale`: the scale a numeral with no key reads in,
    /// which upstream is a scale rather than a key — a numeral told no key
    /// is not in one.
    #[getter]
    fn impliedScale(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        if !self.implied_key {
            return Ok(py.None());
        }
        let key = self.inner.key();
        let scale = music21_rs::scale::Scale::new(
            if key.mode() == "minor" {
                music21_rs::scale::ScaleType::Minor
            } else {
                music21_rs::scale::ScaleType::Major
            },
            key.tonic(),
        );
        crate::scale::ConcreteScale::object(py, scale)
    }

    /// music21's `secondaryRomanNumeralKey`: the key a secondary numeral
    /// establishes, so the `V` of `V/V` in G major is read in D major.
    #[getter]
    fn get_secondaryRomanNumeralKey(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        if let Some(key) = &self.state.secondary_key {
            return Ok(match key {
                Some(key) => key.clone_ref(py),
                None => py.None(),
            });
        }
        if self.blank || self.inner.secondary().is_none() {
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
    fn get_bracketedAlterations(&self) -> Vec<(String, u8)> {
        self.bracketed()
            .iter()
            .map(|(alter, step)| (alteration_mark(*alter), *step))
            .collect()
    }

    #[setter]
    fn set_bracketedAlterations(&mut self, value: Vec<(String, u8)>) {
        self.state.bracketed = Some(
            value
                .into_iter()
                .map(|(mark, step)| (mark_alteration(&mark), step))
                .collect(),
        );
    }

    /// music21's `omittedSteps`: the chord steps the figure leaves out.
    #[getter]
    fn get_omittedSteps(&self) -> Vec<u32> {
        self.state
            .omitted
            .clone()
            .unwrap_or_else(|| self.inner.omitted_steps().to_vec())
            .into_iter()
            .map(u32::from)
            .collect()
    }

    #[setter]
    fn set_omittedSteps(&mut self, value: Vec<u8>) {
        self.state.omitted = Some(value);
    }

    /// music21's `addedSteps`: the notes the figure puts in beside the
    /// chord, each with the accidental it was written with.
    #[getter]
    fn get_addedSteps(&self) -> Vec<(String, u8)> {
        self.added()
            .iter()
            .map(|(alter, step)| (added_mark(*alter), *step))
            .collect()
    }

    #[setter]
    fn set_addedSteps(&mut self, value: Vec<(String, u8)>) {
        self.state.added = Some(
            value
                .into_iter()
                .map(|(mark, step)| (mark_alteration(&mark), step))
                .collect(),
        );
    }

    /// music21's `scaleDegree`, which is a field its parsing steps write
    /// rather than something read back off the figure — so setting it says
    /// which degree the numeral is taken to stand on, and leaves the figure
    /// and the notes alone.
    #[getter]
    fn get_scaleDegree(&self) -> u8 {
        if let Some(degree) = self.state.degree {
            return degree;
        }
        if self.blank {
            return 0;
        }
        self.inner.degree()
    }

    #[setter]
    fn set_scaleDegree(&mut self, value: u8) {
        self.state.degree = Some(value);
    }

    /// music21's `impliedQuality`: the quality the figure states, which is
    /// what the notes read off the scale are respelled to.
    #[getter]
    fn get_impliedQuality(&self) -> String {
        self.implied_quality().name().to_string()
    }

    #[setter]
    fn set_impliedQuality(&mut self, value: &str) {
        self.state.implied_quality = Some(RsImpliedQuality::from_name(value));
    }

    /// music21's `scaleDegreeWithAlteration`: the degree, and how it is
    /// altered from the key.
    #[getter]
    fn scaleDegreeWithAlteration(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let (degree, alteration) = (self.get_scaleDegree(), self.front_alteration());
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
    fn get_key(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        if slf.borrow().implied_key {
            return Ok(py.None());
        }
        if let Some(key) = &slf.borrow().key_object {
            return Ok(key.clone_ref(py));
        }
        let key = crate::key::Key::object(py, slf.borrow().inner.key().clone())?;
        slf.borrow_mut().key_object = Some(key.clone_ref(py));
        Ok(key)
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
            me.key_object = (!value.is_none() && value.extract::<String>().is_err())
                .then(|| value.clone().unbind());
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
    #[setter]
    fn set_frontAlterationAccidental(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        if value.is_none() {
            self.state.alteration = Some(0);
            return Ok(());
        }
        let alter: f64 = value.getattr("alter")?.extract()?;
        self.state.alteration = Some(alter.round() as i8);
        Ok(())
    }

    /// music21's `frontAlterationTransposeInterval`: the alteration as the
    /// interval it moves the root by, which is an augmented unison and not a
    /// minor second — the letter does not change.
    #[getter]
    fn frontAlterationTransposeInterval(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let alteration = self.front_alteration();
        if alteration == 0 {
            return Ok(py.None());
        }
        let interval = RsInterval::from_generic_and_chromatic(1, i32::from(alteration))
            .map_err(roman_error)?;
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
    #[setter]
    fn set_secondaryRomanNumeralKey(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) {
        let _ = py;
        self.state.secondary_key = Some((!value.is_none()).then(|| value.clone().unbind()));
    }

    #[setter]
    fn set_secondaryRomanNumeral(&mut self, value: &Bound<'_, PyAny>) {
        self.state.secondary = Some((!value.is_none()).then(|| value.clone().unbind()));
    }

    #[getter]
    fn get_secondaryRomanNumeral(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        if let Some(secondary) = &self.state.secondary {
            return Ok(match secondary {
                Some(secondary) => secondary.clone_ref(py),
                None => py.None(),
            });
        }
        if self.blank {
            return Ok(py.None());
        }
        let Some(secondary) = self.inner.secondary() else {
            return Ok(py.None());
        };
        let inner = RsRomanNumeral::new(secondary.to_string(), self.inner.key().clone())
            .map_err(roman_error)?;
        Ok(Self::object(py, Self::wrap(inner, self.octave))?.into_any())
    }

    /// music21's `functionalityScore`: how strongly the figure pulls, on
    /// music21's own hundred-point scale. A caller may set one, and what
    /// they set stands.
    #[getter]
    fn get_functionalityScore(&self) -> u8 {
        if let Some(score) = self.score {
            return score;
        }
        if self.blank {
            return 0;
        }
        self.inner.functionality_score()
    }

    #[setter]
    fn set_functionalityScore(&mut self, value: u8) {
        self.score = Some(value);
    }

    /// music21's `_parseOmittedSteps`: takes the `[noN]` groups off a
    /// figure and records what they said.
    fn _parseOmittedSteps(&mut self, workingFigure: &str) -> String {
        let mut figure = workingFigure.to_string();
        self.state.omitted = Some(rs_roman::take_omitted_steps(&mut figure));
        figure
    }

    /// music21's `_parseAddedSteps`: the same for the `[addN]` groups.
    fn _parseAddedSteps(&mut self, workingFigure: &str) -> String {
        let mut figure = workingFigure.to_string();
        self.state.added = Some(rs_roman::take_added_steps(&mut figure));
        figure
    }

    /// music21's `_parseBracketedAlterations`: the same for `[#5]` and `[b3]`.
    fn _parseBracketedAlterations(&mut self, workingFigure: &str) -> String {
        let mut figure = workingFigure.to_string();
        self.state.bracketed = Some(rs_roman::take_bracketed_alterations(&mut figure));
        figure
    }

    /// music21's `_parseFrontAlterations`: takes the flats or sharps off the
    /// front of a figure and records the alteration they make.
    fn _parseFrontAlterations(&mut self, workingFigure: &str) -> String {
        let (alteration, rest) = rs_roman::split_roman_accidental_prefix(workingFigure);
        self.state.alteration = Some(alteration);
        rest.to_string()
    }

    /// music21's `_parseRNAloneAmidstAug6`: takes the numeral off the front
    /// of a figure and records the degree it names.
    ///
    /// An augmented sixth is written by nationality rather than by numeral,
    /// carries its own degree and alteration, and is read in the parallel
    /// minor — so this hands back the scale to read the rest of the figure
    /// in, which is the one it was given unless that happened.
    #[pyo3(signature = (workingFigure, useScale))]
    fn _parseRNAloneAmidstAug6(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        workingFigure: &str,
        useScale: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let read = rs_roman::parse_numeral_alone(workingFigure).map_err(roman_error)?;
        {
            let mut me = slf.borrow_mut();
            me.state.numeral_alone = Some(read.numeral.clone());
            me.state.degree = Some(read.degree);
            if read.alteration != 0 {
                me.state.alteration = Some(read.alteration);
            }
            if !read.bracketed.is_empty() {
                let mut bracketed = me.bracketed();
                bracketed.extend(read.bracketed.iter().copied());
                me.state.bracketed = Some(bracketed);
            }
        }
        let scale = if read.minor {
            parallel_minor(py, useScale)?
        } else {
            useScale.clone().unbind()
        };
        Ok((read.rest, scale).into_pyobject(py)?.into_any().unbind())
    }

    /// music21's `_correctForSecondaryRomanNumeral`: splits an applied
    /// numeral off a figure and records the numeral and the key it makes.
    #[pyo3(signature = (useScale, figure = None))]
    fn _correctForSecondaryRomanNumeral(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        useScale: &Bound<'_, PyAny>,
        figure: Option<&str>,
    ) -> PyResult<Py<PyAny>> {
        let written = match figure {
            Some(figure) => figure.to_string(),
            None => slf.borrow().get_figure(),
        };
        let (primary, secondary) = rs_roman::split_secondary(&written);
        let Some(secondary) = secondary else {
            let mut me = slf.borrow_mut();
            me.state.secondary = Some(None);
            me.state.secondary_key = Some(None);
            drop(me);
            return Ok((primary, useScale.clone())
                .into_pyobject(py)?
                .into_any()
                .unbind());
        };
        let (sixth, seventh, matters) = {
            let me = slf.borrow();
            (
                me.inner.sixth_minor(),
                me.inner.seventh_minor(),
                me.inner.case_matters(),
            )
        };
        let (key, _) = key_and_octave(Some(useScale))?;
        let applied = RsRomanNumeral::with_options(secondary, key, sixth, seventh, matters)
            .map_err(roman_error)?;
        // The key the applied numeral makes is the one its own chord stands
        // in: the `vi` of `V9/vi` in C major roots on A and is minor, so what
        // follows the slash is read in A minor.
        let established = crate::key::Key::object(py, established_key(&applied)?)?;
        let numeral = Py::new(py, Self::initializer(py, Self::wrap(applied, None))?)?.into_any();
        {
            let mut me = slf.borrow_mut();
            me.state.secondary = Some(Some(numeral));
            me.state.secondary_key = Some(Some(established.clone_ref(py)));
        }
        Ok((primary, established)
            .into_pyobject(py)?
            .into_any()
            .unbind())
    }

    /// music21's `_matchAccidentalsToQuality`: respells the third, fifth and
    /// seventh of the chord to the quality named.
    fn _matchAccidentalsToQuality(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        impliedQuality: &str,
    ) -> PyResult<()> {
        let quality = RsImpliedQuality::from_name(impliedQuality);
        slf.borrow_mut().state.implied_quality = Some(quality);
        let written: Vec<u8> = Vec::new();
        let mut pitches = slf.as_super().borrow().value_pitches();
        rs_roman::match_pitches_to_quality(&mut pitches, None, quality, &written)
            .map_err(roman_error)?;
        let chord = RsChord::new(pitches.as_slice()).map_err(roman_error)?;
        slf.as_super().borrow_mut().replace_value(py, chord)
    }

    /// music21's `adjustMinorVIandVIIByQuality`: a numeral on the sixth or
    /// seventh degree of a minor key takes the raised degree when the chord
    /// it names is one only the raised degree gives.
    fn adjustMinorVIandVIIByQuality(&mut self, useScale: &Bound<'_, PyAny>) -> PyResult<()> {
        let (key, _) = key_and_octave(Some(useScale))?;
        if key.mode() != "minor" || !self.inner.case_matters() {
            return Ok(());
        }
        let degree = self.get_scaleDegree();
        if !matches!(degree, 6 | 7) {
            return Ok(());
        }
        let reading = if degree == 6 {
            self.inner.sixth_minor()
        } else {
            self.inner.seventh_minor()
        };
        let wants_raised = matches!(
            self.implied_quality(),
            RsImpliedQuality::Minor
                | RsImpliedQuality::Diminished
                | RsImpliedQuality::HalfDiminished
        );
        let alteration = self.front_alteration();
        let raise = match reading {
            RsMinor67Default::Flat => false,
            RsMinor67Default::Sharp => true,
            RsMinor67Default::Quality => wants_raised,
            RsMinor67Default::Cautionary => match alteration {
                0 => wants_raised,
                sharps if sharps >= 1 => false,
                _ => true,
            },
        };
        if raise {
            self.state.alteration = Some(alteration + 1);
        }
        Ok(())
    }

    /// music21's `bassScaleDegreeFromNotation`: which scale degree the
    /// figured-bass column puts in the bass.
    #[pyo3(signature = (notationObject = None))]
    fn bassScaleDegreeFromNotation(
        &self,
        notationObject: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<u8> {
        let numbers = match notationObject.filter(|value| !value.is_none()) {
            Some(notation) => notation.getattr("numbers")?.extract::<Vec<u8>>()?,
            None => self.inner.figure_numbers(),
        };
        rs_roman::bass_scale_degree_from_notation(self.get_scaleDegree(), &numbers)
            .map_err(roman_error)
    }

    /// music21's `isNeapolitan`.
    #[pyo3(signature = (require1stInversion = true))]
    fn isNeapolitan(&self, require1stInversion: bool) -> bool {
        self.inner.is_neapolitan(require1stInversion)
    }

    /// music21's `isMixture`: whether the figure borrows from the parallel
    /// key.
    ///
    /// Only a major or a minor key has a parallel to borrow from, so a
    /// numeral read in a mode or over a scale is never mixture.
    #[pyo3(signature = (evaluateSecondaryNumeral = false))]
    fn isMixture(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        evaluateSecondaryNumeral: bool,
    ) -> PyResult<bool> {
        let key = Self::get_key(slf, py)?;
        let mode = key
            .bind(py)
            .getattr("mode")
            .ok()
            .and_then(|mode| mode.extract::<String>().ok());
        if !matches!(mode.as_deref(), Some("major" | "minor")) {
            return Ok(false);
        }
        slf.borrow()
            .inner
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

    /// music21's `pivotChord`: the same chord read again in the key the
    /// music turns to, which its `romanText` reader writes here.
    #[getter]
    fn get_pivotChord(&self, py: Python<'_>) -> Py<PyAny> {
        match &self.pivot {
            Some(pivot) => pivot.clone_ref(py),
            None => py.None(),
        }
    }

    #[setter]
    fn set_pivotChord(&mut self, value: &Bound<'_, PyAny>) {
        self.pivot = (!value.is_none()).then(|| value.clone().unbind());
    }

    /// music21's `followsKeyChange`: whether this is the first numeral after
    /// the music declared a new key.
    #[getter]
    fn get_followsKeyChange(&self) -> bool {
        self.follows_key_change
    }

    #[setter]
    fn set_followsKeyChange(&mut self, value: bool) {
        self.follows_key_change = value;
    }

    /// music21's `figuresWritten`: the digits under the numeral, with the
    /// numeral, its accidental and its quality symbol taken off and nothing
    /// expanded.
    #[getter]
    fn get_figuresWritten(&self) -> &str {
        if self.blank {
            return "";
        }
        self.inner.figures_written()
    }

    /// music21's `figuresNotationObj`: the column of numbers the numeral's
    /// digits stand for, expanded out of the shorthand and read as figured
    /// bass — which is what the numeral spells its notes from.
    #[getter]
    fn get_figuresNotationObj(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let column = if self.blank {
            music21_rs::figuredbass::Notation::default()
        } else {
            self.inner.figures_notation().clone()
        };
        Ok(crate::installed_new(
            py,
            "music21.figuredBass.notation",
            "Notation",
            crate::figuredbass::Notation { inner: column },
        )?
        .into_any())
    }

    /// music21's `primaryFigure`: the figure with the secondary numeral
    /// taken off it, and a chord written by name read as the figure it
    /// stands for.
    #[getter]
    fn get_primaryFigure(&self) -> String {
        if self.blank {
            return String::new();
        }
        let figure = self.inner.figure();
        let (primary, _) = music21_rs::roman::split_secondary(figure);
        if primary == "Cad64" {
            return if self.inner.key().mode() == "minor" {
                "i64".to_string()
            } else {
                "I64".to_string()
            };
        }
        primary.to_string()
    }

    /// music21's `scaleCardinality`: how many degrees the collection the
    /// numeral is read over has. A key has seven; a scale has as many as it
    /// has.
    #[getter]
    fn get_scaleCardinality(&self) -> usize {
        match self.inner.scale() {
            Some(scale) => scale
                .pitches()
                .map(|pitches| pitches.len().saturating_sub(1))
                .unwrap_or(7),
            None => 7,
        }
    }

    /// music21's `writeAsChord`: whether the numeral is written out as the
    /// notes it stands for rather than as a figure.
    #[getter]
    fn get_writeAsChord(&self) -> bool {
        self.write_as_chord
    }

    #[setter]
    fn set_writeAsChord(&mut self, value: bool) {
        self.write_as_chord = value;
    }

    /// music21 hashes a numeral by identity, since two numerals that read
    /// the same are still two objects a stream can hold apart.
    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        slf.as_ptr() as isize >> 4
    }

    fn __repr__(&self) -> String {
        let named = self.figureAndKey();
        if named.is_empty() {
            return "<music21.roman.RomanNumeral>".to_string();
        }
        format!("<music21.roman.RomanNumeral {named}>")
    }

    /// Two numerals are the same when they are the same figure in the same
    /// key *and* the same written note — music21 compares the `NotRest` half
    /// as well, so a numeral lengthened to a half note is no longer equal to
    /// the one it was.
    fn __eq__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let Ok(theirs) = other.extract::<PyRef<'_, Self>>() else {
            return Ok(false);
        };
        {
            let me = slf.borrow();
            if theirs.inner.figure() != me.inner.figure()
                || theirs.inner.key().tonic().name() != me.inner.key().tonic().name()
                || theirs.inner.key().mode() != me.inner.key().mode()
            {
                return Ok(false);
            }
        }
        // Only the written note, not the notes sounded: two numerals that
        // read the same figure are the same numeral however their chords are
        // voiced, but one lengthened to a half note is not.
        let py = other.py();
        let theirs = other.extract::<PyRef<'_, Chord>>()?;
        Ok(slf.as_super().borrow().quarter_length(py) == theirs.quarter_length(py))
    }

    /// A copy is one of whatever class this is, since music21 keeps it in a
    /// stream and a bare facade is not something a stream can hold.
    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        py: Python<'py>,
        _memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        Self::copied(slf, py)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        Self::copied(slf, py)
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
