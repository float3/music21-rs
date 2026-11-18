//! music21's `tree.verticality`: what starts, sounds across and stops at
//! one offset of a tree, and runs of such moments.
//!
//! A verticality holds its three tuples of timespans, as music21's does,
//! and asks its tree for the one after or before it when asked, so a tree
//! edited while it is walked is walked as it now stands. What only the
//! crate works out -- `makeElement`, the voice-leading pairs -- is read
//! from the crate's reading of the stream, which a tree keeps only while
//! it is as read.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use std::collections::VecDeque;

use pyo3::exceptions::{PyAttributeError, PyNotImplementedError, PyStopIteration};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyFloat, PyList, PySet, PyString, PyTuple};

use music21::tree::TimespanTree as RsTree;
use music21::tree::Verticality as RsVerticality;

use super::analysis::Horizontality;
use super::{ElementTimespan, TimespanTree};
use crate::duration::op_frac;
use crate::exceptions21::TreeException;
use crate::stream::{answers_to_class, stream_error};

pyo3::create_exception!(music21_rs_facade, VerticalityException, TreeException);

/// What starts, stops and sounds across one offset of a tree: music21's
/// `tree.verticality.Verticality`.
#[pyclass(name = "Verticality", module = "music21.tree.verticality", frozen)]
pub struct TreeVerticality {
    /// The tree it is of, if any: an `OffsetTree` or a `TimespanTree`.
    tree: Option<Py<PyAny>>,
    /// The offset as given, which is how music21 shows it.
    given: Py<PyAny>,
    starts: Py<PyTuple>,
    overlaps: Py<PyTuple>,
    stops: Py<PyTuple>,
    /// Whether it is the empty one a run is padded out with at the tree's
    /// end.
    sentinel: bool,
}

/// The verticality of a tree at an offset, read from the tree as it now
/// stands.
pub(crate) fn verticality_at<'py>(
    tree: &Bound<'py, PyAny>,
    offset: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, TreeVerticality>> {
    let py = tree.py();
    let tuple = |name: &str| -> PyResult<Py<PyTuple>> {
        Ok(tree
            .call_method1(name, (offset,))?
            .cast_into::<PyTuple>()?
            .unbind())
    };
    let made = TreeVerticality {
        tree: Some(tree.clone().unbind()),
        given: offset.clone().unbind(),
        starts: tuple("elementsStartingAt")?,
        overlaps: tuple("elementsOverlappingOffset")?,
        stops: tuple("elementsStoppingAt")?,
        sentinel: false,
    };
    Bound::new(py, made)
}

/// The empty verticality a run is padded out with, at a tree's end.
fn sentinel_of<'py>(tree: &Bound<'py, PyAny>) -> PyResult<Bound<'py, TreeVerticality>> {
    let py = tree.py();
    let made = TreeVerticality {
        tree: Some(tree.clone().unbind()),
        given: tree.getattr("endTime")?.unbind(),
        starts: PyTuple::empty(py).unbind(),
        overlaps: PyTuple::empty(py).unbind(),
        stops: PyTuple::empty(py).unbind(),
        sentinel: true,
    };
    Bound::new(py, made)
}

/// A tuple given for a verticality, or the exception music21 raises.
fn tuple_given(
    py: Python<'_>,
    name: &str,
    given: Option<&Bound<'_, PyAny>>,
) -> PyResult<Py<PyTuple>> {
    let Some(given) = given.filter(|given| !given.is_none()) else {
        return Ok(PyTuple::empty(py).unbind());
    };
    match given.cast::<PyTuple>() {
        Ok(tuple) => Ok(tuple.clone().unbind()),
        Err(_) => Err(VerticalityException::new_err(format!(
            "{name} must be a tuple of ElementTimespans, not {}",
            given.repr()?
        ))),
    }
}

