//! music21's `musedata` package: `MuseDataRecord`, `MuseDataMeasure`,
//! `MuseDataPart`, `MuseDataFile`, `MuseDataWork` and `MuseDataDirectory`,
//! over the crate's MuseData reader.
//!
//! Each object keeps music21's attributes in its `__dict__` -- a record's
//! `src` and `stage`, a part's `src` lines and measure boundaries -- and
//! each question is the crate's, asked of a record or part made from them.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use music21::musedata::{self as rs, Case, Part, Record, Spaces, Stage};
use music21::stream::StreamElement;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyList, PyModule, PyString, PyTuple};

use crate::prebase::ProtoM21Object;

/// The module music21 keeps these in.
const MODULE: &str = "music21.musedata";

/// music21's number for stage 1 and stage 2.
const STAGE_ONE: i64 = 1;
const STAGE_TWO: i64 = 2;

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

pyo3::create_exception!(
    music21_rs_facade,
    MuseDataException,
    crate::Music21Exception
);

/// A crate refusal as music21's exception.
pub(crate) fn refusal(error: &music21::Error) -> PyErr {
    MuseDataException::new_err(crate::pitch::message(error))
}

/// A stage as music21 numbers it.
fn stage_number(stage: Stage) -> i64 {
    match stage {
        Stage::One => STAGE_ONE,
        Stage::Two => STAGE_TWO,
    }
}

/// The stage an object holds: stage 1 where it says so, stage 2 where it
/// says that or nothing, as music21's tests read it.
fn stage_of(object: &Bound<'_, PyAny>) -> PyResult<Stage> {
    let stage = object.getattr("stage")?;
    if !stage.is_none() && stage.extract::<i64>()? == STAGE_ONE {
        return Ok(Stage::One);
    }
    Ok(Stage::Two)
}

/// A string attribute.
fn src_text(object: &Bound<'_, PyAny>) -> PyResult<String> {
    object.getattr("src")?.extract()
}

/// The lines an object holds as `src`.
fn src_lines(object: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    object.getattr("src")?.extract()
}

/// A crate element as this package's object, or `None`.
fn element<'py>(py: Python<'py>, element: StreamElement) -> PyResult<Bound<'py, PyAny>> {
    Ok(crate::stream::element_object(py, &element)?.unwrap_or_else(|| py.None().into_bound(py)))
}

/// A crate pitch as music21's.
fn pitch_object(py: Python<'_>, pitch: music21::Pitch) -> PyResult<Bound<'_, PyAny>> {
    Ok(crate::installed_new(
        py,
        "music21.pitch",
        "Pitch",
        crate::pitch::Pitch::wrap(pitch, false),
    )?
    .into_bound(py)
    .into_any())
}

/// A new object of this module's, built on music21's `ProtoM21Object`.
fn base() -> PyClassInitializer<ProtoM21Object> {
    PyClassInitializer::from(ProtoM21Object::default())
}

// ---------------------------------------------------------------- records

/// music21's `MuseDataRecord`: one line of a measure.
#[pyclass(module = "music21.musedata", extends = ProtoM21Object, subclass, dict)]
pub struct MuseDataRecord;

impl MuseDataRecord {
    /// The crate's answer about the record, read from its line and stage.
    fn asked<T>(slf: &Bound<'_, Self>, ask: impl FnOnce(&Record<'_>) -> T) -> PyResult<T> {
        let line = src_text(slf.as_any())?;
        let stage = stage_of(slf.as_any())?;
        Ok(ask(&Record::new(&line, stage)))
    }
}

