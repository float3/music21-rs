//! music21's `tree.fromStream`: a stream's elements put into an
//! `ElementTree`, an `OffsetTree` or a `TimespanTree`.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyFloat, PyList, PyModule, PyString, PyTuple};

use super::trees::{ElementTree, OffsetTree};
use super::{ElementTimespan, PitchedTimespan, TimespanTree, as_timespans, is_of_any, is_stream};
use crate::duration::op_frac;

/// Which tree a stream is put into.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TreeKind {
    /// Timespans, by offset.
    Timespans,
    /// Elements, one to a node, by `SortTuple`.
    Elements,
    /// Elements by offset.
    Offsets,
}

impl TreeKind {
    /// An empty tree of this kind of a stream.
    fn empty<'py>(self, source: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let py = source.py();
        let made = match self {
            Self::Timespans => Bound::new(py, TimespanTree::of(source)?)?.into_any(),
            Self::Elements => Bound::new(py, ElementTree::of(source)?)?.into_any(),
            Self::Offsets => Bound::new(py, OffsetTree::of(source)?)?.into_any(),
        };
        Ok(made)
    }
}

/// music21's `flatten`: `False`, `True` or `'semiFlat'`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Flattening {
    No,
    Yes,
    SemiFlat,
}

impl Flattening {
    fn of(flatten: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let Some(flatten) = flatten else {
            return Ok(Self::No);
        };
        if flatten
            .extract::<String>()
            .is_ok_and(|said| said == "semiFlat")
        {
            return Ok(Self::SemiFlat);
        }
        Ok(match flatten.is_truthy()? {
            true => Self::Yes,
            false => Self::No,
        })
    }
}

/// A stream's elements with their offsets in it.
fn held<'py>(stream: &Bound<'py, PyAny>) -> PyResult<Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>)>> {
    let py = stream.py();
    let mut placed: Vec<(f64, Bound<'py, PyAny>)> = Vec::new();
    for element in stream.getattr("elements")?.try_iter()? {
        let element = element?;
        let offset: f64 = stream
            .call_method1("elementOffset", (&element,))?
            .extract()?;
        placed.push((offset, element));
    }
    placed
        .into_iter()
        .map(|(offset, element)| Ok((op_frac(py, offset)?, element)))
        .collect()
}

/// A stream's elements in the order music21 stores them: its
/// `_elements` as they were put in, then its `_endElements`. music21
/// walks streams in this order to build a tree one element at a time,
/// which decides the tree's shape.
fn stored<'py>(
    stream: &Bound<'py, PyAny>,
) -> PyResult<Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>)>> {
    let mut keyed = Vec::new();
    for (offset, element) in held(stream)? {
        let position = element.call_method1("sortTuple", (stream,))?;
        let at_end: i64 = position.getattr("atEnd")?.extract()?;
        let index: i64 = position.getattr("insertIndex")?.extract()?;
        keyed.push(((at_end, index), offset, element));
    }
    keyed.sort_by_key(|(key, _, _)| *key);
    Ok(keyed
        .into_iter()
        .map(|(_, offset, element)| (offset, element))
        .collect())
}

/// A stream's elements sorted by their `SortTuple`s in it, each beside it.
fn sorted_tuples<'py>(
    stream: &Bound<'py, PyAny>,
    classList: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyList>> {
    let py = stream.py();
    let tuples = PyList::empty(py);
    for (_, element) in held(stream)? {
        if let Some(classes) = classList
            && !is_of_any(&element, classes)?
        {
            continue;
        }
        let position = element.call_method1("sortTuple", (stream,))?;
        tuples.append(PyTuple::new(py, [position, element])?)?;
    }
    let keywords = PyDict::new(py);
    let first = py.import("operator")?.getattr("itemgetter")?.call1((0,))?;
    keywords.set_item("key", first)?;
    tuples.call_method("sort", (), Some(&keywords))?;
    Ok(tuples)
}

/// A class list given, `None` for none or an empty one.
fn classes_given<'a, 'py>(
    classList: Option<&'a Bound<'py, PyAny>>,
) -> PyResult<Option<&'a Bound<'py, PyAny>>> {
    match classList {
        Some(classes) if !classes.is_none() && classes.is_truthy()? => Ok(Some(classes)),
        _ => Ok(None),
    }
}

