//! music21's own tests, run where music21 is not installed and
//! `music21_rs.provide_music21()` answers `import music21` instead.
//!
//! music21's tests live inside music21's package, so they cannot be imported
//! without importing music21. Each module's source is read by path and run
//! under its own dotted name instead, so its `from music21 import ...` lines
//! are answered by the provider. A module whose imports fail is blocked
//! whole, and what it could not import is the blocker. A module that imports
//! has the names the provider gives its module put over its own, as
//! `install_into_music21` does to an installed music21, and its `Test` class
//! and docstrings are run. A module the provider has nothing for runs
//! music21's own code, which is not music21-rs: its passes are kept apart.
//! music21's test data (`testFiles`, `testPrimitive`), which the provider
//! does not answer for, is read from music21's source wherever a module
//! imports it.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use pyo3::exceptions::{PyAttributeError, PyException, PyImportError};
use pyo3::prelude::*;
use pyo3::types::{PyCFunction, PyDict, PyList, PyModule, PyString, PyTuple, PyType};
use regex::Regex;

/// Long enough for any test music21 runs; a test that hangs on a
/// half-provided object is cut off rather than holding up the run.
const TEST_SECONDS: u32 = 60;

/// How much of a failure's text is kept, from its end.
const DETAIL_CHARACTERS: usize = 2000;

/// music21's modules that hold test data, scores as text, rather than code.
const TEST_DATA: [&str; 2] = ["testFiles", "testPrimitive"];

/// A memory address in a repr, as music21's `test.testRunner.ADDRESS` finds
/// one; its runner rewrites them in each example's expected output to
/// `0x...`, so an object's address never fails a doctest.
static ADDRESS: LazyLock<Regex> = LazyLock::new(|| pattern("0x[0-9A-Fa-f]+"));

static NOT_PROVIDED: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"music21-rs does not provide (\S+?);"));
static NO_MODULE: LazyLock<Regex> = LazyLock::new(|| pattern(r"No module named '([^']+)'"));
static CANNOT_IMPORT: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"cannot import name '([^']+)' from '([^']+)'"));
static MODULE_ATTRIBUTE: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"module '([^']+)' has no attribute '([^']+)'"));
static TYPE_ATTRIBUTE: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"type object '([^']+)' has no attribute '([^']+)'"));
static OBJECT_ATTRIBUTE: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"'([^']+)' object has no attribute '([^']+)'"));
static UNDEFINED: LazyLock<Regex> = LazyLock::new(|| pattern(r"name '([^']+)' is not defined"));
static RAISED: LazyLock<Regex> = LazyLock::new(|| {
    pattern(r"(?m)^\s*(ModuleNotFoundError|ImportError|AttributeError|NameError): (.*)$")
});

fn pattern(text: &str) -> Regex {
    Regex::new(text).expect("a valid pattern")
}

/// How one of music21's modules fared: blocked at its import, or its
/// tests' results.
#[derive(Debug, Default)]
pub(crate) struct Record {
    pub(crate) module: String,
    pub(crate) imported: bool,
    pub(crate) provided: bool,
    pub(crate) blocker: Option<String>,
    pub(crate) error: Option<String>,
    pub(crate) run: usize,
    pub(crate) passed: Vec<String>,
    pub(crate) failures: Vec<String>,
    pub(crate) errors: Vec<String>,
    pub(crate) blockers: BTreeMap<String, String>,
    pub(crate) detail: BTreeMap<String, String>,
}

/// One of music21's modules with a test or a docstring example in it.
struct Source {
    name: String,
    path: PathBuf,
    text: String,
}

/// Which error a failure's text is read for: the first, which stops a
/// doctest there, or the last line of a traceback.
#[derive(Clone, Copy, PartialEq)]
enum Raised {
    First,
    Last,
}

