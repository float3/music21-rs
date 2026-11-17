//! music21's `IntervalNetwork`: a scale or a chord as a graph whose nodes
//! are pitches not yet realized and whose edges are intervals.
//!
//! A network has two ends, its low and high termini, which are the same
//! pitch class an octave or more apart; walking off one end comes back in
//! at the other, so a scale can be realized over any range. Edges are
//! walked ascending, descending or both ways, which is how a melodic minor
//! goes up by one set of steps and down by another. Where two edges leave
//! a node the same way, the node weights choose between them, from a draw
//! the caller supplies: nothing here reaches for a random number generator
//! of its own.
//!
//! ```
//! use music21::Pitch;
//! use music21::scale::intervalnetwork::{IntervalNetwork, PitchRange};
//!
//! let major = IntervalNetwork::bidirected(&["M2", "M2", "m2", "M2", "M2", "M2", "m2"])?;
//! let realized = major.realize_pitches(&Pitch::from_name("G4")?, None, &PitchRange::default())?;
//! let names: Vec<String> = realized.iter().map(Pitch::name_with_octave).collect();
//! assert_eq!(names, ["G4", "A4", "B4", "C5", "D5", "E5", "F#5", "G5"]);
//! # Ok::<(), music21::Error>(())
//! ```

use crate::common::numbertools::weighted_index;
use crate::defaults::{FloatType, IntegerType};
use crate::error::{Error, Result};
use crate::interval::Interval;
use crate::pitch::Pitch;
use crate::scale::scaletype::{SCALE_STARTS, advance};
use crate::scale::{DegreeComparison, ScaleType, Simplification};

/// How close two pitch-space values must be to count as the same.
const EPSILON: FloatType = 0.00001;

/// The most steps an ascending walk takes before giving up on reaching
/// its target: music21's `maxAttempts`.
const MAX_ASCENDING_STEPS: usize = 100;

/// The most steps a descending walk takes. music21 sets no limit; one is
/// set here so that a network that never descends fails instead of hanging.
const MAX_DESCENDING_STEPS: usize = 10_000;

/// How many semitones either side of a pitch a search realizes at least.
const OCTAVE: IntegerType = 12;

/// How many times a weighted network is realized looking for a degree it
/// may skip: 40 tries miss a node that is there once in a trillion.
const WEIGHTED_TRIES: usize = 40;

/// The complaint music21 makes about a walk that does not reach its target.
const ILL_FORMED: &str = "Cannot realize these pitches; is your scale well-formed? \
                          (especially check if you're giving notes without octaves)";

/// A source of draws from `0.0` (inclusive) to `1.0` (exclusive), used to
/// choose between branches by weight.
pub type Draw<'a> = dyn FnMut() -> FloatType + 'a;

/// Either end of a network: music21's `Terminus`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Terminus {
    /// The bottom: the pitch a realization starts from.
    Low,
    /// The top: the same pitch class, where the walk comes back in at the
    /// bottom.
    High,
}

/// Which node an edge connects: a terminus, or a node between them,
/// numbered as it was added.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NodeId {
    /// One of the two ends.
    Terminus(Terminus),
    /// A node between them.
    Index(usize),
}

impl NodeId {
    /// The low terminus.
    pub const LOW: Self = Self::Terminus(Terminus::Low);
    /// The high terminus.
    pub const HIGH: Self = Self::Terminus(Terminus::High);

    /// Whether this is either end of the network.
    #[must_use]
    pub fn is_terminus(self) -> bool {
        matches!(self, Self::Terminus(_))
    }
}

/// The way an edge is walked: music21's `Direction`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Direction {
    /// Both ways.
    Bi,
    /// Up only.
    Ascending,
    /// Down only.
    Descending,
}

/// A pitch the network does not hold yet: music21's `Node`.
///
/// The `degree` is what a scale calls the node, counted from one at the low
/// terminus; two nodes may share one, as the raised and natural sixths of a
/// melodic minor do. The `weight` decides between nodes that both answer.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Node {
    /// Which node this is.
    pub id: NodeId,
    /// The scale degree it stands for.
    pub degree: IntegerType,
    /// How likely it is to be chosen where several could be.
    pub weight: FloatType,
}

impl Node {
    /// A node of weight one.
    #[must_use]
    pub fn new(id: NodeId, degree: IntegerType) -> Self {
        Self {
            id,
            degree,
            weight: 1.0,
        }
    }
}

/// An interval between two nodes, and the way it is walked: music21's
/// `Edge`.
#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    /// The step.
    pub interval: Interval,
    /// The way it is walked, set as connections are added.
    pub direction: Direction,
    /// Carried for music21's sake; nothing reads it.
    pub weight: FloatType,
    /// Its key in the network, if it has been put in one.
    pub id: Option<usize>,
    connections: Vec<(NodeId, NodeId)>,
}

impl Edge {
    /// An edge connecting nothing yet.
    pub fn new(interval: Interval, id: Option<usize>) -> Self {
        Self {
            interval,
            direction: Direction::Bi,
            weight: 1.0,
            id,
            connections: Vec::new(),
        }
    }

    /// Connects `from` to `to`, walked `direction`: music21's
    /// `addDirectedConnection`.
    ///
    /// # Errors
    ///
    /// [`Direction::Bi`], which is not one way. The connection is kept all
    /// the same, as music21 keeps it.
    pub fn connect(&mut self, from: NodeId, to: NodeId, direction: Direction) -> Result<()> {
        self.connections.push((from, to));
        if direction == Direction::Bi {
            return Err(Error::Scale("must request a direction".to_string()));
        }
        self.direction = direction;
        Ok(())
    }

    /// Connects `low` to `high` going up and back going down: music21's
    /// `addBiDirectedConnections`.
    pub fn connect_both(&mut self, low: NodeId, high: NodeId) {
        self.connections.push((low, high));
        self.connections.push((high, low));
        self.direction = Direction::Bi;
    }

    /// Every pair of nodes this edge connects, the way up first.
    #[must_use]
    pub fn connections(&self) -> &[(NodeId, NodeId)] {
        &self.connections
    }

    /// The connections walked `direction`: music21's `getConnections`.
    ///
    /// # Errors
    ///
    /// Asking a one-way edge for both ways.
    pub fn connections_toward(&self, direction: Direction) -> Result<Vec<(NodeId, NodeId)>> {
        if self.direction == direction {
            return Ok(self.connections.clone());
        }
        if direction == Direction::Bi {
            return Err(Error::Scale(
                "cannot request a bi direction from a mono direction".to_string(),
            ));
        }
        if self.direction != Direction::Bi {
            return Ok(Vec::new());
        }

        // A two-way edge holds its way up first and its way down second.
        let index = usize::from(direction == Direction::Descending);
        Ok(self.connections.get(index).into_iter().copied().collect())
    }
}

/// An edge of [`IntervalNetwork::fill_arbitrary`]: its step and the nodes
/// it connects, each pair with the way it is walked.
#[derive(Clone, Debug)]
pub struct EdgeSpec {
    /// The step.
    pub interval: Interval,
    /// Pairs of node ids and the way each is walked.
    pub connections: Vec<(NodeId, NodeId, Direction)>,
}

/// Whether a network's pitches repeat at the octave, which lets a walk
/// start an octave from where it was asked to.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Octaves {
    /// They repeat.
    Duplicating,
    /// They need not.
    Distinct,
}

/// Whether every walk through a network takes the same way.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Pathways {
    /// No node has two ways on in one direction.
    Deterministic,
    /// Some node has, and its weights choose.
    Weighted,
}

/// Whether the two termini count as one degree.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Termini {
    /// The high terminus has the low one's degree.
    Equate,
    /// Each has its own.
    Distinct,
}

/// Whether a degree past either end is folded back into the network.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Moduli {
    /// Degree 9 of a seven-note scale is its degree 2.
    Permit,
    /// It is no degree.
    Forbid,
}

/// Whether a realization starting at the low terminus first works out a
/// range when none is given.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Fill {
    /// Realize a terminus to terminus span either side of the reference.
    IfUnbounded,
    /// Stop at the terminus.
    Never,
}

/// Whether a descending walk keeps the pitch it starts from.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum First {
    /// Keep it.
    Include,
    /// Start below it.
    Exclude,
}

/// The order a realization comes back in.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Order {
    /// Lowest first, as music21 gives a realization by default.
    Ascending,
    /// Highest first.
    Descending,
}

/// Which node `next_pitch` starts from when the pitch it is given is on
/// none: music21's `getNeighbor`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Neighbor {
    /// The node above it.
    Above,
    /// The node below it.
    Below,
    /// None; there is no next pitch.
    Never,
}

