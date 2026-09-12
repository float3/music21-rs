//! music21's `note.Note` over `music21-rs`, enough of it for the chord
//! facade to hand real notes back and forth.

#![allow(non_snake_case)]

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

use music21_rs_crate::{
    Duration as RsDuration, KeySignature as RsKeySignature, Note as RsNote, Notehead as RsNotehead,
    Pitch as RsPitch, StemDirection as RsStemDirection,
};

use crate::interval::transpose_pitch_by_any;
use crate::notation::{Beams, Lyric, Tie, Volume, tie_from_any, volume_from_any};
use crate::pitch::{Pitch, pitch_from_any};

use crate::duration::{
    Duration, adopt_duration, copied_duration, duration_from_any, duration_value_of, op_frac,
    told_sites,
};

/// The names the `note` facade replaces in `music21.note`.
pub const NAMES: &[&str] = &[
    "Note",
    "NoteException",
    "NotRestException",
    "Lyric",
    "LyricException",
];

pyo3::create_exception!(music21_rs_facade, NoteException, crate::Music21Exception);
pyo3::create_exception!(music21_rs_facade, NotRestException, crate::Music21Exception);
/// A note or chord's name with the length written on the end taken from its
/// duration object rather than from its value.
///
/// The object is where an unlinked length lives: a grace note sounds for
/// nothing at all while still being written as a sixteenth, and music21
/// names it by what is written.
pub(crate) fn named_with_duration(holder: &Bound<'_, PyAny>, named: &str, value: &str) -> String {
    let Ok(written) = holder
        .getattr("duration")
        .and_then(|duration| duration.getattr("fullName"))
        .and_then(|name| name.extract::<String>())
    else {
        return named.to_string();
    };
    if written == value {
        return named.to_string();
    }
    named.replacen(value, &written, 1)
}

/// Whether two lists of ornaments or marks hold the same kinds of thing.
///
/// music21 compares them by class rather than by object, and by set as well
/// as by count, which is how a note that has been given a turn stops being
/// equal to the same note without one.
pub(crate) fn same_kinds(
    py: Python<'_>,
    mine: Option<&Py<PyList>>,
    theirs: Option<&Py<PyList>>,
) -> PyResult<bool> {
    let kinds = |list: Option<&Py<PyList>>| -> PyResult<Vec<Py<PyAny>>> {
        let Some(list) = list else {
            return Ok(Vec::new());
        };
        list.bind(py)
            .iter()
            .map(|item| Ok(item.get_type().into_any().unbind()))
            .collect()
    };
    let (mine, theirs) = (kinds(mine)?, kinds(theirs)?);
    if mine.len() != theirs.len() {
        return Ok(false);
    }
    let set = |kinds: Vec<Py<PyAny>>| -> PyResult<Bound<'_, PyAny>> {
        py.import("builtins")?.getattr("set")?.call1((kinds,))
    };
    set(mine)?.eq(set(theirs)?)
}

error_into!(pub(crate) note_error, NoteException);

// The error music21 raises out of the `NotRest` properties — notehead, its
// fill and parentheses, and stem direction — which is a different class
// from the one its `Note` methods raise.
error_into!(not_rest_error, NotRestException);

/// The key signature in force where this note sits, if it sits anywhere.
///
/// The search up the containing streams is music21's `getContextByClass`,
/// since it is music21 that holds the streams. A signature found this way is
/// as often music21's own class as ours, so it is read off its `sharps`.
fn key_signature_around(note: &Bound<'_, PyAny>) -> PyResult<Option<RsKeySignature>> {
    if !note.hasattr("getContextByClass")? {
        return Ok(None);
    }
    let found = note.call_method1("getContextByClass", ("KeySignature",))?;
    if found.is_none() {
        return Ok(None);
    }
    let Ok(sharps) = found.getattr("sharps").and_then(|s| s.extract::<i32>()) else {
        return Ok(None);
    };
    Ok(Some(RsKeySignature::new(sharps)))
}

/// music21's `NotRest.getInstrument`: the instrument stored on this note,
/// or the one in force where it sits.
///
/// The search up the containing streams is music21's, as is the default
/// instrument it falls back to; the crate models neither streams nor
/// instruments. Written against the Python object because music21 writes it
/// once on `NotRest` and both a note and a chord inherit it.
pub(crate) fn instrument_for_note<'py>(
    note: &Bound<'py, PyAny>,
    return_default: bool,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let stored = note.getattr("storedInstrument")?;
    if !stored.is_none() {
        return Ok(Some(stored));
    }
    let py = note.py();
    let Ok(instrument) = py.import("music21.instrument") else {
        return Ok(None);
    };
    let mut found = py.None().into_bound(py);
    if note.hasattr("getContextByClass")? {
        let keywords = PyDict::new(py);
        keywords.set_item("followDerivation", false)?;
        found = note.call_method(
            "getContextByClass",
            (instrument.getattr("Instrument")?,),
            Some(&keywords),
        )?;
    }
    if !found.is_none() {
        return Ok(Some(found));
    }
    if return_default {
        return Ok(Some(instrument.getattr("Instrument")?.call0()?));
    }
    Ok(None)
}

/// music21's `GeneralNote.augmentOrDiminish`: the same note with its length
/// scaled, in place or as a copy.
///
/// Written against the Python object rather than against either facade,
/// because music21 writes it once on `GeneralNote` and both a note and a
/// chord inherit it — and because scaling is entirely a question for the
/// duration, whichever of the two is holding it.
pub(crate) fn augment_or_diminish_note<'py>(
    note: &Bound<'py, PyAny>,
    scalar: f64,
    in_place: bool,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    if scalar <= 0.0 || scalar.is_nan() {
        return Err(NoteException::new_err("scalar must be greater than zero"));
    }
    let target = target_note(note, in_place)?;
    let scaled = target
        .getattr("duration")?
        .call_method1("augmentOrDiminish", (scalar,))?;
    target.setattr("duration", scaled)?;
    Ok(if in_place { None } else { Some(target) })
}

/// music21's `GeneralNote.getGrace`: the same note written as it was and
/// sounding nothing.
pub(crate) fn grace_note<'py>(
    note: &Bound<'py, PyAny>,
    appoggiatura: bool,
    in_place: bool,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let target = target_note(note, in_place)?;
    let grace = target
        .getattr("duration")?
        .call_method1("getGraceDuration", (appoggiatura,))?;
    target.setattr("duration", grace)?;
    Ok(if in_place { None } else { Some(target) })
}

/// The note the change lands on: this one, or a deep copy of it.
fn target_note<'py>(note: &Bound<'py, PyAny>, in_place: bool) -> PyResult<Bound<'py, PyAny>> {
    if in_place {
        return Ok(note.clone());
    }
    note.py()
        .import("copy")?
        .getattr("deepcopy")?
        .call1((note,))
}

/// A Python list holding whatever the value iterates over.
fn list_of(value: &Bound<'_, PyAny>) -> PyResult<Py<PyList>> {
    let list = PyList::empty(value.py());
    if !value.is_none() {
        for item in value.try_iter()? {
            list.append(item?)?;
        }
    }
    Ok(list.unbind())
}

