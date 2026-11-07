//! music21's `sites`: the streams an element stands in, where it stands in
//! each, and the one it was last reached through.
//!
//! music21 keeps a `Sites` object on every element, holding a weak
//! reference to each stream the element is in, and an `activeSite` beside
//! it; `offset`, the beat members and the whole context search are read
//! through them. The package's elements keep the same, in one table here
//! keyed by the element's identity, since a pyo3 class carries no
//! dictionary to hang it on: each row holds weak references only -- to the
//! element, which says whether the row is still its, and to the streams --
//! so nothing here keeps anything alive and nothing here is a cycle.
//!
//! A stream writes itself into the rows of what it holds as it puts them
//! in (`Stream::put`), once it is a Python object a weak reference can
//! name. What it no longer holds it is not told of, as music21's `remove`
//! tells: a site is read as one only while the stream still holds the
//! element, which answers the same for every way a stream lets go.
//!
//! The members are put on the package's `Music21Object`, which every
//! element class is built on, as descriptors (`register`), so an
//! installed class -- which stands on music21's own `Music21Object` --
//! reads music21's instead: installing puts music21's members back on top.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use std::cmp::Ordering as Order;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard};

use pyo3::exceptions::{PyAttributeError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PySet, PyWeakrefMethods, PyWeakrefReference};

use music21::StreamKind;

use crate::stream::Stream;

pyo3::create_exception!(music21_rs_facade, SitesException, crate::Music21Exception);
pyo3::create_exception!(
    music21_rs_facade,
    Music21ObjectException,
    crate::Music21Exception
);

/// music21's `globalSiteIndex`: one count across every stream, so the order
/// things were put in can be compared between streams, as music21's context
/// search compares it.
static INDEX: AtomicU64 = AtomicU64::new(1);

/// The next number in the order things are put into streams.
pub(crate) fn next_index() -> u64 {
    INDEX.fetch_add(1, Ordering::Relaxed)
}

/// A stream as a site: a weak reference to it and its identity.
pub(crate) struct Here {
    reference: Py<PyWeakrefReference>,
    key: usize,
}

impl Here {
    /// A site naming this stream.
    pub(crate) fn of(stream: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            reference: PyWeakrefReference::new(stream)?.unbind(),
            key: stream.as_ptr() as usize,
        })
    }

    pub(crate) fn clone_ref(&self, py: Python<'_>) -> Self {
        Self {
            reference: self.reference.clone_ref(py),
            key: self.key,
        }
    }

    /// The weak reference, for a holder's `__traverse__`.
    pub(crate) fn visit(
        &self,
        visit: &pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.reference)
    }

    /// The stream, while it lives.
    fn stream<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyAny>> {
        self.reference.bind(py).upgrade()
    }
}

/// music21's `sortTuple`: what decides which of two things comes first in
/// a stream, compared field by field.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SortKey {
    pub(crate) at_end: f64,
    pub(crate) offset: f64,
    pub(crate) priority: f64,
    pub(crate) order: f64,
    pub(crate) not_grace: f64,
    pub(crate) index: f64,
}

impl SortKey {
    fn fields(&self) -> [f64; 6] {
        [
            self.at_end,
            self.offset,
            self.priority,
            self.order,
            self.not_grace,
            self.index,
        ]
    }

    fn compare(&self, other: &Self) -> Order {
        for (mine, theirs) in self.fields().iter().zip(other.fields()) {
            let found = mine.total_cmp(&theirs);
            if found != Order::Equal {
                return found;
            }
        }
        Order::Equal
    }

    pub(crate) fn is_before(&self, other: &Self) -> bool {
        self.compare(other) == Order::Less
    }

    pub(crate) fn is_after(&self, other: &Self) -> bool {
        self.compare(other) == Order::Greater
    }

    fn at(self, offset: f64) -> Self {
        Self { offset, ..self }
    }

    /// music21's `ZeroSortTupleLow` and `ZeroSortTupleHigh` at an offset:
    /// before or after everything standing there.
    fn bound(offset: f64, high: bool) -> Self {
        Self {
            at_end: 0.0,
            offset,
            priority: if high {
                f64::INFINITY
            } else {
                f64::NEG_INFINITY
            },
            order: 0.0,
            not_grace: 1.0,
            index: 0.0,
        }
    }
}

/// One stream an element stands in, and when it was put there.
struct Entry {
    here: Here,
    index: u64,
}

/// What music21 keeps on an element for its sites.
struct Record {
    /// The element, so a row left by one that is gone is not read as the
    /// row of a new one at the same address.
    alive: Py<PyWeakrefReference>,
    /// The streams, oldest first: music21's `siteDict`.
    entries: Vec<Entry>,
    /// music21's `_activeSite`, and the offset it stood at there when it
    /// was made one, which is what `offset` answers once that stream is
    /// gone: `_activeSiteStoredOffset`.
    active: Option<Here>,
    stored: Option<f64>,
    /// music21's `_naiveOffset`: the offset of an element no stream holds.
    naive: f64,
}

#[derive(Default)]
struct Table {
    records: HashMap<usize, Record>,
    /// How many rows there were after the last sweep for rows whose
    /// element is gone.
    swept: usize,
}

static TABLE: LazyLock<Mutex<Table>> = LazyLock::new(|| Mutex::new(Table::default()));

fn table() -> MutexGuard<'static, Table> {
    TABLE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// How many rows the table may hold before the first sweep.
const SWEEP_FROM: usize = 1024;

impl Table {
    /// The element's row, if it has one that is its own: a row whose
    /// element still lives is the row of the one living at that address.
    fn row(&mut self, element: &Bound<'_, PyAny>) -> Option<&mut Record> {
        let record = self.records.get_mut(&(element.as_ptr() as usize))?;
        record.alive.bind(element.py()).upgrade()?;
        Some(record)
    }

