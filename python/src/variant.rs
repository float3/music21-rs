//! music21's `variant`: another reading of a stretch of a score, kept
//! beside the music it stands in for.
//!
//! The merging and the fixing are the crate's `variant`, which
//! `variant_parity` holds to music21: two readings merged measure by
//! measure or note by note, an ossia part merged in as variants, a
//! variant refined to the runs that really differ, and every variant made
//! a replacement rather than an insertion or a deletion.
//!
//! **`Variant` is the package's too.** Where music21 is installed its own
//! class answers, as `stream::variant_object` asks for it first; where
//! `provide_music21` stands in for music21 there is no other, and code
//! written for music21 builds and reads variants all the same. Its members
//! are music21's: a `VariantStorage` holding the reading, every member it
//! does not have read off that stream, and no length of its own in the
//! stream holding it unless `exposeTime` says so.
//!
//! **`inPlace=True` is answered here, where it is refused everywhere else
//! in this package.** Elsewhere an edit answers a new stream, because a
//! caller's own objects would otherwise stop being the ones the stream
//! holds -- but every one of these only *adds or replaces variants*: the
//! notes, rests and measures of the stream are left exactly as they were.
//! So an edit in place is one removal and one insertion per variant and no
//! rebuilding at all. A variant the stream already held is rebuilt rather
//! than kept, which is the one thing a caller holding one would notice,
//! and these are the functions that make variants in the first place. A
//! variant an answer puts inside a measure -- which is where
//! `refineVariant` puts the ones it refines -- goes into the measure at
//! the same place, the streams of the two sides coming in the one order.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::PyAttributeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};

use music21::variant::{
    add_variant, make_all_variants_replacements, merge_part_as_ossia,
    merge_variant_measure_streams, merge_variant_scores, merge_variants,
    merge_variants_equal_duration, refine_variant,
};
use music21::{FloatType, Stream as RsStream, StreamElement};

use crate::Walkable;

/// The names this module hands out.
pub const NAMES: &[&str] = &[
    "addVariant",
    "mergeVariants",
    "mergeVariantScores",
    "mergeVariantMeasureStreams",
    "mergeVariantsEqualDuration",
    "mergePartAsOssia",
    "refineVariant",
    "makeAllVariantsReplacements",
];

pyo3::create_exception!(music21_rs_facade, VariantException, crate::Music21Exception);

/// music21's `Variant`: another reading of a stretch of a stream, held in
/// a `VariantStorage` and standing in the stream it is another reading of
/// with no length of its own.
#[pyclass(name = "Variant", module = "music21.variant", extends = crate::base::Music21Object, subclass, dict, weakref)]
pub struct Variant {
    /// The reading: music21's `_stream`.
    stream: Py<PyAny>,
    /// The length of the stretch it replaces, where one was said: music21's
    /// `_replacementQuarterLength`.
    #[pyo3(get, set)]
    _replacementQuarterLength: Option<Py<PyAny>>,
    /// Whether `highestTime` and `highestOffset` answer the reading's
    /// rather than nought: music21's `exposeTime`.
    #[pyo3(get, set)]
    exposeTime: bool,
}

#[pymethods]
impl Variant {
    /// music21's `Variant(givenElements=None, name=None,
    /// givenElementsBehavior=OFFSETS, **keywords)`: the elements go into
    /// the reading, and `name` into `groups`.
    #[new]
    #[pyo3(signature = (givenElements = None, name = None, givenElementsBehavior = None, **keywords))]
    fn new(
        py: Python<'_>,
        givenElements: Option<&Bound<'_, PyAny>>,
        name: Option<&Bound<'_, PyAny>>,
        givenElementsBehavior: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let _ = name;
        let given = PyDict::new(py);
        if let Some(behavior) = givenElementsBehavior.filter(|behavior| !behavior.is_none()) {
            given.set_item("givenElementsBehavior", behavior)?;
        }
        let stream = py
            .get_type::<crate::stream::VariantStorage>()
            .call((givenElements,), Some(&given))?
            .unbind();
        let variant = Self {
            stream,
            _replacementQuarterLength: None,
            exposeTime: false,
        };
        Ok(crate::base::Music21Object::new(&PyTuple::empty(py), keywords)?.add_subclass(variant))
    }

