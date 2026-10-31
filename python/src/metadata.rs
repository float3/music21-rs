//! music21's `metadata.Metadata`, over the crate's metadata module.
//!
//! A score's named values: its title, who wrote it, which number of a
//! collection it is. Every name comes from music21's own vocabulary, and
//! an attribute reads one as music21's `__getattr__` does -- by its unique
//! name, by one of the four grandfathered work ids, or by one of the
//! twenty Humdrum abbreviations, so `md.otl` and `md.title` are the same
//! title.
//!
//! **What a name answers is the primitive the name takes**, as music21
//! answers one: `md['title']` is a tuple of `Text` objects, `md['composer']`
//! of `Contributor`s, `md['dateCreated']` of `DateSingle`s and
//! `md['copyright']` of `Copyright`s -- this package's own, over the crate's
//! `metadata::primitives`. `metadata::value_kind_of` is what says which
//! kind a name takes, which is music21's own `UNIQUE_NAME_TO_VALUE_TYPE`
//! and what its `convertValue` reads.
//!
//! **music21's own private readers run over this object.** `_get`, `_add`
//! and `_set` are the three the facade carries, so
//! `_getStringValueByNamespaceName`, `_getPluralAttribute` and the rest --
//! music21's own, inherited because a private member is not blocked -- read
//! the crate's values through them. `_contents` is a reading of those
//! values rather than the dictionary music21 keeps, since nothing here
//! writes through it.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::PyAttributeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};

use music21_rs_crate::metadata::{
    Metadata as RsMetadata, MetadataQuery, MetadataValue, ValueKind, is_contributor_unique_name,
    primitives as rsprim, unique_name_from_namespace, unique_name_of, unique_name_of_plural,
    value_kind_of,
};

