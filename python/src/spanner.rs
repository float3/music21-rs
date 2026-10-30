//! music21's spanners, over the crate's `spanner` module: the things that
//! join one note to another rather than standing at a point.
//!
//! A slur, a hairpin, an octave line, a glissando, a trill's wavy line, a
//! tremolo between two notes, a bracket, a pedal mark and an arpeggio
//! across staves. music21 keeps each as an element of the stream whose
//! notes it joins -- at offset nought, whatever the notes' offsets -- and
//! holds the notes themselves; this package keeps them the same way, so a
//! spanner handed back names the stream's own notes.
//!
//! **Nothing is installed over music21's own.** A spanner there carries
//! `idLocal`, the style and position it is drawn with, and the site
//! machinery that lets it be filled from a stream; standing in for one
//! would be quietly different rather than missing, as it would for
//! `bar.Barline` and `expressions.TextExpression`.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

use music21_rs_crate::spanner::{Spanner as RsSpanner, SpannerKind};

/// The names this module hands out: the base class and the kinds, each
/// under the name music21 gives it.
pub const NAMES: &[&str] = &["SpannerException", "SpannerBundleException"];

pyo3::create_exception!(music21_rs_facade, SpannerException, crate::Music21Exception);
pyo3::create_exception!(
    music21_rs_facade,
    SpannerBundleException,
    crate::Music21Exception
);

/// music21's `spanner.Spanner`: what joins the notes it holds.
#[pyclass(
    name = "Spanner",
    module = "music21.spanner",
    subclass,
    skip_from_py_object,
    dict
)]
pub struct Spanner {
    /// What the spanner says of itself besides the notes it joins: the
    /// kind, how it is drawn, an octave line's shift, a glissando's kind.
    /// The positions it names are not kept here -- the objects are.
    pub(crate) inner: RsSpanner,
    /// What a caller or a reader has numbered this spanner with:
    /// music21's `idLocal`, which it keeps as a bare attribute and which
    /// a MusicXML document's `number` is read into. Nothing musical turns
    /// on it, so the crate keeps none.
    #[pyo3(get, set)]
    idLocal: Option<Py<PyAny>>,
    /// The notes it joins, in order, as the objects they are: music21's
    /// `getSpannedElements`.
    spanned: Vec<Py<PyAny>>,
    /// The same notes in music21's own `stream.SpannerStorage`, which is
    /// where music21 keeps a spanner's joined objects and what its own code
    /// reaches for.
    ///
    /// **This is the bargain the braille text and the layout facades
    /// strike**: the objects are kept here *and* as the list above, so that
    /// `Spanner.__iter__`, `__contains__`, `purgeLocations` and a stream
    /// taking a spanner in all run as music21's own -- and so that a note
    /// asked for `getSpannerSites` finds this spanner, which it can only do
    /// where it has really been put into such a stream. Nothing of the
    /// crate is in it; it is rebuilt from the list whenever the list
    /// changes, and there is none at all where music21 cannot be asked for
    /// the class.
    storage: Option<Py<PyAny>>,
}

impl Spanner {
    /// A spanner of a kind joining nothing yet.
    pub(crate) fn of_kind(kind: SpannerKind) -> Self {
        let mut inner = RsSpanner::new(kind, Vec::new());
        // What music21's own constructor for each kind starts out with.
        match kind {
            SpannerKind::Ottava => {
                inner.set_shift(music21_rs_crate::spanner::OctaveShift::from_name("8va", true).ok())
            }
            SpannerKind::Line => inner.set_line_type(Some("solid".to_string())),
            SpannerKind::Glissando => inner.set_line_type(Some("wavy".to_string())),
            _ => {}
        }
        Self {
            inner,
            spanned: Vec::new(),
            idLocal: None,
            storage: None,
        }
    }

    /// Builds music21's own `SpannerStorage` afresh over the notes this
    /// joins, where music21 is there to ask for the class.
    ///
    /// Rebuilt rather than edited, since putting the objects in is what
    /// registers this spanner as a site on each of them -- which is the
    /// whole of what `getSpannerSites` reads.
    pub(crate) fn refresh_storage(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let Ok(class) = py
            .import("music21.stream")
            .and_then(|module| module.getattr("SpannerStorage"))
        else {
            return Ok(());
        };
        let held: Vec<Py<PyAny>> = slf
            .borrow()
            .spanned
            .iter()
            .map(|object| object.clone_ref(py))
            .collect();
        let elements = pyo3::types::PyList::empty(py);
        for object in &held {
            elements.append(object.bind(py))?;
        }
        let keywords = PyDict::new(py);
        keywords.set_item("client", slf)?;
        // music21's storage is a `Stream`, and a stream of its refuses
        // anything that is not a `Music21Object` -- which the objects a
        // spanner joins are only where something is installed over music21.
        // A caller with both and no swap (`pip install music21 music21-rs`)
        // joins notes of this package's own, and building the storage over
        // them raised `'music21.note.Note' object has no attribute
        // 'isStream'`: **the wheel refused to make a slur at all for having
        // music21 beside it, where with no music21 it made one.** So a
        // storage music21 will not hold is no storage, which is what a
        // wheel standing alone already answers; music21's own code reads
        // `spannerStorage` to reach the joined objects, and objects it
        // cannot hold are none it could have read.
        match class.call((elements,), Some(&keywords)) {
            Ok(made) => slf.borrow_mut().storage = Some(made.unbind()),
            Err(_) => slf.borrow_mut().storage = None,
        }
        Ok(())
    }

    /// The object for a spanner of the crate's, joining these objects:
    /// the class installed over music21's where there is one, and this
    /// package's own where not.
    pub(crate) fn object(
        py: Python<'_>,
        inner: &RsSpanner,
        spanned: Vec<Py<PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        Self::object_as(py, inner, spanned, crate::stream::Whose::Wheel)
    }

    /// The same, for one kind of streams or the other: one music21's own
    /// streams will hold where they are what it goes into, as they refuse
    /// anything that is not a `Music21Object`.
    pub(crate) fn object_as(
        py: Python<'_>,
        inner: &RsSpanner,
        spanned: Vec<Py<PyAny>>,
        whose: crate::stream::Whose,
    ) -> PyResult<Py<PyAny>> {
        let name = inner.kind().class_name();
        let module = module_of(inner.kind());
        let found = match whose {
            crate::stream::Whose::Music21 => crate::holdable_class(py, module, name),
            crate::stream::Whose::Wheel => crate::installed_class(py, module, name),
        };
        let class = match found {
            Some(class) => class,
            None => py
                .import("music21_rs")
                .or_else(|_| py.import("music21_rs_facade"))?
                .getattr(name)?,
        };
        let made = class.call0()?;
        {
            let mut held = made.extract::<PyRefMut<'_, Self>>()?;
            held.inner = inner.clone();
            held.spanned = spanned;
        }
        Self::refresh_storage(made.cast::<Self>()?)?;
        Ok(made.unbind())
    }
}

#[pymethods]
impl Spanner {
    /// music21's `Spanner(*spannedElements)`: the notes to join, given one
    /// by one or as a list.
    #[new]
    #[pyo3(signature = (*_spannedElements, **_keywords))]
    fn new(
        _spannedElements: &Bound<'_, PyTuple>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        Ok(Self::of_kind(SpannerKind::Slur))
    }

