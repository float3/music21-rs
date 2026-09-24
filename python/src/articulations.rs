//! music21's `articulations` module, over the crate's articulations.
//!
//! `Articulation` holds the crate's `Articulation`, and every one of
//! music21's articulation classes is a class of its own. A class music21
//! derives from two, a spiccato being a staccato and an accent both, extends
//! the first of them here; installed over music21's, it is a subclass of
//! music21's own class too, whose order answers `isinstance` for the rest.
//! music21's hammer-on and pull-off are spanners and stay music21's.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyDict;

use music21_rs_crate::articulations::{Articulation as RsArticulation, ArticulationKind, Finger};
use music21_rs_crate::interval::ChromaticInterval as RsChromaticInterval;

/// music21's `articulations.Articulation`.
#[pyclass(
    name = "Articulation",
    module = "music21.articulations",
    subclass,
    skip_from_py_object
)]
pub struct Articulation {
    pub(crate) inner: RsArticulation,
    /// A fret bend's `bendAlter`, kept as the interval object it was given.
    bend_alter: Option<Py<PyAny>>,
}

impl Articulation {
    /// An articulation of music21's `class`, with the one positional
    /// argument its constructor takes -- a string or fret indication's
    /// number, or a fingering's finger -- and the keywords beside it.
    fn initializer(
        class: &str,
        first: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let kind = ArticulationKind::from_class_name(class).ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err(format!("no articulation class {class}"))
        })?;
        let mut made = Self {
            inner: RsArticulation::of_kind(kind),
            bend_alter: None,
        };
        if let Some(first) = first.filter(|first| !first.is_none()) {
            if kind.has_field("number") {
                made.inner.set_number(first.extract()?);
            } else if kind.has_field("fingerNumber") {
                made.inner.set_finger(finger_from_any(first)?);
            }
        }
        // The fields a subclass's own constructor takes, where given by
        // keyword: `fingerNumber` reaches a fretted pluck's fingering half
        // this way, as music21's cooperative `__init__` passes it on.
        if let Some(keywords) = keywords {
            if let Some(value) = keywords.get_item("fingerNumber")?
                && kind.has_field("fingerNumber")
            {
                made.inner.set_finger(finger_from_any(&value)?);
            }
            if let Some(value) = keywords.get_item("number")?
                && kind.has_field("number")
            {
                made.inner.set_number(value.extract()?);
            }
            if let Some(value) = keywords.get_item("preBend")? {
                made.inner.set_pre_bend(value.extract()?);
            }
            if let Some(value) = keywords.get_item("release")? {
                made.inner.set_release(value.extract()?);
            }
            if let Some(value) = keywords.get_item("withBar")? {
                made.inner.set_with_bar(value.extract()?);
            }
            if let Some(value) = keywords.get_item("bendAlter")?
                && !value.is_none()
            {
                made.set_bend(&value)?;
            }
        }
        Ok(PyClassInitializer::from(made))
    }

    fn set_bend(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let semitones = value
            .getattr("semitones")
            .and_then(|semitones| semitones.extract::<f64>())
            .or_else(|_| {
                value
                    .getattr("chromatic")
                    .and_then(|chromatic| chromatic.getattr("semitones"))
                    .and_then(|semitones| semitones.extract::<f64>())
            })
            .ok();
        self.inner.set_bend_alter(
            semitones.and_then(|semitones| RsChromaticInterval::new(semitones).ok()),
        );
        self.bend_alter = Some(value.clone().unbind());
        Ok(())
    }

    /// Refuses a field music21's class for this kind has not got, as a
    /// missing attribute.
    fn field(slf: &Bound<'_, Self>, field: &str) -> PyResult<()> {
        if slf.borrow().inner.kind().has_field(field) {
            return Ok(());
        }
        Err(pyo3::exceptions::PyAttributeError::new_err(format!(
            "'{}' object has no attribute '{field}'",
            slf.get_type().name()?
        )))
    }
}

#[pymethods]
impl Articulation {
    #[new]
    #[pyo3(signature = (**keywords))]
    fn new(keywords: Option<&Bound<'_, PyDict>>) -> PyResult<PyClassInitializer<Self>> {
        Self::initializer("Articulation", None, keywords)
    }

