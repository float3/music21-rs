//! music21's `stream` module, as far as the wheel needs one: a place to put
//! notes at offsets, and the five walks over such a place that music21 keeps
//! in other modules.
//!
//! A stream here holds the very objects it is handed, at offsets it keeps
//! itself, so what `realizeVolume` or `iterateAllVoiceLeadingQuartets` hands
//! back or changes is the object a caller put in. The walks themselves are
//! the crate's: each stream is read into a `music21_rs::Stream` beside the
//! list of objects standing at each of its positions
//! (`music21_rs::Stream::leaves`), the crate answers by position, and the
//! answer is carried back onto the objects.
//!
//! The walks read music21's own streams as readily as these, through the
//! same `elements` and `elementOffset` both answer to. An element the crate
//! has no value for -- a clef, an instrument, a text mark -- is read as a
//! stand-in that keeps its place, its length and whether it sorts ahead of a
//! note, which is all any of the walks asks of it.
//!
//! None of it is installed over music21. music21's `Stream` is the machinery
//! its whole library is built on, sites and contexts and derivations, and
//! nothing here stands in for that; the classes are the wheel's own, for a
//! caller with no music21 at all.
//!
//! **Five of music21's `Stream` members the crate has ported are left out
//! here, and each is left out for a reason the package cannot talk its way
//! around.** `iter` is music21's iterator view, a filterable thing of its
//! own that this `Stream` answers with plain lists instead.
//! `splitAtDurations` replaces the elements *in the stream itself* and
//! answers a tuple of that same stream -- it is in-place only, and this
//! package refuses `inPlace=True` everywhere, since an edit here answers a
//! new stream and a caller's own objects would otherwise stop being the
//! ones the stream holds. `beatDuration` is a question about the site a
//! stream stands in, and nothing here keeps a site (see `DECISIONS.md`).
//! `activateVariants` and `showVariantAsOssialikePart` need a class for
//! music21's `variant.Variant`, which the package has not got.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::{PyIndexError, PyTypeError};
use pyo3::prelude::*;

use crate::Walkable;
use pyo3::types::{PyDict, PyList, PySlice, PyTuple};

use music21_rs_crate::braille::SegmentDivision;
use music21_rs_crate::voiceleading::QuartetOptions;
use music21_rs_crate::{Stream as RsStream, StreamElement, StreamEvent, StreamKind};

use crate::chord::Chord;
use crate::dynamics::Dynamic;
use crate::harmony::ChordSymbol;
use crate::key::KeySignature;
use crate::meter::TimeSignature;
use crate::note::Note;
use crate::rest::Rest;
use crate::tempo::{MetronomeMark, TempoException};

pyo3::create_exception!(music21_rs_facade, StreamException, crate::Music21Exception);

/// One thing a stream holds: where it stands, music21's `classSortOrder`
/// for it, the order it was put in, and the object itself.
struct Held {
    offset: f64,
    /// music21's `priority`, which comes before `classSortOrder` in its
    /// sort: a caller, or `measures` collecting what was in force, writes
    /// it to put something first whatever its class says.
    priority: i64,
    order: i64,
    index: u64,
    object: Py<PyAny>,
}

/// music21's `stream.Stream`: objects at quarter-length offsets, kept in
/// order of offset, then of `classSortOrder`, then of when each was put in.
///
/// It holds the objects it is given and hands the same ones back. `Score`,
/// `Part`, `PartStaff`, `Measure`, `Voice` and `Opus` are the same class
/// under music21's other names; a score's parts are what
/// `iterateAllVoiceLeadingQuartets` reads as its voices.
#[pyclass(name = "Stream", module = "music21.stream", subclass, dict)]
pub struct Stream {
    kind: StreamKind,
    held: Vec<Held>,
    next: u64,
    /// music21's `id`, where a caller gave one.
    id: Option<Py<PyAny>>,
    /// music21's `atSoundingPitch`: whether the notes are written where
    /// they sound. `None` is music21's `'unknown'`, which is what a stream
    /// says before anything has been decided.
    sounding: Option<bool>,
    /// music21's `leftBarline` and `rightBarline`. Kept beside the
    /// elements, where music21 holds them among them, so `len`, indexing and
    /// the places spanners name their notes by leave them out; `listed`
    /// puts them back for the listings. The objects given, as music21 keeps
    /// the ones it is given.
    left_barline: Option<Py<PyAny>>,
    right_barline: Option<Py<PyAny>>,
    /// What a measure is called and how much of its bar is missing:
    /// music21's `number`, `numberSuffix`, `paddingLeft` and
    /// `paddingRight`, which its `Measure.__init__` gives every measure.
    number: i32,
    number_suffix: Option<String>,
    padding_left: f64,
    padding_right: f64,
    /// music21's `partName` and `partAbbreviation`, where a caller or a
    /// reader has said them. Unsaid, a part answers for the first
    /// instrument it holds, as music21's property does.
    part_name: Option<String>,
    part_abbreviation: Option<String>,
    /// music21's `_unlinkedDuration`: a duration set apart from what the
    /// stream holds, which `duration` and `quarterLength` then answer.
    unlinked_duration: Option<Py<PyAny>>,
}

/// music21's `classSortOrder` for an object, which decides what comes first
/// among things at one offset: 20 for one that says nothing.
fn sort_order(object: &Bound<'_, PyAny>) -> i64 {
    object
        .getattr("classSortOrder")
        .and_then(|order| order.extract::<i64>())
        .unwrap_or(20)
}

/// Where a listed element sorts among those at its offset: as anything
/// held, or after all of them, as music21 keeps a measure's right barline.
#[derive(Clone, Copy)]
enum Placing {
    Held,
    Last,
}

/// How far `recurseRepr` indents what a held stream holds.
const REPR_INDENT: usize = 4;

/// A number with at most four decimal places and at least one, trailing
/// zeros dropped: music21's `common.strTrimFloat`, `1.0` and `1.3333`.
fn str_trim_float(value: f64) -> String {
    let written = format!("{value:.4}");
    let trimmed = written.trim_end_matches('0');
    match trimmed.ends_with('.') {
        true => format!("{trimmed}0"),
        false => trimmed.to_string(),
    }
}

/// How long an object lasts: a stream as long as its `duration`, anything
/// else as long as its duration, a thing with no duration no time at all.
fn length_of(object: &Bound<'_, PyAny>) -> PyResult<f64> {
    if let Ok(stream) = object.cast::<Stream>() {
        // As long as its `duration` says, which is set apart or what it
        // holds.
        return Stream::quarterLength(stream)?.extract();
    }
    // A note of this package hands its duration over without going through
    // Python's attribute lookup, which is most of what an append costs.
    if let Ok(note) = object.cast::<crate::note::Note>()
        && let Ok(mut note) = note.try_borrow_mut()
    {
        let held = note.duration_object(object.py())?;
        drop(note);
        if let Ok(ours) = held.bind(object.py()).cast::<crate::duration::Duration>()
            && let Ok(ours) = ours.try_borrow()
        {
            return Ok(ours.inner.quarter_length());
        }
    }

    let Ok(duration) = object.getattr("duration") else {
        return Ok(0.0);
    };
    if duration.is_none() {
        return Ok(0.0);
    }

    // One of this package's durations is read in Rust, which is what its
    // `quarterLength` answers, without building the Python number.
    if let Ok(ours) = duration.cast::<crate::duration::Duration>() {
        return Ok(ours.borrow().inner.quarter_length());
    }
    duration.getattr("quarterLength")?.extract()
}

/// Whether an object is of a class, named or given: a name matches any
/// class in the object's lineage, or any name in music21's `classes`.
fn is_of_class(object: &Bound<'_, PyAny>, class: &Bound<'_, PyAny>) -> PyResult<bool> {
    if let Ok(name) = class.extract::<String>() {
        let lineage = object.get_type().getattr("__mro__")?;
        for each in lineage.walk()? {
            if each?.getattr("__name__")?.extract::<String>()? == name {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    object.is_instance(class)
}

/// What music21 says an object *is*: its own class and every class it is a
/// kind of, in music21's order, which is what its `classes` answers.
///
/// The crate carries the hierarchy and `class_lineage_parity` holds it to
/// music21's own, down to `Music21Object`. What follows that is the
/// plumbing every music21 object is built on and nothing here models:
/// `ProtoM21Object` and `object`, and `typing.Generic` before `object` for
/// a stream, which music21's `Stream` is one of.
pub fn lineage_of(object: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let stream = object
        .getattr("isStream")
        .and_then(|flag| flag.is_truthy())
        .unwrap_or(false);
    let value = if stream {
        Some(StreamElement::Stream(Box::new(RsStream::with_kind(
            kind_of(object)?,
        ))))
    } else {
        element_value(object, false)?
    };
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let mut names: Vec<String> = value
        .music21_classes()
        .iter()
        .map(|name| (*name).to_string())
        .collect();
    names.push("ProtoM21Object".to_string());
    if stream {
        names.push("Generic".to_string());
    }
    names.push("object".to_string());
    Ok(names)
}

/// Whether an object is of a class, as music21 reads the question: by its
/// own name or by the name of anything it is a kind of.
///
/// A class of this package's answers to its own name and to the names of
/// the classes it extends, which are the ones music21 itself has a class
/// for. The rest of what music21 says a thing *is* -- a `GeneralNote`, a
/// `NotRest`, a `ChordBase`, a `Music21Object` -- is a class nothing here
/// is named after, so `getElementsByClass('GeneralNote')` used to find
/// nothing where music21 finds every note. The crate carries that
/// hierarchy (`StreamElement::music21_classes`), so whatever it has a
/// value for is asked of it.
///
/// It is asked last: reading the value is the costly part of this, and
/// every name a class is actually named after has already answered. The
/// readers do not come through here -- `element_value` itself asks
/// `is_of_class` -- so nothing reads a value to read a value.
pub(crate) fn answers_to_class(
    object: &Bound<'_, PyAny>,
    class: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    answers_to(object, class)
}

fn answers_to(object: &Bound<'_, PyAny>, class: &Bound<'_, PyAny>) -> PyResult<bool> {
    if is_of_class(object, class)? {
        return Ok(true);
    }
    // A class given as the class itself is read as its name. This
    // package's classes do not inherit music21's, so a measure of ours is
    // no instance of `stream.Measure` though it answers to the name, and
    // music21's own code asks for one that way.
    let named = || {
        class
            .getattr("__name__")
            .ok()
            .and_then(|name| name.extract::<String>().ok())
    };
    let Some(name) = class.extract::<String>().ok().or_else(named) else {
        return Ok(false);
    };
    // What the object says it is, which is music21's whole chain for one
    // of its objects and for one of this package's alike. The readers do
    // not ask this -- `is_of_class` matches the class names themselves --
    // because answering it builds a value, and building a value asks what
    // the thing is.
    if let Ok(classes) = object.getattr("classes") {
        for each in classes.walk()? {
            if each?.extract::<String>().is_ok_and(|each| each == name) {
                return Ok(true);
            }
        }
    }
    // A stream is asked by its kind alone, which costs nothing: what it
    // holds has no bearing on what it is.
    if object
        .getattr("isStream")
        .and_then(|flag| flag.is_truthy())
        .unwrap_or(false)
    {
        let kind = kind_of(object)?;
        let empty = StreamElement::Stream(Box::new(RsStream::with_kind(kind)));
        return Ok(empty.is_instance(&name));
    }
    match element_value(object, false) {
        Ok(Some(value)) => Ok(value.is_instance(&name)),
        _ => Ok(false),
    }
}

/// Whether an object is of any of a list of classes, or of the one given,
/// each read as music21 reads it.
fn is_of_any(object: &Bound<'_, PyAny>, classes: &Bound<'_, PyAny>) -> PyResult<bool> {
    if classes.extract::<String>().is_ok() || classes.is_instance_of::<pyo3::types::PyType>() {
        return answers_to(object, classes);
    }
    for class in classes.walk()? {
        if answers_to(object, &class?)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// A measure's number as music21's `Measure('14a')` reads one: the digits
/// it starts with, and what follows as the suffix.
fn read_measure_number(said: &str) -> (i32, Option<String>) {
    let digits: String = said.chars().take_while(char::is_ascii_digit).collect();
    let suffix: String = said.chars().skip(digits.chars().count()).collect();
    (
        digits.parse().unwrap_or(0),
        Some(suffix).filter(|suffix| !suffix.is_empty()),
    )
}

/// The barline a caller gave, as music21's `rightBarline` setter takes
/// one: a barline object, a name read as a type -- `'final'`, `'double'`
/// -- or nothing, which takes the barline away.
fn barline_given(value: Option<&Bound<'_, PyAny>>) -> PyResult<Option<Py<PyAny>>> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(None);
    };
    if let Ok(named) = value.extract::<String>() {
        let py = value.py();
        let kind = music21_rs_crate::bar::BarlineType::from_name(&named)
            .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
        let made = crate::bar::Barline::object(py, music21_rs_crate::Barline::new(kind))?;
        return Ok(Some(made));
    }
    Ok(Some(value.clone().unbind()))
}

/// What music21's `measures` collects into the first measure it answers,
/// where that measure does not state it itself: its `collect` default.
const COLLECTED_BY_MEASURES: [&str; 5] = [
    "Clef",
    "TimeSignature",
    "Instrument",
    "KeySignature",
    "MetronomeMark",
];

/// The names a class filter names: a string as it stands, a class by its
/// `__name__`, or a list of either -- which is what the crate's own walks
/// match an element against.
fn class_names(classes: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let one = |class: &Bound<'_, PyAny>| -> PyResult<String> {
        match class.extract::<String>() {
            Ok(name) => Ok(name),
            Err(_) => class.getattr("__name__")?.extract(),
        }
    };
    if classes.extract::<String>().is_ok() || classes.is_instance_of::<pyo3::types::PyType>() {
        return Ok(vec![one(classes)?]);
    }
    classes.walk()?.map(|class| one(&class?)).collect()
}

impl Stream {
    fn empty(kind: StreamKind) -> Self {
        Self {
            kind,
            held: Vec::new(),
            next: 0,
            id: None,
            sounding: None,
            left_barline: None,
            right_barline: None,
            number: 0,
            number_suffix: None,
            padding_left: 0.0,
            padding_right: 0.0,
            part_name: None,
            part_abbreviation: None,
            unlinked_duration: None,
        }
    }

    /// A stream of a kind holding what music21's constructor would put in
    /// it: at their own offsets, unless every one stands at nought, when
    /// they follow one another -- except that parts or voices given together
    /// stand side by side, since those are music at one time.
    fn filled(
        py: Python<'_>,
        kind: StreamKind,
        given: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let mut stream = Self::empty(kind);
        if let Some(keywords) = keywords {
            if let Some(id) = keywords.get_item("id")? {
                stream.id = Some(id.unbind());
            }
            // What music21's own `Measure.__init__` takes, read the way it
            // reads it: `Measure('14a')` is measure 14 with the suffix a.
            if let Some(number) = keywords.get_item("number")? {
                match number.extract::<String>() {
                    Ok(said) => {
                        let (number, suffix) = read_measure_number(&said);
                        stream.number = number;
                        stream.number_suffix = suffix;
                    }
                    Err(_) => stream.number = number.extract()?,
                }
            }
            if let Some(suffix) = keywords.get_item("numberSuffix")? {
                stream.number_suffix = suffix.extract::<Option<String>>()?;
            }
            if let Some(padding) = keywords.get_item("paddingLeft")? {
                stream.padding_left = padding.extract()?;
            }
            if let Some(padding) = keywords.get_item("paddingRight")? {
                stream.padding_right = padding.extract()?;
            }
        }
        let Some(given) = given.filter(|given| !given.is_none()) else {
            return Ok(stream);
        };
        let elements: Vec<Bound<'_, PyAny>> =
            if given.is_instance_of::<PyList>() || given.is_instance_of::<PyTuple>() {
                given.walk()?.collect::<PyResult<_>>()?
            } else {
                vec![given.clone()]
            };
        let behaviour: String = match keywords {
            Some(keywords) => match keywords.get_item("givenElementsBehavior")? {
                Some(value) => value.str()?.to_string().to_lowercase(),
                None => "offsets".to_string(),
            },
            None => "offsets".to_string(),
        };
        let offset_of = |element: &Bound<'_, PyAny>| -> f64 {
            element
                .getattr("offset")
                .and_then(|offset| offset.extract::<f64>())
                .unwrap_or(0.0)
        };
        let append = if behaviour.ends_with("insert") {
            false
        } else if behaviour.ends_with("append") {
            true
        } else {
            let all_at_nought = elements.iter().all(|element| offset_of(element) == 0.0);
            let side_by_side = elements.iter().all(|element| {
                element
                    .getattr("isStream")
                    .and_then(|flag| flag.is_truthy())
                    .unwrap_or(false)
                    && !is_of_class(element, &pyo3::types::PyString::new(py, "Measure"))
                        .unwrap_or(false)
                    && !is_of_class(element, &pyo3::types::PyString::new(py, "Score"))
                        .unwrap_or(false)
            });
            all_at_nought && !side_by_side
        };
        for element in &elements {
            let offset = if append {
                stream.end(py)?
            } else {
                offset_of(element)
            };
            stream.put(offset, element);
        }
        Ok(stream)
    }

    fn put(&mut self, offset: f64, object: &Bound<'_, PyAny>) {
        self.put_with_priority(offset, object, 0);
    }

    /// Puts an object in with music21's `priority`, which comes before
    /// `classSortOrder` in the order a stream lists what stands at one
    /// offset: `measures` gives what it collects the priority music21
    /// gives it, so the clef it found is listed before the measure.
    fn put_with_priority(&mut self, offset: f64, object: &Bound<'_, PyAny>, priority: i64) {
        self.held.push(Held {
            offset,
            priority,
            order: sort_order(object),
            index: self.next,
            object: object.clone().unbind(),
        });
        self.next += 1;
    }

    /// Where an object stands in this stream, as a number.
    fn offset_of(&self, element: &Bound<'_, PyAny>) -> PyResult<f64> {
        self.held
            .iter()
            .find(|held| held.object.bind(element.py()).is(element))
            .map(|held| held.offset)
            .ok_or_else(|| {
                StreamException::new_err(format!(
                    "an entry for this object {} is not stored in this stream",
                    element
                        .repr()
                        .map(|text| text.to_string())
                        .unwrap_or_default()
                ))
            })
    }

    /// What this stream holds, in music21's order.
    fn sorted(&self) -> Vec<&Held> {
        let mut sorted: Vec<&Held> = self.held.iter().collect();
        sorted.sort_by(|left, right| {
            left.offset
                .total_cmp(&right.offset)
                .then(left.priority.cmp(&right.priority))
                .then(left.order.cmp(&right.order))
                .then(left.index.cmp(&right.index))
        });
        sorted
    }

    /// The offset just past the last thing held.
    fn end(&self, py: Python<'_>) -> PyResult<f64> {
        let mut end: f64 = 0.0;
        for held in &self.held {
            end = end.max(held.offset + length_of(held.object.bind(py))?);
        }
        Ok(end)
    }

    fn highest_time(slf: &Bound<'_, Self>) -> PyResult<f64> {
        slf.borrow().end(slf.py())
    }

    /// The element nearest before an offset: those standing at or before
    /// it, the greatest offset of them, and of those the last in the
    /// stream's own order -- which is what music21's `max` over the sort
    /// tuples picks. `strictly` leaves out what stands at the offset
    /// itself, which is `getElementBeforeOffset`.
    fn nearest_before<'py>(
        slf: &Bound<'py, Self>,
        offset: f64,
        classes: Option<&Bound<'py, PyAny>>,
        strictly: bool,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let py = slf.py();
        let listed: Vec<(f64, Py<PyAny>)> = slf
            .borrow()
            .sorted()
            .iter()
            .map(|held| (held.offset, held.object.clone_ref(py)))
            .collect();
        let mut nearest: Option<(f64, Bound<'py, PyAny>)> = None;
        for (at, object) in listed {
            if at > offset || (strictly && at == offset) {
                continue;
            }
            let object = object.into_bound(py);
            if let Some(classes) = classes.filter(|list| !list.is_none())
                && !is_of_any(&object, classes)?
            {
                continue;
            }
            // Later in the listing at the same offset wins, so `>=`.
            if nearest.as_ref().is_none_or(|(held, _)| at >= *held) {
                nearest = Some((at, object));
            }
        }
        Ok(nearest.map(|(_, object)| object))
    }

    /// What the first instrument anywhere in this stream is called,
    /// under one name or the other: how music21's `partName` and
    /// `partAbbreviation` fall back.
    fn named_by_instrument(
        slf: &Bound<'_, Self>,
        first: &str,
        second: &str,
    ) -> PyResult<Option<String>> {
        let py = slf.py();
        let instrument = pyo3::types::PyString::new(py, "Instrument");
        let held: Vec<Py<PyAny>> = slf
            .borrow()
            .sorted()
            .iter()
            .map(|one| one.object.clone_ref(py))
            .collect();
        for object in held {
            let object = object.into_bound(py);
            if is_of_class(&object, &instrument)? {
                for name in [first, second] {
                    if let Some(said) = text_attribute(&object, name) {
                        return Ok(Some(said));
                    }
                }
            } else if let Ok(inner) = object.cast::<Stream>() {
                // music21 looks into the substreams too, a part's
                // instrument standing in its first measure.
                if let Some(said) = Self::named_by_instrument(inner, first, second)? {
                    return Ok(Some(said));
                }
            }
        }
        Ok(None)
    }

    /// The parts this stream holds, where it holds any: what music21's
    /// `hasPartLikeStreams` answers of it.
    fn part_like<'py>(slf: &Bound<'py, Self>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let py = slf.py();
        if !crate_stream(slf.as_any())?.has_part_like_streams() {
            return Ok(Vec::new());
        }
        let held: Vec<Py<PyAny>> = slf
            .borrow()
            .sorted()
            .iter()
            .map(|one| one.object.clone_ref(py))
            .collect();
        let mut parts = Vec::new();
        for object in held {
            let object = object.into_bound(py);
            if object
                .getattr("isStream")
                .and_then(|flag| flag.is_truthy())
                .unwrap_or(false)
            {
                parts.push(object);
            }
        }
        Ok(parts)
    }

    /// The last measure this stream holds, where it holds any.
    fn last_measure<'py>(slf: &Bound<'py, Self>) -> PyResult<Option<Bound<'py, PyAny>>> {
        let py = slf.py();
        let measure = pyo3::types::PyString::new(py, "Measure");
        let held: Vec<Py<PyAny>> = slf
            .borrow()
            .sorted()
            .iter()
            .map(|one| one.object.clone_ref(py))
            .collect();
        let mut last = None;
        for object in held {
            let object = object.into_bound(py);
            if is_of_class(&object, &measure)? {
                last = Some(object);
            }
        }
        Ok(last)
    }

    /// What of a class stands at the very start of this stream, the first
    /// where there are several: music21's `clef`, `keySignature` and
    /// `timeSignature` are each this of their own class.
    fn opening<'py>(slf: &Bound<'py, Self>, class: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        let py = slf.py();
        let named = pyo3::types::PyString::new(py, class);
        let standing: Vec<Py<PyAny>> = slf
            .borrow()
            .sorted()
            .iter()
            .filter(|held| held.offset == 0.0)
            .map(|held| held.object.clone_ref(py))
            .collect();
        for object in standing {
            let object = object.into_bound(py);
            if is_of_class(&object, &named)? {
                return Ok(Some(object));
            }
        }
        Ok(None)
    }

    /// Writes what the stream opens with, taking out the one of that class
    /// it had: music21's own setters do exactly that, and take `None` as
    /// leaving the stream with none.
    fn set_opening(
        slf: &Bound<'_, Self>,
        class: &str,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        if let Some(standing) = Self::opening(slf, class)? {
            slf.borrow_mut().remove(&standing)?;
        }
        if let Some(value) = value.filter(|value| !value.is_none()) {
            slf.borrow_mut().put(0.0, value);
        }
        Ok(())
    }

    /// Takes out what answers to these classes, or what does not.
    fn remove_where(
        slf: &Bound<'_, Self>,
        classes: &Bound<'_, PyAny>,
        matching: bool,
    ) -> PyResult<()> {
        let py = slf.py();
        let objects: Vec<Py<PyAny>> = slf
            .borrow()
            .held
            .iter()
            .map(|held| held.object.clone_ref(py))
            .collect();
        let mut going = Vec::new();
        for object in objects {
            let object = object.into_bound(py);
            if is_of_any(&object, classes)? == matching {
                going.push(object);
            }
        }
        for object in going {
            slf.borrow_mut().remove(&object)?;
        }
        Ok(())
    }

    /// A stream of this one's class holding what is picked, at the offsets
    /// it stands at here.
    fn picked<'py>(
        slf: &Bound<'py, Self>,
        kind: Option<StreamKind>,
        mut keep: impl FnMut(&Bound<'py, PyAny>) -> PyResult<bool>,
    ) -> PyResult<Bound<'py, Stream>> {
        let py = slf.py();
        let mut out = Self::empty(kind.unwrap_or(slf.borrow().kind));
        for (offset, object, last) in slf.borrow().listed(py)? {
            let object = object.into_bound(py);
            if !keep(&object)? {
                continue;
            }
            match last {
                Placing::Last => out.put_with_priority(offset, &object, i64::MAX),
                Placing::Held => out.put(offset, &object),
            }
        }
        Bound::new(py, out)
    }

    /// What this stream holds, in order, with a measure's own barlines
    /// where music21 lists them: the left first at nought, the right last
    /// at the end. They are kept as `leftBarline` and `rightBarline` and
    /// never in `held`, so `len`, indexing and the positions spanners name
    /// their notes by are untouched; the listings that ask what a stream
    /// holds by class, flattened or walked show them.
    fn listed(&self, py: Python<'_>) -> PyResult<Vec<(f64, Py<PyAny>, Placing)>> {
        let mut out: Vec<(f64, Py<PyAny>, Placing)> = Vec::new();
        if let Some(left) = &self.left_barline {
            out.push((0.0, left.clone_ref(py), Placing::Held));
        }
        out.extend(
            self.sorted()
                .iter()
                .map(|held| (held.offset, held.object.clone_ref(py), Placing::Held)),
        );
        if let Some(right) = &self.right_barline {
            out.push((self.end(py)?, right.clone_ref(py), Placing::Last));
        }
        Ok(out)
    }

    /// [`Stream::recurse`]'s walk: every element with its offset from the
    /// top, a nested stream before what it holds.
    fn recurse_into(
        slf: &Bound<'_, Self>,
        base: f64,
        found: &mut Vec<(f64, Py<PyAny>)>,
    ) -> PyResult<()> {
        let py = slf.py();
        let held = slf.borrow().listed(py)?;
        for (offset, object, _) in held {
            let at = base + offset;
            found.push((at, object.clone_ref(py)));
            if let Ok(inner) = object.bind(py).cast::<Stream>() {
                Self::recurse_into(inner, at, found)?;
            }
        }
        Ok(())
    }

    fn flatten_into(
        slf: &Bound<'_, Self>,
        base: f64,
        out: &mut Vec<(f64, Py<PyAny>)>,
    ) -> PyResult<()> {
        let py = slf.py();
        let held = slf.borrow().listed(py)?;
        for (offset, object, _) in held {
            let object = object.into_bound(py);
            match object.cast::<Stream>() {
                Ok(inner) => Self::flatten_into(inner, base + offset, out)?,
                Err(_) => out.push((base + offset, object.unbind())),
            }
        }
        Ok(())
    }

    fn class_name(slf: &Bound<'_, Self>) -> PyResult<String> {
        slf.get_type().name()?.extract()
    }
}

#[pymethods]
impl Stream {
    /// What music21 says this is: its own class and every class it is a
    /// kind of, as music21's `classes` answers.
    #[getter]
    fn classes(slf: &Bound<'_, Self>) -> PyResult<Vec<String>> {
        crate::stream::lineage_of(slf.as_any())
    }
    #[new]
    #[pyo3(signature = (givenElements = None, *_arguments, **keywords))]
    fn new(
        py: Python<'_>,
        givenElements: Option<&Bound<'_, PyAny>>,
        _arguments: &Bound<'_, PyTuple>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        Self::filled(py, StreamKind::Stream, givenElements, keywords)
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        for held in &self.held {
            visit.call(&held.object)?;
        }
        visit.call(&self.id)
    }

    fn __clear__(&mut self) {
        self.held.clear();
        self.id = None;
    }

    /// A stream is a stream.
    #[classattr]
    fn isStream() -> bool {
        true
    }

    /// music21's sort order for a stream among other things at one offset.
    #[classattr]
    fn classSortOrder() -> i64 {
        -20
    }

    /// What this part is called: music21's `partName`, which is what a
    /// caller or a reader said, and failing that what the first
    /// instrument it holds is called -- its own `partName`, else its
    /// `instrumentName`, as music21 looks.
    ///
    /// music21 keeps that search's answer in a cache and says so; nothing
    /// is kept here, so an instrument renamed after the fact is answered
    /// for straight away.
    #[getter]
    fn get_partName(slf: &Bound<'_, Self>) -> PyResult<Option<String>> {
        if let Some(said) = &slf.borrow().part_name {
            return Ok(Some(said.clone()));
        }
        Self::named_by_instrument(slf, "partName", "instrumentName")
    }

    #[setter]
    fn set_partName(&mut self, value: Option<String>) {
        self.part_name = value;
    }

    /// What this part is called in short: music21's `partAbbreviation`,
    /// which falls back the same way -- the first instrument's own
    /// abbreviation, else its `instrumentAbbreviation`.
    #[getter]
    fn get_partAbbreviation(slf: &Bound<'_, Self>) -> PyResult<Option<String>> {
        if let Some(said) = &slf.borrow().part_abbreviation {
            return Ok(Some(said.clone()));
        }
        Self::named_by_instrument(slf, "partAbbreviation", "instrumentAbbreviation")
    }

    #[setter]
    fn set_partAbbreviation(&mut self, value: Option<String>) {
        self.part_abbreviation = value;
    }

    /// What this measure is called: music21's `number`, nought until
    /// something says, as music21's `Measure.__init__` leaves it.
    #[getter]
    fn get_number(&self) -> i32 {
        self.number
    }

    /// Numbers the measure. A number leaves the suffix as it stands, as
    /// music21's own attribute does; a name is read as music21's
    /// `Measure('14a')` is, the digits as the number and the rest as the
    /// suffix, since the number is kept as a number here.
    #[setter]
    fn set_number(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Ok(said) = value.extract::<String>() {
            let (number, suffix) = read_measure_number(&said);
            self.number = number;
            self.number_suffix = suffix;
            return Ok(());
        }
        self.number = value.extract()?;
        Ok(())
    }

    /// What follows the number, as the `a` of a measure numbered `14a`:
    /// music21's `numberSuffix`.
    #[getter]
    fn get_numberSuffix(&self) -> Option<&str> {
        self.number_suffix.as_deref()
    }

    #[setter]
    fn set_numberSuffix(&mut self, value: Option<String>) {
        self.number_suffix = value.filter(|suffix| !suffix.is_empty());
    }

    /// How much of the bar is missing from the start of this measure:
    /// music21's `paddingLeft`, which is what makes a pickup a pickup.
    #[getter]
    fn get_paddingLeft(&self) -> f64 {
        self.padding_left
    }

    #[setter]
    fn set_paddingLeft(&mut self, value: f64) {
        self.padding_left = value;
    }

    /// How much of the bar is missing from the end of this measure:
    /// music21's `paddingRight`.
    #[getter]
    fn get_paddingRight(&self) -> f64 {
        self.padding_right
    }

    #[setter]
    fn set_paddingRight(&mut self, value: f64) {
        self.padding_right = value;
    }

    /// The barline closing the left side of this measure, or `None`:
    /// music21's `leftBarline`.
    #[getter]
    fn get_leftBarline(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.left_barline.as_ref().map(|bar| bar.clone_ref(py))
    }

    /// Writes the barline on the left of this measure. A name is read as
    /// music21 reads one, so `'final'` is a final barline.
    #[setter]
    fn set_leftBarline(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.left_barline = barline_given(value)?;
        Ok(())
    }

    /// The barline closing this measure, or `None`: music21's
    /// `rightBarline`.
    #[getter]
    fn get_rightBarline(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.right_barline.as_ref().map(|bar| bar.clone_ref(py))
    }

    /// Writes the barline closing this measure.
    #[setter]
    fn set_rightBarline(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.right_barline = barline_given(value)?;
        Ok(())
    }