    /// The notes given are joined here, in `__init__` rather than in
    /// `__new__`, because music21's own `SpannerStorage` is built with the
    /// spanner itself as its client and there is no spanner yet in
    /// `__new__`.
    #[pyo3(signature = (*spannedElements, **keywords))]
    fn __init__(
        slf: &Bound<'_, Self>,
        spannedElements: &Bound<'_, PyTuple>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let py = slf.py();
        // music21 writes these three in its own constructors: whether the
        // reader that made this has finished joining it, whether `fill`
        // has run, and the kinds `fill` looks for.
        slf.setattr("completeStatus", false)?;
        slf.setattr("filledStatus", "unknown")?;
        let kinds = PyList::empty(py);
        for name in fill_element_types(slf.borrow().inner.kind()) {
            kinds.append(name)?;
        }
        slf.setattr("fillElementTypes", kinds)?;
        slf.call_method1("addSpannedElements", spannedElements)?;
        // music21 takes `type`, `transposing`, `lineType`, `tick` and the
        // rest as keywords on the one class that has each; here one class
        // stands for every kind, so a keyword is written as the attribute
        // it names and anything else is left to music21's own base.
        if let Some(keywords) = keywords {
            for (key, value) in keywords {
                let name: String = key.extract()?;
                if slf.as_any().hasattr(name.as_str())? {
                    slf.as_any().setattr(name.as_str(), value)?;
                }
            }
        }
        Ok(())
    }

    /// The stream music21 keeps a spanner's joined objects in: its own
    /// `stream.SpannerStorage`, holding the very notes this joins.
    ///
    /// There is no such class without music21, and nothing of this package
    /// stands in for a stream of music21's, so a wheel on its own says as
    /// much rather than answering a stream that music21's own code would
    /// not accept.
    #[getter]
    fn spannerStorage(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        if slf.borrow().storage.is_none() {
            Self::refresh_storage(slf)?;
        }
        slf.borrow().storage.as_ref().map_or_else(
            || {
                Err(SpannerException::new_err(
                    "stream.SpannerStorage is music21's own, which this package does not \
                     stand in for, so this member needs music21 beside it",
                ))
            },
            |held| Ok(held.clone_ref(py)),
        )
    }

    /// What music21 says this is, as its `classes` answers.
    #[getter]
    fn classes(slf: &Bound<'_, Self>) -> PyResult<Vec<String>> {
        let mut names = vec![slf.get_type().name()?.to_string()];
        let kind = slf.borrow().inner.kind();
        if names[0] != kind.class_name() {
            names.push(kind.class_name().to_string());
        }
        // A hairpin is a `DynamicWedge` to music21 before it is a
        // `Spanner`, which is how its own code picks the two out together.
        if kind.is_wedge() {
            names.push("DynamicWedge".to_string());
        }
        names.push("Spanner".to_string());
        names.push("Music21Object".to_string());
        names.push("ProtoM21Object".to_string());
        names.push("object".to_string());
        Ok(names)
    }

    /// music21's sort order for a spanner among the things at one offset,
    /// which is the same as a note's.
    #[classattr]
    fn classSortOrder() -> i32 {
        20
    }

    /// Whether this is a stream, which a spanner is not. music21 asks this
    /// of everything a stream holds.
    #[classattr]
    fn isStream() -> bool {
        false
    }

