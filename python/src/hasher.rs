//! music21's `alpha.analysis.hasher`: a stream's notes hashed into
//! comparable tuples.
//!
//! The hashing is the crate's `alpha::analysis::hasher`, which
//! `alpha_parity` holds to music21 note for note; the facade reads the
//! stream, asks it, and hands back music21's own `NoteHash` and
//! `NoteHashWithReference` -- which are a tuple and a wrapper round one,
//! so music21's own are the better ones wherever music21 can be asked, and
//! a `namedtuple` of the same names stands in where it cannot.
//!
//! **A hash says what it was made from, and that is the score's own
//! object.** music21 references the element hashed -- a note, a rest, a
//! chord hashed as one, or each *member* of a chord hashed as notes -- so
//! the facade walks the stream the way music21's own `hashStream` walks it
//! and pairs the objects with the crate's hashes, which come in that same
//! order. A count that does not match is refused rather than paired
//! wrongly.
//!
//! music21's own private hashing functions (`_hashMIDIPitchName`,
//! `_hashRoundedDuration` and the rest) are left to music21, which is
//! where its own examples call them: a private member is not blocked, so
//! they run over this object, reading the settings it carries.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

use music21_rs_crate::alpha::analysis::hasher::{HashValue, Hasher as RsHasher, NoteHash};

use crate::Walkable;

/// The names this module hands out.
pub const NAMES: &[&str] = &["Hasher"];

/// A hash with what it was made from, where music21 cannot be asked for
/// its own class: a tuple of the hashed values with the element beside it.
const WITH_REFERENCE: &str = r#"
class NoteHashWithReference:
    """music21's `NoteHashWithReference`, for a wheel with no music21."""

    def __init__(self, hashed):
        self._hashed = hashed
        self.reference = None

    def __getattr__(self, name):
        return getattr(self._hashed, name)

    def __iter__(self):
        return iter(self._hashed)

    def __len__(self):
        return len(self._hashed)

    def __getitem__(self, at):
        return self._hashed[at]

    def __eq__(self, other):
        return tuple(self) == tuple(other)

    def __repr__(self):
        parts = ', '.join(
            f'{name}={value}' for name, value in zip(self._hashed._fields, self._hashed)
        )
        return f'NoteHashWithReference({parts})'
"#;

/// The stand-in, built once.
static WITHOUT_MUSIC21: pyo3::sync::PyOnceLock<Py<PyAny>> = pyo3::sync::PyOnceLock::new();

/// music21's class for a hash of this kind, or one built here where
/// music21 cannot be asked.
fn hash_class<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    if let Some(installed) = crate::installed_class(py, "music21.alpha.analysis.hasher", name) {
        return Ok(installed);
    }
    if let Ok(found) = py
        .import("music21.alpha.analysis.hasher")
        .and_then(|module| module.getattr(name))
    {
        return Ok(found);
    }
    if name == "NoteHash" {
        // A hash with no reference is the tuple itself, which is what
        // music21's `NoteHash` is a subclass of.
        return Ok(py.None().into_bound(py));
    }
    let built = WITHOUT_MUSIC21.get_or_try_init(py, || -> PyResult<Py<PyAny>> {
        Ok(PyModule::from_code(
            py,
            &std::ffi::CString::new(WITH_REFERENCE)?,
            c"music21_rs_hash_reference.py",
            c"music21_rs_hash_reference",
        )?
        .getattr("NoteHashWithReference")?
        .unbind())
    })?;
    Ok(built.bind(py).clone())
}

/// One hashed value as Python holds it.
fn value_of<'py>(py: Python<'py>, value: &HashValue) -> PyResult<Bound<'py, PyAny>> {
    Ok(match value {
        HashValue::Integer(number) => number.into_pyobject(py)?.into_any(),
        HashValue::Float(number) => number.into_pyobject(py)?.into_any(),
        HashValue::Text(text) => pyo3::types::PyString::new(py, text).into_any(),
        HashValue::None => py.None().into_bound(py),
    })
}

