//! music21's `corpus`, found on disk and read with the wheel's `converter`.
//!
//! The scores are music21's own, in the layout its `corpus` directory has
//! (`bach/bwv66.6.mxl`, `essenFolksong/han1.abc`, ...); the wheel ships none.
//! They are looked for in
//!
//! 1. the directory the `MUSIC21_RS_CORPUS` environment variable names, where
//!    it is set;
//! 2. the `corpus` directory of an installed music21, found without importing
//!    it;
//!
//! and where neither is there, asking for a work raises `CorpusException`.
//!
//! `getWork` finds a work by part of its path, as music21's does, and `parse`
//! reads it. There is no cache of parsed scores and no metadata bundle to
//! search: every parse reads the file.
//!
//! Once `install_into_music21` has run, these are music21's own `getWork` and
//! `parse`, with music21's caches and its local corpora.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyList, PyString, PyTuple};

use crate::converter::{self, LazyException};

/// The module's dotted name.
const MODULE: &str = "music21_rs.corpus";

/// The variable naming a directory laid out as music21's corpus.
const CORPUS_VARIABLE: &str = "MUSIC21_RS_CORPUS";

/// The module's public names.
const ALL: [&str; 9] = [
    "CorpusException",
    "getComposer",
    "getCorePaths",
    "getCorpusFilePath",
    "getPaths",
    "getWork",
    "noCorpus",
    "parse",
    "search",
];

/// The extensions of the formats music21's corpus holds, in its order.
const EXTENSIONS: [&str; 17] = [
    ".abc",
    ".capx",
    ".mid",
    ".midi",
    ".xml",
    ".mxl",
    ".musicxml",
    ".md",
    ".musedata",
    ".zip",
    ".krn",
    ".rntxt",
    ".rntext",
    ".romantext",
    ".rtxt",
    ".nwctxt",
    ".nwc",
];

pub(crate) static CORPUS_EXCEPTION: LazyException =
    LazyException::new("CorpusException", "music21.exceptions21");
static CORPUS_EXCEPTIONS: [&LazyException; 1] = [&CORPUS_EXCEPTION];

/// The files under each corpus directory, by directory and extensions.
type Listings = HashMap<(String, Vec<String>), Listing>;

/// One corpus directory's files in some formats: the paths, and the list of
/// `pathlib.Path`s `getPaths` hands out.
struct Listing {
    paths: Vec<PathBuf>,
    listed: Py<PyList>,
}

static LISTINGS: Mutex<Option<Listings>> = Mutex::new(None);

/// The module, built once.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        m.setattr(
            "__doc__",
            "music21's `corpus`, found on disk and read with the wheel's `converter`.\n\n\
             The scores are music21's own, read from the directory \
             `MUSIC21_RS_CORPUS` names, else from an installed music21's `corpus` \
             directory. Once `install_into_music21` has run, these are music21's \
             own `getWork` and `parse`.",
        )?;
        m.add("__all__", ALL.to_vec())?;
        m.add("CORPUS_VARIABLE", CORPUS_VARIABLE)?;
        m.add_function(wrap_pyfunction!(corpus_file_path, &m)?)?;
        let paths = wrap_pyfunction!(get_paths, &m)?;
        m.add("getCorePaths", &paths)?;
        m.add_function(paths)?;
        m.add_function(wrap_pyfunction!(get_work, &m)?)?;
        m.add_function(wrap_pyfunction!(parse, &m)?)?;
        m.add_function(wrap_pyfunction!(crate::corpora::search, &m)?)?;
        m.add_function(wrap_pyfunction!(get_composer, &m)?)?;
        m.add_function(wrap_pyfunction!(no_corpus, &m)?)?;
        m.add(
            "__getattr__",
            converter::exception_getattr(py, MODULE, &CORPUS_EXCEPTIONS)?,
        )?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

