//! music21's `features.base`: a table of what each piece measures, written
//! for machine learning.
//!
//! The reading is the crate's `features`, whose extractors
//! `features_parity` holds to music21 vector for vector. What the facade
//! swaps is the half the crate carries: the **data set** -- its columns,
//! what each column is, and the numbers in it -- and the five ways music21
//! finds an extractor by its id or by its name.
//!
//! **`FeatureExtractor`, `Feature`, `StreamForms` and `DataInstance` stay
//! music21's**, and that is the line rather than an omission.
//! `FeatureExtractor` is the base class every one of music21's extractors
//! was built from before any of this runs -- a class already made keeps the
//! base it was made from, which `rebind` cannot change -- so standing in for
//! it would reach none of them, nor any of its own examples, every one of
//! which is demonstrated on a subclass. `Feature` is a bag music21's own
//! extractors fill attribute by attribute and whose vector a caller indexes
//! and assigns into, with one line of arithmetic in it; `StreamForms` is
//! music21's cache of a stream read a dozen ways, keyed by the string each
//! extractor asks for, and `DataInstance` is that cache with a class value
//! and a path to parse. A facade for any of them would carry almost nothing
//! of the crate.
//!
//! So an extractor class is read here as a **name**: an id or a name picks
//! the crate's reading, and the class music21 holds under it is what
//! `extractorsById` hands back -- which is what its own examples print, and
//! what a caller then calls with a stream of its own.
//!
//! **Parsing stays music21's.** A piece added by a corpus name or a path is
//! given to music21's own `DataInstance`, which decides between the corpus
//! and the converter and opens the file; the facade reads the score it
//! parsed (`stream::crate_stream`) and asks the crate. A set's rows are
//! those `DataInstance` objects, so the id and the class value of each are
//! music21's -- callables and all -- and music21's own output formats, which
//! `getString` and `write` build with the set itself, read every label and
//! every number out of this one.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple, PyType};

use music21_rs_crate::features::{
    DataInstance as RsDataInstance, DataSet as RsDataSet, Extractor as RsExtractor, Feature,
    Library, OutputFormat, Value, all_features_as_list, extractors_by_id, index_of,
};
use music21_rs_crate::metadata::Metadata;
use music21_rs_crate::stream::Stream as RsStream;

use crate::Walkable;

/// The names this module hands out.
pub const NAMES: &[&str] = &[
    "DataSet",
    "allFeaturesAsList",
    "extractorsById",
    "extractorById",
    "vectorById",
    "getIndex",
];

pyo3::create_exception!(music21_rs_facade, FeatureException, crate::Music21Exception);
pyo3::create_exception!(music21_rs_facade, DataSetException, crate::Music21Exception);

error_into!(pub(crate) feature_error, FeatureException);

/// music21's class of that name: the one installed over it where something
/// stands in for it, and music21's own otherwise.
///
/// **A member reaching for one of these needs music21 beside it**, and says
/// so rather than failing on the import. `Feature`, `DataInstance` and the
/// extractor classes are music21's by design here -- the first two because a
/// facade for either would carry almost nothing of the crate, the last
/// because what `extractorsById` hands back is the class a caller then calls
/// -- so a wheel standing on its own has none of them to answer with.
fn music21_class<'py>(py: Python<'py>, module: &str, name: &str) -> PyResult<Bound<'py, PyAny>> {
    if let Some(installed) = crate::installed_class(py, module, name) {
        return Ok(installed);
    }
    py.import(module)
        .and_then(|module| module.getattr(name))
        .map_err(|_| needed(module, name))
}

/// What a member says for a class of music21's that it cannot reach.
fn needed(module: &str, name: &str) -> PyErr {
    FeatureException::new_err(format!(
        "{module}.{name} is music21's own, which this package does not stand in for, so this member needs music21 beside it"
    ))
}

/// Says what music21's own `process` says of an extractor that failed:
/// quietly where the set is quiet, as a warning otherwise.
fn said(py: Python<'_>, quiet: bool, message: &str) -> PyResult<()> {
    if let Ok(environment) = py.import("music21.environment") {
        let said = environment
            .getattr("Environment")?
            .call1(("features.base",))?;
        if quiet {
            said.call_method1("printDebug", (message,))?;
        } else {
            said.call_method1("warn", (message,))?;
        }
    }
    Ok(())
}