/// music21's `alpha.analysis.hasher.Hasher`: what of each note is hashed,
/// and the hashing itself.
#[pyclass(
    name = "Hasher",
    module = "music21.alpha.analysis.hasher",
    subclass,
    skip_from_py_object
)]
pub struct Hasher {
    /// Whether each hash says what it was made from.
    #[pyo3(get, set)]
    includeReference: bool,
    /// Whether a note's pitch is hashed.
    #[pyo3(get, set)]
    hashPitch: bool,
    /// Whether the pitch is its MIDI number rather than its name.
    #[pyo3(get, set)]
    hashMIDI: bool,
    /// Whether a named pitch leaves out its octave.
    #[pyo3(get, set)]
    hashNoteNameOctave: bool,
    /// Whether the octave is hashed.
    #[pyo3(get, set)]
    hashOctave: bool,
    /// Whether the length is hashed.
    #[pyo3(get, set)]
    hashDuration: bool,
    /// Whether lengths and offsets are rounded.
    #[pyo3(get, set)]
    roundDurationAndOffset: bool,
    /// Whether the offset is hashed.
    #[pyo3(get, set)]
    hashOffset: bool,
    /// How many parts of a quarter they are rounded to.
    #[pyo3(get, set)]
    granularity: u32,
    /// Whether the interval from the note before is hashed.
    #[pyo3(get, set)]
    hashIntervalFromLastNote: bool,
    /// Whether whether a note is altered is hashed.
    #[pyo3(get, set)]
    hashIsAccidental: bool,
    /// Whether ties are hashed, which music21 does not use.
    #[pyo3(get, set)]
    hashIsTied: bool,
    /// Whether a chord is hashed note by note.
    #[pyo3(get, set)]
    hashChordsAsNotes: bool,
    /// Whether a chord is hashed as one.
    #[pyo3(get, set)]
    hashChordsAsChords: bool,
    /// Whether a chord's normal order is hashed.
    #[pyo3(get, set)]
    hashNormalOrderString: bool,
    /// Whether a chord's prime form is hashed.
    #[pyo3(get, set)]
    hashPrimeFormString: bool,
    /// What music21 hashes: the classes it looks for in a stream.
    #[pyo3(get, set)]
    validTypes: Py<PyList>,
    /// What music21 remembers while hashing: the interval from the note
    /// before, the key signature in force.
    #[pyo3(get, set)]
    stateVars: Py<PyDict>,
    /// Which function hashes each value, which music21's own machinery
    /// looks up by name.
    #[pyo3(get, set)]
    hashingFunctions: Py<PyDict>,
    /// The values a hash holds, in order.
    #[pyo3(get, set)]
    tupleList: Py<PyList>,
    /// The named tuple a hash is made as.
    #[pyo3(get, set)]
    tupleClass: Py<PyAny>,
}

impl Hasher {
    /// One named value of a hash, as music21's own private hashing
    /// functions answer it: the crate's `hash_value` with the settings
    /// that function implies.
    fn one_value(
        slf: &Bound<'_, Self>,
        key: &str,
        e: Option<&Bound<'_, PyAny>>,
        thisChord: Option<&Bound<'_, PyAny>>,
        tweak: impl Fn(&mut RsHasher),
    ) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let mut settings = slf.borrow().settings();
        tweak(&mut settings);
        let element = match e.filter(|given| !given.is_none()) {
            Some(given) => crate::stream::element_read(given)?,
            None => None,
        };
        let chord = match thisChord.filter(|given| !given.is_none()) {
            Some(given) => match crate::stream::element_read(given)? {
                Some(music21_rs_crate::stream::StreamElement::Chord(chord)) => Some(chord),
                _ => None,
            },
            None => None,
        };
        // music21 reads the offset off the chord where it is given one,
        // and off the element otherwise.
        let offset = thisChord
            .filter(|given| !given.is_none())
            .or(e.filter(|given| !given.is_none()))
            .and_then(|given| given.getattr("offset").ok())
            .and_then(|offset| offset.extract().ok())
            .unwrap_or(0.0);
        // The note before, which music21 keeps in `stateVars` while it
        // hashes.
        let previous = slf
            .borrow()
            .stateVars
            .bind(py)
            .get_item("IntervalFromLastNote")
            .ok()
            .flatten()
            .filter(|given| !given.is_none())
            .and_then(|given| match crate::stream::element_read(&given) {
                Ok(Some(music21_rs_crate::stream::StreamElement::Note(note))) => {
                    Some(note.pitch().clone())
                }
                _ => None,
            });
        let value = settings
            .hash_value(
                key,
                element.as_ref(),
                chord.as_ref(),
                offset,
                previous.as_ref(),
            )
            .map_err(|error| crate::stream::stream_error(&error))?;
        Ok(value_of(py, &value)?.unbind())
    }
}

