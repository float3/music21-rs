//! music21's `noteworthy` package: `constants` and `dictionaries` from
//! the crate's tables, `translate` (`nwctranslate`) and `binaryTranslate`
//! (`nwcbinary`).

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyList, PyModule};

use music21::noteworthy::{
    ALTERATION_TEXTS, BAR_STYLES, CLEF_NAMES, DURATION_VALUES, FLAT_MASK, MIDI_INSTRUMENTS,
    OCTAVE_SHIFT_NAMES, SHARP_MASK, staff_positions,
};

/// The package and its modules, as music21 names them.
const PACKAGE: &str = "music21.noteworthy";
const CONSTANTS: &str = "music21.noteworthy.constants";
const DICTIONARIES: &str = "music21.noteworthy.dictionaries";

/// The length of each NoteWorthy duration name in quarter notes.
const NOTE_LENGTHS: [f64; 7] = [4.0, 2.0, 1.0, 0.5, 0.25, 0.125, 0.0625];

/// The order sharps and flats are added to a key signature, by count.
const SHARP_ORDER: [&str; 7] = ["F", "C", "G", "D", "A", "E", "B"];
const FLAT_ORDER: [&str; 7] = ["B", "E", "A", "D", "G", "C", "F"];

/// The staves `dictionaries` lays out, by music21's name and clef.
const STAVES: [(&str, &str); 4] = [
    ("dictionaryTreble", "TREBLE"),
    ("dictionaryBass", "BASS"),
    ("dictionaryAlto", "ALTO"),
    ("dictionaryTenor", "TENOR"),
];

static BUILT: PyOnceLock<Py<PyDict>> = PyOnceLock::new();

/// A staff's positions as music21's dictionary of them: each position its
/// step, and `octave` the octave of the lowest.
fn staff_dictionary<'py>(py: Python<'py>, clef: &str) -> PyResult<Bound<'py, PyDict>> {
    let (octave, lowest, steps) = staff_positions(clef);
    let made = PyDict::new(py);
    for (position, step) in (lowest..).zip(steps) {
        made.set_item(position, step.to_string())?;
    }
    made.set_item("octave", octave)?;
    Ok(made)
}

/// A key signature's accidentals by count, from one.
fn order_dictionary<'py>(py: Python<'py>, order: &[&str]) -> PyResult<Bound<'py, PyDict>> {
    let made = PyDict::new(py);
    for (count, step) in (1..).zip(order) {
        made.set_item(count, *step)?;
    }
    Ok(made)
}

/// music21's `noteworthy.dictionaries`.
fn dictionaries(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let m = PyModule::new(py, DICTIONARIES)?;
    let lengths = PyDict::new(py);
    for (name, length) in DURATION_VALUES.iter().zip(NOTE_LENGTHS) {
        lengths.set_item(name, length)?;
    }
    lengths.set_item(0, 0)?;
    let all = PyDict::new(py);
    all.set_item("dictionaryNoteLength", &lengths)?;
    m.add("dictionaryNoteLength", lengths)?;
    for (name, clef) in STAVES {
        let staff = staff_dictionary(py, clef)?;
        all.set_item(name, &staff)?;
        m.add(name, staff)?;
    }
    for (name, order) in [
        ("dictionarySharp", SHARP_ORDER),
        ("dictionaryFlat", FLAT_ORDER),
    ] {
        let made = order_dictionary(py, &order)?;
        all.set_item(name, &made)?;
        m.add(name, made)?;
    }
    m.add("dictionaries", all)?;
    Ok(m)
}

/// music21's `noteworthy.constants`, the crate's tables.
fn constants(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let m = PyModule::new(py, CONSTANTS)?;
    m.add("MidiInstruments", PyList::new(py, MIDI_INSTRUMENTS)?)?;
    m.add("ClefNames", PyList::new(py, CLEF_NAMES)?)?;
    m.add("OctaveShiftNames", PyList::new(py, OCTAVE_SHIFT_NAMES)?)?;
    for (name, table) in [("FlatMask", FLAT_MASK), ("SharpMask", SHARP_MASK)] {
        let made = PyDict::new(py);
        for (mask, text) in table {
            made.set_item(mask, text)?;
        }
        m.add(name, made)?;
    }
    m.add("AlterationTexts", PyList::new(py, ALTERATION_TEXTS)?)?;
    m.add("BarStyles", PyList::new(py, BAR_STYLES)?)?;
    m.add("DurationValues", PyList::new(py, DURATION_VALUES)?)?;
    Ok(m)
}

/// One of the package's modules, built with the others on first asking.
pub(crate) fn module<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyModule>> {
    let built = BUILT.get_or_try_init(py, || {
        let made = PyDict::new(py);
        let package = PyModule::new(py, PACKAGE)?;
        let parts = [
            ("binaryTranslate", crate::nwcbinary::module(py)?),
            ("constants", constants(py)?),
            ("dictionaries", dictionaries(py)?),
            ("translate", crate::nwctranslate::module(py)?),
        ];
        let names = PyList::empty(py);
        for (short, part) in parts {
            names.append(short)?;
            package.add(short, &part)?;
            made.set_item(part.name()?, part)?;
        }
        package.add("__all__", names)?;
        made.set_item(PACKAGE, package)?;
        Ok::<_, PyErr>(made.unbind())
    })?;
    let found = built
        .bind(py)
        .get_item(name)?
        .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err(name.to_owned()))?;
    Ok(found.cast_into::<PyModule>()?)
}