#[pymethods]
impl MuseDataRecord {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        let _ = (args, kwargs);
        base().add_subclass(Self)
    }

    #[pyo3(signature = (src = "", parent = None))]
    fn __init__(
        slf: &Bound<'_, Self>,
        src: &str,
        parent: Option<Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let py = slf.py();
        let parent = parent.unwrap_or_else(|| py.None().into_bound(py));
        // The part's stage, reached through the measure.
        let stage = if parent.is_none() {
            py.None().into_bound(py)
        } else {
            parent.getattr("parent")?.getattr("stage")?
        };
        slf.setattr("src", src)?;
        slf.setattr("parent", parent)?;
        slf.setattr("stage", stage)?;
        slf.setattr("_cache", PyDict::new(py))
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        crate::prebase::copied(slf.as_any(), None)
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        crate::prebase::copied(slf.as_any(), Some(memo))
    }

    /// music21's `isRest`.
    fn isRest(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Self::asked(slf, |record| record.is_rest())
    }

    /// music21's `isTied`.
    fn isTied(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Self::asked(slf, |record| record.is_tied())
    }

    /// music21's `isNote`.
    fn isNote(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Self::asked(slf, |record| record.is_note())
    }

    /// music21's `isChord`: a further pitch of the note before.
    fn isChord(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Self::asked(slf, |record| record.is_chord())
    }

    /// music21's `isCueOrGrace`.
    fn isCueOrGrace(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Self::asked(slf, |record| record.is_cue_or_grace())
    }

    /// music21's `isBack`.
    fn isBack(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Self::asked(slf, |record| record.is_back())
    }

    /// music21's `_getPitchParameters`: the pitch's name, `E-4`.
    fn _getPitchParameters(slf: &Bound<'_, Self>) -> PyResult<String> {
        Self::asked(slf, |record| record.pitch_parameters())?.map_err(|error| refusal(&error))
    }

    /// music21's `_getAccidentalObject`: the accidental column 19 shows,
    /// shown always.
    fn _getAccidentalObject<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let Some(mut accidental) = Self::asked(slf, |record| record.shown_accidental())?
            .map_err(|error| refusal(&error))?
        else {
            return Ok(py.None().into_bound(py));
        };
        accidental
            .set_display_type("always")
            .map_err(|error| refusal(&error))?;
        accidental.set_display_status(Some(true));
        Ok(crate::installed_new(
            py,
            "music21.pitch",
            "Accidental",
            crate::pitch::Accidental::from_inner(accidental),
        )?
        .into_bound(py)
        .into_any())
    }

    /// music21's `getPitchObject`.
    fn getPitchObject<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let pitch = Self::asked(slf, |record| record.pitch())?.map_err(|error| refusal(&error))?;
        pitch_object(slf.py(), pitch)
    }

    /// music21's `getQuarterLength`, at the divisions of the record's part,
    /// or those given where it has none.
    #[pyo3(signature = (divisionsPerQuarterNote = None))]
    fn getQuarterLength<'py>(
        slf: &Bound<'py, Self>,
        divisionsPerQuarterNote: Option<f64>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let parent = slf.getattr("parent")?;
        let divisions = if !parent.is_none() {
            parent
                .getattr("parent")?
                .call_method0("getDivisionsPerQuarterNote")?
                .extract()?
        } else if let Some(given) = divisionsPerQuarterNote {
            given
        } else {
            return Err(MuseDataException::new_err(
                "cannot access parent container of this record to obtain divisions per quarter",
            ));
        };
        let length = Self::asked(slf, |record| record.quarter_length(divisions))?
            .map_err(|error| refusal(&error))?;
        crate::subconverters::music21_name(py, "music21.common", "opFrac")?.call1((length,))
    }

    /// music21's `getDots`: none for stage 1.
    fn getDots(slf: &Bound<'_, Self>) -> PyResult<Option<u8>> {
        Self::asked(slf, |record| record.dots())
    }

    /// music21's `getLyrics`: each verse's syllable.
    fn getLyrics(slf: &Bound<'_, Self>) -> PyResult<Option<Vec<String>>> {
        Self::asked(slf, |record| record.lyrics())
    }

    /// music21's `getBeams`: the beam column.
    fn getBeams(slf: &Bound<'_, Self>) -> PyResult<Option<String>> {
        Self::asked(slf, |record| record.beams())
    }

    /// music21's `getSlurSymbols`: the slur marks, in order.
    fn getSlurSymbols(slf: &Bound<'_, Self>) -> PyResult<String> {
        Self::asked(slf, |record| record.slur_marks())
    }

    /// music21's `_getAdditionalNotations`: columns 32 to 43.
    fn _getAdditionalNotations(slf: &Bound<'_, Self>) -> PyResult<Option<String>> {
        Self::asked(slf, |record| record.notations())
    }

    /// music21's `getArticulationObjects`.
    fn getArticulationObjects<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let made = PyList::empty(py);
        for articulation in Self::asked(slf, |record| record.articulations())? {
            made.append(crate::articulations::Articulation::object(
                py,
                articulation,
            )?)?;
        }
        Ok(made)
    }

    /// music21's `getExpressionObjects`.
    fn getExpressionObjects<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let made = PyList::empty(py);
        for expression in Self::asked(slf, |record| record.expressions())? {
            if let Some(object) = crate::stream::music21_expression(py, &expression)? {
                made.append(object)?;
            }
        }
        Ok(made)
    }

    /// music21's `getDynamicObjects`.
    fn getDynamicObjects<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let made = PyList::empty(py);
        for dynamic in Self::asked(slf, |record| record.dynamics())? {
            made.append(element(py, StreamElement::Dynamic(dynamic))?)?;
        }
        Ok(made)
    }

    /// music21's `hasCautionaryAccidental`.
    fn hasCautionaryAccidental(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Self::asked(slf, |record| record.has_cautionary_accidental())
    }
}