impl Hasher {
    /// The crate's hasher as the settings now stand: music21 keeps them as
    /// plain attributes a caller writes, so they are read afresh for every
    /// question rather than kept twice.
    fn settings(&self) -> RsHasher {
        RsHasher {
            include_reference: self.includeReference,
            hash_pitch: self.hashPitch,
            hash_midi: self.hashMIDI,
            hash_note_name_octave: self.hashNoteNameOctave,
            hash_octave: self.hashOctave,
            hash_duration: self.hashDuration,
            round_duration_and_offset: self.roundDurationAndOffset,
            hash_offset: self.hashOffset,
            granularity: self.granularity,
            hash_interval_from_last_note: self.hashIntervalFromLastNote,
            hash_is_accidental: self.hashIsAccidental,
            hash_is_tied: self.hashIsTied,
            hash_chords_as_notes: self.hashChordsAsNotes,
            hash_chords_as_chords: self.hashChordsAsChords,
            hash_normal_order_string: self.hashNormalOrderString,
            hash_prime_form_string: self.hashPrimeFormString,
        }
    }
}

#[pymethods]
impl Hasher {
    /// music21's `Hasher()`, with its defaults: a note is its MIDI pitch,
    /// its length and its offset, each rounded to a 32nd of a quarter, and
    /// a chord is hashed note by note.
    #[new]
    fn new(py: Python<'_>) -> PyResult<Self> {
        let classes = PyList::empty(py);
        // The classes music21 looks for, as its own `validTypes` lists
        // them.
        for (module, name) in [
            ("music21.note", "Note"),
            ("music21.note", "Rest"),
            ("music21.chord", "Chord"),
        ] {
            if let Some(installed) = crate::installed_class(py, module, name) {
                classes.append(installed)?;
            } else if let Ok(found) = py.import(module).and_then(|module| module.getattr(name)) {
                classes.append(found)?;
            } else if let Ok(found) = py
                .import("music21_rs")
                .or_else(|_| py.import("music21_rs_facade"))
                .and_then(|module| module.getattr(name))
            {
                classes.append(found)?;
            }
        }
        let defaults = RsHasher::default();
        Ok(Self {
            includeReference: defaults.include_reference,
            hashPitch: defaults.hash_pitch,
            hashMIDI: defaults.hash_midi,
            hashNoteNameOctave: defaults.hash_note_name_octave,
            hashOctave: defaults.hash_octave,
            hashDuration: defaults.hash_duration,
            roundDurationAndOffset: defaults.round_duration_and_offset,
            hashOffset: defaults.hash_offset,
            granularity: defaults.granularity,
            hashIntervalFromLastNote: defaults.hash_interval_from_last_note,
            hashIsAccidental: defaults.hash_is_accidental,
            hashIsTied: defaults.hash_is_tied,
            hashChordsAsNotes: defaults.hash_chords_as_notes,
            hashChordsAsChords: defaults.hash_chords_as_chords,
            hashNormalOrderString: defaults.hash_normal_order_string,
            hashPrimeFormString: defaults.hash_prime_form_string,
            validTypes: classes.unbind(),
            stateVars: PyDict::new(py).unbind(),
            hashingFunctions: PyDict::new(py).unbind(),
            tupleList: PyList::empty(py).unbind(),
            tupleClass: py.None(),
        })
    }

    /// A note's length, unrounded: music21's `_hashDuration`.
    #[pyo3(signature = (e = None, thisChord = None))]
    fn _hashDuration(
        slf: &Bound<'_, Self>,
        e: Option<&Bound<'_, PyAny>>,
        thisChord: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        Self::one_value(slf, "Duration", e, thisChord, |settings| {
            settings.round_duration_and_offset = false;
        })
    }

    /// The same rounded to the granularity: music21's `_hashRoundedDuration`.
    #[pyo3(signature = (e = None, thisChord = None))]
    fn _hashRoundedDuration(
        slf: &Bound<'_, Self>,
        e: Option<&Bound<'_, PyAny>>,
        thisChord: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        Self::one_value(slf, "Duration", e, thisChord, |settings| {
            settings.round_duration_and_offset = true;
        })
    }

