//! music21's `figuredBass.checker`: a written-out score broken into the
//! moments where any part moves, and each moment held to the voice-leading
//! rules, over the crate's `figuredbass::checker`.
//!
//! The rules are the crate's: which parts cross, and which pairs move at
//! a fifth or an octave and break the rule for it. A quartet once decided
//! is kept in `figuredBass.possibility`'s tables, keyed by its four pitch
//! objects, as music21 keeps it, so the two modules share what they
//! decided.
//!
//! **The score functions walk music21's streams through their own
//! methods.** What they do -- read each part's notes into spans of time,
//! split the spans where another part moves, colour the notes breaking a
//! rule, write a new score of the moments -- is a matter of asking a
//! stream for its notes and its notes for their offsets, which the
//! stream classes answer; there is no arithmetic of their own to port.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

use music21::Pitch as RsPitch;
use music21::figuredbass::checker::{self as rs, ConsecutiveRule};

use crate::pitch::pitch_from_any;

/// A class of music21's `stream`, as the module answering for it has it.
fn stream_class<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    py.import("music21.stream")?.getattr(name)
}

/// music21's `opFrac`.
fn op_frac<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    crate::common::module(value.py(), "music21.common.numberTools")?
        .getattr("opFrac")?
        .call1((value,))
}

/// The music21 exception everything here raises.
fn checker_error(py: Python<'_>, message: &str) -> PyErr {
    PyErr::from_type(
        py.get_type::<crate::Music21Exception>(),
        message.to_string(),
    )
}

/// A possibility's pitches, read by the crate.
fn pitches(possibility: &Bound<'_, PyAny>) -> PyResult<Vec<RsPitch>> {
    possibility
        .try_iter()?
        .map(|pitch| pitch_from_any(&pitch?))
        .collect()
}

/// Part pairs as music21's list of tuples.
fn pairs(py: Python<'_>, found: Vec<(usize, usize)>) -> PyResult<Bound<'_, PyList>> {
    PyList::new(py, found)
}

/// Whether a pitch is the placeholder a resting part is given.
#[pyfunction]
fn _isRest(p: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(rs::is_rest(&pitch_from_any(p)?))
}

/// Each note and rest of the part, keyed by where it starts and ends.
#[pyfunction]
fn createOffsetMapping<'py>(music21Part: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = music21Part.py();
    let mapping = py
        .import("collections")?
        .getattr("defaultdict")?
        .call1((py.get_type::<PyList>(),))?;
    let notes = music21Part
        .call_method0("flatten")?
        .getattr("notesAndRests")?;
    for general_note in notes.try_iter()? {
        let general_note = general_note?;
        let start = general_note.getattr("offset")?;
        let end = op_frac(&start.add(general_note.getattr("quarterLength")?)?)?;
        mapping
            .get_item((start, end))?
            .call_method1("append", (general_note,))?;
    }
    Ok(mapping)
}

/// The mapping split wherever a note of the part starts or ends inside one
/// of its spans, each span holding the notes so far and the part's.
#[pyfunction]
fn correlateHarmonies<'py>(
    currentMapping: &Bound<'py, PyAny>,
    music21Part: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py = music21Part.py();
    let mapping = PyDict::new(py);
    let notes = music21Part
        .call_method0("flatten")?
        .getattr("notesAndRests")?;
    let keywords = PyDict::new(py);
    keywords.set_item("includeEndBoundary", false)?;
    keywords.set_item("mustFinishInSpan", false)?;
    keywords.set_item("mustBeginInSpan", false)?;
    keywords.set_item("includeElementsThatEndAtStart", false)?;
    let spans = py
        .import("builtins")?
        .getattr("sorted")?
        .call1((currentMapping.call_method0("keys")?,))?;
    for span in spans.try_iter()? {
        let span = span?;
        let (start, end) = (span.get_item(0)?, span.get_item(1)?);
        keywords.set_item("offsetEnd", &end)?;
        let in_range = notes.call_method("getElementsByOffset", (&start,), Some(&keywords))?;
        let so_far = currentMapping.get_item(&span)?;
        for general_note in in_range.try_iter()? {
            let general_note = general_note?;
            let offset = general_note.getattr("offset")?;
            let finish = offset.add(general_note.getattr("quarterLength")?)?;
            let new_start = if offset.lt(&start)? {
                start.clone()
            } else {
                offset
            };
            let new_end = if finish.gt(&end)? {
                end.clone()
            } else {
                op_frac(&finish)?
            };
            let held = PyList::new(py, so_far.try_iter()?.collect::<PyResult<Vec<_>>>()?)?;
            held.append(general_note)?;
            mapping.set_item((new_start, new_end), held)?;
        }
    }
    Ok(mapping)
}

