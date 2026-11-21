//! music21's `freezeThaw`: a stream written out as a pickle or as
//! jsonpickle's JSON, and read back.
//!
//! The package's streams and elements pickle themselves, each writing its
//! Rust half as text beside its Python half. What this adds is music21's
//! scaffold around that: before writing, each stream's elements are moved
//! out into `_storedElementOffsetTuples`, `(element, offset or 'end')`, and
//! the stream status lets go of its stream; after reading, both are put
//! back. Sites need no clearing here, since a site counts only while its
//! stream holds the element.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBytes, PyDict, PyList, PyString, PyTuple};

use music21::common::stringtools::md5_hex;

/// The module music21 keeps these in.
const MODULE: &str = "music21.freezeThaw";

/// Where the scaffold keeps a stream's elements while it is written out.
const STORED: &str = "_storedElementOffsetTuples";

/// The offset the scaffold writes for an element kept at the end.
const AT_END: &str = "end";

/// The two formats, as `parseWriteFmt` names them.
const PICKLE: &str = "pickle";
const JSONPICKLE: &str = "jsonpickle";

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

pyo3::create_exception!(
    music21_rs_facade,
    FreezeThawException,
    crate::Music21Exception
);

/// Whether an object says it is a stream.
fn is_stream(object: &Bound<'_, PyAny>) -> bool {
    object
        .getattr("isStream")
        .and_then(|flag| flag.is_truthy())
        .unwrap_or(false)
}

/// Whether music21 names this class among an object's `classes`.
fn is_a(object: &Bound<'_, PyAny>, class: &str) -> bool {
    object
        .getattr("classes")
        .and_then(|classes| classes.contains(class))
        .unwrap_or(false)
}

/// Every element under a stream, at every depth, streams included.
fn walk<'py>(stream: &Bound<'py, PyAny>, found: &mut Vec<Bound<'py, PyAny>>) -> PyResult<()> {
    for element in stream.try_iter()? {
        let element = element?;
        found.push(element.clone());
        if is_stream(&element) {
            walk(&element, found)?;
        }
    }
    Ok(())
}

/// The stream given, or else the one this freezer or thawer holds.
fn given_or_held<'py>(
    slf: &Bound<'py, PyAny>,
    given: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let stream = match given.filter(|stream| !stream.is_none()) {
        Some(stream) => stream,
        None => slf.getattr("stream")?,
    };
    if stream.is_none() {
        return Err(FreezeThawException::new_err(
            "You need to pass in a stream when creating to work",
        ));
    }
    Ok(stream)
}

/// music21's `environLocal` for this module.
fn environment(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    crate::environment::module(py)?
        .getattr("Environment")?
        .call1(("freezeThaw",))
}

/// music21's `StreamFreezeThawBase`: what freezer and thawer share.
#[pyclass(
    name = "StreamFreezeThawBase",
    module = "music21.freezeThaw",
    subclass,
    dict
)]
pub struct StreamFreezeThawBase {
    /// The stream frozen or thawed.
    #[pyo3(get, set)]
    stream: Option<Py<PyAny>>,
}

#[pymethods]
impl StreamFreezeThawBase {
    /// What it holds, shown to Python's collector.
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.stream)
    }

    fn __clear__(&mut self) {
        self.stream = None;
    }
    #[new]
    #[pyo3(signature = (*_arguments, **_keywords))]
    fn new(_arguments: &Bound<'_, PyTuple>, _keywords: Option<&Bound<'_, PyDict>>) -> Self {
        Self { stream: None }
    }

    /// A new file name in `directory` for a pickle: `m21-`, the MD5 of the
    /// time, and `.p`.
    fn getPickleFp<'py>(
        &self,
        py: Python<'py>,
        directory: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let now = py.import("time")?.call_method0("time")?.str()?;
        let name = format!("m21-{}.p", md5_hex(now.to_str()?.as_bytes()));
        let directory = py.import("pathlib")?.getattr("Path")?.call1((directory,))?;
        directory.div(name)
    }

    /// The same, for JSON: `.p.json`.
    fn getJsonFp<'py>(
        &self,
        py: Python<'py>,
        directory: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.getPickleFp(py, directory)?
            .call_method1("with_suffix", (".p.json",))
    }
}