/// The directory the corpus is read from: the one `MUSIC21_RS_CORPUS`
/// names, else an installed music21's `corpus`. Raises `CorpusException`
/// where there is neither.
#[pyfunction]
#[pyo3(name = "getCorpusFilePath")]
pub(crate) fn corpus_file_path(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let named = py
        .import("os")?
        .getattr("environ")?
        .call_method1("get", (CORPUS_VARIABLE,))?;
    if named.is_truthy()? {
        let path = py
            .import("pathlib")?
            .getattr("Path")?
            .call1((named,))?
            .call_method0("expanduser")?;
        if !path.call_method0("is_dir")?.is_truthy()? {
            return Err(CORPUS_EXCEPTION.err(
                py,
                format!("{CORPUS_VARIABLE} names no directory: {}", path.str()?),
            ));
        }
        return Ok(path);
    }
    match music21_corpus(py)? {
        Some(found) => Ok(found),
        None => Err(CORPUS_EXCEPTION.err(
            py,
            format!(
                "no corpus to read: install music21, whose corpus this reads, or set \
                 {CORPUS_VARIABLE} to a directory laid out as music21's corpus"
            ),
        )),
    }
}

/// The corpus directory as a path, or nothing where there is none.
pub(crate) fn corpus_directory(py: Python<'_>) -> Option<PathBuf> {
    let found = corpus_file_path(py).ok()?;
    Some(PathBuf::from(found.str().ok()?.to_string()))
}

/// music21's `corpus` directory, found by where the package is rather than
/// by importing it.
fn music21_corpus(py: Python<'_>) -> PyResult<Option<Bound<'_, PyAny>>> {
    let spec = py
        .import("importlib.util")?
        .call_method1("find_spec", ("music21",));
    let spec = match spec {
        Ok(spec) => spec,
        Err(error)
            if error.is_instance_of::<pyo3::exceptions::PyImportError>(py)
                || error.is_instance_of::<pyo3::exceptions::PyValueError>(py) =>
        {
            return Ok(None);
        }
        Err(error) => Err(error)?,
    };
    if spec.is_none() {
        return Ok(None);
    }
    let locations = spec.getattr("submodule_search_locations")?;
    if locations.is_none() {
        return Ok(None);
    }
    let path_class = py.import("pathlib")?.getattr("Path")?;
    for location in locations.try_iter()? {
        let corpus = path_class.call1((location?,))?.div("corpus")?;
        if corpus.call_method0("is_dir")?.is_truthy()? {
            return Ok(Some(corpus));
        }
    }
    Ok(None)
}

/// Every extension of the formats `fileExtensions` names, as music21's
/// `Corpus.translateExtensions` widens them; all of them for none.
pub(crate) fn extensions(file_extensions: Option<&Bound<'_, PyAny>>) -> PyResult<Vec<String>> {
    let Some(given) = file_extensions.filter(|given| given.is_truthy().unwrap_or(true)) else {
        return Ok(EXTENSIONS.iter().map(|each| (*each).to_owned()).collect());
    };
    let py = given.py();
    let given = if given.is_instance_of::<PyString>() {
        PyList::new(py, [given])?.into_any()
    } else {
        given.clone()
    };
    let mut widened = Vec::new();
    for extension in given.try_iter()? {
        widened.extend(converter::extensions_of(&extension?)?);
    }
    Ok(widened)
}

/// The paths of every work in the corpus, or of those in the formats
/// `fileExtensions` names, sorted, as music21's `corpus.getPaths` lists
/// them.
#[pyfunction]
#[pyo3(name = "getPaths", signature = (fileExtensions=None))]
#[allow(non_snake_case)]
fn get_paths<'py>(
    py: Python<'py>,
    fileExtensions: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyList>> {
    let (_, listed) = listing(py, fileExtensions)?;
    Ok(listed)
}

/// The corpus's files in the formats asked for, listed once for each
/// directory and set of extensions.
pub(crate) fn listing<'py>(
    py: Python<'py>,
    file_extensions: Option<&Bound<'py, PyAny>>,
) -> PyResult<(Vec<PathBuf>, Bound<'py, PyList>)> {
    let root = corpus_file_path(py)?;
    let root_text = root.str()?.to_string();
    let key = (root_text.clone(), extensions(file_extensions)?);
    {
        let held = LISTINGS.lock().expect("corpus listings");
        if let Some(found) = held.as_ref().and_then(|listings| listings.get(&key)) {
            return Ok((found.paths.clone(), found.listed.bind(py).clone()));
        }
    }

    // Every file and directory under the root whose name is a work's.
    let mut paths = Vec::new();
    walk(Path::new(&root_text), &mut paths);
    paths.retain(|path| is_work(path, &key.1));
    paths.sort();
    let path_class = py.import("pathlib")?.getattr("Path")?;
    let made = paths
        .iter()
        .map(|path| path_class.call1((path.as_os_str(),)))
        .collect::<PyResult<Vec<_>>>()?;
    let listed = PyList::new(py, made)?;
    let mut held = LISTINGS.lock().expect("corpus listings");
    held.get_or_insert_with(HashMap::new).insert(
        key,
        Listing {
            paths: paths.clone(),
            listed: listed.clone().unbind(),
        },
    );
    Ok((paths, listed))
}

