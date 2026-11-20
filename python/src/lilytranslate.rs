//! music21's `lily.translate`: a stream as a tree of `lily.lilyObjects`,
//! written as LilyPond and run through an installed LilyPond.
//!
//! `LilypondConverter` walks music21's objects -- this package's or
//! music21's own -- and builds the tree music21 builds, object by object
//! and in music21's order, since the text's indentation follows the order
//! a tree was put together in. Its state is kept as attributes, as
//! music21's is, so examples that set `lpc.context` or read `lpc.inWord`
//! read what music21's would. The crate's `lily` module writes the same
//! text from the crate's streams without this tree.
//!
//! Rendering to PDF, PNG or SVG runs the `lilypond` program exactly as
//! music21 does, through Python's `subprocess.run`.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{IntoPyDict, PyDict, PyList, PyString};

use crate::lilyobjects::{class as ly_class, make};
use crate::plumbing::built::{method, new_class};

/// The module's dotted name.
const MODULE: &str = "music21.lily.translate";

/// The class's name.
const CONVERTER: &str = "LilypondConverter";

const FICTA_DEF: &str =
    "ficta = #(define-music-function () () #{ \\once \\set suggestAccidentals = ##t #})\n    ";

const COLOR_DEF: &str = "color = #(define-music-function (color) (string?) #{
        \\once \\override NoteHead.color = #(x11-color color)
        \\once \\override Stem.color = #(x11-color color)
        \\once \\override Rest.color = #(x11-color color)
        \\once \\override Beam.color = #(x11-color color)
     #})
    ";

const SIMPLE_PAPER: &str = "\\paper { #(define dump-extents #t)
    indent = 0\\mm
    force-assignment = #\"\"
    oddFooterMarkup=##f
    oddHeaderMarkup=##f
    bookTitleMarkup=##f
    }
    ";

const TRANSPARENCY_START: &str =
    "\\override Rest.transparent  = ##t\n     \\override Dots.transparent  = ##t\n     ";

const TRANSPARENCY_STOP: &str =
    "\\revert Rest #'transparent\n     \\revert Dots #'transparent\n     ";

const BOOK_HEADER: &str = "\\include \"lilypond-book-preamble.ly\"\n    ";

/// music21's accidental names and LilyPond's (Dutch) endings for them.
const ACCIDENTALS: [(&str, &str); 8] = [
    ("double-sharp", "isis"),
    ("double-flat", "eses"),
    ("one-and-a-half-sharp", "isih"),
    ("one-and-a-half-flat", "eseh"),
    ("sharp", "is"),
    ("flat", "es"),
    ("half-sharp", "ih"),
    ("half-flat", "eh"),
];

/// music21's barline types and LilyPond's bar lines for them.
const BARLINES: [(&str, &str); 13] = [
    ("regular", "|"),
    ("dotted", ";"),
    ("dashed", "!"),
    ("heavy", "."),
    ("double", "||"),
    ("final", "|."),
    ("heavy-light", ".|"),
    ("heavy-heavy", ".."),
    ("start-repeat", ".|:"),
    ("end-repeat", ":|."),
    ("tick", "'"),
    ("short", ","),
    ("none", ""),
];

/// music21's clef classes, most specific first, and LilyPond's names.
const CLEFS: [(&str, &str); 10] = [
    ("Treble8vbClef", "treble_8"),
    ("Treble8vaClef", "treble^8"),
    ("TrebleClef", "treble"),
    ("Bass8vbClef", "bass_8"),
    ("Bass8vaClef", "bass^8"),
    ("BassClef", "bass"),
    ("AltoClef", "alto"),
    ("TenorClef", "tenor"),
    ("SopranoClef", "soprano"),
    ("PercussionClef", "percussion"),
];

/// The colors variants are shown in.
const VARIANT_COLORS: [&str; 7] = ["blue", "red", "purple", "green", "orange", "yellow", "grey"];

/// What a variant's staff is set up with above its part.
const VARIANT_STAFF: [&str; 9] = [
    "\\remove \"Time_signature_engraver\"",
    "alignAboveContext = #\"{}\"",
    "fontSize = #-3",
    "\\override StaffSymbol.staff-space = #(magstep -3)",
    "\\override StaffSymbol.thickness = #(magstep -3)",
    "\\override TupletBracket.bracket-visibility = ##f",
    "\\override TupletNumber.stencil = ##f",
    "\\override Clef.transparent = ##t",
    "\\override ClefModifier.transparent = ##t",
];

pyo3::create_exception!(
    music21_rs_facade,
    LilyTranslateException,
    crate::Music21Exception
);

fn translate_error(message: impl Into<String>) -> PyErr {
    LilyTranslateException::new_err(message.into())
}

/// The module, built once.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        let exception = py.get_type::<LilyTranslateException>();
        exception.setattr("__module__", MODULE)?;
        m.add("LilyTranslateException", exception)?;
        m.add_function(wrap_pyfunction!(make_letters_only_id, &m)?)?;
        m.add_function(wrap_pyfunction!(cached_corpus_file, &m)?)?;
        m.add(CONVERTER, converter_class(py)?)?;
        m.add("lyo", crate::lilyobjects::module(py)?)?;
        m.add("environLocal", environment(py)?)?;
        m.add(
            "SubConverter",
            crate::subconverters::module(py)?.getattr("SubConverter")?,
        )?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

/// `music21.lily`: the package, holding both modules.
pub(crate) fn package(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, "music21.lily")?;
        m.add("lilyObjects", crate::lilyobjects::module(py)?)?;
        m.add("translate", module(py)?)?;
        m.add("__all__", ["lilyObjects", "translate"])?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

fn environment(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    crate::environment::module(py)?
        .getattr("Environment")?
        .call1(("lily.translate",))
}

/// A corpus work, parsed once: what music21's examples here read.
#[pyfunction]
#[pyo3(name = "_getCachedCorpusFile")]
fn cached_corpus_file<'py>(py: Python<'py>, keyName: &str) -> PyResult<Bound<'py, PyAny>> {
    static PARSED: PyOnceLock<Py<PyDict>> = PyOnceLock::new();
    let parsed = PARSED
        .get_or_try_init(py, || Ok::<_, PyErr>(PyDict::new(py).unbind()))?
        .bind(py);
    if let Some(found) = parsed.get_item(keyName)? {
        return Ok(found);
    }
    let found = crate::corpus::module(py)?.call_method1("parse", (keyName,))?;
    parsed.set_item(keyName, &found)?;
    Ok(found)
}

/// An id made of letters alone: each other character becomes the letter
/// its code point comes to, counted round the alphabet.
#[pyfunction]
#[pyo3(name = "makeLettersOnlyId")]
fn make_letters_only_id(inputString: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(letters_only(&inputString.str()?.to_string()))
}

fn letters_only(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character.is_alphabetic() {
                return character;
            }
            char::from_u32(character as u32 % 26 + 97).unwrap_or('a')
        })
        .collect()
}

