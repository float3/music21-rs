//! `import music21` answered by this package, where music21 is not installed.
//!
//! A library written for music21 imports music21 by name. With music21
//! installed, `install_into_music21()` puts this package's classes inside it.
//! Without it, `provide_music21()` stands in for it: it adds an import hook
//! that answers `music21` and each of music21's modules this package has
//! classes or functions for with a module built from them, under music21's
//! names. Nothing happens until it is called, or until `music21_rs` is
//! imported with `MUSIC21_RS_PROVIDE_MUSIC21=1` in the environment; the `.pth`
//! file the wheel puts in site-packages imports it then.
//!
//! Only what the package has is answered. A module of music21's it has
//! nothing for raises `ModuleNotFoundError` saying so, rather than being
//! faked, and a name a provided module lacks is an `AttributeError` naming
//! what is there.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, OnceLock};

use pyo3::exceptions::{PyAttributeError, PyException, PyImportError, PyModuleNotFoundError};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyCFunction, PyDict, PyList, PyModule, PyString, PyTuple};

use crate::homes;
use crate::plumbing::built::new_class;

/// The variable that switches the provider on at import.
pub(crate) const ENVIRONMENT_VARIABLE: &str = "MUSIC21_RS_PROVIDE_MUSIC21";

/// The private module this one is answered under.
const MODULE: &str = "music21_rs._provide";

/// The private module the table of homes is answered under.
const HOMES_MODULE: &str = "music21_rs._homes";

/// What a provided module's spec says it was loaded from.
const ORIGIN: &str = "music21-rs";

/// music21's module of exceptions, which the package's are built on.
const EXCEPTIONS21: &str = "music21.exceptions21";

/// A package of music21's also carries the names of these submodules of its
/// own: `music21.stream.Stream` is `music21.stream.base.Stream`. The table
/// of homes says so name by name; `_contents(children=True)` does it the
/// way it was done before there was a table.
const REEXPORTED_CHILDREN: [&str; 2] = ["base", "core"];

/// What music21's `exceptions21` defines, each answered by the package's own
/// class of that name.
const EXCEPTIONS21_NAMES: [&str; 10] = [
    "StreamException",
    "ImmutableStreamException",
    "MetadataException",
    "AnalysisException",
    "TreeException",
    "InstrumentException",
    "SubConverterException",
    "GroupException",
    "MeterException",
    "TimeSignatureException",
];

/// What every module object has of its own, which a built module's names
/// must not put over the provided module's.
const MODULE_SPECIALS: [&str; 5] = [
    "__name__",
    "__doc__",
    "__loader__",
    "__spec__",
    "__package__",
];

/// A module of music21's whose names the package keeps on its package: the
/// stream classes music21 defines in `stream.base` are `music21.stream`'s
/// here, and are answered under both names.
const CHILDREN_OF_PACKAGE: [(&str, &str); 1] = [("music21.stream.base", "music21.stream")];

/// The extension module itself, kept as it registers: the provider can be
/// switched on while the module is still being imported, before
/// `import music21_rs` would find it.
static EXTENSION: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

/// The names the extension registers, its private ones included, as
/// `_EXPORTED` hands them out.
static EXPORTED: OnceLock<Vec<String>> = OnceLock::new();

/// Whether `_contents` places names by the table of homes.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Homes {
    Placed,
    Unplaced,
}

/// Whether `_contents` copies each `base` and `core` module's names into
/// its package.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Children {
    Copied,
    Kept,
}

/// The extension module, as [`register`] kept it.
pub(crate) fn extension(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    match EXTENSION.get(py) {
        Some(module) => Ok(module.bind(py).clone()),
        None => Err(PyImportError::new_err(
            "music21_rs's extension module is not registered",
        )),
    }
}

/// Registers the provider on the wheel's module: `provide_music21`, the
/// private `_provide` and `_homes` modules, and the switch the environment
/// sets. `exported` is every name the extension registered.
pub(crate) fn register(m: &Bound<'_, PyModule>, exported: Vec<String>) -> PyResult<()> {
    let py = m.py();
    let _ = EXTENSION.set(py, m.clone().unbind());
    let _ = EXPORTED.set(exported);
    m.add_function(wrap_pyfunction!(provide_music21, m)?)?;

    // The private modules, importable by their dotted names.
    let modules = py.import("sys")?.getattr("modules")?;
    let provide = provide_module(py)?;
    modules.set_item(MODULE, &provide)?;
    m.add("_provide", provide)?;
    let homes = homes_module(py)?;
    modules.set_item(HOMES_MODULE, &homes)?;
    m.add("_homes", homes)?;
    Ok(())
}