/// The module a library's extractor classes live in.
fn library_module(library: Library) -> &'static str {
    match library {
        Library::JSymbolic => "music21.features.jSymbolic",
        Library::Native => "music21.features.native",
    }
}

/// The class music21 holds for the extractor of this id, in this library.
///
/// The crate's table is generated from these very classes, so a miss is a
/// table out of step with the music21 beside it rather than anything a
/// caller did.
fn extractor_class<'py>(
    py: Python<'py>,
    library: Library,
    id: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let module = py
        .import(library_module(library))
        .map_err(|_| needed(library_module(library), "featureExtractors"))?;
    for found in module.getattr("featureExtractors")?.walk()? {
        let found = found?;
        let its: String = found.getattr("id")?.extract()?;
        if its.eq_ignore_ascii_case(id) {
            return Ok(found);
        }
    }
    Err(FeatureException::new_err(format!(
        "{} has no feature extractor with the id {id}",
        library_module(library)
    )))
}

/// The crate's reading for an extractor class of music21's, by its id --
/// or for a bare id, which is how a wheel with no music21 beside it names
/// one, there being no class of music21's to hand over.
///
/// An extractor written outside this package is refused rather than read as
/// something else: what it measures is its own `_process`, which lives in
/// Python and which the crate cannot run.
fn crate_extractor(fe: &Bound<'_, PyAny>) -> PyResult<&'static RsExtractor> {
    let id: String = match fe.extract::<String>() {
        Ok(id) => id,
        Err(_) => fe.getattr("id")?.extract()?,
    };
    extractors_by_id(&[id.as_str()], &Library::ALL)
        .into_iter()
        .next()
        .ok_or_else(|| {
            FeatureException::new_err(format!(
                "this package has no feature extractor with the id {id}: \
                 an extractor of your own cannot be read here"
            ))
        })
}

/// The libraries music21's `library` argument names, in its own order and
/// with its own repeats: a bare string is one name, `all` is both, and a
/// library named twice is looked through twice.
fn libraries_named(value: Option<&Bound<'_, PyAny>>) -> PyResult<Vec<Library>> {
    let names: Vec<String> = match value.filter(|value| !value.is_none()) {
        None => vec!["jSymbolic".to_string(), "native".to_string()],
        Some(value) => match value.extract::<String>() {
            Ok(one) => vec![one],
            Err(_) => {
                let mut names = Vec::new();
                for name in value.walk()? {
                    names.push(name?.extract::<String>()?);
                }
                names
            }
        },
    };
    let mut libraries = Vec::new();
    for name in names {
        let name = name.to_lowercase();
        if name == "jsymbolic" || name == "all" {
            libraries.push(Library::JSymbolic);
        }
        if name == "native" || name == "all" {
            libraries.push(Library::Native);
        }
    }
    Ok(libraries)
}

/// The ids music21's `idOrList` argument names: a bare string is one id.
fn ids_named(value: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    if let Ok(one) = value.extract::<String>() {
        return Ok(vec![one]);
    }
    let mut ids = Vec::new();
    for id in value.walk()? {
        ids.push(id?.extract::<String>()?);
    }
    Ok(ids)
}

/// A value as music21 writes it in a vector: a whole number as an `int`.
fn value_object<'py>(py: Python<'py>, value: Value) -> PyResult<Bound<'py, PyAny>> {
    match value {
        Value::Integer(whole) => Ok(whole.into_pyobject(py)?.into_any()),
        Value::Float(number) => Ok(number.into_pyobject(py)?.into_any()),
    }
}

/// A feature's values as music21's `Feature.vector` is: a plain list.
fn vector_object<'py>(py: Python<'py>, feature: &Feature) -> PyResult<Bound<'py, PyList>> {
    let values = PyList::empty(py);
    for value in feature.values() {
        values.append(value_object(py, *value)?)?;
    }
    Ok(values)
}