/// The missing module or name an import or attribute error is about, as a
/// dotted name, or None for one that names nothing. `modules` is music21's
/// modules by the name `from music21 import *` gives each: a docstring that
/// uses one music21-rs does not provide stops on a `NameError`.
fn blocker_of(kind: &str, message: &str, modules: &BTreeSet<String>) -> Option<String> {
    if kind == "ModuleNotFoundError" || kind == "ImportError" {
        for found in [&*NOT_PROVIDED, &*NO_MODULE] {
            if let Some(found) = found.captures(message) {
                return Some(found[1].to_owned());
            }
        }
        let found = CANNOT_IMPORT.captures(message)?;
        return Some(format!("{}.{}", &found[2], &found[1]));
    }
    if kind == "NameError" {
        let found = UNDEFINED.captures(message)?;
        if !modules.contains(&found[1]) {
            return None;
        }
        return Some(format!("music21.{}", &found[1]));
    }
    for found in [&*MODULE_ATTRIBUTE, &*TYPE_ATTRIBUTE, &*OBJECT_ATTRIBUTE] {
        if let Some(found) = found.captures(message) {
            return Some(format!("{}.{}", &found[1], &found[2]));
        }
    }
    None
}

/// The blocker a failure's text names.
fn blocker_in(text: &str, raised: Raised, modules: &BTreeSet<String>) -> Option<String> {
    let all: Vec<_> = RAISED.captures_iter(text).collect();
    let chosen = match raised {
        Raised::First => all.first()?,
        Raised::Last => all.last()?,
    };
    blocker_of(&chosen[1], &chosen[2], modules)
}

/// `music21.stream.base` for `<root>/music21/stream/base.py`.
fn dotted_name(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path).with_extension("");
    let mut parts: Vec<String> = relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    if parts.last().is_some_and(|last| last == "__init__") {
        parts.pop();
    }
    parts.join(".")
}

/// Every module of music21's with a test or a docstring example in it, in
/// the order `os.walk` finds them, sorted within each directory.
fn module_files(root: &Path, only: &[String]) -> PyResult<Vec<Source>> {
    let mut found = Vec::new();
    walk(root, &root.join("music21"), only, &mut found)?;
    Ok(found)
}

fn walk(root: &Path, directory: &Path, only: &[String], into: &mut Vec<Source>) -> PyResult<()> {
    let mut files = Vec::new();
    let mut subdirectories = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.path().is_dir() {
            let linked = entry.file_type()?.is_symlink();
            if !name.starts_with('.') && !name.starts_with('_') && !linked {
                subdirectories.push(name);
            }
        } else {
            files.push(name);
        }
    }
    files.sort();
    subdirectories.sort();

    // The directory's own modules, then each of its packages'.
    for file in files {
        if !file.ends_with(".py") {
            continue;
        }
        let path = directory.join(&file);
        let name = dotted_name(root, &path);
        if !only.is_empty() && !only.iter().any(|part| name.contains(part.as_str())) {
            continue;
        }
        let text = fs::read_to_string(&path)?;
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned();
        if text.contains("unittest") || text.contains(">>>") {
            into.push(Source { name, path, text });
        }
    }
    for subdirectory in subdirectories {
        walk(root, &directory.join(subdirectory), only, into)?;
    }
    Ok(())
}

/// Whether a module only holds tests of others', as `test_base` does.
fn is_test_module(name: &str) -> bool {
    let last = name.rsplit('.').next().unwrap_or(name);
    last.starts_with("test") || last == "tests"
}

/// `ADDRESS` rewritten to `0x...` throughout an expected output.
fn without_addresses(want: &str) -> String {
    ADDRESS.replace_all(want, "0x...").into_owned()
}

/// The last `DETAIL_CHARACTERS` characters of a failure's text.
fn detail_of(text: &str) -> String {
    let count = text.chars().count();
    text.chars()
        .skip(count.saturating_sub(DETAIL_CHARACTERS))
        .collect()
}

