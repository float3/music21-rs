//! music21's `clef` module, over the crate's clefs.
//!
//! `Clef` holds the crate's `Clef`, and every one of music21's clef classes
//! is a class of its own extending its family's, as in music21, so
//! `TrebleClef()` starts out as music21's does and `isinstance(treble,
//! GClef)` holds.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;

use crate::Walkable;
use pyo3::types::PyDict;

use music21_rs_crate::clef::{Clef as RsClef, ClefKind};
use music21_rs_crate::notation::StemDirection;

use crate::pitch::pitch_from_any;

pyo3::create_exception!(music21_rs_facade, ClefException, crate::Music21Exception);

error_into!(clef_error, ClefException);

/// music21's `clef.Clef`: a clef, with the sign, line, octave change and
/// lowest line of the class it was made as.
#[pyclass(name = "Clef", module = "music21.clef", subclass, skip_from_py_object)]
pub struct Clef {
    pub(crate) inner: RsClef,
    /// Whether a lowest line was ever given to a clef whose class keeps
    /// none, as music21's attribute assignment gives one.
    lowest_line_given: bool,
}

impl Clef {
    fn initializer(class: &str) -> PyResult<PyClassInitializer<Self>> {
        let kind = ClefKind::from_class_name(class)
            .ok_or_else(|| ClefException::new_err(format!("no clef class {class}")))?;
        Ok(PyClassInitializer::from(Self {
            inner: RsClef::of_kind(kind),
            lowest_line_given: false,
        }))
    }

    /// A clef of the class a new one is made as: the one installed over
    /// music21's where there is one, since music21 holds nothing else in a
    /// stream, and this wheel's own where not.
    fn object(py: Python<'_>, inner: RsClef) -> PyResult<Py<PyAny>> {
        let name = inner.kind().class_name();
        let class = match crate::installed_class(py, "music21.clef", name) {
            Some(class) => class,
            None => py
                .import("music21_rs")
                .or_else(|_| py.import("music21_rs_facade"))?
                .getattr(name)?,
        };
        let made = class.call0()?;
        made.extract::<PyRefMut<'_, Self>>()?.inner = inner;
        Ok(made.unbind())
    }

    /// Whether the clef's class keeps a lowest line at all: music21's
    /// `PitchClef` and `PercussionClef` do and the rest have no such
    /// attribute.
    fn keeps_lowest_line(&self) -> bool {
        self.lowest_line_given || self.inner.places_pitches() || self.inner.is_a("PercussionClef")
    }
}

#[pymethods]
impl Clef {
    #[new]
    #[pyo3(signature = (**_keywords))]
    fn new(_keywords: Option<&Bound<'_, PyDict>>) -> PyResult<PyClassInitializer<Self>> {
        Self::initializer("Clef")
    }

    /// music21's `sign`: `'G'`, `'F'`, `'C'`, `'percussion'`, `'none'`, or
    /// `None` for the bare class.
    #[getter]
    fn get_sign(&self) -> Option<&str> {
        self.inner.sign()
    }

    #[setter]
    fn set_sign(&mut self, value: Option<String>) {
        self.inner.set_sign(value);
    }

    /// music21's `line`: the line the sign sits on, from the bottom.
    #[getter]
    fn get_line(&self) -> Option<u8> {
        self.inner.line()
    }

    #[setter]
    fn set_line(&mut self, value: Option<u8>) {
        self.inner.set_line(value);
    }

    /// music21's `octaveChange`; changing it on a pitched clef moves its
    /// lowest line.
    #[getter]
    fn get_octaveChange(&self) -> i32 {
        self.inner.octave_change()
    }

    #[setter]
    fn set_octaveChange(&mut self, value: i32) {
        self.inner.set_octave_change(value);
    }