/// `LilypondConverter`, as a class with its methods Rust.
fn converter_class(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let probe = PyModule::new(py, MODULE)?;
    let accidentals = PyDict::new(py);
    for (name, ending) in ACCIDENTALS {
        accidentals.set_item(name, ending)?;
    }
    let barlines = PyDict::new(py);
    for (name, bar) in BARLINES {
        barlines.set_item(name, bar)?;
    }
    let mut members: Vec<(&str, Bound<'_, PyAny>)> = vec![
        ("fictaDef", PyString::new(py, FICTA_DEF).into_any()),
        ("colorDef", PyString::new(py, COLOR_DEF).into_any()),
        (
            "simplePaperDefinitionScm",
            PyString::new(py, SIMPLE_PAPER).into_any(),
        ),
        (
            "transparencyStartScheme",
            PyString::new(py, TRANSPARENCY_START).into_any(),
        ),
        (
            "transparencyStopScheme",
            PyString::new(py, TRANSPARENCY_STOP).into_any(),
        ),
        ("bookHeader", PyString::new(py, BOOK_HEADER).into_any()),
        ("accidentalConvert", accidentals.into_any()),
        ("barlineDict", barlines.into_any()),
    ];
    let functions = [
        ("__init__", wrap_pyfunction!(init, &probe)?),
        ("findLilyExec", wrap_pyfunction!(find_lily_exec, &probe)?),
        ("setupTools", wrap_pyfunction!(setup_tools, &probe)?),
        ("newContext", wrap_pyfunction!(new_context, &probe)?),
        ("restoreContext", wrap_pyfunction!(restore_context, &probe)?),
        (
            "textFromMusic21Object",
            wrap_pyfunction!(text_from_object, &probe)?,
        ),
        (
            "loadFromMusic21Object",
            wrap_pyfunction!(load_from_object, &probe)?,
        ),
        (
            "loadObjectFromOpus",
            wrap_pyfunction!(load_from_opus, &probe)?,
        ),
        (
            "loadObjectFromScore",
            wrap_pyfunction!(load_from_score, &probe)?,
        ),
        (
            "lyScoreBlockFromScore",
            wrap_pyfunction!(score_block, &probe)?,
        ),
        (
            "lyPartsAndOssiaInitFromScore",
            wrap_pyfunction!(parts_and_ossia_init, &probe)?,
        ),
        ("getLySpacersFromStream", wrap_pyfunction!(spacers, &probe)?),
        (
            "lyGroupedMusicListFromScoreWithParts",
            wrap_pyfunction!(grouped_from_score, &probe)?,
        ),
        (
            "lyNewLyricsFromStream",
            wrap_pyfunction!(new_lyrics, &probe)?,
        ),
        (
            "lyLyricElementFromM21Lyric",
            wrap_pyfunction!(lyric_element, &probe)?,
        ),
        (
            "lySequentialMusicFromStream",
            wrap_pyfunction!(sequential_music, &probe)?,
        ),
        (
            "lyPrefixCompositeMusicFromStream",
            wrap_pyfunction!(prefix_from_stream, &probe)?,
        ),
        (
            "appendObjectsToContextFromStream",
            wrap_pyfunction!(append_stream, &probe)?,
        ),
        (
            "appendM21ObjectToContext",
            wrap_pyfunction!(append_object, &probe)?,
        ),
        (
            "appendContextFromNoteOrRest",
            wrap_pyfunction!(append_note_or_rest, &probe)?,
        ),
        (
            "appendContextFromChord",
            wrap_pyfunction!(append_chord, &probe)?,
        ),
        (
            "contextContents",
            wrap_pyfunction!(context_contents, &probe)?,
        ),
        (
            "setContextContents",
            wrap_pyfunction!(set_context_contents, &probe)?,
        ),
        (
            "lySimpleMusicFromNoteOrRest",
            wrap_pyfunction!(simple_from_note, &probe)?,
        ),
        (
            "appendBeamCode",
            wrap_pyfunction!(append_beam_code, &probe)?,
        ),
        (
            "appendStemCode",
            wrap_pyfunction!(append_stem_code, &probe)?,
        ),
        (
            "lySimpleMusicFromChord",
            wrap_pyfunction!(simple_from_chord, &probe)?,
        ),
        (
            "postEventsFromObject",
            wrap_pyfunction!(post_events, &probe)?,
        ),
        ("lyPitchFromPitch", wrap_pyfunction!(ly_pitch, &probe)?),
        ("baseNameFromPitch", wrap_pyfunction!(base_name, &probe)?),
        (
            "octaveCharactersFromPitch",
            wrap_pyfunction!(octave_characters, &probe)?,
        ),
        (
            "lyMultipliedDurationFromDuration",
            wrap_pyfunction!(multiplied_duration, &probe)?,
        ),
        (
            "lyEmbeddedScmFromClef",
            wrap_pyfunction!(scm_from_clef, &probe)?,
        ),
        (
            "lyEmbeddedScmFromKeySignature",
            wrap_pyfunction!(scm_from_key, &probe)?,
        ),
        (
            "lyEmbeddedScmFromTimeSignature",
            wrap_pyfunction!(scm_from_meter, &probe)?,
        ),
        (
            "lyEmbeddedScmFromMetronomeMark",
            wrap_pyfunction!(scm_from_tempo, &probe)?,
        ),
        (
            "setContextForTupletStart",
            wrap_pyfunction!(tuplet_start, &probe)?,
        ),
        (
            "setContextForTimeFraction",
            wrap_pyfunction!(time_fraction, &probe)?,
        ),
        (
            "setContextForTupletStop",
            wrap_pyfunction!(tuplet_stop, &probe)?,
        ),
        (
            "appendContextFromVariant",
            wrap_pyfunction!(append_variant, &probe)?,
        ),
        (
            "lyPrefixCompositeMusicFromVariant",
            wrap_pyfunction!(prefix_from_variant, &probe)?,
        ),
        (
            "lyOssiaMusicFromVariant",
            wrap_pyfunction!(ossia_from_variant, &probe)?,
        ),
        (
            "setHeaderFromMetadata",
            wrap_pyfunction!(header_from_metadata, &probe)?,
        ),
        ("closeMeasure", wrap_pyfunction!(close_measure, &probe)?),
        (
            "getSchemeForPadding",
            wrap_pyfunction!(padding_scheme, &probe)?,
        ),
        ("writeLyFile", wrap_pyfunction!(write_ly_file, &probe)?),
        (
            "runThroughLily",
            wrap_pyfunction!(run_through_lily, &probe)?,
        ),
        ("createPDF", wrap_pyfunction!(create_pdf, &probe)?),
        ("showPDF", wrap_pyfunction!(show_pdf, &probe)?),
        ("createPNG", wrap_pyfunction!(create_png, &probe)?),
        ("showPNG", wrap_pyfunction!(show_png, &probe)?),
        ("createSVG", wrap_pyfunction!(create_svg, &probe)?),
        ("showSVG", wrap_pyfunction!(show_svg, &probe)?),
    ];
    for (name, function) in functions {
        members.push((name, method(function.into_any(), MODULE, CONVERTER, name)?));
    }
    new_class(
        py,
        MODULE,
        CONVERTER,
        "Translates music21 objects to LilyPond, and runs LilyPond on them.",
        vec![py.get_type::<pyo3::types::PyAny>().into_any()],
        members,
    )
}

// ------------------------------------------------------------- helpers

/// An object of `lily.lilyObjects` built from keywords.
fn lyo<'py>(
    py: Python<'py>,
    name: &str,
    keywords: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    make(py, name, keywords)
}

fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    PyString::new(py, value).into_any()
}

fn none(py: Python<'_>) -> Bound<'_, PyAny> {
    py.None().into_bound(py)
}

/// A `LyEmbeddedScm` holding text.
fn scm<'py>(py: Python<'py>, content: &str) -> PyResult<Bound<'py, PyAny>> {
    ly_class(py, "LyEmbeddedScm")?.call1((content,))
}

/// Whether an object is of a class by name: music21's `classes`, which a
/// music21 object and this package's answer alike.
fn is_a(object: &Bound<'_, PyAny>, class: &str) -> PyResult<bool> {
    let Ok(classes) = object.getattr("classes") else {
        return Ok(false);
    };
    classes.contains(class)
}

fn truthy_attribute(object: &Bound<'_, PyAny>, name: &str) -> bool {
    object
        .getattr(name)
        .and_then(|value| value.is_truthy())
        .unwrap_or(false)
}

/// `getattr(object, name, False) is True`.
fn is_true(object: &Bound<'_, PyAny>, name: &str) -> bool {
    object
        .getattr(name)
        .ok()
        .and_then(|value| value.extract::<bool>().ok())
        .unwrap_or(false)
        && object
            .getattr(name)
            .is_ok_and(|value| value.is_instance_of::<pyo3::types::PyBool>())
}

fn quote(text: &str) -> String {
    format!("\"{}\" ", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Appends to the current context's contents and makes the context the
/// object's parent.
fn append_here(slf: &Bound<'_, PyAny>, object: &Bound<'_, PyAny>) -> PyResult<()> {
    context_contents(slf)?.call_method1("append", (object,))?;
    object.call_method1("setParent", (slf.getattr("context")?,))?;
    Ok(())
}

fn first_tuplet<'py>(object: &Bound<'py, PyAny>) -> PyResult<Option<Bound<'py, PyAny>>> {
    let tuplets = object.getattr("duration")?.getattr("tuplets")?;
    if !tuplets.is_truthy()? {
        return Ok(None);
    }
    Ok(Some(tuplets.get_item(0)?))
}

// ------------------------------------------------------- set up and tools

/// Sets up an empty tree and asks the installed LilyPond its version.
#[pyfunction]
fn init(slf: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    slf.setattr("majorVersion", "1")?;
    slf.setattr("minorVersion", "0")?;
    slf.setattr("versionString", "1.0")?;
    slf.setattr("backend", "ps")?;
    slf.setattr("versionScheme", ly_class(py, "LyEmbeddedScm")?.call0()?)?;
    slf.setattr("headerScheme", ly_class(py, "LyEmbeddedScm")?.call0()?)?;
    slf.setattr("backendString", "--backend=")?;
    slf.setattr("LILYEXEC", "")?;
    let top = ly_class(py, "LyLilypondTop")?.call0()?;
    slf.setattr("topLevelObject", &top)?;
    slf.call_method0("setupTools")?;
    slf.setattr("context", &top)?;
    slf.setattr("storedContexts", PyList::empty(py))?;
    slf.setattr("doNotOutput", PyList::empty(py))?;
    slf.setattr("currentMeasure", none(py))?;
    slf.setattr("addedVariants", PyList::empty(py))?;
    slf.setattr("variantColors", PyList::new(py, VARIANT_COLORS)?)?;
    slf.setattr("coloredVariants", false)?;
    slf.setattr("variantMode", false)?;
    slf.setattr("tempName", none(py))?;
    slf.setattr("inWord", false)?;
    Ok(())
}

/// The LilyPond program: the one music21's settings name where it exists,
/// else where each platform installs it, else `lilypond` on the path.
#[pyfunction]
fn find_lily_exec(slf: &Bound<'_, PyAny>) -> PyResult<String> {
    let py = slf.py();
    let set = environment(py)?
        .get_item("lilypondPath")
        .ok()
        .filter(|path| !path.is_none());
    let exists = |path: &str| std::path::Path::new(path).exists();
    let found = match set {
        Some(path)
            if path
                .call_method0("exists")
                .and_then(|found| found.is_truthy())
                .unwrap_or(false) =>
        {
            path.str()?.to_string()
        }
        _ if cfg!(target_os = "macos") => {
            let app = "/Applications/Lilypond.app/Contents/Resources/bin/lilypond";
            if exists(app) {
                app.to_string()
            } else {
                "lilypond".to_string()
            }
        }
        _ if cfg!(windows) => {
            let program = if exists("c:/Program Files (x86)") {
                r"c:/Program\ Files\ (x86)/lilypond/usr/bin/lilypond"
            } else {
                r"c:/Program\ Files/lilypond/usr/bin/lilypond"
            };
            if exists(program) || exists(&format!("{program}.exe")) {
                program.to_string()
            } else {
                "lilypond".to_string()
            }
        }
        _ => "lilypond".to_string(),
    };
    slf.setattr("LILYEXEC", &found)?;
    Ok(found)
}

/// Asks LilyPond its version, for the file's `\version` line and the
/// backend option it takes.
#[pyfunction]
fn setup_tools(slf: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    let program: String = slf.call_method0("findLilyExec")?.extract()?;
    let ran = std::process::Command::new(&program)
        .arg("--version")
        .output();
    let Ok(ran) = ran else {
        return Err(translate_error(
            "Cannot find a copy of LilyPond installed on your system. Please be sure it is \
             installed. And that your environment.UserSettings()['lilypondPath'] is set to \
             find it.",
        ));
    };
    let said = String::from_utf8_lossy(&ran.stdout).to_string();
    let version = said.split_whitespace().nth(2).unwrap_or("0.0");
    let mut pieces = version.split('.');
    let major = pieces.next().unwrap_or("0").to_string();
    let minor = pieces.next().unwrap_or("0").to_string();
    slf.setattr("majorVersion", &major)?;
    slf.setattr("minorVersion", &minor)?;
    let version_string = format!("\\version {}", quote(&format!("{major}.{minor}")));
    slf.setattr("versionString", &version_string)?;
    slf.setattr("versionScheme", scm(py, &version_string)?)?;
    slf.setattr("headerScheme", scm(py, BOOK_HEADER)?)?;
    slf.setattr("backend", "ps")?;
    let major: i64 = major.parse().unwrap_or(0);
    let minor: i64 = minor.parse().unwrap_or(0);
    let backend = if major >= 2 && minor >= 11 {
        "-dbackend="
    } else {
        "--backend="
    };
    slf.setattr("backendString", backend)?;
    Ok(())
}

