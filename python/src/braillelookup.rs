//! music21's `braille.lookup`: the signs braille is written with.
//!
//! The tables themselves stay music21's -- its own code looks them up by
//! name, and the crate's `braille::lookup` answers the same questions as
//! functions rather than as dictionaries, which is what every transcription
//! here asks. What is replaced is the two functions that *make* a sign out
//! of its dots, which is arithmetic over the braille block and is the
//! crate's: `lookup::cell` and `lookup::dots_add`.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyDict;

use music21::braille::lookup;

use crate::Walkable;

/// The names this module hands out.
pub const NAMES: &[&str] = &["makeBrailleDictionary", "dotsAdd"];

/// The dots a cell holds, written as music21 writes them: each dot in
/// turn, so dots one, two and four are 124, and nought for the blank cell.
fn dots_of(bits: u32) -> u32 {
    (1..=6).fold(0, |dots, dot| {
        if bits & (1 << (dot - 1)) == 0 {
            dots
        } else {
            dots * 10 + dot
        }
    })
}

/// music21's `makeBrailleDictionary`: every one of the sixty-four cells,
/// each against the dots it holds.
#[pyfunction]
fn makeBrailleDictionary<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new(py);
    for bits in 0u32..64 {
        let dots = dots_of(bits);
        out.set_item(dots, lookup::cell(dots).to_string())?;
    }
    Ok(out)
}

/// music21's `dotsAdd`: the cell holding every dot of these numbers,
/// whatever order they are given in.
#[pyfunction]
fn dotsAdd(dotIter: &Bound<'_, PyAny>) -> PyResult<String> {
    let mut dots = Vec::new();
    for number in dotIter.walk()? {
        dots.push(number?.extract::<u32>()?);
    }
    Ok(lookup::dots_add(&dots).to_string())
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(makeBrailleDictionary, m)?)?;
    m.add_function(wrap_pyfunction!(dotsAdd, m)?)?;
    Ok(())
}

static BUILT: pyo3::sync::PyOnceLock<Py<PyModule>> = pyo3::sync::PyOnceLock::new();

/// The letters of the notes, in music21's order.
const STEPS: [char; 7] = ['C', 'D', 'E', 'F', 'G', 'A', 'B'];

/// The note values a note's sign is had for, in music21's order.
const NOTE_VALUES: [&str; 10] = [
    "128th", "64th", "32nd", "16th", "eighth", "quarter", "half", "whole", "breve", "longa",
];

/// The accidentals music21 has signs for, in its order.
const ACCIDENTALS: [&str; 13] = [
    "sharp",
    "double-sharp",
    "triple-sharp",
    "quadruple-sharp",
    "half-sharp",
    "one-and-a-half-sharp",
    "flat",
    "double-flat",
    "triple-flat",
    "quadruple-flat",
    "half-flat",
    "one-and-a-half-flat",
    "natural",
];

const RESTS: [&str; 11] = [
    "dummy", "128th", "64th", "32nd", "16th", "eighth", "quarter", "half", "whole", "breve",
    "longa",
];

const LENGTH_PREFIXES: [&str; 3] = ["larger", "smaller", "xsmall"];

const BARLINES: [&str; 4] = ["final", "double", "dashed", "heavy"];

const FINGERS: [&str; 5] = ["1", "2", "3", "4", "5"];

const BOWINGS: [&str; 2] = ["down bow", "up bow"];

const BEFORE_NOTE: [&str; 6] = [
    "staccato",
    "accent",
    "tenuto",
    "staccatissimo",
    "strong accent",
    "detached legato",
];

const TEXT_EXPRESSIONS: [&str; 6] = [
    "crescendo",
    "cresc.",
    "cr.",
    "decrescendo",
    "decresc.",
    "decr.",
];

/// The characters music21's `alphabet` spells, in its order.
const ALPHABET: &str = "abcdefghijklmnopqrstuvwxyz !',-./:?()^[]*0123456789";

const CHORD_SYMBOLS: [&str; 9] = [
    "plus",
    "minus",
    "diminished_circle",
    "half_diminished_circle",
    "triangle",
    "triangle_line",
    "italics_seven",
    "slash",
    "parentheses",
];

const SYMBOLS: [&str; 31] = [
    "space",
    "double_space",
    "number",
    "letter_sign",
    "dot",
    "tie",
    "uppercase",
    "metronome",
    "common",
    "cut",
    "music_hyphen",
    "transcriber-added_sign",
    "music_asterisk",
    "rh_keyboard",
    "lh_keyboard",
    "word",
    "triplet",
    "tuplet_prefix",
    "finger_change",
    "first_set_missing_fingermark",
    "second_set_missing_fingermark",
    "opening_single_slur",
    "opening_double_slur",
    "closing_double_slur",
    "opening_bracket_slur",
    "closing_bracket_slur",
    "basic_exception",
    "full_inaccord",
    "repeat",
    "print-pagination",
    "braille-music-parenthesis",
];

/// A table music21 keeps as a dictionary, made by asking the crate for
/// the sign of each of its keys in turn.
fn table<'py, K, V>(
    py: Python<'py>,
    keys: impl IntoIterator<Item = K>,
    sign: impl Fn(&K) -> Option<V>,
) -> PyResult<Bound<'py, PyDict>>
where
    K: IntoPyObject<'py> + Clone,
    V: IntoPyObject<'py>,
{
    let out = PyDict::new(py);
    for key in keys {
        if let Some(value) = sign(&key) {
            out.set_item(key, value)?;
        }
    }
    Ok(out)
}