    /// Puts `name` into `groups`, which is the base's and so not there to
    /// write in `__new__`.
    #[pyo3(signature = (givenElements = None, name = None, *_arguments, **_keywords))]
    fn __init__(
        slf: &Bound<'_, Self>,
        givenElements: Option<&Bound<'_, PyAny>>,
        name: Option<&Bound<'_, PyAny>>,
        _arguments: &Bound<'_, PyTuple>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let _ = givenElements;
        if let Some(name) = name.filter(|name| !name.is_none()) {
            slf.getattr("groups")?.call_method1("append", (name,))?;
        }
        Ok(())
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.stream)?;
        visit.call(&self._replacementQuarterLength)
    }

    /// Sorts before a stream at the same offset, as music21's does.
    #[classattr]
    fn classSortOrder() -> i64 {
        VARIANT_SORT_ORDER
    }

    /// A variant is no stream, as music21's `Music21Object` says, though
    /// it reads every other member off one.
    #[classattr]
    fn isStream() -> bool {
        false
    }

    /// The reading: music21's `_stream`.
    #[getter]
    fn _stream(&self, py: Python<'_>) -> Py<PyAny> {
        self.stream.clone_ref(py)
    }

    /// The reading: music21's `containedSite`.
    #[getter]
    fn containedSite(&self, py: Python<'_>) -> Py<PyAny> {
        self.stream.clone_ref(py)
    }

    /// Every member the variant has not got, read off the reading, but for
    /// `flat`, `pitches` and Python's own names, which music21 keeps off
    /// it so a walk does not go into the variant.
    fn __getattr__(&self, py: Python<'_>, attr: &str) -> PyResult<Py<PyAny>> {
        if attr == "flat" || attr == "pitches" || attr.starts_with("__") {
            return Err(PyAttributeError::new_err(attr.to_owned()));
        }
        Ok(self.stream.bind(py).getattr(attr)?.unbind())
    }

    fn __getitem__(&self, py: Python<'_>, key: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Ok(self.stream.bind(py).get_item(key)?.unbind())
    }

    fn __len__(&self, py: Python<'_>) -> PyResult<usize> {
        self.stream.bind(py).len()
    }

    fn __iter__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(self.stream.bind(py).try_iter()?.into_any().unbind())
    }

    /// `object of length 8.0`.
    fn _reprInternal(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!(
            "object of length {}",
            self.containedHighestTime(py)?.bind(py).str()?
        ))
    }

    /// Python's `id` of each element of the reading: music21's
    /// `getElementIds`.
    fn getElementIds(&self, py: Python<'_>) -> PyResult<Vec<usize>> {
        let mut ids = Vec::new();
        for element in self.stream.bind(py).getattr("elements")?.try_iter()? {
            ids.push(element?.as_ptr() as usize);
        }
        Ok(ids)
    }

    /// Puts `new` where `old` stands in the reading, `old` being the
    /// object or Python's `id` of it: music21's `replaceElement`, which a
    /// copy calls with the copied elements.
    fn replaceElement(
        &self,
        py: Python<'_>,
        old: &Bound<'_, PyAny>,
        new: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        if old.is_none() {
            return Ok(());
        }
        let stream = self.stream.bind(py);
        let keywords = PyDict::new(py);
        keywords.set_item("allDerived", false)?;
        let Ok(id) = old.extract::<usize>() else {
            stream.call_method("replace", (old, new), Some(&keywords))?;
            return Ok(());
        };
        for element in stream.getattr("elements")?.try_iter()? {
            let element = element?;
            if element.as_ptr() as usize == id {
                stream.call_method("replace", (element, new), Some(&keywords))?;
                return Ok(());
            }
        }
        Ok(())
    }

    /// Nought, so the variant takes no time in the stream holding it,
    /// unless `exposeTime`: music21's `highestTime`.
    #[getter]
    fn highestTime(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.exposed(py, "highestTime")
    }

    /// The same for `highestOffset`.
    #[getter]
    fn highestOffset(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.exposed(py, "highestOffset")
    }

    /// Shows the reading: music21's `show`.
    #[pyo3(signature = (fmt = None, app = None))]
    fn show(
        &self,
        py: Python<'_>,
        fmt: Option<&Bound<'_, PyAny>>,
        app: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let keywords = PyDict::new(py);
        keywords.set_item("fmt", fmt)?;
        keywords.set_item("app", app)?;
        self.stream
            .bind(py)
            .call_method("show", (), Some(&keywords))?;
        Ok(())
    }

    /// Where the reading ends: music21's `containedHighestTime`.
    #[getter]
    fn containedHighestTime(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(self.stream.bind(py).getattr("highestTime")?.unbind())
    }

    /// Where the reading's last element starts: music21's
    /// `containedHighestOffset`.
    #[getter]
    fn containedHighestOffset(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(self.stream.bind(py).getattr("highestOffset")?.unbind())
    }

    /// The length of the stretch this replaces: the one set, or the
    /// reading's own: music21's `replacementQuarterLength`.
    #[getter]
    fn get_replacementQuarterLength(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        if let Some(said) = &self._replacementQuarterLength {
            return Ok(said.clone_ref(py));
        }
        Ok(self
            .stream
            .bind(py)
            .getattr("duration")?
            .getattr("quarterLength")?
            .unbind())
    }

    #[setter]
    fn set_replacementQuarterLength(&mut self, value: Option<Py<PyAny>>) {
        self._replacementQuarterLength = value;
    }

    /// The same, under its old name: music21's `replacementDuration`.
    ///
    /// * Deprecated in v11: use `replacementQuarterLength`.
    #[getter]
    fn get_replacementDuration(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.get_replacementQuarterLength(py)
    }

    #[setter]
    fn set_replacementDuration(
        &mut self,
        py: Python<'_>,
        value: Option<Py<PyAny>>,
    ) -> PyResult<()> {
        crate::warn_deprecated(
            py,
            "'replacementDuration' is deprecated as of v11 and will be removed in v12; use \
             the synonym replacementQuarterLength instead.",
        )?;
        self._replacementQuarterLength = value;
        Ok(())
    }

    /// `'deletion'` where the reading is shorter than what it replaces,
    /// `'elongation'` where longer, `'replacement'` otherwise: music21's
    /// `lengthType`.
    #[getter]
    fn lengthType(&self, py: Python<'_>) -> PyResult<&'static str> {
        let replaced: f64 = self
            .get_replacementQuarterLength(py)?
            .bind(py)
            .call_method0("__float__")?
            .extract()?;
        let contained: f64 = self
            .containedHighestTime(py)?
            .bind(py)
            .call_method0("__float__")?
            .extract()?;
        Ok(length_type(replaced - contained))
    }
}

