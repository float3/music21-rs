//! music21's `romanText.clercqTemperley`: a de Clercq-Temperley analysis
//! of the rock corpus read into a part.
//!
//! The reading is the crate's `romantext::clercq_temperley`, which
//! `clercq_temperley_parity` holds to music21 on 207 songs -- the six
//! music21 carries, eleven written for the test and 200 made up from the
//! whole grammar -- by outline and then byte for byte as MusicXML. A song
//! is its title, its comments, its rules and the key and meter it opens
//! in; a rule is its name, the section it stands for, its chords, its
//! comment and the measures it expands to.
//!
//! **The part is music21's own.** `toPart` answers a score music21's own
//! code indexes by class -- its own example asks the part for the figure
//! of its first roman numeral -- so the crate's measures are built as
//! music21's `stream.Part` and `stream.Measure` (`stream::music21_from_crate`,
//! which falls back to this package's streams for a wheel with no music21
//! beside it). The *elements* were already music21's: every one is built
//! through the install helper.
//!
//! **A formal division is read off the lyric the crate wrote.** music21
//! writes the name of the rule that brought a section both as a lyric
//! under its first chord and into that chord's
//! `editorial.formalDivision`, and the crate writes the lyric -- so the
//! facade reads the decision back off it rather than deciding again. A
//! numeral carrying no rule's name has an empty list, which is what
//! music21 gives every other numeral.
//!
//! **A rule with a song behind it is expanded by the song.** music21
//! keeps the expansion state on the rule and lets a rule expand itself
//! through its parent; the crate drives it from the song
//! (`CtSong::to_part`), which is why `expandExpansionContent`,
//! `expandSimpleContent`, `insertKsTs` and `addOptionalTieAndLyrics` --
//! the four steps inside that walk -- are refused rather than answered
//! differently, and why `expand` answers only for a rule standing on its
//! own, which is the branch music21's own docstrings show.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString, PyTuple};

use music21_rs_crate::Chord as RsChord;
use music21_rs_crate::romantext::clercq_temperley::{
    CtLabels, CtRule as RsCtRule, CtSong as RsCtSong, GroupKind, fixup_chord_atom, is_same,
};

use crate::Walkable;

/// The names this module hands out.
pub const NAMES: &[&str] = &["CTSong", "CTRule"];

pyo3::create_exception!(music21_rs_facade, CTSongException, crate::Music21Exception);
pyo3::create_exception!(music21_rs_facade, CTRuleException, crate::Music21Exception);

error_into!(pub(crate) song_error, CTSongException);
error_into!(pub(crate) rule_error, CTRuleException);

/// The song a text says, read afresh: the reading is cheap and a caller
/// may have written a new text onto the object.
fn read(text: &str) -> RsCtSong {
    RsCtSong::parse(text)
}

/// music21's `romanText.clercqTemperley.CTSong`: one analysis of the rock
/// corpus, as its text says it.
#[pyclass(
    name = "CTSong",
    module = "music21.romanText.clercqTemperley",
    subclass,
    skip_from_py_object,
    dict
)]
pub struct CTSong {
    /// The title a caller gave, which wins over the text's.
    #[pyo3(get, set)]
    _title: Py<PyAny>,
    /// The text as it was read, its blank lines gone.
    #[pyo3(get, set)]
    text: Py<PyAny>,
    /// The lines the text was read as.
    #[pyo3(get, set)]
    lines: Py<PyAny>,
    /// The year, which the format does not say and a caller may.
    #[pyo3(get, set)]
    year: Py<PyAny>,
    /// Whether each chord carries its figure as a lyric.
    #[pyo3(get, set)]
    labelRomanNumerals: bool,
    /// Whether the first chord of a section carries the rule's name.
    #[pyo3(get, set)]
    labelSubsectionsOnScore: bool,
    /// The keys written into the song so far, which music21 keeps so as
    /// not to write one twice.
    #[pyo3(get, set)]
    keyObjList: Py<PyAny>,
    /// The meters, likewise.
    #[pyo3(get, set)]
    tsList: Py<PyAny>,
    /// The part built, kept as music21 keeps it.
    #[pyo3(get, set)]
    _partObj: Py<PyAny>,
    /// The rules, kept once read.
    _rules: Option<Py<PyAny>>,
}

