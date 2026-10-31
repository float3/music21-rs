//! music21's `stream.makeNotation`: the notation a score leaves unsaid,
//! worked out.
//!
//! The steps are the crate's `makenotation`, which
//! `musicxml_make_notation_parity` holds to music21 byte for byte on 257
//! subjects. What the facade adds is that each answer is written onto
//! music21's **own** objects: the step is run over a crate stream read
//! from the one it was given, and what it decided is carried back by
//! position onto the notes at the same places. Nothing of the stream is
//! rebuilt, so a caller holding a note still holds it afterwards, as it
//! does through music21.
//!
//! **music21 makes the copy, not this package.** Every one of these takes
//! `inPlace`, and with it false music21 begins by copying the stream with
//! its own `coreCopyAsDerivation` -- so the facade asks music21 for that
//! copy and writes onto it. A stream of this package's own, with no
//! music21 beside it, is deep-copied instead.
//!
//! **Only the steps that write onto what they are given are here.**
//! `makeBeams` and `makeTupletBrackets` decide something about each note
//! and write it there, and `getTiePitchSet` and `iterateBeamGroups` only
//! read. `makeMeasures`, `makeRests`, `makeTies`, `realizeOrnaments`,
//! `splitElementsToCompleteTuplets` and `consolidateCompletedTuplets`
//! change *which elements there are* -- they put notes in new measures,
//! fill with new rests, cut a note into tied pieces -- so a position no
//! longer names the same thing on both sides, and each wants a
//! by-position twin in the crate saying what comes to be where. That is
//! the next unit of work in this module, not something to fake here.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::IntoPyObjectExt;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyString};

use music21_rs_crate::StreamElement;
use music21_rs_crate::duration::TupletBracket;
use music21_rs_crate::makenotation::{
    make_beams, make_tuplet_brackets, set_stem_direction_for_beam_groups, tie_pitch_set,
};
use music21_rs_crate::notation::Beams as RsBeams;

use crate::Walkable;

/// The names this module hands out.
pub const NAMES: &[&str] = &[
    "makeBeams",
    "makeTupletBrackets",
    "getTiePitchSet",
    "iterateBeamGroups",
    "setStemDirectionForBeamGroups",
];

pyo3::create_exception!(
    music21_rs_facade,
    MakeNotationException,
    crate::Music21Exception
);

error_into!(pub(crate) notation_error, MakeNotationException);

/// A generator of what the crate answered, so that a caller sees what
/// music21's own generator is: music21's `iterateBeamGroups` yields, and
/// its own examples iterate it.
const GENERATOR: &str = r#"
def build(groups):
    """The groups, yielded one at a time."""
    def iterateBeamGroups(s, skipNoBeams=True, recurse=True):
        yield from groups(s, skipNoBeams, recurse)
    iterateBeamGroups.__qualname__ = 'iterateBeamGroups'
    return iterateBeamGroups
"#;

/// The stream the step is run on: the one it was given where the caller
/// asked for that, and music21's own copy of it otherwise.
///
/// music21 copies with `coreCopyAsDerivation`, which records what the copy
/// was made by; a stream of this package's own is deep-copied, there being
/// no derivation to record.
fn target<'py>(s: &Bound<'py, PyAny>, inPlace: bool, method: &str) -> PyResult<Bound<'py, PyAny>> {
    if inPlace {
        return Ok(s.clone());
    }
    if let Ok(copied) = s.call_method1("coreCopyAsDerivation", (method,)) {
        return Ok(copied);
    }
    s.py()
        .import("copy")?
        .call_method1("deepcopy", (s,))
        .map(|copied| copied.into_any())
}

/// What the crate decided about each leaf, written onto the object at the
/// same place.
fn carry<'py, F>(target: &Bound<'py, PyAny>, run: F) -> PyResult<()>
where
    F: FnOnce(&mut music21_rs_crate::Stream) -> PyResult<()>,
{
    let py = target.py();
    let (mut read, objects) = crate::stream::read_with_leaves(target)?;
    run(&mut read)?;
    for (at, (_, element)) in read.leaves().iter().enumerate() {
        let Some(object) = objects.get(at) else {
            break;
        };
        written(py, element, object.bind(py))?;
    }
    Ok(())
}

/// One leaf's beams, stem direction and tuplet brackets written onto the
/// object standing in its place.
fn written(py: Python<'_>, element: &StreamElement, object: &Bound<'_, PyAny>) -> PyResult<()> {
    let note = match element {
        StreamElement::Note(note) => Some(note),
        _ => None,
    };
    if let Some(note) = note {
        if object.hasattr("beams")? {
            object.setattr("beams", beams_object(py, note.beams().clone())?)?;
        }
        if object.hasattr("stemDirection")? {
            object.setattr("stemDirection", note.stem_direction().as_str())?;
        }
    }
    tuplets_written(element, object)
}