impl TreeVerticality {
    /// The crate's verticality of this one, over the crate tree given.
    fn of<'t, 'a>(&self, py: Python<'_>, tree: &'t RsTree<'a>) -> PyResult<RsVerticality<'t, 'a>> {
        let offset: f64 = self.given.bind(py).extract()?;
        if !self.sentinel {
            return Ok(tree.verticality_at(offset));
        }

        // The crate pads a run longer than the tree with the empty
        // verticality at its end, which is this one.
        let count = tree.all_offsets().len() + 1;
        Ok(tree
            .verticalities_nwise(count, false, false)
            .ok()
            .and_then(|mut runs| runs.pop())
            .and_then(|mut run| run.pop())
            .unwrap_or_else(|| tree.verticality_at(offset)))
    }

    /// The tree as a `TimespanTree`, for what only the crate works out.
    fn timespans<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, TimespanTree>> {
        let tree = self.tree.as_ref().map(|tree| tree.bind(py).clone());
        match tree.map(|tree| tree.cast_into::<TimespanTree>()) {
            Some(Ok(tree)) => Ok(tree),
            _ => Err(PyNotImplementedError::new_err(
                "this is worked out by music21-rs only for a verticality of a TimespanTree \
                 read by asTimespans()",
            )),
        }
    }

    /// The timespans starting here, then those sounding across.
    fn sounding<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let mut spans: Vec<Bound<'py, PyAny>> = self.starts.bind(py).iter().collect();
        spans.extend(self.overlaps.bind(py).iter());
        PyTuple::new(py, spans)
    }

    /// The pitches of what starts or sounds across, each name and octave
    /// once, in the order first met: the elements' own.
    fn pitch_objects<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let mut names: Vec<String> = Vec::new();
        let mut pitches = Vec::new();
        for span in self.sounding(py)?.iter() {
            let Ok(held) = span.getattr("pitches") else {
                continue;
            };
            for pitch in held.try_iter()? {
                let pitch = pitch?;
                let name: String = pitch.getattr("nameWithOctave")?.extract()?;
                if !names.contains(&name) {
                    names.push(name);
                    pitches.push(pitch);
                }
            }
        }
        Ok(pitches)
    }

    /// The same, lowest first.
    fn sorted_pitches<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let mut keyed = Vec::new();
        for pitch in self.pitch_objects(py)? {
            keyed.push((pitch.getattr("ps")?.extract::<f64>()?, pitch));
        }
        keyed.sort_by(|left, right| left.0.total_cmp(&right.0));
        Ok(keyed.into_iter().map(|(_, pitch)| pitch).collect())
    }

    /// The verticality at the tree's position after or before this one.
    fn neighbour<'py>(&self, py: Python<'py>, after: bool) -> PyResult<Option<Bound<'py, PyAny>>> {
        let Some(tree) = &self.tree else {
            return Ok(None);
        };
        let tree = tree.bind(py);
        let method = match after {
            true => "getPositionAfter",
            false => "getPositionBefore",
        };
        let offset = tree.call_method1(method, (self.given.bind(py),))?;
        if offset.is_none() {
            return Ok(None);
        }
        Ok(Some(tree.call_method1("getVerticalityAt", (offset,))?))
    }
}