    /// A note's MIDI number, nought for a rest and one for a chord hashed as a chord: music21's `_hashMIDIPitchName`.
    #[pyo3(signature = (e = None, thisChord = None))]
    fn _hashMIDIPitchName(
        slf: &Bound<'_, Self>,
        e: Option<&Bound<'_, PyAny>>,
        thisChord: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        Self::one_value(slf, "Pitch", e, thisChord, |settings| {
            settings.hash_midi = true;
        })
    }

    /// A note's name with its octave: music21's `_hashPitchName`.
    #[pyo3(signature = (e = None, thisChord = None))]
    fn _hashPitchName(
        slf: &Bound<'_, Self>,
        e: Option<&Bound<'_, PyAny>>,
        thisChord: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        Self::one_value(slf, "Pitch", e, thisChord, |settings| {
            settings.hash_midi = false;
            settings.hash_note_name_octave = false;
        })
    }

    /// Its name without the octave: music21's `_hashPitchNameNoOctave`.
    #[pyo3(signature = (e = None, thisChord = None))]
    fn _hashPitchNameNoOctave(
        slf: &Bound<'_, Self>,
        e: Option<&Bound<'_, PyAny>>,
        thisChord: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        Self::one_value(slf, "Pitch", e, thisChord, |settings| {
            settings.hash_midi = false;
            settings.hash_note_name_octave = true;
        })
    }

    /// A note's octave, less one for a rest or a chord hashed as a chord: music21's `_hashOctave`.
    #[pyo3(signature = (e = None, thisChord = None))]
    fn _hashOctave(
        slf: &Bound<'_, Self>,
        e: Option<&Bound<'_, PyAny>>,
        thisChord: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        Self::one_value(slf, "Octave", e, thisChord, |_| {})
    }

    /// Whether a note is altered, which music21 hashes nothing for yet: its `_hashIsAccidental`.
    #[pyo3(signature = (e = None, thisChord = None))]
    fn _hashIsAccidental(
        slf: &Bound<'_, Self>,
        e: Option<&Bound<'_, PyAny>>,
        thisChord: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        Self::one_value(slf, "IsAccidental", e, thisChord, |_| {})
    }

    /// Where a note stands, unrounded: music21's `_hashOffset`.
    #[pyo3(signature = (e = None, thisChord = None))]
    fn _hashOffset(
        slf: &Bound<'_, Self>,
        e: Option<&Bound<'_, PyAny>>,
        thisChord: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        Self::one_value(slf, "Offset", e, thisChord, |settings| {
            settings.round_duration_and_offset = false;
        })
    }

    /// The same rounded: music21's `_hashRoundedOffset`.
    #[pyo3(signature = (e = None, thisChord = None))]
    fn _hashRoundedOffset(
        slf: &Bound<'_, Self>,
        e: Option<&Bound<'_, PyAny>>,
        thisChord: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        Self::one_value(slf, "Offset", e, thisChord, |settings| {
            settings.round_duration_and_offset = true;
        })
    }

    /// The interval class from the note before: music21's `_hashIntervalFromLastNote`.
    #[pyo3(signature = (e = None, thisChord = None))]
    fn _hashIntervalFromLastNote(
        slf: &Bound<'_, Self>,
        e: Option<&Bound<'_, PyAny>>,
        thisChord: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        Self::one_value(slf, "IntervalFromLastNote", e, thisChord, |_| {})
    }

    /// A chord's normal order: music21's `_hashChordNormalOrderString`.
    #[pyo3(signature = (e = None, thisChord = None))]
    fn _hashChordNormalOrderString(
        slf: &Bound<'_, Self>,
        e: Option<&Bound<'_, PyAny>>,
        thisChord: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        Self::one_value(slf, "NormalOrderString", e, thisChord, |_| {})
    }

    /// Its prime form: music21's `_hashPrimeFormString`.
    #[pyo3(signature = (e = None, thisChord = None))]
    fn _hashPrimeFormString(
        slf: &Bound<'_, Self>,
        e: Option<&Bound<'_, PyAny>>,
        thisChord: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        Self::one_value(slf, "PrimeFormString", e, thisChord, |_| {})
    }