/// Everything under `directory`, as `Path.rglob("*")` finds it: a link to a
/// directory is listed and not followed, and what cannot be read is passed
/// over.
pub(crate) fn walk(directory: &Path, into: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_directory = entry.file_type().is_ok_and(|kind| kind.is_dir());
        into.push(path.clone());
        if is_directory {
            walk(&path, into);
        }
    }
}

/// Whether a path is a work's: not hidden, not private, and in one of the
/// formats asked for.
pub(crate) fn is_work(path: &Path, extensions: &[String]) -> bool {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    if name.starts_with("__") || name.starts_with('.') {
        return false;
    }
    let suffix = suffix_of(&name);
    extensions
        .iter()
        .any(|extension| suffix.ends_with(extension.as_str()))
}

/// A file name's suffix, as `pathlib` reads it: from its last dot, where
/// that is neither its first character nor its last.
fn suffix_of(name: &str) -> &str {
    match name.rfind('.') {
        Some(index) if index > 0 && index < name.len() - 1 => &name[index..],
        _ => "",
    }
}

/// A file name without its suffix.
fn stem_of(name: &str) -> &str {
    &name[..name.len() - suffix_of(name).len()]
}

/// A path as `as_posix` writes it.
fn posix(path: &Path) -> String {
    let text = path.to_string_lossy();
    if cfg!(windows) {
        return text.replace('\\', "/");
    }
    text.into_owned()
}