impl Variant {
    /// The reading's member, where `exposeTime`, and nought otherwise.
    fn exposed(&self, py: Python<'_>, member: &str) -> PyResult<Py<PyAny>> {
        if self.exposeTime {
            return Ok(self.stream.bind(py).getattr(member)?.unbind());
        }
        Ok(0.0_f64.into_pyobject(py)?.into_any().unbind())
    }
}

/// music21's `Stream.classSortOrder` less two: a variant sorts before the
/// streams at its offset.
const VARIANT_SORT_ORDER: i64 = -22;

/// music21's `lengthType` for how much longer the stretch replaced is than
/// the reading.
fn length_type(difference: f64) -> &'static str {
    if difference > 0.0 {
        return "deletion";
    }
    if difference < 0.0 {
        return "elongation";
    }
    "replacement"
}

error_into!(pub(crate) variant_error, VariantException);

/// A merge or a refinement written back into the stream it was given.
///
/// Every one of these only *adds or replaces variants*: the notes, rests
/// and measures of the stream are left exactly as they were, so an edit in
/// place is one removal and one insertion per variant and no rebuilding at
/// all. A variant the stream already held is rebuilt rather than kept,
/// which is the one thing a caller holding one would notice -- and these
/// are the functions that make variants in the first place.
///
/// A variant an answer puts *inside* a measure goes into the measure at
/// the same place: `refineVariant` puts its refined variants there, and
/// the streams of the two sides come in the one order, music21's sort.
fn variants_written(s: &Bound<'_, PyAny>, made: &RsStream) -> PyResult<()> {
    let py = s.py();
    let mut held = Vec::new();
    for variant in s.call_method1("getElementsByClass", ("Variant",))?.walk()? {
        held.push(variant?);
    }
    for variant in held {
        s.call_method1("remove", (variant,))?;
    }
    for event in made.events() {
        if let StreamElement::Variant(variant) = event.element() {
            let built = crate::stream::variant_object(py, variant, crate::stream::Whose::Music21)?;
            s.call_method1("insert", (event.offset(), built))?;
        }
    }
    // The streams it holds, each written the same way: both sides are in
    // music21's sort order, so the nth stream of one is the nth of the
    // other.
    let mut inner = Vec::new();
    for stream in s.call_method1("getElementsByClass", ("Stream",))?.walk()? {
        inner.push(stream?);
    }
    let answered = made
        .events()
        .iter()
        .filter_map(|event| match event.element() {
            StreamElement::Stream(stream) => Some(stream),
            _ => None,
        });
    for (stream, answer) in inner.iter().zip(answered) {
        variants_written(stream, answer)?;
    }
    Ok(())
}

