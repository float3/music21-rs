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

use std::sync::atomic::{AtomicU64, Ordering};

use pyo3::prelude::*;

use crate::Walkable;
use pyo3::types::PyDict;

use music21_rs_crate::instrument::{
    Instrument as RsInstrument, SearchLanguage, ensemble_name_by_size,
};

use crate::interval::{interval_from_any, interval_object};
use crate::pitch::{Pitch, pitch_from_any};

pub use crate::instrument_kinds::NAMES;

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
    /// music21 defines some members on one family alone, so an instrument
    /// outside it has no such attribute at all.
    fn member_of(&self, family: &str, member: &str) -> PyResult<()> {
        if self.inner.is_a(family) {
            return Ok(());
        }
        Err(pyo3::exceptions::PyAttributeError::new_err(format!(
            "'{}' object has no attribute '{member}'",
            self.inner.kind()
        )))
    }

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
    pub(crate) fn object(py: Python<'_>, inner: RsInstrument) -> PyResult<Py<PyAny>> {
        // The class installed over music21's where there is one, since
        // music21 holds nothing else in a stream; this wheel's own where not.
        Self::object_as(py, inner, false)
    }

    /// The same, as a class music21's own streams will hold where asked
    /// for one -- the builder of music21's streams and nothing else, for
    /// the reason `clef::Clef::object_as` carries.
    pub(crate) fn object_as(
        py: Python<'_>,
        inner: RsInstrument,
        holdable: bool,
    ) -> PyResult<Py<PyAny>> {
        let found = if holdable {
            crate::holdable_class(py, "music21.instrument", inner.kind())
        } else {
            crate::installed_class(py, "music21.instrument", inner.kind())
        };
        let class = match found {
            Some(class) => class,
            None => py
                .import("music21_rs")
                .or_else(|_| py.import("music21_rs_facade"))?
                .getattr(inner.kind())?,
        };
        let made = class.call0()?;
        made.extract::<PyRefMut<'_, Self>>()?.inner = inner;
        Ok(made.unbind())
    }
}

/// How many ids `fresh_id` has handed out in this process.
static IDS_GIVEN: AtomicU64 = AtomicU64::new(0);

/// The first half of every id this process hands out, drawn once.
static PROCESS_PREFIX: std::sync::OnceLock<u64> = std::sync::OnceLock::new();

/// The next id of this process: `prefix` and thirty-two hex digits, the
/// shape of music21's `getMd5` ids. The first sixteen are drawn once per
/// process and the last sixteen count up from one, so no two calls in a
/// process answer the same id, and two processes -- workers each writing
/// a score -- do not both name their first part the same.
fn fresh_id(prefix: char) -> String {
    let process = *PROCESS_PREFIX.get_or_init(drawn_prefix);
    let number = IDS_GIVEN.fetch_add(1, Ordering::Relaxed) + 1;
    format!("{prefix}{process:016x}{number:016x}")
}

/// Sixteen hex digits no other process is likely to draw: the standard
/// library's per-process random hashing keys, over the process id and the
/// time.
fn drawn_prefix() -> u64 {
    use std::hash::{BuildHasher, Hasher};

    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u32(std::process::id());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_nanos())
        .unwrap_or_default();
    hasher.write_u128(now);
    hasher.finish()
}

fn language(value: Option<&str>) -> PyResult<SearchLanguage> {
    SearchLanguage::from_name(value.unwrap_or("all")).map_err(instrument_error)
}

#[pymethods]
impl Instrument {
    /// Refuses what music21 answers through a site, which the package
    /// keeps none of.
    fn __getattr__(slf: &Bound<'_, Self>, name: &str) -> PyResult<Py<PyAny>> {
        crate::stream::no_site(slf.as_any(), name)
    }

    /// What music21 says this is: its own class and every class it is a
    /// kind of, as music21's `classes` answers -- so a note is a
    /// `NotRest`, a `GeneralNote` and a `Music21Object` besides.
    #[getter]
    fn classes(slf: &Bound<'_, Self>) -> PyResult<Vec<String>> {
        crate::stream::lineage_of(slf.as_any())
    }
    /// music21's sort order for an instrument among things at one offset:
    /// before everything else, the part being for whoever plays it.
    #[classattr]
    fn classSortOrder() -> i32 {
        -25
    }

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
    fn get_inGMPercMap(&self) -> bool {
        self.inner.in_percussion_map()
    }

    #[setter]
    fn set_inGMPercMap(&mut self, #[pyo3(from_py_with = crate::truthy)] value: bool) {
        self.inner.set_in_percussion_map(value);
    }

    /// music21's `percMapPitch`, which `MidiPercussionMapMixin` carries --
    /// every unpitched percussion instrument and a whistle, which is a
    /// flute in the General MIDI drum map.
    #[getter]
    fn get_percMapPitch(&self) -> PyResult<Option<u8>> {
        self.member_of("MidiPercussionMapMixin", "percMapPitch")?;
        Ok(self.inner.percussion_pitch())
    }

