//! music21's `tree.trees`: `ElementTree`, a tree of a stream's elements by
//! their `SortTuple`s, one to a node, and `OffsetTree`, a tree of them by
//! offset, all those at one offset in one node.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::{PyIndexError, PyNotImplementedError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PySet, PySlice, PyTuple, PyType};

use super::core::{AvlNode, AvlTree, nodes_of, root_of, set_root, shown_position};
use super::node::{
    ElementNode, OffsetNode, offset_of, time_points, update_end_times, update_indices,
};
use crate::exceptions21::TreeException;

/// Whether music21's `common.isListLike` holds: a list or a tuple.
fn list_like(value: &Bound<'_, PyAny>) -> bool {
    value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>()
}

/// A stream kept weakly where it can be, as music21's `wrapWeakref` keeps
/// one.
fn wrapped(value: &Bound<'_, PyAny>) -> PyResult<Option<Py<PyAny>>> {
    if value.is_none() {
        return Ok(None);
    }
    let weakref = value.py().import("weakref")?;
    match weakref.getattr("ref")?.call1((value,)) {
        Ok(reference) => Ok(Some(reference.unbind())),
        Err(_) => Ok(Some(value.clone().unbind())),
    }
}

/// What `wrapped` kept, unwrapped: music21's `unwrapWeakref`.
fn unwrapped<'py>(py: Python<'py>, held: Option<&Py<PyAny>>) -> PyResult<Bound<'py, PyAny>> {
    let Some(held) = held else {
        return Ok(py.None().into_bound(py));
    };
    let held = held.bind(py);
    let reference = py.import("weakref")?.getattr("ReferenceType")?;
    if held.is_instance(&reference)? {
        return held.call0();
    }
    Ok(held.clone())
}

/// An `ElementNode`'s counts.
fn counted<'py>(node: &Bound<'py, AvlNode>) -> PyResult<PyRef<'py, ElementNode>> {
    Ok(node.cast::<ElementNode>()?.borrow())
}

/// How many elements a tree holds.
fn length_of(tree: &Bound<'_, PyAny>) -> PyResult<i64> {
    match root_of(tree)? {
        Some(root) => Ok(counted(&root)?.subtreeElementsStopIndex),
        None => Ok(0),
    }
}

/// An integer index counted from the end where negative, or an
/// `IndexError` outside the tree.
fn index_in(tree: &Bound<'_, PyAny>, index: i64) -> PyResult<i64> {
    let length = length_of(tree)?;
    let index = if index < 0 { length + index } else { index };
    if index < 0 || index >= length {
        return Err(PyIndexError::new_err(""));
    }
    Ok(index)
}

/// A slice's start and stop over a tree.
fn slice_in(tree: &Bound<'_, PyAny>, slice: &Bound<'_, PySlice>) -> PyResult<(i64, i64)> {
    let length = length_of(tree)?;
    let indices = slice.indices(isize::try_from(length).unwrap_or(isize::MAX))?;
    Ok((indices.start as i64, indices.stop as i64))
}

/// A child of a node.
fn child<'py>(node: &Bound<'py, AvlNode>, left: bool) -> Option<Bound<'py, AvlNode>> {
    let held = node.borrow();
    let child = match left {
        true => held.left.as_ref(),
        false => held.right.as_ref(),
    };
    child.map(|child| child.bind(node.py()).clone())
}

/// The node of an `ElementTree` holding the element at an index.
fn node_at<'py>(node: &Bound<'py, AvlNode>, index: i64) -> PyResult<Option<Bound<'py, AvlNode>>> {
    let own = counted(node)?.payloadElementIndex;
    if own == index {
        return Ok(Some(node.clone()));
    }
    if index < own {
        return match child(node, true) {
            Some(left) => node_at(&left, index),
            None => Ok(None),
        };
    }
    match child(node, false) {
        Some(right) => node_at(&right, index),
        None => Ok(None),
    }
}

/// The nodes of an `ElementTree` holding the elements `start..stop`.
fn nodes_in<'py>(
    node: Option<Bound<'py, AvlNode>>,
    start: i64,
    stop: i64,
    into: &mut Vec<Bound<'py, AvlNode>>,
) -> PyResult<()> {
    let Some(node) = node else {
        return Ok(());
    };
    let own = counted(&node)?.payloadElementIndex;
    if start < own {
        nodes_in(child(&node, true), start, stop, into)?;
    }
    if start <= own && own < stop {
        into.push(node.clone());
    }
    if own < stop {
        nodes_in(child(&node, false), start, stop, into)?;
    }
    Ok(())
}

