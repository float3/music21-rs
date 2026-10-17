//! music21's spanners, over the crate's `spanner` module: the things that
//! join one note to another rather than standing at a point.
//!
//! A slur, a hairpin, an octave line, a glissando, a trill's wavy line, a
//! tremolo between two notes, a bracket, a pedal mark and an arpeggio
//! across staves. music21 keeps each as an element of the stream whose
//! notes it joins -- at offset nought, whatever the notes' offsets -- and
//! holds the notes themselves; this package keeps them the same way, so a
//! spanner handed back names the stream's own notes.
//!
//! **Nothing is installed over music21's own.** A spanner there carries
//! `idLocal`, the style and position it is drawn with, and the site
//! machinery that lets it be filled from a stream; standing in for one
//! would be quietly different rather than missing, as it would for
//! `bar.Barline` and `expressions.TextExpression`.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

use music21_rs_crate::spanner::{Spanner as RsSpanner, SpannerKind};

/// The names this module hands out: the base class and the kinds, each
/// under the name music21 gives it.
pub const NAMES: &[&str] = &["SpannerException"];

pyo3::create_exception!(music21_rs_facade, SpannerException, crate::Music21Exception);

/// music21's `spanner.Spanner`: what joins the notes it holds.
#[pyclass(
    name = "Spanner",
    module = "music21.spanner",
    subclass,
    skip_from_py_object
)]
pub struct Spanner {
    /// What the spanner says of itself besides the notes it joins: the
    /// kind, how it is drawn, an octave line's shift, a glissando's kind.
    /// The positions it names are not kept here -- the objects are.
    pub(crate) inner: RsSpanner,
    /// What a caller or a reader has numbered this spanner with:
    /// music21's `idLocal`, which it keeps as a bare attribute and which
    /// a MusicXML document's `number` is read into. Nothing musical turns
    /// on it, so the crate keeps none.
    #[pyo3(get, set)]
    idLocal: Option<Py<PyAny>>,
    /// The notes it joins, in order, as the objects they are: music21's
    /// `getSpannedElements`.
    spanned: Vec<Py<PyAny>>,
}

impl Spanner {
    /// A spanner of a kind joining nothing yet.
    pub(crate) fn of_kind(kind: SpannerKind) -> Self {
        Self {
            inner: RsSpanner::new(kind, Vec::new()),
            spanned: Vec::new(),
            idLocal: None,
        }
    }

    /// The object for a spanner of the crate's, joining these objects:
    /// the class installed over music21's where there is one, and this
    /// package's own where not.
    pub(crate) fn object(
        py: Python<'_>,
        inner: &RsSpanner,
        spanned: Vec<Py<PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let name = inner.kind().class_name();
        let module = module_of(inner.kind());
        let class = match crate::installed_class(py, module, name) {
            Some(class) => class,
            None => py
                .import("music21_rs")
                .or_else(|_| py.import("music21_rs_facade"))?
                .getattr(name)?,
        };
        let made = class.call0()?;
        {
            let mut held = made.extract::<PyRefMut<'_, Self>>()?;
            held.inner = inner.clone();
            held.spanned = spanned;
        }
        Ok(made.unbind())
    }
}

#[pymethods]
impl Spanner {
    /// music21's `Spanner(*spannedElements)`: the notes to join, given one
    /// by one or as a list.
    #[new]
    #[pyo3(signature = (*spannedElements, **_keywords))]
    fn new(
        spannedElements: &Bound<'_, PyTuple>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let mut made = Self::of_kind(SpannerKind::Slur);
        made.addSpannedElements(spannedElements, None)?;
        Ok(made)
    }

    /// What music21 says this is, as its `classes` answers.
    #[getter]
    fn classes(slf: &Bound<'_, Self>) -> PyResult<Vec<String>> {
        let mut names = vec![slf.get_type().name()?.to_string()];
        let kind = slf.borrow().inner.kind();
        if names[0] != kind.class_name() {
            names.push(kind.class_name().to_string());
        }
        // A hairpin is a `DynamicWedge` to music21 before it is a
        // `Spanner`, which is how its own code picks the two out together.
        if kind.is_wedge() {
            names.push("DynamicWedge".to_string());
        }
        names.push("Spanner".to_string());
        names.push("Music21Object".to_string());
        names.push("ProtoM21Object".to_string());
        names.push("object".to_string());
        Ok(names)
    }

