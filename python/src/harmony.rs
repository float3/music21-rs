//! music21's `harmony` module, over the crate's chord symbols.
//!
//! Replaced are the two functions that *read* a chord and name it --
//! `chordSymbolFigureFromChord` and `chordSymbolFromChord` -- and the classes
//! a figure is read into: `ChordSymbol`, `NoChord` and
//! `ChordStepModification`. A symbol is a `Chord` whose notes the crate
//! sounds from the figure the way music21's `_updatePitches` does, octaves,
//! inversion and bass included; the key, the roman numeral and the placement
//! a score gives it are kept beside it for music21's exporters to read.
//! `Harmony` itself stays music21's, since nothing is left of it once its
//! subclasses are ours.
//!
//! The four table functions (`getAbbreviationListGivenChordType` and its
//! neighbours) are left alone: music21's `changeAbbreviationFor` and
//! `addNewChordSymbol` *edit* the table those read, and the crate's is a
//! static one. Replacing the readers without the writers would make the pair
//! disagree, and the table itself is already checked against music21's by
//! `chord_type_parity`. Where music21 is there to ask, a symbol reads that
//! live table instead of the crate's -- the abbreviation it writes, the
//! notation it sounds and a kind a program added -- so an edit made through
//! music21's writers is one the symbol sees.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::{PyKeyError, PyValueError};
use pyo3::prelude::*;

use crate::Walkable;
use crate::spelling::{music21_figure, music21_name};
use pyo3::types::{PyDict, PyList, PyTuple};

use music21_rs_crate::chordsymbol::{
    ChordStepModification as RsChordStepModification,
    ChordStepModificationType as RsChordStepModificationType, ChordSymbolFigure,
    inversion_is_valid_for_kind, voice_chord_notation,
};
use music21_rs_crate::{ChordSymbol as RsChordSymbol, Interval as RsInterval, Pitch as RsPitch};

use crate::chord::Chord;
use crate::interval::interval_from_any;
use crate::pitch::{Pitch, pitch_from_any};

/// The names the `harmony` facade replaces in `music21.harmony`.
pub const NAMES: &[&str] = &[
    "chordSymbolFigureFromChord",
    "chordSymbolFromChord",
    "HarmonyException",
    "ChordSymbol",
    "NoChord",
    "ChordStepModification",
    "ChordStepModificationException",
];

/// The names installed over music21's own: everything but the exception
/// classes, which are music21's to raise.
pub const INSTALLED_NAMES: &[&str] = &[
    "chordSymbolFigureFromChord",
    "chordSymbolFromChord",
    "ChordSymbol",
    "NoChord",
    "ChordStepModification",
];

pyo3::create_exception!(music21_rs_facade, HarmonyException, crate::Music21Exception);

error_into!(harmony_error, HarmonyException);

/// music21's sentence for a chord no kind in its table fits.
const UNIDENTIFIED: &str = "Chord Symbol Cannot Be Identified";

/// The crate's chord behind whatever was handed over.
///
/// music21 passes its own `Chord` here, which under the harness is one of
/// ours. A `ChordSymbol` is a chord of music21's own class, built before the
/// swap, so it is read by its pitches and the root its figure fixed.
pub(crate) fn chord_of(
    py: Python<'_>,
    inChord: &Bound<'_, PyAny>,
) -> PyResult<music21_rs_crate::Chord> {
    if let Ok(chord) = inChord.extract::<PyRef<'_, Chord>>() {
        return Ok(chord.synced_inner(py));
    }
    let mut names = Vec::new();
    for pitch in inChord.getattr("pitches")?.walk()? {
        names.push(pitch?.getattr("nameWithOctave")?.extract::<String>()?);
    }
    let mut chord = music21_rs_crate::Chord::new(names.as_slice())
        .map_err(|error| HarmonyException::new_err(crate::pitch::message(&error)))?;
    if let Ok(overrides) = inChord.getattr("_overrides")
        && let Ok(root) = overrides.call_method1("get", ("root",))
        && !root.is_none()
        && let Ok(name) = root.getattr("nameWithOctave")?.extract::<String>()
        && let Ok(root) = music21_rs_crate::Pitch::from_name(name)
    {
        chord.set_root(Some(root));
    }
    Ok(chord)
}

/// The figure of a chord and its kind, written as music21 writes them.
///
/// A suspended second in inversion is read as a suspended fourth on its bass,
/// and music21 fixes the chord's root there as it says so; the chord handed
/// in is given the same root. The abbreviation is the one music21's live
/// table gives the kind, since `changeAbbreviationFor` edits that table.
fn figure_and_kind(py: Python<'_>, inChord: &Bound<'_, PyAny>) -> PyResult<(String, &'static str)> {
    let chord = chord_of(py, inChord)?;
    if chord.notes().is_empty() {
        return Ok((String::new(), ""));
    }
    let Some(figure) = ChordSymbolFigure::from_chord(&chord) else {
        return Ok((UNIDENTIFIED.to_string(), ""));
    };
    if chord.root().is_some_and(|root| root.name() != figure.root) {
        let bass = inChord.call_method0("bass")?;
        inChord.call_method1("root", (bass,))?;
    }
    // music21's live table where there is a music21 to ask, since
    // `changeAbbreviationFor` edits it; the crate's own where there is not.
    let abbreviation = py
        .import("music21.harmony")
        .and_then(|harmony| harmony.getattr("getAbbreviationListGivenChordType"))
        .and_then(|lookup| lookup.call1((figure.kind,)))
        .and_then(|list| list.get_item(0))
        .and_then(|first| first.extract::<String>())
        .unwrap_or_else(|_| figure.abbreviation.to_string());
    let kind = figure.kind;
    Ok((music21_figure(figure).written_with(&abbreviation), kind))
}

/// music21's `chordSymbolFigureFromChord`: the lead-sheet symbol a chord
/// would be written with.
///
/// With `includeChordType` it answers the kind beside the figure, as music21
/// does — except for an empty chord, which has no kind and which music21
/// answers with a bare empty string either way.
#[pyfunction]
#[pyo3(signature = (inChord, includeChordType = false))]
pub fn chordSymbolFigureFromChord<'py>(
    py: Python<'py>,
    inChord: &Bound<'py, PyAny>,
    includeChordType: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let (figure, kind) = figure_and_kind(py, inChord)?;
    if !includeChordType || figure.is_empty() {
        return Ok(figure.into_pyobject(py)?.into_any());
    }
    Ok(PyTuple::new(py, [figure.as_str(), kind])?.into_any())
}