    /// Drops the rows of elements that are gone, once the table has grown
    /// to twice what the last sweep left: weak references to the elements
    /// carry no callback, so nothing runs while an element is freed.
    fn sweep(&mut self, py: Python<'_>) {
        if self.records.len() < SWEEP_FROM.max(self.swept * 2) {
            return;
        }
        self.records
            .retain(|_, record| record.alive.bind(py).upgrade().is_some());
        self.swept = self.records.len();
    }
}

/// Changes an element's row, made where it has none; nothing for an
/// object a weak reference cannot name, which then has no sites. The
/// change runs under the table's lock and must not call into Python; the
/// weak reference a new row needs is made outside it, since making one
/// may collect garbage, and collecting may run code that reads the table.
fn with_row<T>(element: &Bound<'_, PyAny>, change: impl FnOnce(&mut Record) -> T) -> Option<T> {
    if let Some(record) = table().row(element) {
        return Some(change(record));
    }

    let alive = PyWeakrefReference::new(element).ok()?.unbind();
    let mut table = table();
    let key = element.as_ptr() as usize;
    table.records.insert(
        key,
        Record {
            alive,
            entries: Vec::new(),
            active: None,
            stored: None,
            naive: 0.0,
        },
    );
    table.sweep(element.py());
    table.records.get_mut(&key).map(change)
}

/// Writes a stream into the sites of an element it has just put in, and
/// makes it the element's `activeSite`, as music21's `coreInsert` does.
pub(crate) fn entered(element: &Bound<'_, PyAny>, here: &Here, index: u64, offset: f64) {
    let py = element.py();
    with_row(element, |record| {
        // Put in again, it is put in newly, as music21 removes the site
        // first.
        record.entries.retain(|entry| entry.here.key != here.key);
        record.entries.push(Entry {
            here: here.clone_ref(py),
            index,
        });
        record.active = Some(here.clone_ref(py));
        record.stored = Some(offset);
    });
}

/// Makes a stream the `activeSite` of an element reached through it.
pub(crate) fn reached(element: &Bound<'_, PyAny>, here: &Here, offset: f64) {
    let py = element.py();
    with_row(element, |record| {
        record.active = Some(here.clone_ref(py));
        record.stored = Some(offset);
    });
}

/// What an element's row says, read out of the table.
struct Read {
    entries: Vec<(Here, u64)>,
    active: Option<Here>,
    stored: Option<f64>,
    naive: f64,
}

fn read(element: &Bound<'_, PyAny>) -> Read {
    let py = element.py();
    let mut table = table();
    match table.row(element) {
        Some(record) => Read {
            entries: record
                .entries
                .iter()
                .map(|entry| (entry.here.clone_ref(py), entry.index))
                .collect(),
            active: record.active.as_ref().map(|here| here.clone_ref(py)),
            stored: record.stored,
            naive: record.naive,
        },
        None => Read {
            entries: Vec::new(),
            active: None,
            stored: None,
            naive: 0.0,
        },
    }
}

fn write_active(element: &Bound<'_, PyAny>, active: Option<Here>, stored: Option<f64>) {
    with_row(element, |record| {
        record.active = active;
        record.stored = stored;
    });
}

fn write_naive(element: &Bound<'_, PyAny>, naive: f64) -> PyResult<()> {
    match with_row(element, |record| record.naive = naive) {
        Some(()) => Ok(()),
        None => Err(PyAttributeError::new_err(format!(
            "'{}' object cannot keep an offset of its own",
            element.get_type().name()?
        ))),
    }
}

/// Whether a stream holds an element, and where.
enum Holding {
    At(f64),
    No,
    /// The stream is busy being changed and cannot be read.
    Unknown,
}

fn holding(site: &Bound<'_, PyAny>, element: &Bound<'_, PyAny>) -> Holding {
    if let Ok(stream) = site.cast::<Stream>() {
        return match stream.try_borrow() {
            Ok(stream) => stream.offset_in(element).map_or(Holding::No, Holding::At),
            Err(_) => Holding::Unknown,
        };
    }
    match site
        .call_method1("elementOffset", (element,))
        .and_then(|offset| offset.extract::<f64>())
    {
        Ok(offset) => Holding::At(offset),
        Err(_) => Holding::No,
    }
}

/// The streams holding an element, oldest first, each with when it was
/// put there.
fn live_sites<'py>(element: &Bound<'py, PyAny>) -> Vec<(Bound<'py, PyAny>, u64)> {
    let py = element.py();
    read(element)
        .entries
        .into_iter()
        .filter_map(|(here, index)| {
            let site = here.stream(py)?;
            match holding(&site, element) {
                Holding::No => None,
                _ => Some((site, index)),
            }
        })
        .collect()
}

/// music21's `activeSite`: the stream an element was last put in or
/// reached through, while that stream lives and holds it.
fn active_site<'py>(element: &Bound<'py, PyAny>) -> Option<Bound<'py, PyAny>> {
    let py = element.py();
    let site = read(element).active?.stream(py)?;
    match holding(&site, element) {
        Holding::No => {
            write_active(element, None, None);
            None
        }
        _ => Some(site),
    }
}

/// music21's `offset`: where an element stands in its `activeSite`; where
/// that stream is gone, where it stood there; and with none, its own.
fn offset_of(element: &Bound<'_, PyAny>) -> f64 {
    let py = element.py();
    let read = read(element);
    let Some(active) = read.active else {
        return read.naive;
    };
    let Some(site) = active.stream(py) else {
        return read.stored.unwrap_or(0.0);
    };
    match holding(&site, element) {
        Holding::At(offset) => offset,
        Holding::Unknown => read.stored.unwrap_or(0.0),
        Holding::No => {
            write_active(element, None, None);
            read.naive
        }
    }
}