/// The names this module hands out. Nothing here is installed **over**
/// music21's own `metadata` by `install_into_music21`, so
/// `MUSIC21_MODULES` does not name it -- though the doctest harness swaps
/// it, which is how its own examples run against this.
///
/// This used to say the facade could not keep the package's bargain
/// because it answered strings where music21 answers `Text` and
/// `Contributor`. It answers the primitives now, so that reason is gone.
/// What is left to weigh before installing it everywhere: `RichMetadata`
/// subclasses `Metadata` and carries numbers of its own that nothing here
/// answers, and `_contents` is a *reading* of the crate's values rather
/// than the store music21 keeps -- so music21's own corpus bundles and
/// `freezeThaw`, which go through both, would be running on something
/// nobody has measured. Measure that before adding it to the table.
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
    /// It is signed under `software`, as music21's own constructor signs
    /// one with its version -- its `software` and `all` examples both read
    /// that line back. The crate's `Metadata::new` starts empty, a library
    /// having no version of anybody else's to write.
    #[new]
    #[pyo3(signature = (**keywords))]
    fn new(py: Python<'_>, keywords: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let mut made = Self::wrap(RsMetadata::new());
        made.inner.add_text("software", software_line(py));
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
    ///
    /// What music21 answers through a site is refused, as on every element.
    fn __getattr__<'py>(slf: &Bound<'py, Self>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        if crate::stream::is_site_member(name) {
            return crate::stream::no_site(slf.as_any(), name).map(|found| found.into_bound(py));
        }
        let me = slf.borrow();
        if let Some(unique) = unique_name_of_plural(name) {
            let values: Vec<&str> = me
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
        Ok(match me.inner.string_value(unique) {
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
            .ok_or_else(|| unknown_name(name))?;
        for found in given_values(unique, value)? {
            self.inner.add(unique, found);
        }
        Ok(())
    }

    /// Every value under a name of a caller's own: music21's `getCustom`,
    /// which answers a tuple -- its own `Text` objects, where these are
    /// the strings they say.
    fn getCustom<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyTuple>> {
        let mut found = Vec::new();
        for value in self.inner.get(name) {
            found.push(primitive_object(py, name, value)?);
        }
        PyTuple::new(py, found)
    }

    /// Another value under a name of a caller's own, keeping what is
    /// there: music21's `addCustom`.
    fn addCustom(&mut self, name: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        for found in given_values(name, value)? {
            self.inner.add(name, found);
        }
        Ok(())
    }

    /// The values under a name of a caller's own, in place of whatever was
    /// there: music21's `setCustom`.
    fn setCustom(&mut self, name: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let values = given_values(name, value)?;
        self.inner.set(name, values);
        Ok(())
    }

    /// Everybody who worked on the piece, in the order they were given:
    /// music21's `contributors`, whose own `Contributor` objects these are
    /// the names of.
    #[getter]
    fn contributors<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let mut found = Vec::new();
        for (name, value) in self.inner.contributors() {
            found.push(primitive_object(py, name, value)?);
        }
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
        let mut found = Vec::new();
        for value in self.inner.contributors_by_role(role) {
            found.push(primitive_object(py, "otherContributor", value)?);
        }
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
        // music21 keeps `bestTitle` as a property with no setter of its own.
        if name == "bestTitle" {
            return Err(PyAttributeError::new_err(
                "property 'bestTitle' of 'Metadata' object has no setter",
            ));
        }
        if let Some(unique) = unique_name_of_plural(name) {
            let values = given_values(unique, value)?;
            slf.borrow_mut().inner.set(unique, values);
            return Ok(());
        }
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
    fn __getitem__<'py>(
        &self,
        py: Python<'py>,
        name: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let Ok(name) = name.extract::<String>() else {
            return Err(pyo3::exceptions::PyKeyError::new_err(
                "metadata key must be str",
            ));
        };
        let unique = unique_name_of(&name)
            .or_else(|| unique_name_from_namespace(&name))
            .ok_or_else(|| unknown_name(&name))?;
        let mut found = Vec::new();
        for value in self.inner.get(unique) {
            found.push(primitive_object(py, unique, value)?);
        }
        PyTuple::new(py, found)
    }

    /// Writes every value under one name: music21's `md['title'] = ...`,
    /// which takes one value or several.
    fn __setitem__(&mut self, name: &Bound<'_, PyAny>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let Ok(name) = name.extract::<String>() else {
            return Err(pyo3::exceptions::PyKeyError::new_err(
                "metadata key must be str",
            ));
        };
        let unique = unique_name_of(&name)
            .or_else(|| unique_name_from_namespace(&name))
            .ok_or_else(|| unknown_name(&name))?;
        let values = given_values(unique, value)?;
        self.inner.set(unique, values);
        Ok(())
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
        // music21 hands over a `Contributor`, which is read whole -- every
        // name it goes by and the dates of a life included.
        let read = crate::primitives::read_contributor(c)?;
        let role = read.role().unwrap_or_default().to_string();
        let unique = if is_contributor_unique_name(&role) {
            role
        } else {
            "otherContributor".to_string()
        };
        self.inner.add(unique, MetadataValue::of_contributor(read));
        Ok(())
    }

    /// Every name and value: music21's `all`, which answers the values as
    /// the strings they say, or as the primitives they are where
    /// `returnPrimitives` asks.
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
        #[pyo3(from_py_with = crate::truthy)] skipContributors: bool,
        #[pyo3(from_py_with = crate::truthy)] skipNonContributors: bool,
        #[pyo3(from_py_with = crate::truthy)] returnPrimitives: bool,
        #[pyo3(from_py_with = crate::truthy)] returnSorted: bool,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let pairs: Vec<(&str, &MetadataValue)> = if returnSorted {
            self.inner.all_sorted()
        } else {
            self.inner.all().collect()
        };
        let mut kept = Vec::new();
        for (name, value) in pairs {
            let contributor = is_contributor_unique_name(name);
            if (skipContributors && contributor) || (skipNonContributors && !contributor) {
                continue;
            }
            let said = if returnPrimitives {
                primitive_object(py, name, value)?
            } else {
                pyo3::types::PyString::new(py, value.text()).into_any()
            };
            kept.push(PyTuple::new(
                py,
                [pyo3::types::PyString::new(py, name).into_any(), said],
            )?);
        }
        PyTuple::new(py, kept)
    }

    /// music21's `_get`: every value under a name, as a tuple of the
    /// primitives that name takes. A name outside the vocabulary is a
    /// `KeyError` unless it is said to be a caller's own.
    #[pyo3(signature = (name, isCustom))]
    fn _get<'py>(
        &self,
        py: Python<'py>,
        name: &str,
        #[pyo3(from_py_with = crate::truthy)] isCustom: bool,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let unique = if isCustom {
            name.to_string()
        } else {
            unique_name_of(name)
                .or_else(|| unique_name_from_namespace(name))
                .ok_or_else(|| unknown_name(name))?
                .to_string()
        };
        let mut found = Vec::new();
        for value in self.inner.get(&unique) {
            found.push(primitive_object(py, &unique, value)?);
        }
        PyTuple::new(py, found)
    }

    /// music21's `_add`: one value or several under a name, keeping what
    /// is there.
    #[pyo3(signature = (name, value, isCustom))]
    fn _add(
        &mut self,
        name: &str,
        value: &Bound<'_, PyAny>,
        #[pyo3(from_py_with = crate::truthy)] isCustom: bool,
    ) -> PyResult<()> {
        let unique = if isCustom {
            name.to_string()
        } else {
            unique_name_of(name)
                .or_else(|| unique_name_from_namespace(name))
                .ok_or_else(|| unknown_name(name))?
                .to_string()
        };
        for found in given_values(&unique, value)? {
            self.inner.add(unique.clone(), found);
        }
        Ok(())
    }

    /// music21's `_set`: one value or several under a name, in place of
    /// whatever was there.
    #[pyo3(signature = (name, value, isCustom))]
    fn _set(
        &mut self,
        name: &str,
        value: &Bound<'_, PyAny>,
        #[pyo3(from_py_with = crate::truthy)] isCustom: bool,
    ) -> PyResult<()> {
        let unique = if isCustom {
            name.to_string()
        } else {
            unique_name_of(name)
                .or_else(|| unique_name_from_namespace(name))
                .ok_or_else(|| unknown_name(name))?
                .to_string()
        };
        let values = given_values(&unique, value)?;
        self.inner.set(unique, values);
        Ok(())
    }

    /// What music21 keeps its values in, read off the crate's: a
    /// dictionary of the primitives under each name.
    ///
    /// It is a *reading*, not the store itself -- nothing here writes
    /// through it, `_get`, `_add` and `_set` being the facade's own -- so
    /// what a caller does to the dictionary it is handed reaches nothing.
    #[getter]
    fn _contents<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let found = PyDict::new(py);
        for (name, value) in self.inner.all() {
            let made = primitive_object(py, name, value)?;
            match found.get_item(name)? {
                Some(held) => held.call_method1("append", (made,))?,
                None => {
                    let list = pyo3::types::PyList::empty(py);
                    list.append(made)?;
                    found.set_item(name, &list)?;
                    list.into_any()
                }
            };
        }
        Ok(found)
    }

    /// music21's `convertValue`: whatever it was handed, as the value the
    /// name takes. A value already of that kind is handed back as it
    /// stands, which is what music21's own `is` test shows.
    #[staticmethod]
    fn convertValue<'py>(
        py: Python<'py>,
        uniqueName: &str,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let unique = unique_name_of(uniqueName)
            .or_else(|| unique_name_from_namespace(uniqueName))
            .unwrap_or(uniqueName);
        let already = match kind_of(unique) {
            ValueKind::Text => {
                value.cast::<crate::primitives::Text>().is_ok()
                    && value.cast::<crate::primitives::Copyright>().is_err()
            }
            ValueKind::Copyright => value.cast::<crate::primitives::Copyright>().is_ok(),
            ValueKind::Contributor => value.cast::<crate::primitives::Contributor>().is_ok(),
            ValueKind::Date => value.cast::<crate::primitives::DatePrimitive>().is_ok(),
            ValueKind::Number => value.is_instance_of::<pyo3::types::PyInt>(),
        };
        if already {
            return Ok(value.clone());
        }
        primitive_object(py, unique, &crate_value(unique, value)?)
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
        // music21 reads a query holding any of `*.|+?{}` as a regular
        // expression, and anything callable as a test of its own. The crate
        // takes either as a matcher over each value's text; what compiles
        // the pattern is Python's `re`, there being none in the crate.
        let py = query.map_or_else(|| None, |query| Some(query.py()));
        let compiled = match (py, query) {
            (Some(_), Some(query)) if query.is_callable() => Some(query.clone()),
            (Some(py), _) if asked.contains(['*', '.', '|', '+', '?', '{', '}']) => py
                .import("re")
                .ok()
                .and_then(|re| {
                    let flags = re.getattr("IGNORECASE").ok()?;
                    re.call_method1("compile", (asked.as_str(), flags)).ok()
                })
                .map(|pattern| {
                    pattern
                        .getattr("search")
                        .unwrap_or_else(|_| pattern.clone())
                }),
            _ => None,
        };
        let found = match &compiled {
            None => self
                .inner
                .search(&MetadataQuery::Text(&asked), field.as_deref()),
            Some(matcher) => {
                let ask = |text: &str| -> bool {
                    matcher
                        .call1((text,))
                        .ok()
                        .and_then(|answer| answer.is_truthy().ok())
                        .unwrap_or(false)
                };
                self.inner
                    .search(&MetadataQuery::Matcher(&ask), field.as_deref())
            }
        };
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