/// music21's `chordSymbolFromChord`: the same figure, read back as a chord
/// symbol.
///
#[pyfunction]
pub fn chordSymbolFromChord<'py>(
    py: Python<'py>,
    inChord: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let (figure, _) = figure_and_kind(py, inChord)?;
    let class = crate::installed_class(py, "music21.harmony", "ChordSymbol")
        .unwrap_or_else(|| py.get_type::<ChordSymbol>().into_any());
    let symbol = class.call1((figure,))?;
    // The symbol sounds the chord's own pitches, in the octaves the chord
    // has them, rather than the ones its figure would realize.
    symbol.setattr("pitches", inChord.getattr("pitches")?)?;
    Ok(symbol)
}

/// music21's `getAbbreviationListGivenChordType`: every abbreviation a kind
/// is written with, or a `KeyError` for a kind the table has not got.
#[pyfunction]
#[pyo3(name = "getAbbreviationListGivenChordType")]
fn getAbbreviationListGivenChordType(chordType: String) -> PyResult<Vec<String>> {
    music21_rs_crate::chordsymbol::abbreviations_for_kind(&chordType)
        .map(|list| list.iter().map(|each| (*each).to_string()).collect())
        .ok_or_else(|| PyKeyError::new_err(chordType))
}

/// music21's `getCurrentAbbreviationFor`: the abbreviation a kind is written
/// with.
#[pyfunction]
#[pyo3(name = "getCurrentAbbreviationFor")]
fn getCurrentAbbreviationFor(chordType: String) -> PyResult<String> {
    music21_rs_crate::chordsymbol::current_abbreviation_for_kind(&chordType)
        .map(str::to_string)
        .ok_or_else(|| PyKeyError::new_err(chordType))
}

/// music21's `getNotationStringGivenChordType`: the figured-bass notation
/// a kind realizes from.
#[pyfunction]
#[pyo3(name = "getNotationStringGivenChordType")]
fn getNotationStringGivenChordType(chordType: String) -> PyResult<String> {
    music21_rs_crate::chordsymbol::notation_for_kind(&chordType)
        .map(str::to_string)
        .ok_or_else(|| PyKeyError::new_err(chordType))
}

pyo3::create_exception!(
    music21_rs_facade,
    ChordStepModificationException,
    crate::Music21Exception
);

/// music21's `harmony.ChordStepModification`: one degree added to, taken
/// from or altered in a chord symbol.
///
/// Its three parts are settable one at a time, as music21's MusicXML reader
/// sets them, so each is held as it stands and read into the crate's value
/// only when a chord is sounded.
#[pyclass(
    name = "ChordStepModification",
    module = "music21.harmony",
    subclass,
    skip_from_py_object
)]
pub struct ChordStepModification {
    mod_type: Option<RsChordStepModificationType>,
    degree: Option<u8>,
    /// The interval the degree is moved by, as the object music21 hands
    /// back, or `None` where a caller set none.
    interval: Option<Py<PyAny>>,
}

impl ChordStepModification {
    fn wrap(py: Python<'_>, value: &RsChordStepModification) -> PyResult<Self> {
        Ok(Self {
            mod_type: Some(value.modification_type()),
            degree: Some(value.degree()),
            interval: Some(crate::interval::interval_object(
                py,
                value.interval().clone(),
            )?),
        })
    }

    /// The object music21 holds in a chord symbol's list, of the class
    /// installed over music21's where there is one.
    fn object(py: Python<'_>, value: &RsChordStepModification) -> PyResult<Py<PyAny>> {
        Ok(crate::installed_new(
            py,
            "music21.harmony",
            "ChordStepModification",
            Self::wrap(py, value)?,
        )?
        .into_any())
    }
}

/// The crate's value of a modification, whichever class holds it: one of
/// these, or music21's own, which its MusicXML reader builds.
fn modification_value(value: &Bound<'_, PyAny>) -> PyResult<RsChordStepModification> {
    let mod_type: Option<String> = value.getattr("modType")?.extract()?;
    let mod_type = RsChordStepModificationType::from_music21_name(
        mod_type.as_deref().unwrap_or(""),
    )
    .map_err(|error| ChordStepModificationException::new_err(crate::pitch::message(&error)))?;
    let degree: u8 = value.getattr("degree")?.extract()?;
    let interval = value.getattr("interval")?;
    Ok(if interval.is_none() {
        RsChordStepModification::new(mod_type, degree, 0).map_err(harmony_error)?
    } else {
        RsChordStepModification::with_interval(mod_type, degree, interval_from_any(&interval)?)
    })
}

#[pymethods]
impl ChordStepModification {
    #[new]
    #[pyo3(signature = (modType = None, degree = None, intervalObj = None))]
    fn new(
        py: Python<'_>,
        modType: Option<&Bound<'_, PyAny>>,
        degree: Option<&Bound<'_, PyAny>>,
        intervalObj: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let mut made = Self {
            mod_type: None,
            degree: None,
            interval: None,
        };
        if let Some(value) = modType.filter(|value| !value.is_none()) {
            made.set_modType(value)?;
        }
        if let Some(value) = degree.filter(|value| !value.is_none()) {
            made.set_degree(value)?;
        }
        match intervalObj.filter(|value| !value.is_none()) {
            Some(value) => made.set_interval(py, Some(value))?,
            None => {
                made.interval = Some(crate::interval::interval_object(
                    py,
                    RsInterval::from_name("P1").map_err(harmony_error)?,
                )?)
            }
        }
        Ok(made)
    }