/// music21's `makePitchNameToNotes`: each letter's signs, by note value.
#[pyfunction]
fn makePitchNameToNotes(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let out = PyDict::new(py);
    for step in STEPS {
        let notes = table(py, NOTE_VALUES, |value| {
            lookup::pitch_name_to_note(step, value)
        })?;
        out.set_item(step.to_string(), notes)?;
    }
    Ok(out)
}

/// music21's `clefs`: the prefix, each sign's clefs by line, and the
/// suffix, by whether the hands have changed.
fn clefs(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let out = PyDict::new(py);
    out.set_item("prefix", lookup::clef_prefix().to_string())?;
    for sign in ["G", "C", "F"] {
        out.set_item(sign, table(py, 1..=5, |line| lookup::clef(sign, *line))?)?;
    }
    let suffix = PyDict::new(py);
    for switched in [false, true] {
        suffix.set_item(switched, lookup::clef_suffix(switched).to_string())?;
    }
    out.set_item("suffix", suffix)?;
    Ok(out)
}

/// music21's `fermatas`: by shape, and on a barline or between notes.
fn fermatas(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let out = PyDict::new(py);
    out.set_item(
        "shape",
        table(py, ["normal", "angled", "square"], |shape| {
            lookup::fermata(shape)
        })?,
    )?;
    out.set_item("on barline", lookup::cell(456).to_string())?;
    out.set_item("between notes", lookup::cell(5).to_string())?;
    Ok(out)
}

/// Every cell, in the order music21's `makeBrailleDictionary` makes them.
fn every_cell() -> impl Iterator<Item = char> {
    (0u32..64).map(|bits| lookup::cell(dots_of(bits)))
}

/// A count of signs, `''` for none, as music21's key signatures and
/// naturals have it for nought.
fn counted(count: i64, sign: impl Fn(i64) -> Option<String>) -> Option<String> {
    if count == 0 {
        return Some(String::new());
    }
    sign(count)
}

/// music21's `braille.lookup`, built: its functions and every one of its
/// tables, each sign asked of the crate.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, "music21.braille.lookup")?;
        register(&m)?;
        m.add_function(wrap_pyfunction!(makePitchNameToNotes, &m)?)?;
        let cells = makeBrailleDictionary(py)?;
        m.add("_B", &cells)?;
        m.add("brailleDotDict", &cells)?;
        m.add("pitchNameToNotes", makePitchNameToNotes(py)?)?;
        m.add(
            "octaves",
            table(py, 0..=8, |octave| lookup::octave(*octave))?,
        )?;
        m.add(
            "accidentals",
            table(py, ACCIDENTALS, |name| lookup::accidental(name))?,
        )?;
        m.add(
            "intervals",
            table(py, 2..=8, |size| lookup::interval(*size).map(String::from))?,
        )?;
        m.add(
            "keySignatures",
            table(py, -7..=7, |sharps| counted(*sharps, lookup::key_signature))?,
        )?;
        m.add(
            "naturals",
            table(py, 0..=7, |count| counted(*count, lookup::naturals))?,
        )?;
        m.add(
            "numbersUpper",
            table(py, 0..=9, |digit| {
                lookup::number_upper(*digit).map(String::from)
            })?,
        )?;
        m.add(
            "numbersLower",
            table(py, 0..=9, |digit| {
                lookup::number_lower(*digit).map(String::from)
            })?,
        )?;
        m.add("rests", table(py, RESTS, |value| lookup::rest(value))?)?;
        m.add(
            "lengthPrefixes",
            table(py, LENGTH_PREFIXES, |size| lookup::length_prefix(size))?,
        )?;
        m.add(
            "barlines",
            table(py, BARLINES, |kind| lookup::barline(kind))?,
        )?;
        m.add(
            "fingerMarks",
            table(py, FINGERS, |finger| {
                lookup::finger_mark(finger).map(String::from)
            })?,
        )?;
        m.add("clefs", clefs(py)?)?;
        m.add(
            "bowingSymbols",
            table(py, BOWINGS, |name| lookup::bowing(name))?,
        )?;
        m.add(
            "beforeNoteExpr",
            table(py, BEFORE_NOTE, |name| lookup::before_note_expression(name))?,
        )?;
        m.add(
            "textExpressions",
            table(py, TEXT_EXPRESSIONS, |words| lookup::text_expression(words))?,
        )?;
        m.add(
            "alphabet",
            table(py, ALPHABET.chars().map(String::from), |character| {
                character.chars().next().and_then(lookup::alphabet)
            })?,
        )?;
        m.add(
            "chordSymbols",
            table(py, CHORD_SYMBOLS, |name| lookup::chord_symbol(name))?,
        )?;
        m.add(
            "symbols",
            table(py, SYMBOLS, |name| Some(lookup::symbol(name)))?,
        )?;
        m.add("fermatas", fermatas(py)?)?;
        m.add(
            "ascii_chars",
            table(py, every_cell().map(String::from), |cell| {
                cell.chars()
                    .next()
                    .and_then(lookup::ascii_char)
                    .map(String::from)
            })?,
        )?;
        m.add(
            "binary_dots",
            table(py, every_cell().map(String::from), |cell| {
                let dots = cell.chars().next().and_then(lookup::binary_dots)?;
                Some((dots[0], dots[1], dots[2]))
            })?,
        )?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}