/// music21's `MuseDataRecordIterator` and `MuseDataMeasureIterator`: each
/// item of `src`, or of `src` cut at `boundaries`, made as it is asked for.
#[pyclass(module = "music21.musedata", subclass, dict)]
pub struct MuseDataRecordIterator;

#[pymethods]
impl MuseDataRecordIterator {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(args: &Bound<'_, PyTuple>, kwargs: Option<&Bound<'_, PyDict>>) -> Self {
        let _ = (args, kwargs);
        Self
    }

    fn __init__(
        slf: &Bound<'_, Self>,
        src: &Bound<'_, PyAny>,
        parent: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        slf.setattr("src", src)?;
        slf.setattr("index", 0)?;
        slf.setattr("parent", parent)
    }

    fn __iter__(slf: Bound<'_, Self>) -> Bound<'_, Self> {
        slf
    }

    fn __next__<'py>(slf: &Bound<'py, Self>) -> PyResult<Option<Bound<'py, PyAny>>> {
        let py = slf.py();
        let src = slf.getattr("src")?;
        let index: usize = slf.getattr("index")?.extract()?;
        if index >= src.len()? {
            return Ok(None);
        }
        let made = py
            .get_type::<MuseDataRecord>()
            .call1((src.get_item(index)?, slf.getattr("parent")?))?;
        slf.setattr("index", index + 1)?;
        Ok(Some(made))
    }
}

/// music21's `MuseDataMeasureIterator`: each measure of a part's lines, as
/// its boundaries cut them, made as it is asked for.
#[pyclass(module = "music21.musedata", subclass, dict)]
pub struct MuseDataMeasureIterator;

#[pymethods]
impl MuseDataMeasureIterator {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(args: &Bound<'_, PyTuple>, kwargs: Option<&Bound<'_, PyDict>>) -> Self {
        let _ = (args, kwargs);
        Self
    }

    fn __init__(
        slf: &Bound<'_, Self>,
        src: &Bound<'_, PyAny>,
        boundaries: &Bound<'_, PyAny>,
        parent: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        slf.setattr("src", src)?;
        slf.setattr("boundaries", boundaries)?;
        slf.setattr("index", 0)?;
        slf.setattr("parent", parent)
    }

    fn __iter__(slf: Bound<'_, Self>) -> Bound<'_, Self> {
        slf
    }

    fn __next__<'py>(slf: &Bound<'py, Self>) -> PyResult<Option<Bound<'py, PyAny>>> {
        let py = slf.py();
        let boundaries = slf.getattr("boundaries")?;
        let index: usize = slf.getattr("index")?.extract()?;
        if index >= boundaries.len()? {
            return Ok(None);
        }
        let (start, end): (usize, usize) = boundaries.get_item(index)?.extract()?;
        let lines: Vec<String> = slf.getattr("src")?.extract()?;
        // The end is the measure's last line.
        let held = lines
            .get(start..=end.min(lines.len().saturating_sub(1)))
            .unwrap_or(&[]);
        let made = py
            .get_type::<MuseDataMeasure>()
            .call1((PyList::new(py, held)?, slf.getattr("parent")?))?;
        slf.setattr("index", index + 1)?;
        Ok(Some(made))
    }
}

// --------------------------------------------------------------- measures

/// music21's `MuseDataMeasure`: the lines of one measure.
#[pyclass(module = "music21.musedata", extends = ProtoM21Object, subclass, dict)]
pub struct MuseDataMeasure;