/// How a node is named when asking for it: by scale degree, or as itself.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NodeName {
    /// Every node of this degree, folded into the network if
    /// [`Moduli::Permit`].
    Degree(IntegerType),
    /// That node.
    Id(NodeId),
}

/// The range a realization keeps; either bound may be open.
#[derive(Clone, Debug, Default)]
pub struct PitchRange {
    /// The lowest pitch kept.
    pub min: Option<Pitch>,
    /// The highest pitch kept.
    pub max: Option<Pitch>,
}

impl PitchRange {
    /// A range closed at both ends.
    #[must_use]
    pub fn between(min: Pitch, max: Pitch) -> Self {
        Self {
            min: Some(min),
            max: Some(max),
        }
    }

    /// Whether `pitch` lies within the range, to within [`EPSILON`].
    fn holds(&self, pitch: &Pitch) -> bool {
        let above = self.min.as_ref().is_none_or(|min| at_least(pitch, min));
        let below = self.max.as_ref().is_none_or(|max| at_most(pitch, max));
        above && below
    }

    fn is_open(&self) -> bool {
        self.min.is_none() && self.max.is_none()
    }
}

/// A realization: its pitches, and the node each stands on.
#[derive(Clone, Debug, Default)]
pub struct Realized {
    /// The pitches.
    pub pitches: Vec<Pitch>,
    /// The node each pitch stands on.
    pub nodes: Vec<NodeId>,
}

impl Realized {
    fn push(&mut self, pitch: Pitch, node: NodeId) {
        self.pitches.push(pitch);
        self.nodes.push(node);
    }

    fn reverse(&mut self) {
        self.pitches.reverse();
        self.nodes.reverse();
    }
}

/// Which of a list of pitches a realization holds and which it does not,
/// each as an index into the list.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Matched {
    /// The indices of the pitches found.
    pub matched: Vec<usize>,
    /// The indices of the pitches not found.
    pub unmatched: Vec<usize>,
}

/// A tonic [`IntervalNetwork::find`] tried, and how many pitches it found.
#[derive(Clone, Debug)]
pub struct Found {
    /// How many of the pitches the network holds on this tonic.
    pub matches: usize,
    /// The pitch on the low terminus.
    pub tonic: Pitch,
}

fn at_least(pitch: &Pitch, bound: &Pitch) -> bool {
    pitch.ps() > bound.ps() || (pitch.ps() - bound.ps()).abs() < EPSILON
}

fn at_most(pitch: &Pitch, bound: &Pitch) -> bool {
    pitch.ps() < bound.ps() || (pitch.ps() - bound.ps()).abs() < EPSILON
}

/// The pitch moved by whole semitones, as music21's `Pitch.transpose`
/// moves it by a number.
fn shifted(pitch: &Pitch, semitones: IntegerType) -> Result<Pitch> {
    pitch.transpose(&Interval::from_semitones(semitones)?)
}

/// music21's `Pitch.__eq__`: the same note in the same octave.
fn same_pitch(a: &Pitch, b: &Pitch) -> bool {
    a.name_with_octave() == b.name_with_octave() && a.ps() == b.ps()
}

/// The pitch with an octave of its own: music21 sets `octaveIsImplicit`
/// to false on a reference before walking from it.
fn with_octave(pitch: &Pitch) -> Pitch {
    let mut copy = pitch.clone();
    copy.set_octave_is_implicit(false);
    copy
}

/// The indices of the lowest and highest of `pitches`, the earlier of two
/// equal lows and the later of two equal highs: music21's
/// `filterPitchList`, which sorts on pitch space and then on index.
#[must_use]
pub fn pitch_bounds(pitches: &[Pitch]) -> Option<(usize, usize)> {
    let mut order: Vec<usize> = (0..pitches.len()).collect();
    order.sort_by(|a, b| pitches[*a].ps().total_cmp(&pitches[*b].ps()));
    Some((*order.first()?, *order.last()?))
}

/// A scale or chord as a graph of intervals: music21's `IntervalNetwork`.
#[derive(Clone, Debug, PartialEq)]
pub struct IntervalNetwork {
    edge_id_count: usize,
    node_id_count: usize,
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    /// Whether the pitches repeat at the octave.
    pub octaves: Octaves,
    /// Whether any node branches.
    pub pathways: Pathways,
    /// How pitches are respelled as the network is walked.
    pub simplification: Simplification,
}

impl Default for IntervalNetwork {
    fn default() -> Self {
        Self {
            edge_id_count: 0,
            node_id_count: 0,
            nodes: Vec::new(),
            edges: Vec::new(),
            octaves: Octaves::Distinct,
            pathways: Pathways::Deterministic,
            simplification: Simplification::MaxAccidental,
        }
    }
}

// Building.
impl IntervalNetwork {
    /// A network of these steps walked both ways, from interval names.
    ///
    /// # Errors
    ///
    /// A name that is not an interval.
    pub fn bidirected(steps: &[&str]) -> Result<Self> {
        let steps = steps
            .iter()
            .map(Interval::from_name)
            .collect::<Result<Vec<_>>>()?;
        let mut network = Self::default();
        network.fill_bidirected(&steps);
        Ok(network)
    }

    /// The nodes, in the order they were added.
    #[must_use]
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// The edges, each at the index that is its key.
    #[must_use]
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// The next key a filling method gives an edge: music21's `edgeIdCount`.
    #[must_use]
    pub fn edge_id_count(&self) -> usize {
        self.edge_id_count
    }

    /// The next number a filling method gives a node: music21's
    /// `nodeIdCount`.
    #[must_use]
    pub fn node_id_count(&self) -> usize {
        self.node_id_count
    }

    /// The node with this id.
    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == id)
    }

    fn node_or_err(&self, id: NodeId) -> Result<&Node> {
        self.node(id)
            .ok_or_else(|| Error::Scale(format!("the network has no node {id:?}")))
    }

    /// Removes every node and edge: music21's `clear`.
    pub fn clear(&mut self) {
        self.edge_id_count = 0;
        self.node_id_count = 0;
        self.nodes.clear();
        self.edges.clear();
    }

    /// Adds a node, or replaces the one with its id where it already stands.
    fn put_node(&mut self, node: Node) {
        match self.nodes.iter_mut().find(|held| held.id == node.id) {
            Some(held) => *held = node,
            None => self.nodes.push(node),
        }
    }

    /// The next node of a chain of `count` steps: a numbered one, or the
    /// high terminus after the last step.
    fn chain_node(&mut self, step: usize, count: usize, degree: IntegerType) -> NodeId {
        if step + 1 == count {
            return NodeId::HIGH;
        }
        let id = NodeId::Index(self.node_id_count);
        self.node_id_count += 1;
        self.put_node(Node::new(id, degree));
        id
    }

    fn push_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
        self.edge_id_count += 1;
    }

    /// Replaces the network with a chain of `steps` walked both ways:
    /// music21's `fillBiDirectedEdges`.
    pub fn fill_bidirected(&mut self, steps: &[Interval]) {
        self.clear();
        self.put_node(Node::new(NodeId::LOW, 1));
        let mut previous = NodeId::LOW;
        for (index, step) in steps.iter().enumerate() {
            let degree = index as IntegerType + 2;
            let following = self.chain_node(index, steps.len(), degree);
            if following == NodeId::HIGH {
                self.put_node(Node::new(NodeId::HIGH, degree));
            }
            let mut edge = Edge::new(step.clone(), Some(self.edge_id_count));
            edge.connect_both(previous, following);
            self.push_edge(edge);
            previous = following;
        }
    }

    /// Replaces the network with one chain of steps walked up and another,
    /// given as rising steps, walked down between the same termini:
    /// music21's `fillDirectedEdges`.
    ///
    /// # Errors
    ///
    /// Chains of different lengths, whose degrees cannot be matched up.
    pub fn fill_directed(&mut self, ascending: &[Interval], descending: &[Interval]) -> Result<()> {
        self.clear();
        if ascending.len() != descending.len() {
            return Err(Error::Scale(
                "cannot manage unequal sized directed edges".to_string(),
            ));
        }
        self.put_node(Node::new(NodeId::LOW, 1));

        // The way up, ending on a new high terminus.
        let mut previous = NodeId::LOW;
        for (index, step) in ascending.iter().enumerate() {
            let degree = index as IntegerType + 2;
            let following = self.chain_node(index, ascending.len(), degree);
            if following == NodeId::HIGH {
                self.put_node(Node::new(NodeId::HIGH, degree));
            }
            let mut edge = Edge::new(step.clone(), Some(self.edge_id_count));
            edge.connect(previous, following, Direction::Ascending)?;
            self.push_edge(edge);
            previous = following;
        }

        // The way down, through nodes of its own to the same high terminus,
        // each edge connected from its upper node.
        let mut previous = NodeId::LOW;
        for (index, step) in descending.iter().enumerate() {
            let degree = index as IntegerType + 2;
            let following = self.chain_node(index, descending.len(), degree);
            let mut edge = Edge::new(step.clone(), Some(self.edge_id_count));
            edge.connect(following, previous, Direction::Descending)?;
            self.push_edge(edge);
            previous = following;
        }
        Ok(())
    }

    /// Replaces the network with these nodes and edges: music21's
    /// `fillArbitrary`. Edges are keyed by their place in the list; the
    /// id counters are left at nought, as music21 leaves them.
    ///
    /// # Errors
    ///
    /// A connection to a node not in `nodes`.
    pub fn fill_arbitrary(&mut self, nodes: Vec<Node>, edges: Vec<EdgeSpec>) -> Result<()> {
        self.clear();
        for node in nodes {
            self.put_node(node);
        }
        for (index, spec) in edges.into_iter().enumerate() {
            let mut edge = Edge::new(spec.interval, Some(index));
            for (from, to, direction) in spec.connections {
                self.node_or_err(from)?;
                self.node_or_err(to)?;
                match direction {
                    Direction::Bi => edge.connect_both(from, to),
                    _ => edge.connect(from, to, direction)?,
                }
            }
            self.edges.push(edge);
        }
        Ok(())
    }

    /// Replaces the network with nodes and one-connection edges written
    /// as tables.
    fn fill_table(&mut self, nodes: &[TableNode], edges: &[TableEdge]) -> Result<()> {
        let nodes = nodes
            .iter()
            .map(|(id, degree)| Node::new(*id, *degree))
            .collect();
        let edges = edges
            .iter()
            .map(|(name, from, to, direction)| {
                Ok(EdgeSpec {
                    interval: Interval::from_name(name)?,
                    connections: vec![(*from, *to, *direction)],
                })
            })
            .collect::<Result<Vec<_>>>()?;
        self.fill_arbitrary(nodes, edges)
    }

    /// Replaces the network with a melodic minor, which raises its sixth
    /// and seventh going up and not coming down: music21's
    /// `fillMelodicMinor`.
    ///
    /// # Errors
    ///
    /// None in practice; the intervals are fixed.
    pub fn fill_melodic_minor(&mut self) -> Result<()> {
        self.fill_table(&MELODIC_MINOR_NODES, &MELODIC_MINOR_EDGES)?;
        self.octaves = Octaves::Duplicating;
        self.pathways = Pathways::Deterministic;
        Ok(())
    }
}