/// music21's `StreamFreezer`: a stream made ready to write out, and
/// written. A copy is frozen unless `fastButUnsafe` is given, since
/// freezing empties the streams it walks.
#[pyclass(name = "StreamFreezer", module = "music21.freezeThaw", extends = StreamFreezeThawBase, subclass)]
pub struct StreamFreezer {
    /// Whether this freezer stands at the top of the hierarchy.
    #[pyo3(get, set)]
    topLevel: bool,
    /// Python's `id` of every stream in the hierarchy, once found.
    #[pyo3(get, set)]
    streamIds: Option<Py<PyAny>>,
    /// music21 keeps freezers of spanners' storage here; this needs none.
    #[pyo3(get, set)]
    subStreamFreezers: Py<PyAny>,
}

#[pymethods]
impl StreamFreezer {
    /// What it holds, shown to Python's collector.
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.streamIds)?;
        visit.call(&self.subStreamFreezers)
    }

    fn __clear__(&mut self) {
        self.streamIds = None;
    }
    #[new]
    #[pyo3(signature = (streamObj = None, fastButUnsafe = false, topLevel = true, streamIds = None))]
    fn new(
        py: Python<'_>,
        streamObj: Option<&Bound<'_, PyAny>>,
        #[pyo3(from_py_with = crate::truthy)] fastButUnsafe: bool,
        #[pyo3(from_py_with = crate::truthy)] topLevel: bool,
        streamIds: Option<Py<PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        // Freezing empties the streams it walks, so a copy is frozen.
        let stream = match streamObj.filter(|stream| !stream.is_none()) {
            Some(stream) if fastButUnsafe => Some(stream.clone().unbind()),
            Some(stream) => Some(
                py.import("copy")?
                    .call_method1("deepcopy", (stream,))?
                    .unbind(),
            ),
            None => None,
        };
        let freezer = Self {
            topLevel,
            streamIds,
            subStreamFreezers: PyDict::new(py).into_any().unbind(),
        };
        Ok(PyClassInitializer::from(StreamFreezeThawBase { stream }).add_subclass(freezer))
    }

    /// music21's `packStream`: the stream scaffolded, in the dictionary
    /// that is written out, with the version that wrote it.
    #[pyo3(signature = (streamObj = None))]
    fn packStream<'py>(
        slf: &Bound<'py, Self>,
        streamObj: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyDict>> {
        let py = slf.py();
        let stream = given_or_held(slf.as_any(), streamObj)?;
        Self::setupSerializationScaffold(slf, Some(stream.clone()))?;
        let storage = PyDict::new(py);
        storage.set_item("stream", stream)?;
        storage.set_item("m21Version", crate::defaults::VERSION_TEXT)?;
        Ok(storage)
    }

    /// music21's `setupSerializationScaffold`: every stream in the
    /// hierarchy lets go of its stream status, and has its elements moved
    /// into `_storedElementOffsetTuples`.
    #[pyo3(signature = (streamObj = None))]
    fn setupSerializationScaffold(
        slf: &Bound<'_, Self>,
        streamObj: Option<Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let stream = given_or_held(slf.as_any(), streamObj)?;
        let top = slf.borrow().topLevel;
        if top {
            Self::findActiveStreamIdsInHierarchy(slf, Some(stream.clone()), true, true)?;
        }

        // What music21 does for a variant's and a spanner's storage is done
        // here by their own pickling, which carries what they hold.
        let mut found = Vec::new();
        walk(&stream, &mut found)?;
        for element in found.iter().filter(|element| is_stream(element)) {
            Self::removeStreamStatusClient(element)?;
        }
        Self::removeStreamStatusClient(&stream)?;
        Self::setupStoredElementOffsetTuples(&stream)?;
        if top {
            Self::recursiveClearSites(&stream)?;
        }
        Ok(())
    }

    /// music21's `removeStreamStatusClient`: the stream status lets go of
    /// the stream it reports on.
    #[staticmethod]
    fn removeStreamStatusClient(streamObj: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Ok(status) = streamObj.getattr("streamStatus") {
            status.setattr("client", streamObj.py().None())?;
        }
        Ok(())
    }

    /// music21's `recursiveClearSites`: no element moved out of a stream
    /// stays that stream's `activeSite`. Sites need no clearing, being
    /// counted only while a stream holds the element.
    #[staticmethod]
    fn recursiveClearSites(startObj: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = startObj.py();
        let Ok(stored) = startObj.getattr(STORED) else {
            return Ok(());
        };
        for pair in stored.try_iter()? {
            let element = pair?.get_item(0)?;
            if is_stream(&element) {
                Self::recursiveClearSites(&element)?;
            }
            if is_a(&element, "Spanner") {
                Self::recursiveClearSites(&element.getattr("spannerStorage")?)?;
            }
            if is_a(&element, "Variant") {
                Self::recursiveClearSites(&element.getattr("_stream")?)?;
            }
            element.setattr("activeSite", py.None())?;
        }
        startObj.setattr("activeSite", py.None())
    }

    /// music21's `setupStoredElementOffsetTuples`: the stream's elements,
    /// and those of the streams it holds, moved out into a list of
    /// `(element, offset)`, `'end'` for what is kept at the end. A stream
    /// that already has the list is left as it is.
    #[staticmethod]
    fn setupStoredElementOffsetTuples(streamObj: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = streamObj.py();
        if streamObj.hasattr(STORED)? {
            return Ok(());
        }

        let stored = PyList::empty(py);
        for element in streamObj.getattr("_elements")?.try_iter()? {
            let element = element?;
            let offset = streamObj.call_method1("elementOffset", (&element,))?;
            stored.append((&element, offset))?;
            if is_stream(&element) {
                Self::setupStoredElementOffsetTuples(&element)?;
            }
        }
        for element in streamObj.getattr("_endElements")?.try_iter()? {
            stored.append((element?, AT_END))?;
        }

        // Then out of the stream, which stops being their site.
        for pair in stored.iter() {
            streamObj.call_method1("remove", (pair.get_item(0)?,))?;
        }
        streamObj.setattr(STORED, stored)
    }

    /// music21's `findActiveStreamIdsInHierarchy`: Python's `id` of every
    /// stream in the hierarchy, the storage of its spanners and the
    /// streams of its variants included, kept as `streamIds` too.
    #[pyo3(signature = (hierarchyObject = None, getSpanners = true, getVariants = true))]
    fn findActiveStreamIdsInHierarchy<'py>(
        slf: &Bound<'py, Self>,
        hierarchyObject: Option<Bound<'py, PyAny>>,
        #[pyo3(from_py_with = crate::truthy)] getSpanners: bool,
        #[pyo3(from_py_with = crate::truthy)] getVariants: bool,
    ) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let stream = given_or_held(slf.as_any(), hierarchyObject)?;
        let mut found = Vec::new();
        walk(&stream, &mut found)?;

        let mut ids: Vec<usize> = vec![stream.as_ptr() as usize];
        ids.extend(
            found
                .iter()
                .filter(|element| is_stream(element))
                .map(|element| element.as_ptr() as usize),
        );
        if getSpanners {
            let storage: Vec<usize> = stream
                .getattr("spannerBundle")?
                .call_method0("getSpannerStorageIds")?
                .extract()?;
            ids.extend(storage);
        }
        if getVariants {
            for variant in found.iter().filter(|element| is_a(element, "Variant")) {
                let inner = Self::findActiveStreamIdsInHierarchy(
                    slf,
                    Some(variant.getattr("_stream")?),
                    true,
                    true,
                )?;
                ids.extend(inner.extract::<Vec<usize>>()?);
            }
        }
        ids.sort_unstable();
        ids.dedup();

        let ids = PyList::new(py, ids)?;
        slf.borrow_mut().streamIds = Some(ids.clone().into_any().unbind());
        Ok(ids)
    }

    /// music21's `parseWriteFmt`: `'jsonpickle'` for `'json'` or
    /// `'jsonpickle'` in any case, and `'pickle'` for anything else.
    #[pyo3(signature = (fmt = None))]
    fn parseWriteFmt(&self, fmt: Option<&str>) -> &'static str {
        write_format(fmt)
    }

    /// music21's `write`: the stream written to `fp`, or to a new file in
    /// music21's temporary directory, and the path written. `zipType`
    /// `'zlib'` compresses it.
    #[pyo3(signature = (fmt = Some(PICKLE), fp = None, zipType = None, **keywords))]
    fn write<'py>(
        slf: &Bound<'py, Self>,
        fmt: Option<&str>,
        fp: Option<Bound<'py, PyAny>>,
        zipType: Option<&str>,
        keywords: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let zipped = zip_kind(zipType)?;
        let fmt = write_format(fmt);
        let root = environment(py)?.call_method0("getRootTempDir")?;

        // The path: a new one, or the one given, relative to the
        // temporary directory unless absolute.
        let path = match fp.filter(|fp| !fp.is_none()) {
            None if fmt == JSONPICKLE => slf.as_super().borrow().getJsonFp(py, &root)?,
            None => slf.as_super().borrow().getPickleFp(py, &root)?,
            Some(given) if given.is_instance_of::<PyString>() => relative_to(
                &root,
                &py.import("pathlib")?.getattr("Path")?.call1((given,))?,
            )?,
            Some(given) => relative_to(&root, &given)?,
        };

        let held = slf.getattr("stream")?;
        let storage = Self::packStream(slf, Some(held))?;
        let mut written = encode(py, &storage, fmt, keywords, Protocol::Default)?;
        if zipped == Zip::Zlib {
            written = zlib(py, &written, "compress")?;
        }
        if path.hasattr("write")? {
            path.call_method1("write", (&written,))?;
            return Ok(path);
        }
        let mode = if written.is_instance_of::<PyBytes>() {
            "wb"
        } else {
            "w"
        };
        let file = py
            .import("builtins")?
            .getattr("open")?
            .call1((&path, mode))?;
        file.call_method1("write", (&written,))?;
        file.call_method0("close")?;
        Ok(path)
    }

    /// music21's `writeStr`: the stream written out as bytes for a
    /// pickle, as text for jsonpickle.
    #[pyo3(signature = (fmt = None, **keywords))]
    fn writeStr<'py>(
        slf: &Bound<'py, Self>,
        fmt: Option<&str>,
        keywords: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let fmt = write_format(fmt);
        let held = slf.getattr("stream")?;
        let storage = Self::packStream(slf, Some(held))?;
        encode(py, &storage, fmt, keywords, Protocol::Highest)
    }
}