#[pymethods]
impl MuseDataMeasure {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        let _ = (args, kwargs);
        base().add_subclass(Self)
    }

    #[pyo3(signature = (src = None, parent = None))]
    fn __init__(
        slf: &Bound<'_, Self>,
        src: Option<Bound<'_, PyAny>>,
        parent: Option<Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let py = slf.py();
        let src = src.unwrap_or_else(|| PyList::empty(py).into_any());
        let parent = parent.unwrap_or_else(|| py.None().into_bound(py));
        let stage = if parent.is_none() {
            py.None().into_bound(py)
        } else {
            parent.getattr("stage")?
        };
        slf.setattr("src", src)?;
        slf.setattr("parent", parent)?;
        slf.setattr("stage", stage)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        crate::prebase::copied(slf.as_any(), None)
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        crate::prebase::copied(slf.as_any(), Some(memo))
    }

    /// music21's `_reprInternal`, as music21 writes it.
    fn _reprInternal(&self) -> &'static str {
        "size={len(self.src)}"
    }

    /// music21's `getBarObject`: the barline the measure's record draws.
    fn getBarObject<'py>(slf: &Bound<'py, Self>) -> PyResult<Py<PyAny>> {
        let barline = rs::bar_object(&src_lines(slf.as_any())?).map_err(|error| refusal(&error))?;
        crate::bar::Barline::object(slf.py(), barline)
    }

    /// music21's `getMeasureObject`: a measure numbered as the record says,
    /// opening with its barline.
    fn getMeasureObject<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let measure =
            rs::measure_object(&src_lines(slf.as_any())?).map_err(|error| refusal(&error))?;
        crate::stream::from_crate(slf.py(), &measure)
    }

    /// music21's `hasNotes`.
    fn hasNotes(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Ok(rs::has_notes(&src_lines(slf.as_any())?))
    }

    /// music21's `hasVoices`: whether a record is a `back`.
    fn hasVoices(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Ok(rs::has_voices(&src_lines(slf.as_any())?))
    }

    /// The measure's records, made one at a time.
    fn __iter__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        slf.py()
            .get_type::<MuseDataRecordIterator>()
            .call1((slf.getattr("src")?, slf))
    }

    /// music21's `getRecords`.
    fn getRecords<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let made = PyList::empty(slf.py());
        for record in Self::__iter__(slf)?.try_iter()? {
            made.append(record?)?;
        }
        Ok(made)
    }
}

// ------------------------------------------------------------------ parts

/// music21's `MuseDataPart`: the lines of one part.
#[pyclass(module = "music21.musedata", extends = ProtoM21Object, subclass, dict)]
pub struct MuseDataPart;

impl MuseDataPart {
    /// The crate's part, of the lines and stage the object holds.
    fn value(slf: &Bound<'_, Self>) -> PyResult<Part> {
        Ok(Part::with_stage(
            src_lines(slf.as_any())?,
            stage_of(slf.as_any())?,
        ))
    }
}