    #[getter]
    fn get_placement(&self) -> Option<&str> {
        self.inner.placement()
    }

    #[setter]
    fn set_placement(&mut self, value: Option<String>) {
        self.inner.set_placement(value);
    }

    /// music21's `volumeShift`, held between -1 and 1.
    #[getter]
    fn get_volumeShift(&self) -> f64 {
        self.inner.volume_shift()
    }

    #[setter]
    fn set_volumeShift(&mut self, value: f64) {
        self.inner.set_volume_shift(value);
    }

    #[getter]
    fn get_lengthShift(&self) -> f64 {
        self.inner.length_shift()
    }

    #[setter]
    fn set_lengthShift(&mut self, value: f64) {
        self.inner.set_length_shift(value);
    }

    #[getter]
    fn get_tieAttach(&self) -> &str {
        self.inner.tie_attach()
    }

    #[setter]
    fn set_tieAttach(&mut self, value: String) {
        self.inner.set_tie_attach(value);
    }

    #[getter]
    fn get_displayText(&self) -> Option<&str> {
        self.inner.display_text()
    }

    #[setter]
    fn set_displayText(&mut self, value: Option<String>) {
        self.inner.set_display_text(value);
    }

    /// music21's `name`: the class name as lower-case words.
    #[getter]
    fn name(&self) -> String {
        self.inner.name()
    }

    /// music21's `fingerNumber`: a number, or the text a score wrote.
    #[getter]
    fn get_fingerNumber(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        Self::field(slf, "fingerNumber")?;
        let py = slf.py();
        Ok(match slf.borrow().inner.finger() {
            Some(Finger::Number(number)) => number.into_pyobject(py)?.into_any().unbind(),
            Some(Finger::Written(text)) => text.into_pyobject(py)?.into_any().unbind(),
            None => py.None(),
        })
    }

    #[setter]
    fn set_fingerNumber(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.inner.set_finger(finger_from_any(value)?);
        Ok(())
    }

    #[getter]
    fn get_substitution(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Self::field(slf, "substitution")?;
        Ok(slf.borrow().inner.substitution())
    }

    #[setter]
    fn set_substitution(&mut self, value: bool) {
        self.inner.set_substitution(value);
    }

    #[getter]
    fn get_alternate(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Self::field(slf, "alternate")?;
        Ok(slf.borrow().inner.alternate())
    }

    #[setter]
    fn set_alternate(&mut self, value: bool) {
        self.inner.set_alternate(value);
    }

    #[getter]
    fn get_number(slf: &Bound<'_, Self>) -> PyResult<i32> {
        Self::field(slf, "number")?;
        Ok(slf.borrow().inner.number())
    }

    #[setter]
    fn set_number(&mut self, value: i32) {
        self.inner.set_number(value);
    }

    #[getter]
    fn get_pointDirection(slf: &Bound<'_, Self>) -> PyResult<Option<String>> {
        Self::field(slf, "pointDirection")?;
        Ok(slf.borrow().inner.point_direction().map(str::to_string))
    }

    #[setter]
    fn set_pointDirection(&mut self, value: Option<String>) {
        self.inner.set_point_direction(value);
    }

    #[getter]
    fn get_symbol(slf: &Bound<'_, Self>) -> PyResult<Option<String>> {
        Self::field(slf, "symbol")?;
        Ok(slf.borrow().inner.symbol().map(str::to_string))
    }

    #[setter]
    fn set_symbol(&mut self, value: Option<String>) {
        self.inner.set_symbol(value);
    }

    #[getter]
    fn get_harmonicType(slf: &Bound<'_, Self>) -> PyResult<Option<String>> {
        Self::field(slf, "harmonicType")?;
        Ok(slf.borrow().inner.harmonic_type().map(str::to_string))
    }

    #[setter]
    fn set_harmonicType(&mut self, value: Option<String>) {
        self.inner.set_harmonic_type(value);
    }

