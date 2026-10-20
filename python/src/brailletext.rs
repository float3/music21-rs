//! music21's `braille.text`: braille laid out in lines of so many cells.
//!
//! The laying out is the crate's `braille::text` -- where a heading goes,
//! how a measure number lines up, when a line is full and a new one
//! started, how the hands' lines are written at a keyboard -- and the
//! facade keeps the lines as the objects music21 keeps them as, since
//! music21's own examples write through one (`bt.currentLine.append(...)`)
//! and read the text back afterwards.
//!
//! So a text holds its lines twice: as the crate's values, which every
//! answer comes from, and as a `BrailleTextLine` object apiece. Each call
//! reads the objects' values in, asks the crate, and writes the answer
//! back out -- keeping the object a line already had, so that a caller
//! holding one is holding the text's. It is the bargain the chord facade
//! strikes with its notes.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyList, PyTuple};

use music21_rs_crate::braille::text::{
    BrailleKeyboard as RsKeyboard, BrailleText as RsText, BrailleTextLine as RsLine,
};

/// The names this module hands out.
pub const NAMES: &[&str] = &["BrailleText", "BrailleKeyboard", "BrailleTextLine"];

pyo3::create_exception!(
    music21_rs_facade,
    BrailleTextException,
    crate::Music21Exception
);

/// music21's error for braille that will not fit, out of the crate's.
fn text_error(error: &music21_rs_crate::Error) -> PyErr {
    BrailleTextException::new_err(crate::pitch::message(error))
}

/// music21's `braille.text.BrailleTextLine`: one line, written into cell
/// by cell.
#[pyclass(
    name = "BrailleTextLine",
    module = "music21.braille.text",
    subclass,
    skip_from_py_object
)]
pub struct BrailleTextLine {
    pub(crate) inner: RsLine,
}

impl BrailleTextLine {
    /// The object for a line of the crate's.
    fn object(py: Python<'_>, inner: RsLine) -> PyResult<Py<PyAny>> {
        Ok(crate::installed_new(
            py,
            "music21.braille.text",
            "BrailleTextLine",
            Self { inner },
        )?
        .into_bound(py)
        .into_any()
        .unbind())
    }
}

#[pymethods]
impl BrailleTextLine {
    /// music21's `BrailleTextLine(lineLength=40)`.
    #[new]
    #[pyo3(signature = (lineLength = 40))]
    fn new(lineLength: usize) -> Self {
        Self {
            inner: RsLine::new(lineLength),
        }
    }

    /// Whether the line is part of a heading.
    #[getter]
    fn isHeading(&self) -> bool {
        self.inner.is_heading
    }

    #[setter]
    fn set_isHeading(&mut self, value: bool) {
        self.inner.is_heading = value;
    }

    /// Whether a group of notes has been written on it.
    #[getter]
    fn containsNoteGrouping(&self) -> bool {
        self.inner.contains_note_grouping
    }

    #[setter]
    fn set_containsNoteGrouping(&mut self, value: bool) {
        self.inner.contains_note_grouping = value;
    }

    /// How many cells the line holds.
    #[getter]
    fn lineLength(&self) -> usize {
        self.inner.line_length
    }

    #[setter]
    fn set_lineLength(&mut self, value: usize) {
        self.inner.line_length = value;
    }

    /// Where the next cell is written.
    #[getter]
    fn textLocation(&self) -> usize {
        self.inner.text_location
    }

    #[setter]
    fn set_textLocation(&mut self, value: usize) {
        self.inner.text_location = value;
    }

    /// How far the line has been written.
    #[getter]
    fn highestUsedLocation(&self) -> usize {
        self.inner.highest_used_location
    }

    #[setter]
    fn set_highestUsedLocation(&mut self, value: usize) {
        self.inner.highest_used_location = value;
    }