/// Reads a duration argument: a `Duration`, a quarter length, or a type name
/// such as `"half"`.
/// The duration value an object stands for: one of ours as it stands, and
/// anything else — music21's own `GraceDuration`, say — by what it says its
/// length is.
/// music21's `True`, `False` or `None` and nothing else, read as a bool.
///
/// Its grace-note flags take those three and report anything else as a
/// `ValueError`, which is what its own tests catch — a plain bool argument
/// would raise pyo3's `TypeError` instead.
pub(crate) fn true_false_or_none(value: &Bound<'_, PyAny>) -> PyResult<Option<bool>> {
    if value.is_none() {
        return Ok(None);
    }
    if let Ok(said) = value.extract::<bool>()
        && value.is_instance_of::<pyo3::types::PyBool>()
    {
        return Ok(Some(said));
    }
    Err(PyValueError::new_err("expr must be True, False, or None"))
}

/// Copies of the objects a duration is written inside.
///
/// The tuplets are the duration's own, not something two copies may share:
/// music21's own notation code writes a bracket type into the tuplet object
/// it finds on a note, so two durations holding one tuplet would each be
/// written as whatever the other was. Copied through the copier's memo, so
/// that a tuplet two things really do share is still shared afterwards.
pub(crate) fn deep_copied_objects(
    py: Python<'_>,
    memo: &Bound<'_, PyAny>,
    objects: Option<Vec<Py<PyAny>>>,
) -> PyResult<Option<Vec<Py<PyAny>>>> {
    let Some(objects) = objects else {
        return Ok(None);
    };
    let deepcopy = py.import("copy")?.getattr("deepcopy")?;
    let mut copied = Vec::with_capacity(objects.len());
    for object in objects {
        let one = if memo.is_instance_of::<PyDict>() {
            deepcopy.call1((object.bind(py), memo))?
        } else {
            deepcopy.call1((object.bind(py),))?
        };
        copied.push(one.unbind());
    }
    Ok(Some(copied))
}

/// music21's `note.Note`: a pitch with a duration.
#[pyclass(name = "Note", module = "music21.note", subclass, skip_from_py_object)]
pub struct Note {
    pub(crate) inner: RsNote,
    /// The `Pitch` object music21 hands back from `.pitch`. It is the same
    /// object every time, so `chord.pitches[0].getEnharmonic(inPlace=True)`
    /// reaches the chord; `inner` mirrors whatever it holds.
    pitch: Py<Pitch>,
    /// The `Volume` object music21 hands back from `.volume`, once something
    /// has asked for one. music21 makes it on demand and reads its mere
    /// existence as `hasVolumeInformation`.
    volume: Option<Py<Volume>>,
    /// The `Duration` object music21 hands back from `.duration`, once
    /// something has asked for one. Notes a chord builds are given the
    /// chord's, which is what makes `chord.duration is chord[0].duration`
    /// hold; a note that came with its own keeps it.
    /// The `Duration` object music21 hands back from `.duration`, which may
    /// be one of music21's own subclasses of it — a `GraceDuration` is what
    /// makes a note a grace note, and one replaced by a plain duration of
    /// ours would stop being one.
    duration: Option<Py<PyAny>>,
    /// The chord this note is part of: music21's `_chordAttached`, which its
    /// own `ChordBase` sets on every note it takes in. An edit to the pitch
    /// has to reach the chord through it.
    chord: Option<Py<PyAny>>,
    /// music21's `expressions` and `articulations`: the ornaments written
    /// over the note and the marks written under it.
    ///
    /// The crate models neither, and the lists hold whatever objects a
    /// caller puts in them. They are kept because they are where music21
    /// looks — `makeAccidentals` walks the expressions of every note to see
    /// whether an ornament needs an accidental of its own — and a note with
    /// no such list is a note music21's own notation code cannot process.
    expressions: Option<Py<PyList>>,
    articulations: Option<Py<PyList>>,
    /// The `Tie` object music21 hands back from `.tie`, once something has
    /// asked for one.
    ///
    /// music21's own `splitAtQuarterLength` writes through it — the middle
    /// of a note split across three bars is turned from a stop into a
    /// continue by `e.tie.type = 'continue'` — so the object has to be the
    /// note's own and not one made afresh each time.
    tie: Option<Py<Tie>>,
    /// music21's `lyrics`, as the list object itself.
    ///
    /// Its own MusicXML reader appends each verse to what `n.lyrics` hands
    /// back, so a getter that built a fresh list every time dropped every
    /// word the score was sung to. Once the list exists it is what the note
    /// is sung to, and `synced` writes it into the value.
    lyrics: Option<Py<PyList>>,
    /// music21's `storedInstrument`: the instrument this one note is played
    /// on, when it is not simply the one the part is written for. The crate
    /// models no instruments, so whatever object a caller stores is kept as
    /// it was given.
    stored_instrument: Option<Py<PyAny>>,
    /// Whatever was assigned through `pitches` that was not a pitch.
    ///
    /// music21's `pitches` setter takes the first item of the sequence and
    /// stores it as the pitch without looking at it, so `n.pitches = ('C4',)`
    /// leaves a *string* where the pitch should be, and its own docstring
    /// says so: "Don't use strings, or you will get a string back!". The
    /// note goes on answering every musical question from the pitch it
    /// already had, which is what music21 does too — nothing there reads the
    /// stored value except `.pitch`.
    unread_pitch: Option<Py<PyAny>>,
    /// music21's `style`, once something has asked for one: the object
    /// saying how this is drawn. It is music21's own object — the page is
    /// not something this crate models — and its mere existence is what
    /// `hasStyleInformation` answers, as music21's does.
    style: Option<Py<PyAny>>,
    /// music21's `beams`, as the object itself.
    ///
    /// Its own MusicXML reader reads a note's beams *into* what `n.beams`
    /// hands back — `xmlToBeams(mxBeamList, inputM21=n.beams)` — so a getter
    /// that built a fresh object every time dropped every beam the score
    /// wrote. Once the object exists it is what the note's beams are, and
    /// `synced` writes it into the value.
    beams: Option<Py<Beams>>,
}

impl Note {
    /// Builds the facade around a note, giving its pitch a Python object of
    /// its own.
    pub(crate) fn wrap(py: Python<'_>, inner: RsNote) -> PyResult<Self> {
        // Whether the spelling was chosen or given is the pitch's own answer:
        // `note.Note(63)` spells an E flat that nobody asked for by name.
        let inferred = inner.pitch().spelling_is_inferred();
        let pitch = crate::installed_new(
            py,
            "music21.pitch",
            "Pitch",
            Pitch::wrap(inner.pitch().clone(), inferred),
        )?;
        Ok(Self {
            inner,
            pitch,
            volume: None,
            duration: None,
            chord: None,
            expressions: None,
            articulations: None,
            lyrics: None,
            tie: None,
            style: None,
            beams: None,
            stored_instrument: None,
            unread_pitch: None,
        })
    }