/// music21's own `Feature` for one the crate read, filled as its
/// `fillFeatureAttributes` fills one.
fn feature_object<'py>(
    py: Python<'py>,
    extractor: &RsExtractor,
    feature: &Feature,
) -> PyResult<Bound<'py, PyAny>> {
    let made = music21_class(py, "music21.features.base", "Feature")?.call0()?;
    made.setattr("name", extractor.name())?;
    made.setattr("description", extractor.description())?;
    made.setattr("dimensions", extractor.dimensions())?;
    made.setattr("discrete", extractor.discrete())?;
    made.setattr("vector", vector_object(py, feature)?)?;
    Ok(made)
}

/// Whether the object is a stream at all.
fn is_stream(source: &Bound<'_, PyAny>) -> bool {
    source
        .getattr("isStream")
        .and_then(|flag| flag.is_truthy())
        .unwrap_or(false)
}

/// Whether a stream is one of music21's own, which is what its
/// `DataInstance` takes: anything else it reads as a path to parse.
fn music21_stream(source: &Bound<'_, PyAny>) -> bool {
    let py = source.py();
    py.import("music21.stream")
        .and_then(|module| module.getattr("Stream"))
        .and_then(|class| source.is_instance(&class))
        .unwrap_or(false)
}

/// A piece as music21 holds one: its own `DataInstance`, which is what
/// decides between the corpus and the converter for a name, and what
/// answers the id and the class value afterwards.
///
/// Nothing where the piece is a stream music21 does not hold -- one of this
/// package's own, or any stream at all where there is no music21 to ask --
/// and the set then keeps the piece itself. A *name* or a path has nothing
/// else to read it, so that is refused rather than answered.
fn data_instance<'py>(
    source: &Bound<'py, PyAny>,
    id: Option<&Bound<'py, PyAny>>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py = source.py();
    let Ok(class) = music21_class(py, "music21.features.base", "DataInstance") else {
        if is_stream(source) {
            return Ok(None);
        }
        return Err(needed("music21.features.base", "DataInstance"));
    };
    if source.is_instance(&class)? {
        return Ok(Some(source.clone()));
    }
    if is_stream(source) && !music21_stream(source) {
        return Ok(None);
    }
    let keywords = PyDict::new(py);
    if let Some(id) = id.filter(|id| !id.is_none()) {
        keywords.set_item("id", id)?;
    }
    Ok(Some(class.call((source,), Some(&keywords))?))
}

/// The score a `DataInstance` holds, parsed if it has not been yet, read
/// into the crate.
fn crate_stream_held(data: &Bound<'_, PyAny>) -> PyResult<RsStream> {
    data.call_method0("parseStream")?;
    let stream = data.getattr("stream")?;
    if stream.is_none() {
        return Err(DataSetException::new_err(
            "a data instance with no stream cannot be read",
        ));
    }
    crate::stream::crate_stream(&stream)
}

/// The crate's reading of whatever music21 takes as a piece: a stream, one
/// of music21's own `DataInstance` objects, or a name or path it parses.
fn crate_stream_of(source: &Bound<'_, PyAny>) -> PyResult<RsStream> {
    if is_stream(source) {
        return crate::stream::crate_stream(source);
    }
    match data_instance(source, None)? {
        Some(instance) => crate_stream_held(&instance),
        None => Err(needed("music21.features.base", "DataInstance")),
    }
}

/// A piece prepared as music21's `DataInstance` prepares one.
fn prepared(source: &Bound<'_, PyAny>) -> PyResult<RsDataInstance> {
    let stream = crate_stream_of(source)?;
    RsDataInstance::new(&stream).map_err(feature_error)
}

/// music21's `features.allFeaturesAsList`: every feature every extractor
/// reads of one piece, each as a list of its own.
#[pyfunction]
pub fn allFeaturesAsList(streamInput: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let py = streamInput.py();
    let stream = crate_stream_of(streamInput)?;
    let found = all_features_as_list(&stream).map_err(feature_error)?;
    let rows = PyList::empty(py);
    for values in found {
        let row = PyList::empty(py);
        for value in values {
            row.append(value_object(py, value)?)?;
        }
        rows.append(row)?;
    }
    Ok(rows.into_any().unbind())
}

