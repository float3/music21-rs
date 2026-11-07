//! music21's `braille.segment`: a part cut into segments of groupings,
//! and the groupings themselves -- a run of elements braille writes
//! together.
//!
//! What a grouping *is* is a list of the score's own elements and the
//! signatures and flags they are written under, which is what music21's is;
//! the crate's `braille::segment::BrailleElementGrouping` is the same thing
//! as values, and `crate_grouping` reads one out of the other so that the
//! transcriber can be asked.
//!
//! A segment is music21's dictionary of those groupings by key, and
//! `findSegments` builds them of the part's own objects. The cutting, the
//! grouping attributes, the articulations and the writing are the crate's
//! (`braille::segment`): the part is read with the object at each leaf, and
//! each element the crate hands back is mapped to the object it was read
//! from. What music21 writes onto those objects while it works -- slur and
//! beam marks, affinity codes, sort orders, English, whole-measure rests,
//! a key signature's predecessor, ties and articulations -- is written onto
//! them here too, so code that reads them after `findSegments` reads what
//! it read under music21. An object of this package's own that takes no
//! attribute of its own has the marks written onto its editorial instead.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use std::collections::{BTreeMap, VecDeque};

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use music21::braille::basic::NoteContext;
use music21::braille::segment::{
    Affinity, BrailleElement, BrailleElementGrouping as RsGrouping, BrailleGrandSegment as RsGrand,
    BrailleSegment as RsSegment, Hand, KeyPair, SegmentKey, SegmentOptions, SegmentTranscriber,
    affinity_code, extract_braille_elements, get_raw_segments, prepare_beamed_notes,
    prepare_slurred_notes, split_note_grouping,
};
use music21::braille::text::{BrailleKeyboard as RsKeyboard, BrailleText as RsText};
use music21::meter::TimeSignature;
use music21::pitch::Pitch;
use music21::stream::{Stream as RsStream, StreamElement, StreamKind};

use crate::Walkable;
use crate::brailletext::BrailleText;

/// The names this module hands out.
pub const NAMES: &[&str] = &[
    "BrailleElementGrouping",
    "BrailleSegment",
    "BrailleGrandSegment",
    "findSegments",
    "prepareSlurredNotes",
    "getRawSegments",
    "extractBrailleElements",
    "prepareBeamedNotes",
    "setAffinityCode",
    "areGroupingsIdentical",
    "splitNoteGrouping",
];

pyo3::create_exception!(
    music21_rs_facade,
    BrailleSegmentException,
    crate::Music21Exception
);

/// music21's `braille.segment.BrailleElementGrouping`: the elements
/// braille writes together, and what they are written under.
///
/// music21's mimics a list, so this one answers `len`, indexing,
/// assignment and iteration, and hands anything else on to the list it
/// holds -- its own `__getattr__` does the same, which is what makes
/// `grouping.append(note)` work.
#[pyclass(
    name = "BrailleElementGrouping",
    module = "music21.braille.segment",
    subclass,
    skip_from_py_object
)]
pub struct BrailleElementGrouping {
    /// The elements themselves, the score's own objects.
    #[pyo3(get, set)]
    internalList: Py<PyList>,
    /// The key signature in force.
    #[pyo3(get, set)]
    keySignature: Py<PyAny>,
    /// The time signature in force.
    #[pyo3(get, set)]
    timeSignature: Py<PyAny>,
    /// Whether a chord is written from the top down.
    #[pyo3(get, set)]
    descendingChords: Py<PyAny>,
    /// Whether clefs are written.
    #[pyo3(get, set)]
    showClefSigns: bool,
    /// Whether a fingering's upper choice is written first.
    #[pyo3(get, set)]
    upperFirstInNoteFingering: bool,
    /// Whether the grouping ends with a music hyphen.
    #[pyo3(get, set)]
    withHyphen: bool,
    /// How many times it is repeated after itself.
    #[pyo3(get, set)]
    numRepeats: usize,
}

/// The signatures a grouping starts with: music21's `GROUPING_GLOBALS`,
/// which it builds on first use and shares between groupings, and this
/// package's own key and meter where music21 has not been imported.
fn grouping_globals(py: Python<'_>) -> PyResult<(Py<PyAny>, Py<PyAny>)> {
    let imported = py
        .import("sys")?
        .getattr("modules")?
        .contains("music21.braille.segment")?;
    if imported && let Ok(module) = crate::real_music21(py, "music21.braille.segment") {
        module.call_method0("setGroupingGlobals")?;
        let globals = module.getattr("GROUPING_GLOBALS")?;
        let key = globals.get_item("keySignature")?;
        let meter = globals.get_item("timeSignature")?;
        if !key.is_none() && !meter.is_none() {
            return Ok((key.unbind(), meter.unbind()));
        }
    }
    // The class standing in music21's module where anything is installed
    // there, and this package's own otherwise.
    let class = |module: &str, name: &str| -> PyResult<Bound<'_, PyAny>> {
        if let Some(installed) = crate::installed_class(py, module, name) {
            return Ok(installed);
        }
        py.import("music21_rs")
            .or_else(|_| py.import("music21_rs_facade"))?
            .getattr(name)
    };
    let key = class("music21.key", "KeySignature")?.call1((0,))?.unbind();
    let meter = class("music21.meter.base", "TimeSignature")?
        .call1(("4/4",))?
        .unbind();
    Ok((key, meter))
}

/// The sharps a key signature object is written with, or nothing where it
/// is one of music21's non-traditional signatures.
fn sharps_of(object: &Bound<'_, PyAny>) -> Option<music21::IntegerType> {
    if object
        .getattr("isNonTraditional")
        .and_then(|flag| flag.is_truthy())
        .unwrap_or(false)
    {
        return None;
    }
    object
        .getattr("sharps")
        .and_then(|sharps| sharps.extract())
        .ok()
}

/// How a note stands among its neighbours, which a segment writes onto it.
fn note_context(object: &Bound<'_, PyAny>) -> NoteContext {
    let said = |name: &str| {
        marked(object, name)
            .and_then(|flag| flag.is_truthy().ok())
            .unwrap_or(false)
    };
    NoteContext {
        begin_long_bracket_slur: said("beginLongBracketSlur"),
        end_long_bracket_slur: said("endLongBracketSlur"),
        begin_long_double_slur: said("beginLongDoubleSlur"),
        end_long_double_slur: said("endLongDoubleSlur"),
        short_slur: said("shortSlur"),
        beam_start: said("beamStart"),
        beam_continue: said("beamContinue"),
    }
}

/// One element of a grouping as the crate's value, with what the object
/// says about how it is written.
pub(crate) fn crate_element(object: &Bound<'_, PyAny>) -> PyResult<BrailleElement> {
    let Some(element) = crate::stream::element_read(object)? else {
        return Err(BrailleSegmentException::new_err(format!(
            "{} cannot be transcribed to braille.",
            object
                .repr()
                .map(|repr| repr.to_string())
                .unwrap_or_default()
        )));
    };
    Ok(BrailleElement {
        element,
        offset: object
            .getattr("offset")
            .and_then(|offset| offset.extract())
            .unwrap_or(0.0),
        affinity: Affinity::NoteGrouping,
        sort_order: 0,
        english: None,
        context: note_context(object),
        // music21 writes the signature a key signature cancels onto the
        // element while it is making the groupings.
        outgoing_key: marked(object, "outgoingKeySig")
            .filter(|given| !given.is_none())
            .map(|given| sharps_of(&given)),
        leaf: 0,
        voice_english: Vec::new(),
    })
}

/// A grouping as the crate's value, with the objects it was read from so
/// that what each sign says can be written back onto them.
pub(crate) fn crate_grouping(
    grouping: &Bound<'_, PyAny>,
) -> PyResult<(RsGrouping, Vec<Py<PyAny>>)> {
    let mut elements = Vec::new();
    let mut objects = Vec::new();
    for object in grouping.getattr("internalList")?.walk()? {
        let object = object?;
        elements.push(crate_element(&object)?);
        objects.push(object.unbind());
    }
    let mut read = grouping_attributes(grouping)?;
    read.elements = elements;
    Ok((read, objects))
}

/// What a grouping says of itself, as the crate's grouping with no
/// elements yet.
fn grouping_attributes(grouping: &Bound<'_, PyAny>) -> PyResult<RsGrouping> {
    let key = grouping.getattr("keySignature")?;
    let meter = grouping.getattr("timeSignature")?;
    Ok(RsGrouping {
        elements: Vec::new(),
        key_signature: if key.is_none() {
            Some(0)
        } else {
            sharps_of(&key)
        },
        time_signature: match crate::stream::element_read(&meter)? {
            Some(StreamElement::TimeSignature(meter)) => meter,
            // A grouping with no meter of its own is written in four-four,
            // as music21's default is.
            _ => TimeSignature::new(4, 4).map_err(|error| crate::stream::stream_error(&error))?,
        },
        descending_chords: {
            let given = grouping.getattr("descendingChords")?;
            if given.is_none() {
                None
            } else {
                Some(given.is_truthy()?)
            }
        },
        show_clef_signs: grouping.getattr("showClefSigns")?.is_truthy()?,
        upper_first_in_note_fingering: grouping
            .getattr("upperFirstInNoteFingering")?
            .is_truthy()?,
        with_hyphen: grouping.getattr("withHyphen")?.is_truthy()?,
        num_repeats: grouping.getattr("numRepeats")?.extract()?,
    })
}

/// What each sign says, written onto the object music21 writes it onto.
pub(crate) fn write_english(
    py: Python<'_>,
    objects: &[Py<PyAny>],
    elements: &[BrailleElement],
) -> PyResult<()> {
    for (object, element) in objects.iter().zip(elements) {
        let Some(english) = &element.english else {
            continue;
        };
        let listed = PyList::empty(py);
        for line in english {
            listed.append(line)?;
        }
        if let Ok(editorial) = object.bind(py).getattr("editorial") {
            editorial.setattr("brailleEnglish", listed)?;
        }
    }
    Ok(())
}

impl BrailleElementGrouping {
    /// The parts a copy or a pickle is made of.
    fn state<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, pyo3::types::PyDict>> {
        let py = slf.py();
        let me = slf.borrow();
        let state = pyo3::types::PyDict::new(py);
        state.set_item("internalList", me.internalList.bind(py))?;
        state.set_item("keySignature", me.keySignature.bind(py))?;
        state.set_item("timeSignature", me.timeSignature.bind(py))?;
        state.set_item("descendingChords", me.descendingChords.bind(py))?;
        state.set_item("showClefSigns", me.showClefSigns)?;
        state.set_item("upperFirstInNoteFingering", me.upperFirstInNoteFingering)?;
        state.set_item("withHyphen", me.withHyphen)?;
        state.set_item("numRepeats", me.numRepeats)?;
        Ok(state)
    }

    /// A copy of the grouping, the elements copied where a memo is given
    /// and shared where none is.
    fn copied(
        slf: &Bound<'_, Self>,
        memo: Option<Option<&Bound<'_, PyAny>>>,
    ) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let state = Self::state(slf)?;
        let state = match memo {
            Some(memo) => {
                let copier = py.import("copy")?.getattr("deepcopy")?;
                let copied = match memo {
                    Some(memo) => copier.call1((&state, memo))?,
                    None => copier.call1((&state,))?,
                };
                copied
                    .cast_into::<pyo3::types::PyDict>()
                    .map_err(PyErr::from)?
            }
            None => {
                // Shallow: a new list over the same elements.
                let listed = PyList::empty(py);
                for element in state.get_item("internalList")?.unwrap().try_iter()? {
                    listed.append(element?)?;
                }
                state.set_item("internalList", listed)?;
                state
            }
        };
        let made = crate::blank_installed(&slf.get_type())?;
        Self::__setstate__(made.cast::<Self>()?, state.as_any())?;
        Ok(made.unbind())
    }
}