    /// Whether the notes are written where they sound: music21's
    /// `atSoundingPitch`, which is `True`, `False` or `'unknown'`, and
    /// `'unknown'` until something says.
    ///
    /// Writing it says nothing about the notes themselves;
    /// `toSoundingPitch` and `toWrittenPitch` move them.
    #[getter]
    fn get_atSoundingPitch<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match self.sounding {
            Some(at) => Ok(pyo3::types::PyBool::new(py, at).to_owned().into_any()),
            None => Ok(pyo3::types::PyString::new(py, "unknown").into_any()),
        }
    }

    /// Says whether the notes are written where they sound: music21's
    /// setter, which refuses anything but `True`, `False` and `'unknown'`.
    #[setter]
    fn set_atSoundingPitch(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        if value.is_instance_of::<pyo3::types::PyBool>() {
            self.sounding = Some(value.extract()?);
            return Ok(());
        }
        if value
            .extract::<String>()
            .is_ok_and(|said| said == "unknown")
        {
            self.sounding = None;
            return Ok(());
        }
        Err(StreamException::new_err(format!(
            "not a valid at sounding pitch value: {}",
            value.str()?
        )))
    }

    #[getter]
    fn get_id(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.id.as_ref().map(|id| id.clone_ref(py))
    }

    #[setter]
    fn set_id(&mut self, value: Option<&Bound<'_, PyAny>>) {
        self.id = value
            .filter(|value| !value.is_none())
            .map(|value| value.clone().unbind());
    }

    /// Puts an object, or each of a list of them, after everything held.
    fn append(slf: &Bound<'_, Self>, others: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let others: Vec<Bound<'_, PyAny>> =
            if others.is_instance_of::<PyList>() || others.is_instance_of::<PyTuple>() {
                others.walk()?.collect::<PyResult<_>>()?
            } else {
                vec![others.clone()]
            };
        // The end is found once and carried along, so appending a list
        // reads each held length once rather than once per item.
        let mut offset = slf.borrow().end(py)?;
        for other in &others {
            slf.borrow_mut().put(offset, other);
            offset = offset.max(offset + length_of(other)?);
        }
        Ok(())
    }

    /// Puts an object at an offset: `insert(offset, item)`, `insert(item)`
    /// at the item's own offset or nought, or `insert([offset, item, ...])`.
    #[pyo3(signature = (offsetOrItemOrList, itemOrNone = None))]
    fn insert(
        &mut self,
        offsetOrItemOrList: &Bound<'_, PyAny>,
        itemOrNone: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        if let Some(item) = itemOrNone.filter(|item| !item.is_none()) {
            self.put(offsetOrItemOrList.extract()?, item);
            return Ok(());
        }
        if offsetOrItemOrList.is_instance_of::<PyList>()
            || offsetOrItemOrList.is_instance_of::<PyTuple>()
        {
            let items: Vec<Bound<'_, PyAny>> =
                offsetOrItemOrList.walk()?.collect::<PyResult<_>>()?;
            if !items.len().is_multiple_of(2) {
                return Err(StreamException::new_err(
                    "an insert list must alternate offsets and objects",
                ));
            }
            for pair in items.chunks(2) {
                self.put(pair[0].extract()?, &pair[1]);
            }
            return Ok(());
        }
        let offset = offsetOrItemOrList
            .getattr("offset")
            .and_then(|offset| offset.extract::<f64>())
            .unwrap_or(0.0);
        self.put(offset, offsetOrItemOrList);
        Ok(())
    }

    /// Takes an object out, found by identity.
    fn remove(&mut self, target: &Bound<'_, PyAny>) -> PyResult<()> {
        match self
            .held
            .iter()
            .position(|held| held.object.bind(target.py()).is(target))
        {
            Some(at) => {
                self.held.remove(at);
                Ok(())
            }
            None => Err(StreamException::new_err(format!(
                "cannot find {} in this stream",
                target.repr()?
            ))),
        }
    }

    /// Where an object stands among `elements`, found by identity:
    /// music21's `index`.
    fn index(&self, el: &Bound<'_, PyAny>) -> PyResult<usize> {
        self.sorted()
            .iter()
            .position(|held| held.object.bind(el.py()).is(el))
            .ok_or_else(|| {
                let shown = el.str().map(|text| text.to_string()).unwrap_or_default();
                StreamException::new_err(format!("cannot find object ({shown}) in Stream"))
            })
    }

    /// Puts `replacement` where `target` stands: music21's `replace`. A
    /// replacement already held is refused, and a target not held is
    /// passed over, or with `recurse` looked for in the streams held.
    /// `allDerived` is accepted and has nothing to follow: a stream here
    /// keeps no derivation chain.
    #[pyo3(signature = (target, replacement, *, recurse = false, allDerived = true))]
    fn replace(
        &mut self,
        target: &Bound<'_, PyAny>,
        replacement: &Bound<'_, PyAny>,
        recurse: bool,
        allDerived: bool,
    ) -> PyResult<()> {
        let _ = allDerived;
        let py = target.py();
        if self
            .held
            .iter()
            .any(|held| held.object.bind(py).is(replacement))
        {
            let shown = replacement.str()?.to_string();
            return Err(StreamException::new_err(format!(
                "{shown} already in this stream"
            )));
        }
        if let Some(held) = self
            .held
            .iter_mut()
            .find(|held| held.object.bind(py).is(target))
        {
            held.object = replacement.clone().unbind();
            held.order = sort_order(replacement);
            return Ok(());
        }
        if !recurse {
            return Ok(());
        }
        let nested: Vec<Py<PyAny>> = self
            .held
            .iter()
            .map(|held| held.object.clone_ref(py))
            .collect();
        for inner in nested {
            let Ok(inner) = inner.bind(py).cast::<Stream>() else {
                continue;
            };
            inner
                .borrow_mut()
                .replace(target, replacement, recurse, allDerived)?;
        }
        Ok(())
    }

    /// The first element held whose `id` is `elementId`, or `None`:
    /// music21's `getElementById`, which looks at this stream's own
    /// elements and not into the streams they hold.
    fn getElementById<'py>(
        &self,
        py: Python<'py>,
        elementId: &Bound<'py, PyAny>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        for held in self.sorted() {
            let object = held.object.bind(py);
            let Ok(id) = object.getattr("id") else {
                continue;
            };
            if id.eq(elementId)? {
                return Ok(Some(object.clone()));
            }
        }
        Ok(None)
    }

    /// This stream's own elements, in order, as music21's `iter` hands
    /// them: a `StreamIterator`, which filters as `recurse` does.
    fn iter<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, Walked>> {
        let found = self
            .sorted()
            .iter()
            .map(|held| (held.offset, held.object.clone_ref(py)))
            .collect();
        Bound::new(py, Walked::of(found, self.kind))
    }

    /// The stream holding `el`, this one or one held at any depth below
    /// it, or `None`: music21's `containerInHierarchy`. A stream here keeps
    /// no active site, so `setActiveSite` has nothing to set.
    #[pyo3(signature = (el, *, setActiveSite = true))]
    fn containerInHierarchy<'py>(
        slf: &Bound<'py, Self>,
        el: &Bound<'py, PyAny>,
        setActiveSite: bool,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let py = slf.py();
        let held: Vec<Py<PyAny>> = slf
            .borrow()
            .sorted()
            .iter()
            .map(|held| held.object.clone_ref(py))
            .collect();
        if held.iter().any(|object| object.bind(py).is(el)) {
            return Ok(Some(slf.clone().into_any()));
        }
        let keywords = PyDict::new(py);
        keywords.set_item("setActiveSite", setActiveSite)?;
        let stream = pyo3::types::PyString::new(py, "Stream");
        for object in held {
            let object = object.bind(py);
            if !is_of_class(object, &stream)? {
                continue;
            }
            let found = object.call_method("containerInHierarchy", (el,), Some(&keywords))?;
            if !found.is_none() {
                return Ok(Some(found));
            }
        }
        Ok(None)
    }

    /// Each element on a line of its own with its offset, a held stream's
    /// indented beneath it: music21's `recurseRepr`, which `show('text')`
    /// prints.
    #[pyo3(signature = (
        *,
        prefixSpaces = 0,
        addBreaks = true,
        addIndent = true,
        addEndTimes = false,
        useMixedNumerals = false
    ))]
    fn recurseRepr(
        slf: &Bound<'_, Self>,
        prefixSpaces: usize,
        addBreaks: bool,
        addIndent: bool,
        addEndTimes: bool,
        useMixedNumerals: bool,
    ) -> PyResult<String> {
        let py = slf.py();
        let held: Vec<(f64, Py<PyAny>)> = slf
            .borrow()
            .sorted()
            .iter()
            .map(|held| (held.offset, held.object.clone_ref(py)))
            .collect();
        let written = |value: f64| -> PyResult<String> {
            match useMixedNumerals {
                true => crate::duration::mixed_numeral(py, value),
                false => Ok(str_trim_float(value)),
            }
        };
        let indent = match addIndent {
            true => " ".repeat(prefixSpaces),
            false => String::new(),
        };
        let keywords = PyDict::new(py);
        keywords.set_item("prefixSpaces", prefixSpaces + REPR_INDENT)?;
        keywords.set_item("addBreaks", addBreaks)?;
        keywords.set_item("addIndent", addIndent)?;
        keywords.set_item("addEndTimes", addEndTimes)?;
        keywords.set_item("useMixedNumerals", useMixedNumerals)?;
        let stream = pyo3::types::PyString::new(py, "Stream");

        let mut lines = Vec::new();
        for (offset, object) in held {
            let object = object.bind(py);
            // `{1.0} <...>`, or with end times `{1.0 - 1.3333} <...>`.
            let mut place = written(offset)?;
            if addEndTimes {
                place = format!("{place} - {}", written(offset + length_of(object)?)?);
            }
            lines.push(format!("{indent}{{{place}}} {}", object.repr()?));
            if is_of_class(object, &stream)? {
                let inner = object.call_method("recurseRepr", (), Some(&keywords))?;
                lines.push(inner.extract()?);
            }
        }
        let joint = match addBreaks {
            true => "\n",
            false => " / ",
        };
        Ok(lines.join(joint))
    }

    /// music21's `show` for its text formats: `'text'` (or `'txt'`, `'t'`)
    /// prints the `recurseRepr` listing, taking `addEndTimes` and
    /// `useMixedNumerals`, and `'textline'` answers it on one line. A
    /// format that renders -- MusicXML, MIDI, an image -- is refused: the
    /// wheel writes those documents and leaves showing them to its caller.
    #[pyo3(signature = (fmt = None, app = None, **keywords))]
    fn show<'py>(
        slf: &Bound<'py, Self>,
        fmt: Option<&str>,
        app: Option<&Bound<'py, PyAny>>,
        keywords: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Option<String>> {
        let _ = app;
        let py = slf.py();
        let said = |name: &str| -> PyResult<bool> {
            match keywords
                .map(|given| given.get_item(name))
                .transpose()?
                .flatten()
            {
                Some(value) => value.is_truthy(),
                None => Ok(false),
            }
        };
        let format = fmt.unwrap_or("").to_lowercase();
        match format.as_str() {
            "text" | "txt" | "t" => {
                let listing = Self::recurseRepr(
                    slf,
                    0,
                    true,
                    true,
                    said("addEndTimes")?,
                    said("useMixedNumerals")?,
                )?;
                py.import("builtins")?.getattr("print")?.call1((listing,))?;
                Ok(None)
            }
            "textline" => Ok(Some(Self::recurseRepr(
                slf,
                0,
                false,
                false,
                said("addEndTimes")?,
                said("useMixedNumerals")?,
            )?)),
            _ => Err(StreamException::new_err(format!(
                "cannot show {fmt:?} here: only 'text' and 'textline' are shown; \
                 write the document and open it instead"
            ))),
        }
    }

    /// Takes everything out, keeping what else the stream says of itself:
    /// music21's `clear`. A measure's barlines go too, being among
    /// music21's elements.
    fn clear(&mut self) {
        self.held.clear();
        self.left_barline = None;
        self.right_barline = None;
    }

    /// The element of this stream sounding where another element starts:
    /// music21's `playingWhenAttacked`, which answers the one sounding
    /// there, or of several the first of that element's own class, and
    /// failing that the first.
    ///
    /// The element handed back is this stream's own. Where music21 asks
    /// `isinstance`, the class is read as the name of the element's own
    /// class: nothing in this package answers to music21's class names
    /// above its own.
    #[pyo3(signature = (el, elStream = None))]
    fn playingWhenAttacked<'py>(
        slf: &Bound<'py, Self>,
        el: &Bound<'py, PyAny>,
        elStream: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let py = slf.py();
        let offset = match elStream.filter(|given| !given.is_none()) {
            Some(given) => given
                .call_method1("elementOffset", (el,))?
                .extract::<f64>()?,
            None => slf.borrow().offset_of(el).unwrap_or(0.0),
        };
        let span = music21_rs_crate::stream::OffsetSpan {
            must_begin_in_span: false,
            ..music21_rs_crate::stream::OffsetSpan::at(offset)
        };
        let listed: Vec<(f64, Py<PyAny>)> = slf
            .borrow()
            .sorted()
            .iter()
            .map(|one| (one.offset, one.object.clone_ref(py)))
            .collect();
        let mut there = Vec::new();
        for (at, object) in listed {
            let object = object.into_bound(py);
            if span.takes_at(at, length_of(&object)?) {
                there.push(object);
            }
        }
        if there.len() < 2 {
            return Ok(there.into_iter().next());
        }
        let class = el.get_type().name()?;
        let same = there
            .iter()
            .find(|object| {
                object
                    .get_type()
                    .name()
                    .is_ok_and(|name| name.to_string() == class.to_string())
            })
            .cloned();
        Ok(same.or_else(|| there.into_iter().next()))
    }

    /// Everything this stream holds that sounds while another element
    /// sounds, each at its offset from where that element stands:
    /// music21's `allPlayingWhileSounding`.
    ///
    /// The element searched for stands in `elStream` where one is given,
    /// else at the offset it is found at here; the elements handed back
    /// are this stream's own.
    #[pyo3(signature = (el, elStream = None))]
    fn allPlayingWhileSounding<'py>(
        slf: &Bound<'py, Self>,
        el: &Bound<'py, PyAny>,
        elStream: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, Stream>> {
        let py = slf.py();
        let offset = match elStream.filter(|given| !given.is_none()) {
            Some(given) => given
                .call_method1("elementOffset", (el,))?
                .extract::<f64>()?,
            None => slf.borrow().offset_of(el).unwrap_or(0.0),
        };
        let length = length_of(el)?;
        let end = offset + length;
        // The span the crate reads this by: anything sounding anywhere in
        // the element's own stretch, and a search at one offset where the
        // element lasts no time.
        let span = if end == offset {
            music21_rs_crate::stream::OffsetSpan {
                must_begin_in_span: false,
                ..music21_rs_crate::stream::OffsetSpan::at(offset)
            }
        } else {
            music21_rs_crate::stream::OffsetSpan {
                must_begin_in_span: false,
                include_end_boundary: false,
                include_elements_that_end_at_start: false,
                ..music21_rs_crate::stream::OffsetSpan::between(offset, end)
            }
        };
        let listed: Vec<(f64, Py<PyAny>)> = slf
            .borrow()
            .sorted()
            .iter()
            .map(|one| (one.offset, one.object.clone_ref(py)))
            .collect();
        let mut out = Self::empty(slf.borrow().kind);
        for (at, object) in listed {
            let object = object.into_bound(py);
            if span.takes_at(at, length_of(&object)?) {
                out.put(at - offset, &object);
            }
        }
        Bound::new(py, out)
    }

    /// What the stream holds around one of its elements -- from `before`
    /// quarters before it begins to `after` quarters past where it ends:
    /// music21's `extractContext`.
    ///
    /// The elements are the stream's own. `maxBefore` and `maxAfter` go
    /// unread here, as they do upstream.
    #[pyo3(signature = (searchElement, before = 4.0, after = 4.0, maxBefore = None, maxAfter = None))]
    fn extractContext<'py>(
        slf: &Bound<'py, Self>,
        searchElement: &Bound<'py, PyAny>,
        before: f64,
        after: f64,
        maxBefore: Option<f64>,
        maxAfter: Option<f64>,
    ) -> PyResult<Bound<'py, Stream>> {
        // music21 takes both and reads neither.
        let _ = (maxBefore, maxAfter);
        let py = slf.py();
        let listed: Vec<(f64, Py<PyAny>)> = slf
            .borrow()
            .sorted()
            .iter()
            .map(|one| (one.offset, one.object.clone_ref(py)))
            .collect();
        let found = listed
            .iter()
            .find(|(_, object)| object.bind(py).is(searchElement))
            .ok_or_else(|| StreamException::new_err("Could not find the element in the stream"))?;
        let start = found.0;
        let end = start + length_of(&found.1.bind(py).clone())?;
        let mut out = Self::empty(slf.borrow().kind);
        for (at, object) in &listed {
            if start - before <= *at && *at < end + after {
                out.put(*at, &object.bind(py).clone());
            }
        }
        Bound::new(py, out)
    }

    /// Writes onto each of this stream's notes the interval to what
    /// sounds against it in another stream: music21's
    /// `attachIntervalsBetweenStreams`.
    ///
    /// The interval goes on the note's `editorial.harmonicInterval`,
    /// where music21 puts it, and is `None` for a note with nothing
    /// sounding against it -- music21 clears it first, so a note asked
    /// twice never keeps the older answer. The notes written on are this
    /// stream's own, and only the ones it holds itself, as music21's
    /// `getElementsByClass` there is not recursive.
    fn attachIntervalsBetweenStreams(
        slf: &Bound<'_, Self>,
        cmpStream: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let py = slf.py();
        let read = crate_stream(slf.as_any())?;
        let other = crate_stream(cmpStream)?;
        // The objects the crate counted: everything the stream holds in
        // its own order, the spanners left out as the reading leaves
        // them out.
        let spanner = pyo3::types::PyString::new(py, "Spanner");
        let held: Vec<Py<PyAny>> = slf
            .borrow()
            .sorted()
            .iter()
            .map(|one| one.object.clone_ref(py))
            .collect();
        let mut listed = Vec::new();
        for object in held {
            let object = object.into_bound(py);
            if !is_of_class(&object, &spanner)? {
                listed.push(object);
            }
        }
        for (position, interval) in read.harmonic_intervals(&other) {
            let Some(object) = listed.get(position) else {
                continue;
            };
            let editorial = object.getattr("editorial")?;
            match interval {
                Some(interval) => {
                    let made = Bound::new(py, crate::interval::Interval::wrap(interval))?;
                    editorial.setattr("harmonicInterval", made)?;
                }
                None => editorial.setattr("harmonicInterval", py.None())?,
            }
        }
        Ok(())
    }

    /// Every pitch of everything the stream holds, in order: music21's
    /// `pitches`, which gathers a note's one pitch, a chord's several and
    /// a nested stream's own, each the very object that note holds.
    ///
    /// A key is passed over, as music21 passes it over: it has pitches but
    /// they are not the music's.
    #[getter]
    fn pitches<'py>(slf: &Bound<'py, Self>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let py = slf.py();
        let key = pyo3::types::PyString::new(py, "Key");
        let listed: Vec<Py<PyAny>> = slf
            .borrow()
            .sorted()
            .iter()
            .map(|one| one.object.clone_ref(py))
            .collect();
        let mut out = Vec::new();
        for object in listed {
            let object = object.into_bound(py);
            if is_of_class(&object, &key)? {
                continue;
            }
            // music21 gathers from its `GeneralNote`s and its streams,
            // which is everything carrying pitches once a key is passed
            // over; the package's classes do not answer to music21's
            // class names above their own, so the attribute is the test.
            let Ok(pitches) = object.getattr("pitches") else {
                continue;
            };
            for pitch in pitches.walk()? {
                out.push(pitch?);
            }
        }
        Ok(out)
    }

    /// Every element with where it begins, where it ends and which voice
    /// it is in: music21's `offsetMap`, a list of its `OffsetMap`
    /// named tuples -- `element`, `offset`, `endTime`, `voiceIndex`.
    ///
    /// A stream with voices is read voice by voice, each element's offset
    /// counted from the voice's own start, as music21 reads it: the
    /// elements standing outside the voices come first with no voice. A
    /// barline is left out; this package holds none anyway.
    #[pyo3(signature = (srcObj = None))]
    fn offsetMap<'py>(
        slf: &Bound<'py, Self>,
        srcObj: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let read = match srcObj.filter(|given| !given.is_none()) {
            Some(given) => given.cast::<Stream>()?.clone(),
            None => slf.clone(),
        };
        let entry = py.import("collections")?.call_method1(
            "namedtuple",
            (
                "OffsetMap",
                vec!["element", "offset", "endTime", "voiceIndex"],
            ),
        )?;
        let voice = pyo3::types::PyString::new(py, "Voice");
        let out = PyList::empty(py);
        // Each group of elements with the voice it belongs to, music21's
        // own order: what stands outside the voices first.
        let mut groups: Vec<(Bound<'py, Stream>, Option<usize>)> = Vec::new();
        let voices: Vec<Py<PyAny>> = read
            .borrow()
            .sorted()
            .iter()
            .filter(|one| is_of_class(one.object.bind(py), &voice).unwrap_or(false))
            .map(|one| one.object.clone_ref(py))
            .collect();
        if voices.is_empty() {
            groups.push((read.clone(), None));
        } else {
            let others = Self::picked(&read, Some(StreamKind::Stream), |object| {
                is_of_class(object, &voice).map(|found| !found)
            })?;
            if !others.borrow().held.is_empty() {
                groups.push((others, None));
            }
            for (index, object) in voices.into_iter().enumerate() {
                let inner = object.into_bound(py).cast::<Stream>()?.clone();
                groups.push((Self::flatten(&inner)?, Some(index)));
            }
        }
        for (group, voiceIndex) in groups {
            let listed: Vec<(f64, Py<PyAny>)> = group
                .borrow()
                .sorted()
                .iter()
                .map(|one| (one.offset, one.object.clone_ref(py)))
                .collect();
            for (offset, object) in listed {
                let object = object.into_bound(py);
                let end = offset + length_of(&object)?;
                out.append(entry.call1((object, offset, end, voiceIndex))?)?;
            }
        }
        Ok(out)
    }

    /// The measures numbered from one number to another, in a stream of
    /// this one's class: music21's `measures`, whose range takes both ends
    /// in.
    ///
    /// The first measure stands at nought and the rest keep their distance
    /// from it. Before them stands what was in force where the first
    /// begins and the first does not state itself -- the clef, meter,
    /// instrument, key signature and tempo mark music21's `collect` names
    /// -- found by looking back through the stream. A score answers a
    /// score of each of its parts' measures, as music21's override does.
    ///
    /// The measures and the collected marks are the stream's own objects.
    /// `gatherSpanners` has nothing to do here, the package keeping no
    /// spanners.
    #[pyo3(signature = (numberStart, numberEnd, collect = None, gatherSpanners = true, indicesNotNumbers = false))]
    fn measures<'py>(
        slf: &Bound<'py, Self>,
        numberStart: &Bound<'py, PyAny>,
        numberEnd: &Bound<'py, PyAny>,
        collect: Option<&Bound<'py, PyAny>>,
        gatherSpanners: bool,
        indicesNotNumbers: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let _ = gatherSpanners;
        let py = slf.py();
        let asked: Vec<String> = match collect.filter(|list| !list.is_none()) {
            Some(list) => class_names(list)?,
            None => COLLECTED_BY_MEASURES
                .iter()
                .map(|name| (*name).to_string())
                .collect(),
        };
        let first: isize = numberStart.extract()?;
        let last: isize = numberEnd.extract()?;

        // A score answers out of its parts, each measured the same way.
        let part = pyo3::types::PyString::new(py, "Part");
        let held: Vec<Py<PyAny>> = slf
            .borrow()
            .sorted()
            .iter()
            .map(|one| one.object.clone_ref(py))
            .collect();
        let parts: Vec<Bound<'py, PyAny>> = held
            .iter()
            .map(|object| object.bind(py).clone())
            .filter(|object| is_of_class(object, &part).unwrap_or(false))
            .collect();
        if !parts.is_empty() {
            let out = Self::cloneEmpty(slf, None)?;
            for inner in parts {
                let measured = Self::measures(
                    inner.cast::<Stream>()?,
                    numberStart,
                    numberEnd,
                    collect,
                    gatherSpanners,
                    indicesNotNumbers,
                )?;
                out.cast::<Stream>()?.borrow_mut().put(0.0, &measured);
            }
            return Ok(out);
        }

        let measure = pyo3::types::PyString::new(py, "Measure");
        let mut found: Vec<(f64, Bound<'py, PyAny>)> = Vec::new();
        let mut index: isize = 0;
        for one in slf.borrow().sorted() {
            let object = one.object.bind(py).clone();
            if !is_of_class(&object, &measure)? {
                continue;
            }
            let number = if indicesNotNumbers {
                index
            } else {
                object
                    .getattr("number")
                    .and_then(|number| number.extract::<isize>())
                    .unwrap_or(0)
            };
            index += 1;
            // With indices music21 slices, so the end is left out;
            // with numbers both ends are taken in.
            let inside = if indicesNotNumbers {
                number >= first && number < last
            } else {
                number >= first && number <= last
            };
            if inside {
                found.push((one.offset, object));
            }
        }
        let out = Self::cloneEmpty(slf, None)?;
        let kept = out.cast::<Stream>()?;
        let Some((start, opening)) = found.first().map(|(at, object)| (*at, object.clone())) else {
            return Ok(out);
        };
        // What was in force where the first measure begins, which that
        // measure does not say itself.
        let opening = opening.cast::<Stream>()?;
        let flat = Self::flatten(slf)?;
        for name in &asked {
            if Self::opening(opening, name)?.is_some() {
                continue;
            }
            let named = PyList::new(py, [name.as_str()])?;
            if let Some(standing) = Self::nearest_before(&flat, start, Some(named.as_any()), false)?
            {
                // music21 gives what it collects the measure's priority
                // less one, which sorts it before the measure at the same
                // offset whatever its class says.
                kept.borrow_mut().put_with_priority(0.0, &standing, -1);
            }
        }
        for (at, object) in found {
            kept.borrow_mut().put(at - start, &object);
        }
        Ok(out)
    }

    /// The measure of a number, or `None`: music21's `measure`, which a
    /// score answers as a score of its parts' measures of that number.
    #[pyo3(signature = (measureNumber, collect = None, gatherSpanners = true, indicesNotNumbers = false))]
    fn measure<'py>(
        slf: &Bound<'py, Self>,
        measureNumber: &Bound<'py, PyAny>,
        collect: Option<&Bound<'py, PyAny>>,
        gatherSpanners: bool,
        indicesNotNumbers: bool,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let py = slf.py();
        let measured = Self::measures(
            slf,
            measureNumber,
            measureNumber,
            collect,
            gatherSpanners,
            indicesNotNumbers,
        )?;
        let part = pyo3::types::PyString::new(py, "Part");
        if slf
            .borrow()
            .sorted()
            .iter()
            .any(|one| is_of_class(one.object.bind(py), &part).unwrap_or(false))
        {
            // A score answers the slice of its parts, as music21's does.
            return Ok(Some(measured));
        }
        let measure = pyo3::types::PyString::new(py, "Measure");
        let cell = measured.cast::<Stream>()?;
        let objects: Vec<Py<PyAny>> = cell
            .borrow()
            .sorted()
            .iter()
            .map(|one| one.object.clone_ref(py))
            .collect();
        for object in objects {
            let object = object.into_bound(py);
            if is_of_class(&object, &measure)? {
                return Ok(Some(object));
            }
        }
        Ok(None)
    }

    /// The elements of a class divided in two by what a function says of
    /// each: music21's `splitByClass`, a pair of streams, the first of
    /// those the function answers true for.
    ///
    /// `classObj` of `None` takes everything the stream holds, as music21
    /// does. The objects divided are the stream's own.
    fn splitByClass<'py>(
        slf: &Bound<'py, Self>,
        classObj: &Bound<'py, PyAny>,
        fx: &Bound<'py, PyAny>,
    ) -> PyResult<(Bound<'py, Stream>, Bound<'py, Stream>)> {
        let py = slf.py();
        let listed: Vec<(f64, Py<PyAny>)> = slf
            .borrow()
            .sorted()
            .iter()
            .map(|held| (held.offset, held.object.clone_ref(py)))
            .collect();
        let mut yes = Self::empty(slf.borrow().kind);
        let mut no = Self::empty(slf.borrow().kind);
        for (offset, object) in listed {
            let object = object.into_bound(py);
            if !classObj.is_none() && !is_of_any(&object, classObj)? {
                continue;
            }
            if fx.call1((&object,))?.is_truthy()? {
                yes.put(offset, &object);
            } else {
                no.put(offset, &object);
            }
        }
        Ok((Bound::new(py, yes)?, Bound::new(py, no)?))
    }

    /// Puts an object at an offset, first moving everything from the
    /// element nearest after that offset onwards far enough that it starts
    /// where the new one ends: music21's `insertAndShift`, which changes
    /// the stream and answers nothing.
    ///
    /// Where a thing stands is the stream's own to say, so the offsets are
    /// moved here; which of them move, and by how much, is the rule
    /// `Stream::insert_and_shift` follows.
    #[pyo3(signature = (offsetOrItemOrList, itemOrNone = None))]
    fn insertAndShift(
        slf: &Bound<'_, Self>,
        offsetOrItemOrList: &Bound<'_, PyAny>,
        itemOrNone: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        // music21 takes an offset and an object, or an object standing at
        // the offset it says itself.
        let (offset, item) = match itemOrNone.filter(|item| !item.is_none()) {
            Some(item) => (offsetOrItemOrList.extract::<f64>()?, item.clone()),
            None => {
                let at = offsetOrItemOrList
                    .getattr("offset")
                    .and_then(|offset| offset.extract::<f64>())
                    .unwrap_or(0.0);
                (at, offsetOrItemOrList.clone())
            }
        };
        let end = offset + length_of(&item)?;
        let nearest = slf
            .borrow()
            .held
            .iter()
            .map(|held| held.offset)
            .filter(|at| *at >= offset)
            .fold(None, |best: Option<f64>, at| {
                Some(best.map_or(at, |best| if at < best { at } else { best }))
            });
        if let Some(nearest) = nearest {
            let shift = end - nearest;
            if shift > 0.0 {
                for one in &mut slf.borrow_mut().held {
                    if one.offset >= offset {
                        one.offset += shift;
                    }
                }
            }
        }
        slf.borrow_mut().put(offset, &item);
        Ok(())
    }

    /// The measures this stream holds, gathered by the offset each stands
    /// at: music21's `measureOffsetMap`, whose answer a score builds out of
    /// its parts, so measures standing together in different parts are
    /// listed together.
    ///
    /// Only `Measure` is taken as a class to gather by: music21's other
    /// classes are found by asking an element which measure holds it,
    /// which needs the sites nothing here keeps.
    #[pyo3(signature = (classFilterList = None))]
    fn measureOffsetMap<'py>(
        slf: &Bound<'py, Self>,
        classFilterList: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyDict>> {
        let py = slf.py();
        if let Some(asked) = classFilterList.filter(|list| !list.is_none()) {
            let names = class_names(asked)?;
            if names.iter().any(|name| name != "Measure") {
                return Err(StreamException::new_err(
                    "only Measure is supported here: gathering by another class asks each element which measure holds it, which needs the sites nothing here keeps",
                ));
            }
        }
        let measure = pyo3::types::PyString::new(py, "Measure");
        let part = pyo3::types::PyString::new(py, "Part");
        let listed: Vec<(f64, Py<PyAny>)> = slf
            .borrow()
            .sorted()
            .iter()
            .map(|held| (held.offset, held.object.clone_ref(py)))
            .collect();
        // A score answers out of its parts, each measure at the offset it
        // stands at *in its part*.
        let mut gathered: Vec<(f64, Bound<'py, PyAny>)> = Vec::new();
        let mut parts = false;
        for (_, object) in &listed {
            let object = object.bind(py);
            if is_of_class(object, &part)? {
                parts = true;
                let inner = Self::measureOffsetMap(object.cast::<Stream>()?, None)?;
                for (offset, measures) in inner.iter() {
                    let offset: f64 = offset.extract()?;
                    for held in measures.try_iter()? {
                        gathered.push((offset, held?));
                    }
                }
            }
        }
        if !parts {
            for (offset, object) in &listed {
                let object = object.bind(py);
                if is_of_class(object, &measure)? {
                    gathered.push((*offset, object.clone()));
                }
            }
        }
        let mut offsets: Vec<f64> = Vec::new();
        for (offset, _) in &gathered {
            if !offsets.contains(offset) {
                offsets.push(*offset);
            }
        }
        offsets.sort_by(f64::total_cmp);
        let out = PyDict::new(py);
        for offset in offsets {
            let standing: Vec<&Bound<'py, PyAny>> = gathered
                .iter()
                .filter(|(at, _)| *at == offset)
                .map(|(_, object)| object)
                .collect();
            out.set_item(offset, PyList::new(py, standing)?)?;
        }
        Ok(out)
    }

    /// Appends a copy of an object this many times, each after the last:
    /// music21's `repeatAppend`.
    ///
    /// The copies are copies, as music21's `deepcopy` makes them, so the
    /// object handed in is not the one the stream ends up holding.
    fn repeatAppend(
        slf: &Bound<'_, Self>,
        item: &Bound<'_, PyAny>,
        numberOfTimes: usize,
    ) -> PyResult<()> {
        let py = slf.py();
        for _ in 0..numberOfTimes {
            let copy = py.import("copy")?.call_method1("deepcopy", (item,))?;
            let at = slf.borrow().end(py)?;
            slf.borrow_mut().put(at, &copy);
        }
        Ok(())
    }

    /// Puts a copy of an object at each of these offsets: music21's
    /// `repeatInsert`.
    fn repeatInsert(
        slf: &Bound<'_, Self>,
        item: &Bound<'_, PyAny>,
        offsets: Vec<f64>,
    ) -> PyResult<()> {
        let py = slf.py();
        for offset in offsets {
            let copy = py.import("copy")?.call_method1("deepcopy", (item,))?;
            slf.borrow_mut().put(offset, &copy);
        }
        Ok(())
    }

    /// Adds a note or chord where something already starts within its
    /// length, the two becoming one chord: music21's
    /// `insertIntoNoteOrChord`.
    ///
    /// Where a rest stands there it gives way; where nothing does, the
    /// note or chord handed in is simply put in, and is the very object
    /// the stream then holds. A merged chord is a new object, as
    /// music21's is, keeping the length, articulations, expressions,
    /// lyrics, stem and notehead of what stood there; with `chordsOnly`
    /// a merge of one pitch is still a chord.
    #[pyo3(signature = (offset, noteOrChord, *, chordsOnly = false))]
    fn insertIntoNoteOrChord(
        slf: &Bound<'_, Self>,
        offset: f64,
        noteOrChord: &Bound<'_, PyAny>,
        chordsOnly: bool,
    ) -> PyResult<()> {
        let py = slf.py();
        let Some(added) = element_value(noteOrChord, false)? else {
            return Err(StreamException::new_err(
                "this package has no value for what was handed in",
            ));
        };
        // The span music21 looks in: from the offset to where the new
        // element ends, taking what begins in it and not what starts
        // exactly at the end.
        let span = music21_rs_crate::stream::OffsetSpan {
            include_end_boundary: false,
            ..music21_rs_crate::stream::OffsetSpan::between(offset, offset + added.quarter_length())
        };
        let listed: Vec<(f64, Py<PyAny>)> = slf
            .borrow()
            .sorted()
            .iter()
            .map(|one| (one.offset, one.object.clone_ref(py)))
            .collect();
        let mut targets: Vec<(f64, Bound<'_, PyAny>, StreamElement)> = Vec::new();
        for (at, object) in listed {
            let object = object.into_bound(py);
            if !span.takes_at(at, length_of(&object)?) {
                continue;
            }
            // Only a note, a chord or a rest is something to merge with.
            let Some(value) = element_value(&object, false)? else {
                continue;
            };
            if value.is_instance("GeneralNote") {
                targets.push((at, object, value));
            }
        }
        match targets.len() {
            0 => {
                slf.borrow_mut().put(offset, noteOrChord);
                Ok(())
            }
            1 => {
                let (at, object, value) = targets.remove(0);
                // The merging is the crate's: a stream of the one thing
                // standing there, asked to take the new one in.
                let mut one = RsStream::new();
                one.insert(at, value);
                one.insert_into_note_or_chord(offset, added, chordsOnly)
                    .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
                let mut made = Vec::new();
                for event in one.events() {
                    if let Some(built) = element_object(py, event.element())? {
                        made.push((event.offset(), built));
                    }
                }
                slf.borrow_mut().remove(&object)?;
                let mut held = slf.borrow_mut();
                for (at, built) in made {
                    held.put(at, &built);
                }
                Ok(())
            }
            _ => Err(StreamException::new_err(
                "more than one element found at the specified offset",
            )),
        }
    }

    /// Which beat of which measure an offset falls on: music21's
    /// `beatAndMeasureFromOffset`, a pair of the beat and the measure.
    ///
    /// The measure is this stream's own object, found as music21 finds
    /// it -- the last one standing at or before the offset, in the stream
    /// itself or in the first of its parts that has measures -- and the
    /// beat is the crate's arithmetic over the meter in force, `fixZeros`
    /// counting the second half of a measure cut in two as part of the
    /// measure before it.
    #[pyo3(signature = (searchOffset, fixZeros = true))]
    fn beatAndMeasureFromOffset<'py>(
        slf: &Bound<'py, Self>,
        searchOffset: f64,
        fixZeros: bool,
    ) -> PyResult<(f64, Option<Bound<'py, PyAny>>)> {
        let py = slf.py();
        let stream = crate_stream(slf.as_any())?;
        let (beat, _) = stream
            .beat_and_measure_from_offset(searchOffset, fixZeros)
            .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
        // Where music21 looks for the measure: this stream if it holds
        // measures itself, else the first of its parts that does.
        let measure = pyo3::types::PyString::new(py, "Measure");
        let mut holder = slf.clone();
        if Self::opening(slf, "Measure")?.is_none()
            && !slf
                .borrow()
                .held
                .iter()
                .any(|one| is_of_class(one.object.bind(py), &measure).unwrap_or(false))
        {
            let inner: Vec<Py<PyAny>> = slf
                .borrow()
                .sorted()
                .iter()
                .map(|one| one.object.clone_ref(py))
                .collect();
            for object in inner {
                let object = object.into_bound(py);
                let Ok(cell) = object.cast::<Stream>() else {
                    continue;
                };
                let holds = cell
                    .borrow()
                    .held
                    .iter()
                    .any(|one| is_of_class(one.object.bind(py), &measure).unwrap_or(false));
                if holds {
                    holder = cell.clone();
                    break;
                }
            }
        }
        let named = PyList::new(py, ["Measure"])?;
        let found = Self::nearest_before(&holder, searchOffset, Some(named.as_any()), false)?;
        Ok((beat, found))
    }

    /// A copy with its repeats played out: music21's `expandRepeats`,
    /// which a score answers for each of its parts.
    ///
    /// The repeats are the ones the measures' barlines say, so a stream
    /// whose measures carry none is answered unchanged -- and a repeat
    /// music21 itself refuses to expand is refused here too.
    #[pyo3(signature = (copySpanners = true))]
    fn expandRepeats<'py>(
        slf: &Bound<'py, Self>,
        copySpanners: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        // The package keeps no spanners, so there are none to copy.
        let _ = copySpanners;
        edited(slf, RsStream::expand_repeats)
    }

    /// The barline closing the last measure of this stream: music21's
    /// `finalBarline`, which a stream holding parts answers for each of
    /// them, as a list.
    ///
    /// A stream with no measures answers its own `rightBarline`, and a
    /// stream with neither answers nothing, as music21 does.
    #[getter]
    fn get_finalBarline<'py>(slf: &Bound<'py, Self>) -> PyResult<Option<Bound<'py, PyAny>>> {
        let py = slf.py();
        let parts = Self::part_like(slf)?;
        if !parts.is_empty() {
            let held = PyList::empty(py);
            for part in parts {
                let answer = Self::get_finalBarline(part.cast::<Stream>()?)?;
                match answer {
                    Some(barline) => held.append(barline)?,
                    None => held.append(py.None())?,
                }
            }
            return Ok(Some(held.into_any()));
        }
        match Self::last_measure(slf)? {
            Some(measure) => Ok(measure
                .cast::<Stream>()?
                .borrow()
                .get_rightBarline(py)
                .map(|bar| bar.into_bound(py))),
            None => Ok(slf
                .borrow()
                .get_rightBarline(py)
                .map(|bar| bar.into_bound(py))),
        }
    }

    /// Writes the barline closing the last measure, or each part's where
    /// the stream holds parts -- taking one barline for all of them or a
    /// list, as music21 does. A name is read as a type.
    #[setter]
    fn set_finalBarline(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let parts = Self::part_like(slf)?;
        if !parts.is_empty() {
            let given: Vec<Bound<'_, PyAny>> =
                if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
                    value.try_iter()?.collect::<PyResult<_>>()?
                } else {
                    vec![value.clone()]
                };
            if given.is_empty() {
                return Ok(());
            }
            // music21 goes round the list where there are more parts
            // than barlines given.
            for (at, part) in parts.iter().enumerate() {
                let said = &given[at % given.len()];
                Self::set_finalBarline(part.cast::<Stream>()?, said)?;
            }
            return Ok(());
        }
        let made = barline_given(Some(value))?;
        match Self::last_measure(slf)? {
            Some(measure) => {
                measure.cast::<Stream>()?.borrow_mut().right_barline = made;
            }
            None => slf.borrow_mut().right_barline = made,
        }
        let _ = py;
        Ok(())
    }

    /// Everything in the stream that overlaps something else, gathered by
    /// the offset each overlap begins at: music21's `getOverlaps`.
    ///
    /// The stream is read flattened, as music21 reads it, and the
    /// elements handed back are its own. Each group lists the element
    /// overlapped before the one that found it, which is the order
    /// music21's own dictionary takes them in.
    fn getOverlaps<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyDict>> {
        let py = slf.py();
        let read = read_values(slf.as_any())?;
        let out = PyDict::new(py);
        for (offset, members) in read.stream.overlap_positions() {
            let held = PyList::empty(py);
            for position in members {
                held.append(read.leaves[position].bind(py))?;
            }
            out.set_item(offset, held)?;
        }
        Ok(out)
    }

    /// The element standing at or before an offset, the last of the
    /// stream's order where several stand there: music21's
    /// `getElementAtOrBefore`.
    #[pyo3(signature = (offset, classList = None))]
    fn getElementAtOrBefore<'py>(
        slf: &Bound<'py, Self>,
        offset: f64,
        classList: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        Self::nearest_before(slf, offset, classList, false)
    }

    /// The element standing before an offset, nothing standing at it
    /// counting: music21's `getElementBeforeOffset`.
    #[pyo3(signature = (offset, classList = None))]
    fn getElementBeforeOffset<'py>(
        slf: &Bound<'py, Self>,
        offset: f64,
        classList: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        Self::nearest_before(slf, offset, classList, true)
    }

    /// The element after one this stream holds, in the order the stream is
    /// listed in: music21's `getElementAfterElement`.
    #[pyo3(signature = (element, classList = None))]
    fn getElementAfterElement<'py>(
        slf: &Bound<'py, Self>,
        element: &Bound<'py, PyAny>,
        classList: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let py = slf.py();
        let listed: Vec<Py<PyAny>> = slf
            .borrow()
            .sorted()
            .iter()
            .map(|held| held.object.clone_ref(py))
            .collect();
        let Some(at) = listed.iter().position(|held| held.bind(py).is(element)) else {
            return Err(StreamException::new_err("the object is not in this stream"));
        };
        for object in listed.into_iter().skip(at + 1) {
            let object = object.into_bound(py);
            match classList.filter(|list| !list.is_none()) {
                Some(list) if !is_of_any(&object, list)? => continue,
                _ => return Ok(Some(object)),
            }
        }
        Ok(None)
    }

    /// The meters this stream holds, each where it stands, the stream's
    /// own objects: music21's `getTimeSignatures`.
    ///
    /// With `returnDefault`, a stream stating none answers a four-four of
    /// its own, as music21's does. There is no context to search outside
    /// the stream here, so `searchContext` reaches no further than it.
    #[pyo3(signature = (*, searchContext = true, returnDefault = true, recurse = true, sortByCreationTime = true))]
    fn getTimeSignatures<'py>(
        slf: &Bound<'py, Self>,
        searchContext: bool,
        returnDefault: bool,
        recurse: bool,
        sortByCreationTime: bool,
    ) -> PyResult<Bound<'py, Stream>> {
        let _ = (searchContext, sortByCreationTime);
        let py = slf.py();
        let read = if recurse {
            Self::flatten(slf)?
        } else {
            slf.clone()
        };
        let named = pyo3::types::PyString::new(py, "TimeSignature");
        let found = Self::picked(&read, Some(StreamKind::Stream), |object| {
            is_of_class(object, &named)
        })?;
        if returnDefault && found.borrow().held.is_empty() {
            let made = py
                .get_type::<crate::meter::TimeSignature>()
                .call1(("4/4",))?;
            found.borrow_mut().put(0.0, &made);
        }
        Ok(found)
    }

    /// The instruments this stream holds, the stream's own objects:
    /// music21's `getInstruments`.
    ///
    /// With `returnDefault`, a stream naming no instrument answers a bare
    /// `Instrument` of its own, as music21's does. Nothing here holds the
    /// streams a stream sits in, so `searchActiveSite` reaches no further
    /// than this one.
    #[pyo3(signature = (*, searchActiveSite = true, returnDefault = true, recurse = true))]
    fn getInstruments<'py>(
        slf: &Bound<'py, Self>,
        searchActiveSite: bool,
        returnDefault: bool,
        recurse: bool,
    ) -> PyResult<Bound<'py, Stream>> {
        let _ = searchActiveSite;
        let py = slf.py();
        let read = if recurse {
            Self::flatten(slf)?
        } else {
            slf.clone()
        };
        let named = pyo3::types::PyString::new(py, "Instrument");
        let found = Self::picked(&read, Some(StreamKind::Stream), |object| {
            is_of_class(object, &named)
        })?;
        if returnDefault && found.borrow().held.is_empty() {
            let made = py.get_type::<crate::instrument::Instrument>().call0()?;
            // music21 gives the one it makes up the default part name,
            // which is the empty string.
            made.setattr("partName", "")?;
            found.borrow_mut().put(0.0, &made);
        }
        Ok(found)
    }

    /// The first instrument this stream holds, or `None`: music21's
    /// `getInstrument`.
    #[pyo3(signature = (*, searchActiveSite = true, returnDefault = true, recurse = false))]
    fn getInstrument<'py>(
        slf: &Bound<'py, Self>,
        searchActiveSite: bool,
        returnDefault: bool,
        recurse: bool,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let found = Self::getInstruments(slf, searchActiveSite, returnDefault, recurse)?;
        let py = slf.py();
        Ok(found
            .borrow()
            .sorted()
            .first()
            .map(|held| held.object.bind(py).clone()))
    }

    /// The clef standing at the start of this stream, or `None`: music21's
    /// `clef`, which a measure's own is.
    #[getter]
    fn get_clef<'py>(slf: &Bound<'py, Self>) -> PyResult<Option<Bound<'py, PyAny>>> {
        Self::opening(slf, "Clef")
    }

    /// Writes the clef the stream opens with, taking out the one it had, as
    /// music21's setter does; `None` leaves it with none.
    #[setter]
    fn set_clef(slf: &Bound<'_, Self>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        Self::set_opening(slf, "Clef", value)
    }

    /// The key signature standing at the start of this stream, or `None`:
    /// music21's `keySignature`.
    #[getter]
    fn get_keySignature<'py>(slf: &Bound<'py, Self>) -> PyResult<Option<Bound<'py, PyAny>>> {
        Self::opening(slf, "KeySignature")
    }

    /// Writes the key signature the stream opens with.
    #[setter]
    fn set_keySignature(slf: &Bound<'_, Self>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        Self::set_opening(slf, "KeySignature", value)
    }

    /// The meter standing at the start of this stream, or `None`:
    /// music21's `timeSignature`.
    #[getter]
    fn get_timeSignature<'py>(slf: &Bound<'py, Self>) -> PyResult<Option<Bound<'py, PyAny>>> {
        Self::opening(slf, "TimeSignature")
    }

    /// Writes the meter the stream opens with.
    #[setter]
    fn set_timeSignature(slf: &Bound<'_, Self>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        Self::set_opening(slf, "TimeSignature", value)
    }

    /// The first element the stream holds, or nothing where it holds
    /// none: music21's `first`, which its own code asks of what
    /// `getElementsByClass` hands back.
    fn first<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        Ok(self.elements(py)?.into_iter().next())
    }

    /// The last element the stream holds, or nothing where it holds none:
    /// music21's `last`.
    fn last<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        Ok(self.elements(py)?.into_iter().next_back())
    }

    /// Takes the object at an index out and hands it back, the last where
    /// no index is given: music21's `pop`, whose indices are those of
    /// `elements` and so in the order the stream is listed in.
    #[pyo3(signature = (index = None))]
    fn pop<'py>(slf: &Bound<'py, Self>, index: Option<isize>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let listed: Vec<Py<PyAny>> = slf
            .borrow()
            .sorted()
            .iter()
            .map(|held| held.object.clone_ref(py))
            .collect();
        let count = listed.len();
        let at = match index {
            None => count.checked_sub(1),
            // Python counts backwards from the end for a negative index.
            Some(index) if index < 0 => count.checked_sub(index.unsigned_abs()),
            Some(index) => Some(index.unsigned_abs()).filter(|at| *at < count),
        }
        .ok_or_else(|| pyo3::exceptions::PyIndexError::new_err("stream index out of range"))?;
        let taken = listed[at].bind(py).clone();
        slf.borrow_mut().remove(&taken)?;
        Ok(taken)
    }

    /// Takes out everything answering to one of these classes: music21's
    /// `removeByClass`.
    fn removeByClass(slf: &Bound<'_, Self>, classFilterList: &Bound<'_, PyAny>) -> PyResult<()> {
        Self::remove_where(slf, classFilterList, true)
    }

    /// Takes out everything answering to none of these classes: music21's
    /// `removeByNotOfClass`.
    fn removeByNotOfClass(
        slf: &Bound<'_, Self>,
        classFilterList: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        Self::remove_where(slf, classFilterList, false)
    }

    /// Every spanner in this stream and in the streams it holds:
    /// music21's `spannerBundle`, which gathers them from the whole tree.
    ///
    /// A list here, where music21 answers its own `SpannerBundle`;
    /// iterating it and counting it are what its own code does with one.
    /// This is the way to ask for a slur whichever stream it was put on:
    /// the crate's readers gather a part's spanners onto the score, where
    /// music21's reader leaves them on the part, so `spanners` answers
    /// differently of the two while this answers the same.
    #[getter]
    fn spannerBundle<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let out = PyList::empty(py);
        let spanner = pyo3::types::PyString::new(py, "Spanner");
        let held: Vec<Py<PyAny>> = slf
            .borrow()
            .sorted()
            .iter()
            .map(|one| one.object.clone_ref(py))
            .collect();
        for object in held {
            let object = object.into_bound(py);
            if is_of_class(&object, &spanner)? {
                out.append(&object)?;
            } else if let Ok(inner) = object.cast::<Stream>() {
                for deeper in Self::spannerBundle(inner)?.try_iter()? {
                    out.append(deeper?)?;
                }
            }
        }
        let held: Vec<Py<PyAny>> = out.iter().map(|one| one.unbind()).collect();
        Ok(Bound::new(py, crate::spanner::SpannerBundle::of(held))?.into_any())
    }

    /// The number this measure is written with, its suffix and all:
    /// music21's `measureNumberWithSuffix`, so a measure numbered `14a`
    /// says `14a`.
    fn measureNumberWithSuffix(slf: &Bound<'_, Self>) -> PyResult<String> {
        Ok(crate_stream(slf.as_any())?.number_with_suffix())
    }

    /// How long this measure's bar is, whatever it holds: music21's
    /// `barDuration`, the length of the meter standing in the measure or,
    /// failing that, of the one `bestTimeSignature` makes of its notes.
    #[getter]
    fn barDuration<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let stream = crate_stream(slf.as_any())?;
        let meter = match stream.time_signature_at(0.0) {
            Some(meter) => meter.clone(),
            None => music21_rs_crate::meter::best_time_signature(&stream)
                .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?,
        };
        let made = crate::duration::Duration::object(py, meter.bar_duration())?;
        Ok(made.into_bound(py))
    }

    /// How much of its bar this measure fills, as a proportion: music21's
    /// `barDurationProportion`, so a pickup of one quarter in four-four
    /// answers a quarter.
    ///
    /// A float, where music21 answers a `Fraction` for a proportion no
    /// binary fraction says exactly -- a third of a three-four bar. Every
    /// length and offset in this package is a float, as the crate keeps
    /// them.
    #[pyo3(signature = (barDuration = None))]
    fn barDurationProportion(
        slf: &Bound<'_, Self>,
        barDuration: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<f64> {
        // music21 takes a bar length to measure against; the meter the
        // measure states is what it uses where none is given, which is
        // what the crate reads.
        if barDuration.is_some_and(|given| !given.is_none()) {
            return Err(StreamException::new_err(
                "a barDuration of its own is not supported here: the bar is the measure's own meter",
            ));
        }
        crate_stream(slf.as_any())?
            .bar_duration_proportion(None)
            .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))
    }

    /// The meter that best fits what this measure holds: music21's
    /// `bestTimeSignature`.
    fn bestTimeSignature<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let stream = crate_stream(slf.as_any())?;
        let meter = music21_rs_crate::meter::best_time_signature(&stream)
            .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
        py.get_type::<crate::meter::TimeSignature>()
            .call1((meter.ratio_string(),))
    }

    /// A copy of this measure padded on the left by what it lacks of its
    /// bar, as a pickup: music21's `padAsAnacrusis`.
    ///
    /// music21 changes the measure it is given; the answer here is a new
    /// one, as every other edit in this package is. `useGaps` is music21's
    /// default and the only reading here.
    #[pyo3(signature = (*, useGaps = true, useInitialRests = false))]
    fn padAsAnacrusis<'py>(
        slf: &Bound<'py, Self>,
        useGaps: bool,
        useInitialRests: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        if !useGaps {
            return Err(StreamException::new_err(
                "useGaps=False is not supported here: the bar is measured as music21 measures it by default",
            ));
        }
        remade(slf, |stream| stream.pad_as_anacrusis(None, useInitialRests))
    }

    /// The spanners this stream holds, the stream's own objects:
    /// music21's `spanners`.
    ///
    /// A spanner is an element of the stream whose notes it joins, as
    /// music21 keeps one, standing at nought whatever the notes' offsets.
    #[getter]
    fn spanners<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, Stream>> {
        let py = slf.py();
        let spanner = pyo3::types::PyString::new(py, "Spanner");
        Self::picked(slf, Some(StreamKind::Stream), |object| {
            is_of_class(object, &spanner)
        })
    }

    /// The voices this stream holds directly, the stream's own objects:
    /// music21's `voices`.
    #[getter]
    fn voices<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, Stream>> {
        let py = slf.py();
        let voice = pyo3::types::PyString::new(py, "Voice");
        Self::picked(slf, Some(StreamKind::Stream), |object| {
            is_of_class(object, &voice)
        })
    }

    /// Everything held, in order.
    #[getter]
    fn elements<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, self.sorted().iter().map(|held| held.object.bind(py)))
    }

    /// Where an object stands in this stream.
    fn elementOffset<'py>(&self, element: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        // Through `opFrac`, as music21's: a triplet stands at a third.
        crate::duration::op_frac(element.py(), self.offset_of(element)?)
    }

    /// Moves an object this stream holds to another offset.
    fn setElementOffset(&mut self, element: &Bound<'_, PyAny>, offset: f64) -> PyResult<()> {
        let held = self
            .held
            .iter_mut()
            .find(|held| held.object.bind(element.py()).is(element))
            .ok_or_else(|| StreamException::new_err("this object is not stored in this stream"))?;
        held.offset = offset;
        Ok(())
    }

    /// The analysis a method name asks for: music21's `analyze`.
    ///
    /// `ambitus`, `span` and `range` give the interval from the lowest
    /// pitch sounding to the highest; `key` and the names of music21's
    /// five key weightings give the key the stream is most likely in. The
    /// name is read as music21 reads it, loosest last, so `kostka` finds
    /// Temperley's profile and a bare `key` finds Aarden's.
    ///
    /// A stream with no pitches has no ambitus and answers nothing, as
    /// music21 answers `None`; a key analysis of one raises, as music21
    /// raises.
    #[pyo3(signature = (method, **_keywords))]
    fn analyze<'py>(
        slf: &Bound<'py, Self>,
        method: &str,
        _keywords: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        use music21_rs_crate::stream::Analysis;

        let py = slf.py();
        let stream = crate_stream(slf.as_any())?;
        let found = stream.analyze(method).map_err(|error| {
            // music21 raises its discrete-analysis class for a method
            // nobody answers to and for a stream with nothing to analyse.
            crate::discrete::DiscreteAnalysisException::new_err(crate::pitch::message(&error))
        })?;
        match found {
            None => Ok(None),
            Some(Analysis::Key(key)) => {
                Ok(Some(crate::key::Key::object(py, key)?.bind(py).clone()))
            }
            Some(Analysis::Ambitus(span)) => Ok(Some(
                Bound::new(py, crate::interval::Interval::wrap(span))?.into_any(),
            )),
        }
    }

    /// The scores this opus holds: music21's `Opus.scores`.
    #[getter]
    fn scores<'py>(slf: &Bound<'py, Self>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let py = slf.py();
        let mut found = Vec::new();
        for held in &slf.borrow().held {
            let object = held.object.bind(py);
            if kind_of(object).is_ok_and(|kind| kind == StreamKind::Score) {
                found.push(object.clone());
            }
        }
        Ok(found)
    }

    /// Each score's number, as its metadata gives it: music21's
    /// `Opus.getNumbers`.
    fn getNumbers(slf: &Bound<'_, Self>) -> PyResult<Vec<Option<String>>> {
        Self::scores(slf)?
            .into_iter()
            .map(|score| match score.getattr("metadata") {
                Ok(metadata) if !metadata.is_none() => metadata.getattr("number")?.extract(),
                _ => Ok(None),
            })
            .collect()
    }

    /// The first score whose number a query matches: music21's
    /// `Opus.getScoreByNumber`.
    fn getScoreByNumber<'py>(
        slf: &Bound<'py, Self>,
        opusMatch: &Bound<'_, PyAny>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        Self::score_by(slf, opusMatch, "number")
    }

    /// The first score whose title a query matches: music21's
    /// `Opus.getScoreByTitle`.
    fn getScoreByTitle<'py>(
        slf: &Bound<'py, Self>,
        titleMatch: &Bound<'_, PyAny>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        Self::score_by(slf, titleMatch, "title")
    }

    /// One score of the opus's scores laid out as its parts: music21's
    /// `Opus.mergeScores`.
    fn mergeScores<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let stream = crate_stream(slf.as_any())?;
        let merged = stream
            .merge_scores()
            .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
        from_crate(slf.py(), &merged)
    }

    /// The score's metadata: music21's `metadata`, which is whatever
    /// `Metadata` the stream holds, music21 keeping it among the elements
    /// rather than beside them.
    #[getter]
    fn get_metadata(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.held
            .iter()
            .map(|held| held.object.bind(py))
            .find(|object| object.is_instance_of::<crate::metadata::Metadata>())
            .map(|found| found.clone().unbind())
    }

    /// Writes the metadata, which goes at the start as music21 inserts it,
    /// and takes the place of any the stream already held.
    #[setter]
    fn set_metadata(&mut self, py: Python<'_>, value: Option<&Bound<'_, PyAny>>) {
        self.held.retain(|held| {
            !held
                .object
                .bind(py)
                .is_instance_of::<crate::metadata::Metadata>()
        });
        if let Some(value) = value.filter(|value| !value.is_none()) {
            self.put(0.0, value);
        }
    }

    /// The offset just past the last thing held.
    #[getter]
    fn highestTime<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        crate::duration::op_frac(slf.py(), Self::highest_time(slf)?)
    }

    /// The stream as one line of chords, as music21's `chordify` makes it:
    /// laid out as the first part is, with a chord of every pitch sounding
    /// wherever any part starts or stops a note, a held note tied across.
    ///
    /// The chords are new objects, whatever `copyPitches` says, and
    /// `addPartIdAsGroup` is refused, as this package keeps no groups.
    #[pyo3(signature = (*, addTies = true, addPartIdAsGroup = false, removeRedundantPitches = true, toSoundingPitch = true, copyPitches = true))]
    fn chordify<'py>(
        slf: &Bound<'py, Self>,
        addTies: bool,
        addPartIdAsGroup: bool,
        removeRedundantPitches: bool,
        toSoundingPitch: bool,
        copyPitches: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let _ = copyPitches;
        if addPartIdAsGroup {
            return Err(StreamException::new_err(
                "addPartIdAsGroup needs groups, which this package does not keep",
            ));
        }
        let stream = crate_stream(slf.as_any())?;
        let options = music21_rs_crate::stream::ChordifyOptions {
            add_ties: addTies,
            remove_redundant_pitches: removeRedundantPitches,
            to_sounding_pitch: toSoundingPitch,
        };
        let made = stream
            .chordify_with(&options)
            .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
        from_crate(slf.py(), &made)
    }

    /// The notes and chords that follow one another in the stream, the very
    /// objects it holds, with `None` wherever the line breaks: music21's
    /// `findConsecutiveNotes`, with its keywords.
    #[pyo3(signature = (*, skipRests = false, skipChords = false, skipUnisons = false, skipOctaves = false, skipGaps = false, getOverlaps = false, noNone = false, **_keywords))]
    #[allow(clippy::too_many_arguments)]
    fn findConsecutiveNotes<'py>(
        slf: &Bound<'py, Self>,
        skipRests: bool,
        skipChords: bool,
        skipUnisons: bool,
        skipOctaves: bool,
        skipGaps: bool,
        getOverlaps: bool,
        noNone: bool,
        _keywords: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyList>> {
        let options = music21_rs_crate::stream::ConsecutiveOptions {
            skip_rests: skipRests,
            skip_chords: skipChords,
            skip_unisons: skipUnisons,
            skip_octaves: skipOctaves,
            skip_gaps: skipGaps,
            get_overlaps: getOverlaps,
            no_none: noNone,
        };
        let read = read_values(slf.as_any())?;
        let found = read
            .stream
            .find_consecutive_notes(&options)
            .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
        let py = slf.py();
        let list = PyList::empty(py);
        for position in found {
            match position {
                Some(position) => list.append(read.leaves[position].bind(py))?,
                None => list.append(py.None())?,
            }
        }
        Ok(list)
    }

    /// The intervals between the notes `findConsecutiveNotes` finds next to
    /// each other, in a stream of the same class, each standing where the
    /// note it leaves ends: music21's `melodicIntervals`. An interval to or
    /// from a chord is taken from the chord's first note, and its
    /// `noteStart` and `noteEnd` are the notes themselves.
    #[pyo3(signature = (**skipKeywords))]
    fn melodicIntervals<'py>(
        slf: &Bound<'py, Self>,
        skipKeywords: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let flag = |name: &str| -> PyResult<bool> {
            match skipKeywords
                .map(|keywords| keywords.get_item(name))
                .transpose()?
            {
                Some(Some(value)) => value.is_truthy(),
                _ => Ok(false),
            }
        };
        let options = music21_rs_crate::stream::ConsecutiveOptions {
            skip_rests: flag("skipRests")?,
            skip_chords: flag("skipChords")?,
            skip_unisons: flag("skipUnisons")?,
            skip_octaves: flag("skipOctaves")?,
            skip_gaps: flag("skipGaps")?,
            get_overlaps: flag("getOverlaps")?,
            no_none: flag("noNone")?,
        };
        let read = read_values(slf.as_any())?;
        let steps = read
            .stream
            .melodic_intervals(&options)
            .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
        let made = slf.get_type().call0()?;
        let interval = py.get_type::<crate::interval::Interval>();
        let first_note = |object: &Bound<'py, PyAny>| -> PyResult<Bound<'py, PyAny>> {
            if is_of_class(object, &pyo3::types::PyString::new(py, "Chord"))? {
                object.getattr("notes")?.get_item(0)
            } else {
                Ok(object.clone())
            }
        };
        for step in steps {
            let start = first_note(read.leaves[step.start()].bind(py))?;
            let end = first_note(read.leaves[step.end()].bind(py))?;
            let between = interval.call1((start, end))?;
            made.call_method1("insert", (step.offset(), between))?;
        }
        Ok(made)
    }

    /// A copy with each note and rest cut into pieces of these lengths in
    /// turn: music21's `sliceByQuarterLengths`. Cutting only `target`,
    /// untied, or in place is refused: the copy is a new stream.
    #[pyo3(signature = (quarterLengthList, *, target = None, addTies = true, inPlace = false))]
    fn sliceByQuarterLengths<'py>(
        slf: &Bound<'py, Self>,
        quarterLengthList: &Bound<'py, PyAny>,
        target: Option<&Bound<'py, PyAny>>,
        addTies: bool,
        inPlace: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        refuse_unported(target.is_some(), !addTies, inPlace)?;
        let lengths: Vec<f64> = match quarterLengthList.extract::<f64>() {
            Ok(length) => vec![length],
            Err(_) => quarterLengthList.extract()?,
        };
        edited(slf, |stream| stream.slice_by_quarter_lengths(&lengths))
    }

    /// A copy with each note and rest cut into pieces of the greatest
    /// length that goes into them all, measure by measure: music21's
    /// `sliceByGreatestDivisor`.
    #[pyo3(signature = (*, addTies = true, inPlace = false))]
    fn sliceByGreatestDivisor<'py>(
        slf: &Bound<'py, Self>,
        addTies: bool,
        inPlace: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        refuse_unported(false, !addTies, inPlace)?;
        edited(slf, RsStream::slice_by_greatest_divisor)
    }

    /// A copy with what sounds across each offset cut there: music21's
    /// `sliceAtOffsets`.
    #[pyo3(signature = (offsetList, target = None, addTies = true, inPlace = false, displayTiedAccidentals = false))]
    fn sliceAtOffsets<'py>(
        slf: &Bound<'py, Self>,
        offsetList: Vec<f64>,
        target: Option<&Bound<'py, PyAny>>,
        addTies: bool,
        inPlace: bool,
        displayTiedAccidentals: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let _ = displayTiedAccidentals;
        refuse_unported(target.is_some(), !addTies, inPlace)?;
        edited(slf, |stream| stream.slice_at_offsets(&offsetList))
    }

    /// A copy with each note and rest cut at the beats of its time
    /// signature: music21's `sliceByBeat`.
    #[pyo3(signature = (target = None, addTies = true, inPlace = false, displayTiedAccidentals = false))]
    fn sliceByBeat<'py>(
        slf: &Bound<'py, Self>,
        target: Option<&Bound<'py, PyAny>>,
        addTies: bool,
        inPlace: bool,
        displayTiedAccidentals: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let _ = displayTiedAccidentals;
        refuse_unported(target.is_some(), !addTies, inPlace)?;
        edited(slf, RsStream::slice_by_beat)
    }

    /// A copy with its offsets scaled, held still at `anchorZero`
    /// (`'lowest'`, `'highest'` or `None` for nought) and nested streams at
    /// `anchorZeroRecurse`: music21's `scaleOffsets`.
    #[pyo3(signature = (amountToScale, *, anchorZero = Some("lowest".to_string()), anchorZeroRecurse = None, inPlace = false))]
    fn scaleOffsets<'py>(
        slf: &Bound<'py, Self>,
        amountToScale: f64,
        anchorZero: Option<String>,
        anchorZeroRecurse: Option<String>,
        inPlace: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        refuse_unported(false, false, inPlace)?;
        let anchor = anchor_of(anchorZero.as_deref())?;
        let nested = anchor_of(anchorZeroRecurse.as_deref())?;
        edited(slf, |stream| {
            stream.scale_offsets(amountToScale, anchor, nested)
        })
    }

    /// A copy with every length scaled: music21's `scaleDurations`.
    #[pyo3(signature = (amountToScale, *, inPlace = false))]
    fn scaleDurations<'py>(
        slf: &Bound<'py, Self>,
        amountToScale: f64,
        inPlace: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        refuse_unported(false, false, inPlace)?;
        edited(slf, |stream| stream.scale_durations(amountToScale))
    }

    /// A copy with its offsets and lengths scaled: music21's
    /// `augmentOrDiminish`.
    #[pyo3(signature = (amountToScale, *, inPlace = false))]
    fn augmentOrDiminish<'py>(
        slf: &Bound<'py, Self>,
        amountToScale: f64,
        inPlace: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        refuse_unported(false, false, inPlace)?;
        edited(slf, |stream| stream.augment_or_diminish(amountToScale))
    }

    /// A copy with each note turned upside down about `inversionNote`, C4
    /// where none is given, by step in the key: music21's `invertDiatonic`.
    #[pyo3(signature = (inversionNote = None, *, inPlace = false))]
    fn invertDiatonic<'py>(
        slf: &Bound<'py, Self>,
        inversionNote: Option<&Bound<'py, PyAny>>,
        inPlace: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        refuse_unported(false, false, inPlace)?;
        let pitch = match inversionNote {
            Some(note) => {
                let pitch = if note.hasattr("pitch")? {
                    note.getattr("pitch")?
                } else {
                    note.clone()
                };
                let name: String = pitch.getattr("nameWithOctave")?.extract()?;
                Some(music21_rs_crate::Pitch::from_name(&name).map_err(crate::pitch::pitch_error)?)
            }
            None => None,
        };
        edited(slf, |stream| stream.invert_diatonic(pitch.as_ref()))
    }

    /// A score with a part for each voice: music21's `voicesToParts`.
    #[pyo3(signature = (*, separateById = false))]
    fn voicesToParts<'py>(
        slf: &Bound<'py, Self>,
        separateById: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        edited(slf, |stream| stream.voices_to_parts(separateById))
    }

    /// A score with a part for each voice: music21's `explode`.
    fn explode<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        edited(slf, RsStream::explode)
    }

    /// A score whose parts hold this score's parts as voices, grouped by
    /// `voiceAllocation`, a count to a part or a list of groups of part
    /// indices: music21's `partsToVoices`.
    #[pyo3(signature = (voiceAllocation = None, permitOneVoicePerPart = false, setStems = true))]
    fn partsToVoices<'py>(
        slf: &Bound<'py, Self>,
        voiceAllocation: Option<&Bound<'py, PyAny>>,
        permitOneVoicePerPart: bool,
        setStems: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        use music21_rs_crate::stream::VoiceAllocation;
        let allocation = match voiceAllocation {
            None => VoiceAllocation::default(),
            Some(value) => match value.extract::<usize>() {
                Ok(each) => VoiceAllocation::Each(each),
                Err(_) => {
                    let mut groups = Vec::new();
                    for group in value.try_iter()? {
                        let group = group?;
                        groups.push(match group.extract::<usize>() {
                            Ok(index) => vec![index],
                            Err(_) => group.extract::<Vec<usize>>()?,
                        });
                    }
                    VoiceAllocation::Groups(groups)
                }
            },
        };
        edited(slf, |stream| {
            stream.parts_to_voices(&allocation, permitOneVoicePerPart, setStems)
        })
    }

    /// The parts joined two to a part as voices: music21's `implode`.
    fn implode<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        edited(slf, RsStream::implode)
    }

    /// Where the last element this stream holds itself starts: music21's
    /// `highestOffset`.
    #[getter]
    fn highestOffset<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        crate::duration::op_frac(slf.py(), crate_stream(slf.as_any())?.highest_offset())
    }

    /// Where the first element this stream holds itself starts: music21's
    /// `lowestOffset`.
    #[getter]
    fn lowestOffset<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        crate::duration::op_frac(slf.py(), crate_stream(slf.as_any())?.lowest_offset())
    }

    /// Whether the stream holds a measure itself: music21's `hasMeasures`.
    fn hasMeasures(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Ok(crate_stream(slf.as_any())?.has_measures())
    }

    /// Whether the stream holds a voice itself: music21's `hasVoices`.
    fn hasVoices(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Ok(crate_stream(slf.as_any())?.has_voices())
    }

    /// Whether nothing is missing between the stream's start and its end:
    /// music21's `isGapless`.
    #[getter]
    fn isGapless(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Ok(crate_stream(slf.as_any())?.is_gapless())
    }

    /// Whether no two notes overlap: music21's `isSequence`.
    fn isSequence(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Ok(crate_stream(slf.as_any())?.is_sequence())
    }

    /// Whether the stream is notation as music21 writes it out: music21's
    /// `isWellFormedNotation`.
    fn isWellFormedNotation(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Ok(crate_stream(slf.as_any())?.is_well_formed_notation())
    }

    /// How long the stream lasts, which is its highest time.
    #[getter]
    fn quarterLength<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        if let Some(set) = &slf.borrow().unlinked_duration {
            let length: f64 = set.bind(py).getattr("quarterLength")?.extract()?;
            return crate::duration::op_frac(py, length);
        }
        crate::duration::op_frac(py, Self::highest_time(slf)?)
    }

    /// The length of the whole stream: music21's `duration`, the highest
    /// time unless a duration has been set apart from it.
    #[getter]
    fn get_duration(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        if let Some(set) = &slf.borrow().unlinked_duration {
            return Ok(set.clone_ref(py));
        }
        let length = music21_rs_crate::Duration::new(Self::highest_time(slf)?)
            .map_err(|error| StreamException::new_err(error.to_string()))?;
        crate::duration::Duration::object(py, length)
    }

    /// Sets a duration apart from what the stream holds, or with `None`
    /// goes back to the highest time. Anything but a duration is refused,
    /// as music21 refuses it.
    #[setter]
    fn set_duration(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let Some(value) = value.filter(|given| !given.is_none()) else {
            self.unlinked_duration = None;
            return Ok(());
        };
        if !is_of_class(value, &pyo3::types::PyString::new(value.py(), "Duration"))? {
            let shown = value.str()?.to_string();
            return Err(StreamException::new_err(format!(
                "this must be a Duration object, not {shown}"
            )));
        }
        self.unlinked_duration = Some(value.clone().unbind());
        Ok(())
    }

    /// music21's `sort`. A stream here answers in order whenever it is
    /// read, so there is nothing to do; `force` is accepted for that.
    #[pyo3(signature = (force = false))]
    fn sort(&self, force: bool) {
        let _ = force;
    }

    fn __len__(&self) -> usize {
        self.held.len()
    }

    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        Ok(self.elements(py)?.try_iter()?.into_any())
    }

    fn __contains__(&self, element: &Bound<'_, PyAny>) -> bool {
        self.held
            .iter()
            .any(|held| held.object.bind(element.py()).is(element))
    }

    fn __getitem__<'py>(
        &self,
        py: Python<'py>,
        key: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let elements = self.elements(py)?;
        if key.is_instance_of::<PySlice>() {
            return elements.as_any().get_item(key);
        }
        let index: isize = key
            .extract()
            .map_err(|_| PyTypeError::new_err("a stream is indexed by a number or a slice"))?;
        let length = elements.len() as isize;
        let at = if index < 0 { index + length } else { index };
        if at < 0 || at >= length {
            return Err(PyIndexError::new_err(format!(
                "attempting to access index {index} while elements is of size {length}"
            )));
        }
        elements.get_item(at as usize)
    }

    /// One timeline with the nesting dissolved, holding the same objects at
    /// their offsets from this stream's start.
    fn flatten<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, Stream>> {
        let py = slf.py();
        let mut leaves = Vec::new();
        Self::flatten_into(slf, 0.0, &mut leaves)?;
        let own_right = slf
            .borrow()
            .right_barline
            .as_ref()
            .map(|right| right.clone_ref(py));
        let mut out = Self::empty(slf.borrow().kind);
        for (offset, object) in leaves {
            // A nested measure's right barline is an element like any other
            // once flattened; this stream's own stays last, as music21
            // keeps it at the end.
            let own = own_right.as_ref().is_some_and(|right| right.is(&object));
            match own {
                true => out.put_with_priority(offset, object.bind(py), i64::MAX),
                false => out.put(offset, object.bind(py)),
            }
        }
        Bound::new(py, out)
    }

    /// Everything held at every depth, streams included, in the order the
    /// walk meets them: music21's `recurse`.
    ///
    /// The answer is a [`Walked`], as music21's is an iterator rather than
    /// a stream, because its own code filters what it gets back --
    /// `analysis.discrete` asks a window for `recurse().notes` -- and
    /// because the order is the walk's rather than the sort's: a stream
    /// comes before what it holds, and one part's notes before the next
    /// part's, where a stream would put every offset nought together.
    fn recurse<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, Walked>> {
        let py = slf.py();
        let mut found = Vec::new();
        Self::recurse_into(slf, 0.0, &mut found)?;
        Bound::new(py, Walked::of(found, slf.borrow().kind))
    }

    /// The notes and chords this stream holds directly.
    #[getter]
    fn notes<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, Stream>> {
        Self::picked(slf, Some(StreamKind::Stream), |object| {
            Ok(["isNote", "isChord"].iter().any(|flag| {
                object
                    .getattr(*flag)
                    .and_then(|flag| flag.is_truthy())
                    .unwrap_or(false)
            }))
        })
    }

    /// The notes, chords and rests this stream holds directly.
    #[getter]
    fn notesAndRests<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, Stream>> {
        Self::picked(slf, Some(StreamKind::Stream), |object| {
            for flag in ["isNote", "isChord", "isRest"] {
                if object
                    .getattr(flag)
                    .and_then(|flag| flag.is_truthy())
                    .unwrap_or(false)
                {
                    return Ok(true);
                }
            }
            Ok(false)
        })
    }

    /// What this stream holds directly of the classes given, by class or by
    /// name.
    fn getElementsByClass<'py>(
        slf: &Bound<'py, Self>,
        classFilterList: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, Stream>> {
        Self::picked(slf, Some(StreamKind::Stream), |object| {
            is_of_any(object, classFilterList)
        })
    }

    /// The elements answering to none of these classes: music21's
    /// `getElementsNotOfClass`.
    #[pyo3(signature = (classFilterList))]
    fn getElementsNotOfClass<'py>(
        slf: &Bound<'py, Self>,
        classFilterList: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, Stream>> {
        Self::picked(slf, Some(StreamKind::Stream), |object| {
            is_of_any(object, classFilterList).map(|found| !found)
        })
    }

    /// Whether any element answers to one of these classes: music21's
    /// `hasElementOfClass`.
    ///
    /// * Deprecated in v11: use `bool(s.getElementsByClass(className))`.
    #[pyo3(signature = (className, forceFlat = false))]
    fn hasElementOfClass(
        slf: &Bound<'_, Self>,
        className: &Bound<'_, PyAny>,
        forceFlat: bool,
    ) -> PyResult<bool> {
        let _ = forceFlat;
        let py = slf.py();
        crate::warn_deprecated_once(
            py,
            &HAS_ELEMENT_OF_CLASS_WARNED,
            HAS_ELEMENT_OF_CLASS_DEPRECATED,
        )?;
        let objects: Vec<Py<PyAny>> = slf
            .borrow()
            .held
            .iter()
            .map(|held| held.object.clone_ref(py))
            .collect();
        for object in objects {
            if is_of_any(&object.into_bound(py), className)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The elements standing in a span of offsets: music21's
    /// `getElementsByOffset`, with its four keywords.
    ///
    /// Under music21's defaults an element is found only where it
    /// *begins*, so a half note at nought is not found at the offset
    /// halfway through it; `mustBeginInSpan=False` reaches back to what
    /// sounds at a point.
    #[pyo3(signature = (
        offsetStart,
        offsetEnd = None,
        *,
        includeEndBoundary = true,
        mustFinishInSpan = false,
        mustBeginInSpan = true,
        includeElementsThatEndAtStart = true,
        classList = None
    ))]
    #[allow(clippy::too_many_arguments)]
    fn getElementsByOffset<'py>(
        slf: &Bound<'py, Self>,
        offsetStart: f64,
        offsetEnd: Option<f64>,
        includeEndBoundary: bool,
        mustFinishInSpan: bool,
        mustBeginInSpan: bool,
        includeElementsThatEndAtStart: bool,
        classList: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, Stream>> {
        let span = music21_rs_crate::stream::OffsetSpan {
            start: offsetStart,
            end: offsetEnd,
            include_end_boundary: includeEndBoundary,
            must_finish_in_span: mustFinishInSpan,
            must_begin_in_span: mustBeginInSpan,
            include_elements_that_end_at_start: includeElementsThatEndAtStart,
        };
        // The offsets live beside the objects, so the span is asked of
        // each rather than read into the crate.
        let placed: Vec<(f64, Py<PyAny>)> = slf
            .borrow()
            .sorted()
            .iter()
            .map(|held| (held.offset, held.object.clone_ref(slf.py())))
            .collect();
        let py = slf.py();
        let mut out = Self::empty(slf.borrow().kind);
        for (offset, object) in placed {
            let object = object.into_bound(py);
            if let Some(classList) = classList.filter(|list| !list.is_none())
                && !is_of_any(&object, classList)?
            {
                continue;
            }
            let length: f64 = object
                .getattr("quarterLength")
                .and_then(|value| value.extract())
                .unwrap_or(0.0);
            if span.takes_at(offset, length) {
                out.put(offset, &object);
            }
        }
        Bound::new(py, out)
    }

    /// Whether every pitch sounding is one of the twelve of equal
    /// temperament: music21's `isTwelveTone`.
    fn isTwelveTone(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Ok(crate_stream(slf.as_any())?.is_twelve_tone())
    }

    /// A copy with each element of a class stretched to where the next one
    /// starts, the last to the end of the stream: music21's
    /// `extendDuration`.
    ///
    /// Only what the crate gives a length to is stretched -- a note, a
    /// rest, a chord or a chord symbol. music21 would stretch anything,
    /// every object of its own carrying a duration, where the marks saying
    /// what is in force from a point are timeless here, so asking for one
    /// of those classes leaves the lengths as they stand.
    #[pyo3(signature = (objName, *, inPlace = false))]
    fn extendDuration<'py>(
        slf: &Bound<'py, Self>,
        objName: &Bound<'py, PyAny>,
        inPlace: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        refuse_unported(false, false, inPlace)?;
        let names = class_names(objName)?;
        let asked: Vec<&str> = names.iter().map(String::as_str).collect();
        edited(slf, |stream| stream.extend_duration(&asked))
    }

    /// Takes in everything another stream holds, each at the offset it
    /// stands at there: music21's `mergeElements`.
    ///
    /// With `classFilterList`, only the elements answering to one of those
    /// classes come. The objects themselves come, as music21's do, so a
    /// note is then held by both streams.
    #[pyo3(signature = (other, classFilterList = None))]
    fn mergeElements(
        slf: &Bound<'_, Self>,
        other: &Bound<'_, PyAny>,
        classFilterList: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let py = slf.py();
        let placed: Vec<(f64, Py<PyAny>)> = match other.cast::<Stream>() {
            Ok(stream) => stream
                .borrow()
                .held
                .iter()
                .map(|held| (held.offset, held.object.clone_ref(py)))
                .collect(),
            // music21's own stream, which says where it holds each thing.
            Err(_) => {
                let mut placed = Vec::new();
                for element in other.getattr("elements")?.walk()? {
                    let element = element?;
                    let offset: f64 = other
                        .call_method1("elementOffset", (&element,))?
                        .extract()?;
                    placed.push((offset, element.unbind()));
                }
                placed
            }
        };
        for (offset, object) in placed {
            let object = object.into_bound(py);
            if let Some(classes) = classFilterList.filter(|list| !list.is_none())
                && !is_of_any(&object, classes)?
            {
                continue;
            }
            slf.borrow_mut().put(offset, &object);
        }
        Ok(())
    }

    /// A stream like this one holding nothing: music21's `cloneEmpty`.
    ///
    /// It is of the same class and carries what music21's `mergeAttributes`
    /// copies, of what this package keeps: the id, whether the stream is at
    /// sounding pitch, a measure's number, suffix and padding, and a part's
    /// name and abbreviation. An attribute the stream was never given is
    /// not copied, as music21's own `hasattr` does not copy one.
    #[pyo3(signature = (derivationMethod = None))]
    fn cloneEmpty<'py>(
        slf: &Bound<'py, Self>,
        derivationMethod: Option<&str>,
    ) -> PyResult<Bound<'py, PyAny>> {
        // Derivations say where a stream came from, which this package
        // keeps nothing of.
        let _ = derivationMethod;
        let made = slf.get_type().call0()?;
        if let Some(id) = &slf.borrow().id {
            made.setattr("id", id.bind(slf.py()))?;
        }
        let kept: &[&str] = match slf.borrow().kind {
            StreamKind::Measure => &[
                "atSoundingPitch",
                "number",
                "numberSuffix",
                "paddingLeft",
                "paddingRight",
            ],
            StreamKind::Part | StreamKind::PartStaff => {
                &["atSoundingPitch", "partName", "partAbbreviation"]
            }
            _ => &["atSoundingPitch"],
        };
        for name in kept {
            if let Ok(value) = slf.as_any().getattr(*name) {
                made.setattr(*name, value)?;
            }
        }
        Ok(made)
    }

    /// The stretches of the stream each tempo holds, each a tuple of where
    /// it begins, where it ends and the tempo sounding through it:
    /// music21's `metronomeMarkBoundaries`.
    ///
    /// The stream is read flattened, so a tempo written on a part is found
    /// from the score, and the mark handed back is the stream's own object
    /// wherever it holds one. Where it states no tempo, or none from its
    /// start, a quarter at 120 holds, as it does upstream; so does the
    /// tempo a word or a metric modulation *means*, which music21 builds
    /// afresh too.
    fn metronomeMarkBoundaries<'py>(
        slf: &Bound<'py, Self>,
    ) -> PyResult<Vec<(f64, f64, Bound<'py, PyAny>)>> {
        let py = slf.py();
        let read = read_values(slf.as_any())?;
        let spans = read
            .stream
            .metronome_mark_positions()
            .map_err(|error| TempoException::new_err(crate::pitch::message(&error)))?;
        let mut out = Vec::new();
        for span in spans {
            let mark = match span.position {
                Some(position) => read.leaves[position].bind(py).clone(),
                None => crate::installed_new(
                    py,
                    "music21.tempo",
                    "MetronomeMark",
                    MetronomeMark::wrap(span.mark),
                )?
                .into_bound(py)
                .into_any(),
            };
            out.push((span.start, span.end, mark));
        }
        Ok(out)
    }

    /// A copy with every note, chord, chord symbol and key moved by an
    /// interval: music21's `transpose`, which takes an `Interval`, a name
    /// or a count of semitones.
    #[pyo3(signature = (value, *, inPlace = false, classFilterList = None))]
    fn transpose<'py>(
        slf: &Bound<'py, Self>,
        value: &Bound<'py, PyAny>,
        inPlace: bool,
        classFilterList: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        refuse_unported(
            classFilterList.is_some_and(|list| !list.is_none()),
            false,
            inPlace,
        )?;
        let interval = crate::interval::interval_from_any(value)?;
        edited(slf, |stream| stream.transpose(&interval))
    }

    /// A copy with each run of tied notes joined into one: music21's
    /// `stripTies`.
    #[pyo3(signature = (*, matchByPitch = true, inPlace = false))]
    fn stripTies<'py>(
        slf: &Bound<'py, Self>,
        matchByPitch: bool,
        inPlace: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        refuse_unported(false, false, inPlace)?;
        remade(slf, |stream| stream.strip_ties(matchByPitch))
    }

    /// A copy with every offset and length moved onto the nearest place a
    /// quarter divides into: music21's `quantize`. The divisors are the
    /// ways a quarter may be divided, `[4, 3]` -- sixteenths and triplets
    /// -- where none are given, as music21's are.
    #[pyo3(signature = (quarterLengthDivisors = None, *, processOffsets = true, processDurations = true, inPlace = false, recurse = true))]
    fn quantize<'py>(
        slf: &Bound<'py, Self>,
        quarterLengthDivisors: Option<Vec<u32>>,
        processOffsets: bool,
        processDurations: bool,
        inPlace: bool,
        recurse: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        refuse_unported(false, false, inPlace)?;
        if !processOffsets || !processDurations || !recurse {
            return Err(StreamException::new_err(
                "quantize here moves both the offsets and the lengths, everywhere in the stream",
            ));
        }
        let divisors = quarterLengthDivisors.unwrap_or_default();
        remade(slf, |stream| {
            music21_rs_crate::makenotation::quantize(stream, &divisors)
        })
    }

    /// The stream with its notes taken out and a rest standing in for each
    /// stretch of them: music21's `template`.
    #[pyo3(signature = (*, fillWithRests = true, removeClasses = None, retainVoices = true, removeAll = false, exemptFromRemove = None))]
    fn template<'py>(
        slf: &Bound<'py, Self>,
        fillWithRests: bool,
        removeClasses: Option<&Bound<'py, PyAny>>,
        retainVoices: bool,
        removeAll: bool,
        exemptFromRemove: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        // The classes taken out, and the ones kept whatever else says, are
        // a rule the crate takes as a function, which nothing here can
        // write at run time; what it answers is music21's own default set.
        let given = |asked: Option<&Bound<'py, PyAny>>| {
            asked.is_some_and(|asked| !asked.is_none() && asked.len().unwrap_or(1) > 0)
        };
        if given(removeClasses) || given(exemptFromRemove) {
            return Err(StreamException::new_err(
                "removeClasses and exemptFromRemove are not supported here: the classes taken out are music21's own default set, and removeAll=True takes out everything",
            ));
        }
        let options = music21_rs_crate::stream::TemplateOptions {
            fill_with_rests: fillWithRests,
            retain_voices: retainVoices,
            remove_all: removeAll,
            ..music21_rs_crate::stream::TemplateOptions::default()
        };
        let stream = crate_stream(slf.as_any())?;
        from_crate(slf.py(), &stream.template(&options))
    }

    /// The stream cut in two at a length from its start, each piece a new
    /// stream: music21's `splitAtQuarterLength`, whose answer unpacks as a
    /// pair.
    ///
    /// A measure's clef, key and meter stand in the second piece as they
    /// do in the first, which is what music21's `searchContext` finds.
    #[pyo3(signature = (quarterLength, *, retainOrigin = true, addTies = true, displayTiedAccidentals = false, searchContext = true))]
    fn splitAtQuarterLength<'py>(
        slf: &Bound<'py, Self>,
        quarterLength: f64,
        retainOrigin: bool,
        addTies: bool,
        displayTiedAccidentals: bool,
        searchContext: bool,
    ) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
        let _ = (displayTiedAccidentals, searchContext);
        refuse_unported(false, !addTies, false)?;
        if !retainOrigin {
            return Err(StreamException::new_err(
                "retainOrigin=False is not supported here: both pieces are new streams",
            ));
        }
        let stream = crate_stream(slf.as_any())?;
        let (first, second) = stream
            .split_at_quarter_length(quarterLength, &music21_rs_crate::stream::InForce::default())
            .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
        let py = slf.py();
        Ok((from_crate(py, &first)?, from_crate(py, &second)?))
    }

    /// Ties each run of notes of one note together, where one ends where
    /// the next begins: music21's `extendTies`, which writes the ties onto
    /// the notes the stream holds and answers nothing.
    ///
    /// The tie is written onto the stream's own notes, as music21 writes
    /// it: the crate is asked which notes are tied and the answer is
    /// carried back by position.
    #[pyo3(signature = (ignoreRests = false, pitchAttr = "nameWithOctave"))]
    fn extendTies(slf: &Bound<'_, Self>, ignoreRests: bool, pitchAttr: &str) -> PyResult<()> {
        if pitchAttr != "nameWithOctave" {
            return Err(StreamException::new_err(
                "a pitchAttr other than 'nameWithOctave' is not supported here: notes are tied where they are the same note",
            ));
        }
        let py = slf.py();
        let read = read_values(slf.as_any())?;
        let mut tied = read.stream;
        tied.extend_ties(ignoreRests);
        for (position, (_, element)) in tied.leaves().iter().enumerate() {
            let Some(tie) = tie_of(element) else { continue };
            let object = read.leaves[position].bind(py);
            let made = crate::installed_new(
                py,
                "music21.tie",
                "Tie",
                crate::notation::Tie::wrap(tie.clone()),
            )?;
            object.setattr("tie", made)?;
        }
        Ok(())
    }

    /// Moves the elements this stream holds itself by an offset, those
    /// starting at or after `startOffset` and before `endOffset` where
    /// either is given: music21's `shiftElements`, which moves what the
    /// stream holds and answers nothing.
    ///
    /// Where a thing stands is the stream's own to say, so the offsets are
    /// moved here rather than read back out of the crate; which of them
    /// move is music21's rule, the same one `Stream::shift_elements`
    /// follows.
    #[pyo3(signature = (offset, startOffset = None, endOffset = None, classFilterList = None))]
    fn shiftElements(
        slf: &Bound<'_, Self>,
        offset: f64,
        startOffset: Option<f64>,
        endOffset: Option<f64>,
        classFilterList: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        refuse_unported(
            classFilterList.is_some_and(|list| !list.is_none()),
            false,
            false,
        )?;
        let mut held = slf.borrow_mut();
        for one in &mut held.held {
            let moved = startOffset.is_none_or(|start| one.offset >= start)
                && endOffset.is_none_or(|end| one.offset < end);
            if moved {
                one.offset += offset;
            }
        }
        Ok(())
    }

    /// A copy with a measure's one voice dissolved into the measure
    /// itself: music21's `flattenUnnecessaryVoices`, which `force` applies
    /// to every voice however many there are.
    #[pyo3(signature = (*, force = false, inPlace = false))]
    fn flattenUnnecessaryVoices<'py>(
        slf: &Bound<'py, Self>,
        force: bool,
        inPlace: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        refuse_unported(false, false, inPlace)?;
        remade(slf, |stream| {
            stream.flatten_unnecessary_voices(force);
            Ok(())
        })
    }

    /// A copy with every note where it sounds rather than where it is
    /// written: music21's `toSoundingPitch`, which moves each stretch by
    /// the transposition of the instrument playing it.
    #[pyo3(signature = (*, inPlace = false))]
    fn toSoundingPitch<'py>(slf: &Bound<'py, Self>, inPlace: bool) -> PyResult<Bound<'py, PyAny>> {
        refuse_unported(false, false, inPlace)?;
        edited(slf, RsStream::to_sounding_pitch)
    }

    /// A copy with every note where it is written for its instrument:
    /// music21's `toWrittenPitch`. `ottavasToSounding` leaves the notes
    /// under an octave line where they sound, which is how music21's own
    /// MusicXML writer asks for it.
    #[pyo3(signature = (*, inPlace = false, ottavasToSounding = false))]
    fn toWrittenPitch<'py>(
        slf: &Bound<'py, Self>,
        inPlace: bool,
        ottavasToSounding: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        refuse_unported(false, false, inPlace)?;
        edited(slf, |stream| stream.to_written_pitch_by(ottavasToSounding))
    }

    /// The offsets where something in this stream and something in
    /// another both start, each stream's own elements: music21's
    /// `simultaneousAttacks`.
    fn simultaneousAttacks(
        slf: &Bound<'_, Self>,
        stream2: &Bound<'_, PyAny>,
    ) -> PyResult<Vec<f64>> {
        let ours = crate_stream(slf.as_any())?;
        let theirs = crate_stream(stream2)?;
        Ok(ours.simultaneous_attacks(&theirs))
    }

    /// Whether this stream holds parts, or streams standing side by side
    /// as parts do: music21's `hasPartLikeStreams`.
    fn hasPartLikeStreams(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Ok(crate_stream(slf.as_any())?.has_part_like_streams())
    }

    /// Whether some accidental in the stream has had its showing decided:
    /// music21's `haveAccidentalsBeenMade`.
    ///
    /// music21 keeps a flag and sets it when `makeAccidentals` runs; this
    /// reads the pitches, which is what its own documentation says the
    /// flag means -- true where any accidental has a display status,
    /// "regardless of if makeAccidentals has actually been run".
    fn haveAccidentalsBeenMade(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Ok(music21_rs_crate::makenotation::accidentals_made(
            &crate_stream(slf.as_any())?,
        ))
    }

    /// A stream standing for the gaps in this one, or `None` where there
    /// are none: music21's `findGaps`.
    ///
    /// Each gap is a rest from where it begins for as long as it lasts,
    /// where music21 answers bare `Music21Object`s carrying the same
    /// offset and length; a rest is the thing this package has for a
    /// stretch of nothing.
    fn findGaps<'py>(slf: &Bound<'py, Self>) -> PyResult<Option<Bound<'py, PyAny>>> {
        let stream = crate_stream(slf.as_any())?;
        match stream.find_gaps() {
            Some(gaps) => Ok(Some(from_crate(slf.py(), &gaps)?)),
            None => Ok(None),
        }
    }

    /// A copy cut into measures by the meters it states: music21's
    /// `makeMeasures`.
    ///
    /// `meterStream` is read for the meters and where each stands, as
    /// music21 reads it. A barline the measures are closed with is not
    /// kept here, the package having no class for one.
    #[pyo3(signature = (meterStream = None, *, refStreamOrTimeRange = None, searchContext = false, innerBarline = None, finalBarline = Some("final".to_string()), bestClef = false, inPlace = false))]
    #[allow(clippy::too_many_arguments)]
    fn makeMeasures<'py>(
        slf: &Bound<'py, Self>,
        meterStream: Option<&Bound<'py, PyAny>>,
        refStreamOrTimeRange: Option<&Bound<'py, PyAny>>,
        searchContext: bool,
        innerBarline: Option<&Bound<'py, PyAny>>,
        finalBarline: Option<String>,
        bestClef: bool,
        inPlace: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        refuse_unported(false, false, inPlace)?;
        if refStreamOrTimeRange.is_some_and(|given| !given.is_none())
            || searchContext
            || innerBarline.is_some_and(|given| !given.is_none())
            || bestClef
        {
            return Err(StreamException::new_err(
                "refStreamOrTimeRange, searchContext, innerBarline and bestClef are not supported here",
            ));
        }
        // The package keeps no barlines, so the one the measures are closed
        // with cannot be answered either way; `final` is music21's default
        // and anything else asks for what cannot be seen here.
        if finalBarline.as_deref().is_some_and(|kind| kind != "final") {
            return Err(StreamException::new_err(
                "a finalBarline other than 'final' is not supported here: this package has no class for a barline",
            ));
        }
        let meters = match meterStream.filter(|given| !given.is_none()) {
            Some(given) => meters_of(given)?,
            None => Vec::new(),
        };
        edited(slf, |stream| {
            if meters.is_empty() {
                music21_rs_crate::makenotation::make_measures(stream)
            } else {
                music21_rs_crate::makenotation::make_measures_by(stream, &meters)
            }
        })
    }

    /// A copy with every measure filled out to its bar with rests, its
    /// voices too, and the gaps inside it filled: music21's `makeRests`
    /// with `fillGaps=True` and `timeRangeFromBarDuration=True`.
    ///
    /// That one combination is what the crate answers, so music21's
    /// defaults are refused rather than answered as though they had been
    /// asked for.
    #[pyo3(signature = (refStreamOrTimeRange = None, *, fillGaps = false, timeRangeFromBarDuration = false, inPlace = false, hideRests = false))]
    fn makeRests<'py>(
        slf: &Bound<'py, Self>,
        refStreamOrTimeRange: Option<&Bound<'py, PyAny>>,
        fillGaps: bool,
        timeRangeFromBarDuration: bool,
        inPlace: bool,
        hideRests: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        refuse_unported(false, false, inPlace)?;
        if !fillGaps || !timeRangeFromBarDuration {
            return Err(StreamException::new_err(
                "makeRests here fills the gaps and takes the bar's length, so fillGaps=True and timeRangeFromBarDuration=True are what it answers",
            ));
        }
        if hideRests || refStreamOrTimeRange.is_some_and(|given| !given.is_none()) {
            return Err(StreamException::new_err(
                "hideRests and refStreamOrTimeRange are not supported here",
            ));
        }
        remade(slf, music21_rs_crate::makenotation::make_rests)
    }

    /// A copy with what runs past a barline cut there and tied: music21's
    /// `makeTies`.
    #[pyo3(signature = (meterStream = None, *, inPlace = false, displayTiedAccidentals = false, classFilterList = None))]
    fn makeTies<'py>(
        slf: &Bound<'py, Self>,
        meterStream: Option<&Bound<'py, PyAny>>,
        inPlace: bool,
        displayTiedAccidentals: bool,
        classFilterList: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        refuse_unported(
            classFilterList.is_some_and(|list| !list.is_none()),
            false,
            inPlace,
        )?;
        if meterStream.is_some_and(|given| !given.is_none()) || displayTiedAccidentals {
            return Err(StreamException::new_err(
                "a meterStream and displayTiedAccidentals are not supported here: the meters are the stream's own",
            ));
        }
        remade(slf, music21_rs_crate::makenotation::make_ties)
    }

    /// A copy with its notes beamed by the meter in force, and each beamed
    /// group's stems pointed one way: music21's `makeBeams`.
    #[pyo3(signature = (*, inPlace = false, setStemDirections = true, failOnNoTimeSignature = false))]
    fn makeBeams<'py>(
        slf: &Bound<'py, Self>,
        inPlace: bool,
        setStemDirections: bool,
        failOnNoTimeSignature: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        refuse_unported(false, false, inPlace)?;
        if !setStemDirections || failOnNoTimeSignature {
            return Err(StreamException::new_err(
                "setStemDirections=False and failOnNoTimeSignature=True are not supported here: beaming always points a group's stems, and a stream with no meter is left unbeamed",
            ));
        }
        remade(slf, music21_rs_crate::makenotation::make_beams)
    }

    /// A copy with each accidental's showing decided: music21's
    /// `makeAccidentals`, at its own defaults.
    #[pyo3(signature = (*, inPlace = false, **keywords))]
    fn makeAccidentals<'py>(
        slf: &Bound<'py, Self>,
        inPlace: bool,
        keywords: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        refuse_unported(false, false, inPlace)?;
        if keywords.is_some_and(|keywords| !keywords.is_empty()) {
            return Err(StreamException::new_err(
                "the accidental keywords are not supported here: what is decided is music21's own defaults",
            ));
        }
        remade(slf, |stream| {
            music21_rs_crate::makenotation::make_accidentals(stream);
            Ok(())
        })
    }

    /// A copy with each stretch of overlapping notes put into voices:
    /// music21's `makeVoices` with `fillGaps=False`.
    ///
    /// music21 fills the gaps its voices leave by default; nothing here
    /// does, so `fillGaps=True` is refused rather than answered without
    /// the rests.
    #[pyo3(signature = (*, inPlace = false, fillGaps = true))]
    fn makeVoices<'py>(
        slf: &Bound<'py, Self>,
        inPlace: bool,
        fillGaps: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        refuse_unported(false, false, inPlace)?;
        if fillGaps {
            return Err(StreamException::new_err(
                "fillGaps=True is not supported here: the voices are made without the rests that fill their gaps",
            ));
        }
        remade(slf, |stream| {
            music21_rs_crate::makenotation::make_voices(stream);
            Ok(())
        })
    }

    /// A copy notated as music21's exporter would notate it before
    /// writing: music21's `makeNotation`, which is measures, accidentals,
    /// ties, beams and tuplet brackets in music21's own order.
    #[pyo3(signature = (*, inPlace = false, **keywords))]
    fn makeNotation<'py>(
        slf: &Bound<'py, Self>,
        inPlace: bool,
        keywords: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        refuse_unported(false, false, inPlace)?;
        if keywords.is_some_and(|keywords| !keywords.is_empty()) {
            return Err(StreamException::new_err(
                "the makeNotation keywords are not supported here: each step is taken at music21's own defaults",
            ));
        }
        remade(slf, music21_rs_crate::makenotation::make_notation)
    }

    /// The lyrics sung in the stream, gathered by the number each is sung
    /// under: music21's `lyrics`.
    ///
    /// Each number's line has a place for every note read, holding the
    /// lyric sung there or `None` where that note sings nothing under that
    /// number, so the lines stay level with the notes; a number first met
    /// partway through is padded with the notes before it. The lyric
    /// handed back is the note's own object, as music21's is. `recurse`
    /// answers a list of its own per nested stream, as music21's
    /// `RecursiveLyricList` does.
    #[pyo3(signature = (*, ignoreBarlines = true, recurse = false, skipTies = false))]
    fn lyrics<'py>(
        slf: &Bound<'py, Self>,
        ignoreBarlines: bool,
        recurse: bool,
        skipTies: bool,
    ) -> PyResult<Bound<'py, PyDict>> {
        let options = music21_rs_crate::stream::LyricsOptions {
            ignore_barlines: ignoreBarlines,
            recurse,
            skip_ties: skipTies,
        };
        let py = slf.py();
        let read = read_values(slf.as_any())?;
        let out = PyDict::new(py);
        for (number, line) in read.stream.lyric_positions(&options) {
            out.set_item(number, sung_line(py, &line, &read.leaves)?)?;
        }
        Ok(out)
    }

    /// How long the stream lasts in seconds: music21's `seconds`.
    ///
    /// Only the tempi the stream holds itself count, each from where it
    /// stands until the next and the last to the end. music21 searches a
    /// stream's context for a tempo where it states none from its start;
    /// nothing here holds a stream's containers, so a stream with no tempo
    /// of its own answers `nan` -- which is what music21 answers for a
    /// stream whose search finds nothing -- and nought where it holds
    /// nothing that lasts.
    #[getter]
    fn seconds(slf: &Bound<'_, Self>) -> PyResult<f64> {
        crate_stream(slf.as_any())?
            .seconds(None)
            .map_err(|error| TempoException::new_err(crate::pitch::message(&error)))
    }

    /// How many lines this stream's staff has: music21's `staffLines`,
    /// read from the first staff layout standing at its start.
    #[getter]
    fn staffLines(slf: &Bound<'_, Self>) -> PyResult<i64> {
        Ok(crate_stream(slf.as_any())?.staff_lines())
    }

    /// The parts this stream holds directly.
    #[getter]
    fn parts<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, Stream>> {
        let py = slf.py();
        let part = pyo3::types::PyString::new(py, "Part");
        Self::picked(slf, Some(StreamKind::Score), |object| {
            is_of_class(object, &part)
        })
    }

    #[pyo3(signature = (memo = None))]
    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let deepcopy = py.import("copy")?.getattr("deepcopy")?;
        let copied = slf.get_type().call0()?;
        let held: Vec<(f64, Py<PyAny>)> = slf
            .borrow()
            .sorted()
            .iter()
            .map(|held| (held.offset, held.object.clone_ref(py)))
            .collect();
        {
            let stream = copied.cast::<Stream>()?;
            let mut stream = stream.borrow_mut();
            stream.id = slf.borrow().id.as_ref().map(|id| id.clone_ref(py));
            if let Some(set) = &slf.borrow().unlinked_duration {
                stream.unlinked_duration = Some(deepcopy.call1((set, memo))?.unbind());
            }
            for (offset, object) in held {
                let object = deepcopy.call1((object, memo))?;
                stream.put(offset, &object);
            }
        }
        Ok(copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let copied = slf.get_type().call0()?;
        {
            let stream = copied.cast::<Stream>()?;
            let mut stream = stream.borrow_mut();
            let me = slf.borrow();
            stream.id = me.id.as_ref().map(|id| id.clone_ref(py));
            stream.unlinked_duration = me.unlinked_duration.as_ref().map(|set| set.clone_ref(py));
            for held in me.sorted() {
                stream.put(held.offset, held.object.bind(py));
            }
        }
        Ok(copied)
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let py = slf.py();
        let label = match &slf.borrow().id {
            Some(id) => id.bind(py).str()?.to_string(),
            None => format!("0x{:x}", slf.as_ptr() as usize),
        };
        Ok(format!(
            "<music21.stream.{} {label}>",
            Self::class_name(slf)?
        ))
    }
}