/// What kind of value a name takes, a name of a caller's own taking a
/// text as music21's does.
fn kind_of(unique: &str) -> ValueKind {
    value_kind_of(unique).unwrap_or(ValueKind::Text)
}

/// One value under a name as the primitive that name takes: music21's own
/// `Text`, `Copyright`, `Contributor` or `DateSingle`, which is what every
/// reading of a name answers.
fn primitive_object<'py>(
    py: Python<'py>,
    unique: &str,
    value: &MetadataValue,
) -> PyResult<Bound<'py, PyAny>> {
    match kind_of(unique) {
        ValueKind::Number => {
            let read: i64 = value.text().trim().parse().map_err(|_| {
                pyo3::exceptions::PyValueError::new_err(format!(
                    "invalid literal for int() with base 10: '{}'",
                    value.text()
                ))
            })?;
            Ok(read.into_pyobject(py)?.into_any())
        }
        ValueKind::Date => {
            let date = match value.as_date() {
                Some(date) => date.clone(),
                None => rsprim::DatePrimitive::single(value.text(), rsprim::Relevance::Certain)
                    .map_err(metadata_error)?,
            };
            crate::primitives::date_object(py, date)
        }
        ValueKind::Contributor => {
            let contributor = match value.as_contributor() {
                Some(found) => found.clone(),
                None => match value.role() {
                    Some(role) => rsprim::Contributor::new(role, [rsprim::Text::new(value.text())]),
                    None => {
                        let mut made = rsprim::Contributor::unnamed();
                        made.set_name(rsprim::Text::new(value.text()));
                        made
                    }
                },
            };
            crate::primitives::contributor_object(py, contributor)
        }
        kind => {
            let text = value
                .as_text()
                .cloned()
                .unwrap_or_else(|| rsprim::Text::new(value.text()));
            crate::primitives::text_object(py, text, kind == ValueKind::Copyright)
        }
    }
}