// Degrees and nodes.
impl IntervalNetwork {
    /// The lowest degree: music21's `degreeMin`.
    #[must_use]
    pub fn degree_min(&self) -> Option<IntegerType> {
        self.nodes.iter().map(|node| node.degree).min()
    }

    /// The highest degree: music21's `degreeMax`.
    #[must_use]
    pub fn degree_max(&self) -> Option<IntegerType> {
        self.nodes.iter().map(|node| node.degree).max()
    }

    /// The highest degree but the high terminus's, which repeats the low
    /// one: music21's `degreeMaxUnique`.
    #[must_use]
    pub fn degree_max_unique(&self) -> Option<IntegerType> {
        self.nodes
            .iter()
            .filter(|node| node.id != NodeId::HIGH)
            .map(|node| node.degree)
            .max()
    }

    /// Each node's id and degree, in order: music21's
    /// `getNodeDegreeDictionary`.
    #[must_use]
    pub fn node_degrees(&self, termini: Termini) -> Vec<(NodeId, IntegerType)> {
        let low = self.node(NodeId::LOW).map(|node| node.degree);
        self.nodes
            .iter()
            .map(|node| match (termini, node.id, low) {
                (Termini::Equate, NodeId::HIGH, Some(low)) => (node.id, low),
                _ => (node.id, node.degree),
            })
            .collect()
    }

    /// The degree of a node, the high terminus taking the low one's:
    /// music21's `nodeIdToDegree`.
    ///
    /// # Errors
    ///
    /// A node not in the network.
    pub fn degree_of(&self, id: NodeId) -> Result<IntegerType> {
        self.node_degrees(Termini::Equate)
            .into_iter()
            .find(|(node, _)| *node == id)
            .map(|(_, degree)| degree)
            .ok_or_else(|| Error::Scale(format!("the network has no node {id:?}")))
    }

    /// The direction of every edge meeting a node: music21's
    /// `nodeIdToEdgeDirections`.
    ///
    /// # Errors
    ///
    /// A node no edge meets.
    pub fn edge_directions(&self, id: NodeId) -> Result<Vec<Direction>> {
        let found: Vec<Direction> = self
            .edges
            .iter()
            .filter(|edge| {
                edge.connections
                    .iter()
                    .any(|(from, to)| *from == id || *to == id)
            })
            .map(|edge| edge.direction)
            .collect();
        if found.is_empty() {
            return Err(Error::Scale("failed to match any edges".to_string()));
        }
        Ok(found)
    }

    /// The degree folded into the network's span, so that degree 8 of a
    /// seven-note scale is 1: music21's `degreeModulus`.
    ///
    /// # Errors
    ///
    /// An empty network, or one whose nodes all share a degree.
    pub fn degree_modulus(&self, degree: IntegerType) -> Result<IntegerType> {
        let (Some(min), Some(max)) = (self.degree_min(), self.degree_max()) else {
            return Err(Error::Scale(
                "Cannot compute degreeModulus on an empty network".to_string(),
            ));
        };
        let span = max - min;
        if span == 0 {
            return Err(Error::Scale(
                "Cannot compute degreeModulus on a network of one degree".to_string(),
            ));
        }
        Ok((degree - 1).rem_euclid(span) + min)
    }

    /// The nodes a name stands for: music21's `nodeNameToNodes`.
    ///
    /// # Errors
    ///
    /// A node id the network does not hold.
    pub fn nodes_named(
        &self,
        name: NodeName,
        termini: Termini,
        moduli: Moduli,
    ) -> Result<Vec<NodeId>> {
        let degree = match name {
            NodeName::Id(id) => {
                self.node_or_err(id)?;
                return Ok(vec![id]);
            }
            NodeName::Degree(degree) => degree,
        };
        let degrees = self.node_degrees(termini);
        let exact: Vec<NodeId> = degrees
            .iter()
            .filter(|(_, held)| *held == degree)
            .map(|(id, _)| *id)
            .collect();
        if !exact.is_empty() || moduli == Moduli::Forbid || degrees.is_empty() {
            return Ok(exact);
        }

        // Past either end: the degree folded back into the network.
        let folded = self.degree_modulus(degree)?;
        Ok(degrees
            .iter()
            .filter(|(_, held)| *held == folded)
            .map(|(id, _)| *id)
            .collect())
    }

    /// The node a walk named this way starts from: the low terminus for
    /// none, else the first node the name stands for.
    fn start_of(&self, name: Option<NodeName>) -> Result<NodeId> {
        let Some(name) = name else {
            return Ok(self.node_or_err(NodeId::LOW)?.id);
        };
        self.nodes_named(name, Termini::Equate, Moduli::Permit)?
            .first()
            .copied()
            .ok_or_else(|| Error::Scale(format!("no node is named {name:?}")))
    }

    /// Every edge leaving a node walked `direction`, each as its index and
    /// the node it reaches: music21's `getNext`. Leaving the high terminus
    /// upward is leaving the low one, and the other way round.
    ///
    /// # Errors
    ///
    /// No edge leaving the node that way.
    pub fn next_steps(&self, from: NodeId, direction: Direction) -> Result<Vec<(usize, NodeId)>> {
        let source = match (from, direction) {
            (NodeId::LOW, Direction::Descending) => NodeId::HIGH,
            (NodeId::HIGH, Direction::Ascending) => NodeId::LOW,
            _ => from,
        };
        let mut found = Vec::new();
        for (index, edge) in self.edges.iter().enumerate() {
            for (start, end) in edge.connections_toward(direction)? {
                if start == source {
                    found.push((index, end));
                }
            }
        }
        if found.is_empty() {
            return Err(Error::Scale("could not find any edges".to_string()));
        }
        Ok(found)
    }

    /// One of several ways on, chosen by the weight of the node each
    /// reaches: music21's `weightedSelection`.
    fn choose(&self, steps: &[(usize, NodeId)], draw: &mut Draw<'_>) -> Result<(usize, NodeId)> {
        if let [only] = steps {
            return Ok(*only);
        }
        let weights = steps
            .iter()
            .map(|(_, node)| Ok(self.node_or_err(*node)?.weight))
            .collect::<Result<Vec<_>>>()?;
        let index = weighted_index(&weights, draw())?;
        Ok(steps[index])
    }