impl CTSong {
    /// The text the song was read from.
    fn said(&self, py: Python<'_>) -> PyResult<String> {
        let text = self.text.bind(py);
        if text.is_none() {
            return Ok(String::new());
        }
        Ok(text.str()?.to_str()?.to_string())
    }
}

#[pymethods]
impl CTSong {
    /// Its rules name it as their parent, a cycle through Rust that
    /// Python's collector frees only if it is shown them.
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self._title)?;
        visit.call(&self.text)?;
        visit.call(&self.lines)?;
        visit.call(&self.year)?;
        visit.call(&self.keyObjList)?;
        visit.call(&self.tsList)?;
        visit.call(&self._partObj)?;
        visit.call(&self._rules)
    }

    fn __clear__(&mut self) {
        self._rules = None;
    }

    /// music21's `CTSong(textFile, title=..., year=...)`.
    #[new]
    #[pyo3(signature = (textFile = None, **keywords))]
    fn new(
        py: Python<'_>,
        textFile: Option<Py<PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let _ = textFile;
        let said = |name: &str| -> Py<PyAny> {
            keywords
                .and_then(|keywords| keywords.get_item(name).ok().flatten())
                .map_or_else(|| py.None(), Bound::unbind)
        };
        Ok(Self {
            _title: said("title"),
            text: PyString::new(py, "").into_any().unbind(),
            lines: PyList::empty(py).into_any().unbind(),
            year: said("year"),
            labelRomanNumerals: true,
            labelSubsectionsOnScore: true,
            keyObjList: PyList::empty(py).into_any().unbind(),
            tsList: PyList::empty(py).into_any().unbind(),
            _partObj: py.None(),
            _rules: None,
        })
    }

    /// The text read at once, as music21's own `__init__` reads it --
    /// through the object, so a subclass's `parse` is the one that runs.
    #[pyo3(signature = (textFile = None, **keywords))]
    fn __init__(
        slf: &Bound<'_, Self>,
        textFile: Option<Py<PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let _ = keywords;
        let py = slf.py();
        let given = textFile.unwrap_or_else(|| PyString::new(py, "").into_any().unbind());
        slf.call_method1("parse", (given,))?;
        Ok(())
    }

    /// The text of a song, read from a string or from the file a path
    /// names: music21's `parse`.
    ///
    /// music21 reads the string as the analysis itself where it holds a
    /// `|` and an `S:`, and otherwise opens it as a file -- which is file
    /// IO, so the lines are read here and the reading of them is the
    /// crate's.
    fn parse(slf: &Bound<'_, Self>, textFile: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let given = if textFile.is_none() {
            String::new()
        } else {
            textFile.str()?.to_str()?.to_string()
        };
        let song = if given.contains('|') && given.contains("S:") {
            read(&given)
        } else if given.is_empty() {
            read("")
        } else {
            let keywords = PyDict::new(py);
            keywords.set_item("encoding", "utf-8")?;
            keywords.set_item("errors", "replace")?;
            let opened = py
                .import("io")?
                .call_method("open", (textFile, "r"), Some(&keywords))
                .map_err(|_| CTSongException::new_err(format!("Cannot find file: {given}")))?;
            let mut lines = Vec::new();
            for line in opened.call_method0("readlines")?.walk()? {
                lines.push(line?.str()?.to_str()?.to_string());
            }
            opened.call_method0("close")?;
            RsCtSong::from_lines(lines)
        };
        let listed = PyList::empty(py);
        for line in song.lines() {
            listed.append(PyString::new(py, line))?;
        }
        let mut me = slf.borrow_mut();
        me.text = PyString::new(py, song.text()).into_any().unbind();
        me.lines = listed.into_any().unbind();
        me._rules = None;
        Ok(())
    }

    /// The title: the one a caller gave, else the first line's with its
    /// per cent signs taken off, as music21 reads it.
    #[getter]
    fn title(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let me = slf.borrow();
        let given = me._title.bind(py);
        if !given.is_none() {
            return Ok(given.clone().unbind());
        }
        let said = read(&me.said(py)?).title();
        if said.is_empty() {
            return Ok(py.None());
        }
        Ok(PyString::new(py, &said).into_any().unbind())
    }

    #[setter]
    fn set_title(&mut self, value: Py<PyAny>) {
        self._title = value;
    }

    /// Every comment of the song but the title, each as music21 lists it:
    /// a comment on a rule's line is that rule's name and the comment,
    /// and one on a line of its own is the comment alone.
    #[getter]
    fn comments<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let said = slf.borrow().said(py)?;
        let out = PyList::empty(py);
        for comment in read(&said).comments() {
            let listed = PyList::empty(py);
            for part in comment {
                listed.append(PyString::new(py, &part))?;
            }
            out.append(listed)?;
        }
        Ok(out)
    }

    /// The rules of the song by name, in the order the text names them:
    /// music21's `rules`, which is an ordered dictionary.
    #[getter]
    fn rules(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        if let Some(held) = &slf.borrow()._rules {
            return Ok(held.clone_ref(py));
        }
        let said = slf.borrow().said(py)?;
        let class = crate::installed_class(py, "music21.romanText.clercqTemperley", "CTRule")
            .unwrap_or_else(|| py.get_type::<CTRule>().into_any());
        let out = py.import("collections")?.getattr("OrderedDict")?.call0()?;
        for rule in read(&said).rules() {
            let made = class.call1((rule.text(), slf))?;
            out.set_item(rule.left_hand_side(), made)?;
        }
        let made = out.unbind();
        slf.borrow_mut()._rules = Some(made.clone_ref(py));
        Ok(made)
    }

    /// The meter the song opens in: the one its `S:` rule states, else
    /// four-four, as music21 answers.
    #[getter]
    fn homeTimeSig(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let said = slf.borrow().said(py)?;
        let meter = read(&said)
            .home_time_signature()
            .map_err(song_error)?
            .ok_or_else(|| CTSongException::new_err("a song with no S: rule to read"))?;
        let element = music21_rs_crate::StreamElement::TimeSignature(meter);
        crate::stream::element_object(py, &element)?
            .map(Bound::unbind)
            .ok_or_else(|| CTSongException::new_err("no class for a time signature"))
    }

    /// The key it opens in: the one its `S:` rule states, else C major.
    #[getter]
    fn homeKey(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let said = slf.borrow().said(py)?;
        let key = read(&said)
            .home_key()
            .map_err(song_error)?
            .ok_or_else(|| CTSongException::new_err("a song with no S: rule to read"))?;
        let element = music21_rs_crate::StreamElement::Key(key);
        crate::stream::element_object(py, &element)?
            .map(Bound::unbind)
            .ok_or_else(|| CTSongException::new_err("no class for a key"))
    }

    /// The song played through as a part: music21's `toPart`.
    ///
    /// The part is music21's own where music21 can be asked for one, since
    /// its own code indexes a part by class; the chords in it are built by
    /// the install helper either way.
    #[pyo3(signature = (labelRomanNumerals = true, labelSubsectionsOnScore = true))]
    fn toPart(
        slf: &Bound<'_, Self>,
        #[pyo3(from_py_with = crate::truthy)] labelRomanNumerals: bool,
        #[pyo3(from_py_with = crate::truthy)] labelSubsectionsOnScore: bool,
    ) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        {
            let mut me = slf.borrow_mut();
            me.labelRomanNumerals = labelRomanNumerals;
            me.labelSubsectionsOnScore = labelSubsectionsOnScore;
        }
        let said = slf.borrow().said(py)?;
        let part = read(&said)
            .to_part_with(CtLabels {
                roman_numerals: labelRomanNumerals,
                subsections: labelSubsectionsOnScore,
            })
            .map_err(song_error)?;
        let made = crate::stream::music21_from_crate(py, &part)?;
        let names: Vec<String> = read(&said)
            .rules()
            .iter()
            .map(|rule| rule.left_hand_side().to_string())
            .collect();
        divisions_written(&made, &names)?;
        slf.borrow_mut()._partObj = made.clone().unbind();
        Ok(made.unbind())
    }

    /// music21's own deprecated name for `toPart`, which it will take out.
    #[pyo3(signature = (labelRomanNumerals = true, labelSubsectionsOnScore = true))]
    fn toScore(
        slf: &Bound<'_, Self>,
        #[pyo3(from_py_with = crate::truthy)] labelRomanNumerals: bool,
        #[pyo3(from_py_with = crate::truthy)] labelSubsectionsOnScore: bool,
    ) -> PyResult<Py<PyAny>> {
        Self::toPart(slf, labelRomanNumerals, labelSubsectionsOnScore)
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let py = slf.py();
        let title = Self::title(slf)?;
        let title = title.bind(py);
        let said = if title.is_none() {
            "None".to_string()
        } else {
            title.repr()?.to_str()?.to_string()
        };
        let year = slf.borrow().year.bind(py).str()?.to_str()?.to_string();
        Ok(format!(
            "<music21.romanText.clercqTemperley.CTSong title={said} year={year}>"
        ))
    }
}

