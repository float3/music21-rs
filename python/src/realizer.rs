//! music21's `figuredBass.realizer`: a bass line with figures under it,
//! and every way of voicing it.
//!
//! A line is bass notes with their figures, read in a key and written in a
//! meter; realized, it is every voicing that keeps the rules, and each
//! voicing may be written out as a score. All of it is the crate's
//! `figuredbass::realizer`, which `realizer_parity` holds to music21's
//! output byte for byte as MusicXML.
//!
//! **Where music21 chooses at random the caller may still have it choose.**
//! The crate takes the choice as an argument -- a function given how many
//! voicings there are to choose from -- so `getRandomPossibilityProgression`
//! and the two random realizations choose through Python's own `random`,
//! which is what music21 draws from, and a caller who seeds `random` gets
//! the same realization twice.
//!
//! A voicing is a tuple of pitches, as music21's `Possibility` is, and the
//! scores handed back are this package's own streams.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyList, PyString, PyTuple};

use music21_rs_crate::figuredbass::realizer::{
    FiguredBassLine as RsLine, Realization as RsRealization,
};
use music21_rs_crate::figuredbass::segment::Possibility;
use music21_rs_crate::key::Key as RsKey;
use music21_rs_crate::meter::TimeSignature as RsTimeSignature;
use music21_rs_crate::{Pitch as RsPitch, Stream as RsStream};

use crate::Walkable;

/// The names this module hands out.
pub const NAMES: &[&str] = &["FiguredBassLine", "Realization", "FiguredBassLineException"];

pyo3::create_exception!(
    music21_rs_facade,
    FiguredBassLineException,
    crate::Music21Exception
);

/// music21's error for a line that cannot be read or realized.
fn line_error(error: &music21_rs_crate::Error) -> PyErr {
    FiguredBassLineException::new_err(crate::pitch::message(error))
}

/// The key a Python object stands for: one of this package's, or music21's
/// own read by its tonic and mode.
fn key_of(object: &Bound<'_, PyAny>) -> PyResult<RsKey> {
    if let Ok(ours) = object.extract::<PyRef<'_, crate::key::Key>>() {
        return Ok(ours.inner.clone());
    }
    let tonic = object
        .getattr("tonic")?
        .getattr("name")?
        .extract::<String>()?;
    let mode = object.getattr("mode")?.extract::<String>()?;
    RsKey::from_tonic_mode(&tonic, mode.as_str()).map_err(|error| line_error(&error))
}

/// The meter a Python object stands for: one of this package's, or
/// music21's own read by its ratio.
fn time_of(object: &Bound<'_, PyAny>) -> PyResult<RsTimeSignature> {
    if let Ok(ours) = object.extract::<PyRef<'_, crate::meter::TimeSignature>>() {
        return Ok(ours.inner.clone());
    }
    let ratio = object.getattr("ratioString")?.extract::<String>()?;
    RsTimeSignature::from_ratio_string(&ratio).map_err(|error| line_error(&error))
}

/// A voicing as music21 hands one back: a tuple of pitches, lowest part
/// last, each pitch an object of this package's.
fn voicing_object<'py>(py: Python<'py>, voicing: &Possibility) -> PyResult<Bound<'py, PyAny>> {
    let pitches = voicing
        .iter()
        .map(|pitch| {
            crate::installed_new(
                py,
                "music21.pitch",
                "Pitch",
                crate::pitch::Pitch::wrap(pitch.clone(), false),
            )
            .map(|made| made.into_any())
        })
        .collect::<PyResult<Vec<Py<PyAny>>>>()?;
    Ok(PyTuple::new(py, pitches)?.into_any())
}

/// A voicing read back off a tuple of pitches.
fn voicing_value(object: &Bound<'_, PyAny>) -> PyResult<Possibility> {
    object
        .walk()?
        .map(|pitch| crate::pitch::pitch_from_any(&pitch?))
        .collect()
}

/// A chooser over Python's own `random`, which is where music21 chooses
/// from: `randint(0, count - 1)`, and nothing to choose from is refused as
/// music21's `random.sample` refuses it.
fn random_choice(py: Python<'_>) -> PyResult<impl FnMut(usize) -> usize> {
    let randint = py.import("random")?.getattr("randint")?;
    Ok(move |count: usize| -> usize {
        if count == 0 {
            return 0;
        }
        randint
            .call1((0usize, count - 1))
            .and_then(|found| found.extract::<usize>())
            .unwrap_or(0)
    })
}