    /// One step on from `from`, chosen by weight where it branches: the
    /// interval walked and the node reached.
    fn step_from(
        &self,
        from: NodeId,
        direction: Direction,
        draw: &mut Draw<'_>,
    ) -> Result<(&Interval, NodeId)> {
        let steps = self.next_steps(from, direction)?;
        let (edge, node) = self.choose(&steps, draw)?;
        Ok((&self.edges[edge].interval, node))
    }
}

// Spelling.
impl IntervalNetwork {
    /// The pitch moved by one step and respelled as the network's
    /// simplification says: music21's
    /// `transposePitchAndApplySimplification`. A network told to spell
    /// exactly takes the most common spelling of a step given only in
    /// semitones, which has no spelling of its own.
    ///
    /// # Errors
    ///
    /// A pitch moved past the octaves an integer counts.
    pub fn transpose_step(&self, interval: &Interval, pitch: &Pitch) -> Result<Pitch> {
        let mut rule = self.simplification;
        if rule == Simplification::Exact && interval.is_implicit_diatonic() {
            rule = Simplification::MostCommon;
        }
        advance(pitch, interval, rule)
    }

    /// How many semitones either side of a pitch a search realizes: an
    /// octave, or the widest edge where that is wider. music21's
    /// `_searchSpan`.
    #[must_use]
    pub fn search_span(&self) -> IntegerType {
        let widest = self
            .edges
            .iter()
            .map(|edge| edge.interval.semitones().abs())
            .fold(0.0, FloatType::max);
        OCTAVE.max(widest.ceil() as IntegerType)
    }

    /// The name every terminus takes on a walk from `reference` standing on
    /// `start`, where the network repeats at the octave and `start` is a
    /// terminus.
    fn terminus_name(&self, start: NodeId, reference: &Pitch) -> Option<String> {
        if self.octaves == Octaves::Duplicating && start.is_terminus() {
            return Some(reference.name());
        }
        None
    }

    /// The pitch spelled as `name` where it lands on a terminus and the two
    /// are the same note an octave or more apart: music21's
    /// `_spellTerminus`. Respelling accidentals along a walk can change its
    /// letters, so a walk can reach the octave as C## where it set out from
    /// D.
    ///
    /// # Errors
    ///
    /// A name that is not a pitch.
    pub fn spell_terminus(&self, pitch: Pitch, node: NodeId, name: Option<&str>) -> Result<Pitch> {
        let Some(name) = name else {
            return Ok(pitch);
        };
        if pitch.name() == name || !node.is_terminus() {
            return Ok(pitch);
        }
        let mut respelled = Pitch::from_name(name)?;
        let octave = pitch.implicit_octave();
        respelled.set_octave(Some(octave));
        let octaves = (pitch.ps() - respelled.ps()) / FloatType::from(OCTAVE);
        if octaves != octaves.round() {
            return Ok(pitch);
        }
        respelled.set_octave(Some(octave + octaves.round() as IntegerType));
        Ok(respelled)
    }
}

// Realizing.
impl IntervalNetwork {
    /// The pitches from `reference` on `start` upward: music21's
    /// `realizeAscending`. With no range the walk stops at the high
    /// terminus; with `Fill::IfUnbounded` it first works one out.
    ///
    /// # Errors
    ///
    /// A start the network does not hold, or a walk that never reaches the
    /// top of its range.
    pub fn realize_ascending(
        &self,
        reference: &Pitch,
        start: Option<NodeName>,
        range: &PitchRange,
        fill: Fill,
        draw: &mut Draw<'_>,
    ) -> Result<Realized> {
        let mut pitch = with_octave(reference);
        let start = self.start_of(start)?;
        let range = match (fill, range.is_open()) {
            (Fill::IfUnbounded, true) => self.min_max_range(&pitch, start, draw)?,
            _ => range.clone(),
        };
        if self.octaves == Octaves::Duplicating
            && let Some(min) = &range.min
        {
            pitch = pitch.transpose_below_target(min, true)?;
        }

        let name = self.terminus_name(start, &pitch);
        let mut node = start;
        let mut realized = Realized::default();
        for _ in 0..MAX_ASCENDING_STEPS {
            if range.holds(&pitch) {
                realized.push(pitch.clone(), node);
            }
            if range.max.as_ref().is_some_and(|max| at_least(&pitch, max)) {
                return Ok(realized);
            }
            if node == NodeId::HIGH {
                if range.max.is_none() {
                    return Ok(realized);
                }
                node = NodeId::LOW;
            }
            let (interval, reached) = self.step_from(node, Direction::Ascending, draw)?;
            node = reached;
            pitch = self.transpose_step(interval, &pitch)?;
            pitch = self.spell_terminus(pitch, node, name.as_deref())?;
        }
        Err(Error::Scale(ILL_FORMED.to_string()))
    }

    /// The pitches from `reference` on `start` downward: music21's
    /// `realizeDescending`. With no lower bound the walk stops at the low
    /// terminus.
    ///
    /// # Errors
    ///
    /// A start the network does not hold, or a walk that never descends.
    pub fn realize_descending(
        &self,
        reference: &Pitch,
        start: Option<NodeName>,
        range: &PitchRange,
        walk: DescendingWalk,
        draw: &mut Draw<'_>,
    ) -> Result<Realized> {
        let mut pitch = with_octave(reference);
        let start = self.start_of(start)?;
        let range = match (walk.fill, range.is_open()) {
            (Fill::IfUnbounded, true) => self.min_max_range(&pitch, start, draw)?,
            _ => range.clone(),
        };
        if self.octaves == Octaves::Duplicating
            && let Some(max) = &range.max
        {
            pitch = pitch.transpose_above_target(max, true)?;
        }

        let name = self.terminus_name(start, &pitch);
        let mut node = start;
        let mut realized = Realized::default();
        let mut first = true;
        for _ in 0..MAX_DESCENDING_STEPS {
            if range.holds(&pitch) && (!first || walk.first == First::Include) {
                realized.push(pitch.clone(), node);
            }
            first = false;
            if range.min.as_ref().is_some_and(|min| pitch.ps() <= min.ps()) {
                return Ok(Self::ordered(realized, walk.order));
            }
            if node == NodeId::LOW {
                if range.min.is_none() {
                    return Ok(Self::ordered(realized, walk.order));
                }
                node = NodeId::HIGH;
            }
            let (interval, reached) = self.step_from(node, Direction::Descending, draw)?;
            node = reached;
            pitch = self.transpose_step(&interval.reversed()?, &pitch)?;
            pitch = self.spell_terminus(pitch, node, name.as_deref())?;
        }
        Err(Error::Scale(ILL_FORMED.to_string()))
    }

    /// A descending walk's pitches lowest first, or as walked.
    fn ordered(mut realized: Realized, order: Order) -> Realized {
        if order == Order::Ascending {
            realized.reverse();
        }
        realized
    }

    /// The network realized from `reference` standing on `start`: music21's
    /// `realize`.
    ///
    /// A network that repeats at the octave is walked `direction` over the
    /// range, the whole span between the termini where there is none; one
    /// that does not is walked up and down from the reference whatever the
    /// direction.
    ///
    /// # Errors
    ///
    /// A start the network does not hold, or a walk that fails.
    pub fn realize(
        &self,
        reference: &Pitch,
        start: Option<NodeName>,
        range: &PitchRange,
        direction: Direction,
        order: Order,
        draw: &mut Draw<'_>,
    ) -> Result<Realized> {
        let reference = with_octave(reference);
        let mut realized = match self.octaves {
            Octaves::Duplicating => {
                self.realize_directed(&reference, start, range, direction, draw)?
            }
            Octaves::Distinct => {
                let above = self.realize_ascending(&reference, start, range, Fill::Never, draw)?;
                let walk = DescendingWalk::new(First::Exclude, Fill::Never);
                let mut realized = self.realize_descending(&reference, start, range, walk, draw)?;
                realized.pitches.extend(above.pitches);
                realized.nodes.extend(above.nodes);
                realized
            }
        };
        if order == Order::Descending {
            realized.reverse();
        }
        Ok(realized)
    }

