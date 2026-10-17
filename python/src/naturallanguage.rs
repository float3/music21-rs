//! music21's `languageExcerpts.naturalLanguageObjects`: a pitch, a note
//! or a chord read from what it is called in another language.
//!
//! music21 builds its dictionary by *writing* every pitch's name in the
//! language and turning the pairs round, and the crate's
//! `language_dictionary` does the same over the same names, so there is
//! one table and not two. A name the language calls no pitch by is a C
//! natural, as music21 answers.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use music21_rs_crate::pitch::{Language, from_language_name, language_dictionary};

use crate::Walkable;
use crate::pitch::Pitch;

/// The names this facade replaces in
/// `music21.languageExcerpts.naturalLanguageObjects`.
pub const NAMES: &[&str] = &["generateLanguageDictionary", "toPitch", "toNote", "toChord"];

/// music21's `generateLanguageDictionary`: every name a pitch has in the
/// language, each against the name this crate spells it by.
///
/// A language music21 does not support is an empty dictionary, as its own
/// is.
#[pyfunction]
fn generateLanguageDictionary<'py>(
    py: Python<'py>,
    languageString: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new(py);
    let Some(language) = Language::from_code(languageString) else {
        return Ok(out);
    };
    for (name, spelled) in language_dictionary(language) {
        out.set_item(name, spelled)?;
    }
    Ok(out)
}

/// The pitch read from a name in another language, or a C natural where
/// the language calls no pitch that: music21's `toPitch`.
#[pyfunction]
fn toPitch(py: Python<'_>, pitchString: &str, languageString: &str) -> PyResult<Py<Pitch>> {
    let read = match Language::from_code(languageString) {
        Some(language) => from_language_name(pitchString, language),
        // An unsupported language calls nothing anything, so every name
        // is a C there too.
        None => from_language_name("", Language::German),
    };
    // The spelling is the language's rather than a caller's, as music21's
    // is: it builds its pitch from the name its own dictionary holds.
    crate::installed_new(py, "music21.pitch", "Pitch", Pitch::wrap(read, false))
}

/// A note of that pitch: music21's `toNote`, which builds its note *on*
/// the pitch it read, so the note carries that very object.
#[pyfunction]
fn toNote<'py>(
    py: Python<'py>,
    pitchString: &str,
    languageString: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let pitch = toPitch(py, pitchString, languageString)?;
    built(py, "music21.note", "Note")?.call1((pitch,))
}

/// A chord of those pitches: music21's `toChord`.
#[pyfunction]
fn toChord<'py>(
    py: Python<'py>,
    pitchArray: &Bound<'py, PyAny>,
    languageString: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let pitches = PyList::empty(py);
    for name in pitchArray.walk()? {
        let name: String = name?.extract()?;
        pitches.append(toPitch(py, &name, languageString)?)?;
    }
    built(py, "music21.chord", "Chord")?.call1((pitches,))
}

/// The class to build with: the one standing in music21's module where
/// anything is installed there, and this package's own otherwise.
fn built<'py>(py: Python<'py>, module: &str, name: &str) -> PyResult<Bound<'py, PyAny>> {
    if let Some(installed) = crate::installed_class(py, module, name) {
        return Ok(installed);
    }
    Ok(match name {
        "Note" => py.get_type::<crate::note::Note>().into_any(),
        _ => py.get_type::<crate::chord::Chord>().into_any(),
    })
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(generateLanguageDictionary, m)?)?;
    m.add_function(wrap_pyfunction!(toPitch, m)?)?;
    m.add_function(wrap_pyfunction!(toNote, m)?)?;
    m.add_function(wrap_pyfunction!(toChord, m)?)?;
    Ok(())
}