/// A stream read as the crate holds one.
fn read(stream: &Bound<'_, PyAny>) -> PyResult<RsStream> {
    crate::stream::crate_stream(stream)
}

/// A crate stream as music21's own, which is what these answer.
fn built<'py>(py: Python<'py>, stream: &RsStream) -> PyResult<Bound<'py, PyAny>> {
    crate::stream::music21_from_crate(py, stream)
}

/// music21's `addVariant`: a variant put into a stream at an offset,
/// holding another reading of what stands there.
///
/// The stream it is given is the one it adds to, as music21's does: one
/// variant goes in and every element already there is left alone.
#[pyfunction]
#[pyo3(name = "addVariant")]
#[pyo3(signature = (
    s,
    startOffset,
    sVariant,
    variantName = None,
    variantGroups = None,
    replacementQuarterLength = None,
    *,
    replacementDuration = None,
))]
pub fn add_variant_py(
    s: &Bound<'_, PyAny>,
    startOffset: FloatType,
    sVariant: Option<&Bound<'_, PyAny>>,
    variantName: Option<String>,
    variantGroups: Option<Vec<String>>,
    replacementQuarterLength: Option<FloatType>,
    replacementDuration: Option<FloatType>,
) -> PyResult<()> {
    let py = s.py();
    // music21 renamed the length in v10.3 and warns for the old name, on
    // every call, until v12 takes it away.
    if replacementDuration.is_some() {
        crate::warn_deprecated(py, REPLACEMENT_DURATION_DEPRECATED)?;
        if replacementQuarterLength.is_some() {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "addVariant() received both 'replacementQuarterLength' and its former \
                 name 'replacementDuration'; pass only 'replacementQuarterLength'.",
            ));
        }
    }
    let replacementQuarterLength = replacementQuarterLength.or(replacementDuration);
    let mut stream = read(s)?;
    let contents = match sVariant.filter(|given| !given.is_none()) {
        Some(given) => Some(read(given)?),
        None => None,
    };
    let before = stream
        .events()
        .iter()
        .filter(|event| matches!(event.element(), StreamElement::Variant(_)))
        .count();
    add_variant(
        &mut stream,
        startOffset,
        contents.as_ref(),
        variantName.as_deref(),
        variantGroups.as_deref(),
        replacementQuarterLength,
    );
    // The one the crate added, built as music21's and put where it stands.
    let added = stream
        .events()
        .iter()
        .filter_map(|event| match event.element() {
            StreamElement::Variant(variant) => Some((event.offset(), variant)),
            _ => None,
        })
        .nth(before);
    let Some((offset, variant)) = added else {
        return Ok(());
    };
    let made = crate::stream::variant_object(py, variant, crate::stream::Whose::Music21)?;
    s.call_method1("insert", (offset, made))?;
    Ok(())
}

/// What music21 says for `addVariant`'s old keyword.
const REPLACEMENT_DURATION_DEPRECATED: &str = "addVariant()'s 'replacementDuration' keyword \
     was renamed to 'replacementQuarterLength' in v10.3; 'replacementDuration' is deprecated \
     and will be removed in v12.";