/// A node's own payload, as a Python object.
fn payload_of<'py>(node: &Bound<'py, AvlNode>) -> Bound<'py, PyAny> {
    node.borrow().payload.bind(node.py()).clone()
}

/// A node's position.
fn position_of<'py>(node: &Bound<'py, AvlNode>) -> Bound<'py, PyAny> {
    node.borrow().position.bind(node.py()).clone()
}

/// The position at the far left or right of a tree, `-inf` for an empty
/// one.
fn far_position<'py>(tree: &Bound<'py, PyAny>, leftmost: bool) -> PyResult<Bound<'py, PyAny>> {
    let py = tree.py();
    let Some(mut node) = root_of(tree)? else {
        return Ok(pyo3::types::PyFloat::new(py, f64::NEG_INFINITY).into_any());
    };
    while let Some(next) = child(&node, leftmost) {
        node = next;
    }
    Ok(position_of(&node))
}

/// Marks a tree as no longer the one read from its stream, where it was.
fn edited(tree: &Bound<'_, PyAny>) {
    if let Ok(spans) = tree.cast::<crate::tree::TimespanTree>() {
        spans.borrow_mut().forget_reading();
    }
}

/// A tree of elements, one to a node, by the `SortTuple` each has in the
/// stream the tree is of: music21's `tree.trees.ElementTree`.
#[pyclass(
    name = "ElementTree",
    module = "music21.tree.trees",
    extends = AvlTree,
    subclass
)]
pub struct ElementTree {
    /// The stream the tree is of, kept weakly.
    source: Option<Py<PyAny>>,
    /// The trees holding this one: a `weakref.WeakSet`.
    #[pyo3(get, set)]
    parentTrees: Py<PyAny>,
}

impl ElementTree {
    /// An empty one of a stream.
    pub(crate) fn of(source: &Bound<'_, PyAny>) -> PyResult<PyClassInitializer<Self>> {
        let py = source.py();
        let tree = Self {
            source: wrapped(source)?,
            parentTrees: py.import("weakref")?.getattr("WeakSet")?.call0()?.unbind(),
        };
        Ok(crate::base::built(AvlTree::default()).add_subclass(tree))
    }
}

#[pymethods]
impl ElementTree {
    /// An empty tree of `source`. Elements given are not inserted:
    /// `insert` them once it is made.
    #[new]
    #[pyo3(signature = (elements = None, source = None))]
    pub(crate) fn py_new(
        py: Python<'_>,
        elements: Option<&Bound<'_, PyAny>>,
        source: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        if let Some(elements) = elements
            && elements.is_truthy()?
        {
            return Err(PyNotImplementedError::new_err(
                "a tree made with elements is not ported to music21-rs; insert them once it is made",
            ));
        }
        Self::of(&source.cloned().unwrap_or_else(|| py.None().into_bound(py)))
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        if let Some(source) = &self.source {
            visit.call(source)?;
        }
        visit.call(&self.parentTrees)
    }

    #[classattr]
    fn nodeClass(py: Python<'_>) -> Py<PyType> {
        py.get_type::<ElementNode>().unbind()
    }