    /// What is remembered while hashing, by the flags that ask for it:
    /// music21's `setupValidTypesAndStateVars`.
    fn setupValidTypesAndStateVars(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let (interval, accidental) = {
            let me = slf.borrow();
            (me.hashIntervalFromLastNote, me.hashIsAccidental)
        };
        let state = slf.borrow().stateVars.bind(py).clone();
        if interval {
            state.set_item("IntervalFromLastNote", py.None())?;
        }
        if accidental {
            state.set_item("KeySignature", py.None())?;
            // music21 looks for key signatures too where it hashes
            // whether a note is altered.
            let classes = slf.borrow().validTypes.bind(py).clone();
            let signature = match crate::installed_class(py, "music21.key", "KeySignature") {
                Some(installed) => Some(installed),
                None => py
                    .import("music21.key")
                    .and_then(|module| module.getattr("KeySignature"))
                    .ok()
                    .or_else(|| {
                        py.import("music21_rs")
                            .or_else(|_| py.import("music21_rs_facade"))
                            .and_then(|module| module.getattr("KeySignature"))
                            .ok()
                    }),
            };
            if let Some(signature) = signature
                && !classes.contains(&signature)?
            {
                classes.append(signature)?;
            }
        }
        Ok(())
    }

    /// The values each hash holds and the function that hashes each:
    /// music21's `setupTupleList`, the list being the crate's and the
    /// functions music21's own, which its machinery looks up by name.
    fn setupTupleList(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let names = slf.borrow().settings().tuple_list();
        let listed = PyList::empty(py);
        for name in &names {
            listed.append(*name)?;
        }
        // music21 names the function for each value after the setting that
        // asked for it; these are its own, inherited rather than ported.
        let functions = PyDict::new(py);
        let rounded = slf.borrow().roundDurationAndOffset;
        let midi = slf.borrow().hashMIDI;
        let no_octave = slf.borrow().hashNoteNameOctave;
        for name in &names {
            let found = match *name {
                "Pitch" => {
                    if midi {
                        "_hashMIDIPitchName"
                    } else if no_octave {
                        "_hashPitchNameNoOctave"
                    } else {
                        "_hashPitchName"
                    }
                }
                "Octave" => "_hashOctave",
                "Duration" => {
                    if rounded {
                        "_hashRoundedDuration"
                    } else {
                        "_hashDuration"
                    }
                }
                "Offset" => {
                    if rounded {
                        "_hashRoundedOffset"
                    } else {
                        "_hashOffset"
                    }
                }
                "IntervalFromLastNote" => "_hashIntervalFromLastNote",
                "IsAccidental" => "_hashIsAccidental",
                "NormalOrderString" => "_hashChordNormalOrderString",
                "PrimeFormString" => "_hashPrimeFormString",
                _ => continue,
            };
            if let Ok(function) = slf.as_any().getattr(found) {
                functions.set_item(*name, function)?;
            }
        }
        let made = py
            .import("collections")?
            .getattr("namedtuple")?
            .call1(("NoteHash", &listed))?;
        let mut me = slf.borrow_mut();
        me.tupleList = listed.unbind();
        me.hashingFunctions = functions.unbind();
        me.tupleClass = made.unbind();
        Ok(())
    }