    /// music21's sort order for a spanner among the things at one offset,
    /// which is the same as a note's.
    #[classattr]
    fn classSortOrder() -> i32 {
        20
    }

    /// Whether this is a stream, which a spanner is not. music21 asks this
    /// of everything a stream holds.
    #[classattr]
    fn isStream() -> bool {
        false
    }

    /// The notes this joins, in order: music21's `getSpannedElements`.
    fn getSpannedElements<'py>(slf: &Bound<'py, Self>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let py = slf.py();
        Ok(slf
            .borrow()
            .spanned
            .iter()
            .map(|spanned| spanned.bind(py).clone())
            .collect())
    }

    /// Adds notes to join, at the end: music21's `addSpannedElements`,
    /// which takes them one by one or as a list.
    #[pyo3(signature = (*spannedElements, **_keywords))]
    fn addSpannedElements(
        &mut self,
        spannedElements: &Bound<'_, PyTuple>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        for given in spannedElements.iter() {
            if given.is_instance_of::<PyList>() || given.is_instance_of::<PyTuple>() {
                for one in given.try_iter()? {
                    self.spanned.push(one?.unbind());
                }
            } else {
                self.spanned.push(given.unbind());
            }
        }
        Ok(())
    }

    /// The first note it joins, or `None`: music21's `getFirst`.
    fn getFirst<'py>(slf: &Bound<'py, Self>) -> Option<Bound<'py, PyAny>> {
        let py = slf.py();
        slf.borrow()
            .spanned
            .first()
            .map(|spanned| spanned.bind(py).clone())
    }

    /// The last note it joins, or `None`: music21's `getLast`.
    fn getLast<'py>(slf: &Bound<'py, Self>) -> Option<Bound<'py, PyAny>> {
        let py = slf.py();
        slf.borrow()
            .spanned
            .last()
            .map(|spanned| spanned.bind(py).clone())
    }

    /// Whether an object is one of the notes it joins: music21's
    /// `hasSpannedElement`.
    /// The id of every object it joins: music21's
    /// `getSpannedElementIds`, which is Python's own `id` of each.
    fn getSpannedElementIds(&self, py: Python<'_>) -> Vec<usize> {
        self.spanned
            .iter()
            .map(|object| object.bind(py).as_ptr() as usize)
            .collect()
    }

    fn hasSpannedElement(slf: &Bound<'_, Self>, spannedElement: &Bound<'_, PyAny>) -> bool {
        let py = slf.py();
        slf.borrow()
            .spanned
            .iter()
            .any(|spanned| spanned.bind(py).is(spannedElement))
    }

    /// Puts one note in place of another: music21's
    /// `replaceSpannedElement`.
    fn replaceSpannedElement(
        slf: &Bound<'_, Self>,
        old: &Bound<'_, PyAny>,
        new: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let py = slf.py();
        let mut held = slf.borrow_mut();
        for spanned in &mut held.spanned {
            if spanned.bind(py).is(old) {
                *spanned = new.clone().unbind();
            }
        }
        Ok(())
    }

    /// How many notes it joins: music21's `__len__`.
    fn __len__(&self) -> usize {
        self.spanned.len()
    }

    /// How far and which way an octave line moves the notes under it, as
    /// music21 names it: `8va`, `8vb`, `15ma`, `15mb`, `22da` or `22db`.
    ///
    /// music21 keeps this on `Ottava` alone, and so does this: one `Spanner`
    /// stands for every kind here, so a spanner of another kind raises as
    /// music21's would.
    #[getter]
    fn r#type(slf: &Bound<'_, Self>) -> PyResult<Option<&'static str>> {
        let me = slf.borrow();
        only(slf, SpannerKind::Ottava, "type")?;
        Ok(me.inner.shift().map(|shift| shift.name()))
    }

    /// Writes how far and which way the line moves its notes, by
    /// music21's name for it. Whether the notes are written where they
    /// sound is kept, as music21 keeps `transposing` across a change of
    /// type.
    #[setter(r#type)]
    fn set_type(slf: &Bound<'_, Self>, value: &str) -> PyResult<()> {
        only(slf, SpannerKind::Ottava, "type")?;
        let transposing = slf
            .borrow()
            .inner
            .shift()
            .is_some_and(|shift| shift.transposing());
        let shift = music21_rs_crate::spanner::OctaveShift::from_name(value, transposing)
            .map_err(|error| SpannerException::new_err(crate::pitch::message(&error)))?;
        slf.borrow_mut().inner.set_shift(Some(shift));
        Ok(())
    }

    /// Writes whether the notes under the line are written where they
    /// sound.
    #[setter]
    fn set_transposing(slf: &Bound<'_, Self>, value: bool) -> PyResult<()> {
        only(slf, SpannerKind::Ottava, "transposing")?;
        let Some(shift) = slf.borrow().inner.shift() else {
            return Ok(());
        };
        let named = music21_rs_crate::spanner::OctaveShift::from_name(shift.name(), value)
            .map_err(|error| SpannerException::new_err(crate::pitch::message(&error)))?;
        slf.borrow_mut().inner.set_shift(Some(named));
        Ok(())
    }

    /// What an octave line shifts by: music21's `shiftMagnitude`, 8, 15
    /// or 22.
    #[getter]
    fn shiftMagnitude(slf: &Bound<'_, Self>) -> PyResult<Option<u8>> {
        let me = slf.borrow();
        only(slf, SpannerKind::Ottava, "shiftMagnitude")?;
        Ok(me.inner.shift().map(|shift| shift.magnitude()))
    }

    /// Whether the notes under an octave line are written where they
    /// sound: music21's `transposing`.
    #[getter]
    fn transposing(slf: &Bound<'_, Self>) -> PyResult<bool> {
        let me = slf.borrow();
        only(slf, SpannerKind::Ottava, "transposing")?;
        Ok(me.inner.shift().is_some_and(|shift| shift.transposing()))
    }

    /// How a bracket's line is drawn: music21's `lineType`, `solid`,
    /// `dashed`, `dotted` or `wavy`.
    #[getter]
    fn lineType(slf: &Bound<'_, Self>) -> PyResult<Option<String>> {
        let me = slf.borrow();
        only(slf, SpannerKind::Line, "lineType")?;
        Ok(me.inner.line_type().map(str::to_string))
    }

    /// Writes how the bracket's line is drawn.
    #[setter]
    fn set_lineType(slf: &Bound<'_, Self>, value: Option<String>) -> PyResult<()> {
        only(slf, SpannerKind::Line, "lineType")?;
        slf.borrow_mut().inner.set_line_type(value);
        Ok(())
    }

    /// Which side of the staff it is drawn on, where the score says:
    /// music21's `placement`.
    #[getter]
    fn get_placement(&self) -> Option<&'static str> {
        self.inner.placement().map(|placement| placement.as_str())
    }

    #[setter]
    fn set_placement(&mut self, value: Option<&str>) -> PyResult<()> {
        let placement = match value.filter(|said| !said.is_empty()) {
            Some(said) => Some(
                music21_rs_crate::notation::Placement::from_name(said)
                    .map_err(|error| SpannerException::new_err(crate::pitch::message(&error)))?,
            ),
            None => None,
        };
        self.inner.set_placement(placement);
        Ok(())
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let name = slf.get_type().name()?.to_string();
        let joined: Vec<String> = Self::getSpannedElements(slf)?
            .iter()
            .map(|spanned| {
                spanned
                    .repr()
                    .map(|shown| shown.to_string())
                    .unwrap_or_default()
            })
            .collect();
        let module = module_of(slf.borrow().inner.kind());
        Ok(format!("<{module}.{name} {}>", joined.join("")))
    }
}