    /// [`Self::realize`] for a network that repeats at the octave.
    fn realize_directed(
        &self,
        reference: &Pitch,
        start: Option<NodeName>,
        range: &PitchRange,
        direction: Direction,
        draw: &mut Draw<'_>,
    ) -> Result<Realized> {
        let below_min = |pitch: &Pitch| match &range.min {
            Some(min) => pitch.transpose_below_target(min, false),
            None => Ok(pitch.clone()),
        };
        let above_max = |pitch: &Pitch| match &range.max {
            Some(max) => pitch.transpose_above_target(max, false),
            None => Ok(pitch.clone()),
        };
        match direction {
            Direction::Ascending => {
                let low = below_min(reference)?;
                self.realize_ascending(&low, start, range, Fill::IfUnbounded, draw)
            }
            Direction::Descending => {
                let high = above_max(reference)?;
                let walk = DescendingWalk::new(First::Include, Fill::IfUnbounded);
                self.realize_descending(&high, start, range, walk, draw)
            }
            Direction::Bi => {
                let low = below_min(reference)?;
                let up = self.realize_ascending(&low, start, range, Fill::Never, draw)?;
                let high = above_max(reference)?;
                let walk = DescendingWalk::new(First::Include, Fill::Never);
                let down = self.realize_descending(&high, start, range, walk, draw)?;
                Ok(Self::interleaved(up, down))
            }
        }
    }

    /// Two realizations taken a pitch from each in turn, each pitch once.
    fn interleaved(up: Realized, down: Realized) -> Realized {
        let mut merged = Realized::default();
        let longest = up.pitches.len().max(down.pitches.len());
        for index in 0..longest {
            for side in [&up, &down] {
                let Some(pitch) = side.pitches.get(index) else {
                    continue;
                };
                if merged.pitches.iter().any(|held| same_pitch(held, pitch)) {
                    continue;
                }
                merged.push(pitch.clone(), side.nodes[index]);
            }
        }
        merged
    }

    /// The pitches of [`Self::realize`] walked upward, lowest first:
    /// music21's `realizePitch` with its defaults. A weighted network
    /// branches on a fixed draw.
    ///
    /// # Errors
    ///
    /// As [`Self::realize`].
    pub fn realize_pitches(
        &self,
        reference: &Pitch,
        start: Option<NodeName>,
        range: &PitchRange,
    ) -> Result<Vec<Pitch>> {
        let realized = self.realize(
            reference,
            start,
            range,
            Direction::Ascending,
            Order::Ascending,
            &mut || 0.0,
        )?;
        Ok(realized.pitches)
    }

    /// The steps between successive pitches of a realization from C4:
    /// music21's `realizeIntervals`.
    ///
    /// # Errors
    ///
    /// As [`Self::realize`].
    pub fn realize_intervals(
        &self,
        start: Option<NodeName>,
        range: &PitchRange,
        direction: Direction,
        order: Order,
        draw: &mut Draw<'_>,
    ) -> Result<Vec<Interval>> {
        let reference = Pitch::from_name("C4")?;
        let realized = self.realize(&reference, start, range, direction, order, draw)?;
        realized
            .pitches
            .windows(2)
            .map(|pair| Interval::between_pitches(&pair[0], &pair[1]))
            .collect()
    }

    /// The pitches on the termini of a realization from `reference`:
    /// music21's `realizeTermini`.
    ///
    /// # Errors
    ///
    /// As [`Self::realize`], or a realization with no pitches.
    pub fn realize_termini(
        &self,
        reference: &Pitch,
        start: Option<NodeName>,
        draw: &mut Draw<'_>,
    ) -> Result<(Pitch, Pitch)> {
        let start = Some(NodeName::Id(self.start_of(start)?));
        let open = PitchRange::default();
        let above = self.realize_ascending(reference, start, &open, Fill::Never, draw)?;
        let walk = DescendingWalk::new(First::Exclude, Fill::Never);
        let below = self.realize_descending(reference, start, &open, walk, draw)?;
        let low = below.pitches.first().or(above.pitches.first());
        let high = above.pitches.last().or(below.pitches.last());
        match (low, high) {
            (Some(low), Some(high)) => Ok((low.clone(), high.clone())),
            _ => Err(Error::Scale(ILL_FORMED.to_string())),
        }
    }

    /// The lowest and highest pitches of one span from terminus to terminus
    /// either side of `reference`: music21's `realizeMinMax`.
    ///
    /// # Errors
    ///
    /// As [`Self::realize`].
    pub fn realize_min_max(
        &self,
        reference: &Pitch,
        start: Option<NodeName>,
        draw: &mut Draw<'_>,
    ) -> Result<(Pitch, Pitch)> {
        let start = self.start_of(start)?;
        let range = self.min_max_range(&with_octave(reference), start, draw)?;
        match (range.min, range.max) {
            (Some(min), Some(max)) => Ok((min, max)),
            _ => Err(Error::Scale(ILL_FORMED.to_string())),
        }
    }

    fn min_max_range(
        &self,
        reference: &Pitch,
        start: NodeId,
        draw: &mut Draw<'_>,
    ) -> Result<PitchRange> {
        let name = Some(NodeName::Id(start));
        let (low, high) = self.realize_termini(reference, name, draw)?;

        // An octave past each terminus, so that a span starting anywhere
        // fits.
        let wide = PitchRange::between(shifted(&low, -OCTAVE)?, shifted(&high, OCTAVE)?);
        let above = self.realize_ascending(reference, name, &wide, Fill::Never, draw)?;
        let walk = DescendingWalk {
            first: First::Include,
            fill: Fill::Never,
            order: Order::Ascending,
        };
        let below = self.realize_descending(reference, name, &wide, walk, draw)?;

        let (Some(first), Some(last)) = (above.pitches.first(), above.pitches.last()) else {
            return Err(Error::Scale(ILL_FORMED.to_string()));
        };
        let mut min = last.clone();
        let mut max = first.clone();
        let spans = Self::terminus_span(&above).chain(Self::terminus_span(&below));
        for pitch in spans {
            if pitch.ps() < min.ps() {
                min = pitch.clone();
            }
            if pitch.ps() > max.ps() {
                max = pitch.clone();
            }
        }
        Ok(PitchRange::between(min, max))
    }

    /// The pitches from the first terminus a realization reaches after its
    /// start to the next one, both included.
    fn terminus_span(realized: &Realized) -> impl Iterator<Item = &Pitch> {
        let mut span = Vec::new();
        let mut collecting = false;
        for (index, (pitch, node)) in realized.pitches.iter().zip(&realized.nodes).enumerate() {
            if index == 0 && node.is_terminus() {
                continue;
            }
            if node.is_terminus() && collecting {
                span.push(pitch);
                break;
            }
            if node.is_terminus() {
                collecting = true;
            }
            if collecting {
                span.push(pitch);
            }
        }
        span.into_iter()
    }

    /// The realized pitches on any of `degrees`, each folded into the
    /// network: music21's `realizePitchByDegree`.
    ///
    /// # Errors
    ///
    /// As [`Self::realize`].
    pub fn pitches_by_degree(
        &self,
        reference: &Pitch,
        start: Option<NodeName>,
        degrees: &[IntegerType],
        range: &PitchRange,
        direction: Direction,
        draw: &mut Draw<'_>,
    ) -> Result<Vec<Pitch>> {
        let realized = self.realize(reference, start, range, direction, Order::Ascending, draw)?;
        let wanted = degrees
            .iter()
            .map(|degree| self.degree_modulus(*degree))
            .collect::<Result<Vec<_>>>()?;
        let mut found = Vec::new();
        for (pitch, node) in realized.pitches.into_iter().zip(realized.nodes) {
            let degree = self.node_or_err(node)?.degree;
            if wanted.contains(&self.degree_modulus(degree)?) {
                found.push(pitch);
            }
        }
        Ok(found)
    }
}

/// How a descending realization is walked.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DescendingWalk {
    /// Whether the starting pitch is kept.
    pub first: First,
    /// Whether a range is worked out when none is given.
    pub fill: Fill,
    /// The order the pitches come back in.
    pub order: Order,
}

impl DescendingWalk {
    /// A walk returned lowest first, as music21 returns one by default.
    #[must_use]
    pub fn new(first: First, fill: Fill) -> Self {
        Self {
            first,
            fill,
            order: Order::Ascending,
        }
    }
}

// Finding pitches.
impl IntervalNetwork {
    /// The range a search around `pitch` realizes.
    fn search_range(&self, pitch: &Pitch) -> Result<PitchRange> {
        let span = self.search_span();
        Ok(PitchRange::between(
            shifted(pitch, -span)?,
            shifted(pitch, span)?,
        ))
    }