/// The score's moments, keyed by where each starts and ends, each the
/// notes sounding then from the highest part down.
#[pyfunction]
fn extractHarmonies<'py>(music21Stream: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = music21Stream.py();
    let parts: Vec<Bound<'py, PyAny>> = music21Stream
        .call_method1("getElementsByClass", ("Part",))?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    if parts.len() < 2 {
        return Err(checker_error(
            py,
            "There must be at least two parts to extract harmonies",
        ));
    }
    let mut mapping = createOffsetMapping(&parts[0])?;
    for part in &parts[1..] {
        mapping = correlateHarmonies(&mapping, part)?.into_any();
    }
    Ok(mapping)
}

/// The moments, sorted by when they start.
fn sorted_harmonies<'py>(music21Stream: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let items = extractHarmonies(music21Stream)?.call_method0("items")?;
    music21Stream
        .py()
        .import("builtins")?
        .getattr("sorted")?
        .call1((items,))?
        .try_iter()?
        .collect()
}

/// The score as a new score of its moments, each note lasting the whole of
/// its moment.
#[pyfunction]
fn getVoiceLeadingMoments<'py>(music21Stream: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = music21Stream.py();
    let harmonies = sorted_harmonies(music21Stream)?;
    let parts = music21Stream
        .call_method1("getElementsByClass", ("Part",))?
        .call_method0("stream")?;
    let mut new_parts = Vec::new();
    for part in parts.try_iter()? {
        new_parts.push(
            part?
                .call_method0("flatten")?
                .call_method1("getElementsNotOfClass", ("GeneralNote",))?
                .call_method0("stream")?,
        );
    }
    let first_measure = parts
        .get_item(0)?
        .call_method1("getElementsByClass", ("Measure",))?
        .call_method0("first")?;
    let padding = first_measure.getattr("paddingLeft")?;
    let deep_copy = py.import("copy")?.getattr("deepcopy")?;
    for harmony in harmonies {
        let span = harmony.get_item(0)?;
        let (start, end) = (span.get_item(0)?, span.get_item(1)?);
        for (index, general_note) in harmony.get_item(1)?.try_iter()?.enumerate() {
            let copied = deep_copy.call1((general_note?,))?;
            copied.setattr("quarterLength", end.sub(&start)?)?;
            copied.setattr("tie", py.None())?;
            new_parts[index].call_method1("insert", (start.add(&padding)?, copied))?;
        }
    }
    let keywords = PyDict::new(py);
    keywords.set_item("inPlace", true)?;
    keywords.set_item("cautionaryNotImmediateRepeat", false)?;
    let padded = !padding.eq(0.0)?;
    for part in &new_parts {
        part.call_method("makeNotation", (), Some(&keywords))?;
        if padded {
            part.get_item(0)?.call_method0("padAsAnacrusis")?;
            for measure in part.try_iter()? {
                let measure = measure?;
                let number = measure.getattr("number")?.sub(1)?;
                measure.setattr("number", number)?;
            }
        }
    }
    stream_class(py, "Score")?.call1((PyList::new(py, new_parts)?,))
}

/// The pitch a note sounds, or the resting placeholder for anything else.
#[pyfunction]
fn generalNoteToPitch<'py>(music21GeneralNote: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = music21GeneralNote.py();
    let note_class = py.import("music21.note")?.getattr("Note")?;
    if music21GeneralNote.is_instance(&note_class)? {
        return music21GeneralNote.getattr("pitch");
    }
    let placeholder = py.import("music21.pitch")?.getattr("Pitch")?.call0()?;
    placeholder.setattr("ps", rs::REST_PS)?;
    Ok(placeholder)
}

/// A moment's notes as the possibility the rules read.
fn possibility_of<'py>(notes: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    let mut held = Vec::new();
    for general_note in notes.try_iter()? {
        held.push(generalNoteToPitch(&general_note?)?);
    }
    PyTuple::new(notes.py(), held)
}

/// A line of the report, padded as music21's `f'{value!s:25}'` pads it.
fn padded(value: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(format!("{:<25}", value.str()?.to_string()))
}

/// The report, printed, saying so where there was nothing to report.
fn print_report(py: Python<'_>, mut lines: Vec<String>) -> PyResult<()> {
    if lines.len() == 2 {
        lines.push("No violations to report.".to_string());
    }
    let print = py.import("builtins")?.getattr("print")?;
    for line in lines {
        print.call1((line,))?;
    }
    Ok(())
}

/// The notes of each part sounding at an offset.
fn sounding_at<'py>(
    part: &Bound<'py, PyAny>,
    offset: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let keywords = PyDict::new(part.py());
    keywords.set_item("mustBeginInSpan", false)?;
    part.call_method("getElementsByOffset", (offset, offset), Some(&keywords))
}

