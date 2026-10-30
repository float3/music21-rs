//! music21's `braille.noteGrouping`: a grouping of elements written in
//! braille, one element at a time.
//!
//! The transcribing is the crate's
//! `braille::segment::NoteGroupingTranscriber`, which every score written
//! in braille goes through and `braille_parity` holds to music21. music21
//! writes a grouping element by element and keeps what it has written as
//! it goes -- `trans`, `previousElement` -- and its own examples ask for
//! one element at a time, so the facade keeps the crate's transcriber
//! beside a grouping of the elements given so far and asks it for each.
//!
//! The `translate*` steps stay music21's. They are the branches of its own
//! `transcribeOneElement`, which the crate takes whole, and nothing but
//! that method calls them.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyList;

use music21_rs_crate::braille::segment::{
    BrailleElementGrouping as RsGrouping, NoteGroupingTranscriber as RsTranscriber,
};

use crate::braillesegment::{crate_element, crate_grouping, write_english};

/// The names this module hands out.
pub const NAMES: &[&str] = &["NoteGroupingTranscriber"];

/// music21's `braille.noteGrouping.NoteGroupingTranscriber`.
#[pyclass(
    name = "NoteGroupingTranscriber",
    module = "music21.braille.noteGrouping",
    subclass,
    skip_from_py_object
)]
pub struct NoteGroupingTranscriber {
    /// Whether the first note of the grouping shows its octave.
    #[pyo3(get, set)]
    showLeadingOctave: bool,
    /// The grouping being written, which a caller may set.
    #[pyo3(get, set)]
    brailleElementGrouping: Py<PyAny>,
    /// What a caller said of its own, which wins over the grouping.
    shown_clef_signs: Option<bool>,
    upper_first: Option<bool>,
    /// The elements written so far, as the objects and as the crate's
    /// values: music21 keeps what it has written and what it wrote last,
    /// and the crate's transcriber reads the one before each element too.
    written: Vec<Py<PyAny>>,
    grouping: RsGrouping,
    transcriber: RsTranscriber,
}

impl NoteGroupingTranscriber {
    /// The grouping the facade writes into, started from whatever the
    /// grouping now says of itself.
    fn started(&mut self, py: Python<'_>) -> PyResult<()> {
        let grouping = self.brailleElementGrouping.bind(py);
        let mut read = if grouping.is_none() {
            RsGrouping::default()
        } else {
            crate_grouping(grouping)?.0
        };
        read.elements.clear();
        if let Some(shown) = self.shown_clef_signs {
            read.show_clef_signs = shown;
        }
        if let Some(upper) = self.upper_first {
            read.upper_first_in_note_fingering = upper;
        }
        self.grouping = read;
        self.written.clear();
        Ok(())
    }
}

#[pymethods]
impl NoteGroupingTranscriber {
    /// music21's `NoteGroupingTranscriber(brailleElementGrouping=None)`.
    #[new]
    #[pyo3(signature = (brailleElementGrouping = None))]
    fn new(py: Python<'_>, brailleElementGrouping: Option<Py<PyAny>>) -> Self {
        Self {
            showLeadingOctave: true,
            brailleElementGrouping: brailleElementGrouping.unwrap_or_else(|| py.None()),
            shown_clef_signs: None,
            upper_first: None,
            written: Vec::new(),
            grouping: RsGrouping::default(),
            transcriber: RsTranscriber::default(),
        }
    }

    /// Whether clefs are written: what a caller said, else what the
    /// grouping says, else no. music21's `showClefSigns`.
    #[getter]
    fn showClefSigns(&self, py: Python<'_>) -> PyResult<bool> {
        if let Some(shown) = self.shown_clef_signs {
            return Ok(shown);
        }
        let grouping = self.brailleElementGrouping.bind(py);
        if grouping.is_none() {
            return Ok(false);
        }
        grouping.getattr("showClefSigns")?.is_truthy()
    }

    #[setter]
    fn set_showClefSigns(
        &mut self,
        #[pyo3(from_py_with = crate::truthy_or_none)] new: Option<bool>,
    ) {
        self.shown_clef_signs = new;
    }