    /// Every node that sounds `target` on a realization from `reference`
    /// standing on `start`, in any octave, by degree: music21's
    /// `getRelativeNodeIds`.
    ///
    /// # Errors
    ///
    /// As [`Self::realize`].
    pub fn relative_node_ids(
        &self,
        reference: &Pitch,
        start: Option<NodeName>,
        target: &Pitch,
        comparison: DegreeComparison,
        direction: Direction,
        draw: &mut Draw<'_>,
    ) -> Result<Vec<NodeId>> {
        let start = Some(NodeName::Id(self.start_of(start)?));
        let target = with_octave(target);
        let range = self.search_range(&target)?;
        let realized = self.realize(reference, start, &range, direction, Order::Ascending, draw)?;
        let wanted = comparison.key(&target);
        let mut found: Vec<NodeId> = Vec::new();
        for (pitch, node) in realized.pitches.iter().zip(realized.nodes) {
            if comparison.key(pitch) == wanted && !found.contains(&node) {
                found.push(node);
            }
        }
        let mut degrees = Vec::with_capacity(found.len());
        for node in &found {
            degrees.push(self.node_or_err(*node)?.degree);
        }
        let mut order: Vec<usize> = (0..found.len()).collect();
        order.sort_by_key(|index| degrees[*index]);
        Ok(order.into_iter().map(|index| found[index]).collect())
    }

    /// One node of [`Self::relative_node_ids`], chosen by weight where
    /// several sound the target: music21's `getRelativeNodeId`.
    ///
    /// # Errors
    ///
    /// As [`Self::realize`].
    pub fn relative_node_id(
        &self,
        reference: &Pitch,
        start: Option<NodeName>,
        target: &Pitch,
        comparison: DegreeComparison,
        direction: Direction,
        draw: &mut Draw<'_>,
    ) -> Result<Option<NodeId>> {
        let found =
            self.relative_node_ids(reference, start, target, comparison, direction, draw)?;
        self.weighted_node(&found, draw)
    }

    /// One of `nodes` chosen by weight, or none of none.
    fn weighted_node(&self, nodes: &[NodeId], draw: &mut Draw<'_>) -> Result<Option<NodeId>> {
        match nodes {
            [] => Ok(None),
            [only] => Ok(Some(*only)),
            _ => {
                let weights = nodes
                    .iter()
                    .map(|node| Ok(self.node_or_err(*node)?.weight))
                    .collect::<Result<Vec<_>>>()?;
                Ok(Some(nodes[weighted_index(&weights, draw())?]))
            }
        }
    }

    /// The degree of the node sounding `target`: music21's
    /// `getRelativeNodeDegree`.
    ///
    /// # Errors
    ///
    /// As [`Self::realize`].
    pub fn relative_degree(
        &self,
        reference: &Pitch,
        start: Option<NodeName>,
        target: &Pitch,
        comparison: DegreeComparison,
        direction: Direction,
        draw: &mut Draw<'_>,
    ) -> Result<Option<IntegerType>> {
        let node = self.relative_node_id(reference, start, target, comparison, direction, draw)?;
        node.map(|node| self.degree_of(node)).transpose()
    }

    /// The nodes either side of `target` on a realization, where it lies
    /// below some realized pitch: music21's `getNeighborNodeIds`. The lower
    /// one is none where nothing realized lies below.
    ///
    /// # Errors
    ///
    /// As [`Self::realize`].
    pub fn neighbor_node_ids(
        &self,
        reference: &Pitch,
        start: Option<NodeName>,
        target: &Pitch,
        direction: Direction,
        draw: &mut Draw<'_>,
    ) -> Result<Option<(Option<NodeId>, NodeId)>> {
        let start = Some(NodeName::Id(self.start_of(start)?));
        let target = with_octave(target);
        let range = self.search_range(&target)?;
        let realized = self.realize(reference, start, &range, direction, Order::Ascending, draw)?;
        let mut low = None;
        for (pitch, node) in realized.pitches.iter().zip(realized.nodes) {
            if target.ps() < pitch.ps() {
                return Ok(Some((low, node)));
            }
            low = Some(node);
        }
        Ok(None)
    }

    /// The pitch of the node of degree `target` on a realization from
    /// `reference` standing on `name`: music21's `getPitchFromNodeDegree`.
    ///
    /// Where two nodes have the degree, the one walked `direction` is
    /// taken; a weighted network that may skip it is realized up to forty
    /// times.
    ///
    /// # Errors
    ///
    /// No node named `name` or `target`, or a realization that fails.
    #[allow(clippy::too_many_arguments)]
    pub fn pitch_of_degree(
        &self,
        reference: &Pitch,
        name: NodeName,
        target: NodeName,
        direction: Direction,
        range: &PitchRange,
        termini: Termini,
        draw: &mut Draw<'_>,
    ) -> Result<Option<Pitch>> {
        let Some(start) = self
            .nodes_named(name, Termini::Equate, Moduli::Permit)?
            .first()
            .copied()
        else {
            return Err(Error::Scale(format!("no node is named {name:?}")));
        };
        let wanted = self.target_node(target, direction, termini)?;
        let tries = match self.pathways {
            Pathways::Deterministic => 1,
            Pathways::Weighted => WEIGHTED_TRIES,
        };
        for _ in 0..tries {
            let realized = self.realize(
                reference,
                Some(NodeName::Id(start)),
                range,
                direction,
                Order::Ascending,
                draw,
            )?;
            for (pitch, node) in realized.pitches.iter().zip(&realized.nodes) {
                let both_termini = node.is_terminus() && wanted.is_terminus();
                if *node == wanted || (termini == Termini::Equate && both_termini) {
                    return Ok(Some(pitch.clone()));
                }
            }
        }
        Ok(None)
    }

    /// The node a degree names for a walk `direction`: the only one, the
    /// low terminus of the two, or the first with an edge walked that way.
    fn target_node(
        &self,
        target: NodeName,
        direction: Direction,
        termini: Termini,
    ) -> Result<NodeId> {
        let named = self.nodes_named(target, termini, Moduli::Permit)?;
        let Some(first) = named.first().copied() else {
            return Err(Error::Scale(format!("no node is named {target:?}")));
        };
        if named.len() == 1 || named == [NodeId::LOW, NodeId::HIGH] {
            return Ok(first);
        }
        for node in &named {
            if self.edge_directions(*node)?.contains(&direction) {
                return Ok(*node);
            }
        }
        Ok(first)
    }

    /// Which of `targets` a realization from `reference` standing on `start`
    /// over their range holds: music21's `match`.
    ///
    /// # Errors
    ///
    /// No targets, or a realization that fails.
    pub fn match_pitches(
        &self,
        reference: &Pitch,
        start: Option<NodeName>,
        targets: &[Pitch],
        comparison: DegreeComparison,
    ) -> Result<Matched> {
        let start = Some(NodeName::Id(self.start_of(start)?));
        let Some((low, high)) = pitch_bounds(targets) else {
            return Err(Error::Value(
                "There must be at least one pitch given.".to_string(),
            ));
        };
        let range = PitchRange::between(targets[low].clone(), targets[high].clone());
        let realized = self.realize_pitches(reference, start, &range)?;
        let keys: Vec<String> = realized.iter().map(|pitch| comparison.key(pitch)).collect();
        let mut matched = Matched::default();
        for (index, target) in targets.iter().enumerate() {
            if keys.contains(&comparison.key(target)) {
                matched.matched.push(index);
            } else {
                matched.unmatched.push(index);
            }
        }
        Ok(matched)
    }

    /// The realized pitches none of `targets` matches: music21's
    /// `findMissing`.
    ///
    /// # Errors
    ///
    /// No targets, or a realization that fails.
    pub fn find_missing(
        &self,
        reference: &Pitch,
        start: Option<NodeName>,
        targets: &[Pitch],
        comparison: DegreeComparison,
        range: &PitchRange,
    ) -> Result<Vec<Pitch>> {
        let start = Some(NodeName::Id(self.start_of(start)?));
        let realized = self.realize_pitches(reference, start, range)?;
        if targets.is_empty() {
            return Err(Error::Value(
                "There must be at least one pitch given.".to_string(),
            ));
        }
        let keys: Vec<String> = targets.iter().map(|pitch| comparison.key(pitch)).collect();
        Ok(realized
            .into_iter()
            .filter(|pitch| !keys.contains(&comparison.key(pitch)))
            .collect())
    }

    /// How many of `targets` the network holds standing on each of fifteen
    /// tonics, most first: music21's `find`. Ties keep the higher tonic
    /// first, and the later-tried of two enharmonic ones.
    ///
    /// # Errors
    ///
    /// No targets, or a realization that fails.
    pub fn find(&self, targets: &[Pitch], comparison: DegreeComparison) -> Result<Vec<Found>> {
        let mut found = Vec::with_capacity(SCALE_STARTS.len());
        for name in SCALE_STARTS {
            let tonic = Pitch::from_name(name)?;
            let matched = self.match_pitches(&tonic, None, targets, comparison)?;
            found.push(Found {
                matches: matched.matched.len(),
                tonic,
            });
        }

        // music21 sorts (count, pitch) pairs and reverses them; equal pitches
        // keep the order they were tried in before the reversal.
        found.sort_by(|a, b| {
            a.matches
                .cmp(&b.matches)
                .then(a.tonic.ps().total_cmp(&b.tonic.ps()))
        });
        found.reverse();
        Ok(found)
    }

