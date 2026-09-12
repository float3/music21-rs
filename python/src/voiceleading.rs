//! music21's `voiceLeading.VoiceLeadingQuartet`, over the crate's own.
//!
//! Four notes: two voices each moving from one note to the next, at the same
//! time. What the pair does — parallel fifths, a voice crossing, a proper
//! resolution — is what the class answers.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};

use music21_rs_crate::voiceleading::{
    ParallelRequirement as RsParallel, VoiceLeadingQuartet as RsQuartet,
};
use music21_rs_crate::{Interval as RsInterval, Key as RsKey, Note as RsNote, Pitch as RsPitch};

use crate::note::Note;
use crate::pitch::pitch_from_any;

/// The names the `voiceleading` facade replaces in `music21.voiceLeading`.
pub const NAMES: &[&str] = &["VoiceLeadingQuartet", "VoiceLeadingQuartetException"];

pyo3::create_exception!(
    music21_rs_facade,
    VoiceLeadingQuartetException,
    crate::Music21Exception
);

error_into!(quartet_error, VoiceLeadingQuartetException);

/// One of the four notes, as music21 takes it: a note, a pitch, or a name.
///
/// A pitch becomes a note of no length, as music21's own setter makes one.
fn voice_note(py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Py<Note>> {
    if let Ok(note) = value.extract::<Py<Note>>() {
        return Ok(note);
    }
    let pitch = pitch_from_any(value).map_err(|_| {
        VoiceLeadingQuartetException::new_err(format!("not a valid note specification: {value}"))
    })?;
    Note::object(py, RsNote::from_pitch(pitch))
}

/// The key a quartet is heard in, as music21 takes it.
fn analytic_key(value: &Bound<'_, PyAny>) -> PyResult<RsKey> {
    if let Ok(name) = value.extract::<String>() {
        return RsKey::from_tonic(&name).map_err(|_| {
            VoiceLeadingQuartetException::new_err(format!(
                "got a key signature string that is not supported: {name}"
            ))
        });
    }
    if let Ok(key) = value.extract::<PyRef<'_, crate::key::Key>>() {
        return Ok(key.inner.clone());
    }
    let tonic: String = value.getattr("tonic")?.getattr("name")?.extract()?;
    let mode: Option<String> = value
        .getattr("mode")
        .ok()
        .and_then(|mode| mode.extract().ok());
    RsKey::from_tonic_mode(&tonic, mode.as_deref()).map_err(quartet_error)
}

/// music21's `voiceLeading.VoiceLeadingQuartet`.
#[pyclass(
    name = "VoiceLeadingQuartet",
    module = "music21.voiceLeading",
    subclass,
    skip_from_py_object
)]
pub struct VoiceLeadingQuartet {
    pub(crate) inner: RsQuartet,
    /// The four notes as objects, so `vlq.v1n1` is a real note and the same
    /// one every time.
    notes: Vec<Py<Note>>,
    /// The key as the object it was given as, since music21 hands back what
    /// it was handed.
    key_object: Option<Py<PyAny>>,
}

impl VoiceLeadingQuartet {
    /// Builds one from four note objects and an optional key.
    fn assemble(
        py: Python<'_>,
        notes: Vec<Py<Note>>,
        key: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let pitches: Vec<RsPitch> = notes
            .iter()
            .map(|note| note.borrow(py).synced(py).pitch().clone())
            .collect();
        let mut inner = RsQuartet::new(
            pitches[0].clone(),
            pitches[1].clone(),
            pitches[2].clone(),
            pitches[3].clone(),
        )
        .map_err(quartet_error)?;
        let mut key_object = None;
        if let Some(value) = key.filter(|value| !value.is_none()) {
            inner = inner.with_key(analytic_key(value)?);
            key_object = Some(value.clone().unbind());
        }
        Ok(Self {
            inner,
            notes,
            key_object,
        })
    }

    fn build(
        py: Python<'_>,
        v1n1: &Bound<'_, PyAny>,
        v1n2: &Bound<'_, PyAny>,
        v2n1: &Bound<'_, PyAny>,
        v2n2: &Bound<'_, PyAny>,
        key: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        // `analyticKey` is accepted as a keyword synonym for `key`.
        let named = match keywords {
            Some(keywords) => keywords.get_item("analyticKey")?,
            None => None,
        };
        let key = match &named {
            Some(value) if !value.is_none() => Some(value.as_borrowed().to_owned()),
            _ => key.cloned(),
        };
        let notes = vec![
            voice_note(py, v1n1)?,
            voice_note(py, v1n2)?,
            voice_note(py, v2n1)?,
            voice_note(py, v2n2)?,
        ];
        Self::assemble(py, notes, key.as_ref())
    }