#[pymethods]
impl TreeVerticality {
    #[new]
    #[pyo3(signature = (
        offset = None, overlapTimespans = None, startTimespans = None,
        stopTimespans = None, timespanTree = None
    ))]
    fn py_new(
        py: Python<'_>,
        offset: Option<&Bound<'_, PyAny>>,
        overlapTimespans: Option<&Bound<'_, PyAny>>,
        startTimespans: Option<&Bound<'_, PyAny>>,
        stopTimespans: Option<&Bound<'_, PyAny>>,
        timespanTree: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let given = match offset {
            Some(offset) => offset.clone().unbind(),
            None => PyFloat::new(py, 0.0).into_any().unbind(),
        };
        Ok(Self {
            tree: timespanTree
                .filter(|tree| !tree.is_none())
                .map(|tree| tree.clone().unbind()),
            given,
            starts: tuple_given(py, "startTimespans", startTimespans)?,
            stops: tuple_given(py, "stopTimespans", stopTimespans)?,
            overlaps: tuple_given(py, "overlapTimespans", overlapTimespans)?,
            sentinel: false,
        })
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        if let Some(tree) = &self.tree {
            visit.call(tree)?;
        }
        visit.call(&self.given)?;
        visit.call(&self.starts)?;
        visit.call(&self.overlaps)?;
        visit.call(&self.stops)
    }

    /// The offset.
    #[getter]
    fn offset(&self, py: Python<'_>) -> Py<PyAny> {
        self.given.clone_ref(py)
    }

    /// The tree it is of, where that is a `TimespanTree`.
    #[getter]
    fn timespanTree<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyAny>> {
        let tree = self.tree.as_ref()?.bind(py);
        tree.cast::<TimespanTree>().ok().map(|_| tree.clone())
    }

    /// The tree it is of.
    #[getter]
    fn offsetTree(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.tree.as_ref().map(|tree| tree.clone_ref(py))
    }

    /// The timespans starting here.
    #[getter]
    fn startTimespans(&self, py: Python<'_>) -> Py<PyTuple> {
        self.starts.clone_ref(py)
    }

    /// The timespans sounding across.
    #[getter]
    fn overlapTimespans(&self, py: Python<'_>) -> Py<PyTuple> {
        self.overlaps.clone_ref(py)
    }

    /// The timespans ending here.
    #[getter]
    fn stopTimespans(&self, py: Python<'_>) -> Py<PyTuple> {
        self.stops.clone_ref(py)
    }

    /// Those starting, then those sounding across.
    #[getter]
    fn startAndOverlapTimespans<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.sounding(py)
    }

    /// The timespan sounding the lowest pitch, the last of those sounding
    /// it alike.
    #[getter]
    fn bassTimespan<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        let sorted = py.import("builtins")?.getattr("sorted")?;
        let mut lowest: Option<(Bound<'py, PyAny>, Bound<'py, PyAny>)> = None;
        for span in self.sounding(py)?.iter() {
            let Ok(pitches) = span.getattr("pitches") else {
                continue;
            };
            if !pitches.is_truthy()? {
                continue;
            }
            let low = sorted.call1((pitches,))?.get_item(0)?;
            let lower = match &lowest {
                Some((held, _)) => low.le(held)?,
                None => true,
            };
            if lower {
                lowest = Some((low, span));
            }
        }
        Ok(lowest.map(|(_, span)| span))
    }

    /// The beat strength of the first timespan starting here, `None`
    /// where none starts.
    #[getter]
    fn beatStrength<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        let starts = self.starts.bind(py);
        if starts.is_empty() {
            return Ok(None);
        }
        Ok(Some(
            starts
                .get_item(0)?
                .getattr("element")?
                .getattr("beatStrength")?,
        ))
    }

    /// Each pitch sounding, each name and octave once: the elements' own
    /// pitch objects.
    #[getter]
    fn pitchSet<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PySet>> {
        PySet::new(py, self.pitch_objects(py)?)
    }

    /// One pitch sounding of each pitch class.
    #[getter]
    fn pitchClassSet<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PySet>> {
        let mut classes: Vec<i64> = Vec::new();
        let mut pitches = Vec::new();
        for pitch in self.pitch_objects(py)? {
            let class: i64 = pitch.getattr("pitchClass")?.extract()?;
            if !classes.contains(&class) {
                classes.push(class);
                pitches.push(pitch);
            }
        }
        PySet::new(py, pitches)
    }

    /// A chord of the pitches sounding, lowest first.
    fn toChord<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let pitches = PyList::new(py, self.sorted_pitches(py)?)?;
        py.get_type::<crate::chord::Chord>().call1((pitches,))
    }

    /// The measure of the first timespan starting here.
    #[getter]
    fn measureNumber<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.starts.bind(py).get_item(0)?.getattr("measureNumber")
    }

    /// The next offset a timespan starts at, `None` without a tree.
    #[getter]
    fn nextStartOffset<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        let Some(tree) = &self.tree else {
            return Ok(None);
        };
        let after = tree
            .bind(py)
            .call_method1("getPositionAfter", (self.given.bind(py),))?;
        Ok(Some(after).filter(|after| !after.is_none()))
    }

    /// The verticality at the next offset a timespan starts at, as the
    /// tree now stands.
    #[getter]
    fn nextVerticality<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.neighbour(py, true)
    }

    /// The verticality at the last offset before this one a timespan
    /// starts at, as the tree now stands.
    #[getter]
    fn previousVerticality<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.neighbour(py, false)
    }

    /// How long until the next start, or the end of the tree after the
    /// last; `None` without a tree.
    #[getter]
    fn timeToNextEvent<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        let next = match self.nextStartOffset(py)? {
            Some(next) => next,
            None => match self.timespanTree(py) {
                Some(tree) => tree.getattr("endTime")?,
                None => return Ok(None),
            },
        };
        let length: f64 = next.sub(self.given.bind(py))?.extract()?;
        Ok(Some(op_frac(py, length)?))
    }

    /// A chord of every pitch sounding here, lasting `quarterLength` or,
    /// without one, until the next start, or a rest where nothing sounds.
    #[pyo3(signature = (
        quarterLength = None,
        *,
        addTies = true,
        addPartIdAsGroup = false,
        removeRedundantPitches = true,
        gatherArticulations = None,
        gatherExpressions = None,
        copyPitches = true,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn makeElement<'py>(
        &self,
        py: Python<'py>,
        quarterLength: Option<&Bound<'py, PyAny>>,
        addTies: bool,
        addPartIdAsGroup: bool,
        removeRedundantPitches: bool,
        gatherArticulations: Option<&Bound<'py, PyAny>>,
        gatherExpressions: Option<&Bound<'py, PyAny>>,
        copyPitches: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let length = match quarterLength {
            Some(length) => length.clone(),
            None => match self.timeToNextEvent(py)? {
                Some(length) => length,
                None => PyFloat::new(py, 1.0).into_any(),
            },
        };
        let options = super::makeelement::Options {
            add_ties: addTies,
            group_by_part: addPartIdAsGroup,
            remove_redundant: removeRedundantPitches,
            articulations: super::makeelement::Gather::of(gatherArticulations)?,
            expressions: super::makeelement::Gather::of(gatherExpressions)?,
            copy_pitches: copyPitches,
        };
        let sounding: Vec<Bound<'py, PyAny>> = self.sounding(py)?.iter().collect();
        let has_pitches = !self.pitch_objects(py)?.is_empty();
        super::makeelement::make_element(
            py,
            self.given.bind(py),
            length,
            &sounding,
            has_pitches,
            &options,
        )
    }

    /// Each two pairs of `getPairedMotion`, the earlier pair first, as
    /// music21's `VoiceLeadingQuartet` where all four are notes or, without
    /// `returnObjects`, as the pairs of timespans.
    #[pyo3(signature = (
        *,
        includeRests = true,
        includeOblique = true,
        includeNoMotion = false,
        returnObjects = true,
        partPairNumbers = None,
    ))]
    fn getAllVoiceLeadingQuartets<'py>(
        &self,
        py: Python<'py>,
        includeRests: bool,
        includeOblique: bool,
        includeNoMotion: bool,
        returnObjects: bool,
        partPairNumbers: Option<Vec<(usize, usize)>>,
    ) -> PyResult<Bound<'py, PyList>> {
        let owner = self.timespans(py)?;
        let owner = owner.borrow();
        let tree = owner.crate_tree()?;
        let quartets = self
            .of(py, &tree)?
            .voice_leading_timespans(
                includeRests,
                includeOblique,
                includeNoMotion,
                partPairNumbers.as_deref(),
            )
            .map_err(|error| stream_error(&error))?;
        let out = PyList::empty(py);
        for quartet in quartets {
            let [first, second, third, fourth] = quartet;
            let spans = [
                owner.span(py, &tree, first)?.into_bound(py),
                owner.span(py, &tree, second)?.into_bound(py),
                owner.span(py, &tree, third)?.into_bound(py),
                owner.span(py, &tree, fourth)?.into_bound(py),
            ];
            if !returnObjects {
                let upper = PyTuple::new(py, [&spans[0], &spans[1]])?;
                let lower = PyTuple::new(py, [&spans[2], &spans[3]])?;
                out.append(PyTuple::new(py, [upper, lower])?)?;
                continue;
            }
            if let Some(made) = quartet_of(py, &spans)? {
                out.append(made)?;
            }
        }
        Ok(out)
    }

    /// Each pitched timespan starting here paired with the one before it
    /// in its part, and, with `includeOblique`, each pitched one sounding
    /// across paired with itself.
    #[pyo3(signature = (*, includeRests = true, includeOblique = true))]
    fn getPairedMotion<'py>(
        &self,
        py: Python<'py>,
        includeRests: bool,
        includeOblique: bool,
    ) -> PyResult<Bound<'py, PyList>> {
        let owner = self.timespans(py)?;
        let owner = owner.borrow();
        let tree = owner.crate_tree()?;
        let pairs = self
            .of(py, &tree)?
            .paired_motion(includeRests, includeOblique);
        let out = PyList::empty(py);
        for (before, after) in pairs {
            let pair = [
                owner.span(py, &tree, before)?,
                owner.span(py, &tree, after)?,
            ];
            out.append(PyTuple::new(py, pair)?)?;
        }
        Ok(out)
    }

    /// What Python asks for a name the class lacks: music21's
    /// `AttributeError`, naming the class as music21's does.
    fn __getattr__(&self, name: &str) -> PyResult<Py<PyAny>> {
        Err(PyAttributeError::new_err(format!(
            "'Verticality' object has no attribute '{name}'"
        )))
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let mut names = Vec::new();
        for pitch in self.sorted_pitches(py)? {
            names.push(pitch.getattr("nameWithOctave")?.extract::<String>()?);
        }
        Ok(format!(
            "<music21.tree.verticality.Verticality {} {{{}}}>",
            self.given.bind(py).str()?,
            names.join(" ")
        ))
    }
}

