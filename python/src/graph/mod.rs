//! music21's `graph` package: `plotStream` and the modules `utilities`,
//! `primitives`, `axis`, `plot` and `findPlot`.
//!
//! What a plot shows is the crate's `graph`: the ticks and labels of each
//! axis, the plot a format and some quantities ask for, the layout numbers
//! of each primitive, and colours. Reading values out of a stream is done
//! here, over whatever stream is given, through its own methods, as
//! music21's axes read them. Drawing is matplotlib's, called from here
//! exactly as music21 calls it, so the figures are music21's; where
//! matplotlib is not installed, a graph refuses as music21's does.
//!
//! **The classes are Python classes with Rust methods.** music21's plots
//! are a graph primitive and `PlotStreamMixin` at once, and a caller sets
//! attributes of its own on them (`hist.axisX.extractOneElement = ...`),
//! so each is built with `types.new_class` on `ProtoM21Object`, its state
//! in its `__dict__` as music21's is, and each method a `#[pyfunction]`.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

pub(crate) mod axis;
pub(crate) mod findplot;
pub(crate) mod plot;
pub(crate) mod primitives;
pub(crate) mod utilities;

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyModule, PyTuple};

use crate::plumbing::built::{method, new_class, static_method};

/// The package.
pub(crate) const PACKAGE: &str = "music21.graph";

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

/// Keyword arguments for a call.
macro_rules! kwargs {
    ($py:expr; $($key:expr => $value:expr),* $(,)?) => {{
        let made = pyo3::types::PyDict::new($py);
        $(made.set_item($key, $value)?;)*
        made
    }};
}
pub(crate) use kwargs;

/// One of `graph`'s modules, built on first asking.
pub(crate) fn module<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyModule>> {
    match name {
        PACKAGE => package(py),
        utilities::MODULE => utilities::module(py),
        primitives::MODULE => primitives::module(py),
        axis::MODULE => axis::module(py),
        plot::MODULE => plot::module(py),
        findplot::MODULE => findplot::module(py),
        _ => Err(pyo3::exceptions::PyKeyError::new_err(name.to_owned())),
    }
}

/// A name of music21's, from wherever `import music21` is answered.
pub(crate) fn music21_name<'py>(
    py: Python<'py>,
    module: &str,
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    crate::subconverters::music21_name(py, module, name)
}

/// `getattr(object, name, None)`, `None` given as nothing.
pub(crate) fn optional<'py>(
    object: &Bound<'py, PyAny>,
    name: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    match object.getattr(name) {
        Ok(value) if value.is_none() => Ok(None),
        Ok(value) => Ok(Some(value)),
        Err(error) if error.is_instance_of::<pyo3::exceptions::PyAttributeError>(object.py()) => {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

/// Whether a keyword was given.
pub(crate) fn given(keywords: Option<&Bound<'_, PyDict>>, name: &str) -> PyResult<bool> {
    match keywords {
        Some(keywords) => keywords.contains(name),
        None => Ok(false),
    }
}

/// The keywords given, as a dict of their own.
pub(crate) fn keywords_of<'py>(
    py: Python<'py>,
    keywords: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyDict>> {
    match keywords {
        Some(keywords) => keywords.copy(),
        None => Ok(PyDict::new(py)),
    }
}

/// What a class is built from: its bases, class attributes, methods,
/// static methods and properties.
pub(crate) struct ClassSpec<'py> {
    pub(crate) name: &'static str,
    pub(crate) doc: &'static str,
    pub(crate) bases: Vec<Bound<'py, PyAny>>,
    pub(crate) attributes: Vec<(&'static str, Bound<'py, PyAny>)>,
    pub(crate) methods: Vec<(&'static str, Bound<'py, PyAny>)>,
    pub(crate) statics: Vec<(&'static str, Bound<'py, PyAny>)>,
    pub(crate) properties: Vec<Property<'py>>,
}

/// A property: its getter and, where it can be set, its setter.
pub(crate) struct Property<'py> {
    pub(crate) name: &'static str,
    pub(crate) getter: Bound<'py, PyAny>,
    pub(crate) setter: Option<Bound<'py, PyAny>>,
}

impl<'py> ClassSpec<'py> {
    /// A class with nothing in it yet.
    pub(crate) fn new(
        name: &'static str,
        doc: &'static str,
        bases: Vec<Bound<'py, PyAny>>,
    ) -> Self {
        Self {
            name,
            doc,
            bases,
            attributes: Vec::new(),
            methods: Vec::new(),
            statics: Vec::new(),
            properties: Vec::new(),
        }
    }