    /// Pickled as its three parts, since music21 freezes every score it
    /// parses and a chord symbol's modifications go with it. The state says
    /// whether the interval was left unset, which the constructor would
    /// otherwise fill with a unison.
    fn __reduce__<'py>(
        slf: &Bound<'py, Self>,
    ) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyTuple>, bool)> {
        let py = slf.py();
        let me = slf.borrow();
        let arguments = PyTuple::new(
            py,
            [
                me.mod_type
                    .map(|kind| kind.music21_name())
                    .into_pyobject(py)?
                    .into_any(),
                me.degree.into_pyobject(py)?.into_any(),
                me.interval
                    .as_ref()
                    .map_or_else(|| py.None(), |interval| interval.clone_ref(py))
                    .into_bound(py),
            ],
        )?;
        Ok((slf.get_type().into_any(), arguments, me.interval.is_none()))
    }

    fn __setstate__(&mut self, interval_unset: bool) {
        if interval_unset {
            self.interval = None;
        }
    }

    /// A copy of the same class, its interval copied through the copier's
    /// memo.
    #[pyo3(signature = (memo = None))]
    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let (class, arguments, interval_unset) = Self::__reduce__(slf)?;
        let interval = py
            .import("copy")?
            .getattr("deepcopy")?
            .call1((arguments.get_item(2)?, memo))?;
        let copied = class.call1((arguments.get_item(0)?, arguments.get_item(1)?, interval))?;
        copied
            .extract::<PyRefMut<'_, Self>>()?
            .__setstate__(interval_unset);
        Ok(copied)
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.interval)
    }

    fn __clear__(&mut self) {
        self.interval = None;
    }

    /// music21's `modType`: `'add'`, `'subtract'` or `'alter'`.
    #[getter]
    fn get_modType(&self) -> Option<&'static str> {
        self.mod_type.map(RsChordStepModificationType::music21_name)
    }

    #[setter]
    fn set_modType(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let written = value.extract::<String>().unwrap_or_default();
        self.mod_type = Some(
            RsChordStepModificationType::from_music21_name(&written).map_err(|_| {
                ChordStepModificationException::new_err(format!(
                    "not a valid degree modification type: {}",
                    value.str().map(|text| text.to_string()).unwrap_or_default()
                ))
            })?,
        );
        Ok(())
    }

    /// music21's `degree`: the chord step, where three is the third.
    #[getter]
    fn get_degree(&self) -> Option<u8> {
        self.degree
    }

    #[setter]
    fn set_degree(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let refused = || {
            ChordStepModificationException::new_err(format!(
                "not a valid degree: {}",
                value.str().map(|text| text.to_string()).unwrap_or_default()
            ))
        };
        if value.is_instance_of::<pyo3::types::PyString>() {
            return Err(refused());
        }
        let number: f64 = value.extract().map_err(|_| refused())?;
        self.degree = Some(number as u8);
        Ok(())
    }

    /// music21's `interval`: how far the degree is moved. A number of
    /// semitones is read as music21 reads it, as augmented unisons up to
    /// three and as the interval that many semitones make beyond.
    #[getter]
    fn get_interval(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.interval
            .as_ref()
            .map(|interval| interval.clone_ref(py))
    }

    #[setter]
    fn set_interval(&mut self, py: Python<'_>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let Some(value) = value.filter(|value| !value.is_none()) else {
            self.interval = None;
            return Ok(());
        };
        if let Ok(semitones) = value.extract::<i32>()
            && !value.hasattr("semitones")?
        {
            let made = RsChordStepModification::new(RsChordStepModificationType::Add, 1, semitones)
                .map_err(harmony_error)?;
            self.interval = Some(crate::interval::interval_object(
                py,
                made.interval().clone(),
            )?);
            return Ok(());
        }
        self.interval = Some(value.clone().unbind());
        Ok(())
    }

    fn __eq__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let Ok(other) = other.extract::<PyRef<'_, ChordStepModification>>() else {
            return Ok(false);
        };
        if self.mod_type != other.mod_type || self.degree != other.degree {
            return Ok(false);
        }
        match (&self.interval, &other.interval) {
            (None, None) => Ok(true),
            (Some(mine), Some(theirs)) => mine.bind(py).eq(theirs.bind(py)),
            _ => Ok(false),
        }
    }

    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        slf.as_ptr() as isize >> 4
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let interval = match &self.interval {
            Some(interval) => interval.bind(py).str()?.to_string(),
            None => "None".to_string(),
        };
        Ok(format!(
            "<music21.harmony.ChordStepModification modType={} degree={} interval={interval}>",
            self.get_modType().unwrap_or("None"),
            self.degree
                .map_or_else(|| "None".to_string(), |degree| degree.to_string()),
        ))
    }
}

/// A root or bass written as music21 writes one to a chord symbol: a name,
/// with `b` read as a flat, in the octave below middle C; or a pitch as it
/// stands.
fn symbol_pitch(value: &Bound<'_, PyAny>) -> PyResult<RsPitch> {
    if let Ok(name) = value.extract::<String>() {
        let mut cleaned = String::with_capacity(name.len());
        let mut previous: Option<char> = None;
        for character in name.chars() {
            if character == 'b' && previous.is_some_and(|letter| "ABCDEFGabcdefg".contains(letter))
            {
                cleaned.push('-');
            } else {
                cleaned.push(character);
            }
            previous = Some(character);
        }
        let mut pitch = RsPitch::from_name(&cleaned).map_err(crate::pitch::pitch_error)?;
        pitch.set_octave(Some(3));
        return Ok(pitch);
    }
    pitch_from_any(value)
}

/// music21's `harmony.ChordSymbol`: a chord written as a lead-sheet symbol.
///
/// It is a `Chord` — the notes are the ones the symbol sounds — and beside
/// them it keeps what music21 keeps: the figure as written, the kind, the
/// chord-step modifications, and the key and roman numeral a caller may give
/// it. The notes are worked out by the crate from the kind, the root and the
/// bass (`sound_chord_notation`), which is music21's `_updatePitches`.
#[pyclass(
    name = "ChordSymbol",
    module = "music21.harmony",
    extends = Chord,
    subclass,
    skip_from_py_object
)]
pub struct ChordSymbol {
    /// music21's `_figure`: the figure as written, or `None` once something
    /// has changed the chord, when `figure` is worked out from the notes.
    figure: Option<String>,
    chord_kind: Option<String>,
    chord_kind_str: String,
    modifications: Py<PyList>,
    write_as_chord: bool,
    key: Option<Py<PyAny>>,
    roman: Option<Py<PyAny>>,
    placement: Option<Py<PyAny>>,
}

impl ChordSymbol {
    fn blank(py: Python<'_>) -> Self {
        Self {
            figure: None,
            chord_kind: Some(String::new()),
            chord_kind_str: String::new(),
            modifications: PyList::empty(py).unbind(),
            write_as_chord: false,
            key: None,
            roman: None,
            placement: None,
        }
    }

    /// The state a copy carries, which the `Chord` it is copied as does not.
    fn copied_state(&self, py: Python<'_>) -> PyResult<Self> {
        let copier = py.import("copy")?.getattr("deepcopy")?;
        Ok(Self {
            figure: self.figure.clone(),
            chord_kind: self.chord_kind.clone(),
            chord_kind_str: self.chord_kind_str.clone(),
            modifications: copier
                .call1((self.modifications.bind(py),))?
                .cast_into::<PyList>()?
                .unbind(),
            write_as_chord: self.write_as_chord,
            key: self.key.as_ref().map(|key| key.clone_ref(py)),
            roman: self.roman.as_ref().map(|roman| roman.clone_ref(py)),
            placement: self.placement.as_ref().map(|value| value.clone_ref(py)),
        })
    }

    /// Writes a copy's state into the object the `Chord` half built.
    fn fill(copy: &Bound<'_, PyAny>, state: Self) -> PyResult<()> {
        let copy = copy.cast::<ChordSymbol>()?;
        *copy.borrow_mut() = state;
        Ok(())
    }

