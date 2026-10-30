//! music21's `variant`: another reading of a stretch of a score, kept
//! beside the music it stands in for.
//!
//! The merging and the fixing are the crate's `variant`, which
//! `variant_parity` holds to music21: two readings merged measure by
//! measure or note by note, an ossia part merged in as variants, a
//! variant refined to the runs that really differ, and every variant made
//! a replacement rather than an insertion or a deletion.
//!
//! **`Variant` itself stays music21's, and that is the honest line.** Its
//! own members are readings of, or delegations to, the stream it holds --
//! `containedHighestTime`, `replacementQuarterLength`, `lengthType`,
//! `getElementIds`, `__len__`, `show` and the rest -- so a facade for it
//! would carry almost nothing of the crate. What the crate carries is the
//! *module functions*, and those are what is swapped here;
//! `replacedElements` and `removeReplacedElementsFromStream`, the two
//! members of the class that are the crate's, want a reading of a stream's
//! objects by top-level event, which the wheel has not got yet.
//!
//! **A variant is read and built as music21's own.** The package has no
//! class for one, so `stream::variant_value` reads music21's -- its
//! contents, its groups and how much it replaces -- and
//! `stream::variant_object` builds one again. Without music21 beside it
//! there is no class to build, and a stream built without it would quietly
//! have no variants at all -- which is the whole of what these answer --
//! so each asks for the class first and says outright that it needs it.
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

use pyo3::prelude::*;

use music21_rs_crate::variant::{
    add_variant, make_all_variants_replacements, merge_part_as_ossia,
    merge_variant_measure_streams, merge_variant_scores, merge_variants,
    merge_variants_equal_duration, refine_variant,
};
use music21_rs_crate::{FloatType, Stream as RsStream, StreamElement};

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
            let built = crate::stream::variant_object(py, variant, crate::stream::Whose::Music21)?
                .ok_or_else(no_class)?;
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
    needs_variant_class(py)?;
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
    let made = crate::stream::variant_object(py, variant, crate::stream::Whose::Music21)?
        .ok_or_else(no_class)?;
    s.call_method1("insert", (offset, made))?;
    Ok(())
}

/// What music21 says for `addVariant`'s old keyword.
const REPLACEMENT_DURATION_DEPRECATED: &str = "addVariant()'s 'replacementDuration' keyword \
     was renamed to 'replacementQuarterLength' in v10.3; 'replacementDuration' is deprecated \
     and will be removed in v12.";

/// That music21 is there to be asked for its `Variant` class, which every
/// one of these needs: the answer is made of variants, and a stream built
/// without the class would quietly have none.
fn needs_variant_class(py: Python<'_>) -> PyResult<()> {
    if py
        .import("music21.variant")
        .and_then(|module| module.getattr("Variant"))
        .is_err()
    {
        return Err(no_class());
    }
    Ok(())
}

/// What a wheel with no music21 beside it says for a variant.
fn no_class() -> PyErr {
    VariantException::new_err(
        "a variant is music21's own `variant.Variant`, which this package has no class \
         for: these functions need music21 beside it",
    )
}

/// music21's `mergeVariants`: two readings of one stream merged, what
/// differs kept as a variant.
#[pyfunction]
#[pyo3(name = "mergeVariants")]
#[pyo3(signature = (streamX, streamY, variantName = "variant".to_string(), *, inPlace = false))]
pub fn merge_variants_py<'py>(
    streamX: &Bound<'py, PyAny>,
    streamY: &Bound<'py, PyAny>,
    variantName: String,
    inPlace: bool,
) -> PyResult<Bound<'py, PyAny>> {
    needs_variant_class(streamX.py())?;
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
    inPlace: bool,
) -> PyResult<Bound<'py, PyAny>> {
    needs_variant_class(aScore.py())?;
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
    inPlace: bool,
) -> PyResult<Bound<'py, PyAny>> {
    needs_variant_class(streamX.py())?;
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
    inPlace: bool,
) -> PyResult<Bound<'py, PyAny>> {
    needs_variant_class(py)?;
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
    inPlace: bool,
    compareByMeasureNumber: bool,
    recurseInMeasures: bool,
) -> PyResult<Bound<'py, PyAny>> {
    needs_variant_class(mainPart.py())?;
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
    inPlace: bool,
) -> PyResult<Bound<'py, PyAny>> {
    needs_variant_class(s.py())?;
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
    inPlace: bool,
    recurse: bool,
) -> PyResult<Bound<'py, PyAny>> {
    // music21 skips every variant where `variantNames` is a list at all --
    // both branches of its test `continue`, which its own source marks
    // `# huh????` -- so a stream asked for by name comes back exactly as it
    // was, which is what its own example prints.
    needs_variant_class(streamWithVariants.py())?;
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