    /// Whether an element is in the tree: found at its `SortTuple` in the
    /// source, or else by looking at every element.
    fn __contains__(slf: &Bound<'_, Self>, element: &Bound<'_, PyAny>) -> PyResult<bool> {
        let source = Self::source(slf)?;
        let position = element.call_method1("sortTuple", (source,))?;
        let found = AvlTree::getNodeByPosition(slf.as_super(), &position)?;
        if found.is_some_and(|node| payload_of(&node).is(element)) {
            return Ok(true);
        }
        for held in slf.try_iter()? {
            if held?.is(element) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// An element by index, `None` outside the tree, or a list of them by a
    /// slice.
    fn __getitem__<'py>(
        slf: &Bound<'py, Self>,
        i: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let found = match Self::getNodeByIndex(slf, i) {
            Ok(found) => found,
            Err(error) if error.is_instance_of::<PyIndexError>(py) => {
                return Ok(py.None().into_bound(py));
            }
            Err(error) => return Err(error),
        };
        if found.is_none() {
            return Ok(found);
        }
        if let Ok(nodes) = found.cast::<PyList>() {
            let payloads = PyList::empty(py);
            for node in nodes.iter() {
                payloads.append(payload_of(node.cast::<AvlNode>()?))?;
            }
            return Ok(payloads.into_any());
        }
        Ok(payload_of(found.cast::<AvlNode>()?))
    }

    /// How many elements the tree holds.
    fn __len__(slf: &Bound<'_, Self>) -> PyResult<usize> {
        Ok(usize::try_from(length_of(slf.as_any())?).unwrap_or(0))
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let low = shown_position(&slf.call_method0("lowestPosition")?)?;
        let end = shown_position(&slf.getattr("endTime")?)?;
        let mut shown = format!(
            "<{} {{{}}} ({low} to {end})",
            slf.get_type().name()?,
            slf.len()?
        );
        let source = Self::source(slf)?;
        if !source.is_none() {
            shown.push_str(&format!(" {}", source.repr()?));
        }
        shown.push('>');
        Ok(shown)
    }

    /// Replaces the element at an index, or those of a slice, keeping
    /// their positions.
    fn __setitem__(
        slf: &Bound<'_, Self>,
        i: &Bound<'_, PyAny>,
        new: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        if i.is_instance_of::<pyo3::types::PyInt>() {
            let found = Self::getNodeByIndex(slf, i)?;
            let Ok(node) = found.cast::<AvlNode>() else {
                return Err(PyTypeError::new_err(format!(
                    "Index must be less than {}",
                    length_of(slf.as_any())?
                )));
            };
            node.borrow_mut().payload = new.clone().unbind();
            return Ok(());
        }
        let Ok(slice) = i.cast::<PySlice>() else {
            return Err(PyTypeError::new_err(format!(
                "Indices must be ints or slices, got {}",
                i.str()?
            )));
        };
        let Ok(new) = new.cast::<PyList>() else {
            return Err(PyTypeError::new_err(format!(
                "If {} is a slice, then {} must be a list",
                i.str()?,
                new.str()?
            )));
        };
        let start: i64 = slice.getattr("start")?.extract()?;
        let stop: i64 = slice.getattr("stop")?.extract()?;
        let step: i64 = slice.getattr("step")?.extract()?;
        let places: Vec<i64> = (start..stop)
            .step_by(usize::try_from(step).unwrap_or(1))
            .collect();
        if places.len() != new.len() {
            return Err(PyTypeError::new_err(format!(
                "{} is a slice of len {}, so {} cannot have len {}",
                i.str()?,
                (stop - start) as f64 / step as f64,
                new.str()?,
                new.len()
            )));
        }
        for (place, element) in places.into_iter().zip(new.iter()) {
            Self::__setitem__(slf, &place.into_pyobject(slf.py())?.into_any(), &element)?;
        }
        Ok(())
    }

    /// The tree's `repr`, then each element's text indented beneath.
    fn __str__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let mut lines = vec![slf.repr()?.to_string()];
        for element in slf.try_iter()? {
            for line in element?.str()?.to_string().lines() {
                lines.push(format!("\t{line}"));
            }
        }
        Ok(lines.join("\n"))
    }