/// music21's `VoiceLeadingQuartet` of four timespans' notes, where all four
/// are notes.
fn quartet_of<'py>(
    py: Python<'py>,
    spans: &[Bound<'py, ElementTimespan>; 4],
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let note = PyString::new(py, "Note");
    let mut notes = Vec::new();
    for span in spans {
        let element = span.getattr("element")?;
        if !answers_to_class(&element, &note)? {
            return Ok(None);
        }
        notes.push(element);
    }
    let class = crate::installed_class(py, "music21.voiceLeading", "VoiceLeadingQuartet")
        .unwrap_or_else(|| {
            py.get_type::<crate::voiceleading::VoiceLeadingQuartet>()
                .into_any()
        });
    class
        .call1((&notes[0], &notes[1], &notes[2], &notes[3]))
        .map(Some)
}

/// A run of verticalities: music21's `tree.verticality.VerticalitySequence`.
#[pyclass(
    name = "VerticalitySequence",
    module = "music21.tree.verticality",
    frozen
)]
pub struct VerticalitySequence {
    verticalities: Py<PyTuple>,
}

#[pymethods]
impl VerticalitySequence {
    #[new]
    pub(crate) fn py_new(verticalities: &Bound<'_, PyAny>) -> PyResult<Self> {
        let py = verticalities.py();
        let held = verticalities.try_iter()?.collect::<PyResult<Vec<_>>>()?;
        Ok(Self {
            verticalities: PyTuple::new(py, held)?.unbind(),
        })
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.verticalities)
    }

    fn __len__(&self, py: Python<'_>) -> usize {
        self.verticalities.bind(py).len()
    }

    fn __getitem__<'py>(
        &self,
        py: Python<'py>,
        item: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.verticalities.bind(py).as_any().get_item(item)
    }

    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        Ok(self.verticalities.bind(py).as_any().try_iter()?.into_any())
    }

    /// Each part's timespans across the run -- those sounding across or
    /// starting at the first, then those starting at the rest -- as a
    /// dictionary of the part and its `Horizontality`.
    pub(crate) fn unwrap<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let grouped = PyDict::new(py);
        let add = |span: Bound<'py, PyAny>| -> PyResult<()> {
            let part = span.getattr("part")?;
            match grouped.get_item(&part)? {
                Some(held) => held.call_method1("append", (span,))?,
                None => {
                    grouped.set_item(&part, PyList::new(py, [span])?)?;
                    return Ok(());
                }
            };
            Ok(())
        };
        let verticalities = self.verticalities.bind(py);
        for (place, verticality) in verticalities.iter().enumerate() {
            if place == 0 {
                for span in verticality.getattr("overlapTimespans")?.try_iter()? {
                    add(span?)?;
                }
            }
            for span in verticality.getattr("startTimespans")?.try_iter()? {
                add(span?)?;
            }
        }
        let unwrapped = PyDict::new(py);
        for (part, spans) in grouped.iter() {
            unwrapped.set_item(part, Horizontality::of(&spans)?)?;
        }
        Ok(unwrapped)
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let mut inner = Vec::new();
        for verticality in self.verticalities.bind(py).iter() {
            let shown = verticality.repr()?.to_string();
            let body = shown
                .trim_start_matches("<music21.tree.verticality.Verticality ")
                .trim_end_matches('>');
            inner.push(format!("({body})"));
        }
        Ok(format!(
            "<music21.tree.verticality.VerticalitySequence [\n\t{}\n\t]>",
            inner.join(",\n\t")
        ))
    }
}