/// The name of the rule that brought each section, written into the
/// numeral's editorial as music21 writes it beside the lyric.
fn divisions_written(part: &Bound<'_, PyAny>, names: &[String]) -> PyResult<()> {
    // The numerals and the rests, which are what music21 writes a formal
    // division onto; a stream answers `lyrics` with a method of its own.
    let walked = part.call_method0("recurse")?.getattr("notesAndRests")?;
    for element in walked.walk()? {
        let element = element?;
        let Ok(lyrics) = element.getattr("lyrics") else {
            continue;
        };
        let Ok(editorial) = element.getattr("editorial") else {
            continue;
        };
        let found = PyList::empty(element.py());
        for lyric in lyrics.walk()? {
            let text = lyric?.getattr("text")?;
            if text.is_none() {
                continue;
            }
            let text = text.str()?.to_str()?.to_string();
            if names.contains(&text) {
                found.append(PyString::new(element.py(), &text))?;
            }
        }
        editorial.setattr("formalDivision", found)?;
    }
    Ok(())
}

/// music21's `romanText.clercqTemperley.CTRule`: one rule of a song, a
/// line of its text.
#[pyclass(
    name = "CTRule",
    module = "music21.romanText.clercqTemperley",
    subclass,
    skip_from_py_object,
    dict
)]
pub struct CTRule {
    /// The line as it was written, its name and comment included.
    #[pyo3(get, set)]
    text: Py<PyAny>,
    /// The song holding the rule, where one does.
    _parent: Py<PyAny>,
    /// The chords of the rule, where a caller wrote them itself.
    _musicText: Py<PyAny>,
    /// The rule's name, likewise.
    _LHS: Py<PyAny>,
    /// The measures the rule last expanded to.
    #[pyo3(get, set)]
    measures: Py<PyAny>,
}

