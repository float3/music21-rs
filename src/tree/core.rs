//! music21's `tree.core`: the self-balancing AVL tree its element, offset
//! and timespan trees are built on.
//!
//! The steps -- placing a node, rotating and rebalancing, finding the node
//! at, after or before a position, taking one out, building a balanced tree
//! from a sorted list, counting the elements under each node -- are written
//! once over [`AvlNodes`], so they work on nodes however they are stored.
//! [`AvlTree`] keeps its nodes in a list; the Python package's nodes are
//! Python objects music21's code can reach into.
//!
//! ```
//! use music21::tree::core::AvlTree;
//!
//! let mut tree = AvlTree::new();
//! for position in [20, 10, 5, 30] {
//!     tree.insert(position, position * 2);
//! }
//! assert_eq!(tree.root_position(), Some(&10));
//! assert_eq!(tree.height(), Some(2));
//! assert_eq!(tree.position_after(&10), Some(&20));
//! assert_eq!(tree.position_before(&10), Some(&5));
//! assert_eq!(tree.get(&30), Some(&60));
//! ```

use std::fmt;

/// How a node's position stands to a position given: `node.position <op>
/// position`, as music21 asks it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Compare {
    /// `<`
    Less,
    /// `<=`
    LessOrEqual,
    /// `==`
    Equal,
    /// `>`
    Greater,
}

/// What went wrong in a tree whose nodes were linked by hand.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TreeError {
    /// A rotation needs a child the node does not have.
    MissingChild,
    /// A node is still out of balance after rebalancing.
    Unbalanced,
}

impl fmt::Display for TreeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChild => formatter.write_str("the rotation needs a child the node lacks"),
            Self::Unbalanced => formatter.write_str(
                "Somehow Nodes are still not balanced. node.balance must be between -1 and 1",
            ),
        }
    }
}

impl std::error::Error for TreeError {}

/// A node's height and balance: music21's `height` and `balance`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Shape {
    /// The height of the subtree rooted on the node; a leaf's is 0.
    pub height: i64,
    /// The right subtree's height less the left's.
    pub balance: i64,
}

/// Where the elements under a node stand in the tree's order, as slice
/// bounds: music21's `subtreeElementsStartIndex`, its payload's start and
/// stop, and `subtreeElementsStopIndex`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Indices {
    /// The first element of the subtree.
    pub subtree_start: usize,
    /// The first element of the node's own payload.
    pub payload_start: usize,
    /// One past its payload's last.
    pub payload_stop: usize,
    /// One past the subtree's last.
    pub subtree_stop: usize,
}

/// One node of a balanced tree being built from a sorted list: the item at
/// `middle` of the items `start..stop`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Placement {
    /// The item the node holds.
    pub middle: usize,
    /// The first item of its subtree.
    pub start: usize,
    /// One past the last.
    pub stop: usize,
}

/// The nodes of an AVL tree, however they are stored: what the steps of
/// this module read and write of them.
pub trait AvlNodes {
    /// A handle on one node.
    type Node: Clone;
    /// Where a node stands.
    type Position;
    /// What reading or writing a node can fail with.
    type Error: From<TreeError>;

    /// The node's left child.
    fn left(&self, node: &Self::Node) -> Result<Option<Self::Node>, Self::Error>;

    /// Its right child.
    fn right(&self, node: &Self::Node) -> Result<Option<Self::Node>, Self::Error>;

    /// Sets its left child.
    fn set_left(&mut self, node: &Self::Node, child: Option<Self::Node>)
    -> Result<(), Self::Error>;

    /// Sets its right child.
    fn set_right(
        &mut self,
        node: &Self::Node,
        child: Option<Self::Node>,
    ) -> Result<(), Self::Error>;

    /// Its height and balance as last updated.
    fn shape(&self, node: &Self::Node) -> Result<Shape, Self::Error>;