/// The score's parts, each flattened.
fn flat_parts<'py>(music21Stream: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let mut parts = Vec::new();
    for part in music21Stream
        .call_method1("getElementsByClass", ("Part",))?
        .try_iter()?
    {
        parts.push(part?.call_method0("flatten")?);
    }
    Ok(parts)
}

/// Colours the notes of each moment breaking a rule over one possibility,
/// and with `debug` prints where they are.
#[pyfunction]
#[pyo3(signature = (music21Stream, functionToApply, color = Some("#FF0000".to_string()), debug = false))]
fn checkSinglePossibilities(
    music21Stream: &Bound<'_, PyAny>,
    functionToApply: &Bound<'_, PyAny>,
    color: Option<String>,
    #[pyo3(from_py_with = crate::truthy)] debug: bool,
) -> PyResult<()> {
    let py = music21Stream.py();
    let mut report = Vec::new();
    if debug {
        let name: String = functionToApply.getattr("__name__")?.extract()?;
        report.push(format!("Function To Apply: {name}"));
        report.push(format!("{:<25}Part Numbers:", "(Offset, End Time):"));
    }
    let parts = flat_parts(music21Stream)?;
    for harmony in sorted_harmonies(music21Stream)? {
        let span = harmony.get_item(0)?;
        let found = functionToApply.call1((possibility_of(&harmony.get_item(1)?)?,))?;
        let start = span.get_item(0)?;
        for pair in found.try_iter()? {
            let pair = pair?;
            for part_number in pair.try_iter()? {
                let Some(color) = &color else {
                    continue;
                };
                let index = part_number?.extract::<usize>()? - 1;
                let noted = sounding_at(&parts[index], &start)?.get_item(0)?;
                noted.getattr("style")?.setattr("color", color)?;
            }
            if debug {
                report.push(format!("{}{}", padded(&span)?, pair.str()?));
            }
        }
    }
    if debug {
        print_report(py, report)?;
    }
    Ok(())
}

/// Colours the notes of each two moments in a row breaking a rule over
/// two possibilities, and with `debug` prints where they are.
#[pyfunction]
#[pyo3(signature = (music21Stream, functionToApply, color = Some("#FF0000".to_string()), debug = false))]
fn checkConsecutivePossibilities(
    music21Stream: &Bound<'_, PyAny>,
    functionToApply: &Bound<'_, PyAny>,
    color: Option<String>,
    #[pyo3(from_py_with = crate::truthy)] debug: bool,
) -> PyResult<()> {
    let py = music21Stream.py();
    let mut report = Vec::new();
    if debug {
        let name: String = functionToApply.getattr("__name__")?.extract()?;
        report.push(format!("Function To Apply: {name}"));
        report.push("(Offset A, End Time A):  (Offset B, End Time B): Part Numbers:".to_string());
    }
    let parts = flat_parts(music21Stream)?;
    let harmonies = sorted_harmonies(music21Stream)?;
    let Some(first) = harmonies.first() else {
        return Err(pyo3::exceptions::PyIndexError::new_err(
            "list index out of range",
        ));
    };
    let mut previous_span = first.get_item(0)?;
    let mut previous = possibility_of(&first.get_item(1)?)?;
    for harmony in &harmonies[1..] {
        let span = harmony.get_item(0)?;
        let current = possibility_of(&harmony.get_item(1)?)?;
        let found = functionToApply.call1((&previous, &current))?;
        let (start_a, start_b) = (previous_span.get_item(0)?, span.get_item(0)?);
        for pair in found.try_iter()? {
            let pair = pair?;
            for part_number in pair.try_iter()? {
                let Some(color) = &color else {
                    continue;
                };
                let part = &parts[part_number?.extract::<usize>()? - 1];
                let note_a = sounding_at(part, &start_a)?.call_method0("first")?;
                let note_b = sounding_at(part, &start_b)?.call_method0("first")?;
                if note_a.is_none() || note_b.is_none() {
                    return Err(checker_error(
                        py,
                        "Expected notes to color at the violation offset, but found none",
                    ));
                }
                note_a.getattr("style")?.setattr("color", color)?;
                note_b.getattr("style")?.setattr("color", color)?;
            }
            if debug {
                report.push(format!(
                    "{}{}{}",
                    padded(&previous_span)?,
                    padded(&span)?,
                    pair.str()?
                ));
            }
        }
        previous_span = span;
        previous = current;
    }
    if debug {
        print_report(py, report)?;
    }
    Ok(())
}

/// Every pair of parts where a higher part sings below a lower one.
#[pyfunction]
fn voiceCrossing<'py>(possibA: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
    pairs(possibA.py(), rs::voice_crossings(&pitches(possibA)?))
}