/// music21's `spanner.SpannerBundle`: the spanners of a stream and of
/// everything it holds, gathered to be asked about.
///
/// What music21's own code does with one is count it, walk it and ask it
/// `getByClass`, which is what this answers; its idLocal numbering and
/// its `setPendingSpannedElements` machinery are music21's own.
#[pyclass(
    name = "SpannerBundle",
    module = "music21.spanner",
    subclass,
    skip_from_py_object
)]
pub struct SpannerBundle {
    held: Vec<Py<PyAny>>,
}

impl SpannerBundle {
    /// A bundle of these spanners, in the order they were met.
    pub(crate) fn of(held: Vec<Py<PyAny>>) -> Self {
        Self { held }
    }
}

#[pymethods]
impl SpannerBundle {
    /// music21's `SpannerBundle(*spanners)`.
    #[new]
    #[pyo3(signature = (*spanners))]
    fn new(spanners: &Bound<'_, PyTuple>) -> PyResult<Self> {
        let mut held = Vec::new();
        for given in spanners.iter() {
            if given.is_instance_of::<PyList>() || given.is_instance_of::<PyTuple>() {
                for one in given.try_iter()? {
                    held.push(one?.unbind());
                }
            } else {
                held.push(given.unbind());
            }
        }
        Ok(Self::of(held))
    }