    /// music21's `_parseFigure`: the kind, root, bass and modifications a
    /// figure says, with root and bass fixed on the chord in the octave
    /// below middle C.
    fn parse_figure(slf: &Bound<'_, Self>, figure: &str) -> PyResult<()> {
        let py = slf.py();
        let symbol = match RsChordSymbol::parse_music21(figure) {
            Ok(symbol) => symbol,
            Err(error) => match live_kind(py, figure)? {
                Some(symbol) => symbol,
                // A root no pitch spells is music21's accidental error, let
                // through; anything else a figure it cannot read.
                None => {
                    return Err(crate::pitch::specific_error(&error)
                        .unwrap_or_else(|| PyValueError::new_err(crate::pitch::message(&error))));
                }
            },
        };
        let kind = symbol.kind().unwrap_or_default().to_string();
        let list = PyList::empty(py);
        for modification in symbol.chord_step_modifications() {
            list.append(ChordStepModification::object(py, modification)?)?;
        }
        {
            let mut me = slf.borrow_mut();
            me.chord_kind = Some(kind);
            me.modifications = list.unbind();
        }
        let mut root = symbol.root().clone();
        root.set_octave(Some(3));
        let bass = symbol.bass().cloned().map(|mut bass| {
            bass.set_octave(Some(3));
            bass
        });
        Self::fix_root_and_bass(slf, Some(root), bass)
    }

    /// music21's `_updateFromParameters` before the notes are made: the
    /// root and bass written into the chord's overrides, the bass added as
    /// a note where the chord has none by its name.
    fn fix_root_and_bass(
        slf: &Bound<'_, Self>,
        root: Option<RsPitch>,
        bass: Option<RsPitch>,
    ) -> PyResult<()> {
        let py = slf.py();
        if let Some(root) = root {
            let root =
                crate::installed_new(py, "music21.pitch", "Pitch", Pitch::wrap(root, false))?;
            slf.call_method1("root", (root,))?;
        }
        if let Some(bass) = bass {
            let bass =
                crate::installed_new(py, "music21.pitch", "Pitch", Pitch::wrap(bass, false))?;
            let keywords = PyDict::new(py);
            keywords.set_item("allow_add", true)?;
            slf.call_method("bass", (bass,), Some(&keywords))?;
        }
        Ok(())
    }

    fn override_of<'py>(slf: &Bound<'py, Self>, name: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        let overrides = slf.getattr("_overrides")?;
        let found = overrides.call_method1("get", (name,))?;
        Ok((!found.is_none()).then_some(found))
    }
}