/// Whatever a caller handed over, as the value a name takes: music21's
/// `convertValue`.
fn crate_value(unique: &str, value: &Bound<'_, PyAny>) -> PyResult<MetadataValue> {
    match kind_of(unique) {
        ValueKind::Number => Ok(MetadataValue::new(value.str()?.to_string())),
        ValueKind::Date => {
            // music21 reads a `Text` as the string it says before asking
            // for a date, and falls back to a plain text where no date can
            // be read at all.
            let asked = if value.cast::<crate::primitives::Text>().is_ok() {
                pyo3::types::PyString::new(value.py(), &value.str()?.to_string()).into_any()
            } else {
                value.clone()
            };
            match crate::primitives::read_date(&asked) {
                Ok(date) => Ok(MetadataValue::of_date(date)),
                Err(_) => Ok(MetadataValue::of_text(crate::primitives::read_text(value)?)),
            }
        }
        ValueKind::Contributor => {
            // music21 gives a bare name the role the name is, and leaves a
            // contributor it was handed exactly as it came -- with no role
            // at all where it has none.
            let given = value.cast::<crate::primitives::Contributor>().is_ok()
                || value.hasattr("names").unwrap_or(false);
            let mut read = crate::primitives::read_contributor(value)?;
            if !given && read.role().is_none() {
                read.set_role(Some(unique.to_string()));
            }
            Ok(MetadataValue::of_contributor(read))
        }
        ValueKind::Copyright => {
            let mut read = crate::primitives::read_text(value)?;
            if read.role().is_none() {
                read.set_role(None);
            }
            Ok(MetadataValue::of_text(read))
        }
        ValueKind::Text => Ok(MetadataValue::of_text(crate::primitives::read_text(value)?)),
    }
}

