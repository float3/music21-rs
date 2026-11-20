//! music21's `romanText.rtObjects`: RomanText read into tokens, a tagged
//! line, a measure and the atoms in it, over the crate's
//! `romantext::tokens`.
//!
//! The crate sorts lines and words and reads measure numbers and beats;
//! these classes are music21's, each attribute any Python object, and the
//! handler keeps its tokens in a Python list as music21's does.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::PyClass;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyList, PyModule, PyString};

use music21::common::numbertools::OffsetQL;
use music21::romantext::tokens::{self as raw, AtomKind, Tag};

use crate::prebase::ProtoM21Object;

/// The module music21 keeps these in.
const MODULE: &str = "music21.romanText.rtObjects";

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

pyo3::create_exception!(
    music21_rs_facade,
    RomanTextException,
    crate::Music21Exception
);
pyo3::create_exception!(music21_rs_facade, RTTokenException, crate::Music21Exception);
pyo3::create_exception!(
    music21_rs_facade,
    RTHandlerException,
    crate::Music21Exception
);

/// The patterns music21 sorts lines and words by, kept on the module as
/// music21 keeps them.
const PATTERNS: [(&str, &str); 12] = [
    ("reMeasureTag", r"m[0-9]+[a-h]*-*[0-9]*[a-h]*"),
    ("reVariant", r"var[0-9]+"),
    ("reVariantLetter", r"var([A-Z]+)"),
    ("reOptKeyOpenAtom", r"\?\([A-Ga-g]+[b#]*:"),
    ("reOptKeyCloseAtom", r"\?\)[A-Ga-g]+[b#]*:?"),
    ("reKeyAtom", r"[A-Ga-g]+[b#]*;:"),
    ("reAnalyticKeyAtom", r"[A-Ga-g]+[b#]*:"),
    ("reKeySignatureAtom", r"KS-?[0-7]"),
    ("reBeatAtom", r"b[1-9.]+"),
    ("reRepeatStartAtom", r"\|\|:"),
    ("reRepeatStopAtom", r":\|\|"),
    ("reNoChordAtom", r"(NC|N.C.|nc)"),
];

/// A crate error as music21's exception for it.
fn handler_error(error: music21::Error) -> PyErr {
    RTHandlerException::new_err(error.to_string())
}

/// What the error says, without the crate's naming of the format.
fn said(error: &music21::Error) -> String {
    error
        .format_error()
        .map_or_else(|| error.to_string(), |found| found.message.clone())
}

fn none(py: Python<'_>) -> Py<PyAny> {
    py.None()
}

fn text(py: Python<'_>, value: &str) -> Py<PyAny> {
    PyString::new(py, value).into_any().unbind()
}

/// music21's `RTToken`: one logical entity of a RomanText, its source and
/// the line it is on. A plain token is none of the kinds it can be asked
/// about.
#[pyclass(name = "RTToken", module = "music21.romanText.rtObjects", extends = ProtoM21Object, subclass, dict)]
pub struct RTToken {
    /// The text it was read from.
    #[pyo3(get, set)]
    src: Py<PyAny>,
    /// The line it is on, counted from one.
    #[pyo3(get, set)]
    lineNumber: Py<PyAny>,
}

impl RTToken {
    fn made(py: Python<'_>, src: Py<PyAny>) -> Self {
        Self {
            src,
            lineNumber: 0_i64
                .into_pyobject(py)
                .map_or_else(|_| py.None(), |n| n.into_any().unbind()),
        }
    }
}

#[pymethods]
impl RTToken {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.src)?;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (src = None))]
    fn new(py: Python<'_>, src: Option<Py<PyAny>>) -> PyClassInitializer<Self> {
        crate::base::built(Self::made(py, src.unwrap_or_else(|| text(py, ""))))
    }

    /// Its source, quoted.
    fn _reprInternal(&self, py: Python<'_>) -> PyResult<String> {
        Ok(self.src.bind(py).repr()?.to_string())
    }

    fn isComposer(&self) -> bool {
        false
    }

    fn isTitle(&self) -> bool {
        false
    }

    fn isPiece(&self) -> bool {
        false
    }

    fn isAnalyst(&self) -> bool {
        false
    }

    fn isProofreader(&self) -> bool {
        false
    }

    fn isTimeSignature(&self) -> bool {
        false
    }

    fn isKeySignature(&self) -> bool {
        false
    }

    fn isNote(&self) -> bool {
        false
    }

    /// Occasionally found in a header.
    fn isForm(&self) -> bool {
        false
    }

    fn isMeasure(&self) -> bool {
        false
    }

    fn isPedal(&self) -> bool {
        false
    }

    fn isWork(&self) -> bool {
        false
    }

    fn isMovement(&self) -> bool {
        false
    }

    fn isVersion(&self) -> bool {
        false
    }

    /// Atoms are any untagged data, generally found in a measure.
    fn isAtom(&self) -> bool {
        false
    }
}

/// music21's `RTTagged`: a tag, a colon and its data, `Title: Die
/// Jahrzeiten`.
#[pyclass(name = "RTTagged", module = "music21.romanText.rtObjects", extends = RTToken, subclass)]
pub struct RTTagged {
    /// What comes before the first colon.
    #[pyo3(get, set)]
    tag: Py<PyAny>,
    /// What comes after it, or the whole line where there is none.
    #[pyo3(get, set)]
    data: Py<PyAny>,
}