    /// Sets its height and balance.
    fn set_shape(&mut self, node: &Self::Node, shape: Shape) -> Result<(), Self::Error>;

    /// Its position.
    fn position(&self, node: &Self::Node) -> Result<Self::Position, Self::Error>;

    /// Whether `node.position <compare> position`.
    fn holds(
        &self,
        node: &Self::Node,
        compare: Compare,
        position: &Self::Position,
    ) -> Result<bool, Self::Error>;

    /// A new node at a position, holding nothing.
    fn create(&mut self, position: &Self::Position) -> Result<Self::Node, Self::Error>;

    /// Moves what one node holds -- its position and payload, not its
    /// links or shape -- onto another: music21's `moveAttributes`.
    fn move_attributes(&mut self, from: &Self::Node, to: &Self::Node) -> Result<(), Self::Error>;
}

/// Nodes that count the elements they hold, for [`update_indices`].
pub trait CountedNodes: AvlNodes {
    /// How many elements the node's own payload holds.
    fn payload_count(&self, node: &Self::Node) -> Result<usize, Self::Error>;

    /// Stores where its elements and its subtree's stand.
    fn set_indices(&mut self, node: &Self::Node, indices: Indices) -> Result<(), Self::Error>;
}

/// Sets a node's height and balance from its children's: music21's
/// `AVLNode.update`.
pub fn update<N: AvlNodes>(nodes: &mut N, node: &N::Node) -> Result<(), N::Error> {
    let left = child_height(nodes, nodes.left(node)?)?;
    let right = child_height(nodes, nodes.right(node)?)?;
    let shape = Shape {
        height: left.max(right) + 1,
        balance: right - left,
    };
    nodes.set_shape(node, shape)
}

/// A child's height, -1 for none.
fn child_height<N: AvlNodes>(nodes: &N, child: Option<N::Node>) -> Result<i64, N::Error> {
    match child {
        Some(child) => Ok(nodes.shape(&child)?.height),
        None => Ok(-1),
    }
}

/// A child a rotation needs.
fn needed<N: AvlNodes>(child: Option<N::Node>) -> Result<N::Node, N::Error> {
    child.ok_or_else(|| TreeError::MissingChild.into())
}

/// Makes the left child the centre, the node its right child: music21's
/// `rotateLeftLeft`. Answers the new centre.
pub fn rotate_left_left<N: AvlNodes>(nodes: &mut N, node: &N::Node) -> Result<N::Node, N::Error> {
    let next = needed::<N>(nodes.left(node)?)?;
    let inner = nodes.right(&next)?;
    nodes.set_left(node, inner)?;
    update(nodes, node)?;
    nodes.set_right(&next, Some(node.clone()))?;
    update(nodes, &next)?;
    Ok(next)
}

/// The mirror of [`rotate_left_left`]: music21's `rotateRightRight`.
pub fn rotate_right_right<N: AvlNodes>(nodes: &mut N, node: &N::Node) -> Result<N::Node, N::Error> {
    let next = needed::<N>(nodes.right(node)?)?;
    let inner = nodes.left(&next)?;
    nodes.set_right(node, inner)?;
    update(nodes, node)?;
    nodes.set_left(&next, Some(node.clone()))?;
    update(nodes, &next)?;
    Ok(next)
}

/// The left child rotated right, then the node left: music21's
/// `rotateLeftRight`.
pub fn rotate_left_right<N: AvlNodes>(nodes: &mut N, node: &N::Node) -> Result<N::Node, N::Error> {
    let left = needed::<N>(nodes.left(node)?)?;
    let rotated = rotate_right_right(nodes, &left)?;
    nodes.set_left(node, Some(rotated))?;
    update(nodes, node)?;
    rotate_left_left(nodes, node)
}

