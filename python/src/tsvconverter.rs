//! music21's `romanText.tsvConverter`: the Digital and Cognitive
//! Musicology Lab's tabular analyses read as roman numerals.
//!
//! Every reading here is the crate's `romantext::tsv`, which `tsv_parity`
//! holds to music21 on the four tables music21 ships and on all 68
//! RomanText analyses of the corpus written out in both of the lab's
//! standards: whether a key is minor, the swaps between the two notations,
//! the local key as a key of its own and as a degree of the global one,
//! the key a secondary numeral establishes, the form of a figure, the
//! tones a chord adds, and the chord a row stands for.
//!
//! **`TsvHandler` and `M21toTSV` stay music21's, and that is the harder
//! test.** They are the drivers: one reads a file and builds a music21
//! score out of the rows, the other reads a score and writes the rows
//! back, and both reach the crate through the classes and functions they
//! name -- `rebind` swaps those in the module they live in. So music21's
//! own `toM21Stream` example, which asks the built score for the figure of
//! the numeral in its first measure, runs as written.
//!
//! **A row says its label twice over.** music21's `_changeRepresentation`
//! writes the lab's spelling back onto the row as music21's -- the chord,
//! the degree, the form, the degree it is read in and the key in force --
//! and its own examples read those fields afterwards, so the facade asks
//! `TabChord::spelled` for all five and writes them back. `tabToM21` then
//! reads the row *as it now stands*, which is `element_as_spelled`; asking
//! for the lab's reading again would convert a version 2 label twice and
//! put a `d` in front of every seventh.
//!
//! **A row is read through whatever object holds it.** Every field is a
//! plain attribute, as music21's are -- its own `populateFromRow` writes
//! them with `setattr` -- so the facade reads them back with `getattr`,
//! which is what lets one reading serve both of the lab's standards and
//! music21's own classes besides. The lab renamed four columns between its
//! two standards, and `TabChordV2` answers the older names as music21
//! does, by reading the newer ones.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyDict;

use music21_rs_crate::FloatType;
use music21_rs_crate::Key as RsKey;
use music21_rs_crate::romantext::tsv::{
    Direction, TabChord as RsTabChord, Version, character_swaps, form_of, handle_added_tones,
    is_minor, local_key, local_key_as_roman, secondary_key,
};

/// The names this module hands out.
pub const NAMES: &[&str] = &[
    "TabChord",
    "TabChordV2",
    "isMinor",
    "characterSwaps",
    "getLocalKey",
    "localKeyAsRn",
    "getSecondaryKey",
    "getForm",
    "handleAddedTones",
];

pyo3::create_exception!(music21_rs_facade, TsvException, crate::Music21Exception);

error_into!(pub(crate) tsv_error, TsvException);

/// Which way music21's `direction` reads.
fn direction_of(direction: &str) -> PyResult<Direction> {
    match direction {
        "m21-DCML" => Ok(Direction::IntoDcml),
        "DCML-m21" => Ok(Direction::IntoMusic21),
        other => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "Direction must be 'm21-DCML' or 'DCML-m21', not {other}"
        ))),
    }
}

/// What a row's field says, or nothing where it says nothing.
fn said(object: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<String>> {
    let Ok(value) = object.getattr(name) else {
        return Ok(None);
    };
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(value.str()?.to_str()?.to_string()))
}

/// The same where music21 keeps a plain string.
fn text(object: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    Ok(said(object, name)?.unwrap_or_default())
}

/// A row read as the crate holds one, off whatever object holds its
/// fields.
fn crate_row(object: &Bound<'_, PyAny>) -> PyResult<RsTabChord> {
    let version = match object
        .getattr("dcml_version")
        .and_then(|value| value.extract::<u8>())
    {
        Ok(2) => Version::Two,
        _ => Version::One,
    };
    let number = |name: &str| -> PyResult<Option<FloatType>> {
        let Ok(value) = object.getattr(name) else {
            return Ok(None);
        };
        if value.is_none() {
            return Ok(None);
        }
        Ok(Some(value.extract::<FloatType>()?))
    };
    let mut extra = Vec::new();
    let held = object
        .getattr("extra")
        .ok()
        .and_then(|held| held.cast_into::<PyDict>().ok());
    if let Some(held) = held {
        for (name, value) in held.iter() {
            extra.push((
                name.str()?.to_str()?.to_string(),
                value.str()?.to_str()?.to_string(),
            ));
        }
    }
    Ok(RsTabChord {
        version,
        chord: text(object, "chord")?,
        numeral: text(object, "numeral")?,
        relativeroot: said(object, "relativeroot")?,
        form: said(object, "form")?,
        figbass: said(object, "figbass")?,
        changes: said(object, "changes")?,
        phraseend: said(object, "phraseend")?,
        pedal: said(object, "pedal")?,
        local_key: text(object, "local_key")?,
        global_key: text(object, "global_key")?,
        beat: number("beat")?.unwrap_or(1.0),
        measure: number("measure")?.unwrap_or(1.0) as i32,
        timesig: text(object, "timesig")?,
        volta: said(object, "volta")?.filter(|volta| !volta.is_empty()),
        length: number("length")?,
        extra,
    })
}