    /// The notes this joins, in order: music21's `getSpannedElements`.
    fn getSpannedElements<'py>(slf: &Bound<'py, Self>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let py = slf.py();
        Ok(slf
            .borrow()
            .spanned
            .iter()
            .map(|spanned| spanned.bind(py).clone())
            .collect())
    }

    /// Adds notes to join, at the end: music21's `addSpannedElements`,
    /// which takes them one by one or as a list.
    #[pyo3(signature = (*spannedElements, **_keywords))]
    fn addSpannedElements(
        slf: &Bound<'_, Self>,
        spannedElements: &Bound<'_, PyTuple>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let py = slf.py();
        let mut given: Vec<Bound<'_, PyAny>> = Vec::new();
        for one in spannedElements.iter() {
            if one.is_instance_of::<PyList>() || one.is_instance_of::<PyTuple>() {
                for each in one.try_iter()? {
                    given.push(each?);
                }
            } else {
                given.push(one);
            }
        }
        {
            let mut me = slf.borrow_mut();
            for one in given {
                // music21 asks its storage first, so an object already
                // joined is not joined twice.
                if me.spanned.iter().any(|held| held.bind(py).is(&one)) {
                    continue;
                }
                me.spanned.push(one.unbind());
            }
        }
        Self::refresh_storage(slf)
    }

    /// The first note it joins, or `None`: music21's `getFirst`.
    fn getFirst<'py>(slf: &Bound<'py, Self>) -> Option<Bound<'py, PyAny>> {
        let py = slf.py();
        slf.borrow()
            .spanned
            .first()
            .map(|spanned| spanned.bind(py).clone())
    }

    /// The last note it joins, or `None`: music21's `getLast`.
    fn getLast<'py>(slf: &Bound<'py, Self>) -> Option<Bound<'py, PyAny>> {
        let py = slf.py();
        slf.borrow()
            .spanned
            .last()
            .map(|spanned| spanned.bind(py).clone())
    }

    /// Whether an object is one of the notes it joins: music21's
    /// `hasSpannedElement`.
    /// The id of every object it joins: music21's
    /// `getSpannedElementIds`, which is Python's own `id` of each.
    fn getSpannedElementIds(&self, py: Python<'_>) -> Vec<usize> {
        self.spanned
            .iter()
            .map(|object| object.bind(py).as_ptr() as usize)
            .collect()
    }

    fn hasSpannedElement(slf: &Bound<'_, Self>, spannedElement: &Bound<'_, PyAny>) -> bool {
        let py = slf.py();
        slf.borrow()
            .spanned
            .iter()
            .any(|spanned| spanned.bind(py).is(spannedElement))
    }

    /// Puts one note in place of another: music21's
    /// `replaceSpannedElement`.
    fn replaceSpannedElement(
        slf: &Bound<'_, Self>,
        old: &Bound<'_, PyAny>,
        new: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let py = slf.py();
        {
            let mut held = slf.borrow_mut();
            for spanned in &mut held.spanned {
                if spanned.bind(py).is(old) {
                    *spanned = new.clone().unbind();
                }
            }
        }
        Self::refresh_storage(slf)
    }

    /// How many notes it joins: music21's `__len__`.
    fn __len__(&self) -> usize {
        self.spanned.len()
    }

    /// A copy of a spanner joins the **same** objects the original joins,
    /// which is what music21's own `__deepcopy__` is written for: a stream
    /// being copied replaces them afterwards with its own copies, and a
    /// spanner copied on its own is still about the notes it was about.
    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let copied = crate::blank_installed(&slf.get_type())?;
        {
            let me = slf.borrow();
            let mut copy = copied.extract::<PyRefMut<'_, Self>>()?;
            copy.inner = me.inner.clone();
            copy.spanned = me.spanned.iter().map(|one| one.clone_ref(py)).collect();
            copy.idLocal = me.idLocal.as_ref().map(|one| one.clone_ref(py));
        }
        // What music21 keeps on the object itself goes across as it does
        // for any other facade, and then the storage is built afresh over
        // the objects the copy joins.
        for name in ["completeStatus", "filledStatus", "fillElementTypes"] {
            if let Ok(held) = slf.getattr(name) {
                copied.setattr(name, held)?;
            }
        }
        Self::refresh_storage(copied.cast::<Self>()?)?;
        Ok(copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::__deepcopy__(slf, None)
    }

    /// What music21 compares two spanners by: its own
    /// `equalityAttributes`, which is the storage holding what each joins.
    #[classattr]
    fn equalityAttributes(py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(PyTuple::new(py, ["spannerStorage"])?.into_any().unbind())
    }

    /// Whether an object is the first this joins: music21's `isFirst`,
    /// which compares by identity.
    fn isFirst(slf: &Bound<'_, Self>, spannedElement: &Bound<'_, PyAny>) -> bool {
        Self::getFirst(slf).is_some_and(|first| first.is(spannedElement))
    }

    /// Whether an object is the last this joins: music21's `isLast`.
    fn isLast(slf: &Bound<'_, Self>, spannedElement: &Bound<'_, PyAny>) -> bool {
        Self::getLast(slf).is_some_and(|last| last.is(spannedElement))
    }

    /// Those of the notes it joins that answer to a class: music21's
    /// `getSpannedElementsByClass`, which asks its storage.
    fn getSpannedElementsByClass<'py>(
        slf: &Bound<'py, Self>,
        classFilterList: &Bound<'py, PyAny>,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let mut found = Vec::new();
        for object in Self::getSpannedElements(slf)? {
            if crate::stream::answers_to_class(&object, classFilterList)? {
                found.push(object);
            }
        }
        Ok(found)
    }

    /// Puts an object at the front of what this joins: music21's
    /// `insertFirstSpannedElement`, which adds it and then moves it from
    /// last to first -- so an object the spanner already joined is left
    /// where it was.
    fn insertFirstSpannedElement(
        slf: &Bound<'_, Self>,
        firstEl: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let py = slf.py();
        let before = slf.borrow().spanned.len();
        slf.call_method1("addSpannedElements", (firstEl,))?;
        if before == 0 {
            return Ok(());
        }
        {
            let mut me = slf.borrow_mut();
            if let Some(last) = me.spanned.pop_if(|last| last.bind(py).is(firstEl)) {
                me.spanned.insert(0, last);
            }
        }
        Self::refresh_storage(slf)
    }

    /// music21's `purgeLocations`, which purges the storage holding what
    /// this joins and then does what every `Music21Object` does.
    #[pyo3(signature = (rescanIsDead = false))]
    fn purgeLocations(slf: &Bound<'_, Self>, rescanIsDead: bool) -> PyResult<()> {
        let py = slf.py();
        if let Some(storage) = slf.borrow().storage.as_ref().map(|held| held.clone_ref(py)) {
            let keywords = PyDict::new(py);
            keywords.set_item("rescanIsDead", rescanIsDead)?;
            storage
                .bind(py)
                .call_method("purgeLocations", (), Some(&keywords))?;
        }
        if let Ok(class) = py
            .import("music21.base")
            .and_then(|module| module.getattr("Music21Object"))
        {
            class
                .getattr("purgeLocations")?
                .call1((slf, rescanIsDead))?;
        }
        Ok(())
    }

    /// music21's `purgeOrphans`, likewise.
    #[pyo3(signature = (excludeStorageStreams = true))]
    fn purgeOrphans(slf: &Bound<'_, Self>, excludeStorageStreams: bool) -> PyResult<()> {
        let py = slf.py();
        if let Some(storage) = slf.borrow().storage.as_ref().map(|held| held.clone_ref(py)) {
            storage
                .bind(py)
                .call_method1("purgeOrphans", (excludeStorageStreams,))?;
        }
        if let Ok(class) = py
            .import("music21.base")
            .and_then(|module| module.getattr("Music21Object"))
        {
            class
                .getattr("purgeOrphans")?
                .call1((slf, excludeStorageStreams))?;
        }
        Ok(())
    }

    /// Joins everything of the kinds this spanner fills with that stands
    /// between the first note it joins and the last: music21's `fill`.
    ///
    /// The search is music21's own over the stream it is given -- the
    /// offsets in a hierarchy and the filter by class are a stream's
    /// business, and the streams here are music21's wherever anything is
    /// installed. What this adds is the order music21 keeps: the last note
    /// is taken out and put back at the end, so that the first and the last
    /// are still the two they were.
    #[pyo3(signature = (
        searchStream = None,
        *,
        includeEndBoundary = false,
        mustFinishInSpan = false,
        mustBeginInSpan = true,
        includeElementsThatEndAtStart = false
    ))]
    fn fill(
        slf: &Bound<'_, Self>,
        searchStream: Option<&Bound<'_, PyAny>>,
        includeEndBoundary: bool,
        mustFinishInSpan: bool,
        mustBeginInSpan: bool,
        includeElementsThatEndAtStart: bool,
    ) -> PyResult<()> {
        let py = slf.py();
        let kinds = slf.getattr("fillElementTypes")?;
        if !kinds.is_truthy()? {
            return Ok(());
        }
        if slf
            .getattr("filledStatus")
            .and_then(|said| said.eq(true))
            .unwrap_or(false)
        {
            return Ok(());
        }
        let Some(start) = Self::getFirst(slf) else {
            return Ok(());
        };
        let searched = match searchStream.filter(|given| !given.is_none()) {
            Some(given) => given.clone(),
            None => {
                let site = start.getattr("activeSite")?;
                if site.is_none() {
                    return Err(SpannerException::new_err(
                        "Spanner.fill() requires a searchStream or getFirst().activeSite",
                    ));
                }
                site
            }
        };
        let last = Self::getLast(slf).filter(|last| !last.is(&start));
        // music21 takes the last note out so that whatever is found goes in
        // before it, and puts back the offset and the site that taking it
        // out cleared.
        let kept = match &last {
            None => None,
            Some(last) => {
                let offset = last.getattr("offset").ok();
                let site = last.getattr("activeSite").ok();
                {
                    let mut me = slf.borrow_mut();
                    me.spanned.retain(|held| !held.bind(py).is(last));
                }
                Self::refresh_storage(slf)?;
                Some((offset, site))
            }
        };
        let restore = |slf: &Bound<'_, Self>| -> PyResult<()> {
            if let (Some(last), Some((offset, site))) = (&last, &kept) {
                slf.call_method1("addSpannedElements", (last,))?;
                if let Some(offset) = offset {
                    last.setattr("offset", offset)?;
                }
                if let Some(site) = site {
                    last.setattr("activeSite", site)?;
                }
            }
            Ok(())
        };
        let Ok(from) = start.call_method1("getOffsetInHierarchy", (&searched,)) else {
            restore(slf)?;
            return Ok(());
        };
        let to = match &last {
            Some(last) => {
                let Ok(at) = last.call_method1("getOffsetInHierarchy", (&searched,)) else {
                    restore(slf)?;
                    return Ok(());
                };
                at.add(last.getattr("quarterLength")?)?
            }
            None => from.add(start.getattr("quarterLength")?)?,
        };
        let keywords = PyDict::new(py);
        keywords.set_item("includeEndBoundary", includeEndBoundary)?;
        keywords.set_item("mustFinishInSpan", mustFinishInSpan)?;
        keywords.set_item("mustBeginInSpan", mustBeginInSpan)?;
        keywords.set_item(
            "includeElementsThatEndAtStart",
            includeElementsThatEndAtStart,
        )?;
        let found = searched
            .call_method0("recurse")?
            .call_method(
                "getElementsByOffsetInHierarchy",
                (from, to),
                Some(&keywords),
            )?
            .call_method1("getElementsByClass", (kinds,))?;
        for one in crate::Walkable::walk(&found)? {
            let one = one?;
            if one.is(&start) || last.as_ref().is_some_and(|last| one.is(last)) {
                continue;
            }
            slf.call_method1("addSpannedElements", (one,))?;
        }
        restore(slf)?;
        slf.setattr("filledStatus", true)?;
        Ok(())
    }

    /// How far and which way an octave line moves the notes under it, as
    /// music21 names it: `8va`, `8vb`, `15ma`, `15mb`, `22da` or `22db`.
    ///
    /// music21 keeps this on `Ottava` alone, and so does this: one `Spanner`
    /// stands for every kind here, so a spanner of another kind raises as
    /// music21's would.
    #[getter]
    fn r#type(slf: &Bound<'_, Self>) -> PyResult<Option<&'static str>> {
        let me = slf.borrow();
        only(slf, SpannerKind::Ottava, "type")?;
        Ok(me.inner.shift().map(|shift| shift.name()))
    }

    /// Writes how far and which way the line moves its notes, by
    /// music21's name for it. Whether the notes are written where they
    /// sound is kept, as music21 keeps `transposing` across a change of
    /// type.
    #[setter(r#type)]
    fn set_type(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        only(slf, SpannerKind::Ottava, "type")?;
        let transposing = slf
            .borrow()
            .inner
            .shift()
            .is_some_and(|shift| shift.transposing());
        let named = ottava_type_named(value)?;
        let shift = music21_rs_crate::spanner::OctaveShift::from_name(&named, transposing)
            .map_err(|_| {
                SpannerException::new_err(format!(
                    "cannot create Ottava of type: {}",
                    value
                        .repr()
                        .map_or_else(|_| named.clone(), |shown| shown.to_string())
                ))
            })?;
        slf.borrow_mut().inner.set_shift(Some(shift));
        Ok(())
    }

    /// The six names an octave line's shift may go by: music21's
    /// `validOttavaTypes`.
    #[classattr]
    fn validOttavaTypes(py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(
            PyTuple::new(py, ["8va", "8vb", "15ma", "15mb", "22da", "22db"])?
                .into_any()
                .unbind(),
        )
    }

    /// The four ways a line may be drawn: music21's `validLineTypes`,
    /// which both `Line` and `Glissando` carry.
    #[classattr]
    fn validLineTypes(py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(PyTuple::new(py, ["solid", "dashed", "dotted", "wavy"])?
            .into_any()
            .unbind())
    }

    /// The five ticks a line may end with: music21's `validTickTypes`.
    #[classattr]
    fn validTickTypes(py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(PyTuple::new(py, ["up", "down", "arrow", "both", "none"])?
            .into_any()
            .unbind())
    }

    /// The five kinds of slide a glissando may be: music21's
    /// `validSlideTypes`.
    #[classattr]
    fn validSlideTypes(py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(PyTuple::new(
            py,
            ["chromatic", "continuous", "diatonic", "white", "black"],
        )?
        .into_any()
        .unbind())
    }

    /// The tick at the start of a line: music21's `startTick`.
    #[getter]
    fn startTick(slf: &Bound<'_, Self>) -> PyResult<&'static str> {
        only(slf, SpannerKind::Line, "startTick")?;
        Ok(slf.borrow().inner.ends().start.as_str())
    }

    #[setter]
    fn set_startTick(slf: &Bound<'_, Self>, value: &str) -> PyResult<()> {
        only(slf, SpannerKind::Line, "startTick")?;
        let found = read_tick(value)?;
        let mut me = slf.borrow_mut();
        let mut ends = me.inner.ends();
        ends.start = found;
        me.inner.set_ends(ends);
        Ok(())
    }

    /// The tick at the end of a line: music21's `endTick`.
    #[getter]
    fn endTick(slf: &Bound<'_, Self>) -> PyResult<&'static str> {
        only(slf, SpannerKind::Line, "endTick")?;
        Ok(slf.borrow().inner.ends().end.as_str())
    }

    #[setter]
    fn set_endTick(slf: &Bound<'_, Self>, value: &str) -> PyResult<()> {
        only(slf, SpannerKind::Line, "endTick")?;
        let found = read_tick(value)?;
        let mut me = slf.borrow_mut();
        let mut ends = me.inner.ends();
        ends.end = found;
        me.inner.set_ends(ends);
        Ok(())
    }

    /// The tick at both ends: music21's `tick`, which answers the start's
    /// and writes both.
    #[getter]
    fn tick(slf: &Bound<'_, Self>) -> PyResult<&'static str> {
        only(slf, SpannerKind::Line, "tick")?;
        Ok(slf.borrow().inner.ends().start.as_str())
    }

    #[setter]
    fn set_tick(slf: &Bound<'_, Self>, value: &str) -> PyResult<()> {
        only(slf, SpannerKind::Line, "tick")?;
        let found = read_tick(value)?;
        let mut me = slf.borrow_mut();
        let mut ends = me.inner.ends();
        ends.start = found;
        ends.end = found;
        me.inner.set_ends(ends);
        Ok(())
    }

    /// How long the tick at the start is, in tenths of a staff space:
    /// music21's `startHeight`.
    #[getter]
    fn startHeight<'py>(slf: &Bound<'py, Self>) -> PyResult<Option<Bound<'py, PyAny>>> {
        only(slf, SpannerKind::Line, "startHeight")?;
        let held = slf.borrow().inner.ends().start_height;
        held.map(|found| written_number(slf.py(), found))
            .transpose()
    }

    #[setter]
    fn set_startHeight(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        only(slf, SpannerKind::Line, "startHeight")?;
        let found = read_height(value)?;
        let mut me = slf.borrow_mut();
        let mut ends = me.inner.ends();
        ends.start_height = Some(found);
        me.inner.set_ends(ends);
        Ok(())
    }

    /// How long the tick at the end is: music21's `endHeight`.
    #[getter]
    fn endHeight<'py>(slf: &Bound<'py, Self>) -> PyResult<Option<Bound<'py, PyAny>>> {
        only(slf, SpannerKind::Line, "endHeight")?;
        let held = slf.borrow().inner.ends().end_height;
        held.map(|found| written_number(slf.py(), found))
            .transpose()
    }

    #[setter]
    fn set_endHeight(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        only(slf, SpannerKind::Line, "endHeight")?;
        let found = read_height(value)?;
        let mut me = slf.borrow_mut();
        let mut ends = me.inner.ends();
        ends.end_height = Some(found);
        me.inner.set_ends(ends);
        Ok(())
    }

    /// What a glissando says beside the line: music21's `label`, which is
    /// drawn and which no reader of music21's takes back, so nothing of
    /// the crate holds it.
    #[getter]
    fn label(slf: &Bound<'_, Self>) -> PyResult<Option<Py<PyAny>>> {
        only(slf, SpannerKind::Glissando, "label")?;
        Ok(slf
            .getattr("__dict__")
            .and_then(|held| held.get_item("_label"))
            .ok()
            .map(pyo3::Bound::unbind))
    }

    #[setter]
    fn set_label(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        only(slf, SpannerKind::Glissando, "label")?;
        slf.getattr("__dict__")?.set_item("_label", value)?;
        Ok(())
    }

    /// Writes whether the notes under the line are written where they
    /// sound.
    #[setter]
    fn set_transposing(slf: &Bound<'_, Self>, value: bool) -> PyResult<()> {
        only(slf, SpannerKind::Ottava, "transposing")?;
        let Some(shift) = slf.borrow().inner.shift() else {
            return Ok(());
        };
        let named = music21_rs_crate::spanner::OctaveShift::from_name(shift.name(), value)
            .map_err(|error| SpannerException::new_err(crate::pitch::message(&error)))?;
        slf.borrow_mut().inner.set_shift(Some(named));
        Ok(())
    }

    /// Which way an octave line moves the notes under it: music21's
    /// `shiftDirection`, `up` for a mark ending `a` and `down` for one
    /// ending `b`, the other way round when read in reverse.
    #[pyo3(signature = (reverse = false))]
    fn shiftDirection(slf: &Bound<'_, Self>, reverse: bool) -> PyResult<Option<&'static str>> {
        only(slf, SpannerKind::Ottava, "shiftDirection")?;
        let Some(shift) = slf.borrow().inner.shift() else {
            return Ok(None);
        };
        Ok(Some(if shift.up() != reverse { "up" } else { "down" }))
    }

    /// How far an octave line moves the notes under it, as an interval:
    /// music21's `interval`, which is the crate's `OctaveShift::interval`
    /// turned round where it is read in reverse.
    #[pyo3(signature = (reverse = false))]
    fn interval<'py>(slf: &Bound<'py, Self>, reverse: bool) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        only(slf, SpannerKind::Ottava, "interval")?;
        let Some(shift) = slf.borrow().inner.shift() else {
            return Err(SpannerException::new_err(
                "Cannot get an interval from an Ottava with no type",
            ));
        };
        let found = if reverse {
            shift
                .interval()
                .reversed()
                .map_err(|error| SpannerException::new_err(crate::pitch::message(&error)))?
        } else {
            shift.interval()
        };
        Ok(crate::interval::interval_object(py, found)?.into_bound(py))
    }

    /// Moves the notes under an octave line to where they sound, and says
    /// the line no longer transposes: music21's `performTransposition`.
    ///
    /// The notes are moved by music21's own transposition of each pitch in
    /// place, which is what keeps the caller's own pitch objects; the
    /// interval is the crate's.
    fn performTransposition(slf: &Bound<'_, Self>) -> PyResult<()> {
        only(slf, SpannerKind::Ottava, "performTransposition")?;
        if !Self::transposing(slf)? {
            return Ok(());
        }
        Self::set_transposing(slf, false)?;
        let found = Self::interval(slf, false)?;
        Self::move_spanned(slf, &found)
    }

    /// Moves them back to where they are written, and says the line
    /// transposes again: music21's `undoTransposition`.
    fn undoTransposition(slf: &Bound<'_, Self>) -> PyResult<()> {
        only(slf, SpannerKind::Ottava, "undoTransposition")?;
        if Self::transposing(slf)? {
            return Ok(());
        }
        Self::set_transposing(slf, true)?;
        let found = Self::interval(slf, true)?;
        Self::move_spanned(slf, &found)
    }

    /// What an octave line shifts by: music21's `shiftMagnitude`, 8, 15
    /// or 22.
    #[getter]
    fn shiftMagnitude(slf: &Bound<'_, Self>) -> PyResult<Option<u8>> {
        let me = slf.borrow();
        only(slf, SpannerKind::Ottava, "shiftMagnitude")?;
        Ok(me.inner.shift().map(|shift| shift.magnitude()))
    }

    /// Whether the notes under an octave line are written where they
    /// sound: music21's `transposing`.
    #[getter]
    fn transposing(slf: &Bound<'_, Self>) -> PyResult<bool> {
        let me = slf.borrow();
        only(slf, SpannerKind::Ottava, "transposing")?;
        Ok(me.inner.shift().is_some_and(|shift| shift.transposing()))
    }

    /// How a bracket's line is drawn: music21's `lineType`, `solid`,
    /// `dashed`, `dotted` or `wavy`.
    #[getter]
    fn lineType(slf: &Bound<'_, Self>) -> PyResult<Option<String>> {
        let me = slf.borrow();
        drawn_with_a_line(slf, "lineType")?;
        Ok(me.inner.line_type().map(str::to_string))
    }

    /// Writes how the line is drawn, refusing a name no line is drawn
    /// with, as music21 refuses one.
    #[setter]
    fn set_lineType(slf: &Bound<'_, Self>, value: Option<String>) -> PyResult<()> {
        drawn_with_a_line(slf, "lineType")?;
        if let Some(said) = &value {
            let named = said.to_lowercase();
            if !["solid", "dashed", "dotted", "wavy"].contains(&named.as_str()) {
                return Err(SpannerException::new_err(format!(
                    "not a valid value: {said}"
                )));
            }
            slf.borrow_mut().inner.set_line_type(Some(named));
            return Ok(());
        }
        slf.borrow_mut().inner.set_line_type(None);
        Ok(())
    }

    /// Which side of the staff it is drawn on, where the score says:
    /// music21's `placement`.
    #[getter]
    fn get_placement(&self) -> Option<&'static str> {
        self.inner.placement().map(|placement| placement.as_str())
    }

    #[setter]
    fn set_placement(&mut self, value: Option<&str>) -> PyResult<()> {
        let placement = match value.filter(|said| !said.is_empty()) {
            Some(said) => Some(
                music21_rs_crate::notation::Placement::from_name(said)
                    .map_err(|error| SpannerException::new_err(crate::pitch::message(&error)))?,
            ),
            None => None,
        };
        self.inner.set_placement(placement);
        Ok(())
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let name = slf.get_type().name()?.to_string();
        let joined: Vec<String> = Self::getSpannedElements(slf)?
            .iter()
            .map(|spanned| {
                spanned
                    .repr()
                    .map(|shown| shown.to_string())
                    .unwrap_or_default()
            })
            .collect();
        let kind = slf.borrow().inner.kind();
        let module = module_of(kind);
        // music21's `Ottava._reprInternal` says the shift and whether the
        // notes under it are written where they sound.
        let said = if kind == SpannerKind::Ottava {
            let shift = slf.borrow().inner.shift();
            let transposing = if shift.is_some_and(|shift| shift.transposing()) {
                "transposing"
            } else {
                "non-transposing"
            };
            format!("{} {transposing}", shift.map_or("", |shift| shift.name()))
        } else {
            String::new()
        };
        let inside = format!("{said}{}", joined.join(""));
        if inside.is_empty() {
            return Ok(format!("<{module}.{name}>"));
        }
        Ok(format!("<{module}.{name} {inside}>"))
    }
}