#[pymethods]
impl BrailleElementGrouping {
    /// music21's `BrailleElementGrouping(*listElements)`.
    #[new]
    #[pyo3(signature = (*listElements))]
    fn new(py: Python<'_>, listElements: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<Self> {
        let held = PyList::empty(py);
        // music21 takes one iterable of elements, as its own
        // `BrailleElementGrouping(*listElements)` unpacks.
        for given in listElements.iter() {
            for element in given.walk()? {
                held.append(element?)?;
            }
        }
        let (keySignature, timeSignature) = grouping_globals(py)?;
        Ok(Self {
            internalList: held.unbind(),
            keySignature,
            timeSignature,
            descendingChords: true.into_pyobject(py)?.to_owned().into_any().unbind(),
            showClefSigns: false,
            upperFirstInNoteFingering: true,
            withHyphen: false,
            numRepeats: 0,
        })
    }

    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, pyo3::types::PyIterator>> {
        self.internalList.bind(py).as_any().try_iter()
    }

    fn __len__(&self, py: Python<'_>) -> usize {
        self.internalList.bind(py).len()
    }

    fn __getitem__<'py>(
        &self,
        py: Python<'py>,
        item: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.internalList.bind(py).as_any().get_item(item)
    }

    fn __setitem__(
        &self,
        py: Python<'_>,
        pos: &Bound<'_, PyAny>,
        item: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.internalList.bind(py).as_any().set_item(pos, item)
    }

    /// Anything else is the list's, as music21's own `__getattr__` hands
    /// it on: `grouping.append(note)` is the list's `append`.
    fn __getattr__<'py>(&self, py: Python<'py>, attr: &str) -> PyResult<Bound<'py, PyAny>> {
        if attr.starts_with("__") {
            return Err(pyo3::exceptions::PyAttributeError::new_err(
                attr.to_string(),
            ));
        }
        self.internalList.bind(py).as_any().getattr(attr)
    }

    /// Every element as what its signs say, which is how music21 writes a
    /// grouping out: each element's English, a voice's elements in turn,
    /// and a note of the repeats and the hyphen at the end.
    fn __str__(&self, py: Python<'_>) -> PyResult<String> {
        use music21::braille::lookup::symbol;
        let mut written: Vec<String> = Vec::new();
        let said = |object: &Bound<'_, PyAny>| -> String {
            match object
                .getattr("editorial")
                .and_then(|editorial| editorial.getattr("brailleEnglish"))
            {
                Ok(english) if !english.is_none() => match english.extract::<Vec<String>>() {
                    Ok(lines) => lines.join("\n"),
                    Err(_) => object
                        .str()
                        .map(|text| text.to_string())
                        .unwrap_or_default(),
                },
                _ => object
                    .str()
                    .map(|text| text.to_string())
                    .unwrap_or_default(),
            }
        };
        let mut previous_was_voice = false;
        for object in self.internalList.bind(py).iter() {
            let is_voice = object
                .getattr("classes")
                .and_then(|classes| classes.contains("Voice"))
                .unwrap_or(false);
            if is_voice {
                if previous_was_voice {
                    written.push(format!("full inaccord {}", symbol("full_inaccord")));
                }
                for inner in object.try_iter()? {
                    written.push(said(&inner?));
                }
                previous_was_voice = true;
                continue;
            }
            written.push(said(&object));
            previous_was_voice = false;
        }
        if self.numRepeats > 0 {
            written.push(format!("** Grouping x {} **", self.numRepeats + 1));
        }
        if self.withHyphen {
            written.push(format!("music hyphen {}", symbol("music_hyphen")));
        }
        Ok(written.join("\n"))
    }

    /// A copy of the grouping: music21's own `splitNoteGrouping` takes
    /// one and empties its list, so the elements are shared and only the
    /// list itself is new -- which is what `copy.copy` of music21's own
    /// grouping leaves it with.
    fn __copy__(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        Self::copied(slf, None)
    }

    /// The same with the elements copied too, through the copier's memo.
    #[pyo3(signature = (memo = None))]
    fn __deepcopy__(slf: &Bound<'_, Self>, memo: Option<&Bound<'_, PyAny>>) -> PyResult<Py<PyAny>> {
        Self::copied(slf, Some(memo))
    }

    /// What a pickle writes: the class and the parts, as every facade
    /// here pickles itself rather than leaving it to music21.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let state = Self::state(slf)?;
        let made = pyo3::types::PyTuple::new(
            py,
            [
                slf.get_type().into_any(),
                pyo3::types::PyTuple::empty(py).into_any(),
                state.into_any(),
            ],
        )?;
        Ok(made.into_any().unbind())
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let read = |name: &str| -> PyResult<Option<Bound<'_, PyAny>>> {
            Ok(state.get_item(name).ok().filter(|value| !value.is_none()))
        };
        let mut me = slf.borrow_mut();
        if let Some(held) = read("internalList")? {
            let listed = PyList::empty(py);
            for element in held.try_iter()? {
                listed.append(element?)?;
            }
            me.internalList = listed.unbind();
        }
        if let Some(key) = read("keySignature")? {
            me.keySignature = key.unbind();
        }
        if let Some(meter) = read("timeSignature")? {
            me.timeSignature = meter.unbind();
        }
        if let Some(descending) = read("descendingChords")? {
            me.descendingChords = descending.unbind();
        }
        if let Some(shown) = read("showClefSigns")? {
            me.showClefSigns = shown.is_truthy()?;
        }
        if let Some(upper) = read("upperFirstInNoteFingering")? {
            me.upperFirstInNoteFingering = upper.is_truthy()?;
        }
        if let Some(hyphen) = read("withHyphen")? {
            me.withHyphen = hyphen.is_truthy()?;
        }
        if let Some(repeats) = read("numRepeats")? {
            me.numRepeats = repeats.extract()?;
        }
        Ok(())
    }

    /// music21's own `repr`, which is the list's.
    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!(
            "<music21.braille.segment.BrailleElementGrouping {}>",
            self.internalList.bind(py).as_any().repr()?
        ))
    }
}

// ---------------------------------------------------------------------------
// Keys, affinities and the marks music21 writes on what it transcribes.

/// One of music21's own names in its `braille.segment`, where music21 has
/// already been imported: a wheel standing on its own must not pull the
/// whole of music21 in to name a key.
fn music21s<'py>(py: Python<'py>, name: &str) -> Option<Bound<'py, PyAny>> {
    let modules = py.import("sys").ok()?.getattr("modules").ok()?;
    modules
        .get_item("music21.braille.segment")
        .ok()?
        .getattr(name)
        .ok()
}

/// music21's own `SegmentKey` where music21 is imported, and a named tuple
/// of the same fields and defaults where it is not: a key is four values,
/// nothing musical, so music21's class is the better one, as with
/// `music21_or_namedtuple`.
fn segment_key_class(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    static BUILT: pyo3::sync::PyOnceLock<Py<PyAny>> = pyo3::sync::PyOnceLock::new();
    if let Some(found) = music21s(py, "SegmentKey") {
        return Ok(found);
    }
    let class = BUILT.get_or_try_init(py, || -> PyResult<Py<PyAny>> {
        let keywords = PyDict::new(py);
        keywords.set_item("defaults", (0, 0, py.None(), py.None()))?;
        keywords.set_item("module", "music21.braille.segment")?;
        let made = py.import("collections")?.getattr("namedtuple")?.call(
            ("SegmentKey", ["measure", "ordinal", "affinity", "hand"]),
            Some(&keywords),
        )?;
        Ok(made.unbind())
    })?;
    Ok(class.bind(py).clone())
}

/// music21's own `Affinity` where music21 is imported, and the same
/// `IntEnum` built here where it is not.
fn affinity_class(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    static BUILT: pyo3::sync::PyOnceLock<Py<PyAny>> = pyo3::sync::PyOnceLock::new();
    if let Some(found) = music21s(py, "Affinity") {
        return Ok(found);
    }
    let class = BUILT.get_or_try_init(py, || -> PyResult<Py<PyAny>> {
        let members = PyList::empty(py);
        for (name, value) in [
            ("_LOWEST", -1),
            ("SIGNATURE", 3),
            ("TTEXT", 4),
            ("MMARK", 5),
            ("LONG_TEXTEXPR", 6),
            ("INACCORD", 7),
            ("SPLIT1_NOTEGROUP", 8),
            ("NOTEGROUP", 9),
            ("SPLIT2_NOTEGROUP", 10),
        ] {
            members.append((name, value))?;
        }
        let keywords = PyDict::new(py);
        keywords.set_item("module", "music21.braille.segment")?;
        let made = py
            .import("enum")?
            .getattr("IntEnum")?
            .call(("Affinity", members), Some(&keywords))?;
        Ok(made.unbind())
    })?;
    Ok(class.bind(py).clone())
}

/// The crate's affinity for music21's number for one, nothing for a number
/// music21 has no grouping of.
fn affinity_from(code: i64) -> Option<Affinity> {
    Some(match code {
        3 => Affinity::Signature,
        4 => Affinity::TempoText,
        5 => Affinity::MetronomeMark,
        6 => Affinity::LongTextExpression,
        7 => Affinity::Inaccord,
        8 => Affinity::SplitNoteGroupingA,
        9 => Affinity::NoteGrouping,
        10 => Affinity::SplitNoteGroupingB,
        _ => return None,
    })
}

/// music21's `Affinity` member for one of the crate's.
fn affinity_object<'py>(py: Python<'py>, affinity: Affinity) -> PyResult<Bound<'py, PyAny>> {
    affinity_class(py)?.call1((affinity as i64,))
}

/// A key of music21's read as the crate's.
fn rs_key(key: &Bound<'_, PyAny>) -> PyResult<SegmentKey> {
    let code: i64 = key.getattr("affinity")?.extract()?;
    let affinity = affinity_from(code).ok_or_else(|| {
        BrailleSegmentException::new_err(format!("{code} is not an affinity braille groups by"))
    })?;
    let hand = key.getattr("hand")?;
    let hand = if hand.is_none() {
        None
    } else {
        match hand.extract::<String>()?.as_str() {
            "right" => Some(Hand::Right),
            "left" => Some(Hand::Left),
            other => {
                return Err(BrailleSegmentException::new_err(format!(
                    "{other} is not a hand braille knows"
                )));
            }
        }
    };
    Ok(SegmentKey {
        measure: key.getattr("measure")?.extract()?,
        ordinal: key.getattr("ordinal")?.extract()?,
        affinity,
        hand,
    })
}

/// One of the crate's keys as music21's `SegmentKey`.
fn py_key(py: Python<'_>, key: SegmentKey) -> PyResult<Py<PyAny>> {
    let hand = match key.hand {
        Some(Hand::Right) => Some("right"),
        Some(Hand::Left) => Some("left"),
        None => None,
    };
    Ok(segment_key_class(py)?
        .call1((
            key.measure,
            key.ordinal,
            affinity_object(py, key.affinity)?,
            hand,
        ))?
        .unbind())
}

/// The marks music21 sets on a note for the time it is transcribed, and
/// takes off again once it is: `basic.TEMPORARY_ATTRIBUTES`.
const TEMPORARY_ATTRIBUTES: [&str; 7] = [
    "beginLongBracketSlur",
    "endLongBracketSlur",
    "beginLongDoubleSlur",
    "endLongDoubleSlur",
    "shortSlur",
    "beamStart",
    "beamContinue",
];

/// Writes a mark onto an object, as music21 writes it. An object of this
/// package's own, which keeps no attributes beyond its own, has it written
/// onto its editorial instead, which every element has and which
/// [`note_context`] reads it back from.
fn mark<'py>(object: &Bound<'py, PyAny>, name: &str, value: impl IntoPyObject<'py> + Clone) {
    if object.setattr(name, value.clone()).is_ok() {
        return;
    }
    if let Ok(editorial) = object.getattr("editorial") {
        let _ = editorial.setattr(name, value);
    }
}

/// A mark read off an object, or off its editorial where the object could
/// not take it.
fn marked<'py>(object: &Bound<'py, PyAny>, name: &str) -> Option<Bound<'py, PyAny>> {
    if let Ok(found) = object.getattr(name) {
        return Some(found);
    }
    object
        .getattr("editorial")
        .and_then(|editorial| editorial.getattr(name))
        .ok()
}