    /// Each node's payload, in order.
    fn __iter__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let payloads = PyList::empty(slf.py());
        for node in nodes_of(slf.as_any())? {
            payloads.append(payload_of(&node))?;
        }
        Ok(payloads.try_iter()?.into_any())
    }

    /// Counts the elements and works out their ends again, after the tree
    /// has changed.
    #[pyo3(signature = (initialPosition = None, initialEndTime = None, visitedParents = None))]
    fn _updateNodes(
        slf: &Bound<'_, Self>,
        initialPosition: Option<&Bound<'_, PyAny>>,
        initialEndTime: Option<&Bound<'_, PyAny>>,
        visitedParents: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        // music21 tells the trees holding this one when its span moves,
        // then raises: trees have no offsets yet. No tree here holds
        // another that way, so there is none to tell.
        let _ = (initialPosition, initialEndTime, visitedParents);
        edited(slf.as_any());
        if let Some(root) = root_of(slf.as_any())? {
            update_indices(&root, None)?;
            update_end_times(&root)?;
        }
        Ok(())
    }

    /// Takes an element out of the node at a position, and the node out
    /// once it holds nothing, without counting again.
    pub(crate) fn _removeElementAtPosition(
        slf: &Bound<'_, Self>,
        element: &Bound<'_, PyAny>,
        position: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let Some(node) = AvlTree::getNodeByPosition(slf.as_super(), position)? else {
            return Ok(());
        };
        let payload = payload_of(&node);
        if let Ok(list) = payload.cast::<PyList>() {
            if list.contains(element)? {
                list.call_method1("remove", (element,))?;
            }
            if list.is_empty() {
                AvlTree::removeNode(slf.as_super(), position)?;
            }
            return Ok(());
        }
        if payload.is(element) {
            node.borrow_mut().payload = slf.py().None();
        }
        if payload_of(&node).is_none() {
            AvlTree::removeNode(slf.as_super(), position)?;
        }
        Ok(())
    }

    /// Where an element likely stands in the tree: its `SortTuple` in the
    /// source.
    fn getPositionFromElementUnsafe<'py>(
        slf: &Bound<'py, Self>,
        el: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        el.call_method1("sortTuple", (Self::source(slf)?,))
    }

    /// Replaces the tree with a balanced one of (position, element) pairs
    /// sorted by strictly rising position, counted as it is built.
    fn populateFromSortedList(
        slf: &Bound<'_, Self>,
        listOfTuples: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        use music21::tree::core::{self as avl, Placement};

        edited(slf.as_any());
        let items: Vec<Bound<'_, PyAny>> = listOfTuples.try_iter()?.collect::<PyResult<_>>()?;
        let class = slf.getattr("nodeClass")?;
        let mut make =
            |_: &mut super::core::Nodes<'_>, placed: Placement| -> Result<_, super::core::Failed> {
                let item = &items[placed.middle];
                let made = class.call1((item.get_item(0)?, item.get_item(1)?))?;
                let node = made.cast_into::<AvlNode>().map_err(PyErr::from)?;
                let index = |value: usize| i64::try_from(value).unwrap_or(i64::MAX);
                if let Ok(element) = node.cast::<ElementNode>() {
                    let mut held = element.borrow_mut();
                    held.payloadElementIndex = index(placed.middle);
                    held.subtreeElementsStartIndex = index(placed.start);
                    held.subtreeElementsStopIndex = index(placed.stop);
                }
                Ok(node)
            };
        let root = avl::build_sorted(&mut super::core::Nodes::linked(), items.len(), &mut make)?;
        set_root(slf.as_any(), root)
    }

    /// The node holding the element at an index, or a list of them by a
    /// slice.
    fn getNodeByIndex<'py>(
        slf: &Bound<'py, Self>,
        i: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let tree = slf.as_any();
        if let Ok(slice) = i.cast::<PySlice>() {
            let mut found = Vec::new();
            let (start, stop) = slice_in(tree, slice)?;
            nodes_in(root_of(tree)?, start, stop, &mut found)?;
            return Ok(PyList::new(py, found)?.into_any());
        }
        let Ok(index) = i.extract::<i64>() else {
            return Err(PyTypeError::new_err(format!(
                "Indices must be integers or slices, got {}",
                i.str()?
            )));
        };
        let Some(root) = root_of(tree)? else {
            return Err(PyIndexError::new_err(""));
        };
        let found = node_at(&root, index_in(tree, index)?)?;
        Ok(found.map_or_else(|| py.None().into_bound(py), Bound::into_any))
    }

    /// The nodes, in order.
    fn iterNodes<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        AvlTree::__iter__(slf.as_super())
    }

    /// Where an element stands among the tree's, found at `position` or
    /// its likely one, or else by looking at every element.
    #[pyo3(signature = (element, position = None))]
    fn index(
        slf: &Bound<'_, Self>,
        element: &Bound<'_, PyAny>,
        position: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<i64> {
        let position = match position {
            Some(position) if !position.is_none() => position.clone(),
            _ => slf.call_method1("getPositionFromElementUnsafe", (element,))?,
        };
        let found = AvlTree::getNodeByPosition(slf.as_super(), &position)?;
        if let Some(node) = found.filter(|node| payload_of(node).is(element)) {
            return Ok(counted(&node)?.payloadElementIndex);
        }
        for (place, held) in slf.try_iter()?.enumerate() {
            if held?.is(element) {
                return Ok(place as i64);
            }
        }
        Err(PyValueError::new_err(format!(
            "{} not in Tree at position {}.",
            element.str()?,
            position.str()?
        )))
    }

    /// The likely positions of elements.
    fn _getPositionsFromElements<'py>(
        slf: &Bound<'py, Self>,
        elements: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyList>> {
        let positions = PyList::empty(slf.py());
        for element in elements.try_iter()? {
            positions.append(slf.call_method1("getPositionFromElementUnsafe", (element?,))?)?;
        }
        Ok(positions)
    }

    /// Inserts an element or timespan, or several, at their own positions
    /// or at those given.
    #[pyo3(signature = (positionsOrElements, elements = None))]
    fn insert(
        slf: &Bound<'_, Self>,
        positionsOrElements: &Bound<'_, PyAny>,
        elements: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let py = slf.py();
        let initial_position = slf.call_method0("lowestPosition")?;
        let initial_end = slf.getattr("endTime")?;
        let (elements, positions) = match elements.filter(|elements| !elements.is_none()) {
            None => (positionsOrElements.clone(), None),
            Some(elements) => {
                let positions = positionsOrElements;
                let positions = if !list_like(positions) || positions.hasattr("shortRepr")? {
                    PyList::new(py, [positions])?.into_any()
                } else {
                    positions.clone()
                };
                (elements.clone(), Some(positions))
            }
        };
        let many = list_like(&elements)
            || elements.is_instance_of::<PySet>()
            || elements.is_instance_of::<pyo3::types::PyFrozenSet>();
        let elements = match many {
            true => PyList::new(py, elements.try_iter()?.collect::<PyResult<Vec<_>>>()?)?,
            false => PyList::new(py, [elements])?,
        };
        let positions = match positions {
            Some(positions) => positions,
            None => slf
                .call_method1("_getPositionsFromElements", (&elements,))?
                .into_any(),
        };
        for (place, element) in elements.iter().enumerate() {
            slf.call_method1("_insertCore", (positions.get_item(place)?, element))?;
        }
        slf.call_method1("_updateNodes", (initial_position, initial_end))?;
        Ok(())
    }

    /// Puts one element at a position, making the node, without counting
    /// again.
    fn _insertCore(
        slf: &Bound<'_, Self>,
        position: &Bound<'_, PyAny>,
        el: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        AvlTree::createNodeAtPosition(slf.as_super(), position)?;
        if let Some(node) = AvlTree::getNodeByPosition(slf.as_super(), position)? {
            node.borrow_mut().payload = el.clone().unbind();
        }
        Ok(())
    }

    /// The latest position, `-inf` for an empty tree.
    fn highestPosition<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        far_position(slf.as_any(), false)
    }

    /// The earliest position, `-inf` for an empty tree.
    fn lowestPosition<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        far_position(slf.as_any(), true)
    }

    /// The stream the tree is of.
    #[getter]
    pub(crate) fn source<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        unwrapped(slf.py(), slf.borrow().source.as_ref())
    }

    #[setter(source)]
    fn set_source(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        slf.borrow_mut().source = wrapped(value)?;
        Ok(())
    }

    /// The latest end of any element, `inf` for an empty tree.
    #[getter]
    fn endTime<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        match root_of(slf.as_any())? {
            Some(root) => Ok(counted(&root)?.endTimeHigh.bind(py).clone()),
            None => Ok(pyo3::types::PyFloat::new(py, f64::INFINITY).into_any()),
        }
    }
}

