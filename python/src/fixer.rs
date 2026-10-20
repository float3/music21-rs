//! music21's `alpha.analysis.fixer`: a score read by optical music
//! recognition mended against a performance of it.
//!
//! `EnharmonicFixer` is the one class here, and the respelling is the
//! crate's `alpha::analysis::fixer::enharmonic_fix` -- music21's five
//! cases for one change, which `alpha_parity` holds to music21 and the
//! crate's own tests hold to the answers its examples print. The facade
//! reads the two notes and hands the write back.
//!
//! **A change here names two note objects and no streams.** music21's own
//! examples build the changes by hand -- `(midiNote, omrNote,
//! ChangeOps.Substitution)` -- and hand the fixer `None` for both
//! streams, so the fix is over the two notes and nothing else. That is why
//! the crate's step takes two pitches rather than two streams and a pair
//! of positions, and it is the whole reason this class could be swapped.
//!
//! **A natural is written and says nothing.** music21 spells a natural
//! with the empty string, so `nameWithOctave` loses the one thing most of
//! these cases turn on -- whether the reading carries a natural sign --
//! and `pitch_read` therefore takes the accidental off the pitch object
//! as well as the name.
//!
//! **music21 writes in two ways and the crate says which.** Its first
//! four cases set the accidental on the pitch the note already has, and
//! its fifth hands the note the pitch played whole; `EnharmonicFix` is
//! that pair, so a caller holding the reading's pitch object sees what
//! music21 would have done to it.
//!
//! `DeleteFixer`, `OrnamentFixer`, `TrillFixer` and `TurnFixer` stay
//! music21's. Each reads its changes as *elements of two streams* --
//! `DeleteFixer` asks the note read for the measure holding it, and the
//! ornament fixers walk both readings for the notes within a length --
//! where the crate's `delete_measures` and `OrnamentFixer` take positions
//! in two crate streams. Swapping them means reading both streams in with
//! their objects and matching each change against them, and no example of
//! music21's would say whether that was done right; music21's own test
//! suite runs those three either way.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;

use music21_rs_crate::IntegerType;
use music21_rs_crate::alpha::analysis::aligner::ChangeOp;
use music21_rs_crate::alpha::analysis::fixer::{EnharmonicFix, enharmonic_fix, interval_too_big};
use music21_rs_crate::pitch::{Accidental, Pitch as RsPitch};

/// The names this module hands out.
pub const NAMES: &[&str] = &["EnharmonicFixer"];

/// Whether an object answers to a music21 class by name, which is how a
/// note is told from a rest or a chord.
fn answers_to(object: &Bound<'_, PyAny>, name: &str) -> bool {
    object
        .getattr("classes")
        .and_then(|classes| classes.contains(name))
        .unwrap_or(false)
}

/// A music21 note's pitch read as the crate holds one, the accidental it
/// is *written* with included.
///
/// music21 spells a natural with the empty string, so a name alone says
/// nothing about whether the reading carries a natural sign -- which is
/// what music21's first two cases turn on.
fn pitch_read(object: &Bound<'_, PyAny>) -> PyResult<RsPitch> {
    let pitch = object.getattr("pitch")?;
    let mut read = crate::pitch::pitch_from_any(&pitch)?;
    let accidental = pitch.getattr("accidental")?;
    if accidental.is_none() {
        read.set_written_accidental(None);
    } else {
        let name: String = accidental.getattr("name")?.extract()?;
        read.set_written_accidental(Some(
            Accidental::new(name.as_str()).map_err(crate::pitch::accidental_error)?,
        ));
    }
    Ok(read)
}

/// What the alignment made of a pair, as the crate holds it.
///
/// music21 reads a change as no change only where it really is one of its
/// own `ChangeOps`; anything else -- a bare number, say -- is read as a
/// change, and the facade must be as particular.
fn op_read(py: Python<'_>, op: &Bound<'_, PyAny>) -> PyResult<ChangeOp> {
    let class = crate::aligner::change_ops(py)?;
    let ours = class
        .cast::<pyo3::types::PyType>()
        .ok()
        .is_some_and(|class| op.is_instance(class).unwrap_or(false));
    if ours && op.eq(3_i64)? {
        return Ok(ChangeOp::NoChange);
    }
    Ok(ChangeOp::Substitution)
}