/// music21's `features.extractorsById`: the extractor classes with any of
/// these ids, read without regard to case, hyphens or spaces, with `all`
/// finding every one.
#[pyfunction]
#[pyo3(signature = (idOrList, library = None))]
pub fn extractorsById(
    idOrList: &Bound<'_, PyAny>,
    library: Option<&Bound<'_, PyAny>>,
) -> PyResult<Py<PyAny>> {
    let py = idOrList.py();
    let ids = ids_named(idOrList)?;
    let wanted: Vec<&str> = ids.iter().map(String::as_str).collect();
    let found = PyList::empty(py);
    // One library at a time, because the crate answers the rows and the
    // classes are music21's: both libraries have a `P22`, so which library
    // a row came from is what says which class it names.
    for library in libraries_named(library)? {
        for extractor in extractors_by_id(&wanted, &[library]) {
            found.append(extractor_class(py, library, extractor.id())?)?;
        }
    }
    Ok(found.into_any().unbind())
}

/// music21's `features.extractorById`: the first extractor class any of
/// these ids names, or nothing.
#[pyfunction]
#[pyo3(signature = (idOrList, library = None))]
pub fn extractorById(
    idOrList: &Bound<'_, PyAny>,
    library: Option<&Bound<'_, PyAny>>,
) -> PyResult<Py<PyAny>> {
    let py = idOrList.py();
    let found = extractorsById(idOrList, library)?;
    let found = found.bind(py);
    if found.len()? == 0 {
        return Ok(py.None());
    }
    Ok(found.get_item(0)?.unbind())
}

/// music21's `features.vectorById`: the vector an extractor reads of a
/// piece, or nothing where no extractor has that id.
#[pyfunction]
#[pyo3(signature = (streamObj, vectorId, library = None))]
pub fn vectorById(
    streamObj: &Bound<'_, PyAny>,
    vectorId: &Bound<'_, PyAny>,
    library: Option<&Bound<'_, PyAny>>,
) -> PyResult<Py<PyAny>> {
    let py = streamObj.py();
    let ids = ids_named(vectorId)?;
    let wanted: Vec<&str> = ids.iter().map(String::as_str).collect();
    for library in libraries_named(library)? {
        if let Some(extractor) = extractors_by_id(&wanted, &[library]).into_iter().next() {
            let data = prepared(streamObj)?;
            let feature = extractor.extract(&data).map_err(feature_error)?;
            return Ok(vector_object(py, &feature)?.into_any().unbind());
        }
    }
    Ok(py.None())
}

/// music21's `features.getIndex`: where in its library the extractor of
/// that name stands, and which library that is.
#[pyfunction]
#[pyo3(signature = (featureString, extractorType = None))]
pub fn getIndex(
    py: Python<'_>,
    featureString: &str,
    extractorType: Option<&str>,
) -> PyResult<Py<PyAny>> {
    let library = match extractorType {
        None => None,
        Some("jsymbolic") => Some(Library::JSymbolic),
        Some("native") => Some(Library::Native),
        // music21 looks in neither library for a kind it does not know.
        Some(_) => return Ok(py.None()),
    };
    match index_of(featureString, library) {
        None => Ok(py.None()),
        Some((at, library)) => Ok(PyTuple::new(
            py,
            [
                at.into_pyobject(py)?.into_any(),
                library.name().into_pyobject(py)?.into_any(),
            ],
        )?
        .into_any()
        .unbind()),
    }
}

/// music21's `features.DataSet`: the features of many pieces in one table,
/// each row labelled with its piece's class.
#[pyclass(
    name = "DataSet",
    module = "music21.features.base",
    subclass,
    skip_from_py_object,
    dict
)]
pub struct DataSet {
    /// What the class column is called, nothing for a set with no class.
    class_label: Option<String>,
    /// The extractor classes as given, which music21 keeps.
    classes: Vec<Py<PyAny>>,
    /// The crate's reading for each of them, in the same order.
    extractors: Vec<&'static RsExtractor>,
    /// The pieces added, in order.
    pieces: Vec<Piece>,
    /// What was read: a row per piece and a feature per extractor.
    rows: Vec<Vec<Feature>>,
}

