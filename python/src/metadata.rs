//! music21's `metadata.Metadata`, over the crate's metadata module.
//!
//! A score's named values: its title, who wrote it, which number of a
//! collection it is. Every name comes from music21's own vocabulary, and
//! an attribute reads one as music21's `__getattr__` does -- by its unique
//! name, by one of the four grandfathered work ids, or by one of the
//! twenty Humdrum abbreviations, so `md.otl` and `md.title` are the same
//! title.
//!
//! **What a name answers is a string here**, where music21 answers its own
//! `Text` and `Contributor` objects: `md['title']` gives a tuple of
//! strings and `all(returnPrimitives=True)` is refused. Those classes
//! carry a language, a role and an encoding scheme that nothing in the
//! crate reads, and a caller who needs them has music21 installed.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::PyAttributeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};

use music21_rs_crate::metadata::{
    Metadata as RsMetadata, MetadataQuery, MetadataValue, is_contributor_unique_name,
    unique_name_of,
};

/// The names this module hands out. Nothing here is installed **over**
/// music21's own `metadata`, so `MUSIC21_MODULES` does not name it.
///
/// The facade's rule is that what it does not carry raises rather than
/// falling back. This class cannot keep that bargain: it answers strings
/// where music21 answers `Text` and `Contributor`, so installing it over
/// music21's own would be silently different rather than missing --
/// `RichMetadata` subclasses `Metadata`, the MusicXML writer reads a
/// `Text`'s language, and the corpus bundles read the dates. It is here
/// for a caller who has no music21 at all, which is the whole of the
/// wheel's audience for it.
pub const NAMES: &[&str] = &["Metadata", "MetadataException"];

pyo3::create_exception!(
    music21_rs_facade,
    MetadataException,
    crate::Music21Exception
);

/// music21's `metadata.Metadata`.
#[pyclass(
    name = "Metadata",
    module = "music21.metadata",
    subclass,
    skip_from_py_object
)]
pub struct Metadata {
    pub(crate) inner: RsMetadata,
}