    /// Every cell, written or not: music21's `allChars`.
    #[getter]
    fn allChars<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let out = PyList::empty(py);
        for cell in self.inner.cells() {
            out.append(cell)?;
        }
        Ok(out)
    }

    /// Writes text at the end, after a space where asked: music21's
    /// `append`.
    #[pyo3(signature = (text, addSpace = true))]
    fn append(&mut self, text: &str, addSpace: bool) -> PyResult<()> {
        self.inner
            .append(text, addSpace)
            .map_err(|error| text_error(&error))
    }

    /// Writes text at a place on the line: music21's `insert`.
    fn insert(&mut self, textLocation: usize, text: &str) -> PyResult<()> {
        self.inner
            .insert(textLocation, text)
            .map_err(|error| text_error(&error))
    }

    /// Whether text would fit at the end: music21's `canAppend`.
    #[pyo3(signature = (text, addSpace = true))]
    fn canAppend(&self, text: &str, addSpace: bool) -> bool {
        self.inner.can_append(text, addSpace)
    }

    /// Whether text would fit at a place: music21's `canInsert`.
    fn canInsert(&self, textLocation: usize, text: &str) -> bool {
        self.inner.can_insert(textLocation, text)
    }

    /// Turns a hyphen at the end back into a space: music21's
    /// `lastHyphenToSpace`.
    fn lastHyphenToSpace(&mut self) {
        self.inner.last_hyphen_to_space();
    }

    /// The line as far as it is written, which is music21's `str`.
    fn __str__(&self) -> String {
        self.inner.text()
    }

    /// music21's own `repr`, which shows every cell of the line as Python
    /// writes a string.
    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let written: String = self.inner.cells().concat();
        Ok(format!(
            "<music21.braille.text.BrailleTextLine {}>",
            pyo3::types::PyString::new(py, &written).repr()?
        ))
    }
}

/// A text's lines, held as the crate's values and as the objects music21
/// keeps them as.
struct Lines;

impl Lines {
    /// The values the line objects now hold, which is what a caller
    /// writing through one has changed.
    fn read(py: Python<'_>, objects: &[Py<PyAny>]) -> PyResult<Vec<RsLine>> {
        let mut values = Vec::with_capacity(objects.len());
        for object in objects {
            values.push(
                object
                    .bind(py)
                    .extract::<PyRef<'_, BrailleTextLine>>()?
                    .inner
                    .clone(),
            );
        }
        Ok(values)
    }

    /// The crate's lines written back onto the objects, each line keeping
    /// the object it had so that a caller holding one is holding the
    /// text's.
    fn write(py: Python<'_>, objects: &mut Vec<Py<PyAny>>, values: &[RsLine]) -> PyResult<()> {
        for (at, value) in values.iter().enumerate() {
            match objects.get(at) {
                Some(object) => {
                    object
                        .bind(py)
                        .extract::<PyRefMut<'_, BrailleTextLine>>()?
                        .inner = value.clone();
                }
                None => objects.push(BrailleTextLine::object(py, value.clone())?),
            }
        }
        objects.truncate(values.len());
        Ok(())
    }
}

/// music21's `braille.text.BrailleText`: braille laid out in lines, with
/// headings, measure numbers and the signs of a hand at the keyboard.
#[pyclass(
    name = "BrailleText",
    module = "music21.braille.text",
    subclass,
    skip_from_py_object
)]
pub struct BrailleText {
    inner: RsText,
    lines: Vec<Py<PyAny>>,
}

impl BrailleText {
    fn of(py: Python<'_>, inner: RsText) -> PyResult<Self> {
        let mut lines = Vec::new();
        Lines::write(py, &mut lines, &inner.all_lines)?;
        Ok(Self { inner, lines })
    }

    /// The crate's text as it stands, with whatever a caller wrote
    /// through a line object read back into it.
    fn synced(&mut self, py: Python<'_>) -> PyResult<()> {
        self.inner.all_lines = Lines::read(py, &self.lines)?;
        Ok(())
    }

