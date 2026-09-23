//! music21's `instrument` module, over the crate's instruments.
//!
//! `Instrument` holds the crate's `Instrument`, and every one of music21's
//! instrument classes is a class of its own extending its family's, as in
//! music21, so `Clarinet()` starts out as music21's does and
//! `isinstance(clarinet, WoodwindInstrument)` holds. The classes are declared
//! in `instrument_kinds.rs`, generated from the crate's table of kinds.
//!
//! None of it is installed over music21. music21's instruments are held by
//! its streams and read by its MusicXML and MIDI writers through part ids,
//! instrument ids and sites this wheel does not model, so music21 keeps its
//! own inside music21 and these are for the wheel on its own.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyDict;

use music21_rs_crate::instrument::{
    Instrument as RsInstrument, SearchLanguage, ensemble_name_by_size,
};

use crate::interval::{interval_from_any, interval_object};
use crate::pitch::{Pitch, pitch_from_any};

pyo3::create_exception!(
    music21_rs_facade,
    InstrumentException,
    crate::Music21Exception
);

error_into!(instrument_error, InstrumentException);

/// music21's `instrument.Instrument`: one of music21's instruments, with
/// what an instrument of its class starts out as.
#[pyclass(
    name = "Instrument",
    module = "music21.instrument",
    subclass,
    skip_from_py_object
)]
pub struct Instrument {
    pub(crate) inner: RsInstrument,
    /// music21's `partId`, `instrumentId` and the two print flags: what a
    /// score writer reads and nothing here does.
    #[pyo3(get, set)]
    partId: Option<String>,
    #[pyo3(get, set)]
    instrumentId: Option<String>,
    #[pyo3(get, set)]
    printPartName: Option<bool>,
    #[pyo3(get, set)]
    printPartAbbreviation: Option<bool>,
}

impl Instrument {
    /// The initializer every instrument class builds on: an instrument of
    /// `kind`, named `name` where one is given.
    pub(crate) fn initializer(
        kind: &str,
        name: Option<String>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let mut inner = RsInstrument::of_kind(kind).map_err(instrument_error)?;
        if name.is_some() {
            inner.set_name(name);
        }
        Ok(PyClassInitializer::from(Self::wrap(inner)))
    }

    fn wrap(inner: RsInstrument) -> Self {
        Self {
            inner,
            partId: None,
            instrumentId: None,
            printPartName: None,
            printPartAbbreviation: None,
        }
    }

    /// An instrument of the wheel's class for its kind.
    fn object(py: Python<'_>, inner: RsInstrument) -> PyResult<Py<PyAny>> {
        let class = py
            .import("music21_rs")
            .or_else(|_| py.import("music21_rs_facade"))?
            .getattr(inner.kind())?;
        let made = class.call0()?;
        made.extract::<PyRefMut<'_, Self>>()?.inner = inner;
        Ok(made.unbind())
    }
}

/// Thirty-two random hex digits, as music21's `getMd5` gives for an id.
fn random_hex(py: Python<'_>) -> PyResult<String> {
    py.import("uuid")?
        .call_method0("uuid4")?
        .getattr("hex")?
        .extract()
}

fn language(value: Option<&str>) -> PyResult<SearchLanguage> {
    SearchLanguage::from_name(value.unwrap_or("all")).map_err(instrument_error)
}

#[pymethods]
impl Instrument {
    #[new]
    #[pyo3(signature = (instrumentName = None, **_keywords))]
    fn new(
        instrumentName: Option<String>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Self::initializer("Instrument", instrumentName)
    }

    /// The kind of instrument this is: music21's class name for it.
    #[getter]
    fn kind(&self) -> &str {
        self.inner.kind()
    }

    #[getter]
    fn get_instrumentName(&self) -> Option<&str> {
        self.inner.name()
    }

    #[setter]
    fn set_instrumentName(&mut self, value: Option<String>) {
        self.inner.set_name(value);
    }