/// What music21's `common.deprecated` says for `hasElementOfClass`.
const HAS_ELEMENT_OF_CLASS_DEPRECATED: &str = "Stream.hasElementOfClass was deprecated on v11 \
     and will disappear at or after v12. use `bool(s.getElementsByClass(className))` instead";

/// Whether `hasElementOfClass` has warned yet in this process.
static HAS_ELEMENT_OF_CLASS_WARNED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Refuses what the wheel's copies of music21's editing methods cannot do:
/// edit one target, cut without ties, or change the stream in place.
fn refuse_unported(target: bool, untied: bool, in_place: bool) -> PyResult<()> {
    let refused = if target {
        "a target"
    } else if untied {
        "addTies=False"
    } else if in_place {
        "inPlace=True"
    } else {
        return Ok(());
    };
    Err(StreamException::new_err(format!(
        "{refused} is not supported here: the result is a new stream"
    )))
}

/// music21's `anchorZero` and `anchorZeroRecurse`.
fn anchor_of(name: Option<&str>) -> PyResult<music21_rs_crate::stream::Anchor> {
    use music21_rs_crate::stream::Anchor;
    match name {
        None => Ok(Anchor::Zero),
        Some("lowest") => Ok(Anchor::Lowest),
        Some("highest") => Ok(Anchor::Highest),
        Some(other) => Err(StreamException::new_err(format!(
            "an anchorZero value is not supported: {other}"
        ))),
    }
}