#[pymethods]
impl MuseDataPart {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        let _ = (args, kwargs);
        base().add_subclass(Self)
    }

    #[pyo3(signature = (src = None, stage = None))]
    fn __init__(
        slf: &Bound<'_, Self>,
        src: Option<Vec<String>>,
        stage: Option<i64>,
    ) -> PyResult<()> {
        let py = slf.py();
        let mut lines = src.unwrap_or_default();

        // The stage where none is given, and stage 1's blank lead cut.
        let stage = match stage {
            Some(given) => Some(given),
            None if !lines.is_empty() => Some(stage_number(Part::stage_of(&lines))),
            None => None,
        };
        if stage == Some(STAGE_ONE) {
            lines = Part::scrubbed(&lines).map_err(|error| refusal(&error))?;
        }
        slf.setattr("src", PyList::new(py, lines)?)?;
        slf.setattr("_measureBoundaries", PyList::empty(py))?;
        slf.setattr("_divisionsPerQuarter", py.None())?;
        slf.setattr("_divisionsPerQuarterNote", py.None())?;
        slf.setattr("stage", stage)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        crate::prebase::copied(slf.as_any(), None)
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        crate::prebase::copied(slf.as_any(), Some(memo))
    }

    fn _reprInternal(&self) -> &'static str {
        ""
    }

    /// music21's `_determineStage`: 2 where any line is an attribute record.
    fn _determineStage(slf: &Bound<'_, Self>) -> PyResult<i64> {
        Ok(stage_number(Part::stage_of(&src_lines(slf.as_any())?)))
    }

    /// music21's `_scrubStage1`: the lines without the blank ones they open
    /// with.
    fn _scrubStage1(&self, src: Vec<String>) -> PyResult<Vec<String>> {
        Part::scrubbed(&src).map_err(|error| refusal(&error))
    }

    /// music21's `_getDigitsFollowingTag`.
    fn _getDigitsFollowingTag(&self, line: &str, tag: &str) -> String {
        Part::digits_after(line, tag)
    }

    /// music21's `_getAlphasFollowingTag`.
    #[pyo3(signature = (line, tag, keepSpace = false, keepCase = false))]
    fn _getAlphasFollowingTag(
        &self,
        line: &str,
        tag: &str,
        keepSpace: bool,
        keepCase: bool,
    ) -> String {
        let spaces = if keepSpace {
            Spaces::Kept
        } else {
            Spaces::Dropped
        };
        let case = if keepCase { Case::Kept } else { Case::Lowered };
        Part::letters_after(line, tag, spaces, case)
    }

    /// music21's `getWorkNumber`: the opus number, as written.
    fn getWorkNumber(slf: &Bound<'_, Self>) -> PyResult<Option<String>> {
        Ok(Self::value(slf)?.work_number())
    }

    /// music21's `getMovementNumber`, as written.
    fn getMovementNumber(slf: &Bound<'_, Self>) -> PyResult<Option<String>> {
        Ok(Self::value(slf)?.movement_number())
    }

    /// music21's `getDirective`: usually a tempo word.
    fn getDirective(slf: &Bound<'_, Self>) -> PyResult<Option<String>> {
        Self::value(slf)?
            .directive()
            .map_err(|error| refusal(&error))
    }

    /// music21's `getSource`: where the encoding was taken from.
    fn getSource(slf: &Bound<'_, Self>) -> PyResult<Option<String>> {
        Ok(Self::value(slf)?.source())
    }

    /// music21's `getWorkTitle`.
    fn getWorkTitle(slf: &Bound<'_, Self>) -> PyResult<Option<String>> {
        Ok(Self::value(slf)?.work_title())
    }

    /// music21's `getMovementTitle`.
    fn getMovementTitle(slf: &Bound<'_, Self>) -> PyResult<Option<String>> {
        Ok(Self::value(slf)?.movement_title())
    }

    /// music21's `getPartName`.
    fn getPartName(slf: &Bound<'_, Self>) -> PyResult<Option<String>> {
        Ok(Self::value(slf)?.part_name())
    }

    /// music21's `getGroupMemberships`.
    fn getGroupMemberships(slf: &Bound<'_, Self>) -> PyResult<Vec<String>> {
        Ok(Self::value(slf)?.group_memberships())
    }

    /// music21's `getGroupMembershipsTotal`: how many parts the group has.
    #[pyo3(signature = (membership = "score"))]
    fn getGroupMembershipsTotal(slf: &Bound<'_, Self>, membership: &str) -> PyResult<Option<i32>> {
        Self::value(slf)?
            .group_total(membership)
            .map_err(|error| refusal(&error))
    }

    /// music21's `getGroupMembershipNumber`: text for stage 1, a number for
    /// stage 2, as music21 hands them back.
    #[pyo3(signature = (membership = "score"))]
    fn getGroupMembershipNumber<'py>(
        slf: &Bound<'py, Self>,
        membership: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let part = Self::value(slf)?;
        let Some(written) = part
            .group_number(membership)
            .map_err(|error| refusal(&error))?
        else {
            return Ok(py.None().into_bound(py));
        };
        if part.stage() == Stage::One {
            return Ok(PyString::new(py, &written).into_any());
        }
        let number: i64 = written.trim().parse().map_err(|_| {
            pyo3::exceptions::PyValueError::new_err(format!(
                "invalid literal for int() with base 10: '{written}'"
            ))
        })?;
        Ok(number.into_pyobject(py)?.into_any())
    }

    /// music21's `_getAttributesRecord`.
    fn _getAttributesRecord(slf: &Bound<'_, Self>) -> PyResult<String> {
        Self::value(slf)?
            .attributes()
            .map_err(|error| refusal(&error))
    }

    /// music21's `getKeyParameters`: the signature's sharps.
    fn getKeyParameters(slf: &Bound<'_, Self>) -> PyResult<i32> {
        Self::value(slf)?
            .key_parameters()
            .map_err(|error| refusal(&error))
    }

    /// music21's `getKeySignature`.
    fn getKeySignature<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let key = Self::value(slf)?
            .key_signature()
            .map_err(|error| refusal(&error))?;
        element(slf.py(), key.into())
    }

    /// music21's `getTimeSignatureParameters`: the meter as `3/4`.
    fn getTimeSignatureParameters(slf: &Bound<'_, Self>) -> PyResult<String> {
        Self::value(slf)?
            .time_signature_parameters()
            .map_err(|error| refusal(&error))
    }

    /// music21's `getTimeSignatureObject`.
    fn getTimeSignatureObject<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let meter = Self::value(slf)?
            .time_signature()
            .map_err(|error| refusal(&error))?;
        element(slf.py(), meter.into())
    }

    /// music21's `_getNumberOfStaves`.
    fn _getNumberOfStaves(slf: &Bound<'_, Self>) -> PyResult<i32> {
        Self::value(slf)?
            .staff_count()
            .map_err(|error| refusal(&error))
    }

    /// music21's `_getClefParameters`: each staff's clef code, as written.
    fn _getClefParameters(slf: &Bound<'_, Self>) -> PyResult<Vec<String>> {
        Self::value(slf)?
            .clef_parameters()
            .map_err(|error| refusal(&error))
    }

    /// music21's `getClefObject`: the clef of staff `voice`.
    #[pyo3(signature = (voice = 1))]
    fn getClefObject<'py>(slf: &Bound<'py, Self>, voice: usize) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        match Self::value(slf)?
            .clef_for(voice)
            .map_err(|error| refusal(&error))?
        {
            Some(clef) => element(py, clef.into()),
            None => Ok(py.None().into_bound(py)),
        }
    }

    /// music21's `_getTranspositionParameters`: `X:`, in base 40.
    fn _getTranspositionParameters(slf: &Bound<'_, Self>) -> PyResult<Option<i32>> {
        Self::value(slf)?
            .transposition_parameters()
            .map_err(|error| refusal(&error))
    }

    /// music21's `getTranspositionIntervalObject`.
    fn getTranspositionIntervalObject(slf: &Bound<'_, Self>) -> PyResult<Option<Py<PyAny>>> {
        let py = slf.py();
        Self::value(slf)?
            .transposition()
            .map_err(|error| refusal(&error))?
            .map(|interval| crate::interval::interval_object(py, interval))
            .transpose()
    }

    /// music21's `getDivisionsPerQuarterNote`: worked out once, then kept.
    fn getDivisionsPerQuarterNote<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let kept = slf.getattr("_divisionsPerQuarterNote")?;
        if !kept.is_none() {
            return Ok(kept);
        }
        let part = Self::value(slf)?;
        let divisions = part.divisions().map_err(|error| refusal(&error))?;
        // music21 counts stage 2's in whole numbers.
        let made = if part.stage() == Stage::Two {
            (divisions as i64).into_pyobject(py)?.into_any()
        } else {
            divisions.into_pyobject(py)?.into_any()
        };
        slf.setattr("_divisionsPerQuarterNote", &made)?;
        Ok(made)
    }

    /// music21's `_getMeasureBoundaryIndices`: each measure's first and
    /// last line, of the part's lines or those given.
    #[pyo3(signature = (src = None))]
    fn _getMeasureBoundaryIndices(
        slf: &Bound<'_, Self>,
        src: Option<Vec<String>>,
    ) -> PyResult<Vec<(usize, usize)>> {
        let lines = match src {
            Some(given) => given,
            None => src_lines(slf.as_any())?,
        };
        let part = Part::with_stage(lines, stage_of(slf.as_any())?);
        let bounds = part.measure_bounds().map_err(|error| refusal(&error))?;
        Ok(bounds
            .into_iter()
            .map(|(start, end)| (start, end.saturating_sub(1)))
            .collect())
    }

    /// music21's `update`: the divisions and measure boundaries worked out.
    fn update(slf: &Bound<'_, Self>) -> PyResult<()> {
        let divisions = Self::getDivisionsPerQuarterNote(slf)?;
        slf.setattr("_divisionsPerQuarter", divisions)?;
        let bounds = Self::_getMeasureBoundaryIndices(slf, None)?;
        slf.setattr("_measureBoundaries", bounds)
    }

    /// The part's measures, made one at a time.
    fn __iter__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        slf.py().get_type::<MuseDataMeasureIterator>().call1((
            slf.getattr("src")?,
            slf.getattr("_measureBoundaries")?,
            slf,
        ))
    }

    /// music21's `getMeasures`.
    fn getMeasures<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let made = PyList::empty(slf.py());
        for measure in Self::__iter__(slf)?.try_iter()? {
            made.append(measure?)?;
        }
        Ok(made)
    }
}

