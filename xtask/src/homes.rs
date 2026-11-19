//! Works out `python/src/homes.rs` from music21 itself.
//!
//! `provide_music21()` places a class by its module string. A function or a
//! constant has none, so this finds, for each name the wheel exports and
//! places nowhere, the module of music21's that defines it: a function or
//! class by its `__module__`, a constant by the module whose source assigns
//! it. A name music21 defines in two places is an error unless [`CHOSEN`]
//! settles it, and one it defines nowhere is listed in `NO_HOME`, the
//! package's own.
//!
//! It then lists, for each module the package answers for, the names music21
//! imports into it from where they live -- `music21.stream.StreamException`,
//! `music21.search.noteNameSearch` -- so that those resolve as in music21.
//!
//! It needs the wheel installed (`maturin develop`) and music21 importable,
//! the submodule's where no other is.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::error::Error;
use std::fs;
use std::path::Path;

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};

use crate::bench::{add_dependency_venv, add_submodule_fallback};

/// Names music21 defines in more than one module, and the one the package's
/// own is: `features.jSymbolic` has an `extractorsById` of its own library.
const CHOSEN: [(&str, &str); 1] = [("extractorsById", "music21.features.base")];

/// Where the table is written, from the workspace root.
const HOMES_FILE: &str = "python/src/homes.rs";

/// What `homes` is asked to do.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Mode {
    /// Rewrite the table.
    Write,
    /// Fail where the table is not what music21 says now.
    Check,
    /// Print music21's public names against the provided ones.
    Coverage,
}

impl Mode {
    pub(crate) fn parse(args: &[String]) -> Self {
        if args.iter().any(|arg| arg == "--check") {
            return Self::Check;
        }
        if args.iter().any(|arg| arg == "--coverage") {
            return Self::Coverage;
        }
        Self::Write
    }
}

/// The three tables `homes.rs` holds, each sorted.
struct Tables {
    homes: BTreeMap<String, BTreeSet<String>>,
    also: BTreeMap<String, BTreeSet<String>>,
    no_home: BTreeSet<String>,
}

/// Runs `homes` and answers the exit code.
pub(crate) fn run(workspace_root: &Path, mode: Mode) -> Result<i32, Box<dyn Error>> {
    let path = workspace_root.join(HOMES_FILE);
    let written = Python::attach(|py| -> PyResult<Option<String>> {
        add_dependency_venv(py, workspace_root)?;
        add_submodule_fallback(py, workspace_root)?;
        py.import("warnings")?
            .call_method1("simplefilter", ("ignore",))?;
        let modules = music21_modules(py)?;
        if mode == Mode::Coverage {
            print_coverage(py, &modules)?;
            return Ok(None);
        }
        Ok(Some(render(&work_out(py, &modules)?)))
    })?;
    let Some(written) = written else {
        return Ok(0);
    };
    if mode == Mode::Write {
        fs::write(&path, written)?;
        return Ok(0);
    }
    if fs::read_to_string(&path)? != written {
        eprintln!(
            "{} is stale: run `cargo run -p xtask --features python -- homes`",
            path.display()
        );
        return Ok(1);
    }
    Ok(0)
}

/// Every module of music21's by name, tests left out, in the order
/// `pkgutil.walk_packages` finds them.
fn music21_modules(py: Python<'_>) -> PyResult<Vec<(String, Bound<'_, PyModule>)>> {
    let music21 = py.import("music21")?;
    let mut modules = vec![("music21".to_owned(), music21.clone())];
    let found = py
        .import("pkgutil")?
        .call_method1("walk_packages", (music21.getattr("__path__")?, "music21."))?;
    let importlib = py.import("importlib")?;
    for each in found.try_iter()? {
        let name: String = each?.getattr("name")?.extract()?;
        let last = name.rsplit('.').next().unwrap_or(&name);
        if last.starts_with("test") {
            continue;
        }
        // A module that needs a package music21 does not (its pytest
        // plugin needs pytest) defines nothing the table places.
        let module = match importlib.call_method1("import_module", (&name,)) {
            Ok(module) => module.cast_into::<PyModule>()?,
            Err(error) if missing_outside_music21(py, &error) => continue,
            Err(error) => return Err(error),
        };
        modules.push((name, module));
    }
    Ok(modules)
}

/// Whether `error` is a `ModuleNotFoundError` for a package other than
/// music21's own.
fn missing_outside_music21(py: Python<'_>, error: &PyErr) -> bool {
    if !error.is_instance_of::<pyo3::exceptions::PyModuleNotFoundError>(py) {
        return false;
    }
    let missing: Option<String> = error
        .value(py)
        .getattr("name")
        .ok()
        .and_then(|name| name.extract().ok());
    missing.is_some_and(|name| name != "music21" && !name.starts_with("music21."))
}