/// The tie an element carries, where it is one that can carry one.
fn tie_of(element: &StreamElement) -> Option<&music21_rs_crate::notation::Tie> {
    match element {
        StreamElement::Note(note) => note.tie(),
        StreamElement::Chord(chord) => chord.tie(),
        StreamElement::Rest(rest) => rest.tie(),
        _ => None,
    }
}

/// The meters a `meterStream` holds, each where it stands: music21 reads
/// one for the measures it cuts a stream into.
fn meters_of(
    stream: &Bound<'_, PyAny>,
) -> PyResult<Vec<(f64, music21_rs_crate::meter::TimeSignature)>> {
    let read = crate_stream(stream)?;
    Ok(read
        .events()
        .iter()
        .filter_map(|event| match event.element() {
            StreamElement::TimeSignature(meter) => Some((event.offset(), meter.clone())),
            _ => None,
        })
        .collect())
}

/// A crate stream and the objects at each position, for a caller that
/// carries an answer back onto them: what `read_values` reads.
pub(crate) fn read_with_leaves(stream: &Bound<'_, PyAny>) -> PyResult<(RsStream, Vec<Py<PyAny>>)> {
    let read = read_values(stream)?;
    Ok((read.stream, read.leaves))
}

/// One element -- music21's own object or this package's -- read as the
/// value a walk reads it as, with a stand-in where the walks use one.
///
/// It is what reading a whole stream reads each element into, for a caller
/// handed the elements themselves rather than the stream holding them:
/// music21's searches take a stream iterator as readily as a stream, and
/// its translations iterate whatever they are given.
pub(crate) fn element_read(object: &Bound<'_, PyAny>) -> PyResult<Option<StreamElement>> {
    element_value(object, true)
}

