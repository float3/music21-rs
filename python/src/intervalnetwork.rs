//! music21's `scale.intervalNetwork` over the crate's
//! [`music21::scale::intervalnetwork`]: `IntervalNetwork`, `Node`, `Edge`
//! and the enums `Direction` and `Terminus`.
//!
//! The enums are Python `enum.Enum`s, as music21's are, so code comparing
//! against `Direction.ASCENDING` or iterating the members works unchanged.
//! pyo3 cannot build a class on `enum.Enum`, so each is made with the enum
//! module's functional form, with the `repr` written here.
//!
//! A network's `nodes` and `edges` are built afresh from the crate's
//! network on each reading: changing a node read from one does not change
//! the network.

#![allow(non_snake_case)]

use pyo3::exceptions::{PyAttributeError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyList, PyModule, PyTuple};

use music21::Pitch as RsPitch;
use music21::scale::intervalnetwork::{
    DescendingWalk, Direction as RsDirection, Draw, Edge as RsEdge, EdgeSpec,
    IntervalNetwork as RsNetwork, Moduli, Neighbor, Node as RsNode, NodeId, NodeName, Octaves,
    Order, Pathways, PitchRange, Realized, Termini, Terminus, pitch_bounds,
};
use music21::scale::{DegreeComparison, Simplification};

use crate::interval::{interval_from_any, interval_object};
use crate::pitch::{Pitch, pitch_from_any};
use crate::scale::IntervalNetworkException;

/// The names this module hands out.
pub const NAMES: &[&str] = &[
    "Direction",
    "Terminus",
    "Node",
    "Edge",
    "IntervalNetwork",
    "BoundIntervalNetwork",
];

/// The module music21 keeps these classes in.
const MODULE: &str = "music21.scale.intervalNetwork";

pyo3::create_exception!(music21_rs_facade, EdgeException, crate::Music21Exception);

error_into!(network_error, IntervalNetworkException);
error_into!(edge_error, EdgeException);

/// The `Terminus` and `Direction` classes, made once at registration.
static TERMINUS: PyOnceLock<Py<PyAny>> = PyOnceLock::new();
static DIRECTION: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

/// `Direction.ASCENDING` and the like: the class's name and the member's.
#[pyfunction]
fn member_name(member: &Bound<'_, PyAny>) -> PyResult<String> {
    let class = member.get_type().name()?.to_string();
    let name: String = member.getattr("name")?.extract()?;
    Ok(format!("{class}.{name}"))
}

/// An enum of music21's, its members and their values in music21's order.
fn made_enum<'py>(
    m: &Bound<'py, PyModule>,
    name: &str,
    members: &[(&str, &str)],
    doc: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py = m.py();
    let keywords = PyDict::new(py);
    keywords.set_item("module", MODULE)?;
    keywords.set_item("qualname", name)?;
    let class = py
        .import("enum")?
        .getattr("Enum")?
        .call((name, members.to_vec()), Some(&keywords))?;
    let shown = py
        .import("functools")?
        .getattr("partialmethod")?
        .call1((wrap_pyfunction!(member_name, m)?,))?;
    class.setattr("__repr__", &shown)?;
    class.setattr("__str__", &shown)?;
    class.setattr("__doc__", doc)?;
    Ok(class)
}

/// A member of one of the two enums, by name.
fn member<'py>(
    py: Python<'py>,
    class: &PyOnceLock<Py<PyAny>>,
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let Some(class) = class.get(py) else {
        return Err(PyAttributeError::new_err(
            "music21.scale.intervalNetwork is not registered",
        ));
    };
    let class = class.bind(py);

    // Installed, music21's code compares against its own enum, which never
    // equals the package's: hand out music21's member where it has one.
    if crate::music21_installed(py)
        && let Ok(real) = crate::real_music21(py, MODULE)
        && let Ok(theirs) = real.getattr(class.getattr("__name__")?.extract::<String>()?)
        && !theirs.is(class)
    {
        return theirs.getattr(name);
    }
    class.getattr(name)
}

/// The value an enum member carries, which music21's own members share.
fn member_value(value: &Bound<'_, PyAny>) -> Option<String> {
    value.getattr("value").ok()?.extract().ok()
}

// Node ids, node names and directions between Python and the crate.

fn id_to_py(py: Python<'_>, id: NodeId) -> PyResult<Py<PyAny>> {
    Ok(match id {
        NodeId::Terminus(Terminus::Low) => member(py, &TERMINUS, "LOW")?.unbind(),
        NodeId::Terminus(Terminus::High) => member(py, &TERMINUS, "HIGH")?.unbind(),
        NodeId::Index(index) => index.into_pyobject(py)?.into_any().unbind(),
    })
}

fn ids_to_py(py: Python<'_>, ids: &[NodeId]) -> PyResult<Vec<Py<PyAny>>> {
    ids.iter().map(|id| id_to_py(py, *id)).collect()
}

/// A node id as music21 prints one.
fn id_repr(id: NodeId) -> String {
    match id {
        NodeId::Terminus(Terminus::Low) => "Terminus.LOW".to_string(),
        NodeId::Terminus(Terminus::High) => "Terminus.HIGH".to_string(),
        NodeId::Index(index) => index.to_string(),
    }
}

/// A node id given as a `Terminus` member, a number or a `Node`.
fn id_from_py(value: &Bound<'_, PyAny>) -> PyResult<NodeId> {
    if let Ok(node) = value.extract::<PyRef<'_, Node>>() {
        return Ok(node.inner.id);
    }
    match member_value(value).as_deref() {
        Some("terminusLow") => return Ok(NodeId::LOW),
        Some("terminusHigh") => return Ok(NodeId::HIGH),
        _ => {}
    }
    if let Ok(index) = value.extract::<usize>() {
        return Ok(NodeId::Index(index));
    }
    Err(IntervalNetworkException::new_err(format!(
        "cannot filter by: {}",
        value.str()?
    )))
}

/// A node named as music21 names one for a walk: a `Node`, a degree, a
/// `Terminus` member, or `None` for the low terminus.
fn name_from_py(value: Option<&Bound<'_, PyAny>>) -> PyResult<Option<NodeName>> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(None);
    };
    name_given(value).map(Some)
}

/// A node name that has to be there: music21 refuses `None` where it looks
/// the name up itself.
fn name_required(value: Option<&Bound<'_, PyAny>>) -> PyResult<NodeName> {
    match value.filter(|value| !value.is_none()) {
        Some(value) => name_given(value),
        None => Err(IntervalNetworkException::new_err("cannot filter by: None")),
    }
}

fn name_given(value: &Bound<'_, PyAny>) -> PyResult<NodeName> {
    if value.is_instance_of::<pyo3::types::PyString>() {
        return Err(IntervalNetworkException::new_err(format!(
            "Strings like {} are no longer valid nodeIds.",
            value.repr()?
        )));
    }
    if value.extract::<PyRef<'_, Node>>().is_err()
        && member_value(value).is_none()
        && let Ok(degree) = value.extract::<i32>()
    {
        return Ok(NodeName::Degree(degree));
    }
    id_from_py(value).map(NodeName::Id)
}