/// Finds music21's test data in its source, where the provider, which
/// raises for every module of music21's it lacks, does not answer.
#[pyclass(module = "xtask", name = "TestData", frozen)]
struct TestData {
    root: PathBuf,
    provider: Py<PyAny>,
}

#[pymethods]
impl TestData {
    #[pyo3(signature = (fullname, path=None, target=None))]
    fn find_spec<'py>(
        &self,
        py: Python<'py>,
        fullname: &str,
        path: Option<&Bound<'py, PyAny>>,
        target: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let _ = (path, target);
        if !fullname.starts_with("music21.") {
            return Ok(None);
        }
        let contents = self.provider.bind(py).getattr("contents")?;
        if contents.contains(fullname)? {
            return Ok(None);
        }
        let last = fullname.rsplit('.').next().unwrap_or(fullname);
        if !TEST_DATA.contains(&last) {
            return Ok(None);
        }
        let mut file = self.root.clone();
        file.extend(fullname.split('.'));
        let file = file.with_extension("py");
        if !file.is_file() {
            return Ok(None);
        }
        let spec = py
            .import("importlib.util")?
            .call_method1("spec_from_file_location", (fullname, file))?;
        Ok(Some(spec))
    }
}

/// The module run from its source under its own name.
fn load<'py>(py: Python<'py>, source: &Source) -> PyResult<Bound<'py, PyModule>> {
    let module = PyModule::new(py, &source.name)?;
    let path = source.path.to_string_lossy().into_owned();
    module.setattr("__file__", &path)?;
    let is_package = path.ends_with("__init__.py");
    let package = if is_package {
        source.name.as_str()
    } else {
        source
            .name
            .rsplit_once('.')
            .map_or("", |(parent, _)| parent)
    };
    module.setattr("__package__", package)?;
    if is_package {
        let directory = source.path.parent().unwrap_or(Path::new(""));
        module.setattr("__path__", vec![directory.to_string_lossy().into_owned()])?;
    }

    // Registered while it runs, as an import would: a dataclass looks its
    // module up by name to read its annotations. Not left there, so that no
    // other module imports music21's own code in place of the provider's.
    let ran = registered(py, &source.name, module.as_any(), || {
        let builtins = py.import("builtins")?;
        let code = builtins
            .getattr("compile")?
            .call1((&source.text, &path, "exec"))?;
        builtins.getattr("exec")?.call1((code, module.dict()))?;
        Ok(())
    });
    ran?;
    Ok(module)
}

/// Runs `body` with `module` in `sys.modules` under `name`, and puts back
/// what was there.
fn registered<T>(
    py: Python<'_>,
    name: &str,
    module: &Bound<'_, PyAny>,
    body: impl FnOnce() -> PyResult<T>,
) -> PyResult<T> {
    let modules = py.import("sys")?.getattr("modules")?;
    let previous = modules.call_method1("get", (name,))?;
    modules.set_item(name, module)?;
    let done = body();
    if previous.is_none() {
        modules.call_method1("pop", (name, py.None()))?;
    } else {
        modules.set_item(name, previous)?;
    }
    done
}

/// What the provider answers for the module, or nothing.
fn provided_names<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyDict>> {
    let names = PyDict::new(py);
    let provided = match py
        .import("importlib")?
        .call_method1("import_module", (name,))
    {
        Ok(provided) => provided,
        Err(error) if error.is_instance_of::<PyImportError>(py) => return Ok(names),
        Err(error) => return Err(error),
    };
    let origin = attribute(&provided, "__spec__")?
        .map(|spec| attribute(&spec, "origin"))
        .transpose()?
        .flatten();
    let from_provider = match origin {
        Some(origin) => origin.eq("music21-rs")?,
        None => false,
    };
    if !from_provider {
        return Ok(names);
    }
    for (key, value) in provided.getattr("__dict__")?.cast::<PyDict>()?.iter() {
        if !key.extract::<String>()?.starts_with('_') {
            names.set_item(key, value)?;
        }
    }
    Ok(names)
}