/// The right child rotated left, then the node right: music21's
/// `rotateRightLeft`.
pub fn rotate_right_left<N: AvlNodes>(nodes: &mut N, node: &N::Node) -> Result<N::Node, N::Error> {
    let right = needed::<N>(nodes.right(node)?)?;
    let rotated = rotate_left_left(nodes, &right)?;
    nodes.set_right(node, Some(rotated))?;
    update(nodes, node)?;
    rotate_right_right(nodes, node)
}

/// Rebalances the subtree rooted on a node, answering its new centre:
/// music21's `rebalance`.
pub fn rebalance<N: AvlNodes>(nodes: &mut N, node: &N::Node) -> Result<N::Node, N::Error> {
    let balance = nodes.shape(node)?.balance;
    let centre = if balance > 1 {
        let right = needed::<N>(nodes.right(node)?)?;
        if nodes.shape(&right)?.balance >= 0 {
            rotate_right_right(nodes, node)?
        } else {
            rotate_right_left(nodes, node)?
        }
    } else if balance < -1 {
        let left = needed::<N>(nodes.left(node)?)?;
        if nodes.shape(&left)?.balance <= 0 {
            rotate_left_left(nodes, node)?
        } else {
            rotate_left_right(nodes, node)?
        }
    } else {
        node.clone()
    };

    let balance = nodes.shape(&centre)?.balance;
    if !(-1..=1).contains(&balance) {
        return Err(TreeError::Unbalanced.into());
    }
    Ok(centre)
}

/// Places a node at a position under `root` unless one is there, answering
/// the new root: music21's `createNodeAtPosition`.
pub fn insert<N: AvlNodes>(
    nodes: &mut N,
    root: Option<N::Node>,
    position: &N::Position,
) -> Result<N::Node, N::Error> {
    let Some(node) = root else {
        return nodes.create(position);
    };

    // Down to where the position belongs, rebalancing on the way back up.
    if nodes.holds(&node, Compare::Greater, position)? {
        let left = nodes.left(&node)?;
        let placed = insert(nodes, left, position)?;
        nodes.set_left(&node, Some(placed))?;
        update(nodes, &node)?;
    } else if nodes.holds(&node, Compare::Less, position)? {
        let right = nodes.right(&node)?;
        let placed = insert(nodes, right, position)?;
        nodes.set_right(&node, Some(placed))?;
        update(nodes, &node)?;
    }
    rebalance(nodes, &node)
}

/// The node at a position: music21's `getNodeByPosition`.
pub fn find<N: AvlNodes>(
    nodes: &N,
    root: Option<N::Node>,
    position: &N::Position,
) -> Result<Option<N::Node>, N::Error> {
    let mut current = root;
    while let Some(node) = current {
        if nodes.holds(&node, Compare::Equal, position)? {
            return Ok(Some(node));
        }
        let left = nodes.left(&node)?;
        if left.is_some() && nodes.holds(&node, Compare::Greater, position)? {
            current = left;
            continue;
        }
        let right = nodes.right(&node)?;
        if right.is_some() && nodes.holds(&node, Compare::Less, position)? {
            current = right;
            continue;
        }
        return Ok(None);
    }
    Ok(None)
}

/// The first node after a position: music21's `getNodeAfter`.
pub fn node_after<N: AvlNodes>(
    nodes: &N,
    root: Option<N::Node>,
    position: &N::Position,
) -> Result<Option<N::Node>, N::Error> {
    let Some(node) = root else {
        return Ok(None);
    };
    let right = nodes.right(&node)?;
    if right.is_some() && nodes.holds(&node, Compare::LessOrEqual, position)? {
        return node_after(nodes, right, position);
    }
    if nodes.holds(&node, Compare::Greater, position)? {
        let left = nodes.left(&node)?;
        return Ok(node_after(nodes, left, position)?.or(Some(node)));
    }
    Ok(None)
}