/// music21's `mergeVariants`: two readings of one stream merged, what
/// differs kept as a variant.
#[pyfunction]
#[pyo3(name = "mergeVariants")]
#[pyo3(signature = (streamX, streamY, variantName = "variant".to_string(), *, inPlace = false))]
pub fn merge_variants_py<'py>(
    streamX: &Bound<'py, PyAny>,
    streamY: &Bound<'py, PyAny>,
    variantName: String,
    #[pyo3(from_py_with = crate::truthy)] inPlace: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let made =
        merge_variants(&read(streamX)?, &read(streamY)?, &variantName).map_err(variant_error)?;
    if inPlace {
        variants_written(streamX, &made)?;
        return Ok(streamX.py().None().into_bound(streamX.py()));
    }
    built(streamX.py(), &made)
}

/// music21's `mergeVariantScores`: the same for two scores, part by part.
#[pyfunction]
#[pyo3(name = "mergeVariantScores")]
#[pyo3(signature = (aScore, vScore, variantName = "variant".to_string(), *, inPlace = false))]
pub fn merge_variant_scores_py<'py>(
    aScore: &Bound<'py, PyAny>,
    vScore: &Bound<'py, PyAny>,
    variantName: String,
    #[pyo3(from_py_with = crate::truthy)] inPlace: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let made = merge_variant_scores(&read(aScore)?, &read(vScore)?, &variantName)
        .map_err(variant_error)?;
    if inPlace {
        variants_written(aScore, &made)?;
        return Ok(aScore.py().None().into_bound(aScore.py()));
    }
    built(aScore.py(), &made)
}

/// music21's `mergeVariantMeasureStreams`: the same measure by measure,
/// each measure that differs a variant of its own.
#[pyfunction]
#[pyo3(name = "mergeVariantMeasureStreams")]
#[pyo3(signature = (streamX, streamY, variantName = "variant".to_string(), *, inPlace = false))]
pub fn merge_variant_measure_streams_py<'py>(
    streamX: &Bound<'py, PyAny>,
    streamY: &Bound<'py, PyAny>,
    variantName: String,
    #[pyo3(from_py_with = crate::truthy)] inPlace: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let made = merge_variant_measure_streams(&read(streamX)?, &read(streamY)?, &variantName)
        .map_err(variant_error)?;
    if inPlace {
        variants_written(streamX, &made)?;
        return Ok(streamX.py().None().into_bound(streamX.py()));
    }
    built(streamX.py(), &made)
}

/// music21's `mergeVariantsEqualDuration`: several readings of one length
/// merged, each named in turn.
#[pyfunction]
#[pyo3(name = "mergeVariantsEqualDuration")]
#[pyo3(signature = (streams, variantNames, *, inPlace = false))]
pub fn merge_variants_equal_duration_py<'py>(
    py: Python<'py>,
    streams: &Bound<'py, PyAny>,
    variantNames: Vec<String>,
    #[pyo3(from_py_with = crate::truthy)] inPlace: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let mut read_streams = Vec::new();
    for stream in streams.walk()? {
        read_streams.push(read(&stream?)?);
    }
    let given: Vec<&RsStream> = read_streams.iter().collect();
    let names: Vec<&str> = variantNames.iter().map(String::as_str).collect();
    let made = merge_variants_equal_duration(&given, &names).map_err(variant_error)?;
    if inPlace {
        variants_written(&streams.get_item(0)?, &made)?;
        return Ok(py.None().into_bound(py));
    }
    built(py, &made)
}

/// music21's `mergePartAsOssia`: an ossia part merged into a part, each of
/// its measures a variant where that measure stands.
#[pyfunction]
#[pyo3(name = "mergePartAsOssia")]
#[pyo3(signature = (
    mainPart,
    ossiaPart,
    ossiaName = String::new(),
    inPlace = false,
    compareByMeasureNumber = false,
    recurseInMeasures = false,
))]
pub fn merge_part_as_ossia_py<'py>(
    mainPart: &Bound<'py, PyAny>,
    ossiaPart: &Bound<'py, PyAny>,
    ossiaName: String,
    #[pyo3(from_py_with = crate::truthy)] inPlace: bool,
    #[pyo3(from_py_with = crate::truthy)] compareByMeasureNumber: bool,
    #[pyo3(from_py_with = crate::truthy)] recurseInMeasures: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let made = merge_part_as_ossia(
        &read(mainPart)?,
        &read(ossiaPart)?,
        &ossiaName,
        compareByMeasureNumber,
        recurseInMeasures,
    )
    .map_err(variant_error)?;
    if inPlace {
        variants_written(mainPart, &made)?;
        return Ok(mainPart.py().None().into_bound(mainPart.py()));
    }
    built(mainPart.py(), &made)
}

