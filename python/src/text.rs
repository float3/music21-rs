//! music21's `text`: a score's words, read off its notes or standing on
//! the page by themselves.
//!
//! `assembleLyrics`, `assembleAllLyrics`, `prependArticle` and
//! `postpendArticle` are the crate's, and a `TextBox` is a piece of text
//! with the page it stands on and the style it is drawn with -- music21's
//! own `TextStyle` object where music21 is there to ask, as every styled
//! thing in this package is drawn, since its exporters read the page off
//! that object.
//!
//! The class a box is drawn with is music21's `TextStyle`, which the
//! install helper carries over from the class this one replaces
//! (`_styleClass`); building that attribute here would import music21 the
//! moment the package was imported, which
//! `test_importing_the_package_imports_no_music21` is there to stop.
//!
//! `LanguageDetector` and `Trigram` are the crate's too: the wheel carries
//! the `language-detection` feature and its seven excerpts, so a text's
//! language is read here without music21. `Trigram.makeWords` and
//! `likely` are left to music21, since they draw letters at random, and so
//! is a trigram's `lut`, the table those read.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use music21::features::language;
use music21::text as rs;

pyo3::create_exception!(music21_rs_facade, TextException, crate::Music21Exception);

/// The names this module hands out.
pub const NAMES: &[&str] = &[
    "assembleLyrics",
    "assembleAllLyrics",
    "prependArticle",
    "postpendArticle",
    "TextBox",
    "TextException",
    "LanguageDetector",
    "Trigram",
];

/// The names installed over music21's own: the two classes that read a
/// text's language, which hold no music and so stand in wherever music21
/// builds one.
pub const INSTALLED_NAMES: &[&str] = &["LanguageDetector", "Trigram"];

/// music21's `assembleLyrics`: one line of a stream's lyrics, read note by
/// note, with a space between words and none inside a word a hyphen split.
#[pyfunction]
#[pyo3(signature = (streamIn, lineNumber = 1, *, wordSeparator = " ".to_string()))]
fn assembleLyrics(
    streamIn: &Bound<'_, PyAny>,
    lineNumber: i32,
    wordSeparator: String,
) -> PyResult<String> {
    let read = crate::stream::crate_stream(streamIn)?;
    Ok(rs::assemble_lyrics(&read, lineNumber, &wordSeparator))
}

/// music21's `assembleAllLyrics`: every line of them, one after another.
#[pyfunction]
#[pyo3(signature = (
    streamIn,
    maxLyrics = 10,
    lyricSeparation = "\n".to_string(),
    *,
    wordSeparator = " ".to_string(),
))]
fn assembleAllLyrics(
    streamIn: &Bound<'_, PyAny>,
    maxLyrics: i32,
    lyricSeparation: String,
    wordSeparator: String,
) -> PyResult<String> {
    let read = crate::stream::crate_stream(streamIn)?;
    Ok(rs::assemble_all_lyrics(
        &read,
        maxLyrics,
        &lyricSeparation,
        &wordSeparator,
    ))
}

/// music21's `prependArticle`: a title's article put back in front of it,
/// `Ale is Dear, The` read as `The Ale is Dear`.
#[pyfunction]
#[pyo3(signature = (src, language = None))]
fn prependArticle(src: &str, language: Option<&str>) -> PyResult<String> {
    rs::prepend_article(src, language).map_err(|error| TextException::new_err(said(&error)))
}

/// music21's `postpendArticle`: a title's article moved to the end of it,
/// `The Ale is Dear` read as `Ale is Dear, The`.
#[pyfunction]
#[pyo3(signature = (src, language = None))]
fn postpendArticle(src: &str, language: Option<&str>) -> PyResult<String> {
    rs::postpend_article(src, language).map_err(|error| TextException::new_err(said(&error)))
}

/// What a refusal of the crate's says.
fn said(error: &music21::Error) -> String {
    crate::pitch::message(error)
}