/// The payload of an `OffsetTree`'s node, a list.
fn listed<'py>(node: &Bound<'py, AvlNode>) -> PyResult<Bound<'py, PyList>> {
    Ok(payload_of(node).cast_into::<PyList>()?)
}

/// An `OffsetNode`'s own start and stop.
fn own_span(node: &Bound<'_, AvlNode>) -> PyResult<(i64, i64)> {
    let held = node.cast::<OffsetNode>()?.borrow();
    Ok((
        held.payloadElementsStartIndex,
        held.payloadElementsStopIndex,
    ))
}

/// The element of an `OffsetTree` at an index.
fn element_at<'py>(node: &Bound<'py, AvlNode>, index: i64) -> PyResult<Option<Bound<'py, PyAny>>> {
    let (start, stop) = own_span(node)?;
    if start <= index && index < stop {
        return Ok(Some(listed(node)?.get_item((index - start) as usize)?));
    }
    let next = match index < start {
        true => child(node, true),
        false => child(node, false),
    };
    match next {
        Some(next) => element_at(&next, index),
        None => Ok(None),
    }
}

/// The elements of an `OffsetTree` at the indices `start..stop`.
fn elements_in<'py>(
    node: Option<Bound<'py, AvlNode>>,
    start: i64,
    stop: i64,
    into: &Bound<'py, PyList>,
) -> PyResult<()> {
    let Some(node) = node else {
        return Ok(());
    };
    let (own_start, own_stop) = own_span(&node)?;
    if start < own_start {
        elements_in(child(&node, true), start, stop, into)?;
    }
    if start < own_stop && own_start < stop {
        let payload = listed(&node)?;
        let from = (start - own_start).max(0);
        let to = (stop - own_start).min(payload.len() as i64).max(from);
        for element in payload.get_slice(from as usize, to as usize).iter() {
            into.append(element)?;
        }
    }
    if own_stop <= stop {
        elements_in(child(&node, false), start, stop, into)?;
    }
    Ok(())
}