/// One piece of a data set, as music21's `DataInstance` holds one where
/// music21 is there to hold it.
struct Piece {
    /// music21's own `DataInstance`, which answers the stream, the id and
    /// the class value where there is one.
    instance: Option<Py<PyAny>>,
    /// The piece as given, where music21 holds no instance for it.
    source: Py<PyAny>,
    /// The id as given, which may be a function of the stream.
    id: Py<PyAny>,
    /// The class value as given, likewise.
    class_value: Py<PyAny>,
    /// The id and the class value once read, which `process` reads.
    read: Option<(Py<PyAny>, Py<PyAny>)>,
}

impl Piece {
    /// The piece read into the crate.
    fn stream(&self, py: Python<'_>) -> PyResult<RsStream> {
        match &self.instance {
            Some(instance) => crate_stream_held(instance.bind(py)),
            None => crate::stream::crate_stream(self.source.bind(py)),
        }
    }

    /// The name the piece goes by: music21's `getId` where music21 holds
    /// it, and otherwise the id given -- called with the stream where it is
    /// a function, as music21 calls one -- or the piece's own title, with
    /// spaces made underscores either way.
    fn named<'py>(&self, py: Python<'py>, stream: &RsStream) -> PyResult<Bound<'py, PyAny>> {
        if let Some(instance) = &self.instance {
            return instance.bind(py).call_method0("getId");
        }
        let given = self.id.bind(py);
        let found = if given.is_callable() {
            given.call1((self.source.bind(py),))?
        } else if !given.is_none() {
            given.clone()
        } else {
            stream
                .metadata()
                .and_then(Metadata::title)
                .unwrap_or_default()
                .into_pyobject(py)?
                .into_any()
        };
        let Ok(text) = found.extract::<String>() else {
            return Err(pyo3::exceptions::PyAttributeError::new_err(
                found.str()?.to_string(),
            ));
        };
        Ok(text.replace(' ', "_").into_pyobject(py)?.into_any())
    }

    /// The class the piece belongs to: music21's `getClassValue`, nothing
    /// at all written as the empty string.
    fn classed<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        if let Some(instance) = &self.instance {
            return instance.bind(py).call_method0("getClassValue");
        }
        let given = self.class_value.bind(py);
        if given.is_none() {
            return Ok(String::new().into_pyobject(py)?.into_any());
        }
        if given.is_callable() {
            return given.call1((self.source.bind(py),));
        }
        Ok(given.clone())
    }
}

#[pymethods]
impl DataSet {
    /// music21's `DataSet()`.
    #[new]
    #[pyo3(signature = (classLabel = None, featureExtractors = None))]
    fn new(classLabel: Option<String>, featureExtractors: Option<Py<PyAny>>) -> Self {
        let _ = featureExtractors;
        Self {
            class_label: classLabel,
            classes: Vec::new(),
            extractors: Vec::new(),
            pieces: Vec::new(),
            rows: Vec::new(),
        }
    }

    /// The extractors given are read at once, as music21's `__init__`
    /// reads them -- and through the object, so a subclass's
    /// `addFeatureExtractors` is the one that runs.
    #[pyo3(signature = (classLabel = None, featureExtractors = None))]
    fn __init__(
        slf: &Bound<'_, Self>,
        classLabel: Option<String>,
        featureExtractors: Option<Py<PyAny>>,
    ) -> PyResult<()> {
        let py = slf.py();
        slf.borrow_mut().class_label = classLabel;
        // music21 keeps these three as plain attributes a caller writes.
        slf.setattr("failFast", false)?;
        slf.setattr("quiet", true)?;
        slf.setattr("runParallel", true)?;
        if let Some(given) = featureExtractors.filter(|given| !given.bind(py).is_none()) {
            slf.call_method1("addFeatureExtractors", (given,))?;
        }
        Ok(())
    }

    /// The pieces added, as music21's `dataInstances` holds them: its own
    /// `DataInstance` for each, where music21 is there to hold one.
    #[getter]
    fn dataInstances(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let found = PyList::empty(py);
        for piece in &self.pieces {
            if let Some(instance) = &piece.instance {
                found.append(instance.bind(py))?;
            }
        }
        Ok(found.into_any().unbind())
    }