/// music21's `text.TextBox`: words standing anywhere on a page, which a
/// score's title and its credits are.
#[pyclass(
    name = "TextBox",
    module = "music21.text",
    extends = crate::base::Music21Object,
    subclass,
    skip_from_py_object,
    weakref
)]
pub struct TextBox {
    /// The words, or nothing where none were given -- which music21 keeps
    /// apart from the empty string, since a box with no text at all says
    /// nothing in its `repr`.
    content: Option<String>,
    /// The page it stands on, the first where nothing says.
    page: i64,
    /// How it is drawn, built the first time anything asks: music21's own
    /// `TextStyle` where music21 is there to ask.
    style: Option<Py<PyAny>>,
    /// Where the text was told to stand, which is written into the style
    /// when that is built: music21's `TextBox(content, x, y)`.
    placed: (Py<PyAny>, Py<PyAny>),
}

#[pymethods]
impl TextBox {
    /// Refuses a member nothing here defines, as Python refuses it.
    fn __getattr__(slf: &Bound<'_, Self>, name: &str) -> PyResult<Py<PyAny>> {
        crate::stream::no_site(slf.as_any(), name)
    }

    /// music21's `TextBox(content=None, x=500, y=500)`, which hangs the
    /// text from the top of the page and centres it across.
    #[new]
    #[pyo3(signature = (content = None, x = None, y = None, **_keywords))]
    fn new(
        py: Python<'_>,
        content: Option<String>,
        x: Option<&Bound<'_, PyAny>>,
        y: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let given = |value: Option<&Bound<'_, PyAny>>| -> PyResult<Py<PyAny>> {
            match value.filter(|value| !value.is_none()) {
                Some(value) => Ok(value.clone().unbind()),
                // music21's own default, five hundred tenths each way.
                None => Ok(500i32.into_pyobject(py)?.into_any().unbind()),
            }
        };
        Ok(crate::base::built(Self {
            content,
            page: 1,
            style: None,
            placed: (given(x)?, given(y)?),
        }))
    }

    /// music21's `classSortOrder`, which puts a text box before the text
    /// expressions standing where it does.
    #[classattr]
    fn classSortOrder() -> i32 {
        -31
    }

    /// What music21 says this object is.
    #[getter]
    fn classes(&self) -> Vec<String> {
        vec!["TextBox".to_string(), "Music21Object".to_string()]
    }

    /// The words: music21's `content`, which takes a number as readily as
    /// a string and keeps what it is given as text.
    #[getter]
    fn content(&self) -> Option<&str> {
        self.content.as_deref()
    }

    #[setter]
    fn set_content(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.content = match value.filter(|value| !value.is_none()) {
            None => None,
            Some(value) => Some(value.str()?.to_string()),
        };
        Ok(())
    }

    /// The page it stands on: music21's `page`, which must be a whole
    /// number of one or more.
    #[getter]
    fn page(&self) -> i64 {
        self.page
    }

    #[setter]
    fn set_page(&mut self, value: Option<i64>) -> PyResult<()> {
        self.page = match value {
            None => 1,
            Some(page) if page >= 1 => page,
            Some(page) => {
                return Err(TextException::new_err(format!(
                    "Page must be a positive integer, not {page}"
                )));
            }
        };
        Ok(())
    }

    /// How it is drawn, where it stands and in what font: music21's
    /// `style`, which is its own `TextStyle` object where music21 is there
    /// to ask. The same object every time, so a caller writing into it is
    /// writing into this box's style, as music21's is.
    #[getter]
    fn style(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        if let Some(held) = &slf.borrow().style {
            return Ok(held.clone_ref(py));
        }
        let made = crate::notation::new_style(slf.as_any(), None)?;
        // Where the text hangs and how it is aligned, as music21's
        // `__init__` writes them: where it was told to stand, and five
        // hundred tenths each way where it was told nothing.
        let style = made.bind(py);
        let placed = {
            let me = slf.borrow();
            (me.placed.0.clone_ref(py), me.placed.1.clone_ref(py))
        };
        style.setattr("absoluteX", placed.0)?;
        style.setattr("absoluteY", placed.1)?;
        style.setattr("alignVertical", "top")?;
        style.setattr("alignHorizontal", "center")?;
        slf.borrow_mut().style = Some(made.clone_ref(py));
        Ok(made)
    }

    #[setter]
    fn set_style(slf: &Bound<'_, Self>, value: Option<Py<PyAny>>) {
        slf.borrow_mut().style = value;
    }

    /// Whether anything has asked how it is drawn: music21's
    /// `hasStyleInformation`.
    #[getter]
    fn hasStyleInformation(&self) -> bool {
        self.style.is_some()
    }

    /// music21's `_reprInternal`: the words, cut at ten characters.
    fn __repr__(&self) -> String {
        match &self.content {
            None => "<music21.text.TextBox>".to_string(),
            Some(content) if content.chars().count() > 10 => {
                let short: String = content.chars().take(10).collect();
                format!("<music21.text.TextBox {:?}>", format!("{short}..."))
            }
            Some(content) => format!("<music21.text.TextBox {content:?}>"),
        }
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let copied = slf.get_type().call0()?;
        let held = {
            let me = slf.borrow();
            (
                me.content.clone(),
                me.page,
                me.style.as_ref().map(|style| style.clone_ref(py)),
            )
        };
        let style = match held.2 {
            None => None,
            Some(style) => {
                let copier = py.import("copy")?.getattr("deepcopy")?;
                Some(match memo {
                    Some(memo) => copier.call1((style, memo))?.unbind(),
                    None => copier.call1((style,))?.unbind(),
                })
            }
        };
        let placed = {
            let me = slf.borrow();
            (me.placed.0.clone_ref(py), me.placed.1.clone_ref(py))
        };
        {
            let mut copy = copied.extract::<PyRefMut<'_, Self>>()?;
            copy.content = held.0;
            copy.page = held.1;
            copy.style = style;
            copy.placed = placed;
        }
        crate::base::copied_from(slf.as_any(), &copied)?;
        Ok(copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::__deepcopy__(slf, None)
    }
}