/// The sites in the order music21's `yieldSites` gives them: oldest first,
/// or newest first by creation time, with a priority target first of all.
fn ordered_sites<'py>(
    element: &Bound<'py, PyAny>,
    by_creation: bool,
    first: Option<&Bound<'py, PyAny>>,
) -> Vec<Bound<'py, PyAny>> {
    let mut sites: Vec<Bound<'py, PyAny>> = live_sites(element)
        .into_iter()
        .map(|(site, _)| site)
        .collect();
    if by_creation {
        sites.reverse();
    }
    if let Some(first) = first
        && let Some(at) = sites.iter().position(|site| site.is(first))
    {
        let moved = sites.remove(at);
        sites.insert(0, moved);
    }
    sites
}

/// music21's `recursionType` of a stream: how a context search looks
/// inside it.
fn recursion_of(site: &Bound<'_, PyAny>) -> String {
    if let Ok(stream) = site.cast::<Stream>() {
        let kind = stream
            .try_borrow()
            .map(|stream| stream.kind_now())
            .unwrap_or(StreamKind::Stream);
        return match kind {
            StreamKind::Part | StreamKind::PartStaff => "flatten",
            StreamKind::Score | StreamKind::Opus => "elementsOnly",
            _ => "elementsFirst",
        }
        .to_string();
    }
    site.getattr("recursionType")
        .and_then(|kind| kind.str())
        .map(|kind| kind.to_string())
        .unwrap_or_else(|_| "elementsFirst".to_string())
}

/// Whether a stream is a measure, as music21's `isMeasure` says.
fn is_measure(site: &Bound<'_, PyAny>) -> bool {
    if let Ok(stream) = site.cast::<Stream>() {
        return stream
            .try_borrow()
            .is_ok_and(|stream| stream.kind_now() == StreamKind::Measure);
    }
    site.getattr("isMeasure")
        .and_then(|flag| flag.is_truthy())
        .unwrap_or(false)
}

/// An element's `sortTuple` in a stream holding it.
fn sort_key_in(element: &Bound<'_, PyAny>, site: &Bound<'_, PyAny>) -> Option<SortKey> {
    let stream = site.cast::<Stream>().ok()?;
    stream.try_borrow().ok()?.place_of(element)
}

/// One context music21's `contextSites` yields.
struct Context<'py> {
    site: Bound<'py, PyAny>,
    at: SortKey,
    recursion: String,
}

/// How [`context_sites`] walks.
#[derive(Clone, Copy)]
struct Walk {
    by_creation: bool,
    target_only: bool,
}

/// music21's `contextSites`: each stream an element stands in, and each
/// stream those stand in, outwards, with where the element would stand in
/// each and how a search looks inside it. Derivations are not followed:
/// the package keeps none.
fn context_sites<'py>(element: &Bound<'py, PyAny>, walk: Walk) -> Vec<Context<'py>> {
    let mut memo = HashSet::new();
    let mut out = Vec::new();
    if element.cast::<Stream>().is_ok() {
        // A stream is its own first context, ahead of everything at its
        // start.
        let own = SortKey {
            at_end: 0.0,
            offset: 0.0,
            priority: f64::NEG_INFINITY,
            order: crate::stream::sort_order_of(element) as f64,
            not_grace: 1.0,
            index: 0.0,
        };
        out.push(Context {
            site: element.clone(),
            at: own,
            recursion: recursion_of(element),
        });
        memo.insert(element.as_ptr() as usize);
    }
    contexts_from(element, &mut memo, 0.0, walk, None, &mut out);
    out
}

fn contexts_from<'py>(
    element: &Bound<'py, PyAny>,
    memo: &mut HashSet<usize>,
    offset_append: f64,
    walk: Walk,
    target: Option<Bound<'py, PyAny>>,
    out: &mut Vec<Context<'py>>,
) {
    let target = match target {
        None if !walk.by_creation => active_site(element),
        given => given,
    };
    for site in ordered_sites(element, walk.by_creation, target.as_ref()) {
        if memo.contains(&(site.as_ptr() as usize)) {
            continue;
        }
        let Some(placed) = sort_key_in(element, &site) else {
            continue;
        };
        let position = placed.at(placed.offset + offset_append);
        out.push(Context {
            site: site.clone(),
            at: position,
            recursion: recursion_of(&site),
        });
        memo.insert(site.as_ptr() as usize);

        let mut outer = Vec::new();
        let inner = Walk {
            target_only: false,
            ..walk
        };
        contexts_from(&site, memo, position.offset, inner, None, &mut outer);
        for context in outer {
            if walk.target_only
                && !target
                    .as_ref()
                    .is_some_and(|target| target.is(&context.site))
            {
                break;
            }
            out.push(Context {
                at: position.at(context.at.offset),
                ..context
            });
        }
        if walk.target_only {
            break;
        }
    }
}

/// music21's `ElementSearch`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Search {
    Before,
    After,
    AtOrBefore,
    AtOrAfter,
    BeforeOffset,
    AfterOffset,
    AtOrBeforeOffset,
    AtOrAfterOffset,
    BeforeNotSelf,
    AfterNotSelf,
    All,
}

impl Search {
    fn read(given: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let Some(given) = given else {
            return Ok(Self::AtOrBefore);
        };
        let said = match given.getattr("value") {
            Ok(value) => value.str()?.to_string(),
            Err(_) => given.str()?.to_string(),
        };
        Ok(match said.as_str() {
            "getElementBefore" => Self::Before,
            "getElementAfter" => Self::After,
            "getElementAtOrBefore" => Self::AtOrBefore,
            "getElementAtOrAfter" => Self::AtOrAfter,
            "getElementBeforeOffset" => Self::BeforeOffset,
            "getElementAfterOffset" => Self::AfterOffset,
            "getElementAtOrBeforeOffset" => Self::AtOrBeforeOffset,
            "getElementAtOrAfterOffset" => Self::AtOrAfterOffset,
            "getElementBeforeNotSelf" => Self::BeforeNotSelf,
            "getElementAfterNotSelf" => Self::AfterNotSelf,
            "all" => Self::All,
            _ => {
                return Err(PyValueError::new_err(format!(
                    "Invalid getElementMethod: {said}"
                )));
            }
        })
    }