/// `getattr(object, name, None)`.
fn attribute<'py>(object: &Bound<'py, PyAny>, name: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
    match object.getattr(name) {
        Ok(value) if value.is_none() => Ok(None),
        Ok(value) => Ok(Some(value)),
        Err(error) if error.is_instance_of::<PyAttributeError>(object.py()) => Ok(None),
        Err(error) => Err(error),
    }
}

/// The module's `Test` class and its docstrings, the docstrings found
/// before anything is put over the classes that carry them.
fn gather<'py>(py: Python<'py>, module: &Bound<'py, PyModule>) -> PyResult<Bound<'py, PyAny>> {
    let unittest = py.import("unittest")?;
    let suite = unittest.getattr("TestSuite")?.call0()?;
    if let Some(case) = attribute(module.as_any(), "Test")?
        && let Ok(case) = case.cast::<PyType>()
        && case.is_subclass(&unittest.getattr("TestCase")?)?
    {
        let loaded = unittest
            .getattr("defaultTestLoader")?
            .call_method1("loadTestsFromTestCase", (case,))?;
        suite.call_method1("addTests", (loaded,))?;
    }

    // The finder keeps what `inspect.getmodule` says comes from the module,
    // which asks `sys.modules`: where the provider's module of that name was
    // imported first, every class here would read as the provider's, and
    // none of their docstrings would be found.
    let doctest = py.import("doctest")?;
    let kwargs = PyDict::new(py);
    kwargs.set_item("exclude_empty", true)?;
    let finder = doctest.getattr("DocTestFinder")?.call((), Some(&kwargs))?;
    let name = module.name()?.to_string();
    let found = registered(py, &name, module.as_any(), || {
        finder.call_method1("find", (module, &name))
    });
    let mut examples: Vec<(String, Bound<'py, PyAny>)> = match found {
        Ok(found) => found
            .try_iter()?
            .map(|example| {
                let example = example?;
                Ok((example.getattr("name")?.extract()?, example))
            })
            .collect::<PyResult<_>>()?,
        Err(error) if error.is_instance_of::<PyException>(py) => Vec::new(),
        Err(error) => return Err(error),
    };
    examples.sort_by(|a, b| a.0.cmp(&b.0));

    // Each docstring with examples, addresses taken out of what it expects.
    let flags: i64 = doctest.getattr("ELLIPSIS")?.extract::<i64>()?
        | doctest.getattr("NORMALIZE_WHITESPACE")?.extract::<i64>()?;
    for (_, example) in examples {
        let steps = example.getattr("examples")?;
        if !steps.is_truthy()? {
            continue;
        }
        for step in steps.try_iter()? {
            let step = step?;
            let want: String = step.getattr("want")?.extract()?;
            step.setattr("want", without_addresses(&want))?;
        }
        let kwargs = PyDict::new(py);
        kwargs.set_item("optionflags", flags)?;
        let case = doctest
            .getattr("DocTestCase")?
            .call((example,), Some(&kwargs))?;
        suite.call_method1("addTest", (case,))?;
    }
    Ok(suite)
}

/// Every test in a suite, nested suites opened.
fn leaves<'py>(suite: &Bound<'py, PyAny>, into: &mut Vec<Bound<'py, PyAny>>) -> PyResult<()> {
    let suite_class = suite.py().import("unittest")?.getattr("TestSuite")?;
    for child in suite.try_iter()? {
        let child = child?;
        if child.is_instance(&suite_class)? {
            leaves(&child, into)?;
        } else {
            into.push(child);
        }
    }
    Ok(())
}

