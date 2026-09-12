//! music21's `chord.Chord` over `music21-rs`, with music21's names,
//! properties and `repr`.

#![allow(non_snake_case)]

use std::sync::Mutex;

use pyo3::exceptions::{PyIndexError, PyKeyError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

use music21_rs::{
    Chord as RsChord, ChordTableAddress as RsChordTableAddress, Duration as RsDuration,
    Interval as RsInterval, Key as RsKey, Note as RsNote, Notehead as RsNotehead, Pitch as RsPitch,
    StemDirection as RsStemDirection, Volume as RsVolume,
};

use crate::duration::{Duration, duration_from_any};
use crate::interval::interval_from_any;
use crate::notation::{Beams, Tie, Volume, volume_from_any};
use crate::note::{Note, augment_or_diminish_note, grace_note, instrument_for_note, note_from_any};
use crate::pitch::{Accidental, Pitch, message, pitch_from_any};

/// The names the key facade provides, for swapping into `music21.key`.
/// The names the `chord` facade replaces in `music21.chord`.
pub const NAMES: &[&str] = &["Chord", "ChordException"];

pyo3::create_exception!(music21_rs_facade, ChordException, crate::Music21Exception);

/// The duration a caller asked for by keyword, if they asked for one.
///
/// music21 lets any note-like object be timed as it is built, and hands the
/// keywords it does not read itself on to `Duration` — so `Chord(notes,
/// type='whole')` is a whole note and `RomanNumeral('I', k,
/// quarterLength=4.0)` lasts a bar. A duration object given by keyword *is*
/// the object the thing built carries, which is what makes
/// `chord.Chord('A4 C#5', duration=d).duration is d` hold.
pub(crate) fn duration_from_keywords(
    py: Python<'_>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<Option<Py<PyAny>>> {
    let Some(keywords) = keywords else {
        return Ok(None);
    };
    if let Some(value) = keywords.get_item("duration")? {
        if value.hasattr("quarterLength")? {
            return Ok(Some(value.clone().unbind()));
        }
        return Ok(Some(
            crate::installed_new(
                py,
                "music21.duration",
                "Duration",
                Duration::wrap(duration_from_any(&value)?),
            )?
            .into_any(),
        ));
    }
    if let Some(value) = keywords.get_item("quarterLength")? {
        let length = RsDuration::new(value.extract::<f64>()?).map_err(chord_error)?;
        return Ok(Some(
            crate::installed_new(py, "music21.duration", "Duration", Duration::wrap(length))?
                .into_any(),
        ));
    }
    if keywords.contains("type")? || keywords.contains("dots")? {
        return Ok(Some(
            crate::installed_new(
                py,
                "music21.duration",
                "Duration",
                Duration::new(None, Some(keywords))?,
            )?
            .into_any(),
        ));
    }
    Ok(None)
}

error_into!(chord_error, ChordException);

/// music21's `chord.Chord`.
#[pyclass(
    name = "Chord",
    module = "music21.chord",
    subclass,
    skip_from_py_object
)]
pub struct Chord {
    /// The pitches and the analysis read off them. Every method that asks a
    /// musical question goes through this.
    pub(crate) inner: RsChord,
    /// The `Note` objects music21 hands back from `chord[i]` and `.notes`.
    /// They are the same objects every time, so notation written through one
    /// of them sticks; their pitches mirror `inner`, and structural changes
    /// rebuild them from it.
    notes: Vec<Py<Note>>,
    /// The chord's own `Duration`, kept as the Python object music21 hands
    /// back so `chord.duration is d` holds and edits through it stick. It
    /// may be one of music21's own subclasses of it — a `GraceDuration` —
    /// which one of ours put in its place would stop being.
    duration: Option<Py<PyAny>>,
    /// The chord's own `Volume`, likewise.
    volume: Option<Py<Volume>>,
    /// music21's `storedInstrument`: the instrument this chord is played on,
    /// when it is not simply the one the part is written for. The crate
    /// models no instruments, so the object is kept as it was given.
    stored_instrument: Option<Py<PyAny>>,
    /// music21's `expressions` and `articulations`: the ornaments written
    /// over the chord and the marks written under it. music21 keeps them on
    /// every `NotRest`, and its own notation code reads them off a chord as
    /// readily as off a note, so the lists are held here rather than left to
    /// the notes.
    expressions: Option<Py<PyList>>,
    articulations: Option<Py<PyList>>,
    /// music21's `_overrides`: the answers a caller has fixed rather than
    /// letting the chord work them out.
    ///
    /// The crate models the same thing — `Chord::set_root` records an answer
    /// that wins over the inferred one — but music21's own doctests reach
    /// into the dictionary directly, and writing `None` there is how they
    /// say "this chord has no root at all". So the dictionary is the storage
    /// the facade reads, made on first asking and kept.
    overrides: Option<Py<PyDict>>,
    /// music21's `beams`, as the object itself, once something has asked
    /// for one — music21's own readers write into what it hands back.
    beams: Option<Py<Beams>>,
    /// music21's `style`, once something has asked for one: the object
    /// saying how this is drawn. It is music21's own object — the page is
    /// not something this crate models — and its mere existence is what
    /// `hasStyleInformation` answers, as music21's does.
    style: Option<Py<PyAny>>,
    /// What the chord has already worked out. See [`ChordCache`].
    ///
    /// A mutex rather than a `RefCell` because pyo3 asks a `#[pyclass]` to be
    /// `Sync`; it is never contended — the interpreter holds one object at a
    /// time — and a lock it never waits on costs nothing beside the work it
    /// saves.
    cache: Mutex<ChordCache>,
}

/// music21's `_cache`: the answers a chord has already worked out.
///
/// music21 decorates twenty-one of `Chord`'s members with `@cacheMethod`,
/// which keeps each under its own name in a `_cache` dictionary that
/// `clearCache()` empties, and puts `root`, `bass` and `inversion` there by
/// hand. The facade worked every one of them out afresh on each asking,
/// which is what made `Chord.commonName` ten times slower here than in
/// music21 on a chord that had already answered, and what made music21's own
/// `collapseArpeggios` twenty times slower.
///
/// The crate underneath deliberately caches nothing — a value type that
/// answers the same question twice is doing arithmetic, not keeping state.
/// The facade is the other thing: it is music21's object, and how music21's
/// object behaves is part of what it has to get right.
///
/// Fields rather than a dictionary because each answer has its own type, and
/// the whole of it is emptied at once, which is what `clearCache` does.
#[derive(Default)]
struct ChordCache {
    common_name: Option<String>,
    quality: Option<&'static str>,
    normal_order: Option<Vec<u32>>,
    chord_tables_address: Option<RsChordTableAddress>,
    // music21 caches only the address, because its own prime form and
    // interval vector are read off that. The crate works each out from the
    // pitches instead, so caching the address alone would buy nothing; these
    // are kept beside it. Every one is a pure function of the notes, so what
    // a caller sees is unchanged — it is only reached for sooner.
    pitched_common_name: Option<String>,
    forte_class: Option<String>,
    prime_form: Option<Vec<u32>>,
    prime_form_string: Option<String>,
    interval_vector: Option<Vec<u32>>,
    interval_vector_string: Option<String>,
    ordered_pitch_classes: Option<Vec<u32>>,
    ordered_pitch_classes_string: Option<String>,
    /// The fourteen `is...` questions, two bits each: whether it has been
    /// asked, and what the answer was.
    ///
    /// Fourteen answers fit in a pair of `u16`s, so the cache allocates
    /// nothing and adds nothing to the cost of building a chord.
    asked: u16,
    answers: u16,
}

/// Which bit of [`ChordCache::asked`] each `is...` question sits in. The
/// order is music21's own, and only the position matters.
const ISTRIAD: u32 = 0;
const ISSEVENTH: u32 = 1;
const ISMAJORTRIAD: u32 = 2;
const ISMINORTRIAD: u32 = 3;
const ISDIMINISHEDTRIAD: u32 = 4;
const ISAUGMENTEDTRIAD: u32 = 5;
const ISDOMINANTSEVENTH: u32 = 6;
const ISDIMINISHEDSEVENTH: u32 = 7;
const ISHALFDIMINISHEDSEVENTH: u32 = 8;
const ISFALSEDIMINISHEDSEVENTH: u32 = 9;
const ISINCOMPLETEMAJORTRIAD: u32 = 10;
const ISINCOMPLETEMINORTRIAD: u32 = 11;
const ISCONSONANT: u32 = 12;
const ISNINTH: u32 = 13;

/// Answers from the cache, or works it out and keeps it.
///
/// `$slot` is the field on [`ChordCache`]; the borrow is dropped before
/// `$compute` runs, since working the answer out reads the chord.
macro_rules! cached {
    ($self:ident, $slot:ident, $compute:expr) => {{
        if let Some(value) = $self.cache().$slot.clone() {
            return value;
        }
        let value = $compute;
        $self.cache().$slot = Some(value.clone());
        value
    }};
}

/// The same, for the `is...` questions, which share one map.
macro_rules! cached_predicate {
    ($self:ident, $bit:expr, $compute:expr) => {{
        let bit = 1u16 << $bit;
        {
            let cache = $self.cache();
            if cache.asked & bit != 0 {
                return cache.answers & bit != 0;
            }
        }
        let value = $compute;
        let mut cache = $self.cache();
        cache.asked |= bit;
        if value {
            cache.answers |= bit;
        }
        value
    }};
}

impl Chord {
    /// The colour the chord is written in: what its style says if it has
    /// one, since that is where music21 keeps it, and what the value says
    /// otherwise.
    pub(crate) fn colour(&self, py: Python<'_>) -> Option<String> {
        if self.style.is_some() {
            return crate::notation::style_colour(py, self.style.as_ref());
        }
        self.inner.color().map(str::to_string)
    }

    /// Builds the facade around a chord, giving each of its notes a Python
    /// object of its own.
    pub(crate) fn from_inner(py: Python<'_>, inner: RsChord) -> PyResult<Self> {
        let mut chord = Self {
            inner,
            notes: Vec::new(),
            duration: None,
            volume: None,
            stored_instrument: None,
            expressions: None,
            articulations: None,
            overrides: None,
            style: None,
            beams: None,
            cache: Mutex::default(),
        };
        chord.rebuild_notes(py)?;
        Ok(chord)
    }

    fn build(
        py: Python<'_>,
        notes: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        // music21 makes the chord's duration before reading the notes, and
        // hands it to every note it builds. A duration given by keyword *is*
        // that object, so `chord.Chord('A4 C#5', duration=d).duration is d`.
        let mut quick = true;
        let mut shared = crate::installed_new(
            py,
            "music21.duration",
            "Duration",
            Duration::wrap(RsDuration::quarter()),
        )?
        .into_any();
        if let Some(given) = duration_from_keywords(py, keywords)? {
            quick = false;
            shared = given;
        }
        let adopted = adopted_notes(py, notes, &shared, quick)?;
        let duration = adopted.taken.unwrap_or(shared);
        Self::from_notes(py, adopted.notes, duration)
    }

    /// Builds the facade around note objects the caller already holds,
    /// reading the chord off them, with the duration object they share.
    fn from_notes(py: Python<'_>, notes: Vec<Py<Note>>, duration: Py<PyAny>) -> PyResult<Self> {
        let inners: Vec<RsNote> = notes
            .iter()
            .map(|note| note.borrow(py).synced(py))
            .collect();
        let mut inner = RsChord::new(inners.as_slice()).map_err(chord_error)?;
        if let Some(value) = crate::duration::duration_value_of(py, &duration) {
            inner.set_duration(value);
        }
        Ok(Self {
            inner,
            notes,
            duration: Some(duration),
            volume: None,
            stored_instrument: None,
            expressions: None,
            articulations: None,
            overrides: None,
            style: None,
            beams: None,
            cache: Mutex::default(),
        })
    }

    /// Replaces the whole chord, rebuilding the note objects: what a caller
    /// standing on a chord does when the chord it stands for changes.
    /// The chord's own pitches as values, for a caller that wants to work
    /// on them and hand the whole chord back.
    pub(crate) fn value_pitches(&self) -> Vec<music21_rs::Pitch> {
        self.inner.pitches()
    }

    pub(crate) fn replace_value(&mut self, py: Python<'_>, inner: RsChord) -> PyResult<()> {
        self.replace_inner(py, inner)
    }