/// music21's `_changeRepresentation` for whichever class holds the row:
/// the lab's spelling written back as music21's, or the other way about.
fn change_representation(object: &Bound<'_, PyAny>) -> PyResult<()> {
    let direction = match said(object, "representationType")?.as_deref() {
        Some("m21") => {
            object.setattr("representationType", "DCML")?;
            "m21-DCML"
        }
        Some("DCML") => {
            object.setattr("representationType", "m21")?;
            "DCML-m21"
        }
        _ => {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "Data source must specify representation type as 'm21' or 'DCML'.",
            ));
        }
    };
    let row = crate_row(object)?;
    let spelled = row.spelled(direction_of(direction)?);
    object.setattr("local_key", spelled.local_key)?;
    object.setattr("numeral", spelled.numeral)?;
    if let Some(root) = spelled.relativeroot {
        object.setattr("relativeroot", root)?;
    }
    if direction == "DCML-m21" {
        object.setattr("form", spelled.form)?;
        if row.version == Version::Two {
            object.setattr("chord", spelled.chord)?;
        }
    }
    Ok(())
}

/// music21's `tabToM21`: the chord the row stands for, as a roman numeral
/// in the key in force.
fn tab_to_m21<'py>(object: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = object.py();
    if said(object, "representationType")?.as_deref() == Some("DCML") {
        // Through the object, so a subclass's own conversion is the one
        // that runs, as music21 calls its own.
        object.call_method0("_changeRepresentation")?;
    }
    let row = crate_row(object)?;
    let element = row.element_as_spelled().map_err(tsv_error)?;
    let made = crate::stream::element_object(py, &element)?
        .ok_or_else(|| TsvException::new_err("a row must stand for a chord or for no harmony"))?;
    let editorial = made.getattr("editorial")?;
    if row.version == Version::One {
        // music21 says these three of a version 1 row and not of a
        // version 2 one, whose tables carry none of them.
        for name in ["op", "no", "mov"] {
            let value = row
                .extra
                .iter()
                .find(|(held, _)| held == name)
                .map(|(_, value)| value.clone())
                .unwrap_or_default();
            editorial.setattr(name, value)?;
        }
    }
    editorial.setattr("pedal", row.pedal)?;
    editorial.setattr("phraseend", py.None())?;
    // How long the chord sounds is worked out by whatever holds it.
    made.setattr("quarterLength", 0.0)?;
    Ok(made)
}

/// music21's `populateFromRow`: each column the standard names written
/// onto the row, and the rest kept beside it.
fn populate_from_row(
    object: &Bound<'_, PyAny>,
    row: &Bound<'_, PyAny>,
    headIndices: &Bound<'_, PyAny>,
    extraIndices: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py = object.py();
    for pair in headIndices.call_method0("items")?.try_iter()? {
        let pair = pair?;
        let name = pair.get_item(0)?;
        let found = pair.get_item(1)?;
        let (at, coerce) = (found.get_item(0)?, found.get_item(1)?);
        let held = name.str()?;
        let held = held.to_str()?;
        if !object.hasattr(held).unwrap_or(false) {
            continue;
        }
        object.setattr(held, coerce.call1((row.get_item(at)?,))?)?;
    }
    let extra = PyDict::new(py);
    for pair in extraIndices.call_method0("items")?.try_iter()? {
        let pair = pair?;
        let at = pair.get_item(0)?;
        let name = pair.get_item(1)?;
        let cell = row.get_item(at)?;
        if cell.is_truthy()? {
            extra.set_item(name, cell)?;
        }
    }
    object.setattr("extra", extra)?;
    Ok(())
}