    /// What was read, as music21's `features` holds it: a row of its own
    /// `Feature` objects per piece.
    #[getter]
    fn features(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let rows = PyList::empty(py);
        for features in &self.rows {
            let row = PyList::empty(py);
            for (extractor, feature) in self.extractors.iter().zip(features) {
                row.append(feature_object(py, extractor, feature)?)?;
            }
            rows.append(row)?;
        }
        Ok(rows.into_any().unbind())
    }

    /// music21's `getClassLabel`.
    fn getClassLabel(&self) -> Option<String> {
        self.class_label.clone()
    }

    /// music21's `addFeatureExtractors`: one extractor class or a
    /// collection of them. An extractor's bare id does as well, which is
    /// how a wheel with no music21 beside it names one.
    fn addFeatureExtractors(&mut self, values: &Bound<'_, PyAny>) -> PyResult<()> {
        let given: Vec<Bound<'_, PyAny>> = if values.is_instance_of::<PyType>()
            || values.is_instance_of::<pyo3::types::PyString>()
        {
            vec![values.clone()]
        } else {
            let mut found = Vec::new();
            for value in values.walk()? {
                found.push(value?);
            }
            found
        };
        for value in given {
            self.extractors.push(crate_extractor(&value)?);
            self.classes.push(value.unbind());
        }
        Ok(())
    }

    /// music21's `getAttributeLabels`: the name of every column.
    #[pyo3(signature = (includeClassLabel = true, includeId = true))]
    fn getAttributeLabels(&self, includeClassLabel: bool, includeId: bool) -> Vec<String> {
        self.set()
            .attribute_labels(includeClassLabel && self.class_label.is_some(), includeId)
    }

    /// music21's `getDiscreteLabels`: whether each column is discrete.
    #[pyo3(signature = (includeClassLabel = true, includeId = true))]
    fn getDiscreteLabels(&self, includeClassLabel: bool, includeId: bool) -> Vec<Option<bool>> {
        self.set()
            .discrete_labels(includeClassLabel && self.class_label.is_some(), includeId)
    }

    /// music21's `getClassPositionLabels`: whether each column is the
    /// class's.
    #[pyo3(signature = (includeId = true, includeClassLabel = true))]
    fn getClassPositionLabels(
        &self,
        includeId: bool,
        includeClassLabel: bool,
    ) -> Vec<Option<bool>> {
        self.set()
            .class_position_labels(includeId, includeClassLabel && self.class_label.is_some())
    }

    /// music21's `addData`: a stream, one of music21's `DataInstance`
    /// objects, or a name or path music21 parses.
    #[pyo3(signature = (dataOrStreamOrPath, classValue = None, id = None))]
    fn addData(
        &mut self,
        dataOrStreamOrPath: &Bound<'_, PyAny>,
        classValue: Option<&Bound<'_, PyAny>>,
        id: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let py = dataOrStreamOrPath.py();
        let Some(label) = self.class_label.clone() else {
            return Err(DataSetException::new_err(
                "cannot add data unless a class label for this DataSet has been set.",
            ));
        };
        let value = classValue.map_or_else(|| py.None(), |value| value.clone().unbind());
        let instance = data_instance(dataOrStreamOrPath, id)?;
        if let Some(instance) = &instance {
            instance.call_method1("setClassLabel", (label, value.clone_ref(py)))?;
        }
        self.pieces.push(Piece {
            instance: instance.map(Bound::unbind),
            source: dataOrStreamOrPath.clone().unbind(),
            id: id.map_or_else(|| py.None(), |id| id.clone().unbind()),
            class_value: value,
            read: None,
        });
        Ok(())
    }