/// The last node before a position: music21's `getNodeBefore`.
pub fn node_before<N: AvlNodes>(
    nodes: &N,
    root: Option<N::Node>,
    position: &N::Position,
) -> Result<Option<N::Node>, N::Error> {
    let Some(node) = root else {
        return Ok(None);
    };
    if nodes.holds(&node, Compare::Less, position)? {
        let right = nodes.right(&node)?;
        return Ok(node_before(nodes, right, position)?.or(Some(node)));
    }
    let left = nodes.left(&node)?;
    if left.is_some() {
        return node_before(nodes, left, position);
    }
    Ok(None)
}

/// Takes out the node at a position, answering the new root: music21's
/// `removeNode`. A node with two children takes on what the leftmost node
/// of its right subtree holds, and that node goes instead.
pub fn remove<N: AvlNodes>(
    nodes: &mut N,
    root: Option<N::Node>,
    position: &N::Position,
) -> Result<Option<N::Node>, N::Error> {
    let Some(node) = root else {
        return Ok(None);
    };
    let left = nodes.left(&node)?;
    let right = nodes.right(&node)?;

    let kept = if nodes.holds(&node, Compare::Equal, position)? {
        match (left, right) {
            (Some(_), Some(right)) => {
                let mut next = right.clone();
                while let Some(further) = nodes.left(&next)? {
                    next = further;
                }
                nodes.move_attributes(&next, &node)?;
                let moved = nodes.position(&next)?;
                let rest = remove(nodes, Some(right), &moved)?;
                nodes.set_right(&node, rest)?;
                update(nodes, &node)?;
                Some(node)
            }
            (left, right) => left.or(right),
        }
    } else if nodes.holds(&node, Compare::Greater, position)? {
        let rest = remove(nodes, left, position)?;
        nodes.set_left(&node, rest)?;
        update(nodes, &node)?;
        Some(node)
    } else if nodes.holds(&node, Compare::Less, position)? {
        let rest = remove(nodes, right, position)?;
        nodes.set_right(&node, rest)?;
        update(nodes, &node)?;
        Some(node)
    } else {
        Some(node)
    };

    match kept {
        Some(node) => Ok(Some(rebalance(nodes, &node)?)),
        None => Ok(None),
    }
}

/// Every node under `root`, left to right.
pub fn in_order<N: AvlNodes>(nodes: &N, root: Option<N::Node>) -> Result<Vec<N::Node>, N::Error> {
    let mut found = Vec::new();
    let mut pending = Vec::new();
    let mut current = root;
    loop {
        while let Some(node) = current {
            current = nodes.left(&node)?;
            pending.push(node);
        }
        let Some(node) = pending.pop() else {
            return Ok(found);
        };
        current = nodes.right(&node)?;
        found.push(node);
    }
}

/// A balanced tree of `count` sorted items, each node made by `make` from
/// where its item stands: music21's `populateFromSortedList`. Answers the
/// root.
pub fn build_sorted<N, F>(
    nodes: &mut N,
    count: usize,
    make: &mut F,
) -> Result<Option<N::Node>, N::Error>
where
    N: AvlNodes,
    F: FnMut(&mut N, Placement) -> Result<N::Node, N::Error>,
{
    build_range(nodes, 0, count, make)
}

/// The balanced subtree of the items `start..stop`.
fn build_range<N, F>(
    nodes: &mut N,
    start: usize,
    stop: usize,
    make: &mut F,
) -> Result<Option<N::Node>, N::Error>
where
    N: AvlNodes,
    F: FnMut(&mut N, Placement) -> Result<N::Node, N::Error>,
{
    if start >= stop {
        return Ok(None);
    }
    let middle = start + (stop - start) / 2;
    let node = make(
        nodes,
        Placement {
            middle,
            start,
            stop,
        },
    )?;
    let left = build_range(nodes, start, middle, make)?;
    nodes.set_left(&node, left)?;
    let right = build_range(nodes, middle + 1, stop, make)?;
    nodes.set_right(&node, right)?;
    update(nodes, &node)?;
    Ok(Some(node))
}