/// A `unittest.TestResult` cutting each test off after `TEST_SECONDS` and
/// naming each that passed in `passed`.
fn timed_result<'py>(py: Python<'py>, passed: &Bound<'py, PyList>) -> PyResult<Bound<'py, PyAny>> {
    let base = py.import("unittest")?.getattr("TestResult")?;
    let result = base.call0()?;
    let signal = py.import("signal")?;

    // Each override sets the alarm, starting it or stopping it, then calls
    // `TestResult`'s own method on the result.
    let alarm = |seconds: u32| -> PyResult<Bound<'py, PyCFunction>> {
        let signal = signal.clone().unbind();
        let held = result.clone().unbind();
        let base = base.clone().unbind();
        let method = if seconds == 0 {
            "stopTest"
        } else {
            "startTest"
        };
        PyCFunction::new_closure(
            py,
            None,
            None,
            move |args: &Bound<'_, PyTuple>, _: Option<&Bound<'_, PyDict>>| -> PyResult<()> {
                let py = args.py();
                let test = args.get_item(0)?;
                signal.bind(py).call_method1("alarm", (seconds,))?;
                base.bind(py).call_method1(method, (held.bind(py), &test))?;
                Ok(())
            },
        )
    };
    result.setattr("startTest", alarm(TEST_SECONDS)?)?;
    result.setattr("stopTest", alarm(0)?)?;

    let held = result.clone().unbind();
    let base_held = base.clone().unbind();
    let list = passed.clone().unbind();
    let success = PyCFunction::new_closure(
        py,
        None,
        None,
        move |args: &Bound<'_, PyTuple>, _: Option<&Bound<'_, PyDict>>| -> PyResult<()> {
            let py = args.py();
            let test = args.get_item(0)?;
            base_held
                .bind(py)
                .call_method1("addSuccess", (held.bind(py), &test))?;
            list.bind(py).append(test.str()?)?;
            Ok(())
        },
    )?;
    result.setattr("addSuccess", success)?;
    Ok(result)
}

/// How one module fared: blocked at its import, or its tests' results.
fn run_module(
    py: Python<'_>,
    source: &Source,
    package_names: &Bound<'_, PyDict>,
    modules: &BTreeSet<String>,
) -> PyResult<Record> {
    let mut record = Record {
        module: source.name.clone(),
        ..Record::default()
    };
    let module = match load(py, source) {
        Ok(module) => module,
        Err(error) => {
            record_blocked(py, &mut record, &error, modules)?;
            return Ok(record);
        }
    };
    record.imported = true;

    // The tests found, then the provider's names put over music21's.
    let suite = gather(py, &module)?;
    let overlay = provided_names(py, &source.name)?;
    module.dict().update(overlay.as_mapping())?;
    record.provided = !overlay.is_empty() || is_test_module(&source.name);
    let doc_test_case = py.import("doctest")?.getattr("DocTestCase")?;
    let mut tests = Vec::new();
    leaves(&suite, &mut tests)?;
    for test in &tests {
        if !test.is_instance(&doc_test_case)? {
            continue;
        }
        let globs = package_names.copy()?;
        globs.update(module.dict().as_mapping())?;
        test.getattr("_dt_test")?.setattr("globs", globs)?;
    }

    // The run, each test cut off at the time allowed.
    let passed = PyList::empty(py);
    let result = timed_result(py, &passed)?;
    if let Err(error) = suite.call_method1("run", (&result,)) {
        record.error = Some(format!("the run stopped: {}", error.value(py).repr()?));
    }
    py.import("signal")?.call_method1("alarm", (0,))?;
    record.run = result.getattr("testsRun")?.extract()?;
    record.passed = passed.extract()?;
    for kind in ["failures", "errors"] {
        for pair in result.getattr(kind)?.try_iter()? {
            let pair = pair?;
            let test = pair.get_item(0)?;
            let text: String = pair.get_item(1)?.extract()?;
            let test_name = test.str()?.to_string();
            let raised = if test.is_instance(&doc_test_case)? {
                Raised::First
            } else {
                Raised::Last
            };
            if let Some(blocker) = blocker_in(&text, raised, modules) {
                record.blockers.insert(test_name.clone(), blocker);
            }
            record.detail.insert(test_name.clone(), detail_of(&text));
            if kind == "failures" {
                record.failures.push(test_name);
            } else {
                record.errors.push(test_name);
            }
        }
    }
    Ok(record)
}