    /// Replaces the pitches and everything read off them, rebuilding the
    /// note objects to match. Notation on the old notes does not survive a
    /// structural change, which is what music21 does too.
    /// Makes the chord the pitches given, keeping how long it sounds: a
    /// chord that was timed and then told what its notes are is still that
    /// long, which is what music21's own `pitches` setter leaves alone.
    fn replace_pitches(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let mut replaced = chord_from_any(Some(value))?;
        if let Some(duration) = self.inner.duration().cloned() {
            replaced.set_duration(duration);
        }
        self.replace_inner(py, replaced)
    }

    fn replace_inner(&mut self, py: Python<'_>, inner: RsChord) -> PyResult<()> {
        self.inner = inner;
        self.clear_cache();
        self.rebuild_notes(py)
    }

    /// Throws away what the chord had worked out. music21 does this wherever
    /// the notes change — `add`, `remove`, the `pitches` and `pitchNames`
    /// setters, `sortDiatonicAscending`, `semiClosedPosition` — and every one
    /// of those goes through one of the three places this is called from.
    pub(crate) fn clear_cache(&mut self) {
        *self.cache() = ChordCache::default();
    }

    /// The cache, whoever last held it. A lock is only ever poisoned by a
    /// panic while it was held, and what is behind it is answers that can be
    /// worked out again, so the value is taken rather than the panic
    /// repeated.
    fn cache(&self) -> std::sync::MutexGuard<'_, ChordCache> {
        self.cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Replaces the chord with one of its own reductions, keeping the note
    /// objects whose notes survive. A reduction drops notes rather than
    /// rewriting them, and music21 drops them out of its own list, so the
    /// note and pitch objects of the survivors come through:
    /// `c3.pitches[0] is p1` after `removeRedundantPitches(inPlace=True)`.
    fn reduce_inner(&mut self, py: Python<'_>, inner: RsChord) -> PyResult<()> {
        let mut spare: Vec<Option<Py<Note>>> = self
            .notes
            .iter()
            .map(|note| Some(note.clone_ref(py)))
            .collect();
        let mut kept: Vec<Py<Note>> = Vec::with_capacity(inner.notes().len());
        for note in inner.notes() {
            let survivor = spare.iter_mut().find(|held| {
                held.as_ref()
                    .is_some_and(|held| held.borrow(py).inner.pitch() == note.pitch())
            });
            match survivor {
                Some(slot) => kept.push(slot.take().expect("the slot it found was filled")),
                None => kept.push(Note::object(py, note.clone())?),
            }
        }
        self.inner = inner;
        self.notes = kept;
        self.clear_cache();
        Ok(())
    }

    /// Takes note objects as the chord's own, rebuilding the value from
    /// what they say. music21's `add` keeps the very note it is given —
    /// `chordify` writes the part's name on a note's pitch and then adds the
    /// note — so the objects come through rather than being made again.
    fn take_notes(slf: &Bound<'_, Self>, mut notes: Vec<Py<Note>>, run_sort: bool) -> PyResult<()> {
        let py = slf.py();
        if run_sort {
            notes.sort_by(|left, right| {
                let left = left.borrow(py).inner.pitch().ps();
                let right = right.borrow(py).inner.pitch().ps();
                left.partial_cmp(&right)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        let inners: Vec<RsNote> = notes
            .iter()
            .map(|note| note.borrow(py).synced(py))
            .collect();
        let mut rebuilt = RsChord::new(inners.as_slice()).map_err(chord_error)?;
        {
            let me = slf.borrow();
            if let Some(duration) = me.inner.duration().cloned() {
                rebuilt.set_duration(duration);
            }
            if let Some(root) = me.inner.overridden_root().cloned() {
                rebuilt.set_root(Some(root));
            }
        }
        let mut me = slf.borrow_mut();
        me.inner = rebuilt;
        me.notes = notes;
        // The notes have changed, so nothing the chord had worked out about
        // itself still describes it. This is the path `add` takes, and
        // music21 has a test for exactly it: `testCacheClearedOnAdd` asks a
        // consonant triad whether it is consonant, adds a C#5, and asks
        // again.
        me.clear_cache();
        drop(me);
        Chord::note_objects(slf);
        Ok(())
    }

    fn rebuild_notes(&mut self, py: Python<'_>) -> PyResult<()> {
        self.notes = self
            .inner
            .notes()
            .iter()
            .cloned()
            .map(|note| Note::object(py, note))
            .collect::<PyResult<_>>()?;
        Ok(())
    }

    /// The note objects, each told which chord holds it. Handing a note out
    /// without that leaves its pitch unable to find its way back here, so
    /// every accessor that gives Python a note or a pitch goes through this.
    fn note_objects(slf: &Bound<'_, Self>) -> Vec<Py<Note>> {
        let py = slf.py();
        let notes: Vec<Py<Note>> = slf
            .borrow()
            .notes
            .iter()
            .map(|note| note.clone_ref(py))
            .collect();
        for note in &notes {
            Note::attach_to_chord(py, note, slf.as_any());
        }
        notes
    }

    /// Takes the pitch one of our notes now carries: music21's chord and its
    /// notes hold one pitch between them, so an edit through the note's
    /// pitch object is an edit to the chord.
    pub(crate) fn adopt_note_pitch(
        slf: &Bound<'_, Self>,
        note: &Py<Note>,
        pitch: &RsPitch,
    ) -> PyResult<()> {
        let mut me = slf.borrow_mut();
        let Some(index) = me
            .notes
            .iter()
            .position(|held| held.as_ptr() == note.as_ptr())
        else {
            return Ok(());
        };
        me.inner.notes_mut()[index].set_pitch(pitch.clone());
        // A note of it was renamed, so what the chord had worked out about
        // itself was worked out about a chord it no longer is. This is the
        // chord half of music21's `pitchChanged`.
        me.clear_cache();
        Ok(())
    }

    /// The chord as it stands, with whatever its note objects carry written
    /// into it — the ties, the lyrics, the volumes — and everything the
    /// chord itself was told kept: its duration, and a root or bass a caller
    /// fixed. Anything that works on the chord as a value works on this.
    pub(crate) fn synced_inner(&self, py: Python<'_>) -> RsChord {
        let mut chord = self.inner.clone();
        for (held, note) in chord.notes_mut().iter_mut().zip(&self.notes) {
            *held = note.borrow(py).synced(py);
        }
        chord
    }

    /// The chord with the notation its note objects carry written back onto
    /// it, for the few questions that read notation rather than pitch.
    fn with_note_notation(&self, py: Python<'_>) -> PyResult<RsChord> {
        let notes: Vec<RsNote> = self
            .notes
            .iter()
            .map(|note| note.borrow(py).synced(py))
            .collect();
        let mut chord = RsChord::new(notes.as_slice()).map_err(chord_error)?;
        if let Some(duration) = self.inner.duration() {
            chord.set_duration(duration.clone());
        }
        Ok(chord)
    }

    /// The note object holding the first pitch that matches, the way
    /// music21's per-note accessors take a pitch.
    /// music21 hands back the pitches it dropped when reducing in place,
    /// and the reduced chord when not.
    fn deliver_reduced(
        &mut self,
        py: Python<'_>,
        reduced: RsChord,
        removed: Vec<RsPitch>,
        in_place: bool,
    ) -> PyResult<Option<Py<PyAny>>> {
        if in_place {
            self.reduce_inner(py, reduced)?;
            let dropped: Vec<Pitch> = removed
                .into_iter()
                .map(|pitch| Pitch::wrap(pitch, false))
                .collect();
            return Ok(Some(PyList::new(py, dropped)?.into_any().unbind()));
        }
        Ok(Some(
            crate::installed_new(py, "music21.chord", "Chord", Self::from_inner(py, reduced)?)?
                .into_any(),
        ))
    }

    /// The note a per-note setter writes to: the one the target names, or
    /// the first note when music21 lets the target be left out.
    fn first_or_named(
        &self,
        py: Python<'_>,
        target: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<Note>> {
        match target.filter(|target| !target.is_none()) {
            Some(target) => self.note_object(py, target),
            None => self
                .notes
                .first()
                .map(|note| note.clone_ref(py))
                .ok_or_else(|| ChordException::new_err("the chord has no notes")),
        }
    }

    fn note_object(&self, py: Python<'_>, target: &Bound<'_, PyAny>) -> PyResult<Py<Note>> {
        let wanted = if let Ok(name) = target.extract::<String>() {
            RsPitch::from_name(name).map_err(chord_error)?
        } else if let Ok(index) = target.extract::<usize>() {
            return self
                .notes
                .get(index)
                .map(|note| note.clone_ref(py))
                .ok_or_else(|| PyIndexError::new_err("list index out of range"));
        } else {
            // The very pitch object first, as music21 looks: a chord with
            // two D4s in it is asked about one of them, not about the note.
            if let Ok(given) = target.extract::<Py<Pitch>>()
                && let Some(note) = self
                    .notes
                    .iter()
                    .find(|note| Note::get_pitch(note.bind(py)).is(&given))
            {
                return Ok(note.clone_ref(py));
            }
            pitch_from_any(target)?
        };
        self.notes
            .iter()
            .find(|note| {
                note.borrow(py).inner.pitch().name_with_octave() == wanted.name_with_octave()
            })
            .or_else(|| {
                self.notes
                    .iter()
                    .find(|note| note.borrow(py).inner.pitch().name() == wanted.name())
            })
            .map(|note| note.clone_ref(py))
            .ok_or_else(|| {
                ChordException::new_err(format!(
                    "the given pitch is not in the Chord: {}",
                    wanted.name_with_octave()
                ))
            })
    }

    fn duration_object(&mut self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        if let Some(duration) = &self.duration {
            return Ok(duration.clone_ref(py));
        }
        let created = crate::installed_new(
            py,
            "music21.duration",
            "Duration",
            Duration::wrap(
                self.inner
                    .duration()
                    .cloned()
                    .unwrap_or_else(RsDuration::quarter),
            ),
        )?
        .into_any();
        self.duration = Some(created.clone_ref(py));
        Ok(created)
    }

    pub(crate) fn quarter_length(&self, py: Python<'_>) -> f64 {
        match &self.duration {
            Some(duration) => crate::duration::duration_value_of(py, duration)
                .map_or(1.0, |value| value.quarter_length()),
            None => self
                .inner
                .duration()
                .map_or(1.0, music21_rs::Duration::quarter_length),
        }
    }

    /// A pitch the chord answers with, as a Python object. music21's
    /// `root()`, `bass()` and the chord steps hand back one of the chord's
    /// own pitches rather than a copy — `chord.root() is chord.pitches[0]`
    /// holds — so a value the chord carries comes back as the pitch object
    /// of the note carrying it, and only a value it does not carry (an
    /// overridden root from outside the chord) comes back loose.
    fn own_pitch(slf: &Bound<'_, Self>, pitch: Option<&RsPitch>) -> PyResult<Option<Py<Pitch>>> {
        let Some(pitch) = pitch else {
            return Ok(None);
        };
        let py = slf.py();
        for note in Self::note_objects(slf) {
            if note.borrow(py).inner.pitch() == pitch {
                return Ok(Some(Note::get_pitch(note.bind(py))));
            }
        }
        Ok(Some(crate::installed_new(
            py,
            "music21.pitch",
            "Pitch",
            Pitch::wrap(pitch.clone(), false),
        )?))
    }

    /// The root the pitches imply. music21 raises out of `_findRoot` when
    /// there are none to read it from, which is the only way this fails; the
    /// repr in the message is the empty chord's.
    fn found_root(&self) -> PyResult<Option<RsPitch>> {
        if self.inner.pitches().is_empty() {
            return Err(ChordException::new_err(
                "no pitches in chord <music21.chord.Chord >",
            ));
        }
        Ok(self.inner.found_root().cloned())
    }
}

/// Reads whatever music21's `Chord(...)` accepts: a space-separated string, a
/// sequence of names, pitches, notes, chords or MIDI numbers, or nothing.
/// A sequence of plain integers is spelled the way music21 spells one, which
/// is why it does not go through the note path.
pub(crate) fn chord_from_any(value: Option<&Bound<'_, PyAny>>) -> PyResult<RsChord> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(RsChord::empty());
    };
    if let Ok(text) = value.extract::<String>() {
        return RsChord::new(text).map_err(chord_error);
    }
    let items: Vec<Bound<'_, PyAny>> = value
        .try_iter()
        .map_err(|_| ChordException::new_err("Chord needs a string or a sequence"))?
        .collect::<PyResult<_>>()?;
    let all_integers = !items.is_empty()
        && items.iter().all(|item| {
            item.extract::<String>().is_err()
                && item.extract::<i32>().is_ok()
                && item.extract::<PyRef<Pitch>>().is_err()
        });
    if all_integers {
        let midi: Vec<i32> = items
            .iter()
            .map(|item| item.extract::<i32>())
            .collect::<PyResult<_>>()?;
        return RsChord::new(midi).map_err(chord_error);
    }
    let mut notes: Vec<RsNote> = Vec::with_capacity(items.len());
    for item in &items {
        if let Ok(chord) = item.extract::<PyRef<Chord>>() {
            notes.extend(chord.inner.notes().iter().cloned());
        } else {
            notes.push(note_from_any(item).map_err(|_| {
                PyTypeError::new_err(format!(
                    "Could not process input argument {}",
                    item.repr()
                        .map_or_else(|_| "?".to_string(), |r| r.to_string())
                ))
            })?);
        }
    }
    RsChord::new(notes.as_slice()).map_err(chord_error)
}

/// The note objects a chord's contents argument names, keeping whatever
/// `Note` or `Pitch` objects it was handed. music21 appends the very objects
/// given to it — `chord[0] is n1`, `chord.pitches[0] is p1` — so a chord
/// built from them shares them rather than copying their values out.
///
/// Everything else (a name, a number, another chord) has no object to keep
/// and becomes a note of our own.
/// Whether the chord's duration is still up for grabs. music21's
/// `quickDuration`: with no duration keyword, the first note handed in gives
/// the chord its duration, and every note the chord builds itself before
/// that shares the chord's.
struct AdoptedNotes {
    notes: Vec<Py<Note>>,
    /// The duration object the chord should take, when a note handed in gave
    /// it one.
    taken: Option<Py<PyAny>>,
}

fn adopted_notes(
    py: Python<'_>,
    value: Option<&Bound<'_, PyAny>>,
    shared: &Py<PyAny>,
    mut quick: bool,
) -> PyResult<AdoptedNotes> {
    let fresh = |chord: RsChord| -> PyResult<Vec<Py<Note>>> {
        chord
            .notes()
            .iter()
            .cloned()
            .map(|note| {
                let object = Note::object(py, note)?;
                object.borrow_mut(py).share_duration(py, shared);
                Ok(object)
            })
            .collect()
    };
    let loose = |notes| Ok(AdoptedNotes { notes, taken: None });
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return loose(Vec::new());
    };
    if value.extract::<String>().is_ok() {
        return loose(fresh(chord_from_any(Some(value))?)?);
    }
    let Ok(items) = value
        .try_iter()
        .and_then(|items| items.collect::<PyResult<Vec<Bound<'_, PyAny>>>>())
    else {
        return loose(fresh(chord_from_any(Some(value))?)?);
    };
    // A list of plain integers is a pitch-class or MIDI list, spelled as a
    // whole rather than one number at a time.
    let all_integers = !items.is_empty()
        && items.iter().all(|item| {
            item.extract::<String>().is_err()
                && item.extract::<i32>().is_ok()
                && item.extract::<PyRef<Pitch>>().is_err()
        });
    if all_integers {
        return loose(fresh(chord_from_any(Some(value))?)?);
    }
    let mut notes: Vec<Py<Note>> = Vec::with_capacity(items.len());
    let mut taken: Option<Py<PyAny>> = None;
    let mut use_duration = Some(shared.clone_ref(py));
    for item in &items {
        if let Ok(note) = item.extract::<Py<Note>>() {
            if quick {
                taken = Some(note.borrow_mut(py).duration_object(py)?);
                use_duration = None;
                quick = false;
            }
            notes.push(note);
            continue;
        }
        let built = if let Ok(pitch) = item.extract::<Py<Pitch>>() {
            Note::object_for_pitch(py, pitch)?
        } else if let Ok(chord) = item.extract::<PyRef<Chord>>() {
            // music21 deep-copies the notes it takes out of another chord.
            for note in chord.inner.notes() {
                notes.push(Note::object(py, note.clone())?);
            }
            continue;
        } else {
            // music21 names the class a caller should have reached for
            // rather than saying it could not read the argument.
            if item.getattr("classes").is_ok_and(|classes| {
                classes
                    .extract::<Vec<String>>()
                    .is_ok_and(|classes| classes.iter().any(|name| name == "Unpitched"))
            }) {
                return Err(PyTypeError::new_err(format!(
                    "Use a PercussionChord to contain Unpitched objects; got [{}]",
                    item.repr()
                        .map_or_else(|_| "?".to_string(), |value| value.to_string())
                )));
            }
            let note = note_from_any(item).map_err(|_| {
                PyTypeError::new_err(format!(
                    "Could not process input argument {}",
                    item.repr()
                        .map_or_else(|_| "?".to_string(), |r| r.to_string())
                ))
            })?;
            Note::object(py, note)?
        };
        if let Some(duration) = &use_duration {
            built.borrow_mut(py).share_duration(py, duration);
        }
        notes.push(built);
    }
    Ok(AdoptedNotes { notes, taken })
}

/// pyo3 hands a `Vec<u8>` to Python as `bytes`; pitch-class lists must come
/// back as a list of numbers, so they travel as `u32`.
fn into_numbers(values: Vec<u8>) -> Vec<u32> {
    values.into_iter().map(u32::from).collect()
}

/// music21 refuses a bare string where a sequence of notes or pitches is
/// wanted, since a string is itself iterable and would come apart letter by
/// letter.
fn require_iterable(value: &Bound<'_, PyAny>, field: &str) -> PyResult<()> {
    if value.extract::<String>().is_ok() || value.try_iter().is_err() {
        return Err(PyTypeError::new_err(format!(
            "{field} must be set with an iterable"
        )));
    }
    Ok(())
}

/// music21's `chord.tables.ChordTableAddress`: where a chord's set class
/// sits in the Forte tables.
///
/// music21 makes this a `NamedTuple`, so it is indexable and compares equal
/// to a plain tuple, and its `repr` names the four fields.
#[pyclass(
    name = "ChordTableAddress",
    module = "music21.chord.tables",
    subclass,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct ChordTableAddress {
    inner: RsChordTableAddress,
}

#[pymethods]
impl ChordTableAddress {
    /// music21 freezes a score by pickling it, and what this object is lives
    /// in Rust where a pickle cannot see it — so it is written out as text,
    /// and read back into a fresh one of these.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let Some(inner) = crate::unpickled::<_, RsChordTableAddress>(slf, state)? else {
            return Ok(());
        };
        slf.borrow_mut().inner = inner;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (cardinality = 0, forteClass = 0, inversion = 0, pcOriginal = 0))]
    fn new(cardinality: u8, forteClass: u8, inversion: i8, pcOriginal: u8) -> Self {
        Self {
            inner: RsChordTableAddress {
                cardinality,
                forte_class: forteClass,
                inversion,
                pitch_class_original: pcOriginal,
            },
        }
    }