impl Metadata {
    /// The facade around metadata of the crate's.
    pub(crate) fn wrap(inner: RsMetadata) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl Metadata {
    /// music21's sort order for metadata among the things standing at one
    /// offset: before the parts of the score it heads.
    #[classattr]
    fn classSortOrder() -> i32 {
        -30
    }

    /// music21's `Metadata(**keywords)`, each keyword a name of the
    /// vocabulary.
    ///
    /// music21 starts a new metadata with its own version under
    /// `software`; this starts empty, as the crate's does, so that what a
    /// caller reads back is what they wrote.
    #[new]
    #[pyo3(signature = (**keywords))]
    fn new(keywords: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let mut made = Self::wrap(RsMetadata::new());
        if let Some(keywords) = keywords {
            for (key, value) in keywords {
                let name: String = key.extract()?;
                made.write(&name, &value)?;
            }
        }
        Ok(made)
    }

    /// Reads one name: music21's `__getattr__`, which answers a string or
    /// nothing.
    fn __getattr__(&self, name: &str) -> PyResult<Option<String>> {
        let Some(unique) = unique_name_of(name) else {
            return Err(PyAttributeError::new_err(format!(
                "invalid attributeName: {name}"
            )));
        };
        Ok(self.inner.string_value(unique))
    }

    /// Writes one name: music21's `__setattr__` for a name of the
    /// vocabulary.
    fn __setattr__(&mut self, name: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.write(name, value)
    }

    /// Every value under one name, as music21's `md['title']` answers
    /// them -- strings here, where music21 answers its own `Text`.
    fn __getitem__<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyTuple>> {
        let Some(unique) = unique_name_of(name) else {
            return Err(MetadataException::new_err(format!("invalid name: {name}")));
        };
        PyTuple::new(
            py,
            self.inner
                .get(unique)
                .iter()
                .map(|value| value.text().to_string()),
        )
    }

    /// Writes every value under one name: music21's `md['title'] = ...`,
    /// which takes one value or several.
    fn __setitem__(&mut self, name: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.write(name, value)
    }

    /// Adds somebody who worked on the piece: music21's `addContributor`,
    /// which takes its own `Contributor`; here the role and the name are
    /// given as they stand.
    #[pyo3(signature = (role, name))]
    fn addContributor(&mut self, role: &str, name: &str) {
        self.inner.add_contributor(role, name);
    }

    /// Every name and value: music21's `all`.
    ///
    /// `returnPrimitives` is refused, there being no `Text` or
    /// `Contributor` here to hand back.
    #[pyo3(signature = (
        *,
        skipContributors = false,
        skipNonContributors = false,
        returnPrimitives = false,
        returnSorted = true
    ))]
    fn all<'py>(
        &self,
        py: Python<'py>,
        skipContributors: bool,
        skipNonContributors: bool,
        returnPrimitives: bool,
        returnSorted: bool,
    ) -> PyResult<Bound<'py, PyTuple>> {
        if returnPrimitives {
            return Err(MetadataException::new_err(
                "returnPrimitives needs music21's own Text and Contributor, which this package does not carry",
            ));
        }
        let pairs: Vec<(&str, &MetadataValue)> = if returnSorted {
            self.inner.all_sorted()
        } else {
            self.inner.all().collect()
        };
        let kept: Vec<(String, String)> = pairs
            .into_iter()
            .filter(|(name, _)| {
                let contributor = is_contributor_unique_name(name);
                !(skipContributors && contributor) && !(skipNonContributors && !contributor)
            })
            .map(|(name, value)| (name.to_string(), value.text().to_string()))
            .collect();
        PyTuple::new(py, kept)
    }

    /// Whether a query is found, and the field it was found under:
    /// music21's `search`.
    ///
    /// A query holding one of `*.|+?{}` is a regular expression to
    /// music21. Nothing here compiles a pattern, so a query is read as
    /// text and matched by containment, either case ignored.
    #[pyo3(signature = (query = None, field = None, **keywords))]
    fn search(
        &self,
        query: Option<&Bound<'_, PyAny>>,
        field: Option<&str>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<(bool, Option<String>)> {
        let mut asked: Option<String> = query
            .filter(|query| !query.is_none())
            .map(pyo3::Bound::str)
            .transpose()?
            .map(|text| text.to_string());
        let mut field = field.map(str::to_string);
        if asked.is_none() && field.is_none() {
            // music21 reads a lone keyword as the field and the query.
            let Some(keywords) = keywords.filter(|keywords| !keywords.is_empty()) else {
                return Ok((false, None));
            };
            let (key, value) = keywords
                .iter()
                .next()
                .ok_or_else(|| MetadataException::new_err("no field to search"))?;
            field = Some(key.extract()?);
            asked = Some(value.str()?.to_string());
        }
        let Some(asked) = asked else {
            return Ok((false, None));
        };
        let found = self
            .inner
            .search(&MetadataQuery::Text(&asked), field.as_deref());
        Ok((
            found.is_some(),
            found.and_then(|found| found.field().map(str::to_string)),
        ))
    }

    /// The title, as music21 writes it in a repr.
    fn __repr__(&self) -> String {
        match self.inner.title() {
            Some(title) => format!("<music21.metadata.Metadata {title}>"),
            None => "<music21.metadata.Metadata>".to_string(),
        }
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Metadata {
    /// Writes one name, taking one value or several.
    fn write(&mut self, name: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let Some(unique) = unique_name_of(name) else {
            return Err(PyAttributeError::new_err(format!(
                "invalid attributeName: {name}"
            )));
        };
        if value.is_none() {
            self.inner.set(unique, Vec::new());
            return Ok(());
        }
        let texts: Vec<String> = if let Ok(text) = value.extract::<String>() {
            vec![text]
        } else if let Ok(several) = value.extract::<Vec<String>>() {
            several
        } else {
            vec![value.str()?.to_string()]
        };
        let values = texts
            .into_iter()
            .map(|text| {
                if is_contributor_unique_name(unique) {
                    MetadataValue::with_role(text, unique)
                } else {
                    MetadataValue::new(text)
                }
            })
            .collect();
        self.inner.set(unique, values);
        Ok(())
    }
}

/// Adds the metadata classes to a module.
///
/// The exception is not among them: building one builds music21's own
/// `Music21Exception` and so imports music21, which this wheel is not to
/// do for a name nobody has asked for. `lib.rs`'s table hands it out on
/// first asking.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Metadata>()?;
    Ok(())
}