/// The marks of transcribing taken off an object again: music21's
/// `_cleanupAttributes`.
fn clean_up(object: &Bound<'_, PyAny>) {
    for name in TEMPORARY_ATTRIBUTES {
        if object.hasattr(name).unwrap_or(false) {
            let _ = object.delattr(name);
        }
        if let Ok(editorial) = object.getattr("editorial")
            && editorial.hasattr(name).unwrap_or(false)
        {
            let _ = editorial.delattr(name);
        }
    }
}

/// Whether an object is of a class music21 names.
fn has_class(object: &Bound<'_, PyAny>, name: &str) -> bool {
    object
        .getattr("classes")
        .and_then(|classes| classes.contains(name))
        .unwrap_or(false)
}

/// Whether music21 counts an element a `NotRest`.
fn is_not_rest(element: &StreamElement) -> bool {
    matches!(
        element,
        StreamElement::Note(_)
            | StreamElement::Chord(_)
            | StreamElement::ChordSymbol(_)
            | StreamElement::Unpitched(_)
            | StreamElement::PercussionChord(_)
    )
}

/// The class standing in music21's module where anything is installed
/// there, and this package's own otherwise.
fn class_of<'py>(py: Python<'py>, module: &str, name: &str) -> PyResult<Bound<'py, PyAny>> {
    if let Some(installed) = crate::installed_class(py, module, name) {
        return Ok(installed);
    }
    py.import("music21_rs")
        .or_else(|_| py.import("music21_rs_facade"))?
        .getattr(name)
}

/// A new, empty grouping of this module's.
fn new_grouping(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    class_of(py, "music21.braille.segment", "BrailleElementGrouping")?.call0()
}

/// A grouping holding these objects, with the attributes of another, as
/// music21 makes one: `copy.copy` of the other, its list replaced.
fn grouping_like<'py>(
    py: Python<'py>,
    like: Option<&Bound<'py, PyAny>>,
    objects: &[Py<PyAny>],
) -> PyResult<Bound<'py, PyAny>> {
    let made = match like {
        Some(like) => py.import("copy")?.getattr("copy")?.call1((like,))?,
        None => new_grouping(py)?,
    };
    let listed = PyList::empty(py);
    for object in objects {
        listed.append(object.bind(py))?;
    }
    made.setattr("internalList", listed)?;
    Ok(made)
}

// ---------------------------------------------------------------------------
// A stream's objects at each place the crate counts.

/// A part (or a measure) read into the crate, with the object behind each
/// element braille can be handed back: its leaves, its measures' own
/// barlines and its voices, each by the leaf it starts at.
struct Located {
    stream: RsStream,
    leaves: Vec<Py<PyAny>>,
    left_barlines: BTreeMap<usize, Py<PyAny>>,
    right_barlines: BTreeMap<usize, Py<PyAny>>,
    voices: BTreeMap<usize, Py<PyAny>>,
    /// Each measure's direct elements that music21 counts notes, with
    /// their leaf: what `prepareBeamedNotes` marks.
    notes: Vec<usize>,
}

/// A stream's inner streams in the order the crate reads them: its
/// elements by offset, those at one offset as listed.
fn inner_streams<'py>(stream: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let mut placed: Vec<(f64, Bound<'py, PyAny>)> = Vec::new();
    for element in stream.getattr("elements")?.walk()? {
        let element = element?;
        let is_stream = element
            .getattr("isStream")
            .and_then(|flag| flag.is_truthy())
            .unwrap_or(false);
        if !is_stream {
            continue;
        }
        let offset: f64 = stream
            .call_method1("elementOffset", (&element,))?
            .extract()?;
        placed.push((offset, element));
    }
    placed.sort_by(|left, right| left.0.total_cmp(&right.0));
    Ok(placed.into_iter().map(|(_, element)| element).collect())
}

impl Located {
    /// A part or a measure read, with everything braille may hand back.
    fn of(stream: &Bound<'_, PyAny>) -> PyResult<Self> {
        let (read, leaves) = crate::stream::crate_stream_with_leaves(stream)?;
        let mut located = Self {
            stream: read,
            leaves,
            left_barlines: BTreeMap::new(),
            right_barlines: BTreeMap::new(),
            voices: BTreeMap::new(),
            notes: Vec::new(),
        };
        let stream_value = located.stream.clone();
        if stream_value.kind() == StreamKind::Measure {
            located.measure(stream, &stream_value, 0)?;
            return Ok(located);
        }
        let mut leaf = 0;
        let mut inner = inner_streams(stream)?.into_iter();
        for event in stream_value.events() {
            let StreamElement::Stream(held) = event.element() else {
                leaf += 1;
                continue;
            };
            if let Some(object) = inner.next()
                && held.kind() == StreamKind::Measure
            {
                located.measure(&object, held, leaf)?;
            }
            leaf += held.leaves().len();
        }
        Ok(located)
    }

    /// A measure's barlines, voices and notes, its leaves starting at
    /// `base`.
    fn measure(
        &mut self,
        object: &Bound<'_, PyAny>,
        measure: &RsStream,
        base: usize,
    ) -> PyResult<()> {
        if measure.left_barline().is_some()
            && let Ok(barline) = object.getattr("leftBarline")
            && !barline.is_none()
        {
            self.left_barlines.insert(base, barline.unbind());
        }
        if measure.right_barline().is_some()
            && let Ok(barline) = object.getattr("rightBarline")
            && !barline.is_none()
        {
            self.right_barlines
                .insert(base + measure.leaves().len(), barline.unbind());
        }
        let mut leaf = base;
        let mut inner = inner_streams(object)?.into_iter();
        for event in measure.events() {
            match event.element() {
                StreamElement::Stream(held) => {
                    if let Some(voice) = inner.next() {
                        self.voices.entry(leaf).or_insert_with(|| voice.unbind());
                    }
                    leaf += held.leaves().len();
                }
                element => {
                    if is_not_rest(element) {
                        self.notes.push(leaf);
                    }
                    leaf += 1;
                }
            }
        }
        Ok(())
    }

    /// The object an element of braille's was read from.
    fn object(&self, py: Python<'_>, element: &BrailleElement) -> Option<Py<PyAny>> {
        let leaf = element.leaf;
        let found = match &element.element {
            StreamElement::Stream(_) => self.voices.get(&leaf),
            StreamElement::Barline(_) if element.offset == 0.0 => self
                .left_barlines
                .get(&leaf)
                .or_else(|| self.leaves.get(leaf)),
            StreamElement::Barline(_) => self
                .right_barlines
                .get(&leaf)
                .or_else(|| self.leaves.get(leaf)),
            _ => self.leaves.get(leaf),
        };
        found.map(|object| object.clone_ref(py))
    }

    /// What music21 has marked on each leaf, as the crate reads it.
    fn contexts(&self, py: Python<'_>) -> Vec<NoteContext> {
        self.leaves
            .iter()
            .map(|object| note_context(object.bind(py)))
            .collect()
    }
}

/// The options music21's segmenting functions take.
#[allow(clippy::too_many_arguments)]
fn segment_options(
    setHand: Option<&str>,
    descendingChords: Option<bool>,
    maxLineLength: usize,
    showLongSlursAndTiesTogether: Option<bool>,
    showShortSlursAndTiesTogether: bool,
    slurLongPhraseWithBrackets: bool,
) -> PyResult<(Option<Hand>, SegmentOptions)> {
    let hand = match setHand {
        None => None,
        Some("right") => Some(Hand::Right),
        Some("left") => Some(Hand::Left),
        Some(other) => {
            return Err(BrailleSegmentException::new_err(format!(
                "{other} is not a hand braille knows"
            )));
        }
    };
    let mut options = SegmentOptions::default();
    options.descending_chords = descendingChords;
    options.max_line_length = maxLineLength;
    options.show_long_slurs_and_ties_together = showLongSlursAndTiesTogether;
    options.show_short_slurs_and_ties_together = showShortSlursAndTiesTogether;
    options.slur_long_phrase_with_brackets = slurLongPhraseWithBrackets;
    Ok((hand, options))
}

/// The slurs of a part marked on its notes, as music21 marks them: only
/// what is so.
fn mark_slurs(py: Python<'_>, located: &Located, options: &SegmentOptions) {
    let contexts = prepare_slurred_notes(&located.stream, options);
    for (object, context) in located.leaves.iter().zip(contexts) {
        let object = object.bind(py);
        for (name, said) in [
            ("shortSlur", context.short_slur),
            ("beginLongBracketSlur", context.begin_long_bracket_slur),
            ("endLongBracketSlur", context.end_long_bracket_slur),
            ("beginLongDoubleSlur", context.begin_long_double_slur),
            ("endLongDoubleSlur", context.end_long_double_slur),
        ] {
            if said {
                mark(object, name, true);
            }
        }
    }
}

/// What music21 writes on each element it extracts: its affinity and sort
/// order, and its English, which until it is transcribed is its `str`.
fn mark_extracted(object: &Bound<'_, PyAny>, element: &BrailleElement) -> PyResult<()> {
    let py = object.py();
    if !matches!(element.element, StreamElement::BrailleSegmentDivision(_)) {
        if let Ok(editorial) = object.getattr("editorial") {
            editorial.setattr("affinityCode", affinity_object(py, element.affinity)?)?;
        }
        // A class of this package's keeps its sort order as its class's,
        // and takes no other.
        let _ = object.setattr("classSortOrder", element.sort_order);
    }
    if let Ok(editorial) = object.getattr("editorial") {
        let english = PyList::empty(py);
        english.append(object.str()?)?;
        editorial.setattr("brailleEnglish", english)?;
    }
    Ok(())
}

/// A grouping of the crate's as music21's, holding the objects it was read
/// from and marking each as music21's extraction does.
fn extracted_grouping<'py>(
    py: Python<'py>,
    located: &Located,
    elements: &[BrailleElement],
) -> PyResult<Bound<'py, PyAny>> {
    let grouping = new_grouping(py)?;
    let listed = grouping.getattr("internalList")?;
    for element in elements {
        let Some(object) = located.object(py, element) else {
            continue;
        };
        mark_extracted(object.bind(py), element)?;
        listed.call_method1("append", (object,))?;
    }
    Ok(grouping)
}

