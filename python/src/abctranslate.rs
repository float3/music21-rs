//! music21's `abcFormat.translate`: handlers of ABC tokens turned into
//! parts, scores and opuses, over the crate's `abc::translate`.
//!
//! A handler's tokens, with what processing wrote on them, are read into
//! the crate's tokens and translated there; the stream that comes back is
//! this package's. The steps music21 exposes for one token or one line
//! (`parseABCNote`, `metadataToM21Object`, `addABCLyrics`) put what the
//! crate builds into the streams they are handed.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use music21::abc::translate as rs;
use music21::stream::StreamKind;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyModule};

use crate::abcformat::{
    ABCChord, ABCCrescStart, ABCDimStart, ABCMetadata, ABCNote, ABCSlurStart,
    metadata_for_translate, note_for_translate, processed_value, refusal,
};

/// The module music21 keeps these in.
const MODULE: &str = "music21.abcFormat.translate";

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

pyo3::create_exception!(
    music21_rs_facade,
    ABCTranslateException,
    crate::Music21Exception
);

/// A crate stream as this package's.
fn stream_object<'py>(py: Python<'py>, stream: &music21::Stream) -> PyResult<Bound<'py, PyAny>> {
    crate::stream::from_crate(py, stream)
}

/// What `made` holds moved into `into`, where the caller handed a stream
/// to fill; `made` itself where not.
fn filled<'py>(
    made: Bound<'py, PyAny>,
    into: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let Some(into) = into.filter(|given| !given.is_none()) else {
        return Ok(made);
    };
    for element in made.getattr("elements")?.try_iter()? {
        let element = element?;
        let offset = made.call_method1("elementOffset", (&element,))?;
        into.call_method1("coreInsert", (offset, element))?;
    }
    into.call_method0("coreElementsChanged")?;
    Ok(into.clone())
}

/// music21's `abcToStreamPart`: one voice's handler, header included, as a
/// part. A part handed in is filled; the spanner bundle music21 threads
/// through is not needed, each spanner going into the part it ends in.
#[pyfunction]
#[pyo3(signature = (abcHandler, inputM21 = None, spannerBundle = None))]
fn abcToStreamPart<'py>(
    abcHandler: &Bound<'py, PyAny>,
    inputM21: Option<&Bound<'py, PyAny>>,
    spannerBundle: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let _ = spannerBundle;
    let py = abcHandler.py();
    let (handler, spanners) = processed_value(abcHandler)?;
    let part = rs::part_stream(&handler.tokens, &spanners).map_err(|error| refusal(&error))?;
    filled(stream_object(py, &part)?, inputM21)
}

/// music21's `abcToStreamScore`: a tune's handler as a score of a part per
/// voice, with its metadata. An `I:abc-version` line sets the handler's
/// version, as in music21.
#[pyfunction]
#[pyo3(signature = (abcHandler, inputM21 = None))]
fn abcToStreamScore<'py>(
    abcHandler: &Bound<'py, PyAny>,
    inputM21: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = abcHandler.py();
    let (handler, spanners) = processed_value(abcHandler)?;
    if let Some(version) = rs::named_version(&handler) {
        abcHandler.setattr("abcVersion", version)?;
    }
    let score = rs::score_of(&handler, &spanners).map_err(|error| refusal(&error))?;
    filled(stream_object(py, &score)?, inputM21)
}

/// music21's `abcToStreamOpus`: an opus of the handler's tunes, or of the
/// one numbered `number`.
#[pyfunction]
#[pyo3(signature = (abcHandler, inputM21 = None, number = None))]
fn abcToStreamOpus<'py>(
    abcHandler: &Bound<'py, PyAny>,
    inputM21: Option<&Bound<'py, PyAny>>,
    number: Option<i32>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = abcHandler.py();
    let (handler, spanners) = processed_value(abcHandler)?;
    let opus = rs::opus_of(&handler, &spanners, number).map_err(|error| refusal(&error))?;
    filled(stream_object(py, &opus)?, inputM21)
}

