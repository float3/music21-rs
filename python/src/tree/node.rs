//! music21's `tree.node`: `ElementNode`, a node that knows where its
//! element stands among the tree's and when the elements under it end, and
//! `OffsetNode`, one holding a list of elements at one offset.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::PyAttributeError;
use pyo3::prelude::*;
use pyo3::types::{PyList, PySet};

use music21::tree::core::{self as avl, AvlNodes, CountedNodes, Indices};

use super::core::{AvlNode, Failed, Nodes, shown_position};

/// A node holding one element, aware of the element's index in the tree
/// and of the earliest and latest end of the elements under it: music21's
/// `tree.node.ElementNode`.
#[pyclass(name = "ElementNode", module = "music21.tree.node", extends = AvlNode, subclass)]
pub struct ElementNode {
    /// The index of its element among the tree's; -1 until counted.
    #[pyo3(get, set)]
    pub(crate) payloadElementIndex: i64,
    /// The latest end of any element under it.
    #[pyo3(get, set)]
    pub(crate) endTimeHigh: Py<PyAny>,
    /// The earliest end of any element under it.
    #[pyo3(get, set)]
    pub(crate) endTimeLow: Py<PyAny>,
    /// The index of the first element under it.
    #[pyo3(get, set)]
    pub(crate) subtreeElementsStartIndex: i64,
    /// One past the index of the last.
    #[pyo3(get, set)]
    pub(crate) subtreeElementsStopIndex: i64,
}

impl ElementNode {
    /// One not yet counted.
    fn uncounted(py: Python<'_>) -> Self {
        Self {
            payloadElementIndex: -1,
            endTimeHigh: py.None(),
            endTimeLow: py.None(),
            subtreeElementsStartIndex: -1,
            subtreeElementsStopIndex: -1,
        }
    }
}

/// A node's position as an offset: a `SortTuple`'s `offset`.
pub(crate) fn offset_of<'py>(position: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if position.hasattr("shortRepr")? {
        return position.getattr("offset");
    }
    Ok(position.clone())
}

/// The lower of two, the first where neither is lower: Python's `min`.
fn lower<'py>(first: Bound<'py, PyAny>, second: Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if second.lt(&first)? {
        return Ok(second);
    }
    Ok(first)
}

/// The higher of two, the first where neither is higher: Python's `max`.
fn higher<'py>(first: Bound<'py, PyAny>, second: Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if second.gt(&first)? {
        return Ok(second);
    }
    Ok(first)
}

/// Where an element held at a position ends: its `endTime` where it has
/// one, a timespan's or a tree's, else its position and length.
fn end_of<'py>(
    element: &Bound<'py, PyAny>,
    position: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    match element.getattr("endTime") {
        Ok(end) => Ok(end),
        Err(error) if error.is_instance_of::<PyAttributeError>(element.py()) => {
            let length = element.getattr("duration")?.getattr("quarterLength")?;
            offset_of(position)?.add(length)
        }
        Err(error) => Err(error),
    }
}

/// The earliest and latest ends of what one node holds.
fn own_ends<'py>(node: &Bound<'py, AvlNode>) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py = node.py();
    let (position, payload) = {
        let held = node.borrow();
        (
            held.position.bind(py).clone(),
            held.payload.bind(py).clone(),
        )
    };
    if node.cast::<OffsetNode>().is_err() {
        let end = end_of(&payload, &position)?;
        return Ok((end.clone(), end));
    }

    // music21 takes `min` and `max` of the ends, which raise on an empty
    // payload.
    let mut ends = Vec::new();
    for element in payload.try_iter()? {
        ends.push(end_of(&element?, &position)?);
    }
    let builtins = py.import("builtins")?;
    let list = PyList::new(py, &ends)?;
    Ok((
        builtins.getattr("min")?.call1((&list,))?,
        builtins.getattr("max")?.call1((&list,))?,
    ))
}

/// Works out the earliest and latest end under each node from `node`
/// down: music21's `updateEndTimes`.
pub(crate) fn update_end_times(node: &Bound<'_, AvlNode>) -> PyResult<()> {
    let (mut low, mut high) = own_ends(node)?;
    let children = {
        let held = node.borrow();
        [
            held.left.as_ref().map(|child| child.clone_ref(node.py())),
            held.right.as_ref().map(|child| child.clone_ref(node.py())),
        ]
    };
    for child in children.into_iter().flatten() {
        let child = child.bind(node.py());
        update_end_times(child)?;
        let below = child.cast::<ElementNode>()?.borrow();
        low = lower(low, below.endTimeLow.bind(node.py()).clone())?;
        high = higher(high, below.endTimeHigh.bind(node.py()).clone())?;
    }
    let mut counted = node.cast::<ElementNode>()?.borrow_mut();
    counted.endTimeLow = low.unbind();
    counted.endTimeHigh = high.unbind();
    Ok(())
}

impl CountedNodes for Nodes<'_> {
    fn payload_count(&self, node: &Self::Node) -> Result<usize, Failed> {
        let payload = node.borrow().payload.bind(node.py()).clone();
        if node.cast::<OffsetNode>().is_ok() {
            return Ok(payload.len()?);
        }
        Ok(usize::from(!payload.is_none()))
    }

    fn set_indices(&mut self, node: &Self::Node, indices: Indices) -> Result<(), Failed> {
        let index = |value: usize| i64::try_from(value).unwrap_or(i64::MAX);
        if let Ok(offsets) = node.cast::<OffsetNode>() {
            let mut held = offsets.borrow_mut();
            held.payloadElementsStartIndex = index(indices.payload_start);
            held.payloadElementsStopIndex = index(indices.payload_stop);
        }
        let Ok(element) = node.cast::<ElementNode>() else {
            return Ok(());
        };
        let mut held = element.borrow_mut();
        if node.cast::<OffsetNode>().is_err() {
            held.payloadElementIndex = index(indices.payload_start);
        }
        held.subtreeElementsStartIndex = index(indices.subtree_start);
        held.subtreeElementsStopIndex = index(indices.subtree_stop);
        Ok(())
    }
}

