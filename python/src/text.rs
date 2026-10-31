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
//! `LanguageDetector` and `Trigram` are not here: guessing which language
//! a line of text is in is a table of letter triplets rather than
//! anything musical, and the crate has none.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyDict;

use music21_rs_crate::text as rs;

pyo3::create_exception!(music21_rs_facade, TextException, crate::Music21Exception);

/// The names this module hands out.
pub const NAMES: &[&str] = &[
    "assembleLyrics",
    "assembleAllLyrics",
    "prependArticle",
    "postpendArticle",
    "TextBox",
    "TextException",
];

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
fn said(error: &music21_rs_crate::Error) -> String {
    crate::pitch::message(error)
}

/// music21's `text.TextBox`: words standing anywhere on a page, which a
/// score's title and its credits are.
#[pyclass(
    name = "TextBox",
    module = "music21.text",
    subclass,
    skip_from_py_object
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
    /// Refuses what music21 answers through a site, which the package
    /// keeps none of.
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
    ) -> PyResult<Self> {
        let given = |value: Option<&Bound<'_, PyAny>>| -> PyResult<Py<PyAny>> {
            match value.filter(|value| !value.is_none()) {
                Some(value) => Ok(value.clone().unbind()),
                // music21's own default, five hundred tenths each way.
                None => Ok(500i32.into_pyobject(py)?.into_any().unbind()),
            }
        };
        Ok(Self {
            content,
            page: 1,
            style: None,
            placed: (given(x)?, given(y)?),
        })
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
        Ok(copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::__deepcopy__(slf, None)
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<TextBox>()?;
    m.add_function(wrap_pyfunction!(assembleLyrics, m)?)?;
    m.add_function(wrap_pyfunction!(assembleAllLyrics, m)?)?;
    m.add_function(wrap_pyfunction!(prependArticle, m)?)?;
    m.add_function(wrap_pyfunction!(postpendArticle, m)?)?;
    Ok(())
}