impl RTTagged {
    fn is(&self, py: Python<'_>, tag: Tag) -> PyResult<bool> {
        let named: String = self.tag.bind(py).extract()?;
        Ok(raw::tag_of(&named) == Some(tag))
    }

    fn initializer(py: Python<'_>, src: &str) -> PyClassInitializer<Self> {
        let (tag, data) = raw::split_tagged(src);
        crate::base::built(RTToken::made(py, text(py, src))).add_subclass(Self {
            tag: text(py, &tag),
            data: text(py, &data),
        })
    }
}

#[pymethods]
impl RTTagged {
    #[new]
    #[pyo3(signature = (src = ""))]
    fn new(py: Python<'_>, src: &str) -> PyClassInitializer<Self> {
        Self::initializer(py, src)
    }

    fn isComposer(&self, py: Python<'_>) -> PyResult<bool> {
        self.is(py, Tag::Composer)
    }

    fn isTitle(&self, py: Python<'_>) -> PyResult<bool> {
        self.is(py, Tag::Title)
    }

    fn isPiece(&self, py: Python<'_>) -> PyResult<bool> {
        self.is(py, Tag::Piece)
    }

    fn isAnalyst(&self, py: Python<'_>) -> PyResult<bool> {
        self.is(py, Tag::Analyst)
    }

    fn isProofreader(&self, py: Python<'_>) -> PyResult<bool> {
        self.is(py, Tag::Proofreader)
    }

    fn isTimeSignature(&self, py: Python<'_>) -> PyResult<bool> {
        self.is(py, Tag::TimeSignature)
    }

    fn isKeySignature(&self, py: Python<'_>) -> PyResult<bool> {
        self.is(py, Tag::KeySignature)
    }

    fn isNote(&self, py: Python<'_>) -> PyResult<bool> {
        self.is(py, Tag::Note)
    }

    fn isForm(&self, py: Python<'_>) -> PyResult<bool> {
        self.is(py, Tag::Form)
    }

    fn isPedal(&self, py: Python<'_>) -> PyResult<bool> {
        self.is(py, Tag::Pedal)
    }

    /// The version of RomanText the file follows, 1.0 where none is given.
    fn isVersion(&self, py: Python<'_>) -> PyResult<bool> {
        self.is(py, Tag::Version)
    }

    /// The work, also tagged `Madrigal`.
    fn isWork(&self, py: Python<'_>) -> PyResult<bool> {
        self.is(py, Tag::Work)
    }

    fn isMovement(&self, py: Python<'_>) -> PyResult<bool> {
        self.is(py, Tag::Movement)
    }

    /// How vi, vio and VI read in minor.
    fn isSixthMinor(&self, py: Python<'_>) -> PyResult<bool> {
        self.is(py, Tag::SixthMinor)
    }

    /// How vii, viio and VII read in minor.
    fn isSeventhMinor(&self, py: Python<'_>) -> PyResult<bool> {
        self.is(py, Tag::SeventhMinor)
    }
}

/// music21's `RTMeasure`: a measure line, its number or range, any repeat
/// letter or variant, and the atoms in it.
#[pyclass(name = "RTMeasure", module = "music21.romanText.rtObjects", extends = RTToken, subclass)]
pub struct RTMeasure {
    /// The measure number or range as written.
    #[pyo3(get, set)]
    tag: Py<PyAny>,
    /// The chords, beats and keys.
    #[pyo3(get, set)]
    data: Py<PyAny>,
    /// One measure number, or two for a range.
    #[pyo3(get, set)]
    number: Py<PyAny>,
    /// The repeat letter of each number.
    #[pyo3(get, set)]
    repeatLetter: Py<PyAny>,
    /// A one-measure variant's number.
    #[pyo3(get, set)]
    variantNumber: Py<PyAny>,
    /// A longer variant's letters.
    #[pyo3(get, set)]
    variantLetter: Py<PyAny>,
    /// Whether it copies other measures.
    #[pyo3(get, set)]
    isCopyDefinition: Py<PyAny>,
    /// The atoms read from its data.
    #[pyo3(get, set)]
    atoms: Py<PyAny>,
}

impl RTMeasure {
    fn initializer(py: Python<'_>, src: &str) -> PyResult<PyClassInitializer<Self>> {
        let mut made = Self {
            tag: text(py, ""),
            data: text(py, ""),
            number: PyList::empty(py).into_any().unbind(),
            repeatLetter: PyList::empty(py).into_any().unbind(),
            variantNumber: none(py),
            variantLetter: none(py),
            isCopyDefinition: false.into_pyobject(py)?.to_owned().into_any().unbind(),
            atoms: PyList::empty(py).into_any().unbind(),
        };
        if !src.is_empty() {
            let line = raw::read_measure_line(src).map_err(handler_error)?;
            made.tag = text(py, &line.tag);
            made.data = text(py, &line.data);
            made.number = PyList::new(py, &line.number)?.into_any().unbind();
            made.repeatLetter = PyList::new(py, &line.repeat_letter)?.into_any().unbind();
            made.variantNumber = line.variant_number.into_pyobject(py)?.into_any().unbind();
            made.variantLetter = line.variant_letter.into_pyobject(py)?.into_any().unbind();
            made.isCopyDefinition = line
                .is_copy_definition
                .into_pyobject(py)?
                .to_owned()
                .into_any()
                .unbind();
        }
        Ok(crate::base::built(RTToken::made(py, text(py, src))).add_subclass(made))
    }
}