#[pyfunction]
fn new_context(slf: &Bound<'_, PyAny>, newContext: &Bound<'_, PyAny>) -> PyResult<()> {
    slf.getattr("storedContexts")?
        .call_method1("append", (slf.getattr("context")?,))?;
    slf.setattr("context", newContext)
}

#[pyfunction]
fn restore_context(slf: &Bound<'_, PyAny>) -> PyResult<()> {
    let stored = slf.getattr("storedContexts")?;
    if stored.len()? == 0 {
        return slf.setattr("context", slf.getattr("topLevelObject")?);
    }
    slf.setattr("context", stored.call_method0("pop")?)
}

// ------------------------------------------------------ whole trees

/// The whole LilyPond file for an object, blank lines dropped.
#[pyfunction]
fn text_from_object(slf: &Bound<'_, PyAny>, m21ObjectIn: &Bound<'_, PyAny>) -> PyResult<String> {
    let py = slf.py();
    slf.call_method1("loadFromMusic21Object", (m21ObjectIn,))?;
    let written = slf.getattr("topLevelObject")?.str()?.to_string();
    let collapsed = py
        .import("re")?
        .call_method1("sub", (r"\s*\n\s*\n", "\n", written))?
        .str()?
        .to_string();
    Ok(collapsed.trim().to_string())
}

/// Builds the tree for any object: a score or an opus as it is, anything
/// else inside a part inside a score.
#[pyfunction]
fn load_from_object(slf: &Bound<'_, PyAny>, m21ObjectIn: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    let mut object = m21ObjectIn.clone();
    let stream = py.import("music21.stream")?;
    if is_a(&object, "Stream")? && has_variants(&object)? {
        let variant = py.import("music21.variant")?;
        let kwargs = PyDict::new(py);
        kwargs.set_item("recurse", true)?;
        object = variant.call_method("makeAllVariantsReplacements", (&object,), Some(&kwargs))?;
        variant.call_method1("makeVariantBlocks", (&object,))?;
    }
    let kwargs = PyDict::new(py);
    kwargs.set_item("makeNotation", false)?;
    if is_a(&object, "Score")? {
        slf.call_method("loadObjectFromScore", (&object,), Some(&kwargs))?;
        return Ok(());
    }
    if is_a(&object, "Opus")? {
        slf.call_method("loadObjectFromOpus", (&object,), Some(&kwargs))?;
        return Ok(());
    }
    let score = stream.getattr("Score")?.call0()?;
    if !is_a(&object, "Stream")? || is_a(&object, "Measure")? || is_a(&object, "Voice")? {
        let part = stream.getattr("Part")?.call0()?;
        part.call_method1("insert", (0, &object))?;
        score.call_method1("insert", (0, part))?;
    } else {
        score.call_method1("insert", (0, &object))?;
    }
    slf.call_method("loadObjectFromScore", (score,), Some(&kwargs))?;
    Ok(())
}

fn has_variants(stream: &Bound<'_, PyAny>) -> PyResult<bool> {
    let variant = stream.py().import("music21.variant")?.getattr("Variant")?;
    stream
        .call_method1("getElementsByClass", (variant,))?
        .is_truthy()
}

/// The file's head: the version, the book preamble and the color
/// function.
fn file_head<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    Ok(vec![
        slf.getattr("versionScheme")?,
        slf.getattr("headerScheme")?,
        scm(slf.py(), COLOR_DEF)?,
    ])
}

/// `\paper { }` and the layout that drops empty staves.
fn file_tail<'py>(py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let head = lyo(py, "LyOutputDefHead", &[("defType", text(py, "paper"))])?;
    let body = lyo(py, "LyOutputDefBody", &[("outputDefHead", head)])?;
    let output = lyo(py, "LyOutputDef", &[("outputDefBody", body)])?;
    Ok(vec![output, ly_class(py, "LyLayout")?.call0()?])
}

#[pyfunction]
#[pyo3(signature = (slf, opusIn, makeNotation = true))]
fn load_from_opus(
    slf: &Bound<'_, PyAny>,
    opusIn: &Bound<'_, PyAny>,
    makeNotation: bool,
) -> PyResult<()> {
    let py = slf.py();
    let mut contents = file_head(slf)?;
    for score in opusIn.getattr("scores")?.try_iter()? {
        let mut score = score?;
        if makeNotation {
            let kwargs = PyDict::new(py);
            kwargs.set_item("inPlace", false)?;
            score = score.call_method("makeNotation", (), Some(&kwargs))?;
        }
        let header = ly_class(py, "LyLilypondHeader")?.call0()?;
        let block = slf.call_method1("lyScoreBlockFromScore", (&score,))?;
        let metadata = score.getattr("metadata")?;
        if !metadata.is_none() {
            header_from_metadata(slf, Some(&metadata), Some(&header))?;
        }
        contents.push(header);
        contents.push(block);
    }
    contents.extend(file_tail(py)?);
    set_context_contents(slf, &PyList::new(py, contents)?.into_any())
}

#[pyfunction]
#[pyo3(signature = (slf, scoreIn, makeNotation = true))]
fn load_from_score(
    slf: &Bound<'_, PyAny>,
    scoreIn: &Bound<'_, PyAny>,
    makeNotation: bool,
) -> PyResult<()> {
    let py = slf.py();
    let mut score = scoreIn.clone();
    if makeNotation {
        let kwargs = PyDict::new(py);
        kwargs.set_item("inPlace", false)?;
        score = score.call_method("makeNotation", (), Some(&kwargs))?;
    }
    let mut contents = file_head(slf)?;
    let header = ly_class(py, "LyLilypondHeader")?.call0()?;
    let block = slf.call_method1("lyScoreBlockFromScore", (&score,))?;
    contents.push(header.clone());
    contents.push(block);
    contents.extend(file_tail(py)?);
    let metadata = score.getattr("metadata")?;
    if !metadata.is_none() {
        header_from_metadata(slf, Some(&metadata), Some(&header))?;
    }
    set_context_contents(slf, &PyList::new(py, contents)?.into_any())
}

#[pyfunction]
fn score_block<'py>(
    slf: &Bound<'py, PyAny>,
    scoreIn: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let composite = ly_class(py, "LyCompositeMusic")?.call0()?;
    new_context(slf, &composite)?;
    let has_parts = scoreIn.hasattr("parts")? && scoreIn.getattr("parts")?.is_truthy()?;
    if has_parts {
        let grouped = if has_variants(scoreIn)? {
            let init = slf.call_method1("lyPartsAndOssiaInitFromScore", (scoreIn,))?;
            let kwargs = PyDict::new(py);
            kwargs.set_item("scoreInit", init)?;
            slf.call_method(
                "lyGroupedMusicListFromScoreWithParts",
                (scoreIn,),
                Some(&kwargs),
            )?
        } else {
            slf.call_method1("lyGroupedMusicListFromScoreWithParts", (scoreIn,))?
        };
        composite.setattr("groupedMusicList", grouped)?;
    } else {
        let prefix = slf.call_method1("lyPrefixCompositeMusicFromStream", (scoreIn,))?;
        composite.setattr("prefixCompositeMusic", prefix)?;
    }
    let music = lyo(py, "LyMusic", &[("compositeMusic", composite)])?;
    let body = lyo(py, "LyScoreBody", &[("music", music)])?;
    let block = lyo(py, "LyScoreBlock", &[("scoreBody", body)])?;
    restore_context(slf)?;
    Ok(block)
}

/// Each part's staff and each of its variants' staves, started with
/// spacers as long as the part.
#[pyfunction]
fn parts_and_ossia_init<'py>(
    slf: &Bound<'py, PyAny>,
    scoreIn: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let music_list = ly_class(py, "LyMusicList")?.call0()?;
    let mut contents = Vec::new();
    let variant_class = py.import("music21.variant")?.getattr("Variant")?;
    for part in scoreIn.getattr("parts")?.try_iter()? {
        let part = part?;
        let part_id = letters_only(&part.getattr("id")?.str()?.to_string());
        let spacer = spacers(slf, &part, true)?;
        let music = format!("{{ \\stopStaff {spacer}}}");
        let staff = |id: String| {
            lyo(
                py,
                "LyPrefixCompositeMusic",
                &[
                    ("type", text(py, "new")),
                    ("optionalId", ly_class(py, "LyOptionalId")?.call1((id,))?),
                    ("simpleString", text(py, "Staff")),
                    ("music", text(py, &music)),
                ],
            )
        };
        contents.push(staff(part_id.clone())?);

        let mut added: Vec<String> = Vec::new();
        for variant in part
            .call_method1("getElementsByClass", (&variant_class,))?
            .try_iter()?
        {
            let name = variant?.getattr("groups")?.get_item(0)?.str()?.to_string();
            if added.contains(&name) {
                continue;
            }
            slf.getattr("addedVariants")?
                .call_method1("append", (&name,))?;
            added.push(name.clone());
            let variant_staff = staff(format!("{}{part_id}", letters_only(&name)))?;
            let modifications: Vec<String> = VARIANT_STAFF
                .iter()
                .map(|line| line.replace("{}", &part_id))
                .collect();
            let modification =
                ly_class(py, "LyContextModification")?.call1((PyList::new(py, modifications)?,))?;
            variant_staff.setattr("optionalContextMod", modification)?;
            contents.push(variant_staff);
        }
    }
    music_list.setattr("contents", PyList::new(py, contents)?)?;
    Ok(music_list)
}

/// A spacer as long as each measure.
#[pyfunction]
#[pyo3(signature = (slf, streamIn, measuresOnly = true))]
fn spacers(
    slf: &Bound<'_, PyAny>,
    streamIn: &Bound<'_, PyAny>,
    measuresOnly: bool,
) -> PyResult<String> {
    let _ = measuresOnly;
    let mut out = String::new();
    for element in streamIn.try_iter()? {
        let element = element?;
        if !is_a(&element, "Measure")? {
            continue;
        }
        let duration = element.getattr("duration")?;
        if duration.getattr("quarterLength")?.extract::<f64>()? == 0.0 {
            continue;
        }
        match multiplied_duration(slf, &duration) {
            Ok(written) => out.push_str(&format!("s{}", written.str()?)),
            Err(_) => {
                for component in duration.getattr("components")?.try_iter()? {
                    let written = multiplied_duration(slf, &component?)?;
                    out.push_str(&format!("s{}", written.str()?));
                }
            }
        }
    }
    Ok(out)
}