/// Counts the elements under each node from `node` down and stores where
/// each node's stand: music21's `updateIndices`. `parent_stop` is where the
/// elements before the subtree end, if any come before it.
pub fn update_indices<N: CountedNodes>(
    nodes: &mut N,
    node: &N::Node,
    parent_stop: Option<usize>,
) -> Result<Indices, N::Error> {
    let (subtree_start, payload_start) = match nodes.left(node)? {
        Some(left) => {
            let below = update_indices(nodes, &left, parent_stop)?;
            (below.subtree_start, below.subtree_stop)
        }
        None => {
            let start = parent_stop.unwrap_or(0);
            (start, start)
        }
    };
    let payload_stop = payload_start + nodes.payload_count(node)?;
    let subtree_stop = match nodes.right(node)? {
        Some(right) => update_indices(nodes, &right, Some(payload_stop))?.subtree_stop,
        None => payload_stop,
    };

    let indices = Indices {
        subtree_start,
        payload_start,
        payload_stop,
        subtree_stop,
    };
    nodes.set_indices(node, indices)?;
    Ok(indices)
}

/// One node of an [`AvlTree`].
#[derive(Clone, Debug)]
struct Slot<P, T> {
    position: P,
    payload: Option<T>,
    shape: Shape,
    left: Option<usize>,
    right: Option<usize>,
}

/// An AVL tree of payloads by position, its nodes kept in a list.
///
/// A node can stand at a position without a payload, as music21's
/// `createNodeAtPosition` leaves one.
#[derive(Clone, Debug)]
pub struct AvlTree<P, T> {
    slots: Vec<Slot<P, T>>,
    root: Option<usize>,
}

impl<P, T> Default for AvlTree<P, T> {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            root: None,
        }
    }
}

impl<P: PartialOrd + Clone, T> AvlNodes for AvlTree<P, T> {
    type Node = usize;
    type Position = P;
    type Error = TreeError;

    fn left(&self, node: &usize) -> Result<Option<usize>, TreeError> {
        Ok(self.slots[*node].left)
    }

    fn right(&self, node: &usize) -> Result<Option<usize>, TreeError> {
        Ok(self.slots[*node].right)
    }

    fn set_left(&mut self, node: &usize, child: Option<usize>) -> Result<(), TreeError> {
        self.slots[*node].left = child;
        Ok(())
    }

    fn set_right(&mut self, node: &usize, child: Option<usize>) -> Result<(), TreeError> {
        self.slots[*node].right = child;
        Ok(())
    }

    fn shape(&self, node: &usize) -> Result<Shape, TreeError> {
        Ok(self.slots[*node].shape)
    }

    fn set_shape(&mut self, node: &usize, shape: Shape) -> Result<(), TreeError> {
        self.slots[*node].shape = shape;
        Ok(())
    }

    fn position(&self, node: &usize) -> Result<P, TreeError> {
        Ok(self.slots[*node].position.clone())
    }

    fn holds(&self, node: &usize, compare: Compare, position: &P) -> Result<bool, TreeError> {
        let held = &self.slots[*node].position;
        Ok(match compare {
            Compare::Less => held < position,
            Compare::LessOrEqual => held <= position,
            Compare::Equal => held == position,
            Compare::Greater => held > position,
        })
    }

    fn create(&mut self, position: &P) -> Result<usize, TreeError> {
        self.slots.push(Slot {
            position: position.clone(),
            payload: None,
            shape: Shape::default(),
            left: None,
            right: None,
        });
        Ok(self.slots.len() - 1)
    }

    fn move_attributes(&mut self, from: &usize, to: &usize) -> Result<(), TreeError> {
        let position = self.slots[*from].position.clone();
        let payload = self.slots[*from].payload.take();
        let target = &mut self.slots[*to];
        target.position = position;
        target.payload = payload;
        Ok(())
    }
}