    /// A Python note object whose pitch already points back at it.
    pub(crate) fn object(py: Python<'_>, inner: RsNote) -> PyResult<Py<Self>> {
        let note = crate::installed_new(py, "music21.note", "Note", Self::wrap(py, inner)?)?;
        Self::claim_pitch(py, &note);
        Ok(note)
    }

    /// A note built around a pitch object the caller already holds, so that
    /// `chord.Chord([p1, p2]).pitches[0] is p1`, as music21's is.
    pub(crate) fn object_for_pitch(py: Python<'_>, pitch: Py<Pitch>) -> PyResult<Py<Self>> {
        let inner = RsNote::from_pitch(pitch.borrow(py).inner.clone());
        let note = crate::installed_new(
            py,
            "music21.note",
            "Note",
            Self {
                inner,
                pitch,
                volume: None,
                duration: None,
                chord: None,
                expressions: None,
                articulations: None,
                lyrics: None,
                tie: None,
                style: None,
                beams: None,
                stored_instrument: None,
                unread_pitch: None,
            },
        )?;
        Self::claim_pitch(py, &note);
        Ok(note)
    }

    /// Points the note's pitch object back at the note, so an edit through
    /// the pitch finds its way home.
    fn claim_pitch(py: Python<'_>, note: &Py<Self>) {
        let pitch = note.borrow(py).pitch.clone_ref(py);
        pitch.borrow_mut(py).owner = Some(note.clone_ref(py));
    }

    /// Takes the value a `Pitch` object now holds and passes it on: into the
    /// note, and through the note into its chord. The pitch object itself is
    /// left alone, because it is the one calling and is already borrowed.
    pub(crate) fn adopt_pitch(py: Python<'_>, note: &Py<Self>, pitch: &RsPitch) -> PyResult<()> {
        note.borrow_mut(py).inner.set_pitch(pitch.clone());
        // Anything a note has worked out about itself was worked out from
        // the pitch it has just been given, so it is thrown away — music21's
        // `pitchChanged`, which is what its own pitch setters end with.
        note.bind(py).call_method0("pitchChanged")?;
        Self::tell_chord(py, note, pitch)
    }

    /// Sends the note's own pitch the other way, out to its pitch object and
    /// to its chord: what a setter on the note ends with.
    /// The note's pitch as it stands now.
    ///
    /// The object is the source: a caller holds it and edits it, and one
    /// pitch object may belong to more than one note — music21's `chordify`
    /// and `Verticality.makeElement` hand a chord the very pitches of the
    /// notes they were built from, so renaming one there renames the note it
    /// came off.
    pub(crate) fn pitch_value(&self, py: Python<'_>) -> RsPitch {
        self.pitch.borrow(py).inner.clone()
    }

    pub(crate) fn broadcast_pitch(py: Python<'_>, note: &Py<Self>) -> PyResult<()> {
        let (object, value) = {
            let me = note.borrow(py);
            (me.pitch.clone_ref(py), me.inner.pitch().clone())
        };
        object.borrow_mut(py).inner = value.clone();
        Self::tell_chord(py, note, &value)
    }

    /// Writes a new pitch for this note into the chord holding it, when a
    /// chord of ours is holding it.
    fn tell_chord(py: Python<'_>, note: &Py<Self>, pitch: &RsPitch) -> PyResult<()> {
        let Some(chord) = note
            .borrow(py)
            .chord
            .as_ref()
            .map(|chord| chord.clone_ref(py))
        else {
            return Ok(());
        };
        // music21's own `ChordBase` writes itself here as well; it keeps its
        // own pitches and wants nothing from us.
        let Ok(chord) = chord.cast_bound::<crate::chord::Chord>(py).cloned() else {
            return Ok(());
        };
        crate::chord::Chord::adopt_note_pitch(&chord, note, pitch)
    }

    /// Tells the note which chord holds it, and makes sure its pitch object
    /// knows the note.
    pub(crate) fn attach_to_chord(py: Python<'_>, note: &Py<Self>, chord: &Bound<'_, PyAny>) {
        note.borrow_mut(py).chord = Some(chord.clone().unbind());
        Self::claim_pitch(py, note);
    }

    /// The duration this note actually has: the object music21 hands out
    /// when something has asked for one, since an edit through that object
    /// is an edit to the note, and `inner`'s otherwise.
    pub(crate) fn duration_value(&self, py: Python<'_>) -> Option<RsDuration> {
        match &self.duration {
            Some(object) => duration_value_of(py, object),
            None => self.inner.duration().cloned(),
        }
    }

    /// The `Duration` object for this note, made on first asking as music21
    /// makes one on first asking.
    pub(crate) fn duration_object(&mut self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        if let Some(object) = &self.duration {
            return Ok(object.clone_ref(py));
        }
        let inner = self
            .inner
            .duration()
            .cloned()
            .unwrap_or_else(RsDuration::quarter);
        let created =
            crate::installed_new(py, "music21.duration", "Duration", Duration::wrap(inner))?
                .into_any();
        self.duration = Some(created.clone_ref(py));
        Ok(created)
    }

    /// Takes whatever a caller wrote as a duration and holds it, as an
    /// object if that is what was given and as a fresh one if not.
    pub(crate) fn attach_duration(
        &mut self,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let inner = duration_from_any(value)?;
        // A duration object is kept as it stands, whether it is one of ours
        // or one of music21's own kinds of duration; anything else — a
        // length, a note-value name — becomes one of ours.
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
        self.inner.set_duration(inner);
        self.duration = Some(duration.clone_ref(py));
        Ok(duration)
    }

    /// Writes a volume straight in, letting go of whatever object was
    /// standing for the old one.
    pub(crate) fn replace_volume(&mut self, volume: Option<music21_rs_crate::Volume>) {
        self.inner.set_volume(volume);
        self.volume = None;
    }

    /// The colour the note is written in: what its style says if it has
    /// one, since that is where music21 keeps it, and what the value says
    /// otherwise.
    pub(crate) fn colour(&self, py: Python<'_>) -> Option<String> {
        if self.style.is_some() {
            return crate::notation::style_colour(py, self.style.as_ref());
        }
        self.inner.color().map(str::to_string)
    }

