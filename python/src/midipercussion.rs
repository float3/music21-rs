//! music21's `midi.percussion`: which drum a MIDI key plays, and which
//! key plays a drum.
//!
//! Both are the crate's: `instrument::for_percussion_key` names the drum
//! a key plays *and how it is played*, since a key says both -- key 36 is
//! a bass drum played `midi-1` and key 35 the same drum played `acoustic`
//! -- and `Instrument::percussion_pitch` is the key a drum is struck on.
//! `PercussionMapper` keeps no state, as music21's does not either.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;

use music21_rs_crate::instrument;

pyo3::create_exception!(
    music21_rs_facade,
    MIDIPercussionException,
    crate::Music21Exception
);

/// The names this module hands out.
pub const NAMES: &[&str] = &["PercussionMapper", "MIDIPercussionException"];

/// music21's `midi.percussion.PercussionMapper`: the General MIDI
/// percussion map, read either way.
#[pyclass(
    name = "PercussionMapper",
    module = "music21.midi.percussion",
    subclass,
    skip_from_py_object
)]
pub struct PercussionMapper;

#[pymethods]
impl PercussionMapper {
    #[new]
    fn new() -> Self {
        Self
    }

    /// A mapper holds nothing of its own, so a copy is a new one of the
    /// same class -- which is also what the class installed over music21's
    /// asks for when music21 copies it.
    #[pyo3(signature = (_memo = None))]
    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        slf.get_type().call0()
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        slf.get_type().call0()
    }

    /// Every key the map names a drum for, each against the class of
    /// that drum: music21's `reverseInstrumentMapping`.
    ///
    /// It is read off `instrument::percussion_keys`, the one table behind
    /// both answers, rather than written out again -- so a key the crate
    /// names no instrument for is missing here, as it is there. The nine
    /// sounds the map names but has no class for are
    /// `namedSoundMapping`'s and not here, as music21 keeps them apart.
    #[getter]
    fn reverseInstrumentMapping<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, pyo3::types::PyDict>> {
        let out = pyo3::types::PyDict::new(py);
        for key in instrument::percussion_keys() {
            let Ok(named) = instrument::for_percussion_key(key) else {
                continue;
            };
            let class = match crate::installed_class(py, "music21.instrument", named.kind()) {
                Some(class) => class,
                None => py
                    .import("music21_rs")
                    .or_else(|_| py.import("music21_rs_facade"))?
                    .getattr(named.kind())?,
            };
            out.set_item(key, class)?;
        }
        Ok(out)
    }

    /// The General MIDI sounds that are no instrument, each against the
    /// name music21 gives it: music21's `namedSoundMapping`, read off
    /// `instrument::named_percussion_sounds`.
    #[getter]
    fn namedSoundMapping<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, pyo3::types::PyDict>> {
        let out = pyo3::types::PyDict::new(py);
        for (key, name) in instrument::named_percussion_sounds() {
            out.set_item(key, name)?;
        }
        Ok(out)
    }

    /// The drum a key plays, and how it is played: music21's
    /// `midiPitchToInstrument`, which takes the key as a number or as a
    /// pitch. A key the map names no instrument and no sound for is
    /// refused in music21's own words, unless `raiseIfUnmapped` is false,
    /// which answers a bare unpitched percussion instrument keeping the
    /// key.
    #[pyo3(signature = (midiPitch, *, raiseIfUnmapped = true))]
    fn midiPitchToInstrument<'py>(
        &self,
        py: Python<'py>,
        midiPitch: &Bound<'py, PyAny>,
        #[pyo3(from_py_with = crate::truthy)] raiseIfUnmapped: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let key: i64 = match midiPitch.extract::<i64>() {
            Ok(key) => key,
            // A pitch, whose `midi` is the key it is struck on.
            Err(_) => midiPitch.getattr("midi")?.extract()?,
        };
        let within = u8::try_from(key).ok();
        let named = match within {
            Some(key) if !raiseIfUnmapped => Some(instrument::for_percussion_key_or_unmapped(key)),
            Some(key) => instrument::for_percussion_key(key).ok(),
            None => None,
        };
        let named = named.ok_or_else(|| {
            MIDIPercussionException::new_err(format!("{key} does not map to a valid instrument!"))
        })?;
        Ok(crate::instrument::Instrument::object(py, named)?.into_bound(py))
    }

    /// The key a drum is struck on: music21's `midiInstrumentToPitch`,
    /// which refuses an instrument the map has no key for.
    fn midiInstrumentToPitch<'py>(
        &self,
        py: Python<'py>,
        midiInstrument: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        // The instrument as a value of the crate's, whether the object is
        // one of this package's or music21's own.
        let (_, read) = crate::instrument::stored_instrument(Some(midiInstrument))?;
        let read = read.ok_or_else(|| {
            MIDIPercussionException::new_err("nothing was given to look up".to_string())
        })?;
        let Some(key) = read.percussion_pitch() else {
            return Err(MIDIPercussionException::new_err(format!(
                "{} is not in the GM Percussion Map!",
                midiInstrument.repr()?
            )));
        };
        let pitch = music21_rs_crate::Pitch::from_midi(i32::from(key))
            .map_err(|error| MIDIPercussionException::new_err(crate::pitch::message(&error)))?;
        Ok(crate::installed_new(
            py,
            "music21.pitch",
            "Pitch",
            crate::pitch::Pitch::wrap(pitch, true),
        )?
        .into_bound(py)
        .into_any())
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PercussionMapper>()?;
    Ok(())
}