/// music21's `parseTokens`: each token of a measure's or part's handler
/// put into `dst`, spanners into `p`. Hands back the transposition a clef
/// asks for and whether a clef was set.
#[pyfunction]
fn parseTokens<'py>(
    mh: &Bound<'py, PyAny>,
    dst: &Bound<'py, PyAny>,
    p: &Bound<'py, PyAny>,
    useMeasures: bool,
    lyricTargets: &Bound<'py, PyAny>,
) -> PyResult<(i64, bool)> {
    let mut transposition = 0;
    let mut clef_set = false;
    for token in mh.getattr("tokens")?.try_iter()? {
        let token = token?;
        if token.is_instance_of::<ABCMetadata>() {
            if token.getattr("tag")?.eq("w")? {
                addABCLyrics(&token.getattr("data")?.extract::<String>()?, lyricTargets)?;
                lyricTargets.call_method0("clear")?;
            }
            (transposition, clef_set) =
                metadataToM21Object(&token, dst, transposition, clef_set, useMeasures)?;
            continue;
        }
        if token.is_instance_of::<ABCNote>() {
            let made = parseABCNote(&token, dst)?;
            let is_rest = made
                .getattr("isRest")
                .is_ok_and(|rest| rest.is_truthy().unwrap_or(false));
            if !made.is_none() && !is_rest && !token.getattr("inGrace")?.is_truthy()? {
                lyricTargets.call_method1("append", (made,))?;
            }
            continue;
        }
        let member = if token.is_instance_of::<ABCSlurStart>() {
            "slurObj"
        } else if token.is_instance_of::<ABCCrescStart>() {
            "crescObj"
        } else if token.is_instance_of::<ABCDimStart>() {
            "dimObj"
        } else {
            continue;
        };
        p.call_method1("coreAppend", (token.getattr(member)?,))?;
    }
    dst.call_method0("coreElementsChanged")?;
    Ok((transposition, clef_set))
}

/// music21's `addABCLyrics`: the syllables of a `w:` line put on the notes
/// they are sung to.
#[pyfunction]
fn addABCLyrics(lyricData: &str, lyricTargets: &Bound<'_, PyAny>) -> PyResult<()> {
    let count = lyricTargets.len()?;
    for (at, syllable) in rs::lyric_syllables(lyricData, count) {
        lyricTargets
            .get_item(at)?
            .call_method1("addLyric", (syllable,))?;
    }
    Ok(())
}

/// Keyword arguments `setActiveSite=False`, as music21 appends notes.
fn no_active_site(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let keywords = PyDict::new(py);
    keywords.set_item("setActiveSite", false)?;
    Ok(keywords)
}

/// music21's `parseABCNote`: the chord symbol over a note token and the
/// note, chord or rest it stands for, appended to `dst`; the note, chord
/// or rest is handed back, or `None`.
#[pyfunction]
fn parseABCNote<'py>(
    t: &Bound<'py, PyAny>,
    dst: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = t.py();
    let (note, spanners) = note_for_translate(t)?;
    let (symbol, made) = rs::note_of(&note).map_err(|error| refusal(&error))?;
    if let Some(symbol) = symbol
        && let Some(object) = crate::stream::element_object(py, &symbol.into())?
    {
        dst.call_method("coreAppend", (object,), Some(&no_active_site(py)?))?;
        dst.call_method0("coreElementsChanged")?;
    }
    let Some(made) = made else {
        return Ok(py.None().into_bound(py));
    };
    let Some(object) = crate::stream::element_object(py, &made)? else {
        return Ok(py.None().into_bound(py));
    };
    if t.is_instance_of::<ABCChord>() {
        dst.call_method1("coreAppend", (&object,))?;
        return Ok(object);
    }

    // The spanners in force take the note, and music21 empties the token's
    // articulations as it reads them.
    for spanner in &spanners {
        spanner
            .bind(py)
            .call_method1("addSpannedElements", (&object,))?;
    }
    t.getattr("articulations")?.call_method0("clear")?;
    dst.call_method("coreAppend", (&object,), Some(&no_active_site(py)?))?;
    Ok(object)
}