    /// Each measure of a stream hashed on its own: music21's
    /// `hashMeasures`.
    fn hashMeasures<'py>(slf: &Bound<'py, Self>, s: &Bound<'py, PyAny>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let out = PyList::empty(py);
        for measure in s
            .call_method0("recurse")?
            .call_method1("getElementsByClass", ("Measure",))?
            .walk()?
        {
            out.append(Self::hashStream(slf, &measure?)?)?;
        }
        Ok(out.into_any().unbind())
    }

    /// A stream's notes, rests and chords hashed: music21's `hashStream`,
    /// which is the crate's hashing.
    fn hashStream<'py>(slf: &Bound<'py, Self>, s: &Bound<'py, PyAny>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        Self::setupValidTypesAndStateVars(slf)?;
        Self::setupTupleList(slf)?;
        let (inner, reference) = {
            let me = slf.borrow();
            (me.settings(), me.includeReference)
        };
        let stream = crate::stream::crate_stream(s)?;
        let hashes = inner
            .hash_stream(&stream)
            .map_err(|error| crate::stream::stream_error(&error))?;
        let objects = if reference {
            Self::hashed_objects(slf, s)?
        } else {
            Vec::new()
        };
        if reference && objects.len() != hashes.len() {
            return Err(crate::stream::StreamException::new_err(format!(
                "{} hashes were made of {} elements, so no hash could say what it was made from",
                hashes.len(),
                objects.len()
            )));
        }
        let out = PyList::empty(py);
        for (at, hash) in hashes.iter().enumerate() {
            out.append(Self::made(slf, hash, objects.get(at))?)?;
        }
        Ok(out.into_any().unbind())
    }

    /// A hash made from a named tuple, with what it was made from beside
    /// it: music21's `addNoteHashWithReferenceToFinalHash`.
    fn addNoteHashWithReferenceToFinalHash(
        slf: &Bound<'_, Self>,
        finalHash: &Bound<'_, PyAny>,
        tupleHash: &Bound<'_, PyAny>,
        reference: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let py = slf.py();
        let made = hash_class(py, "NoteHashWithReference")?.call1((tupleHash,))?;
        made.setattr("reference", reference)?;
        finalHash.call_method1("append", (made,))?;
        Ok(())
    }

    /// The same without a reference: music21's `addNoteHashToFinalHash`.
    fn addNoteHashToFinalHash(
        slf: &Bound<'_, Self>,
        finalHash: &Bound<'_, PyAny>,
        tupleHash: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let py = slf.py();
        let class = hash_class(py, "NoteHash")?;
        let made = if class.is_none() {
            tupleHash.clone()
        } else {
            class.call1((tupleHash,))?
        };
        finalHash.call_method1("append", (made,))?;
        Ok(())
    }

    /// One hash, made and referenced as music21 makes one: music21's
    /// `addHashToFinalHash`.
    fn addHashToFinalHash(
        slf: &Bound<'_, Self>,
        singleNoteHash: &Bound<'_, PyAny>,
        finalHash: &Bound<'_, PyAny>,
        reference: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let py = slf.py();
        let class = slf.borrow().tupleClass.bind(py).clone();
        let tupleHash = if class.is_none() {
            singleNoteHash.clone()
        } else {
            class.call_method1("_make", (singleNoteHash,))?
        };
        if slf.borrow().includeReference {
            return Self::addNoteHashWithReferenceToFinalHash(
                slf, finalHash, &tupleHash, reference,
            );
        }
        Self::addNoteHashToFinalHash(slf, finalHash, &tupleHash)
    }
}

impl Hasher {
    /// What each hash was made from, in the order the hashes come: the
    /// element itself, or each member of a chord hashed note by note, as
    /// music21's own `hashStream` references them.
    fn hashed_objects(slf: &Bound<'_, Self>, s: &Bound<'_, PyAny>) -> PyResult<Vec<Py<PyAny>>> {
        let (as_notes, as_chords) = {
            let me = slf.borrow();
            (me.hashChordsAsNotes, me.hashChordsAsChords)
        };
        let mut out = Vec::new();
        for element in s.call_method0("recurse")?.walk()? {
            let element = element?;
            let answers = |name: &str| {
                element
                    .getattr("classes")
                    .and_then(|classes| classes.contains(name))
                    .unwrap_or(false)
            };
            if answers("Chord") {
                if as_notes {
                    for note in element.try_iter()? {
                        out.push(note?.unbind());
                    }
                } else if as_chords {
                    out.push(element.unbind());
                }
                continue;
            }
            if answers("Note") || answers("Rest") {
                out.push(element.unbind());
            }
        }
        Ok(out)
    }

    /// One of the crate's hashes as music21 hands one back.
    fn made(
        slf: &Bound<'_, Self>,
        hash: &NoteHash,
        reference: Option<&Py<PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let values = PyList::empty(py);
        for value in &hash.values {
            values.append(value_of(py, value)?)?;
        }
        let class = slf.borrow().tupleClass.bind(py).clone();
        let tupleHash = if class.is_none() {
            PyTuple::new(py, values.iter())?.into_any()
        } else {
            class.call_method1("_make", (values,))?
        };
        let Some(reference) = reference else {
            let class = hash_class(py, "NoteHash")?;
            return Ok(if class.is_none() {
                tupleHash.unbind()
            } else {
                class.call1((tupleHash,))?.unbind()
            });
        };
        let made = hash_class(py, "NoteHashWithReference")?.call1((tupleHash,))?;
        made.setattr("reference", reference.bind(py))?;
        Ok(made.unbind())
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Hasher>()?;
    Ok(())
}