#[pymethods]
impl ChordSymbol {
    #[new]
    #[pyo3(signature = (*_arguments, **_keywords))]
    fn new(
        py: Python<'_>,
        _arguments: &Bound<'_, PyTuple>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(PyClassInitializer::from(Chord::build(py, None, None)?).add_subclass(Self::blank(py)))
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.modifications)?;
        visit.call(&self.key)?;
        visit.call(&self.roman)?;
        visit.call(&self.placement)?;
        Ok(())
    }

    fn __clear__(&mut self) {
        self.key = None;
        self.roman = None;
        self.placement = None;
    }

    /// music21's `ChordSymbol(figure, root, bass, inversion, kind, kindStr,
    /// **keywords)`: a figure to read, or a root, bass and kind as MusicXML
    /// gives them. A symbol takes no time unless it is given some.
    #[pyo3(signature = (
        figure = None, root = None, bass = None, inversion = None, kind = None,
        kindStr = None, updatePitches = true, **keywords
    ))]
    #[allow(clippy::too_many_arguments)]
    fn __init__(
        slf: &Bound<'_, Self>,
        figure: Option<&Bound<'_, PyAny>>,
        root: Option<&Bound<'_, PyAny>>,
        bass: Option<&Bound<'_, PyAny>>,
        inversion: Option<&Bound<'_, PyAny>>,
        kind: Option<String>,
        kindStr: Option<String>,
        updatePitches: bool,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let py = slf.py();
        py.get_type::<Chord>()
            .getattr("__init__")?
            .call((slf,), keywords)?;
        let figure: Option<String> = figure
            .filter(|value| !value.is_none())
            .map(|value| value.str().map(|text| text.to_string()))
            .transpose()?;
        {
            let mut me = slf.borrow_mut();
            *me = Self::blank(py);
            me.chord_kind = Some(kind.clone().unwrap_or_default());
            me.chord_kind_str = kindStr.clone().unwrap_or_default();
            me.figure = figure.clone();
        }
        let root = root
            .filter(|value| !value.is_none())
            .map(symbol_pitch)
            .transpose()?;
        let bass = bass
            .filter(|value| !value.is_none())
            .map(symbol_pitch)
            .transpose()?;
        let given_root = root.is_some();
        let given_bass = bass.is_some();
        Self::fix_root_and_bass(slf, root.clone(), bass.clone())?;
        let figured = figure.as_deref().is_some_and(|figure| !figure.is_empty());
        if figured {
            slf.call_method0("_parseFigure")?;
        }
        if Self::override_of(slf, "bass")?.is_none()
            && let Some(root) = Self::override_of(slf, "root")?
        {
            let keywords = PyDict::new(py);
            keywords.set_item("allow_add", true)?;
            slf.call_method("bass", (root,), Some(&keywords))?;
        }
        if updatePitches && (figured || given_root || given_bass) {
            slf.call_method0("_updatePitches")?;
        }
        // music21's `_updateFromParameters` once the notes are made: the
        // root fixed, then the inversion set, then the bass fixed again,
        // since setting an inversion lets go of the bass.
        Self::fix_root_and_bass(slf, root, None)?;
        if let Some(inversion) = inversion.filter(|value| !value.is_none()) {
            let keywords = PyDict::new(py);
            keywords.set_item("transposeOnSet", true)?;
            slf.call_method("inversion", (inversion,), Some(&keywords))?;
        }
        Self::fix_root_and_bass(slf, None, bass)?;
        let timed = keywords.is_some_and(|keywords| {
            keywords.contains("duration").unwrap_or(false)
                || keywords.contains("quarterLength").unwrap_or(false)
        });
        if !timed {
            let none = crate::duration::Duration::object(
                py,
                music21_rs_crate::Duration::new(0.0).map_err(harmony_error)?,
            )?;
            slf.setattr("duration", none)?;
        }
        if !kind.unwrap_or_default().is_empty() || !kindStr.unwrap_or_default().is_empty() {
            slf.call_method0("_updatePitches")?;
        }
        Ok(())
    }

    /// music21's `_updatePitches`: the notes the kind sounds on the root
    /// and bass the chord holds, with the chord-step modifications applied,
    /// root and bass then fixed on those notes.
    fn _updatePitches(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let (Some(root), Some(bass)) = (
            Self::override_of(slf, "root")?,
            Self::override_of(slf, "bass")?,
        ) else {
            return Ok(());
        };
        let Some(kind) = slf.borrow().chord_kind.clone() else {
            return Ok(());
        };
        let root = pitch_from_any(&root)?;
        let bass = pitch_from_any(&bass)?;
        let modifications: Vec<RsChordStepModification> = slf
            .borrow()
            .modifications
            .bind(py)
            .iter()
            .map(|each| modification_value(&each))
            .collect::<PyResult<_>>()?;
        let notation = live_notation(py, &kind);
        let voicing = voice_chord_notation(
            &root,
            &kind,
            notation.as_deref(),
            Some(&bass),
            &modifications,
        )
        .map_err(|error| {
            let said = crate::pitch::message(&error);
            if said.starts_with("Degree not in specified chord") {
                ChordStepModificationException::new_err(said)
            } else {
                harmony_error(error)
            }
        })?;
        let objects = voicing
            .pitches()
            .iter()
            .map(|pitch| {
                crate::installed_new(
                    py,
                    "music21.pitch",
                    "Pitch",
                    Pitch::wrap(pitch.clone(), false),
                )
            })
            .collect::<PyResult<Vec<_>>>()?;
        slf.setattr("pitches", PyTuple::new(py, &objects)?)?;
        // The root and bass are where music21's own objects end up, looked
        // up in the chord as its setters look: the note that is that pitch,
        // else the first of its name, else the pitch on its own. So `A10/C`
        // roots on the A3 it holds, `C7/B-` stands on its B-2, and
        // `Ab10/F#` roots on an A3 it does not sound.
        let held = |wanted: &RsPitch| -> PyResult<Py<Pitch>> {
            let same = |object: &&Py<Pitch>| {
                object.borrow(py).inner.name_with_octave() == wanted.name_with_octave()
            };
            let named = |object: &&Py<Pitch>| object.borrow(py).inner.name() == wanted.name();
            if let Some(found) = objects
                .iter()
                .find(same)
                .or_else(|| objects.iter().find(named))
            {
                return Ok(found.clone_ref(py));
            }
            crate::installed_new(
                py,
                "music21.pitch",
                "Pitch",
                Pitch::wrap(wanted.clone(), false),
            )
        };
        let keywords = PyDict::new(py);
        keywords.set_item("allow_add", true)?;
        slf.call_method("bass", (held(voicing.bass())?,), Some(&keywords))?;
        slf.call_method1("root", (held(voicing.root())?,))?;
        Ok(())
    }

    /// music21's `_parseFigure`, for a figure already set. The sentence
    /// music21 writes for a chord it cannot name is no figure to read.
    fn _parseFigure(slf: &Bound<'_, Self>) -> PyResult<()> {
        let figure = slf.borrow().figure.clone();
        match figure.filter(|figure| !figure.is_empty() && figure != UNIDENTIFIED) {
            Some(figure) => Self::parse_figure(slf, &figure),
            None => Ok(()),
        }
    }

    /// music21's `figure`: as written, or worked out from the notes where
    /// nothing was written or the chord has changed since.
    #[getter]
    fn get_figure(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        if let Some(figure) = slf.borrow().figure.clone() {
            return Ok(figure.into_pyobject(py)?.into_any().unbind());
        }
        Ok(Self::findFigure(slf)?.unbind())
    }

    #[setter]
    fn set_figure(slf: &Bound<'_, Self>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let figure = value
            .filter(|value| !value.is_none())
            .map(|value| value.str().map(|text| text.to_string()))
            .transpose()?;
        slf.borrow_mut().figure = figure.clone();
        if figure.is_some_and(|figure| !figure.is_empty()) {
            slf.call_method0("_parseFigure")?;
            slf.call_method0("_updatePitches")?;
        }
        Ok(())
    }

    /// music21's `findFigure`. A symbol with a kind or modifications is
    /// written from those — the root, the kind's abbreviation, the bass, and
    /// each modification spelled out as ` add b9` — since there is no reading
    /// a modified chord back off its notes; one with neither is read off its
    /// notes as `chordSymbolFigureFromChord` reads any chord.
    fn findFigure<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let (kind, modifications) = {
            let me = slf.borrow();
            (
                me.chord_kind.clone().unwrap_or_default(),
                me.modifications.clone_ref(py),
            )
        };
        let modifications = modifications.bind(py);
        if kind.is_empty() && modifications.is_empty() {
            return chordSymbolFigureFromChord(py, slf.as_any(), false);
        }
        let root = slf.call_method0("root")?;
        if root.is_none() {
            return Err(HarmonyException::new_err(
                "Cannot find figure. No root to the chord found",
            ));
        }
        let root_name: String = root.getattr("name")?.extract()?;
        let mut figure = root_name.clone();
        let kind = music21_rs_crate::chordsymbol::resolve_kind_alias(&kind);
        if music21_rs_crate::chordsymbol::notation_for_kind(kind).is_some() {
            figure.push_str(&abbreviation_for(py, kind));
        }
        let bass = slf.call_method0("bass")?;
        if !bass.is_none() {
            let bass_name: String = bass.getattr("name")?.extract()?;
            if bass_name != root_name {
                figure.push('/');
                figure.push_str(&bass_name);
            }
        }
        for modification in modifications.iter() {
            let mod_type: String = modification.getattr("modType")?.extract()?;
            let degree = modification.getattr("degree")?.str()?.to_string();
            let interval = modification.getattr("interval")?;
            let prefix = if interval.is_none() {
                String::new()
            } else {
                let semitones: f64 = interval.getattr("semitones")?.extract()?;
                let sign = if semitones > 0.0 { "#" } else { "b" };
                sign.repeat(semitones.abs().round() as usize)
            };
            figure.push_str(&format!(" {mod_type} {prefix}{degree}"));
        }
        Ok(figure.into_pyobject(py)?.into_any())
    }

    /// music21's `inversionIsValid`: whether the kind can stand in the
    /// inversion, read off the kind and not the notes.
    fn inversionIsValid(&self, inversion: Option<i64>) -> bool {
        let (Some(inversion), Some(kind)) = (inversion, &self.chord_kind) else {
            return false;
        };
        u8::try_from(inversion).is_ok_and(|inversion| inversion_is_valid_for_kind(kind, inversion))
    }

    /// music21's `transpose`, which clears the figure so that the next one
    /// read is worked out from the moved notes.
    #[pyo3(signature = (value, *, inPlace = false))]
    fn transpose(
        slf: &Bound<'_, Self>,
        value: &Bound<'_, PyAny>,
        inPlace: bool,
    ) -> PyResult<Option<Py<PyAny>>> {
        let py = slf.py();
        let keywords = PyDict::new(py);
        keywords.set_item("inPlace", inPlace)?;
        let moved = py
            .get_type::<Chord>()
            .getattr("transpose")?
            .call((slf, value), Some(&keywords))?;
        if inPlace {
            slf.borrow_mut().figure = None;
            return Ok(None);
        }
        let mut state = slf.borrow().copied_state(py)?;
        state.figure = None;
        Self::fill(&moved, state)?;
        Ok(Some(moved.unbind()))
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        py: Python<'py>,
        memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copy = py
            .get_type::<Chord>()
            .getattr("__deepcopy__")?
            .call1((slf, memo))?;
        Self::fill(&copy, slf.borrow().copied_state(py)?)?;
        Ok(copy)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let copy = py.get_type::<Chord>().getattr("__copy__")?.call1((slf,))?;
        Self::fill(&copy, slf.borrow().copied_state(py)?)?;
        Ok(copy)
    }

    /// A symbol is written out as a chord and read back, with what it says
    /// beside the notes frozen alongside.
    fn __reduce__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let (callable, arguments, state) = py
            .get_type::<Chord>()
            .getattr("__reduce__")?
            .call1((slf,))?
            .extract::<(Py<PyAny>, Py<PyAny>, Py<PyAny>)>()?;
        let me = slf.borrow();
        let symbol = PyDict::new(py);
        symbol.set_item("figure", me.figure.as_deref())?;
        symbol.set_item("chordKind", me.chord_kind.as_deref())?;
        symbol.set_item("chordKindStr", &me.chord_kind_str)?;
        symbol.set_item("chordStepModifications", me.modifications.bind(py))?;
        symbol.set_item("writeAsChord", me.write_as_chord)?;
        symbol.set_item("key", me.key.as_ref())?;
        symbol.set_item("placement", me.placement.as_ref())?;
        Ok(PyTuple::new(
            py,
            [
                callable,
                arguments,
                PyTuple::new(py, [state, symbol.into_any().unbind()])?
                    .into_any()
                    .unbind(),
            ],
        )?
        .into_any())
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let (chord, symbol) = match state.extract::<(Py<PyAny>, Py<PyAny>)>() {
            Ok(pair) if pair.1.bind(py).is_instance_of::<PyDict>() => pair,
            _ => {
                py.get_type::<Chord>()
                    .getattr("__setstate__")?
                    .call1((slf, state))?;
                return Ok(());
            }
        };
        py.get_type::<Chord>()
            .getattr("__setstate__")?
            .call1((slf, chord))?;
        let symbol = symbol.bind(py);
        let read = |name: &str| -> PyResult<Option<Bound<'_, PyAny>>> {
            Ok(symbol.get_item(name).ok().filter(|value| !value.is_none()))
        };
        let mut me = slf.borrow_mut();
        me.figure = read("figure")?.map(|value| value.extract()).transpose()?;
        me.chord_kind = read("chordKind")?
            .map(|value| value.extract())
            .transpose()?;
        me.chord_kind_str = read("chordKindStr")?
            .map(|value| value.extract())
            .transpose()?
            .unwrap_or_default();
        if let Some(list) = read("chordStepModifications")? {
            me.modifications = list.cast_into::<PyList>()?.unbind();
        }
        me.write_as_chord = read("writeAsChord")?
            .map(|value| value.extract())
            .transpose()?
            .unwrap_or(false);
        me.key = read("key")?.map(Bound::unbind);
        me.placement = read("placement")?.map(Bound::unbind);
        Ok(())
    }

    /// music21's `chordKind`: one of the kinds of its table, such as
    /// `'dominant-seventh'`.
    #[getter]
    fn get_chordKind(&self) -> Option<String> {
        self.chord_kind.clone()
    }

    #[setter]
    fn set_chordKind(&mut self, value: Option<String>) {
        self.chord_kind = value;
    }

    /// music21's `chordKindStr`: how the kind is written, where MusicXML
    /// says.
    #[getter]
    fn get_chordKindStr(&self) -> String {
        self.chord_kind_str.clone()
    }

    #[setter]
    fn set_chordKindStr(&mut self, value: Option<String>) {
        self.chord_kind_str = value.unwrap_or_default();
    }

    /// music21's `chordStepModifications`, the list itself.
    #[getter]
    fn get_chordStepModifications(&self, py: Python<'_>) -> Py<PyList> {
        self.modifications.clone_ref(py)
    }

    #[setter]
    fn set_chordStepModifications(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let list = PyList::empty(value.py());
        for item in value.walk()? {
            list.append(item?)?;
        }
        self.modifications = list.unbind();
        Ok(())
    }

    /// music21's `getChordStepModifications`.
    fn getChordStepModifications(&self, py: Python<'_>) -> Py<PyList> {
        self.modifications.clone_ref(py)
    }

    /// music21's `addChordStepModification`: a modification the symbol has
    /// not got is added, and by default the notes are sounded again.
    #[pyo3(signature = (degree, *, updatePitches = true))]
    fn addChordStepModification(
        slf: &Bound<'_, Self>,
        degree: &Bound<'_, PyAny>,
        updatePitches: bool,
    ) -> PyResult<()> {
        let py = slf.py();
        let is_modification = degree.hasattr("modType")? && degree.hasattr("degree")?;
        if !is_modification {
            return Err(HarmonyException::new_err(format!(
                "cannot add this object as a degree: {}",
                degree.str()?
            )));
        }
        let list = slf.borrow().modifications.clone_ref(py);
        if !list.bind(py).contains(degree)? {
            list.bind(py).append(degree)?;
        }
        if updatePitches {
            Self::_updatePitches(slf)?;
        }
        Ok(())
    }

    /// music21's `writeAsChord`: whether the notes are written out rather
    /// than the symbol. Writing them out gives a symbol that took no time a
    /// quarter note.
    #[getter]
    fn get_writeAsChord(&self) -> bool {
        self.write_as_chord
    }

    #[setter]
    fn set_writeAsChord(slf: &Bound<'_, Self>, value: bool) -> PyResult<()> {
        slf.borrow_mut().write_as_chord = value;
        if value {
            let length: f64 = slf.getattr("quarterLength")?.extract()?;
            if length == 0.0 {
                slf.setattr("quarterLength", 1.0)?;
            }
        }
        Ok(())
    }

    /// music21's `key`: the key the symbol is heard in, which says nothing
    /// to its notes and everything to its roman numeral.
    #[getter]
    fn get_key(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.key.as_ref().map(|key| key.clone_ref(py))
    }

    #[setter]
    fn set_key(&mut self, py: Python<'_>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.key = match value.filter(|value| !value.is_none()) {
            Some(value) if value.extract::<String>().is_ok() => {
                Some(key_class(py).call1((value,))?.unbind())
            }
            Some(value) => {
                self.roman = None;
                Some(value.clone().unbind())
            }
            None => {
                self.roman = None;
                None
            }
        };
        Ok(())
    }

    /// music21's `romanNumeral`: the chord read as a numeral in its key, or
    /// in the major key of its root where it has none.
    #[getter]
    fn get_romanNumeral(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        if let Some(roman) = &slf.borrow().roman {
            return Ok(roman.clone_ref(py));
        }
        if slf.getattr("pitches")?.len()? == 0 {
            return Ok(numeral_class(py).call0()?.unbind());
        }
        let stored = slf.borrow().write_as_chord;
        slf.borrow_mut().write_as_chord = true;
        let held = slf.borrow().key.as_ref().map(|key| key.clone_ref(py));
        let key = match held {
            Some(key) => key.into_bound(py),
            None => {
                let root = slf.call_method0("root")?;
                key_class(py).call1((root,))?
            }
        };
        let roman = crate::roman::romanNumeralFromChord(py, slf.as_any(), Some(&key), false)?
            .into_bound(py)
            .into_any();
        slf.borrow_mut().write_as_chord = stored;
        slf.borrow_mut().roman = Some(roman.clone().unbind());
        Ok(roman.unbind())
    }

    #[setter]
    fn set_romanNumeral(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let is_numeral = value
            .getattr("classes")
            .ok()
            .is_some_and(|classes| classes.contains("RomanNumeral").unwrap_or(false));
        if is_numeral {
            slf.borrow_mut().roman = Some(value.clone().unbind());
            return Ok(());
        }
        match numeral_class(py).call1((value,)) {
            Ok(numeral) => {
                slf.borrow_mut().roman = Some(numeral.unbind());
                Ok(())
            }
            Err(_) => Err(HarmonyException::new_err(format!(
                "not a valid pitch specification: {}",
                value.str()?
            ))),
        }
    }

    /// music21's `_roman`, the numeral worked out so far or `None`, which
    /// its MusicXML writer reads directly.
    #[getter(_roman)]
    fn get_roman_slot(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.roman.as_ref().map(|roman| roman.clone_ref(py))
    }

    #[setter(_roman)]
    fn set_roman_slot(&mut self, value: Option<&Bound<'_, PyAny>>) {
        self.roman = value
            .filter(|value| !value.is_none())
            .map(|value| value.clone().unbind());
    }

    /// music21's `placement`: above or below the staff, where a score says.
    #[getter]
    fn get_placement(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.placement.as_ref().map(|value| value.clone_ref(py))
    }

    #[setter]
    fn set_placement(&mut self, value: Option<&Bound<'_, PyAny>>) {
        self.placement = value
            .filter(|value| !value.is_none())
            .map(|value| value.clone().unbind());
    }

    /// music21's sort order for a harmony in a stream: one before any
    /// other object at the same offset, so a symbol comes before the notes
    /// it names.
    #[classattr]
    fn classSortOrder() -> i32 {
        19
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let figure: String = Self::get_figure(slf)?.bind(slf.py()).str()?.to_string();
        let mut summary = figure;
        if slf.borrow().write_as_chord {
            let names: Vec<String> = slf
                .getattr("pitches")?
                .walk()?
                .map(|pitch| pitch?.getattr("name")?.extract::<String>())
                .collect::<PyResult<_>>()?;
            summary.push_str(": ");
            summary.push_str(&names.join(" "));
        }
        let class = slf.get_type().qualname()?;
        Ok(if summary.is_empty() {
            format!("<music21.harmony.{class}>")
        } else {
            format!("<music21.harmony.{class} {summary}>")
        })
    }
}