/// music21's `alpha.analysis.fixer.EnharmonicFixer`: where the two
/// readings spell one sound differently, the performance's spelling wins.
#[pyclass(
    name = "EnharmonicFixer",
    module = "music21.alpha.analysis.fixer",
    subclass,
    skip_from_py_object
)]
pub struct EnharmonicFixer {
    /// The changes an alignment found, each a note played, a note read and
    /// what became of it.
    #[pyo3(get, set)]
    changes: Py<PyAny>,
    /// The performance.
    #[pyo3(get, set)]
    midiStream: Py<PyAny>,
    /// The reading.
    #[pyo3(get, set)]
    omrStream: Py<PyAny>,
}

#[pymethods]
impl EnharmonicFixer {
    /// music21's `EnharmonicFixer(changes, midiStream, omrStream)`.
    #[new]
    #[pyo3(signature = (changes = None, midiStream = None, omrStream = None))]
    fn new(
        py: Python<'_>,
        changes: Option<Py<PyAny>>,
        midiStream: Option<Py<PyAny>>,
        omrStream: Option<Py<PyAny>>,
    ) -> Self {
        Self {
            changes: changes.unwrap_or_else(|| py.None()),
            midiStream: midiStream.unwrap_or_else(|| py.None()),
            omrStream: omrStream.unwrap_or_else(|| py.None()),
        }
    }

    /// Whether both references are notes, a rest or a chord being nothing
    /// to respell: music21's `checkIfNoteInstance`.
    fn checkIfNoteInstance(&self, midiRef: &Bound<'_, PyAny>, omrRef: &Bound<'_, PyAny>) -> bool {
        answers_to(midiRef, "Note") && answers_to(omrRef, "Note")
    }

    /// Whether the two notes sound alike: music21's `isEnharmonic`.
    fn isEnharmonic(
        &self,
        midiRef: &Bound<'_, PyAny>,
        omrRef: &Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        Ok(pitch_read(midiRef)?.is_enharmonic(&pitch_read(omrRef)?))
    }

    /// Whether the reading carries an accidental at all: music21's
    /// `hasAcc`.
    fn hasAcc(&self, omrRef: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(pitch_read(omrRef)?.written_accidental().is_some())
    }

    /// Whether it carries a natural: music21's `hasNatAcc`.
    fn hasNatAcc(&self, omrRef: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(pitch_read(omrRef)?
            .written_accidental()
            .is_some_and(|accidental| accidental.name() == "natural"))
    }

    /// Whether it carries a sharp or a flat: music21's `hasSharpFlatAcc`.
    fn hasSharpFlatAcc(&self, omrRef: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(pitch_read(omrRef)?
            .written_accidental()
            .is_some_and(|accidental| accidental.name() != "natural"))
    }

    /// Whether the two stand on the same letter: music21's `stepEq`, which
    /// is the one comparison it makes off the objects themselves.
    fn stepEq(&self, midiRef: &Bound<'_, PyAny>, omrRef: &Bound<'_, PyAny>) -> PyResult<bool> {
        midiRef.getattr("step")?.eq(omrRef.getattr("step")?)
    }

    /// Whether they do not: music21's `stepNotEq`.
    fn stepNotEq(&self, midiRef: &Bound<'_, PyAny>, omrRef: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(!self.stepEq(midiRef, omrRef)?)
    }

    /// Whether the two are too far apart to be two spellings of one
    /// sound: music21's `intervalTooBig`, which reads the interval class
    /// and not the interval.
    #[pyo3(signature = (midiRef, omrRef, setInt = 5))]
    fn intervalTooBig(
        &self,
        midiRef: &Bound<'_, PyAny>,
        omrRef: &Bound<'_, PyAny>,
        setInt: IntegerType,
    ) -> PyResult<bool> {
        interval_too_big(&pitch_read(midiRef)?, &pitch_read(omrRef)?, setInt)
            .map_err(crate::interval::interval_error)
    }