/// The table in `figuredBass.possibility` a rule keeps its quartets in.
fn table_of(py: Python<'_>, rule: ConsecutiveRule) -> PyResult<Bound<'_, PyAny>> {
    let name = match rule {
        ConsecutiveRule::ParallelFifths => "parallelFifthsTable",
        ConsecutiveRule::ParallelOctaves => "parallelOctavesTable",
        ConsecutiveRule::HiddenFifths => "hiddenFifthsTable",
        ConsecutiveRule::HiddenOctaves => "hiddenOctavesTable",
    };
    crate::possibility::module(py)?.getattr(name)
}

/// Every pair of parts breaking the rule from one possibility to the next,
/// each quartet looked up in the shared table before it is decided.
fn broken<'py>(
    possibA: &Bound<'py, PyAny>,
    possibB: &Bound<'py, PyAny>,
    rule: ConsecutiveRule,
) -> PyResult<Bound<'py, PyList>> {
    let py = possibA.py();
    let (a, b) = (pitches(possibA)?, pitches(possibB)?);
    if a.len() != b.len() {
        return Err(pyo3::exceptions::PyValueError::new_err(format!(
            "zip() argument 2 is {} than argument 1",
            if b.len() < a.len() {
                "shorter"
            } else {
                "longer"
            }
        )));
    }
    let table = table_of(py, rule)?;
    let mut found = Vec::new();
    let candidates = rs::candidates(&a, &b, rule)
        .map_err(|error| checker_error(py, &crate::pitch::message(&error)))?;
    for (higher, lower) in candidates {
        let (high, low) = (higher - 1, lower - 1);
        let quartet = PyTuple::new(
            py,
            [
                possibA.get_item(low)?,
                possibB.get_item(low)?,
                possibA.get_item(high)?,
                possibB.get_item(high)?,
            ],
        )?;
        let breaks = match table.get_item(&quartet) {
            Ok(known) => known.is_truthy()?,
            Err(_) => {
                let decided = rs::breaks_rule((&a[low], &b[low]), (&a[high], &b[high]), rule)
                    .map_err(|error| checker_error(py, &crate::pitch::message(&error)))?;
                table.set_item(&quartet, decided)?;
                decided
            }
        };
        if breaks {
            found.push((higher, lower));
        }
    }
    pairs(py, found)
}

/// Every pair of parts moving in parallel fifths.
#[pyfunction]
fn parallelFifths<'py>(
    possibA: &Bound<'py, PyAny>,
    possibB: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    broken(possibA, possibB, ConsecutiveRule::ParallelFifths)
}

/// The outer parts, where they move into a fifth by similar motion.
#[pyfunction]
fn hiddenFifths<'py>(
    possibA: &Bound<'py, PyAny>,
    possibB: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    broken(possibA, possibB, ConsecutiveRule::HiddenFifths)
}

/// Every pair of parts moving in parallel octaves.
#[pyfunction]
fn parallelOctaves<'py>(
    possibA: &Bound<'py, PyAny>,
    possibB: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    broken(possibA, possibB, ConsecutiveRule::ParallelOctaves)
}

/// The outer parts, where they move into an octave by similar motion.
#[pyfunction]
fn hiddenOctaves<'py>(
    possibA: &Bound<'py, PyAny>,
    possibB: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    broken(possibA, possibB, ConsecutiveRule::HiddenOctaves)
}

static BUILT: pyo3::sync::PyOnceLock<Py<PyModule>> = pyo3::sync::PyOnceLock::new();

/// music21's `figuredBass.checker`, built once.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, "music21.figuredBass.checker")?;
        m.add_function(wrap_pyfunction!(_isRest, &m)?)?;
        m.add_function(wrap_pyfunction!(getVoiceLeadingMoments, &m)?)?;
        m.add_function(wrap_pyfunction!(extractHarmonies, &m)?)?;
        m.add_function(wrap_pyfunction!(createOffsetMapping, &m)?)?;
        m.add_function(wrap_pyfunction!(correlateHarmonies, &m)?)?;
        m.add_function(wrap_pyfunction!(checkSinglePossibilities, &m)?)?;
        m.add_function(wrap_pyfunction!(checkConsecutivePossibilities, &m)?)?;
        m.add_function(wrap_pyfunction!(voiceCrossing, &m)?)?;
        m.add_function(wrap_pyfunction!(parallelFifths, &m)?)?;
        m.add_function(wrap_pyfunction!(hiddenFifths, &m)?)?;
        m.add_function(wrap_pyfunction!(parallelOctaves, &m)?)?;
        m.add_function(wrap_pyfunction!(hiddenOctaves, &m)?)?;
        m.add_function(wrap_pyfunction!(generalNoteToPitch, &m)?)?;
        m.add("possibility", crate::possibility::module(py)?)?;
        m.add("Music21Exception", py.get_type::<crate::Music21Exception>())?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}