/// The name music21's `Ottava.type` setter reads out of what it was
/// given: a number, a size and a direction, or a name.
///
/// This is music21's own reading of its own keyword, quirks and all -- a
/// bare `22` is not one of the numbers it handles, and a pair whose size
/// is neither 8 nor 15 loses the size altogether -- so it is read here
/// rather than in the crate, which spells a shift by its name.
fn ottava_type_named(value: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Ok(size) = value.extract::<i64>() {
        return Ok(match size {
            8 => "8va".to_string(),
            15 => "15ma".to_string(),
            other => other.to_string(),
        });
    }
    if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
        let mut parts = Vec::new();
        let given: Vec<Bound<'_, PyAny>> = value.try_iter()?.collect::<PyResult<_>>()?;
        if let Some(first) = given.first() {
            let said = first.str()?.to_string();
            if said == "8" {
                parts.push("8v".to_string());
            } else if said == "15" {
                parts.push("15m".to_string());
            }
        }
        let down = given
            .get(1)
            .map(|way| way.str().map(|said| said.to_string()))
            .transpose()?
            .is_some_and(|said| said == "down");
        parts.push(if down { "b" } else { "a" }.to_string());
        return Ok(parts.join(""));
    }
    Ok(value.str()?.to_string().to_lowercase())
}