/// Whether `module` is where music21 defines `name`, not imports it.
fn defines(module: &Bound<'_, PyModule>, name: &str) -> PyResult<bool> {
    let py = module.py();
    let Some(value) = module.dict().get_item(name)? else {
        return Ok(false);
    };
    if value.is_instance_of::<PyModule>() {
        return Ok(false);
    }

    // A class, a function, or one wrapped as `lru_cache` wraps them.
    let home = value.getattr("__module__").ok();
    let home = home.and_then(|home| home.extract::<String>().ok());
    if let Some(home) = home
        && value.is_callable()
    {
        return Ok(home == module.name()?.to_string());
    }

    // A constant, by the module whose source assigns it.
    let source = module
        .getattr("__file__")
        .ok()
        .filter(|file| !file.is_none());
    let Some(source) = source else {
        return Ok(false);
    };
    let source: String = source.extract()?;
    let text = fs::read_to_string(&source)
        .map_err(|error| pyo3::exceptions::PyOSError::new_err(error.to_string()))?;
    let _ = py;
    Ok(assigns(&text, name))
}

/// Whether a line of `text` starts by assigning or annotating `name`, as
/// `^name\s*[:=]` finds one with `re.MULTILINE`.
fn assigns(text: &str, name: &str) -> bool {
    let starts = std::iter::once(0).chain(text.match_indices('\n').map(|(at, _)| at + 1));
    for start in starts {
        let Some(rest) = text[start..].strip_prefix(name) else {
            continue;
        };
        let rest = rest.trim_start_matches(char::is_whitespace);
        if rest.starts_with(':') || rest.starts_with('=') {
            return true;
        }
    }
    false
}

/// The wheel's private provider module.
fn provider(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    py.import("music21_rs._provide").map(Bound::into_any)
}

/// `_provide._contents(...)`: each module provided, mapped to its names.
fn provided(
    py: Python<'_>,
    homes: bool,
    children: bool,
) -> PyResult<BTreeMap<String, BTreeSet<String>>> {
    let kwargs = PyDict::new(py);
    kwargs.set_item("homes", homes)?;
    kwargs.set_item("children", children)?;
    let contents = provider(py)?.call_method("_contents", (), Some(&kwargs))?;
    let mut provided = BTreeMap::new();
    for (module, names) in contents.cast::<PyDict>()?.iter() {
        let names: Vec<String> = names.cast::<PyDict>()?.keys().extract()?;
        let names: BTreeSet<String> = names.into_iter().collect();
        provided.insert(module.extract()?, names);
    }
    Ok(provided)
}

/// The three tables, worked out from music21's modules.
fn work_out(py: Python<'_>, modules: &[(String, Bound<'_, PyModule>)]) -> PyResult<Tables> {
    let own = provided(py, false, false)?;
    let placed: BTreeSet<&String> = own.values().flatten().collect();
    let wheel = py.import("music21_rs")?;
    let exported: Vec<String> = provider(py)?.getattr("_EXPORTED")?.extract()?;
    let mut exported: BTreeSet<String> = exported.into_iter().collect();
    exported.extend(
        wheel
            .getattr("__exception_names__")?
            .extract::<Vec<String>>()?,
    );

    // Each name placed nowhere, by the one module of music21's defining it.
    let mut homes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut no_home = BTreeSet::new();
    for name in exported.iter().filter(|name| !placed.contains(name)) {
        let mut found = Vec::new();
        for (module_name, module) in modules {
            if defines(module, name)? {
                found.push(module_name.clone());
            }
        }
        if let Some((_, chosen)) = CHOSEN.iter().find(|(each, _)| each == name) {
            if !found.iter().any(|module| module == chosen) {
                return Err(pyo3::exceptions::PySystemExit::new_err(format!(
                    "music21 does not define {name} in {chosen}"
                )));
            }
            found = vec![(*chosen).to_owned()];
        }
        if found.len() > 1 {
            return Err(pyo3::exceptions::PySystemExit::new_err(format!(
                "music21 defines {name} in {found:?}: add it to CHOSEN"
            )));
        }
        match found.pop() {
            Some(module) => {
                homes.entry(module).or_default().insert(name.clone());
            }
            None => {
                no_home.insert(name.clone());
            }
        }
    }

    // Every module provided, with the homes placed, and the identity of
    // music21's own object for each name in each.
    let mut placed_in = own.clone();
    for (module, names) in &homes {
        placed_in
            .entry(module.clone())
            .or_default()
            .extend(names.iter().cloned());
    }
    let by_name: HashMap<&str, &Bound<'_, PyModule>> = modules
        .iter()
        .map(|(name, module)| (name.as_str(), module))
        .collect();
    let mut theirs: HashMap<String, BTreeSet<usize>> = HashMap::new();
    for (module_name, names) in &placed_in {
        let Some(module) = by_name.get(module_name.as_str()) else {
            continue;
        };
        for name in names {
            if let Some(value) = module.dict().get_item(name)? {
                theirs
                    .entry(name.clone())
                    .or_default()
                    .insert(value.as_ptr() as usize);
            }
        }
    }

    // The names music21 imports into each module from where they live.
    let mut also: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (module_name, names) in &placed_in {
        let Some(module) = by_name.get(module_name.as_str()) else {
            continue;
        };
        for (name, value) in module.dict().iter() {
            let name: String = name.extract()?;
            if names.contains(&name) || name.starts_with("__") {
                continue;
            }
            let Some(identities) = theirs.get(&name) else {
                continue;
            };
            if value.is_instance_of::<PyModule>() {
                continue;
            }
            if identities.contains(&(value.as_ptr() as usize)) {
                also.entry(module_name.clone()).or_default().insert(name);
            }
        }
    }
    Ok(Tables {
        homes,
        also,
        no_home,
    })
}