    /// The pitch `step_size` steps on from `origin` walking `direction` on a
    /// realization from `reference` standing on `name`: music21's
    /// `nextPitch`. An origin on no node steps from its `neighbor`.
    ///
    /// # Errors
    ///
    /// No node named `name`, or an origin with no node to step from.
    #[allow(clippy::too_many_arguments)]
    pub fn next_pitch(
        &self,
        reference: &Pitch,
        name: NodeName,
        origin: &Pitch,
        direction: Direction,
        step_size: usize,
        neighbor: Neighbor,
        draw: &mut Draw<'_>,
    ) -> Result<Option<Pitch>> {
        let (mut node, mut pitch) =
            self.node_and_pitch_at(reference, name, origin, direction, neighbor, draw)?;
        let named = self.nodes_named(name, Termini::Equate, Moduli::Permit)?;
        let terminus = named
            .first()
            .and_then(|first| self.terminus_name(*first, reference));
        let mut next = None;
        for _ in 0..step_size {
            let (interval, reached) = self.step_from(node, direction, draw)?;
            node = reached;
            pitch = match direction {
                Direction::Ascending => self.transpose_step(interval, &pitch)?,
                _ => self.transpose_step(&interval.reversed()?, &pitch)?,
            };
            pitch = self.spell_terminus(pitch, node, terminus.as_deref())?;
            next = Some(pitch.clone());
        }
        Ok(next)
    }

    /// The node `next_pitch` steps from and its pitch nearest `origin`: at
    /// or below it when stepping up from a match or down from a lower
    /// neighbor, else at or above it.
    fn node_and_pitch_at(
        &self,
        reference: &Pitch,
        name: NodeName,
        origin: &Pitch,
        direction: Direction,
        neighbor: Neighbor,
        draw: &mut Draw<'_>,
    ) -> Result<(NodeId, Pitch)> {
        let origin_ps = origin.ps();
        let span = FloatType::from(self.search_span());
        let range = PitchRange::between(
            Pitch::from_number(origin_ps - span)?,
            Pitch::from_number(origin_ps + span)?,
        );
        let realized = self.realize(
            reference,
            Some(name),
            &range,
            direction,
            Order::Ascending,
            draw,
        )?;
        let (node, at_or_below) =
            self.start_node(&realized, origin_ps, direction, neighbor, draw)?;

        // The pitch of that node nearest the origin on the side wanted, the
        // first of equals.
        let mut best: Option<&Pitch> = None;
        for (pitch, held) in realized.pitches.iter().zip(&realized.nodes) {
            if Some(*held) != node {
                continue;
            }
            let ps = pitch.ps();
            let better = match (at_or_below, best) {
                (true, _) if ps > origin_ps => false,
                (false, _) if ps < origin_ps => false,
                (_, None) => true,
                (true, Some(held)) => ps > held.ps(),
                (false, Some(held)) => ps < held.ps(),
            };
            if better {
                best = Some(pitch);
            }
        }
        match (node, best) {
            (Some(node), Some(pitch)) => Ok((node, pitch.clone())),
            _ => Err(Error::Scale(
                "Could not find a pitch for the requested node degree".to_string(),
            )),
        }
    }

    /// The node sounding `origin_ps`, chosen by weight among several, or
    /// its neighbor; and whether its pitch is sought at or below the
    /// origin.
    fn start_node(
        &self,
        realized: &Realized,
        origin_ps: FloatType,
        direction: Direction,
        neighbor: Neighbor,
        draw: &mut Draw<'_>,
    ) -> Result<(Option<NodeId>, bool)> {
        let mut matched: Vec<NodeId> = Vec::new();
        let mut low = None;
        let mut high = None;
        for (pitch, node) in realized.pitches.iter().zip(&realized.nodes) {
            let ps = pitch.ps();
            if ps == origin_ps {
                if !matched.contains(node) {
                    matched.push(*node);
                }
            } else if high.is_none() {
                if ps < origin_ps {
                    low = Some(*node);
                } else {
                    high = Some(*node);
                }
            }
        }

        let ascending = direction == Direction::Ascending;
        if !matched.is_empty() {
            let mut degrees = Vec::with_capacity(matched.len());
            for node in &matched {
                degrees.push((self.node_or_err(*node)?.degree, *node));
            }
            degrees.sort_by_key(|(degree, _)| *degree);
            let sorted: Vec<NodeId> = degrees.into_iter().map(|(_, node)| node).collect();
            return Ok((self.weighted_node(&sorted, draw)?, ascending));
        }
        Ok(match neighbor {
            Neighbor::Never => (None, false),
            Neighbor::Below => (low, true),
            Neighbor::Above => (high, false),
        })
    }
}

impl ScaleType {
    /// The network music21 realizes this scale through: its steps walked
    /// both ways, or, for the scales that come down by other steps, music21's
    /// own nodes and edges.
    ///
    /// # Errors
    ///
    /// None in practice; the steps are fixed.
    pub fn network(self) -> Result<IntervalNetwork> {
        let mut network = IntervalNetwork::default();
        match self {
            Self::MelodicMinor => network.fill_melodic_minor()?,
            Self::RagAsawari => network.fill_table(&RAG_ASAWARI_NODES, &RAG_ASAWARI_EDGES)?,
            Self::RagMarwa => network.fill_table(&RAG_MARWA_NODES, &RAG_MARWA_EDGES)?,
            _ => {
                let steps = self
                    .realization_steps()
                    .into_iter()
                    .map(Interval::from_name)
                    .collect::<Result<Vec<_>>>()?;
                network.fill_bidirected(&steps);
            }
        }
        network.octaves = Octaves::Duplicating;
        network.simplification = self.simplification();
        Ok(network)
    }
}

/// A node of a network written as a table: its id and degree.
type TableNode = (NodeId, IntegerType);