/// music21's `romanText.tsvConverter.TabChord`: one row of a table in the
/// lab's first standard.
#[pyclass(
    name = "TabChord",
    module = "music21.romanText.tsvConverter",
    subclass,
    skip_from_py_object,
    dict
)]
pub struct TabChord {
    /// The degree alone.
    #[pyo3(get, set)]
    numeral: Py<PyAny>,
    /// The degree the chord is read in, written after a slash.
    #[pyo3(get, set)]
    relativeroot: Py<PyAny>,
    /// Which of the two notations the row is spelled in.
    #[pyo3(get, set)]
    representationType: Py<PyAny>,
    /// Every column the standard does not name.
    #[pyo3(get, set)]
    extra: Py<PyAny>,
    /// Which of the lab's standards the row is written in.
    #[pyo3(get, set)]
    dcml_version: Py<PyAny>,
    /// The label the annotator wrote.
    #[pyo3(get, set)]
    chord: Py<PyAny>,
    /// The meter in force, as a ratio.
    #[pyo3(get, set)]
    timesig: Py<PyAny>,
    /// The note held under the chord.
    #[pyo3(get, set)]
    pedal: Py<PyAny>,
    /// How the chord is built over the degree.
    #[pyo3(get, set)]
    form: Py<PyAny>,
    /// The figured bass under it.
    #[pyo3(get, set)]
    figbass: Py<PyAny>,
    /// The tones it adds or leaves out.
    #[pyo3(get, set)]
    changes: Py<PyAny>,
    /// Whether a phrase ends here.
    #[pyo3(get, set)]
    phraseend: Py<PyAny>,
    /// The key in force.
    #[pyo3(get, set)]
    local_key: Py<PyAny>,
    /// The key the piece is in.
    #[pyo3(get, set)]
    global_key: Py<PyAny>,
    /// The beat of the measure the chord falls on, counted from one.
    #[pyo3(get, set)]
    beat: Py<PyAny>,
    /// The measure it falls in.
    #[pyo3(get, set)]
    measure: Py<PyAny>,
    /// The label the annotator wrote in an older column.
    #[pyo3(get, set)]
    altchord: Py<PyAny>,
    /// The beat counted from the start of the piece.
    #[pyo3(get, set)]
    totbeat: Py<PyAny>,
    /// How long the chord sounds, which the lab's first standard says.
    #[pyo3(get, set)]
    length: Py<PyAny>,
}

#[pymethods]
impl TabChord {
    #[new]
    fn new(py: Python<'_>) -> PyResult<Self> {
        let blank = || pyo3::types::PyString::new(py, "").into_any().unbind();
        Ok(Self {
            numeral: blank(),
            relativeroot: py.None(),
            representationType: py.None(),
            extra: PyDict::new(py).into_any().unbind(),
            dcml_version: 1_i64.into_pyobject(py)?.into_any().unbind(),
            chord: blank(),
            timesig: blank(),
            pedal: py.None(),
            form: py.None(),
            figbass: py.None(),
            changes: py.None(),
            phraseend: py.None(),
            local_key: blank(),
            global_key: blank(),
            beat: 1.0_f64.into_pyobject(py)?.into_any().unbind(),
            measure: 1_i64.into_pyobject(py)?.into_any().unbind(),
            altchord: py.None(),
            totbeat: py.None(),
            length: py.None(),
        })
    }

    /// The lab's own name for the whole label: music21's `combinedChord`,
    /// which is an alias for `chord`.
    #[getter]
    fn combinedChord(&self, py: Python<'_>) -> Py<PyAny> {
        self.chord.clone_ref(py)
    }

    #[setter]
    fn set_combinedChord(&mut self, value: Py<PyAny>) {
        self.chord = value;
    }

    /// music21's `_changeRepresentation`.
    fn _changeRepresentation(slf: &Bound<'_, Self>) -> PyResult<()> {
        change_representation(slf.as_any())
    }

    /// music21's `tabToM21`.
    fn tabToM21<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        tab_to_m21(slf.as_any())
    }

    /// music21's `populateFromRow`.
    fn populateFromRow(
        slf: &Bound<'_, Self>,
        row: &Bound<'_, PyAny>,
        headIndices: &Bound<'_, PyAny>,
        extraIndices: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        populate_from_row(slf.as_any(), row, headIndices, extraIndices)
    }
}