// ------------------------------------------------------------------ files

/// music21's `MuseDataFile`: the parts of one file.
#[pyclass(module = "music21.musedata", extends = ProtoM21Object, subclass, dict)]
pub struct MuseDataFile;

#[pymethods]
impl MuseDataFile {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        let _ = (args, kwargs);
        base().add_subclass(Self)
    }

    fn __init__(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        slf.setattr("parts", PyList::empty(py))?;
        slf.setattr("filename", py.None())?;
        slf.setattr("file", py.None())?;
        slf.setattr("encoding", "utf-8")
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        crate::prebase::copied(slf.as_any(), None)
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        crate::prebase::copied(slf.as_any(), Some(memo))
    }

    fn _reprInternal(&self) -> &'static str {
        ""
    }

    /// music21's `open`: the file opened for reading as bytes.
    fn open(slf: &Bound<'_, Self>, fp: &Bound<'_, PyAny>) -> PyResult<()> {
        let file = slf
            .py()
            .import("builtins")?
            .getattr("open")?
            .call1((fp, "rb"))?;
        slf.setattr("file", file)?;
        slf.setattr("filename", fp)
    }

    /// music21's `read`: the file decoded in its encoding, or Latin-1, and
    /// read.
    fn read(slf: &Bound<'_, Self>) -> PyResult<()> {
        let contents = slf.getattr("file")?.call_method0("read")?;
        let encoding = slf.getattr("encoding")?;
        let text = match contents.call_method1("decode", (&encoding,)) {
            Ok(text) => text,
            Err(error)
                if error.is_instance_of::<pyo3::exceptions::PyUnicodeDecodeError>(slf.py()) =>
            {
                contents.call_method1("decode", ("ISO-8859-1", "ignore"))?
            }
            Err(error) => return Err(error),
        };
        Self::readstr(slf, text.extract()?)
    }

    /// music21's `close`.
    fn close(slf: &Bound<'_, Self>) -> PyResult<()> {
        slf.getattr("file")?.call_method0("close")?;
        Ok(())
    }

    /// music21's `readstr`: each part of the text read and kept.
    fn readstr(slf: &Bound<'_, Self>, input_str: &str) -> PyResult<()> {
        let py = slf.py();
        let parts = slf.getattr("parts")?;
        for read in rs::split_parts(input_str).map_err(|error| refusal(&error))? {
            let part = py
                .get_type::<MuseDataPart>()
                .call1((PyList::new(py, read.lines())?, stage_number(read.stage())))?;
            part.call_method0("update")?;
            parts.call_method1("append", (part,))?;
        }
        Ok(())
    }
}

