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
    unique_name_from_namespace, unique_name_of, unique_name_of_plural,
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
///
/// A `dict` class, as the package's `Stream` is: `__setattr__` is the only
/// call music21 makes for an attribute, so everything outside the metadata
/// vocabulary -- `Music21Object`'s own `_id` and the rest -- has to have
/// somewhere to go.
#[pyclass(
    name = "Metadata",
    module = "music21.metadata",
    subclass,
    dict,
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
    /// nothing -- or, for one of its four plural names, every value under
    /// the name it stands for.
    fn __getattr__<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        if let Some(unique) = unique_name_of_plural(name) {
            let values: Vec<&str> = self
                .inner
                .get(unique)
                .iter()
                .map(MetadataValue::text)
                .collect();
            return Ok(PyTuple::new(py, values)?.into_any());
        }
        let Some(unique) = unique_name_of(name) else {
            return Err(PyAttributeError::new_err(format!(
                "invalid attributeName: {name}"
            )));
        };
        Ok(match self.inner.string_value(unique) {
            Some(text) => pyo3::types::PyString::new(py, &text).into_any(),
            None => py.None().into_bound(py),
        })
    }

    /// Another value under a name, keeping what is there: music21's
    /// `add`, which takes one value or several and reads the name as its
    /// unique one or its namespaced one. A name outside the vocabulary is
    /// a `KeyError`, as music21 raises.
    fn add(&mut self, name: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let unique = unique_name_of(name)
            .or_else(|| unique_name_from_namespace(name))
            .ok_or_else(|| {
                pyo3::exceptions::PyKeyError::new_err(format!(
                    "Name='{name}' is not a standard metadata name"
                ))
            })?;
        for text in texts_of(value)? {
            if music21_rs_crate::metadata::is_contributor_unique_name(unique) {
                self.inner.add_contributor(unique, text);
            } else {
                self.inner.add_text(unique, text);
            }
        }
        Ok(())
    }

    /// Every value under a name of a caller's own: music21's `getCustom`,
    /// which answers a tuple -- its own `Text` objects, where these are
    /// the strings they say.
    fn getCustom<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyTuple>> {
        let values: Vec<&str> = self
            .inner
            .get(name)
            .iter()
            .map(MetadataValue::text)
            .collect();
        PyTuple::new(py, values)
    }

    /// Another value under a name of a caller's own, keeping what is
    /// there: music21's `addCustom`.
    fn addCustom(&mut self, name: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        for text in texts_of(value)? {
            self.inner.add_text(name, text);
        }
        Ok(())
    }

    /// The values under a name of a caller's own, in place of whatever was
    /// there: music21's `setCustom`.
    fn setCustom(&mut self, name: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let values = texts_of(value)?
            .into_iter()
            .map(MetadataValue::new)
            .collect();
        self.inner.set(name, values);
        Ok(())
    }

    /// Everybody who worked on the piece, in the order they were given:
    /// music21's `contributors`, whose own `Contributor` objects these are
    /// the names of.
    #[getter]
    fn contributors<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let found: Vec<&str> = self
            .inner
            .contributors()
            .map(|(_, value)| value.text())
            .collect();
        PyTuple::new(py, found)
    }

    /// Everybody in a role, or those with no role of their own where none
    /// is asked for: music21's `getContributorsByRole`, which answers its
    /// own `Contributor` objects where these are the names they say.
    #[pyo3(signature = (role))]
    fn getContributorsByRole<'py>(
        &self,
        py: Python<'py>,
        role: Option<&str>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let found: Vec<&str> = self
            .inner
            .contributors_by_role(role)
            .into_iter()
            .map(MetadataValue::text)
            .collect();
        PyTuple::new(py, found)
    }

    /// The namespaced name a unique name stands for: music21's
    /// `uniqueNameToNamespaceName`.
    #[staticmethod]
    fn uniqueNameToNamespaceName(uniqueName: &str) -> Option<&'static str> {
        music21_rs_crate::metadata::namespace_name(uniqueName)
    }

    /// The unique name a namespaced one stands for: music21's
    /// `namespaceNameToUniqueName`.
    #[staticmethod]
    fn namespaceNameToUniqueName(namespaceName: &str) -> Option<&'static str> {
        music21_rs_crate::metadata::unique_name_from_namespace(namespaceName)
    }

    /// Whether a unique name is one of the contributor names: music21's
    /// `isContributorUniqueName`.
    #[staticmethod]
    #[pyo3(signature = (uniqueName))]
    fn isContributorUniqueName(uniqueName: Option<&str>) -> bool {
        uniqueName.is_some_and(music21_rs_crate::metadata::is_contributor_unique_name)
    }

    /// Whether the vocabulary knows a name, by its unique name or its
    /// namespaced one: music21's `isStandardName`.
    fn isStandardName(&self, name: &str) -> bool {
        music21_rs_crate::metadata::is_standard_name(name)
    }

    /// music21 freezes a score by pickling it, and what this object is
    /// lives in Rust where a pickle cannot see it.
    ///
    /// Thawing has to go through here rather than through music21's own
    /// `Music21Object.__setstate__`, which writes the whole dictionary
    /// with `object.__setattr__` -- CPython refuses that for an object
    /// whose class defines `__setattr__` in C, as this one does for the
    /// metadata vocabulary, so a frozen score could not be read back at
    /// all while this class stood in for music21's.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Some(inner) = crate::unpickled::<_, RsMetadata>(slf, state)? {
            slf.borrow_mut().inner = inner;
        }
        Ok(())
    }

    /// The first title the metadata has of the four music21 looks at:
    /// its `bestTitle`.
    #[getter]
    fn bestTitle(&self) -> Option<String> {
        self.inner.best_title()
    }

    /// Writes one name: music21's `__setattr__` for a name of the
    /// vocabulary, and the object's own attribute for anything else.
    ///
    /// music21 says why in as many words: `__setattr__` is the only call
    /// it gets, so after the vocabulary it hands the rest to
    /// `super().__setattr__` for every bare attribute, its own base
    /// classes' included. Without that an installed class could not even
    /// be built -- music21's `Music21Object.__init__` writes `_id`
    /// through here.
    fn __setattr__(slf: &Bound<'_, Self>, name: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        if unique_name_of(name).is_none() {
            // Written into the object's own dictionary, which is what
            // `object.__setattr__` would do -- and that cannot be called
            // here, a type that overrides `__setattr__` being refused it.
            slf.getattr("__dict__")?.set_item(name, value)?;
            return Ok(());
        }
        slf.borrow_mut().write(name, value)
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

    /// Adds somebody who worked on the piece: music21's
    /// `addContributor`, which takes its own `Contributor` -- anything
    /// answering to a role and a name will do -- or the two as they stand,
    /// which is what this package took before.
    #[pyo3(signature = (c, name = None))]
    fn addContributor(&mut self, c: &Bound<'_, PyAny>, name: Option<&str>) -> PyResult<()> {
        if let Some(name) = name {
            // A role and a name, which is what this package took before
            // music21's own one-argument form was carried.
            self.inner.add_contributor(c.extract::<String>()?, name);
            return Ok(());
        }
        // music21 hands over a `Contributor`, whose role and name are what
        // there is to read; anything that answers those two will do.
        let role = c.getattr("role")?;
        let role = if role.is_none() {
            String::new()
        } else {
            role.str()?.to_string()
        };
        let named = c.getattr("name")?;
        let named = if named.is_none() {
            String::new()
        } else {
            named.str()?.to_string()
        };
        self.inner.add_contributor(role, named);
        Ok(())
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

/// A value given as one piece of text or several, as music21 takes
/// either.
fn texts_of(value: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    if value.is_none() {
        return Ok(Vec::new());
    }
    if let Ok(text) = value.extract::<String>() {
        return Ok(vec![text]);
    }
    if let Ok(several) = value.extract::<Vec<String>>() {
        return Ok(several);
    }
    Ok(vec![value.str()?.to_string()])
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