/// The switch for code that cannot call `provide_music21` itself: set, it
/// answers `import music21` from this package from its first import on.
pub(crate) fn provide_if_asked(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    let asked = py
        .import("os")?
        .getattr("environ")?
        .call_method1("get", (ENVIRONMENT_VARIABLE,))?;
    if asked.is_none() || asked.extract::<String>()? != "1" {
        return Ok(());
    }
    provide_music21(py)
}

/// `music21_rs._provide`: the provider's members under the names the
/// package's Python had.
fn provide_module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let m = PyModule::new(py, MODULE)?;
    m.setattr(
        "__doc__",
        "`import music21` answered by this package, where music21 is not installed.",
    )?;
    m.add("ENVIRONMENT_VARIABLE", ENVIRONMENT_VARIABLE)?;
    m.add("Music21Exception", music21_exception(py)?)?;
    m.add("Music21DeprecationWarning", deprecation_warning(py)?)?;
    m.add_function(wrap_pyfunction!(provide_music21, &m)?)?;
    m.add_function(wrap_pyfunction!(providing_py, &m)?)?;
    m.add_function(wrap_pyfunction!(contents_py, &m)?)?;
    m.add_class::<Provider>()?;
    let exported = EXPORTED.get().cloned().unwrap_or_default();
    m.add("_EXPORTED", PyTuple::new(py, exported)?)?;
    Ok(m)
}

/// `music21_rs._homes`: the table of homes as dicts and a tuple.
fn homes_module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let m = PyModule::new(py, HOMES_MODULE)?;
    m.setattr(
        "__doc__",
        "Where music21 keeps each function and constant this package exports.",
    )?;
    m.add("HOMES", table(py, homes::HOMES)?)?;
    m.add("ALSO", table(py, homes::ALSO)?)?;
    m.add("NO_HOME", PyTuple::new(py, homes::NO_HOME)?)?;
    Ok(m)
}

/// A table of homes as a dict of tuples.
fn table<'py>(py: Python<'py>, rows: &[(&str, &[&str])]) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    for (module, names) in rows {
        dict.set_item(*module, PyTuple::new(py, *names)?)?;
    }
    Ok(dict)
}

/// What every exception music21 raises is, as music21 has it.
pub(crate) fn music21_exception(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    static CLASS: PyOnceLock<Py<PyAny>> = PyOnceLock::new();
    let class = CLASS.get_or_try_init(py, || {
        let base = py.get_type::<PyException>().into_any();
        new_class(
            py,
            EXCEPTIONS21,
            "Music21Exception",
            "What every exception music21 raises is, as music21 has it.",
            vec![base],
            vec![],
        )
        .map(Bound::unbind)
    })?;
    Ok(class.bind(py).clone())
}

/// music21's warning for a member on its way out.
fn deprecation_warning(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    static CLASS: PyOnceLock<Py<PyAny>> = PyOnceLock::new();
    let class = CLASS.get_or_try_init(py, || {
        let base = py.get_type::<pyo3::exceptions::PyUserWarning>().into_any();
        new_class(
            py,
            EXCEPTIONS21,
            "Music21DeprecationWarning",
            "music21's warning for a member on its way out.",
            vec![base],
            vec![],
        )
        .map(Bound::unbind)
    })?;
    Ok(class.bind(py).clone())
}