    /// music21's `addMultipleData`: several pieces with their class values
    /// and their ids, each sequence as long as the list of pieces.
    #[pyo3(signature = (dataList, classValues, ids = None))]
    fn addMultipleData<'py>(
        slf: &Bound<'py, Self>,
        dataList: &Bound<'py, PyAny>,
        classValues: &Bound<'py, PyAny>,
        ids: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<()> {
        let py = slf.py();
        let mut pieces = Vec::new();
        for piece in dataList.walk()? {
            pieces.push(piece?);
        }
        let spread = |values: &Bound<'py, PyAny>, what: &str| -> PyResult<Vec<Bound<'py, PyAny>>> {
            if values.is_callable() {
                // music21 runs these in worker processes of its own, so a
                // function no pickle can carry is refused here rather than
                // failing there.
                py.import("pickle")?
                    .call_method1("dumps", (values,))
                    .map_err(|_| {
                        DataSetException::new_err(format!(
                            "{what} if a function must be pickleable. \
                             Lambda and some other functions are not."
                        ))
                    })?;
                return Ok(vec![values.clone(); pieces.len()]);
            }
            let mut found = Vec::new();
            for value in values.walk()? {
                found.push(value?);
            }
            if found.len() != pieces.len() {
                return Err(DataSetException::new_err(format!(
                    "If {what} is not a function, it must have the same length as dataList"
                )));
            }
            Ok(found)
        };
        let values = spread(classValues, "classValues")?;
        let names = match ids.filter(|ids| !ids.is_none()) {
            None => vec![py.None().into_bound(py); pieces.len()],
            Some(ids) => spread(ids, "ids")?,
        };
        for ((piece, value), name) in pieces.iter().zip(values).zip(names) {
            slf.call_method1("addData", (piece, value, name))?;
        }
        Ok(())
    }

    /// music21's `process`: every feature of every piece. An extractor that
    /// cannot read its feature leaves a blank one in its place and says so,
    /// unless `failFast` is set.
    fn process(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let extractors = slf.borrow().extractors.clone();
        let fail_fast = slf
            .getattr("failFast")
            .and_then(|flag| flag.is_truthy())
            .unwrap_or(false);
        let quiet = slf
            .getattr("quiet")
            .and_then(|flag| flag.is_truthy())
            .unwrap_or(true);
        let mut rows = Vec::new();
        let mut read = Vec::new();
        for at in 0..slf.borrow().pieces.len() {
            // Read out from under the borrow: every step of this calls back
            // into Python, which may reach the set itself.
            let stream = {
                let me = slf.borrow();
                me.pieces[at].stream(py)?
            };
            let (named, classed) = {
                let me = slf.borrow();
                (
                    me.pieces[at].named(py, &stream)?.unbind(),
                    me.pieces[at].classed(py)?.unbind(),
                )
            };
            read.push((named, classed));
            let data = RsDataInstance::new(&stream).map_err(feature_error)?;
            let mut row = Vec::with_capacity(extractors.len());
            for extractor in &extractors {
                match extractor.extract(&data) {
                    Ok(feature) => row.push(feature),
                    Err(error) => {
                        if fail_fast {
                            return Err(feature_error(error));
                        }
                        said(
                            py,
                            quiet,
                            &format!("failed feature extractor: {} {error}", extractor.id()),
                        )?;
                        row.push(extractor.blank());
                    }
                }
            }
            rows.push(row);
        }
        let mut me = slf.borrow_mut();
        for (piece, found) in me.pieces.iter_mut().zip(read) {
            piece.read = Some(found);
        }
        me.rows = rows;
        Ok(())
    }

    /// music21's `getFeaturesAsList`: a row per piece, its id first and its
    /// class value last.
    #[pyo3(signature = (includeClassLabel = true, includeId = true, concatenateLists = true))]
    fn getFeaturesAsList(
        &self,
        py: Python<'_>,
        includeClassLabel: bool,
        includeId: bool,
        concatenateLists: bool,
    ) -> PyResult<Py<PyAny>> {
        let table = PyList::empty(py);
        for (piece, features) in self.pieces.iter().zip(&self.rows) {
            let row = PyList::empty(py);
            let (named, classed) = match &piece.read {
                Some((named, classed)) => (named.bind(py), classed.bind(py)),
                None => continue,
            };
            if includeId {
                row.append(named)?;
            }
            for feature in features {
                let values = vector_object(py, feature)?;
                if concatenateLists {
                    for value in values.iter() {
                        row.append(value)?;
                    }
                } else {
                    row.append(values)?;
                }
            }
            if includeClassLabel {
                row.append(classed)?;
            }
            table.append(row)?;
        }
        Ok(table.into_any().unbind())
    }