/// music21's `StreamThawer`: a stream read back from what a freezer wrote.
#[pyclass(name = "StreamThawer", module = "music21.freezeThaw", extends = StreamFreezeThawBase, subclass)]
pub struct StreamThawer;

#[pymethods]
impl StreamThawer {
    #[new]
    fn new() -> PyClassInitializer<Self> {
        PyClassInitializer::from(StreamFreezeThawBase { stream: None }).add_subclass(Self)
    }

    /// music21's `teardownSerializationScaffold`: every stream in the
    /// hierarchy given its elements and its stream status back, variants'
    /// and spanners' storage too.
    #[pyo3(signature = (streamObj = None))]
    fn teardownSerializationScaffold(
        slf: &Bound<'_, Self>,
        streamObj: Option<Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let stream = given_or_held(slf.as_any(), streamObj)?;
        Self::restoreElementsFromTuples(&stream)?;
        Self::restoreStreamStatusClient(&stream)?;

        let mut found = Vec::new();
        walk(&stream, &mut found)?;
        for element in found {
            if is_a(&element, "Variant") {
                Self::teardownSerializationScaffold(slf, Some(element.getattr("_stream")?))?;
            } else if is_a(&element, "Spanner") {
                Self::teardownSerializationScaffold(slf, Some(element.getattr("spannerStorage")?))?;
            } else if is_stream(&element) {
                Self::restoreStreamStatusClient(&element)?;
            }
        }
        Ok(())
    }