/// The wheel's own `Direction` member of this name, such as `"ASCENDING"`.
pub(crate) fn direction_member<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    member(py, &DIRECTION, name)
}

fn direction_to_py(py: Python<'_>, direction: RsDirection) -> PyResult<Bound<'_, PyAny>> {
    let name = match direction {
        RsDirection::Bi => "BI",
        RsDirection::Ascending => "ASCENDING",
        RsDirection::Descending => "DESCENDING",
    };
    member(py, &DIRECTION, name)
}

/// A `Direction` member, or ascending for none.
fn direction_from_py(value: Option<&Bound<'_, PyAny>>) -> PyResult<RsDirection> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(RsDirection::Ascending);
    };
    match member_value(value).as_deref() {
        Some("bi") => Ok(RsDirection::Bi),
        Some("ascending") => Ok(RsDirection::Ascending),
        Some("descending") => Ok(RsDirection::Descending),
        _ => Err(IntervalNetworkException::new_err(format!(
            "cannot match direction specification: {}",
            value.repr()?
        ))),
    }
}

/// music21's `comparisonAttribute`, as the crate's comparison.
fn comparison_from(attribute: &str) -> PyResult<DegreeComparison> {
    DegreeComparison::from_music21_name(attribute).ok_or_else(|| {
        PyAttributeError::new_err(format!("'Pitch' object has no attribute '{attribute}'"))
    })
}

fn optional_pitch(value: Option<&Bound<'_, PyAny>>) -> PyResult<Option<RsPitch>> {
    value
        .filter(|value| !value.is_none())
        .map(pitch_from_any)
        .transpose()
}

fn range_from(
    min: Option<&Bound<'_, PyAny>>,
    max: Option<&Bound<'_, PyAny>>,
) -> PyResult<PitchRange> {
    Ok(PitchRange {
        min: optional_pitch(min)?,
        max: optional_pitch(max)?,
    })
}

fn wrap(pitch: RsPitch) -> Pitch {
    let inferred = pitch.spelling_is_inferred();
    Pitch::wrap(pitch, inferred)
}

fn wrap_all(pitches: Vec<RsPitch>) -> Vec<Pitch> {
    pitches.into_iter().map(wrap).collect()
}

/// A realization as music21 hands one back: its pitches and node ids.
fn realized_to_py(py: Python<'_>, realized: Realized) -> PyResult<(Vec<Pitch>, Vec<Py<PyAny>>)> {
    let ids = ids_to_py(py, &realized.nodes)?;
    Ok((wrap_all(realized.pitches), ids))
}

/// Runs a crate call with draws from `generator`, or from Python's
/// `random.random` where none is given, and raises what either raised.
fn drawing<T>(
    py: Python<'_>,
    generator: Option<&Bound<'_, PyAny>>,
    call: impl FnOnce(&mut Draw<'_>) -> music21::Result<T>,
) -> PyResult<T> {
    let generator = generator.filter(|value| !value.is_none());
    let mut failure: Option<PyErr> = None;
    let result = {
        let mut draw = || {
            let drawn = match generator {
                Some(generator) => generator.call0(),
                None => py
                    .import("random")
                    .and_then(|random| random.call_method0("random")),
            };
            match drawn.and_then(|value| value.extract::<f64>()) {
                Ok(value) => value,
                Err(error) => {
                    failure.get_or_insert(error);
                    0.0
                }
            }
        };
        call(&mut draw)
    };
    if let Some(error) = failure {
        return Err(error);
    }
    result.map_err(network_error)
}

/// The pitches of a `pitchTarget` argument: each object as given, a name
/// read into a new `Pitch`, and the crate's value of each. One pitch alone
/// is a list of one.
fn target_pitches<'py>(
    target: &Bound<'py, PyAny>,
) -> PyResult<(Vec<Bound<'py, PyAny>>, Vec<RsPitch>)> {
    let py = target.py();
    let given: Vec<Bound<'py, PyAny>> =
        if target.is_instance_of::<PyList>() || target.is_instance_of::<PyTuple>() {
            target.try_iter()?.collect::<PyResult<_>>()?
        } else {
            vec![target.clone()]
        };
    let mut objects = Vec::with_capacity(given.len());
    let mut pitches = Vec::with_capacity(given.len());
    for value in given {
        let pitch = pitch_from_any(&value)?;
        if value.is_instance_of::<pyo3::types::PyString>() {
            objects.push(Bound::new(py, wrap(pitch.clone()))?.into_any());
        } else {
            objects.push(value);
        }
        pitches.push(pitch);
    }
    Ok((objects, pitches))
}

fn at_least_one(pitches: &[RsPitch]) -> PyResult<()> {
    if pitches.is_empty() {
        return Err(PyValueError::new_err(
            "There must be at least one pitch given.",
        ));
    }
    Ok(())
}

/// `getNext`'s answer: the edges leaving a node and the node each reaches.
type NextSteps = (Vec<Py<Edge>>, Vec<Py<Node>>);

/// `getNeighborNodeIds`'s answer: the ids below and above a pitch.
type Neighbors = Option<(Option<Py<PyAny>>, Py<PyAny>)>;

/// `filterPitchList`'s answer: the pitches, the lowest and the highest.
type Bounded<'py> = (Vec<Bound<'py, PyAny>>, Bound<'py, PyAny>, Bound<'py, PyAny>);

/// `match`'s answer: the pitches found and those not.
type Split<'py> = (Vec<Bound<'py, PyAny>>, Vec<Bound<'py, PyAny>>);

/// music21's `Node`: a pitch the network does not hold yet.
#[pyclass(
    name = "Node",
    module = "music21.scale.intervalNetwork",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct Node {
    inner: RsNode,
}

#[pymethods]
impl Node {
    #[new]
    #[pyo3(signature = (id, degree, weight = 1.0))]
    fn new(id: &Bound<'_, PyAny>, degree: i32, weight: f64) -> PyResult<Self> {
        let mut inner = RsNode::new(id_from_py(id)?, degree);
        inner.weight = weight;
        Ok(Self { inner })
    }

    #[getter]
    fn id(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        id_to_py(py, self.inner.id)
    }

    #[setter]
    fn set_id(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.inner.id = id_from_py(value)?;
        Ok(())
    }

    #[getter]
    fn degree(&self) -> i32 {
        self.inner.degree
    }

    #[setter]
    fn set_degree(&mut self, value: i32) {
        self.inner.degree = value;
    }

    #[getter]
    fn weight(&self) -> f64 {
        self.inner.weight
    }

    #[setter]
    fn set_weight(&mut self, value: f64) {
        self.inner.weight = value;
    }

    /// Nodes are equal when their id, degree and weight are.
    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Node>>()
            .is_ok_and(|other| other.inner == self.inner)
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.inner.id.hash(&mut hasher);
        self.inner.degree.hash(&mut hasher);
        self.inner.weight.to_bits().hash(&mut hasher);
        hasher.finish()
    }

    fn __repr__(&self) -> String {
        format!("<{MODULE}.Node id={}>", id_repr(self.inner.id))
    }
}