impl<P: PartialOrd + Clone, T> AvlTree<P, T> {
    /// An empty tree.
    pub fn new() -> Self {
        Self::default()
    }

    /// A balanced tree of items sorted by strictly rising position.
    pub fn from_sorted(items: Vec<(P, T)>) -> Self {
        let mut tree = Self::new();
        let count = items.len();
        let mut items: Vec<Option<(P, T)>> = items.into_iter().map(Some).collect();
        let mut make = |tree: &mut Self, placed: Placement| {
            let (position, payload) = items[placed.middle].take().ok_or(TreeError::MissingChild)?;
            let node = tree.create(&position)?;
            tree.slots[node].payload = Some(payload);
            Ok(node)
        };
        tree.root = build_sorted(&mut tree, count, &mut make).unwrap_or_default();
        tree
    }

    /// Puts a payload at a position, replacing what was there.
    pub fn insert(&mut self, position: P, payload: T) {
        // Every step on a tree built only through these methods holds.
        if let Ok(root) = insert(self, self.root, &position) {
            self.root = Some(root);
        }
        if let Ok(Some(node)) = find(self, self.root, &position) {
            self.slots[node].payload = Some(payload);
        }
    }

    /// The payload at a position.
    pub fn get(&self, position: &P) -> Option<&T> {
        let node = find(self, self.root, position).ok()??;
        self.slots[node].payload.as_ref()
    }

    /// Takes out the node at a position.
    pub fn remove(&mut self, position: &P) {
        if let Ok(root) = remove(self, self.root, position) {
            self.root = root;
        }
    }

    /// The first position after one given.
    pub fn position_after(&self, position: &P) -> Option<&P> {
        let node = node_after(self, self.root, position).ok()??;
        Some(&self.slots[node].position)
    }

    /// The last position before one given.
    pub fn position_before(&self, position: &P) -> Option<&P> {
        let node = node_before(self, self.root, position).ok()??;
        Some(&self.slots[node].position)
    }

    /// The root's position.
    pub fn root_position(&self) -> Option<&P> {
        self.root.map(|root| &self.slots[root].position)
    }

    /// The root's height; none for an empty tree.
    pub fn height(&self) -> Option<i64> {
        self.root.map(|root| self.slots[root].shape.height)
    }