    #[getter]
    fn cardinality(&self) -> u8 {
        self.inner.cardinality
    }

    #[getter]
    fn forteClass(&self) -> u8 {
        self.inner.forte_class
    }

    #[getter]
    fn inversion(&self) -> i8 {
        self.inner.inversion
    }

    #[getter]
    fn pcOriginal(&self) -> u8 {
        self.inner.pitch_class_original
    }

    fn __len__(&self) -> usize {
        4
    }

    fn __getitem__(&self, py: Python<'_>, index: isize) -> PyResult<Py<PyAny>> {
        Ok(match index {
            0 | -4 => self
                .inner
                .cardinality
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            1 | -3 => self
                .inner
                .forte_class
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            2 | -2 => self.inner.inversion.into_pyobject(py)?.into_any().unbind(),
            3 | -1 => self
                .inner
                .pitch_class_original
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            _ => return Err(PyIndexError::new_err("tuple index out of range")),
        })
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        if let Ok(other) = other.extract::<PyRef<'_, Self>>() {
            return other.inner == self.inner;
        }
        other.extract::<(u8, u8, i8, u8)>().is_ok_and(|values| {
            values
                == (
                    self.inner.cardinality,
                    self.inner.forte_class,
                    self.inner.inversion,
                    self.inner.pitch_class_original,
                )
        })
    }

    /// music21 restores hashing on identity where it defines equality, so
    /// that a set or a dictionary can hold one of these however it compares.
    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        slf.as_ptr() as isize >> 4
    }

    fn __repr__(&self) -> String {
        format!(
            "ChordTableAddress(cardinality={}, forteClass={}, inversion={}, pcOriginal={})",
            self.inner.cardinality,
            self.inner.forte_class,
            self.inner.inversion,
            self.inner.pitch_class_original
        )
    }
}

/// One pitch's place in a key: which degree it is, and how it is altered
/// from that degree.
type ScaleDegree = (Option<usize>, Option<Accidental>);

/// Fixes an answer by hand, in the dictionary music21 keeps them in: its own
/// `root()` and `bass()` setters write there, and its `harmony` module reads
/// the entries straight back out.
fn override_with(chord: &Bound<'_, Chord>, key: &str, value: &RsPitch) -> PyResult<()> {
    let py = chord.py();
    // The chord's own pitch object where the chord has that pitch — music21
    // looks for it by octave and then by name alone, so that an edit to the
    // chord's octaves is an edit to the answer it was told. Only a pitch the
    // chord does not carry is stored loose.
    let pitch = match chord_pitch_named(chord, value)? {
        Some(pitch) => pitch,
        None => crate::installed_new(
            py,
            "music21.pitch",
            "Pitch",
            Pitch::wrap(value.clone(), false),
        )?,
    };
    let overrides = chord.borrow_mut()._overrides(py);
    overrides.bind(py).set_item(key, pitch)
}

/// The chord's own object for a pitch, matched as music21 matches it: the
/// same note in the same octave, then the same note in any octave.
fn chord_pitch_named(chord: &Bound<'_, Chord>, value: &RsPitch) -> PyResult<Option<Py<Pitch>>> {
    let py = chord.py();
    let notes = Chord::note_objects(chord);
    for note in &notes {
        if note.borrow(py).inner.pitch().name_with_octave() == value.name_with_octave() {
            return Ok(Some(Note::get_pitch(note.bind(py))));
        }
    }
    for note in &notes {
        if note.borrow(py).inner.pitch().name() == value.name() {
            return Ok(Some(Note::get_pitch(note.bind(py))));
        }
    }
    Ok(None)
}

/// Throws away an answer fixed by hand, which is what `find=True` does: it
/// asks the chord to work the answer out again and to go on doing so.
fn clear_override(chord: &Bound<'_, Chord>, key: &str) -> PyResult<()> {
    let py = chord.py();
    let Some(overrides) = chord.borrow().overrides.as_ref().map(|d| d.clone_ref(py)) else {
        return Ok(());
    };
    let overrides = overrides.bind(py);
    if overrides.contains(key)? {
        overrides.del_item(key)?;
    }
    Ok(())
}

/// A chord's fixed answers moved by an interval: music21 transposes every
/// pitch in `_overrides` along with the chord, so a chord symbol whose root
/// was fixed is, once transposed, a chord on the note that root moved to.
fn moved_overrides(
    py: Python<'_>,
    overrides: Option<&Py<PyDict>>,
    value: &Bound<'_, PyAny>,
) -> PyResult<Option<Py<PyDict>>> {
    let Some(overrides) = overrides else {
        return Ok(None);
    };
    let moved = PyDict::new(py);
    for (key, fixed) in overrides.bind(py).iter() {
        if let Ok(pitch) = fixed.extract::<PyRef<'_, Pitch>>() {
            drop(pitch);
            moved.set_item(key, fixed.call_method1("transpose", (value,))?)?;
        } else {
            moved.set_item(key, fixed)?;
        }
    }
    Ok(Some(moved.unbind()))
}

/// The answer fixed by hand for `key` in a chord's `_overrides`, if one has
/// been fixed at all. The outer `Option` says whether there is an entry; the
/// value inside it may itself be `None`, which is the entry saying there is
/// no such pitch.
fn overridden<'py>(chord: &Bound<'py, Chord>, key: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py = chord.py();
    let Some(overrides) = chord.borrow().overrides.as_ref().map(|d| d.clone_ref(py)) else {
        return Ok(None);
    };
    overrides.bind(py).get_item(key)
}

/// The key a chord is in: its own, or the nearest one in the streams that
/// hold it.
///
/// The search is music21's `getContextByClass`, since it is music21 that
/// holds the streams — this crate models no such containment and wants
/// none. A chord that is not living in one simply has no key.
fn chord_key_in_force(chord: &Bound<'_, Chord>) -> PyResult<Option<RsKey>> {
    if let Ok(own) = chord.getattr("key")
        && !own.is_none()
        && let Some(key) = key_value(&own)
    {
        return Ok(Some(key));
    }
    if !chord.hasattr("getContextByClass")? {
        return Ok(None);
    }
    let found = chord.call_method1("getContextByClass", ("Scale",))?;
    if found.is_none() {
        return Ok(None);
    }
    Ok(key_value(&found))
}