/// Puts a stream's elements into a tree from a sorted list, which is
/// faster than one at a time: music21's
/// `makeFastShallowTreeFromSortedStream`. Answers the tree given.
#[pyfunction]
#[pyo3(signature = (inputStream, *, outputTree, classList = None))]
fn makeFastShallowTreeFromSortedStream<'py>(
    inputStream: &Bound<'py, PyAny>,
    outputTree: &Bound<'py, PyAny>,
    classList: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let tuples = sorted_tuples(inputStream, classes_given(classList)?)?;
    outputTree.call_method1("populateFromSortedList", (tuples,))?;
    let root = outputTree.getattr("rootNode")?;
    if !root.is_none() {
        root.call_method0("updateEndTimes")?;
    }
    Ok(outputTree.clone())
}

/// Where a stream ends, from where it starts in the stream read.
fn end_of<'py>(
    stream: &Bound<'py, PyAny>,
    start: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let length = stream.getattr("duration")?.getattr("quarterLength")?;
    op_frac(stream.py(), start.add(length)?.extract()?)
}

/// The timespan of an element: a `PitchedTimespan` of a note, chord or
/// stream, an `ElementTimespan` of anything else.
fn timespan_of<'py>(
    element: &Bound<'py, PyAny>,
    parentage: &Bound<'py, PyTuple>,
    parent_offset: &Bound<'py, PyAny>,
    parent_end: &Bound<'py, PyAny>,
    offset: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = element.py();
    let length = element.getattr("duration")?.getattr("quarterLength")?;
    let end = op_frac(py, offset.add(length)?.extract()?)?;
    let keywords = PyDict::new(py);
    keywords.set_item("parentage", parentage)?;
    keywords.set_item("parentOffset", parent_offset)?;
    keywords.set_item("parentEndTime", parent_end)?;
    let arguments = (offset, end, element);
    let pitched = is_stream(element) || is_of_any(element, &PyString::new(py, "NotRest"))?;
    match pitched {
        true => py
            .get_type::<PitchedTimespan>()
            .call(arguments, Some(&keywords)),
        false => py
            .get_type::<ElementTimespan>()
            .call(arguments, Some(&keywords)),
    }
}

/// The parentage as music21's timespans hold it: the nearest stream first.
fn reversed<'py>(parentage: &[Bound<'py, PyAny>]) -> PyResult<Bound<'py, PyTuple>> {
    let py = parentage[0].py();
    PyTuple::new(py, parentage.iter().rev())
}

/// What `listOfTreesByClass` is asked.
struct ByClass<'py> {
    classLists: Vec<Bound<'py, PyAny>>,
    flatten: Flattening,
    kind: TreeKind,
}