    /// music21's `lowestLine`: the diatonic note number of the note on the
    /// lowest line.
    #[getter]
    fn get_lowestLine(slf: &Bound<'_, Self>) -> PyResult<Option<i32>> {
        let me = slf.borrow();
        if !me.keeps_lowest_line() {
            return Err(pyo3::exceptions::PyAttributeError::new_err(format!(
                "'{}' object has no attribute 'lowestLine'",
                slf.get_type().name()?
            )));
        }
        Ok(me.inner.lowest_line())
    }

    #[setter]
    fn set_lowestLine(&mut self, value: Option<i32>) {
        self.inner.set_lowest_line(value);
        self.lowest_line_given = true;
    }

    /// music21's `name`: the class name without `Clef`, lower case first.
    #[getter]
    fn name(&self) -> String {
        self.inner.name()
    }

    /// music21's `getStemDirectionForPitches`: `'up'` or `'down'` for one
    /// pitch or a list of them.
    #[pyo3(signature = (pitches, *, firstLastOnly = true, extremePitchOnly = false))]
    fn getStemDirectionForPitches(
        &self,
        pitches: &Bound<'_, PyAny>,
        firstLastOnly: bool,
        extremePitchOnly: bool,
    ) -> PyResult<&'static str> {
        let read = match pitch_from_any(pitches) {
            Ok(one) => vec![one],
            Err(_) => {
                let mut read = Vec::new();
                for item in pitches.walk()? {
                    read.push(pitch_from_any(&item?)?);
                }
                read
            }
        };
        let direction = self
            .inner
            .stem_direction_for_pitches(&read, firstLastOnly, extremePitchOnly)
            .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?;
        Ok(match direction {
            StemDirection::Down => "down",
            _ => "up",
        })
    }

    fn _reprInternal(&self) -> &'static str {
        ""
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        Ok(format!("<music21.clef.{}>", slf.get_type().name()?))
    }

    /// music21's equality: the same class, sign, line and octave change.
    fn __eq__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let Ok(theirs) = other.extract::<PyRef<'_, Self>>() else {
            return Ok(false);
        };
        let me = slf.borrow();
        Ok(me.inner.kind() == theirs.inner.kind()
            && me.inner.sign() == theirs.inner.sign()
            && me.inner.line() == theirs.inner.line()
            && me.inner.octave_change() == theirs.inner.octave_change())
    }

    /// music21 hashes a clef by identity, however it compares.
    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        slf.as_ptr() as isize >> 4
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.get_type().call0()?;
        {
            let me = slf.borrow();
            let mut copy = copied.extract::<PyRefMut<'_, Self>>()?;
            copy.inner = me.inner.clone();
            copy.lowest_line_given = me.lowest_line_given;
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
        extra.set_item("lowestLineGiven", slf.borrow().lowest_line_given)?;
        crate::pickled_extra(slf, &slf.borrow().inner, Some(&extra))
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let (inner, extra) = crate::unpickled_extra::<_, RsClef>(slf, state)?;
        let mut me = slf.borrow_mut();
        if let Some(inner) = inner {
            me.inner = inner;
        }
        if let Some(extra) = extra
            && let Ok(given) = extra.bind(py).get_item("lowestLineGiven")
        {
            me.lowest_line_given = given.extract().unwrap_or(false);
        }
        Ok(())
    }
}

macro_rules! clef_kinds {
    ($(($class:ident, $name:literal, $parent:ident, [$($chain:ident),*])),* $(,)?) => {
        $(
            #[doc = concat!("music21's `clef.", $name, "`.")]
            #[pyclass(name = $name, module = "music21.clef", extends = $parent, subclass)]
            pub struct $class;

            #[pymethods]
            impl $class {
                #[new]
                #[pyo3(signature = (**_keywords))]
                fn new(_keywords: Option<&Bound<'_, PyDict>>) -> PyResult<PyClassInitializer<Self>> {
                    Ok(Clef::initializer($name)?
                        $(.add_subclass($chain))*
                        .add_subclass($class))
                }
            }
        )*

        fn register_kinds(m: &Bound<'_, PyModule>) -> PyResult<()> {
            $(m.add_class::<$class>()?;)*
            Ok(())
        }

        /// The names this facade replaces in `music21.clef`.
        pub const NAMES: &[&str] = &[
            "Clef",
            "ClefException",
            "clefFromString",
            "bestClef",
            $($name),*
        ];
    };
}