/// The bracket each of a leaf's tuplets is drawn with, written onto the
/// tuplets music21 keeps on the object's duration.
fn tuplets_written(element: &StreamElement, object: &Bound<'_, PyAny>) -> PyResult<()> {
    let Some(duration) = element.duration() else {
        return Ok(());
    };
    let tuplets = duration.tuplets();
    if tuplets.is_empty() {
        return Ok(());
    }
    let Ok(held) = object
        .getattr("duration")
        .and_then(|held| held.getattr("tuplets"))
    else {
        return Ok(());
    };
    for (at, tuplet) in tuplets.iter().enumerate() {
        let Ok(theirs) = held.get_item(at) else {
            break;
        };
        theirs.setattr("type", tuplet.tuplet_type().map(|kind| kind.as_str()))?;
        // music21 writes a bracket as `True`, `False` or `'slur'`.
        let bracket = match tuplet.bracket() {
            TupletBracket::Bracket => true.into_bound_py_any(theirs.py())?,
            TupletBracket::None => false.into_bound_py_any(theirs.py())?,
            TupletBracket::Slur => "slur".into_bound_py_any(theirs.py())?,
        };
        theirs.setattr("bracket", bracket)?;
    }
    Ok(())
}

/// The beams of one note, as music21 holds them.
fn beams_object(py: Python<'_>, beams: RsBeams) -> PyResult<Py<PyAny>> {
    Ok(crate::installed_new(
        py,
        "music21.beam",
        "Beams",
        crate::notation::Beams::wrap(beams),
    )?
    .into_any())
}

/// music21's `makeBeams`: every note beamed by the meter in force, and the
/// stems of each beamed group pointed one way.
#[pyfunction]
#[pyo3(name = "makeBeams")]
#[pyo3(signature = (s, *, inPlace = false, setStemDirections = true, failOnNoTimeSignature = false))]
pub fn make_beams_py<'py>(
    s: &Bound<'py, PyAny>,
    #[pyo3(from_py_with = crate::truthy)] inPlace: bool,
    #[pyo3(from_py_with = crate::truthy)] setStemDirections: bool,
    #[pyo3(from_py_with = crate::truthy)] failOnNoTimeSignature: bool,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let _ = failOnNoTimeSignature;
    let made = target(s, inPlace, "makeBeams")?;
    let py = made.py();
    let (mut read, objects) = crate::stream::read_with_leaves(&made)?;
    make_beams(&mut read).map_err(notation_error)?;
    for (at, (_, element)) in read.leaves().iter().enumerate() {
        let Some(object) = objects.get(at) else {
            break;
        };
        let object = object.bind(py);
        let StreamElement::Note(note) = element else {
            continue;
        };
        if object.hasattr("beams")? {
            object.setattr("beams", beams_object(py, note.beams().clone())?)?;
        }
        // The crate points the stems as it beams; music21 asks for that
        // separately, so what it did not ask for is not carried back.
        if setStemDirections && object.hasattr("stemDirection")? {
            object.setattr("stemDirection", note.stem_direction().as_str())?;
        }
    }
    Ok((!inPlace).then_some(made))
}

/// music21's `makeTupletBrackets`: each tuplet of a stream drawn with the
/// bracket its place in its run asks for.
#[pyfunction]
#[pyo3(name = "makeTupletBrackets")]
#[pyo3(signature = (s, *, inPlace = false))]
pub fn make_tuplet_brackets_py<'py>(
    s: &Bound<'py, PyAny>,
    #[pyo3(from_py_with = crate::truthy)] inPlace: bool,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let made = target(s, inPlace, "makeTupletBrackets")?;
    carry(&made, |read| {
        make_tuplet_brackets(read);
        Ok(())
    })?;
    Ok((!inPlace).then_some(made))
}