/// The same as the value it *is*, with no stand-in: what a caller asking
/// about one element wants, where a walk wants the stand-in.
///
/// A percussion chord is the pair of them: the walks read one as a rest
/// of its length, and a caller asking what it ties wants the strokes.
pub(crate) fn element_written(object: &Bound<'_, PyAny>) -> PyResult<Option<StreamElement>> {
    element_value(object, false)
}

/// music21's error for what a stream cannot answer, out of the crate's.
pub(crate) fn stream_error(error: &music21_rs_crate::Error) -> PyErr {
    StreamException::new_err(crate::pitch::message(error))
}

/// music21's `findConsecutiveNotes` keywords, which its own analysis
/// functions pass on.
pub(crate) fn consecutive_options(
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<music21_rs_crate::stream::ConsecutiveOptions> {
    let flag = |name: &str| -> PyResult<bool> {
        match keywords {
            Some(keywords) => match keywords.get_item(name)? {
                Some(value) => value.is_truthy(),
                None => Ok(false),
            },
            None => Ok(false),
        }
    };
    Ok(music21_rs_crate::stream::ConsecutiveOptions {
        skip_rests: flag("skipRests")?,
        skip_chords: flag("skipChords")?,
        skip_unisons: flag("skipUnisons")?,
        skip_octaves: flag("skipOctaves")?,
        skip_gaps: flag("skipGaps")?,
        get_overlaps: flag("getOverlaps")?,
        no_none: flag("noNone")?,
    })
}

/// A stream some module-level function of music21's changes, answered as
/// a new stream of this package's: the shape `instrument`'s four take.
///
/// music21 changes the stream it is given where `inPlace` is asked for;
/// the answer here is always a new stream, as every edit in this package
/// is, so that is refused rather than answered differently.
pub(crate) fn changed<'py>(
    stream: &Bound<'py, PyAny>,
    in_place: bool,
    change: impl FnOnce(&mut RsStream) -> music21_rs_crate::Result<()>,
) -> PyResult<Bound<'py, PyAny>> {
    refuse_unported(false, false, in_place)?;
    let mut read = crate_stream(stream)?;
    change(&mut read).map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
    from_crate(stream.py(), &read)
}

/// The stream changed in place by the crate, as a new stream of this
/// package's: the crate's own answer is a change to a stream it is given,
/// and music21 hands back a copy unless asked for `inPlace`.
fn remade<'py>(
    slf: &Bound<'py, Stream>,
    change: impl FnOnce(&mut RsStream) -> music21_rs_crate::Result<()>,
) -> PyResult<Bound<'py, PyAny>> {
    let mut stream = crate_stream(slf.as_any())?;
    change(&mut stream).map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
    from_crate(slf.py(), &stream)
}

/// One line of lyrics as music21 answers it: the note's own `Lyric` where
/// one is sung, `None` where that note sings nothing under the number, and
/// a list of its own for a nested stream.
fn sung_line<'py>(
    py: Python<'py>,
    line: &[music21_rs_crate::stream::SungAt],
    leaves: &[Py<PyAny>],
) -> PyResult<Bound<'py, PyList>> {
    use music21_rs_crate::stream::SungAt;
    let out = PyList::empty(py);
    for sung in line {
        match sung {
            SungAt::Lyric(place) => {
                let lyrics = leaves[place.position].bind(py).getattr("lyrics")?;
                out.append(lyrics.get_item(place.index)?)?;
            }
            SungAt::Unsung => out.append(py.None())?,
            SungAt::Nested(inner) => out.append(sung_line(py, inner, leaves)?)?,
        }
    }
    Ok(out)
}

/// The stream edited by the crate, as a new stream of this package's.
fn edited<'py>(
    slf: &Bound<'py, Stream>,
    edit: impl FnOnce(&RsStream) -> music21_rs_crate::Result<RsStream>,
) -> PyResult<Bound<'py, PyAny>> {
    let stream = crate_stream(slf.as_any())?;
    let made =
        edit(&stream).map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
    from_crate(slf.py(), &made)
}

/// The class a stream of this shape is built as: music21's own where
/// music21 can be asked, and this package's where it cannot.
pub(crate) fn stream_class<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    if let Ok(found) = py
        .import("music21.stream")
        .and_then(|module| module.getattr(name))
    {
        return Ok(found);
    }
    wheel_stream_class(py, name)
}

/// This package's own stream class of that name.
fn wheel_stream_class<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    py.import("music21_rs")
        .or_else(|_| py.import("music21_rs_facade"))?
        .getattr(name)
}

/// Whether music21's own streams will hold this object.
///
/// music21 refuses to put anything that is not a `Music21Object` at a point
/// in time, which its own classes and the installed ones are and a bare
/// facade class of this package's is not. Asked of an object rather than
/// worked out from what is installed, because the two can differ: a caller
/// may hand over objects of either kind whatever has been installed.
pub(crate) fn music21_holds(object: &Bound<'_, PyAny>) -> bool {
    let py = object.py();
    py.import("music21.base")
        .and_then(|module| module.getattr("Music21Object"))
        .and_then(|class| object.is_instance(&class))
        .unwrap_or(false)
}

/// A stream class of that name for a stream that is to hold `sample`:
/// music21's where music21 will hold it, and this package's where it will
/// not.
///
/// `stream_class` alone answers music21's wherever music21 can be imported,
/// which is right for a stream built out of nothing and wrong for one built
/// to hold objects a caller gave. A caller with `pip install music21
/// music21-rs` and no swap has objects of this package's own, and music21's
/// stream refuses them -- so what was a score of its notes came back as
/// `The object you tried to add to the Stream, <music21.note.Note C#>, is
/// not a Music21Object`. It is the same fault the spanner's storage had.
pub(crate) fn stream_class_like<'py>(
    py: Python<'py>,
    name: &str,
    sample: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    match sample {
        Some(sample) if !music21_holds(sample) => wheel_stream_class(py, name),
        _ => stream_class(py, name),
    }
}

/// Whose streams a crate stream is built as.
///
/// A wheel on its own has only its own; a facade standing inside music21
/// must answer music21's, because music21's own code indexes a part by
/// class (`p[roman.RomanNumeral][0]`) and a stream of ours is not one of
/// its. It is the same choice `SearchMatch`, `spanner.Line` and `Style`
/// make: music21's where music21 can be asked, the wheel's otherwise.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Whose {
    /// This package's own `Score`, `Part`, `Measure` and the rest.
    Wheel,
    /// music21's own, for a facade answering inside music21.
    Music21,
}

/// A measure's barline, as music21's own class wherever music21 can be
/// asked for one.
///
/// It is the choice `spanner.Line` makes, and a barline is the clearest
/// case for it: it carries nothing of the crate but its type and, where it
/// is a repeat, its direction and count, while music21's own readers and
/// writers look for those two classes by name.
///
/// **It matters more here than it does for a line.** A bare facade class of
/// this package's is not a `Music21Object`, so music21's own streams refuse
/// it outright -- and installing `music21.bar` to get round that is worse
/// than it looks, because any corpus score parsed beside the installed
/// class is written into music21's pickle cache carrying it, and that cache
/// is one directory for the whole machine. A later process reading that
/// pickle calls `_thawed`, which installs nothing for a module
/// `install_into_music21` does not carry and so hands back the bare class
/// again -- in a process with no facade in sight. music21's own barline is
/// nobody's to install, and so it travels.
fn barline_object<'py>(
    py: Python<'py>,
    barline: &music21_rs_crate::bar::Barline,
    whose: Whose,
) -> PyResult<Bound<'py, PyAny>> {
    if whose == Whose::Music21
        && let Ok(module) = py.import("music21.bar")
    {
        return match barline.repeat_direction() {
            Some(direction) => {
                let given = PyDict::new(py);
                given.set_item("direction", direction.as_str())?;
                given.set_item("times", barline.repeat_times())?;
                module.getattr("Repeat")?.call((), Some(&given))
            }
            None => module
                .getattr("Barline")?
                .call1((barline.bar_type().as_str(),)),
        };
    }
    Ok(crate::bar::Barline::object(py, barline.clone())?.into_bound(py))
}

/// A crate stream as this wheel's objects: a stream of its kind holding a
/// wheel object for each element the wheel has a class for, at the offsets
/// they stand at, with what the stream is called and numbered as the
/// attributes music21 keeps them in.
///
/// What the wheel has no class for is left out: words and repeat marks,
/// unpitched strokes, barlines, pedal bounces and gaps, a manuscript's
/// breaks, spanners and a score's metadata.
pub(crate) fn from_crate<'py>(py: Python<'py>, stream: &RsStream) -> PyResult<Bound<'py, PyAny>> {
    from_crate_as(py, stream, Whose::Wheel)
}

/// The same as music21's own streams, where music21 is already imported.
///
/// The *elements* are already music21's wherever anything is installed --
/// `element_object` builds every one through `installed_new` -- so what
/// differs is the container and how an element goes into it.
pub(crate) fn music21_from_crate<'py>(
    py: Python<'py>,
    stream: &RsStream,
) -> PyResult<Bound<'py, PyAny>> {
    // Already imported, not merely importable: deciding must not cost the
    // 370ms of importing music21.
    let loaded = py
        .import("sys")?
        .getattr("modules")?
        .contains("music21.stream")?;
    let whose = match loaded {
        true => Whose::Music21,
        false => Whose::Wheel,
    };
    from_crate_as(py, stream, whose)
}

/// A crate stream as one kind of streams or the other.
pub(crate) fn from_crate_as<'py>(
    py: Python<'py>,
    stream: &RsStream,
    whose: Whose,
) -> PyResult<Bound<'py, PyAny>> {
    let made = match whose {
        Whose::Wheel => {
            let class = match stream.kind() {
                StreamKind::Score => py.get_type::<Score>(),
                StreamKind::Part => py.get_type::<Part>(),
                StreamKind::PartStaff => py.get_type::<PartStaff>(),
                StreamKind::Measure => py.get_type::<Measure>(),
                StreamKind::Voice => py.get_type::<Voice>(),
                StreamKind::Opus => py.get_type::<Opus>(),
                _ => py.get_type::<Stream>(),
            };
            class.call0()?
        }
        Whose::Music21 => {
            let name = match stream.kind() {
                StreamKind::Score => "Score",
                StreamKind::Part => "Part",
                StreamKind::PartStaff => "PartStaff",
                StreamKind::Measure => "Measure",
                StreamKind::Voice => "Voice",
                StreamKind::Opus => "Opus",
                _ => "Stream",
            };
            py.import("music21.stream")?.getattr(name)?.call0()?
        }
    };
    let mut objects = Vec::new();
    for event in stream.events() {
        if let Some(object) = element_object_as(py, event.element(), whose)? {
            objects.push((event.offset(), object));
        }
    }
    match whose {
        Whose::Wheel => {
            let cell = made.cast::<Stream>()?;
            let mut held = cell.borrow_mut();
            for (offset, object) in &objects {
                held.put(*offset, object);
            }
            if let Some(id) = stream.id() {
                held.id = Some(pyo3::types::PyString::new(py, id).into_any().unbind());
            }
        }
        Whose::Music21 => {
            for (offset, object) in &objects {
                made.call_method1("insert", (*offset, object))?;
            }
            if let Some(id) = stream.id() {
                made.setattr("id", id)?;
            }
        }
    }
    // The spanners, which name the leaves they join by position: each
    // goes in at nought, as music21 keeps one, holding the very objects
    // just built.
    if !stream.spanners().is_empty() {
        let leaves: Vec<Py<PyAny>> = match whose {
            Whose::Wheel => {
                let cell = made.cast::<Stream>()?;
                let held = cell.borrow();
                leaves_of(py, &held)?
            }
            Whose::Music21 => music21_leaves_of(&made)?,
        };
        for spanner in stream.spanners() {
            let joined: Vec<Py<PyAny>> = spanner
                .spanned()
                .iter()
                .filter_map(|place| place.and_then(|place| leaves.get(place)))
                .map(|object| object.clone_ref(py))
                .collect();
            let built = crate::spanner::Spanner::object_as(py, spanner, joined, whose)?;
            match whose {
                Whose::Wheel => {
                    let cell = made.cast::<Stream>()?;
                    cell.borrow_mut().put(0.0, built.bind(py));
                }
                Whose::Music21 => {
                    made.call_method1("insert", (0.0, built.bind(py)))?;
                }
            }
        }
    }
    // The metadata a reader found, which music21 keeps among the elements.
    if let Some(metadata) = stream.metadata() {
        // The installed class, so music21's own `metadata` setter can put
        // it into the stream: it inserts the object, and its streams hold
        // nothing that is not a `Music21Object`.
        let carried = crate::installed_new(
            py,
            "music21.metadata",
            "Metadata",
            crate::metadata::Metadata::wrap(metadata.clone()),
        )?;
        made.setattr("metadata", carried)?;
    }
    if let Some(at) = stream.at_sounding_pitch() {
        made.setattr("atSoundingPitch", at)?;
    }
    match stream.kind() {
        StreamKind::Measure => {
            // Through music21's own setters, which put them among the
            // measure's elements; the wheel's measures keep them beside.
            if let Some(left) = stream.left_barline() {
                made.setattr("leftBarline", barline_object(py, left, whose)?)?;
            }
            if let Some(right) = stream.right_barline() {
                made.setattr("rightBarline", barline_object(py, right, whose)?)?;
            }
            made.setattr("number", stream.number())?;
            made.setattr("numberSuffix", stream.number_suffix())?;
            made.setattr("paddingLeft", stream.padding_left())?;
            made.setattr("paddingRight", stream.padding_right())?;
        }
        StreamKind::Part | StreamKind::PartStaff => {
            made.setattr("partName", stream.name())?;
            made.setattr("partAbbreviation", stream.abbreviation())?;
        }
        _ => {}
    }
    Ok(made)
}

/// Every leaf of a music21 stream, in the order the crate counts them:
/// what `recurse` walks past the streams themselves.
fn music21_leaves_of(stream: &Bound<'_, PyAny>) -> PyResult<Vec<Py<PyAny>>> {
    let mut out = Vec::new();
    for element in stream.call_method0("recurse")?.walk()? {
        let element = element?;
        let held = element
            .getattr("isStream")
            .and_then(|flag| flag.is_truthy())
            .unwrap_or(false);
        if held {
            continue;
        }

        // music21 holds a measure's own barlines among its elements; the
        // crate keeps them on the measure and counts no leaf for them.
        let container = element.getattr("activeSite")?;
        if !container.is_none() && !is_inner_barline(&container, &element)? && is_barline(&element)?
        {
            continue;
        }
        out.push(element.unbind());
    }
    Ok(out)
}

/// Every leaf of a stream of this package's, in the order the crate
/// counts them: a nested stream's leaves where it holds one, itself
/// otherwise. A spanner names what it joins by that position.
fn leaves_of(py: Python<'_>, stream: &Stream) -> PyResult<Vec<Py<PyAny>>> {
    let mut out = Vec::new();
    for one in stream.sorted() {
        let object = one.object.bind(py);
        if let Ok(inner) = object.cast::<Stream>() {
            out.extend(leaves_of(py, &inner.borrow())?);
        } else {
            out.push(one.object.clone_ref(py));
        }
    }
    Ok(out)
}

/// The wheel's object for one element of a crate stream, where it has a
/// class for it.
pub(crate) fn element_object<'py>(
    py: Python<'py>,
    element: &StreamElement,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    element_object_as(py, element, Whose::Wheel)
}

/// An element of a kind this package carries a value for, built as the
/// class the stream it is going into will hold.
///
/// `holdable_new` for music21's streams and `installed_new` for this
/// package's: a class of this package's own must read the same whether or
/// not music21 happens to be importable, and a subclass of music21's
/// carries machinery of music21's that changes what the wheel answers --
/// `analysis.attributeCount` counted a clef and a meter as lengths of
/// nought on a machine with music21 installed and not on one without.
fn element_new<T>(
    py: Python<'_>,
    module: &str,
    name: &str,
    value: T,
    holdable: bool,
) -> PyResult<Py<T>>
where
    T: pyo3::PyClass<Frozen = pyo3::pyclass::boolean_struct::False>,
    T: Into<pyo3::PyClassInitializer<T>>,
{
    if holdable {
        crate::holdable_new(py, module, name, value)
    } else {
        crate::installed_new(py, module, name, value)
    }
}

/// The same, with the nested streams built as one kind or the other.
fn element_object_as<'py>(
    py: Python<'py>,
    element: &StreamElement,
    whose: Whose,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    // A class music21 will hold, so an element built here is one its own
    // streams accept: they refuse anything that is not a `Music21Object`,
    // and a bare facade class is not one. Where nothing is installed and
    // music21 is nonetheless there -- a caller with both and no swap, which
    // is `pip install music21 music21-rs` -- the subclass installing would
    // build is built for the purpose and bound into nothing.
    // Whether what is built has to be a class music21's own streams will
    // hold. Only for music21's streams: a class of this package's own must
    // read the same whether or not music21 happens to be importable.
    let holdable = whose == Whose::Music21;
    let class_named = |module: &str, name: &str| -> PyResult<Bound<'py, PyAny>> {
        let found = if holdable {
            crate::holdable_class(py, module, name)
        } else {
            crate::installed_class(py, module, name)
        };
        if let Some(held) = found {
            return Ok(held);
        }
        py.import("music21_rs")
            .or_else(|_| py.import("music21_rs_facade"))?
            .getattr(name)
    };
    let made = match element {
        StreamElement::Stream(inner) => from_crate_as(py, inner, whose)?,
        StreamElement::Note(note) => element_new(
            py,
            "music21.note",
            "Note",
            Note::wrap(py, note.clone())?,
            holdable,
        )?
        .into_bound(py)
        .into_any(),
        StreamElement::Chord(chord) if chord.numeral().is_some() => {
            let numeral = chord.numeral().cloned().ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err("a numeral chord with no numeral")
            })?;
            crate::roman::RomanNumeral::object_on_as(
                py,
                crate::roman::RomanNumeral::wrap(numeral, None),
                chord.clone(),
                holdable,
            )?
            .into_bound(py)
            .into_any()
        }
        StreamElement::Chord(chord) => element_new(
            py,
            "music21.chord",
            "Chord",
            Chord::from_inner(py, chord.clone())?,
            holdable,
        )?
        .into_bound(py)
        .into_any(),
        StreamElement::Rest(rest) => element_new(
            py,
            "music21.note",
            "Rest",
            Rest::wrap(py, rest.clone()),
            holdable,
        )?
        .into_bound(py)
        .into_any(),
        StreamElement::Clef(clef) => {
            crate::clef::Clef::object_as(py, clef.clone(), holdable)?.into_bound(py)
        }
        StreamElement::Instrument(instrument) => {
            crate::instrument::Instrument::object_as(py, (**instrument).clone(), holdable)?
                .into_bound(py)
        }
        StreamElement::KeySignature(signature) => {
            let made = class_named("music21.key", "KeySignature")?.call1((signature.sharps(),))?;
            // A non-traditional signature says its pitches, in music21's
            // spelling; it used to be dropped here.
            if signature.is_non_traditional() {
                made.setattr("isNonTraditional", true)?;
                let names: Vec<String> = signature
                    .altered_pitches()
                    .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?
                    .iter()
                    .map(|pitch| crate::spelling::music21_name(&pitch.to_string()))
                    .collect();
                made.setattr("alteredPitches", names)?;
            }
            made
        }
        StreamElement::Key(key) => {
            class_named("music21.key", "Key")?.call1((key.tonic().name(), key.mode()))?
        }
        StreamElement::TimeSignature(meter) => {
            class_named("music21.meter.base", "TimeSignature")?.call1((meter.ratio_string(),))?
        }
        StreamElement::MetronomeMark(mark) => element_new(
            py,
            "music21.tempo",
            "MetronomeMark",
            MetronomeMark::wrap(mark.clone()),
            holdable,
        )?
        .into_bound(py)
        .into_any(),
        StreamElement::Barline(barline) => {
            crate::bar::Barline::object(py, barline.clone())?.into_bound(py)
        }
        StreamElement::Dynamic(dynamic) => element_new(
            py,
            "music21.dynamics",
            "Dynamic",
            Dynamic::wrap(dynamic.clone()),
            holdable,
        )?
        .into_bound(py)
        .into_any(),
        StreamElement::ChordSymbol(symbol) => {
            if symbol.is_no_chord() {
                class_named("music21.harmony", "NoChord")?.call0()?
            } else {
                let keywords = PyDict::new(py);
                keywords.set_item("root", symbol.root().name())?;
                if let Some(bass) = symbol.bass() {
                    keywords.set_item("bass", bass.name())?;
                }
                keywords.set_item("kind", symbol.kind().unwrap_or("major"))?;
                if let Some(text) = symbol.kind_text() {
                    keywords.set_item("kindStr", text)?;
                }
                class_named("music21.harmony", "ChordSymbol")?.call((), Some(&keywords))?
            }
        }
        StreamElement::TextExpression(text) => element_new(
            py,
            "music21.expressions",
            "TextExpression",
            crate::expressions::TextExpression::wrap(text.clone()),
            holdable,
        )?
        .into_bound(py)
        .into_any(),
        StreamElement::Variant(variant) => match variant_object(py, variant, whose)? {
            Some(made) => made,
            None => return Ok(None),
        },
        StreamElement::TempoText(_)
        | StreamElement::RepeatExpression(_)
        | StreamElement::Unpitched(_)
        | StreamElement::PercussionChord(_)
        | StreamElement::PedalObject(_)
        | StreamElement::RehearsalMark(_)
        | StreamElement::MetricModulation(_)
        | StreamElement::Break(_)
        | StreamElement::Layout(_)
        | StreamElement::TextBox(_)
        | StreamElement::GlobalComment(_)
        | StreamElement::BrailleSegmentDivision(_) => return Ok(None),
    };
    carry_notation(py, element, &made)?;
    Ok(Some(made))
}

/// Puts what a note or chord carries besides its pitches onto the object
/// built for it: the articulations and the ornaments.
///
/// The crate reads these and the writers write them, so a score read here
/// carries them; without this they lived in the value alone and
/// `note.articulations` was empty. A fermata and an arpeggio are left
/// behind: this package has no class for either, and building music21's
/// would be depending on music21, which it does not.
fn carry_notation(
    py: Python<'_>,
    element: &StreamElement,
    made: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let (articulations, expressions) = match element {
        StreamElement::Note(note) => (note.articulations(), note.expressions()),
        StreamElement::Chord(chord) => (chord.articulations(), chord.expressions()),
        StreamElement::Rest(rest) => (rest.articulations(), rest.expressions()),
        _ => return Ok(()),
    };
    if !articulations.is_empty() {
        let held = PyList::empty(py);
        for articulation in articulations {
            held.append(crate::articulations::Articulation::object(
                py,
                articulation.clone(),
            )?)?;
        }
        made.setattr("articulations", held)?;
    }
    let ornaments: Vec<_> = expressions
        .iter()
        .filter_map(|expression| match expression {
            music21_rs_crate::expressions::Expression::Ornament(ornament) => {
                Some((**ornament).clone())
            }
            _ => None,
        })
        .collect();
    if !ornaments.is_empty() {
        let held = PyList::empty(py);
        for ornament in ornaments {
            held.append(crate::expressions::Ornament::object(py, ornament)?)?;
        }
        made.setattr("expressions", held)?;
    }
    Ok(())
}