/// The raw segments of a part, built of the part's own objects, its beamed
/// notes marked: music21's `getRawSegments` over a part already read.
fn raw_segments(
    py: Python<'_>,
    located: &Located,
    hand: Option<Hand>,
    options: SegmentOptions,
) -> PyResult<Vec<Py<PyAny>>> {
    let contexts = located.contexts(py);
    let segments = get_raw_segments(&located.stream, hand, &contexts, options);
    // The beams of every measure's notes, which music21 marks as it goes.
    if let Some(last) = segments.last() {
        for &leaf in &located.notes {
            let (Some(object), Some(context)) = (located.leaves.get(leaf), last.contexts.get(leaf))
            else {
                continue;
            };
            let object = object.bind(py);
            mark(object, "beamStart", context.beam_start);
            mark(object, "beamContinue", context.beam_continue);
        }
    }
    let class = class_of(py, "music21.braille.segment", "BrailleSegment")?;
    let mut out = Vec::new();
    for segment in segments {
        let made = class.call1((options.max_line_length,))?;
        made.setattr("endHyphen", segment.end_hyphen)?;
        made.setattr("beginsMidMeasure", segment.begins_mid_measure)?;
        for (key, grouping) in &segment.groupings {
            let grouping = extracted_grouping(py, located, &grouping.elements)?;
            made.set_item(py_key(py, *key)?, grouping)?;
        }
        out.push(made.unbind());
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// A segment of the facade's read into the crate's and written back.

/// A segment's groupings read into the crate, with the key and objects
/// each was read from: the objects of a grouping by place, and the object
/// at each leaf, which a voice's notes and a split grouping are found by.
struct ReadGroupings {
    groupings: BTreeMap<SegmentKey, RsGrouping>,
    keys: BTreeMap<SegmentKey, Py<PyAny>>,
    objects: BTreeMap<SegmentKey, Vec<Py<PyAny>>>,
    leaves: Vec<Option<Py<PyAny>>>,
    contexts: Vec<NoteContext>,
}

/// One object of a grouping as the crate's element: a voice as the stream
/// it is, with the objects at its leaves, anything else as
/// [`crate_element`] reads it.
fn segment_element(
    object: &Bound<'_, PyAny>,
) -> PyResult<(BrailleElement, Option<Vec<Py<PyAny>>>)> {
    if has_class(object, "Voice") {
        let (voice, leaves) = crate::stream::read_with_leaves(object)?;
        let element = BrailleElement {
            element: StreamElement::Stream(Box::new(voice)),
            offset: object
                .getattr("offset")
                .and_then(|offset| offset.extract())
                .unwrap_or(0.0),
            affinity: Affinity::Inaccord,
            sort_order: 10,
            english: None,
            context: NoteContext::default(),
            outgoing_key: None,
            leaf: 0,
            voice_english: Vec::new(),
        };
        return Ok((element, Some(leaves)));
    }
    Ok((crate_element(object)?, None))
}

impl ReadGroupings {
    fn of(groupings: &Bound<'_, PyDict>) -> PyResult<Self> {
        let py = groupings.py();
        let mut read = Self {
            groupings: BTreeMap::new(),
            keys: BTreeMap::new(),
            objects: BTreeMap::new(),
            leaves: Vec::new(),
            contexts: Vec::new(),
        };
        for (key, grouping) in groupings.iter() {
            let rs = rs_key(&key)?;
            let mut elements = Vec::new();
            let mut objects = Vec::new();
            for object in grouping.getattr("internalList")?.walk()? {
                let object = object?;
                let (mut element, inner) = segment_element(&object)?;
                element.leaf = read.leaves.len();
                match inner {
                    Some(inner) => {
                        for leaf in inner {
                            read.contexts.push(note_context(leaf.bind(py)));
                            read.leaves.push(Some(leaf));
                        }
                    }
                    None => {
                        read.contexts.push(element.context);
                        read.leaves.push(Some(object.clone().unbind()));
                    }
                }
                elements.push(element);
                objects.push(object.unbind());
            }
            let mut value = grouping_attributes(&grouping)?;
            value.elements = elements;
            read.groupings.insert(rs, value);
            read.keys.insert(rs, key.unbind());
            read.objects.insert(rs, objects);
        }
        Ok(read)
    }

    /// The python key for one of the crate's: the one it was read from,
    /// or a new one.
    fn key(&self, py: Python<'_>, key: SegmentKey) -> PyResult<Py<PyAny>> {
        match self.keys.get(&key) {
            Some(found) => Ok(found.clone_ref(py)),
            None => py_key(py, key),
        }
    }

    /// The objects at the leaves of a grouping's elements.
    fn objects_at(&self, py: Python<'_>, elements: &[BrailleElement]) -> Vec<Py<PyAny>> {
        elements
            .iter()
            .filter_map(|element| self.leaves.get(element.leaf).and_then(Option::as_ref))
            .map(|object| object.clone_ref(py))
            .collect()
    }

    /// What transcribing said, written back: each sign's English onto its
    /// object, a voice's notes' too, and each grouping the crate made
    /// (the halves of a split) into the segment as a grouping of the
    /// objects it holds.
    fn write_back(
        &self,
        groupings: &Bound<'_, PyDict>,
        written: &BTreeMap<SegmentKey, RsGrouping>,
    ) -> PyResult<()> {
        let py = groupings.py();
        for (key, grouping) in written {
            let split = matches!(
                key.affinity,
                Affinity::SplitNoteGroupingA | Affinity::SplitNoteGroupingB
            );
            let objects = match self.objects.get(key) {
                Some(objects) if !split => objects.iter().map(|o| o.clone_ref(py)).collect(),
                _ => {
                    let objects = self.objects_at(py, &grouping.elements);
                    let whole = SegmentKey {
                        affinity: Affinity::NoteGrouping,
                        ..*key
                    };
                    let like = match self.keys.get(&whole) {
                        Some(found) => groupings.get_item(found.bind(py))?,
                        None => None,
                    };
                    let made = grouping_like(py, like.as_ref(), &objects)?;
                    made.setattr("withHyphen", grouping.with_hyphen)?;
                    groupings.set_item(self.key(py, *key)?, made)?;
                    objects
                }
            };
            write_english(py, &objects, &grouping.elements)?;
            for element in &grouping.elements {
                for (index, english) in element.voice_english.iter().enumerate() {
                    let Some(english) = english else {
                        continue;
                    };
                    if let Some(Some(object)) = self.leaves.get(element.leaf + index) {
                        let listed = PyList::new(py, english)?;
                        if let Ok(editorial) = object.bind(py).getattr("editorial") {
                            editorial.setattr("brailleEnglish", listed)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// The marks of transcribing taken off the objects of a grouping, and
    /// off a voice's notes where `voices` asks.
    fn clean_up(&self, py: Python<'_>, key: SegmentKey, voices: bool) {
        let Some(grouping) = self.groupings.get(&key) else {
            return;
        };
        for element in &grouping.elements {
            let count = match &element.element {
                StreamElement::Stream(voice) if voices => voice.leaves().len(),
                StreamElement::Stream(_) => 0,
                _ => 1,
            };
            for leaf in element.leaf..element.leaf + count {
                if let Some(Some(object)) = self.leaves.get(leaf) {
                    clean_up(object.bind(py));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The segments.

/// music21's error for a step of the crate's that failed.
fn segment_error(error: &music21::Error) -> PyErr {
    BrailleSegmentException::new_err(crate::pitch::message(error))
}

/// The pitch of a note music21 kept as the last one written.
fn pitch_of(object: Option<&Bound<'_, PyAny>>) -> Option<Pitch> {
    let object = object.filter(|object| !object.is_none())?;
    match crate::stream::element_read(object).ok()?? {
        StreamElement::Note(note) => Some(note.pitch().clone()),
        _ => None,
    }
}

/// A key music21 keeps, as the crate's, nothing for `None`.
fn optional_key(key: Option<&Py<PyAny>>, py: Python<'_>) -> PyResult<Option<SegmentKey>> {
    match key.map(|key| key.bind(py)).filter(|key| !key.is_none()) {
        Some(key) => Ok(Some(rs_key(key)?)),
        None => Ok(None),
    }
}

/// The last note among some groupings and their objects whose pitch is
/// this one: the object music21 kept as its `lastNote`.
fn last_note_object<'a>(
    pitch: &Pitch,
    groupings: impl Iterator<Item = (&'a RsGrouping, &'a [Py<PyAny>])>,
) -> Option<&'a Py<PyAny>> {
    let mut found = None;
    for (grouping, objects) in groupings {
        for (element, object) in grouping.elements.iter().zip(objects) {
            if let StreamElement::Note(note) = &element.element
                && note.pitch() == pitch
            {
                found = Some(object);
            }
        }
    }
    found
}

/// music21's `braille.segment.BrailleSegment`: a part's measures cut into
/// groupings by key, written out as braille.
///
/// It is a dictionary of groupings, as music21's is, holding the score's
/// own objects; each step of writing reads them into the crate's segment,
/// asks the crate's `SegmentTranscriber`, and writes what it said back:
/// the lines onto the text, each sign's English onto its object, and the
/// halves of a split grouping into the dictionary.
#[pyclass(
    name = "BrailleSegment",
    module = "music21.braille.segment",
    extends = BrailleText,
    subclass,
    skip_from_py_object
)]
pub struct BrailleSegment {
    /// The groupings by key: music21's `_groupingDict`.
    #[pyo3(get, name = "_groupingDict")]
    groupings: Py<PyDict>,
    /// The keys still to write, a deque.
    #[pyo3(get, set)]
    groupingKeysToProcess: Py<PyAny>,
    /// The key being written.
    #[pyo3(get, set)]
    currentGroupingKey: Option<Py<PyAny>>,
    /// The key written before it.
    #[pyo3(get, set)]
    previousGroupingKey: Option<Py<PyAny>>,
    /// The last note written, which decides the next note's octave.
    #[pyo3(get, set)]
    lastNote: Option<Py<PyAny>>,
    /// Whether clefs are written.
    #[pyo3(get, set)]
    showClefSigns: bool,
    /// Whether a fingering's upper choice is written first.
    #[pyo3(get, set)]
    upperFirstInNoteFingering: bool,
    /// Whether chords are written from the top down.
    #[pyo3(get, set)]
    descendingChords: bool,
    /// Whether a key signature cancels the one before it.
    #[pyo3(get, set)]
    cancelOutgoingKeySig: bool,
    /// How many dummy rests start the music, for testing line breaks.
    #[pyo3(get, set)]
    dummyRestLength: Option<usize>,
    /// Whether the first measure's number is written.
    #[pyo3(get, set)]
    showFirstMeasureNumber: bool,
    /// Whether a heading is written.
    #[pyo3(get, set)]
    showHeading: bool,
    /// Whether every octave mark is left out.
    #[pyo3(get, set)]
    suppressOctaveMarks: bool,
    /// Whether the last grouping ends in a music hyphen.
    #[pyo3(get, set)]
    endHyphen: bool,
    /// Whether the segment starts in the middle of a measure.
    #[pyo3(get, set)]
    beginsMidMeasure: bool,
}

impl BrailleSegment {
    /// A segment with nothing in it, as music21's starts.
    fn fresh(py: Python<'_>) -> PyResult<Self> {
        Ok(Self {
            groupings: PyDict::new(py).unbind(),
            groupingKeysToProcess: py
                .import("collections")?
                .getattr("deque")?
                .call0()?
                .unbind(),
            currentGroupingKey: None,
            previousGroupingKey: None,
            lastNote: None,
            showClefSigns: false,
            upperFirstInNoteFingering: true,
            descendingChords: true,
            cancelOutgoingKeySig: true,
            dummyRestLength: None,
            showFirstMeasureNumber: true,
            showHeading: true,
            suppressOctaveMarks: false,
            endHyphen: false,
            beginsMidMeasure: false,
        })
    }

    /// The groupings, read: with `instead` standing for the grouping under
    /// one key where a caller hands over its own.
    fn read_groupings(
        slf: &Bound<'_, Self>,
        instead: Option<(&Bound<'_, PyAny>, &Bound<'_, PyAny>)>,
    ) -> PyResult<ReadGroupings> {
        let py = slf.py();
        let groupings = slf.borrow().groupings.bind(py).copy()?;
        if let Some((key, grouping)) = instead {
            groupings.set_item(key, grouping)?;
        }
        ReadGroupings::of(&groupings)
    }

    /// The segment as the crate's, out of groupings already read.
    fn crate_segment(slf: &Bound<'_, Self>, read: &ReadGroupings) -> PyResult<RsSegment> {
        let line_length: usize = slf.getattr("lineLength")?.extract()?;
        let hand: Option<String> = slf.getattr("showHand")?.extract()?;
        let me = slf.borrow();
        let mut options = SegmentOptions::default();
        options.cancel_outgoing_key_sig = me.cancelOutgoingKeySig;
        options.descending_chords = Some(me.descendingChords);
        options.dummy_rest_length = me.dummyRestLength;
        options.max_line_length = line_length;
        options.show_clef_signs = me.showClefSigns;
        options.show_first_measure_number = me.showFirstMeasureNumber;
        options.show_hand = match hand.as_deref() {
            Some("right") => Some(Hand::Right),
            Some("left") => Some(Hand::Left),
            _ => None,
        };
        options.show_heading = me.showHeading;
        options.suppress_octave_marks = me.suppressOctaveMarks;
        options.upper_first_in_note_fingering = me.upperFirstInNoteFingering;
        Ok(RsSegment {
            groupings: read.groupings.clone(),
            options,
            descending_chords: me.descendingChords,
            end_hyphen: me.endHyphen,
            begins_mid_measure: me.beginsMidMeasure,
            contexts: read.contexts.clone(),
        })
    }

    /// One step of writing: the segment and the text read into the
    /// crate's, with the keys and the last note music21 keeps, the step
    /// run, and everything it changed written back. `also` is a grouping
    /// of the caller's the step reads, whose last note may become the
    /// segment's.
    fn step<R>(
        slf: &Bound<'_, Self>,
        groupings: Option<ReadGroupings>,
        also: Option<(&RsGrouping, &[Py<PyAny>])>,
        run: impl FnOnce(&mut SegmentTranscriber, &mut RsSegment) -> music21::error::Result<R>,
    ) -> PyResult<(R, RsSegment, ReadGroupings)> {
        let py = slf.py();
        let read = match groupings {
            Some(read) => read,
            None => ReadGroupings::of(&PyDict::new(py))?,
        };
        let mut segment = Self::crate_segment(slf, &read)?;
        let text = crate::brailletext::text_value(slf.as_super())?;
        let mut transcriber = SegmentTranscriber::new(text);
        let (keys, current, previous, last) = {
            let me = slf.borrow();
            (
                me.groupingKeysToProcess.clone_ref(py),
                me.currentGroupingKey.as_ref().map(|key| key.clone_ref(py)),
                me.previousGroupingKey.as_ref().map(|key| key.clone_ref(py)),
                me.lastNote.as_ref().map(|note| note.clone_ref(py)),
            )
        };
        for key in keys.bind(py).walk()? {
            transcriber.keys_mut().push_back(rs_key(&key?)?);
        }
        transcriber.set_current(optional_key(current.as_ref(), py)?);
        transcriber.set_previous(optional_key(previous.as_ref(), py)?);
        let last_pitch = pitch_of(last.as_ref().map(|note| note.bind(py)));
        transcriber.set_last_note(last_pitch.clone());
        let answer = run(&mut transcriber, &mut segment).map_err(|error| segment_error(&error))?;
        crate::brailletext::set_text_value(slf.as_super(), transcriber.text().clone())?;
        let deque = py.import("collections")?.getattr("deque")?.call0()?;
        for key in transcriber.keys_mut().iter() {
            deque.call_method1("append", (read.key(py, *key)?,))?;
        }
        let to_object = |key: Option<SegmentKey>| -> PyResult<Option<Py<PyAny>>> {
            key.map(|key| read.key(py, key)).transpose()
        };
        let current = to_object(transcriber.current())?;
        let previous = to_object(transcriber.previous())?;
        let last = match transcriber.last_note() {
            None => None,
            Some(pitch) if Some(pitch) == last_pitch.as_ref() => last,
            Some(pitch) => {
                let held = read.groupings.iter().filter_map(|(key, grouping)| {
                    read.objects
                        .get(key)
                        .map(|objects| (grouping, objects.as_slice()))
                });
                last_note_object(pitch, held.chain(also))
                    .map(|object| object.clone_ref(py))
                    .or(last)
            }
        };
        {
            let mut me = slf.borrow_mut();
            me.groupingKeysToProcess = deque.unbind();
            me.currentGroupingKey = current;
            me.previousGroupingKey = previous;
            me.lastNote = last;
        }
        let groupings = slf.borrow().groupings.clone_ref(py);
        read.write_back(groupings.bind(py), &segment.groupings)?;
        Ok((answer, segment, read))
    }

    /// A step that writes the grouping being written, which music21
    /// cleans the marks of transcribing off afterwards where `clean` asks.
    fn grouping_step(
        slf: &Bound<'_, Self>,
        clean: bool,
        run: impl FnOnce(&mut SegmentTranscriber, &mut RsSegment) -> music21::error::Result<()>,
    ) -> PyResult<()> {
        let py = slf.py();
        let read = Self::read_groupings(slf, None)?;
        let current = optional_key(slf.borrow().currentGroupingKey.as_ref(), py)?;
        let (_, _, read) = Self::step(slf, Some(read), None, run)?;
        if clean && let Some(current) = current {
            read.clean_up(py, current, false);
        }
        Ok(())
    }

    /// music21's own `repr` for a text, under the class's own name.
    fn text_repr(slf: &Bound<'_, PyAny>) -> PyResult<String> {
        let py = slf.py();
        let written: String = py
            .get_type::<BrailleText>()
            .getattr("__repr__")?
            .call1((slf,))?
            .extract()?;
        let class = slf.get_type();
        let name = format!(
            "<{}.{} ",
            class.getattr("__module__")?.str()?,
            class.getattr("__qualname__")?.str()?
        );
        Ok(written.replacen("<music21.braille.text.BrailleText ", &name, 1))
    }
}

/// The name music21 gives a grouping of an affinity, from its number.
fn affinity_name(key: &Bound<'_, PyAny>) -> PyResult<&'static str> {
    let code: i64 = key.getattr("affinity")?.extract()?;
    affinity_from(code)
        .map(Affinity::name)
        .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err(code))
}

#[pymethods]
impl BrailleSegment {
    /// music21's `BrailleSegment(lineLength=40)`.
    #[new]
    #[pyo3(signature = (lineLength = 40))]
    fn new(py: Python<'_>, lineLength: usize) -> PyResult<PyClassInitializer<Self>> {
        let text = RsText::new(lineLength, None).map_err(|error| segment_error(&error))?;
        Ok(crate::brailletext::text_initializer(py, text)?.add_subclass(Self::fresh(py)?))
    }

    fn __traverse__(&self, visit: pyo3::PyVisit<'_>) -> Result<(), pyo3::PyTraverseError> {
        visit.call(&self.groupings)?;
        visit.call(&self.groupingKeysToProcess)?;
        visit.call(&self.currentGroupingKey)?;
        visit.call(&self.previousGroupingKey)?;
        visit.call(&self.lastNote)?;
        Ok(())
    }

    fn __clear__(&mut self) {
        Python::attach(|py| {
            self.groupings.bind(py).clear();
            let _ = self.groupingKeysToProcess.bind(py).call_method0("clear");
        });
        self.currentGroupingKey = None;
        self.previousGroupingKey = None;
        self.lastNote = None;
    }

    fn __setitem__(
        &self,
        py: Python<'_>,
        item: &Bound<'_, PyAny>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.groupings.bind(py).set_item(item, value)
    }

    /// The grouping under a key, an empty one put there where there is
    /// none, as music21's segment is a `defaultdict`.
    fn __getitem__<'py>(
        &self,
        py: Python<'py>,
        item: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let groupings = self.groupings.bind(py);
        if let Some(found) = groupings.get_item(item)? {
            return Ok(found);
        }
        let made = new_grouping(py)?;
        groupings.set_item(item, &made)?;
        Ok(made)
    }

    fn __delitem__(&self, py: Python<'_>, item: &Bound<'_, PyAny>) -> PyResult<()> {
        self.groupings.bind(py).del_item(item)
    }

    /// Anything else is the dictionary's, as music21's own `__getattr__`
    /// hands it on: `segment.keys()` is the dictionary's `keys`.
    fn __getattr__<'py>(&self, py: Python<'py>, item: &str) -> PyResult<Bound<'py, PyAny>> {
        if item.starts_with("__") {
            return Err(pyo3::exceptions::PyAttributeError::new_err(
                item.to_string(),
            ));
        }
        self.groupings.bind(py).as_any().getattr(item)
    }

    fn __contains__(&self, py: Python<'_>, item: &Bound<'_, PyAny>) -> PyResult<bool> {
        self.groupings.bind(py).contains(item)
    }

    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, pyo3::types::PyIterator>> {
        self.groupings.bind(py).as_any().try_iter()
    }

    fn __len__(&self, py: Python<'_>) -> usize {
        self.groupings.bind(py).len()
    }

    /// The braille written so far: music21's `brailleText`, the text's
    /// own `str`.
    #[getter]
    fn brailleText(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        Ok(py
            .get_type::<BrailleText>()
            .getattr("__str__")?
            .call1((slf,))?
            .unbind())
    }

    /// Each grouping in English under the measure and kind it is, as
    /// music21 lists a segment.
    fn __str__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let py = slf.py();
        let items = py.import("builtins")?.getattr("sorted")?.call1((slf
            .borrow()
            .groupings
            .bind(py)
            .items(),))?;
        let mut entries = Vec::new();
        let mut previous_split = false;
        for item in items.walk()? {
            let item = item?;
            let key = item.get_item(0)?;
            let grouping = item.get_item(1)?;
            let code: Option<i64> = key.getattr("affinity")?.extract().ok();
            if previous_split {
                previous_split = code == Some(Affinity::SplitNoteGroupingA as i64);
                continue;
            }
            entries.push(format!(
                "Measure {}, {} {}:\n{}\n===",
                key.getattr("measure")?.str()?,
                affinity_name(&key)?,
                key.getattr("ordinal")?.extract::<i64>()? + 1,
                grouping.str()?
            ));
            previous_split = code == Some(Affinity::SplitNoteGroupingA as i64);
        }
        Ok([
            "---begin segment---".to_string(),
            "<music21.braille.segment BrailleSegment>".to_string(),
            entries.join("\n"),
            "---end segment---".to_string(),
        ]
        .join("\n"))
    }

    /// music21's own `repr`, which is the text's.
    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        Self::text_repr(slf.as_any())
    }

    /// The whole segment in braille: heading, first measure number, dummy
    /// rests and each grouping in turn: music21's `transcribe`. Each note
    /// grouping's marks of transcribing are taken off it afterwards, as
    /// music21 takes them.
    fn transcribe(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let read = Self::read_groupings(slf, None)?;
        let (_, _, read) = Self::step(slf, Some(read), None, |transcriber, segment| {
            transcriber.transcribe(segment)
        })?;
        for key in read.groupings.keys() {
            if key.affinity == Affinity::NoteGrouping {
                read.clean_up(py, *key, false);
            }
        }
        Self::brailleText(slf)
    }

    /// As many dummy rests as `dummyRestLength` says: music21's
    /// `addDummyRests`.
    fn addDummyRests(slf: &Bound<'_, Self>) -> PyResult<()> {
        Self::step(slf, None, None, |transcriber, segment| {
            transcriber.add_dummy_rests(segment)
        })?;
        Ok(())
    }

    /// The number of the first measure still to write: music21's
    /// `extractMeasureNumber`.
    fn extractMeasureNumber(slf: &Bound<'_, Self>) -> PyResult<()> {
        let read = Self::read_groupings(slf, None)?;
        Self::step(slf, Some(read), None, |transcriber, segment| {
            transcriber.extract_measure_number(segment)
        })?;
        Ok(())
    }

    /// The heading out of the signatures, tempo and metronome mark the
    /// segment starts with: music21's `extractHeading`.
    fn extractHeading(slf: &Bound<'_, Self>) -> PyResult<()> {
        Self::grouping_step(slf, false, |transcriber, segment| {
            transcriber.extract_heading(segment)
        })
    }

    /// The voices of the grouping being written, in accord: music21's
    /// `extractInaccordGrouping`.
    fn extractInaccordGrouping(slf: &Bound<'_, Self>) -> PyResult<()> {
        Self::grouping_step(slf, false, |transcriber, segment| {
            transcriber.extract_inaccord_grouping(segment)
        })
    }

    /// The long text of the grouping being written: music21's
    /// `extractLongExpressionGrouping`.
    fn extractLongExpressionGrouping(slf: &Bound<'_, Self>) -> PyResult<()> {
        Self::grouping_step(slf, false, |transcriber, segment| {
            transcriber.extract_long_expression_grouping(segment)
        })
    }

    /// The signatures of the grouping being written: music21's
    /// `extractSignatureGrouping`.
    fn extractSignatureGrouping(slf: &Bound<'_, Self>) -> PyResult<()> {
        Self::grouping_step(slf, false, |transcriber, segment| {
            transcriber.extract_signature_grouping(segment)
        })
    }

    /// The tempo words of the grouping being written, which start a heading
    /// of their own: music21's `extractTempoTextGrouping`.
    fn extractTempoTextGrouping(slf: &Bound<'_, Self>) -> PyResult<()> {
        Self::grouping_step(slf, false, |transcriber, segment| {
            transcriber.extract_tempo_text_grouping(segment)
        })
    }

    /// The notes of the grouping being written, split across lines where
    /// they do not fit: music21's `extractNoteGrouping`.
    fn extractNoteGrouping(slf: &Bound<'_, Self>) -> PyResult<()> {
        Self::grouping_step(slf, true, |transcriber, segment| {
            transcriber.extract_note_grouping(segment)
        })
    }

    /// Whether a grouping shows its first note's octave, which also makes
    /// its last note the segment's `lastNote`: music21's
    /// `showLeadingOctaveFromNoteGrouping`.
    fn showLeadingOctaveFromNoteGrouping(
        slf: &Bound<'_, Self>,
        noteGrouping: &Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        let (grouping, objects) = crate_grouping(noteGrouping)?;
        let suppress = slf.borrow().suppressOctaveMarks;
        let (shown, _, _) = Self::step(
            slf,
            None,
            Some((&grouping, objects.as_slice())),
            |transcriber, _| Ok(transcriber.show_leading_octave_from(&grouping, suppress)),
        )?;
        Ok(shown)
    }

    /// Whether braille this long has to be split to fit what is left of
    /// the line: music21's `needsSplitToFit`, which takes the length of
    /// whatever it is given.
    fn needsSplitToFit(
        slf: &Bound<'_, Self>,
        brailleNoteGrouping: &Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        let cells = brailleNoteGrouping.len()?;
        let (needs, _, _) = Self::step(slf, None, None, |transcriber, _| {
            Ok(transcriber.needs_split_for(cells))
        })?;
        Ok(needs)
    }

    /// A grouping split where the line has room, each half written and
    /// kept under the key being written: music21's
    /// `splitNoteGroupingAndTranscribe`.
    #[pyo3(signature = (noteGrouping, showLeadingOctaveOnFirst = false, addSpaceToFirst = false))]
    fn splitNoteGroupingAndTranscribe(
        slf: &Bound<'_, Self>,
        noteGrouping: &Bound<'_, PyAny>,
        #[pyo3(from_py_with = crate::truthy)] showLeadingOctaveOnFirst: bool,
        #[pyo3(from_py_with = crate::truthy)] addSpaceToFirst: bool,
    ) -> PyResult<(String, String)> {
        let py = slf.py();
        let current = slf
            .borrow()
            .currentGroupingKey
            .as_ref()
            .map(|key| key.clone_ref(py))
            .ok_or_else(|| {
                BrailleSegmentException::new_err("no grouping of the segment is being written")
            })?;
        let read = Self::read_groupings(slf, Some((current.bind(py), noteGrouping)))?;
        let (split, _, _) = Self::step(slf, Some(read), None, |transcriber, segment| {
            transcriber.split_and_transcribe(segment, showLeadingOctaveOnFirst, addSpaceToFirst)
        })?;
        split.ok_or_else(|| BrailleSegmentException::new_err("no split of the grouping fits"))
    }

    /// The signs for a grouping repeated so many times: music21's
    /// `addRepeatSymbols`.
    fn addRepeatSymbols(slf: &Bound<'_, Self>, repeatTimes: usize) -> PyResult<()> {
        Self::step(slf, None, None, |transcriber, _| {
            transcriber.add_repeat_symbols(repeatTimes)
        })?;
        Ok(())
    }

    /// A new segment with each run of note groupings put together under
    /// the first of their keys: music21's `consolidate`.
    fn consolidate(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let line_length = slf.getattr("lineLength")?;
        let made =
            class_of(py, "music21.braille.segment", "BrailleSegment")?.call1((line_length,))?;
        let items = py.import("builtins")?.getattr("sorted")?.call1((slf
            .borrow()
            .groupings
            .bind(py)
            .items(),))?;
        let mut first: Option<Bound<'_, PyAny>> = None;
        for item in items.walk()? {
            let item = item?;
            let key = item.get_item(0)?;
            let grouping = item.get_item(1)?;
            let code: i64 = key.getattr("affinity")?.extract()?;
            if code != Affinity::NoteGrouping as i64 {
                made.set_item(&key, grouping)?;
                first = None;
                continue;
            }
            let into = first.get_or_insert(key).clone();
            let held = made.get_item(&into)?;
            for element in grouping.walk()? {
                held.call_method1("append", (element?,))?;
            }
        }
        Ok(made.unbind())
    }

    /// Repeated measures counted, key signatures told the one they follow,
    /// a measure's only rest made a whole-measure rest and every grouping
    /// told the signatures and options it is written under: music21's
    /// `addGroupingAttributes`, the crate's deciding and the objects
    /// written onto.
    fn addGroupingAttributes(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let read = Self::read_groupings(slf, None)?;
        let mut segment = Self::crate_segment(slf, &read)?;
        segment.add_grouping_attributes();
        let groupings = slf.borrow().groupings.clone_ref(py);
        let groupings = groupings.bind(py);
        for (key, object) in &read.keys {
            if !segment.groupings.contains_key(key) {
                groupings.del_item(object.bind(py))?;
            }
        }
        let (clefs, upper) = {
            let me = slf.borrow();
            (me.showClefSigns, me.upperFirstInNoteFingering)
        };
        let mut current_key = class_of(py, "music21.key", "KeySignature")?.call1((0,))?;
        let mut current_meter =
            class_of(py, "music21.meter.base", "TimeSignature")?.call1(("4/4",))?;
        for (key, written) in &segment.groupings {
            let (Some(object), Some(objects)) = (read.keys.get(key), read.objects.get(key)) else {
                continue;
            };
            let Some(grouping) = groupings.get_item(object.bind(py))? else {
                continue;
            };
            for (element, held) in written.elements.iter().zip(objects) {
                let held = held.bind(py);
                match &element.element {
                    StreamElement::TimeSignature(_) if key.affinity == Affinity::Signature => {
                        current_meter = held.clone();
                    }
                    StreamElement::Key(_) | StreamElement::KeySignature(_)
                        if key.affinity == Affinity::Signature =>
                    {
                        mark(held, "outgoingKeySig", current_key.clone());
                        current_key = held.clone();
                    }
                    StreamElement::Rest(rest)
                        if rest.full_measure() == music21::rest::FullMeasure::Yes
                            && !held
                                .getattr("fullMeasure")?
                                .is(pyo3::types::PyBool::new(py, true)) =>
                    {
                        held.setattr("fullMeasure", true)?;
                    }
                    _ => {}
                }
            }
            grouping.setattr("keySignature", &current_key)?;
            grouping.setattr("timeSignature", &current_meter)?;
            grouping.setattr(
                "descendingChords",
                written.descending_chords.unwrap_or(true),
            )?;
            grouping.setattr("showClefSigns", clefs)?;
            grouping.setattr("upperFirstInNoteFingering", upper)?;
            grouping.setattr("withHyphen", written.with_hyphen)?;
            grouping.setattr("numRepeats", written.num_repeats)?;
        }
        slf.borrow_mut().descendingChords = segment.descending_chords;
        Ok(())
    }

    /// Runs of four or more of one articulation written once doubled, and a
    /// staccato or tenuto over a tie written as a slur: music21's
    /// `fixArticulations`, the crate's deciding and each note's own
    /// articulations, tie and slur mark changed to match.
    fn fixArticulations(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let read = Self::read_groupings(slf, None)?;
        let mut segment = Self::crate_segment(slf, &read)?;
        segment.fix_articulations();
        for (key, written) in &segment.groupings {
            let Some(objects) = read.objects.get(key) else {
                continue;
            };
            let before = &read.groupings[key];
            for ((element, was), object) in
                written.elements.iter().zip(&before.elements).zip(objects)
            {
                let (StreamElement::Note(note), StreamElement::Note(old)) =
                    (&element.element, &was.element)
                else {
                    continue;
                };
                let object = object.bind(py);
                if note.tie().is_none() && old.tie().is_some() {
                    object.setattr("tie", py.None())?;
                }
                if element.context.short_slur && !was.context.short_slur {
                    mark(object, "shortSlur", true);
                }
                fix_articulations_of(object, note)?;
            }
        }
        Ok(())
    }
}

/// A note's own articulations changed to match the crate's: one doubled
/// where the crate holds more of it, and every one of a name the crate
/// dropped taken off.
fn fix_articulations_of(object: &Bound<'_, PyAny>, note: &music21::note::Note) -> PyResult<()> {
    let py = object.py();
    let held: Vec<Bound<'_, PyAny>> = object
        .getattr("articulations")?
        .walk()?
        .collect::<PyResult<_>>()?;
    let names: Vec<String> = held
        .iter()
        .map(|articulation| articulation.getattr("name")?.extract())
        .collect::<PyResult<_>>()?;
    let wanted = |name: &str| {
        note.articulations()
            .iter()
            .filter(|articulation| articulation.name() == name)
            .count()
    };
    let mut changed = false;
    let mut kept: Vec<Bound<'_, PyAny>> = Vec::new();
    for (articulation, name) in held.iter().zip(&names) {
        if wanted(name) == 0 {
            changed = true;
            continue;
        }
        kept.push(articulation.clone());
    }
    let mut seen: Vec<&String> = Vec::new();
    for (articulation, name) in held.iter().zip(&names) {
        if seen.contains(&name) {
            continue;
        }
        seen.push(name);
        let have = names.iter().filter(|other| *other == name).count();
        for _ in have..wanted(name) {
            kept.push(articulation.clone());
            changed = true;
        }
    }
    if changed {
        object.setattr("articulations", PyList::new(py, kept)?)?;
    }
    Ok(())
}

/// music21's `braille.segment.BrailleGrandSegment`: the segments of a
/// piano's two hands put together, written bar over bar.
///
/// music21's derives from its segment and its keyboard at once; this one
/// is a segment, and keeps the keyboard's own state -- which line each hand
/// is on, how wide the measure numbers are -- beside the text it shares.
#[pyclass(
    name = "BrailleGrandSegment",
    module = "music21.braille.segment",
    extends = BrailleSegment,
    subclass,
    skip_from_py_object
)]
pub struct BrailleGrandSegment {
    right_hand_line: Option<usize>,
    left_hand_line: Option<usize>,
    /// How many digits the highest measure number has.
    #[pyo3(get, set)]
    highestMeasureNumberLength: usize,
    /// The pairs of keys still to write, a deque.
    #[pyo3(get, set)]
    allKeyPairs: Py<PyAny>,
    /// The pair written before the one being written.
    #[pyo3(get, set)]
    previousGroupingPair: Option<Py<PyAny>>,
    /// The pair being written.
    #[pyo3(get, set)]
    currentGroupingPair: Option<Py<PyAny>>,
}

/// A pair of keys music21 keeps, as the crate's.
fn rs_pair(pair: &Bound<'_, PyAny>) -> PyResult<KeyPair> {
    let py = pair.py();
    let right = pair.get_item(0)?.unbind();
    let left = pair.get_item(1)?.unbind();
    Ok((
        optional_key(Some(&right), py)?,
        optional_key(Some(&left), py)?,
    ))
}

impl BrailleGrandSegment {
    /// The text under the segment.
    fn text<'py>(slf: &Bound<'py, Self>) -> Bound<'py, BrailleText> {
        slf.as_super().as_super().clone()
    }

    /// The keyboard as the crate keeps one, out of the text and the hands'
    /// lines.
    fn keyboard(slf: &Bound<'_, Self>) -> PyResult<RsKeyboard> {
        let (right_hand_line, left_hand_line, highest_measure_number_length) = {
            let me = slf.borrow();
            (
                me.right_hand_line,
                me.left_hand_line,
                me.highestMeasureNumberLength,
            )
        };
        Ok(RsKeyboard {
            text: crate::brailletext::text_value(&Self::text(slf))?,
            right_hand_line,
            left_hand_line,
            highest_measure_number_length,
        })
    }

    /// The keyboard's answer written back.
    fn written(slf: &Bound<'_, Self>, keyboard: RsKeyboard) -> PyResult<()> {
        {
            let mut me = slf.borrow_mut();
            me.right_hand_line = keyboard.right_hand_line;
            me.left_hand_line = keyboard.left_hand_line;
            me.highestMeasureNumberLength = keyboard.highest_measure_number_length;
        }
        crate::brailletext::set_text_value(&Self::text(slf), keyboard.text)
    }

    /// The segment as the crate's, both hands' groupings in it.
    fn crate_grand(slf: &Bound<'_, Self>, read: &ReadGroupings) -> PyResult<RsGrand> {
        let line_length: usize = slf.getattr("lineLength")?.extract()?;
        Ok(RsGrand {
            groupings: read.groupings.clone(),
            line_length,
            right_contexts: read.contexts.clone(),
            left_contexts: read.contexts.clone(),
        })
    }

    /// A step of writing the two hands: the groupings and keyboard read,
    /// the step run and what it changed written back.
    fn grand_step(
        slf: &Bound<'_, Self>,
        run: impl FnOnce(&mut RsGrand, &mut RsKeyboard) -> music21::error::Result<()>,
    ) -> PyResult<(RsGrand, ReadGroupings)> {
        let py = slf.py();
        let read = BrailleSegment::read_groupings(slf.as_super(), None)?;
        let mut grand = Self::crate_grand(slf, &read)?;
        let mut keyboard = Self::keyboard(slf)?;
        run(&mut grand, &mut keyboard).map_err(|error| segment_error(&error))?;
        Self::written(slf, keyboard)?;
        let groupings = slf.as_super().borrow().groupings.clone_ref(py);
        read.write_back(groupings.bind(py), &grand.groupings)?;
        Ok((grand, read))
    }

    /// A pair of the crate's keys as music21's tuple of keys.
    fn pair_object(
        py: Python<'_>,
        read: &ReadGroupings,
        (right, left): KeyPair,
    ) -> PyResult<Py<PyAny>> {
        let right = right.map(|key| read.key(py, key)).transpose()?;
        let left = left.map(|key| read.key(py, key)).transpose()?;
        Ok((right, left).into_pyobject(py)?.into_any().unbind())
    }

    /// The marks of transcribing taken off both hands' groupings in a pair,
    /// voices and all, as music21's `brailleFromKey` takes them.
    fn clean_pair(py: Python<'_>, read: &ReadGroupings, (right, left): KeyPair) {
        for key in [right, left].into_iter().flatten() {
            read.clean_up(py, key, true);
        }
    }
}