/// The crate's key behind whatever key object this is.
///
/// A facade `Key` carries one already. Anything else is read off its tonic
/// and mode, since a key found in a stream is as often music21's own as
/// ours — the chord doctests build one without the key facade in place.
fn key_value(value: &Bound<'_, PyAny>) -> Option<RsKey> {
    if let Ok(key) = value.extract::<PyRef<'_, crate::key::Key>>() {
        return Some(key.inner.clone());
    }
    let tonic = value.getattr("tonic").ok()?;
    let name: String = tonic
        .getattr("name")
        .ok()
        .and_then(|name| name.extract().ok())
        .or_else(|| tonic.extract().ok())?;
    let mode: Option<String> = value
        .getattr("mode")
        .ok()
        .and_then(|mode| mode.extract().ok());
    RsKey::from_tonic_mode(&name, mode.as_deref()).ok()
}

#[pymethods]
impl Chord {
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
        visit.call(&self.duration)?;
        visit.call(&self.volume)?;
        visit.call(&self.stored_instrument)?;
        visit.call(&self.expressions)?;
        visit.call(&self.articulations)?;
        visit.call(&self.overrides)?;
        visit.call(&self.beams)?;
        visit.call(&self.style)?;
        Ok(())
    }

    fn __clear__(&mut self) {
        self.notes.clear();
        self.duration = None;
        self.volume = None;
        self.stored_instrument = None;
        self.expressions = None;
        self.articulations = None;
        self.overrides = None;
        self.beams = None;
        self.style = None;
    }

    /// A chord is written out as text and read back, and its notes are made
    /// again from what it says. Its ornaments, its marks and the root a
    /// caller fixed are Python objects the value does not carry, so they go
    /// beside it.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        let py = slf.py();
        let extra = PyDict::new(py);
        {
            let chord = slf.borrow();
            extra.set_item("expressions", chord.expressions.as_ref())?;
            extra.set_item("articulations", chord.articulations.as_ref())?;
            extra.set_item("storedInstrument", chord.stored_instrument.as_ref())?;
            extra.set_item("_overrides", chord.overrides.as_ref())?;
            // As on a note: the tuplets a chord is written inside live on
            // the duration object rather than in the value.
            extra.set_item("duration", chord.duration.as_ref())?;
        }
        slf.borrow_mut().settle_beams(py);
        crate::pickled_extra(slf, &slf.borrow().inner, Some(&extra))
    }

    fn __setstate__(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        state: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let (inner, extra) = crate::unpickled_extra::<_, RsChord>(slf, state)?;
        let Some(inner) = inner else {
            return Ok(());
        };
        let rebuilt = Self::from_inner(py, inner)?;
        *slf.borrow_mut() = rebuilt;
        if let Some(extra) = extra {
            let extra = extra.bind(py);
            let mut chord = slf.borrow_mut();
            chord.expressions = extra.get_item("expressions")?.extract().ok();
            chord.articulations = extra.get_item("articulations")?.extract().ok();
            chord.overrides = extra.get_item("_overrides")?.extract().ok();
            chord.duration = extra
                .get_item("duration")
                .ok()
                .filter(|duration| !duration.is_none())
                .map(pyo3::Bound::unbind);
            chord.stored_instrument = Some(extra.get_item("storedInstrument")?.unbind())
                .filter(|value| !value.is_none(py));
        }
        Ok(())
    }

    #[new]
    #[pyo3(signature = (notes = None, **keywords))]
    /// music21 builds its objects in `__init__`; pyo3 builds them in
    /// `__new__`. A Python subclass of a music21 class constructs the base
    /// with *its own* arguments and then calls `super().__init__` with
    /// music21's — `Harte('C:maj')` reaches `Chord.__init__(pitches)` — so
    /// the work belongs in `__init__`, which is where music21 does it and
    /// where a subclass can reach it. `__new__` only has to hand back an
    /// object for `__init__` to fill in; doing the work here as well would
    /// do it twice for every direct caller.
    fn new(
        py: Python<'_>,
        notes: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let _ = (notes, keywords);
        Self::build(py, None, None)
    }

    /// music21's construction proper, which a direct caller reaches through
    /// `__new__` having already run it and a subclass reaches through
    /// `super().__init__`.
    #[pyo3(signature = (notes = None, **keywords))]
    fn __init__(
        slf: &Bound<'_, Self>,
        notes: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let built = Self::build(slf.py(), notes, keywords)?;
        *slf.borrow_mut() = built;
        Ok(())
    }

    // ---- contents --------------------------------------------------------

    /// music21's `.pitches`: the very pitch objects its notes hold, so
    /// `chord.pitches[0] is chord[0].pitch` and an edit through either lands
    /// on the chord.
    #[getter]
    fn get_pitches<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyTuple>> {
        let py = slf.py();
        let pitches: Vec<Py<Pitch>> = Self::note_objects(slf)
            .iter()
            .map(|note| Note::get_pitch(note.bind(py)))
            .collect();
        PyTuple::new(py, pitches)
    }

    #[setter]
    fn set_pitches(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        require_iterable(value, "pitches")?;
        self.replace_pitches(py, value)
    }

    #[setter]
    fn set_pitchNames(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        require_iterable(value, "pitchNames")?;
        self.replace_pitches(py, value)
    }

    #[getter]
    fn get_notes<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(slf.py(), Self::note_objects(slf))
    }

    /// music21 keeps the notes in `_notes` and its own code reaches for the
    /// slot — its tests tie a note by writing through it — so the slot
    /// answers here too, as the chord's own note objects.
    #[getter]
    fn _notes<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        PyList::new(slf.py(), Self::note_objects(slf))
    }

    #[setter]
    fn set__notes(slf: &Bound<'_, Self>, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        Self::set_notes(&mut slf.borrow_mut(), py, value)
    }

    #[setter]
    fn set_notes(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        require_iterable(value, "notes")?;
        let mut notes: Vec<RsNote> = Vec::new();
        for item in value.try_iter()? {
            let item = item?;
            let note = item.extract::<PyRef<Note>>().map_err(|_| {
                PyTypeError::new_err("every element of notes must be a note.Note object")
            })?;
            notes.push(note.synced(py));
        }
        let replaced = RsChord::new(notes.as_slice()).map_err(chord_error)?;
        self.replace_inner(py, replaced)
    }

    #[getter]
    fn pitchNames(&self) -> Vec<String> {
        self.inner.pitch_names()
    }

    #[getter]
    fn pitchClasses(&self) -> Vec<u32> {
        into_numbers(self.inner.note_pitch_classes())
    }

    #[getter]
    fn orderedPitchClasses(&self) -> Vec<u32> {
        cached!(
            self,
            ordered_pitch_classes,
            into_numbers(self.inner.pitch_classes())
        )
    }

    #[getter]
    fn orderedPitchClassesString(&self) -> String {
        cached!(
            self,
            ordered_pitch_classes_string,
            self.inner.ordered_pitch_classes_string()
        )
    }

    #[getter]
    fn multisetCardinality(&self) -> usize {
        self.inner.multiset_cardinality()
    }

    #[getter]
    fn pitchClassCardinality(&self) -> usize {
        self.inner.pitch_class_cardinality()
    }

    fn __len__(&self) -> usize {
        self.inner.notes().len()
    }

    fn __getitem__(slf: &Bound<'_, Self>, key: &Bound<'_, PyAny>) -> PyResult<Py<Note>> {
        let py = slf.py();
        let notes = Self::note_objects(slf);
        if let Ok(index) = key.extract::<isize>() {
            let length = notes.len() as isize;
            let resolved = if index < 0 { index + length } else { index };
            if resolved < 0 || resolved >= length {
                return Err(PyIndexError::new_err("list index out of range"));
            }
            return Ok(notes[resolved as usize].clone_ref(py));
        }
        // The very pitch object first, as music21 looks: a chord with two
        // D4s in it is asked about one of them.
        if let Ok(given) = key.extract::<Py<Pitch>>()
            && let Some(note) = notes
                .iter()
                .find(|note| Note::get_pitch(note.bind(py)).is(&given))
        {
            return Ok(note.clone_ref(py));
        }
        let wanted = if let Ok(name) = key.extract::<String>() {
            name.to_uppercase()
        } else {
            pitch_from_any(key)?.name_with_octave()
        };
        notes
            .iter()
            .find(|note| note.borrow(py).inner.pitch().name_with_octave() == wanted)
            .map(|note| note.clone_ref(py))
            .ok_or_else(|| {
                PyKeyError::new_err(format!(
                    "No note in the chord matches {}",
                    key.repr()
                        .map_or_else(|_| wanted.clone(), |r| r.to_string())
                ))
            })
    }

    fn __setitem__(
        &mut self,
        py: Python<'_>,
        key: &Bound<'_, PyAny>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let length = self.inner.notes().len() as isize;
        let resolved = match key.extract::<isize>() {
            Ok(index) if index < 0 => index + length,
            Ok(index) => index,
            Err(_) => {
                let wanted = if let Ok(name) = key.extract::<String>() {
                    name.to_uppercase()
                } else {
                    pitch_from_any(key)?.name_with_octave()
                };
                self.inner
                    .notes()
                    .iter()
                    .position(|note| note.pitch().name_with_octave() == wanted)
                    .map(|index| index as isize)
                    .ok_or_else(|| {
                        PyKeyError::new_err(format!("No note in the chord matches {wanted}"))
                    })?
            }
        };
        if resolved < 0 || resolved >= length {
            return Err(PyIndexError::new_err("list index out of range"));
        }
        let note = note_from_any(value)
            .map_err(|_| PyValueError::new_err("Chord index must be set to a valid note object"))?;
        let mut notes = self.inner.notes().to_vec();
        notes[resolved as usize] = note;
        let duration = self.inner.duration().cloned();
        let mut replaced = RsChord::new(notes.as_slice()).map_err(chord_error)?;
        if let Some(duration) = duration {
            replaced.set_duration(duration);
        }
        self.replace_inner(py, replaced)
    }

    fn __iter__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::get_notes(slf)?
            .into_any()
            .try_iter()
            .map(Bound::into_any)
    }

    #[pyo3(signature = (notes, *, runSort = true))]
    fn add(slf: &Bound<'_, Self>, notes: &Bound<'_, PyAny>, runSort: bool) -> PyResult<()> {
        let py = slf.py();
        let shared = slf.borrow_mut().duration_object(py)?;
        // One note is a list of one: the reader below takes a sequence, and
        // a single note handed in has to come through as the object it is.
        let one;
        let notes = if notes.extract::<String>().is_err() && notes.try_iter().is_err() {
            one = PyList::new(py, [notes])?.into_any();
            &one
        } else {
            notes
        };
        let mut held = Chord::note_objects(slf);
        held.extend(adopted_notes(py, Some(notes), &shared, false)?.notes);
        Self::take_notes(slf, held, runSort)
    }

    fn remove(&mut self, py: Python<'_>, removeItem: &Bound<'_, PyAny>) -> PyResult<()> {
        let mut reduced = self.with_note_notation(py)?;
        let result = if let Ok(name) = removeItem.extract::<String>() {
            reduced.remove_named(&name)
        } else if removeItem.extract::<PyRef<'_, Pitch>>().is_ok()
            || removeItem.extract::<PyRef<'_, Note>>().is_ok()
        {
            reduced.remove(&pitch_from_any(removeItem)?)
        } else {
            // music21 takes only a name, a Pitch or a NotRest; anything else
            // is refused by type rather than looked for and not found.
            return Err(PyValueError::new_err(format!(
                "Cannot remove {} from a chord; try a Pitch or Note object",
                removeItem.str()?
            )));
        };
        result.map_err(|error| PyValueError::new_err(message(&error)))?;
        self.replace_inner(py, reduced)
    }

    // ---- duration --------------------------------------------------------

    #[getter]
    fn get_duration(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let duration = slf.borrow_mut().duration_object(py)?;
        crate::duration::adopt_duration(py, &duration, slf.as_any());
        Ok(duration)
    }

    #[setter]
    fn set_duration(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let inner = duration_from_any(value)?;
        // A duration object is kept as it stands, whether it is one of ours
        // or one of music21's own kinds of duration.
        let duration = if value.hasattr("quarterLength")? {
            value.clone().unbind()
        } else {
            crate::installed_new(
                py,
                "music21.duration",
                "Duration",
                Duration::wrap(inner.clone()),
            )?
            .into_any()
        };
        crate::duration::adopt_duration(py, &duration, slf.as_any());
        let had_one = slf.borrow().duration.is_some();
        {
            let mut chord = slf.borrow_mut();
            chord.inner.set_duration(inner);
            chord.duration = Some(duration.clone_ref(py));
        }
        // As on a note: a chord already timed that is timed again is a
        // change the streams holding it have to hear about.
        if had_one {
            crate::duration::told_sites(
                slf.as_any(),
                &duration.bind(py).getattr("quarterLength")?,
            )?;
        }
        Ok(())
    }

    #[getter]
    fn get_quarterLength<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::duration::op_frac(py, self.quarter_length(py))
    }

    #[setter]
    fn set_quarterLength(&mut self, py: Python<'_>, value: f64) -> PyResult<()> {
        let inner = RsDuration::new(value).map_err(chord_error)?;
        self.inner.set_duration(inner.clone());
        match &self.duration {
            // Through the duration's own setter, so the written values it
            // stands for are worked out again: a whole note lengthened to
            // four and three quarters is written as two notes tied.
            Some(duration) => duration.bind(py).setattr("quarterLength", value)?,
            None => {
                self.duration = Some(
                    crate::installed_new(
                        py,
                        "music21.duration",
                        "Duration",
                        Duration::wrap(inner),
                    )?
                    .into_any(),
                );
            }
        }
        Ok(())
    }

    // ---- names -----------------------------------------------------------

    #[getter]
    fn commonName(&self) -> String {
        cached!(self, common_name, self.inner.common_name())
    }

    #[getter]
    fn pitchedCommonName(&self) -> String {
        cached!(self, pitched_common_name, self.inner.pitched_common_name())
    }

    #[getter]
    fn fullName(slf: &Bound<'_, Self>) -> PyResult<String> {
        let named = slf.borrow().inner.full_name();
        let timed = slf
            .borrow()
            .inner
            .duration()
            .cloned()
            .unwrap_or_else(RsDuration::quarter)
            .full_name();
        Ok(crate::note::named_with_duration(
            slf.as_any(),
            &named,
            &timed,
        ))
    }

    #[getter]
    fn quality(&self) -> &'static str {
        cached!(self, quality, self.inner.quality().as_str())
    }

    // ---- members ---------------------------------------------------------

    /// music21's `_overrides`: the answers fixed by hand, as a live
    /// dictionary. Writing `_overrides['root'] = None` is how music21's own
    /// doctests say a chord has no root.
    #[getter]
    fn _overrides(&mut self, py: Python<'_>) -> Py<PyDict> {
        let overrides = self
            .overrides
            .get_or_insert_with(|| PyDict::new(py).unbind());
        overrides.clone_ref(py)
    }

    #[pyo3(signature = (newroot = None, *, find = None))]
    fn root(
        slf: &Bound<'_, Self>,
        newroot: Option<&Bound<'_, PyAny>>,
        find: Option<bool>,
    ) -> PyResult<Option<Py<Pitch>>> {
        // An answer fixed by hand in `_overrides` wins, `None` included.
        if newroot.is_none_or(|value| value.is_none())
            && find != Some(true)
            && let Some(fixed) = overridden(slf, "root")?
        {
            if fixed.is_none() {
                return Ok(None);
            }
            let pitch = pitch_from_any(&fixed)?;
            return Self::own_pitch(slf, Some(&pitch));
        }
        let found = {
            let mut me = slf.borrow_mut();
            if let Some(newroot) = newroot.filter(|value| !value.is_none()) {
                let root = pitch_from_any(newroot)?;
                me.inner.set_root(Some(root.clone()));
                me.clear_cache();
                drop(me);
                // music21's own setter writes the pitch it was given into
                // `_overrides`, and its `harmony` module reads it back from
                // there — the root of a chord symbol is the root it was
                // named with, not one read off the notes.
                override_with(slf, "root", &root)?;
                return Ok(None);
            }
            match find {
                // `find=True` throws away any override and runs the search
                // again.
                Some(true) => {
                    me.inner.set_root(None);
                    me.clear_cache();
                    drop(me);
                    clear_override(slf, "root")?;
                    slf.borrow_mut().found_root()?
                }
                // `find=False` asks only whether a root was ever set, and
                // never runs the search — which is how a caller tells an
                // overridden root from an inferred one.
                Some(false) => me.inner.overridden_root().cloned(),
                None => match me.inner.overridden_root() {
                    Some(root) => Some(root.clone()),
                    None => me.found_root()?,
                },
            }
        };
        Self::own_pitch(slf, found.as_ref())
    }

    #[pyo3(signature = (newbass = None, *, find = None, allow_add = false))]
    fn bass(
        slf: &Bound<'_, Self>,
        newbass: Option<&Bound<'_, PyAny>>,
        find: Option<bool>,
        allow_add: bool,
    ) -> PyResult<Option<Py<Pitch>>> {
        let py = slf.py();
        if let Some(newbass) = newbass.filter(|value| !value.is_none()) {
            let bass = pitch_from_any(newbass)?;
            let known = slf
                .borrow()
                .inner
                .pitches()
                .iter()
                .any(|pitch| pitch.name() == bass.name());
            if !known {
                if !allow_add {
                    return Err(ChordException::new_err(format!(
                        "Pitch {} not found in chord",
                        bass.name_with_octave()
                    )));
                }
                // music21 puts a bass the chord does not have in front of
                // the notes it does: that is how a chord symbol names a bass
                // outside the chord.
                let mut pitches = vec![bass.clone()];
                pitches.extend(slf.borrow().inner.pitches().iter().cloned());
                let mut rebuilt = RsChord::new(pitches.as_slice()).map_err(chord_error)?;
                {
                    let me = slf.borrow();
                    if let Some(duration) = me.inner.duration().cloned() {
                        rebuilt.set_duration(duration);
                    }
                    // The chord is being rebuilt around a note it did not
                    // have; a root somebody fixed by hand is still that root.
                    if let Some(root) = me.inner.overridden_root().cloned() {
                        rebuilt.set_root(Some(root));
                    }
                }
                slf.borrow_mut().replace_inner(py, rebuilt)?;
            }
            {
                let mut me = slf.borrow_mut();
                me.inner.set_bass(Some(bass.clone()));
                me.clear_cache();
            }
            override_with(slf, "bass", &bass)?;
            return Ok(None);
        }
        // An answer fixed by hand wins, as it does for the root: music21
        // keeps both in the same dictionary and reads them back from there.
        if find != Some(true)
            && let Some(fixed) = overridden(slf, "bass")?
        {
            if fixed.is_none() {
                return Ok(None);
            }
            let pitch = pitch_from_any(&fixed)?;
            return Self::own_pitch(slf, Some(&pitch));
        }
        let found = {
            let mut me = slf.borrow_mut();
            match find {
                Some(true) => {
                    me.inner.set_bass(None);
                    me.clear_cache();
                    drop(me);
                    clear_override(slf, "bass")?;
                    slf.borrow().inner.found_bass().cloned()
                }
                Some(false) => me.inner.overridden_bass().cloned(),
                None => me.inner.bass().cloned(),
            }
        };
        Self::own_pitch(slf, found.as_ref())
    }

    /// music21's `_findBass`: the lowest written pitch, with no regard for
    /// any bass a caller has set. `bass()` is the way to ask; this is here
    /// because music21's own docstring for it is one of the ones we run.
    fn _findBass(slf: &Bound<'_, Self>) -> PyResult<Option<Py<Pitch>>> {
        let found = slf.borrow().inner.found_bass().cloned();
        Self::own_pitch(slf, found.as_ref())
    }

    #[getter]
    fn third(slf: &Bound<'_, Self>) -> PyResult<Option<Py<Pitch>>> {
        let found = slf.borrow().inner.third().cloned();
        Self::own_pitch(slf, found.as_ref())
    }

    #[getter]
    fn fifth(slf: &Bound<'_, Self>) -> PyResult<Option<Py<Pitch>>> {
        let found = slf.borrow().inner.fifth().cloned();
        Self::own_pitch(slf, found.as_ref())
    }

    #[getter]
    fn seventh(slf: &Bound<'_, Self>) -> PyResult<Option<Py<Pitch>>> {
        let found = slf.borrow().inner.seventh().cloned();
        Self::own_pitch(slf, found.as_ref())
    }

    #[pyo3(signature = (chordStep, testRoot = None))]
    fn getChordStep(
        slf: &Bound<'_, Self>,
        chordStep: u8,
        testRoot: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Option<Py<Pitch>>> {
        let found = {
            let me = slf.borrow();
            match testRoot.filter(|value| !value.is_none()) {
                Some(testRoot) => {
                    let root = pitch_from_any(testRoot)?;
                    me.inner.chord_step_with_root(chordStep, &root).cloned()
                }
                None => {
                    drop(me);
                    if Self::root(slf, None, None)?.is_none() {
                        return Err(ChordException::new_err(
                            "Cannot run getChordStep without a root",
                        ));
                    }
                    let me = slf.borrow();
                    me.inner.chord_step(chordStep).cloned()
                }
            }
        };
        Self::own_pitch(slf, found.as_ref())
    }

    #[pyo3(signature = (chordStep, testRoot = None))]
    fn semitonesFromChordStep(
        &self,
        chordStep: u8,
        testRoot: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Option<u8>> {
        match testRoot.filter(|value| !value.is_none()) {
            Some(testRoot) => {
                let root = pitch_from_any(testRoot)?;
                Ok(self
                    .inner
                    .semitones_from_chord_step_with_root(chordStep, &root))
            }
            None => Ok(self.inner.semitones_from_chord_step(chordStep)),
        }
    }

    fn isSeventhOfType(&self, intervalArray: Vec<u32>) -> bool {
        let array: Vec<u8> = intervalArray.iter().map(|value| *value as u8).collect();
        self.inner.is_seventh_of_type(&array)
    }

    /// music21's `formatVectorString`, which is a static method there and
    /// works on any list of pitch classes.
    #[staticmethod]
    fn formatVectorString(vectorList: Vec<u32>) -> String {
        let values: Vec<u8> = vectorList.iter().map(|value| *value as u8).collect();
        music21_rs::format_vector_string(&values)
    }

    fn intervalFromChordStep(&self, chordStep: u8) -> Option<crate::interval::Interval> {
        self.inner
            .interval_from_chord_step(chordStep)
            .map(crate::interval::Interval::wrap)
    }

    fn hasRepeatedChordStep(&self, chordStep: u8) -> bool {
        self.inner.has_repeated_chord_step(chordStep)
    }

    fn containsTriad(&self) -> bool {
        self.inner.contains_triad()
    }

    fn containsSeventh(&self) -> bool {
        self.inner.contains_seventh()
    }

    fn hasAnyRepeatedDiatonicNote(&self) -> bool {
        self.inner.has_any_repeated_diatonic_note()
    }

    fn hasAnyEnharmonicSpelledPitches(&self) -> bool {
        self.inner.has_any_enharmonic_spelled_pitches()
    }

    // ---- inversion -------------------------------------------------------

    #[pyo3(signature = (newInversion = None, *, find = true, testRoot = None, transposeOnSet = true))]
    fn inversion(
        slf: &Bound<'_, Self>,
        newInversion: Option<&Bound<'_, PyAny>>,
        find: bool,
        testRoot: Option<&Bound<'_, PyAny>>,
        transposeOnSet: bool,
    ) -> PyResult<Option<i32>> {
        if slf.borrow().inner.pitches().is_empty() {
            return Ok(Some(-1));
        }
        if let Some(newInversion) = newInversion.filter(|value| !value.is_none()) {
            let Ok(inversion) = newInversion.extract::<i32>() else {
                return Err(ChordException::new_err(format!(
                    "Inversion must be an integer, got: {}",
                    newInversion.get_type()
                )));
            };
            if !transposeOnSet {
                // music21 records the answer without moving anything, which
                // is how a chord badly spelt or with a note added is told
                // what inversion it stands in.
                let py = slf.py();
                let overrides = slf.borrow_mut()._overrides(py);
                overrides.bind(py).set_item("inversion", inversion)?;
                return Ok(None);
            }
            let inversion = u8::try_from(inversion).map_err(|_| {
                ChordException::new_err("Could not invert chord: inversion may not exist")
            })?;
            clear_override(slf, "inversion")?;
            clear_override(slf, "bass")?;
            slf.borrow_mut()
                .inner
                .set_inversion(inversion)
                .map_err(chord_error)?;
            return Ok(None);
        }
        if let Some(testRoot) = testRoot.filter(|value| !value.is_none()) {
            let root = pitch_from_any(testRoot)?;
            return Ok(Some(
                slf.borrow()
                    .inner
                    .inversion_from_root(&root)
                    .map_or(-1, i32::from),
            ));
        }
        // An answer fixed by hand stands, unless the caller asks for the
        // search outright.
        if !find && let Some(fixed) = overridden(slf, "inversion")? {
            return Ok(Some(fixed.extract::<i32>().unwrap_or(-1)));
        }
        Ok(Some(slf.borrow().inner.inversion().map_or(-1, i32::from)))
    }

    fn inversionName(&self) -> PyResult<Option<i32>> {
        self.inner.inversion_name().map_err(chord_error)
    }

    fn inversionText(&self) -> String {
        self.inner.inversion_text()
    }

    // ---- set theory ------------------------------------------------------

    #[getter]
    fn normalOrder(&self) -> Vec<u32> {
        cached!(self, normal_order, into_numbers(self.inner.normal_order()))
    }

    #[getter]
    fn normalOrderString(&self) -> String {
        self.inner.normal_order_string()
    }

    #[getter]
    fn primeForm(&self) -> Vec<u32> {
        cached!(self, prime_form, into_numbers(self.inner.prime_form()))
    }

    #[getter]
    fn primeFormString(&self) -> String {
        cached!(self, prime_form_string, self.inner.prime_form_string())
    }

    #[getter]
    fn intervalVector(&self) -> Vec<u32> {
        cached!(
            self,
            interval_vector,
            self.inner
                .interval_class_vector()
                .map(into_numbers)
                .unwrap_or_else(|| vec![0; 6])
        )
    }

    #[getter]
    fn intervalVectorString(&self) -> String {
        cached!(
            self,
            interval_vector_string,
            self.inner.interval_vector_string()
        )
    }

    #[getter]
    fn forteClass(&self) -> String {
        cached!(
            self,
            forte_class,
            self.inner
                .forte_class()
                .unwrap_or_else(|| "N/A".to_string())
        )
    }

    #[getter]
    fn forteClassNumber(&self) -> Option<u8> {
        self.inner.forte_class_number()
    }

    #[getter]
    fn forteClassTn(&self) -> String {
        self.inner
            .forte_class_tn()
            .unwrap_or_else(|| "N/A".to_string())
    }

    #[getter]
    fn forteClassTnI(&self) -> String {
        self.inner
            .forte_class_tni()
            .unwrap_or_else(|| "N/A".to_string())
    }

    fn geometricNormalForm(&self) -> Vec<u32> {
        into_numbers(self.inner.geometric_normal_form())
    }

    #[getter]
    fn hasZRelation(&self) -> bool {
        self.inner.has_z_relation()
    }

    fn getZRelation(&self, py: Python<'_>) -> PyResult<Option<Chord>> {
        match self
            .inner
            .z_relation()
            .and_then(|name| RsChord::from_forte_class(&name).ok())
        {
            Some(related) => Ok(Some(Self::from_inner(py, related)?)),
            None => Ok(None),
        }
    }

    fn areZRelations(&self, other: &Chord) -> bool {
        self.inner.are_z_relations(&other.inner)
    }

    #[getter]
    fn isPrimeFormInversion(&self) -> bool {
        self.inner.is_prime_form_inversion()
    }

    #[pyo3(signature = (*, requireIntervallicEvenness = false))]
    fn isTranspositionallySymmetrical(&self, requireIntervallicEvenness: bool) -> bool {
        self.inner
            .is_transpositionally_symmetrical(requireIntervallicEvenness)
    }

    // ---- predicates ------------------------------------------------------

    fn isTriad(&self) -> bool {
        cached_predicate!(self, ISTRIAD, self.inner.is_triad())
    }

    fn isSeventh(&self) -> bool {
        cached_predicate!(self, ISSEVENTH, self.inner.is_seventh())
    }

    fn isMajorTriad(&self) -> bool {
        cached_predicate!(self, ISMAJORTRIAD, self.inner.is_major_triad())
    }

    fn isMinorTriad(&self) -> bool {
        cached_predicate!(self, ISMINORTRIAD, self.inner.is_minor_triad())
    }

    fn isDiminishedTriad(&self) -> bool {
        cached_predicate!(self, ISDIMINISHEDTRIAD, self.inner.is_diminished_triad())
    }

    fn isAugmentedTriad(&self) -> bool {
        cached_predicate!(self, ISAUGMENTEDTRIAD, self.inner.is_augmented_triad())
    }

    fn isDominantSeventh(&self) -> bool {
        cached_predicate!(self, ISDOMINANTSEVENTH, self.inner.is_dominant_seventh())
    }

    fn isDiminishedSeventh(&self) -> bool {
        cached_predicate!(
            self,
            ISDIMINISHEDSEVENTH,
            self.inner.is_diminished_seventh()
        )
    }

    fn isHalfDiminishedSeventh(&self) -> bool {
        cached_predicate!(
            self,
            ISHALFDIMINISHEDSEVENTH,
            self.inner.is_half_diminished_seventh()
        )
    }

    fn isFalseDiminishedSeventh(&self) -> bool {
        cached_predicate!(
            self,
            ISFALSEDIMINISHEDSEVENTH,
            self.inner.is_false_diminished_seventh()
        )
    }

    fn isIncompleteMajorTriad(&self) -> bool {
        cached_predicate!(
            self,
            ISINCOMPLETEMAJORTRIAD,
            self.inner.is_incomplete_major_triad()
        )
    }

    fn isIncompleteMinorTriad(&self) -> bool {
        cached_predicate!(
            self,
            ISINCOMPLETEMINORTRIAD,
            self.inner.is_incomplete_minor_triad()
        )
    }

    fn isConsonant(&self) -> bool {
        cached_predicate!(self, ISCONSONANT, self.inner.is_consonant())
    }

    fn isNinth(&self) -> bool {
        cached_predicate!(self, ISNINTH, self.inner.is_ninth())
    }

    #[pyo3(signature = (*, permitAnyInversion = false))]
    fn isAugmentedSixth(&self, permitAnyInversion: bool) -> bool {
        self.inner.is_augmented_sixth(permitAnyInversion)
    }

    #[pyo3(signature = (*, restrictDoublings = false, permitAnyInversion = false))]
    fn isItalianAugmentedSixth(&self, restrictDoublings: bool, permitAnyInversion: bool) -> bool {
        self.inner
            .is_italian_augmented_sixth(permitAnyInversion, restrictDoublings)
    }

    #[pyo3(signature = (*, permitAnyInversion = false))]
    fn isFrenchAugmentedSixth(&self, permitAnyInversion: bool) -> bool {
        self.inner.is_french_augmented_sixth(permitAnyInversion)
    }

    #[pyo3(signature = (*, permitAnyInversion = false))]
    fn isGermanAugmentedSixth(&self, permitAnyInversion: bool) -> bool {
        self.inner.is_german_augmented_sixth(permitAnyInversion)
    }

    #[pyo3(signature = (*, permitAnyInversion = false))]
    fn isSwissAugmentedSixth(&self, permitAnyInversion: bool) -> bool {
        self.inner.is_swiss_augmented_sixth(permitAnyInversion)
    }

    fn canBeDominantV(&self) -> bool {
        self.inner.can_be_dominant_v()
    }

    fn canBeTonic(&self) -> bool {
        self.inner.can_be_tonic()
    }

    #[getter]
    fn isRest(&self) -> bool {
        false
    }

    #[getter]
    fn isChord(&self) -> bool {
        true
    }

    #[getter]
    fn isNote(&self) -> bool {
        false
    }

    // ---- reshaping -------------------------------------------------------

    #[pyo3(signature = (forceOctave = None, *, inPlace = false, leaveRedundantPitches = false))]
    fn closedPosition(
        &mut self,
        py: Python<'_>,
        forceOctave: Option<i32>,
        inPlace: bool,
        leaveRedundantPitches: bool,
    ) -> PyResult<Option<Chord>> {
        let closed = self
            .inner
            .closed_position(forceOctave, leaveRedundantPitches);
        if inPlace {
            self.replace_inner(py, closed)?;
            Ok(None)
        } else {
            Ok(Some(Self::from_inner(py, closed)?))
        }
    }

    #[pyo3(signature = (forceOctave = None, *, inPlace = false, leaveRedundantPitches = false))]
    fn semiClosedPosition(
        &mut self,
        py: Python<'_>,
        forceOctave: Option<i32>,
        inPlace: bool,
        leaveRedundantPitches: bool,
    ) -> PyResult<Option<Chord>> {
        let moved = self
            .inner
            .semi_closed_position(forceOctave, leaveRedundantPitches);
        if inPlace {
            self.replace_inner(py, moved)?;
            Ok(None)
        } else {
            Ok(Some(Self::from_inner(py, moved)?))
        }
    }

    #[pyo3(signature = (*, inPlace = false))]
    fn removeRedundantPitches(
        &mut self,
        py: Python<'_>,
        inPlace: bool,
    ) -> PyResult<Option<Py<PyAny>>> {
        let (reduced, removed) = self.inner.remove_redundant_pitches_reporting();
        self.deliver_reduced(py, reduced, removed, inPlace)
    }

    #[pyo3(signature = (*, inPlace = false))]
    fn removeRedundantPitchNames(
        &mut self,
        py: Python<'_>,
        inPlace: bool,
    ) -> PyResult<Option<Py<PyAny>>> {
        let (reduced, removed) = self.inner.remove_redundant_pitch_names_reporting();
        self.deliver_reduced(py, reduced, removed, inPlace)
    }

    #[pyo3(signature = (*, inPlace = false))]
    fn removeRedundantPitchClasses(
        &mut self,
        py: Python<'_>,
        inPlace: bool,
    ) -> PyResult<Option<Py<PyAny>>> {
        let (reduced, removed) = self.inner.remove_redundant_pitch_classes_reporting();
        self.deliver_reduced(py, reduced, removed, inPlace)
    }

    #[pyo3(signature = (*, inPlace = false))]
    fn sortAscending(&mut self, py: Python<'_>, inPlace: bool) -> PyResult<Option<Chord>> {
        let sorted = self.inner.sort_ascending();
        if inPlace {
            self.replace_inner(py, sorted)?;
            Ok(None)
        } else {
            Ok(Some(Self::from_inner(py, sorted)?))
        }
    }

    #[pyo3(signature = (*, inPlace = false))]
    fn sortDiatonicAscending(&mut self, py: Python<'_>, inPlace: bool) -> PyResult<Option<Chord>> {
        let sorted = self.inner.sort_diatonic_ascending();
        if inPlace {
            self.replace_inner(py, sorted)?;
            Ok(None)
        } else {
            Ok(Some(Self::from_inner(py, sorted)?))
        }
    }

    #[pyo3(signature = (*, inPlace = false))]
    fn sortChromaticAscending(&mut self, py: Python<'_>, inPlace: bool) -> PyResult<Option<Chord>> {
        let sorted = self.inner.sort_chromatic_ascending();
        if inPlace {
            self.replace_inner(py, sorted)?;
            Ok(None)
        } else {
            Ok(Some(Self::from_inner(py, sorted)?))
        }
    }

    #[pyo3(signature = (*, inPlace = false))]
    fn sortFrequencyAscending(&mut self, py: Python<'_>, inPlace: bool) -> PyResult<Option<Chord>> {
        let sorted = self.inner.sort_frequency_ascending();
        if inPlace {
            self.replace_inner(py, sorted)?;
            Ok(None)
        } else {
            Ok(Some(Self::from_inner(py, sorted)?))
        }
    }

    /// music21's `GeneralNote.augmentOrDiminish`.
    #[pyo3(signature = (scalar, *, inPlace = false))]
    fn augmentOrDiminish<'py>(
        slf: &Bound<'py, Self>,
        scalar: f64,
        inPlace: bool,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        augment_or_diminish_note(slf.as_any(), scalar, inPlace)
    }

    /// music21's `NotRest.getInstrument`.
    #[pyo3(signature = (*, returnDefault = true))]
    fn getInstrument<'py>(
        slf: &Bound<'py, Self>,
        returnDefault: bool,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_for_note(slf.as_any(), returnDefault)
    }

    /// music21's `storedInstrument`: the instrument this chord is played on,
    /// when it is not simply the one the part is written for.
    #[getter]
    fn get_storedInstrument(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.stored_instrument
            .as_ref()
            .map(|instrument| instrument.clone_ref(py))
    }

    #[setter]
    fn set_storedInstrument(&mut self, value: Option<&Bound<'_, PyAny>>) {
        self.stored_instrument = value
            .filter(|value| !value.is_none())
            .map(|value| value.clone().unbind());
    }

    /// music21's `GeneralNote.getGrace`.
    #[pyo3(signature = (*, appoggiatura = false, inPlace = false))]
    fn getGrace<'py>(
        slf: &Bound<'py, Self>,
        appoggiatura: bool,
        inPlace: bool,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        grace_note(slf.as_any(), appoggiatura, inPlace)
    }

    #[pyo3(signature = (value, *, inPlace = false))]
    fn transpose(
        slf: &Bound<'_, Self>,
        value: &Bound<'_, PyAny>,
        inPlace: bool,
    ) -> PyResult<Option<Py<PyAny>>> {
        let py = slf.py();
        let interval: RsInterval = interval_from_any(value)?;
        // From the chord as its notes now stand: a tie written on one of
        // them lives in the object until something reads it as a value, and
        // a transposed chord that had lost its ties could not be stripped.
        let moved = slf
            .borrow()
            .synced_inner(py)
            .transpose(&interval)
            .map_err(chord_error)?;
        let overrides = moved_overrides(py, slf.borrow().overrides.as_ref(), value)?;
        if inPlace {
            let mut chord = slf.borrow_mut();
            chord.replace_inner(py, moved)?;
            chord.overrides = overrides;
            return Ok(None);
        }
        let mut built = Self::from_inner(py, moved)?;
        built.overrides = overrides;
        let copy = crate::copy_as_same_type(slf, built)?;
        crate::derived_from(&copy, slf.as_any(), "transpose")?;
        Ok(Some(copy.unbind()))
    }

    /// music21's `chordTablesAddress`: where this chord's set class sits in
    /// the Forte tables.
    #[getter]
    fn chordTablesAddress(&self) -> ChordTableAddress {
        // The Forte address is what the prime form, the interval vector and
        // every Forte-class member are read off, so caching it is most of
        // what caching those would buy.
        if let Some(inner) = self.cache().chord_tables_address {
            return ChordTableAddress { inner };
        }
        let inner = self.inner.chord_tables_address_entry();
        self.cache().chord_tables_address = Some(inner);
        ChordTableAddress { inner }
    }

    /// music21's `scaleDegrees`: what degree of the key each pitch is, and
    /// how it is altered from that degree.
    ///
    /// The key comes from the chord's own `key` when it has one — a roman
    /// numeral does — and otherwise from the nearest one in the streams the
    /// chord sits in, which is where music21 looks for it. A chord in no
    /// stream and with no key of its own has no answer, as upstream.
    #[getter]
    fn scaleDegrees(slf: &Bound<'_, Self>) -> PyResult<Option<Vec<ScaleDegree>>> {
        let Some(key) = chord_key_in_force(slf)? else {
            return Ok(None);
        };
        let Ok(scale) = key.as_scale() else {
            return Ok(None);
        };
        let degrees = slf
            .borrow()
            .inner
            .scale_degrees(&scale)
            .map_err(chord_error)?;
        Ok(Some(
            degrees
                .into_iter()
                .map(|(degree, accidental)| (degree, accidental.map(Accidental::from_inner)))
                .collect(),
        ))
    }

    /// music21's `notehead`: the shape the chord as a whole is drawn with,
    /// which a chord has in its own right and not only through its notes.
    #[getter]
    fn get_notehead(&self) -> &'static str {
        self.inner.notehead().as_str()
    }

    #[setter]
    fn set_notehead(&mut self, value: &str) -> PyResult<()> {
        self.inner
            .set_notehead(RsNotehead::from_name(value).map_err(chord_error)?);
        Ok(())
    }

    /// music21's `noteheadFill`.
    #[getter]
    fn get_noteheadFill(&self) -> Option<bool> {
        self.inner.notehead_fill()
    }

    #[setter]
    fn set_noteheadFill(&mut self, value: Option<bool>) {
        self.inner.set_notehead_fill(value);
    }

    /// music21's `noteheadParenthesis`.
    #[getter]
    fn get_noteheadParenthesis(&self) -> bool {
        self.inner.notehead_parenthesis()
    }

    #[setter]
    fn set_noteheadParenthesis(&mut self, value: bool) {
        self.inner.set_notehead_parenthesis(value);
    }

    /// music21's `stemDirection`: which way the chord's own stem points.
    #[getter]
    fn get_stemDirection(&self) -> &'static str {
        self.inner.stem_direction().as_str()
    }

    #[setter]
    fn set_stemDirection(&mut self, value: &str) -> PyResult<()> {
        self.inner
            .set_stem_direction(RsStemDirection::from_name(value).map_err(chord_error)?);
        Ok(())
    }

    /// music21's `beams`: the beams joining the chord's flags to its
    /// neighbours'.
    #[getter]
    fn get_beams(slf: &Bound<'_, Self>) -> PyResult<Py<Beams>> {
        let py = slf.py();
        if let Some(beams) = &slf.borrow().beams {
            return Ok(beams.clone_ref(py));
        }
        let beams = crate::installed_new(
            py,
            "music21.beam",
            "Beams",
            Beams::wrap(slf.borrow().inner.beams().clone()),
        )?;
        slf.borrow_mut().beams = Some(beams.clone_ref(py));
        Ok(beams)
    }

    /// Setting them keeps the object given, as music21 does.
    #[setter]
    fn set_beams(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let beams = value.extract::<Py<Beams>>()?;
        self.inner.set_beams(beams.borrow(py).inner.clone());
        self.beams = Some(beams);
        Ok(())
    }

    // ---- notation --------------------------------------------------------

    /// music21's `style`: the object saying how this is drawn, made on
    /// first asking and the same one after that. It is music21's own — the
    /// page is not something this crate models — with the colour, which it
    /// does model, written into it.
    #[getter]
    fn get_style(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        if let Some(style) = &slf.borrow().style {
            return Ok(style.clone_ref(py));
        }
        let colour = slf.borrow().inner.color().map(str::to_string);
        let style = crate::notation::new_style(slf.as_any(), colour.as_deref())?;
        slf.borrow_mut().style = Some(style.clone_ref(py));
        Ok(style)
    }

    #[setter]
    fn set_style(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let colour: Option<String> = value
            .getattr("color")
            .ok()
            .and_then(|colour| colour.extract().ok());
        slf.borrow_mut().inner.set_color(colour);
        slf.borrow_mut().style = Some(value.clone().unbind());
        Ok(())
    }

    /// music21's `hasStyleInformation`: whether a style object has been made
    /// for this yet, which is what its own code asks before making one.
    #[getter]
    fn hasStyleInformation(&self) -> bool {
        self.style.is_some()
    }

    /// music21's `getColor`: the note's own colour when it has one, and the
    /// chord's otherwise.
    fn getColor(&self, py: Python<'_>, pitchTarget: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
        let note = self.note_object(py, pitchTarget)?;
        let color = note.borrow(py).colour(py);
        Ok(color.or_else(|| self.colour(py)))
    }

    #[pyo3(signature = (value, pitchTarget = None))]
    fn setColor(
        &mut self,
        py: Python<'_>,
        value: Option<String>,
        pitchTarget: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        match pitchTarget.filter(|target| !target.is_none()) {
            None => {
                self.inner.set_color(value);
                Ok(())
            }
            Some(target) => {
                let note = self.note_object(py, target)?;
                note.borrow_mut(py).inner.set_color(value);
                Ok(())
            }
        }
    }

    fn getNotehead(&self, py: Python<'_>, p: &Bound<'_, PyAny>) -> PyResult<&'static str> {
        let note = self.note_object(py, p)?;
        let notehead = note.borrow(py).inner.notehead();
        Ok(notehead.as_str())
    }

    #[pyo3(signature = (nh, pitchTarget = None))]
    fn setNotehead(
        &mut self,
        py: Python<'_>,
        nh: Option<&str>,
        pitchTarget: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let target = self.first_or_named(py, pitchTarget)?;
        target.borrow_mut(py).set_notehead(nh)
    }

    fn getNoteheadFill(&self, py: Python<'_>, p: &Bound<'_, PyAny>) -> PyResult<Option<bool>> {
        let note = self.note_object(py, p)?;
        let fill = note.borrow(py).inner.notehead_fill();
        Ok(fill)
    }

    #[pyo3(signature = (nh, pitchTarget = None))]
    fn setNoteheadFill(
        &mut self,
        py: Python<'_>,
        nh: &Bound<'_, PyAny>,
        pitchTarget: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let target = self.first_or_named(py, pitchTarget)?;
        target.borrow_mut(py).set_noteheadFill(nh)
    }

    fn getStemDirection(&self, py: Python<'_>, p: &Bound<'_, PyAny>) -> PyResult<&'static str> {
        let note = self.note_object(py, p)?;
        let direction = note.borrow(py).inner.stem_direction();
        Ok(direction.as_str())
    }

    #[pyo3(signature = (stemDirection, pitchTarget = None))]
    fn setStemDirection(
        &mut self,
        py: Python<'_>,
        stemDirection: Option<&str>,
        pitchTarget: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let target = self.first_or_named(py, pitchTarget)?;
        target.borrow_mut(py).set_stemDirection(stemDirection)
    }

    fn getTie(&self, py: Python<'_>, p: &Bound<'_, PyAny>) -> PyResult<Option<Tie>> {
        match self.note_object(py, p) {
            Ok(note) => {
                let tie = note.borrow(py).inner.tie().cloned().map(Tie::wrap);
                Ok(tie)
            }
            Err(_) => Ok(None),
        }
    }

    #[pyo3(signature = (tieObjOrStr, pitchTarget = None))]
    fn setTie(
        &mut self,
        py: Python<'_>,
        tieObjOrStr: &Bound<'_, PyAny>,
        pitchTarget: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let target = self.first_or_named(py, pitchTarget)?;
        crate::note::Note::set_tie(target.bind(py), Some(tieObjOrStr))
    }

    #[getter]
    fn get_tie(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Option<Py<Tie>>> {
        // The first note that carries one, as its own object: music21's own
        // note-splitting writes through the tie it reads back.
        for note in Chord::note_objects(slf) {
            if let Some(tie) = crate::note::Note::get_tie(note.bind(py))? {
                return Ok(Some(tie));
            }
        }
        Ok(None)
    }

    /// music21 gives every note the very tie object it was handed, so
    /// `id(chord.tie) == id(chord[0].tie)` after setting one.
    #[setter]
    fn set_tie(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        for note in Chord::note_objects(slf) {
            crate::note::Note::set_tie(note.bind(py), value)?;
        }
        Ok(())
    }

    fn getVolume(slf: &Bound<'_, Self>, p: &Bound<'_, PyAny>) -> PyResult<Py<Volume>> {
        let py = slf.py();
        let note = slf.borrow().note_object(py, p)?;
        // The note's own volume object, told that the chord is what it
        // belongs to: music21's `_getVolume(forceClient=self)`.
        let volume = crate::note::Note::get_volume(note.bind(py), py)?;
        volume.borrow_mut(py).set_client(Some(slf.as_any()));
        Ok(volume)
    }

    #[pyo3(signature = (vol, target = None))]
    fn setVolume(
        &mut self,
        py: Python<'_>,
        vol: &Bound<'_, PyAny>,
        target: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let note = self.first_or_named(py, target)?;
        let parsed = volume_from_any(vol)?;
        note.borrow_mut(py).replace_volume(Some(parsed));
        Ok(())
    }

    /// music21's `_volume`: the chord's own volume object, or nothing where
    /// it has none. Its own tests read the slot to tell a chord that has one
    /// from a chord whose notes have theirs.
    #[getter]
    fn _volume(&self, py: Python<'_>) -> Option<Py<Volume>> {
        self.volume.as_ref().map(|volume| volume.clone_ref(py))
    }

    #[setter]
    fn set__volume(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        Self::set_volume(slf, py, value)
    }

    /// Whether the chord carries a volume of its own. Volumes on its
    /// components do not count — music21 asks only whether `_volume` was
    /// ever set, which is why `setVolumes` leaves this false until something
    /// reads `.volume` and creates the averaged one.
    fn hasVolumeInformation(&self) -> bool {
        self.volume.is_some()
    }

    /// music21's `simplifyEnharmonics`: respells the chord so its pitches
    /// read as simply as they can.
    #[pyo3(signature = (*, inPlace = false, keyContext = None))]
    fn simplifyEnharmonics(
        &mut self,
        py: Python<'_>,
        inPlace: bool,
        keyContext: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Option<Chord>> {
        let key_context = match keyContext.filter(|value| !value.is_none()) {
            Some(value) => {
                let sharps: i32 = value.getattr("sharps")?.extract()?;
                Some(music21_rs::KeySignature::new(sharps))
            }
            None => None,
        };
        let simplified = self
            .inner
            .simplify_enharmonics(key_context)
            .map_err(chord_error)?;
        if inPlace {
            self.replace_inner(py, simplified)?;
            Ok(None)
        } else {
            Ok(Some(Self::from_inner(py, simplified)?))
        }
    }

    fn hasComponentVolumes(&self, py: Python<'_>) -> bool {
        self.notes
            .iter()
            .any(|note| note.borrow(py).inner.has_volume_information())
    }

    fn setVolumes(&mut self, py: Python<'_>, volumes: &Bound<'_, PyAny>) -> PyResult<()> {
        let mut parsed: Vec<RsVolume> = Vec::new();
        for item in volumes.try_iter()? {
            parsed.push(volume_from_any(&item?)?);
        }
        if parsed.is_empty() {
            return Err(ChordException::new_err(
                "setVolumes needs at least one volume",
            ));
        }
        self.volume = None;
        for (index, note) in self.notes.iter().enumerate() {
            note.borrow_mut(py)
                .replace_volume(Some(parsed[index % parsed.len()].clone()));
        }
        Ok(())
    }

    #[getter]
    fn get_volume(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<Volume>> {
        if let Some(volume) = &slf.borrow().volume {
            return Ok(volume.clone_ref(py));
        }
        let velocities: Vec<i32> = slf
            .borrow()
            .notes
            .iter()
            .filter_map(|note| note.borrow(py).inner.volume().velocity())
            .collect();
        let inner = if velocities.is_empty() {
            RsVolume::new()
        } else {
            let total: i32 = velocities.iter().sum();
            let mean = f64::from(total) / velocities.len() as f64;
            RsVolume::from_velocity(mean.round_ties_even() as i32)
        };
        // The volume knows whose it is, as a note's does: music21 reads a
        // volume's client to find the dynamic in force where it sounds.
        let created = crate::installed_new(
            py,
            "music21.volume",
            "Volume",
            Volume::owned_by(inner, slf.clone().into_any().unbind()),
        )?;
        slf.borrow_mut().volume = Some(created.clone_ref(py));
        Ok(created)
    }

    /// As on a note: the object given is kept when nothing else has claimed
    /// it and copied when something has, and either way the chord is what
    /// the volume is the volume of.
    #[setter]
    fn set_volume(slf: &Bound<'_, Self>, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        for note in &slf.borrow().notes {
            note.borrow_mut(py).inner.set_volume(None);
        }
        let object = match value.extract::<Py<Volume>>() {
            Ok(object) if object.borrow(py).is_claimed() => {
                crate::installed_new(py, "music21.volume", "Volume", object.borrow(py).clone())?
            }
            Ok(object) => object,
            Err(_) => crate::installed_new(
                py,
                "music21.volume",
                "Volume",
                Volume::wrap(volume_from_any(value)?),
            )?,
        };
        object.borrow_mut(py).set_client(Some(slf.as_any()));
        slf.borrow_mut().volume = Some(object);
        Ok(())
    }

    #[getter]
    fn get_lyrics<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        match self.notes.first() {
            Some(note) => crate::note::Note::get_lyrics(note.bind(py)),
            None => Ok(PyList::empty(py)),
        }
    }

    /// A chord is sung to one text, which its first note carries.
    #[setter]
    fn set_lyrics(&mut self, py: Python<'_>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        if let Some(note) = self.notes.first() {
            crate::note::Note::set_lyrics(note.bind(py), value)?;
        }
        Ok(())
    }

    /// music21's `expressions`: the ornaments written over this chord.
    #[getter]
    fn get_expressions(&mut self, py: Python<'_>) -> Py<PyList> {
        let list = self
            .expressions
            .get_or_insert_with(|| PyList::empty(py).unbind());
        list.clone_ref(py)
    }

    #[setter]
    fn set_expressions(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let list = PyList::empty(py);
        for item in value.try_iter()? {
            list.append(item?)?;
        }
        self.expressions = Some(list.unbind());
        Ok(())
    }

    /// music21's `articulations`: the marks written under this chord.
    #[getter]
    fn get_articulations(&mut self, py: Python<'_>) -> Py<PyList> {
        let list = self
            .articulations
            .get_or_insert_with(|| PyList::empty(py).unbind());
        list.clone_ref(py)
    }

    #[setter]
    fn set_articulations(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let list = PyList::empty(py);
        for item in value.try_iter()? {
            list.append(item?)?;
        }
        self.articulations = Some(list.unbind());
        Ok(())
    }

    #[getter]
    fn get_lyric(&self, py: Python<'_>) -> Option<String> {
        self.notes
            .first()
            .and_then(|note| note.borrow(py).synced(py).lyric())
    }

    #[setter]
    fn set_lyric(&mut self, py: Python<'_>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        match self.notes.first() {
            Some(note) => note.borrow_mut(py).set_lyric(py, value),
            None => Ok(()),
        }
    }

    /// music21's `insertLyric`, which a chord passes to its first note as it
    /// passes every other lyric question.
    #[pyo3(signature = (text, index = 0, *, applyRaw = false, identifier = None))]
    fn insertLyric(
        &mut self,
        py: Python<'_>,
        text: &Bound<'_, PyAny>,
        index: usize,
        applyRaw: bool,
        identifier: Option<String>,
    ) -> PyResult<()> {
        match self.notes.first() {
            Some(note) => note
                .borrow_mut(py)
                .insertLyric(text, index, applyRaw, identifier),
            None => Ok(()),
        }
    }

    #[pyo3(signature = (text, lyricNumber = None, *, applyRaw = false, lyricIdentifier = None))]
    fn addLyric(
        &mut self,
        py: Python<'_>,
        text: &Bound<'_, PyAny>,
        lyricNumber: Option<i32>,
        applyRaw: bool,
        lyricIdentifier: Option<String>,
    ) -> PyResult<()> {
        match self.notes.first() {
            Some(note) => {
                note.borrow_mut(py)
                    .addLyric(text, lyricNumber, applyRaw, lyricIdentifier)
            }
            None => Err(ChordException::new_err(
                "an empty chord has nothing to sing",
            )),
        }
    }

    /// music21's `annotateIntervals`: the interval from the lowest pitch up
    /// to each of the others, written on as lyrics.
    #[pyo3(signature = (*, inPlace = false, stripSpecifiers = true, sortPitches = true, returnList = false))]
    fn annotateIntervals(
        &mut self,
        py: Python<'_>,
        inPlace: bool,
        stripSpecifiers: bool,
        sortPitches: bool,
        returnList: bool,
    ) -> PyResult<Option<Py<PyAny>>> {
        let names = self
            .inner
            .annotate_intervals(stripSpecifiers, sortPitches)
            .map_err(chord_error)?;
        if returnList {
            return Ok(Some(PyList::new(py, names)?.into_any().unbind()));
        }
        if inPlace {
            for name in names {
                let text = name.into_pyobject(py)?;
                self.addLyric(py, text.as_any(), None, false, None)?;
            }
            return Ok(None);
        }
        let mut annotated = Self::from_inner(py, self.inner.clone())?;
        for name in names {
            let text = name.into_pyobject(py)?;
            annotated.addLyric(py, text.as_any(), None, false, None)?;
        }
        Ok(Some(Py::new(py, annotated)?.into_any()))
    }

    /// music21's `Pitch.getStringHarmonic`, which reads the notehead off the
    /// chord: a chord whose second note is a diamond sounds the harmonic its
    /// two pitches pick out.
    pub(crate) fn getStringHarmonic(&self, py: Python<'_>) -> PyResult<Option<Chord>> {
        let sounded = self
            .with_note_notation(py)?
            .string_harmonic()
            .map_err(chord_error)?;
        match sounded {
            Some(chord) => Ok(Some(Self::from_inner(py, chord)?)),
            None => Ok(None),
        }
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        let py = other.py();
        let Ok(other) = other.extract::<PyRef<Chord>>() else {
            return false;
        };
        if self.quarter_length(py) != other.quarter_length(py) {
            return false;
        }
        let mut ours: Vec<String> = self
            .inner
            .pitches()
            .iter()
            .map(RsPitch::name_with_octave)
            .collect();
        let mut theirs: Vec<String> = other
            .inner
            .pitches()
            .iter()
            .map(RsPitch::name_with_octave)
            .collect();
        ours.sort();
        theirs.sort();
        if ours != theirs {
            return false;
        }
        // As on a note: the ornaments over it and the marks under it count,
        // by their kinds.
        crate::note::same_kinds(py, self.expressions.as_ref(), other.expressions.as_ref())
            .unwrap_or(false)
            && crate::note::same_kinds(
                py,
                self.articulations.as_ref(),
                other.articulations.as_ref(),
            )
            .unwrap_or(false)
    }

    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        slf.as_ptr() as isize >> 4
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let borrowed = slf.borrow();
        let names: Vec<String> = borrowed
            .inner
            .pitches()
            .iter()
            .map(RsPitch::name_with_octave)
            .collect();
        Ok(format!(
            "<music21.chord.{} {}>",
            slf.get_type().qualname()?,
            names.join(" ")
        ))
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        py: Python<'py>,
        _memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        Self::__copy__(slf, py)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.borrow_mut().copied_value(py)?;
        crate::copy_as_same_type(slf, copied)
    }
}

