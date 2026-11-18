//! music21's `tree.core`: `AVLNode` and `AVLTree`, the self-balancing tree
//! every tree of music21's is built on.
//!
//! The nodes are Python objects music21's code reaches into -- it sets
//! `leftChild` by hand and calls `update()` -- so they are kept as such,
//! and the crate's `tree::core` steps run over them through [`Nodes`].

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyList, PyType};

use music21::tree::core::{self as avl, AvlNodes, Compare, Shape, TreeError};

use crate::exceptions21::TreeException;
use crate::prebase::ProtoM21Object;

/// A failure while walking Python nodes: music21's `TreeException` for a
/// tree out of shape, or whatever Python raised.
pub(crate) struct Failed(PyErr);

impl From<TreeError> for Failed {
    fn from(error: TreeError) -> Self {
        Self(TreeException::new_err(error.to_string()))
    }
}

impl From<PyErr> for Failed {
    fn from(error: PyErr) -> Self {
        Self(error)
    }
}

impl From<Failed> for PyErr {
    fn from(failed: Failed) -> Self {
        failed.0
    }
}

/// The Python nodes of a tree, as the crate's steps read and write them.
/// New nodes are made of `class`, the tree's `nodeClass`.
pub(crate) struct Nodes<'py> {
    pub(crate) class: Option<Bound<'py, PyAny>>,
}

impl<'py> Nodes<'py> {
    /// Nodes that make new ones of a tree's `nodeClass`.
    pub(crate) fn of_tree(tree: &Bound<'py, PyAny>) -> PyResult<Self> {
        Ok(Self {
            class: Some(tree.getattr("nodeClass")?),
        })
    }

    /// Nodes that are only walked and relinked.
    pub(crate) fn linked() -> Self {
        Self { class: None }
    }
}

/// A child as a bound node.
fn bound<'py>(py: Python<'py>, child: Option<&Py<AvlNode>>) -> Option<Bound<'py, AvlNode>> {
    child.map(|child| child.bind(py).clone())
}

impl<'py> AvlNodes for Nodes<'py> {
    type Node = Bound<'py, AvlNode>;
    type Position = Bound<'py, PyAny>;
    type Error = Failed;

    fn left(&self, node: &Self::Node) -> Result<Option<Self::Node>, Failed> {
        Ok(bound(node.py(), node.borrow().left.as_ref()))
    }

    fn right(&self, node: &Self::Node) -> Result<Option<Self::Node>, Failed> {
        Ok(bound(node.py(), node.borrow().right.as_ref()))
    }

    fn set_left(&mut self, node: &Self::Node, child: Option<Self::Node>) -> Result<(), Failed> {
        node.borrow_mut().left = child.map(Bound::unbind);
        Ok(())
    }

    fn set_right(&mut self, node: &Self::Node, child: Option<Self::Node>) -> Result<(), Failed> {
        node.borrow_mut().right = child.map(Bound::unbind);
        Ok(())
    }

    fn shape(&self, node: &Self::Node) -> Result<Shape, Failed> {
        let held = node.borrow();
        Ok(Shape {
            height: held.height,
            balance: held.balance,
        })
    }

    fn set_shape(&mut self, node: &Self::Node, shape: Shape) -> Result<(), Failed> {
        let mut held = node.borrow_mut();
        held.height = shape.height;
        held.balance = shape.balance;
        Ok(())
    }

    fn position(&self, node: &Self::Node) -> Result<Self::Position, Failed> {
        Ok(node.borrow().position.bind(node.py()).clone())
    }

    fn holds(
        &self,
        node: &Self::Node,
        compare: Compare,
        position: &Self::Position,
    ) -> Result<bool, Failed> {
        let held = self.position(node)?;
        let answer = match compare {
            Compare::Less => held.lt(position)?,
            Compare::LessOrEqual => held.le(position)?,
            Compare::Equal => held.eq(position)?,
            Compare::Greater => held.gt(position)?,
        };
        Ok(answer)
    }