/// Which way a walk of a tree's verticalities goes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Direction {
    Forward,
    Backward,
}

/// Where a walk of verticalities stands.
enum WalkState {
    /// Nothing asked yet.
    Fresh,
    /// The last verticality handed out, whose neighbour comes next.
    After(Py<PyAny>),
    /// Run out.
    Done,
}

/// A tree's verticalities, each worked out from the last as the tree
/// stands when it is asked for: music21's `iterateVerticalities`.
#[pyclass(name = "VerticalityWalk", module = "music21.tree.timespanTree")]
pub struct VerticalityWalk {
    tree: Py<PyAny>,
    direction: Direction,
    state: WalkState,
}

impl VerticalityWalk {
    pub(crate) fn of(tree: &Bound<'_, PyAny>, direction: Direction) -> Self {
        Self {
            tree: tree.clone().unbind(),
            direction,
            state: WalkState::Fresh,
        }
    }

    /// The next verticality, or none once run out.
    fn step<'py>(&mut self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        let tree = self.tree.bind(py);
        let next = match &self.state {
            WalkState::Done => None,
            WalkState::Fresh => {
                if tree.getattr("rootNode")?.is_none() {
                    None
                } else {
                    let end = match self.direction {
                        Direction::Forward => tree.call_method0("lowestPosition")?,
                        Direction::Backward => tree.call_method0("highestPosition")?,
                    };
                    Some(tree.call_method1("getVerticalityAt", (end,))?)
                }
            }
            WalkState::After(last) => {
                let name = match self.direction {
                    Direction::Forward => "nextVerticality",
                    Direction::Backward => "previousVerticality",
                };
                Some(last.bind(py).getattr(name)?).filter(|next| !next.is_none())
            }
        };
        self.state = match &next {
            Some(next) => WalkState::After(next.clone().unbind()),
            None => WalkState::Done,
        };
        Ok(next)
    }
}