    fn before(self) -> bool {
        matches!(
            self,
            Self::Before
                | Self::BeforeOffset
                | Self::AtOrBefore
                | Self::AtOrBeforeOffset
                | Self::BeforeNotSelf
        )
    }

    fn after(self) -> bool {
        matches!(
            self,
            Self::After
                | Self::AfterOffset
                | Self::AtOrAfter
                | Self::AtOrAfterOffset
                | Self::AfterNotSelf
        )
    }

    fn at(self) -> bool {
        matches!(
            self,
            Self::AtOrBefore | Self::AtOrAfter | Self::AtOrBeforeOffset | Self::AtOrAfterOffset
        )
    }

    fn not_self(self) -> bool {
        matches!(self, Self::BeforeNotSelf | Self::AfterNotSelf)
    }

    /// Where the search starts: the element's own place, or for a search
    /// by offset alone, before or after all that stands at its offset.
    fn start(self, place: SortKey) -> SortKey {
        match self {
            Self::BeforeOffset | Self::AtOrAfterOffset => SortKey::bound(place.offset, false),
            Self::AfterOffset | Self::AtOrBeforeOffset => SortKey::bound(place.offset, true),
            _ => place,
        }
    }
}

/// The name a class is asked for by, or `None` for any class.
fn class_name(class: Option<&Bound<'_, PyAny>>) -> PyResult<Option<String>> {
    let Some(class) = class.filter(|class| !class.is_none()) else {
        return Ok(None);
    };
    if let Ok(name) = class.extract::<String>() {
        return Ok(Some(name).filter(|name| !name.is_empty()));
    }
    Ok(Some(class.getattr("__name__")?.extract()?))
}

fn answers(object: &Bound<'_, PyAny>, class: Option<&str>) -> PyResult<bool> {
    match class {
        None => Ok(true),
        Some(name) => crate::stream::answers_to_class(
            object,
            pyo3::types::PyString::new(object.py(), name).as_any(),
        ),
    }
}

/// music21's check that what a search found does not contradict the
/// direction searched, where it and the element stand in one stream.
fn well_formed(
    element: &Bound<'_, PyAny>,
    found: &Bound<'_, PyAny>,
    site: &Bound<'_, PyAny>,
    search: Search,
) -> bool {
    let (Some(own), Some(theirs)) = (sort_key_in(element, site), sort_key_in(found, site)) else {
        return true;
    };
    !(search.before() && own.is_before(&theirs) || search.after() && own.is_after(&theirs))
}

/// What a context search found in a site, made the site's own active
/// element where the site holds it, as music21's `coreSelfActiveSite`.
fn found_in<'py>(found: Bound<'py, PyAny>, site: &Bound<'py, PyAny>) -> Bound<'py, PyAny> {
    if let Ok(stream) = site.cast::<Stream>()
        && let Ok(stream) = stream.try_borrow()
    {
        stream.reach(&found);
    }
    found
}

/// The search behind music21's `getContextByClass`.
fn context_by_class<'py>(
    element: &Bound<'py, PyAny>,
    class: Option<&str>,
    search: Search,
    walk: Walk,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    if search.at() && class.is_some() && answers(element, class)? {
        return Ok(Some(element.clone()));
    }

    for context in context_sites(element, walk) {
        let site = &context.site;
        let Ok(stream) = site.cast::<Stream>() else {
            continue;
        };
        let start = search.start(context.at);
        let after = !search.before();
        let site_answers = || answers(site, class);
        let elements_first =
            context.recursion == "elementsOnly" || context.recursion == "elementsFirst";
        if elements_first
            && let Some((found, _)) = Stream::nearest(stream, class, &start, after, false)?
            && well_formed(element, &found, site, search)
        {
            return Ok(Some(found_in(found, site)));
        }
        if context.recursion == "elementsOnly" {
            continue;
        }
        if search.after() && !search.not_self() && site_answers()? {
            return Ok(Some(site.clone()));
        }
        if let Some((found, _)) = Stream::nearest(stream, class, &start, after, true)?
            && well_formed(element, &found, site, search)
        {
            return Ok(Some(found_in(found, site)));
        }
        if search.before() && !(search.not_self() && element.is(site)) && site_answers()? {
            return Ok(Some(site.clone()));
        }
    }
    Ok(None)
}

/// music21's `_getTimeSignatureForBeat`: the meter in force, by offset.
fn meter_for_beat<'py>(element: &Bound<'py, PyAny>) -> PyResult<Option<Bound<'py, PyAny>>> {
    context_by_class(
        element,
        Some("TimeSignature"),
        Search::AtOrBeforeOffset,
        Walk {
            by_creation: false,
            target_only: false,
        },
    )
}

/// The meter in force and where in its bar an element stands.
fn beat_place<'py>(
    element: &Bound<'py, PyAny>,
) -> PyResult<Option<(Bound<'py, PyAny>, Bound<'py, PyAny>)>> {
    let Some(meter) = meter_for_beat(element)? else {
        return Ok(None);
    };
    let place = meter.call_method1("getMeasureOffsetOrMeterModulusOffset", (element,))?;
    Ok(Some((meter, place)))
}

/// music21's `_getMeasureOffset`: where an element stands in the measure
/// holding it, its padding counted, or failing a measure, its `offset`.
fn measure_offset(element: &Bound<'_, PyAny>, padded: bool) -> PyResult<f64> {
    let padding = |measure: &Bound<'_, PyAny>| -> f64 {
        if !padded {
            return 0.0;
        }
        measure
            .getattr("paddingLeft")
            .and_then(|padding| padding.extract::<f64>())
            .unwrap_or(0.0)
    };
    if let Some(active) = active_site(element)
        && is_measure(&active)
        && let Holding::At(offset) = holding(&active, element)
    {
        return Ok(offset + padding(&active));
    }
    let walk = Walk {
        by_creation: true,
        target_only: false,
    };
    if let Some(measure) = context_by_class(element, Some("Measure"), Search::AtOrBefore, walk)?
        && let Holding::At(offset) = holding(&measure, element)
    {
        return Ok(offset + padding(&measure));
    }
    Ok(offset_of(element))
}