impl<'py> ByClass<'py> {
    /// One tree per class list of a stream, the streams it holds read
    /// through.
    fn trees(
        &self,
        stream: &Bound<'py, PyAny>,
        parentage: &[Bound<'py, PyAny>],
        initial: &Bound<'py, PyAny>,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let py = stream.py();
        let last = &parentage[parentage.len() - 1];
        let count = self.classLists.len().max(1);
        let trees = (0..count)
            .map(|_| self.kind.empty(last))
            .collect::<PyResult<Vec<_>>>()?;

        for (offset, element) in stored(stream)? {
            let offset = op_frac(py, offset.add(initial)?.extract()?)?;
            let nested = is_stream(&element);
            if nested {
                self.nest(&trees, &element, parentage, &offset)?;
            }
            if nested && self.flatten != Flattening::SemiFlat {
                continue;
            }
            self.place(&trees, &element, parentage, initial, &offset)?;
        }
        Ok(trees)
    }

    /// The trees of a stream held, put into the trees being built: their
    /// elements where flattening, else the trees themselves.
    fn nest(
        &self,
        trees: &[Bound<'py, PyAny>],
        element: &Bound<'py, PyAny>,
        parentage: &[Bound<'py, PyAny>],
        offset: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let mut inner_parentage = parentage.to_vec();
        inner_parentage.push(element.clone());
        let contained = self.trees(element, &inner_parentage, offset)?;
        for (tree, inner) in trees.iter().zip(contained) {
            if self.flatten == Flattening::No {
                tree.call_method1("insert", (inner.call_method0("lowestPosition")?, &inner))?;
                continue;
            }
            let all = inner.get_item(pyo3::types::PySlice::full(element.py()))?;
            tree.call_method1("insert", (all,))?;
        }
        Ok(())
    }

    /// One element put into each tree whose class list it answers to.
    fn place(
        &self,
        trees: &[Bound<'py, PyAny>],
        element: &Bound<'py, PyAny>,
        parentage: &[Bound<'py, PyAny>],
        initial: &Bound<'py, PyAny>,
        offset: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let last = &parentage[parentage.len() - 1];
        let parent_end = end_of(last, initial)?;
        for (tree, classes) in trees.iter().zip(&self.classLists) {
            if classes.is_truthy()? && !is_of_any(element, classes)? {
                continue;
            }
            if self.kind == TreeKind::Timespans {
                let span =
                    timespan_of(element, &reversed(parentage)?, initial, &parent_end, offset)?;
                tree.call_method1("insert", (span,))?;
                continue;
            }
            tree.call_method1("insert", (offset, element))?;
        }
        Ok(())
    }
}

/// One tree per class list of a stream's elements, those of the streams it
/// holds too: music21's `listOfTreesByClass`.
#[pyfunction]
#[pyo3(signature = (
    inputStream, *, classLists = None, currentParentage = None, initialOffset = None,
    flatten = None, useTimespans = false
))]
fn listOfTreesByClass<'py>(
    inputStream: &Bound<'py, PyAny>,
    classLists: Option<&Bound<'py, PyAny>>,
    currentParentage: Option<&Bound<'py, PyAny>>,
    initialOffset: Option<&Bound<'py, PyAny>>,
    flatten: Option<&Bound<'py, PyAny>>,
    #[pyo3(from_py_with = crate::truthy)] useTimespans: bool,
) -> PyResult<Bound<'py, PyList>> {
    let py = inputStream.py();
    let classLists = match classLists {
        Some(lists) if !lists.is_none() => lists.try_iter()?.collect::<PyResult<Vec<_>>>()?,
        _ => Vec::new(),
    };
    let parentage = match currentParentage {
        Some(given) if !given.is_none() => given.try_iter()?.collect::<PyResult<Vec<_>>>()?,
        _ => vec![inputStream.clone()],
    };
    let initial = match initialOffset {
        Some(offset) => offset.clone(),
        None => PyFloat::new(py, 0.0).into_any(),
    };
    let asked = ByClass {
        classLists,
        flatten: Flattening::of(flatten)?,
        kind: match useTimespans {
            true => TreeKind::Timespans,
            false => TreeKind::Offsets,
        },
    };
    PyList::new(py, asked.trees(inputStream, &parentage, &initial)?)
}

/// Whether a stream holds a stream.
fn holds_streams(stream: &Bound<'_, PyAny>) -> PyResult<bool> {
    for element in stream.getattr("elements")?.try_iter()? {
        if is_stream(&element?) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// A stream's elements put into a tree: an `ElementTree` by `SortTuple`,
/// an `OffsetTree` with `groupOffsets`, a `TimespanTree` with
/// `useTimespans`: music21's `asTree`.
#[pyfunction]
#[pyo3(signature = (inputStream, *, flatten = None, classList = None, useTimespans = None, groupOffsets = None))]
fn asTree<'py>(
    inputStream: &Bound<'py, PyAny>,
    flatten: Option<&Bound<'py, PyAny>>,
    classList: Option<&Bound<'py, PyAny>>,
    useTimespans: Option<&Bound<'py, PyAny>>,
    groupOffsets: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    as_tree(inputStream, flatten, classList, useTimespans, groupOffsets)
}

/// [`asTree`], for `Stream.asTree`.
pub(crate) fn as_tree<'py>(
    inputStream: &Bound<'py, PyAny>,
    flatten: Option<&Bound<'py, PyAny>>,
    classList: Option<&Bound<'py, PyAny>>,
    useTimespans: Option<&Bound<'py, PyAny>>,
    groupOffsets: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = inputStream.py();
    let flag = |value: Option<&Bound<'py, PyAny>>| match value {
        Some(value) => crate::truthy(value),
        None => Ok(false),
    };
    let useTimespans = flag(useTimespans)?;
    let groupOffsets = flag(groupOffsets)?;
    let flattening = Flattening::of(flatten)?;
    if useTimespans {
        let flatten = flatten
            .cloned()
            .unwrap_or_else(|| pyo3::types::PyBool::new(py, false).to_owned().into_any());
        return as_timespans_of(inputStream, Some(&flatten), classList);
    }
    let kind = match groupOffsets {
        true => TreeKind::Offsets,
        false => TreeKind::Elements,
    };

    // A stream that is flat, or not read through, is put in from a sorted
    // list.
    if kind == TreeKind::Elements && (flattening == Flattening::No || !holds_streams(inputStream)?)
    {
        let tree = kind.empty(inputStream)?;
        return makeFastShallowTreeFromSortedStream(inputStream, &tree, classList);
    }
    let tree = kind.empty(inputStream)?;
    let walk = Walk {
        tree: &tree,
        flattening,
        classes: classes_given(classList)?,
        kind,
    };
    walk.put(inputStream, inputStream, &PyFloat::new(py, 0.0).into_any())?;
    Ok(tree)
}