/// A tick music21 draws a line's end with, refused by name as music21
/// refuses one.
fn read_tick(value: &str) -> PyResult<music21_rs_crate::spanner::LineEnd> {
    music21_rs_crate::spanner::LineEnd::from_name(value)
        .map_err(|_| SpannerException::new_err(format!("not a valid value: {value}")))
}

/// A tick's length in tenths, which music21 refuses unless it is a number
/// at or above nought.
fn read_height(value: &Bound<'_, PyAny>) -> PyResult<f64> {
    let refused = || {
        SpannerException::new_err(format!(
            "not a valid value: {}",
            value
                .str()
                .map_or_else(|_| "None".to_string(), |said| said.to_string())
        ))
    };
    if value.is_instance_of::<pyo3::types::PyBool>() {
        return Err(refused());
    }
    let found: f64 = value.extract().map_err(|_| refused())?;
    if found < 0.0 {
        return Err(refused());
    }
    Ok(found)
}

/// music21's `spanner.SpannerBundle`: the spanners of a stream and of
/// everything it holds, gathered to be asked about.
///
/// What music21's own code does with one is count it, walk it and ask it
/// `getByClass`, which is what this answers; its idLocal numbering and
/// its `setPendingSpannedElements` machinery are music21's own.
#[pyclass(
    name = "SpannerBundle",
    module = "music21.spanner",
    subclass,
    skip_from_py_object
)]
pub struct SpannerBundle {
    held: Vec<Py<PyAny>>,
}