/// music21's `figuredBass.realizer.FiguredBassLine`.
#[pyclass(
    name = "FiguredBassLine",
    module = "music21.figuredBass.realizer",
    subclass,
    skip_from_py_object
)]
pub struct FiguredBassLine {
    inner: RsLine,
    key: Py<PyAny>,
    time: Py<PyAny>,
}

#[pymethods]
impl FiguredBassLine {
    /// music21's `FiguredBassLine(inKey, inTime)`, C major and common time
    /// where neither is said.
    #[new]
    #[pyo3(signature = (inKey = None, inTime = None))]
    fn make(
        py: Python<'_>,
        inKey: Option<&Bound<'_, PyAny>>,
        inTime: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let key = match inKey.filter(|given| !given.is_none()) {
            Some(given) => given.clone(),
            None => crate::class_of(py, "music21.key", "Key")?.call1(("C",))?,
        };
        let time = match inTime.filter(|given| !given.is_none()) {
            Some(given) => given.clone(),
            None => crate::class_of(py, "music21.meter", "TimeSignature")?.call1(("4/4",))?,
        };
        let inner = RsLine::in_key_and_time(&key_of(&key)?, time_of(&time)?)
            .map_err(|error| line_error(&error))?;
        Ok(Self {
            inner,
            key: key.unbind(),
            time: time.unbind(),
        })
    }

    /// The key its figures are read in: music21's `inKey`.
    #[getter]
    fn get_inKey(&self, py: Python<'_>) -> Py<PyAny> {
        self.key.clone_ref(py)
    }

    #[setter]
    fn set_inKey(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.inner = RsLine::in_key_and_time(&key_of(value)?, self.inner.time_signature().clone())
            .map_err(|error| line_error(&error))?;
        self.key = value.clone().unbind();
        Ok(())
    }

    /// The meter its realizations are written in: music21's `inTime`.
    #[getter]
    fn get_inTime(&self, py: Python<'_>) -> Py<PyAny> {
        self.time.clone_ref(py)
    }

    #[setter]
    fn set_inTime(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let time = time_of(value)?;
        self.inner = RsLine::in_key_and_time(&key_of(self.key.bind(value.py()))?, time)
            .map_err(|error| line_error(&error))?;
        self.time = value.clone().unbind();
        Ok(())
    }

    /// Adds a bass note with the figures written under it, or a chord
    /// symbol or roman numeral standing for the harmony: music21's
    /// `addElement`.
    ///
    /// As music21 does, the figures are written onto the note given as its
    /// lyrics and the string onto its `editorial.notationString`, so a
    /// caller's own note carries what it was added with. Anything else is
    /// refused with music21's own sentence.
    #[pyo3(signature = (bassObject, notationString = None))]
    fn addElement(
        &mut self,
        bassObject: &Bound<'_, PyAny>,
        notationString: Option<String>,
    ) -> PyResult<()> {
        let py = bassObject.py();
        // music21 writes the string onto the object before it reads what
        // the object is, so anything with no editorial is refused there
        // rather than here.
        bassObject
            .getattr("editorial")?
            .setattr("notationString", notationString.clone())?;
        let length = bassObject
            .getattr("duration")?
            .getattr("quarterLength")?
            .extract::<f64>()?;
        let note = PyString::new(py, "Note").into_any();
        let chordish = PyString::new(py, "Chord").into_any();
        if crate::stream::answers_to_class(bassObject, &note)? {
            let pitch = crate::pitch::pitch_from_any(&bassObject.getattr("pitch")?)?;
            self.inner
                .add_element(pitch, length, notationString.as_deref());
            addLyricsToBassNote(bassObject, notationString.unwrap_or_default())?;
            return Ok(());
        }
        if crate::stream::answers_to_class(bassObject, &chordish)? {
            let bass = crate::pitch::pitch_from_any(&bassObject.call_method0("bass")?)?;
            let mut names = Vec::new();
            for pitch in bassObject.getattr("pitches")?.walk()? {
                let name = pitch?.getattr("name")?.extract::<String>()?;
                if !names.contains(&name) {
                    names.push(name);
                }
            }
            self.inner.add_chord(bass, length, names);
            return Ok(());
        }
        Err(FiguredBassLineException::new_err(format!(
            "Not a valid bassObject (only note.Note, harmony.ChordSymbol, \
             and roman.RomanNumeral supported) was {}",
            bassObject.repr()?
        )))
    }