    #[setter]
    fn set_percMapPitch(&mut self, value: Option<u8>) -> PyResult<()> {
        self.member_of("MidiPercussionMapMixin", "percMapPitch")?;
        self.inner.set_percussion_pitch(value);
        Ok(())
    }

    /// music21's `muted`, which three of its percussion classes carry: a
    /// conga drum, a cuica and a surdo. Muting is the *drum* played rather
    /// than a flag beside it -- a muted conga drum is key 62 where an open
    /// one is 63 -- so this reads the key.
    ///
    /// Asked of anything else it raises, as music21 does: the property is
    /// defined on those three classes and nowhere above them.
    #[getter]
    fn get_muted(slf: &Bound<'_, Self>) -> PyResult<bool> {
        slf.borrow().inner.muted().ok_or_else(|| {
            pyo3::exceptions::PyAttributeError::new_err(format!(
                "'{}' object has no attribute 'muted'",
                slf.get_type()
                    .name()
                    .map(|name| name.to_string())
                    .unwrap_or_else(|_| "Instrument".to_string())
            ))
        })
    }

    #[setter]
    fn set_muted(
        slf: &Bound<'_, Self>,
        #[pyo3(from_py_with = crate::truthy)] value: bool,
    ) -> PyResult<()> {
        // Read first, so that anything the property is not defined on
        // raises rather than quietly doing nothing.
        let _ = Self::get_muted(slf)?;
        slf.borrow_mut().inner.set_muted(value);
        Ok(())
    }

    /// music21's `stringPitches`, which only its string instruments have.
    #[getter]
    fn get_stringPitches(&self) -> PyResult<Vec<Pitch>> {
        self.member_of("StringInstrument", "stringPitches")?;
        let pitches = self.inner.string_pitches().ok_or_else(|| {
            InstrumentException::new_err("cannot get stringPitches for these instruments")
        })?;
        Ok(pitches
            .iter()
            .map(|pitch| Pitch::wrap(pitch.clone(), false))
            .collect())
    }

    /// Takes pitches or their names, as music21's setter does.
    #[setter]
    fn set_stringPitches(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.member_of("StringInstrument", "stringPitches")?;
        let mut pitches = Vec::new();
        for item in value.walk()? {
            pitches.push(pitch_from_any(&item?)?);
        }
        self.inner.set_string_pitches(pitches);
        Ok(())
    }

    /// music21's `modifier`, which `MidiPercussionMapMixin` has: every
    /// unpitched percussion instrument, and a whistle besides, the map
    /// being a synthesizer's layout rather than a family of instruments.
    #[getter]
    fn get_modifier(&self) -> PyResult<Option<&str>> {
        self.member_of("MidiPercussionMapMixin", "modifier")?;
        Ok(self.inner.modifier())
    }

    #[setter]
    fn set_modifier(&mut self, modifier: &str) -> PyResult<()> {
        self.member_of("MidiPercussionMapMixin", "modifier")?;
        self.inner.set_modifier(modifier);
        Ok(())
    }

    /// music21's `headless`, which only its `Tambourine` has: whether it is
    /// the rock and pop kind, a ring of jingles with no drum head.
    #[getter]
    fn get_headless(&self) -> PyResult<bool> {
        self.member_of("Tambourine", "headless")?;
        Ok(self.inner.headless().unwrap_or(false))
    }

    #[setter]
    fn set_headless(&mut self, #[pyo3(from_py_with = crate::truthy)] value: bool) -> PyResult<()> {
        self.member_of("Tambourine", "headless")?;
        self.inner.set_headless(value);
        Ok(())
    }

    /// music21's `pitched`, which only its `Whistle` has: whether it sounds
    /// a definite pitch, as a tin whistle does and a referee's does not.
    #[getter]
    fn get_pitched(&self) -> PyResult<bool> {
        self.member_of("Whistle", "pitched")?;
        Ok(self.inner.pitched().unwrap_or(false))
    }

    #[setter]
    fn set_pitched(&mut self, #[pyo3(from_py_with = crate::truthy)] value: bool) -> PyResult<()> {
        self.member_of("Whistle", "pitched")?;
        self.inner.set_pitched(value);
        Ok(())
    }

    /// music21's `bestName`: the part's name, the part's abbreviation, the
    /// instrument's name or its abbreviation, the first given.
    fn bestName(&self) -> Option<&str> {
        self.inner.best_name()
    }

    /// music21's `partIdRandomize`: a part id no other one given out in
    /// this process has, `P` and thirty-two hex digits. The ids are counted
    /// rather than drawn at random, so a program gives the same ids on every
    /// run.
    fn partIdRandomize(&mut self) {
        self.partId = Some(fresh_id('P'));
    }