    /// Folds the lyric objects into the value and lets them go, so a method
    /// that works on the value works on what Python has actually got and the
    /// next reader builds the objects again from the answer.
    fn settle_lyrics(&mut self, py: Python<'_>) {
        let Some(lyrics) = self.lyrics.take() else {
            return;
        };
        let verses: Vec<music21_rs_crate::notation::Lyric> = lyrics
            .bind(py)
            .iter()
            .filter_map(|verse| verse.extract::<PyRef<'_, Lyric>>().ok())
            .map(|verse| verse.synced(py))
            .collect();
        let held = self.inner.lyrics_mut();
        held.clear();
        held.extend(verses);
    }

    /// Hands this note a `Duration` object to share, the way a chord shares
    /// its own with the notes it builds.
    pub(crate) fn share_duration(&mut self, py: Python<'_>, duration: &Py<PyAny>) {
        if let Some(value) = duration_value_of(py, duration) {
            self.inner.set_duration(value);
        }
        self.duration = Some(duration.clone_ref(py));
    }

    fn quarter_length(&self, py: Python<'_>) -> f64 {
        self.duration_value(py)
            .as_ref()
            .map_or(1.0, RsDuration::quarter_length)
    }

    /// This note with whatever its duration and volume objects now say
    /// written into it, for the answers the crate reads off a whole note.
    pub(crate) fn synced(&self, py: Python<'_>) -> RsNote {
        let mut note = self.inner.clone();
        // The pitch object is what a caller holds and edits, and one pitch
        // object may belong to more than one note: music21's `chordify` with
        // `copyPitches=False` hands a chord the very pitches of the notes it
        // was built from, and raising one of those raises the note it came
        // off. Reading the object rather than only writing to it is what
        // makes that hold.
        note.set_pitch(self.pitch.borrow(py).inner.clone());
        // music21's own readers write beams into the object `n.beams` gave
        // them, so the object is where the score's beaming is.
        if let Some(beams) = &self.beams {
            note.set_beams(beams.borrow_mut(py).settled_value(py));
        }
        if let Some(duration) = self.duration_value(py) {
            note.set_duration(duration);
        }
        if let Some(volume) = &self.volume {
            note.set_volume(Some(volume.borrow(py).inner.clone()));
        }
        // music21 keeps the colour on the style, so an edit through
        // `n.style.color` is an edit to the note.
        if self.style.is_some() {
            note.set_color(crate::notation::style_colour(py, self.style.as_ref()));
        }
        // music21 keeps the tie on an object a caller may still be holding,
        // and its own note-splitting writes through it.
        if let Some(tie) = &self.tie {
            note.set_tie(Some(tie.borrow(py).inner.clone()));
        }
        // Every note music21 has sounds for some length. The crate lets a
        // note carry none — a pitch nobody has said a length for — but one
        // reached through here has been asked as music21 asks, and music21's
        // answer for a note nobody has timed is a quarter.
        if note.duration().is_none() {
            note.set_duration(RsDuration::quarter());
        }
        if let Some(lyrics) = &self.lyrics {
            let verses = note.lyrics_mut();
            verses.clear();
            verses.extend(
                lyrics
                    .bind(py)
                    .iter()
                    .filter_map(|verse| verse.extract::<PyRef<'_, Lyric>>().ok())
                    .map(|verse| verse.synced(py)),
            );
        }
        note
    }

    /// music21 orders notes by pitch alone, and refuses anything without a
    /// `.pitch` — its `__lt__` answers `NotImplemented` and Python raises.
    /// The message is written out here rather than left to Python because
    /// pyo3 puts the module into the type name, so CPython's own wording
    /// would say `music21.note.Note` where music21 says `Note`.
    fn ordered(
        slf: &Bound<'_, Self>,
        other: &Bound<'_, PyAny>,
        operator: &str,
        compare: impl Fn(f64, f64) -> bool,
    ) -> PyResult<bool> {
        let Ok(pitch) = other
            .getattr("pitch")
            .and_then(|pitch| pitch_from_any(&pitch))
        else {
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "'{operator}' not supported between instances of '{}' and '{}'",
                slf.get_type().name()?,
                other.get_type().name()?,
            )));
        };
        Ok(compare(slf.borrow().pitch_value(slf.py()).ps(), pitch.ps()))
    }

    /// A detached copy: new pitch and duration objects, and no chord.
    /// A copy of the note as a Python object of the class it was asked on,
    /// with its pitch a copy of the very pitch object it had — so anything
    /// written on that pitch, such as the part name `chordify` tags it
    /// with, comes across — and the copy owning its pitch as the original
    /// did.
    fn copied_object<'py>(
        slf: &Bound<'py, Self>,
        py: Python<'py>,
        memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let mut copied = slf.borrow().copied(py)?;
        // Through the copier's own record of what it has copied, so a pitch
        // object something else holds as well — a chord symbol fixes its
        // root to the pitch of one of its notes — is copied once and shared
        // by the copies, as music21's own copying shares it.
        let deepcopy = py.import("copy")?.getattr("deepcopy")?;
        let held = slf.borrow().pitch.bind(py).clone();
        let pitch = match memo {
            Some(memo) => deepcopy.call1((held, memo))?,
            None => deepcopy.call1((held,))?,
        };
        if let Ok(pitch) = pitch.extract::<Py<Pitch>>() {
            copied.pitch = pitch;
        }
        let object = crate::copy_as_same_type(slf, copied)?;
        Self::claim_pitch(py, &object.clone().cast_into::<Self>()?.unbind());
        Ok(object)
    }

    fn copied(&self, py: Python<'_>) -> PyResult<Self> {
        let mut copy = Self::wrap(py, self.synced(py))?;
        // The ornaments and the marks come across: music21 copies a note to
        // realize a mordent and then takes the mordent off the copy, and a
        // copy with none would have nothing to take.
        copy.expressions = copied_list(py, self.expressions.as_ref())?;
        copy.articulations = copied_list(py, self.articulations.as_ref())?;
        copy.style = crate::notation::copied_style(py, self.style.as_ref());
        // The duration comes across as the kind of duration it is: a grace
        // note whose copy carried a plain duration would stop being one.
        if let Some(duration) = &self.duration {
            copy.duration = Some(copied_duration(py, duration)?);
        }
        Ok(copy)
    }
}

/// A list copied the way `copy.deepcopy` would copy it, which is what a
/// note's ornaments and marks are when the note is copied.
pub(crate) fn copied_list(
    py: Python<'_>,
    list: Option<&Py<PyList>>,
) -> PyResult<Option<Py<PyList>>> {
    let Some(list) = list else {
        return Ok(None);
    };
    let copier = py.import("copy")?.getattr("deepcopy")?;
    let copied = PyList::empty(py);
    for item in list.bind(py).iter() {
        copied.append(copier.call1((item,))?)?;
    }
    Ok(Some(copied.unbind()))
}

/// Reads a note argument: a `Note`, a pitch, or a name. A facade note comes
/// back with its duration object's value written in, since that object is
/// where an edit like `n.duration.type = 'half'` landed.
pub(crate) fn note_from_any(value: &Bound<'_, PyAny>) -> PyResult<RsNote> {
    if let Ok(facade) = value.extract::<PyRef<Note>>() {
        return Ok(facade.synced(value.py()));
    }
    Ok(RsNote::from_pitch(pitch_from_any(value)?))
}