    /// The bass line as a part, its figures written under each note as
    /// lyrics and cut into measures: music21's `generateBassLine`.
    fn generateBassLine<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let made = slf
            .borrow()
            .inner
            .generate_bass_line()
            .map_err(|error| line_error(&error))?;
        made_stream(py, &made)
    }

    /// Lays a written part over the line, which every realization keeps as
    /// it stands: music21's `overlayPart`.
    fn overlayPart(&mut self, music21Part: &Bound<'_, PyAny>) -> PyResult<()> {
        let part = crate::stream::crate_stream(music21Part)?;
        self.inner.overlay_part(&part);
        Ok(())
    }

    /// A segment for each bass note, with the parts laid over the line held
    /// to where they stand: music21's `retrieveSegments`.
    #[pyo3(signature = (fbRules = None, numParts = 4, maxPitch = None))]
    fn retrieveSegments<'py>(
        slf: &Bound<'py, Self>,
        fbRules: Option<&Bound<'py, PyAny>>,
        numParts: usize,
        maxPitch: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let py = slf.py();
        let (rules, top) = read_rules(py, fbRules, maxPitch)?;
        let found = slf
            .borrow()
            .inner
            .segments(&rules, numParts, &top)
            .map_err(|error| line_error(&error))?;
        found
            .iter()
            .map(|segment| crate::segment::segment_object(py, segment))
            .collect()
    }

    /// Every way of voicing the line: music21's `realize`.
    ///
    /// A line with nothing in it is refused with music21's own sentence.
    #[pyo3(signature = (fbRules = None, numParts = 4, maxPitch = None))]
    fn realize<'py>(
        slf: &Bound<'py, Self>,
        fbRules: Option<&Bound<'py, PyAny>>,
        numParts: usize,
        maxPitch: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, Realization>> {
        let py = slf.py();
        let (rules, top) = read_rules(py, fbRules, maxPitch)?;
        let me = slf.borrow();
        let realization = me
            .inner
            .realize(&rules, numParts, &top)
            .map_err(|error| line_error(&error))?;
        Bound::new(
            py,
            Realization {
                inner: realization,
                key: me.key.clone_ref(py),
                time: me.time.clone_ref(py),
            },
        )
    }
}

/// music21's `addLyricsToBassNote`: the figures written under a bass note,
/// one to a line, each padded to the width of the widest.
#[pyfunction]
#[pyo3(signature = (bassNote, notationString = String::new()))]
fn addLyricsToBassNote(bassNote: &Bound<'_, PyAny>, notationString: String) -> PyResult<()> {
    let notation = music21_rs_crate::figuredbass::Notation::parse(&notationString)
        .map_err(|error| line_error(&error))?;
    let figures = notation.figure_strings();
    let width = figures
        .iter()
        .map(|figure| figure.chars().count())
        .max()
        .unwrap_or(0);
    for figure in figures {
        bassNote.call_method1("addLyric", (format!("{figure:>width$}"),))?;
    }
    Ok(())
}

/// music21's `figuredBass.realizer.Realization`.
#[pyclass(
    name = "Realization",
    module = "music21.figuredBass.realizer",
    subclass,
    skip_from_py_object
)]
pub struct Realization {
    inner: RsRealization,
    key: Py<PyAny>,
    time: Py<PyAny>,
}

#[pymethods]
impl Realization {
    /// Whether realizations are written in keyboard style, the upper parts
    /// as chords over the bass, or in chorale style with a staff per part:
    /// music21's `keyboardStyleOutput`.
    #[getter]
    fn get_keyboardStyleOutput(&self) -> bool {
        self.inner.keyboard_style_output()
    }

    #[setter]
    fn set_keyboardStyleOutput(&mut self, #[pyo3(from_py_with = crate::truthy)] value: bool) {
        self.inner.set_keyboard_style_output(value);
    }

    /// The key the line was read in.
    #[getter]
    fn get_inKey(&self, py: Python<'_>) -> Py<PyAny> {
        self.key.clone_ref(py)
    }

    /// The meter the realizations are written in.
    #[getter]
    fn get_inTime(&self, py: Python<'_>) -> Py<PyAny> {
        self.time.clone_ref(py)
    }