impl Chord {
    /// Writes whatever Python has done to the beam objects into the value,
    /// which is where every other reader looks.
    fn settle_beams(&mut self, py: Python<'_>) {
        if let Some(beams) = &self.beams {
            self.inner.set_beams(beams.borrow_mut(py).settled_value(py));
        }
    }

    /// This chord as a fresh value, its notes, duration and volume copied
    /// rather than shared.
    pub(crate) fn copied_value(&mut self, py: Python<'_>) -> PyResult<Self> {
        self.settle_beams(py);
        let mut copied = Self::from_inner(py, self.inner.clone())?;
        // Each note is copied as it stands *now*, not as the chord's own
        // value last saw it: a lyric written on a note lives in the object
        // until something reads the note as a value, and a copy taken from
        // the stale value would not be sung to anything.
        for (target, source) in copied.notes.iter().zip(&self.notes) {
            target.borrow_mut(py).inner = source.borrow(py).synced(py);
        }
        if let Some(duration) = &self.duration {
            copied.duration = Some(crate::duration::copied_duration(py, duration)?);
        }
        if let Some(volume) = &self.volume {
            copied.volume = Some(crate::installed_new(
                py,
                "music21.volume",
                "Volume",
                volume.borrow(py).clone(),
            )?);
        }
        // The ornaments and the marks come across. A score is deep-copied on
        // its way out to a file, and an arpeggio the copy had lost would not
        // be written.
        copied.expressions = crate::note::copied_list(py, self.expressions.as_ref())?;
        copied.articulations = crate::note::copied_list(py, self.articulations.as_ref())?;
        copied.style = crate::notation::copied_style(py, self.style.as_ref());
        Ok(copied)
    }
}