    /// music21's `getUniqueClassValues`: each class value once, in the
    /// order first met.
    fn getUniqueClassValues(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let found = PyList::empty(py);
        for piece in &self.pieces {
            let value = piece.classed(py)?;
            if !found.contains(&value)? {
                found.append(value)?;
            }
        }
        Ok(found.into_any().unbind())
    }

    /// music21's `_getOutputFormat`: its own writer for a format's name.
    fn _getOutputFormat(slf: &Bound<'_, Self>, featureFormat: &str) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let Some(format) = OutputFormat::from_name(featureFormat) else {
            return Ok(py.None());
        };
        let name = match format {
            OutputFormat::Tab => "OutputTabOrange",
            OutputFormat::Csv => "OutputCSV",
            OutputFormat::Arff => "OutputARFF",
        };
        let keywords = PyDict::new(py);
        keywords.set_item("dataSet", slf)?;
        Ok(music21_class(py, "music21.features.outputFormats", name)?
            .call((), Some(&keywords))?
            .unbind())
    }

    /// music21's `_getOutputFormatFromFilePath`: the writer a file's
    /// extension names, or nothing.
    fn _getOutputFormatFromFilePath(
        slf: &Bound<'_, Self>,
        fp: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let path: String = fp.str()?.extract()?;
        let Some((_, extension)) = path.rsplit_once('.') else {
            return Ok(py.None());
        };
        Self::_getOutputFormat(slf, extension)
    }

    /// music21's `getString`: the table written in a format.
    #[pyo3(signature = (outputFmt = "tab"))]
    fn getString(slf: &Bound<'_, Self>, outputFmt: &str) -> PyResult<Py<PyAny>> {
        let writer = slf.call_method1("_getOutputFormat", (outputFmt,))?;
        if writer.is_none() {
            return Err(DataSetException::new_err(format!(
                "no output format could be defined from {outputFmt}"
            )));
        }
        Ok(writer.call_method0("getString")?.unbind())
    }

    /// music21's `write`: the table written to a file by music21's own
    /// writer, which reads every label and every number out of this set.
    #[pyo3(signature = (fp = None, format = None, includeClassLabel = true))]
    fn write(
        slf: &Bound<'_, Self>,
        fp: Option<&Bound<'_, PyAny>>,
        format: Option<&str>,
        includeClassLabel: bool,
    ) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let writer = match (format, fp) {
            (Some(format), _) => slf.call_method1("_getOutputFormat", (format,))?,
            (None, Some(fp)) => slf.call_method1("_getOutputFormatFromFilePath", (fp,))?,
            (None, None) => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "Specify a format or a file path",
                ));
            }
        };
        if writer.is_none() {
            let named = match fp {
                None => "None".to_string(),
                Some(fp) => fp.str()?.to_string(),
            };
            return Err(DataSetException::new_err(format!(
                "no output format could be defined from file path {named} or format {}",
                format.unwrap_or("None")
            )));
        }
        let keywords = PyDict::new(py);
        keywords.set_item("fp", fp.map_or_else(|| py.None(), |fp| fp.clone().unbind()))?;
        keywords.set_item("includeClassLabel", includeClassLabel)?;
        Ok(writer.call_method("write", (), Some(&keywords))?.unbind())
    }
}

impl DataSet {
    /// The crate's set for what has been asked of this one, which is what
    /// answers every question about the columns.
    fn set(&self) -> RsDataSet {
        let mut set = RsDataSet::new(self.class_label.clone().unwrap_or_default());
        set.add_extractors(self.extractors.iter().copied());
        set
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<DataSet>()?;
    m.add_function(wrap_pyfunction!(allFeaturesAsList, m)?)?;
    m.add_function(wrap_pyfunction!(extractorsById, m)?)?;
    m.add_function(wrap_pyfunction!(extractorById, m)?)?;
    m.add_function(wrap_pyfunction!(vectorById, m)?)?;
    m.add_function(wrap_pyfunction!(getIndex, m)?)?;
    Ok(())
}