/// A module whose import stopped: the error, and the module or name it
/// stopped at.
fn record_blocked(
    py: Python<'_>,
    record: &mut Record,
    error: &PyErr,
    modules: &BTreeSet<String>,
) -> PyResult<()> {
    let value = error.value(py);
    let lines: Vec<String> = py
        .import("traceback")?
        .call_method1("format_exception_only", (value,))?
        .extract()?;
    record.error = Some(lines.concat().trim().to_owned());
    let kind = value.get_type().name()?.to_string();
    let blocks =
        value.is_instance_of::<PyImportError>() || value.is_instance_of::<PyAttributeError>();
    if blocks {
        record.blocker = blocker_of(&kind, &value.str()?.to_string(), modules);
    }
    if record.blocker.is_none() {
        record.blocker = Some(format!("<{kind} importing {}>", record.module));
    }
    Ok(())
}

/// What a docstring of music21's sees: the package's names and its modules,
/// as music21's own runner gives it a copy of the package, whose `__init__`
/// has imported every module. The provided package imports its modules on
/// first asking, so each is asked for here.
fn package_globals<'py>(music21: &Bound<'py, PyModule>) -> PyResult<Bound<'py, PyDict>> {
    let py = music21.py();
    let names = PyDict::new(py);
    let listed: Vec<String> = py
        .import("builtins")?
        .getattr("dir")?
        .call1((music21,))?
        .extract()?;
    for key in listed {
        if key.starts_with('_') {
            continue;
        }
        match music21.getattr(key.as_str()) {
            Ok(value) => names.set_item(key, value)?,
            Err(error)
                if error.is_instance_of::<PyImportError>(py)
                    || error.is_instance_of::<PyAttributeError>(py) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(names)
}

/// music21's own modules, by the name `from music21 import *` gives each.
fn music21_modules(py: Python<'_>, package: &Path) -> PyResult<BTreeSet<String>> {
    let mut modules = BTreeSet::new();
    for entry in fs::read_dir(package)? {
        let entry = entry?.file_name().to_string_lossy().into_owned();
        let name = entry.strip_suffix(".py").unwrap_or(&entry);
        let identifier = PyString::new(py, name)
            .call_method0("isidentifier")?
            .is_truthy()?;
        if identifier && !name.starts_with('_') {
            modules.insert(name.to_owned());
        }
    }
    Ok(modules)
}

/// The exception a test that runs too long is cut off with.
fn alarm_handler(py: Python<'_>) -> PyResult<Bound<'_, PyCFunction>> {
    let namespace = PyDict::new(py);
    let timed_out = py
        .import("builtins")?
        .getattr("type")?
        .call1((
            "_TimedOut",
            PyTuple::new(py, [py.get_type::<PyException>()])?,
            namespace,
        ))?
        .unbind();
    PyCFunction::new_closure(
        py,
        Some(c"_alarm"),
        None,
        move |args: &Bound<'_, PyTuple>, _: Option<&Bound<'_, PyDict>>| -> PyResult<()> {
            let py = args.py();
            let raised = timed_out
                .bind(py)
                .call1((format!("took longer than {TEST_SECONDS} seconds"),))?;
            Err(PyErr::from_value(raised))
        },
    )
}

/// Every module's record, music21 answered by the provider throughout.
/// `root` is music21's checkout; `only`, a comma-separated list, keeps the
/// modules whose names hold one of its parts.
pub(crate) fn run(py: Python<'_>, root: &Path, only: &str) -> PyResult<Vec<Record>> {
    py.import("music21_rs")?.call_method0("provide_music21")?;
    let mut provider = None;
    for finder in py.import("sys")?.getattr("meta_path")?.try_iter()? {
        let finder = finder?;
        if finder.hasattr("contents")? {
            provider = Some(finder);
            break;
        }
    }
    let provider =
        provider.ok_or_else(|| PyImportError::new_err("no provider on sys.meta_path"))?;
    let data = Bound::new(
        py,
        TestData {
            root: root.to_path_buf(),
            provider: provider.unbind(),
        },
    )?;
    py.import("sys")?
        .getattr("meta_path")?
        .call_method1("insert", (0, data))?;
    let music21 = py.import("music21")?;
    let package_names = package_globals(&music21)?;

    // Each module run with the alarm armed, and the old handler put back.
    let signal = py.import("signal")?;
    let previous =
        signal.call_method1("signal", (signal.getattr("SIGALRM")?, alarm_handler(py)?))?;
    let modules = music21_modules(py, &root.join("music21"))?;
    let only: Vec<String> = only
        .split(',')
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect();
    let mut records = Vec::new();
    let ran = module_files(root, &only).and_then(|sources| {
        for source in &sources {
            records.push(run_module(py, source, &package_names, &modules)?);
        }
        Ok(())
    });
    signal.call_method1("signal", (signal.getattr("SIGALRM")?, previous))?;
    ran?;
    Ok(records)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    use super::{Raised, blocker_in, blocker_of, detail_of, dotted_name, without_addresses};

    fn modules() -> BTreeSet<String> {
        ["stream", "note"]
            .iter()
            .map(|name| (*name).to_owned())
            .collect()
    }

    #[test]
    fn an_import_error_names_what_was_missing() {
        let found = |kind, message| blocker_of(kind, message, &modules());
        assert_eq!(
            found(
                "ModuleNotFoundError",
                "music21-rs does not provide music21.mei; it answers"
            ),
            Some("music21.mei".to_owned())
        );
        assert_eq!(
            found(
                "ImportError",
                "cannot import name 'Foo' from 'music21.bar' (unknown)"
            ),
            Some("music21.bar.Foo".to_owned())
        );
        assert_eq!(found("ImportError", "something else"), None);
        assert_eq!(
            found("AttributeError", "'Note' object has no attribute 'tie'"),
            Some("Note.tie".to_owned())
        );
        assert_eq!(
            found("NameError", "name 'stream' is not defined"),
            Some("music21.stream".to_owned())
        );
        assert_eq!(found("NameError", "name 'x' is not defined"), None);
    }

    #[test]
    fn a_doctest_is_blocked_by_its_first_error_and_a_test_by_its_last() {
        let text = "  AttributeError: module 'a' has no attribute 'b'\n\
                    NameError: name 'note' is not defined\n";
        assert_eq!(
            blocker_in(text, Raised::First, &modules()),
            Some("a.b".to_owned())
        );
        assert_eq!(
            blocker_in(text, Raised::Last, &modules()),
            Some("music21.note".to_owned())
        );
        assert_eq!(blocker_in("ValueError: x", Raised::Last, &modules()), None);
    }

    #[test]
    fn a_module_is_named_by_its_path() {
        let root = Path::new("/m");
        assert_eq!(
            dotted_name(root, Path::new("/m/music21/stream/base.py")),
            "music21.stream.base"
        );
        assert_eq!(
            dotted_name(root, Path::new("/m/music21/stream/__init__.py")),
            "music21.stream"
        );
    }

    #[test]
    fn addresses_are_rewritten_and_detail_is_cut_from_the_end() {
        assert_eq!(without_addresses("<X at 0x7fA0>, 0x"), "<X at 0x...>, 0x");
        let long = "a".repeat(10) + &"b".repeat(2000);
        assert_eq!(detail_of(&long), "b".repeat(2000));
    }
}