/// Answer `import music21` from this package, music21 not being installed.
///
/// Code written for music21 then runs on these classes unchanged, as far as
/// the names it imports are ones this package has::
///
///     import music21_rs
///     music21_rs.provide_music21()
///
///     from music21 import chord
///     chord.Chord('C4 E4 G4').commonName   # 'major triad'
///
/// It refuses, with `ImportError`, where music21 itself can be imported:
/// `install_into_music21()` is the way to run music21 on these classes
/// then. Calling it again does nothing.
///
/// Call it before anything that imports music21's exceptions, so that the
/// package's exceptions are built on the `Music21Exception` it provides.
#[pyfunction]
pub(crate) fn provide_music21(py: Python<'_>) -> PyResult<()> {
    if providing(py)? {
        return Ok(());
    }
    refuse_real_music21(py)?;

    // In before the rest is worked out: working it out builds the package's
    // exceptions, and building one imports `music21.exceptions21` for its
    // base, which has to be the one answered here.
    let first = PyDict::new(py);
    first.set_item("music21", PyDict::new(py))?;
    first.set_item(EXCEPTIONS21, exceptions21(py)?)?;
    let mut children = BTreeMap::new();
    children.insert(
        "music21".to_owned(),
        BTreeSet::from(["exceptions21".to_owned()]),
    );
    let finder = Bound::new(
        py,
        Provider {
            contents: Mutex::new(first.unbind()),
            children: Mutex::new(children),
        },
    )?;
    py.import("sys")?
        .getattr("meta_path")?
        .call_method1("insert", (0, &finder))?;
    let worked_out = contents(py, Homes::Placed, Children::Kept)?;
    Provider::update(&finder, worked_out)
}

/// Whether `provide_music21()` is answering `import music21`.
pub(crate) fn providing(py: Python<'_>) -> PyResult<bool> {
    let finders = py.import("sys")?.getattr("meta_path")?;
    for finder in finders.try_iter()? {
        if finder?.is_instance_of::<Provider>() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Whether `provide_music21()` is answering `import music21`.
#[pyfunction]
#[pyo3(name = "providing")]
fn providing_py(py: Python<'_>) -> PyResult<bool> {
    providing(py)
}

/// Refuses where music21 itself is imported or can be.
fn refuse_real_music21(py: Python<'_>) -> PyResult<()> {
    let loaded = py
        .import("sys")?
        .getattr("modules")?
        .call_method1("get", ("music21",))?;
    if !loaded.is_none() {
        return Err(PyImportError::new_err(format!(
            "music21 is already imported (from {}), so music21-rs will not stand in \
             for it; music21_rs.install_into_music21() runs music21 on this package's \
             classes instead",
            where_from(&loaded)?
        )));
    }

    // A directory called music21 with no `__init__.py` is a namespace
    // package, which is not music21: a checkout of this project has one.
    let spec = py
        .import("importlib.util")?
        .call_method1("find_spec", ("music21",))?;
    if spec.is_none() {
        return Ok(());
    }
    let origin = spec.getattr("origin")?;
    if origin.is_none() || spec.getattr("loader")?.is_none() {
        return Ok(());
    }
    Err(PyImportError::new_err(format!(
        "music21 is installed ({}), so music21-rs will not stand in for it; \
         music21_rs.install_into_music21() runs music21 on this package's classes \
         instead",
        origin.str()?
    )))
}

/// Where a module was loaded from: its file, or its repr.
fn where_from(module: &Bound<'_, PyAny>) -> PyResult<String> {
    let file = attribute(module, "__file__")?;
    if let Some(file) = file.filter(|file| file.is_truthy().unwrap_or(false)) {
        return Ok(file.str()?.to_string());
    }
    Ok(module.repr()?.to_string())
}

/// `getattr(object, name, None)`: an `AttributeError` is no value, any
/// other error is raised.
pub(crate) fn attribute<'py>(
    object: &Bound<'py, PyAny>,
    name: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    match object.getattr(name) {
        Ok(value) if value.is_none() => Ok(None),
        Ok(value) => Ok(Some(value)),
        Err(error) if error.is_instance_of::<PyAttributeError>(object.py()) => Ok(None),
        Err(error) => Err(error),
    }
}

/// `music21.exceptions21` as provided before the rest is worked out.
fn exceptions21(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let names = PyDict::new(py);
    names.set_item("Music21Exception", music21_exception(py)?)?;
    names.set_item("Music21DeprecationWarning", deprecation_warning(py)?)?;
    Ok(names)
}

/// The names a module answers with, added to `contents` empty where it has
/// none yet, as `dict.setdefault` adds them.
fn names_of<'py>(contents: &Bound<'py, PyDict>, module: &str) -> PyResult<Bound<'py, PyDict>> {
    if let Some(names) = contents.get_item(module)? {
        return Ok(names.cast_into::<PyDict>()?);
    }
    let names = PyDict::new(contents.py());
    contents.set_item(module, &names)?;
    Ok(names)
}