    #[getter]
    fn get_instrumentAbbreviation(&self) -> Option<&str> {
        self.inner.abbreviation()
    }

    #[setter]
    fn set_instrumentAbbreviation(&mut self, value: Option<String>) {
        self.inner.set_abbreviation(value);
    }

    #[getter]
    fn get_partName(&self) -> Option<&str> {
        self.inner.part_name()
    }

    #[setter]
    fn set_partName(&mut self, value: Option<String>) {
        self.inner.set_part_name(value);
    }

    #[getter]
    fn get_partAbbreviation(&self) -> Option<&str> {
        self.inner.part_abbreviation()
    }

    #[setter]
    fn set_partAbbreviation(&mut self, value: Option<String>) {
        self.inner.set_part_abbreviation(value);
    }

    #[getter]
    fn get_instrumentSound(&self) -> Option<&str> {
        self.inner.sound()
    }

    #[setter]
    fn set_instrumentSound(&mut self, value: Option<String>) {
        self.inner.set_sound(value);
    }

    #[getter]
    fn get_midiProgram(&self) -> Option<u8> {
        self.inner.midi_program()
    }

    #[setter]
    fn set_midiProgram(&mut self, value: Option<u8>) {
        self.inner.set_midi_program(value);
    }

    #[getter]
    fn get_midiChannel(&self) -> Option<u8> {
        self.inner.midi_channel()
    }

    #[setter]
    fn set_midiChannel(&mut self, value: Option<u8>) {
        self.inner.set_midi_channel(value);
    }

    #[getter]
    fn get_lowestNote(&self) -> Option<Pitch> {
        self.inner
            .lowest()
            .map(|pitch| Pitch::wrap(pitch.clone(), false))
    }

    #[setter]
    fn set_lowestNote(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.inner.set_lowest(
            value
                .filter(|value| !value.is_none())
                .map(pitch_from_any)
                .transpose()?,
        );
        Ok(())
    }

    #[getter]
    fn get_highestNote(&self) -> Option<Pitch> {
        self.inner
            .highest()
            .map(|pitch| Pitch::wrap(pitch.clone(), false))
    }

    #[setter]
    fn set_highestNote(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.inner.set_highest(
            value
                .filter(|value| !value.is_none())
                .map(pitch_from_any)
                .transpose()?,
        );
        Ok(())
    }

    /// The interval from what is written to what sounds.
    #[getter]
    fn get_transposition(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.inner
            .transposition()
            .map(|interval| interval_object(py, interval.clone()))
            .transpose()
    }

    #[setter]
    fn set_transposition(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.inner.set_transposition(
            value
                .filter(|value| !value.is_none())
                .map(interval_from_any)
                .transpose()?,
        );
        Ok(())
    }

    #[getter]
    fn inGMPercMap(&self) -> bool {
        self.inner.in_percussion_map()
    }

    #[getter]
    fn percMapPitch(&self) -> Option<u8> {
        self.inner.percussion_pitch()
    }

    /// music21's `bestName`: the part's name, the part's abbreviation, the
    /// instrument's name or its abbreviation, the first given.
    fn bestName(&self) -> Option<&str> {
        self.inner.best_name()
    }

    /// music21's `partIdRandomize`: a part id no other part will have, `P`
    /// and thirty-two hex digits.
    fn partIdRandomize(&mut self, py: Python<'_>) -> PyResult<()> {
        self.partId = Some(format!("P{}", random_hex(py)?));
        Ok(())
    }

    /// music21's `instrumentIdRandomize`: the same for the instrument id,
    /// `I` and thirty-two hex digits.
    fn instrumentIdRandomize(&mut self, py: Python<'_>) -> PyResult<()> {
        self.instrumentId = Some(format!("I{}", random_hex(py)?));
        Ok(())
    }

    /// music21's `autoAssignMidiChannel`: takes a free channel and says which.
    #[pyo3(signature = (usedChannels, maxMidi = 16))]
    fn autoAssignMidiChannel(&mut self, usedChannels: Vec<u8>, maxMidi: u8) -> PyResult<u8> {
        self.inner
            .auto_assign_midi_channel(&usedChannels, maxMidi)
            .map_err(instrument_error)
    }