/// music21's `romanText.tsvConverter.TabChordV2`: one row of a table in
/// the lab's second standard, which renamed four of the columns.
#[pyclass(
    name = "TabChordV2",
    module = "music21.romanText.tsvConverter",
    subclass,
    skip_from_py_object,
    dict
)]
pub struct TabChordV2 {
    /// The degree alone.
    #[pyo3(get, set)]
    numeral: Py<PyAny>,
    /// The degree the chord is read in, written after a slash.
    #[pyo3(get, set)]
    relativeroot: Py<PyAny>,
    /// Which of the two notations the row is spelled in.
    #[pyo3(get, set)]
    representationType: Py<PyAny>,
    /// Every column the standard does not name.
    #[pyo3(get, set)]
    extra: Py<PyAny>,
    /// Which of the lab's standards the row is written in.
    #[pyo3(get, set)]
    dcml_version: Py<PyAny>,
    /// The label the annotator wrote.
    #[pyo3(get, set)]
    chord: Py<PyAny>,
    /// The meter in force, as a ratio.
    #[pyo3(get, set)]
    timesig: Py<PyAny>,
    /// The note held under the chord.
    #[pyo3(get, set)]
    pedal: Py<PyAny>,
    /// How the chord is built over the degree.
    #[pyo3(get, set)]
    form: Py<PyAny>,
    /// The figured bass under it.
    #[pyo3(get, set)]
    figbass: Py<PyAny>,
    /// The tones it adds or leaves out.
    #[pyo3(get, set)]
    changes: Py<PyAny>,
    /// Whether a phrase ends here.
    #[pyo3(get, set)]
    phraseend: Py<PyAny>,
    /// The measure the chord falls in, as this standard names it.
    #[pyo3(get, set)]
    mn: Py<PyAny>,
    /// Where in the measure it falls, in whole notes from nought.
    #[pyo3(get, set)]
    mn_onset: Py<PyAny>,
    /// Which time through a repeat the row belongs to.
    #[pyo3(get, set)]
    volta: Py<PyAny>,
    /// The key the piece is in, as this standard names it.
    #[pyo3(get, set)]
    globalkey: Py<PyAny>,
    /// The key in force, likewise.
    #[pyo3(get, set)]
    localkey: Py<PyAny>,
}

#[pymethods]
impl TabChordV2 {
    #[new]
    fn new(py: Python<'_>) -> PyResult<Self> {
        let blank = || pyo3::types::PyString::new(py, "").into_any().unbind();
        Ok(Self {
            numeral: blank(),
            relativeroot: py.None(),
            representationType: py.None(),
            extra: PyDict::new(py).into_any().unbind(),
            dcml_version: 2_i64.into_pyobject(py)?.into_any().unbind(),
            chord: blank(),
            timesig: blank(),
            pedal: py.None(),
            form: py.None(),
            figbass: py.None(),
            changes: py.None(),
            phraseend: py.None(),
            mn: 0_i64.into_pyobject(py)?.into_any().unbind(),
            mn_onset: 0.0_f64.into_pyobject(py)?.into_any().unbind(),
            volta: blank(),
            globalkey: blank(),
            localkey: blank(),
        })
    }

    /// The lab's own name for the whole label: music21's `combinedChord`.
    #[getter]
    fn combinedChord(&self, py: Python<'_>) -> Py<PyAny> {
        self.chord.clone_ref(py)
    }

    #[setter]
    fn set_combinedChord(&mut self, value: Py<PyAny>) {
        self.chord = value;
    }

    /// The beat of the measure the chord falls on, counted from one:
    /// music21's `beat`, which this standard writes as `mn_onset` in whole
    /// notes from nought.
    #[getter]
    fn beat(&self, py: Python<'_>) -> PyResult<FloatType> {
        Ok(self.mn_onset.bind(py).extract::<FloatType>()? * 4.0 + 1.0)
    }

    #[setter]
    fn set_beat(&mut self, py: Python<'_>, beat: Option<FloatType>) -> PyResult<()> {
        self.mn_onset = match beat {
            Some(beat) => ((beat - 1.0) / 4.0).into_pyobject(py)?.into_any().unbind(),
            None => py.None(),
        };
        Ok(())
    }

    /// The measure it falls in: music21's `measure`, which this standard
    /// writes as `mn`.
    #[getter]
    fn measure(&self, py: Python<'_>) -> PyResult<i64> {
        self.mn.bind(py).extract::<FloatType>().map(|mn| mn as i64)
    }

    #[setter]
    fn set_measure(&mut self, py: Python<'_>, measure: Option<FloatType>) -> PyResult<()> {
        self.mn = match measure {
            Some(measure) => (measure as i64).into_pyobject(py)?.into_any().unbind(),
            None => py.None(),
        };
        Ok(())
    }

    /// The key in force: music21's `local_key`, written `localkey` here.
    #[getter]
    fn local_key(&self, py: Python<'_>) -> Py<PyAny> {
        self.localkey.clone_ref(py)
    }

    #[setter]
    fn set_local_key(&mut self, value: Py<PyAny>) {
        self.localkey = value;
    }

    /// The key the piece is in: music21's `global_key`, written
    /// `globalkey` here.
    #[getter]
    fn global_key(&self, py: Python<'_>) -> Py<PyAny> {
        self.globalkey.clone_ref(py)
    }