impl SpannerBundle {
    /// A bundle of these spanners, in the order they were met.
    pub(crate) fn of(held: Vec<Py<PyAny>>) -> Self {
        Self { held }
    }
}

#[pymethods]
impl SpannerBundle {
    /// music21's `SpannerBundle(*spanners)`.
    #[new]
    #[pyo3(signature = (*spanners))]
    fn new(spanners: &Bound<'_, PyTuple>) -> PyResult<Self> {
        let mut held = Vec::new();
        for given in spanners.iter() {
            if given.is_instance_of::<PyList>() || given.is_instance_of::<PyTuple>() {
                for one in given.try_iter()? {
                    held.push(one?.unbind());
                }
            } else {
                held.push(given.unbind());
            }
        }
        Ok(Self::of(held))
    }

    /// How many spanners it holds.
    fn __len__(&self) -> usize {
        self.held.len()
    }

    /// Another spanner into the bundle: music21's `append`.
    fn append(&mut self, other: &Bound<'_, PyAny>) {
        self.held.push(other.clone().unbind());
    }

    /// Takes a spanner out: music21's `remove`, which refuses one the
    /// bundle has not got.
    fn remove(slf: &Bound<'_, Self>, item: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let before = slf.borrow().held.len();
        slf.borrow_mut().held.retain(|held| !held.bind(py).is(item));
        if slf.borrow().held.len() == before {
            return Err(SpannerBundleException::new_err(format!(
                "cannot match object for removal: {}",
                item.str()?
            )));
        }
        Ok(())
    }

    /// Python's own `id` of each spanner's storage: music21's
    /// `getSpannerStorageIds`, which its readers tell two bundles'
    /// spanners apart by.
    fn getSpannerStorageIds(slf: &Bound<'_, Self>) -> PyResult<Vec<usize>> {
        let py = slf.py();
        let mut found = Vec::new();
        for spanner in &slf.borrow().held {
            found.push(spanner.bind(py).getattr("spannerStorage")?.as_ptr() as usize);
        }
        Ok(found)
    }

    /// Those whose joining is finished, or those whose is not, as a bundle
    /// of their own: music21's `getByCompleteStatus`.
    fn getByCompleteStatus(slf: &Bound<'_, Self>, completeStatus: bool) -> PyResult<Self> {
        let py = slf.py();
        let mut found = Vec::new();
        for spanner in &slf.borrow().held {
            let held = spanner.bind(py);
            if held
                .getattr("completeStatus")
                .and_then(|said| said.eq(completeStatus))
                .unwrap_or(false)
            {
                found.push(spanner.clone_ref(py));
            }
        }
        Ok(Self::of(found))
    }