    /// Reads the four notes again, after one of them has been replaced.
    fn refresh(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<()> {
        let (notes, key) = {
            let me = slf.borrow();
            (
                me.notes.iter().map(|note| note.clone_ref(py)).collect(),
                me.key_object.as_ref().map(|key| key.clone_ref(py)),
            )
        };
        let rebuilt = Self::assemble(py, notes, key.as_ref().map(|key| key.bind(py)))?;
        let mut me = slf.borrow_mut();
        me.inner = rebuilt.inner;
        Ok(())
    }

    /// The same quartet again, sharing its notes as music21's copy does.
    fn copied(&self, py: Python<'_>) -> Self {
        Self {
            inner: self.inner.clone(),
            notes: self.notes.iter().map(|note| note.clone_ref(py)).collect(),
            key_object: self.key_object.as_ref().map(|key| key.clone_ref(py)),
        }
    }

    /// One of the two intervals, as an object carrying the notes it spans.
    fn interval_object(
        py: Python<'_>,
        inner: &RsInterval,
        start: &Py<Note>,
        end: &Py<Note>,
        harmonic: bool,
    ) -> PyResult<Py<PyAny>> {
        let spanning = crate::interval::Interval::between(
            inner.clone(),
            start.clone_ref(py).into_any(),
            end.clone_ref(py).into_any(),
            if harmonic { "harmonic" } else { "melodic" },
        );
        Ok(crate::installed_new(py, "music21.interval", "Interval", spanning)?.into_any())
    }
}

#[pymethods]
impl VoiceLeadingQuartet {
    /// The Python objects this holds, shown to the cycle collector. A note
    /// and the pitch it hands out point at each other through Rust, and
    /// without this neither of them is ever freed.
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        for held in &self.notes {
            visit.call(held)?;
        }
        visit.call(&self.key_object)?;
        Ok(())
    }

    fn __clear__(&mut self) {
        self.notes.clear();
        self.key_object = None;
    }