/// `dict.setdefault`, its answer unused.
fn set_default(dict: &Bound<'_, PyDict>, key: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
    if !dict.contains(key)? {
        dict.set_item(key, value)?;
    }
    Ok(())
}

/// Every module this package answers for, mapped to its names.
///
/// A class says which module of music21's it belongs to; a function or a
/// constant does not, and the table of homes says it for them, with the
/// names music21 imports into each module from where they live. `homes`
/// false leaves the table out, which is what the table is worked out
/// against, and `children` copies each `base` and `core` module's names
/// into its package instead, as this did before there was a table.
#[pyfunction]
#[pyo3(name = "_contents", signature = (homes=true, children=false))]
fn contents_py(py: Python<'_>, homes: bool, children: bool) -> PyResult<Bound<'_, PyDict>> {
    let homes = if homes {
        Homes::Placed
    } else {
        Homes::Unplaced
    };
    let children = if children {
        Children::Copied
    } else {
        Children::Kept
    };
    contents(py, homes, children)
}

/// Every module this package answers for; see [`contents_py`].
pub(crate) fn contents(
    py: Python<'_>,
    homes: Homes,
    children: Children,
) -> PyResult<Bound<'_, PyDict>> {
    let ext = extension(py)?;
    let contents = PyDict::new(py);
    contents.set_item("music21", PyDict::new(py))?;
    contents.set_item(EXCEPTIONS21, exceptions21(py)?)?;

    // The classes each module of music21's is installed with.
    for (module, names) in crate::MUSIC21_MODULES {
        for name in names {
            if let Some(value) = attribute(&ext, name)? {
                names_of(&contents, module)?.set_item(name, value)?;
            }
        }
    }
    warn_on_unprovided_base(py, &ext)?;

    // Every public name, by the module it says it belongs to.
    let exported = EXPORTED.get().cloned().unwrap_or_default();
    let exceptions = crate::EXCEPTION_NAMES.iter().map(|name| (*name).to_owned());
    for name in exported.into_iter().chain(exceptions) {
        if name.starts_with('_') {
            continue;
        }
        let value = ext.getattr(name.as_str())?;
        let module = attribute(&value, "__module__")?;
        let Some(module) = module.and_then(|module| module.extract::<String>().ok()) else {
            continue;
        };
        if !module.starts_with("music21.") {
            continue;
        }
        set_default(&names_of(&contents, &module)?, &name, &value)?;
    }

    // Modules built whole in Rust, `music21.common` among them, each on
    // first asking: every name each has, its private ones too, since
    // music21's examples reach for some, and its `__all__`, which a star
    // import of it follows.
    let built_names = PyDict::new(py);
    for module in crate::built_modules() {
        let built = crate::music21_module(py, &module)?;
        let names = names_of(&contents, &module)?;
        for (name, value) in built.dict().iter() {
            let name: String = name.extract()?;
            if !MODULE_SPECIALS.contains(&name.as_str()) {
                names.set_item(&name, &value)?;
            }
            if !name.starts_with("__") {
                set_default(&built_names, &name, &value)?;
            }
        }
    }

    // The converter and the corpus, under music21's names.
    for (module, written) in [
        ("music21.converter", crate::converter::module(py)?),
        ("music21.corpus", crate::corpus::module(py)?),
    ] {
        let names = names_of(&contents, module)?;
        for name in written.getattr("__all__")?.try_iter()? {
            let name: String = name?.extract()?;
            names.set_item(&name, written.getattr(name.as_str())?)?;
        }
    }
    let exceptions21 = names_of(&contents, EXCEPTIONS21)?;
    for name in EXCEPTIONS21_NAMES {
        if let Some(exception) = attribute(&ext, name)? {
            set_default(&exceptions21, name, &exception)?;
        }
    }
    // music21 defines it there and imports it into `corpus`.
    let corpus_exception = names_of(&contents, "music21.corpus")?.get_item("CorpusException")?;
    exceptions21.set_item("CorpusException", corpus_exception)?;

    if homes == Homes::Placed {
        place_homes(&contents, &ext, &built_names)?;
    }
    for (child, parent) in CHILDREN_OF_PACKAGE {
        let Some(names) = contents.get_item(parent)? else {
            continue;
        };
        let into = names_of(&contents, child)?;
        for (name, value) in names.cast::<PyDict>()?.iter() {
            set_default(&into, &name.extract::<String>()?, &value)?;
        }
    }
    add_parents(&contents)?;
    if children == Children::Copied {
        copy_children(&contents)?;
    }
    names_of(&contents, "music21")?.set_item("Music21Exception", music21_exception(py)?)?;
    Ok(contents)
}