// --- what each element answers ---------------------------------------------

/// What a member of a class reads, as music21's `Music21Object` reads it.
fn read_member<'py>(element: &Bound<'py, PyAny>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    let py = element.py();
    let nothing = || py.None().into_bound(py);
    let nan = || pyo3::types::PyFloat::new(py, f64::NAN).into_any();
    Ok(match name {
        "sites" => Bound::new(
            py,
            Sites {
                element: element.clone().unbind(),
            },
        )?
        .into_any(),
        "activeSite" => active_site(element).unwrap_or_else(nothing),
        "offset" => crate::duration::op_frac(py, offset_of(element))?,
        "beat" => match beat_place(element)? {
            Some((meter, place)) => meter.call_method1("getBeatProportion", (place,))?,
            None => nan(),
        },
        "beatStr" => match beat_place(element)? {
            Some((meter, place)) => meter.call_method1("getBeatProportionStr", (place,))?,
            None => pyo3::types::PyString::new(py, "nan").into_any(),
        },
        "beatDuration" => match beat_place(element)? {
            Some((meter, place)) => meter.call_method1("getBeatDuration", (place,))?,
            None => crate::meter::frozen_duration(py, 0.0)?.into_bound(py),
        },
        "beatStrength" => match beat_place(element)? {
            Some((meter, place)) => {
                let keywords = PyDict::new(py);
                keywords.set_item("forcePositionMatch", true)?;
                keywords.set_item("permitMeterModulus", false)?;
                meter.call_method("getAccentWeight", (place,), Some(&keywords))?
            }
            None => nan(),
        },
        "measureNumber" => measure_number(element)?.unwrap_or_else(nothing),
        _ => {
            return Err(PyAttributeError::new_err(format!(
                "'{}' object has no attribute '{name}'",
                element.get_type().name()?
            )));
        }
    })
}

/// music21's `measureNumber`: the number of the measure holding the
/// element, its `activeSite` first, else the last measure among its
/// contexts.
fn measure_number<'py>(element: &Bound<'py, PyAny>) -> PyResult<Option<Bound<'py, PyAny>>> {
    if let Some(active) = active_site(element)
        && is_measure(&active)
    {
        return Ok(Some(active.getattr("number")?));
    }
    let walk = Walk {
        by_creation: false,
        target_only: false,
    };
    let mut number = None;
    for context in context_sites(element, walk) {
        if is_measure(&context.site) {
            number = Some(context.site.getattr("number")?);
        }
    }
    Ok(number)
}

/// What writing a member does: `offset` moves the element in its
/// `activeSite` or sets its own, and `activeSite` names a stream holding
/// it.
fn write_member(element: &Bound<'_, PyAny>, name: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
    match name {
        "offset" => {
            let offset: f64 = value.extract().map_err(|_| {
                PyTypeError::new_err(format!(
                    "Cannot set offset to {}",
                    value
                        .repr()
                        .map(|shown| shown.to_string())
                        .unwrap_or_default()
                ))
            })?;
            match active_site(element) {
                Some(site) => {
                    site.call_method1("setElementOffset", (element, offset))?;
                    Ok(())
                }
                None => write_naive(element, offset),
            }
        }
        "activeSite" => {
            if value.is_none() {
                write_active(element, None, None);
                return Ok(());
            }
            let Ok(offset) = value
                .call_method1("elementOffset", (element,))
                .and_then(|offset| offset.extract::<f64>())
            else {
                return Err(SitesException::new_err(format!(
                    "activeSite cannot be set for object {} not in the Stream {}",
                    element.str()?,
                    value.str()?
                )));
            };
            write_active(element, Some(Here::of(value)?), Some(offset));
            Ok(())
        }
        _ => Err(PyAttributeError::new_err(format!(
            "property '{name}' of '{}' object has no setter",
            element.get_type().name()?
        ))),
    }
}

/// music21's `Music21Object` members the package answers through sites:
/// the properties, then the methods.
const PROPERTIES: [&str; 8] = [
    "sites",
    "activeSite",
    "offset",
    "beat",
    "beatStr",
    "beatStrength",
    "beatDuration",
    "measureNumber",
];

/// A member answered through sites, put on each element class: a property,
/// or a method bound to the element it is read off.
#[pyclass(name = "SiteMember", module = "music21_rs", frozen)]
pub struct SiteMember {
    name: &'static str,
    function: Option<Py<PyAny>>,
}

#[pymethods]
impl SiteMember {
    fn __get__<'py>(
        slf: &Bound<'py, Self>,
        instance: Option<&Bound<'py, PyAny>>,
        _owner: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let Some(instance) = instance.filter(|instance| !instance.is_none()) else {
            return Ok(slf.clone().into_any());
        };
        let me = slf.get();
        match &me.function {
            Some(function) => py
                .import("types")?
                .getattr("MethodType")?
                .call1((function.bind(py), instance)),
            None => read_member(instance, me.name),
        }
    }

    fn __set__(&self, instance: &Bound<'_, PyAny>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        write_member(instance, self.name, value)
    }

    fn __delete__(&self, instance: &Bound<'_, PyAny>) -> PyResult<()> {
        Err(PyAttributeError::new_err(format!(
            "property '{}' of '{}' object has no deleter",
            self.name,
            instance.get_type().name()?
        )))
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.function)
    }

    fn __repr__(&self) -> String {
        format!("<music21_rs site member '{}'>", self.name)
    }
}

// --- the methods ------------------------------------------------------------