/// The numbers and letters music21's `_getMeasureNumberData` gives.
fn number_data<'py>(py: Python<'py>, src: &str) -> PyResult<Bound<'py, pyo3::types::PyTuple>> {
    let (numbers, letters) =
        raw::measure_number_data(src).map_err(|error| PyValueError::new_err(said(&error)))?;
    pyo3::types::PyTuple::new(
        py,
        [
            PyList::new(py, numbers)?.into_any(),
            PyList::new(py, letters)?.into_any(),
        ],
    )
}

#[pymethods]
impl RTMeasure {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.atoms)?;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (src = ""))]
    fn new(py: Python<'_>, src: &str) -> PyResult<PyClassInitializer<Self>> {
        Self::initializer(py, src)
    }

    /// The measure numbers a tag names, and the repeat letter of each.
    fn _getMeasureNumberData<'py>(
        &self,
        py: Python<'py>,
        src: &str,
    ) -> PyResult<Bound<'py, pyo3::types::PyTuple>> {
        number_data(py, src)
    }

    /// Its number, or its range.
    fn _reprInternal(&self, py: Python<'_>) -> PyResult<String> {
        let number = self.number.bind(py);
        if number.len()? == 1 {
            return Ok(number.get_item(0)?.str()?.to_string());
        }
        Ok(format!(
            "{}-{}",
            number.get_item(0)?.str()?,
            number.get_item(1)?.str()?
        ))
    }

    fn isMeasure(&self) -> bool {
        true
    }

    /// The measures it copies, and their repeat letters.
    fn getCopyTarget<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, pyo3::types::PyTuple>> {
        let data: String = self.data.bind(py).extract()?;
        number_data(py, data.replace('=', "").trim())
    }
}

/// music21's `RTAtom`: untagged data in a measure, which knows the measure
/// it is in.
#[pyclass(name = "RTAtom", module = "music21.romanText.rtObjects", extends = RTToken, subclass)]
pub struct RTAtom {
    /// The measure it is in.
    #[pyo3(get, set)]
    container: Py<PyAny>,
}

/// An atom's initializer, under the token's.
fn atom(
    py: Python<'_>,
    src: Py<PyAny>,
    container: Option<Py<PyAny>>,
) -> PyClassInitializer<RTAtom> {
    crate::base::built(RTToken::made(py, src)).add_subclass(RTAtom {
        container: container.unwrap_or_else(|| none(py)),
    })
}

#[pymethods]
impl RTAtom {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.container)?;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (src = None, container = None))]
    fn new(
        py: Python<'_>,
        src: Option<Py<PyAny>>,
        container: Option<Py<PyAny>>,
    ) -> PyClassInitializer<Self> {
        atom(py, src.unwrap_or_else(|| text(py, "")), container)
    }

    fn isAtom(&self) -> bool {
        true
    }
}

/// An atom class with nothing of its own, under `$base`, whose source is
/// `$default` where none is given.
macro_rules! atom_class {
    ($name:ident, $base:ident, $default:literal, $doc:literal) => {
        #[doc = $doc]
        #[pyclass(module = "music21.romanText.rtObjects", extends = $base, subclass)]
        pub struct $name;

        impl $name {
            fn init(
                py: Python<'_>,
                src: Py<PyAny>,
                container: Option<Py<PyAny>>,
            ) -> PyClassInitializer<Self> {
                $base::init(py, src, container).add_subclass(Self)
            }
        }

        #[pymethods]
        impl $name {
            #[new]
            #[pyo3(signature = (src = None, container = None))]
            fn new(
                py: Python<'_>,
                src: Option<Py<PyAny>>,
                container: Option<Py<PyAny>>,
            ) -> PyClassInitializer<Self> {
                Self::init(py, src.unwrap_or_else(|| text(py, $default)), container)
            }
        }
    };
}

impl RTAtom {
    fn init(
        py: Python<'_>,
        src: Py<PyAny>,
        container: Option<Py<PyAny>>,
    ) -> PyClassInitializer<Self> {
        atom(py, src, container)
    }
}

atom_class!(RTOpenParens, RTAtom, "(", "music21's `RTOpenParens`: `(`.");
atom_class!(
    RTCloseParens,
    RTAtom,
    ")",
    "music21's `RTCloseParens`: `)`."
);
atom_class!(
    RTPhraseMarker,
    RTAtom,
    "",
    "music21's `RTPhraseMarker`: a phrase's mark."
);
atom_class!(
    RTPhraseBoundary,
    RTPhraseMarker,
    "||",
    "music21's `RTPhraseBoundary`: `||`, between phrases."
);
atom_class!(
    RTEllisonStart,
    RTPhraseMarker,
    "|*",
    "music21's `RTEllisonStart`: `|*`, where phrases start to overlap."
);
atom_class!(
    RTEllisonStop,
    RTPhraseMarker,
    "*|",
    "music21's `RTEllisonStop`: `*|`, where they stop."
);
atom_class!(RTRepeat, RTAtom, "", "music21's `RTRepeat`: a repeat sign.");
atom_class!(
    RTRepeatStart,
    RTRepeat,
    "||:",
    "music21's `RTRepeatStart`: `||:`."
);
atom_class!(
    RTRepeatStop,
    RTRepeat,
    ":||",
    "music21's `RTRepeatStop`: `:||`."
);