/// Each name the table of homes places, in the module it names.
fn place_homes(
    contents: &Bound<'_, PyDict>,
    ext: &Bound<'_, PyModule>,
    built_names: &Bound<'_, PyDict>,
) -> PyResult<()> {
    let py = contents.py();
    let known = built_names.copy()?;
    known.set_item("Music21Exception", music21_exception(py)?)?;
    for (module, names) in homes::HOMES.iter().chain(homes::ALSO) {
        let into = names_of(contents, module)?;
        for name in *names {
            let value = match known.get_item(name)? {
                Some(value) => value,
                None => ext.getattr(*name)?,
            };
            set_default(&into, name, &value)?;
        }
    }
    Ok(())
}

/// Every package of a module answered, empty where nothing else fills it.
fn add_parents(contents: &Bound<'_, PyDict>) -> PyResult<()> {
    let modules: Vec<String> = contents.keys().extract()?;
    for module in modules {
        let parts: Vec<&str> = module.split('.').collect();
        for end in 1..parts.len() {
            names_of(contents, &parts[..end].join("."))?;
        }
    }
    Ok(())
}

/// Each `base` and `core` module's names copied into its package, the
/// longest names first.
fn copy_children(contents: &Bound<'_, PyDict>) -> PyResult<()> {
    let mut modules: Vec<String> = contents.keys().extract()?;
    modules.sort_by_key(|module| std::cmp::Reverse(module.chars().count()));
    for module in modules {
        let Some((parent, child)) = module.rsplit_once('.') else {
            continue;
        };
        if !REEXPORTED_CHILDREN.contains(&child) {
            continue;
        }
        let Some(into) = contents.get_item(parent)? else {
            continue;
        };
        let into = into.cast_into::<PyDict>()?;
        let from = names_of(contents, &module)?;
        for (name, value) in from.iter() {
            set_default(&into, &name.extract::<String>()?, &value)?;
        }
    }
    Ok(())
}

/// Warns where the package's exceptions were built before the provider.
///
/// The package's exceptions are built on whatever `Music21Exception` was
/// there when the first of them was built, and a class cannot be rebuilt
/// on another base afterwards.
fn warn_on_unprovided_base(py: Python<'_>, ext: &Bound<'_, PyModule>) -> PyResult<()> {
    let chord_exception = ext.getattr("ChordException")?;
    let base = music21_exception(py)?;
    let subclass = py
        .import("builtins")?
        .getattr("issubclass")?
        .call1((chord_exception, base))?
        .is_truthy()?;
    if subclass {
        return Ok(());
    }
    let kwargs = PyDict::new(py);
    kwargs.set_item("stacklevel", 1)?;
    py.import("warnings")?.getattr("warn")?.call(
        (
            "music21_rs's exceptions were built before provide_music21() was called, \
             so catching music21.exceptions21.Music21Exception will not catch them",
        ),
        Some(&kwargs),
    )?;
    Ok(())
}

/// The import hook: finds music21's modules here, and builds them.
#[pyclass(module = "music21_rs._provide", name = "_Provider", frozen)]
pub(crate) struct Provider {
    /// Each module answered for, mapped to its names.
    contents: Mutex<Py<PyDict>>,
    /// Each package answered for, mapped to its submodules.
    children: Mutex<BTreeMap<String, BTreeSet<String>>>,
}