    fn __str__(&self) -> String {
        let mut written = String::new();
        if let Some(part) = &self.partId {
            written.push_str(part);
            written.push_str(": ");
        }
        written.push_str(&self.inner.to_string());
        written
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let class = slf.get_type().name()?;
        let written = slf.borrow().__str__();
        Ok(format!(
            "<music21.instrument.{class} {}>",
            pyo3::types::PyString::new(slf.py(), &written).repr()?
        ))
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.get_type().call0()?;
        {
            let me = slf.borrow();
            let mut copy = copied.extract::<PyRefMut<'_, Self>>()?;
            copy.inner = me.inner.clone();
            copy.partId = me.partId.clone();
            copy.instrumentId = me.instrumentId.clone();
            copy.printPartName = me.printPartName;
            copy.printPartAbbreviation = me.printPartAbbreviation;
        }
        Ok(copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::__deepcopy__(slf, None)
    }

    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        let py = slf.py();
        let extra = PyDict::new(py);
        {
            let me = slf.borrow();
            extra.set_item("partId", me.partId.as_deref())?;
            extra.set_item("instrumentId", me.instrumentId.as_deref())?;
        }
        crate::pickled_extra(slf, &slf.borrow().inner, Some(&extra))
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let (inner, extra) = crate::unpickled_extra::<_, RsInstrument>(slf, state)?;
        let mut me = slf.borrow_mut();
        if let Some(inner) = inner {
            me.inner = inner;
        }
        if let Some(extra) = extra {
            let extra = extra.bind(py);
            me.partId = extra
                .get_item("partId")
                .ok()
                .and_then(|value| value.extract().ok());
            me.instrumentId = extra
                .get_item("instrumentId")
                .ok()
                .and_then(|value| value.extract().ok());
        }
        Ok(())
    }
}

/// music21's `fromString`: the instrument a score's name for one means.
#[pyfunction]
#[pyo3(signature = (instrumentString, language = None))]
fn fromString(
    py: Python<'_>,
    instrumentString: &str,
    language: Option<&str>,
) -> PyResult<Py<PyAny>> {
    let found = RsInstrument::from_name(instrumentString, self::language(language)?)
        .map_err(instrument_error)?;
    Instrument::object(py, found)
}

/// music21's `instrumentFromMidiProgram`.
#[pyfunction]
fn instrumentFromMidiProgram(py: Python<'_>, number: u8) -> PyResult<Py<PyAny>> {
    let found = RsInstrument::from_midi_program(number).map_err(instrument_error)?;
    Instrument::object(py, found)
}

/// music21's `getAllNamesForInstrument`: every name a score may call this
/// kind of instrument, by language, found by the nearest kind in its family
/// that the name tables know.
#[pyfunction]
#[pyo3(signature = (instrumentClass, language = None))]
fn getAllNamesForInstrument<'py>(
    py: Python<'py>,
    instrumentClass: PyRef<'py, Instrument>,
    language: Option<&str>,
) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new(py);
    for (language, names) in instrumentClass.inner.all_names(self::language(language)?) {
        out.set_item(language.as_str(), names)?;
    }
    Ok(out)
}

/// music21's `ensembleNameBySize`: what so many players are called.
#[pyfunction]
fn ensembleNameBySize(number: usize) -> Option<&'static str> {
    ensemble_name_by_size(number)
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Instrument>()?;
    crate::instrument_kinds::register(m)?;
    m.add_function(wrap_pyfunction!(fromString, m)?)?;
    m.add_function(wrap_pyfunction!(instrumentFromMidiProgram, m)?)?;
    m.add_function(wrap_pyfunction!(getAllNamesForInstrument, m)?)?;
    m.add_function(wrap_pyfunction!(ensembleNameBySize, m)?)?;
    Ok(())
}