#[pyfunction]
#[pyo3(signature = (slf, scoreIn, scoreInit = None))]
fn grouped_from_score<'py>(
    slf: &Bound<'py, PyAny>,
    scoreIn: &Bound<'py, PyAny>,
    scoreInit: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let grouped = ly_class(py, "LyGroupedMusicList")?.call0()?;
    let simultaneous = ly_class(py, "LySimultaneousMusic")?.call0()?;
    let music_list = ly_class(py, "LyMusicList")?.call0()?;
    simultaneous.setattr("musicList", &music_list)?;
    grouped.setattr("simultaneousMusic", &simultaneous)?;
    new_context(slf, &music_list)?;
    let mut contents = Vec::new();
    match scoreInit.filter(|init| !init.is_none()) {
        None => {
            for part in scoreIn.getattr("parts")?.try_iter()? {
                contents.push(slf.call_method1("lyPrefixCompositeMusicFromStream", (part?,))?);
            }
        }
        Some(init) => {
            contents.push(init.clone());
            for part in scoreIn.getattr("parts")?.try_iter()? {
                let kwargs = PyDict::new(py);
                kwargs.set_item("type", "context")?;
                kwargs.set_item("beforeMatter", "startStaff")?;
                contents.push(slf.call_method(
                    "lyPrefixCompositeMusicFromStream",
                    (part?,),
                    Some(&kwargs),
                )?);
            }
        }
    }
    restore_context(slf)?;
    music_list.setattr("contents", PyList::new(py, contents)?)?;
    Ok(grouped)
}

// --------------------------------------------------------------- lyrics

#[pyfunction]
#[pyo3(signature = (slf, streamIn, streamId = None, alignment = "alignBelowContext"))]
fn new_lyrics<'py>(
    slf: &Bound<'py, PyAny>,
    streamIn: &Bound<'py, PyAny>,
    streamId: Option<String>,
    alignment: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let kwargs = PyDict::new(py);
    kwargs.set_item("skipTies", true)?;
    let lyrics = streamIn.call_method("lyrics", (), Some(&kwargs))?;
    let stream_id = streamId.unwrap_or(letters_only(&streamIn.getattr("id")?.str()?.to_string()));
    let stream_id = format!("#{}", quote(&stream_id));

    let mut numbers: Vec<Bound<'py, PyAny>> = lyrics
        .call_method0("keys")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    let sorted = py
        .import("builtins")?
        .call_method1("sorted", (PyList::new(py, &numbers)?,))?;
    numbers = sorted.try_iter()?.collect::<PyResult<_>>()?;
    let mut grouped_lists = Vec::new();
    for number in numbers {
        let alignment = lyo(
            py,
            "LyPropertyOperation",
            &[
                ("mode", text(py, "set")),
                ("value1", text(py, alignment)),
                ("value2", text(py, &stream_id)),
            ],
        )?;
        let mut lyric_list = vec![alignment];
        slf.setattr("inWord", false)?;
        for element in lyrics.get_item(&number)?.try_iter()? {
            let element = element?;
            if element.is_instance_of::<PyList>() {
                continue;
            }
            lyric_list.push(slf.call_method1("lyLyricElementFromM21Lyric", (element,))?);
        }
        slf.setattr("inWord", false)?;
        let music_list = ly_class(py, "LyMusicList")?.call1((PyList::new(py, lyric_list)?,))?;
        let sequential = lyo(py, "LySequentialMusic", &[("musicList", music_list)])?;
        grouped_lists.push(lyo(
            py,
            "LyGroupedMusicList",
            &[("sequentialMusic", sequential)],
        )?);
    }
    lyo(
        py,
        "LyNewLyrics",
        &[(
            "groupedMusicLists",
            PyList::new(py, grouped_lists)?.into_any(),
        )],
    )
}

/// A lyric's syllable, quoted, with `--` after a word's first and middle
/// syllables and `__` after its last; `_` for no syllable.
#[pyfunction]
fn lyric_element<'py>(
    slf: &Bound<'py, PyAny>,
    m21Lyric: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let mut in_word: bool = slf.getattr("inWord")?.is_truthy()?;
    let written = if m21Lyric.is_none() {
        " _ ".to_string()
    } else {
        let lyric_text = m21Lyric.getattr("text")?;
        if lyric_text.is_none() {
            String::new()
        } else if lyric_text
            .extract::<String>()
            .is_ok_and(|text| text.is_empty())
        {
            " _ ".to_string()
        } else {
            let mut written = quote(&lyric_text.str()?.to_string()).trim_end().to_string();
            let syllabic = m21Lyric.getattr("syllabic")?;
            let syllabic = if syllabic.is_none() {
                String::new()
            } else {
                syllabic.str()?.to_string()
            };
            if syllabic == "end" {
                written.push_str("__");
                in_word = false;
            } else if syllabic == "begin" || syllabic == "middle" {
                written.push_str(" --");
                in_word = true;
            }
            written
        }
    };
    slf.setattr("inWord", in_word)?;
    ly_class(py, "LyLyricElement")?.call1((written,))
}

// ------------------------------------------------------------- streams

#[pyfunction]
#[pyo3(signature = (slf, streamIn, beforeMatter = None))]
fn sequential_music<'py>(
    slf: &Bound<'py, PyAny>,
    streamIn: &Bound<'py, PyAny>,
    beforeMatter: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let contents = PyList::empty(py);
    let music_list = lyo(
        py,
        "LyMusicList",
        &[("contents", contents.clone().into_any())],
    )?;
    let before = beforeMatter.cloned().unwrap_or_else(|| none(py));
    let sequential = lyo(
        py,
        "LySequentialMusic",
        &[("musicList", music_list.clone()), ("beforeMatter", before)],
    )?;
    new_context(slf, &music_list)?;
    slf.call_method1("appendObjectsToContextFromStream", (streamIn,))?;
    let closing = slf.call_method0("closeMeasure")?;
    if !closing.is_none() {
        contents.append(closing)?;
    }
    restore_context(slf)?;
    Ok(sequential)
}