/// music21's `setStemDirectionForBeamGroups`: the stems of each beamed
/// group pointed one way.
#[pyfunction]
#[pyo3(name = "setStemDirectionForBeamGroups")]
#[pyo3(signature = (s, *, setNewStems = true, overrideConsistentStemDirections = false))]
pub fn set_stem_direction_py(
    s: &Bound<'_, PyAny>,
    #[pyo3(from_py_with = crate::truthy)] setNewStems: bool,
    #[pyo3(from_py_with = crate::truthy)] overrideConsistentStemDirections: bool,
) -> PyResult<()> {
    if !setNewStems || overrideConsistentStemDirections {
        return Err(MakeNotationException::new_err(
            "the crate points the stems of a beamed group one way and says nothing of \
             a group it has not beamed, so setNewStems=False and \
             overrideConsistentStemDirections=True are refused rather than answered \
             differently",
        ));
    }
    carry(s, |read| {
        set_stem_direction_for_beam_groups(read);
        Ok(())
    })
}

/// music21's `getTiePitchSet`: the notes a note or chord ties forward.
#[pyfunction]
#[pyo3(name = "getTiePitchSet")]
pub fn get_tie_pitch_set_py<'py>(
    py: Python<'py>,
    prior: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let Some(element) = crate::stream::element_written(prior)? else {
        return Ok(None);
    };
    let Some(names) = tie_pitch_set(&element) else {
        return Ok(None);
    };
    let out = py.import("builtins")?.getattr("set")?.call0()?;
    for name in names {
        out.call_method1("add", (PyString::new(py, &name),))?;
    }
    Ok(Some(out))
}

/// The groups of notes a stream's beams make, each as the notes
/// themselves: the list music21's `iterateBeamGroups` yields.
///
/// A group runs from the note whose first beam starts to the one whose
/// first beam stops, which is what a caller reads off the beams each note
/// carries; where the caller does not skip them, a note with no beams is
/// a group of its own.
fn beam_groups<'py>(
    py: Python<'py>,
    s: &Bound<'py, PyAny>,
    skipNoBeams: bool,
    recurse: bool,
) -> PyResult<Bound<'py, PyList>> {
    let walked = if recurse {
        s.call_method0("recurse")?
    } else {
        s.call_method0("iter")?
    };
    let out = PyList::empty(py);
    let mut current = PyList::empty(py);
    let mut inside = false;
    for note in walked.getattr("notes")?.walk()? {
        let note = note?;
        let beams = note.getattr("beams")?;
        let first = if beams.is_truthy()? && beams.call_method1("getByNumber", (1,))?.is_truthy()? {
            Some(
                beams
                    .call_method1("getTypeByNumber", (1,))?
                    .str()?
                    .to_str()?
                    .to_string(),
            )
        } else {
            None
        };
        if first.as_deref() == Some("start") {
            inside = true;
        }
        if inside {
            current.append(&note)?;
        }
        if first.as_deref() == Some("stop") {
            out.append(&current)?;
            current = PyList::empty(py);
            inside = false;
        } else if !skipNoBeams && !inside {
            out.append(PyList::new(py, [note])?)?;
        }
    }
    if !current.is_empty() {
        out.append(current)?;
    }
    Ok(out)
}

/// The generator music21's `iterateBeamGroups` is, built once.
static GROUPS: pyo3::sync::PyOnceLock<Py<PyAny>> = pyo3::sync::PyOnceLock::new();

/// music21's `iterateBeamGroups`, which is a generator: the groups its own
/// examples print come out of one, so one is what the facade hands back.
pub(crate) fn iterate_beam_groups(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let built = GROUPS.get_or_try_init(py, || -> PyResult<Py<PyAny>> {
        let module = PyModule::from_code(
            py,
            &std::ffi::CString::new(GENERATOR)?,
            c"music21_rs_beam_groups.py",
            c"music21_rs_beam_groups",
        )?;
        let groups = wrap_pyfunction!(beam_groups_py, &module)?;
        Ok(module.getattr("build")?.call1((groups,))?.unbind())
    })?;
    Ok(built.bind(py).clone())
}

/// The groups as a list, which the generator above yields from.
#[pyfunction]
#[pyo3(name = "_beamGroups")]
fn beam_groups_py<'py>(
    py: Python<'py>,
    s: &Bound<'py, PyAny>,
    #[pyo3(from_py_with = crate::truthy)] skipNoBeams: bool,
    #[pyo3(from_py_with = crate::truthy)] recurse: bool,
) -> PyResult<Bound<'py, PyList>> {
    beam_groups(py, s, skipNoBeams, recurse)
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(make_beams_py, m)?)?;
    m.add_function(wrap_pyfunction!(make_tuplet_brackets_py, m)?)?;
    m.add_function(wrap_pyfunction!(set_stem_direction_py, m)?)?;
    m.add_function(wrap_pyfunction!(get_tie_pitch_set_py, m)?)?;
    m.add("iterateBeamGroups", iterate_beam_groups(m.py())?)?;
    Ok(())
}