    /// Runs one of the crate's steps over the text and writes the lines
    /// back onto the objects.
    fn step(
        slf: &Bound<'_, Self>,
        run: impl FnOnce(&mut RsText) -> music21_rs_crate::error::Result<()>,
    ) -> PyResult<()> {
        let py = slf.py();
        let mut me = slf.borrow_mut();
        me.synced(py)?;
        run(&mut me.inner).map_err(|error| text_error(&error))?;
        let values = me.inner.all_lines.clone();
        let mut lines = std::mem::take(&mut me.lines);
        drop(me);
        Lines::write(py, &mut lines, &values)?;
        slf.borrow_mut().lines = lines;
        Ok(())
    }
}

#[pymethods]
impl BrailleText {
    /// music21's `BrailleText(lineLength=40, showHand=None)`.
    #[new]
    #[pyo3(signature = (lineLength = 40, showHand = None))]
    fn new(py: Python<'_>, lineLength: usize, showHand: Option<&str>) -> PyResult<Self> {
        let inner = RsText::new(lineLength, showHand).map_err(|error| text_error(&error))?;
        Self::of(py, inner)
    }

    /// How many cells a line holds.
    #[getter]
    fn lineLength(&self) -> usize {
        self.inner.line_length
    }

    #[setter]
    fn set_lineLength(&mut self, value: usize) {
        self.inner.line_length = value;
    }

    /// Every line, as the objects the text keeps.
    #[getter]
    fn allLines<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let out = PyList::empty(py);
        for line in &self.lines {
            out.append(line.bind(py))?;
        }
        Ok(out)
    }

    /// The line being written: music21's `currentLine`, the last of them.
    #[getter]
    fn currentLine(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.lines.last().map(|line| line.clone_ref(py))
    }

    /// Whether lines of notes start with the right hand's sign.
    #[getter]
    fn rightHandSymbol(&self) -> bool {
        self.inner.right_hand_symbol
    }

    #[setter]
    fn set_rightHandSymbol(&mut self, value: bool) {
        self.inner.right_hand_symbol = value;
    }

    /// Whether they start with the left hand's.
    #[getter]
    fn leftHandSymbol(&self) -> bool {
        self.inner.left_hand_symbol
    }

    #[setter]
    fn set_leftHandSymbol(&mut self, value: bool) {
        self.inner.left_hand_symbol = value;
    }

    /// Where each heading's lines start and end.
    #[getter]
    fn allHeadings<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let out = PyList::empty(py);
        for (start, end) in &self.inner.all_headings {
            out.append(PyTuple::new(py, [*start, *end])?)?;
        }
        Ok(out)
    }

    /// Which hand's sign the notes' lines start with. music21's getter
    /// never answers what was set, and this does not either.
    #[getter]
    fn showHand(&self, py: Python<'_>) -> Py<PyAny> {
        py.None()
    }

    #[setter]
    fn set_showHand(&mut self, newHand: Option<&str>) -> PyResult<()> {
        self.inner
            .set_show_hand(newHand)
            .map_err(|error| text_error(&error))
    }

    /// Starts a new line: music21's `makeNewLine`.
    fn makeNewLine(slf: &Bound<'_, Self>) -> PyResult<()> {
        Self::step(slf, |text| {
            text.make_new_line();
            Ok(())
        })
    }

    /// A heading, a line each, on lines of its own: music21's
    /// `addHeading`.
    fn addHeading(slf: &Bound<'_, Self>, heading: &str) -> PyResult<()> {
        Self::step(slf, |text| text.add_heading(heading))
    }

    /// An expression long enough to be split across lines: music21's
    /// `addLongExpression`.
    fn addLongExpression(slf: &Bound<'_, Self>, longExpr: &str) -> PyResult<()> {
        Self::step(slf, |text| text.add_long_expression(longExpr))
    }

    /// A group of notes on a line of its own: music21's `addToNewLine`.
    fn addToNewLine(slf: &Bound<'_, Self>, brailleNoteGrouping: &str) -> PyResult<()> {
        Self::step(slf, |text| text.add_to_new_line(brailleNoteGrouping))
    }

    /// Writes on the current line where it fits, and on a new one where it
    /// does not: music21's `appendOrInsertCurrent`.
    #[pyo3(signature = (brailleExpr, addSpace = true))]
    fn appendOrInsertCurrent(
        slf: &Bound<'_, Self>,
        brailleExpr: &str,
        addSpace: bool,
    ) -> PyResult<()> {
        Self::step(slf, |text| {
            text.append_or_insert_current(brailleExpr, addSpace)
        })
    }

    /// An inaccord, which is written on its own lines: music21's
    /// `addInaccord`.
    fn addInaccord(slf: &Bound<'_, Self>, inaccord: &str) -> PyResult<()> {
        Self::step(slf, |text| text.add_inaccord(inaccord))
    }

    /// A measure number, which starts a line: music21's
    /// `addMeasureNumber`, taking the number itself or the braille of it.
    fn addMeasureNumber(slf: &Bound<'_, Self>, measureNumber: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Ok(number) = measureNumber.extract::<i64>() {
            return Self::step(slf, |text| text.add_measure_number_value(number));
        }
        let written = measureNumber.str()?.to_string();
        Self::step(slf, |text| text.add_measure_number(&written))
    }

    /// The hand's sign and the dots of a group, where the text asks for
    /// them: music21's `optionalAddKeyboardSymbolsAndDots`.
    #[pyo3(signature = (noteGrouping = None))]
    fn optionalAddKeyboardSymbolsAndDots(
        slf: &Bound<'_, Self>,
        noteGrouping: Option<&str>,
    ) -> PyResult<()> {
        Self::step(slf, |text| {
            // The crate says whether it wrote anything, which music21's
            // own caller reads off the line instead.
            text.optional_add_keyboard_symbols_and_dots(noteGrouping)
                .map(|_| ())
        })
    }

    /// The signatures, written after what is on the line: music21's
    /// `addSignatures`.
    fn addSignatures(slf: &Bound<'_, Self>, signatures: &str) -> PyResult<()> {
        Self::step(slf, |text| text.add_signatures(signatures))
    }

    /// Centres every heading over the lines of notes under it: music21's
    /// `recenterHeadings`.
    fn recenterHeadings(slf: &Bound<'_, Self>) -> PyResult<()> {
        Self::step(slf, |text| {
            text.recenter_headings();
            Ok(())
        })
    }

    /// The whole text, a line at a time: music21's `str`.
    fn __str__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let py = slf.py();
        let mut me = slf.borrow_mut();
        me.synced(py)?;
        Ok(me.inner.render())
    }

    /// music21's own `repr`: how many lines, how many headings and how
    /// wide.
    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let me = slf.borrow();
        let lines = me.lines.len();
        let headings = me.inner.all_headings.len();
        Ok(format!(
            "<music21.braille.text.BrailleText {lines} line{}, {headings} heading{}, {} cols>",
            if lines == 1 { "" } else { "s" },
            if headings == 1 { "" } else { "s" },
            me.inner.line_length,
        ))
    }
}