#[pyfunction]
#[pyo3(signature = (slf, streamIn, contextType = None, r#type = None, beforeMatter = None))]
fn prefix_from_stream<'py>(
    slf: &Bound<'py, PyAny>,
    streamIn: &Bound<'py, PyAny>,
    contextType: Option<String>,
    r#type: Option<String>,
    beforeMatter: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let stream_id = letters_only(&streamIn.getattr("id")?.str()?.to_string());
    let optional_id = || ly_class(py, "LyOptionalId")?.call1((&stream_id,));
    let (context, id) = match contextType {
        Some(context) => (context, Some(optional_id()?)),
        None if is_a(streamIn, "Part")? => ("Staff".to_string(), Some(optional_id()?)),
        None => ("Voice".to_string(), None),
    };

    let mut modifications: Vec<String> = Vec::new();
    if streamIn
        .getattr("streamStatus")?
        .getattr("beams")?
        .extract::<bool>()
        .unwrap_or(false)
    {
        modifications.push("\\autoBeamOff ".to_string());
    }
    if let Ok(lines) = streamIn.getattr("staffLines")
        && lines.extract::<i64>().is_ok_and(|lines| lines != 5)
    {
        modifications.push(format!(
            "\\override StaffSymbol.line-count = #{}",
            lines.str()?
        ));
    }

    let kwargs = PyDict::new(py);
    kwargs.set_item("streamId", &stream_id)?;
    let lyrics = slf.call_method("lyNewLyricsFromStream", (streamIn,), Some(&kwargs))?;
    let kwargs = PyDict::new(py);
    kwargs.set_item("beforeMatter", beforeMatter)?;
    let sequential = slf.call_method("lySequentialMusicFromStream", (streamIn,), Some(&kwargs))?;
    let grouped = lyo(py, "LyGroupedMusicList", &[("sequentialMusic", sequential)])?;
    let composite = lyo(
        py,
        "LyCompositeMusic",
        &[("groupedMusicList", grouped), ("newLyrics", lyrics)],
    )?;
    let music = lyo(py, "LyMusic", &[("compositeMusic", composite)])?;
    let modification = if modifications.is_empty() {
        none(py)
    } else {
        ly_class(py, "LyContextModification")?.call1((PyList::new(py, modifications)?,))?
    };
    lyo(
        py,
        "LyPrefixCompositeMusic",
        &[
            ("type", text(py, r#type.as_deref().unwrap_or("new"))),
            ("optionalId", id.unwrap_or_else(|| none(py))),
            ("simpleString", text(py, &context)),
            ("optionalContextMod", modification),
            ("music", music),
        ],
    )
}

/// Appends everything in a stream to the context, what stands at one
/// offset together: voices side by side, variants with what they replace.
#[pyfunction]
fn append_stream(slf: &Bound<'_, PyAny>, streamObject: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    let iterator = py
        .import("music21.stream.iterator")?
        .getattr("OffsetIterator")?;
    for group in iterator.call1((streamObject,))?.try_iter()? {
        let group = group?;
        if group.len()? == 1 {
            let sole = group.get_item(0)?;
            sole.setattr("activeSite", streamObject)?;
            slf.call_method1("appendM21ObjectToContext", (sole,))?;
            continue;
        }
        let mut voices = Vec::new();
        let mut variants = Vec::new();
        let mut others = Vec::new();
        for element in group.try_iter()? {
            let element = element?;
            if is_a(&element, "Voice")? {
                voices.push(element);
            } else if is_a(&element, "Variant")? {
                variants.push(element);
            } else {
                element.setattr("activeSite", streamObject)?;
                others.push(element);
            }
        }
        if !variants.is_empty() {
            for variant in &variants {
                variant.setattr("activeSite", streamObject)?;
            }
            let kwargs = PyDict::new(py);
            kwargs.set_item("activeSite", streamObject)?;
            kwargs.set_item("coloredVariants", slf.getattr("coloredVariants")?)?;
            slf.call_method(
                "appendContextFromVariant",
                (PyList::new(py, &variants)?,),
                Some(&kwargs),
            )?;
        }
        if !voices.is_empty() {
            let grouped = ly_class(py, "LyGroupedMusicList")?.call0()?;
            let simultaneous = ly_class(py, "LySimultaneousMusic")?.call0()?;
            let music_list = ly_class(py, "LyMusicList")?.call0()?;
            simultaneous.setattr("musicList", &music_list)?;
            grouped.setattr("simultaneousMusic", &simultaneous)?;
            let skipped = slf.getattr("doNotOutput")?;
            let mut contents = Vec::new();
            for voice in voices {
                if skipped.contains(&voice)? {
                    continue;
                }
                contents.push(slf.call_method1("lyPrefixCompositeMusicFromStream", (voice,))?);
            }
            music_list.setattr("contents", PyList::new(py, contents)?)?;
            context_contents(slf)?.call_method1("append", (&grouped,))?;
            grouped.call_method1("setParent", (slf.getattr("context")?,))?;
        }
        for element in others {
            slf.call_method1("appendM21ObjectToContext", (element,))?;
        }
    }
    Ok(())
}

/// Appends one object of any kind to the context.
#[pyfunction]
fn append_object(slf: &Bound<'_, PyAny>, thisObject: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    if slf.getattr("doNotOutput")?.contains(thisObject)? {
        return Ok(());
    }

    // A complex duration is written as the objects it splits into.
    if !is_a(thisObject, "Stream")?
        && thisObject
            .getattr("duration")?
            .getattr("type")?
            .extract::<String>()?
            == "complex"
    {
        for part in split_at_durations(thisObject)? {
            slf.call_method1("appendM21ObjectToContext", (part,))?;
        }
        return Ok(());
    }

    let context = slf.getattr("context")?;
    let contents = context_contents(slf)?;
    if is_true(thisObject, "startTransparency") {
        contents.call_method1("append", (scm(py, TRANSPARENCY_START)?,))?;
    }
    let append = |object: Bound<'_, PyAny>| -> PyResult<()> {
        contents.call_method1("append", (&object,))?;
        object.call_method1("setParent", (&context,))?;
        Ok(())
    };
    if is_a(thisObject, "Measure")? {
        // LilyPond puts no braces round a measure, but closes each.
        let closing = slf.call_method0("closeMeasure")?;
        if !closing.is_none() {
            append(closing)?;
        }
        let padding = slf.call_method1("getSchemeForPadding", (thisObject,))?;
        if !padding.is_none() {
            append(padding)?;
        }
        slf.call_method1("appendObjectsToContextFromStream", (thisObject,))?;
        slf.setattr("currentMeasure", thisObject)?;
    } else if is_a(thisObject, "Stream")? {
        append(slf.call_method1("lyPrefixCompositeMusicFromStream", (thisObject,))?)?;
    } else if is_a(thisObject, "Note")? || is_a(thisObject, "Rest")? {
        slf.call_method1("appendContextFromNoteOrRest", (thisObject,))?;
    } else if is_a(thisObject, "Chord")? {
        slf.call_method1("appendContextFromChord", (thisObject,))?;
    } else if is_a(thisObject, "Clef")? {
        append(slf.call_method1("lyEmbeddedScmFromClef", (thisObject,))?)?;
    } else if is_a(thisObject, "KeySignature")? {
        append(slf.call_method1("lyEmbeddedScmFromKeySignature", (thisObject,))?)?;
    } else if is_a(thisObject, "TimeSignature")? && !truthy_attribute(slf, "variantMode") {
        append(slf.call_method1("lyEmbeddedScmFromTimeSignature", (thisObject,))?)?;
    } else if is_a(thisObject, "MetronomeMark")? {
        let written = slf.call_method1("lyEmbeddedScmFromMetronomeMark", (thisObject,))?;
        if !written.is_none() {
            append(written)?;
        }
    } else if is_a(thisObject, "Variant")? {
        let kwargs = PyDict::new(py);
        kwargs.set_item("coloredVariants", slf.getattr("coloredVariants")?)?;
        slf.call_method("appendContextFromVariant", (thisObject,), Some(&kwargs))?;
    } else if is_a(thisObject, "SystemLayout")? {
        append(scm(py, "\\break")?)?;
    } else if is_a(thisObject, "PageLayout")? {
        append(scm(py, "\\pageBreak")?)?;
    }
    if is_true(thisObject, "stopTransparency") {
        contents.call_method1("append", (scm(py, TRANSPARENCY_STOP)?,))?;
    }
    Ok(())
}

/// An object of a complex duration as one copy per written value, notes
/// tied across them: music21's `splitAtDurations`, where the object has
/// none of its own.
fn split_at_durations<'py>(object: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    if object.hasattr("splitAtDurations")? {
        return object
            .call_method0("splitAtDurations")?
            .try_iter()?
            .collect();
    }
    let py = object.py();
    let deepcopy = py.import("copy")?.getattr("deepcopy")?;
    let duration_class = py.import("music21.duration")?.getattr("Duration")?;
    let tie_class = py.import("music21.tie")?.getattr("Tie")?;
    let components: Vec<Bound<'py, PyAny>> = object
        .getattr("duration")?
        .getattr("components")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    let last = components.len().saturating_sub(1);
    let mut pieces = Vec::new();
    for (at, component) in components.iter().enumerate() {
        let piece = deepcopy.call1((object,))?;
        let kwargs = PyDict::new(py);
        kwargs.set_item("type", component.getattr("type")?)?;
        kwargs.set_item("dots", component.getattr("dots")?)?;
        piece.setattr("duration", duration_class.call((), Some(&kwargs))?)?;

        // Each piece of a note but the last is tied to the next.
        if is_a(object, "NotRest")? && last > 0 {
            let kind = match at {
                0 => "start",
                _ if at == last => "stop",
                _ => "continue",
            };
            piece.setattr("tie", tie_class.call1((kind,))?)?;
        }
        pieces.push(piece);
    }
    Ok(pieces)
}

#[pyfunction]
fn append_note_or_rest(slf: &Bound<'_, PyAny>, noteOrRest: &Bound<'_, PyAny>) -> PyResult<()> {
    // Grace notes are left out until LilyPond's are written.
    if noteOrRest
        .getattr("duration")?
        .getattr("isGrace")?
        .is_truthy()?
    {
        return Ok(());
    }
    tuplet_start(slf, noteOrRest)?;
    slf.call_method1("appendBeamCode", (noteOrRest,))?;
    slf.call_method1("appendStemCode", (noteOrRest,))?;
    let music = slf.call_method1("lySimpleMusicFromNoteOrRest", (noteOrRest,))?;
    append_here(slf, &music)?;
    tuplet_stop(slf, noteOrRest)
}

#[pyfunction]
fn append_chord(slf: &Bound<'_, PyAny>, chordIn: &Bound<'_, PyAny>) -> PyResult<()> {
    tuplet_start(slf, chordIn)?;
    slf.call_method1("appendBeamCode", (chordIn,))?;
    slf.call_method1("appendStemCode", (chordIn,))?;
    let music = slf.call_method1("lySimpleMusicFromChord", (chordIn,))?;
    append_here(slf, &music)?;
    tuplet_stop(slf, chordIn)
}

#[pyfunction]
fn context_contents<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let context = slf.getattr("context")?;
    match context.getattr("contents") {
        Ok(contents) if !contents.is_none() => Ok(contents),
        _ => Err(translate_error(format!(
            "Cannot get a currentMusicList from contextObject {}",
            context.repr()?
        ))),
    }
}

#[pyfunction]
fn set_context_contents(slf: &Bound<'_, PyAny>, contents: &Bound<'_, PyAny>) -> PyResult<()> {
    slf.getattr("context")?.setattr("contents", contents)
}

// -------------------------------------------------- notes and chords

/// The color overrides a note's style asks for, where it is shown.
fn color_parts(object: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    if !object.getattr("hasStyleInformation")?.is_truthy()? {
        return Ok(Vec::new());
    }
    let style = object.getattr("style")?;
    let color = style.getattr("color")?;
    let hidden = style.getattr("hideObjectOnPrint")?;
    if !color.is_truthy()? || hidden.is_truthy()? {
        return Ok(Vec::new());
    }
    let color = quote(&color.str()?.to_string()).trim_end().to_string();
    Ok(vec![
        format!("\\override NoteHead.color = {color}\n"),
        format!("\\override Stem.color = {color}\n"),
    ])
}

fn is_hidden(object: &Bound<'_, PyAny>) -> PyResult<bool> {
    if !object.getattr("hasStyleInformation")?.is_truthy()? {
        return Ok(false);
    }
    object
        .getattr("style")?
        .getattr("hideObjectOnPrint")?
        .is_truthy()
}

/// `! ` for an accidental always shown, `? ` for one in parentheses.
fn accidental_marks(pitch: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let accidental = pitch.getattr("accidental")?;
    let mut marks = Vec::new();
    if accidental.is_none() {
        return Ok(marks);
    }
    if accidental
        .getattr("displayType")?
        .extract::<String>()
        .is_ok_and(|kind| kind == "always")
    {
        marks.push("! ".to_string());
    }
    if accidental
        .getattr("displayStyle")?
        .extract::<String>()
        .is_ok_and(|style| style == "parentheses")
    {
        marks.push("? ".to_string());
    }
    Ok(marks)
}

#[pyfunction]
fn simple_from_note<'py>(
    slf: &Bound<'py, PyAny>,
    noteOrRest: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let parts = PyList::empty(py);
    for part in color_parts(noteOrRest)? {
        parts.append(part)?;
    }
    if is_a(noteOrRest, "Note")? {
        if is_hidden(noteOrRest)? {
            parts.append("s ")?;
        } else {
            let pitch = noteOrRest.getattr("pitch")?;
            parts.append(slf.call_method1("lyPitchFromPitch", (&pitch,))?)?;
            for mark in accidental_marks(&pitch)? {
                parts.append(mark)?;
            }
        }
    } else if is_a(noteOrRest, "Rest")? {
        parts.append(if is_hidden(noteOrRest)? { "s " } else { "r " })?;
    }
    parts.append(slf.call_method1(
        "lyMultipliedDurationFromDuration",
        (noteOrRest.getattr("duration")?,),
    )?)?;
    if let Ok(beams) = noteOrRest.getattr("beams")
        && beams.is_truthy()?
    {
        let kind = beams
            .getattr("beamsList")?
            .get_item(0)?
            .getattr("type")?
            .extract::<String>()?;
        if kind == "start" {
            parts.append("[ ")?;
        } else if kind == "stop" {
            parts.append("] ")?;
        }
    }
    let element = lyo(py, "LySimpleElement", &[("parts", parts.into_any())])?;
    let events = slf.call_method1("postEventsFromObject", (noteOrRest,))?;
    let chord = ly_class(py, "LyEventChord")?.call(
        (element,),
        Some(&[("postEvents", events)].into_py_dict(py)?),
    )?;
    lyo(py, "LySimpleMusic", &[("eventChord", chord)])
}