    fn create(&mut self, position: &Self::Position) -> Result<Self::Node, Failed> {
        let made = match &self.class {
            Some(class) => class.call1((position,))?,
            None => {
                let py = position.py();
                Bound::new(py, AvlNode::new(position.clone().unbind(), py.None()))?.into_any()
            }
        };
        Ok(made.cast_into::<AvlNode>().map_err(PyErr::from)?)
    }

    fn move_attributes(&mut self, from: &Self::Node, to: &Self::Node) -> Result<(), Failed> {
        let (position, payload) = {
            let held = from.borrow();
            (
                held.position.clone_ref(from.py()),
                held.payload.clone_ref(from.py()),
            )
        };
        let mut target = to.borrow_mut();
        target.position = position;
        target.payload = payload;
        Ok(())
    }
}

/// A position as music21 shows it in a node: a `SortTuple` by its short
/// form, anything else as text.
pub(crate) fn shown_position(position: &Bound<'_, PyAny>) -> PyResult<String> {
    if position.hasattr("shortRepr")? {
        return Ok(position.call_method0("shortRepr")?.str()?.to_string());
    }
    Ok(position.str()?.to_string())
}

/// `Class[PayloadType]`, as music21's generic classes are written: a
/// `types.GenericAlias`, which a class statement takes as the class.
pub(crate) fn generic_alias<'py>(
    class: &Bound<'py, PyType>,
    item: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = class.py();
    py.import("types")?
        .getattr("GenericAlias")?
        .call1((class, item))
}

/// An AVL tree node holding a position and a payload, and nothing about
/// music: music21's `tree.core.AVLNode`.
#[pyclass(name = "AVLNode", module = "music21.tree.core", subclass, weakref)]
pub struct AvlNode {
    /// Where the node stands: an offset, or a `SortTuple`.
    #[pyo3(get, set)]
    pub(crate) position: Py<PyAny>,
    /// What it holds.
    #[pyo3(get, set)]
    pub(crate) payload: Py<PyAny>,
    /// Its right subtree's height less its left's.
    #[pyo3(get, set)]
    pub(crate) balance: i64,
    /// The height of the subtree rooted on it.
    #[pyo3(get, set)]
    pub(crate) height: i64,
    #[pyo3(get, set, name = "leftChild")]
    pub(crate) left: Option<Py<AvlNode>>,
    #[pyo3(get, set, name = "rightChild")]
    pub(crate) right: Option<Py<AvlNode>>,
}

impl AvlNode {
    /// A node at a position, holding a payload.
    pub(crate) fn new(position: Py<PyAny>, payload: Py<PyAny>) -> Self {
        Self {
            position,
            payload,
            balance: 0,
            height: 0,
            left: None,
            right: None,
        }
    }
}

#[pymethods]
impl AvlNode {
    #[new]
    #[pyo3(signature = (position, payload = None))]
    fn py_new(py: Python<'_>, position: Py<PyAny>, payload: Option<Py<PyAny>>) -> Self {
        Self::new(position, payload.unwrap_or_else(|| py.None()))
    }