/// Counts the elements under each node from `node` down.
pub(crate) fn update_indices(
    node: &Bound<'_, AvlNode>,
    parent_stop: Option<usize>,
) -> PyResult<()> {
    avl::update_indices(&mut Nodes::linked(), node, parent_stop)?;
    Ok(())
}

/// The position at the far end of a node's subtree, leftmost or
/// rightmost.
fn far_position<'py>(node: &Bound<'py, AvlNode>, leftmost: bool) -> PyResult<Bound<'py, PyAny>> {
    let nodes = Nodes::linked();
    let mut current = node.clone();
    loop {
        let next = match leftmost {
            true => nodes.left(&current)?,
            false => nodes.right(&current)?,
        };
        let Some(next) = next else {
            return Ok(nodes.position(&current)?);
        };
        current = next;
    }
}

#[pymethods]
impl ElementNode {
    #[new]
    #[pyo3(signature = (position, payload = None))]
    fn py_new(
        py: Python<'_>,
        position: Py<PyAny>,
        payload: Option<Py<PyAny>>,
    ) -> PyClassInitializer<Self> {
        let node = AvlNode::new(position, payload.unwrap_or_else(|| py.None()));
        PyClassInitializer::from(node).add_subclass(Self::uncounted(py))
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.endTimeHigh)?;
        visit.call(&self.endTimeLow)
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let py = slf.py();
        let node = slf.as_super().borrow();
        let held = slf.borrow();
        Ok(format!(
            "<ElementNode: Start:{} Indices:(l:{} *{}* r:{}) Payload:{}>",
            shown_position(node.position.bind(py))?,
            held.subtreeElementsStartIndex,
            held.payloadElementIndex,
            held.subtreeElementsStopIndex,
            node.payload.bind(py).repr()?
        ))
    }

    /// The lowest position under this node.
    #[getter]
    fn lowestPosition<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        far_position(slf.as_super(), true)
    }

    /// The highest position under this node.
    #[getter]
    fn highestPosition<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        far_position(slf.as_super(), false)
    }

    /// Counts the elements of this node and those under it, from
    /// `parentStopIndex` where elements come before them.
    #[pyo3(signature = (parentStopIndex = None))]
    fn updateIndices(slf: &Bound<'_, Self>, parentStopIndex: Option<usize>) -> PyResult<()> {
        update_indices(slf.as_super(), parentStopIndex)
    }

    /// Works out the earliest and latest end under this node and each
    /// below it.
    fn updateEndTimes(slf: &Bound<'_, Self>) -> PyResult<()> {
        update_end_times(slf.as_super())
    }
}

/// A node holding every element that starts at one offset, in a list:
/// music21's `tree.node.OffsetNode`.
#[pyclass(name = "OffsetNode", module = "music21.tree.node", extends = ElementNode, subclass)]
pub struct OffsetNode {
    /// The index of the first element of its own payload.
    #[pyo3(get, set)]
    pub(crate) payloadElementsStartIndex: i64,
    /// One past the index of the last.
    #[pyo3(get, set)]
    pub(crate) payloadElementsStopIndex: i64,
}

#[pymethods]
impl OffsetNode {
    /// A node at an offset with an empty payload; a payload given is not
    /// kept, as in music21.
    #[new]
    #[pyo3(signature = (offset, payload = None))]
    fn py_new(
        py: Python<'_>,
        offset: Py<PyAny>,
        payload: Option<Py<PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let _ = payload;
        let node = AvlNode::new(offset, PyList::empty(py).into_any().unbind());
        Ok(PyClassInitializer::from(node)
            .add_subclass(ElementNode::uncounted(py))
            .add_subclass(Self {
                payloadElementsStartIndex: -1,
                payloadElementsStopIndex: -1,
            }))
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let py = slf.py();
        let element = slf.as_super();
        let node = element.as_super().borrow();
        let counted = element.borrow();
        let held = slf.borrow();
        Ok(format!(
            "<OffsetNode {} Indices:{},{},{},{} Length:{}>",
            node.position.bind(py).str()?,
            counted.subtreeElementsStartIndex,
            held.payloadElementsStartIndex,
            held.payloadElementsStopIndex,
            counted.subtreeElementsStopIndex,
            node.payload.bind(py).len()?
        ))
    }

    /// Where each element of the payload ends, unsorted: a timespan's
    /// `endTime`, an element's position plus its length.
    fn payloadEndTimes<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let (position, payload) = {
            let node = slf.as_super().as_super().borrow();
            (
                node.position.bind(py).clone(),
                node.payload.bind(py).clone(),
            )
        };
        let ends = PyList::empty(py);
        for element in payload.try_iter()? {
            ends.append(end_of(&element?, &position)?)?;
        }
        Ok(ends)
    }
}

/// Every position and end in a node's subtree, for `allTimePoints`.
pub(crate) fn time_points<'py>(
    node: &Bound<'py, AvlNode>,
    into: &Bound<'py, PySet>,
) -> PyResult<()> {
    into.add(node.borrow().position.bind(node.py()))?;
    let ends = OffsetNode::payloadEndTimes(node.cast::<OffsetNode>()?)?;
    for end in ends.iter() {
        into.add(end)?;
    }
    Ok(())
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ElementNode>()?;
    m.add_class::<OffsetNode>()?;
    Ok(())
}