/// `\set stemLeftBeamCount` and `stemRightBeamCount` for an object's
/// beams.
#[pyfunction]
fn append_beam_code(slf: &Bound<'_, PyAny>, noteOrChord: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    let Ok(beams) = noteOrChord.getattr("beams") else {
        return Ok(());
    };
    if beams.is_none() {
        return Ok(());
    }
    let (mut left, mut right) = (0, 0);
    for beam in beams.try_iter()? {
        let beam = beam?;
        match beam
            .getattr("type")?
            .extract::<String>()
            .unwrap_or_default()
            .as_str()
        {
            "start" => right += 1,
            "continue" => {
                right += 1;
                left += 1;
            }
            "stop" => left += 1,
            "partial" => {
                if beam
                    .getattr("direction")?
                    .extract::<String>()
                    .is_ok_and(|direction| direction == "left")
                {
                    left += 1;
                } else {
                    right += 1;
                }
            }
            _ => {}
        }
    }
    if left > 0 {
        append_here(
            slf,
            &scm(py, &format!("\\set stemLeftBeamCount = #{left}"))?,
        )?;
    }
    if right > 0 {
        append_here(
            slf,
            &scm(py, &format!("\\set stemRightBeamCount = #{right}"))?,
        )?;
    }
    Ok(())
}

#[pyfunction]
fn append_stem_code(slf: &Bound<'_, PyAny>, noteOrChord: &Bound<'_, PyAny>) -> PyResult<()> {
    let Ok(direction) = noteOrChord.getattr("stemDirection") else {
        return Ok(());
    };
    if direction.is_none() {
        return Ok(());
    }
    let direction = direction.str()?.to_string().to_uppercase();
    if direction != "UP" && direction != "DOWN" {
        return Ok(());
    }
    let written = format!("\\once \\override Stem.direction = #{direction} ");
    append_here(slf, &scm(slf.py(), &written)?)
}

#[pyfunction]
fn simple_from_chord<'py>(
    slf: &Bound<'py, PyAny>,
    chordObj: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    slf.call_method1("appendBeamCode", (chordObj,))?;
    let hidden = chordObj.getattr("hasStyleInformation")?.is_truthy()?
        && chordObj
            .getattr("style")?
            .getattr("hideObjectOnPrint")?
            .extract::<bool>()
            .unwrap_or(false);
    let body = if hidden {
        ly_class(py, "LyPitch")?.call1(("s ", ""))?
    } else {
        slf.call_method1("appendStemCode", (chordObj,))?;
        let mut elements = Vec::new();
        for pitch in chordObj.getattr("pitches")?.try_iter()? {
            let pitch = pitch?;
            let parts = PyList::empty(py);
            parts.append(slf.call_method1("lyPitchFromPitch", (&pitch,))?)?;
            for mark in accidental_marks(&pitch)? {
                parts.append(mark)?;
            }
            elements.push(lyo(
                py,
                "LyChordBodyElement",
                &[("parts", parts.into_any())],
            )?);
        }
        lyo(
            py,
            "LyChordBody",
            &[("chordBodyElements", PyList::new(py, elements)?.into_any())],
        )?
    };
    let duration = slf.call_method1(
        "lyMultipliedDurationFromDuration",
        (chordObj.getattr("duration")?,),
    )?;
    let events = slf.call_method1("postEventsFromObject", (chordObj,))?;
    let element = lyo(
        py,
        "LyNoteChordElement",
        &[
            ("chordBody", body),
            ("optionalNoteModeDuration", duration),
            ("postEvents", events),
        ],
    )?;
    let chord = lyo(py, "LyEventChord", &[("noteChordElement", element)])?;
    lyo(py, "LySimpleMusic", &[("eventChord", chord)])
}

/// A tie that does not end here, and fermatas.
#[pyfunction]
fn post_events<'py>(
    slf: &Bound<'py, PyAny>,
    generalNote: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    let events = PyList::empty(slf.py());
    if let Ok(tie) = generalNote.getattr("tie")
        && !tie.is_none()
        && !tie
            .getattr("type")?
            .extract::<String>()
            .is_ok_and(|kind| kind == "stop")
    {
        events.append("~ ")?;
    }
    if let Ok(expressions) = generalNote.getattr("expressions") {
        for expression in expressions.try_iter()? {
            if is_a(&expression?, "Fermata")? {
                events.append("\\fermata ")?;
            }
        }
    }
    Ok(events)
}

#[pyfunction]
fn ly_pitch<'py>(
    slf: &Bound<'py, PyAny>,
    pitchObj: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let name = base_name(slf, pitchObj)?;
    let octave = octave_characters(slf, pitchObj)?;
    ly_class(slf.py(), "LyPitch")?.call1((name, octave))
}

#[pyfunction]
fn base_name(slf: &Bound<'_, PyAny>, pitchObj: &Bound<'_, PyAny>) -> PyResult<String> {
    let mut name = pitchObj.getattr("step")?.str()?.to_string().to_lowercase();
    let accidental = pitchObj.getattr("accidental")?;
    if !accidental.is_none() {
        let table = slf.getattr("accidentalConvert")?;
        let accidental_name = accidental.getattr("name")?;
        if table.contains(&accidental_name)? {
            name.push_str(&table.get_item(accidental_name)?.str()?.to_string());
        }
    }
    Ok(name)
}

#[pyfunction]
fn octave_characters(slf: &Bound<'_, PyAny>, pitchObj: &Bound<'_, PyAny>) -> PyResult<String> {
    let _ = slf;
    let octave: i64 = pitchObj.getattr("octave")?.extract()?;
    if octave < 3 {
        return Ok(",".repeat((3 - octave) as usize));
    }
    Ok("'".repeat((octave - 3) as usize))
}

/// A duration of one type as a steno duration: its number and dots.
#[pyfunction]
fn multiplied_duration<'py>(
    slf: &Bound<'py, PyAny>,
    durationObj: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let duration_module = py.import("music21.duration")?;
    let number = match duration_module
        .call_method1("convertTypeToNumber", (durationObj.getattr("type")?,))
    {
        Ok(number) => number.extract::<f64>()?,
        Err(error) => {
            return Err(translate_error(format!(
                "DurationException for durationObject {}: {}",
                durationObj.str()?,
                error.value(py).str()?
            )));
        }
    };
    if number == 0.0 {
        return Err(translate_error(format!(
            "Cannot translate an object of zero duration {}",
            durationObj.str()?
        )));
    }
    let written = if number < 1.0 {
        if number == 0.5 {
            text(py, "\\breve")
        } else if number == 0.25 {
            text(py, "\\longa")
        } else {
            return Err(translate_error(
                "Cannot support durations longer than longa",
            ));
        }
    } else {
        (number as i64).into_pyobject(py)?.into_any()
    };
    let dots: i64 = durationObj.getattr("dots")?.extract()?;
    let steno = ly_class(py, "LyStenoDuration")?.call1((written, dots))?;
    ly_class(py, "LyMultipliedDuration")?.call1((steno,))
}

// ----------------------------------------------------- clefs, keys, ...

#[pyfunction]
fn scm_from_clef<'py>(
    slf: &Bound<'py, PyAny>,
    clefObj: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let mut name = "";
    for (class, lily) in CLEFS {
        if is_a(clefObj, class)? {
            name = lily;
            break;
        }
    }
    let made = ly_class(py, "LyEmbeddedScm")?.call0()?;
    let indent = made.getattr("newlineIndent")?.str()?.to_string();
    made.setattr("content", format!("\\clef {}{indent}", quote(name)))?;
    Ok(made)
}

#[pyfunction]
fn scm_from_key<'py>(
    slf: &Bound<'py, PyAny>,
    keyObj: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let key = if is_a(keyObj, "Key")? {
        keyObj.clone()
    } else {
        keyObj.call_method1("asKey", ("major",))?
    };
    let name = base_name(slf, &key.getattr("tonic")?)?;
    let mode = key.getattr("mode")?.str()?.to_string();
    let made = ly_class(py, "LyEmbeddedScm")?.call0()?;
    let indent = made.getattr("newlineIndent")?.str()?.to_string();
    made.setattr("content", format!("\\key {name} \\{mode} {indent}"))?;
    Ok(made)
}

#[pyfunction]
fn scm_from_meter<'py>(
    slf: &Bound<'py, PyAny>,
    ts: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let made = ly_class(slf.py(), "LyEmbeddedScm")?.call0()?;
    let indent = made.getattr("newlineIndent")?.str()?.to_string();
    made.setattr(
        "content",
        format!("\\time {}{indent}", ts.getattr("ratioString")?.str()?),
    )?;
    Ok(made)
}

#[pyfunction]
fn scm_from_tempo<'py>(
    slf: &Bound<'py, PyAny>,
    mm: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let number = mm.getattr("number")?;
    if number.is_none() {
        return Ok(none(py));
    }
    let duration = slf.call_method1(
        "lyMultipliedDurationFromDuration",
        (mm.getattr("referent")?,),
    )?;
    let event = lyo(
        py,
        "LyTempoEvent",
        &[
            ("stenoDuration", duration.getattr("stenoDur")?),
            ("scalar", number),
        ],
    )?;
    let made = ly_class(py, "LyEmbeddedScm")?.call0()?;
    let indent = made.getattr("newlineIndent")?.str()?.to_string();
    let written: String = event.call_method0("stringOutput")?.extract()?;
    made.setattr("content", format!("{written}{indent}"))?;
    Ok(made)
}

// ------------------------------------------------------------- tuplets