    /// How many spanners it holds.
    fn __len__(&self) -> usize {
        self.held.len()
    }

    /// Another spanner into the bundle: music21's `append`.
    fn append(&mut self, other: &Bound<'_, PyAny>) {
        self.held.push(other.clone().unbind());
    }

    /// Those carrying an `idLocal`, as a bundle of their own: music21's
    /// `getByIdLocal`, whose `idLocal` is an attribute a caller or a
    /// reader writes rather than anything the crate keeps.
    #[pyo3(signature = (idLocal = None))]
    fn getByIdLocal<'py>(
        slf: &Bound<'py, Self>,
        idLocal: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, Self>> {
        let py = slf.py();
        let mut found = Vec::new();
        for spanner in &slf.borrow().held {
            let held = spanner.bind(py);
            let same = match (held.getattr("idLocal"), idLocal) {
                (Ok(mine), Some(wanted)) => mine.eq(wanted)?,
                (Ok(mine), None) => mine.is_none(),
                (Err(_), _) => false,
            };
            if same {
                found.push(spanner.clone_ref(py));
            }
        }
        Bound::new(py, Self::of(found))
    }

    /// Those joining an element, as a bundle of their own: music21's
    /// `getBySpannedElement`, which asks each by the object itself.
    fn getBySpannedElement<'py>(
        slf: &Bound<'py, Self>,
        spannedElement: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, Self>> {
        let py = slf.py();
        let mut found = Vec::new();
        for spanner in &slf.borrow().held {
            let held = spanner.bind(py);
            if held
                .call_method1("hasSpannedElement", (spannedElement,))?
                .is_truthy()?
            {
                found.push(spanner.clone_ref(py));
            }
        }
        Bound::new(py, Self::of(found))
    }

    fn __iter__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let listed = PyList::empty(py);
        for spanner in &slf.borrow().held {
            listed.append(spanner.bind(py))?;
        }
        listed.as_any().try_iter().map(|held| held.into_any())
    }

    fn __getitem__<'py>(slf: &Bound<'py, Self>, index: usize) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        slf.borrow()
            .held
            .get(index)
            .map(|spanner| spanner.bind(py).clone())
            .ok_or_else(|| pyo3::exceptions::PyIndexError::new_err("bundle index out of range"))
    }

    /// The spanners answering to a class, as a bundle of their own:
    /// music21's `getByClass`.
    fn getByClass(slf: &Bound<'_, Self>, className: &Bound<'_, PyAny>) -> PyResult<Self> {
        let py = slf.py();
        let held: Vec<Py<PyAny>> = slf
            .borrow()
            .held
            .iter()
            .map(|one| one.clone_ref(py))
            .collect();
        let mut kept = Vec::new();
        for spanner in held {
            let spanner = spanner.into_bound(py);
            if crate::stream::answers_to_class(&spanner, className)? {
                kept.push(spanner.unbind());
            }
        }
        Ok(Self::of(kept))
    }

    fn __repr__(&self) -> String {
        format!(
            "<music21.spanner.SpannerBundle of size {}>",
            self.held.len()
        )
    }
}

/// Which of music21's modules a kind of spanner lives in.
fn module_of(kind: SpannerKind) -> &'static str {
    match kind {
        SpannerKind::Crescendo | SpannerKind::Diminuendo => "music21.dynamics",
        SpannerKind::TrillExtension
        | SpannerKind::TremoloSpanner
        | SpannerKind::ArpeggioMark
        | SpannerKind::PedalMark => "music21.expressions",
        SpannerKind::Neume => "music21.volpiano",
        _ => "music21.spanner",
    }
}

