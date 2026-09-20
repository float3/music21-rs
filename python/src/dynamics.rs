//! music21's `dynamics.Dynamic`, over the crate's dynamics module.
//!
//! A dynamic is a mark and the loudness it stands for. Either may be given:
//! a mark implies a loudness, and a loudness names the mark it falls under
//! while keeping its own value, so a dynamic built from `0.98` is an `fff`
//! that is still louder than one.
//!
//! `DynamicWedge`, `Crescendo` and `Diminuendo` stay music21's. They are
//! spanners — two ends and everything between them — which is stream
//! machinery this crate does not model.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyDict;

use music21_rs_crate::FloatType;
use music21_rs_crate::dynamics::{
    Dynamic as RsDynamic, dynamic_str_from_decimal as rs_dynamic_str_from_decimal,
};

/// The names the `dynamics` facade replaces in `music21.dynamics`.
///
/// The wedges are left where they are; see the module comment.
pub const NAMES: &[&str] = &["Dynamic", "DynamicException", "dynamicStrFromDecimal"];

pyo3::create_exception!(music21_rs_facade, DynamicException, crate::Music21Exception);

error_into!(dynamic_error, DynamicException);

/// music21's `dynamics.Dynamic`.
#[pyclass(
    name = "Dynamic",
    module = "music21.dynamics",
    subclass,
    skip_from_py_object
)]
pub struct Dynamic {
    pub(crate) inner: RsDynamic,
    /// music21 keeps the two names as plain attributes, written afresh
    /// whenever the mark is, so a caller may name a mark of their own.
    long_name: Option<String>,
    english_name: Option<String>,
    placement: Option<String>,
}

impl Dynamic {
    /// The facade around a dynamic of the crate's.
    pub(crate) fn wrap(inner: RsDynamic) -> Self {
        let mut made = Self {
            inner,
            long_name: None,
            english_name: None,
            placement: None,
        };
        made.name_from_value();
        made
    }

    /// Reads the two names off the mark, which is what music21's `value`
    /// setter does and why naming a mark of one's own leaves them empty.
    fn name_from_value(&mut self) {
        self.long_name = self.inner.long_name().map(str::to_string);
        self.english_name = self.inner.english_name().map(str::to_string);
    }

    /// The same dynamic again, names and all.
    fn copied(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            long_name: self.long_name.clone(),
            english_name: self.english_name.clone(),
            placement: self.placement.clone(),
        }
    }
}