/// music21's `getOffsetBySite`: where an element stands in a stream
/// holding it, or its own offset for `None`.
#[pyfunction]
#[pyo3(signature = (element, site, *, returnSpecial = false))]
fn getOffsetBySite<'py>(
    element: &Bound<'py, PyAny>,
    site: Option<&Bound<'py, PyAny>>,
    returnSpecial: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let _ = returnSpecial;
    let py = element.py();
    let Some(site) = site.filter(|site| !site.is_none()) else {
        return crate::duration::op_frac(py, read(element).naive);
    };
    site.call_method1("elementOffset", (element,)).map_err(|_| {
        SitesException::new_err(format!(
            "an entry for this object {} is not stored in stream {}",
            element
                .repr()
                .map(|shown| shown.to_string())
                .unwrap_or_default(),
            site.repr()
                .map(|shown| shown.to_string())
                .unwrap_or_default()
        ))
    })
}

/// music21's `setOffsetBySite`: moves an element in a stream holding it,
/// or sets its own offset for `None`.
#[pyfunction]
fn setOffsetBySite(
    element: &Bound<'_, PyAny>,
    site: Option<&Bound<'_, PyAny>>,
    value: f64,
) -> PyResult<()> {
    match site.filter(|site| !site.is_none()) {
        Some(site) => {
            site.call_method1("setElementOffset", (element, value))?;
            Ok(())
        }
        None => write_naive(element, value),
    }
}

/// music21's `getOffsetInHierarchy`: where an element stands counted from
/// the start of a stream holding it at any depth.
#[pyfunction]
fn getOffsetInHierarchy<'py>(
    element: &Bound<'py, PyAny>,
    site: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    if let Ok(found) = getOffsetBySite(element, Some(site), false) {
        return Ok(found);
    }
    let walk = Walk {
        by_creation: false,
        target_only: false,
    };
    for context in context_sites(element, walk) {
        if context.site.is(site) {
            return crate::duration::op_frac(element.py(), context.at.offset);
        }
    }
    Err(SitesException::new_err(format!(
        "Element {} is not in hierarchy of {}",
        element.str()?,
        site.str()?
    )))
}

static CONTEXT_TUPLE: pyo3::sync::PyOnceLock<Py<PyAny>> = pyo3::sync::PyOnceLock::new();
static SORT_TUPLE: pyo3::sync::PyOnceLock<Py<PyAny>> = pyo3::sync::PyOnceLock::new();

/// music21's `contextSites`, as a list of its `ContextTuple`s -- or with
/// `returnSortTuples`, of `ContextSortTuple`s carrying a `SortTuple`.
#[pyfunction]
#[pyo3(signature = (element, *, callerFirst = None, memo = None, offsetAppend = 0.0, sortByCreationTime = false, priorityTarget = None, returnSortTuples = false, followDerivation = true, priorityTargetOnly = false))]
#[allow(clippy::too_many_arguments)]
fn contextSites<'py>(
    element: &Bound<'py, PyAny>,
    callerFirst: Option<&Bound<'py, PyAny>>,
    memo: Option<&Bound<'py, PyAny>>,
    offsetAppend: f64,
    #[pyo3(from_py_with = crate::truthy)] sortByCreationTime: bool,
    priorityTarget: Option<&Bound<'py, PyAny>>,
    #[pyo3(from_py_with = crate::truthy)] returnSortTuples: bool,
    #[pyo3(from_py_with = crate::truthy)] followDerivation: bool,
    #[pyo3(from_py_with = crate::truthy)] priorityTargetOnly: bool,
) -> PyResult<Bound<'py, PyList>> {
    let _ = (callerFirst, memo, priorityTarget, followDerivation);
    let py = element.py();
    let walk = Walk {
        by_creation: sortByCreationTime,
        target_only: priorityTargetOnly,
    };
    let (name, fields) = if returnSortTuples {
        ("ContextSortTuple", "site offset recurseType")
    } else {
        ("ContextTuple", "site offset recurseType")
    };
    let tuple = crate::music21_or_namedtuple(py, "music21.base", name, fields, &CONTEXT_TUPLE)?;
    let out = PyList::empty(py);
    for context in context_sites(element, walk) {
        let at = context.at.at(context.at.offset + offsetAppend);
        let offset = if returnSortTuples {
            sort_tuple(py, &at)?
        } else {
            crate::duration::op_frac(py, at.offset)?
        };
        out.append(tuple.call1((context.site, offset, context.recursion))?)?;
    }
    Ok(out)
}

/// A `SortKey` as music21's `SortTuple`.
fn sort_tuple<'py>(py: Python<'py>, key: &SortKey) -> PyResult<Bound<'py, PyAny>> {
    let class = crate::music21_or_namedtuple(
        py,
        "music21.sorting",
        "SortTuple",
        "atEnd offset priority classSortOrder isNotGrace insertIndex",
        &SORT_TUPLE,
    )?;
    let whole = |value: f64| -> PyResult<Bound<'py, PyAny>> {
        if value.is_finite() {
            Ok((value as i64).into_pyobject(py)?.into_any())
        } else {
            Ok(pyo3::types::PyFloat::new(py, value).into_any())
        }
    };
    class.call1((
        whole(key.at_end)?,
        crate::duration::op_frac(py, key.offset)?,
        whole(key.priority)?,
        whole(key.order)?,
        whole(key.not_grace)?,
        whole(key.index)?,
    ))
}

/// music21's `getContextByClass`: the nearest element of a class in the
/// element's streams and theirs, outwards.
#[pyfunction]
#[pyo3(signature = (element, className, *, getElementMethod = None, sortByCreationTime = false, followDerivation = true, priorityTargetOnly = false))]
fn getContextByClass<'py>(
    element: &Bound<'py, PyAny>,
    className: Option<&Bound<'py, PyAny>>,
    getElementMethod: Option<&Bound<'py, PyAny>>,
    #[pyo3(from_py_with = crate::truthy)] sortByCreationTime: bool,
    #[pyo3(from_py_with = crate::truthy)] followDerivation: bool,
    #[pyo3(from_py_with = crate::truthy)] priorityTargetOnly: bool,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let search = Search::read(getElementMethod)?;
    if priorityTargetOnly && followDerivation {
        return Err(PyValueError::new_err(
            "priorityTargetOnly and followDerivation cannot both be True",
        ));
    }
    let class = class_name(className)?;
    let walk = Walk {
        by_creation: sortByCreationTime,
        target_only: priorityTargetOnly,
    };
    context_by_class(element, class.as_deref(), search, walk)
}