/// Refuses a member music21 keeps on one kind of spanner alone, naming the
/// class as music21's error names it.
fn only(slf: &Bound<'_, Spanner>, kind: SpannerKind, member: &str) -> PyResult<()> {
    if slf.borrow().inner.kind() == kind {
        return Ok(());
    }
    let class = Spanner::classes(slf)?
        .first()
        .cloned()
        .unwrap_or_else(|| "Spanner".to_string());
    Err(pyo3::exceptions::PyAttributeError::new_err(format!(
        "'{class}' object has no attribute '{member}'"
    )))
}

/// music21's kinds of spanner, each the same `Spanner` under its own name
/// and in the module music21 keeps it in.
macro_rules! spanner_kinds {
    ($(($class:ident, $name:literal, $module:literal, $kind:expr, $doc:literal)),* $(,)?) => {
        $(
            #[doc = $doc]
            #[pyclass(name = $name, module = $module, extends = Spanner, subclass)]
            pub struct $class;

            #[pymethods]
            impl $class {
                #[new]
                #[pyo3(signature = (*spannedElements, **keywords))]
                fn new(
                    spannedElements: &Bound<'_, PyTuple>,
                    keywords: Option<&Bound<'_, PyDict>>,
                ) -> PyResult<PyClassInitializer<Self>> {
                    let mut made = Spanner::of_kind($kind);
                    made.addSpannedElements(spannedElements, keywords)?;
                    Ok(PyClassInitializer::from(made).add_subclass($class))
                }
            }
        )*

        /// Adds the spanner classes to a module.
        pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
            m.add_class::<Spanner>()?;
            m.add_class::<SpannerBundle>()?;
            $(m.add_class::<$class>()?;)*
            Ok(())
        }

        /// The names this module hands out beside the exception.
        pub const CLASS_NAMES: &[&str] = &["Spanner", "SpannerBundle", $($name),*];
    };
}

spanner_kinds!(
    (
        Slur,
        "Slur",
        "music21.spanner",
        SpannerKind::Slur,
        "music21's `Slur`: a curve over the notes played in one breath or bow."
    ),
    (
        Crescendo,
        "Crescendo",
        "music21.dynamics",
        SpannerKind::Crescendo,
        "music21's `Crescendo`: a hairpin opening out, growing louder."
    ),
    (
        Diminuendo,
        "Diminuendo",
        "music21.dynamics",
        SpannerKind::Diminuendo,
        "music21's `Diminuendo`: a hairpin closing, growing softer."
    ),
    (
        Ottava,
        "Ottava",
        "music21.spanner",
        SpannerKind::Ottava,
        "music21's `Ottava`: an octave line, `8va` or `15mb`."
    ),
    (
        Line,
        "Line",
        "music21.spanner",
        SpannerKind::Line,
        "music21's `Line`: a bracket or line over a passage."
    ),
    (
        Glissando,
        "Glissando",
        "music21.spanner",
        SpannerKind::Glissando,
        "music21's `Glissando`: a slide from one note to the next."
    ),
    (
        TrillExtension,
        "TrillExtension",
        "music21.expressions",
        SpannerKind::TrillExtension,
        "music21's `TrillExtension`: the wavy line carrying a trill on."
    ),
    (
        TremoloSpanner,
        "TremoloSpanner",
        "music21.expressions",
        SpannerKind::TremoloSpanner,
        "music21's `TremoloSpanner`: a tremolo alternating between two notes."
    ),
    (
        ArpeggioMarkSpanner,
        "ArpeggioMarkSpanner",
        "music21.expressions",
        SpannerKind::ArpeggioMark,
        "music21's `ArpeggioMarkSpanner`: one arpeggio across several chords."
    ),
    (
        PedalMark,
        "PedalMark",
        "music21.expressions",
        SpannerKind::PedalMark,
        "music21's `PedalMark`: how long a pedal is held."
    ),
    (
        Neume,
        "Neume",
        "music21.volpiano",
        SpannerKind::Neume,
        "music21's `volpiano.Neume`: the notes sung to one syllable as one figure."
    ),
);