/// music21's `RTChord`: a roman numeral in a measure, with the offset and
/// length a translator gives it.
#[pyclass(module = "music21.romanText.rtObjects", extends = RTAtom, subclass)]
pub struct RTChord {
    /// Its offset in its measure.
    #[pyo3(get, set)]
    offset: Py<PyAny>,
    /// Its length in quarter notes.
    #[pyo3(get, set)]
    quarterLength: Py<PyAny>,
}

impl RTChord {
    fn init(
        py: Python<'_>,
        src: Py<PyAny>,
        container: Option<Py<PyAny>>,
    ) -> PyClassInitializer<Self> {
        atom(py, src, container).add_subclass(Self {
            offset: none(py),
            quarterLength: none(py),
        })
    }
}

#[pymethods]
impl RTChord {
    #[new]
    #[pyo3(signature = (src = None, container = None))]
    fn new(
        py: Python<'_>,
        src: Option<Py<PyAny>>,
        container: Option<Py<PyAny>>,
    ) -> PyClassInitializer<Self> {
        Self::init(py, src.unwrap_or_else(|| text(py, "")), container)
    }
}

/// music21's `RTNoChord`: `NC`, where no chord sounds, with the offset and
/// length a translator gives it.
#[pyclass(module = "music21.romanText.rtObjects", extends = RTAtom, subclass)]
pub struct RTNoChord {
    /// Its offset in its measure.
    #[pyo3(get, set)]
    offset: Py<PyAny>,
    /// Its length in quarter notes.
    #[pyo3(get, set)]
    quarterLength: Py<PyAny>,
}

impl RTNoChord {
    fn init(
        py: Python<'_>,
        src: Py<PyAny>,
        container: Option<Py<PyAny>>,
    ) -> PyClassInitializer<Self> {
        atom(py, src, container).add_subclass(Self {
            offset: none(py),
            quarterLength: none(py),
        })
    }
}

#[pymethods]
impl RTNoChord {
    #[new]
    #[pyo3(signature = (src = None, container = None))]
    fn new(
        py: Python<'_>,
        src: Option<Py<PyAny>>,
        container: Option<Py<PyAny>>,
    ) -> PyClassInitializer<Self> {
        Self::init(py, src.unwrap_or_else(|| text(py, "")), container)
    }
}

/// A beat as music21 keeps one: a float, or a `Fraction`.
fn offset_object(py: Python<'_>, value: OffsetQL) -> PyResult<Bound<'_, PyAny>> {
    match value {
        OffsetQL::Float(value) => Ok(pyo3::types::PyFloat::new(py, value).into_any()),
        OffsetQL::Fraction {
            numerator,
            denominator,
        } => py
            .import("fractions")?
            .getattr("Fraction")?
            .call1((numerator, denominator)),
    }
}

/// A module of music21's where it is answered for, else the wheel, for a
/// class that is the same in both.
fn class_of<'py>(py: Python<'py>, module: &str, name: &str) -> PyResult<Bound<'py, PyAny>> {
    py.import(module)
        .or_else(|_| py.import("music21_rs"))?
        .getattr(name)
}

/// The source of the token a class method was called on.
fn source_of<T: PyClass>(slf: &Bound<'_, T>) -> PyResult<String> {
    slf.as_any().getattr("src")?.extract()
}

/// music21's `RTBeat`: `b2.5`, the beat what follows stands on.
#[pyclass(module = "music21.romanText.rtObjects", extends = RTAtom, subclass)]
pub struct RTBeat;

impl RTBeat {
    fn init(
        py: Python<'_>,
        src: Py<PyAny>,
        container: Option<Py<PyAny>>,
    ) -> PyClassInitializer<Self> {
        atom(py, src, container).add_subclass(Self)
    }
}

#[pymethods]
impl RTBeat {
    #[new]
    #[pyo3(signature = (src = None, container = None))]
    fn new(
        py: Python<'_>,
        src: Option<Py<PyAny>>,
        container: Option<Py<PyAny>>,
    ) -> PyClassInitializer<Self> {
        Self::init(py, src.unwrap_or_else(|| text(py, "")), container)
    }

    /// The beat as a float or a fraction, whatever the meter.
    fn getBeatFloatOrFrac<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let src = source_of(slf)?;
        let beat = raw::beat_float_or_frac(&src).map_err(|error| {
            if src.split('.').count() > 3 {
                RTTokenException::new_err(said(&error))
            } else {
                PyValueError::new_err(said(&error))
            }
        })?;
        offset_object(py, beat)
    }

    /// The offset the beat stands at in a meter, or nought where the meter
    /// has no such beat.
    fn getOffset<'py>(
        slf: &Bound<'py, Self>,
        timeSignature: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let beat = Self::getBeatFloatOrFrac(slf)?;
        match timeSignature.call_method1("getOffsetFromBeat", (beat,)) {
            Ok(offset) => Ok(offset),
            Err(error) => {
                let refused = class_of(py, "music21.exceptions21", "TimeSignatureException")?;
                if error.is_instance(py, &refused) {
                    return Ok(pyo3::types::PyFloat::new(py, 0.0).into_any());
                }
                Err(error)
            }
        }
    }
}