/// music21's `text.Trigram`: how often each letter follows each pair of
/// letters in a text, which is what a language is told by.
#[pyclass(
    name = "Trigram",
    module = "music21.text",
    subclass,
    skip_from_py_object
)]
pub struct Trigram {
    /// The crate's counts.
    held: language::Trigram,
    /// Whether anything has been counted yet: music21's `length` is
    /// `None` until something has.
    measured: bool,
}

impl Trigram {
    fn of(held: language::Trigram) -> Self {
        Self {
            held,
            measured: true,
        }
    }
}

#[pymethods]
impl Trigram {
    /// music21's `Trigram(excerptList=None)`: a string read letter by
    /// letter, or a list of words read each followed by a space.
    #[new]
    #[pyo3(signature = (excerptList = None))]
    fn new(excerptList: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let mut made = Self {
            held: language::Trigram::default(),
            measured: false,
        };
        if let Some(excerpt) = excerptList.filter(|given| !given.is_none()) {
            made.parseExcerpt(excerpt)?;
        }
        Ok(made)
    }

    /// The square root of the sum of every count squared, or `None` before
    /// anything is counted: music21's `length`.
    #[getter]
    fn length(&self) -> Option<f64> {
        self.measured.then(|| self.held.length())
    }

    /// Counts a text's trigrams into these: music21's `parseExcerpt`,
    /// which reads a list as words and anything else letter by letter.
    fn parseExcerpt(&mut self, excerpt: &Bound<'_, PyAny>) -> PyResult<()> {
        if excerpt.is_instance_of::<PyList>() {
            let words: Vec<String> = excerpt.extract()?;
            self.held.add_words(words.iter().map(String::as_str));
        } else {
            self.held.add_text(&excerpt.extract::<String>()?);
        }
        self.measured = true;
        Ok(())
    }

    /// music21's `measure`, which works out `length`. Counting does that
    /// here, so all this does is say a trigram with nothing counted has a
    /// length of nought.
    fn measure(&mut self) {
        self.measured = true;
    }