    /// Every position and payload, left to right.
    pub fn iter(&self) -> impl Iterator<Item = (&P, Option<&T>)> {
        in_order(self, self.root)
            .unwrap_or_default()
            .into_iter()
            .map(|node| {
                (
                    &self.slots[node].position,
                    self.slots[node].payload.as_ref(),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The positions of a tree left to right, with each node's height.
    fn shapes(tree: &AvlTree<i32, ()>) -> Vec<(i32, i64)> {
        in_order(tree, tree.root)
            .unwrap_or_default()
            .into_iter()
            .map(|node| (tree.slots[node].position, tree.slots[node].shape.height))
            .collect()
    }

    #[test]
    fn placing_nodes_keeps_the_tree_balanced() {
        let mut tree = AvlTree::new();
        for position in [0, 2, 4, 6, 8, 10, 12] {
            tree.insert(position, ());
        }
        assert_eq!(
            shapes(&tree),
            [(0, 0), (2, 1), (4, 0), (6, 2), (8, 0), (10, 1), (12, 0)]
        );
    }

    #[test]
    fn taking_out_a_node_with_two_children_moves_its_successor_up() {
        let mut tree = AvlTree::new();
        for position in [20, 10, 5, 30] {
            tree.insert(position, position);
        }
        tree.remove(&30);
        assert_eq!(tree.root_position(), Some(&10));
        assert_eq!(tree.height(), Some(1));
        tree.remove(&10);
        assert_eq!(tree.root_position(), Some(&20));
        assert_eq!(tree.get(&20), Some(&20));
        tree.remove(&9);
        assert_eq!(tree.height(), Some(1));
    }

    #[test]
    fn a_sorted_list_is_built_balanced() {
        let tree = AvlTree::from_sorted((0..1000).map(|i| (i, i)).collect());
        assert_eq!(tree.root_position(), Some(&500));
        assert_eq!(tree.height(), Some(9));
        assert_eq!(tree.iter().count(), 1000);
    }

    #[test]
    fn positions_after_and_before_skip_the_one_given() {
        let tree = AvlTree::from_sorted(vec![(1, ()), (3, ()), (5, ())]);
        assert_eq!(tree.position_after(&3), Some(&5));
        assert_eq!(tree.position_after(&5), None);
        assert_eq!(tree.position_before(&1), None);
        assert_eq!(tree.position_before(&4), Some(&3));
    }

    /// Counts one element per node, as music21's `ElementNode` does.
    struct Counted {
        tree: AvlTree<i32, ()>,
        indices: Vec<Indices>,
    }

    impl AvlNodes for Counted {
        type Node = usize;
        type Position = i32;
        type Error = TreeError;

        fn left(&self, node: &usize) -> Result<Option<usize>, TreeError> {
            self.tree.left(node)
        }
        fn right(&self, node: &usize) -> Result<Option<usize>, TreeError> {
            self.tree.right(node)
        }
        fn set_left(&mut self, node: &usize, child: Option<usize>) -> Result<(), TreeError> {
            self.tree.set_left(node, child)
        }
        fn set_right(&mut self, node: &usize, child: Option<usize>) -> Result<(), TreeError> {
            self.tree.set_right(node, child)
        }
        fn shape(&self, node: &usize) -> Result<Shape, TreeError> {
            self.tree.shape(node)
        }
        fn set_shape(&mut self, node: &usize, shape: Shape) -> Result<(), TreeError> {
            self.tree.set_shape(node, shape)
        }
        fn position(&self, node: &usize) -> Result<i32, TreeError> {
            self.tree.position(node)
        }
        fn holds(&self, node: &usize, compare: Compare, position: &i32) -> Result<bool, TreeError> {
            self.tree.holds(node, compare, position)
        }
        fn create(&mut self, position: &i32) -> Result<usize, TreeError> {
            self.tree.create(position)
        }
        fn move_attributes(&mut self, from: &usize, to: &usize) -> Result<(), TreeError> {
            self.tree.move_attributes(from, to)
        }
    }

    impl CountedNodes for Counted {
        fn payload_count(&self, _node: &usize) -> Result<usize, TreeError> {
            Ok(1)
        }
        fn set_indices(&mut self, node: &usize, indices: Indices) -> Result<(), TreeError> {
            self.indices[*node] = indices;
            Ok(())
        }
    }

    #[test]
    fn indices_count_the_elements_under_each_node() -> Result<(), TreeError> {
        let tree = AvlTree::from_sorted(vec![(0, ()), (4, ()), (6, ())]);
        let root = tree.root.ok_or(TreeError::MissingChild)?;
        let count = tree.slots.len();
        let mut counted = Counted {
            tree,
            indices: vec![Indices::default(); count],
        };
        let top = update_indices(&mut counted, &root, None)?;
        assert_eq!(
            top,
            Indices {
                subtree_start: 0,
                payload_start: 1,
                payload_stop: 2,
                subtree_stop: 3,
            }
        );
        let right = counted.right(&root)?.ok_or(TreeError::MissingChild)?;
        assert_eq!(counted.indices[right].payload_start, 2);
        Ok(())
    }

    #[test]
    fn rotating_without_the_child_needed_is_an_error() {
        let mut tree: AvlTree<i32, ()> = AvlTree::new();
        tree.insert(1, ());
        let root = tree.root.unwrap_or_default();
        assert_eq!(
            rotate_left_left(&mut tree, &root),
            Err(TreeError::MissingChild)
        );
    }
}