#[pymethods]
impl Dynamic {
    /// music21 freezes a score by pickling it, and what this object is lives
    /// in Rust where a pickle cannot see it — so it is written out as text,
    /// and read back into a fresh one of these.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        let me = slf.borrow();
        // The names go with it: a mark nobody has a word for keeps none.
        let written = (
            me.inner.clone(),
            me.long_name.clone(),
            me.english_name.clone(),
            me.placement.clone(),
        );
        drop(me);
        crate::pickled(slf, &written)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        type State = (RsDynamic, Option<String>, Option<String>, Option<String>);
        let Some((inner, long_name, english_name, placement)) =
            crate::unpickled::<_, State>(slf, state)?
        else {
            return Ok(());
        };
        let mut me = slf.borrow_mut();
        me.inner = inner;
        me.long_name = long_name;
        me.english_name = english_name;
        me.placement = placement;
        Ok(())
    }

    /// music21's `Dynamic(value)`, where the value is a mark or a loudness.
    ///
    /// Anything that is not a string is read as a loudness, as it is
    /// upstream, and nothing at all is niente.
    #[new]
    #[pyo3(signature = (value = None, **keywords))]
    fn new(
        value: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let _ = keywords;
        let Some(value) = value.filter(|value| !value.is_none()) else {
            // music21 reads a dynamic given nothing as the loudness `None`,
            // which names niente.
            return Ok(Self::wrap(RsDynamic::new("n")));
        };
        if let Ok(mark) = value.extract::<String>() {
            return Ok(Self::wrap(RsDynamic::new(mark)));
        }
        let scalar: FloatType = value.extract()?;
        Ok(Self::wrap(
            RsDynamic::from_scalar(scalar).map_err(dynamic_error)?,
        ))
    }

    /// music21 writes a dynamic a little to the left of where it stands and
    /// below the top line, since the place a reader would otherwise choose is
    /// wrong often enough to be worth saying; a score exported from one of
    /// these says so too.
    ///
    /// It is written onto the music21 half, which a dynamic standing in for
    /// music21's own has and one built on its own does not.
    #[pyo3(signature = (value = None, **keywords))]
    fn __init__(
        slf: &Bound<'_, Self>,
        value: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let _ = (value, keywords);
        let Ok(style) = slf.getattr("style") else {
            return Ok(());
        };
        style.setattr("absoluteX", -36)?;
        // Below the top line, which lines a dynamic up with a sixteenth.
        style.setattr("absoluteY", -80)?;
        Ok(())
    }

    /// music21's `_reprInternal`, which its `__repr__` writes after the
    /// class name.
    fn _reprInternal(&self) -> String {
        self.inner.value().to_string()
    }

    /// music21's `value`: the mark as written, which names the dynamic and
    /// says how loud it is where nothing louder or softer was asked for.
    #[getter]
    fn get_value(&self) -> String {
        self.inner.value().to_string()
    }

    #[setter]
    fn set_value(&mut self, value: String) {
        self.inner.set_value(value);
        self.name_from_value();
    }

    /// The Italian word for the mark, where music21 has one.
    #[getter]
    fn get_longName(&self) -> Option<String> {
        self.long_name.clone()
    }

    #[setter]
    fn set_longName(&mut self, value: Option<String>) {
        self.long_name = value;
    }

    /// What the mark means in English, where music21 says.
    #[getter]
    fn get_englishName(&self) -> Option<String> {
        self.english_name.clone()
    }

    #[setter]
    fn set_englishName(&mut self, value: Option<String>) {
        self.english_name = value;
    }

    /// Where the mark is written, above the staff or below it. music21 keeps
    /// this off the style, since between two staves of a piano part it says
    /// which hands play it and not merely where the ink goes.
    #[getter]
    fn get_placement(&self) -> Option<String> {
        self.placement.clone()
    }

    #[setter]
    fn set_placement(&mut self, value: Option<String>) {
        self.placement = value;
    }

    /// music21's `volumeScalar`: how loud the dynamic is, between nought and
    /// one. The loudness a caller gave, else the mark's own.
    #[getter]
    fn get_volumeScalar(&self) -> FloatType {
        self.inner.volume_scalar()
    }

    #[setter]
    fn set_volumeScalar(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let Ok(scalar) = value.extract::<FloatType>() else {
            // music21 asks whether it is a number at all, and says the same
            // thing for a number out of range as for something that is not
            // one.
            return Err(DynamicException::new_err(format!(
                "cannot set as volume scalar to: {value}"
            )));
        };
        self.inner.set_volume_scalar(scalar).map_err(dynamic_error)
    }

    /// music21 restores hashing on identity where it defines equality, so
    /// that a set or a dictionary can hold one of these however it compares.
    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        slf.as_ptr() as isize >> 4
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.borrow().copied();
        crate::copy_as_same_type(slf, copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.borrow().copied();
        crate::copy_as_same_type(slf, copied)
    }
}

/// music21's `dynamicStrFromDecimal`: the mark a loudness falls under.
#[pyfunction]
#[pyo3(signature = (n))]
pub fn dynamicStrFromDecimal(n: Option<FloatType>) -> &'static str {
    // music21 reads no loudness at all as niente, as it reads nought.
    rs_dynamic_str_from_decimal(n.unwrap_or(0.0))
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Dynamic>()?;
    m.add_function(wrap_pyfunction!(dynamicStrFromDecimal, m)?)?;
    Ok(())
}