/// The paths matching `workName`, as music21's `Corpus.getWorkList` matches
/// them: a path holding the name, those with a `/` before it where there
/// are any, then those of the movement asked for.
pub(crate) fn work_list(
    work_name: &Bound<'_, PyAny>,
    movement: Option<&Bound<'_, PyAny>>,
    file_extensions: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<PathBuf>> {
    let (paths, _) = listing(work_name.py(), file_extensions)?;
    work_list_in(paths, work_name, movement)
}

/// [`work_list`] among `paths`, which a local corpus lists for itself.
pub(crate) fn work_list_in(
    paths: Vec<PathBuf>,
    work_name: &Bound<'_, PyAny>,
    movement: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<PathBuf>> {
    let py = work_name.py();
    let named = work_name.str()?.to_string();
    let work_name = if named.starts_with("schumann/") {
        PyString::new(py, &named.replace("schumann/", "schumann_robert/")).into_any()
    } else {
        work_name.clone()
    };
    let wanted: String = py
        .import("pathlib")?
        .getattr("PurePath")?
        .call1((work_name,))?
        .call_method0("as_posix")?
        .call_method0("lower")?
        .extract()?;

    // Every path holding the name, then only those with a `/` before it.
    let lowered = |path: &PathBuf| posix(path).to_lowercase();
    let mut results: Vec<PathBuf> = paths
        .into_iter()
        .filter(|path| lowered(path).contains(&wanted))
        .collect();
    let slashed = format!("/{wanted}");
    let stricter: Vec<PathBuf> = results
        .iter()
        .filter(|path| lowered(path).contains(&slashed))
        .cloned()
        .collect();
    if !stricter.is_empty() {
        results = stricter;
    }
    let chosen = match movement {
        Some(movement) if !results.is_empty() => movements(results, movement)?,
        _ => results,
    };
    let unique: BTreeSet<PathBuf> = chosen.into_iter().collect();
    Ok(unique.into_iter().collect())
}

/// The names a movement's file can have.
fn movement_names(movement: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    if movement.is_instance_of::<PyList>() || movement.is_instance_of::<PyTuple>() {
        let numbers = movement
            .try_iter()?
            .map(|number| Ok(number?.str()?.to_string()))
            .collect::<PyResult<Vec<String>>>()?;
        return Ok(vec![
            numbers.concat(),
            numbers.join("-"),
            format!("movement{}", numbers.join("-")),
            format!("movement{}", numbers.join("-0")),
        ]);
    }
    let number = movement.str()?.to_string();
    Ok(vec![
        format!("0{number}"),
        number.clone(),
        format!("movement{number}"),
    ])
}

/// The paths of the movement asked for: those named for it, else those
/// whose names start with it.
fn movements(results: Vec<PathBuf>, movement: &Bound<'_, PyAny>) -> PyResult<Vec<PathBuf>> {
    let names = movement_names(movement)?;
    let mut sorted = results;
    sorted.sort();
    let mut found = Vec::new();
    for path in sorted {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut partial = true;
        if !suffix_of(&name).is_empty() {
            for each in &names {
                if stem_of(&name).to_lowercase() == each.to_lowercase() {
                    found.push(path.clone());
                    partial = false;
                }
            }
        }
        if !found.is_empty() {
            continue;
        }
        if partial {
            for each in &names {
                if name.starts_with(&each.to_lowercase()) {
                    found.push(path.clone());
                }
            }
        }
    }
    Ok(found)
}

/// Whether there is no corpus to read: music21's `noCorpus`, which asks
/// the core corpus.
#[pyfunction]
#[pyo3(name = "noCorpus")]
fn no_corpus(py: Python<'_>) -> bool {
    corpus_directory(py).is_none()
}

/// The paths of the core corpus's works that belong to a composer or a
/// collection, sorted.
#[pyfunction]
#[pyo3(name = "getComposer", signature = (composerName, *, fileExtensions = None))]
#[allow(non_snake_case)]
fn get_composer<'py>(
    py: Python<'py>,
    composerName: &str,
    fileExtensions: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyList>> {
    let core = crate::corpora::corpora_module(py)?
        .getattr("CoreCorpus")?
        .call0()?;
    crate::corpuswork::composer_paths(&core, composerName, fileExtensions)
}

/// The path of the work whose path holds `workName`, or a list of the
/// paths where several do, as music21's `corpus.getWork` finds it. A name
/// ending `.xml` finds the `.mxl` of that name too.
#[pyfunction]
#[pyo3(name = "getWork", signature = (workName, movementNumber=None, *, fileExtensions=None))]
#[allow(non_snake_case)]
fn get_work<'py>(
    py: Python<'py>,
    workName: &Bound<'py, PyAny>,
    movementNumber: Option<&Bound<'py, PyAny>>,
    fileExtensions: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    if converter::installed(py)? {
        let kwargs = PyDict::new(py);
        kwargs.set_item("fileExtensions", given_extensions(py, fileExtensions))?;
        return py.import("music21.corpus")?.call_method(
            "getWork",
            (workName, movementNumber),
            Some(&kwargs),
        );
    }
    if !workName.is_truthy()? {
        return Err(
            CORPUS_EXCEPTION.err(py, "a work name must be provided as an argument".to_owned())
        );
    }

    // The work by its name, or the `.mxl` of a `.xml` asked for.
    let name = workName.str()?.to_string();
    let mut found = work_list(workName, movementNumber, fileExtensions)?;
    let compressed = name.ends_with(".xml") || name.ends_with(".musicxml");
    if found.is_empty() && compressed {
        let stem: String = py
            .import("os.path")?
            .call_method1("splitext", (&name,))?
            .get_item(0)?
            .extract()?;
        let zipped = PyString::new(py, &format!("{stem}.mxl")).into_any();
        found = work_list(&zipped, movementNumber, fileExtensions)?;
    }
    if found.is_empty() {
        let kind = if compressed {
            "n xml, musicxml, or mxl"
        } else {
            ""
        };
        return Err(CORPUS_EXCEPTION.err(
            py,
            format!(
                "Could not find a{kind} work that met this criterion: {}; if you are \
                 searching for a file on disk, use \"converter\" instead of \"corpus\".",
                workName.str()?
            ),
        ));
    }
    let path_class = py.import("pathlib")?.getattr("Path")?;
    let made = found
        .iter()
        .map(|path| path_class.call1((path.as_os_str(),)))
        .collect::<PyResult<Vec<_>>>()?;
    if made.len() == 1 {
        return Ok(made.into_iter().next().expect("one path"));
    }
    Ok(PyList::new(py, made)?.into_any())
}

/// `fileExtensions` as given, `()` where it was not.
fn given_extensions<'py>(py: Python<'py>, given: Option<&Bound<'py, PyAny>>) -> Bound<'py, PyAny> {
    match given {
        Some(given) => given.clone(),
        None => PyTuple::empty(py).into_any(),
    }
}

/// The work `getWork` finds, read by the wheel's `converter.parse`, its
/// metadata's `corpusFilePath` saying where in the corpus it is, as
/// music21's `corpus.parse` reads it.
///
/// Where `getWork` finds several, the first is read.
#[pyfunction]
#[pyo3(signature = (
    workName,
    movementNumber=None,
    *,
    number=None,
    fileExtensions=None,
    forceSource=None,
    format=None
))]
#[allow(non_snake_case)]
pub(crate) fn parse<'py>(
    py: Python<'py>,
    workName: &Bound<'py, PyAny>,
    movementNumber: Option<&Bound<'py, PyAny>>,
    number: Option<&Bound<'py, PyAny>>,
    fileExtensions: Option<&Bound<'py, PyAny>>,
    forceSource: Option<&Bound<'py, PyAny>>,
    format: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    if converter::installed(py)? {
        let kwargs = PyDict::new(py);
        kwargs.set_item("number", number)?;
        kwargs.set_item("fileExtensions", given_extensions(py, fileExtensions))?;
        let force = match forceSource {
            Some(force) => force.clone(),
            None => pyo3::types::PyBool::new(py, false).to_owned().into_any(),
        };
        kwargs.set_item("forceSource", force)?;
        kwargs.set_item("format", format)?;
        return py.import("music21.corpus")?.call_method(
            "parse",
            (workName, movementNumber),
            Some(&kwargs),
        );
    }
    let found = get_work(py, workName, movementNumber, fileExtensions)?;
    let path = if found.is_instance_of::<PyList>() {
        found.get_item(0)?
    } else {
        found
    };
    let read = converter::parse_path(&path, number, format)?;
    say_where(&read, &path)?;
    Ok(read)
}

/// Writes where in the corpus a work is on its metadata.
fn say_where(read: &Bound<'_, PyAny>, path: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = read.py();
    if read.getattr("metadata")?.is_none() {
        let metadata = crate::provide::extension(py)?
            .getattr("Metadata")?
            .call0()?;
        read.setattr("metadata", metadata)?;
    }
    let root = corpus_file_path(py)?;
    let place = match path.call_method1("relative_to", (root,)) {
        Ok(relative) => relative.call_method0("as_posix")?,
        Err(error) if error.is_instance_of::<pyo3::exceptions::PyValueError>(py) => {
            path.str()?.into_any()
        }
        Err(error) => return Err(error),
    };
    read.getattr("metadata")?.setattr("corpusFilePath", place)
}

#[cfg(test)]
mod tests {
    use super::{stem_of, suffix_of};

    #[test]
    fn a_suffix_and_a_stem_are_pathlib_s() {
        assert_eq!(suffix_of("bwv66.6.mxl"), ".mxl");
        assert_eq!(stem_of("bwv66.6.mxl"), "bwv66.6");
        assert_eq!(suffix_of(".hidden"), "");
        assert_eq!(stem_of("movement2"), "movement2");
    }
}