    /// music21's `getByClassIdLocalComplete`, which is the three asked one
    /// after another.
    fn getByClassIdLocalComplete<'py>(
        slf: &Bound<'py, Self>,
        className: &Bound<'py, PyAny>,
        idLocal: Option<&Bound<'py, PyAny>>,
        completeStatus: bool,
    ) -> PyResult<Bound<'py, Self>> {
        let py = slf.py();
        let by_class = Bound::new(py, Self::getByClass(slf, className)?)?;
        let by_id = Self::getByIdLocal(&by_class, idLocal)?;
        Bound::new(py, Self::getByCompleteStatus(&by_id, completeStatus)?)
    }

    /// Numbers every spanner of a class from one: music21's
    /// `setIdLocalByClass`, six being as far as a MusicXML document
    /// numbers them.
    #[pyo3(signature = (className, maxId = 6))]
    fn setIdLocalByClass(
        slf: &Bound<'_, Self>,
        className: &Bound<'_, PyAny>,
        maxId: usize,
    ) -> PyResult<()> {
        let py = slf.py();
        let found = Self::getByClass(slf, className)?;
        for (at, spanner) in found.held.iter().enumerate() {
            spanner.bind(py).setattr("idLocal", (at % maxId) + 1)?;
        }
        Ok(())
    }

    /// Numbers every spanner the bundle holds by its class: music21's
    /// `setIdLocals`. A crescendo and a diminuendo are numbered together
    /// as the `DynamicWedge`s they both are, which is music21's rule and
    /// what a MusicXML document's numbering wants.
    fn setIdLocals(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let held: Vec<Py<PyAny>> = slf
            .borrow()
            .held
            .iter()
            .map(|one| one.clone_ref(py))
            .collect();
        let mut classes: Vec<String> = Vec::new();
        for spanner in held {
            for name in spanner.bind(py).getattr("classes")?.try_iter()? {
                let name: String = name?.extract()?;
                if name == "Crescendo" || name == "Diminuendo" {
                    continue;
                }
                if !classes.contains(&name) {
                    classes.push(name);
                }
                break;
            }
        }
        for name in classes {
            Self::setIdLocalByClass(slf, pyo3::types::PyString::new(py, &name).as_any(), 6)?;
        }
        Ok(())
    }

    /// Puts one object in place of another in every spanner that joins it,
    /// answering those that changed: music21's `replaceSpannedElement`.
    fn replaceSpannedElement<'py>(
        slf: &Bound<'py, Self>,
        old: &Bound<'py, PyAny>,
        new: &Bound<'py, PyAny>,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let py = slf.py();
        if old.is_instance_of::<pyo3::types::PyInt>() {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "send elements to replaceSpannedElement(), not ids.",
            ));
        }
        let held: Vec<Py<PyAny>> = slf
            .borrow()
            .held
            .iter()
            .map(|one| one.clone_ref(py))
            .collect();
        let mut changed = Vec::new();
        for spanner in held {
            let spanner = spanner.into_bound(py);
            if spanner
                .call_method1("hasSpannedElement", (old,))?
                .is_truthy()?
            {
                spanner.call_method1("replaceSpannedElement", (old, new))?;
                changed.push(spanner);
            }
        }
        Ok(changed)
    }

    /// Remembers that a spanner is waiting for the next object of a class
    /// to be its first: music21's `setPendingSpannedElementAssignment`.
    ///
    /// This is a *reader's* bookkeeping -- a hairpin read out of a
    /// `<direction>` has no note yet -- and music21 keeps it on the bundle
    /// where its own readers can reach it. The crate's readers keep a queue
    /// of their own inside themselves, so what is kept here is kept for
    /// music21's readers rather than for the crate.
    #[pyo3(signature = (sp, className, offsetInScore = None))]
    fn setPendingSpannedElementAssignment<'py>(
        slf: &Bound<'py, Self>,
        sp: &Bound<'py, PyAny>,
        className: &str,
        offsetInScore: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let made = pending_reference(py, sp, className, offsetInScore)?;
        slf.getattr("__dict__")?;
        let waiting = Self::pending(slf)?;
        waiting.call_method1("append", (&made,))?;
        Ok(made)
    }

    /// Gives a waiting spanner the object it was waiting for, where the
    /// object answers to the class it named and stands where it was told:
    /// music21's `freePendingSpannedElementAssignment`. Only the spanner at
    /// the front of the queue gets each object, which is why two started
    /// together take two successive notes.
    #[pyo3(signature = (spannedElementCandidate, offsetInScore = None))]
    fn freePendingSpannedElementAssignment(
        slf: &Bound<'_, Self>,
        spannedElementCandidate: &Bound<'_, PyAny>,
        offsetInScore: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let py = slf.py();
        let waiting = Self::pending(slf)?;
        if waiting.len() == 0 {
            return Ok(());
        }
        let classes = spannedElementCandidate.getattr("classSet")?;
        let mut taken: Option<usize> = None;
        for (at, reference) in waiting.try_iter()?.enumerate() {
            let reference = reference?;
            let named = reference.getattr("className")?;
            if !classes.contains(&named)? {
                continue;
            }
            let at_offset = reference.getattr("offsetInScore")?;
            let matches = match offsetInScore.filter(|given| !given.is_none()) {
                None => true,
                Some(given) => given.eq(&at_offset)?,
            };
            if !matches {
                continue;
            }
            reference
                .getattr("spanner")?
                .call_method1("insertFirstSpannedElement", (spannedElementCandidate,))?;
            taken = Some(at);
            break;
        }
        if let Some(at) = taken {
            waiting.call_method1("pop", (at,))?;
        }
        let _ = py;
        Ok(())
    }

    /// Every waiting spanner, and nothing left waiting afterwards:
    /// music21's `popPendingSpannedElementAssignments`.
    fn popPendingSpannedElementAssignments<'py>(
        slf: &Bound<'py, Self>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let waiting = Self::pending(slf)?;
        let answered = waiting.call_method0("copy")?;
        slf.getattr("__dict__")?
            .set_item("_pendingSpannedElementAssignment", PyList::empty(py))?;
        Ok(answered)
    }

    /// Those carrying an `idLocal`, as a bundle of their own: music21's
    /// `getByIdLocal`, whose `idLocal` is an attribute a caller or a
    /// reader writes rather than anything the crate keeps.
    #[pyo3(signature = (idLocal = None))]
    fn getByIdLocal<'py>(
        slf: &Bound<'py, Self>,
        idLocal: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, Self>> {
        let py = slf.py();
        let mut found = Vec::new();
        for spanner in &slf.borrow().held {
            let held = spanner.bind(py);
            let same = match (held.getattr("idLocal"), idLocal) {
                (Ok(mine), Some(wanted)) => mine.eq(wanted)?,
                (Ok(mine), None) => mine.is_none(),
                (Err(_), _) => false,
            };
            if same {
                found.push(spanner.clone_ref(py));
            }
        }
        Bound::new(py, Self::of(found))
    }

    /// Those joining an element, as a bundle of their own: music21's
    /// `getBySpannedElement`, which asks each by the object itself.
    fn getBySpannedElement<'py>(
        slf: &Bound<'py, Self>,
        spannedElement: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, Self>> {
        let py = slf.py();
        let mut found = Vec::new();
        for spanner in &slf.borrow().held {
            let held = spanner.bind(py);
            if held
                .call_method1("hasSpannedElement", (spannedElement,))?
                .is_truthy()?
            {
                found.push(spanner.clone_ref(py));
            }
        }
        Bound::new(py, Self::of(found))
    }

    fn __iter__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let listed = PyList::empty(py);
        for spanner in &slf.borrow().held {
            listed.append(spanner.bind(py))?;
        }
        listed.as_any().try_iter().map(|held| held.into_any())
    }

    fn __getitem__<'py>(slf: &Bound<'py, Self>, index: usize) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        slf.borrow()
            .held
            .get(index)
            .map(|spanner| spanner.bind(py).clone())
            .ok_or_else(|| pyo3::exceptions::PyIndexError::new_err("bundle index out of range"))
    }

    /// The spanners answering to a class, as a bundle of their own:
    /// music21's `getByClass`.
    fn getByClass(slf: &Bound<'_, Self>, className: &Bound<'_, PyAny>) -> PyResult<Self> {
        let py = slf.py();
        let held: Vec<Py<PyAny>> = slf
            .borrow()
            .held
            .iter()
            .map(|one| one.clone_ref(py))
            .collect();
        let mut kept = Vec::new();
        for spanner in held {
            let spanner = spanner.into_bound(py);
            if crate::stream::answers_to_class(&spanner, className)? {
                kept.push(spanner.unbind());
            }
        }
        Ok(Self::of(kept))
    }

    fn __repr__(&self) -> String {
        format!(
            "<music21.spanner.SpannerBundle of size {}>",
            self.held.len()
        )
    }
}

impl Spanner {
    /// Moves every pitch of every note this joins by an interval, in
    /// place, as music21's own transposition of an octave line does.
    fn move_spanned(slf: &Bound<'_, Self>, by: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let keywords = PyDict::new(py);
        keywords.set_item("inPlace", true)?;
        for object in Self::getSpannedElements(slf)? {
            let Ok(pitches) = object.getattr("pitches") else {
                continue;
            };
            for pitch in crate::Walkable::walk(&pitches)? {
                pitch?.call_method("transpose", (by,), Some(&keywords))?;
            }
        }
        Ok(())
    }
}

