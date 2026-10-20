//! music21's `braille.segment`, as far as the groupings it is written out
//! of: a run of elements braille writes together.
//!
//! What a grouping *is* is a list of the score's own elements and the
//! signatures and flags they are written under, which is what music21's is;
//! the crate's `braille::segment::BrailleElementGrouping` is the same thing
//! as values, and `crate_grouping` reads one out of the other so that the
//! transcriber can be asked.
//!
//! The segments themselves -- `BrailleSegment`, `BrailleGrandSegment` and
//! the functions that cut a part into them -- are still music21's here.
//! The crate has them (`braille::segment`), and `braille_parity` holds
//! every one of its scores to music21 through them; what is missing is the
//! facade, which is the one piece of the braille package left.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyList;

use music21_rs_crate::braille::basic::NoteContext;
use music21_rs_crate::braille::segment::{
    Affinity, BrailleElement, BrailleElementGrouping as RsGrouping,
};
use music21_rs_crate::meter::TimeSignature;

use crate::Walkable;

/// The names this module hands out.
pub const NAMES: &[&str] = &["BrailleElementGrouping"];

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
/// package's own key and meter where music21 cannot be asked.
fn grouping_globals(py: Python<'_>) -> PyResult<(Py<PyAny>, Py<PyAny>)> {
    if let Ok(module) = py.import("music21.braille.segment") {
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
fn sharps_of(object: &Bound<'_, PyAny>) -> Option<music21_rs_crate::IntegerType> {
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
        object
            .getattr(name)
            .and_then(|flag| flag.is_truthy())
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
        outgoing_key: object
            .getattr("outgoingKeySig")
            .ok()
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
    let key = grouping.getattr("keySignature")?;
    let meter = grouping.getattr("timeSignature")?;
    let read = RsGrouping {
        elements,
        key_signature: if key.is_none() {
            Some(0)
        } else {
            sharps_of(&key)
        },
        time_signature: match crate::stream::element_read(&meter)? {
            Some(music21_rs_crate::stream::StreamElement::TimeSignature(meter)) => meter,
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
    };
    Ok((read, objects))
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
        use music21_rs_crate::braille::lookup::symbol;
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

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<BrailleElementGrouping>()?;
    Ok(())
}