#[pymethods]
impl BrailleGrandSegment {
    /// music21's `BrailleGrandSegment(lineLength=40)`.
    #[new]
    #[pyo3(signature = (lineLength = 40))]
    fn new(py: Python<'_>, lineLength: usize) -> PyResult<PyClassInitializer<Self>> {
        let keyboard = RsKeyboard::new(lineLength);
        Ok(crate::brailletext::text_initializer(py, keyboard.text)?
            .add_subclass(BrailleSegment::fresh(py)?)
            .add_subclass(Self {
                right_hand_line: keyboard.right_hand_line,
                left_hand_line: keyboard.left_hand_line,
                highestMeasureNumberLength: keyboard.highest_measure_number_length,
                allKeyPairs: py
                    .import("collections")?
                    .getattr("deque")?
                    .call0()?
                    .unbind(),
                previousGroupingPair: None,
                currentGroupingPair: None,
            }))
    }

    fn __traverse__(&self, visit: pyo3::PyVisit<'_>) -> Result<(), pyo3::PyTraverseError> {
        visit.call(&self.allKeyPairs)?;
        visit.call(&self.previousGroupingPair)?;
        visit.call(&self.currentGroupingPair)?;
        Ok(())
    }

    fn __clear__(&mut self) {
        Python::attach(|py| {
            let _ = self.allKeyPairs.bind(py).call_method0("clear");
        });
        self.previousGroupingPair = None;
        self.currentGroupingPair = None;
    }