/// A tick's length as music21 keeps it: the number it was given, whole
/// where it is whole, which is what its own examples print.
fn written_number(py: Python<'_>, value: f64) -> PyResult<Bound<'_, PyAny>> {
    if value.fract() == 0.0 {
        return Ok((value as i64).into_pyobject(py)?.into_any());
    }
    Ok(value.into_pyobject(py)?.into_any())
}

impl SpannerBundle {
    /// The queue of spanners waiting for their first object, kept in the
    /// object's own dictionary as music21 keeps it on the instance.
    fn pending<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let held = slf.getattr("__dict__")?;
        if let Ok(waiting) = held.get_item("_pendingSpannedElementAssignment")
            && let Ok(listed) = waiting.cast_into::<PyList>()
        {
            return Ok(listed);
        }
        let made = PyList::empty(py);
        held.set_item("_pendingSpannedElementAssignment", &made)?;
        Ok(made)
    }
}

/// One waiting spanner, as music21's `PendingAssignmentRef` keeps it: the
/// spanner, the class of object it is waiting for, and where in the score
/// that object must stand.
fn pending_reference<'py>(
    py: Python<'py>,
    spanner: &Bound<'py, PyAny>,
    class_name: &str,
    offset: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    static BUILT: pyo3::sync::PyOnceLock<Py<PyAny>> = pyo3::sync::PyOnceLock::new();
    let class = crate::music21_or_namedtuple(
        py,
        "music21.spanner",
        "PendingAssignmentRef",
        "spanner className offsetInScore",
        &BUILT,
    )?;
    let keywords = PyDict::new(py);
    keywords.set_item("spanner", spanner)?;
    keywords.set_item("className", class_name)?;
    keywords.set_item(
        "offsetInScore",
        offset.map_or_else(|| py.None().into_bound(py), pyo3::Bound::clone),
    )?;
    class.call((), Some(&keywords))
}

/// The kinds of element `fill` looks for, as music21 sets them: an octave
/// line takes everything that sounds and a trill extension everything
/// written, and every other kind fills with nothing.
///
/// music21 holds the classes themselves here; these are their names, which
/// is what `getElementsByClass` reads either way and what lets a wheel with
/// no music21 beside it answer at all.
fn fill_element_types(kind: SpannerKind) -> &'static [&'static str] {
    match kind {
        SpannerKind::Ottava => &["NotRest"],
        SpannerKind::TrillExtension => &["GeneralNote"],
        _ => &[],
    }
}

/// Which of music21's modules a kind of spanner lives in.
fn module_of(kind: SpannerKind) -> &'static str {
    match kind {
        SpannerKind::Crescendo | SpannerKind::Diminuendo => "music21.dynamics",
        SpannerKind::TrillExtension
        | SpannerKind::TremoloSpanner
        | SpannerKind::ArpeggioMark
        | SpannerKind::PedalMark => "music21.expressions",
        SpannerKind::Neume => "music21.volpiano",
        _ => "music21.spanner",
    }
}

/// Refuses a member music21 keeps on one kind of spanner alone, naming the
/// class as music21's error names it.
fn only(slf: &Bound<'_, Spanner>, kind: SpannerKind, member: &str) -> PyResult<()> {
    if slf.borrow().inner.kind() == kind {
        return Ok(());
    }
    let class = Spanner::classes(slf)?
        .first()
        .cloned()
        .unwrap_or_else(|| "Spanner".to_string());
    Err(pyo3::exceptions::PyAttributeError::new_err(format!(
        "'{class}' object has no attribute '{member}'"
    )))
}

/// A line type is drawn on both a bracket and a glissando, so either
/// answers for it where music21's own classes both carry it.
fn drawn_with_a_line(slf: &Bound<'_, Spanner>, member: &str) -> PyResult<()> {
    let kind = slf.borrow().inner.kind();
    if kind == SpannerKind::Line || kind == SpannerKind::Glissando {
        return Ok(());
    }
    only(slf, SpannerKind::Line, member)
}

/// music21's kinds of spanner, each the same `Spanner` under its own name
/// and in the module music21 keeps it in.
macro_rules! spanner_kinds {
    ($(($class:ident, $name:literal, $module:literal, $kind:expr, $doc:literal)),* $(,)?) => {
        $(
            #[doc = $doc]
            #[pyclass(name = $name, module = $module, extends = Spanner, subclass)]
            pub struct $class;

            #[pymethods]
            impl $class {
                /// The notes given are joined by the `__init__` this
                /// inherits, there being no spanner yet in `__new__` for
                /// music21's own `SpannerStorage` to be built with.
                #[new]
                #[pyo3(signature = (*_spannedElements, **_keywords))]
                fn new(
                    _spannedElements: &Bound<'_, PyTuple>,
                    _keywords: Option<&Bound<'_, PyDict>>,
                ) -> PyResult<PyClassInitializer<Self>> {
                    Ok(PyClassInitializer::from(Spanner::of_kind($kind)).add_subclass($class))
                }
            }
        )*

        /// Adds the spanner classes to a module.
        pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
            m.add_class::<Spanner>()?;
            m.add_class::<SpannerBundle>()?;
            $(m.add_class::<$class>()?;)*
            Ok(())
        }

        /// The names this module hands out beside the exception.
        pub const CLASS_NAMES: &[&str] = &["Spanner", "SpannerBundle", $($name),*];
    };
}

spanner_kinds!(
    (
        Slur,
        "Slur",
        "music21.spanner",
        SpannerKind::Slur,
        "music21's `Slur`: a curve over the notes played in one breath or bow."
    ),
    (
        Crescendo,
        "Crescendo",
        "music21.dynamics",
        SpannerKind::Crescendo,
        "music21's `Crescendo`: a hairpin opening out, growing louder."
    ),
    (
        Diminuendo,
        "Diminuendo",
        "music21.dynamics",
        SpannerKind::Diminuendo,
        "music21's `Diminuendo`: a hairpin closing, growing softer."
    ),
    (
        Ottava,
        "Ottava",
        "music21.spanner",
        SpannerKind::Ottava,
        "music21's `Ottava`: an octave line, `8va` or `15mb`."
    ),
    (
        Line,
        "Line",
        "music21.spanner",
        SpannerKind::Line,
        "music21's `Line`: a bracket or line over a passage."
    ),
    (
        Glissando,
        "Glissando",
        "music21.spanner",
        SpannerKind::Glissando,
        "music21's `Glissando`: a slide from one note to the next."
    ),
    (
        TrillExtension,
        "TrillExtension",
        "music21.expressions",
        SpannerKind::TrillExtension,
        "music21's `TrillExtension`: the wavy line carrying a trill on."
    ),
    (
        TremoloSpanner,
        "TremoloSpanner",
        "music21.expressions",
        SpannerKind::TremoloSpanner,
        "music21's `TremoloSpanner`: a tremolo alternating between two notes."
    ),
    (
        ArpeggioMarkSpanner,
        "ArpeggioMarkSpanner",
        "music21.expressions",
        SpannerKind::ArpeggioMark,
        "music21's `ArpeggioMarkSpanner`: one arpeggio across several chords."
    ),
    (
        PedalMark,
        "PedalMark",
        "music21.expressions",
        SpannerKind::PedalMark,
        "music21's `PedalMark`: how long a pedal is held."
    ),
    (
        Neume,
        "Neume",
        "music21.volpiano",
        SpannerKind::Neume,
        "music21's `volpiano.Neume`: the notes sung to one syllable as one figure."
    ),
);