/// music21's `braille.text.BrailleKeyboard`: the two hands at once.
#[pyclass(
    name = "BrailleKeyboard",
    module = "music21.braille.text",
    extends = BrailleText,
    subclass,
    skip_from_py_object
)]
pub struct BrailleKeyboard {
    /// Which of the text's lines each hand is on, and how wide the measure
    /// numbers are: the keyboard's own state, the lines themselves being
    /// the text's.
    right_hand_line: Option<usize>,
    left_hand_line: Option<usize>,
    highest_measure_number_length: usize,
}

impl BrailleKeyboard {
    /// The keyboard as the crate keeps one, out of the text and the two
    /// hands' lines.
    fn keyboard(slf: &Bound<'_, Self>) -> PyResult<RsKeyboard> {
        let py = slf.py();
        // A subclass and the class under it share one borrow flag, so
        // what the keyboard itself says is read and let go of before the
        // text is borrowed to write into.
        let (right_hand_line, left_hand_line, highest_measure_number_length) = {
            let me = slf.borrow();
            (
                me.right_hand_line,
                me.left_hand_line,
                me.highest_measure_number_length,
            )
        };
        let mut text = slf.as_super().borrow_mut();
        text.synced(py)?;
        Ok(RsKeyboard {
            text: text.inner.clone(),
            right_hand_line,
            left_hand_line,
            highest_measure_number_length,
        })
    }