    /// Nothing, as music21's base class does: its `OMRMidiFixer.fix`.
    ///
    /// Every fixer calls it first, and `EnharmonicFixer` is no exception.
    fn fix(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let changes = slf.borrow().changes.clone_ref(py);
        let changes = changes.bind(py);
        if changes.is_none() {
            return Ok(());
        }
        for change in changes.try_iter()? {
            let change = change?;
            let midiRef = change.get_item(0)?;
            let omrRef = change.get_item(1)?;
            let op = change.get_item(2)?;
            omrRef.getattr("style")?.setattr("color", "black")?;
            if !slf.borrow().checkIfNoteInstance(&midiRef, &omrRef) {
                continue;
            }
            let played = pitch_read(&midiRef)?;
            let read = pitch_read(&omrRef)?;
            let Some(fix) = enharmonic_fix(&played, &read, op_read(py, &op)?)
                .map_err(crate::interval::interval_error)?
            else {
                continue;
            };
            match fix {
                // music21 writes the accidental onto the pitch the note
                // already has, so a caller holding that pitch sees it.
                EnharmonicFix::Accidental(accidental) => {
                    let written = accidental.map(|accidental| accidental.name().to_string());
                    omrRef.getattr("pitch")?.setattr("accidental", written)?;
                }
                // And in its fifth case hands the note the pitch played.
                EnharmonicFix::Played => {
                    omrRef.setattr("pitch", midiRef.getattr("pitch")?)?;
                }
            }
        }
        Ok(())
    }

    /// A copy holding what this one holds.
    fn __copy__(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        Self::copied(slf, None)
    }

    /// The same with everything held copied too, through the copier's
    /// memo.
    #[pyo3(signature = (memo = None))]
    fn __deepcopy__(slf: &Bound<'_, Self>, memo: Option<&Bound<'_, PyAny>>) -> PyResult<Py<PyAny>> {
        Self::copied(slf, Some(memo))
    }

    /// What a pickle writes: the class and the parts, as every facade here
    /// pickles itself rather than leaving it to music21.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let made = pyo3::types::PyTuple::new(
            py,
            [
                slf.get_type().into_any(),
                pyo3::types::PyTuple::empty(py).into_any(),
                Self::state(slf)?.into_any(),
            ],
        )?;
        Ok(made.into_any().unbind())
    }

    /// The parts read back, music21's own dictionary state taken as
    /// readily as the one written here.
    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let read = |name: &str| state.get_item(name).ok();
        let mut me = slf.borrow_mut();
        if let Some(value) = read("changes") {
            me.changes = value.unbind();
        }
        if let Some(value) = read("midiStream") {
            me.midiStream = value.unbind();
        }
        if let Some(value) = read("omrStream") {
            me.omrStream = value.unbind();
        }
        Ok(())
    }
}

impl EnharmonicFixer {
    /// The parts a copy or a pickle carries.
    fn state<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, pyo3::types::PyDict>> {
        let py = slf.py();
        let me = slf.borrow();
        let state = pyo3::types::PyDict::new(py);
        state.set_item("changes", me.changes.bind(py))?;
        state.set_item("midiStream", me.midiStream.bind(py))?;
        state.set_item("omrStream", me.omrStream.bind(py))?;
        Ok(state)
    }

    /// A copy of the fixer, everything it holds copied too where a deep
    /// copy asks for it.
    fn copied(
        slf: &Bound<'_, Self>,
        memo: Option<Option<&Bound<'_, PyAny>>>,
    ) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let carry = |held: &Py<PyAny>| -> PyResult<Py<PyAny>> {
            let Some(memo) = memo else {
                return Ok(held.clone_ref(py));
            };
            Ok(py
                .import("copy")?
                .call_method1("deepcopy", (held.bind(py), memo))?
                .unbind())
        };
        let value = {
            let me = slf.borrow();
            Self {
                changes: carry(&me.changes)?,
                midiStream: carry(&me.midiStream)?,
                omrStream: carry(&me.omrStream)?,
            }
        };
        Ok(crate::copy_as_same_type(slf, value)?.unbind())
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<EnharmonicFixer>()?;
    Ok(())
}