/// music21's `refineVariant`: a variant cut down to the runs of notes
/// that really differ from what it stands in for.
#[pyfunction]
#[pyo3(name = "refineVariant")]
#[pyo3(signature = (s, sVariant, *, inPlace = false))]
pub fn refine_variant_py<'py>(
    s: &Bound<'py, PyAny>,
    sVariant: &Bound<'py, PyAny>,
    #[pyo3(from_py_with = crate::truthy)] inPlace: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let mut stream = read(s)?;
    let at = variant_place(s, sVariant, &stream)?;
    refine_variant(&mut stream, at).map_err(variant_error)?;
    if inPlace {
        variants_written(s, &stream)?;
        return Ok(s.py().None().into_bound(s.py()));
    }
    built(s.py(), &stream)
}

/// Which of a stream's events the variant given is: the crate names a
/// variant by where it stands among them, and music21 by the object.
///
/// The variants of a stream come in one order, so the *n*th variant
/// music21 holds is the *n*th the crate read.
fn variant_place(
    s: &Bound<'_, PyAny>,
    given: &Bound<'_, PyAny>,
    stream: &RsStream,
) -> PyResult<usize> {
    let mut which = None;
    for (at, held) in s
        .call_method1("getElementsByClass", ("Variant",))?
        .walk()?
        .enumerate()
    {
        if held?.is(given) {
            which = Some(at);
            break;
        }
    }
    let Some(which) = which else {
        return Err(VariantException::new_err(
            "the variant given is not one the stream holds",
        ));
    };
    stream
        .events()
        .iter()
        .enumerate()
        .filter(|(_, event)| matches!(event.element(), StreamElement::Variant(_)))
        .map(|(at, _)| at)
        .nth(which)
        .ok_or_else(|| VariantException::new_err("the variant given is not one the stream holds"))
}

/// music21's `makeAllVariantsReplacements`: every variant made to stand in
/// for as long as it lasts, rather than to add or to take away.
#[pyfunction]
#[pyo3(name = "makeAllVariantsReplacements")]
#[pyo3(signature = (streamWithVariants, variantNames = None, inPlace = false, recurse = false))]
pub fn make_all_variants_replacements_py<'py>(
    streamWithVariants: &Bound<'py, PyAny>,
    variantNames: Option<Vec<String>>,
    #[pyo3(from_py_with = crate::truthy)] inPlace: bool,
    #[pyo3(from_py_with = crate::truthy)] recurse: bool,
) -> PyResult<Bound<'py, PyAny>> {
    // music21 skips every variant where `variantNames` is a list at all --
    // both branches of its test `continue`, which its own source marks
    // `# huh????` -- so a stream asked for by name comes back exactly as it
    // was, which is what its own example prints.
    if variantNames.is_some() {
        let py = streamWithVariants.py();
        if inPlace {
            return Ok(py.None().into_bound(py));
        }
        return py
            .import("copy")?
            .call_method1("deepcopy", (streamWithVariants,));
    }
    let made = make_all_variants_replacements(&read(streamWithVariants)?, recurse)
        .map_err(variant_error)?;
    if inPlace {
        variants_written(streamWithVariants, &made)?;
        return Ok(streamWithVariants
            .py()
            .None()
            .into_bound(streamWithVariants.py()));
    }
    built(streamWithVariants.py(), &made)
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Variant>()?;
    m.add_function(wrap_pyfunction!(add_variant_py, m)?)?;
    m.add_function(wrap_pyfunction!(merge_variants_py, m)?)?;
    m.add_function(wrap_pyfunction!(merge_variant_scores_py, m)?)?;
    m.add_function(wrap_pyfunction!(merge_variant_measure_streams_py, m)?)?;
    m.add_function(wrap_pyfunction!(merge_variants_equal_duration_py, m)?)?;
    m.add_function(wrap_pyfunction!(merge_part_as_ossia_py, m)?)?;
    m.add_function(wrap_pyfunction!(refine_variant_py, m)?)?;
    m.add_function(wrap_pyfunction!(make_all_variants_replacements_py, m)?)?;
    Ok(())
}