impl CTRule {
    /// The line the rule was read from.
    fn said(&self, py: Python<'_>) -> PyResult<String> {
        let text = self.text.bind(py);
        if text.is_none() {
            return Ok(String::new());
        }
        Ok(text.str()?.to_str()?.to_string())
    }
}

#[pymethods]
impl CTRule {
    /// The song it belongs to, which holds it: see `CTSong.__traverse__`.
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.text)?;
        visit.call(&self._parent)?;
        visit.call(&self._musicText)?;
        visit.call(&self._LHS)?;
        visit.call(&self.measures)
    }

    fn __clear__(&mut self) {
        Python::attach(|py| self._parent = py.None());
    }

    /// music21's `CTRule(text, parent)`.
    #[new]
    #[pyo3(signature = (text = None, parent = None))]
    fn new(py: Python<'_>, text: Option<Py<PyAny>>, parent: Option<Py<PyAny>>) -> Self {
        Self {
            text: text.unwrap_or_else(|| PyString::new(py, "").into_any().unbind()),
            _parent: parent.unwrap_or_else(|| py.None()),
            _musicText: py.None(),
            _LHS: py.None(),
            measures: PyList::empty(py).into_any().unbind(),
        }
    }

    /// The song holding the rule: music21's `parent`, a weak reference
    /// back to it.
    #[getter]
    fn parent(&self, py: Python<'_>) -> Py<PyAny> {
        self._parent.clone_ref(py)
    }

    #[setter]
    fn set_parent(&mut self, value: Py<PyAny>) {
        self._parent = value;
    }

    /// The chords of the rule, its name and comment left off.
    #[getter]
    fn musicText(slf: &Bound<'_, Self>) -> PyResult<String> {
        let py = slf.py();
        let given = slf.borrow()._musicText.bind(py).clone();
        if !given.is_none() {
            return Ok(given.str()?.to_str()?.to_string());
        }
        let said = slf.borrow().said(py)?;
        Ok(RsCtRule::new(&said).music_text().to_string())
    }

    #[setter]
    fn set_musicText(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self._musicText = PyString::new(py, value.str()?.to_str()?)
            .into_any()
            .unbind();
        Ok(())
    }

    /// The comment after the rule's chords, where it wrote one.
    #[getter]
    fn comment(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let said = slf.borrow().said(py)?;
        Ok(match RsCtRule::new(&said).comment() {
            Some(comment) => PyString::new(py, comment).into_any().unbind(),
            None => py.None(),
        })
    }

    /// The rule's name, which the text writes before its colon.
    #[getter]
    fn LHS(slf: &Bound<'_, Self>) -> PyResult<String> {
        let py = slf.py();
        let given = slf.borrow()._LHS.bind(py).clone();
        if !given.is_none() {
            return Ok(given.str()?.to_str()?.to_string());
        }
        let said = slf.borrow().said(py)?;
        Ok(RsCtRule::new(&said).left_hand_side().to_string())
    }

    #[setter]
    fn set_LHS(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self._LHS = PyString::new(py, value.str()?.to_str()?)
            .into_any()
            .unbind();
        Ok(())
    }

    /// The section the rule stands for, its name written out.
    #[getter]
    fn sectionName(slf: &Bound<'_, Self>) -> PyResult<String> {
        let py = slf.py();
        let said = slf.borrow().said(py)?;
        Ok(RsCtRule::new(&said).section_name())
    }

    /// The groups the rule's text makes, each with what it stands for and
    /// how many times it is played: music21's `_measureGroups`.
    fn _measureGroups<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let said = slf.borrow().said(py)?;
        let out = PyList::empty(py);
        for group in RsCtRule::new(&said).measure_groups() {
            let kind = match group.kind {
                GroupKind::Measure => "|",
                GroupKind::Expansion => "$",
                GroupKind::Loose => "",
                GroupKind::Continued => "?",
            };
            out.append(PyTuple::new(
                py,
                [
                    PyString::new(py, &group.content).into_any(),
                    PyString::new(py, kind).into_any(),
                    group.repeats.into_pyobject(py)?.into_any(),
                ],
            )?)?;
        }
        Ok(out)
    }

    /// The measures the rule expands to: music21's `expand`.
    ///
    /// Only a rule standing on its own is answered -- the branch music21's
    /// own docstrings show. A song expands its own rules here
    /// (`CtSong::to_part`), keeping the state music21 keeps on each rule,
    /// so a rule with a song behind it is expanded by asking the song.
    #[pyo3(signature = (tsContext = None, keyContext = None))]
    fn expand(
        slf: &Bound<'_, Self>,
        tsContext: Option<&Bound<'_, PyAny>>,
        keyContext: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        if !slf.borrow()._parent.bind(py).is_none() {
            return Err(CTRuleException::new_err(
                "a rule with a song behind it is expanded by the song, which keeps the \
                 state music21 keeps on the rule: ask the song for its part",
            ));
        }
        let said = slf.borrow().said(py)?;
        let meter = match tsContext.filter(|given| !given.is_none()) {
            Some(given) => Some(
                music21_rs_crate::TimeSignature::from_ratio_string(
                    given.getattr("ratioString")?.str()?.to_str()?,
                )
                .map_err(rule_error)?,
            ),
            None => None,
        };
        let key = match keyContext.filter(|given| !given.is_none()) {
            Some(given) => Some(
                music21_rs_crate::Key::from_tonic(
                    given.getattr("tonicPitchNameWithCase")?.str()?.to_str()?,
                )
                .map_err(rule_error)?,
            ),
            None => None,
        };
        let measures = RsCtRule::new(&said)
            .expand_in(meter, key)
            .map_err(rule_error)?;
        let out = PyList::empty(py);
        for measure in &measures {
            out.append(crate::stream::music21_from_crate(py, measure)?)?;
        }
        slf.borrow_mut().measures = out.clone().into_any().unbind();
        Ok(out.into_any().unbind())
    }

    /// A figure as music21 reads one: music21's `fixupChordAtom`.
    #[staticmethod]
    fn fixupChordAtom(atom: &str) -> String {
        fixup_chord_atom(atom)
    }

    /// Whether a numeral sounds the notes the chord before it sounded,
    /// which is what decides a tie: music21's `isSame`.
    fn isSame(
        &self,
        rn: &Bound<'_, PyAny>,
        lastChord: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        let read = |object: &Bound<'_, PyAny>| -> PyResult<RsChord> {
            let mut names = Vec::new();
            for pitch in object.getattr("pitches")?.walk()? {
                names.push(crate::pitch::pitch_from_any(&pitch?)?.name_with_octave());
            }
            RsChord::new(names.join(" ")).map_err(rule_error)
        };
        let Some(last) = lastChord.filter(|given| !given.is_none()) else {
            return Ok(false);
        };
        Ok(is_same(&read(rn)?, Some(&read(last)?)))
    }

    /// One step of the expansion, which the crate drives from the song.
    #[pyo3(signature = (content = None, numReps = 1))]
    fn expandExpansionContent(&self, content: Option<&str>, numReps: usize) -> PyResult<()> {
        let _ = (content, numReps);
        Err(step_refused("expandExpansionContent"))
    }

    /// The same.
    #[pyo3(signature = (content = None, numReps = 1))]
    fn expandSimpleContent(&self, content: Option<&str>, numReps: usize) -> PyResult<()> {
        let _ = (content, numReps);
        Err(step_refused("expandSimpleContent"))
    }

    /// The same: the crate writes the key and the meter into a measure as
    /// it builds it.
    #[pyo3(signature = (m = None, ts = None, keyObj = None))]
    fn insertKsTs(
        &self,
        m: Option<&Bound<'_, PyAny>>,
        ts: Option<&Bound<'_, PyAny>>,
        keyObj: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let _ = (m, ts, keyObj);
        Err(step_refused("insertKsTs"))
    }

    /// The same: the crate ties the chord before as it writes the one
    /// after.
    #[pyo3(signature = (m = None, atom = None, newChord = None))]
    fn addOptionalTieAndLyrics(
        &self,
        m: Option<&Bound<'_, PyAny>>,
        atom: Option<&str>,
        newChord: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let _ = (m, atom, newChord);
        Err(step_refused("addOptionalTieAndLyrics"))
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let py = slf.py();
        let said = slf.borrow().text.bind(py).repr()?.to_str()?.to_string();
        Ok(format!(
            "<music21.romanText.clercqTemperley.CTRule text={said}>"
        ))
    }
}

/// What a step inside the expansion says for itself.
fn step_refused(name: &str) -> PyErr {
    CTRuleException::new_err(format!(
        "{name} is a step inside the expansion, which the crate drives from the song \
         rather than from the rule keeping the state: ask the song for its part"
    ))
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<CTSong>()?;
    m.add_class::<CTRule>()?;
    Ok(())
}