/// music21's other stream classes, each the same `Stream` under its name.
macro_rules! stream_kinds {
    ($(($class:ident, $name:literal, $kind:expr, $order:literal, $doc:literal)),* $(,)?) => {
        $(
            #[doc = $doc]
            #[pyclass(name = $name, module = "music21.stream", extends = Stream, subclass)]
            pub struct $class;

            #[pymethods]
            impl $class {
                /// music21's sort order for this kind of stream among the
                /// things standing at one offset. A voice sorts after the
                /// clef, key and meter of the measure holding it, where
                /// every other stream sorts before them.
                #[classattr]
                fn classSortOrder() -> i64 {
                    $order
                }

                #[new]
                #[pyo3(signature = (givenElements = None, *_arguments, **keywords))]
                fn new(
                    py: Python<'_>,
                    givenElements: Option<&Bound<'_, PyAny>>,
                    _arguments: &Bound<'_, PyTuple>,
                    keywords: Option<&Bound<'_, PyDict>>,
                ) -> PyResult<PyClassInitializer<Self>> {
                    Ok(PyClassInitializer::from(Stream::filled(
                        py,
                        $kind,
                        givenElements,
                        keywords,
                    )?)
                    .add_subclass($class))
                }
            }
        )*

        fn register_kinds(m: &Bound<'_, PyModule>) -> PyResult<()> {
            $(m.add_class::<$class>()?;)*
            Ok(())
        }
    };
}

stream_kinds!(
    (
        Score,
        "Score",
        StreamKind::Score,
        -20,
        "music21's `Score`: parts at one time."
    ),
    (
        Part,
        "Part",
        StreamKind::Part,
        -20,
        "music21's `Part`: one voice's line through the piece."
    ),
    (
        PartStaff,
        "PartStaff",
        StreamKind::PartStaff,
        -20,
        "music21's `PartStaff`: one staff of a part written on several."
    ),
    (
        Measure,
        "Measure",
        StreamKind::Measure,
        -20,
        "music21's `Measure`: one bar."
    ),
    (
        Voice,
        "Voice",
        StreamKind::Voice,
        5,
        "music21's `Voice`: one line inside a bar."
    ),
    (
        Opus,
        "Opus",
        StreamKind::Opus,
        -20,
        "music21's `Opus`: a collection of scores."
    ),
);

impl Stream {
    /// The first score whose metadata answers a query in one field, asked
    /// of the metadata itself so that music21's own answers where it is
    /// installed and the package's own where it is not.
    fn score_by<'py>(
        slf: &Bound<'py, Self>,
        query: &Bound<'_, PyAny>,
        field: &str,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        for score in Self::scores(slf)? {
            let Ok(metadata) = score.getattr("metadata") else {
                continue;
            };
            if metadata.is_none() {
                continue;
            }
            let keywords = pyo3::types::PyDict::new(slf.py());
            keywords.set_item("field", field)?;
            let (found, _): (bool, Option<String>) = metadata
                .call_method("search", (query,), Some(&keywords))?
                .extract()?;
            if found {
                return Ok(Some(score));
            }
        }
        Ok(None)
    }
}

/// A stream read into the crate, beside the objects standing at each of the
/// crate stream's positions (`music21_rs::Stream::leaves`).
struct Read {
    stream: RsStream,
    leaves: Vec<Py<PyAny>>,
}

/// Which of music21's classes a stream object is, read off its lineage.
fn kind_of(object: &Bound<'_, PyAny>) -> PyResult<StreamKind> {
    if let Ok(stream) = object.cast::<Stream>() {
        return Ok(stream.borrow().kind);
    }
    for kind in [
        StreamKind::Opus,
        StreamKind::Score,
        StreamKind::PartStaff,
        StreamKind::Part,
        StreamKind::Measure,
        StreamKind::Voice,
    ] {
        if is_of_class(
            object,
            &pyo3::types::PyString::new(object.py(), kind.as_str()),
        )? {
            return Ok(kind);
        }
    }
    Ok(StreamKind::Stream)
}

/// A music21 variant read as the crate holds one.
pub(crate) fn variant_value(
    object: &Bound<'_, PyAny>,
) -> PyResult<music21_rs_crate::variant::Variant> {
    let contents = object.getattr("containedSite")?;
    let mut value = music21_rs_crate::variant::Variant::from_stream(crate_stream(&contents)?);
    for group in object.getattr("groups")?.walk()? {
        value.groups_mut().push(group?.str()?.to_str()?.to_string());
    }
    // music21 keeps the length a caller set apart from the one it works
    // out, which is what `_replacementQuarterLength` says: the crate does
    // the same, so only a length that was set is carried.
    if let Ok(said) = object.getattr("_replacementQuarterLength")
        && !said.is_none()
    {
        value.set_replacement_quarter_length(Some(said.extract()?));
    }
    Ok(value)
}

/// A variant as music21 holds one, where music21 can be asked for its
/// class; the package has none of its own.
pub(crate) fn variant_object<'py>(
    py: Python<'py>,
    variant: &music21_rs_crate::variant::Variant,
    whose: Whose,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let Ok(class) = py
        .import("music21.variant")
        .and_then(|module| module.getattr("Variant"))
    else {
        return Ok(None);
    };
    let made = class.call0()?;
    for event in variant.contents().events() {
        if let Some(object) = element_object_as(py, event.element(), whose)? {
            made.call_method1("insert", (event.offset(), object))?;
        }
    }
    let groups = made.getattr("groups")?;
    for name in variant.groups() {
        groups.call_method1("append", (name,))?;
    }
    // The length it stands in for, which the crate answers as the length
    // of the variant itself where nothing was said.
    made.setattr(
        "replacementQuarterLength",
        variant.replacement_quarter_length(),
    )?;
    Ok(Some(made))
}

/// The crate's value of one element, or `None` for one it has no value for.
fn element_value(object: &Bound<'_, PyAny>, stand_ins: bool) -> PyResult<Option<StreamElement>> {
    let py = object.py();
    // music21's own `variant.Variant`, which holds a stream of the
    // reading it stands in for. The package has no class for one, so a
    // variant is always music21's; what is read of it is its contents,
    // its groups and how much of the stream it replaces.
    if is_of_class(object, &pyo3::types::PyString::new(py, "Variant"))? {
        return Ok(Some(variant_value(object)?.into()));
    }
    // music21's own `braille.objects` divisions, which the package has no
    // class for: where braille starts a new segment.
    if !stand_ins
        && is_of_class(
            object,
            &pyo3::types::PyString::new(py, "BrailleSegmentDivision"),
        )?
    {
        let optional = pyo3::types::PyString::new(py, "BrailleOptionalSegmentDivision");
        let division = match is_of_class(object, &optional)? {
            true => SegmentDivision::Optional,
            false => SegmentDivision::Required,
        };
        return Ok(Some(StreamElement::BrailleSegmentDivision(division)));
    }
    if let Ok(symbol) = object.cast::<ChordSymbol>() {
        return Ok(Some(StreamElement::ChordSymbol(
            crate::harmony::crate_value(symbol)?,
        )));
    }
    if let Ok(chord) = object.extract::<PyRef<'_, Chord>>() {
        let mut value = chord.synced_inner(py);
        drop(chord);
        // A numeral is a chord that says which numeral it is.
        if let Ok(numeral) = object.extract::<PyRef<'_, crate::roman::RomanNumeral>>() {
            value.set_numeral(Some(numeral.inner.clone()));
        }
        // music21 hides a chord's notes one by one.
        if !stand_ins && let Ok(notes) = object.getattr("notes") {
            for (index, note) in notes.walk()?.enumerate() {
                let hidden = hidden_on_print(&note?);
                if let Some(held) = value.notes_mut().get_mut(index) {
                    held.set_hidden(hidden);
                }
            }
        }
        *value.expressions_mut() = expressions_of(object)?;
        *value.articulations_mut() = articulations_of(object)?;
        if !stand_ins && let Some(duration) = written_duration(object, value.duration())? {
            value = value.with_duration(duration);
        }
        if !stand_ins && let Some(duration) = inferred_duration(object, value.duration()) {
            value = value.with_duration(duration);
        }
        return Ok(Some(StreamElement::Chord(value)));
    }
    // music21's own percussion classes, read for a score writer. The walks
    // keep them as the stand-ins they have always been.
    if !stand_ins && is_of_class(object, &pyo3::types::PyString::new(py, "Unpitched"))? {
        return Ok(Some(StreamElement::Unpitched(unpitched_value(object)?)));
    }
    if !stand_ins && is_of_class(object, &pyo3::types::PyString::new(py, "PercussionChord"))? {
        return percussion_chord_value(object).map(|chord| Some(chord.into()));
    }
    if let Ok(note) = object.extract::<PyRef<'_, Note>>() {
        let mut value = note.synced(py);
        drop(note);
        value.set_hidden(hidden_on_print(object));
        *value.expressions_mut() = expressions_of(object)?;
        *value.articulations_mut() = articulations_of(object)?;
        if !stand_ins && let Some(duration) = written_duration(object, value.duration())? {
            value.set_duration(duration);
        }
        if !stand_ins && let Some(duration) = inferred_duration(object, value.duration()) {
            value.set_duration(duration);
        }
        return Ok(Some(StreamElement::Note(value)));
    }
    if let Ok(rest) = object.extract::<PyRef<'_, Rest>>() {
        let mut value = rest.synced(py);
        drop(rest);
        value.set_hidden(hidden_on_print(object));
        if let Some(shift) = object
            .getattr("stepShift")
            .ok()
            .and_then(|shift| shift.extract::<i32>().ok())
        {
            value.set_step_shift(shift);
        }
        let styled = object
            .getattr("hasStyleInformation")
            .and_then(|said| said.is_truthy())
            .unwrap_or(false);
        if styled {
            value.set_size(
                object
                    .getattr("style")
                    .and_then(|style| style.getattr("noteSize"))
                    .ok()
                    .and_then(|size| size.extract::<String>().ok())
                    .and_then(|size| music21_rs_crate::notation::NoteSize::from_name(&size)),
            );
        }
        *value.expressions_mut() = expressions_of(object)?;
        *value.articulations_mut() = articulations_of(object)?;
        if !stand_ins && let Some(duration) = written_duration(object, Some(value.duration()))? {
            value.set_duration(duration);
        }
        if !stand_ins && let Some(duration) = inferred_duration(object, Some(value.duration())) {
            value.set_duration(duration);
        }
        return Ok(Some(StreamElement::Rest(value)));
    }
    if let Ok(dynamic) = object.extract::<PyRef<'_, Dynamic>>() {
        let mut value = dynamic.inner.clone();
        drop(dynamic);
        value.set_placement(placement_of(object));
        return Ok(Some(StreamElement::Dynamic(value)));
    }
    if let Ok(key) = object.extract::<PyRef<'_, crate::key::Key>>() {
        let mut value = key.inner.clone();
        drop(key);
        value.set_color(style_color(object));
        return Ok(Some(StreamElement::Key(value)));
    }
    if let Ok(key) = object.extract::<PyRef<'_, KeySignature>>() {
        let mut value = key.signature.clone();
        drop(key);
        value.set_color(style_color(object));
        return Ok(Some(StreamElement::KeySignature(value)));
    }
    if let Ok(meter) = object.extract::<PyRef<'_, TimeSignature>>() {
        let mut value = meter.inner.clone();
        drop(meter);
        value.set_color(style_color(object));
        value.set_hidden(hidden_on_print(object));
        return Ok(Some(StreamElement::TimeSignature(value)));
    }
    if let Ok(mark) = object.extract::<PyRef<'_, MetronomeMark>>() {
        return Ok(Some(StreamElement::MetronomeMark(mark.inner.clone())));
    }
    // The walks keep a metric modulation as the stand-in it has always been.
    if !stand_ins
        && let Ok(modulation) = object.extract::<PyRef<'_, crate::tempo::MetricModulation>>()
    {
        let mut value = modulation.written_value(py)?;
        drop(modulation);
        value.set_placement(placement_of(object));
        return Ok(Some(StreamElement::from(value)));
    }
    if let Ok(clef) = object.extract::<PyRef<'_, crate::clef::Clef>>() {
        let mut value = clef.inner.clone();
        drop(clef);
        value.set_color(style_color(object));
        return Ok(Some(StreamElement::Clef(value)));
    }
    if is_of_class(object, &pyo3::types::PyString::new(py, "TextExpression"))? {
        let mut text = music21_rs_crate::expressions::TextExpression::new(
            object.getattr("content")?.str()?.to_string(),
        );
        text.set_placement(placement_of(object));
        return Ok(Some(StreamElement::TextExpression(text)));
    }
    if is_of_class(object, &pyo3::types::PyString::new(py, "RepeatExpression"))? {
        let class: String = object.get_type().getattr("__name__")?.extract()?;
        // A subclass of music21's own is read as the kind it derives from.
        let mut kind = music21_rs_crate::RepeatExpressionKind::from_class_name(&class);
        if kind.is_none() {
            for base in object.get_type().getattr("__mro__")?.walk()? {
                let name: String = base?.getattr("__name__")?.extract()?;
                kind = music21_rs_crate::RepeatExpressionKind::from_class_name(&name);
                if kind.is_some() {
                    break;
                }
            }
        }
        let Some(kind) = kind else {
            return Err(StreamException::new_err(format!(
                "a repeat expression of class {class} has no kind this crate knows"
            )));
        };
        let mut mark = music21_rs_crate::RepeatExpression::new(kind);
        if let Ok(text) = object.call_method0("getText")
            && !text.is_none()
        {
            mark.set_text(text.str()?.to_string());
        }
        mark.set_use_symbol(object.getattr("useSymbol")?.is_truthy()?);
        // A sign is placed as the mark is, and words as the text expression
        // music21 writes them from.
        if mark.use_symbol() {
            mark.set_placement(placement_of(object));
        } else if let Ok(expression) = object.call_method0("getTextExpression")
            && !expression.is_none()
        {
            mark.set_placement(placement_of(&expression));
        }
        return Ok(Some(StreamElement::RepeatExpression(mark)));
    }
    if is_of_class(object, &pyo3::types::PyString::new(py, "TempoText"))? {
        let mut text =
            music21_rs_crate::TempoText::new(text_attribute(object, "text").unwrap_or_default());
        // music21 keeps where the words sit on the text expression inside.
        if let Ok(expression) = object.call_method0("getTextExpression")
            && !expression.is_none()
        {
            text.set_placement(placement_of(&expression));
        }
        return Ok(Some(StreamElement::TempoText(text)));
    }
    // A barline standing among a stream's elements.
    if object.cast::<crate::bar::Barline>().is_ok()
        || is_of_class(object, &pyo3::types::PyString::new(py, "Barline"))?
    {
        return Ok(Some(StreamElement::Barline(barline_value(object)?)));
    }
    if is_of_class(object, &pyo3::types::PyString::new(py, "Clef"))? {
        return clef_value(object).map(|clef| Some(StreamElement::Clef(clef)));
    }
    if is_of_class(object, &pyo3::types::PyString::new(py, "Instrument"))? {
        return instrument_value(object).map(|instrument| Some(StreamElement::from(instrument)));
    }
    // What music21 itself built, read off its attributes.
    if !stand_ins && let Some(value) = music21_value(object)? {
        return Ok(Some(value));
    }
    // Something the crate has no value for: a clef, a barline, an
    // instrument, music21's own metre. What the walks read of one is where it
    // stands, how long it lasts and whether it sorts ahead of a note -- a
    // voice does not pair across a clef that stands where its last note
    // started -- so it is held by a stand-in with the same: a rest as long as
    // it is, or a tempo mark where it takes no time. The object itself is
    // still the one listed at that position. One that takes no time and
    // sorts with the notes, a spanner music21 keeps at the start of a part,
    // takes part in no walk and is left out.
    if !stand_ins {
        if is_of_class(object, &pyo3::types::PyString::new(py, "PedalObject"))? {
            let class: String = object.get_type().getattr("__name__")?.extract()?;
            let mut kind = music21_rs_crate::PedalObjectKind::from_class_name(&class);
            if kind.is_none() {
                for base in object.get_type().getattr("__mro__")?.walk()? {
                    let name: String = base?.getattr("__name__")?.extract()?;
                    kind = music21_rs_crate::PedalObjectKind::from_class_name(&name);
                    if kind.is_some() {
                        break;
                    }
                }
            }
            // music21's own base class, which is none of the three, writes
            // nothing.
            let Some(kind) = kind else {
                return Ok(None);
            };
            let mut value = music21_rs_crate::PedalObject::new(kind);
            value.set_placement(placement_of(object));
            return Ok(Some(StreamElement::PedalObject(value)));
        }
        // music21's volpiano breaks.
        for kind in [
            music21_rs_crate::volpiano::Break::Line,
            music21_rs_crate::volpiano::Break::Page,
            music21_rs_crate::volpiano::Break::Column,
        ] {
            if is_of_class(object, &pyo3::types::PyString::new(py, kind.class_name()))? {
                return Ok(Some(StreamElement::Break(kind)));
            }
        }
        if let Some(layout) = layout_value(object)? {
            return Ok(Some(StreamElement::Layout(layout)));
        }
        if is_of_class(object, &pyo3::types::PyString::new(py, "TextBox"))? {
            return Ok(Some(StreamElement::TextBox(Box::new(text_box_value(
                object,
            )?))));
        }
        if is_of_class(object, &pyo3::types::PyString::new(py, "RehearsalMark"))? {
            // music21 writes whatever the mark holds as text.
            let mut mark = music21_rs_crate::expressions::RehearsalMark::new(
                object.getattr("content")?.str()?.to_string(),
            );
            let styled = object
                .getattr("hasStyleInformation")
                .and_then(|said| said.is_truthy())
                .unwrap_or(false);
            if styled && let Ok(style) = object.getattr("style") {
                mark.set_placement(placement_of(&style));
                mark.set_enclosure(
                    style
                        .getattr("enclosure")
                        .ok()
                        .filter(|enclosure| !enclosure.is_none())
                        .and_then(|enclosure| enclosure.str().ok())
                        .map(|enclosure| enclosure.to_string()),
                );
            }
            return Ok(Some(StreamElement::RehearsalMark(mark)));
        }
        // Read for a score writer, something a score writes out as a note or
        // a direction cannot be left out without the score saying less.
        for written in ["GeneralNote", "Expression", "TempoIndication", "Dynamic"] {
            if is_of_class(object, &pyo3::types::PyString::new(py, written))? {
                let class: String = object.get_type().getattr("__name__")?.extract()?;
                return Err(StreamException::new_err(format!(
                    "a {class} cannot be read into a score yet"
                )));
            }
        }
        return Ok(None);
    }
    let length = length_of(object)?;
    if length > 0.0 {
        let duration = music21_rs_crate::Duration::new(length)
            .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
        return Ok(Some(StreamElement::Rest(music21_rs_crate::Rest::new(
            duration,
        ))));
    }
    if sort_order(object) < 20 {
        return Ok(Some(StreamElement::MetronomeMark(
            music21_rs_crate::MetronomeMark::new(60.0),
        )));
    }
    Ok(None)
}

/// A music21 `Unpitched`, read off its attributes.
fn unpitched_value(object: &Bound<'_, PyAny>) -> PyResult<music21_rs_crate::Unpitched> {
    let step: String = object.getattr("displayStep")?.str()?.to_string();
    let octave: i32 = object.getattr("displayOctave")?.extract()?;
    let mut stroke =
        music21_rs_crate::Unpitched::displayed_at(step.chars().next().unwrap_or('B'), octave)
            .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
    read_written(object, stroke.written_mut())?;
    Ok(stroke)
}

/// A music21 `PercussionChord`, read off its attributes: each member as the
/// stroke or the note it is, and what is written of the chord as a whole.
fn percussion_chord_value(
    object: &Bound<'_, PyAny>,
) -> PyResult<music21_rs_crate::PercussionChord> {
    use music21_rs_crate::PercussionNote;
    let py = object.py();
    let mut members = Vec::new();
    for member in object.getattr("notes")?.walk()? {
        let member = member?;
        if let Ok(note) = member.extract::<PyRef<'_, Note>>() {
            members.push(PercussionNote::Note(note.synced(py)));
        } else {
            members.push(PercussionNote::Unpitched(unpitched_value(&member)?));
        }
    }
    let mut chord = music21_rs_crate::PercussionChord::new(members)
        .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
    // The chord's own notation, read onto a note and carried across.
    let mut carrier = music21_rs_crate::Note::from_name("C4")
        .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
    read_written(object, &mut carrier)?;
    let written = chord.written_mut();
    if let Some(duration) = carrier.duration() {
        written.set_duration(duration.clone());
    }
    written.set_stem_direction(carrier.stem_direction());
    written.set_beams(carrier.beams().clone());
    written.set_color(carrier.color().map(str::to_string));
    written.set_stored_instrument(carrier.stored_instrument().cloned());
    if carrier.has_volume_information() {
        written.set_volume(Some(carrier.volume()));
    }
    *written.expressions_mut() = carrier.expressions().to_vec();
    *written.articulations_mut() = carrier.articulations().to_vec();
    Ok(chord)
}

/// Reads what music21 writes about one of its own `NotRest` objects -- its
/// length, stem, notehead, beams, tie, lyrics, volume, colour and marks --
/// onto a note.
fn read_written(object: &Bound<'_, PyAny>, note: &mut music21_rs_crate::Note) -> PyResult<()> {
    use music21_rs_crate::notation::{Notehead, StemDirection};
    let py = object.py();
    let failed =
        |error: music21_rs_crate::Error| StreamException::new_err(crate::pitch::message(&error));
    if let Ok(duration) = object.getattr("duration") {
        if let Some(value) = crate::duration::duration_value_of(py, &duration.clone().unbind()) {
            note.set_duration(value);
        }
        if let Some(written) = written_duration(object, note.duration())? {
            note.set_duration(written);
        }
        if let Some(inferred) = inferred_duration(object, note.duration()) {
            note.set_duration(inferred);
        }
    }
    if let Some(name) = text_attribute(object, "stemDirection") {
        note.set_stem_direction(StemDirection::from_name(&name).map_err(failed)?);
    }
    if let Some(name) = text_attribute(object, "notehead") {
        note.set_notehead(Notehead::from_name(&name).map_err(failed)?);
    }
    if let Ok(fill) = object.getattr("noteheadFill") {
        note.set_notehead_fill(fill.extract::<bool>().ok());
    }
    if let Ok(parenthesis) = object.getattr("noteheadParenthesis") {
        note.set_notehead_parenthesis(parenthesis.is_truthy()?);
    }
    if let Ok(beams) = object.getattr("beams") {
        if let Ok(mut ours) = beams.extract::<PyRefMut<'_, crate::notation::Beams>>() {
            note.set_beams(ours.settled_value(py));
        } else if let Some(read) = music21_beams(&beams)? {
            note.set_beams(read);
        }
    }
    if let Ok(tie) = object.getattr("tie")
        && !tie.is_none()
    {
        if let Ok(ours) = tie.extract::<PyRef<'_, crate::notation::Tie>>() {
            note.set_tie(Some(ours.inner.clone()));
        } else if let Some(read) = music21_tie(&tie)? {
            note.set_tie(Some(read));
        }
    }
    if let Ok(lyrics) = object.getattr("lyrics") {
        let mut verses = Vec::new();
        for lyric in lyrics.walk()? {
            let lyric = lyric?;
            if let Ok(ours) = lyric.extract::<PyRef<'_, crate::notation::Lyric>>() {
                verses.push(ours.synced(py));
            } else {
                verses.push(music21_lyric(&lyric)?);
            }
        }
        *note.lyrics_mut() = verses;
    }
    let has_volume = object
        .call_method0("hasVolumeInformation")
        .and_then(|said| said.is_truthy())
        .unwrap_or(false);
    if has_volume && let Ok(volume) = object.getattr("volume") {
        if let Ok(ours) = volume.extract::<PyRef<'_, crate::notation::Volume>>() {
            note.set_volume(Some(ours.inner.clone()));
        } else {
            note.set_volume(Some(music21_volume(&volume)?));
        }
    }
    if let Ok(stored) = object.getattr("storedInstrument")
        && !stored.is_none()
    {
        note.set_stored_instrument(Some(instrument_value(&stored)?));
    }
    note.set_color(style_color(object));
    note.set_hidden(hidden_on_print(object));
    // Whether it is drawn cue-sized or full, which music21 keeps on the
    // style and its writer writes on the note's written value.
    if object
        .getattr("hasStyleInformation")
        .and_then(|said| said.is_truthy())
        .unwrap_or(false)
        && let Ok(style) = object.getattr("style")
        && let Some(size) = text_attribute(&style, "noteSize")
    {
        note.set_size(music21_rs_crate::notation::NoteSize::from_name(&size));
    }
    *note.expressions_mut() = expressions_of(object)?;
    *note.articulations_mut() = articulations_of(object)?;
    Ok(())
}

/// A tie music21 built: which end of the tie the note is, and how the
/// line over it is drawn.
fn music21_tie(object: &Bound<'_, PyAny>) -> PyResult<Option<music21_rs_crate::notation::Tie>> {
    use music21_rs_crate::notation::{Tie as RsTie, TieStyle};
    let failed =
        |error: music21_rs_crate::Error| StreamException::new_err(crate::pitch::message(&error));
    let Some(kind) = text_attribute(object, "type") else {
        return Ok(None);
    };
    let mut tie = RsTie::from_name(&kind).map_err(failed)?;
    if let Some(style) = text_attribute(object, "style") {
        tie.set_style(TieStyle::from_name(&style).map_err(failed)?);
    }
    tie.set_placement(placement_of(object));
    Ok(Some(tie))
}

/// A lyric music21 built: what is sung, which verse it belongs to, and
/// whether it is a whole word or part of one.
fn music21_lyric(object: &Bound<'_, PyAny>) -> PyResult<music21_rs_crate::notation::Lyric> {
    use music21_rs_crate::notation::{Lyric as RsLyric, Syllabic};
    let failed =
        |error: music21_rs_crate::Error| StreamException::new_err(crate::pitch::message(&error));
    // Built unsung and then given its text, which is not the same as
    // `Lyric::new`: that *says* the syllabic, and the crate writes a
    // `<syllabic>` only for one that was said. music21 keeps the same
    // difference between `_syllabic` and the `syllabic` property, so a bare
    // `Lyric()` -- the empty one music21 numbers beside a real verse --
    // writes none on either side.
    let mut lyric = RsLyric::unsung();
    match object
        .getattr("text")
        .ok()
        .filter(|text| !text.is_none())
        .and_then(|text| text.extract::<String>().ok())
    {
        Some(text) => lyric.set_text(text),
        // `lyric.text = None` takes even the empty string away.
        None => lyric.clear_text(),
    }
    if let Some(number) = object
        .getattr("number")
        .ok()
        .and_then(|number| number.extract::<i32>().ok())
    {
        lyric.set_number(number);
    }
    // `_syllabic` and not `syllabic`: the property *works one out* where
    // none was set -- `single` for a word with no hyphens, `composite` for
    // a lyric in parts -- and a score writer must write only what the
    // document said, as this package's own lyric holds only that.
    if let Some(syllabic) = text_attribute(object, "_syllabic") {
        lyric.set_syllabic(Syllabic::from_name(&syllabic).map_err(failed)?);
    }
    if let Some(elision) = text_attribute(object, "elisionBefore") {
        lyric.set_elision_before(elision);
    }
    lyric.set_identifier(
        object
            .getattr("_identifier")
            .ok()
            .filter(|held| !held.is_none())
            .and_then(|held| held.extract::<String>().ok()),
    );
    lyric.set_color(style_color(object));
    // A lyric keeps how it is justified and where it sits on its *style*,
    // not as attributes of its own as a note's articulations do.
    if let Ok(style) = object.getattr("style") {
        if let Some(name) = text_attribute(&style, "justify") {
            lyric.set_justify(music21_rs_crate::notation::Justification::from_name(&name).ok());
        }
        lyric.set_placement(placement_of(&style));
    }
    lyric.set_hidden(hidden_on_print(object));
    // A lyric sung to more than one syllable keeps them as components, and
    // music21 writes an elision between each pair.
    if let Ok(components) = object.getattr("components")
        && !components.is_none()
    {
        let mut parts = Vec::new();
        for component in components.walk()? {
            parts.push(music21_lyric(&component?)?);
        }
        if !parts.is_empty() {
            lyric.set_components(parts);
        }
    }
    Ok(lyric)
}

/// A volume music21 built: how loud the note is played.
fn music21_volume(object: &Bound<'_, PyAny>) -> PyResult<music21_rs_crate::Volume> {
    let mut volume = music21_rs_crate::Volume::new();
    volume.set_velocity(
        object
            .getattr("velocity")
            .ok()
            .filter(|velocity| !velocity.is_none())
            .and_then(|velocity| velocity.extract::<i32>().ok()),
    );
    if let Ok(relative) = object.getattr("velocityIsRelative") {
        volume.set_velocity_is_relative(relative.is_truthy()?);
    }
    Ok(volume)
}

/// The beams music21 wrote on a note, each with what it does and which way
/// it is drawn. Nothing where none are written.
fn music21_beams(object: &Bound<'_, PyAny>) -> PyResult<Option<music21_rs_crate::notation::Beams>> {
    use music21_rs_crate::notation::{Beam as RsBeam, BeamDirection, BeamType, Beams as RsBeams};
    let Ok(held) = object.getattr("beamsList") else {
        return Ok(None);
    };
    let mut beams = RsBeams::default();
    for beam in held.walk()? {
        let beam = beam?;
        let kind =
            text_attribute(&beam, "type").and_then(|name| BeamType::from_music21_name(&name));
        let direction = text_attribute(&beam, "direction")
            .and_then(|name| BeamDirection::from_music21_name(&name));
        let mut read = RsBeam::new(kind, direction);
        read.set_number(
            beam.getattr("number")
                .ok()
                .and_then(|number| number.extract::<u32>().ok()),
        );
        beams.beams_mut().push(read);
    }
    if beams.beams().is_empty() {
        return Ok(None);
    }
    Ok(Some(beams))
}

/// An attribute as text, where it is set and is text.
fn text_attribute(object: &Bound<'_, PyAny>, name: &str) -> Option<String> {
    object
        .getattr(name)
        .ok()
        .filter(|value| !value.is_none())
        .and_then(|value| value.extract::<String>().ok())
}

/// An attribute as a small whole number, where it is set.
fn u8_attribute(object: &Bound<'_, PyAny>, name: &str) -> Option<u8> {
    object
        .getattr(name)
        .ok()
        .filter(|value| !value.is_none())
        .and_then(|value| value.extract::<u8>().ok())
}

/// A duration with the tuplets its object was set to written into it, as a
/// score writer needs them: the facade keeps set tuplets as Python objects,
/// which carry how each is bracketed. Nothing where none were set.
fn written_duration(
    object: &Bound<'_, PyAny>,
    duration: Option<&music21_rs_crate::Duration>,
) -> PyResult<Option<music21_rs_crate::Duration>> {
    let py = object.py();
    let Ok(held) = object.getattr("duration") else {
        return Ok(None);
    };
    if let Some(grace) = crate::duration::grace_value(&held)? {
        return Ok(Some(grace));
    }
    let written = match held.extract::<PyRef<'_, crate::duration::Duration>>() {
        Ok(ours) => ours.written_value(py),
        // A duration music21 itself built: its components say how it is
        // written and its tuplets how they are bracketed, which is the
        // whole of what a score writer reads off one.
        Err(_) => music21_written_duration(&held)?,
    };
    let Some(mut written) = written else {
        return Ok(None);
    };
    let length = duration.map_or(1.0, music21_rs_crate::Duration::quarter_length);
    // A score may say a note lasts other than what it is written as -- a
    // file's durations are rounded to its divisions -- and music21 keeps
    // both, writing the one as a length and the other as a note value.
    if (written.quarter_length() - length).abs() > 1e-9 * length.max(1.0) {
        written.set_linked(false);
        written
            .set_quarter_length(length)
            .map_err(|error| StreamException::new_err(error.to_string()))?;
    }
    Ok(Some(written))
}

/// How a duration music21 built is *written*: its note values and the
/// tuplets they are written inside.
///
/// The crate works a duration's values out from its length until something
/// says them, so a score writer has to be told them -- which for this
/// package's own duration is `Duration::written_value` over the Python
/// tuplet objects it keeps, and for music21's is the same two lists read
/// off `components` and `tuplets`.
fn music21_written_duration(
    held: &Bound<'_, PyAny>,
) -> PyResult<Option<music21_rs_crate::Duration>> {
    use music21_rs_crate::DurationType as RsType;
    let Ok(components) = held.getattr("components") else {
        return Ok(None);
    };
    let mut written = music21_rs_crate::Duration::default();
    written.clear();
    written.set_tuplets(Vec::new());
    let mut any = false;
    for component in components.walk()? {
        let component = component?;
        let Some(kind) =
            text_attribute(&component, "type").and_then(|name| RsType::from_music21_name(&name))
        else {
            // A piece cut from a longer value may have no type at all,
            // which is music21's `inexpressible` and nothing to write.
            return Ok(None);
        };
        let dots = u8_attribute(&component, "dots").unwrap_or(0);
        written.add_duration_tuple(kind, u32::from(dots));
        any = true;
    }
    if !any {
        return Ok(None);
    }
    let mut tuplets = Vec::new();
    if let Ok(held_tuplets) = held.getattr("tuplets") {
        for tuplet in held_tuplets.walk()? {
            tuplets.push(music21_tuplet(&tuplet?)?);
        }
    }
    written.set_tuplets(tuplets);
    Ok(Some(written))
}