/// An edge of a network written as a table: its step, the nodes it
/// connects, and the way it is walked.
type TableEdge = (&'static str, NodeId, NodeId, Direction);

const UP: Direction = Direction::Ascending;
const DOWN: Direction = Direction::Descending;
const BOTH: Direction = Direction::Bi;
const LOW: NodeId = NodeId::LOW;
const HIGH: NodeId = NodeId::HIGH;

const fn at(index: usize) -> NodeId {
    NodeId::Index(index)
}

/// music21's `fillMelodicMinor`: A B C D E, then F# G# going up and G F
/// coming down.
const MELODIC_MINOR_NODES: [TableNode; 10] = [
    (LOW, 1),
    (at(0), 2),
    (at(1), 3),
    (at(2), 4),
    (at(3), 5),
    (at(4), 6),
    (at(5), 6),
    (at(6), 7),
    (at(7), 7),
    (HIGH, 8),
];

const MELODIC_MINOR_EDGES: [TableEdge; 10] = [
    ("M2", LOW, at(0), BOTH),
    ("m2", at(0), at(1), BOTH),
    ("M2", at(1), at(2), BOTH),
    ("M2", at(2), at(3), BOTH),
    ("M2", at(3), at(4), UP),
    ("M2", at(4), at(6), UP),
    ("m2", at(6), HIGH, UP),
    ("M2", HIGH, at(7), DOWN),
    ("M2", at(7), at(5), DOWN),
    ("m2", at(5), at(3), DOWN),
];

/// music21's `AbstractRagAsawari`: C D F G A- going up, the natural minor
/// coming down.
const RAG_ASAWARI_NODES: [TableNode; 12] = [
    (LOW, 1),
    (at(0), 2),
    (at(1), 4),
    (at(2), 5),
    (at(3), 6),
    (HIGH, 8),
    (at(4), 7),
    (at(5), 6),
    (at(6), 5),
    (at(7), 4),
    (at(8), 3),
    (at(9), 2),
];

const RAG_ASAWARI_EDGES: [TableEdge; 12] = [
    ("M2", LOW, at(0), UP),
    ("m3", at(0), at(1), UP),
    ("M2", at(1), at(2), UP),
    ("m2", at(2), at(3), UP),
    ("M3", at(3), HIGH, UP),
    ("M2", HIGH, at(4), DOWN),
    ("M2", at(4), at(5), DOWN),
    ("m2", at(5), at(6), DOWN),
    ("M2", at(6), at(7), DOWN),
    ("M2", at(7), at(8), DOWN),
    ("m2", at(8), at(9), DOWN),
    ("M2", at(9), LOW, DOWN),
];

/// music21's `AbstractRagMarwa`: going up it dips from B back to A before
/// the octave, and coming down it starts from the D- above the octave.
const RAG_MARWA_NODES: [TableNode; 14] = [
    (LOW, 1),
    (at(0), 2),
    (at(1), 3),
    (at(2), 4),
    (at(3), 5),
    (at(4), 6),
    (at(5), 7),
    (HIGH, 8),
    (at(6), 7),
    (at(7), 6),
    (at(8), 5),
    (at(9), 4),
    (at(10), 3),
    (at(11), 2),
];

const RAG_MARWA_EDGES: [TableEdge; 14] = [
    ("m2", LOW, at(0), UP),
    ("A2", at(0), at(1), UP),
    ("M2", at(1), at(2), UP),
    ("m3", at(2), at(3), UP),
    ("M2", at(3), at(4), UP),
    ("-M2", at(4), at(5), UP),
    ("m3", at(5), HIGH, UP),
    ("-m2", HIGH, at(6), DOWN),
    ("d3", at(6), at(7), DOWN),
    ("M2", at(7), at(8), DOWN),
    ("m3", at(8), at(9), DOWN),
    ("M2", at(9), at(10), DOWN),
    ("A2", at(10), at(11), DOWN),
    ("m2", at(11), LOW, DOWN),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn pitch(name: &str) -> Pitch {
        Pitch::from_name(name).unwrap()
    }

    fn names(pitches: &[Pitch]) -> Vec<String> {
        pitches.iter().map(Pitch::name_with_octave).collect()
    }

    fn major() -> IntervalNetwork {
        IntervalNetwork::bidirected(&["M2", "M2", "m2", "M2", "M2", "M2", "m2"]).unwrap()
    }

    fn melodic_minor() -> IntervalNetwork {
        let mut network = IntervalNetwork::default();
        network.fill_melodic_minor().unwrap();
        network
    }

    fn no_draw() -> FloatType {
        0.0
    }

    #[test]
    fn a_major_scale_is_realized_on_any_degree() {
        let network = major();
        let realized = network
            .realize_pitches(
                &pitch("C#"),
                Some(NodeName::Degree(7)),
                &PitchRange::default(),
            )
            .unwrap();
        assert_eq!(
            names(&realized),
            ["D3", "E3", "F#3", "G3", "A3", "B3", "C#4", "D4"]
        );
        let ranged = network
            .realize_pitches(
                &pitch("F4"),
                Some(NodeName::Degree(1)),
                &PitchRange::between(pitch("C3"), pitch("C4")),
            )
            .unwrap();
        assert_eq!(
            names(&ranged),
            ["C3", "D3", "E3", "F3", "G3", "A3", "Bb3", "C4"]
        );
    }

    #[test]
    fn a_fifth_cycle_need_not_repeat_at_the_octave() {
        let network = IntervalNetwork::bidirected(&["P5", "P5", "P5"]).unwrap();
        let realized = network
            .realize_pitches(
                &pitch("C4"),
                Some(NodeName::Degree(1)),
                &PitchRange::between(pitch("C4"), pitch("C11")),
            )
            .unwrap();
        assert_eq!(realized.len(), 13);
        assert_eq!(realized[12].name_with_octave(), "B#10");
    }

    #[test]
    fn a_melodic_minor_comes_down_by_other_steps() {
        let network = melodic_minor();
        let mut draw = no_draw;
        let down = network
            .realize(
                &pitch("G4"),
                Some(NodeName::Id(NodeId::HIGH)),
                &PitchRange::default(),
                Direction::Descending,
                Order::Ascending,
                &mut draw,
            )
            .unwrap();
        assert_eq!(
            names(&down.pitches),
            ["G3", "A3", "Bb3", "C4", "D4", "Eb4", "F4", "G4"]
        );
        let sixth = network
            .pitch_of_degree(
                &pitch("C4"),
                NodeName::Degree(1),
                NodeName::Degree(6),
                Direction::Descending,
                &PitchRange::default(),
                Termini::Equate,
                &mut draw,
            )
            .unwrap()
            .unwrap();
        assert_eq!(sixth.name_with_octave(), "Ab4");
    }

    #[test]
    fn the_next_pitch_steps_from_a_neighbor() {
        let network = melodic_minor();
        let mut draw = no_draw;
        let next = network
            .next_pitch(
                &pitch("C4"),
                NodeName::Degree(1),
                &pitch("C#4"),
                Direction::Ascending,
                1,
                Neighbor::Below,
                &mut draw,
            )
            .unwrap()
            .unwrap();
        assert_eq!(next.name_with_octave(), "D4");
        let neighbors = network
            .neighbor_node_ids(
                &pitch("C4"),
                Some(NodeName::Degree(1)),
                &pitch("C#"),
                Direction::Ascending,
                &mut draw,
            )
            .unwrap();
        assert_eq!(neighbors, Some((Some(NodeId::HIGH), NodeId::Index(0))));
    }

    #[test]
    fn find_ranks_the_tonics_by_matches() {
        let targets: Vec<Pitch> = ["g", "a", "b", "d", "f#"].map(pitch).to_vec();
        let found = major()
            .find(&targets, DegreeComparison::PitchClass)
            .unwrap();
        let top: Vec<(usize, String)> = found[..4]
            .iter()
            .map(|found| (found.matches, found.tonic.name()))
            .collect();
        assert_eq!(
            top,
            [
                (5, "G".to_string()),
                (5, "D".to_string()),
                (4, "A".to_string()),
                (4, "C".to_string())
            ]
        );
    }

    #[test]
    fn rag_marwa_sounds_one_note_on_two_degrees() {
        let network = ScaleType::RagMarwa.network().unwrap();
        let mut draw = no_draw;
        let mut degrees = |target: &str, direction| {
            let nodes = network
                .relative_node_ids(
                    &pitch("C4"),
                    Some(NodeName::Degree(1)),
                    &pitch(target),
                    DegreeComparison::Ps,
                    direction,
                    &mut draw,
                )
                .unwrap();
            nodes
                .into_iter()
                .map(|node| network.node(node).unwrap().degree)
                .collect::<Vec<_>>()
        };
        assert_eq!(degrees("A4", Direction::Ascending), [5, 7]);
        assert_eq!(degrees("D-3", Direction::Descending), [2, 7]);
    }

    #[test]
    fn a_degree_past_the_ends_folds_back() {
        let network = major();
        assert_eq!(network.degree_modulus(0).unwrap(), 7);
        assert_eq!(network.degree_modulus(9).unwrap(), 2);
        assert_eq!(
            network
                .nodes_named(NodeName::Degree(-1), Termini::Equate, Moduli::Permit)
                .unwrap(),
            [NodeId::Index(4)]
        );
    }

    #[test]
    fn a_one_way_edge_has_no_both_ways() {
        let mut edge = Edge::new(Interval::from_name("M3").unwrap(), Some(0));
        edge.connect(NodeId::Index(0), NodeId::Index(1), Direction::Ascending)
            .unwrap();
        assert!(edge.connections_toward(Direction::Bi).is_err());
        assert!(
            edge.connections_toward(Direction::Descending)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn weights_choose_between_branches() {
        let mut network = IntervalNetwork::default();
        let mut heavy = Node::new(NodeId::Index(0), 2);
        heavy.weight = 3.0;
        let nodes = vec![
            Node::new(NodeId::LOW, 1),
            heavy,
            Node::new(NodeId::Index(1), 2),
            Node::new(NodeId::HIGH, 3),
        ];
        let edge = |name: &str, from, to| EdgeSpec {
            interval: Interval::from_name(name).unwrap(),
            connections: vec![(from, to, Direction::Ascending)],
        };
        let edges = vec![
            edge("M2", NodeId::LOW, NodeId::Index(0)),
            edge("m2", NodeId::LOW, NodeId::Index(1)),
            edge("P4", NodeId::Index(0), NodeId::HIGH),
            edge("A4", NodeId::Index(1), NodeId::HIGH),
        ];
        network.fill_arbitrary(nodes, edges).unwrap();
        let open = PitchRange::default();
        let mut low_draw = || 0.1;
        let heavy_way = network
            .realize_ascending(&pitch("C4"), None, &open, Fill::Never, &mut low_draw)
            .unwrap();
        assert_eq!(names(&heavy_way.pitches), ["C4", "D4", "G4"]);
        let mut high_draw = || 0.9;
        let light_way = network
            .realize_ascending(&pitch("C4"), None, &open, Fill::Never, &mut high_draw)
            .unwrap();
        assert_eq!(names(&light_way.pitches), ["C4", "Db4", "G4"]);
    }
}