/// `homes.rs`'s text.
fn render(tables: &Tables) -> String {
    let mut text = String::from(
        "//! Where music21 keeps each function and constant this package exports.\n\
         //!\n\
         //! Generated from music21 by `cargo run -p xtask --features python -- homes`;\n\
         //! run that again rather than editing this by hand.\n\
         //!\n\
         //! [`HOMES`] is the module of music21's that defines each name\n\
         //! `provide_music21` would otherwise place nowhere. [`ALSO`] is the names\n\
         //! music21 imports into a module the package answers for from where they\n\
         //! live. [`NO_HOME`] is the package's own names, which music21 has nowhere.\n\n",
    );
    text += &table(
        "Each module of music21's and the names it defines.",
        "HOMES",
        &tables.homes,
    );
    text += "\n";
    text += &table(
        "Each module of music21's and the names it imports from where they live.",
        "ALSO",
        &tables.also,
    );
    text += "\n/// The package's names music21 has nowhere.\n#[rustfmt::skip]\n";
    text += "pub(crate) const NO_HOME: &[&str] = &[\n";
    for name in &tables.no_home {
        text += &format!("    {name:?},\n");
    }
    text + "];\n"
}

/// One table as a Rust constant.
fn table(doc: &str, name: &str, entries: &BTreeMap<String, BTreeSet<String>>) -> String {
    let mut text =
        format!("/// {doc}\n#[rustfmt::skip]\npub(crate) const {name}: &[(&str, &[&str])] = &[\n");
    for (module, names) in entries {
        text += &format!("    ({module:?}, &[\n");
        for each in names {
            text += &format!("        {each:?},\n");
        }
        text += "    ]),\n";
    }
    text + "];\n"
}

/// For each module provided: music21's public names in it, and how many of
/// them are provided without the table and with it.
fn print_coverage(py: Python<'_>, modules: &[(String, Bound<'_, PyModule>)]) -> PyResult<()> {
    let before = with_children(provided(py, false, true)?);
    let after = with_children(provided(py, true, false)?);
    let by_name: HashMap<&str, &Bound<'_, PyModule>> = modules
        .iter()
        .map(|(name, module)| (name.as_str(), module))
        .collect();
    println!(
        "{:<45} {:>7} {:>6} {:>6}",
        "module", "music21", "before", "after"
    );
    let mut totals = [0; 3];
    for (module_name, names) in &after {
        let Some(module) = by_name.get(module_name.as_str()) else {
            continue;
        };
        let public = public_names(module)?;
        let empty = BTreeSet::new();
        let counts = [
            public.len(),
            public
                .intersection(before.get(module_name).unwrap_or(&empty))
                .count(),
            public.intersection(names).count(),
        ];
        println!(
            "{module_name:<45} {:>7} {:>6} {:>6}",
            counts[0], counts[1], counts[2]
        );
        for (total, count) in totals.iter_mut().zip(counts) {
            *total += count;
        }
    }
    println!(
        "{:<45} {:>7} {:>6} {:>6}",
        "total", totals[0], totals[1], totals[2]
    );
    Ok(())
}

/// A module's `__all__`, or where it has none, its names that are not
/// private, not modules, and not imported from outside music21.
fn public_names(module: &Bound<'_, PyModule>) -> PyResult<BTreeSet<String>> {
    let listed = module
        .getattr("__all__")
        .ok()
        .filter(|all| all.is_truthy().unwrap_or(false));
    if let Some(listed) = listed {
        return listed.try_iter()?.map(|name| name?.extract()).collect();
    }
    let mut public = BTreeSet::new();
    for (name, value) in module.dict().iter() {
        let name: String = name.extract()?;
        if name.starts_with('_') || value.is_instance_of::<PyModule>() {
            continue;
        }
        let home = match value.getattr("__module__") {
            Ok(home) => home.str()?.to_string(),
            Err(_) => "music21".to_owned(),
        };
        if home.starts_with("music21") {
            public.insert(name);
        }
    }
    Ok(public)
}

/// Each module's names with its submodules', as `music21.stream` answers
/// for `iterator`.
fn with_children(
    contents: BTreeMap<String, BTreeSet<String>>,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut names = contents.clone();
    for module in contents.keys() {
        if let Some((parent, child)) = module.rsplit_once('.')
            && let Some(into) = names.get_mut(parent)
        {
            into.insert(child.to_owned());
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::assigns;

    #[test]
    fn a_constant_is_found_where_a_line_assigns_it() {
        assert!(assigns("x = 1\nNAME: int = 2\n", "NAME"));
        assert!(assigns("NAME=3", "NAME"));
        assert!(!assigns("NAMES = 1\n  NAME = 2\n", "NAME"));
        assert!(!assigns("NAME.attribute = 1\n", "NAME"));
    }
}