#[pymethods]
impl Note {
    /// The Python objects this holds, shown to the cycle collector. A note
    /// and the pitch it hands out point at each other through Rust, and
    /// without this neither of them is ever freed.
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.pitch)?;
        visit.call(&self.volume)?;
        visit.call(&self.duration)?;
        visit.call(&self.chord)?;
        visit.call(&self.expressions)?;
        visit.call(&self.articulations)?;
        visit.call(&self.tie)?;
        visit.call(&self.lyrics)?;
        visit.call(&self.stored_instrument)?;
        visit.call(&self.unread_pitch)?;
        visit.call(&self.style)?;
        visit.call(&self.beams)?;
        Ok(())
    }

    fn __clear__(&mut self) {
        self.volume = None;
        self.duration = None;
        self.chord = None;
        self.expressions = None;
        self.articulations = None;
        self.tie = None;
        self.lyrics = None;
        self.stored_instrument = None;
        self.unread_pitch = None;
        self.style = None;
        self.beams = None;
    }

    /// A note is written out as text and read back, and its pitch and its
    /// duration are made again from what it says. Its ornaments and marks
    /// are Python objects the crate does not model, so they are frozen
    /// beside it — music21 freezes every score it parses, and a thawed note
    /// with no articulations on it would have lost what the score said.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        let py = slf.py();
        let extra = PyDict::new(py);
        {
            let note = slf.borrow();
            extra.set_item("expressions", note.expressions.as_ref())?;
            extra.set_item("articulations", note.articulations.as_ref())?;
            extra.set_item("storedInstrument", note.stored_instrument.as_ref())?;
            // The duration object goes with it: how long the note sounds is
            // in the value, but the tuplets it is written inside and whether
            // it is a grace note are the object's, and a score is frozen to
            // a file and read back.
            extra.set_item("duration", note.duration.as_ref())?;
        }
        crate::pickled_extra(slf, &slf.borrow().synced(py), Some(&extra))
    }

    fn __setstate__(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        state: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let (inner, extra) = crate::unpickled_extra::<_, RsNote>(slf, state)?;
        let Some(inner) = inner else {
            return Ok(());
        };
        let rebuilt = Self::wrap(py, inner)?;
        *slf.borrow_mut() = rebuilt;
        let pitch = slf.borrow().pitch.clone_ref(py);
        pitch.borrow_mut(py).owner = Some(slf.clone().unbind());
        if let Some(extra) = extra {
            let extra = extra.bind(py);
            let mut note = slf.borrow_mut();
            note.expressions = extra.get_item("expressions")?.extract().ok();
            note.articulations = extra.get_item("articulations")?.extract().ok();
            note.duration = extra
                .get_item("duration")
                .ok()
                .filter(|duration| !duration.is_none())
                .map(pyo3::Bound::unbind);
            note.stored_instrument = Some(extra.get_item("storedInstrument")?.unbind())
                .filter(|value| !value.is_none(py));
        }
        Ok(())
    }

    #[new]
    #[pyo3(signature = (pitch = None, **keywords))]
    fn new(
        py: Python<'_>,
        pitch: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let inner = match pitch.filter(|value| !value.is_none()) {
            Some(value) => RsNote::from_pitch(crate::pitch::pitch_from_any_with_keywords(
                py, value, keywords,
            )?),
            None => match crate::pitch::pitch_from_keywords(py, keywords)? {
                Some(pitch) => RsNote::from_pitch(pitch),
                None => RsNote::from_name("C4").map_err(note_error)?,
            },
        };
        let mut note = Self::wrap(py, inner)?;
        // A note built on a pitch object keeps that object, as music21's
        // does: its own harmony code fixes a chord's root to the pitch of
        // one of its notes, and moving the note has to move the root.
        if let Some(given) = pitch
            .filter(|value| !value.is_none())
            .and_then(|value| value.cast::<Pitch>().ok())
        {
            note.inner.set_pitch(given.borrow().inner.clone());
            note.pitch = given.clone().unbind();
        }
        // music21 hands the same keywords on to `Duration`, so `type='eighth',
        // dots=2` is an eighth with two dots and not a quarter.
        if Duration::keywords_say_duration(keywords)? {
            let duration = match keywords.and_then(|keywords| keywords.get_item("duration").ok()?) {
                Some(value) => value,
                None => Py::new(py, Duration::new(None, keywords)?)?
                    .into_bound(py)
                    .into_any(),
            };
            note.attach_duration(py, &duration)?;
        }
        Ok(note)
    }

    /// The note's own `Pitch` object, the same one every time: an edit
    /// through it is an edit to the note, and to the chord holding the note.
    pub(crate) fn get_pitch(slf: &Bound<'_, Self>) -> Py<Pitch> {
        let py = slf.py();
        let pitch = slf.borrow().pitch.clone_ref(py);
        pitch.borrow_mut(py).owner = Some(slf.clone().unbind());
        pitch
    }

    /// Setting it keeps the pitch object given, as music21 does: its own
    /// `Verticality.makeElement` gives a copied note the very pitch of the
    /// note it was copied from, and renaming it there renames both.
    #[setter]
    fn set_pitch(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        if let Ok(object) = value.extract::<Py<Pitch>>() {
            let inner = object.borrow(py).inner.clone();
            {
                let mut me = slf.borrow_mut();
                me.pitch = object;
                me.inner.set_pitch(inner.clone());
                me.unread_pitch = None;
            }
            let note = slf.clone().unbind();
            Self::claim_pitch(py, &note);
            return Self::tell_chord(py, &note, &inner);
        }
        let pitch = pitch_from_any(value)?;
        {
            let mut me = slf.borrow_mut();
            me.inner.set_pitch(pitch);
            me.unread_pitch = None;
        }
        Self::broadcast_pitch(py, &slf.clone().unbind())
    }

    /// music21's `.pitch`, which is whatever is stored there — a `Pitch`
    /// unless something put a bare value in through `pitches`.
    #[getter(pitch)]
    fn pitch_attribute(slf: &Bound<'_, Self>) -> Py<PyAny> {
        let py = slf.py();
        if let Some(unread) = &slf.borrow().unread_pitch {
            return unread.clone_ref(py);
        }
        Self::get_pitch(slf).into_any()
    }

    #[getter]
    fn get_name(&self, py: Python<'_>) -> String {
        self.pitch_value(py).name()
    }

    #[setter]
    fn set_name(slf: &Bound<'_, Self>, value: &str) -> PyResult<()> {
        let name = {
            let me = slf.borrow();
            match me.inner.pitch().octave() {
                Some(octave) if !value.chars().any(|ch| ch.is_ascii_digit()) => {
                    format!("{value}{octave}")
                }
                _ => value.to_string(),
            }
        };
        let pitch = RsPitch::from_name(name).map_err(note_error)?;
        slf.borrow_mut().inner.set_pitch(pitch);
        Self::broadcast_pitch(slf.py(), &slf.clone().unbind())
    }

    #[getter]
    fn get_nameWithOctave(&self, py: Python<'_>) -> String {
        self.pitch_value(py).name_with_octave()
    }

    /// Setting it renames the note's pitch, which is what music21 does.
    #[setter]
    fn set_nameWithOctave(slf: &Bound<'_, Self>, py: Python<'_>, value: &str) -> PyResult<()> {
        let pitch = slf.borrow().pitch.clone_ref(py);
        pitch.bind(py).setattr("nameWithOctave", value)?;
        let renamed = pitch.borrow(py).inner.clone();
        slf.borrow_mut().inner.set_pitch(renamed);
        Ok(())
    }

    #[getter]
    fn get_step(&self, py: Python<'_>) -> String {
        self.pitch_value(py)
            .name()
            .chars()
            .next()
            .unwrap_or('C')
            .to_string()
    }

    /// music21's `step` setter, which writes through to the pitch and keeps
    /// the accidental and the octave: `n.step = 'D'` on a `C#4` gives `D#4`.
    #[setter]
    fn set_step(slf: &Bound<'_, Self>, value: &str) -> PyResult<()> {
        // Through the note's own pitch object, so the change lands where a
        // caller holding that pitch will see it, and so the one reading of
        // a step name lives in one place.
        Self::get_pitch(slf).setattr(slf.py(), "step", value)
    }

    /// Always an `int`, as music21's `Pitch.octave` is.
    #[getter]
    fn get_octave(&self, py: Python<'_>) -> i32 {
        self.pitch_value(py)
            .octave()
            .unwrap_or_else(crate::pitch::default_octave)
    }

    /// music21's `octaveIsImplicit`, read and written through the note's own
    /// pitch object so a caller holding that pitch sees the change.
    #[getter]
    fn get_octaveIsImplicit(&self, py: Python<'_>) -> bool {
        self.pitch_value(py).octave_is_implicit()
    }

    #[setter]
    fn set_octaveIsImplicit(slf: &Bound<'_, Self>, value: bool) -> PyResult<()> {
        Self::get_pitch(slf).setattr(slf.py(), "octaveIsImplicit", value)?;
        Self::broadcast_pitch(slf.py(), &slf.clone().unbind())
    }

    #[setter]
    fn set_octave(slf: &Bound<'_, Self>, value: Option<i32>) -> PyResult<()> {
        let name = {
            let me = slf.borrow();
            match value {
                Some(octave) => format!("{}{octave}", me.inner.pitch().name()),
                None => me.inner.pitch().name(),
            }
        };
        let pitch = RsPitch::from_name(name).map_err(note_error)?;
        slf.borrow_mut().inner.set_pitch(pitch);
        Self::broadcast_pitch(slf.py(), &slf.clone().unbind())
    }

    /// music21's `.pitches`, the chord-shaped view of a note: its one pitch
    /// in a tuple.
    #[getter]
    fn get_pitches<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(slf.py(), [Self::get_pitch(slf)])
    }

    /// Setting it takes the first pitch of a list or tuple and ignores the
    /// rest, since a note has only one; anything that is not a sequence is
    /// refused.
    #[setter]
    fn set_pitches(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let refused = || {
            NoteException::new_err(format!(
                "cannot set pitches with provided object: {}",
                value
                    .str()
                    .map_or_else(|_| "?".to_string(), |v| v.to_string())
            ))
        };
        if !value.is_instance_of::<PyList>() && !value.is_instance_of::<PyTuple>() {
            return Err(refused());
        }
        let Some(first) = value.try_iter()?.next() else {
            return Err(refused());
        };
        let first = first?;
        // music21 stores the first item without looking at it. A value that
        // is not a pitch is kept as it was given and handed back by `.pitch`,
        // which is the footgun its own docstring warns about.
        if pitch_from_any(&first).is_err() || first.extract::<String>().is_ok() {
            slf.borrow_mut().unread_pitch = Some(first.unbind());
            return Ok(());
        }
        Self::set_pitch(slf, &first)
    }

    #[getter]
    fn fullName(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<String> {
        let value = slf.borrow().synced(py);
        let named = value.full_name();
        let timed = value
            .duration()
            .map(RsDuration::full_name)
            .unwrap_or_default();
        Ok(crate::note::named_with_duration(
            slf.as_any(),
            &named,
            &timed,
        ))
    }

    /// music21's `.duration`, the same object every time: `n.duration.type =
    /// 'half'` is how music21's own doctests lengthen a note.
    #[getter]
    fn get_duration(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let duration = slf.borrow_mut().duration_object(py)?;
        adopt_duration(py, &duration, slf.as_any())?;
        Ok(duration)
    }

    #[setter]
    fn set_duration(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let had_one = slf.borrow().duration.is_some();
        let duration = slf.borrow_mut().attach_duration(py, value)?;
        adopt_duration(py, &duration, slf.as_any())?;
        // Replacing the duration a note already had changes how long the
        // note is, and the streams holding it keep that length; music21
        // tells them so here, and a note whose length nobody has asked for
        // yet has nothing to tell.
        if had_one {
            told_sites(slf.as_any(), &duration.bind(py).getattr("quarterLength")?)?;
        }
        Ok(())
    }

    #[getter]
    fn get_quarterLength<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        op_frac(py, self.quarter_length(py))
    }

    #[setter]
    fn set_quarterLength(&mut self, py: Python<'_>, value: f64) -> PyResult<()> {
        let inner = RsDuration::new(value).map_err(note_error)?;
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

    #[getter]
    fn isRest(&self) -> bool {
        false
    }

    #[getter]
    fn isNote(&self) -> bool {
        true
    }

    #[getter]
    fn isChord(&self) -> bool {
        false
    }

    // ---- notation --------------------------------------------------------

    #[getter]
    pub(crate) fn get_tie(slf: &Bound<'_, Self>) -> PyResult<Option<Py<Tie>>> {
        let py = slf.py();
        if let Some(tie) = &slf.borrow().tie {
            return Ok(Some(tie.clone_ref(py)));
        }
        let Some(value) = slf.borrow().inner.tie().cloned() else {
            return Ok(None);
        };
        let tie = crate::installed_new(py, "music21.tie", "Tie", Tie::wrap(value))?;
        slf.borrow_mut().tie = Some(tie.clone_ref(py));
        Ok(Some(tie))
    }

    /// A tie object handed over is kept, as music21 keeps it: its own
    /// `splitAtQuarterLength` writes through the object it reads back.
    #[setter]
    pub(crate) fn set_tie(slf: &Bound<'_, Self>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let py = slf.py();
        let Some(value) = value.filter(|value| !value.is_none()) else {
            let mut note = slf.borrow_mut();
            note.inner.set_tie(None);
            note.tie = None;
            return Ok(());
        };
        let inner = tie_from_any(value)?;
        let tie = match value.extract::<Py<Tie>>() {
            Ok(object) => object,
            Err(_) => crate::installed_new(py, "music21.tie", "Tie", Tie::wrap(inner.clone()))?,
        };
        let mut note = slf.borrow_mut();
        note.inner.set_tie(Some(inner));
        note.tie = Some(tie);
        Ok(())
    }

    #[getter]
    fn get_notehead(&self) -> &'static str {
        self.inner.notehead().as_str()
    }

    #[setter]
    pub(crate) fn set_notehead(&mut self, value: Option<&str>) -> PyResult<()> {
        let notehead = match value {
            None | Some("") => RsNotehead::Normal,
            Some(name) => RsNotehead::from_name(name).map_err(not_rest_error)?,
        };
        self.inner.set_notehead(notehead);
        Ok(())
    }

    #[getter]
    fn get_noteheadFill(&self) -> Option<bool> {
        self.inner.notehead_fill()
    }

    #[setter]
    pub(crate) fn set_noteheadFill(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let fill = if value.is_none() {
            None
        } else if let Ok(flag) = value.extract::<bool>() {
            Some(flag)
        } else {
            match value.extract::<String>()?.as_str() {
                "none" | "default" => None,
                "filled" | "yes" => Some(true),
                "notfilled" | "no" => Some(false),
                other => {
                    return Err(not_rest_error(music21_rs_crate::Error::Notation(format!(
                        "not a valid notehead fill value: '{other}'"
                    ))));
                }
            }
        };
        self.inner.set_notehead_fill(fill);
        Ok(())
    }

    #[getter]
    fn get_noteheadParenthesis(&self) -> bool {
        self.inner.notehead_parenthesis()
    }

    #[setter]
    fn set_noteheadParenthesis(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let parenthesis = if let Ok(flag) = value.extract::<bool>() {
            flag
        } else if let Ok(number) = value.extract::<i64>() {
            number != 0
        } else {
            match value.extract::<String>()?.as_str() {
                "yes" => true,
                "no" => false,
                other => {
                    return Err(not_rest_error(music21_rs_crate::Error::Notation(format!(
                        "notehead parentheses must be True or False, not '{other}'"
                    ))));
                }
            }
        };
        self.inner.set_notehead_parenthesis(parenthesis);
        Ok(())
    }

    #[getter]
    fn get_stemDirection(&self) -> &'static str {
        self.inner.stem_direction().as_str()
    }

    #[setter]
    pub(crate) fn set_stemDirection(&mut self, value: Option<&str>) -> PyResult<()> {
        let direction = match value {
            None => RsStemDirection::Unspecified,
            Some(name) => RsStemDirection::from_name(name).map_err(not_rest_error)?,
        };
        self.inner.set_stem_direction(direction);
        Ok(())
    }

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

    /// music21's `.volume`, made on first asking and the same object after
    /// that, so `n.volume.velocity = 20` sticks.
    #[getter]
    pub(crate) fn get_volume(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<Volume>> {
        if let Some(volume) = &slf.borrow().volume {
            return Ok(volume.clone_ref(py));
        }
        // The volume knows whose it is, which is how music21 tells a volume
        // already spoken for from a loose one.
        let inner = slf.borrow().inner.volume();
        let created = crate::installed_new(
            py,
            "music21.volume",
            "Volume",
            Volume::owned_by(inner, slf.clone().into_any().unbind()),
        )?;
        slf.borrow_mut().volume = Some(created.clone_ref(py));
        Ok(created)
    }

    /// music21 takes the volume object itself when nothing else has claimed
    /// it, and a copy of it when something has — either way the note is what
    /// the volume is the volume of, which is what `Volume.client` answers.
    #[setter]
    fn set_volume(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let Some(value) = value.filter(|value| !value.is_none()) else {
            let mut me = slf.borrow_mut();
            me.inner.set_volume(None);
            me.volume = None;
            return Ok(());
        };
        let inner = volume_from_any(value)?;
        slf.borrow_mut().inner.set_volume(Some(inner.clone()));
        let object = match value.extract::<Py<Volume>>() {
            Ok(object) => {
                if object.borrow(py).is_claimed() {
                    crate::installed_new(py, "music21.volume", "Volume", object.borrow(py).clone())?
                } else {
                    object
                }
            }
            Err(_) => crate::installed_new(py, "music21.volume", "Volume", Volume::wrap(inner))?,
        };
        object.borrow_mut(py).set_client(Some(slf.as_any()));
        slf.borrow_mut().volume = Some(object);
        Ok(())
    }

    /// Whether this note carries a volume at all. music21 asks only whether
    /// the object is there, which is why reading `.volume` once makes this
    /// true.
    fn hasVolumeInformation(&self) -> bool {
        self.volume.is_some()
    }

    /// music21's `lyrics`: the list itself, the same one every time. Its
    /// own MusicXML reader appends each verse to what this hands back.
    #[getter]
    pub(crate) fn get_lyrics<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        if let Some(lyrics) = &slf.borrow().lyrics {
            return Ok(lyrics.bind(py).clone());
        }
        let verses = slf.borrow().inner.lyrics().to_vec();
        let list = PyList::empty(py);
        for verse in verses {
            list.append(crate::installed_new(
                py,
                "music21.note",
                "Lyric",
                Lyric::wrap(verse),
            )?)?;
        }
        slf.borrow_mut().lyrics = Some(list.clone().unbind());
        Ok(list)
    }

    /// Setting them replaces every verse at once, which is how music21 says
    /// a note is sung to something else, or to nothing.
    #[setter]
    pub(crate) fn set_lyrics(
        slf: &Bound<'_, Self>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let py = slf.py();
        let list = PyList::empty(py);
        if let Some(value) = value.filter(|value| !value.is_none()) {
            for item in value.try_iter()? {
                let item = item?;
                if item.extract::<PyRef<'_, Lyric>>().is_ok() {
                    list.append(item)?;
                    continue;
                }
                let verse = music21_rs_crate::notation::Lyric::new(item.extract::<String>()?);
                list.append(crate::installed_new(
                    py,
                    "music21.note",
                    "Lyric",
                    Lyric::wrap(verse),
                )?)?;
            }
        }
        let mut note = slf.borrow_mut();
        note.inner.lyrics_mut().clear();
        note.lyrics = Some(list.unbind());
        Ok(())
    }

    #[getter]
    fn get_lyric(&self, py: Python<'_>) -> Option<String> {
        // Through the value as it stands: a verse appended to the list the
        // note handed out has not reached the value until something reads it.
        self.synced(py).lyric()
    }

    /// music21 takes a string here, splitting it into a verse per line, or a
    /// `Lyric` to hold as the one verse, or `None` to clear them.
    #[setter]
    pub(crate) fn set_lyric(
        &mut self,
        py: Python<'_>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        self.settle_lyrics(py);
        let Some(value) = value.filter(|value| !value.is_none()) else {
            return self.inner.set_lyric(None).map_err(note_error);
        };
        if let Ok(lyric) = value.extract::<PyRef<'_, Lyric>>() {
            self.inner.set_lyric(None).map_err(note_error)?;
            self.inner.lyrics_mut().push(lyric.inner.clone());
            return Ok(());
        }
        let text = value.str()?.to_string();
        self.inner.set_lyric(Some(&text)).map_err(note_error)
    }

    /// music21's `insertLyric`: puts a syllable in front of the verse at
    /// `index` and moves the rest down a line.
    #[pyo3(signature = (text, index = 0, *, applyRaw = false, identifier = None))]
    pub(crate) fn insertLyric(
        &mut self,
        text: &Bound<'_, PyAny>,
        index: usize,
        applyRaw: bool,
        identifier: Option<String>,
    ) -> PyResult<()> {
        self.settle_lyrics(text.py());
        let text = if text.is_none() {
            String::new()
        } else {
            text.str()?.to_string()
        };
        self.inner
            .insert_lyric(&text, index, applyRaw)
            .map_err(note_error)?;
        if identifier.is_some()
            && let Some(lyric) = self.inner.lyrics_mut().get_mut(index)
        {
            lyric.set_identifier(identifier);
        }
        Ok(())
    }

    #[pyo3(signature = (text, lyricNumber = None, *, applyRaw = false, lyricIdentifier = None))]
    pub(crate) fn addLyric(
        &mut self,
        text: &Bound<'_, PyAny>,
        lyricNumber: Option<i32>,
        applyRaw: bool,
        lyricIdentifier: Option<String>,
    ) -> PyResult<()> {
        self.settle_lyrics(text.py());
        let text = if text.is_none() {
            String::new()
        } else {
            text.str()?.to_string()
        };
        self.inner
            .add_lyric(&text, lyricNumber, applyRaw)
            .map_err(note_error)?;
        if lyricIdentifier.is_some()
            && let Some(lyric) = self.inner.lyrics_mut().last_mut()
        {
            lyric.set_identifier(lyricIdentifier);
        }
        Ok(())
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
    ) -> PyResult<Option<Py<Self>>> {
        let py = slf.py();
        let mut pitch = transpose_pitch_by_any(slf.borrow().inner.pitch(), value)?;
        // A move given as a number of semitones says how far, not how to
        // spell what it lands on, so music21 lets the key signature in force
        // decide: a semitone above F is F# in D major and G- in B-flat minor.
        if value.extract::<i32>().is_ok()
            && let Some(signature) = key_signature_around(slf.as_any())?
        {
            pitch = pitch.respelled_for(&signature).map_err(note_error)?;
        }
        if inPlace {
            slf.borrow_mut().inner.set_pitch(pitch);
            Self::broadcast_pitch(py, &slf.clone().unbind())?;
            return Ok(None);
        }
        let mut moved = RsNote::from_pitch(pitch);
        if let Some(duration) = slf.borrow().inner.duration().cloned() {
            moved.set_duration(duration);
        }
        let mut copy = Self::wrap(py, moved)?;
        copy.style = crate::notation::copied_style(py, slf.borrow().style.as_ref());
        let copy = crate::copy_as_same_type(slf, copy)?;
        Self::claim_pitch(py, &copy.clone().cast_into::<Self>()?.unbind());
        crate::derived_from(&copy, slf.as_any(), "transpose")?;
        Ok(Some(copy.cast_into::<Self>()?.unbind()))
    }

    /// music21 compares two notes on what it lists as their equality
    /// attributes: the pitch, the duration, the tie, and how the note is
    /// written — its notehead and its beams. What is sung to it and how loud
    /// it is do not count; the ornaments over it and the marks under it do,
    /// by their kinds rather than by the objects themselves.
    fn __eq__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let Ok(other) = other.extract::<PyRef<Note>>() else {
            return Ok(false);
        };
        let (mine, theirs) = (self.synced(py), other.synced(py));
        let same = mine.pitch() == theirs.pitch()
            && self.quarter_length(py) == other.quarter_length(py)
            && mine.tie() == theirs.tie()
            && mine.notehead() == theirs.notehead()
            && mine.notehead_fill() == theirs.notehead_fill()
            && mine.notehead_parenthesis() == theirs.notehead_parenthesis()
            && mine.beams() == theirs.beams();
        if !same {
            return Ok(false);
        }
        Ok(
            same_kinds(py, self.expressions.as_ref(), other.expressions.as_ref())?
                && same_kinds(
                    py,
                    self.articulations.as_ref(),
                    other.articulations.as_ref(),
                )?,
        )
    }

    fn __lt__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        Self::ordered(slf, other, "<", |left, right| left < right)
    }

    fn __le__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        Self::ordered(slf, other, "<=", |left, right| left <= right)
    }

    fn __gt__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        Self::ordered(slf, other, ">", |left, right| left > right)
    }

    fn __ge__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        Self::ordered(slf, other, ">=", |left, right| left >= right)
    }

    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        slf.as_ptr() as isize >> 4
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        Ok(format!(
            "<music21.note.{} {}>",
            slf.get_type().qualname()?,
            slf.borrow().pitch_value(slf.py()).name()
        ))
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        py: Python<'py>,
        memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        Self::copied_object(slf, py, Some(memo))
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        Self::copied_object(slf, py, None)
    }

    /// music21's `storedInstrument`.
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

    /// music21's `NotRest.getInstrument`.
    #[pyo3(signature = (*, returnDefault = true))]
    fn getInstrument<'py>(
        slf: &Bound<'py, Self>,
        returnDefault: bool,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_for_note(slf.as_any(), returnDefault)
    }

    /// music21's `beams`: the beams joining this note's flags to its
    /// neighbours'. The object knows the note it came off, so an edit
    /// through it is an edit to the note.
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

    /// Setting them keeps the object given, as music21 does: its own
    /// `stripTies` clears a note's beams by handing it a fresh `Beams()` and
    /// writing into that afterwards.
    #[setter]
    fn set_beams(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let beams = value.extract::<Py<Beams>>()?;
        self.inner.set_beams(beams.borrow(py).inner.clone());
        self.beams = Some(beams);
        Ok(())
    }

    /// music21's `expressions`: the ornaments written over this note. The
    /// same list every time, so appending to it sticks.
    #[getter]
    fn get_expressions(&mut self, py: Python<'_>) -> Py<PyList> {
        let list = self
            .expressions
            .get_or_insert_with(|| PyList::empty(py).unbind());
        list.clone_ref(py)
    }

    #[setter]
    fn set_expressions(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.expressions = Some(list_of(value)?);
        Ok(())
    }

    /// music21's `articulations`: the marks written under this note.
    #[getter]
    fn get_articulations(&mut self, py: Python<'_>) -> Py<PyList> {
        let list = self
            .articulations
            .get_or_insert_with(|| PyList::empty(py).unbind());
        list.clone_ref(py)
    }

    #[setter]
    fn set_articulations(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.articulations = Some(list_of(value)?);
        Ok(())
    }

    /// music21's `_chordAttached`, which its own `ChordBase` sets on every
    /// note it takes in. Keeping the slot is what lets music21's chord
    /// classes hold facade notes at all.
    #[getter(_chordAttached)]
    fn get_chordAttached(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.chord.as_ref().map(|chord| chord.clone_ref(py))
    }

    #[setter(_chordAttached)]
    fn set_chordAttached(&mut self, value: Option<&Bound<'_, PyAny>>) {
        self.chord = value
            .filter(|value| !value.is_none())
            .map(|value| value.clone().unbind());
    }

    /// music21's `pitchChanged`: the note's pitch has been edited through the
    /// pitch object, so whatever the note had worked out about itself is no
    /// longer about this note, and the chord holding it is in the same case.
    fn pitchChanged(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        if slf.hasattr("_cache")? {
            slf.setattr("_cache", PyDict::new(py))?;
        }
        let Some(chord) = slf.borrow().chord.as_ref().map(|chord| chord.clone_ref(py)) else {
            return Ok(());
        };
        let chord = chord.bind(py);
        if chord.hasattr("clearCache")? {
            chord.call_method0("clearCache")?;
        }
        Ok(())
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Note>()?;
    Ok(())
}