/// music21's `Edge`: an interval between two nodes, and the way it is
/// walked.
#[pyclass(
    name = "Edge",
    module = "music21.scale.intervalNetwork",
    skip_from_py_object
)]
pub struct Edge {
    inner: RsEdge,
    /// The interval as given, which music21 hands back as itself.
    interval: Py<PyAny>,
    /// The id as given: a number, or whatever else music21 would keep.
    id: Py<PyAny>,
}

impl Edge {
    /// An edge of a network, as a Python object.
    fn object(py: Python<'_>, edge: &RsEdge) -> PyResult<Py<Edge>> {
        let id = match edge.id {
            Some(id) => id.into_pyobject(py)?.into_any().unbind(),
            None => py.None(),
        };
        let interval = interval_object(py, edge.interval.clone())?;
        Py::new(
            py,
            Self {
                inner: edge.clone(),
                interval,
                id,
            },
        )
    }

    fn connections_to_py(&self, py: Python<'_>, pairs: &[(NodeId, NodeId)]) -> PyResult<Py<PyAny>> {
        let mut tuples = Vec::with_capacity(pairs.len());
        for (from, to) in pairs {
            tuples.push(PyTuple::new(
                py,
                [id_to_py(py, *from)?, id_to_py(py, *to)?],
            )?);
        }
        Ok(PyList::new(py, tuples)?.into_any().unbind())
    }
}

#[pymethods]
impl Edge {
    #[new]
    #[pyo3(signature = (intervalData, id = None, direction = None))]
    fn new(
        py: Python<'_>,
        intervalData: &Bound<'_, PyAny>,
        id: Option<&Bound<'_, PyAny>>,
        direction: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let interval = interval_from_any(intervalData)?;
        let object = if intervalData.is_instance_of::<pyo3::types::PyString>() {
            interval_object(py, interval.clone())?
        } else {
            intervalData.clone().unbind()
        };
        let id = id.map_or_else(|| py.None(), |id| id.clone().unbind());
        let key = id.bind(py).extract::<usize>().ok();
        let mut inner = RsEdge::new(interval, key);
        inner.direction = match direction {
            Some(direction) => direction_from_py(Some(direction))?,
            None => RsDirection::Bi,
        };
        Ok(Self {
            inner,
            interval: object,
            id,
        })
    }

    #[getter]
    fn interval(&self, py: Python<'_>) -> Py<PyAny> {
        self.interval.clone_ref(py)
    }

    #[setter]
    fn set_interval(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.inner.interval = interval_from_any(value)?;
        self.interval = value.clone().unbind();
        Ok(())
    }

    #[getter]
    fn direction<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        direction_to_py(py, self.inner.direction)
    }

    #[setter]
    fn set_direction(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.inner.direction = direction_from_py(Some(value))?;
        Ok(())
    }

    #[getter]
    fn weight(&self) -> f64 {
        self.inner.weight
    }

    #[setter]
    fn set_weight(&mut self, value: f64) {
        self.inner.weight = value;
    }

    #[getter]
    fn id(&self, py: Python<'_>) -> Py<PyAny> {
        self.id.clone_ref(py)
    }

    #[setter]
    fn set_id(&mut self, value: &Bound<'_, PyAny>) {
        self.inner.id = value.extract::<usize>().ok();
        self.id = value.clone().unbind();
    }

    /// Connects two nodes, or node ids, the first to the second, walked
    /// `direction`, which has to be one way.
    #[pyo3(signature = (node1, node2, direction = None))]
    fn addDirectedConnection(
        &mut self,
        node1: &Bound<'_, PyAny>,
        node2: &Bound<'_, PyAny>,
        direction: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let (from, to) = (id_from_py(node1)?, id_from_py(node2)?);
        let direction = match direction.filter(|value| !value.is_none()) {
            Some(direction) => direction_from_py(Some(direction))?,
            None => RsDirection::Bi,
        };
        self.inner.connect(from, to, direction).map_err(edge_error)
    }

    /// Connects two nodes up from the first and back down.
    fn addBiDirectedConnections(
        &mut self,
        node1: &Bound<'_, PyAny>,
        node2: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.inner
            .connect_both(id_from_py(node1)?, id_from_py(node2)?);
        Ok(())
    }

    /// The pairs of node ids this edge connects walked `direction`, or its
    /// own way when none is given.
    #[pyo3(signature = (direction = None))]
    fn getConnections(
        &self,
        py: Python<'_>,
        direction: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let direction = match direction.filter(|value| !value.is_none()) {
            Some(direction) => direction_from_py(Some(direction))?,
            None => self.inner.direction,
        };
        let pairs = self
            .inner
            .connections_toward(direction)
            .map_err(edge_error)?;
        self.connections_to_py(py, &pairs)
    }

    #[getter]
    fn connections(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.getConnections(py, None)
    }

    /// Edges are equal when everything they hold is.
    fn __eq__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let Ok(other) = other.extract::<PyRef<'_, Edge>>() else {
            return Ok(false);
        };
        Ok(other.inner == self.inner && other.id.bind(py).eq(self.id.bind(py))?)
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let pairs: Vec<String> = self
            .inner
            .connections()
            .iter()
            .map(|(from, to)| format!("({}, {})", id_repr(*from), id_repr(*to)))
            .collect();
        let direction = direction_to_py(py, self.inner.direction)?
            .str()?
            .to_string();
        let name: String = self.interval.bind(py).getattr("name")?.extract()?;
        Ok(format!(
            "<{MODULE}.Edge {direction} {name} [{}]>",
            pairs.join(", ")
        ))
    }
}

/// music21's `IntervalNetwork`: a scale or a chord as a graph of
/// intervals.
#[pyclass(
    name = "IntervalNetwork",
    module = "music21.scale.intervalNetwork",
    subclass,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct IntervalNetwork {
    pub(crate) inner: RsNetwork,
}

impl IntervalNetwork {
    /// A network holding `inner`, as a Python object.
    pub(crate) fn object(py: Python<'_>, inner: RsNetwork) -> PyResult<Py<PyAny>> {
        Ok(Py::new(py, Self { inner })?.into_any())
    }

    fn node_object(&self, py: Python<'_>, id: NodeId) -> PyResult<Py<Node>> {
        let Some(node) = self.inner.node(id) else {
            return Err(pyo3::exceptions::PyKeyError::new_err(id_repr(id)));
        };
        Py::new(
            py,
            Node {
                inner: node.clone(),
            },
        )
    }

    fn node_objects(&self, py: Python<'_>, ids: &[NodeId]) -> PyResult<Vec<Py<Node>>> {
        ids.iter().map(|id| self.node_object(py, *id)).collect()
    }

    fn intervals(list: &Bound<'_, PyAny>) -> PyResult<Vec<music21::Interval>> {
        list.try_iter()?
            .map(|step| interval_from_any(&step?))
            .collect()
    }