/// Every value a caller handed over, as music21 takes one or several --
/// and a string whole, which is iterable and must not be walked.
fn given_values(unique: &str, value: &Bound<'_, PyAny>) -> PyResult<Vec<MetadataValue>> {
    if value.is_none() {
        return Ok(Vec::new());
    }
    let one = value.extract::<String>().is_ok()
        || value.cast::<crate::primitives::Text>().is_ok()
        || value.cast::<crate::primitives::Contributor>().is_ok()
        || value.cast::<crate::primitives::DatePrimitive>().is_ok()
        || value.hasattr("role").unwrap_or(false)
        || value.hasattr("isTranslated").unwrap_or(false)
        || value.hasattr("relevance").unwrap_or(false);
    if one {
        return Ok(vec![crate_value(unique, value)?]);
    }
    match crate::Walkable::walk(value) {
        Err(_) => Ok(vec![crate_value(unique, value)?]),
        Ok(walk) => {
            let mut read = Vec::new();
            for item in walk {
                read.push(crate_value(unique, &item?)?);
            }
            Ok(read)
        }
    }
}

/// music21's own `KeyError` for a name outside the vocabulary.
fn unknown_name(name: &str) -> PyErr {
    pyo3::exceptions::PyKeyError::new_err(format!(
        "Name='{name}' is not a standard metadata name. \
         Call addCustom/setCustom/getCustom for custom names."
    ))
}

/// The line music21's own constructor writes under `software`: its
/// version, which its own `software` and `all` examples read back. The
/// crate's `Metadata::new` starts empty on purpose -- a library reads no
/// version of anybody else's -- so the facade adds it, and where there is
/// no music21 to ask, this package's own version stands there instead.
fn software_line(py: Python<'_>) -> String {
    match py
        .import("music21")
        .and_then(|module| module.getattr("__version__"))
        .and_then(|version| Ok(version.str()?.to_string()))
    {
        Ok(version) => format!("music21 v{version}"),
        Err(_) => format!("music21_rs v{}", env!("CARGO_PKG_VERSION")),
    }
}

/// A crate error as music21's `MetadataException`.
fn metadata_error(error: music21_rs_crate::Error) -> PyErr {
    crate::pitch::specific_error(&error)
        .unwrap_or_else(|| MetadataException::new_err(crate::pitch::message(&error)))
}

impl Metadata {
    /// Writes one name, taking one value or several.
    fn write(&mut self, name: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let Some(unique) = unique_name_of(name) else {
            return Err(PyAttributeError::new_err(format!(
                "invalid attributeName: {name}"
            )));
        };
        let values = given_values(unique, value)?;
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