    /// Whether a fingering's upper choice is written first: the same three
    /// answers. music21's `upperFirstInFingering`.
    #[getter]
    fn upperFirstInFingering(&self, py: Python<'_>) -> PyResult<bool> {
        if let Some(upper) = self.upper_first {
            return Ok(upper);
        }
        let grouping = self.brailleElementGrouping.bind(py);
        if grouping.is_none() {
            return Ok(true);
        }
        grouping.getattr("upperFirstInNoteFingering")?.is_truthy()
    }

    #[setter]
    fn set_upperFirstInFingering(
        &mut self,
        #[pyo3(from_py_with = crate::truthy_or_none)] new: Option<bool>,
    ) {
        self.upper_first = new;
    }

    /// The braille written so far, a sign at a time: music21's `trans`.
    #[getter]
    fn trans<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let out = PyList::empty(py);
        for sign in self.transcriber.transcribed() {
            out.append(sign)?;
        }
        Ok(out)
    }

    /// The last element written: music21's `previousElement`.
    #[getter]
    fn previousElement(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.written.last().map(|object| object.clone_ref(py))
    }

    /// Nothing written yet: music21's `reset`.
    fn reset(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let mut me = slf.borrow_mut();
        me.transcriber = RsTranscriber::default();
        me.transcriber.show_leading_octave = me.showLeadingOctave;
        me.started(py)
    }

    /// One element written, the signs added to what has been written and
    /// its English onto the element: music21's `transcribeOneElement`.
    fn transcribeOneElement(slf: &Bound<'_, Self>, el: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let element = crate_element(el)?;
        let mut me = slf.borrow_mut();
        me.transcriber.show_leading_octave = me.showLeadingOctave;
        if let Some(shown) = me.shown_clef_signs {
            me.grouping.show_clef_signs = shown;
        }
        if let Some(upper) = me.upper_first {
            me.grouping.upper_first_in_note_fingering = upper;
        }
        me.grouping.elements.push(element);
        let at = me.grouping.elements.len() - 1;
        let me = &mut *me;
        me.transcriber
            .transcribe_element(&mut me.grouping, at)
            .map_err(|error| crate::stream::stream_error(&error))?;
        me.written.push(el.clone().unbind());
        let elements = me.grouping.elements.clone();
        let objects: Vec<Py<PyAny>> = me.written.iter().map(|one| one.clone_ref(py)).collect();
        write_english(py, &objects, &elements)
    }

    /// A whole grouping written: music21's `transcribeGroup`, which starts
    /// afresh and takes the grouping it is given.
    #[pyo3(signature = (brailleElementGrouping = None))]
    fn transcribeGroup(
        slf: &Bound<'_, Self>,
        brailleElementGrouping: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<String> {
        let py = slf.py();
        if let Some(given) = brailleElementGrouping.filter(|given| !given.is_none()) {
            slf.borrow_mut().brailleElementGrouping = given.clone().unbind();
        }
        Self::reset(slf)?;
        let grouping = slf.borrow().brailleElementGrouping.bind(py).clone();
        if grouping.is_none() {
            return Ok(String::new());
        }
        let (mut read, objects) = crate_grouping(&grouping)?;
        let mut me = slf.borrow_mut();
        me.transcriber.show_leading_octave = me.showLeadingOctave;
        if let Some(shown) = me.shown_clef_signs {
            read.show_clef_signs = shown;
        }
        if let Some(upper) = me.upper_first {
            read.upper_first_in_note_fingering = upper;
        }
        let written = me
            .transcriber
            .transcribe_group(&mut read)
            .map_err(|error| crate::stream::stream_error(&error))?;
        me.grouping = read;
        me.written = objects.iter().map(|object| object.clone_ref(py)).collect();
        let elements = me.grouping.elements.clone();
        drop(me);
        write_english(py, &objects, &elements)?;
        Ok(written)
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<NoteGroupingTranscriber>()?;
    Ok(())
}