impl Provider {
    /// The modules answered for now.
    fn contents_now<'py>(&self, py: Python<'py>) -> Bound<'py, PyDict> {
        let held = self.contents.lock().expect("provider contents");
        held.bind(py).clone()
    }

    /// The submodules of `package` answered for now.
    fn children_of(&self, package: &str) -> BTreeSet<String> {
        let held = self.children.lock().expect("provider children");
        held.get(package).cloned().unwrap_or_default()
    }

    /// Whether `module` has submodules answered for.
    fn is_package(&self, module: &str) -> bool {
        let held = self.children.lock().expect("provider children");
        held.contains_key(module)
    }

    /// What `find_spec` raises for a module of music21's not answered for.
    fn missing(&self, py: Python<'_>, fullname: &str) -> PyResult<String> {
        let contents = self.contents_now(py);
        let mut parent = parent_of(fullname);
        while !parent.is_empty() && !contents.contains(parent)? {
            parent = parent_of(parent);
        }
        let offered: Vec<String> = self.children_of(parent).into_iter().collect();
        let offered = if offered.is_empty() {
            "nothing".to_owned()
        } else {
            offered.join(", ")
        };
        Ok(format!(
            "music21-rs does not provide {fullname}; it answers for music21 here, music21 \
             itself not being installed, and its modules under {parent} are: {offered}"
        ))
    }

    /// The `__getattr__` a provided module answers a missing name with: a
    /// submodule, imported, or an `AttributeError`.
    fn lazy_children<'py>(
        slf: &Bound<'py, Self>,
        fullname: &str,
    ) -> PyResult<Bound<'py, PyCFunction>> {
        let py = slf.py();
        let provider = slf.clone().unbind();
        let fullname = fullname.to_owned();
        PyCFunction::new_closure(
            py,
            Some(c"__getattr__"),
            None,
            move |args: &Bound<'_, PyTuple>,
                  _: Option<&Bound<'_, PyDict>>|
                  -> PyResult<Py<PyAny>> {
                let py = args.py();
                let name: String = args.get_item(0)?.extract()?;
                if provider.get().children_of(&fullname).contains(&name) {
                    let module = py
                        .import("importlib")?
                        .call_method1("import_module", (format!("{fullname}.{name}"),))?;
                    return Ok(module.unbind());
                }
                Err(PyAttributeError::new_err(format!(
                    "module {} has no attribute {}: music21-rs, which answers for \
                     music21 here, does not provide it",
                    PyString::new(py, &fullname).repr()?,
                    PyString::new(py, &name).repr()?
                )))
            },
        )
    }

    /// The `__dir__` a provided module answers with: its names and its
    /// submodules, sorted.
    fn listing<'py>(
        slf: &Bound<'py, Self>,
        fullname: &str,
        names: &Bound<'py, PyDict>,
    ) -> PyResult<Bound<'py, PyCFunction>> {
        let py = slf.py();
        let provider = slf.clone().unbind();
        let fullname = fullname.to_owned();
        let names = names.clone().unbind();
        PyCFunction::new_closure(
            py,
            Some(c"__dir__"),
            None,
            move |args: &Bound<'_, PyTuple>,
                  _: Option<&Bound<'_, PyDict>>|
                  -> PyResult<Vec<String>> {
                let keys: Vec<String> = names.bind(args.py()).keys().extract()?;
                let mut listed: BTreeSet<String> = keys.into_iter().collect();
                listed.extend(provider.get().children_of(&fullname));
                Ok(listed.into_iter().collect())
            },
        )
    }
}