    /// music21's `instrumentIdRandomize`: the same for the instrument id,
    /// `I` and thirty-two hex digits, counted the same way.
    fn instrumentIdRandomize(&mut self) {
        self.instrumentId = Some(fresh_id('I'));
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
        // music21's `MidiPercussionMapMixin._reprInternal` writes the
        // modifier after the name, so a bongo drum reads
        // `<music21.instrument.BongoDrums 'Bongo Drums' high>` -- which is
        // how a caller sees *which* drum of the pair it is.
        let modifier = slf.borrow().inner.modifier().map(str::to_string);
        Ok(format!(
            "<music21.instrument.{class} {}{}>",
            pyo3::types::PyString::new(slf.py(), &written).repr()?,
            modifier.map_or_else(String::new, |modifier| format!(" {modifier}"))
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

/// What a note's `storedInstrument` setter takes: nothing, or an instrument,
/// kept as the object it is, with the crate's value where the object is one
/// of ours. Anything else is music21's `TypeError`.
pub(crate) fn stored_instrument(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<(Option<Py<PyAny>>, Option<RsInstrument>)> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok((None, None));
    };
    if let Ok(ours) = value.extract::<PyRef<'_, Instrument>>() {
        return Ok((Some(value.clone().unbind()), Some(ours.inner.clone())));
    }
    let named = value
        .getattr("classSet")
        .and_then(|classes| classes.contains("music21.instrument.Instrument"))
        .unwrap_or(false);
    if !named {
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "Expected Instrument; got {}",
            value.get_type().repr()?
        )));
    }
    Ok((Some(value.clone().unbind()), None))
}

/// music21's `instrumentFromMidiProgram`.
#[pyfunction]
fn instrumentFromMidiProgram(py: Python<'_>, number: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    // music21 looks the number up and only then asks what it was, so any
    // integer gets as far as not being found.
    let Ok(whole) = number.extract::<i64>() else {
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "Expected int, got {}",
            number.get_type().repr()?
        )));
    };
    let program = u8::try_from(whole).map_err(|_| {
        InstrumentException::new_err(format!("No instrument found for MIDI program {whole}"))
    })?;
    let found = RsInstrument::from_midi_program(program).map_err(instrument_error)?;
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

/// music21's `unbundleInstruments`: each note's stored instrument moved
/// out of the note and into the stream beside it.
///
/// The answer is a new stream, as every other edit in this package is, so
/// `inPlace=True` is refused -- music21 changes the stream it is given
/// there.
#[pyfunction]
#[pyo3(signature = (streamIn, *, inPlace = false))]
fn unbundleInstruments<'py>(
    streamIn: &Bound<'py, PyAny>,
    #[pyo3(from_py_with = crate::truthy)] inPlace: bool,
) -> PyResult<Bound<'py, PyAny>> {
    crate::stream::changed(streamIn, inPlace, |stream| {
        music21_rs_crate::instrument::unbundle_instruments(stream);
        Ok(())
    })
}

/// music21's `bundleInstruments`: an instrument standing in the stream
/// moved onto the notes that sound it.
#[pyfunction]
#[pyo3(signature = (streamIn, *, inPlace = false))]
fn bundleInstruments<'py>(
    streamIn: &Bound<'py, PyAny>,
    #[pyo3(from_py_with = crate::truthy)] inPlace: bool,
) -> PyResult<Bound<'py, PyAny>> {
    crate::stream::changed(streamIn, inPlace, |stream| {
        music21_rs_crate::instrument::bundle_instruments(stream);
        Ok(())
    })
}

/// music21's `deduplicate`: the instruments standing at one offset settled
/// into one, so a track name and a program change read together are one
/// instrument.
#[pyfunction]
#[pyo3(signature = (s, *, inPlace = false))]
fn deduplicate<'py>(
    s: &Bound<'py, PyAny>,
    #[pyo3(from_py_with = crate::truthy)] inPlace: bool,
) -> PyResult<Bound<'py, PyAny>> {
    crate::stream::changed(s, inPlace, |stream| {
        music21_rs_crate::instrument::deduplicate(stream);
        Ok(())
    })
}

/// music21's `partitionByInstrument`: a score of one part per instrument,
/// each holding what that instrument plays.
#[pyfunction]
fn partitionByInstrument<'py>(streamObj: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let read = crate::stream::crate_stream(streamObj)?;
    crate::stream::from_crate(
        streamObj.py(),
        &music21_rs_crate::instrument::partition_by_instrument(&read),
    )
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Instrument>()?;
    crate::instrument_kinds::register(m)?;
    m.add_function(wrap_pyfunction!(fromString, m)?)?;
    m.add_function(wrap_pyfunction!(instrumentFromMidiProgram, m)?)?;
    m.add_function(wrap_pyfunction!(getAllNamesForInstrument, m)?)?;
    m.add_function(wrap_pyfunction!(ensembleNameBySize, m)?)?;
    m.add_function(wrap_pyfunction!(unbundleInstruments, m)?)?;
    m.add_function(wrap_pyfunction!(bundleInstruments, m)?)?;
    m.add_function(wrap_pyfunction!(deduplicate, m)?)?;
    m.add_function(wrap_pyfunction!(partitionByInstrument, m)?)?;
    Ok(())
}