/// music21's `metadataToM21Object`: a meter, key, clef or tempo line put
/// into `dst`, at its start where measures are used. Hands back the
/// transposition a clef asks for and whether a clef was set.
#[pyfunction]
fn metadataToM21Object(
    t: &Bound<'_, PyAny>,
    dst: &Bound<'_, PyAny>,
    postTransposition: i64,
    clefSet: bool,
    useMeasures: bool,
) -> PyResult<(i64, bool)> {
    let py = t.py();
    let line = metadata_for_translate(t)?;
    let put = |member: &str, element: music21::stream::StreamElement| -> PyResult<()> {
        let Some(object) = crate::stream::element_object(py, &element)? else {
            return Ok(());
        };
        if useMeasures {
            return dst.setattr(member, object);
        }
        dst.call_method1("coreAppend", (object,))?;
        Ok(())
    };
    match line.tag.as_str() {
        "M" => {
            if let Some(meter) = line.meter().map_err(|error| refusal(&error))? {
                put("timeSignature", meter.into())?;
            }
        }
        "K" => {
            put("keySignature", line.key().map_err(|error| refusal(&error))?)?;
            if let Some((clef, transposition)) = line.clef().map_err(|error| refusal(&error))? {
                put("clef", clef.into())?;
                return Ok((transposition.into(), false));
            }
        }
        "Q" => {
            let mark = line.tempo().map_err(|error| refusal(&error))?;
            if let Some(object) = crate::stream::element_object(py, &mark.into())? {
                dst.call_method1("coreAppend", (object,))?;
            }
        }
        _ => {}
    }
    Ok((postTransposition, clefSet))
}

/// music21's `reBar`: each measure of a part holding more than its last
/// meter's bar cut in two, the measures after numbered on. In place, the
/// part's measures are replaced; otherwise a copy is.
#[pyfunction]
#[pyo3(signature = (music21Part, *, inPlace = false))]
fn reBar<'py>(
    music21Part: &Bound<'py, PyAny>,
    inPlace: bool,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py = music21Part.py();
    let part = if inPlace {
        music21Part.clone()
    } else {
        py.import("copy")?
            .call_method1("deepcopy", (music21Part,))?
    };
    let read = crate::stream::crate_stream(&part)?;
    let cut = rs::re_bar_part(&read)
        .map_err(|error| ABCTranslateException::new_err(crate::pitch::message(&error)))?;

    // The measures put back in place of the old ones.
    let measure = crate::subconverters::music21_name(py, "music21.stream", "Measure")?;
    for old in part
        .call_method1("getElementsByClass", (&measure,))?
        .call_method0("stream")?
        .try_iter()?
    {
        part.call_method1("remove", (old?,))?;
    }
    for event in cut.events() {
        let Some(stream) = event.element().as_stream() else {
            continue;
        };
        if stream.kind() != StreamKind::Measure {
            continue;
        }
        let made = stream_object(py, stream)?;
        part.call_method1("insert", (event.offset(), made))?;
    }
    if inPlace {
        return Ok(None);
    }
    Ok(Some(part))
}

/// The module music21 calls `abcFormat.translate`.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        m.add_function(wrap_pyfunction!(abcToStreamPart, &m)?)?;
        m.add_function(wrap_pyfunction!(abcToStreamScore, &m)?)?;
        m.add_function(wrap_pyfunction!(abcToStreamOpus, &m)?)?;
        m.add_function(wrap_pyfunction!(parseTokens, &m)?)?;
        m.add_function(wrap_pyfunction!(addABCLyrics, &m)?)?;
        m.add_function(wrap_pyfunction!(parseABCNote, &m)?)?;
        m.add_function(wrap_pyfunction!(metadataToM21Object, &m)?)?;
        m.add_function(wrap_pyfunction!(reBar, &m)?)?;
        let exception = py.get_type::<ABCTranslateException>();
        exception.setattr("__module__", MODULE)?;
        m.add("ABCTranslateException", exception)?;
        m.add(
            "environLocal",
            crate::environment::module(py)?
                .getattr("Environment")?
                .call1(("abcFormat.translate",))?,
        )?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}