    #[classmethod]
    fn __class_getitem__<'py>(
        cls: &Bound<'py, PyType>,
        item: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        generic_alias(cls, item)
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.position)?;
        visit.call(&self.payload)?;
        if let Some(left) = &self.left {
            visit.call(left)?;
        }
        if let Some(right) = &self.right {
            visit.call(right)?;
        }
        Ok(())
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let py = slf.py();
        let held = slf.borrow();
        let height = |child: &Option<Py<AvlNode>>| match child {
            Some(child) => child.bind(py).borrow().height.to_string(),
            None => "None".to_string(),
        };
        Ok(format!(
            "<{}: Start:{} Height:{} L:{} R:{}>",
            slf.get_type().name()?,
            held.position.bind(py).str()?,
            held.height,
            height(&held.left),
            height(&held.right),
        ))
    }

    /// Gives another node this one's position and payload, for a removal
    /// that swaps one node for another.
    fn moveAttributes(slf: &Bound<'_, Self>, other: &Bound<'_, Self>) -> PyResult<()> {
        Ok(Nodes::linked().move_attributes(slf, other)?)
    }

    /// This node and those under it, one to a line, indented by depth.
    pub(crate) fn debug(slf: &Bound<'_, Self>) -> PyResult<String> {
        Ok(Self::_getDebugPieces(slf)?.join("\n"))
    }

    /// The lines of `debug`.
    fn _getDebugPieces(slf: &Bound<'_, Self>) -> PyResult<Vec<String>> {
        let mut lines = vec![slf.repr()?.to_string()];
        let children = {
            let held = slf.borrow();
            [
                (
                    "L",
                    held.left.as_ref().map(|child| child.clone_ref(slf.py())),
                ),
                (
                    "R",
                    held.right.as_ref().map(|child| child.clone_ref(slf.py())),
                ),
            ]
        };
        for (side, child) in children {
            let Some(child) = child else {
                continue;
            };
            let below = Self::_getDebugPieces(child.bind(slf.py()))?;
            let mut below = below.into_iter();
            if let Some(first) = below.next() {
                lines.push(format!("\t{side}: {first}"));
            }
            lines.extend(below.map(|line| format!("\t{line}")));
        }
        Ok(lines)
    }

    /// Sets the height and balance from the children's, which are trusted.
    fn update(slf: &Bound<'_, Self>) -> PyResult<()> {
        Ok(avl::update(&mut Nodes::linked(), slf)?)
    }

    /// Makes the left child the centre; answers it.
    fn rotateLeftLeft<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, Self>> {
        Ok(avl::rotate_left_left(&mut Nodes::linked(), slf)?)
    }

    /// Rotates the left child right, then this node left; answers the new
    /// centre.
    fn rotateLeftRight<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, Self>> {
        Ok(avl::rotate_left_right(&mut Nodes::linked(), slf)?)
    }

    /// Rotates the right child left, then this node right; answers the new
    /// centre.
    fn rotateRightLeft<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, Self>> {
        Ok(avl::rotate_right_left(&mut Nodes::linked(), slf)?)
    }

    /// Makes the right child the centre; answers it.
    fn rotateRightRight<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, Self>> {
        Ok(avl::rotate_right_right(&mut Nodes::linked(), slf)?)
    }

    /// Rebalances the subtree rooted here; answers its new centre.
    fn rebalance<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, Self>> {
        Ok(avl::rebalance(&mut Nodes::linked(), slf)?)
    }
}

/// A tree's root node.
pub(crate) fn root_of<'py>(tree: &Bound<'py, PyAny>) -> PyResult<Option<Bound<'py, AvlNode>>> {
    let tree = tree.cast::<AvlTree>()?;
    Ok(bound(tree.py(), tree.borrow().root.as_ref()))
}

/// Sets a tree's root node.
pub(crate) fn set_root(tree: &Bound<'_, PyAny>, root: Option<Bound<'_, AvlNode>>) -> PyResult<()> {
    tree.cast::<AvlTree>()?.borrow_mut().root = root.map(Bound::unbind);
    Ok(())
}

/// A tree's nodes, left to right.
pub(crate) fn nodes_of<'py>(tree: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, AvlNode>>> {
    Ok(avl::in_order(&Nodes::linked(), root_of(tree)?)?)
}

/// The position of a node, or none.
fn position_of<'py>(node: Option<Bound<'py, AvlNode>>) -> Option<Bound<'py, PyAny>> {
    node.map(|node| node.borrow().position.bind(node.py()).clone())
}

/// A tree of `AVLNode`s by position, for subclasses to hold music21's
/// objects in: music21's `tree.core.AVLTree`.
#[pyclass(
    name = "AVLTree",
    module = "music21.tree.core",
    extends = ProtoM21Object,
    subclass,
    weakref
)]
#[derive(Default)]
pub struct AvlTree {
    #[pyo3(get, set, name = "rootNode")]
    pub(crate) root: Option<Py<AvlNode>>,
}

#[pymethods]
impl AvlTree {
    #[new]
    fn py_new() -> PyClassInitializer<Self> {
        crate::base::built(Self::default())
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        if let Some(root) = &self.root {
            visit.call(root)?;
        }
        Ok(())
    }