#[pyfunction]
fn tuplet_start<'py>(
    slf: &Bound<'py, PyAny>,
    inObj: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let Some(tuplet) = first_tuplet(inObj)? else {
        return Ok(none(py));
    };
    if !tuplet
        .getattr("type")?
        .extract::<String>()
        .is_ok_and(|kind| kind == "start")
    {
        return Ok(none(py));
    }
    let actual = tuplet
        .getattr("tupletActual")?
        .get_item(0)?
        .extract::<f64>()? as i64;
    let normal = tuplet
        .getattr("tupletNormal")?
        .get_item(0)?
        .extract::<f64>()? as i64;
    time_fraction(
        slf,
        &text(py, &actual.to_string()),
        &text(py, &normal.to_string()),
    )
}

/// A new context for music scaled by actual/normal, inside a `\tuplet`
/// appended to the current one.
#[pyfunction]
fn time_fraction<'py>(
    slf: &Bound<'py, PyAny>,
    actual: &Bound<'py, PyAny>,
    normal: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let fraction = format!("{}/{}", actual.str()?, normal.str()?);
    let music_list = ly_class(py, "LyMusicList")?.call0()?;
    let sequential = lyo(
        py,
        "LySequentialMusic",
        &[("musicList", music_list.clone())],
    )?;
    let prefix = lyo(
        py,
        "LyPrefixCompositeMusic",
        &[
            ("type", text(py, "tuplet")),
            ("fraction", text(py, &fraction)),
            ("music", sequential),
        ],
    )?;
    append_here(slf, &prefix)?;
    new_context(slf, &music_list)?;
    Ok(music_list)
}

#[pyfunction]
fn tuplet_stop(slf: &Bound<'_, PyAny>, inObj: &Bound<'_, PyAny>) -> PyResult<()> {
    let Some(tuplet) = first_tuplet(inObj)? else {
        return Ok(());
    };
    if tuplet
        .getattr("type")?
        .extract::<String>()
        .is_ok_and(|kind| kind == "stop")
    {
        restore_context(slf)?;
    }
    Ok(())
}

// ------------------------------------------------------------ variants

/// A variant, or variants standing together, beside what they replace.
#[pyfunction]
#[pyo3(signature = (slf, variantObjectOrList, activeSite = None, coloredVariants = false))]
fn append_variant(
    slf: &Bound<'_, PyAny>,
    variantObjectOrList: &Bound<'_, PyAny>,
    activeSite: Option<&Bound<'_, PyAny>>,
    coloredVariants: bool,
) -> PyResult<()> {
    let py = slf.py();
    let site = activeSite.cloned().unwrap_or_else(|| none(py));
    let mut music = Vec::new();
    let colored = |kwargs: &Bound<'_, PyDict>| kwargs.set_item("coloredVariants", coloredVariants);
    if !variantObjectOrList.is_instance_of::<PyList>() {
        let variant = variantObjectOrList;
        let replaced = variant.call_method1("replacedElements", (&site,))?;
        let kwargs = PyDict::new(py);
        colored(&kwargs)?;
        music.push(slf.call_method(
            "lyPrefixCompositeMusicFromVariant",
            (variant, &replaced),
            Some(&kwargs),
        )?);
        music.push(slf.call_method1("lySequentialMusicFromStream", (replaced,))?);
    } else {
        // The variants by name, in the order their names first come.
        let mut by_name: Vec<(String, Vec<Bound<'_, PyAny>>)> = Vec::new();
        for variant in variantObjectOrList.try_iter()? {
            let variant = variant?;
            let groups = variant.getattr("groups")?;
            let name = if groups.is_truthy()? {
                groups.get_item(0)?.str()?.to_string()
            } else {
                "variant".to_string()
            };
            match by_name.iter_mut().find(|(found, _)| *found == name) {
                Some((_, list)) => list.push(variant),
                None => by_name.push((name, vec![variant])),
            }
        }
        let mut longest = -1.0;
        let mut longest_replaced = py.import("music21.stream")?.getattr("Stream")?.call0()?;
        for (_, list) in by_name {
            let kwargs = PyDict::new(py);
            colored(&kwargs)?;
            let replaced = if list.len() == 1 {
                let replaced = list[0].call_method1("replacedElements", (&site,))?;
                music.push(slf.call_method(
                    "lyPrefixCompositeMusicFromVariant",
                    (&list[0], &replaced),
                    Some(&kwargs),
                )?);
                replaced
            } else {
                kwargs.set_item("activeSite", &site)?;
                let found = slf.call_method(
                    "lyPrefixCompositeMusicFromRelatedVariants",
                    (PyList::new(py, &list)?,),
                    Some(&kwargs),
                )?;
                music.push(found.get_item(0)?);
                found.get_item(1)?
            };
            let length: f64 = replaced
                .getattr("duration")?
                .getattr("quarterLength")?
                .extract()?;
            if longest < length {
                longest = length;
                longest_replaced = replaced;
            }
        }
        music.push(slf.call_method1("lySequentialMusicFromStream", (&longest_replaced,))?);
        let skipped = slf.getattr("doNotOutput")?;
        for element in longest_replaced.try_iter()? {
            skipped.call_method1("append", (element?,))?;
        }
    }
    let music_list = ly_class(py, "LyMusicList")?.call0()?;
    music_list.setattr("contents", PyList::new(py, music)?)?;
    let simultaneous = ly_class(py, "LySimultaneousMusic")?.call0()?;
    simultaneous.setattr("musicList", music_list)?;
    let grouped = ly_class(py, "LyGroupedMusicList")?.call0()?;
    grouped.setattr("simultaneousMusic", simultaneous)?;
    append_here(slf, &grouped)
}

/// A variant's own staff, above the part it varies.
#[pyfunction]
#[pyo3(signature = (slf, variantObject, replacedElements, coloredVariants = false))]
fn prefix_from_variant<'py>(
    slf: &Bound<'py, PyAny>,
    variantObject: &Bound<'py, PyAny>,
    replacedElements: &Bound<'py, PyAny>,
    coloredVariants: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let stream = py.import("music21.stream")?;
    let clef_class = py.import("music21.clef")?.getattr("Clef")?;
    let replaced_clef = replacedElements
        .get_item(0)?
        .call_method1("getContextByClass", (clef_class,))?;
    let mut container =
        variantObject.call_method1("getContextByClass", (stream.getattr("Part")?,))?;
    if container.is_none() {
        container =
            variantObject.call_method1("getContextByClass", (stream.getattr("Stream")?,))?;
    }
    if container.is_none() {
        return Err(translate_error(format!(
            "Cannot find a container stream for {}",
            variantObject.str()?
        )));
    }
    if !replaced_clef.is_none()
        && !variantObject
            .getattr("elements")?
            .contains(&replaced_clef)?
    {
        variantObject.call_method1("insert", (0, &replaced_clef))?;
    }
    let groups = variantObject.getattr("groups")?;
    let name = if groups.is_truthy()? {
        groups.get_item(0)?.str()?.to_string()
    } else {
        "variant".to_string()
    };
    let added = slf.getattr("addedVariants")?;
    let new_variant = !added.contains(&name)?;
    if new_variant {
        added.call_method1("append", (&name,))?;
    }
    let container_id = letters_only(&container.getattr("id")?.str()?.to_string());
    let variant_id =
        ly_class(py, "LyOptionalId")?.call1((format!("{}{container_id}", letters_only(&name)),))?;
    if coloredVariants {
        let at: usize = added.call_method1("index", (&name,))?.extract()?;
        let color = slf.getattr("variantColors")?.get_item(at % 6)?;
        let notes = variantObject
            .getattr("_stream")?
            .call_method0("recurse")?
            .getattr("notesAndRests")?;
        for note in notes.try_iter()? {
            note?.getattr("style")?.setattr("color", &color)?;
        }
    }

    // A hidden rest at the start is a spacer, written before the ossia.
    let mut music = Vec::new();
    let rest_class = py.import("music21.note")?.getattr("Rest")?;
    let mut spacer_length = 0.0;
    for rest in variantObject
        .call_method1("getElementsByClass", (rest_class,))?
        .try_iter()?
    {
        let rest = rest?;
        if !rest
            .getattr("style")?
            .getattr("hideObjectOnPrint")?
            .is_truthy()?
        {
            continue;
        }
        spacer_length = rest
            .getattr("duration")?
            .getattr("quarterLength")?
            .extract::<f64>()?;
        if spacer_length > 0.0 {
            music.push(slf.call_method1("lySimpleMusicFromNoteOrRest", (&rest,))?);
        }
        variantObject.call_method1("remove", (&rest,))?;
        break;
    }
    let ossia = slf.call_method1("lyOssiaMusicFromVariant", (variantObject,))?;
    let replaced_length: f64 = variantObject
        .getattr("replacementQuarterLength")?
        .extract()?;
    let variant_length: f64 = variantObject
        .getattr("containedHighestTime")?
        .extract::<f64>()?
        - spacer_length;
    slf.setattr("variantMode", true)?;
    if variant_length != replaced_length {
        let tuplet = py
            .import("music21.common")?
            .call_method1("decimalToTuplet", (replaced_length / variant_length,))?;
        let fraction = format!(
            "{}/{}",
            tuplet.get_item(1)?.str()?,
            tuplet.get_item(0)?.str()?
        );
        let scaled = lyo(
            py,
            "LyPrefixCompositeMusic",
            &[
                ("type", text(py, "tuplet")),
                ("fraction", text(py, &fraction)),
                ("music", ossia),
            ],
        )?;
        music.push(lyo(py, "LySequentialMusic", &[("musicList", scaled)])?);
    } else {
        music.push(ossia);
    }
    let music_list = ly_class(py, "LyMusicList")?.call1((PyList::new(py, music)?,))?;
    let with_spacer = lyo(py, "LySequentialMusic", &[("musicList", music_list)])?;
    let kind = if new_variant { "new" } else { "context" };
    let made = lyo(
        py,
        "LyPrefixCompositeMusic",
        &[
            ("type", text(py, kind)),
            ("optionalId", variant_id),
            ("simpleString", text(py, "Staff")),
            ("music", with_spacer),
        ],
    )?;
    slf.setattr("variantMode", false)?;
    Ok(made)
}