    /// The line the right hand is written on.
    #[getter]
    fn rightHandLine(slf: &Bound<'_, Self>) -> Option<Py<PyAny>> {
        let at = slf.borrow().right_hand_line?;
        crate::brailletext::line_object(&Self::text(slf), at)
    }

    /// The line the left hand is written on.
    #[getter]
    fn leftHandLine(slf: &Bound<'_, Self>) -> Option<Py<PyAny>> {
        let at = slf.borrow().left_hand_line?;
        crate::brailletext::line_object(&Self::text(slf), at)
    }

    /// A line for each hand: music21's `makeNewLines`.
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
            .map_err(|error| segment_error(&error))?;
        Self::written(slf, keyboard)
    }

    /// Every key, paired with the other hand's key for the same measure,
    /// place and kind: music21's `yieldCombinedGroupingKeys`.
    fn yieldCombinedGroupingKeys<'py>(
        slf: &Bound<'py, Self>,
    ) -> PyResult<Bound<'py, pyo3::types::PyIterator>> {
        let py = slf.py();
        let pairs = PyList::new(py, combined_keys(slf.as_super())?)?;
        pairs.as_any().try_iter()
    }

    /// Both hands in braille, a heading and then each measure's pair of
    /// groupings bar over bar: music21's `transcribe`.
    fn transcribe(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let (grand, read) =
            Self::grand_step(slf, |grand, keyboard| grand.transcribe_into(keyboard))?;
        let pairs = grand.combined_keys();
        for pair in &pairs {
            Self::clean_pair(py, &read, *pair);
        }
        let current = pairs
            .last()
            .map(|pair| Self::pair_object(py, &read, *pair))
            .transpose()?;
        let previous = pairs
            .len()
            .checked_sub(2)
            .map(|at| Self::pair_object(py, &read, pairs[at]))
            .transpose()?;
        {
            let mut me = slf.borrow_mut();
            me.allKeyPairs = py
                .import("collections")?
                .getattr("deque")?
                .call0()?
                .unbind();
            me.currentGroupingPair = current;
            me.previousGroupingPair = previous;
        }
        BrailleSegment::brailleText(slf.as_super())
    }

    /// The heading out of the pairs the segment starts with, which it takes
    /// off `allKeyPairs`: music21's `extractHeading`.
    fn extractHeading(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let held = slf.borrow().allKeyPairs.clone_ref(py);
        let mut pairs = VecDeque::new();
        for pair in held.bind(py).walk()? {
            pairs.push_back(rs_pair(&pair?)?);
        }
        let mut left = pairs.clone();
        let (_, read) = Self::grand_step(slf, |grand, keyboard| {
            grand.extract_heading(&mut left, keyboard)
        })?;
        let deque = py.import("collections")?.getattr("deque")?.call0()?;
        for pair in left {
            deque.call_method1("append", (Self::pair_object(py, &read, pair)?,))?;
        }
        slf.borrow_mut().allKeyPairs = deque.unbind();
        Ok(())
    }

    /// The pair being written, bar over bar: music21's
    /// `extractNoteGrouping`.
    fn extractNoteGrouping(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let pair = match slf.borrow().currentGroupingPair.as_ref() {
            Some(pair) => rs_pair(pair.bind(py))?,
            None => (None, None),
        };
        let (_, read) = Self::grand_step(slf, |grand, keyboard| {
            grand.extract_note_grouping(pair, keyboard)
        })?;
        Self::clean_pair(py, &read, pair);
        Ok(())
    }

    /// Each pair of groupings in English under its measure and hand, as
    /// music21 lists a grand segment.
    fn __str__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let py = slf.py();
        let groupings = slf.as_super().borrow().groupings.clone_ref(py);
        let groupings = groupings.bind(py);
        let heading = |key: &Bound<'_, PyAny>, hand: &str| -> PyResult<String> {
            let grouping = match groupings.get_item(key)? {
                Some(grouping) => grouping.str()?.to_string(),
                None => "None".to_string(),
            };
            Ok(format!(
                "Measure {} {hand}, {} {}:\n{grouping}",
                key.getattr("measure")?.str()?,
                affinity_name(key)?,
                key.getattr("ordinal")?.extract::<i64>()? + 1,
            ))
        };
        let mut pairs = String::new();
        for pair in combined_keys(slf.as_super())? {
            let right = pair.bind(py).get_item(0)?;
            let left = pair.bind(py).get_item(1)?;
            let right_full = if right.is_none() {
                String::new()
            } else {
                heading(&right, "Right")?
            };
            let left_full = if left.is_none() {
                String::new()
            } else {
                format!("\n{}", heading(&left, "Left")?)
            };
            pairs.push_str(&[right_full, left_full, "====\n".to_string()].join("\n"));
        }
        Ok([
            "---begin grand segment---".to_string(),
            "<music21.braille.segment BrailleGrandSegment>\n===".to_string(),
            pairs,
            "---end grand segment---".to_string(),
        ]
        .join("\n"))
    }

    /// music21's own `repr`, which is the text's.
    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        BrailleSegment::text_repr(slf.as_any())
    }
}