    /// The class, built in `module`.
    pub(crate) fn build(self, module: &str) -> PyResult<Bound<'py, PyAny>> {
        let py = self
            .bases
            .first()
            .map(Bound::py)
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("a class needs a base"))?;
        let mut members: Vec<(&str, Bound<'py, PyAny>)> = self.attributes;
        for (name, function) in self.methods {
            members.push((name, method(function, module, self.name, name)?));
        }
        for (name, function) in self.statics {
            members.push((name, static_method(function, module, self.name, name)?));
        }
        let property = py.import("builtins")?.getattr("property")?;
        for made in self.properties {
            let getter = method(made.getter, module, self.name, made.name)?;
            let doc = getter.getattr("__doc__")?;
            let setter = match made.setter {
                Some(setter) => method(setter, module, self.name, made.name)?,
                None => py.None().into_bound(py),
            };
            members.push((made.name, property.call1((getter, setter, py.None(), doc))?));
        }
        new_class(py, module, self.name, self.doc, self.bases, members)
    }
}

/// `ProtoM21Object`, which every class here is built on.
pub(crate) fn proto(py: Python<'_>) -> Bound<'_, PyAny> {
    py.get_type::<crate::prebase::ProtoM21Object>().into_any()
}

/// A built module's attribute.
pub(crate) fn built<'py>(py: Python<'py>, module: &str, name: &str) -> PyResult<Bound<'py, PyAny>> {
    self::module(py, module)?.getattr(name)
}

/// The two items of a pair, a list or a tuple, as Python unpacks one.
pub(crate) fn pair<'py>(
    items: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let [first, second] = unpacked::<2>(items)?;
    Ok((first, second))
}

/// The items of a sequence of exactly `N`, as Python unpacks one.
pub(crate) fn unpacked<'py, const N: usize>(
    items: &Bound<'py, PyAny>,
) -> PyResult<[Bound<'py, PyAny>; N]> {
    let found: Vec<Bound<'py, PyAny>> = items.try_iter()?.collect::<PyResult<_>>()?;
    let count = found.len();
    found.try_into().map_err(|_| {
        let message = if count > N {
            format!("too many values to unpack (expected {N})")
        } else {
            format!("not enough values to unpack (expected {N}, got {count})")
        };
        pyo3::exceptions::PyValueError::new_err(message)
    })
}

/// An object's state for `copy` and `pickle`: its attributes, as an
/// object with no Rust state of its own is copied.
#[pyfunction]
#[pyo3(name = "__getstate__")]
pub(crate) fn state_of<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    slf.getattr("__dict__")?.cast::<PyDict>()?.copy()
}

/// A tuple of strings.
pub(crate) fn strings<'py>(py: Python<'py>, items: &[&str]) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyTuple::new(py, items)?.into_any())
}

/// Shows a plot of a stream: the plots `findPlot.getPlotsToMake` finds
/// for the format and values, each made, given its axes where it needs
/// them, and run; the last is answered.
#[pyfunction]
#[pyo3(signature = (streamObj, graphFormat=None, xValue=None, yValue=None, zValue=None, **keywords))]
fn plotStream<'py>(
    streamObj: &Bound<'py, PyAny>,
    graphFormat: Option<&Bound<'py, PyAny>>,
    xValue: Option<&Bound<'py, PyAny>>,
    yValue: Option<&Bound<'py, PyAny>>,
    zValue: Option<&Bound<'py, PyAny>>,
    keywords: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = streamObj.py();
    let make = built(py, findplot::MODULE, "getPlotsToMake")?.call1((
        graphFormat,
        xValue,
        yValue,
        zValue,
    ))?;
    let mut made = py.None().into_bound(py);
    for info in make.try_iter()? {
        let info = info?;
        let (class, axes) = if info.is_instance_of::<PyTuple>() {
            (info.get_item(0)?, Some(info.get_item(1)?))
        } else {
            (info, None)
        };
        made = class.call((streamObj,), keywords)?;
        if let Some(axes) = axes.filter(|axes| axes.is_truthy().unwrap_or(false)) {
            for item in axes.call_method0("items")?.try_iter()? {
                let (letter, axis_class): (String, Bound<'py, PyAny>) = item?.extract()?;
                let attribute = format!("axis{}", letter.to_uppercase());
                made.setattr(attribute, axis_class.call1((&made, letter))?)?;
            }
        }
        made.call_method0("run")?;
    }
    Ok(made)
}

/// `music21.graph`, built: `plotStream` and the five modules.
fn package(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, PACKAGE)?;
        m.add(
            "__all__",
            [
                "axis",
                "findPlot",
                "plot",
                "plotStream",
                "primitives",
                "utilities",
            ],
        )?;
        m.add("utilities", utilities::module(py)?)?;
        m.add("primitives", primitives::module(py)?)?;
        m.add("axis", axis::module(py)?)?;
        m.add("plot", plot::module(py)?)?;
        m.add("findPlot", findplot::module(py)?)?;
        m.add_function(wrap_pyfunction!(plotStream, &m)?)?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}