/// What `asTree` puts a stream into, and how.
struct Walk<'a, 'py> {
    tree: &'a Bound<'py, PyAny>,
    flattening: Flattening,
    classes: Option<&'a Bound<'py, PyAny>>,
    kind: TreeKind,
}

impl<'py> Walk<'_, 'py> {
    /// Puts the elements of a stream, standing at `initial` in the stream
    /// read, into the tree, those of the streams it holds too.
    fn put(
        &self,
        stream: &Bound<'py, PyAny>,
        parent: &Bound<'py, PyAny>,
        initial: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py = stream.py();
        // music21 sorts the stream it is given, and takes the streams in
        // it as they are stored.
        let elements = match stream.is(&self.tree.getattr("source")?) {
            true => held(stream)?,
            false => stored(stream)?,
        };
        for (offset, element) in elements {
            let offset = op_frac(py, offset.add(initial)?.extract()?)?;
            if is_stream(&element) && self.flattening != Flattening::No {
                self.put(&element, &element, &offset)?;
                if self.flattening != Flattening::SemiFlat {
                    continue;
                }
            }
            if let Some(classes) = self.classes
                && !is_of_any(&element, classes)?
            {
                continue;
            }
            if self.kind == TreeKind::Offsets {
                self.tree.call_method1("insert", (offset, element))?;
                continue;
            }
            let keywords = PyDict::new(py);
            keywords.set_item("offset", &offset)?;
            let position = element.call_method1("sortTuple", (parent,))?.call_method(
                "modify",
                (),
                Some(&keywords),
            )?;
            self.tree.call_method1("insert", (position, element))?;
        }
        Ok(())
    }
}

/// A stream's elements as timespans in a `TimespanTree`: music21's
/// `asTimespans`. Read through, or of a stream holding no streams, it is
/// the tree `Stream.asTimespans` reads; otherwise a tree of the trees of
/// the streams held.
#[pyfunction]
#[pyo3(signature = (inputStream, *, flatten = None, classList = None))]
fn asTimespans<'py>(
    inputStream: &Bound<'py, PyAny>,
    flatten: Option<&Bound<'py, PyAny>>,
    classList: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    as_timespans_of(inputStream, flatten, classList)
}

/// [`asTimespans`], for `Stream.asTimespans`.
pub(crate) fn as_timespans_of<'py>(
    inputStream: &Bound<'py, PyAny>,
    flatten: Option<&Bound<'py, PyAny>>,
    classList: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = inputStream.py();
    let flattening = Flattening::of(flatten)?;
    if flattening != Flattening::No || !holds_streams(inputStream)? {
        let flatten = flatten
            .cloned()
            .unwrap_or_else(|| pyo3::types::PyBool::new(py, false).to_owned().into_any());
        return Ok(as_timespans(inputStream, Some(&flatten), classList)?.into_any());
    }

    // A tree of trees, as music21 builds one: everything where no classes
    // are asked for.
    let classes = match classes_given(classList)? {
        Some(classes) => classes.clone(),
        None => PyTuple::new(py, [py.get_type::<crate::base::Music21Object>()])?.into_any(),
    };
    let asked = ByClass {
        classLists: vec![classes],
        flatten: flattening,
        kind: TreeKind::Timespans,
    };
    let initial = PyFloat::new(py, 0.0).into_any();
    let mut trees = asked.trees(inputStream, std::slice::from_ref(inputStream), &initial)?;
    Ok(trees.remove(0))
}

/// music21's `tree.fromStream`, built whole.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let module = PyModule::new(py, "fromStream")?;
    module.add_function(wrap_pyfunction!(listOfTreesByClass, &module)?)?;
    module.add_function(wrap_pyfunction!(asTree, &module)?)?;
    module.add_function(wrap_pyfunction!(
        makeFastShallowTreeFromSortedStream,
        &module
    )?)?;
    module.add_function(wrap_pyfunction!(asTimespans, &module)?)?;
    for name in [
        "listOfTreesByClass",
        "asTree",
        "makeFastShallowTreeFromSortedStream",
        "asTimespans",
    ] {
        module
            .getattr(name)?
            .setattr("__module__", "music21.tree.fromStream")?;
    }
    Ok(module)
}