    /// music21's `restoreElementsFromTuples`: what
    /// `_storedElementOffsetTuples` lists put back into the stream, each
    /// at its offset or at the end, and the same for the streams it holds.
    #[staticmethod]
    fn restoreElementsFromTuples(streamObj: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Ok(stored) = streamObj.getattr(STORED) {
            for pair in stored.try_iter()? {
                let pair = pair?;
                let (element, offset) = (pair.get_item(0)?, pair.get_item(1)?);
                if offset.extract::<String>().is_ok_and(|said| said == AT_END) {
                    streamObj.call_method1("storeAtEnd", (element,))?;
                } else {
                    streamObj.call_method1("insert", (offset, element))?;
                }
            }
            streamObj.delattr(STORED)?;
        }

        // A stream a spanner holds may have been restored already.
        for element in streamObj.try_iter()? {
            let element = element?;
            if is_stream(&element) {
                Self::restoreElementsFromTuples(&element)?;
            }
        }
        Ok(())
    }

    /// music21's `restoreStreamStatusClient`: the stream status reports on
    /// its stream again.
    #[staticmethod]
    fn restoreStreamStatusClient(streamObj: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Ok(status) = streamObj.getattr("streamStatus") {
            status.setattr("client", streamObj)?;
        }
        Ok(())
    }