#[pymethods]
impl VerticalityWalk {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.tree)?;
        if let WalkState::After(last) = &self.state {
            visit.call(last)?;
        }
        Ok(())
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.step(py)?.ok_or_else(|| PyStopIteration::new_err(()))
    }
}

/// Where a walk of runs of verticalities stands.
enum RunState {
    /// No run handed out yet.
    Fresh,
    /// Each new verticality slides the run along.
    Sliding,
    /// Run out.
    Done,
}

/// Runs of `n` verticalities, each one on from the last, padded out at
/// the end where asked: music21's `iterateVerticalitiesNwise`, which reads
/// them with `more_itertools.windowed`.
#[pyclass(name = "VerticalityRuns", module = "music21.tree.timespanTree")]
pub struct VerticalityRuns {
    walk: Py<VerticalityWalk>,
    sentinel: Py<PyAny>,
    /// How many sentinels still come after the walk.
    padding: usize,
    size: usize,
    direction: Direction,
    run: VecDeque<Py<PyAny>>,
    state: RunState,
}

impl VerticalityRuns {
    /// Runs of `n` of a tree's verticalities.
    pub(crate) fn of(
        tree: &Bound<'_, PyAny>,
        size: usize,
        direction: Direction,
        pad: bool,
    ) -> PyResult<Self> {
        let py = tree.py();
        Ok(Self {
            walk: Py::new(py, VerticalityWalk::of(tree, direction))?,
            sentinel: sentinel_of(tree)?.into_any().unbind(),
            padding: if pad { size - 1 } else { 0 },
            size,
            direction,
            run: VecDeque::new(),
            state: RunState::Fresh,
        })
    }