/// The crate's value of a chord symbol, which a stream is read into: its
/// root, kind and bass, and the time it holds.
///
/// A symbol with no root -- `NoChord` -- sounds nothing, and the crate has no
/// value for that; it is held by a C major triad, which nothing a stream is
/// read for asks the sound of. Only its place and its length are read.
pub(crate) fn crate_value(slf: &Bound<'_, ChordSymbol>) -> PyResult<RsChordSymbol> {
    let root = slf.call_method0("root")?;
    let mut symbol = if root.is_none() {
        RsChordSymbol::from_kind(
            RsPitch::from_name("C").map_err(harmony_error)?,
            "major",
            None,
        )
        .map_err(harmony_error)?
    } else {
        let root = pitch_from_any(&root)?;
        let bass = slf.call_method0("bass")?;
        let bass = if bass.is_none() {
            None
        } else {
            Some(pitch_from_any(&bass)?)
        };
        let kind = slf.borrow().chord_kind.clone().unwrap_or_default();
        match RsChordSymbol::from_kind(root.clone(), &kind, bass.clone()) {
            Ok(symbol) => symbol,
            // A kind music21's live table has and the crate's has not.
            Err(_) => {
                let mut symbol = RsChordSymbol::parse_music21(music21_name(&root.name()))
                    .map_err(harmony_error)?;
                symbol.set_root(root);
                symbol.set_bass(bass);
                symbol.with_kind(&kind)
            }
        }
    };
    let length: f64 = slf
        .getattr("duration")?
        .getattr("quarterLength")?
        .extract()?;
    symbol.set_duration(music21_rs_crate::Duration::new(length).map_err(harmony_error)?);
    Ok(symbol)
}