/// music21's `MuseDataWork`: the files of a work.
#[pyclass(module = "music21.musedata", extends = ProtoM21Object, subclass, dict)]
pub struct MuseDataWork;

/// One path or several: music21's `common.isIterable`, which a string is
/// not.
fn several<'py>(given: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let iterable = given.hasattr("__iter__")? && !given.is_instance_of::<PyString>();
    if !iterable {
        return Ok(vec![given.clone()]);
    }
    given.try_iter()?.collect()
}

#[pymethods]
impl MuseDataWork {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        let _ = (args, kwargs);
        base().add_subclass(Self)
    }

    fn __init__(slf: &Bound<'_, Self>) -> PyResult<()> {
        slf.setattr("files", PyList::empty(slf.py()))?;
        slf.setattr("encoding", "utf-8")
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        crate::prebase::copied(slf.as_any(), None)
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        crate::prebase::copied(slf.as_any(), Some(memo))
    }

    fn _reprInternal(&self) -> &'static str {
        ""
    }

    /// music21's `addFile`: each file of a path or paths read.
    fn addFile(slf: &Bound<'_, Self>, fp: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        for path in several(fp)? {
            let file = py.get_type::<MuseDataFile>().call0()?;
            file.setattr("encoding", slf.getattr("encoding")?)?;
            file.call_method1("open", (path,))?;
            file.call_method0("read")?;
            file.call_method0("close")?;
            slf.getattr("files")?.call_method1("append", (file,))?;
        }
        Ok(())
    }

    /// music21's `addString`: each text read as a file.
    fn addString(slf: &Bound<'_, Self>, input_str: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        for text in several(input_str)? {
            let file = py.get_type::<MuseDataFile>().call0()?;
            file.call_method1("readstr", (text,))?;
            slf.getattr("files")?.call_method1("append", (file,))?;
        }
        Ok(())
    }

    /// music21's `getParts`: every part of every file.
    fn getParts<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let made = PyList::empty(slf.py());
        for file in slf.getattr("files")?.try_iter()? {
            for part in file?.getattr("parts")?.try_iter()? {
                made.append(part?)?;
            }
        }
        Ok(made)
    }
}