/// music21's `next`: the next element, of a class where one is given, in
/// the element's streams and theirs.
#[pyfunction]
#[pyo3(signature = (element, className = None, *, activeSiteOnly = false))]
fn next<'py>(
    element: &Bound<'py, PyAny>,
    className: Option<&Bound<'py, PyAny>>,
    #[pyo3(from_py_with = crate::truthy)] activeSiteOnly: bool,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let class = class_name(className)?;
    let walk = Walk {
        by_creation: false,
        target_only: activeSiteOnly,
    };
    let contexts: Vec<Bound<'py, PyAny>> = context_sites(element, walk)
        .into_iter()
        .map(|context| context.site)
        .collect();
    let mut from = element.clone();
    for _ in 0..20 {
        let found = context_by_class(&from, class.as_deref(), Search::AfterNotSelf, walk)?;
        let Some(found) = found else {
            return Ok(None);
        };
        if contexts.iter().any(|site| site.is(&found)) {
            if found.len().unwrap_or(0) > 0 {
                let first = found.get_item(0)?;
                if !first.is(element) {
                    return Ok(Some(first));
                }
            }
            from = found;
            continue;
        }
        if !found.is(element) {
            return Ok(Some(found));
        }
    }
    Err(PyErr::new::<crate::Music21Exception, _>(
        "Maximum recursion!",
    ))
}

/// music21's `previous`: the element before, of a class where one is
/// given, in the element's streams and theirs.
#[pyfunction]
#[pyo3(signature = (element, className = None, *, activeSiteOnly = false))]
fn previous<'py>(
    element: &Bound<'py, PyAny>,
    className: Option<&Bound<'py, PyAny>>,
    #[pyo3(from_py_with = crate::truthy)] activeSiteOnly: bool,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let class = class_name(className)?;
    let walk = Walk {
        by_creation: false,
        target_only: activeSiteOnly,
    };
    let found = context_by_class(element, class.as_deref(), Search::BeforeNotSelf, walk)?;
    let mut in_element = false;
    if element.cast::<Stream>().is_ok()
        && let Some(found) = &found
    {
        let all = Walk {
            by_creation: false,
            target_only: false,
        };
        in_element = context_sites(found, all)
            .iter()
            .any(|context| context.site.is(element));
    }
    if let Some(found) = found
        && found.is_truthy()?
        && !found.is(element)
        && !in_element
    {
        return Ok(Some(found));
    }

    // Up a level: what stands before the element in its `activeSite`.
    let Some(active) = active_site(element) else {
        return Ok(None);
    };
    let Ok(stream) = active.cast::<Stream>() else {
        return Ok(None);
    };
    let Some(place) = sort_key_in(element, &active) else {
        return Ok(None);
    };
    match Stream::nearest(stream, class.as_deref(), &place, false, false)? {
        Some((found, _)) => Ok(Some(found)),
        None if answers(&active, class.as_deref())? => Ok(Some(active)),
        None => Ok(None),
    }
}

/// music21's `_getMeasureOffset`.
#[pyfunction]
#[pyo3(signature = (element, includeMeasurePadding = true))]
fn _getMeasureOffset<'py>(
    element: &Bound<'py, PyAny>,
    #[pyo3(from_py_with = crate::truthy)] includeMeasurePadding: bool,
) -> PyResult<Bound<'py, PyAny>> {
    crate::duration::op_frac(
        element.py(),
        measure_offset(element, includeMeasurePadding)?,
    )
}

/// music21's `_getTimeSignatureForBeat`.
#[pyfunction]
fn _getTimeSignatureForBeat<'py>(element: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    meter_for_beat(element)?.ok_or_else(|| {
        Music21ObjectException::new_err("this object does not have a TimeSignature in Sites")
    })
}

// --- music21's `Sites` ------------------------------------------------------

/// music21's `Sites`: the streams an element stands in, read afresh from
/// the element each time, so it answers for what holds the element now.
#[pyclass(name = "Sites", module = "music21.sites")]
pub struct Sites {
    element: Py<PyAny>,
}

impl Sites {
    fn listed<'py>(
        &self,
        py: Python<'py>,
        by_creation: bool,
        first: Option<&Bound<'py, PyAny>>,
        no_none: bool,
    ) -> Vec<Bound<'py, PyAny>> {
        let element = self.element.bind(py);
        let mut sites = ordered_sites(element, false, None);
        if !no_none {
            sites.insert(0, py.None().into_bound(py));
        }
        if by_creation {
            sites.reverse();
        }
        if let Some(first) = first.filter(|first| !first.is_none())
            && let Some(at) = sites.iter().position(|site| site.is(first))
        {
            let moved = sites.remove(at);
            sites.insert(0, moved);
        }
        sites
    }
}