    #[classmethod]
    fn __class_getitem__<'py>(
        cls: &Bound<'py, PyType>,
        item: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        generic_alias(cls, item)
    }

    /// The class of the nodes this tree makes.
    #[classattr]
    fn nodeClass(py: Python<'_>) -> Py<PyType> {
        py.get_type::<AvlNode>().unbind()
    }

    /// The nodes, left to right.
    pub(crate) fn __iter__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let nodes = PyList::new(slf.py(), nodes_of(slf.as_any())?)?;
        Ok(nodes.try_iter()?.into_any())
    }

    /// Replaces the tree with a balanced one of (position, payload) pairs
    /// sorted by strictly rising position.
    fn populateFromSortedList(
        slf: &Bound<'_, Self>,
        listOfTuples: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let items: Vec<Bound<'_, PyAny>> = listOfTuples.try_iter()?.collect::<PyResult<_>>()?;
        let class = slf.getattr("nodeClass")?;
        let mut make = |_: &mut Nodes<'_>, placed: avl::Placement| -> Result<_, Failed> {
            let item = &items[placed.middle];
            let made = class.call1((item.get_item(0)?, item.get_item(1)?))?;
            Ok(made.cast_into::<AvlNode>().map_err(PyErr::from)?)
        };
        let root = avl::build_sorted(&mut Nodes::linked(), items.len(), &mut make)?;
        set_root(slf.as_any(), root)
    }

    /// Makes a node at a position, unless one is there, and rebalances.
    pub(crate) fn createNodeAtPosition(
        slf: &Bound<'_, Self>,
        position: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let mut nodes = Nodes::of_tree(slf.as_any())?;
        let root = avl::insert(&mut nodes, root_of(slf.as_any())?, position)?;
        set_root(slf.as_any(), Some(root))
    }

    /// The nodes, one to a line, indented by depth; nothing for an empty
    /// tree.
    pub(crate) fn debug(slf: &Bound<'_, Self>) -> PyResult<String> {
        match root_of(slf.as_any())? {
            Some(root) => AvlNode::debug(&root),
            None => Ok(String::new()),
        }
    }

    /// The node at a position, or `None`.
    pub(crate) fn getNodeByPosition<'py>(
        slf: &Bound<'py, Self>,
        position: &Bound<'py, PyAny>,
    ) -> PyResult<Option<Bound<'py, AvlNode>>> {
        Ok(avl::find(
            &Nodes::linked(),
            root_of(slf.as_any())?,
            position,
        )?)
    }

    /// The first node after a position, or `None`.
    fn getNodeAfter<'py>(
        slf: &Bound<'py, Self>,
        position: &Bound<'py, PyAny>,
    ) -> PyResult<Option<Bound<'py, AvlNode>>> {
        Ok(avl::node_after(
            &Nodes::linked(),
            root_of(slf.as_any())?,
            position,
        )?)
    }

    /// The first position after one given, or `None`.
    fn getPositionAfter<'py>(
        slf: &Bound<'py, Self>,
        position: &Bound<'py, PyAny>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        Ok(position_of(Self::getNodeAfter(slf, position)?))
    }

    /// The last node before a position, or `None`.
    fn getNodeBefore<'py>(
        slf: &Bound<'py, Self>,
        position: &Bound<'py, PyAny>,
    ) -> PyResult<Option<Bound<'py, AvlNode>>> {
        Ok(avl::node_before(
            &Nodes::linked(),
            root_of(slf.as_any())?,
            position,
        )?)
    }

    /// The last position before one given, or `None`.
    fn getPositionBefore<'py>(
        slf: &Bound<'py, Self>,
        position: &Bound<'py, PyAny>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        Ok(position_of(Self::getNodeBefore(slf, position)?))
    }

    /// Takes out the node at a position, with its payload, and rebalances;
    /// a position no node stands at changes nothing.
    pub(crate) fn removeNode(slf: &Bound<'_, Self>, position: &Bound<'_, PyAny>) -> PyResult<()> {
        let root = avl::remove(&mut Nodes::linked(), root_of(slf.as_any())?, position)?;
        set_root(slf.as_any(), root)
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<AvlNode>()?;
    m.add_class::<AvlTree>()?;
    Ok(())
}