/// The ends of the elements under a node, by the tree's `elementEndTime`.
fn end_in<'py>(
    tree: &Bound<'py, PyAny>,
    element: &Bound<'py, PyAny>,
    node: &Bound<'py, AvlNode>,
) -> PyResult<Bound<'py, PyAny>> {
    tree.call_method1("elementEndTime", (element, node))
}

/// The elements under a node ending at an offset.
fn stopping_in<'py>(
    tree: &Bound<'py, PyAny>,
    node: Option<Bound<'py, AvlNode>>,
    offset: &Bound<'py, PyAny>,
    into: &Bound<'py, PyList>,
) -> PyResult<()> {
    let Some(node) = node else {
        return Ok(());
    };
    let (low, high) = {
        let held = counted(&node)?;
        (
            held.endTimeLow.bind(tree.py()).clone(),
            held.endTimeHigh.bind(tree.py()).clone(),
        )
    };
    if !(low.le(offset)? && offset.le(&high)?) {
        return Ok(());
    }
    stopping_in(tree, child(&node, true), offset, into)?;
    for element in listed(&node)?.iter() {
        if end_in(tree, &element, &node)?.eq(offset)? {
            into.append(element)?;
        }
    }
    stopping_in(tree, child(&node, false), offset, into)
}

/// The elements under a node sounding across an offset.
fn overlapping_in<'py>(
    tree: &Bound<'py, PyAny>,
    node: Option<Bound<'py, AvlNode>>,
    offset: &Bound<'py, PyAny>,
    into: &Bound<'py, PyList>,
) -> PyResult<()> {
    let Some(node) = node else {
        return Ok(());
    };
    let position = position_of(&node);
    let high = counted(&node)?.endTimeHigh.bind(tree.py()).clone();
    if position.lt(offset)? && offset.lt(&high)? {
        overlapping_in(tree, child(&node, true), offset, into)?;
        for element in listed(&node)?.iter() {
            if offset.lt(end_in(tree, &element, &node)?)? {
                into.append(element)?;
            }
        }
        return overlapping_in(tree, child(&node, false), offset, into);
    }
    if offset.le(&position)? {
        return overlapping_in(tree, child(&node, true), offset, into);
    }
    Ok(())
}

/// A tree of elements by offset, those at one offset in one node's list:
/// music21's `tree.trees.OffsetTree`.
#[pyclass(
    name = "OffsetTree",
    module = "music21.tree.trees",
    extends = ElementTree,
    subclass
)]
pub struct OffsetTree;

impl OffsetTree {
    /// An empty one of a stream.
    pub(crate) fn of(source: &Bound<'_, PyAny>) -> PyResult<PyClassInitializer<Self>> {
        Ok(ElementTree::of(source)?.add_subclass(Self))
    }
}

#[pymethods]
impl OffsetTree {
    #[new]
    #[pyo3(signature = (elements = None, source = None))]
    pub(crate) fn py_new(
        py: Python<'_>,
        elements: Option<&Bound<'_, PyAny>>,
        source: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(ElementTree::py_new(py, elements, source)?.add_subclass(Self))
    }

    #[classattr]
    fn nodeClass(py: Python<'_>) -> Py<PyType> {
        py.get_type::<OffsetNode>().unbind()
    }

    /// Whether an element is among those starting at its own `offset`.
    fn __contains__(slf: &Bound<'_, Self>, element: &Bound<'_, PyAny>) -> PyResult<bool> {
        let Ok(offset) = element.getattr("offset") else {
            return Err(TreeException::new_err(
                "element must be a Music21Object, i.e., must have offset",
            ));
        };
        let candidates = Self::elementsStartingAt(slf, &offset)?;
        candidates.contains(element)
    }