    /// The keyboard's answer written back: the text's lines onto their
    /// objects and the hands' lines onto the keyboard.
    fn written(slf: &Bound<'_, Self>, keyboard: RsKeyboard) -> PyResult<()> {
        let py = slf.py();
        {
            let mut me = slf.borrow_mut();
            me.right_hand_line = keyboard.right_hand_line;
            me.left_hand_line = keyboard.left_hand_line;
            me.highest_measure_number_length = keyboard.highest_measure_number_length;
        }
        let values = keyboard.text.all_lines.clone();
        let mut lines = {
            let mut text = slf.as_super().borrow_mut();
            text.inner = keyboard.text;
            std::mem::take(&mut text.lines)
        };
        Lines::write(py, &mut lines, &values)?;
        slf.as_super().borrow_mut().lines = lines;
        Ok(())
    }
}

#[pymethods]
impl BrailleKeyboard {
    /// music21's `BrailleKeyboard(lineLength=40)`.
    #[new]
    #[pyo3(signature = (lineLength = 40))]
    fn new(py: Python<'_>, lineLength: usize) -> PyResult<PyClassInitializer<Self>> {
        let keyboard = RsKeyboard::new(lineLength);
        Ok(
            PyClassInitializer::from(BrailleText::of(py, keyboard.text)?).add_subclass(Self {
                right_hand_line: keyboard.right_hand_line,
                left_hand_line: keyboard.left_hand_line,
                highest_measure_number_length: keyboard.highest_measure_number_length,
            }),
        )
    }

    /// How many digits the highest measure number has, so the numbers
    /// line up.
    #[getter]
    fn highestMeasureNumberLength(&self) -> usize {
        self.highest_measure_number_length
    }

    #[setter]
    fn set_highestMeasureNumberLength(&mut self, value: usize) {
        self.highest_measure_number_length = value;
    }

    /// The line the right hand is written on, as the text's own object.
    #[getter]
    fn rightHandLine(slf: &Bound<'_, Self>) -> Option<Py<PyAny>> {
        let py = slf.py();
        let at = slf.borrow().right_hand_line?;
        slf.as_super()
            .borrow()
            .lines
            .get(at)
            .map(|line| line.clone_ref(py))
    }

    /// The line the left hand is written on.
    #[getter]
    fn leftHandLine(slf: &Bound<'_, Self>) -> Option<Py<PyAny>> {
        let py = slf.py();
        let at = slf.borrow().left_hand_line?;
        slf.as_super()
            .borrow()
            .lines
            .get(at)
            .map(|line| line.clone_ref(py))
    }

    /// A line for each hand, the right one on the current line if it is
    /// empty: music21's `makeNewLines`.
    fn makeNewLines(slf: &Bound<'_, Self>) -> PyResult<()> {
        let mut keyboard = Self::keyboard(slf)?;
        keyboard.make_new_lines();
        Self::written(slf, keyboard)
    }

    /// A measure's groups for each hand: music21's `addNoteGroupings`.
    fn addNoteGroupings(
        slf: &Bound<'_, Self>,
        measureNumber: &Bound<'_, PyAny>,
        noteGroupingR: &str,
        noteGroupingL: &str,
    ) -> PyResult<()> {
        let number = measureNumber.str()?.to_string();
        let mut keyboard = Self::keyboard(slf)?;
        keyboard
            .add_note_groupings(&number, noteGroupingR, noteGroupingL)
            .map_err(|error| text_error(&error))?;
        Self::written(slf, keyboard)
    }

    /// music21's own `repr` for one of these, which is the text's.
    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        Ok(BrailleText::__repr__(slf.as_super())?.replace("BrailleText ", "BrailleKeyboard "))
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<BrailleTextLine>()?;
    m.add_class::<BrailleText>()?;
    m.add_class::<BrailleKeyboard>()?;
    Ok(())
}