    /// A quartet is written out as text and read back, and its four notes
    /// are made again from what it says.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        state: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let Some(inner) = crate::unpickled::<_, RsQuartet>(slf, state)? else {
            return Ok(());
        };
        let notes = [inner.v1n1(), inner.v1n2(), inner.v2n1(), inner.v2n2()]
            .into_iter()
            .map(|pitch| Note::object(py, RsNote::from_pitch(pitch.clone())))
            .collect::<PyResult<Vec<_>>>()?;
        let mut me = slf.borrow_mut();
        me.inner = inner;
        me.notes = notes;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (v1n1, v1n2, v2n1, v2n2, key = None, **keywords))]
    fn new(
        py: Python<'_>,
        v1n1: &Bound<'_, PyAny>,
        v1n2: &Bound<'_, PyAny>,
        v2n1: &Bound<'_, PyAny>,
        v2n2: &Bound<'_, PyAny>,
        key: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        Self::build(py, v1n1, v1n2, v2n1, v2n2, key, keywords)
    }

    /// Built again here, since a Python subclass hands `__new__` its own
    /// arguments and only then calls `super().__init__` with music21's.
    #[pyo3(signature = (v1n1, v1n2, v2n1, v2n2, key = None, **keywords))]
    fn __init__(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        v1n1: &Bound<'_, PyAny>,
        v1n2: &Bound<'_, PyAny>,
        v2n1: &Bound<'_, PyAny>,
        v2n2: &Bound<'_, PyAny>,
        key: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let built = Self::build(py, v1n1, v1n2, v2n1, v2n2, key, keywords)?;
        let mut me = slf.borrow_mut();
        me.inner = built.inner;
        me.notes = built.notes;
        me.key_object = built.key_object;
        Ok(())
    }

    #[getter]
    fn get_v1n1(&self, py: Python<'_>) -> Py<Note> {
        self.notes[0].clone_ref(py)
    }

    #[setter]
    fn set_v1n1(slf: &Bound<'_, Self>, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        slf.borrow_mut().notes[0] = voice_note(py, value)?;
        Self::refresh(slf, py)
    }

    #[getter]
    fn get_v1n2(&self, py: Python<'_>) -> Py<Note> {
        self.notes[1].clone_ref(py)
    }

    #[setter]
    fn set_v1n2(slf: &Bound<'_, Self>, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        slf.borrow_mut().notes[1] = voice_note(py, value)?;
        Self::refresh(slf, py)
    }

    #[getter]
    fn get_v2n1(&self, py: Python<'_>) -> Py<Note> {
        self.notes[2].clone_ref(py)
    }

    #[setter]
    fn set_v2n1(slf: &Bound<'_, Self>, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        slf.borrow_mut().notes[2] = voice_note(py, value)?;
        Self::refresh(slf, py)
    }

    #[getter]
    fn get_v2n2(&self, py: Python<'_>) -> Py<Note> {
        self.notes[3].clone_ref(py)
    }

    #[setter]
    fn set_v2n2(slf: &Bound<'_, Self>, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        slf.borrow_mut().notes[3] = voice_note(py, value)?;
        Self::refresh(slf, py)
    }

    /// music21's `key`: the key the progression is heard in, which the
    /// counterpoint rules consult.
    #[getter]
    fn get_key(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let Some(key) = &self.key_object else {
            return Ok(py.None());
        };
        Ok(key.clone_ref(py))
    }

    #[setter]
    fn set_key(slf: &Bound<'_, Self>, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        if value.is_none() {
            let mut me = slf.borrow_mut();
            me.key_object = None;
            me.inner.set_key(None);
            return Ok(());
        }
        let key = analytic_key(value)?;
        // A key given as a string is handed back as the key it names.
        let stored = match value.extract::<String>() {
            Ok(_) => crate::key::Key::object(py, key.clone())?,
            Err(_) => value.clone().unbind(),
        };
        let mut me = slf.borrow_mut();
        me.key_object = Some(stored);
        me.inner.set_key(Some(key));
        Ok(())
    }

    /// music21's `vIntervals`: the two harmonic intervals, first to first and
    /// second to second.
    #[getter]
    fn vIntervals<'py>(slf: &Bound<'py, Self>, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let me = slf.borrow();
        let intervals = me.inner.vertical_intervals();
        let first = Self::interval_object(py, &intervals[0], &me.notes[0], &me.notes[2], true)?;
        let second = Self::interval_object(py, &intervals[1], &me.notes[1], &me.notes[3], true)?;
        PyTuple::new(py, [first, second])
    }

    /// music21's `hIntervals`: the two melodic intervals, one per voice.
    #[getter]
    fn hIntervals<'py>(slf: &Bound<'py, Self>, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let me = slf.borrow();
        let intervals = me.inner.horizontal_intervals();
        let first = Self::interval_object(py, &intervals[0], &me.notes[0], &me.notes[1], false)?;
        let second = Self::interval_object(py, &intervals[1], &me.notes[2], &me.notes[3], false)?;
        PyTuple::new(py, [first, second])
    }

    /// music21's `motionType`: what the two voices do together.
    #[pyo3(signature = (*, allowAntiParallel = false))]
    fn motionType(&self, py: Python<'_>, allowAntiParallel: bool) -> PyResult<Py<PyAny>> {
        let motion = self.inner.motion_type(allowAntiParallel);
        Ok(py
            .import("music21.voiceLeading")?
            .getattr("MotionType")?
            .call1((motion.as_str(),))?
            .unbind())
    }

    fn noMotion(&self) -> bool {
        self.inner.no_motion()
    }

    fn obliqueMotion(&self) -> bool {
        self.inner.oblique_motion()
    }

    fn similarMotion(&self) -> bool {
        self.inner.similar_motion()
    }

    /// music21's `parallelMotion`: the voices move the same way through the
    /// same generic interval, or through one named interval.
    #[pyo3(signature = (requiredInterval = None, allowOctaveDisplacement = false))]
    fn parallelMotion(
        &self,
        requiredInterval: Option<&Bound<'_, PyAny>>,
        allowOctaveDisplacement: bool,
    ) -> PyResult<bool> {
        let required = match requiredInterval.filter(|value| !value.is_none()) {
            Some(value) => Some(parallel_requirement(value)?),
            None => None,
        };
        Ok(self
            .inner
            .parallel_motion(required.as_ref(), allowOctaveDisplacement))
    }

    fn contraryMotion(&self) -> bool {
        self.inner.contrary_motion()
    }

    fn outwardContraryMotion(&self) -> bool {
        self.inner.outward_contrary_motion()
    }

    fn inwardContraryMotion(&self) -> bool {
        self.inner.inward_contrary_motion()
    }

    /// music21's `antiParallelMotion`: contrary motion between two spellings
    /// of the same simple interval, a fifth opening out to a twelfth.
    #[pyo3(signature = (simpleName = None))]
    fn antiParallelMotion(&self, simpleName: Option<&Bound<'_, PyAny>>) -> PyResult<bool> {
        let required = match simpleName.filter(|value| !value.is_none()) {
            Some(value) => Some(interval_argument(value)?),
            None => None,
        };
        Ok(self.inner.anti_parallel_motion(required.as_ref()))
    }

    fn parallelInterval(&self, thisInterval: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(self
            .inner
            .parallel_interval(&interval_argument(thisInterval)?))
    }

    fn parallelFifth(&self) -> bool {
        self.inner.parallel_fifth()
    }

    fn parallelOctave(&self) -> bool {
        self.inner.parallel_octave()
    }

    fn parallelUnison(&self) -> bool {
        self.inner.parallel_unison()
    }

    fn parallelUnisonOrOctave(&self) -> bool {
        self.inner.parallel_unison_or_octave()
    }

    fn hiddenInterval(&self, thisInterval: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(self
            .inner
            .hidden_interval(&interval_argument(thisInterval)?))
    }

    fn hiddenFifth(&self) -> bool {
        self.inner.hidden_fifth()
    }

    fn hiddenOctave(&self) -> bool {
        self.inner.hidden_octave()
    }

    fn voiceOverlap(&self) -> bool {
        self.inner.voice_overlap()
    }

    fn voiceCrossing(&self) -> bool {
        self.inner.voice_crossing()
    }

    /// music21's `isProperResolution`.
    fn isProperResolution(&self) -> PyResult<bool> {
        self.inner.is_proper_resolution().map_err(quartet_error)
    }

    fn leapNotSetWithStep(&self) -> bool {
        self.inner.leap_not_set_with_step()
    }

    /// music21's `modalOpening`.
    fn modalOpening(&self) -> PyResult<bool> {
        self.inner.modal_opening().map_err(quartet_error)
    }

    /// music21's `opensIncorrectly`, the older name for the other answer.
    fn opensIncorrectly(&self) -> PyResult<bool> {
        Ok(!self.modalOpening()?)
    }

    /// music21's `clausulaVera`.
    fn clausulaVera(&self) -> PyResult<bool> {
        self.inner.clausula_vera().map_err(quartet_error)
    }

    /// music21's `closesIncorrectly`, the older name for the other answer.
    fn closesIncorrectly(&self) -> PyResult<bool> {
        Ok(!self.clausulaVera()?)
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        py: Python<'py>,
        _memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.borrow().copied(py);
        crate::copy_as_same_type(slf, copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.borrow().copied(py);
        crate::copy_as_same_type(slf, copied)
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        let named = |index: usize| {
            self.notes[index]
                .borrow(py)
                .synced(py)
                .pitch()
                .name_with_octave()
        };
        format!(
            "<music21.voiceLeading.VoiceLeadingQuartet v1n1={}, v1n2={}, v2n1={}, v2n2={}>",
            named(0),
            named(1),
            named(2),
            named(3)
        )
    }
}