    /// How alike two texts' trigrams are, from nought to one: music21's
    /// `similarity`.
    fn similarity(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let Ok(other) = other.cast::<Self>() else {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "can't compare Trigram with non-Trigram",
            ));
        };
        let other = other.borrow();
        let ours = self.length();
        let theirs = other.length();
        if ours.unwrap_or(0.0) == 0.0 || theirs.unwrap_or(0.0) == 0.0 {
            // Nothing in common with an empty trigram, and music21 divides
            // by its length anyway: Python's own refusal, word for word.
            let product = ours.into_pyobject(py)?.mul(theirs.into_pyobject(py)?)?;
            return Ok(0i64.into_pyobject(py)?.div(product)?.unbind());
        }
        Ok(self
            .held
            .similarity(&other.held)
            .into_pyobject(py)?
            .into_any()
            .unbind())
    }

    /// One less [`Self::similarity`]: music21's `-`.
    fn __sub__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let alike = self.similarity(py, other)?;
        Ok(1.0f64.into_pyobject(py)?.sub(alike)?.unbind())
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.get_type().call0()?;
        {
            let me = slf.borrow();
            let mut copy = copied.extract::<PyRefMut<'_, Self>>()?;
            copy.held = me.held.clone();
            copy.measured = me.measured;
        }
        Ok(copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::__deepcopy__(slf, None)
    }
}

/// music21's `text.LanguageDetector`: which of seven languages a text is
/// most like, by its trigrams.
#[pyclass(
    name = "LanguageDetector",
    module = "music21.text",
    subclass,
    skip_from_py_object
)]
pub struct LanguageDetector {
    /// The text it was given, which music21 keeps and reads nothing of.
    #[pyo3(get, set)]
    text: Py<PyAny>,
    /// The trigram of each language's excerpt, built the first time
    /// anything asks.
    trigrams: Option<Py<PyDict>>,
}

/// Each language's trigram by its code, in music21's order.
fn excerpt_trigrams(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let made = PyDict::new(py);
    for code in language::LANGUAGE_CODES {
        let Some(known) = language::known_trigram(code) else {
            continue;
        };
        made.set_item(code, Py::new(py, Trigram::of(known.clone()))?)?;
    }
    Ok(made)
}

#[pymethods]
impl LanguageDetector {
    /// music21's `LanguageDetector(text=None)`.
    #[new]
    #[pyo3(signature = (text = None))]
    fn new(py: Python<'_>, text: Option<Py<PyAny>>) -> Self {
        Self {
            text: text.unwrap_or_else(|| py.None()),
            trigrams: None,
        }
    }

    fn __traverse__(&self, visit: pyo3::PyVisit<'_>) -> Result<(), pyo3::PyTraverseError> {
        visit.call(&self.text)?;
        visit.call(&self.trigrams)?;
        Ok(())
    }

    fn __clear__(&mut self) {
        self.trigrams = None;
    }