/// A figure read against music21's live table of kinds, which a program may
/// have added to with `addNewChordSymbol`: a root and exactly one of the
/// abbreviations music21 now holds for a kind the crate's table has not got.
fn live_kind(py: Python<'_>, figure: &str) -> PyResult<Option<RsChordSymbol>> {
    let Ok(table) = py
        .import("music21.harmony")
        .and_then(|harmony| harmony.getattr("CHORD_TYPES"))
    else {
        return Ok(None);
    };
    let compact: String = figure.chars().filter(|c| !c.is_whitespace()).collect();
    let mut letters = compact.char_indices();
    if !letters
        .next()
        .is_some_and(|(_, first)| "ABCDEFGabcdefg".contains(first))
    {
        return Ok(None);
    }
    let end = letters
        .find(|(_, c)| !matches!(c, '#' | '-'))
        .map_or(compact.len(), |(at, _)| at);
    let (root, written) = compact.split_at(end);
    let Ok(root) = RsPitch::from_name(root) else {
        return Ok(None);
    };
    for item in table.call_method0("items")?.walk()? {
        let (kind, entry): (String, Bound<'_, PyAny>) = item?.extract()?;
        // A kind the crate knows is read by the crate's reading, which
        // refuses what music21 refuses; only one added since is looked up.
        if music21_rs_crate::chordsymbol::notation_for_kind(&kind).is_some() {
            continue;
        }
        let abbreviations: Vec<String> = entry.get_item(1)?.extract()?;
        if abbreviations.iter().any(|each| each == written) {
            let mut symbol =
                RsChordSymbol::parse_music21(music21_name(&root.name())).map_err(harmony_error)?;
            symbol.set_root(root);
            return Ok(Some(symbol.with_kind(&kind)));
        }
    }
    Ok(None)
}

/// A kind's notation, from music21's live table where there is one and the
/// crate's own where not.
fn live_notation(py: Python<'_>, kind: &str) -> Option<String> {
    let resolved = music21_rs_crate::chordsymbol::resolve_kind_alias(kind);
    match py
        .import("music21.harmony")
        .and_then(|harmony| harmony.getattr("CHORD_TYPES"))
    {
        Ok(table) => table
            .get_item(resolved)
            .ok()
            .and_then(|entry| entry.get_item(0).ok())
            .and_then(|notation| notation.extract::<String>().ok()),
        Err(_) => music21_rs_crate::chordsymbol::notation_for_kind(resolved).map(str::to_string),
    }
}

/// music21's `harmony.NoChord`: a symbol saying that nothing sounds, so a
/// chord before it stops there. It has no root, no bass and no notes, and is
/// written `N.C.` unless it says otherwise.
#[pyclass(
    name = "NoChord",
    module = "music21.harmony",
    extends = ChordSymbol,
    subclass,
    skip_from_py_object
)]
pub struct NoChord;