/// The key a key atom's source names, its ending stripped.
fn key_named<'py>(py: Python<'py>, src: &str, footer: &str) -> PyResult<Bound<'py, PyAny>> {
    let name = src.trim_end_matches(|character| footer.contains(character));
    let name = music21::key::convert_key_string_to_music21_key_string(name);
    class_of(py, "music21.key", "Key")?.call1((name,))
}

/// The key signature of so many sharps.
fn key_signature<'py>(py: Python<'py>, sharps: Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    class_of(py, "music21.key", "KeySignature")?.call1((sharps,))
}

/// music21's `RTKeyTypeAtom`: what key atoms share, `RTKey` and
/// `RTAnalyticKey`.
#[pyclass(module = "music21.romanText.rtObjects", extends = RTAtom, subclass)]
pub struct RTKeyTypeAtom;

impl RTKeyTypeAtom {
    fn init(
        py: Python<'_>,
        src: Py<PyAny>,
        container: Option<Py<PyAny>>,
    ) -> PyClassInitializer<Self> {
        atom(py, src, container).add_subclass(Self)
    }
}

#[pymethods]
impl RTKeyTypeAtom {
    #[new]
    #[pyo3(signature = (src = None, container = None))]
    fn new(
        py: Python<'_>,
        src: Option<Py<PyAny>>,
        container: Option<Py<PyAny>>,
    ) -> PyClassInitializer<Self> {
        Self::init(py, src.unwrap_or_else(|| text(py, "")), container)
    }

    /// What is stripped off the end to leave the key's name.
    #[classattr]
    fn footerStrip() -> &'static str {
        ";:"
    }

    /// The key it names: a `Key`, not a `KeySignature`.
    fn getKey<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let footer: String = slf.getattr("footerStrip")?.extract()?;
        key_named(slf.py(), &source_of(slf)?, &footer)
    }

    /// The key signature of the key it names.
    fn getKeySignature<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let sharps = slf.call_method0("getKey")?.getattr("sharps")?;
        key_signature(slf.py(), sharps)
    }
}

/// A key atom class, under `RTKeyTypeAtom`, with its own ending.
macro_rules! key_atom {
    ($name:ident, $footer:literal, $doc:literal) => {
        #[doc = $doc]
        #[pyclass(module = "music21.romanText.rtObjects", extends = RTKeyTypeAtom, subclass)]
        pub struct $name;

        impl $name {
            fn init(
                py: Python<'_>,
                src: Py<PyAny>,
                container: Option<Py<PyAny>>,
            ) -> PyClassInitializer<Self> {
                RTKeyTypeAtom::init(py, src, container).add_subclass(Self)
            }
        }

        #[pymethods]
        impl $name {
            #[new]
            #[pyo3(signature = (src = None, container = None))]
            fn new(
                py: Python<'_>,
                src: Option<Py<PyAny>>,
                container: Option<Py<PyAny>>,
            ) -> PyClassInitializer<Self> {
                Self::init(py, src.unwrap_or_else(|| text(py, "")), container)
            }

            /// What is stripped off the end to leave the key's name.
            #[classattr]
            fn footerStrip() -> &'static str {
                $footer
            }
        }
    };
}

key_atom!(
    RTKey,
    ";:",
    "music21's `RTKey`: `g;:`, a new key and its key signature."
);
key_atom!(
    RTAnalyticKey,
    ":",
    "music21's `RTAnalyticKey`: `g:`, a new key for the analysis only."
);

/// music21's `RTKeySignature`: `KS-2`, a key signature alone.
#[pyclass(module = "music21.romanText.rtObjects", extends = RTAtom, subclass)]
pub struct RTKeySignature;

impl RTKeySignature {
    fn init(
        py: Python<'_>,
        src: Py<PyAny>,
        container: Option<Py<PyAny>>,
    ) -> PyClassInitializer<Self> {
        atom(py, src, container).add_subclass(Self)
    }
}

#[pymethods]
impl RTKeySignature {
    #[new]
    #[pyo3(signature = (src = None, container = None))]
    fn new(
        py: Python<'_>,
        src: Option<Py<PyAny>>,
        container: Option<Py<PyAny>>,
    ) -> PyClassInitializer<Self> {
        Self::init(py, src.unwrap_or_else(|| text(py, "")), container)
    }

    /// The key signature of the sharps after `KS`, flats negative.
    fn getKeySignature<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let sharps = raw::key_signature_sharps(&source_of(slf)?)
            .map_err(|error| PyValueError::new_err(said(&error)))?;
        key_signature(py, sharps.into_pyobject(py)?.into_any())
    }
}