/// Where a key sorts among a grand segment's: measure, place, affinity,
/// the right hand first.
type KeyOrder = (i64, i64, i64, i64);

/// A segment's keys, each paired with the other hand's key for the same
/// measure, place and kind: music21's `yieldCombinedGroupingKeys`, over
/// the keys themselves, which music21's own examples make of numbers no
/// grouping has.
fn combined_keys(segment: &Bound<'_, BrailleSegment>) -> PyResult<Vec<Py<PyAny>>> {
    let py = segment.py();
    let groupings = segment.borrow().groupings.clone_ref(py);
    let mut keys: Vec<(KeyOrder, Bound<'_, PyAny>)> = Vec::new();
    for key in groupings.bind(py).keys() {
        let hand: Option<String> = key.getattr("hand")?.extract()?;
        let order = (
            key.getattr("measure")?.extract()?,
            key.getattr("ordinal")?.extract()?,
            key.getattr("affinity")?.extract()?,
            if hand.as_deref() == Some("right") {
                -1
            } else {
                1
            },
        );
        keys.push((order, key));
    }
    keys.sort_by_key(|(order, _)| *order);
    let matches =
        |one: KeyOrder, other: KeyOrder| one.0 == other.0 && one.1 == other.1 && one.2 == other.2;
    let none = || py.None().into_bound(py);
    let mut out: Vec<Py<PyAny>> = Vec::new();
    let mut push = |right: Bound<'_, PyAny>, left: Bound<'_, PyAny>| -> PyResult<()> {
        out.push((right, left).into_pyobject(py)?.into_any().unbind());
        Ok(())
    };
    let mut stored_right: Option<(KeyOrder, Bound<'_, PyAny>)> = None;
    let mut stored_left: Option<(KeyOrder, Bound<'_, PyAny>)> = None;
    let inaccord = Affinity::Inaccord as i64;
    for (order, key) in keys {
        let hand: Option<String> = key.getattr("hand")?.extract()?;
        match hand.as_deref() {
            Some("right") => {
                if let Some((left_order, left)) = stored_left.take() {
                    let as_inaccord = (order.0, order.1, inaccord, order.3);
                    if matches(order, left_order)
                        || (order.2 == Affinity::NoteGrouping as i64
                            && matches(as_inaccord, left_order))
                    {
                        push(key, left)?;
                    } else {
                        push(none(), left)?;
                        stored_right = Some((order, key));
                    }
                } else {
                    if let Some((_, right)) = stored_right.take() {
                        push(right, none())?;
                    }
                    stored_right = Some((order, key));
                }
            }
            Some("left") => {
                if let Some((right_order, right)) = stored_right.take() {
                    if matches(order, right_order) {
                        push(right, key)?;
                    } else if right_order.2 < inaccord {
                        push(right, none())?;
                        push(none(), key)?;
                    } else {
                        push(right, none())?;
                        stored_left = Some((order, key));
                    }
                } else {
                    if let Some((_, left)) = stored_left.take() {
                        push(none(), left)?;
                    }
                    stored_left = Some((order, key));
                }
            }
            _ => {}
        }
    }
    if let Some((_, right)) = stored_right {
        push(right, none())?;
    }
    if let Some((_, left)) = stored_left {
        push(none(), left)?;
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// The module's functions.

/// A part's notes marked with the slurs braille writes over them:
/// music21's `prepareSlurredNotes`.
#[pyfunction]
#[pyo3(signature = (
    music21Part,
    *,
    slurLongPhraseWithBrackets = true,
    showShortSlursAndTiesTogether = false,
    showLongSlursAndTiesTogether = None,
))]
fn prepareSlurredNotes(
    music21Part: &Bound<'_, PyAny>,
    #[pyo3(from_py_with = crate::truthy)] slurLongPhraseWithBrackets: bool,
    #[pyo3(from_py_with = crate::truthy)] showShortSlursAndTiesTogether: bool,
    #[pyo3(from_py_with = crate::truthy_or_none)] showLongSlursAndTiesTogether: Option<bool>,
) -> PyResult<()> {
    let located = Located::of(music21Part)?;
    let (_, options) = segment_options(
        None,
        None,
        40,
        showLongSlursAndTiesTogether,
        showShortSlursAndTiesTogether,
        slurLongPhraseWithBrackets,
    )?;
    mark_slurs(music21Part.py(), &located, &options);
    Ok(())
}

/// A part cut into segments of groupings of its own objects, its beamed
/// notes marked: music21's `getRawSegments`.
#[pyfunction]
#[pyo3(signature = (music21Part, *, setHand = None, maxLineLength = 40))]
fn getRawSegments(
    music21Part: &Bound<'_, PyAny>,
    setHand: Option<&str>,
    maxLineLength: usize,
) -> PyResult<Vec<Py<PyAny>>> {
    let located = Located::of(music21Part)?;
    let (hand, options) = segment_options(setHand, None, maxLineLength, None, false, true)?;
    raw_segments(music21Part.py(), &located, hand, options)
}

/// A part cut into segments ready to transcribe: its notes marked with
/// their slurs and beams, its groupings told their attributes and its
/// articulations fixed, all onto the part's own objects as music21 writes
/// them: music21's `findSegments`.
///
/// `segmentBreaks` is taken and passed over, as music21's own passes it
/// over.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (
    music21Part,
    *,
    setHand = None,
    cancelOutgoingKeySig = true,
    descendingChords = None,
    dummyRestLength = None,
    maxLineLength = 40,
    segmentBreaks = None,
    showClefSigns = false,
    showFirstMeasureNumber = true,
    showHand = None,
    showHeading = true,
    showLongSlursAndTiesTogether = None,
    showShortSlursAndTiesTogether = false,
    slurLongPhraseWithBrackets = true,
    suppressOctaveMarks = false,
    upperFirstInNoteFingering = true,
))]
fn findSegments(
    music21Part: &Bound<'_, PyAny>,
    setHand: Option<&str>,
    #[pyo3(from_py_with = crate::truthy)] cancelOutgoingKeySig: bool,
    #[pyo3(from_py_with = crate::truthy_or_none)] descendingChords: Option<bool>,
    dummyRestLength: Option<usize>,
    maxLineLength: usize,
    segmentBreaks: Option<&Bound<'_, PyAny>>,
    #[pyo3(from_py_with = crate::truthy)] showClefSigns: bool,
    #[pyo3(from_py_with = crate::truthy)] showFirstMeasureNumber: bool,
    showHand: Option<&str>,
    #[pyo3(from_py_with = crate::truthy)] showHeading: bool,
    #[pyo3(from_py_with = crate::truthy_or_none)] showLongSlursAndTiesTogether: Option<bool>,
    #[pyo3(from_py_with = crate::truthy)] showShortSlursAndTiesTogether: bool,
    #[pyo3(from_py_with = crate::truthy)] slurLongPhraseWithBrackets: bool,
    #[pyo3(from_py_with = crate::truthy)] suppressOctaveMarks: bool,
    #[pyo3(from_py_with = crate::truthy)] upperFirstInNoteFingering: bool,
) -> PyResult<Vec<Py<PyAny>>> {
    let py = music21Part.py();
    let _ = segmentBreaks;
    let (hand, options) = segment_options(
        setHand,
        descendingChords,
        maxLineLength,
        showLongSlursAndTiesTogether,
        showShortSlursAndTiesTogether,
        slurLongPhraseWithBrackets,
    )?;
    mark_slurs(py, &Located::of(music21Part)?, &options);
    let located = Located::of(music21Part)?;
    let segments = raw_segments(py, &located, hand, options)?;
    let mut descending = descendingChords;
    for segment in &segments {
        let segment = segment.bind(py);
        segment.setattr("showClefSigns", showClefSigns)?;
        segment.setattr("upperFirstInNoteFingering", upperFirstInNoteFingering)?;
        if let Some(descending) = descending {
            segment.setattr("descendingChords", descending)?;
        }
        segment.call_method0("addGroupingAttributes")?;
        descending = Some(segment.getattr("descendingChords")?.is_truthy()?);
        segment.setattr("cancelOutgoingKeySig", cancelOutgoingKeySig)?;
        segment.setattr("dummyRestLength", dummyRestLength)?;
        segment.setattr("showFirstMeasureNumber", showFirstMeasureNumber)?;
        segment.setattr("showHand", showHand)?;
        segment.setattr("showHeading", showHeading)?;
        segment.setattr("suppressOctaveMarks", suppressOctaveMarks)?;
        segment.call_method0("fixArticulations")?;
    }
    Ok(segments)
}