clef_kinds!(
    (NoClef, "NoClef", Clef, []),
    (PercussionClef, "PercussionClef", Clef, []),
    (PitchClef, "PitchClef", Clef, []),
    (CClef, "CClef", PitchClef, [PitchClef]),
    (FClef, "FClef", PitchClef, [PitchClef]),
    (GClef, "GClef", PitchClef, [PitchClef]),
    (JianpuClef, "JianpuClef", NoClef, [NoClef]),
    (TabClef, "TabClef", PitchClef, [PitchClef]),
    (AltoClef, "AltoClef", CClef, [PitchClef, CClef]),
    (Bass8vaClef, "Bass8vaClef", FClef, [PitchClef, FClef]),
    (Bass8vbClef, "Bass8vbClef", FClef, [PitchClef, FClef]),
    (BassClef, "BassClef", FClef, [PitchClef, FClef]),
    (CBaritoneClef, "CBaritoneClef", CClef, [PitchClef, CClef]),
    (FBaritoneClef, "FBaritoneClef", FClef, [PitchClef, FClef]),
    (
        FrenchViolinClef,
        "FrenchViolinClef",
        GClef,
        [PitchClef, GClef]
    ),
    (GSopranoClef, "GSopranoClef", GClef, [PitchClef, GClef]),
    (
        MezzoSopranoClef,
        "MezzoSopranoClef",
        CClef,
        [PitchClef, CClef]
    ),
    (SopranoClef, "SopranoClef", CClef, [PitchClef, CClef]),
    (SubBassClef, "SubBassClef", FClef, [PitchClef, FClef]),
    (TenorClef, "TenorClef", CClef, [PitchClef, CClef]),
    (TrebleClef, "TrebleClef", GClef, [PitchClef, GClef]),
    (
        Treble8vaClef,
        "Treble8vaClef",
        TrebleClef,
        [PitchClef, GClef, TrebleClef]
    ),
    (
        Treble8vbClef,
        "Treble8vbClef",
        TrebleClef,
        [PitchClef, GClef, TrebleClef]
    ),
);

/// music21's `clefFromString`: the clef a string like `'G2'` names.
#[pyfunction]
#[pyo3(signature = (clefString, octaveShift = 0))]
fn clefFromString(py: Python<'_>, clefString: &str, octaveShift: i32) -> PyResult<Py<PyAny>> {
    let clef = RsClef::from_string(clefString, octaveShift).map_err(clef_error)?;
    Clef::object(py, clef)
}

/// music21's `bestClef`: the clef that best fits the notes and chords of a
/// stream, or of everything inside it with `recurse`.
#[pyfunction]
#[pyo3(signature = (streamObj, allowTreble8vb = false, recurse = false))]
fn bestClef(
    py: Python<'_>,
    streamObj: &Bound<'_, PyAny>,
    allowTreble8vb: bool,
    recurse: bool,
) -> PyResult<Py<PyAny>> {
    let walked = if recurse {
        streamObj.call_method0("recurse")?
    } else {
        streamObj.call_method0("iter")?
    };
    let mut pitches = Vec::new();
    for element in walked.getattr("notesAndRests")?.walk()? {
        for pitch in element?.getattr("pitches")?.walk()? {
            pitches.push(pitch_from_any(&pitch?)?);
        }
    }
    Clef::object(py, RsClef::best_for(&pitches, allowTreble8vb))
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Clef>()?;
    register_kinds(m)?;
    m.add_function(wrap_pyfunction!(clefFromString, m)?)?;
    m.add_function(wrap_pyfunction!(bestClef, m)?)?;
    Ok(())
}