/// What a caller asks of a parallel motion.
///
/// A number is how wide the interval must be, whatever it is spelled, and so
/// is a generic interval; anything else names the interval itself.
fn parallel_requirement(value: &Bound<'_, PyAny>) -> PyResult<RsParallel> {
    if let Ok(steps) = value.extract::<i32>() {
        return Ok(RsParallel::Wide(steps));
    }
    if let Ok(generic) = value.extract::<PyRef<'_, crate::interval::GenericInterval>>() {
        return Ok(RsParallel::Wide(generic.inner.semi_simple_undirected()));
    }
    Ok(RsParallel::Named(Box::new(interval_argument(value)?)))
}

/// An interval argument, which music21 takes as an object or as a name.
fn interval_argument(value: &Bound<'_, PyAny>) -> PyResult<RsInterval> {
    if let Ok(name) = value.extract::<String>() {
        return RsInterval::from_name(&name).map_err(quartet_error);
    }
    if let Ok(facade) = value.extract::<PyRef<'_, crate::interval::Interval>>() {
        return Ok(facade.inner.clone());
    }
    // A diatonic interval names one too, without saying how many semitones.
    if let Ok(diatonic) = value.extract::<PyRef<'_, crate::interval::DiatonicInterval>>() {
        return RsInterval::from_name(diatonic.inner.name()).map_err(quartet_error);
    }
    let name: String = value.getattr("name")?.extract()?;
    RsInterval::from_name(&name).map_err(quartet_error)
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<VoiceLeadingQuartet>()?;
    Ok(())
}