/// music21's `MuseDataDirectory`: the part files of a work in a directory,
/// or among a list of names such as a zip's.
#[pyclass(module = "music21.musedata", extends = ProtoM21Object, subclass, dict)]
pub struct MuseDataDirectory;

#[pymethods]
impl MuseDataDirectory {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        let _ = (args, kwargs);
        base().add_subclass(Self)
    }

    fn __init__(slf: &Bound<'_, Self>, dirOrList: &Bound<'_, PyAny>) -> PyResult<()> {
        slf.setattr("paths", PyList::empty(slf.py()))?;
        slf.setattr("groups", PyDict::new(slf.py()))?;
        Self::_prepareGroups(slf, dirOrList)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        crate::prebase::copied(slf.as_any(), None)
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        crate::prebase::copied(slf.as_any(), Some(memo))
    }

    fn _reprInternal(&self) -> &'static str {
        ""
    }

    /// music21's `_prepareGroups`: the part paths of a directory or list.
    fn _prepareGroups(slf: &Bound<'_, Self>, dirOrList: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let os = py.import("os")?;
        let path = os.getattr("path")?;
        let all: Vec<String> =
            if !dirOrList.is_instance_of::<PyString>() && dirOrList.hasattr("__iter__")? {
                dirOrList.extract()?
            } else if path.call_method1("isdir", (dirOrList,))?.is_truthy()? {
                let mut names: Vec<String> = os.call_method1("listdir", (dirOrList,))?.extract()?;
                names.sort();
                names
                    .into_iter()
                    .map(|name| path.call_method1("join", (dirOrList, name))?.extract())
                    .collect::<PyResult<_>>()?
            } else {
                return Err(MuseDataException::new_err(format!(
                    "cannot get files from the following entity {}",
                    dirOrList.repr()?
                )));
            };
        slf.setattr("paths", rs::part_paths(&all))
    }

    /// music21's `isMusedataFile`: by the name alone.
    fn isMusedataFile(&self, fp: &str) -> bool {
        rs::is_musedata_file(fp)
    }

    /// music21's `getPaths`: the sorted part paths.
    #[pyo3(signature = (group = None))]
    fn getPaths<'py>(
        slf: &Bound<'py, Self>,
        group: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let _ = group;
        slf.getattr("paths")
    }
}

/// The package music21 calls `musedata`, its submodules included.
pub(crate) fn package(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        m.add_class::<MuseDataRecord>()?;
        m.add_class::<MuseDataRecordIterator>()?;
        m.add_class::<MuseDataMeasure>()?;
        m.add_class::<MuseDataMeasureIterator>()?;
        m.add_class::<MuseDataPart>()?;
        m.add_class::<MuseDataFile>()?;
        m.add_class::<MuseDataWork>()?;
        m.add_class::<MuseDataDirectory>()?;
        let exception = py.get_type::<MuseDataException>();
        exception.setattr("__module__", MODULE)?;
        m.add("MuseDataException", exception)?;
        m.add("base40", crate::base40::base40_module(py)?)?;
        m.add("base12_26", crate::base40::base12_26_module(py)?)?;
        m.add("translate", crate::musedatatranslate::module(py)?)?;
        m.add(
            "environLocal",
            crate::environment::module(py)?
                .getattr("Environment")?
                .call1(("musedata",))?,
        )?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}