/// music21's `fromForteClass`: the chord a Forte class name, or a
/// `(cardinality, number)` pair, stands for, in its prime form.
#[pyfunction]
#[pyo3(name = "fromForteClass")]
fn fromForteClass(py: Python<'_>, notation: &Bound<'_, PyAny>) -> PyResult<Chord> {
    let name = match notation.extract::<String>() {
        Ok(text) => text,
        Err(_) => {
            let parts: Vec<u8> = notation.extract().map_err(|_| {
                ChordException::new_err(
                    "cannot extract set-class representation from the given notation",
                )
            })?;
            let [cardinality, number] = parts[..] else {
                return Err(ChordException::new_err(
                    "a Forte class is a cardinality and a number",
                ));
            };
            format!("{cardinality}-{number}")
        }
    };
    Chord::from_inner(py, RsChord::from_forte_class(&name).map_err(chord_error)?)
}

/// music21's `fromIntervalVector`: the chord with this interval vector, its
/// Z-related partner with `getZRelation`, or `None` where no set class has
/// it.
#[pyfunction]
#[pyo3(name = "fromIntervalVector", signature = (vector, getZRelation = false))]
fn fromIntervalVector(
    py: Python<'_>,
    vector: Vec<u8>,
    getZRelation: bool,
) -> PyResult<Option<Chord>> {
    let vector: [u8; 6] = vector
        .try_into()
        .map_err(|_| ChordException::new_err("An interval vector has six entries"))?;
    RsChord::from_interval_vector(&vector, getZRelation)
        .map(|inner| Chord::from_inner(py, inner))
        .transpose()
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(fromForteClass, m)?)?;
    m.add_function(wrap_pyfunction!(fromIntervalVector, m)?)?;
    m.add_class::<Chord>()?;
    m.add_class::<ChordTableAddress>()?;
    Ok(())
}