/// An optional key's start or end: a key that does not change the
/// analysis.
macro_rules! optional_key {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[pyclass(module = "music21.romanText.rtObjects", extends = RTAtom, subclass)]
        pub struct $name;

        impl $name {
            fn init(
                py: Python<'_>,
                src: Py<PyAny>,
                container: Option<Py<PyAny>>,
            ) -> PyClassInitializer<Self> {
                atom(py, src, container).add_subclass(Self)
            }
        }

        #[pymethods]
        impl $name {
            #[new]
            #[pyo3(signature = (src = None, container = None))]
            fn new(
                py: Python<'_>,
                src: Option<Py<PyAny>>,
                container: Option<Py<PyAny>>,
            ) -> PyClassInitializer<Self> {
                Self::init(py, src.unwrap_or_else(|| text(py, "")), container)
            }

            /// The key it names.
            fn getKey<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
                let name = raw::optional_key_name(&source_of(slf)?);
                class_of(slf.py(), "music21.key", "Key")?.call1((name,))
            }
        }
    };
}

optional_key!(
    RTOptionalKeyOpen,
    "music21's `RTOptionalKeyOpen`: `?(Bb:`, an optional key's start."
);
optional_key!(
    RTOptionalKeyClose,
    "music21's `RTOptionalKeyClose`: `?)Bb:`, an optional key's end."
);

/// An atom of a kind, as the class music21 makes for it.
fn atom_object<'py>(
    py: Python<'py>,
    kind: AtomKind,
    word: &str,
    container: Option<Py<PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let src = text(py, word);
    let made = match kind {
        AtomKind::PhraseBoundary => {
            Bound::new(py, RTPhraseBoundary::init(py, src, container))?.into_any()
        }
        AtomKind::OpenParens => Bound::new(py, RTOpenParens::init(py, src, container))?.into_any(),
        AtomKind::CloseParens => {
            Bound::new(py, RTCloseParens::init(py, src, container))?.into_any()
        }
        AtomKind::Beat => Bound::new(py, RTBeat::init(py, src, container))?.into_any(),
        AtomKind::OptionalKeyOpen => {
            Bound::new(py, RTOptionalKeyOpen::init(py, src, container))?.into_any()
        }
        AtomKind::OptionalKeyClose => {
            Bound::new(py, RTOptionalKeyClose::init(py, src, container))?.into_any()
        }
        AtomKind::Key => Bound::new(py, RTKey::init(py, src, container))?.into_any(),
        AtomKind::AnalyticKey => {
            Bound::new(py, RTAnalyticKey::init(py, src, container))?.into_any()
        }
        AtomKind::KeySignature => {
            Bound::new(py, RTKeySignature::init(py, src, container))?.into_any()
        }
        AtomKind::RepeatStart => {
            Bound::new(py, RTRepeatStart::init(py, src, container))?.into_any()
        }
        AtomKind::RepeatStop => Bound::new(py, RTRepeatStop::init(py, src, container))?.into_any(),
        AtomKind::NoChord => Bound::new(py, RTNoChord::init(py, src, container))?.into_any(),
        AtomKind::Chord => Bound::new(py, RTChord::init(py, src, container))?.into_any(),
    };
    Ok(made)
}

/// music21's `RTHandler`: a RomanText read into tokens, the header's
/// tagged lines and then the body's measures and tags.
#[pyclass(module = "music21.romanText.rtObjects", subclass, dict)]
pub struct RTHandler {
    /// The tokens, in order.
    #[pyo3(get, set)]
    _tokens: Py<PyAny>,
    /// The line the body starts on.
    #[pyo3(get, set)]
    currentLineNumber: Py<PyAny>,
}

impl RTHandler {
    fn made(py: Python<'_>) -> Self {
        Self {
            _tokens: PyList::empty(py).into_any().unbind(),
            currentLineNumber: 0_i64
                .into_pyobject(py)
                .map_or_else(|_| py.None(), |n| n.into_any().unbind()),
        }
    }

    /// Its tokens, or music21's refusal where it has none.
    fn held_tokens<'py>(&self, py: Python<'py>, refusal: &str) -> PyResult<Bound<'py, PyAny>> {
        let tokens = self._tokens.bind(py);
        if !tokens.is_truthy()? {
            return Err(RTHandlerException::new_err(refusal.to_owned()));
        }
        Ok(tokens.clone())
    }
}

/// Lines given as a Python list of text.
fn lines_of(lines: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    lines.extract()
}