    #[setter]
    fn set_global_key(&mut self, value: Py<PyAny>) {
        self.globalkey = value;
    }

    /// music21's `_changeRepresentation`.
    fn _changeRepresentation(slf: &Bound<'_, Self>) -> PyResult<()> {
        change_representation(slf.as_any())
    }

    /// music21's `tabToM21`.
    fn tabToM21<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        tab_to_m21(slf.as_any())
    }

    /// music21's `populateFromRow`.
    fn populateFromRow(
        slf: &Bound<'_, Self>,
        row: &Bound<'_, PyAny>,
        headIndices: &Bound<'_, PyAny>,
        extraIndices: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        populate_from_row(slf.as_any(), row, headIndices, extraIndices)
    }
}

/// music21's `isMinor`: whether a key is written in lower case, which is
/// how the lab says minor.
#[pyfunction]
#[pyo3(name = "isMinor")]
pub fn is_minor_py(test_key: &str) -> bool {
    is_minor(test_key)
}

/// music21's `characterSwaps`: the swaps between the lab's notation and
/// music21's figures.
///
/// `minor` is music21's own argument, which none of its branches reads.
#[pyfunction]
#[pyo3(name = "characterSwaps")]
#[pyo3(signature = (preString, minor = true, direction = "m21-DCML"))]
pub fn character_swaps_py(preString: &str, minor: bool, direction: &str) -> PyResult<String> {
    let _ = minor;
    Ok(character_swaps(preString, direction_of(direction)?))
}

/// music21's `getLocalKey`: the local key as a key of its own, read as a
/// degree of the global one.
#[pyfunction]
#[pyo3(name = "getLocalKey")]
#[pyo3(signature = (local_key, global_key, convertDCMLToM21 = false))]
pub fn get_local_key_py(
    local_key: &str,
    global_key: &str,
    convertDCMLToM21: bool,
) -> PyResult<String> {
    local_key_py(local_key, global_key, convertDCMLToM21)
}

/// The crate's reading of it.
fn local_key_py(local: &str, global: &str, from_dcml: bool) -> PyResult<String> {
    local_key(local, global, from_dcml).map_err(tsv_error)
}

/// music21's `localKeyAsRn`: the local key as a degree of the global one.
#[pyfunction]
#[pyo3(name = "localKeyAsRn")]
pub fn local_key_as_rn_py(
    local_key: &Bound<'_, PyAny>,
    global_key: &Bound<'_, PyAny>,
) -> PyResult<String> {
    local_key_as_roman(&key_read(local_key)?, &key_read(global_key)?).map_err(tsv_error)
}

/// A key read as the crate holds one, off whatever object says it.
fn key_read(object: &Bound<'_, PyAny>) -> PyResult<RsKey> {
    let named = match object.getattr("tonicPitchNameWithCase") {
        Ok(name) => name.str()?.to_str()?.to_string(),
        Err(_) => object.str()?.to_str()?.to_string(),
    };
    RsKey::from_tonic(&named).map_err(tsv_error)
}

/// music21's `getSecondaryKey`: the key a secondary numeral establishes,
/// read in the key in force.
#[pyfunction]
#[pyo3(name = "getSecondaryKey")]
pub fn get_secondary_key_py(rn: &str, local_key: &str) -> PyResult<String> {
    secondary_key(rn, local_key).map_err(tsv_error)
}

/// music21's `getForm`: how a chord is built over its degree, as the lab
/// writes it.
#[pyfunction]
#[pyo3(name = "getForm")]
pub fn get_form_py(rn: &Bound<'_, PyAny>) -> PyResult<&'static str> {
    let figure = rn.getattr("figure")?;
    Ok(form_of(figure.str()?.to_str()?))
}

/// music21's `handleAddedTones`: the tones a label adds or leaves out,
/// written as music21 writes them.
#[pyfunction]
#[pyo3(name = "handleAddedTones")]
pub fn handle_added_tones_py(dcmlChord: &str) -> String {
    handle_added_tones(dcmlChord)
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<TabChord>()?;
    m.add_class::<TabChordV2>()?;
    m.add_function(wrap_pyfunction!(is_minor_py, m)?)?;
    m.add_function(wrap_pyfunction!(character_swaps_py, m)?)?;
    m.add_function(wrap_pyfunction!(get_local_key_py, m)?)?;
    m.add_function(wrap_pyfunction!(local_key_as_rn_py, m)?)?;
    m.add_function(wrap_pyfunction!(get_secondary_key_py, m)?)?;
    m.add_function(wrap_pyfunction!(get_form_py, m)?)?;
    m.add_function(wrap_pyfunction!(handle_added_tones_py, m)?)?;
    Ok(())
}