    /// An element by index, or a list of them by a slice.
    fn __getitem__<'py>(
        slf: &Bound<'py, Self>,
        i: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let tree = slf.as_any();
        if let Ok(slice) = i.cast::<PySlice>() {
            let found = PyList::empty(py);
            let (start, stop) = slice_in(tree, slice)?;
            elements_in(root_of(tree)?, start, stop, &found)?;
            return Ok(found.into_any());
        }
        let Ok(index) = i.extract::<i64>() else {
            return Err(PyTypeError::new_err(format!(
                "Indices must be integers or slices, got {}",
                i.str()?
            )));
        };
        let Some(root) = root_of(tree)? else {
            return Err(PyIndexError::new_err(""));
        };
        let found = element_at(&root, index_in(tree, index)?)?;
        Ok(found.unwrap_or_else(|| py.None().into_bound(py)))
    }

    /// Each element of each node, in order.
    fn __iter__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let elements = PyList::empty(slf.py());
        for node in nodes_of(slf.as_any())? {
            for element in listed(&node)?.iter() {
                elements.append(element)?;
            }
        }
        Ok(elements.try_iter()?.into_any())
    }

    /// Where an element held at a node ends: the node's position and the
    /// element's length.
    #[staticmethod]
    fn elementEndTime<'py>(
        el: &Bound<'py, PyAny>,
        node: &Bound<'py, AvlNode>,
    ) -> PyResult<Bound<'py, PyAny>> {
        position_of(node).add(el.getattr("duration")?.getattr("quarterLength")?)
    }

    /// Where an element likely stands: its `offset`.
    fn getPositionFromElementUnsafe<'py>(
        slf: &Bound<'py, Self>,
        el: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let _ = slf;
        el.getattr("offset")
    }

    /// Adds an element where the tree ends.
    fn append(slf: &Bound<'_, Self>, el: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let initial_position = slf.call_method0("lowestPosition")?;
        let mut end = slf.getattr("endTime")?;
        if end.extract::<f64>().is_ok_and(f64::is_infinite) {
            end = 0_i64.into_pyobject(py)?.into_any();
        }
        slf.call_method1("_insertCore", (end, el))?;
        slf.call_method1("_updateNodes", (initial_position, py.None()))?;
        Ok(())
    }

    /// How a node's payload is sorted: by an element's `SortTuple` without
    /// its `atEnd` and `offset`.
    #[staticmethod]
    fn _insertCorePayloadSortKey<'py>(x: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let sort = x.call_method0("sortTuple")?;
        sort.get_item(PySlice::new(x.py(), 2, isize::MAX, 1))
    }

    /// Puts one element in the node at a position, making the node, and
    /// sorts the node's payload, without counting again.
    fn _insertCore(
        slf: &Bound<'_, Self>,
        position: &Bound<'_, PyAny>,
        el: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let tree = slf.as_super().as_super();
        AvlTree::createNodeAtPosition(tree, position)?;
        let Some(node) = AvlTree::getNodeByPosition(tree, position)? else {
            return Ok(());
        };
        let payload = listed(&node)?;
        payload.append(el)?;
        let keywords = PyDict::new(slf.py());
        keywords.set_item("key", slf.getattr("_insertCorePayloadSortKey")?)?;
        payload.call_method("sort", (), Some(&keywords))?;
        Ok(())
    }

    /// A new tree of the same class holding the same elements.
    fn copy<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let made = slf.get_type().call0()?;
        let elements = PyList::new(slf.py(), slf.try_iter()?.collect::<PyResult<Vec<_>>>()?)?;
        made.call_method1("insert", (elements,))?;
        made.setattr("source", slf.getattr("source")?)?;
        made.setattr(
            "parentTrees",
            slf.getattr("parentTrees")?.call_method0("copy")?,
        )?;
        Ok(made)
    }

    /// The elements starting at a position.
    pub(crate) fn elementsStartingAt<'py>(
        slf: &Bound<'py, Self>,
        position: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let py = slf.py();
        let tree = slf.as_super().as_super();
        let Some(node) = AvlTree::getNodeByPosition(tree, position)? else {
            return Ok(PyTuple::empty(py));
        };
        let payload = payload_of(&node);
        if let Ok(list) = payload.cast::<PyList>() {
            return Ok(list.to_tuple());
        }
        if payload.is_none() {
            return Ok(PyTuple::empty(py));
        }
        PyTuple::new(py, [payload])
    }

    /// The elements ending at an offset, by where they start.
    fn elementsStoppingAt<'py>(
        slf: &Bound<'py, Self>,
        offset: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let found = PyList::empty(slf.py());
        stopping_in(slf.as_any(), root_of(slf.as_any())?, offset, &found)?;
        Ok(found.to_tuple())
    }

    /// The elements sounding across an offset: started before it, ending
    /// after.
    fn elementsOverlappingOffset<'py>(
        slf: &Bound<'py, Self>,
        offset: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let found = PyList::empty(slf.py());
        overlapping_in(slf.as_any(), root_of(slf.as_any())?, offset, &found)?;
        Ok(found.to_tuple())
    }

    /// Takes out an element or several, from their own offsets or those
    /// given; without `runUpdate` the tree is left uncounted.
    #[pyo3(signature = (elements, offsets = None, runUpdate = true))]
    fn removeElements(
        slf: &Bound<'_, Self>,
        elements: &Bound<'_, PyAny>,
        offsets: Option<&Bound<'_, PyAny>>,
        runUpdate: bool,
    ) -> PyResult<()> {
        let py = slf.py();
        let initial_position = slf.call_method0("lowestPosition")?;
        let initial_end = slf.getattr("endTime")?;
        let elements = match elements.hasattr("offset")? {
            true => PyList::new(py, [elements])?.into_any(),
            false => elements.clone(),
        };
        let offsets = match offsets.filter(|offsets| !offsets.is_none()) {
            Some(offsets) if !list_like(offsets) => Some(PyList::new(py, [offsets])?.into_any()),
            other => other.cloned(),
        };
        if let Some(offsets) = &offsets
            && offsets.len()? != elements.len()?
        {
            return Err(TreeException::new_err(
                "Number of elements and number of offsets must be the same",
            ));
        }
        let element_tree = slf.as_super();
        for (place, element) in elements.try_iter()?.enumerate() {
            let element = element?;
            let position = match &offsets {
                Some(offsets) => offsets.get_item(place)?,
                None => element.getattr("offset")?,
            };
            ElementTree::_removeElementAtPosition(element_tree, &element, &position)?;
        }
        edited(slf.as_any());
        if runUpdate {
            slf.call_method1("_updateNodes", (initial_position, initial_end))?;
        }
        Ok(())
    }

    /// Each offset an element starts at, once, in order.
    fn allOffsets<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyTuple>> {
        let mut offsets = Vec::new();
        for node in nodes_of(slf.as_any())? {
            offsets.push(offset_of(&position_of(&node))?);
        }
        PyTuple::new(slf.py(), offsets)
    }

    /// Each offset an element starts or ends at, once, in order.
    fn allTimePoints<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyTuple>> {
        let py = slf.py();
        let points = PySet::empty(py)?;
        for node in nodes_of(slf.as_any())? {
            time_points(&node, &points)?;
        }
        let sorted = py.import("builtins")?.getattr("sorted")?.call1((points,))?;
        Ok(sorted.cast_into::<PyList>()?.to_tuple())
    }

    /// The offsets an element starts at, or with `includeStopPoints` also
    /// ends at, where another sounds across; with `returnVerticality` each
    /// as a dictionary of it and its verticality.
    #[pyo3(signature = (includeStopPoints = false, returnVerticality = false))]
    fn overlapTimePoints<'py>(
        slf: &Bound<'py, Self>,
        includeStopPoints: bool,
        returnVerticality: bool,
    ) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let points = match includeStopPoints {
            true => Self::allTimePoints(slf)?,
            false => Self::allOffsets(slf)?,
        };
        let out = PyList::empty(py);
        for point in points.iter() {
            let overlapping = slf.call_method1("elementsOverlappingOffset", (&point,))?;
            if !overlapping.is_truthy()? {
                continue;
            }
            if !returnVerticality {
                out.append(point)?;
                continue;
            }
            let entry = PyDict::new(py);
            entry.set_item(&point, slf.call_method1("getVerticalityAt", (&point,))?)?;
            out.append(entry)?;
        }
        Ok(out)
    }

    /// What starts, stops and sounds across an offset. Only a timespan
    /// tree read from a stream answers it.
    fn getVerticalityAt(slf: &Bound<'_, Self>, offset: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let _ = (slf, offset);
        Err(PyNotImplementedError::new_err(
            "a verticality of an OffsetTree is not ported to music21-rs; \
             a TimespanTree read by asTimespans() has them",
        ))
    }

    /// The offsets more than one element starts at, each with a list of
    /// those elements.
    fn simultaneityDict<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyDict>> {
        let found = PyDict::new(slf.py());
        for node in nodes_of(slf.as_any())? {
            let payload = listed(&node)?;
            if payload.len() > 1 {
                found.set_item(position_of(&node), payload.get_slice(0, payload.len()))?;
            }
        }
        Ok(found)
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ElementTree>()?;
    m.add_class::<OffsetTree>()?;
    Ok(())
}
