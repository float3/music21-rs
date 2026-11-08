//! music21's `derivation.Derivation`: what an element was made from, and
//! how.
//!
//! Every element has one (`Music21Object.derivation`, made on first
//! asking). A copy names the element it was copied from, and a stream made
//! by a transformation -- `flatten`, `transpose`, `makeMeasures` -- names
//! the stream it was made from and the method, as music21's do; the
//! context search follows them back (`sites`). The element it belongs to
//! is held weakly and what it was made from strongly, as music21 holds
//! them: an origin lives as long as what was made from it.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use std::collections::HashSet;

use pyo3::prelude::*;
use pyo3::types::{PyList, PyWeakrefMethods, PyWeakrefReference};

/// What a derivation's element is held by: weakly, or strongly where the
/// object cannot be named by a weak reference.
enum Client {
    Weak(Py<PyWeakrefReference>),
    Strong(Py<PyAny>),
}

/// music21's `Derivation`.
#[pyclass(name = "Derivation", module = "music21.derivation")]
#[derive(Default)]
pub struct Derivation {
    client: Option<Client>,
    /// Whether an element was ever given: once it is gone, the origin goes
    /// with it, as music21's does.
    had_client: bool,
    method: Option<Py<PyAny>>,
    origin: Option<Py<PyAny>>,
}

impl Derivation {
    /// A derivation of `client` from `origin` by `method`.
    pub(crate) fn of(
        client: &Bound<'_, PyAny>,
        origin: Option<&Bound<'_, PyAny>>,
        method: Option<&str>,
    ) -> Self {
        let py = client.py();
        let mut made = Self::default();
        made.set_client(Some(client));
        made.origin = origin.map(|origin| origin.clone().unbind());
        made.method =
            method.map(|method| pyo3::types::PyString::new(py, method).into_any().unbind());
        made
    }

    /// The element, while it lives; once it is gone the origin is let go.
    fn live_client<'py>(&mut self, py: Python<'py>) -> Option<Bound<'py, PyAny>> {
        let found = match &self.client {
            Some(Client::Weak(reference)) => reference.bind(py).upgrade(),
            Some(Client::Strong(held)) => Some(held.bind(py).clone()),
            None => None,
        };
        if found.is_none() && self.had_client {
            self.client = None;
            self.had_client = false;
            self.origin = None;
        }
        found
    }

    fn set_client(&mut self, client: Option<&Bound<'_, PyAny>>) {
        self.client = client.map(|client| match PyWeakrefReference::new(client) {
            Ok(reference) => Client::Weak(reference.unbind()),
            Err(_) => Client::Strong(client.clone().unbind()),
        });
        self.had_client = self.client.is_some();
    }
}

/// What an object was made from, read without making a derivation for one
/// that has none.
pub(crate) fn origin_of<'py>(object: &Bound<'py, PyAny>) -> Option<Bound<'py, PyAny>> {
    let derivation = crate::base::derivation_held(object)?;
    let origin = derivation.getattr("origin").ok()?;
    (!origin.is_none()).then_some(origin)
}

/// What an object was made from, what that was made from, and so on back:
/// music21's `Derivation.chain`. A chain that comes round again stops.
pub(crate) fn chain_of<'py>(object: &Bound<'py, PyAny>) -> Vec<Bound<'py, PyAny>> {
    let mut seen = HashSet::from([object.as_ptr() as usize]);
    let mut found = Vec::new();
    let mut next = origin_of(object);
    while let Some(origin) = next {
        if !seen.insert(origin.as_ptr() as usize) {
            break;
        }
        next = origin_of(&origin);
        found.push(origin);
    }
    found
}

#[pymethods]
impl Derivation {
    /// music21's `Derivation(client=None)`.
    #[new]
    #[pyo3(signature = (client = None))]
    fn new(client: Option<&Bound<'_, PyAny>>) -> Self {
        let mut made = Self::default();
        made.set_client(client.filter(|client| !client.is_none()));
        made
    }

    /// The element this derivation is of, or `None` once it is gone.
    #[getter]
    fn get_client(&mut self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.live_client(py).map(Bound::unbind)
    }

    #[setter]
    fn set_client_object(&mut self, client: Option<&Bound<'_, PyAny>>) {
        self.set_client(client.filter(|client| !client.is_none()));
    }

    /// What the element was made from.
    #[getter]
    fn get_origin(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.origin.as_ref().map(|origin| origin.clone_ref(py))
    }

    #[setter]
    fn set_origin(&mut self, origin: Option<&Bound<'_, PyAny>>) {
        self.origin = origin
            .filter(|origin| !origin.is_none())
            .map(|origin| origin.clone().unbind());
    }

    /// Python's `id` of the origin.
    #[getter]
    fn originId(&self) -> Option<usize> {
        self.origin.as_ref().map(|origin| origin.as_ptr() as usize)
    }

    /// The name of the method the element was made by.
    #[getter]
    fn get_method(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.method.as_ref().map(|method| method.clone_ref(py))
    }

    #[setter]
    fn set_method(&mut self, method: Option<&Bound<'_, PyAny>>) {
        self.method = method
            .filter(|method| !method.is_none())
            .map(|method| method.clone().unbind());
    }

    /// What the element was made from, and what that was made from, back
    /// to the first.
    fn chain<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let mut found = Vec::new();
        if let Some(origin) = &self.origin {
            let origin = origin.bind(py);
            let mut rest = chain_of(origin);
            found.push(origin.clone());
            found.append(&mut rest);
        }
        Ok(PyList::new(py, found)?.try_iter()?.into_any())
    }

    /// The oldest thing in the chain, or `None` where there is none.
    #[getter]
    fn rootDerivation<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        let found: Vec<Bound<'py, PyAny>> = self.chain(py)?.try_iter()?.collect::<PyResult<_>>()?;
        Ok(found.last().cloned())
    }

    fn __repr__(&mut self, py: Python<'_>) -> PyResult<String> {
        let client = match self.live_client(py) {
            Some(client) => client.str()?.to_string(),
            None => "None".to_string(),
        };
        let origin = match &self.origin {
            Some(origin) => origin.bind(py).str()?.to_string(),
            None => "None".to_string(),
        };
        let via = match &self.method {
            Some(method) if method.bind(py).is_truthy()? => {
                format!(" via {}", method.bind(py).repr()?)
            }
            _ => String::new(),
        };
        Ok(format!("<Derivation of {client} from {origin}{via}>"))
    }

    /// The same element and origin, as music21 copies one; the method is
    /// not carried.
    #[pyo3(signature = (_memo = None))]
    fn __deepcopy__(&mut self, py: Python<'_>, _memo: Option<&Bound<'_, PyAny>>) -> Self {
        let mut made = Self::default();
        let client = self.live_client(py);
        made.set_client(client.as_ref());
        made.origin = self.origin.as_ref().map(|origin| origin.clone_ref(py));
        made
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        if let Some(Client::Strong(held)) = &self.client {
            visit.call(held)?;
        }
        visit.call(&self.method)?;
        visit.call(&self.origin)
    }

    fn __clear__(&mut self) {
        self.client = None;
        self.origin = None;
        self.method = None;
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Derivation>()?;
    Ok(())
}