    /// The languages known, by code, in music21's order: its
    /// `languageCodes`.
    #[classattr]
    fn languageCodes() -> Vec<&'static str> {
        language::LANGUAGE_CODES.to_vec()
    }

    /// Each language's name by its code: music21's `languageLong`.
    #[classattr]
    fn languageLong(py: Python<'_>) -> PyResult<Py<PyDict>> {
        let names = PyDict::new(py);
        for code in language::LANGUAGE_CODES {
            names.set_item(code, language::language_name(code))?;
        }
        Ok(names.unbind())
    }

    /// Each language's trigram by its code: music21's `readExcerpts`,
    /// whose excerpts are built into the wheel rather than read off disk.
    #[classmethod]
    fn readExcerpts<'py>(
        _cls: &Bound<'py, pyo3::types::PyType>,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyDict>> {
        excerpt_trigrams(py)
    }

    /// The trigram of each language's excerpt: music21's `trigrams`.
    #[getter]
    fn trigrams(&mut self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        if let Some(held) = &self.trigrams {
            return Ok(held.clone_ref(py));
        }
        let made = excerpt_trigrams(py)?.unbind();
        self.trigrams = Some(made.clone_ref(py));
        Ok(made)
    }

    #[setter]
    fn set_trigrams(&mut self, value: Py<PyDict>) {
        self.trigrams = Some(value);
    }

    /// The code of the language a passage is most like, `None` for no
    /// passage and an empty string for one like none of them: music21's
    /// `mostLikelyLanguage`.
    fn mostLikelyLanguage(&self, excerpt: &Bound<'_, PyAny>) -> PyResult<Option<&'static str>> {
        if !excerpt.is_truthy()? {
            return Ok(None);
        }
        let text: String = excerpt.extract()?;
        Ok(Some(language::most_likely_language(&text).unwrap_or("")))
    }

    /// One more than the place of that language among `languageCodes`, or
    /// nought for no passage: music21's `mostLikelyLanguageNumeric`.
    #[pyo3(signature = (excerpt = None))]
    fn mostLikelyLanguageNumeric(&self, excerpt: Option<&str>) -> PyResult<usize> {
        language::most_likely_language_numeric(excerpt.unwrap_or(""))
            .map_err(|error| TextException::new_err(said(&error)))
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.get_type().call0()?;
        copied.setattr("text", slf.borrow().text.clone_ref(slf.py()))?;
        Ok(copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::__deepcopy__(slf, None)
    }
}

/// A crate text box as music21's own `TextBox`, for a score built into
/// music21's streams: music21's reader makes its own.
pub(crate) fn music21_object<'py>(
    py: Python<'py>,
    text_box: &rs::TextBox,
) -> PyResult<Bound<'py, PyAny>> {
    let made = py
        .import("music21.text")?
        .getattr("TextBox")?
        .call1((text_box.content.as_str(),))?;
    made.setattr("page", text_box.page)?;
    let style = made.getattr("style")?;
    music21_style(py, &style, &text_box.style)?;
    // Whether it says them or not: music21's reader sets these over what
    // the constructor starts with.
    style.setattr("justify", text_box.style.justify())?;
    style.setattr("alignHorizontal", text_box.style.align_horizontal())?;
    style.setattr("alignVertical", text_box.style.align_vertical())?;
    Ok(made)
}

/// Writes what a crate text style says into music21's style object,
/// leaving what it does not say as the object has it.
pub(crate) fn music21_style(
    py: Python<'_>,
    style: &Bound<'_, PyAny>,
    read: &music21::style::TextStyle,
) -> PyResult<()> {
    let value = |said: &Option<music21::style::StyleValue>| -> PyResult<Option<Py<PyAny>>> {
        Ok(match said {
            None => None,
            Some(music21::style::StyleValue::Whole(whole)) => {
                Some(whole.into_pyobject(py)?.into_any().unbind())
            }
            Some(music21::style::StyleValue::Decimal(number)) => {
                Some(number.into_pyobject(py)?.into_any().unbind())
            }
            Some(music21::style::StyleValue::Text(text)) => {
                Some(text.into_pyobject(py)?.into_any().unbind())
            }
        })
    };
    let places = [
        ("absoluteX", &read.absolute_x),
        ("absoluteY", &read.absolute_y),
        ("relativeX", &read.relative_x),
        ("relativeY", &read.relative_y),
        ("fontSize", &read.font_size),
    ];
    for (name, said) in places {
        if let Some(given) = value(said)? {
            style.setattr(name, given)?;
        }
    }
    if let Some(family) = &read.font_family {
        style.setattr("fontFamily", family.clone())?;
    }
    if let Some(font_style) = read.font_style() {
        style.setattr("fontStyle", font_style)?;
    }
    if let Some(weight) = read.font_weight() {
        style.setattr("fontWeight", weight)?;
    }
    if let Some(color) = &read.color {
        style.setattr("color", color)?;
    }
    if let Some(justify) = read.justify() {
        style.setattr("justify", justify)?;
    }
    if let Some(align) = read.align_horizontal() {
        style.setattr("alignHorizontal", align)?;
    }
    if let Some(align) = read.align_vertical() {
        style.setattr("alignVertical", align)?;
    }
    Ok(())
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Trigram>()?;
    m.add_class::<LanguageDetector>()?;
    m.add_class::<TextBox>()?;
    m.add_function(wrap_pyfunction!(assembleLyrics, m)?)?;
    m.add_function(wrap_pyfunction!(assembleAllLyrics, m)?)?;
    m.add_function(wrap_pyfunction!(prependArticle, m)?)?;
    m.add_function(wrap_pyfunction!(postpendArticle, m)?)?;
    Ok(())
}