    #[getter]
    fn get_pitchType(slf: &Bound<'_, Self>) -> PyResult<Option<String>> {
        Self::field(slf, "pitchType")?;
        Ok(slf.borrow().inner.pitch_type().map(str::to_string))
    }

    #[setter]
    fn set_pitchType(&mut self, value: Option<String>) {
        self.inner.set_pitch_type(value);
    }

    /// A fret bend's interval, the object it was given.
    #[getter]
    fn get_bendAlter(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        Self::field(slf, "bendAlter")?;
        let py = slf.py();
        Ok(slf
            .borrow()
            .bend_alter
            .as_ref()
            .map_or_else(|| py.None(), |value| value.clone_ref(py)))
    }

    #[setter]
    fn set_bendAlter(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        if value.is_none() {
            self.inner.set_bend_alter(None);
            self.bend_alter = None;
            return Ok(());
        }
        self.set_bend(value)
    }

    #[getter]
    fn get_preBend(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Self::field(slf, "preBend")?;
        Ok(slf.borrow().inner.pre_bend())
    }

    #[setter]
    fn set_preBend(&mut self, value: bool) {
        self.inner.set_pre_bend(value);
    }

    /// Through `opFrac`, as music21 writes it when it reads a bend: a
    /// release one division into a quarter of 10,080 is `Fraction(1, 10080)`.
    #[getter]
    fn get_release<'py>(slf: &Bound<'py, Self>) -> PyResult<Option<Bound<'py, PyAny>>> {
        Self::field(slf, "release")?;
        let release = slf.borrow().inner.release();
        release
            .map(|release| crate::duration::op_frac(slf.py(), release))
            .transpose()
    }

    #[setter]
    fn set_release(&mut self, value: Option<f64>) {
        self.inner.set_release(value);
    }

    #[getter]
    fn get_withBar(slf: &Bound<'_, Self>) -> PyResult<Option<String>> {
        Self::field(slf, "withBar")?;
        Ok(slf.borrow().inner.with_bar().map(str::to_string))
    }

    #[setter]
    fn set_withBar(&mut self, value: Option<String>) {
        self.inner.set_with_bar(value);
    }

    /// music21's `_reprInternal`: a fingering writes its finger and a string
    /// or fret indication its number, whichever its class reaches first.
    fn _reprInternal(&self) -> String {
        let kind = self.inner.kind();
        if kind.has_field("number") {
            return self.inner.number().to_string();
        }
        if kind.has_field("fingerNumber") {
            return match self.inner.finger() {
                Some(Finger::Number(number)) => number.to_string(),
                Some(Finger::Written(text)) => text.clone(),
                None => "None".to_string(),
            };
        }
        String::new()
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let written = slf.borrow()._reprInternal();
        let class = slf.get_type().name()?;
        Ok(if written.is_empty() {
            format!("<music21.articulations.{class}>")
        } else {
            format!("<music21.articulations.{class} {written}>")
        })
    }

    /// music21's equality: an articulation equals one of the same class,
    /// whatever else either says.
    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Self>>()
            .is_ok_and(|theirs| theirs.inner.kind() == self.inner.kind())
    }

    /// music21 hashes an articulation by identity, however it compares.
    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        slf.as_ptr() as isize >> 4
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let copied = slf.get_type().call0()?;
        {
            let me = slf.borrow();
            let bend = match (&me.bend_alter, memo) {
                (Some(bend), Some(memo)) => Some(
                    py.import("copy")?
                        .getattr("deepcopy")?
                        .call1((bend.bind(py), memo))?
                        .unbind(),
                ),
                (Some(bend), None) => Some(bend.clone_ref(py)),
                (None, _) => None,
            };
            let mut copy = copied.extract::<PyRefMut<'_, Self>>()?;
            copy.inner = me.inner.clone();
            copy.bend_alter = bend;
        }
        Ok(copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::__deepcopy__(slf, None)
    }

    /// music21 freezes a score by pickling it, and what this object is lives
    /// in Rust where a pickle cannot see it.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        let py = slf.py();
        let extra = PyDict::new(py);
        extra.set_item("bendAlter", slf.borrow().bend_alter.as_ref())?;
        crate::pickled_extra(slf, &slf.borrow().inner, Some(&extra))
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let (inner, extra) = crate::unpickled_extra::<_, RsArticulation>(slf, state)?;
        let mut me = slf.borrow_mut();
        if let Some(inner) = inner {
            me.inner = inner;
        }
        if let Some(extra) = extra
            && let Ok(bend) = extra.bind(py).get_item("bendAlter")
            && !bend.is_none()
        {
            me.bend_alter = Some(bend.unbind());
        }
        Ok(())
    }
}