/// A music21 tuplet: so many notes in the time of so many, and how the
/// bracket over them is drawn.
fn music21_tuplet(object: &Bound<'_, PyAny>) -> PyResult<music21_rs_crate::Tuplet> {
    use music21_rs_crate::{
        DurationType as RsType, Tuplet as RsTuplet, TupletBracket, TupletShow, TupletType,
    };
    let failed =
        |error: music21_rs_crate::Error| StreamException::new_err(crate::pitch::message(&error));
    let written = |name: &str| -> PyResult<Option<(RsType, u32)>> {
        let Ok(held) = object.getattr(name) else {
            return Ok(None);
        };
        if held.is_none() {
            return Ok(None);
        }
        let Some(kind) =
            text_attribute(&held, "type").and_then(|name| RsType::from_music21_name(&name))
        else {
            return Ok(None);
        };
        Ok(Some((
            kind,
            u32::from(u8_attribute(&held, "dots").unwrap_or(0)),
        )))
    };
    let actual = object
        .getattr("numberNotesActual")
        .and_then(|count| count.extract::<u32>())
        .unwrap_or(3);
    let normal = object
        .getattr("numberNotesNormal")
        .and_then(|count| count.extract::<u32>())
        .unwrap_or(2);
    let (kind, dots) = written("durationActual")?.unwrap_or((RsType::Eighth, 0));
    let mut tuplet = RsTuplet::new(actual, normal, kind, dots);
    tuplet.set_duration_actual(written("durationActual")?);
    tuplet.set_duration_normal(written("durationNormal")?);
    if let Some(name) = text_attribute(object, "type") {
        tuplet.set_tuplet_type(Some(TupletType::from_name(&name).map_err(failed)?));
    }
    // music21 says a bracket with `True`, `False` or the word `slur`, which
    // is what this package's own setter takes.
    if let Ok(bracket) = object.getattr("bracket") {
        tuplet.set_bracket(match bracket.extract::<String>().as_deref() {
            Ok("slur") => TupletBracket::Slur,
            _ if bracket.is_truthy()? => TupletBracket::Bracket,
            _ => TupletBracket::None,
        });
    }
    tuplet.set_placement(placement_of(object));
    for (name, set) in [("tupletActualShow", true), ("tupletNormalShow", false)] {
        let show = text_attribute(object, name)
            .map(|name| TupletShow::from_name(&name))
            .transpose()
            .map_err(failed)?;
        if set {
            tuplet.set_actual_show(show);
        } else {
            tuplet.set_normal_show(show);
        }
    }
    Ok(tuplet)
}

/// A duration saying, as its object's does, whether the way it is written
/// was worked out from its length: music21's `expressionIsInferred`, which
/// making notation asks before it writes a length another way. Nothing
/// where the duration says so already.
fn inferred_duration(
    object: &Bound<'_, PyAny>,
    duration: Option<&music21_rs_crate::Duration>,
) -> Option<music21_rs_crate::Duration> {
    let inferred = object
        .getattr("duration")
        .ok()?
        .getattr("expressionIsInferred")
        .ok()?
        .extract::<bool>()
        .ok()?;
    let mut duration = duration.cloned().unwrap_or_default();
    if duration.expression_is_inferred() == inferred {
        return None;
    }
    duration.set_expression_is_inferred(inferred);
    Some(duration)
}

/// A note's or chord's `articulations`, which are this wheel's own.
fn articulations_of(object: &Bound<'_, PyAny>) -> PyResult<Vec<music21_rs_crate::Articulation>> {
    let mut out = Vec::new();
    let Ok(articulations) = object.getattr("articulations") else {
        return Ok(out);
    };
    for articulation in articulations.walk()? {
        let articulation = articulation?;
        if let Ok(ours) = articulation.extract::<PyRef<'_, crate::articulations::Articulation>>() {
            out.push(ours.inner.clone());
        } else if let Some(read) = music21_articulation(&articulation)? {
            out.push(read);
        }
    }
    Ok(out)
}

/// A note's or chord's `expressions`, those the crate carries.
///
/// An ornament is this wheel's own; a fermata and an arpeggio mark are
/// music21's, read off their attributes. Anything else music21 files there
/// is left out, and a MusicXML writer does not write it.
fn expressions_of(
    object: &Bound<'_, PyAny>,
) -> PyResult<Vec<music21_rs_crate::expressions::Expression>> {
    use music21_rs_crate::expressions::{ArpeggioType, Expression, Fermata, FermataType};
    let py = object.py();
    let mut out = Vec::new();
    let Ok(expressions) = object.getattr("expressions") else {
        return Ok(out);
    };
    for expression in expressions.walk()? {
        let expression = expression?;
        if let Ok(ornament) = expression.extract::<PyRef<'_, crate::expressions::Ornament>>() {
            out.push(Expression::Ornament(Box::new(ornament.inner.clone())));
        } else if let Some(read) = music21_ornament(&expression)? {
            out.push(Expression::Ornament(Box::new(read)));
        } else if is_of_class(&expression, &pyo3::types::PyString::new(py, "Fermata"))? {
            let mut fermata = Fermata::new();
            fermata.set_fermata_type(
                if text_attribute(&expression, "type").as_deref() == Some("upright") {
                    FermataType::Upright
                } else {
                    FermataType::Inverted
                },
            );
            fermata.set_shape(text_attribute(&expression, "shape"));
            out.push(Expression::Fermata(fermata));
        } else if is_of_class(&expression, &pyo3::types::PyString::new(py, "ArpeggioMark"))? {
            let kind = text_attribute(&expression, "type").unwrap_or_default();
            let arpeggio = ArpeggioType::from_name(&kind)
                .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
            out.push(Expression::Arpeggio(arpeggio));
        }
    }
    Ok(out)
}

/// The colour music21 keeps on an object's style, where it has one.
fn style_color(object: &Bound<'_, PyAny>) -> Option<String> {
    let styled = object
        .getattr("hasStyleInformation")
        .and_then(|said| said.is_truthy())
        .unwrap_or(false);
    if !styled {
        return None;
    }
    text_attribute(&object.getattr("style").ok()?, "color").filter(|color| !color.is_empty())
}

/// Whether music21 leaves an object off the printed page.
fn hidden_on_print(object: &Bound<'_, PyAny>) -> bool {
    let styled = object
        .getattr("hasStyleInformation")
        .and_then(|said| said.is_truthy())
        .unwrap_or(false);
    styled
        && object
            .getattr("style")
            .and_then(|style| style.getattr("hideObjectOnPrint"))
            .and_then(|hidden| hidden.is_truthy())
            .unwrap_or(false)
}

/// An object's `placement`, where it is `above` or `below`.
fn placement_of(object: &Bound<'_, PyAny>) -> Option<music21_rs_crate::Placement> {
    text_attribute(object, "placement")
        .and_then(|name| music21_rs_crate::Placement::from_name(&name).ok())
}

/// An articulation music21 itself built, read by the name of its class.
///
/// This package's articulations are not installed -- music21's own tables
/// hold its classes and its readers look them up there -- so a score holds
/// music21's whenever nothing stands in for them.
fn music21_articulation(
    object: &Bound<'_, PyAny>,
) -> PyResult<Option<music21_rs_crate::Articulation>> {
    use music21_rs_crate::articulations::ArticulationKind;
    let class: String = object.get_type().getattr("__name__")?.extract()?;
    let Some(kind) = ArticulationKind::from_class_name(&class) else {
        return Ok(None);
    };
    let mut read = music21_rs_crate::Articulation::of_kind(kind);
    read.set_placement(text_attribute(object, "placement"));
    // The fields past the base ones, each where music21's class for the
    // kind carries it (`ArticulationKind::has_field`) -- which is the same
    // list the install helper reads, so a field added to one is added to
    // both. A `<other-articulation>` writes its `displayText`, which is
    // what a mark music21 has no class of its own for comes back as.
    read.set_display_text(text_attribute(object, "displayText"));
    if kind.has_field("pointDirection") {
        read.set_point_direction(text_attribute(object, "pointDirection"));
    }
    if kind.has_field("symbol") {
        read.set_symbol(text_attribute(object, "symbol"));
    }
    if kind.has_field("harmonicType") {
        read.set_harmonic_type(text_attribute(object, "harmonicType"));
    }
    if kind.has_field("pitchType") {
        read.set_pitch_type(text_attribute(object, "pitchType"));
    }
    if kind.has_field("withBar") {
        read.set_with_bar(text_attribute(object, "withBar"));
    }
    if kind.has_field("number")
        && let Some(number) = object
            .getattr("number")
            .ok()
            .filter(|held| !held.is_none())
            .and_then(|held| held.extract::<i32>().ok())
    {
        read.set_number(number);
    }
    if kind.has_field("substitution") {
        read.set_substitution(
            object
                .getattr("substitution")
                .and_then(|said| said.is_truthy())
                .unwrap_or(false),
        );
    }
    if kind.has_field("alternate") {
        read.set_alternate(
            object
                .getattr("alternate")
                .and_then(|said| said.is_truthy())
                .unwrap_or(false),
        );
    }
    if kind.has_field("preBend") {
        read.set_pre_bend(
            object
                .getattr("preBend")
                .and_then(|said| said.is_truthy())
                .unwrap_or(false),
        );
    }
    if kind.has_field("release") {
        read.set_release(
            object
                .getattr("release")
                .ok()
                .filter(|held| !held.is_none())
                .and_then(|held| held.extract::<f64>().ok()),
        );
    }
    if kind.has_field("bendAlter")
        && let Ok(alter) = object.getattr("bendAlter")
        && !alter.is_none()
        && let Ok(semitones) = alter
            .getattr("semitones")
            .and_then(|semitones| semitones.extract::<f64>())
    {
        read.set_bend_alter(Some(
            music21_rs_crate::interval::ChromaticInterval::new(semitones)
                .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?,
        ));
    }
    if let Some(finger) = text_attribute(object, "fingerNumber") {
        read.set_finger(Some(music21_rs_crate::articulations::Finger::from_written(
            &finger,
        )));
    } else if let Some(number) = object
        .getattr("fingerNumber")
        .ok()
        .filter(|held| !held.is_none())
        .and_then(|held| held.extract::<i32>().ok())
    {
        read.set_finger(Some(music21_rs_crate::articulations::Finger::Number(
            number,
        )));
    }
    Ok(Some(read))
}

/// An ornament music21 itself built, read by the name of its class and the
/// size it is played at.
fn music21_ornament(
    object: &Bound<'_, PyAny>,
) -> PyResult<Option<music21_rs_crate::expressions::Ornament>> {
    use music21_rs_crate::expressions::{Ornament, OrnamentKind};
    let class: String = object.get_type().getattr("__name__")?.extract()?;
    let Some(kind) = OrnamentKind::from_class_name(&class) else {
        return Ok(None);
    };
    let mut read = Ornament::of_kind(kind);
    read.set_placement(text_attribute(object, "placement"));
    Ok(Some(read))
}

/// A music21 clef this wheel does not hold, read off its attributes.
fn clef_value(object: &Bound<'_, PyAny>) -> PyResult<music21_rs_crate::clef::Clef> {
    use music21_rs_crate::clef::{Clef as RsClef, ClefKind};
    let class: String = object.get_type().getattr("__name__")?.extract()?;
    let mut clef = ClefKind::from_class_name(&class).map_or_else(RsClef::default, RsClef::of_kind);
    clef.set_sign(text_attribute(object, "sign"));
    clef.set_line(u8_attribute(object, "line"));
    clef.set_color(style_color(object));
    if let Some(change) = object
        .getattr("octaveChange")
        .ok()
        .and_then(|value| value.extract::<i32>().ok())
    {
        clef.set_octave_change(change);
    }
    Ok(clef)
}

/// An element music21 itself built, read off its attributes.
///
/// Every branch above reads an element by *casting to this package's own
/// class*, which an installed class passes and a plain music21 object does
/// not -- so a caller who has music21 beside the wheel and has installed
/// nothing (`pip install music21 music21-rs`, and the wheel's own tests
/// where music21 is importable) could not write a score music21 parsed at
/// all: the note fell through to the refusal at the end of this function.
///
/// So each kind that had only a cast is read here by what music21 keeps on
/// it. It runs *after* every branch above, so an installed object still
/// takes the richer path through its own class and nothing that worked
/// before reads differently. It is not asked on the walks' side
/// (`stand_ins`), where music21's own objects have always been held by the
/// stand-ins the walks were verified with.
fn music21_value(object: &Bound<'_, PyAny>) -> PyResult<Option<StreamElement>> {
    let py = object.py();
    let named = |name: &str| is_of_class(object, &pyo3::types::PyString::new(py, name));
    // A chord symbol is a chord before it is a symbol, so it is asked first.
    if named("ChordSymbol")? {
        return Ok(Some(StreamElement::ChordSymbol(
            crate::harmony::read_music21(object)?,
        )));
    }
    if named("Chord")? {
        return Ok(Some(StreamElement::Chord(music21_chord(object)?)));
    }
    if named("Note")? {
        return Ok(Some(StreamElement::Note(music21_note(object)?)));
    }
    if named("Rest")? {
        return Ok(Some(StreamElement::Rest(music21_rest(object)?)));
    }
    let failed =
        |error: music21_rs_crate::Error| StreamException::new_err(crate::pitch::message(&error));
    // A key is a key signature before it is a key, so it is asked first.
    if named("Key")? {
        let tonic = object
            .getattr("tonic")?
            .getattr("name")?
            .extract::<String>()?;
        let mode = object
            .getattr("mode")
            .ok()
            .filter(|mode| !mode.is_none())
            .and_then(|mode| mode.extract::<String>().ok());
        let mut key =
            music21_rs_crate::Key::from_tonic_mode(&tonic, mode.as_deref()).map_err(failed)?;
        key.set_color(style_color(object));
        return Ok(Some(StreamElement::Key(key)));
    }
    if named("KeySignature")? {
        let mut signature = match object
            .getattr("isNonTraditional")
            .and_then(|said| said.is_truthy())
        {
            Ok(true) => {
                let mut altered = Vec::new();
                for pitch in object.getattr("alteredPitches")?.walk()? {
                    altered.push(crate::pitch::pitch_from_any(&pitch?)?);
                }
                music21_rs_crate::KeySignature::from_altered_pitches(altered)
            }
            _ => music21_rs_crate::KeySignature::new(
                object.getattr("sharps")?.extract::<i32>().unwrap_or(0),
            ),
        };
        signature.set_color(style_color(object));
        return Ok(Some(StreamElement::KeySignature(signature)));
    }
    if named("TimeSignature")? {
        let ratio = object.getattr("ratioString")?.extract::<String>()?;
        let mut meter =
            music21_rs_crate::meter::TimeSignature::from_ratio_string(&ratio).map_err(failed)?;
        // `common` and `cut`, which music21 draws in place of the numbers.
        meter.set_symbol(text_attribute(object, "symbol"));
        meter.set_color(style_color(object));
        meter.set_hidden(hidden_on_print(object));
        return Ok(Some(StreamElement::TimeSignature(meter)));
    }
    if named("Dynamic")? {
        let mut dynamic =
            music21_rs_crate::dynamics::Dynamic::new(object.getattr("value")?.extract::<String>()?);
        dynamic.set_placement(placement_of(object));
        return Ok(Some(StreamElement::Dynamic(dynamic)));
    }
    // A metric modulation is two marks, and is asked for before a mark.
    if named("MetricModulation")? {
        if let Some(value) = music21_modulation(object)? {
            return Ok(Some(StreamElement::from(value)));
        }
        return Ok(None);
    }
    if named("MetronomeMark")? {
        return Ok(Some(StreamElement::MetronomeMark(music21_mark(object)?)));
    }
    Ok(None)
}

/// A music21 metronome mark: what it says, how fast, and in what unit.
///
/// music21 implies each half from the other -- a word within two beats of a
/// number, a number from a word -- and says which it was *given* through
/// `numberImplicit` and `textImplicit`, so only what was given is read and
/// the crate implies the rest as music21 does.
fn music21_mark(object: &Bound<'_, PyAny>) -> PyResult<music21_rs_crate::tempo::MetronomeMark> {
    use music21_rs_crate::tempo::MetronomeMark as RsMark;
    let implicit = |name: &str| {
        object
            .getattr(name)
            .and_then(|said| said.is_truthy())
            .unwrap_or(false)
    };
    let number = (!implicit("numberImplicit"))
        .then(|| {
            object
                .getattr("number")
                .ok()
                .filter(|number| !number.is_none())
                .and_then(|number| number.extract::<f64>().ok())
        })
        .flatten();
    let text = (!implicit("textImplicit"))
        .then(|| text_attribute(object, "text"))
        .flatten();
    let mut mark = match (number, text) {
        (Some(number), Some(text)) => RsMark::with_number_and_text(number, text),
        (Some(number), None) => RsMark::new(number),
        (None, Some(text)) => RsMark::from_text(text),
        (None, None) => RsMark::default(),
    };
    if let Ok(referent) = object.getattr("referent")
        && let Ok(length) = referent
            .getattr("quarterLength")
            .and_then(|length| length.extract::<f64>())
        && let Ok(duration) = music21_rs_crate::Duration::new(length)
    {
        mark.set_referent(duration);
    }
    mark.set_placement(placement_of(object));
    // How fast it is played where that is not what it says.
    if let Ok(sounding) = object.getattr("numberSounding")
        && !sounding.is_none()
        && let Ok(sounding) = sounding.extract::<f64>()
        && mark.quarter_bpm().is_none()
    {
        mark.set_number_sounding(Some(sounding));
    }
    Ok(mark)
}

/// A music21 metric modulation: the marks either side of it, each read with
/// no number where the document gave none, which is what a file gives.
fn music21_modulation(
    object: &Bound<'_, PyAny>,
) -> PyResult<Option<music21_rs_crate::tempo::MetricModulation>> {
    let mut modulation = music21_rs_crate::tempo::MetricModulation::new();
    for (name, set) in [("oldMetronome", true), ("newMetronome", false)] {
        let Ok(held) = object.getattr(name) else {
            continue;
        };
        if held.is_none() {
            continue;
        }
        let mark = music21_mark(&held)?;
        if set {
            modulation.set_old_metronome(Some(mark));
        } else {
            modulation.set_new_metronome(Some(mark));
        }
    }
    Ok(Some(modulation))
}

/// A music21 note: the pitch it stands on and everything written on it.
fn music21_note(object: &Bound<'_, PyAny>) -> PyResult<music21_rs_crate::Note> {
    let pitch = crate::pitch::pitch_from_any(&object.getattr("pitch")?)?;
    let mut note = music21_rs_crate::Note::from_pitch(pitch);
    read_written(object, &mut note)?;
    Ok(note)
}

/// A music21 rest: how long it lasts, where it is drawn and what is on it.
fn music21_rest(object: &Bound<'_, PyAny>) -> PyResult<music21_rs_crate::Rest> {
    let mut carrier = music21_rs_crate::Note::from_name("C4")
        .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
    read_written(object, &mut carrier)?;
    let mut rest = music21_rs_crate::Rest::new(carrier.duration().cloned().unwrap_or_default());
    rest.set_hidden(carrier.hidden());
    *rest.expressions_mut() = carrier.expressions().to_vec();
    *rest.articulations_mut() = carrier.articulations().to_vec();
    if let Some(tie) = carrier.tie() {
        rest.set_tie(Some(tie.clone()));
    }
    *rest.lyrics_mut() = carrier.lyrics().to_vec();
    if let Some(shift) = object
        .getattr("stepShift")
        .ok()
        .and_then(|shift| shift.extract::<i32>().ok())
    {
        rest.set_step_shift(shift);
    }
    let styled = object
        .getattr("hasStyleInformation")
        .and_then(|said| said.is_truthy())
        .unwrap_or(false);
    if styled {
        rest.set_size(
            object
                .getattr("style")
                .and_then(|style| style.getattr("noteSize"))
                .ok()
                .and_then(|size| size.extract::<String>().ok())
                .and_then(|size| music21_rs_crate::notation::NoteSize::from_name(&size)),
        );
    }
    Ok(rest)
}

/// A music21 chord: each of its notes, and what is written on the chord
/// itself. music21 keeps a chord's length on the chord and not on its
/// notes, so the chord's own duration is the one written back over them.
fn music21_chord(object: &Bound<'_, PyAny>) -> PyResult<music21_rs_crate::Chord> {
    let mut notes = Vec::new();
    for note in object.getattr("notes")?.walk()? {
        notes.push(music21_note(&note?)?);
    }
    let mut chord = music21_rs_crate::Chord::new(notes)
        .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
    let mut carrier = music21_rs_crate::Note::from_name("C4")
        .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
    read_written(object, &mut carrier)?;
    if let Some(duration) = carrier.duration() {
        chord.set_duration(duration.clone());
    }
    chord.set_color(carrier.color().map(str::to_string));
    *chord.expressions_mut() = carrier.expressions().to_vec();
    *chord.articulations_mut() = carrier.articulations().to_vec();
    // music21 beams a chord, not its notes, and its own examples beam one.
    chord.set_beams(carrier.beams().clone());
    // Not the tie: music21's `Chord.tie` answers the tie of its *first
    // note*, which each note already carries and the writer already reads,
    // so taking it here would write it twice.
    // music21 hides a chord's notes one by one, and keeps the chord's
    // lyrics on the chord where the crate keeps them on its first note.
    if !carrier.lyrics().is_empty()
        && let Some(first) = chord.notes_mut().first_mut()
    {
        *first.lyrics_mut() = carrier.lyrics().to_vec();
    }
    for (index, note) in object.getattr("notes")?.walk()?.enumerate() {
        let hidden = hidden_on_print(&note?);
        if let Some(held) = chord.notes_mut().get_mut(index) {
            held.set_hidden(hidden);
        }
    }
    Ok(chord)
}

/// A music21 instrument, read off its attributes: this wheel's instruments
/// are not installed, so a score holds music21's own.
fn instrument_value(object: &Bound<'_, PyAny>) -> PyResult<music21_rs_crate::Instrument> {
    use music21_rs_crate::Instrument as RsInstrument;
    let class: String = object.get_type().getattr("__name__")?.extract()?;
    let mut instrument = RsInstrument::of_kind(&class).unwrap_or_default();
    instrument.set_name(text_attribute(object, "instrumentName"));
    instrument.set_abbreviation(text_attribute(object, "instrumentAbbreviation"));
    instrument.set_part_name(text_attribute(object, "partName"));
    instrument.set_part_abbreviation(text_attribute(object, "partAbbreviation"));
    instrument.set_part_id(text_attribute(object, "partId"));
    instrument.set_instrument_id(text_attribute(object, "instrumentId"));
    instrument.set_midi_program(u8_attribute(object, "midiProgram"));
    instrument.set_midi_channel(u8_attribute(object, "midiChannel"));
    if object.hasattr("percMapPitch")? {
        instrument.set_percussion_pitch(u8_attribute(object, "percMapPitch"));
    }
    let transposition = match object.getattr("transposition") {
        Ok(interval) if !interval.is_none() => {
            let name: String = interval.getattr("directedName")?.extract()?;
            Some(
                music21_rs_crate::Interval::from_name(&name)
                    .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?,
            )
        }
        _ => None,
    };
    instrument.set_transposition(transposition);
    Ok(instrument)
}

/// music21's `id` where it was given rather than taken from a memory
/// address: music21 reads any whole number from `defaults.
/// minIdNumberToConsiderMemoryLocation` up as the latter.
fn given_id(object: &Bound<'_, PyAny>) -> Option<String> {
    let id = object.getattr("id").ok().filter(|id| !id.is_none())?;
    if let Ok(number) = id.extract::<i64>() {
        return (number < 100_000_001).then(|| number.to_string());
    }
    id.extract::<String>().ok()
}

/// What a stream is called and numbered: a measure's number, a part's name,
/// a score's metadata.
/// A length music21 keeps as the `int` or `float` it read.
fn tenths_of(object: &Bound<'_, PyAny>, name: &str) -> Option<music21_rs_crate::layout::Tenths> {
    use music21_rs_crate::layout::Tenths;
    let value = object.getattr(name).ok().filter(|value| !value.is_none())?;
    if value.is_instance_of::<pyo3::types::PyInt>() {
        return value.extract::<i64>().ok().map(Tenths::Whole);
    }
    value.extract::<f64>().ok().map(Tenths::Fraction)
}

fn whole_of(object: &Bound<'_, PyAny>, name: &str) -> Option<i64> {
    let value = object.getattr(name).ok().filter(|value| !value.is_none())?;
    value
        .extract::<i64>()
        .ok()
        .or_else(|| value.extract::<f64>().ok().map(|value| value as i64))
}

fn flag_of(object: &Bound<'_, PyAny>, name: &str) -> Option<bool> {
    object
        .getattr(name)
        .ok()
        .filter(|value| !value.is_none())
        .and_then(|value| value.is_truthy().ok())
}

fn page_layout_of(object: &Bound<'_, PyAny>) -> music21_rs_crate::layout::PageLayout {
    music21_rs_crate::layout::PageLayout {
        page_number: whole_of(object, "pageNumber"),
        left_margin: tenths_of(object, "leftMargin"),
        right_margin: tenths_of(object, "rightMargin"),
        top_margin: tenths_of(object, "topMargin"),
        bottom_margin: tenths_of(object, "bottomMargin"),
        page_height: tenths_of(object, "pageHeight"),
        page_width: tenths_of(object, "pageWidth"),
        is_new: flag_of(object, "isNew"),
    }
}

fn system_layout_of(object: &Bound<'_, PyAny>) -> music21_rs_crate::layout::SystemLayout {
    music21_rs_crate::layout::SystemLayout {
        left_margin: tenths_of(object, "leftMargin"),
        right_margin: tenths_of(object, "rightMargin"),
        top_margin: tenths_of(object, "topMargin"),
        bottom_margin: tenths_of(object, "bottomMargin"),
        distance: tenths_of(object, "distance"),
        top_distance: tenths_of(object, "topDistance"),
        is_new: flag_of(object, "isNew"),
    }
}

fn staff_layout_of(object: &Bound<'_, PyAny>) -> music21_rs_crate::layout::StaffLayout {
    use music21_rs_crate::layout::{StaffLayout, StaffType};
    StaffLayout {
        distance: tenths_of(object, "distance"),
        staff_number: whole_of(object, "staffNumber"),
        staff_size: object
            .getattr("staffSize")
            .ok()
            .filter(|value| !value.is_none())
            .and_then(|value| value.extract::<f64>().ok()),
        staff_lines: whole_of(object, "staffLines"),
        hidden: flag_of(object, "hidden"),
        staff_type: object
            .getattr("staffType")
            .and_then(|kind| kind.getattr("value"))
            .ok()
            .and_then(|value| value.extract::<String>().ok())
            .and_then(|value| StaffType::from_name(&value))
            .unwrap_or_default(),
    }
}

/// A value of music21's style as the crate keeps it: an `int`, a `float`,
/// or text.
fn style_value_of(
    object: &Bound<'_, PyAny>,
    name: &str,
) -> Option<music21_rs_crate::style::StyleValue> {
    use music21_rs_crate::style::StyleValue;
    let value = object.getattr(name).ok().filter(|value| !value.is_none())?;
    if value.is_instance_of::<pyo3::types::PyInt>() {
        return value.extract::<i64>().ok().map(StyleValue::Whole);
    }
    if let Ok(number) = value.extract::<f64>() {
        return Some(StyleValue::Decimal(number));
    }
    value
        .str()
        .ok()
        .map(|text| StyleValue::Text(text.to_string()))
}

/// music21's `TextBox`, with the style it is drawn in.
fn text_box_value(object: &Bound<'_, PyAny>) -> PyResult<music21_rs_crate::text::TextBox> {
    let content = text_attribute(object, "content").unwrap_or_default();
    let mut read = music21_rs_crate::text::TextBox::new(content);
    read.page = whole_of(object, "page").unwrap_or(1);
    let style = object.getattr("style")?;
    let words = |name: &str| text_attribute(&style, name);
    let text = &mut read.style;
    text.absolute_x = style_value_of(&style, "absoluteX");
    text.absolute_y = style_value_of(&style, "absoluteY");
    text.relative_x = style_value_of(&style, "relativeX");
    text.relative_y = style_value_of(&style, "relativeY");
    text.font_size = style_value_of(&style, "fontSize");
    text.color = words("color");
    text.font_family = style
        .getattr("fontFamily")
        .ok()
        .filter(|family| !family.is_none())
        .and_then(|family| family.extract::<Vec<String>>().ok());
    let refused = |error: music21_rs_crate::Error| StreamException::new_err(error.to_string());
    text.set_font_style(words("fontStyle").as_deref())
        .map_err(refused)?;
    text.set_font_weight(words("fontWeight").as_deref())
        .map_err(refused)?;
    text.set_justify(words("justify").as_deref())
        .map_err(refused)?;
    text.set_align_horizontal(words("alignHorizontal").as_deref())
        .map_err(refused)?;
    text.set_align_vertical(words("alignVertical").as_deref())
        .map_err(refused)?;
    Ok(read)
}

/// One of music21's layout objects, where the object is one.
fn layout_value(object: &Bound<'_, PyAny>) -> PyResult<Option<music21_rs_crate::layout::Layout>> {
    use music21_rs_crate::layout::{Layout, ScoreLayout};
    let py = object.py();
    let is = |class: &str| is_of_class(object, &pyo3::types::PyString::new(py, class));
    Ok(Some(if is("ScoreLayout")? {
        let staff_layouts = match object.getattr("staffLayoutList") {
            Ok(list) => list
                .try_iter()?
                .map(|staff| staff.map(|staff| staff_layout_of(&staff)))
                .collect::<PyResult<Vec<_>>>()?,
            Err(_) => Vec::new(),
        };
        let inner = |name: &str| object.getattr(name).ok().filter(|value| !value.is_none());
        Layout::Score(Box::new(ScoreLayout {
            scaling_millimeters: tenths_of(object, "scalingMillimeters"),
            scaling_tenths: tenths_of(object, "scalingTenths"),
            page_layout: inner("pageLayout").map(|page| page_layout_of(&page)),
            system_layout: inner("systemLayout").map(|system| system_layout_of(&system)),
            staff_layouts,
        }))
    } else if is("PageLayout")? {
        Layout::Page(page_layout_of(object))
    } else if is("SystemLayout")? {
        Layout::System(system_layout_of(object))
    } else if is("StaffLayout")? {
        Layout::Staff(staff_layout_of(object))
    } else {
        return Ok(None);
    }))
}

fn read_labels(object: &Bound<'_, PyAny>, stream: &mut RsStream) -> PyResult<()> {
    stream.set_id(given_id(object));
    // How measures are numbered, said on a measure's style.
    let styled = object
        .getattr("hasStyleInformation")
        .and_then(|said| said.is_truthy())
        .unwrap_or(false);
    if styled
        && let Ok(numbering) = object
            .getattr("style")
            .and_then(|style| style.getattr("measureNumbering"))
        && !numbering.is_none()
    {
        stream.set_measure_numbering(Some(numbering.str()?.to_string()));
    }
    if let Some(number) = object
        .getattr("number")
        .ok()
        .and_then(|number| number.extract::<i32>().ok())
    {
        stream.set_number(number);
    }
    stream.set_number_suffix(text_attribute(object, "numberSuffix"));
    // How much of its bar a pickup or a measure cut short leaves unfilled.
    let padding = |name: &str| {
        object
            .getattr(name)
            .ok()
            .and_then(|padding| padding.extract::<f64>().ok())
            .unwrap_or(0.0)
    };
    stream.set_padding_left(padding("paddingLeft"));
    stream.set_padding_right(padding("paddingRight"));
    // music21's `atSoundingPitch` is a bool or the string 'unknown'.
    stream.set_at_sounding_pitch(
        object
            .getattr("atSoundingPitch")
            .ok()
            .filter(|at| at.is_instance_of::<pyo3::types::PyBool>())
            .and_then(|at| at.extract::<bool>().ok()),
    );
    if let Ok(show) = object.getattr("showNumber") {
        stream.set_number_hidden(show.str()?.to_str()?.to_lowercase().ends_with("never"));
    }
    stream.set_name(text_attribute(object, "partName"));
    stream.set_abbreviation(text_attribute(object, "partAbbreviation"));
    let styled = object
        .getattr("hasStyleInformation")
        .and_then(|said| said.is_truthy())
        .unwrap_or(false);
    if styled && let Ok(style) = object.getattr("style") {
        let hidden = |name: &str| {
            style
                .getattr(name)
                .and_then(|shown| shown.is_truthy())
                .is_ok_and(|shown| !shown)
        };
        stream.set_name_hidden(hidden("printPartName"));
        stream.set_abbreviation_hidden(hidden("printPartAbbreviation"));
    }
    if let Ok(metadata) = object.getattr("metadata")
        && !metadata.is_none()
    {
        stream.set_metadata(Some(metadata_value(&metadata)?));
    }
    read_staff_groups(object, stream)?;
    read_ending(object, stream)?;
    for (name, set) in [
        (
            "leftBarline",
            RsStream::set_left_barline as fn(&mut RsStream, _),
        ),
        ("rightBarline", RsStream::set_right_barline),
    ] {
        if let Ok(barline) = object.getattr(name)
            && !barline.is_none()
        {
            set(stream, Some(barline_value(&barline)?));
        }
    }
    Ok(())
}

