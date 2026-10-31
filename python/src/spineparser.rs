//! music21's `humdrum.spineParser.GlobalComment` and `MiscTandem`: a `!!`
//! line and an instruction with no object of its own, each kept in the
//! score where it was written.
//!
//! The rest of `spineParser` is the reader, which is `from_humdrum`.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyDict;

use music21_rs_crate::humdrum::{GlobalComment as RsGlobalComment, MiscTandem as RsMiscTandem};

/// music21's `GlobalComment`: a comment on the whole score.
///
/// **Nothing is installed over music21's own.** Once installed, readers
/// hand back music21's class, and this one stands in only where music21 is
/// not there.
#[pyclass(
    name = "GlobalComment",
    module = "music21.humdrum.spineParser",
    subclass,
    skip_from_py_object
)]
pub struct GlobalComment {
    pub(crate) inner: RsGlobalComment,
}

impl GlobalComment {
    /// The facade around a comment of the crate's.
    pub(crate) fn wrap(inner: RsGlobalComment) -> Self {
        Self { inner }
    }
}

/// What music21 keeps of a written comment: the bangs gone, then the
/// spaces either side (`'!! said '` is `'said'`).
fn said_of(comment: &str) -> &str {
    if !comment.starts_with("!!") {
        return comment.trim();
    }
    comment.trim_start_matches('!').trim()
}

#[pymethods]
impl GlobalComment {
    /// music21's `GlobalComment(comment='')`.
    #[new]
    #[pyo3(signature = (comment = "", **_keywords))]
    fn new(comment: &str, _keywords: Option<&Bound<'_, PyDict>>) -> Self {
        Self::wrap(RsGlobalComment::new(said_of(comment), 0))
    }

    /// What music21 says this is, as its `classes` answers.
    #[getter]
    fn classes(slf: &Bound<'_, Self>) -> PyResult<Vec<String>> {
        crate::stream::lineage_of(slf.as_any())
    }

    /// What it says: music21's `comment`.
    #[getter]
    fn get_comment(&self) -> &str {
        self.inner.comment()
    }

    #[setter]
    fn set_comment(&mut self, value: &str) {
        self.inner = RsGlobalComment::new(value, self.inner.priority());
    }

    /// music21's `priority`, which sorts it among what stands at its
    /// offset.
    #[getter]
    fn get_priority(&self) -> i32 {
        self.inner.priority()
    }

    #[setter]
    fn set_priority(&mut self, value: i32) {
        self.inner = RsGlobalComment::new(self.inner.comment(), value);
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        // music21 writes the comment as Python writes a string.
        let shown = pyo3::types::PyString::new(py, self.inner.comment()).repr()?;
        Ok(format!(
            "<music21.humdrum.spineParser.GlobalComment {shown}>"
        ))
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

/// music21's `MiscTandem`: an instruction of a spine the reader has no
/// object for.
///
/// **Nothing is installed over music21's own**, as for `GlobalComment`.
#[pyclass(
    name = "MiscTandem",
    module = "music21.humdrum.spineParser",
    subclass,
    skip_from_py_object
)]
pub struct MiscTandem {
    pub(crate) inner: RsMiscTandem,
}

impl MiscTandem {
    /// The facade around an instruction of the crate's.
    pub(crate) fn wrap(inner: RsMiscTandem) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl MiscTandem {
    /// music21's `MiscTandem(tandem='')`.
    #[new]
    #[pyo3(signature = (tandem = "", **_keywords))]
    fn new(tandem: &str, _keywords: Option<&Bound<'_, PyDict>>) -> Self {
        Self::wrap(RsMiscTandem::new(tandem, 0))
    }

    /// What music21 says this is, as its `classes` answers.
    #[getter]
    fn classes(slf: &Bound<'_, Self>) -> PyResult<Vec<String>> {
        crate::stream::lineage_of(slf.as_any())
    }

    /// The instruction as written: music21's `tandem`.
    #[getter]
    fn get_tandem(&self) -> &str {
        self.inner.tandem()
    }

    #[setter]
    fn set_tandem(&mut self, value: &str) {
        self.inner = RsMiscTandem::new(value, self.inner.priority());
    }

    /// music21's `priority`: the line it was written on.
    #[getter]
    fn get_priority(&self) -> i32 {
        self.inner.priority()
    }

    #[setter]
    fn set_priority(&mut self, value: i32) {
        self.inner = RsMiscTandem::new(self.inner.tandem(), value);
    }

    fn __repr__(&self) -> String {
        format!(
            "<music21.humdrum.spineParser.MiscTandem {}>",
            self.inner.tandem()
        )
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<GlobalComment>()?;
    m.add_class::<MiscTandem>()
}