/// A finger as music21 keeps one: a whole number as itself, and anything
/// else as the text it writes.
fn finger_from_any(value: &Bound<'_, PyAny>) -> PyResult<Option<Finger>> {
    if value.is_none() {
        return Ok(None);
    }
    if let Ok(number) = value.extract::<i32>() {
        return Ok(Some(Finger::Number(number)));
    }
    Ok(Some(Finger::Written(value.str()?.extract()?)))
}

macro_rules! articulation_kinds {
    ($(($class:ident, $name:literal, $parent:ident, [$($chain:ident),*])),* $(,)?) => {
        $(
            #[doc = concat!("music21's `articulations.", $name, "`.")]
            #[pyclass(name = $name, module = "music21.articulations", extends = $parent, subclass)]
            pub struct $class;

            #[pymethods]
            impl $class {
                #[new]
                #[pyo3(signature = (first = None, **keywords))]
                fn new(
                    first: Option<&Bound<'_, PyAny>>,
                    keywords: Option<&Bound<'_, PyDict>>,
                ) -> PyResult<PyClassInitializer<Self>> {
                    Ok(Articulation::initializer($name, first, keywords)?
                        $(.add_subclass($chain))*
                        .add_subclass($class))
                }
            }
        )*

        fn register_kinds(m: &Bound<'_, PyModule>) -> PyResult<()> {
            $(m.add_class::<$class>()?;)*
            Ok(())
        }

        /// The names this facade replaces in `music21.articulations`.
        pub const NAMES: &[&str] = &["Articulation", $($name),*];
    };
}