    /// The next verticality of the walk, then the padding.
    fn pull(&mut self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        if let Some(next) = self.walk.bind(py).borrow_mut().step(py)? {
            return Ok(Some(next.unbind()));
        }
        if self.padding == 0 {
            return Ok(None);
        }
        self.padding -= 1;
        Ok(Some(self.sentinel.clone_ref(py)))
    }

    /// The run as it stands, earliest first.
    fn sequence(&self, py: Python<'_>) -> PyResult<VerticalitySequence> {
        let mut members: Vec<Py<PyAny>> = self.run.iter().map(|each| each.clone_ref(py)).collect();
        if self.direction == Direction::Backward {
            members.reverse();
        }
        Ok(VerticalitySequence {
            verticalities: PyTuple::new(py, members)?.unbind(),
        })
    }
}

#[pymethods]
impl VerticalityRuns {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.walk)?;
        visit.call(&self.sentinel)?;
        for each in &self.run {
            visit.call(each)?;
        }
        Ok(())
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<VerticalitySequence> {
        match self.state {
            RunState::Done => Err(PyStopIteration::new_err(())),
            RunState::Fresh => {
                // The first run takes `n`; a walk shorter than that is
                // padded with the sentinel, once, as `windowed` pads it.
                while self.run.len() < self.size {
                    let Some(next) = self.pull(py)? else {
                        break;
                    };
                    self.run.push_back(next);
                }
                if self.run.is_empty() {
                    self.state = RunState::Done;
                    return Err(PyStopIteration::new_err(()));
                }
                if self.run.len() < self.size {
                    while self.run.len() < self.size {
                        self.run.push_back(self.sentinel.clone_ref(py));
                    }
                    self.state = RunState::Done;
                    return self.sequence(py);
                }
                self.state = RunState::Sliding;
                self.sequence(py)
            }
            RunState::Sliding => {
                let Some(next) = self.pull(py)? else {
                    self.state = RunState::Done;
                    return Err(PyStopIteration::new_err(()));
                };
                self.run.pop_front();
                self.run.push_back(next);
                self.sequence(py)
            }
        }
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<TreeVerticality>()?;
    m.add_class::<VerticalitySequence>()?;
    Ok(())
}