#[pymethods]
impl NoChord {
    #[new]
    #[pyo3(signature = (*_arguments, **_keywords))]
    fn new(
        py: Python<'_>,
        _arguments: &Bound<'_, PyTuple>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(PyClassInitializer::from(Chord::build(py, None, None)?)
            .add_subclass(ChordSymbol::blank(py))
            .add_subclass(NoChord))
    }

    #[pyo3(signature = (figure = None, kind = Some("none".to_string()), kindStr = None, **keywords))]
    fn __init__(
        slf: &Bound<'_, Self>,
        figure: Option<&Bound<'_, PyAny>>,
        kind: Option<String>,
        kindStr: Option<String>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let py = slf.py();
        let written: Option<String> = figure
            .filter(|value| !value.is_none())
            .map(|value| value.str().map(|text| text.to_string()))
            .transpose()?;
        let kind_str = kindStr
            .filter(|text| !text.is_empty())
            .or_else(|| written.clone().filter(|text| !text.is_empty()))
            .unwrap_or_else(|| "N.C.".to_string());
        let arguments = PyDict::new(py);
        if let Some(keywords) = keywords {
            arguments.update(keywords.as_mapping())?;
        }
        arguments.set_item("figure", written.as_deref())?;
        arguments.set_item("kind", kind)?;
        arguments.set_item("kindStr", &kind_str)?;
        py.get_type::<ChordSymbol>()
            .getattr("__init__")?
            .call((slf,), Some(&arguments))?;
        let symbol = slf.as_super();
        let mut symbol = symbol.borrow_mut();
        if symbol.figure.is_none() {
            symbol.figure = Some(kind_str);
        }
        Ok(())
    }

    /// A no-chord has no root, whatever it is told.
    #[pyo3(signature = (newroot = None, *, find = None))]
    fn root(&self, newroot: Option<&Bound<'_, PyAny>>, find: Option<bool>) -> Option<Py<PyAny>> {
        let _ = (newroot, find);
        None
    }

    /// Nor a bass.
    #[pyo3(signature = (newbass = None, *, find = None, allow_add = false))]
    fn bass(
        &self,
        newbass: Option<&Bound<'_, PyAny>>,
        find: Option<bool>,
        allow_add: bool,
    ) -> Option<Py<PyAny>> {
        let _ = (newbass, find, allow_add);
        None
    }

    /// Everything a no-chord says is set already; there is no figure to read.
    fn _parseFigure(&self) {}

    /// A no-chord moved anywhere is still a no-chord: a copy of it, or
    /// nothing done in place.
    #[pyo3(signature = (_value, *, inPlace = false))]
    fn transpose(
        slf: &Bound<'_, Self>,
        _value: &Bound<'_, PyAny>,
        inPlace: bool,
    ) -> PyResult<Option<Py<PyAny>>> {
        if inPlace {
            return Ok(None);
        }
        let py = slf.py();
        Ok(Some(
            py.import("copy")?
                .getattr("deepcopy")?
                .call1((slf,))?
                .unbind(),
        ))
    }
}

/// The abbreviation a kind is written with: the first music21's live table
/// gives, since `changeAbbreviationFor` edits it, or the crate's own where
/// there is no music21 to ask.
fn abbreviation_for(py: Python<'_>, kind: &str) -> String {
    py.import("music21.harmony")
        .and_then(|harmony| harmony.getattr("getAbbreviationListGivenChordType"))
        .and_then(|lookup| lookup.call1((kind,)))
        .and_then(|list| list.get_item(0))
        .and_then(|first| first.extract::<String>())
        .unwrap_or_else(|_| {
            music21_rs_crate::chordsymbol::current_abbreviation_for_kind(kind)
                .unwrap_or("")
                .to_string()
        })
}

/// The key class a symbol builds, installed over music21's where it is.
fn key_class(py: Python<'_>) -> Bound<'_, PyAny> {
    crate::installed_class(py, "music21.key", "Key")
        .unwrap_or_else(|| py.get_type::<crate::key::Key>().into_any())
}

/// The roman numeral class a symbol builds, installed over music21's where
/// it is.
fn numeral_class(py: Python<'_>) -> Bound<'_, PyAny> {
    crate::installed_class(py, "music21.roman", "RomanNumeral")
        .unwrap_or_else(|| py.get_type::<crate::roman::RomanNumeral>().into_any())
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(getAbbreviationListGivenChordType, m)?)?;
    m.add_function(wrap_pyfunction!(getCurrentAbbreviationFor, m)?)?;
    m.add_function(wrap_pyfunction!(getNotationStringGivenChordType, m)?)?;
    m.add_function(wrap_pyfunction!(chordSymbolFigureFromChord, m)?)?;
    m.add_function(wrap_pyfunction!(chordSymbolFromChord, m)?)?;
    m.add_class::<ChordSymbol>()?;
    m.add_class::<ChordStepModification>()?;
    m.add_class::<NoChord>()?;
    Ok(())
}