    /// How many ways there are of voicing the whole line: music21's
    /// `getNumSolutions`.
    fn getNumSolutions(&self) -> usize {
        self.inner.num_solutions()
    }

    /// Every way of voicing the line, each a list of voicings: music21's
    /// `getAllPossibilityProgressions`.
    fn getAllPossibilityProgressions<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        self.inner
            .all_possibility_progressions()
            .iter()
            .map(|progression| progression_object(py, progression))
            .collect()
    }

    /// One way of voicing the line, each voicing chosen at random through
    /// Python's own `random`: music21's
    /// `getRandomPossibilityProgression`.
    fn getRandomPossibilityProgression<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let found = self
            .inner
            .possibility_progression_by(random_choice(py)?)
            .map_err(|error| line_error(&error))?;
        progression_object(py, &found)
    }

    /// One way of voicing the line written out as a score: music21's
    /// `generateRealizationFromPossibilityProgression`.
    fn generateRealizationFromPossibilityProgression<'py>(
        &self,
        possibilityProgression: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = possibilityProgression.py();
        let progression = possibilityProgression
            .walk()?
            .map(|voicing| voicing_value(&voicing?))
            .collect::<PyResult<Vec<Possibility>>>()?;
        let made = self
            .inner
            .generate_realization_from_possibility_progression(&progression)
            .map_err(|error| line_error(&error))?;
        made_stream(py, &made)
    }

    /// Every way of voicing the line, written out one after another in one
    /// score: music21's `generateAllRealizations`.
    fn generateAllRealizations<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let made = self
            .inner
            .generate_all_realizations()
            .map_err(|error| line_error(&error))?;
        made_stream(py, &made)
    }

    /// One way of voicing the line written out as a score, chosen at
    /// random: music21's `generateRandomRealization`.
    fn generateRandomRealization<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let made = self
            .inner
            .generate_random_realization(random_choice(py)?)
            .map_err(|error| line_error(&error))?;
        made_stream(py, &made)
    }

    /// So many ways of voicing the line, written out one after another in
    /// one score: music21's `generateRandomRealizations`. Asked for more
    /// than there are, it answers every one of them.
    #[pyo3(signature = (amountToGenerate = 20))]
    fn generateRandomRealizations<'py>(
        &self,
        py: Python<'py>,
        amountToGenerate: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        let made = self
            .inner
            .generate_random_realizations(amountToGenerate, random_choice(py)?)
            .map_err(|error| line_error(&error))?;
        made_stream(py, &made)
    }
}

/// A progression of voicings as music21 hands one back: a list of tuples of
/// pitches.
fn progression_object<'py>(
    py: Python<'py>,
    progression: &[Possibility],
) -> PyResult<Bound<'py, PyAny>> {
    let voicings = progression
        .iter()
        .map(|voicing| voicing_object(py, voicing))
        .collect::<PyResult<Vec<Bound<'py, PyAny>>>>()?;
    Ok(PyList::new(py, voicings)?.into_any())
}

/// The rules and the top pitch a member was given, music21's defaults where
/// it was given neither.
fn read_rules<'py>(
    _py: Python<'py>,
    fbRules: Option<&Bound<'py, PyAny>>,
    maxPitch: Option<&Bound<'py, PyAny>>,
) -> PyResult<(music21_rs_crate::figuredbass::rules::Rules, RsPitch)> {
    let rules = match fbRules.filter(|given| !given.is_none()) {
        Some(given) => crate::segment::rules_of(given)?,
        None => music21_rs_crate::figuredbass::rules::Rules::default(),
    };
    let top = match maxPitch.filter(|given| !given.is_none()) {
        Some(given) => crate::pitch::pitch_from_any(given)?,
        None => RsPitch::from_name("B5").map_err(|error| line_error(&error))?,
    };
    Ok((rules, top))
}

/// A score the crate wrote, as this package's own streams.
fn made_stream<'py>(py: Python<'py>, made: &RsStream) -> PyResult<Bound<'py, PyAny>> {
    crate::stream::from_crate(py, made)
}

/// Adds the realizer classes to a module.
///
/// The exception is not among them: building one builds music21's own
/// `Music21Exception` and so imports music21, which this package is not to
/// do for a name nobody has asked for. `lib.rs`'s table hands it out on
/// first asking.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<FiguredBassLine>()?;
    m.add_class::<Realization>()?;
    m.add_function(wrap_pyfunction!(addLyricsToBassNote, m)?)?;
    Ok(())
}