#[pymethods]
impl RTHandler {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self._tokens)?;
        Ok(())
    }

    #[new]
    fn new(py: Python<'_>) -> Self {
        Self::made(py)
    }

    /// The lines before the first measure, and the lines from it.
    fn splitAtHeader<'py>(
        &self,
        py: Python<'py>,
        lines: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, pyo3::types::PyTuple>> {
        let read = lines_of(lines)?;
        let Some(end) = raw::header_end(&read) else {
            let arguments = (
                "Cannot find the first measure definition in this file. Dumping contexts: %s",
                lines.clone().unbind(),
            );
            return Err(RomanTextException::new_err(arguments));
        };
        pyo3::types::PyTuple::new(
            py,
            [
                PyList::new(py, &read[..end])?,
                PyList::new(py, &read[end..])?,
            ],
        )
    }

    /// The header's lines as tagged tokens.
    fn tokenizeHeader<'py>(
        &mut self,
        py: Python<'py>,
        lines: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyList>> {
        let read = lines_of(lines)?;
        let out = PyList::empty(py);
        for (index, line) in read.iter().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let token = Bound::new(py, RTTagged::initializer(py, line))?;
            token.setattr("lineNumber", index + 1)?;
            out.append(token)?;
        }
        self.currentLineNumber = (read.len() + 1).into_pyobject(py)?.into_any().unbind();
        Ok(out)
    }

    /// The body's lines as measures, with their atoms, and tagged tokens.
    fn tokenizeBody<'py>(
        &self,
        py: Python<'py>,
        lines: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyList>> {
        let read = lines_of(lines)?;
        let start: i64 = self.currentLineNumber.bind(py).extract()?;
        let out = PyList::empty(py);
        for (index, line) in read.iter().enumerate() {
            let number = start + index as i64;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            body_token(py, line, number, &out).map_err(|error| {
                RTHandlerException::new_err(format!(
                    "At line {number} ({line}) an exception was raised:\n{error}"
                ))
            })?;
        }
        Ok(out)
    }

    /// A measure's data as atoms, each knowing the measure it is in.
    #[pyo3(signature = (line, container = None))]
    fn tokenizeAtoms<'py>(
        &self,
        py: Python<'py>,
        line: &str,
        container: Option<Py<PyAny>>,
    ) -> PyResult<Bound<'py, PyList>> {
        atoms_list(py, line, container)
    }

    /// Reads a RomanText into tokens, after any it holds.
    fn tokenize(slf: &Bound<'_, Self>, src: &str) -> PyResult<()> {
        let py = slf.py();
        let lines = PyList::new(py, src.split('\n'))?;
        let split = slf.call_method1("splitAtHeader", (lines,))?;
        let header = slf.call_method1("tokenizeHeader", (split.get_item(0)?,))?;
        let body = slf.call_method1("tokenizeBody", (split.get_item(1)?,))?;
        let tokens = slf.borrow()._tokens.clone_ref(py);
        tokens.bind(py).call_method1("extend", (header,))?;
        tokens.bind(py).call_method1("extend", (body,))?;
        Ok(())
    }

    /// Reads a whole RomanText into tokens, in place of any it held.
    fn process(slf: &Bound<'_, Self>, src: &str) -> PyResult<()> {
        slf.borrow_mut()._tokens = PyList::empty(slf.py()).into_any().unbind();
        slf.call_method1("tokenize", (src,))?;
        Ok(())
    }

    /// Whether it holds at least `countRequired` movements.
    #[pyo3(signature = (countRequired = 2))]
    fn definesMovements(&self, py: Python<'_>, countRequired: usize) -> PyResult<bool> {
        let tokens = self.held_tokens(py, "must create tokens first")?;
        let mut count = 0;
        for token in tokens.try_iter()? {
            if token?.call_method0("isMovement")?.is_truthy()? {
                count += 1;
                if count >= countRequired {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// Whether it holds any movement.
    fn definesMovement(&self, py: Python<'_>) -> PyResult<bool> {
        self.definesMovements(py, 1)
    }

    /// A handler for each movement, each with the header's tokens where
    /// `duplicateHeader`, else the header in a handler of its own first.
    #[pyo3(signature = (duplicateHeader = true))]
    fn splitByMovement<'py>(
        &self,
        py: Python<'py>,
        duplicateHeader: bool,
    ) -> PyResult<Bound<'py, PyList>> {
        let post = PyList::empty(py);
        let mut sub = PyList::empty(py);
        let handler = |tokens: &Bound<'py, PyList>| -> PyResult<Bound<'py, RTHandler>> {
            let made = Bound::new(py, RTHandler::made(py))?;
            made.borrow_mut()._tokens = tokens.clone().into_any().unbind();
            Ok(made)
        };
        for token in self._tokens.bind(py).try_iter()? {
            let token = token?;
            if token.call_method0("isMovement")?.is_truthy()? {
                post.append(handler(&sub)?)?;
                sub = PyList::empty(py);
            }
            sub.append(token)?;
        }
        if !sub.is_empty() {
            post.append(handler(&sub)?)?;
        }
        if !duplicateHeader {
            return Ok(post);
        }
        let first = post.get_item(0)?;
        let (head, start) = if first.call_method0("definesMovement")?.is_truthy()? {
            (None, 0)
        } else {
            (Some(first), 1)
        };
        let joined = PyList::empty(py);
        for each in post.iter().skip(start) {
            match &head {
                Some(head) => joined.append(head.add(&each)?)?,
                None => joined.append(each)?,
            }
        }
        Ok(joined)
    }

    /// Its tokens; music21 refuses to hand out none.
    #[getter]
    fn get_tokens<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.held_tokens(py, "must process tokens before calling split")
    }

    #[setter]
    fn set_tokens(&mut self, tokens: Py<PyAny>) {
        self._tokens = tokens;
    }

    fn __len__(&self, py: Python<'_>) -> PyResult<usize> {
        self._tokens.bind(py).len()
    }

    /// A handler of this class with both handlers' tokens.
    fn __add__<'py>(
        slf: &Bound<'py, Self>,
        other: &Bound<'py, Self>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let made = slf.get_type().call0()?;
        let joined = slf
            .borrow()
            ._tokens
            .bind(py)
            .add(other.borrow()._tokens.bind(py))?;
        made.setattr("tokens", joined)?;
        Ok(made)
    }
}