#[pymethods]
impl Sites {
    /// The streams, oldest first, `None` among them unless `excludeNone`:
    /// music21's `get`.
    #[pyo3(signature = (*, sortByCreationTime = false, priorityTarget = None, excludeNone = false))]
    fn get<'py>(
        &self,
        py: Python<'py>,
        #[pyo3(from_py_with = crate::truthy)] sortByCreationTime: bool,
        priorityTarget: Option<&Bound<'py, PyAny>>,
        #[pyo3(from_py_with = crate::truthy)] excludeNone: bool,
    ) -> Vec<Bound<'py, PyAny>> {
        self.listed(py, sortByCreationTime, priorityTarget, excludeNone)
    }

    /// The same, one at a time: music21's `yieldSites`.
    #[pyo3(signature = (*, excludeNone = false, sortByCreationTime = false, priorityTarget = None))]
    fn yieldSites<'py>(
        &self,
        py: Python<'py>,
        #[pyo3(from_py_with = crate::truthy)] excludeNone: bool,
        #[pyo3(from_py_with = crate::truthy)] sortByCreationTime: bool,
        priorityTarget: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let listed = self.listed(py, sortByCreationTime, priorityTarget, excludeNone);
        Ok(PyList::new(py, listed)?.as_any().try_iter()?.into_any())
    }

    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.yieldSites(py, true, false, None)
    }

    /// How many sites, `None` counted: music21's `len`.
    fn __len__(&self, py: Python<'_>) -> usize {
        self.listed(py, false, None, false).len()
    }

    fn __contains__(&self, py: Python<'_>, site: &Bound<'_, PyAny>) -> bool {
        site.is_none()
            || self
                .listed(py, false, None, true)
                .iter()
                .any(|one| one.is(site))
    }

    /// How many streams hold the element: music21's `getSiteCount`.
    fn getSiteCount(&self, py: Python<'_>) -> usize {
        self.listed(py, false, None, true).len()
    }

    /// The identities of the sites, `None` among them: music21's
    /// `getSiteIds`.
    fn getSiteIds<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PySet>> {
        let ids = PySet::empty(py)?;
        ids.add(py.None())?;
        for site in self.listed(py, false, None, true) {
            ids.add(site.as_ptr() as usize)?;
        }
        Ok(ids)
    }

    /// Whether a site of this identity holds the element: music21's
    /// `hasSiteId`.
    fn hasSiteId(&self, py: Python<'_>, siteId: Option<usize>) -> bool {
        let Some(siteId) = siteId else {
            return true;
        };
        self.listed(py, false, None, true)
            .iter()
            .any(|site| site.as_ptr() as usize == siteId)
    }

    /// The site of this identity: music21's `getById`.
    fn getById<'py>(&self, py: Python<'py>, siteId: Option<usize>) -> PyResult<Bound<'py, PyAny>> {
        let Some(siteId) = siteId else {
            return Ok(py.None().into_bound(py));
        };
        self.listed(py, false, None, true)
            .into_iter()
            .find(|site| site.as_ptr() as usize == siteId)
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err(siteId))
    }

    /// The sites whose class is the one named: music21's
    /// `getSitesByClass`, which compares the class's own name.
    fn getSitesByClass<'py>(
        &self,
        py: Python<'py>,
        className: &Bound<'py, PyAny>,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let name = class_name(Some(className))?.unwrap_or_default();
        let mut found = Vec::new();
        for site in self.listed(py, false, None, true) {
            let own: String = site
                .getattr("classes")
                .and_then(|classes| classes.get_item(0))
                .and_then(|first| first.extract())
                .or_else(|_| site.get_type().name().map(|named| named.to_string()))?;
            if own == name {
                found.push(site);
            }
        }
        Ok(found)
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.element)
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let element = self.element.bind(py);
        Ok(format!("<music21.sites.Sites for {}>", element.repr()?))
    }
}

/// The classes that get the members here: the package's `Music21Object`,
/// which every element class is built on.
fn element_classes(py: Python<'_>) -> Vec<Bound<'_, pyo3::types::PyType>> {
    vec![py.get_type::<crate::base::Music21Object>()]
}

/// Every member put on the classes, which installing replaces with
/// music21's own.
pub(crate) const MEMBERS: [&str; 17] = [
    "sites",
    "activeSite",
    "offset",
    "beat",
    "beatStr",
    "beatStrength",
    "beatDuration",
    "measureNumber",
    "getOffsetBySite",
    "setOffsetBySite",
    "getOffsetInHierarchy",
    "getContextByClass",
    "contextSites",
    "next",
    "previous",
    "_getMeasureOffset",
    "_getTimeSignatureForBeat",
];

/// Puts the members on every element class that does not define them
/// already, and registers `Sites`.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add_class::<Sites>()?;
    let methods: Vec<(&'static str, Py<PyAny>)> = vec![
        (
            "getOffsetBySite",
            wrap_pyfunction!(getOffsetBySite, m)?.into_any().unbind(),
        ),
        (
            "setOffsetBySite",
            wrap_pyfunction!(setOffsetBySite, m)?.into_any().unbind(),
        ),
        (
            "getOffsetInHierarchy",
            wrap_pyfunction!(getOffsetInHierarchy, m)?
                .into_any()
                .unbind(),
        ),
        (
            "getContextByClass",
            wrap_pyfunction!(getContextByClass, m)?.into_any().unbind(),
        ),
        (
            "contextSites",
            wrap_pyfunction!(contextSites, m)?.into_any().unbind(),
        ),
        ("next", wrap_pyfunction!(next, m)?.into_any().unbind()),
        (
            "previous",
            wrap_pyfunction!(previous, m)?.into_any().unbind(),
        ),
        (
            "_getMeasureOffset",
            wrap_pyfunction!(_getMeasureOffset, m)?.into_any().unbind(),
        ),
        (
            "_getTimeSignatureForBeat",
            wrap_pyfunction!(_getTimeSignatureForBeat, m)?
                .into_any()
                .unbind(),
        ),
    ];
    for class in element_classes(py) {
        for name in PROPERTIES {
            if !class.hasattr(name)? {
                class.setattr(
                    name,
                    Bound::new(
                        py,
                        SiteMember {
                            name,
                            function: None,
                        },
                    )?,
                )?;
            }
        }
        for (name, function) in &methods {
            if !class.hasattr(*name)? {
                let member = SiteMember {
                    name,
                    function: Some(function.clone_ref(py)),
                };
                class.setattr(*name, Bound::new(py, member)?)?;
            }
        }
    }
    Ok(())
}