    /// music21's `unpackStream`: the stream out of what was read, its
    /// scaffold taken down. A different version having written it is
    /// warned of.
    fn unpackStream<'py>(
        slf: &Bound<'py, Self>,
        storage: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let version = storage.get_item("m21Version")?;
        if !version.eq(crate::defaults::VERSION_TEXT)? {
            environment(py)?.call_method1(
                "warn",
                ("this pickled file is out of date and may not function properly.",),
            )?;
        }
        let stream = storage.get_item("stream")?;
        Self::teardownSerializationScaffold(slf, Some(stream.clone()))?;
        Ok(stream)
    }

    /// music21's `parseOpenFmt`: `'jsonpickle'` for what starts `{"`, and
    /// `'pickle'` for anything else.
    fn parseOpenFmt(&self, storage: &Bound<'_, PyAny>) -> PyResult<&'static str> {
        let start = if let Ok(bytes) = storage.cast::<PyBytes>() {
            bytes.as_bytes().starts_with(b"{\"")
        } else {
            storage.extract::<String>()?.starts_with("{\"")
        };
        Ok(if start { JSONPICKLE } else { PICKLE })
    }

    /// music21's `open`: the stream read from a file a freezer wrote, a
    /// path relative to music21's temporary directory if not found as
    /// given, into `stream`.
    #[pyo3(signature = (fp, zipType = None))]
    fn open(slf: &Bound<'_, Self>, fp: &Bound<'_, PyAny>, zipType: Option<&str>) -> PyResult<()> {
        let py = slf.py();
        let exists = py
            .import("os")?
            .getattr("path")?
            .call_method1("exists", (fp,))?
            .is_truthy()?;
        let path = if exists {
            fp.clone()
        } else {
            environment(py)?.call_method0("getRootTempDir")?.div(fp)?
        };
        let file = py
            .import("builtins")?
            .getattr("open")?
            .call1((&path, "rb"))?;
        let mut data = file.call_method0("read")?;
        file.call_method0("close")?;

        let format = slf.borrow().parseOpenFmt(&data)?;
        if format == PICKLE && zip_kind(zipType)? == Zip::Zlib {
            data = zlib(py, &data, "decompress")?;
        }
        if format == JSONPICKLE {
            data = data.call_method1("decode", ("utf-8",))?;
        }
        let storage = decode(py, &data, format)?;
        let stream = Self::unpackStream(slf, &storage)?;
        slf.setattr("stream", stream)
    }

    /// music21's `openStr`: the stream read from what `writeStr` wrote,
    /// in the format given or the one it starts with, into `stream`.
    #[pyo3(signature = (fileData, pickleFormat = None))]
    fn openStr(
        slf: &Bound<'_, Self>,
        fileData: &Bound<'_, PyAny>,
        pickleFormat: Option<&str>,
    ) -> PyResult<()> {
        let py = slf.py();
        let format = match pickleFormat {
            Some(PICKLE) => PICKLE,
            Some(JSONPICKLE) => JSONPICKLE,
            Some(other) => {
                return Err(FreezeThawException::new_err(format!(
                    "bad StreamFreezer format: {other}"
                )));
            }
            None => slf.borrow().parseOpenFmt(fileData)?,
        };
        let storage = decode(py, fileData, format)?;
        let stream = Self::unpackStream(slf, &storage)?;
        slf.setattr("stream", stream)
    }
}