    fn descending(
        &self,
        py: Python<'_>,
        reference: &Bound<'_, PyAny>,
        nodeId: Option<&Bound<'_, PyAny>>,
        range: &PitchRange,
        walk: DescendingWalk,
    ) -> PyResult<(Vec<Pitch>, Vec<Py<PyAny>>)> {
        let reference = pitch_from_any(reference)?;
        let start = name_from_py(nodeId)?;
        let realized = drawing(py, None, |draw| {
            self.inner
                .realize_descending(&reference, start, range, walk, draw)
        })?;
        realized_to_py(py, realized)
    }
}

#[pymethods]
impl IntervalNetwork {
    #[new]
    #[pyo3(signature = (
        edgeList = None,
        octaveDuplicating = false,
        deterministic = true,
        pitchSimplification = Some("maxAccidental".to_string()),
    ))]
    fn new(
        edgeList: Option<&Bound<'_, PyAny>>,
        #[pyo3(from_py_with = crate::truthy)] octaveDuplicating: bool,
        #[pyo3(from_py_with = crate::truthy)] deterministic: bool,
        pitchSimplification: Option<String>,
    ) -> PyResult<Self> {
        let mut network = Self {
            inner: RsNetwork::default(),
        };
        if let Some(steps) = edgeList.filter(|value| !value.is_none()) {
            network.inner.fill_bidirected(&Self::intervals(steps)?);
        }
        network.set_octaveDuplicating(octaveDuplicating);
        network.set_deterministic(deterministic);
        network.set_pitchSimplification(pitchSimplification)?;
        Ok(network)
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> Self {
        self.clone()
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, IntervalNetwork>>()
            .is_ok_and(|other| other.inner == self.inner)
    }

    #[getter]
    fn edgeIdCount(&self) -> usize {
        self.inner.edge_id_count()
    }

    #[getter]
    fn nodeIdCount(&self) -> usize {
        self.inner.node_id_count()
    }

    /// The nodes by id, built afresh on each reading.
    #[getter]
    fn nodes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let ordered = py.import("collections")?.getattr("OrderedDict")?.call0()?;
        for node in self.inner.nodes() {
            ordered.set_item(
                id_to_py(py, node.id)?,
                Node {
                    inner: node.clone(),
                },
            )?;
        }
        Ok(ordered)
    }

    /// The edges by id, built afresh on each reading.
    #[getter]
    fn edges<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let ordered = py.import("collections")?.getattr("OrderedDict")?.call0()?;
        for (index, edge) in self.inner.edges().iter().enumerate() {
            ordered.set_item(index, Edge::object(py, edge)?)?;
        }
        Ok(ordered)
    }

    #[getter]
    fn octaveDuplicating(&self) -> bool {
        self.inner.octaves == Octaves::Duplicating
    }

    #[setter]
    fn set_octaveDuplicating(&mut self, value: bool) {
        self.inner.octaves = if value {
            Octaves::Duplicating
        } else {
            Octaves::Distinct
        };
    }

    #[getter]
    fn deterministic(&self) -> bool {
        self.inner.pathways == Pathways::Deterministic
    }

    #[setter]
    fn set_deterministic(&mut self, value: bool) {
        self.inner.pathways = if value {
            Pathways::Deterministic
        } else {
            Pathways::Weighted
        };
    }

    /// How pitches are respelled walking the network: `'maxAccidental'`,
    /// `'simplifyEnharmonic'`, `'mostCommon'`, or `None`.
    #[getter]
    fn pitchSimplification(&self) -> Option<&'static str> {
        self.inner.simplification.music21_name()
    }

    #[setter]
    fn set_pitchSimplification(&mut self, value: Option<String>) -> PyResult<()> {
        let Some(rule) = Simplification::from_music21_name(value.as_deref()) else {
            return Err(IntervalNetworkException::new_err(format!(
                "unknown pitchSimplification type {}, allowable values are \"maxAccidental\" \
                 (default), \"simplifyEnharmonic\", \"mostCommon\", or None (or \"none\")",
                value.unwrap_or_default()
            )));
        };
        self.inner.simplification = rule;
        Ok(())
    }

    /// Removes every node and edge.
    fn clear(&mut self) {
        self.inner.clear();
    }

    /// Replaces the network with a chain of steps walked both ways.
    fn fillBiDirectedEdges(&mut self, edgeList: &Bound<'_, PyAny>) -> PyResult<()> {
        self.inner.fill_bidirected(&Self::intervals(edgeList)?);
        Ok(())
    }

    /// Replaces the network with one chain walked up and another, given
    /// rising, walked down.
    fn fillDirectedEdges(
        &mut self,
        ascendingEdgeList: &Bound<'_, PyAny>,
        descendingEdgeList: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let ascending = Self::intervals(ascendingEdgeList)?;
        let descending = Self::intervals(descendingEdgeList)?;
        self.inner
            .fill_directed(&ascending, &descending)
            .map_err(network_error)
    }

    /// Replaces the network with nodes given as dictionaries of `id`,
    /// `degree` and `weight`, and edges as dictionaries of `interval` and
    /// `connections`, each connection two node ids and a `Direction`.
    fn fillArbitrary(
        &mut self,
        nodes: &Bound<'_, PyAny>,
        edges: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let mut built = Vec::new();
        for node in nodes.try_iter()? {
            let node = node?;
            let mut read = RsNode::new(
                id_from_py(&node.get_item("id")?)?,
                node.get_item("degree")?.extract()?,
            );
            if node.contains("weight")? {
                read.weight = node.get_item("weight")?.extract()?;
            }
            built.push(read);
        }
        let mut specs = Vec::new();
        for edge in edges.try_iter()? {
            let edge = edge?;
            let mut connections = Vec::new();
            for connection in edge.get_item("connections")?.try_iter()? {
                let connection = connection?;
                connections.push((
                    id_from_py(&connection.get_item(0)?)?,
                    id_from_py(&connection.get_item(1)?)?,
                    direction_from_py(Some(&connection.get_item(2)?))?,
                ));
            }
            specs.push(EdgeSpec {
                interval: interval_from_any(&edge.get_item("interval")?)?,
                connections,
            });
        }
        self.inner
            .fill_arbitrary(built, specs)
            .map_err(network_error)
    }

    /// Replaces the network with a melodic minor.
    fn fillMelodicMinor(&mut self) -> PyResult<()> {
        self.inner.fill_melodic_minor().map_err(network_error)
    }

    /// One edge and its node from parallel lists, chosen by the nodes'
    /// weights, drawing from `randomGenerator` if given.
    #[pyo3(signature = (edges, nodes, *, randomGenerator = None))]
    fn weightedSelection<'py>(
        &self,
        edges: &Bound<'py, PyAny>,
        nodes: &Bound<'py, PyAny>,
        randomGenerator: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
        let py = edges.py();
        let mut weights = Vec::new();
        for node in nodes.try_iter()? {
            weights.push(node?.getattr("weight")?.extract::<f64>()?);
        }
        let index = drawing(py, randomGenerator, |draw| {
            music21::common::numbertools::weighted_index(&weights, draw())
        })?;
        Ok((edges.get_item(index)?, nodes.get_item(index)?))
    }

    #[getter]
    fn degreeMin(&self) -> Option<i32> {
        self.inner.degree_min()
    }

    #[getter]
    fn degreeMax(&self) -> Option<i32> {
        self.inner.degree_max()
    }

    #[getter]
    fn degreeMaxUnique(&self) -> Option<i32> {
        self.inner.degree_max_unique()
    }

    #[getter]
    fn terminusLowNodes(&self, py: Python<'_>) -> PyResult<Vec<Py<Node>>> {
        Ok(vec![self.node_object(py, NodeId::LOW)?])
    }

    #[getter]
    fn terminusHighNodes(&self, py: Python<'_>) -> PyResult<Vec<Py<Node>>> {
        Ok(vec![self.node_object(py, NodeId::HIGH)?])
    }

    /// Each node id and its degree, the high terminus taking the low one's
    /// if `equateTermini`.
    #[pyo3(signature = (equateTermini = true))]
    fn getNodeDegreeDictionary<'py>(
        &self,
        py: Python<'py>,
        #[pyo3(from_py_with = crate::truthy)] equateTermini: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let termini = if equateTermini {
            Termini::Equate
        } else {
            Termini::Distinct
        };
        let ordered = py.import("collections")?.getattr("OrderedDict")?.call0()?;
        for (id, degree) in self.inner.node_degrees(termini) {
            ordered.set_item(id_to_py(py, id)?, degree)?;
        }
        Ok(ordered)
    }

    /// The degree of the node with this id.
    fn nodeIdToDegree(&self, nId: &Bound<'_, PyAny>) -> PyResult<i32> {
        self.inner
            .degree_of(id_from_py(nId)?)
            .map_err(|_| pyo3::exceptions::PyKeyError::new_err(nId.clone().unbind()))
    }

    /// The direction of every edge meeting a node.
    fn nodeIdToEdgeDirections<'py>(
        &self,
        py: Python<'py>,
        nId: &Bound<'py, PyAny>,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let directions = self
            .inner
            .edge_directions(id_from_py(nId)?)
            .map_err(network_error)?;
        directions
            .into_iter()
            .map(|direction| direction_to_py(py, direction))
            .collect()
    }

    /// A degree folded into the network's span.
    fn degreeModulus(&self, degree: Option<i32>) -> PyResult<i32> {
        let Some(degree) = degree else {
            return Err(IntervalNetworkException::new_err(
                "Degree of None given to degreeModulus",
            ));
        };
        self.inner.degree_modulus(degree).map_err(network_error)
    }

    /// The nodes a degree, a `Terminus` member or a `Node` names.
    #[pyo3(signature = (nodeId, *, equateTermini = true, permitDegreeModuli = true))]
    fn nodeNameToNodes(
        &self,
        py: Python<'_>,
        nodeId: Option<&Bound<'_, PyAny>>,
        #[pyo3(from_py_with = crate::truthy)] equateTermini: bool,
        #[pyo3(from_py_with = crate::truthy)] permitDegreeModuli: bool,
    ) -> PyResult<Vec<Py<Node>>> {
        let name = name_required(nodeId)?;
        let termini = if equateTermini {
            Termini::Equate
        } else {
            Termini::Distinct
        };
        let moduli = if permitDegreeModuli {
            Moduli::Permit
        } else {
            Moduli::Forbid
        };
        let ids = self
            .inner
            .nodes_named(name, termini, moduli)
            .map_err(network_error)?;
        self.node_objects(py, &ids)
    }

    /// Every edge leaving a node walked `direction`, and the node each
    /// reaches.
    fn getNext(
        &self,
        py: Python<'_>,
        nodeStart: &Bound<'_, PyAny>,
        direction: &Bound<'_, PyAny>,
    ) -> PyResult<NextSteps> {
        let steps = self
            .inner
            .next_steps(id_from_py(nodeStart)?, direction_from_py(Some(direction))?)
            .map_err(network_error)?;
        let mut edges = Vec::with_capacity(steps.len());
        let mut nodes = Vec::with_capacity(steps.len());
        for (edge, node) in steps {
            edges.push(Edge::object(py, &self.inner.edges()[edge])?);
            nodes.push(self.node_object(py, node)?);
        }
        Ok((edges, nodes))
    }

    /// The pitch `stepSize` steps on from `pitchOrigin`, on a realization
    /// from `pitchReference` standing on `nodeName`.
    #[pyo3(signature = (
        pitchReference,
        nodeName,
        pitchOrigin,
        *,
        direction = None,
        stepSize = 1,
        getNeighbor = None,
        randomGenerator = None,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn nextPitch(
        &self,
        py: Python<'_>,
        pitchReference: &Bound<'_, PyAny>,
        nodeName: Option<&Bound<'_, PyAny>>,
        pitchOrigin: Option<&Bound<'_, PyAny>>,
        direction: Option<&Bound<'_, PyAny>>,
        stepSize: usize,
        getNeighbor: Option<&Bound<'_, PyAny>>,
        randomGenerator: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Option<Pitch>> {
        let Some(origin) = pitchOrigin.filter(|value| !value.is_none()) else {
            return Err(PyTypeError::new_err(
                "No pitch origin for calling next on this pitch!",
            ));
        };
        let origin = pitch_from_any(origin)?;
        let reference = pitch_from_any(pitchReference)?;
        let direction = direction_from_py(direction)?;
        let neighbor = neighbor_from(getNeighbor)?;
        let name = name_required(nodeName)?;
        let next = drawing(py, randomGenerator, |draw| {
            self.inner.next_pitch(
                &reference, name, &origin, direction, stepSize, neighbor, draw,
            )
        })?;
        Ok(next.map(wrap))
    }

    /// The pitches from `pitchReference` upward.
    #[pyo3(signature = (pitchReference, nodeId = None, minPitch = None, maxPitch = None, *, fillMinMaxIfNone = false))]
    fn realizeAscending(
        &self,
        py: Python<'_>,
        pitchReference: &Bound<'_, PyAny>,
        nodeId: Option<&Bound<'_, PyAny>>,
        minPitch: Option<&Bound<'_, PyAny>>,
        maxPitch: Option<&Bound<'_, PyAny>>,
        #[pyo3(from_py_with = crate::truthy)] fillMinMaxIfNone: bool,
    ) -> PyResult<(Vec<Pitch>, Vec<Py<PyAny>>)> {
        let reference = pitch_from_any(pitchReference)?;
        let start = name_from_py(nodeId)?;
        let range = range_from(minPitch, maxPitch)?;
        let fill = fill_from(fillMinMaxIfNone);
        let realized = drawing(py, None, |draw| {
            self.inner
                .realize_ascending(&reference, start, &range, fill, draw)
        })?;
        realized_to_py(py, realized)
    }

    /// The pitches from `pitchReference` downward.
    #[pyo3(signature = (
        pitchReference,
        nodeId = None,
        minPitch = None,
        maxPitch = None,
        *,
        includeFirst = false,
        fillMinMaxIfNone = false,
        reverse = true,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn realizeDescending(
        &self,
        py: Python<'_>,
        pitchReference: &Bound<'_, PyAny>,
        nodeId: Option<&Bound<'_, PyAny>>,
        minPitch: Option<&Bound<'_, PyAny>>,
        maxPitch: Option<&Bound<'_, PyAny>>,
        #[pyo3(from_py_with = crate::truthy)] includeFirst: bool,
        #[pyo3(from_py_with = crate::truthy)] fillMinMaxIfNone: bool,
        #[pyo3(from_py_with = crate::truthy)] reverse: bool,
    ) -> PyResult<(Vec<Pitch>, Vec<Py<PyAny>>)> {
        let range = range_from(minPitch, maxPitch)?;
        let walk = DescendingWalk {
            first: if includeFirst {
                music21::scale::intervalnetwork::First::Include
            } else {
                music21::scale::intervalnetwork::First::Exclude
            },
            fill: fill_from(fillMinMaxIfNone),
            order: if reverse {
                Order::Ascending
            } else {
                Order::Descending
            },
        };
        self.descending(py, pitchReference, nodeId, &range, walk)
    }

    /// The network realized from `pitchReference` standing on `nodeId`:
    /// its pitches and node ids.
    #[pyo3(signature = (pitchReference, nodeId = None, minPitch = None, maxPitch = None, direction = None, reverse = false))]
    fn realize(
        &self,
        py: Python<'_>,
        pitchReference: Option<&Bound<'_, PyAny>>,
        nodeId: Option<&Bound<'_, PyAny>>,
        minPitch: Option<&Bound<'_, PyAny>>,
        maxPitch: Option<&Bound<'_, PyAny>>,
        direction: Option<&Bound<'_, PyAny>>,
        #[pyo3(from_py_with = crate::truthy)] reverse: bool,
    ) -> PyResult<(Vec<Pitch>, Vec<Py<PyAny>>)> {
        let realized = self.realized(
            py,
            pitchReference,
            nodeId,
            minPitch,
            maxPitch,
            direction,
            reverse,
        )?;
        realized_to_py(py, realized)
    }

    /// The pitches of [`Self::realize`].
    #[pyo3(signature = (pitchReference, nodeId = None, minPitch = None, maxPitch = None, direction = None, reverse = false))]
    fn realizePitch(
        &self,
        py: Python<'_>,
        pitchReference: Option<&Bound<'_, PyAny>>,
        nodeId: Option<&Bound<'_, PyAny>>,
        minPitch: Option<&Bound<'_, PyAny>>,
        maxPitch: Option<&Bound<'_, PyAny>>,
        direction: Option<&Bound<'_, PyAny>>,
        #[pyo3(from_py_with = crate::truthy)] reverse: bool,
    ) -> PyResult<Vec<Pitch>> {
        let realized = self.realized(
            py,
            pitchReference,
            nodeId,
            minPitch,
            maxPitch,
            direction,
            reverse,
        )?;
        Ok(wrap_all(realized.pitches))
    }

    /// The steps between successive pitches of a realization from C4.
    #[pyo3(signature = (nodeId = None, minPitch = None, maxPitch = None, direction = None, reverse = false))]
    fn realizeIntervals(
        &self,
        py: Python<'_>,
        nodeId: Option<&Bound<'_, PyAny>>,
        minPitch: Option<&Bound<'_, PyAny>>,
        maxPitch: Option<&Bound<'_, PyAny>>,
        direction: Option<&Bound<'_, PyAny>>,
        #[pyo3(from_py_with = crate::truthy)] reverse: bool,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let start = name_from_py(nodeId)?;
        let range = range_from(minPitch, maxPitch)?;
        let direction = direction_from_py(direction)?;
        let order = order_from(reverse);
        let intervals = drawing(py, None, |draw| {
            self.inner
                .realize_intervals(start, &range, direction, order, draw)
        })?;
        intervals
            .into_iter()
            .map(|interval| interval_object(py, interval))
            .collect()
    }

    /// The pitches on the termini of a realization.
    #[pyo3(signature = (pitchReference, nodeId = None))]
    fn realizeTermini(
        &self,
        py: Python<'_>,
        pitchReference: &Bound<'_, PyAny>,
        nodeId: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<(Pitch, Pitch)> {
        let reference = pitch_from_any(pitchReference)?;
        let start = name_from_py(nodeId)?;
        let (low, high) = drawing(py, None, |draw| {
            self.inner.realize_termini(&reference, start, draw)
        })?;
        Ok((wrap(low), wrap(high)))
    }

    /// The lowest and highest pitches of a span from terminus to terminus
    /// either side of `pitchReference`.
    #[pyo3(signature = (pitchReference, nodeId = None))]
    fn realizeMinMax(
        &self,
        py: Python<'_>,
        pitchReference: &Bound<'_, PyAny>,
        nodeId: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<(Pitch, Pitch)> {
        let reference = pitch_from_any(pitchReference)?;
        let start = name_from_py(nodeId)?;
        let (low, high) = drawing(py, None, |draw| {
            self.inner.realize_min_max(&reference, start, draw)
        })?;
        Ok((wrap(low), wrap(high)))
    }

    /// The realized pitches on any of `nodeDegreeTargets`.
    #[pyo3(signature = (pitchReference, nodeId = None, nodeDegreeTargets = vec![1], minPitch = None, maxPitch = None, direction = None))]
    #[allow(clippy::too_many_arguments)]
    fn realizePitchByDegree(
        &self,
        py: Python<'_>,
        pitchReference: &Bound<'_, PyAny>,
        nodeId: Option<&Bound<'_, PyAny>>,
        nodeDegreeTargets: Vec<i32>,
        minPitch: Option<&Bound<'_, PyAny>>,
        maxPitch: Option<&Bound<'_, PyAny>>,
        direction: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Vec<Pitch>> {
        let reference = pitch_from_any(pitchReference)?;
        let start = name_from_py(nodeId)?;
        let range = range_from(minPitch, maxPitch)?;
        let direction = direction_from_py(direction)?;
        let found = drawing(py, None, |draw| {
            self.inner.pitches_by_degree(
                &reference,
                start,
                &nodeDegreeTargets,
                &range,
                direction,
                draw,
            )
        })?;
        Ok(wrap_all(found))
    }

    /// The ids of every node sounding `pitchTarget`, by degree.
    #[pyo3(signature = (pitchReference, nodeId, pitchTarget, *, comparisonAttribute = "ps", direction = None))]
    fn getRelativeNodeIds(
        &self,
        py: Python<'_>,
        pitchReference: &Bound<'_, PyAny>,
        nodeId: Option<&Bound<'_, PyAny>>,
        pitchTarget: &Bound<'_, PyAny>,
        comparisonAttribute: &str,
        direction: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let reference = pitch_from_any(pitchReference)?;
        let start = name_from_py(nodeId)?;
        let target = pitch_from_any(pitchTarget)?;
        let comparison = comparison_from(comparisonAttribute)?;
        let direction = direction_from_py(direction)?;
        let ids = drawing(py, None, |draw| {
            self.inner
                .relative_node_ids(&reference, start, &target, comparison, direction, draw)
        })?;
        ids_to_py(py, &ids)
    }

    /// The id of the node sounding `pitchTarget`, chosen by weight among
    /// several, or `None`.
    #[pyo3(signature = (pitchReference, nodeId, pitchTarget, *, comparisonAttribute = "ps", direction = None, randomGenerator = None))]
    #[allow(clippy::too_many_arguments)]
    fn getRelativeNodeId(
        &self,
        py: Python<'_>,
        pitchReference: &Bound<'_, PyAny>,
        nodeId: Option<&Bound<'_, PyAny>>,
        pitchTarget: &Bound<'_, PyAny>,
        comparisonAttribute: &str,
        direction: Option<&Bound<'_, PyAny>>,
        randomGenerator: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Option<Py<PyAny>>> {
        let reference = pitch_from_any(pitchReference)?;
        let start = name_from_py(nodeId)?;
        let target = pitch_from_any(pitchTarget)?;
        let comparison = comparison_from(comparisonAttribute)?;
        let direction = direction_from_py(direction)?;
        let id = drawing(py, randomGenerator, |draw| {
            self.inner
                .relative_node_id(&reference, start, &target, comparison, direction, draw)
        })?;
        id.map(|id| id_to_py(py, id)).transpose()
    }

    /// The ids of the nodes either side of `pitchTarget`, or `None` where
    /// nothing realized lies above it.
    #[pyo3(signature = (pitchReference, nodeName, pitchTarget, direction = None))]
    fn getNeighborNodeIds(
        &self,
        py: Python<'_>,
        pitchReference: &Bound<'_, PyAny>,
        nodeName: Option<&Bound<'_, PyAny>>,
        pitchTarget: &Bound<'_, PyAny>,
        direction: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Neighbors> {
        let reference = pitch_from_any(pitchReference)?;
        let start = name_from_py(nodeName)?;
        let target = pitch_from_any(pitchTarget)?;
        let direction = direction_from_py(direction)?;
        let found = drawing(py, None, |draw| {
            self.inner
                .neighbor_node_ids(&reference, start, &target, direction, draw)
        })?;
        let Some((low, high)) = found else {
            return Ok(None);
        };
        let low = low.map(|low| id_to_py(py, low)).transpose()?;
        Ok(Some((low, id_to_py(py, high)?)))
    }

    /// The degree of the node sounding `pitchTarget`, or `None`.
    #[pyo3(signature = (pitchReference, nodeId, pitchTarget, comparisonAttribute = "ps", direction = None, *, randomGenerator = None))]
    #[allow(clippy::too_many_arguments)]
    fn getRelativeNodeDegree(
        &self,
        py: Python<'_>,
        pitchReference: &Bound<'_, PyAny>,
        nodeId: Option<&Bound<'_, PyAny>>,
        pitchTarget: &Bound<'_, PyAny>,
        comparisonAttribute: &str,
        direction: Option<&Bound<'_, PyAny>>,
        randomGenerator: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Option<i32>> {
        let reference = pitch_from_any(pitchReference)?;
        let start = name_from_py(nodeId)?;
        let target = pitch_from_any(pitchTarget)?;
        let comparison = comparison_from(comparisonAttribute)?;
        let direction = direction_from_py(direction)?;
        drawing(py, randomGenerator, |draw| {
            self.inner
                .relative_degree(&reference, start, &target, comparison, direction, draw)
        })
    }

    /// The pitch of the node of degree `nodeDegreeTarget`.
    #[pyo3(signature = (pitchReference, nodeName, nodeDegreeTarget, direction = None, minPitch = None, maxPitch = None, equateTermini = true))]
    #[allow(clippy::too_many_arguments)]
    fn getPitchFromNodeDegree(
        &self,
        py: Python<'_>,
        pitchReference: &Bound<'_, PyAny>,
        nodeName: Option<&Bound<'_, PyAny>>,
        nodeDegreeTarget: Option<&Bound<'_, PyAny>>,
        direction: Option<&Bound<'_, PyAny>>,
        minPitch: Option<&Bound<'_, PyAny>>,
        maxPitch: Option<&Bound<'_, PyAny>>,
        #[pyo3(from_py_with = crate::truthy)] equateTermini: bool,
    ) -> PyResult<Option<Pitch>> {
        let reference = pitch_from_any(pitchReference)?;
        let name = name_required(nodeName)?;
        let target = name_required(nodeDegreeTarget)?;
        let direction = direction_from_py(direction)?;
        let range = range_from(minPitch, maxPitch)?;
        let termini = if equateTermini {
            Termini::Equate
        } else {
            Termini::Distinct
        };
        let found = drawing(py, None, |draw| {
            self.inner
                .pitch_of_degree(&reference, name, target, direction, &range, termini, draw)
        })?;
        Ok(found.map(wrap))
    }

    /// The pitches given, read into `Pitch` objects, with the lowest and
    /// the highest.
    #[staticmethod]
    fn filterPitchList<'py>(pitchTarget: &Bound<'py, PyAny>) -> PyResult<Bounded<'py>> {
        let (objects, pitches) = target_pitches(pitchTarget)?;
        at_least_one(&pitches)?;
        let Some((low, high)) = pitch_bounds(&pitches) else {
            return Err(PyValueError::new_err(
                "There must be at least one pitch given.",
            ));
        };
        let (low, high) = (objects[low].clone(), objects[high].clone());
        Ok((objects, low, high))
    }

    /// The pitches of `pitchTarget` a realization holds, and those it does
    /// not.
    #[pyo3(name = "match", signature = (pitchReference, nodeId, pitchTarget, comparisonAttribute = "pitchClass"))]
    fn match_pitches<'py>(
        &self,
        pitchReference: &Bound<'py, PyAny>,
        nodeId: Option<&Bound<'py, PyAny>>,
        pitchTarget: &Bound<'py, PyAny>,
        comparisonAttribute: &str,
    ) -> PyResult<Split<'py>> {
        let reference = pitch_from_any(pitchReference)?;
        let start = name_from_py(nodeId)?;
        let (objects, pitches) = target_pitches(pitchTarget)?;
        at_least_one(&pitches)?;
        let comparison = comparison_from(comparisonAttribute)?;
        let matched = self
            .inner
            .match_pitches(&reference, start, &pitches, comparison)
            .map_err(network_error)?;
        let pick = |indices: &[usize]| {
            indices
                .iter()
                .map(|index| objects[*index].clone())
                .collect()
        };
        Ok((pick(&matched.matched), pick(&matched.unmatched)))
    }

    /// The realized pitches no pitch of `pitchTarget` matches.
    #[pyo3(signature = (pitchReference, nodeId, pitchTarget, comparisonAttribute = "pitchClass", minPitch = None, maxPitch = None, direction = None))]
    #[allow(clippy::too_many_arguments)]
    fn findMissing(
        &self,
        pitchReference: &Bound<'_, PyAny>,
        nodeId: Option<&Bound<'_, PyAny>>,
        pitchTarget: &Bound<'_, PyAny>,
        comparisonAttribute: &str,
        minPitch: Option<&Bound<'_, PyAny>>,
        maxPitch: Option<&Bound<'_, PyAny>>,
        direction: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Vec<Pitch>> {
        // music21 takes a direction here and does not use it.
        direction_from_py(direction)?;
        let reference = pitch_from_any(pitchReference)?;
        let start = name_from_py(nodeId)?;
        let (_, pitches) = target_pitches(pitchTarget)?;
        let comparison = comparison_from(comparisonAttribute)?;
        let range = range_from(minPitch, maxPitch)?;
        let missing = self
            .inner
            .find_missing(&reference, start, &pitches, comparison, &range)
            .map_err(network_error)?;
        Ok(wrap_all(missing))
    }

    /// For each of fifteen tonics on the low terminus, how many of
    /// `pitchTarget` the network holds, most first; the first
    /// `resultsReturned`, or all for `None`.
    #[pyo3(signature = (pitchTarget, resultsReturned = Some(4), comparisonAttribute = "pitchClass"))]
    fn find(
        &self,
        pitchTarget: &Bound<'_, PyAny>,
        resultsReturned: Option<usize>,
        comparisonAttribute: &str,
    ) -> PyResult<Vec<(usize, Pitch)>> {
        let (_, pitches) = target_pitches(pitchTarget)?;
        at_least_one(&pitches)?;
        let comparison = comparison_from(comparisonAttribute)?;
        let mut found = self
            .inner
            .find(&pitches, comparison)
            .map_err(network_error)?;
        if let Some(count) = resultsReturned {
            found.truncate(count);
        }
        Ok(found
            .into_iter()
            .map(|found| (found.matches, wrap(found.tonic)))
            .collect())
    }

    /// The pitch moved by the interval and respelled as the network's
    /// `pitchSimplification` says.
    fn transposePitchAndApplySimplification(
        &self,
        intervalObj: &Bound<'_, PyAny>,
        pitchObj: &Bound<'_, PyAny>,
    ) -> PyResult<Pitch> {
        let interval = interval_from_any(intervalObj)?;
        let pitch = pitch_from_any(pitchObj)?;
        let moved = self
            .inner
            .transpose_step(&interval, &pitch)
            .map_err(network_error)?;
        Ok(wrap(moved))
    }

    fn _searchSpan(&self) -> i32 {
        self.inner.search_span()
    }

    fn _spellTerminus(
        &self,
        p: &Bound<'_, PyAny>,
        nodeObj: &Bound<'_, PyAny>,
        terminusName: Option<String>,
    ) -> PyResult<Pitch> {
        let spelled = self
            .inner
            .spell_terminus(
                pitch_from_any(p)?,
                id_from_py(nodeObj)?,
                terminusName.as_deref(),
            )
            .map_err(network_error)?;
        Ok(wrap(spelled))
    }
}

impl IntervalNetwork {
    #[allow(clippy::too_many_arguments)]
    fn realized(
        &self,
        py: Python<'_>,
        pitchReference: Option<&Bound<'_, PyAny>>,
        nodeId: Option<&Bound<'_, PyAny>>,
        minPitch: Option<&Bound<'_, PyAny>>,
        maxPitch: Option<&Bound<'_, PyAny>>,
        direction: Option<&Bound<'_, PyAny>>,
        reverse: bool,
    ) -> PyResult<Realized> {
        let Some(reference) = pitchReference.filter(|value| !value.is_none()) else {
            return Err(IntervalNetworkException::new_err(
                "pitchReference cannot be None",
            ));
        };
        let reference = pitch_from_any(reference)?;
        let start = name_from_py(nodeId)?;
        let range = range_from(minPitch, maxPitch)?;
        let direction = direction_from_py(direction)?;
        let order = order_from(reverse);
        drawing(py, None, |draw| {
            self.inner
                .realize(&reference, start, &range, direction, order, draw)
        })
    }
}

fn fill_from(fill: bool) -> music21::scale::intervalnetwork::Fill {
    if fill {
        music21::scale::intervalnetwork::Fill::IfUnbounded
    } else {
        music21::scale::intervalnetwork::Fill::Never
    }
}

fn order_from(reverse: bool) -> Order {
    if reverse {
        Order::Descending
    } else {
        Order::Ascending
    }
}

/// music21's `getNeighbor`: true or a direction for the node above, a
/// descending direction for the node below, anything else for none.
fn neighbor_from(value: Option<&Bound<'_, PyAny>>) -> PyResult<Neighbor> {
    let Some(value) = value else {
        return Ok(Neighbor::Above);
    };
    if let Some(direction) = member_value(value) {
        return Ok(match direction.as_str() {
            "descending" => Neighbor::Below,
            "bi" | "ascending" => Neighbor::Above,
            _ => Neighbor::Never,
        });
    }
    if value.extract::<bool>().unwrap_or(false) {
        return Ok(Neighbor::Above);
    }
    Ok(Neighbor::Never)
}

/// music21's `BoundIntervalNetwork`, kept by music21 for an old paper: an
/// `IntervalNetwork` by another name.
#[pyclass(
    name = "BoundIntervalNetwork",
    module = "music21.scale.intervalNetwork",
    extends = IntervalNetwork,
    skip_from_py_object
)]
pub struct BoundIntervalNetwork;

#[pymethods]
impl BoundIntervalNetwork {
    #[new]
    #[pyo3(signature = (
        edgeList = None,
        octaveDuplicating = false,
        deterministic = true,
        pitchSimplification = Some("maxAccidental".to_string()),
    ))]
    fn new(
        edgeList: Option<&Bound<'_, PyAny>>,
        #[pyo3(from_py_with = crate::truthy)] octaveDuplicating: bool,
        #[pyo3(from_py_with = crate::truthy)] deterministic: bool,
        pitchSimplification: Option<String>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let network = IntervalNetwork::new(
            edgeList,
            octaveDuplicating,
            deterministic,
            pitchSimplification,
        )?;
        Ok(PyClassInitializer::from(network).add_subclass(Self))
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    let direction = made_enum(
        m,
        "Direction",
        &[
            ("BI", "bi"),
            ("ASCENDING", "ascending"),
            ("DESCENDING", "descending"),
        ],
        "The way a scale is walked: Direction.ASCENDING, Direction.DESCENDING, \
         or Direction.BI for both.",
    )?;
    m.add("Direction", &direction)?;
    let _ = DIRECTION.set(py, direction.unbind());
    let terminus = made_enum(
        m,
        "Terminus",
        &[("LOW", "terminusLow"), ("HIGH", "terminusHigh")],
        "Either end of a scale's network: Terminus.LOW or Terminus.HIGH.",
    )?;
    m.add("Terminus", &terminus)?;
    let _ = TERMINUS.set(py, terminus.unbind());
    m.add_class::<Node>()?;
    m.add_class::<Edge>()?;
    m.add_class::<IntervalNetwork>()?;
    m.add_class::<BoundIntervalNetwork>()?;
    Ok(())
}