#[pymethods]
impl Provider {
    /// The table of modules, the one Python object the hook holds; a lock
    /// held elsewhere is passed over, as the collector cannot wait.
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        match self.contents.try_lock() {
            Ok(contents) => visit.call(&*contents),
            Err(_) => Ok(()),
        }
    }

    /// Each module answered for, mapped to its names.
    #[getter]
    fn contents(&self, py: Python<'_>) -> Py<PyDict> {
        self.contents_now(py).unbind()
    }

    /// Answers for these modules from now on, and fills in any of them
    /// already imported while they were being worked out.
    fn update(slf: &Bound<'_, Self>, contents: Bound<'_, PyDict>) -> PyResult<()> {
        let py = slf.py();
        let mut children: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let modules: Vec<String> = contents.keys().extract()?;
        for module in &modules {
            if let Some((parent, child)) = module.rsplit_once('.') {
                children
                    .entry(parent.to_owned())
                    .or_default()
                    .insert(child.to_owned());
            }
        }
        *slf.get().contents.lock().expect("provider contents") = contents.unbind();
        *slf.get().children.lock().expect("provider children") = children;

        let loaded = py.import("sys")?.getattr("modules")?;
        for fullname in &modules {
            let module = loaded.call_method1("get", (fullname,))?;
            if module.is_none() {
                continue;
            }
            if module.getattr("__spec__")?.getattr("loader")?.is(slf) {
                Self::exec_module(slf, &module)?;
            }
        }
        Ok(())
    }

    #[pyo3(signature = (fullname, path=None, target=None))]
    fn find_spec<'py>(
        slf: &Bound<'py, Self>,
        fullname: &str,
        path: Option<&Bound<'py, PyAny>>,
        target: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let _ = (path, target);
        let py = slf.py();
        if fullname != "music21" && !fullname.starts_with("music21.") {
            return Ok(None);
        }
        let provider = slf.get();
        if !provider.contents_now(py).contains(fullname)? {
            return Err(PyModuleNotFoundError::new_err(
                provider.missing(py, fullname)?,
            ));
        }
        let kwargs = PyDict::new(py);
        kwargs.set_item("origin", ORIGIN)?;
        kwargs.set_item("is_package", provider.is_package(fullname))?;
        let spec = py
            .import("importlib.machinery")?
            .getattr("ModuleSpec")?
            .call((fullname, slf), Some(&kwargs))?;
        Ok(Some(spec))
    }

    fn create_module(&self, spec: &Bound<'_, PyAny>) -> Option<()> {
        let _ = spec;
        None
    }

    fn exec_module(slf: &Bound<'_, Self>, module: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let fullname: String = module.getattr("__name__")?.extract()?;
        let names = slf
            .get()
            .contents_now(py)
            .get_item(&fullname)?
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err(fullname.clone()))?
            .cast_into::<PyDict>()?;
        module.setattr(
            "__doc__",
            format!("music21's {fullname}, as far as music21-rs provides it."),
        )?;
        module
            .getattr("__dict__")?
            .call_method1("update", (&names,))?;
        let listed = match names.get_item("__all__")? {
            Some(listed) => PyList::new(py, listed.try_iter()?.collect::<PyResult<Vec<_>>>()?)?,
            None => {
                let public: BTreeSet<String> = names
                    .keys()
                    .extract::<Vec<String>>()?
                    .into_iter()
                    .filter(|name| !name.starts_with('_'))
                    .collect();
                PyList::new(py, public)?
            }
        };
        module.setattr("__all__", listed)?;
        if fullname == "music21" {
            stand_in(module)?;
        }
        module.setattr("__getattr__", Self::lazy_children(slf, &fullname)?)?;
        module.setattr("__dir__", Self::listing(slf, &fullname, &names)?)?;
        Ok(())
    }
}

/// The provided `music21` package's version and its mark.
fn stand_in(module: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = module.py();
    let version = env!("CARGO_PKG_VERSION");
    module.setattr(
        "__doc__",
        format!(
            "music21-rs {version} standing in for music21, which is not installed: \
             music21's modules and names, as far as music21-rs provides them."
        ),
    )?;
    module.setattr("__version__", version)?;
    module.setattr("VERSION", PyTuple::new(py, version_parts(version))?)?;
    module.setattr("VERSION_STR", version)?;
    module.setattr("__music21_rs__", true)?;
    Ok(())
}

/// The numbers of a version, each part's digits read as one.
fn version_parts(version: &str) -> Vec<u64> {
    version
        .split('.')
        .filter_map(|part| {
            let digits: String = part.chars().filter(char::is_ascii_digit).collect();
            digits.parse().ok()
        })
        .collect()
}

/// A dotted name's package: `music21` for `music21.stream`, nothing for
/// `music21`.
fn parent_of(fullname: &str) -> &str {
    fullname.rsplit_once('.').map_or("", |(parent, _)| parent)
}

#[cfg(test)]
mod tests {
    use super::{parent_of, version_parts};

    #[test]
    fn a_version_is_read_as_its_numbers() {
        assert_eq!(version_parts("0.21.0"), [0, 21, 0]);
        assert_eq!(version_parts("10.2b1.x"), [10, 21]);
    }

    #[test]
    fn a_module_s_parent_is_its_package() {
        assert_eq!(parent_of("music21.stream.base"), "music21.stream");
        assert_eq!(parent_of("music21"), "");
    }
}