/// music21's reading of a format name to write in.
fn write_format(fmt: Option<&str>) -> &'static str {
    let Some(fmt) = fmt else {
        return PICKLE;
    };
    match fmt.trim().to_lowercase().as_str() {
        "jsonpickle" | "json" => JSONPICKLE,
        _ => PICKLE,
    }
}

/// How what is written is compressed.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Zip {
    None,
    Zlib,
}

/// The compression music21 names, `zlib` being the only one.
fn zip_kind(zipType: Option<&str>) -> PyResult<Zip> {
    match zipType {
        None => Ok(Zip::None),
        Some("zlib") => Ok(Zip::Zlib),
        Some(other) => Err(FreezeThawException::new_err(format!(
            "Unknown zipType {other}"
        ))),
    }
}

/// Which pickle protocol to write: Python's default, which `write` uses
/// so that files can be shared, or the highest, which `writeStr` uses.
#[derive(Clone, Copy)]
enum Protocol {
    Default,
    Highest,
}

/// The storage written out in a format.
fn encode<'py>(
    py: Python<'py>,
    storage: &Bound<'py, PyDict>,
    fmt: &str,
    keywords: Option<&Bound<'py, PyDict>>,
    protocol: Protocol,
) -> PyResult<Bound<'py, PyAny>> {
    if fmt == JSONPICKLE {
        return py
            .import("jsonpickle")?
            .getattr("encode")?
            .call((storage,), keywords);
    }
    let pickle = py.import("pickle")?;
    match protocol {
        Protocol::Default => pickle.call_method1("dumps", (storage,)),
        Protocol::Highest => pickle.call_method1("dumps", (storage, -1)),
    }
}

/// What was written out read back in a format.
fn decode<'py>(
    py: Python<'py>,
    data: &Bound<'py, PyAny>,
    fmt: &str,
) -> PyResult<Bound<'py, PyAny>> {
    if fmt == JSONPICKLE {
        return py.import("jsonpickle")?.call_method1("decode", (data,));
    }
    py.import("pickle")?.call_method1("loads", (data,))
}

/// Text or bytes compressed or decompressed with zlib.
fn zlib<'py>(
    py: Python<'py>,
    data: &Bound<'py, PyAny>,
    direction: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let data = match data.cast::<PyString>() {
        Ok(text) => text.call_method0("encode")?,
        Err(_) => data.clone(),
    };
    py.import("zlib")?.call_method1(direction, (data,))
}

/// A path under `root` unless it is absolute; anything not a path, such
/// as an open buffer, as it is.
fn relative_to<'py>(
    root: &Bound<'py, PyAny>,
    path: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    if !path.hasattr("is_absolute")? || path.call_method0("is_absolute")?.is_truthy()? {
        return Ok(path.clone());
    }
    root.div(path)
}

/// The music21 module `freezeThaw`, built.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        let exception = py.get_type::<FreezeThawException>();
        exception.setattr("__module__", MODULE)?;
        m.add("FreezeThawException", exception)?;
        m.add_class::<StreamFreezeThawBase>()?;
        m.add_class::<StreamFreezer>()?;
        m.add_class::<StreamThawer>()?;
        m.add("environLocal", environment(py)?)?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}