/// A measure's data as a list of atoms.
fn atoms_list<'py>(
    py: Python<'py>,
    line: &str,
    container: Option<Py<PyAny>>,
) -> PyResult<Bound<'py, PyList>> {
    let out = PyList::empty(py);
    for (kind, word) in raw::atoms(line) {
        let held = container.as_ref().map(|container| container.clone_ref(py));
        out.append(atom_object(py, kind, &word, held)?)?;
    }
    Ok(out)
}

/// One line of the body, as a measure with its atoms or a tagged token.
fn body_token(py: Python<'_>, line: &str, number: i64, out: &Bound<'_, PyList>) -> PyResult<()> {
    if raw::measure_tag_length(line).is_none() {
        let token = Bound::new(py, RTTagged::initializer(py, line))?;
        token.setattr("lineNumber", number)?;
        return out.append(token);
    }
    let measure = Bound::new(py, RTMeasure::initializer(py, line)?)?;
    measure.setattr("lineNumber", number)?;
    let data: String = measure.getattr("data")?.extract()?;
    let atoms = atoms_list(py, &data, Some(measure.clone().into_any().unbind()))?;
    for atom in atoms.iter() {
        atom.setattr("lineNumber", number)?;
    }
    measure.setattr("atoms", atoms)?;
    out.append(measure)
}

/// music21's `RTFile`: a RomanText file, read into a handler.
#[pyclass(module = "music21.romanText.rtObjects", extends = ProtoM21Object, subclass, dict)]
pub struct RTFile {
    /// The open file.
    #[pyo3(get, set)]
    file: Py<PyAny>,
    /// Its name.
    #[pyo3(get, set)]
    filename: Py<PyAny>,
}

#[pymethods]
impl RTFile {
    #[new]
    fn new(py: Python<'_>) -> PyClassInitializer<Self> {
        crate::base::built(Self {
            file: none(py),
            filename: none(py),
        })
    }

    /// Opens a file to read, as UTF-8.
    fn open(&mut self, py: Python<'_>, filename: Py<PyAny>) -> PyResult<()> {
        let keywords = pyo3::types::PyDict::new(py);
        keywords.set_item("encoding", "utf-8")?;
        let opened = py
            .import("io")?
            .getattr("open")?
            .call((filename.bind(py),), Some(&keywords))?;
        self.file = opened.unbind();
        self.filename = filename;
        Ok(())
    }

    /// Takes a file-like object, such as a `StringIO`, as the open file.
    fn openFileLike(&mut self, fileLike: Py<PyAny>) {
        self.file = fileLike;
    }

    fn _reprInternal(&self) -> &'static str {
        ""
    }

    /// Closes the file.
    fn close(&self, py: Python<'_>) -> PyResult<()> {
        self.file.bind(py).call_method0("close")?;
        Ok(())
    }

    /// Reads the open file into a handler.
    fn read<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let read = slf.borrow().file.bind(slf.py()).call_method0("read")?;
        slf.call_method1("readstr", (read,))
    }

    /// Reads a RomanText into a handler.
    fn readstr<'py>(&self, py: Python<'py>, strSrc: &str) -> PyResult<Bound<'py, PyAny>> {
        let handler = Bound::new(py, RTHandler::made(py))?;
        handler.call_method1("process", (strSrc,))?;
        Ok(handler.into_any())
    }
}

/// music21's `romanText.rtObjects`, built.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        for (name, exception) in [
            ("RomanTextException", py.get_type::<RomanTextException>()),
            ("RTTokenException", py.get_type::<RTTokenException>()),
            ("RTHandlerException", py.get_type::<RTHandlerException>()),
        ] {
            exception.setattr("__module__", MODULE)?;
            m.add(name, exception)?;
        }
        let re = py.import("re")?;
        for (name, pattern) in PATTERNS {
            m.add(name, re.call_method1("compile", (pattern,))?)?;
        }
        let environment = crate::environment::module(py)?.getattr("Environment")?;
        m.add("environLocal", environment.call1(("romanText.rtObjects",))?)?;
        m.add_class::<RTToken>()?;
        m.add_class::<RTTagged>()?;
        m.add_class::<RTMeasure>()?;
        m.add_class::<RTAtom>()?;
        m.add_class::<RTChord>()?;
        m.add_class::<RTNoChord>()?;
        m.add_class::<RTBeat>()?;
        m.add_class::<RTKeyTypeAtom>()?;
        m.add_class::<RTKey>()?;
        m.add_class::<RTAnalyticKey>()?;
        m.add_class::<RTKeySignature>()?;
        m.add_class::<RTOpenParens>()?;
        m.add_class::<RTCloseParens>()?;
        m.add_class::<RTOptionalKeyOpen>()?;
        m.add_class::<RTOptionalKeyClose>()?;
        m.add_class::<RTPhraseMarker>()?;
        m.add_class::<RTPhraseBoundary>()?;
        m.add_class::<RTEllisonStart>()?;
        m.add_class::<RTEllisonStop>()?;
        m.add_class::<RTRepeat>()?;
        m.add_class::<RTRepeatStart>()?;
        m.add_class::<RTRepeatStop>()?;
        m.add_class::<RTHandler>()?;
        m.add_class::<RTFile>()?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}