/// A measure's or voice's elements braille writes, in braille's order,
/// each marked with its affinity, sort order and English: music21's
/// `extractBrailleElements`.
#[pyfunction]
fn extractBrailleElements<'py>(
    music21MeasureOrVoice: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = music21MeasureOrVoice.py();
    let located = Located::of(music21MeasureOrVoice)?;
    let contexts = located.contexts(py);
    let grouping = extract_braille_elements(&located.stream, 0, &contexts);
    extracted_grouping(py, &located, &grouping.elements)
}

/// A measure's beamed notes marked as braille groups them: music21's
/// `prepareBeamedNotes`. The meter is the measure's own, or the one music21
/// finds in force around it.
#[pyfunction]
fn prepareBeamedNotes(music21Measure: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = music21Measure.py();
    let located = Located::of(music21Measure)?;
    let measure = &located.stream;
    let mut items: Vec<(f64, StreamElement)> = Vec::new();
    let mut leaves: Vec<Option<usize>> = Vec::new();
    if let Some(barline) = measure.left_barline() {
        items.push((0.0, StreamElement::Barline(barline.clone())));
        leaves.push(None);
    }
    let mut leaf = 0;
    for event in measure.events() {
        if let StreamElement::Stream(inner) = event.element() {
            leaf += inner.leaves().len();
            continue;
        }
        items.push((event.offset(), event.element().clone()));
        leaves.push(Some(leaf));
        leaf += 1;
    }
    if let Some(barline) = measure.right_barline() {
        items.push((
            measure.end_offset(),
            StreamElement::Barline(barline.clone()),
        ));
        leaves.push(None);
    }
    let meter = measure
        .events()
        .iter()
        .find_map(|event| match event.element() {
            StreamElement::TimeSignature(meter) if event.offset() <= 0.0 => Some(meter.clone()),
            _ => None,
        })
        .or_else(|| {
            let found = music21Measure
                .call_method1("getContextByClass", ("TimeSignature",))
                .ok()?;
            match crate::stream::element_read(&found).ok()?? {
                StreamElement::TimeSignature(meter) => Some(meter),
                _ => None,
            }
        });
    let flags = prepare_beamed_notes(&items, meter.as_ref());
    for (((_, element), leaf), (start, continued)) in items.iter().zip(&leaves).zip(flags) {
        let (Some(leaf), true) = (leaf, is_not_rest(element)) else {
            continue;
        };
        if let Some(object) = located.leaves.get(*leaf) {
            let object = object.bind(py);
            mark(object, "beamStart", start);
            mark(object, "beamContinue", continued);
        }
    }
    Ok(())
}

/// An object marked with the affinity braille groups it by and the order it
/// sorts in: music21's `setAffinityCode`.
#[pyfunction]
fn setAffinityCode(music21Object: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = music21Object.py();
    let refused = || {
        BrailleSegmentException::new_err(format!(
            "{} cannot be transcribed to braille.",
            music21Object
                .str()
                .map(|text| text.to_string())
                .unwrap_or_default()
        ))
    };
    let read = if has_class(music21Object, "Voice") {
        Some(segment_element(music21Object)?.0.element)
    } else {
        crate::stream::element_read(music21Object)?
    };
    let Some(element) = read else {
        return Err(refused());
    };
    if matches!(element, StreamElement::BrailleSegmentDivision(_)) {
        return Ok(());
    }
    let Some((affinity, order)) = affinity_code(&element) else {
        return Err(refused());
    };
    music21Object
        .getattr("editorial")?
        .setattr("affinityCode", affinity_object(py, affinity)?)?;
    let _ = music21Object.setattr("classSortOrder", order);
    Ok(())
}

/// Whether two groupings hold equal elements, place by place: music21's
/// `areGroupingsIdentical`.
#[pyfunction]
fn areGroupingsIdentical(
    noteGroupingA: &Bound<'_, PyAny>,
    noteGroupingB: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    if noteGroupingA.len()? != noteGroupingB.len()? {
        return Ok(false);
    }
    for (one, other) in noteGroupingA.try_iter()?.zip(noteGroupingB.try_iter()?) {
        if one?.ne(other?)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// A grouping split in two at the middle of its bar, or that many beat
/// divisions earlier, each half a grouping of the same objects with the
/// grouping's attributes, their beamed notes marked again: music21's
/// `splitNoteGrouping`.
#[pyfunction]
#[pyo3(signature = (noteGrouping, beatDivisionOffset = 0))]
fn splitNoteGrouping<'py>(
    noteGrouping: &Bound<'py, PyAny>,
    beatDivisionOffset: usize,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py = noteGrouping.py();
    let (mut read, objects) = crate_grouping(noteGrouping)?;
    for (leaf, element) in read.elements.iter_mut().enumerate() {
        element.leaf = leaf;
    }
    let (first, second) = split_note_grouping(&mut read, beatDivisionOffset)
        .map_err(|error| segment_error(&error))?
        .ok_or_else(|| {
            BrailleSegmentException::new_err(format!(
                "beatDivisionOffset {beatDivisionOffset} is outside of the meter's beat divisions"
            ))
        })?;
    let mut halves = Vec::new();
    for half in [first, second] {
        let held: Vec<Py<PyAny>> = half
            .elements
            .iter()
            .filter_map(|element| objects.get(element.leaf))
            .map(|object| object.clone_ref(py))
            .collect();
        for element in &half.elements {
            let Some(object) = objects.get(element.leaf) else {
                continue;
            };
            if is_not_rest(&element.element) {
                mark(object.bind(py), "beamStart", element.context.beam_start);
                mark(
                    object.bind(py),
                    "beamContinue",
                    element.context.beam_continue,
                );
            }
        }
        halves.push(grouping_like(py, Some(noteGrouping), &held)?);
    }
    let second = halves.pop().expect("two halves");
    let first = halves.pop().expect("two halves");
    Ok((first, second))
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<BrailleElementGrouping>()?;
    m.add_class::<BrailleSegment>()?;
    m.add_class::<BrailleGrandSegment>()?;
    m.add_function(wrap_pyfunction!(findSegments, m)?)?;
    m.add_function(wrap_pyfunction!(prepareSlurredNotes, m)?)?;
    m.add_function(wrap_pyfunction!(getRawSegments, m)?)?;
    m.add_function(wrap_pyfunction!(extractBrailleElements, m)?)?;
    m.add_function(wrap_pyfunction!(prepareBeamedNotes, m)?)?;
    m.add_function(wrap_pyfunction!(setAffinityCode, m)?)?;
    m.add_function(wrap_pyfunction!(areGroupingsIdentical, m)?)?;
    m.add_function(wrap_pyfunction!(splitNoteGrouping, m)?)?;
    Ok(())
}