#[pyfunction]
fn ossia_from_variant<'py>(
    slf: &Bound<'py, PyAny>,
    variantIn: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let contents = PyList::empty(py);
    let music_list = lyo(
        py,
        "LyMusicList",
        &[("contents", contents.clone().into_any())],
    )?;
    let ossia = lyo(py, "LyOssiaMusic", &[("musicList", music_list.clone())])?;
    new_context(slf, &music_list)?;
    slf.setattr("variantMode", true)?;
    slf.call_method1(
        "appendObjectsToContextFromStream",
        (variantIn.getattr("_stream")?,),
    )?;
    let closing = slf.call_method0("closeMeasure")?;
    if !closing.is_none() {
        contents.append(closing)?;
    }
    restore_context(slf)?;
    slf.setattr("variantMode", false)?;
    Ok(ossia)
}

// ------------------------------------------------- headers and measures

/// A header with the metadata's title and alternative title.
#[pyfunction]
#[pyo3(signature = (slf, metadataObject = None, lpHeader = None))]
fn header_from_metadata<'py>(
    slf: &Bound<'py, PyAny>,
    metadataObject: Option<&Bound<'py, PyAny>>,
    lpHeader: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let header = match lpHeader.filter(|header| !header.is_none()) {
        Some(header) => header.clone(),
        None => ly_class(py, "LyLilypondHeader")?.call0()?,
    };
    let mut body = header.getattr("lilypondHeaderBody")?;
    if body.is_none() {
        body = ly_class(py, "LyLilypondHeaderBody")?.call0()?;
        header.setattr("lilypondHeaderBody", &body)?;
    }
    let assignments = body.getattr("assignments")?;
    if let Some(metadata) = metadataObject.filter(|metadata| !metadata.is_none()) {
        for (id, value) in [
            ("title", metadata.getattr("bestTitle")?),
            ("subtitle", metadata.getattr("alternativeTitle")?),
        ] {
            if !value.is_truthy()? {
                continue;
            }
            let init = lyo(py, "LyIdentifierInit", &[("string", value)])?;
            let assignment = lyo(
                py,
                "LyAssignment",
                &[("assignmentId", text(py, id)), ("identifierInit", init)],
            )?;
            assignments.call_method1("append", (&assignment,))?;
            assignment.call_method1("setParent", (&body,))?;
        }
    }
    body.setattr("assignments", assignments)?;
    Ok(header)
}

/// The bar line ending the measure before, with its number in a comment,
/// or a bar check alone where asked or among variants.
#[pyfunction]
#[pyo3(signature = (slf, barChecksOnly = false))]
fn close_measure<'py>(slf: &Bound<'py, PyAny>, barChecksOnly: bool) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let measure = slf.getattr("currentMeasure")?;
    slf.setattr("currentMeasure", none(py))?;
    if measure.is_none() {
        return Ok(measure);
    }
    let checks_only = barChecksOnly || is_true(slf, "variantMode");
    let made = ly_class(py, "LyEmbeddedScm")?.call0()?;
    let right = measure.getattr("rightBarline")?;
    let mut bar = if checks_only {
        "|".to_string()
    } else if right.is_none() {
        format!("\\bar {}", quote("|"))
    } else {
        let sign = slf
            .getattr("barlineDict")?
            .get_item(right.getattr("type")?)?
            .str()?
            .to_string();
        format!("\\bar {}", quote(&sign))
    };
    let number = measure.getattr("number")?;
    if !number.is_none() {
        bar.push_str(&format!(" %{{ end measure {} %}} ", number.str()?));
    }
    made.setattr("content", bar)?;
    Ok(made)
}

/// `\partial` for a measure with a pickup: what is left of the bar, in
/// thirty-second notes.
#[pyfunction]
fn padding_scheme<'py>(
    slf: &Bound<'py, PyAny>,
    measureObject: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let padding: f64 = measureObject.getattr("paddingLeft")?.extract()?;
    if padding == 0.0 {
        return Ok(none(py));
    }
    let meters = measureObject.call_method0("getTimeSignatures")?;
    let bar = if meters.is_truthy()? {
        meters
            .get_item(0)?
            .getattr("barDuration")?
            .getattr("quarterLength")?
            .extract::<f64>()?
    } else {
        4.0
    };
    let remaining = bar - padding;
    if remaining <= 0.0 {
        return Err(translate_error(
            "your first pickup measure is non-existent!",
        ));
    }
    let made = ly_class(py, "LyEmbeddedScm")?.call0()?;
    made.setattr(
        "content",
        format!("\\partial 32*{} ", (remaining * 8.0) as i64),
    )?;
    Ok(made)
}

// -------------------------------------------------- files and LilyPond

/// Writes the tree to a file, a new temporary one where none is named.
#[pyfunction]
#[pyo3(signature = (slf, ext = "", fp = None))]
fn write_ly_file<'py>(
    slf: &Bound<'py, PyAny>,
    ext: &str,
    fp: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let written = slf.getattr("topLevelObject")?.str()?.to_string();
    let fp = match fp.filter(|fp| !fp.is_none()) {
        Some(fp) => fp.clone(),
        None => {
            let kwargs = PyDict::new(py);
            kwargs.set_item("returnPathlib", false)?;
            environment(py)?.call_method("getTempFile", (ext,), Some(&kwargs))?
        }
    };
    let path = py.import("pathlib")?.getattr("Path")?.call1((fp,))?;
    std::fs::write(path.str()?.to_string(), written)?;
    slf.setattr("tempName", &path)?;
    Ok(path)
}

/// Writes the tree and runs LilyPond on it, answering the file LilyPond
/// made. The program is run as music21 runs it, with no shell.
#[pyfunction]
#[pyo3(signature = (slf, format, backend = None, fileName = None, skipWriting = false))]
fn run_through_lily<'py>(
    slf: &Bound<'py, PyAny>,
    format: Option<String>,
    backend: Option<String>,
    fileName: Option<&Bound<'py, PyAny>>,
    skipWriting: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let program: String = slf.call_method0("findLilyExec")?.extract()?;
    let file = match fileName.filter(|file| !file.is_none()) {
        None => write_ly_file(slf, "ly", None)?,
        Some(file) if !skipWriting => write_ly_file(slf, "ly", Some(file))?,
        Some(file) => file.clone(),
    };
    let file = file.str()?.to_string();
    let mut command = vec![program];
    if let Some(format) = &format {
        command.extend(["-f".to_string(), format.clone()]);
    }
    if let Some(backend) = &backend {
        command.push(format!("{}{backend}", slf.getattr("backendString")?.str()?));
    }
    command.extend(["-o".to_string(), file.clone(), file.clone()]);
    let kwargs = PyDict::new(py);
    kwargs.set_item("check", false)?;
    py.import("subprocess")?
        .getattr("run")?
        .call((PyList::new(py, command)?,), Some(&kwargs))?;
    let _ = std::fs::remove_file(format!("{file}.eps"));

    let format = format.unwrap_or_default();
    let mut made = format!("{file}.{format}");
    if !std::path::Path::new(&made).exists() {
        // Not at the full path: LilyPond may have written it here.
        let name = std::path::Path::new(&made)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !std::path::Path::new(&name).exists() {
            return Err(translate_error(format!(
                "cannot find {name} or the full path {made} original file was {file}"
            )));
        }
        made = name;
    }
    py.import("pathlib")?.getattr("Path")?.call1((made,))
}

fn run_with<'py>(
    slf: &Bound<'py, PyAny>,
    format: &str,
    backend: &str,
    fileName: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let kwargs = PyDict::new(py);
    kwargs.set_item("format", format)?;
    kwargs.set_item("backend", backend)?;
    kwargs.set_item("fileName", fileName)?;
    slf.call_method("runThroughLily", (), Some(&kwargs))
}

#[pyfunction]
#[pyo3(signature = (slf, fileName = None))]
fn create_pdf<'py>(
    slf: &Bound<'py, PyAny>,
    fileName: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    slf.getattr("headerScheme")?.setattr("content", "")?;
    run_with(slf, "pdf", "ps", fileName)
}

#[pyfunction]
fn show_pdf(slf: &Bound<'_, PyAny>) -> PyResult<()> {
    let made = slf.call_method0("createPDF")?;
    if !made.call_method0("exists")?.is_truthy()? {
        return Err(pyo3::exceptions::PyRuntimeError::new_err(
            "Something went wrong with PDF Creation",
        ));
    }
    let shown = made.str()?.to_string();
    if cfg!(windows) {
        std::process::Command::new("cmd")
            .args(["/C", "start", "/wait", &shown])
            .status()?;
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(&shown).status()?;
    }
    Ok(())
}

/// A PNG, with a white border where PIL is installed.
#[pyfunction]
#[pyo3(signature = (slf, fileName = None))]
fn create_png<'py>(
    slf: &Bound<'py, PyAny>,
    fileName: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let made = run_with(slf, "png", "eps", fileName)?;
    if let Ok(image) = py.import("PIL.Image")
        && let Ok(ops) = py.import("PIL.ImageOps")
    {
        let bordered = image
            .call_method1("open", (made.str()?,))
            .and_then(|opened| ops.call_method1("expand", (opened, 10, "white")))
            .and_then(|expanded| expanded.call_method1("save", (made.str()?,)));
        let _ = bordered;
    }
    Ok(made)
}

#[pyfunction]
fn show_png<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let made = match slf.call_method0("createPNG") {
        Ok(made) => made,
        Err(error) if error.is_instance_of::<LilyTranslateException>(py) => {
            return Err(translate_error(format!(
                "Problems creating PNG file: ({})",
                error.value(py).str()?
            )));
        }
        Err(error) => return Err(error),
    };
    launch(py, &made, "png")
}

#[pyfunction]
#[pyo3(signature = (slf, fileName = None))]
fn create_svg<'py>(
    slf: &Bound<'py, PyAny>,
    fileName: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    slf.getattr("headerScheme")?.setattr("content", "")?;
    run_with(slf, "svg", "svg", fileName)
}

#[pyfunction]
#[pyo3(signature = (slf, fileName = None))]
fn show_svg<'py>(
    slf: &Bound<'py, PyAny>,
    fileName: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let made = slf.call_method1("createSVG", (fileName,))?;
    launch(slf.py(), &made, "svg")
}

/// Opens a file as music21's `SubConverter.launch` does.
fn launch<'py>(
    py: Python<'py>,
    file: &Bound<'py, PyAny>,
    format: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let kwargs = PyDict::new(py);
    kwargs.set_item("fmt", format)?;
    crate::subconverters::module(py)?
        .getattr("SubConverter")?
        .call0()?
        .call_method("launch", (file,), Some(&kwargs))
}