articulation_kinds!(
    (Caesura, "Caesura", Articulation, []),
    (DynamicArticulation, "DynamicArticulation", Articulation, []),
    (LengthArticulation, "LengthArticulation", Articulation, []),
    (PitchArticulation, "PitchArticulation", Articulation, []),
    (TechnicalIndication, "TechnicalIndication", Articulation, []),
    (TimbreArticulation, "TimbreArticulation", Articulation, []),
    (Accent, "Accent", DynamicArticulation, [DynamicArticulation]),
    (Bowing, "Bowing", TechnicalIndication, [TechnicalIndication]),
    (
        BreathMark,
        "BreathMark",
        LengthArticulation,
        [LengthArticulation]
    ),
    (
        DetachedLegato,
        "DetachedLegato",
        LengthArticulation,
        [LengthArticulation]
    ),
    (
        Fingering,
        "Fingering",
        TechnicalIndication,
        [TechnicalIndication]
    ),
    (
        FretIndication,
        "FretIndication",
        TechnicalIndication,
        [TechnicalIndication]
    ),
    (
        HandbellIndication,
        "HandbellIndication",
        TechnicalIndication,
        [TechnicalIndication]
    ),
    (
        Harmonic,
        "Harmonic",
        TechnicalIndication,
        [TechnicalIndication]
    ),
    (
        HarpIndication,
        "HarpIndication",
        TechnicalIndication,
        [TechnicalIndication]
    ),
    (
        IndeterminateSlide,
        "IndeterminateSlide",
        PitchArticulation,
        [PitchArticulation]
    ),
    (
        OrganIndication,
        "OrganIndication",
        TechnicalIndication,
        [TechnicalIndication]
    ),
    (
        Staccato,
        "Staccato",
        LengthArticulation,
        [LengthArticulation]
    ),
    (Stress, "Stress", DynamicArticulation, [DynamicArticulation]),
    (Tenuto, "Tenuto", LengthArticulation, [LengthArticulation]),
    (
        Unstress,
        "Unstress",
        DynamicArticulation,
        [DynamicArticulation]
    ),
    (
        WindIndication,
        "WindIndication",
        TechnicalIndication,
        [TechnicalIndication]
    ),
    (
        BrassIndication,
        "BrassIndication",
        WindIndication,
        [TechnicalIndication, WindIndication]
    ),
    (
        Doit,
        "Doit",
        IndeterminateSlide,
        [PitchArticulation, IndeterminateSlide]
    ),
    (DownBow, "DownBow", Bowing, [TechnicalIndication, Bowing]),
    (
        Falloff,
        "Falloff",
        IndeterminateSlide,
        [PitchArticulation, IndeterminateSlide]
    ),
    (
        FretBend,
        "FretBend",
        FretIndication,
        [TechnicalIndication, FretIndication]
    ),
    (
        FretTap,
        "FretTap",
        FretIndication,
        [TechnicalIndication, FretIndication]
    ),
    (
        FrettedPluck,
        "FrettedPluck",
        FretIndication,
        [TechnicalIndication, FretIndication]
    ),
    (
        HarpFingerNails,
        "HarpFingerNails",
        HarpIndication,
        [TechnicalIndication, HarpIndication]
    ),
    (
        OpenString,
        "OpenString",
        Bowing,
        [TechnicalIndication, Bowing]
    ),
    (
        OrganHeel,
        "OrganHeel",
        OrganIndication,
        [TechnicalIndication, OrganIndication]
    ),
    (
        OrganToe,
        "OrganToe",
        OrganIndication,
        [TechnicalIndication, OrganIndication]
    ),
    (
        Pizzicato,
        "Pizzicato",
        Bowing,
        [TechnicalIndication, Bowing]
    ),
    (
        Plop,
        "Plop",
        IndeterminateSlide,
        [PitchArticulation, IndeterminateSlide]
    ),
    (
        Scoop,
        "Scoop",
        IndeterminateSlide,
        [PitchArticulation, IndeterminateSlide]
    ),
    (
        Spiccato,
        "Spiccato",
        Staccato,
        [LengthArticulation, Staccato]
    ),
    (
        Staccatissimo,
        "Staccatissimo",
        Staccato,
        [LengthArticulation, Staccato]
    ),
    (
        Stopped,
        "Stopped",
        WindIndication,
        [TechnicalIndication, WindIndication]
    ),
    (
        StringHarmonic,
        "StringHarmonic",
        Bowing,
        [TechnicalIndication, Bowing]
    ),
    (
        StringIndication,
        "StringIndication",
        Bowing,
        [TechnicalIndication, Bowing]
    ),
    (
        StringThumbPosition,
        "StringThumbPosition",
        Bowing,
        [TechnicalIndication, Bowing]
    ),
    (
        StrongAccent,
        "StrongAccent",
        Accent,
        [DynamicArticulation, Accent]
    ),
    (
        TonguingIndication,
        "TonguingIndication",
        WindIndication,
        [TechnicalIndication, WindIndication]
    ),
    (UpBow, "UpBow", Bowing, [TechnicalIndication, Bowing]),
    (
        WoodwindIndication,
        "WoodwindIndication",
        WindIndication,
        [TechnicalIndication, WindIndication]
    ),
    (
        DoubleTongue,
        "DoubleTongue",
        TonguingIndication,
        [TechnicalIndication, WindIndication, TonguingIndication]
    ),
    (
        NailPizzicato,
        "NailPizzicato",
        Pizzicato,
        [TechnicalIndication, Bowing, Pizzicato]
    ),
    (
        SnapPizzicato,
        "SnapPizzicato",
        Pizzicato,
        [TechnicalIndication, Bowing, Pizzicato]
    ),
    (
        StringFingering,
        "StringFingering",
        StringIndication,
        [TechnicalIndication, Bowing, StringIndication]
    ),
    (
        TripleTongue,
        "TripleTongue",
        TonguingIndication,
        [TechnicalIndication, WindIndication, TonguingIndication]
    ),
);

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Articulation>()?;
    register_kinds(m)
}