/// The alternative ending a measure is in: the first `RepeatBracket` it is
/// spanned by, as music21's `setRbSpanners` takes it.
fn read_ending(object: &Bound<'_, PyAny>, stream: &mut RsStream) -> PyResult<()> {
    if stream.kind() != StreamKind::Measure {
        return Ok(());
    }
    let Ok(sites) = object.call_method0("getSpannerSites") else {
        return Ok(());
    };
    for spanner in sites.walk()? {
        let spanner = spanner?;
        if !is_of_class(
            &spanner,
            &pyo3::types::PyString::new(object.py(), "RepeatBracket"),
        )? {
            continue;
        }
        let numbers: Vec<u32> = spanner.getattr("numberRange")?.extract()?;
        let starts = spanner.call_method1("isFirst", (object,))?.is_truthy()?;
        let stops = spanner.call_method1("isLast", (object,))?.is_truthy()?;
        stream.set_ending(Some(music21_rs_crate::Ending::new(numbers, starts, stops)));
        break;
    }
    Ok(())
}

/// A music21 barline or repeat sign.
fn barline_value(barline: &Bound<'_, PyAny>) -> PyResult<music21_rs_crate::Barline> {
    use music21_rs_crate::{Barline, BarlineType, RepeatDirection};
    let invalid =
        |error: music21_rs_crate::Error| StreamException::new_err(crate::pitch::message(&error));
    let bar_type = BarlineType::from_name(&text_attribute(barline, "type").unwrap_or_default())
        .map_err(invalid)?;
    let direction = match text_attribute(barline, "direction").as_deref() {
        Some("start") => Some(RepeatDirection::Start),
        Some("end") => Some(RepeatDirection::End),
        _ => None,
    };
    let mut read = match direction {
        Some(direction) => {
            let times = barline
                .getattr("times")
                .ok()
                .and_then(|times| times.extract::<u32>().ok());
            Barline::repeat(direction, times)
        }
        None => Barline::default(),
    };
    read.set_bar_type(bar_type);
    Ok(read)
}

/// A score's `StaffGroup` spanners, each naming the parts it spans by where
/// they stand among the score's parts.
fn read_staff_groups(object: &Bound<'_, PyAny>, stream: &mut RsStream) -> PyResult<()> {
    use music21_rs_crate::{BarTogether, StaffGroup};
    let py = object.py();
    let Ok(bundle) = object.getattr("spannerBundle") else {
        return Ok(());
    };
    let mut parts: Vec<Bound<'_, PyAny>> = Vec::new();
    for element in object.getattr("elements")?.walk()? {
        let element = element?;
        if matches!(
            kind_of(&element),
            Ok(StreamKind::Part | StreamKind::PartStaff)
        ) && element
            .getattr("isStream")
            .and_then(|flag| flag.is_truthy())
            .unwrap_or(false)
        {
            parts.push(element);
        }
    }
    if parts.is_empty() {
        return Ok(());
    }
    for group in bundle.call_method1("getByClass", ("StaffGroup",))?.walk()? {
        let group = group?;
        let mut positions = Vec::new();
        for spanned in group.call_method0("getSpannedElements")?.walk()? {
            let spanned = spanned?;
            if let Some(position) = parts.iter().position(|part| part.is(&spanned)) {
                positions.push(position);
            }
        }
        let mut read = StaffGroup::new(positions);
        read.set_name(text_attribute(&group, "name"));
        read.set_abbreviation(text_attribute(&group, "abbreviation"));
        read.set_symbol(text_attribute(&group, "symbol").as_deref())
            .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
        let bar_together = group.getattr("barTogether")?;
        read.set_bar_together(if bar_together.is_none() {
            None
        } else if let Ok(flag) = bar_together.cast::<pyo3::types::PyBool>() {
            Some(if flag.is_true() {
                BarTogether::Yes
            } else {
                BarTogether::No
            })
        } else {
            Some(BarTogether::Mensurstrich)
        });
        let hidden = group
            .getattr("style")
            .and_then(|style| style.getattr("hideObjectOnPrint"))
            .map(|hidden| hidden.is_truthy().unwrap_or(false))
            .unwrap_or(false);
        read.set_name_hidden(hidden);
        stream.add_staff_group(read);
    }
    let _ = py;
    Ok(())
}

/// A music21 `Metadata`, name by name in the order music21 keeps them.
pub(crate) fn metadata_value(metadata: &Bound<'_, PyAny>) -> PyResult<music21_rs_crate::Metadata> {
    use music21_rs_crate::MetadataValue;
    // The package's own metadata is already the crate's, and answers no
    // `Text` or `Contributor` to walk.
    if let Ok(ours) = metadata.cast::<crate::metadata::Metadata>() {
        return Ok(ours.borrow().inner.clone());
    }
    let kwargs = PyDict::new(metadata.py());
    kwargs.set_item("returnPrimitives", true)?;
    kwargs.set_item("returnSorted", false)?;
    let mut out = music21_rs_crate::Metadata::new();
    for pair in metadata.call_method("all", (), Some(&kwargs))?.walk()? {
        let pair = pair?;
        let unique_name: String = pair.get_item(0)?.extract()?;
        let value = pair.get_item(1)?;
        // A contributor writes as its name, which is not always what
        // `str` gives it.
        let text = match value.getattr("name") {
            Ok(name) if !name.is_none() && value.hasattr("role")? => name.str()?.to_string(),
            _ => value.str()?.to_string(),
        };
        let entry = match text_attribute(&value, "role") {
            Some(role) => MetadataValue::with_role(text, role),
            None => MetadataValue::new(text),
        };
        out.add(unique_name, entry);
    }
    Ok(out)
}

/// A score, this wheel's or music21's, as the crate's `Stream`, with what it
/// is called and numbered: what the crate's MusicXML writer is handed.
///
/// Unlike the reading the walks do, an element the crate has no value for is
/// left out rather than held by a stand-in, since a writer would write the
/// stand-in.
///
/// Every spanner anywhere in the score is gathered onto the stream handed in,
/// in the order music21's `spannerBundle` holds them, which is the order
/// music21 numbers them in; each names what it joins by position among that
/// stream's leaves.
pub fn crate_stream(stream: &Bound<'_, PyAny>) -> PyResult<RsStream> {
    let mut leaves = Vec::new();
    let mut held = Vec::new();
    let mut read = read_into_with(stream, &mut leaves, false, &mut held)?;
    if let Ok(bundle) = stream.getattr("spannerBundle") {
        for spanner in bundle.walk()? {
            if let Some(value) = spanner_value(&spanner?, &leaves)? {
                read.add_spanner(value);
            }
        }
    } else {
        // This package's own streams hold their spanners among their
        // elements, as music21's do, and have no bundle to ask.
        let py = stream.py();
        for spanner in held {
            if let Some(value) = spanner_value(spanner.bind(py), &leaves)? {
                read.add_spanner(value);
            }
        }
    }
    Ok(read)
}

/// Reads a stream, this wheel's or music21's, into the crate with every
/// element as the value it is, beside the objects at each position.
fn read_values(stream: &Bound<'_, PyAny>) -> PyResult<Read> {
    let mut leaves = Vec::new();
    let stream = read_into(stream, &mut leaves, false)?;
    Ok(Read { stream, leaves })
}

/// Reads a stream, this wheel's or music21's, into the crate.
fn read(stream: &Bound<'_, PyAny>) -> PyResult<Read> {
    let mut leaves = Vec::new();
    let stream = read_into(stream, &mut leaves, true)?;
    Ok(Read { stream, leaves })
}

fn read_into(
    stream: &Bound<'_, PyAny>,
    leaves: &mut Vec<Py<PyAny>>,
    stand_ins: bool,
) -> PyResult<RsStream> {
    let mut spanners = Vec::new();
    read_into_with(stream, leaves, stand_ins, &mut spanners)
}

/// [`read_into`], gathering the spanners it meets: they join the leaves,
/// so they can only be read once every leaf has a position.
fn read_into_with(
    stream: &Bound<'_, PyAny>,
    leaves: &mut Vec<Py<PyAny>>,
    stand_ins: bool,
    spanners: &mut Vec<Py<PyAny>>,
) -> PyResult<RsStream> {
    let mut placed: Vec<(f64, Bound<'_, PyAny>)> = Vec::new();
    for element in stream.getattr("elements")?.walk()? {
        let element = element?;
        let offset: f64 = stream
            .call_method1("elementOffset", (&element,))?
            .extract()?;
        placed.push((offset, element));
    }
    // Stable, as the crate's own ordering is, so the positions the crate
    // counts are the order these objects are listed in.
    placed.sort_by(|left, right| left.0.total_cmp(&right.0));
    let mut events = Vec::new();
    for (offset, element) in placed {
        let is_stream = element
            .getattr("isStream")
            .and_then(|flag| flag.is_truthy())
            .unwrap_or(false);
        // A spanner joins the leaves rather than standing among them:
        // it is read beside the stream, once every leaf is known.
        if is_of_class(
            &element,
            &pyo3::types::PyString::new(element.py(), "Spanner"),
        )? {
            spanners.push(element.clone().unbind());
            continue;
        }
        if is_stream {
            let mut inner = read_into_with(&element, leaves, stand_ins, spanners)?;
            inner.set_kind(kind_of(&element)?);
            events.push(StreamEvent::new(
                offset,
                StreamElement::Stream(Box::new(inner)),
            ));
        } else if !stand_ins && is_barline(&element)? {
            // A measure's own barlines are read with the measure; one
            // standing inside it is an element of its own. music21 keeps
            // the measure's among its elements all the same -- its
            // `rightBarline` is the first barline of `_endElements`, which
            // `elements` lists -- so one read with the measure is passed
            // over here, or the measure would hold it twice. Nothing
            // noticed while the MusicXML writer was the only reader of
            // these: it writes nothing for a barline element either way,
            // and braille writes the one it is given.
            if is_inner_barline(stream, &element)? {
                leaves.push(element.clone().unbind());
                events.push(StreamEvent::new(
                    offset,
                    StreamElement::Barline(barline_value(&element)?),
                ));
            }
        } else if let Some(value) = element_value(&element, stand_ins)? {
            leaves.push(element.unbind());
            events.push(StreamEvent::new(offset, value));
        }
    }
    let mut read = RsStream::from_events(events);
    read.set_kind(kind_of(stream)?);
    read_labels(stream, &mut read)?;
    Ok(read)
}

/// Whether an element of a stream is a barline at all.
fn is_barline(element: &Bound<'_, PyAny>) -> PyResult<bool> {
    is_of_class(
        element,
        &pyo3::types::PyString::new(element.py(), "Barline"),
    )
}

/// Whether an element of a stream is a barline that is not the stream's own
/// left or right one.
fn is_inner_barline(stream: &Bound<'_, PyAny>, element: &Bound<'_, PyAny>) -> PyResult<bool> {
    if !is_barline(element)? {
        return Ok(false);
    }
    for end in ["leftBarline", "rightBarline"] {
        if stream.getattr(end).is_ok_and(|barline| barline.is(element)) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// A music21 spanner of a kind the crate carries, naming what it joins by
/// position among `leaves`. One joining anything outside them -- a slur
/// from one part into another -- is left out.
fn spanner_value(
    spanner: &Bound<'_, PyAny>,
    leaves: &[Py<PyAny>],
) -> PyResult<Option<music21_rs_crate::Spanner>> {
    use music21_rs_crate::{Spanner, SpannerKind};
    let py = spanner.py();
    let is = |class: &str| is_of_class(spanner, &pyo3::types::PyString::new(py, class));
    let kind = if is("Slur")? {
        SpannerKind::Slur
    } else if is("Crescendo")? {
        SpannerKind::Crescendo
    } else if is("Diminuendo")? {
        SpannerKind::Diminuendo
    } else if is("Ottava")? {
        SpannerKind::Ottava
    } else if is("Line")? {
        SpannerKind::Line
    } else if is("PedalMark")? {
        SpannerKind::PedalMark
    } else if is("ArpeggioMarkSpanner")? {
        SpannerKind::ArpeggioMark
    } else if is("TrillExtension")? {
        SpannerKind::TrillExtension
    } else if is("Glissando")? {
        SpannerKind::Glissando
    } else if is("TremoloSpanner")? {
        SpannerKind::TremoloSpanner
    } else if is("Neume")? {
        SpannerKind::Neume
    } else {
        return Ok(None);
    };
    // An element the score does not hold -- music21's reader can leave a
    // spanner holding one -- is still one of the spanner's.
    let mut positions = Vec::new();
    for spanned in spanner.call_method0("getSpannedElements")?.walk()? {
        let spanned = spanned?;
        positions.push(leaves.iter().position(|leaf| leaf.bind(py).is(&spanned)));
    }
    let mut value = Spanner::with_unplaced(kind, positions);
    value.set_placement(placement_of(spanner));
    if let Some(offset) = spanner
        .getattr("offset")
        .ok()
        .and_then(|offset| offset.extract::<f64>().ok())
    {
        value.set_offset(offset);
    }
    if kind == SpannerKind::Glissando {
        value.set_glissando_details(Some(music21_rs_crate::Glissando {
            slide_type: music21_rs_crate::SlideType::from_name(
                &text_attribute(spanner, "slideType").unwrap_or_default(),
            )
            .unwrap_or_default(),
            label: spanner
                .getattr("label")
                .ok()
                .filter(|label| !label.is_none())
                .map(|label| label.str().map(|text| text.to_string()))
                .transpose()?,
        }));
    }
    if kind == SpannerKind::TremoloSpanner {
        let marks = spanner
            .getattr("numberOfMarks")
            .ok()
            .and_then(|marks| marks.extract::<u8>().ok())
            .unwrap_or(3);
        value
            .set_number_of_marks(Some(marks))
            .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
    }
    if kind == SpannerKind::Ottava {
        let name = text_attribute(spanner, "type").unwrap_or_default();
        let transposing = spanner
            .getattr("transposing")
            .and_then(|transposing| transposing.is_truthy())
            .unwrap_or(true);
        value.set_shift(Some(
            music21_rs_crate::OctaveShift::from_name(&name, transposing)
                .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?,
        ));
    }
    if kind.is_wedge() {
        value.set_spread(
            spanner
                .getattr("spread")
                .ok()
                .and_then(|spread| spread.extract::<f64>().ok()),
        );
    }
    let styled = spanner
        .getattr("hasStyleInformation")
        .and_then(|said| said.is_truthy())
        .unwrap_or(false);
    if styled && let Ok(style) = spanner.getattr("style") {
        value.set_line_type(text_attribute(&style, "lineType"));
    }
    // A glissando's line is its own attribute, not its style's.
    if kind == SpannerKind::Glissando {
        value.set_line_type(text_attribute(spanner, "lineType"));
    }
    if kind == SpannerKind::Line {
        let tick = |name: &str| -> PyResult<music21_rs_crate::LineEnd> {
            let written = text_attribute(spanner, name).unwrap_or_else(|| "down".to_string());
            music21_rs_crate::LineEnd::from_name(&written)
                .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))
        };
        let height = |name: &str| {
            spanner
                .getattr(name)
                .ok()
                .filter(|height| !height.is_none())
                .and_then(|height| height.extract::<f64>().ok())
        };
        value.set_ends(music21_rs_crate::LineEnds {
            start: tick("startTick")?,
            end: tick("endTick")?,
            start_height: height("startHeight"),
            end_height: height("endHeight"),
        });
        value.set_line_type(text_attribute(spanner, "lineType"));
    }
    if kind == SpannerKind::ArpeggioMark {
        let name = text_attribute(spanner, "type").unwrap_or_else(|| "normal".to_string());
        value.set_arpeggio(
            music21_rs_crate::expressions::ArpeggioType::from_name(&name)
                .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?,
        );
    }
    if kind == SpannerKind::PedalMark {
        // Both are enums in music21, whose values are the names.
        let named = |name: &str| {
            spanner
                .getattr(name)
                .ok()
                .filter(|value| !value.is_none())
                .and_then(|value| value.getattr("value").ok())
                .and_then(|value| value.extract::<String>().ok())
        };
        value.set_pedal(Some(music21_rs_crate::Pedal {
            pedal_type: named("pedalType")
                .and_then(|name| music21_rs_crate::PedalType::from_name(&name)),
            form: named("pedalForm").and_then(|name| music21_rs_crate::PedalForm::from_name(&name)),
            abbreviated: spanner
                .getattr("abbreviated")
                .and_then(|flag| flag.is_truthy())
                .unwrap_or(false),
        }));
    }
    Ok(Some(value))
}

/// Whether a flag music21 takes as `True`, `False` or an object is `True`
/// itself.
fn is_true(value: &Bound<'_, PyAny>) -> bool {
    value
        .cast::<pyo3::types::PyBool>()
        .is_ok_and(|flag| flag.is_true())
}

/// music21's `volume.realizeVolume`: realizes the volume of every note and
/// chord of a stream under the dynamic in force where it stands, in place.
///
/// Each volume keeps what it realized, which is what `cachedRealized` hands
/// back; with `setAbsoluteVelocity` the answer also becomes the velocity,
/// no longer relative, so the stream plays as marked from velocities alone.
/// `useDynamicContext` is `True` for the stream's own dynamics, `False` for
/// none, or a dynamic to realize everything under.
#[pyfunction]
#[pyo3(signature = (
    srcStream,
    setAbsoluteVelocity = false,
    useDynamicContext = None,
    useVelocity = true,
    useArticulations = None,
))]
fn realizeVolume(
    py: Python<'_>,
    srcStream: &Bound<'_, PyAny>,
    setAbsoluteVelocity: bool,
    useDynamicContext: Option<&Bound<'_, PyAny>>,
    useVelocity: bool,
    useArticulations: Option<&Bound<'_, PyAny>>,
) -> PyResult<()> {
    let read = read(srcStream)?;
    let elements = read.stream.leaves();
    let false_ = pyo3::types::PyBool::new(py, false).to_owned().into_any();
    let true_ = pyo3::types::PyBool::new(py, true).to_owned().into_any();
    let context = useDynamicContext.cloned().unwrap_or_else(|| true_.clone());
    let articulations = useArticulations.cloned().unwrap_or_else(|| true_.clone());
    let from_stream = is_true(&context)
        && elements
            .iter()
            .any(|(_, element)| matches!(element, StreamElement::Dynamic(_)));
    let in_force = music21_rs_crate::volume::dynamics_in_force(&read.stream);
    for (position, (_, element)) in elements.iter().enumerate() {
        if !matches!(
            element,
            StreamElement::Note(_) | StreamElement::Chord(_) | StreamElement::ChordSymbol(_)
        ) {
            continue;
        }
        let dynamic = if from_stream {
            in_force[position]
                .map(|found| read.leaves[found].bind(py).clone())
                .unwrap_or_else(|| false_.clone())
        } else if is_true(&context) {
            false_.clone()
        } else {
            context.clone()
        };
        let volume = read.leaves[position].bind(py).getattr("volume")?;
        let keywords = PyDict::new(py);
        keywords.set_item("useDynamicContext", dynamic)?;
        keywords.set_item("useVelocity", useVelocity)?;
        keywords.set_item("useArticulations", &articulations)?;
        let realized = volume.call_method("getRealized", (), Some(&keywords))?;
        if setAbsoluteVelocity {
            volume.setattr("velocityIsRelative", false)?;
            volume.setattr("velocityScalar", realized)?;
        }
    }
    Ok(())
}

/// music21's `tempo.interpolateElements`: carries what stands between two
/// objects of one stream into another in which both also stand, keeping
/// each thing's place between them.
///
/// Something already in the destination stays where it is; anything else is
/// put in at the same proportion of the way across, or, without `autoAdd`,
/// is an error.
#[pyfunction]
#[pyo3(signature = (element1, element2, sourceStream, destinationStream, autoAdd = true))]
fn interpolateElements(
    element1: &Bound<'_, PyAny>,
    element2: &Bound<'_, PyAny>,
    sourceStream: &Bound<'_, PyAny>,
    destinationStream: &Bound<'_, PyAny>,
    autoAdd: bool,
) -> PyResult<()> {
    let offset_in = |element: &Bound<'_, PyAny>,
                     stream: &Bound<'_, PyAny>,
                     which: &str,
                     place: &str|
     -> PyResult<f64> {
        stream
            .call_method1("elementOffset", (element,))
            .and_then(|offset| offset.extract())
            .map_err(|_| TempoException::new_err(format!("could not find {which} in {place}")))
    };
    let start = (
        offset_in(element1, sourceStream, "element1", "sourceStream")?,
        offset_in(element1, destinationStream, "element1", "destinationStream")?,
    );
    let end = (
        offset_in(element2, sourceStream, "element2", "sourceStream")?,
        offset_in(element2, destinationStream, "element2", "destinationStream")?,
    );
    let between: Vec<(f64, Bound<'_, PyAny>)> = sourceStream
        .getattr("elements")?
        .walk()?
        .map(|element| {
            let element = element?;
            let offset: f64 = sourceStream
                .call_method1("elementOffset", (&element,))?
                .extract()?;
            Ok((offset, element))
        })
        .collect::<PyResult<Vec<_>>>()?
        .into_iter()
        .filter(|(offset, _)| start.0 <= *offset && *offset <= end.0)
        .collect();
    for (offset, element) in between {
        if destinationStream
            .call_method1("elementOffset", (&element,))
            .is_ok()
        {
            continue;
        }
        if !autoAdd {
            let id = element.getattr("id").map_or_else(
                |_| "None".to_string(),
                |id| id.repr().map(|text| text.to_string()).unwrap_or_default(),
            );
            return Err(TempoException::new_err(format!(
                "Could not find element {} with id {id} in destinationStream and autoAdd is false",
                element.repr()?
            )));
        }
        let landed = music21_rs_crate::tempo::interpolated_offset(offset, start, end)
            .map_err(|error| TempoException::new_err(crate::pitch::message(&error)))?;
        destinationStream.call_method1("insert", (landed, &element))?;
    }
    Ok(())
}

/// music21's `harmony.realizeChordSymbolDurations`: gives each chord symbol
/// of a piece the time until the next, the last until the end of the piece,
/// and hands back the piece flattened, holding those same symbols.
#[pyfunction]
fn realizeChordSymbolDurations<'py>(piece: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = piece.py();
    let flat = piece.call_method0("flatten")?;
    let read = read(&flat)?;
    for (position, length) in music21_rs_crate::chordsymbol::chord_symbol_durations(&read.stream) {
        read.leaves[position]
            .bind(py)
            .getattr("duration")?
            .setattr("quarterLength", length)?;
    }
    Ok(flat)
}

/// What sounds or stands in each part of a score at one time: music21's
/// `voiceLeading.Verticality`, as far as `getVerticalityFromObject` builds
/// one.
#[pyclass(name = "Verticality", module = "music21.voiceLeading")]
pub struct Verticality {
    /// Each part's number and what it holds there.
    content: Py<PyDict>,
}

#[pymethods]
impl Verticality {
    #[new]
    #[pyo3(signature = (contentDict = None))]
    fn new(py: Python<'_>, contentDict: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let content = match contentDict {
            Some(given) => given.copy()?,
            None => PyDict::new(py),
        };
        Ok(Self {
            content: content.unbind(),
        })
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.content)
    }

    /// Each part's number and the objects it holds at this time.
    #[getter]
    fn contentDict(&self, py: Python<'_>) -> Py<PyDict> {
        self.content.clone_ref(py)
    }

    /// Every object, part by part.
    #[getter]
    fn objects<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let out = PyList::empty(py);
        let content = self.content.bind(py);
        let mut parts: Vec<i64> = content
            .keys()
            .iter()
            .map(|key| key.extract::<i64>())
            .collect::<PyResult<_>>()?;
        parts.sort_unstable();
        for part in parts {
            if let Some(held) = content.get_item(part)? {
                for object in held.walk()? {
                    out.append(object?)?;
                }
            }
        }
        Ok(out)
    }

    /// The objects one part holds at this time.
    fn getObjectsByPart<'py>(&self, py: Python<'py>, partNum: i64) -> PyResult<Bound<'py, PyAny>> {
        match self.content.bind(py).get_item(partNum)? {
            Some(held) => Ok(held),
            None => Ok(PyList::empty(py).into_any()),
        }
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!(
            "<music21.voiceLeading.Verticality contentDict={}>",
            self.content.bind(py).repr()?
        ))
    }
}

/// music21's `voiceLeading.getVerticalityFromObject`: what sounds or stands
/// in each part of a score at the time one of its objects stands, as a
/// `Verticality` keyed by part number.
///
/// A stream holding no parts is read as one part. A spanner, which music21
/// keeps at the start of a part and lists at every time a note there
/// sounds, is not listed.
#[pyfunction]
#[pyo3(signature = (music21Obj, scoreObjectIsFrom, classFilterList = None))]
fn getVerticalityFromObject(
    py: Python<'_>,
    music21Obj: &Bound<'_, PyAny>,
    scoreObjectIsFrom: &Bound<'_, PyAny>,
    classFilterList: Option<&Bound<'_, PyAny>>,
) -> PyResult<Verticality> {
    let read = read(scoreObjectIsFrom)?;
    let Some(position) = read
        .leaves
        .iter()
        .position(|leaf| leaf.bind(py).is(music21Obj))
    else {
        return Err(StreamException::new_err(format!(
            "{} is not in this score",
            music21Obj.repr()?
        )));
    };
    let elements = read.stream.leaves();
    let offset = elements[position].0;
    let content = PyDict::new(py);
    for (part, mut positions) in
        music21_rs_crate::voiceleading::verticality_positions_at(&read.stream, offset)
    {
        // music21 lists what starts together by `classSortOrder`, which the
        // stand-ins for what the crate cannot read do not carry; the objects
        // do. Stable, so the order the part holds them in breaks a tie.
        positions.sort_by(|left, right| {
            elements[*left].0.total_cmp(&elements[*right].0).then(
                sort_order(read.leaves[*left].bind(py))
                    .cmp(&sort_order(read.leaves[*right].bind(py))),
            )
        });
        let held = PyList::empty(py);
        for position in positions {
            let object = read.leaves[position].bind(py);
            let keep = match classFilterList.filter(|filter| !filter.is_none()) {
                Some(filter) => is_of_any(object, filter)?,
                None => true,
            };
            if keep {
                held.append(object)?;
            }
        }
        if !held.is_empty() {
            content.set_item(part, held)?;
        }
    }
    Ok(Verticality {
        content: content.unbind(),
    })
}

/// music21's `voiceLeading.iterateAllVoiceLeadingQuartets`: every quartet of
/// two voices moving together in a score, each made of the score's own note
/// objects.
///
/// The voices are the score's parts, or the stream itself where it holds
/// none. music21 hands back a generator; this is a list of the same.
#[pyfunction]
#[pyo3(signature = (
    s,
    *,
    includeRests = true,
    includeOblique = true,
    includeNoMotion = false,
    reverse = false,
))]
fn iterateAllVoiceLeadingQuartets<'py>(
    py: Python<'py>,
    s: &Bound<'py, PyAny>,
    includeRests: bool,
    includeOblique: bool,
    includeNoMotion: bool,
    reverse: bool,
) -> PyResult<Bound<'py, PyList>> {
    let read = read(s)?;
    let mut found = music21_rs_crate::voiceleading::voice_leading_quartet_positions(
        &read.stream,
        QuartetOptions {
            include_rests: includeRests,
            include_oblique: includeOblique,
            include_no_motion: includeNoMotion,
        },
    );
    if reverse {
        // Stable, so the quartets at one time keep their order, as music21
        // walks the times backwards and each one forwards.
        found.sort_by(|left, right| right.0.total_cmp(&left.0));
    }
    let class = crate::installed_class(py, "music21.voiceLeading", "VoiceLeadingQuartet")
        .unwrap_or_else(|| {
            py.get_type::<crate::voiceleading::VoiceLeadingQuartet>()
                .into_any()
        });
    let out = PyList::empty(py);
    for (_, notes) in found {
        let notes = notes.map(|position| read.leaves[position].bind(py).clone());
        out.append(class.call1((&notes[0], &notes[1], &notes[2], &notes[3]))?)?;
    }
    Ok(out)
}

/// What a walk hands back: the elements it found, each with where it
/// stood, in the order it found them -- music21's `StreamIterator`.
///
/// music21's filters answer one of these rather than a stream, and its own
/// code leans on that: it filters again (`recurse().notes`), asks how many
/// there are, indexes into them and iterates them. A stream cannot stand
/// in for it, because a stream sorts what it holds and a walk's order is
/// not a sort -- `recurse` meets one part's notes before the next part's,
/// where a stream would put every offset nought together.
#[pyclass(name = "StreamIterator", module = "music21.stream.iterator", subclass)]
pub struct Walked {
    /// What was found, each with its offset from the start of the stream
    /// walked, in the walk's own order.
    found: Vec<(f64, Py<PyAny>)>,
    /// The kind of stream `stream()` gathers them back into, which is the
    /// kind of the one walked.
    kind: StreamKind,
}

impl Walked {
    /// The elements found by a walk, in its order.
    pub(crate) fn of(found: Vec<(f64, Py<PyAny>)>, kind: StreamKind) -> Self {
        Self { found, kind }
    }

    /// Those of them a test answers for, in the same order.
    fn picked(
        &self,
        py: Python<'_>,
        wanted: impl Fn(&Bound<'_, PyAny>) -> PyResult<bool>,
    ) -> PyResult<Self> {
        let mut found = Vec::new();
        for (offset, object) in &self.found {
            if wanted(object.bind(py))? {
                found.push((*offset, object.clone_ref(py)));
            }
        }
        Ok(Self::of(found, self.kind))
    }
}

#[pymethods]
impl Walked {
    /// How many were found.
    fn __len__(&self) -> usize {
        self.found.len()
    }

    /// Whether any were.
    fn __bool__(&self) -> bool {
        !self.found.is_empty()
    }

    /// Each in turn.
    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let objects = PyTuple::new(py, self.found.iter().map(|(_, object)| object.bind(py)))?;
        objects.as_any().try_iter().map(Bound::into_any)
    }

    /// The one at an index, counting from the end for a negative one, or
    /// the list a stretch of them makes: music21's own iterator answers a
    /// list for a slice, and its own code asks for one
    /// (`thisStreamIter[3:5]`).
    fn __getitem__(&self, py: Python<'_>, index: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let count = self.found.len();
        if let Ok(slice) = index.cast::<pyo3::types::PySlice>() {
            let read = slice.indices(count as isize)?;
            let out = pyo3::types::PyList::empty(py);
            let mut at = read.start;
            while (read.step > 0 && at < read.stop) || (read.step < 0 && at > read.stop) {
                if let Some((_, object)) =
                    usize::try_from(at).ok().and_then(|at| self.found.get(at))
                {
                    out.append(object.bind(py))?;
                }
                at += read.step;
            }
            return Ok(out.into_any().unbind());
        }
        let index: isize = index.extract()?;
        let at = if index < 0 {
            count.checked_sub(index.unsigned_abs())
        } else {
            Some(index.unsigned_abs())
        };
        at.and_then(|at| self.found.get(at))
            .map(|(_, object)| object.clone_ref(py))
            .ok_or_else(|| {
                pyo3::exceptions::PyIndexError::new_err(format!(
                    "attempting to access index {index} while elements is of size {count}"
                ))
            })
    }

    /// The first found, or nothing where none was: music21's `first`.
    fn first(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.found.first().map(|(_, object)| object.clone_ref(py))
    }

    /// The last found, or nothing where none was: music21's `last`.
    fn last(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.found.last().map(|(_, object)| object.clone_ref(py))
    }

    /// The notes and chords among them: music21's `notes`.
    #[getter]
    fn notes(&self, py: Python<'_>) -> PyResult<Self> {
        self.picked(py, |object| {
            Ok(["isNote", "isChord"].iter().any(|flag| {
                object
                    .getattr(*flag)
                    .and_then(|flag| flag.is_truthy())
                    .unwrap_or(false)
            }))
        })
    }

    /// The notes, chords and rests among them: music21's `notesAndRests`.
    #[getter]
    fn notesAndRests(&self, py: Python<'_>) -> PyResult<Self> {
        self.picked(py, |object| {
            Ok(["isNote", "isChord", "isRest"].iter().any(|flag| {
                object
                    .getattr(*flag)
                    .and_then(|flag| flag.is_truthy())
                    .unwrap_or(false)
            }))
        })
    }

    /// Those answering to one of these classes, by name or as the class
    /// itself: music21's `getElementsByClass`.
    #[pyo3(signature = (classFilterList))]
    fn getElementsByClass(
        &self,
        py: Python<'_>,
        classFilterList: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        self.picked(py, |object| is_of_any(object, classFilterList))
    }

    /// Those answering to none of them: music21's
    /// `getElementsNotOfClass`.
    #[pyo3(signature = (classFilterList))]
    fn getElementsNotOfClass(
        &self,
        py: Python<'_>,
        classFilterList: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        self.picked(py, |object| {
            is_of_any(object, classFilterList).map(|found| !found)
        })
    }

    /// Them gathered into a stream, each where it stood: music21's
    /// `stream`.
    fn stream<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, Stream>> {
        let mut out = Stream::empty(self.kind);
        for (offset, object) in &self.found {
            out.put(*offset, object.bind(py));
        }
        Bound::new(py, out)
    }

    fn __repr__(&self) -> String {
        format!("<music21.stream.iterator.StreamIterator for {}>", self.kind)
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Stream>()?;
    m.add_class::<Walked>()?;
    register_kinds(m)?;
    m.add_class::<Verticality>()?;
    m.add_function(wrap_pyfunction!(realizeVolume, m)?)?;
    m.add_function(wrap_pyfunction!(interpolateElements, m)?)?;
    m.add_function(wrap_pyfunction!(realizeChordSymbolDurations, m)?)?;
    m.add_function(wrap_pyfunction!(getVerticalityFromObject, m)?)?;
    m.add_function(wrap_pyfunction!(iterateAllVoiceLeadingQuartets, m)?)?;
    Ok(())
}
