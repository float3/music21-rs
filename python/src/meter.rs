//! music21's `meter.base` and `meter.core`, over the crate's `TimeSignature`
//! and `MeterTerminal`.
//!
//! music21 derives everything about a meter from a `MeterSequence` partition
//! tree — one for beats, one for beaming, one for display, one for accent
//! weights — and the crate carries all four, so the beat, accent and depth
//! questions are answered from them here. A `MeterTerminal` or
//! `MeterSequence` cannot change once made, as music21's cannot: a meter is
//! divided by assigning it a new sequence, `ts.beatSequence =
//! ts.beatSequence.partitioned(2)`.
//!
//! `SenzaMisuraTimeSignature` and `bestTimeSignature` stay music21's. The
//! first is a meter with no numbers in it at all, which this crate cannot
//! express; the second builds a `TimeSignature` out of a stream, and leaving
//! it alone means music21's own function is run over *our* meters.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::{PyIndexError, PyTypeError, PyValueError, PyZeroDivisionError};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;

use crate::Walkable;
use pyo3::types::PyDict;
use pyo3::types::PyList;
use pyo3::types::PyTuple;

use music21::duration::DurationType as RsDurationType;
use music21::meter::BeamedNote as RsBeamedNote;
use music21::meter::MeterTerminal as RsMeterTerminal;
use music21::meter::OffsetAlign as RsOffsetAlign;
use music21::meter::TimeSignature as RsTimeSignature;
use music21::{
    Duration as RsDuration, FloatType, Rest as RsRest, Stream as RsStream, UnsignedIntegerType,
};

use crate::chord::chord_from_any;
use crate::note::note_from_any;

use crate::duration::{Duration, duration_from_any};

/// The names the `meter` facade replaces in `music21.meter.base`.
pub const NAMES: &[&str] = &["TimeSignature", "MeterException", "TimeSignatureException"];

/// The names this facade replaces in `music21.meter.core`.
pub const CORE_NAMES: &[&str] = &["MeterTerminal", "MeterSequence"];

pyo3::create_exception!(music21_rs_facade, MeterException, crate::Music21Exception);
pyo3::create_exception!(
    music21_rs_facade,
    TimeSignatureException,
    crate::Music21Exception
);

error_into!(meter_error, MeterException);
error_into!(time_signature_error, TimeSignatureException);

/// The ratio music21 reads a value as: `common` and `c` are `4/4`, `cut`
/// and capella's `allabreve` are `2/2`, whatever case they are written in.
/// This is also the value music21 caches a meter's sequences under.
fn named_ratio(value: &str) -> &str {
    match value.to_ascii_lowercase().as_str() {
        "common" | "c" => "4/4",
        "cut" | "allabreve" => "2/2",
        _ => value,
    }
}

/// Refuses a meter counting more than music21's `maxMeterNumerator`, as
/// music21 does, before anything is built for it.
fn checked_numerator(value: &str) -> PyResult<()> {
    let Ok(parts) = RsTimeSignature::parts(value) else {
        return Ok(());
    };
    let denominator = parts
        .iter()
        .map(|(_, denominator)| u128::from(*denominator))
        .fold(1u128, |left, right| {
            let (mut a, mut b) = (left, right);
            while b != 0 {
                (a, b) = (b, a % b);
            }
            left / a.max(1) * right
        });
    let numerator: u128 = parts
        .iter()
        .map(|(numerator, part)| u128::from(*numerator) * (denominator / u128::from(*part).max(1)))
        .sum();
    if numerator > u128::from(music21::meter::MAX_NUMERATOR) {
        return Err(TimeSignatureException::new_err(format!(
            "cannot make a TimeSignature with a numerator above {}: {value}",
            music21::meter::MAX_NUMERATOR
        )));
    }
    Ok(())
}

/// music21 writes `common` and `cut` for the two meters that have names, and
/// takes a bare ratio otherwise. The name is the meter's `symbol`.
fn read_meter(value: &str) -> PyResult<RsTimeSignature> {
    checked_numerator(value)?;
    // music21 reads the two names whatever case they are written in.
    let value = value.trim();
    let (mut meter, symbol) = match value.to_ascii_lowercase().as_str() {
        "common" | "c" => (RsTimeSignature::common(), Some("common")),
        "cut" | "allabreve" => (RsTimeSignature::cut(), Some("cut")),
        _ => (
            RsTimeSignature::from_ratio_string(value).map_err(meter_error)?,
            None,
        ),
    };
    meter.set_symbol(symbol.map(str::to_string));
    Ok(meter)
}

/// music21's second argument partitions the bar into that many parts.
///
/// A partition is exactly what this crate does not model — but asking for as
/// many parts as the meter already has beats asks for the partition it
/// music21 partitions the beat sequence into that many parts and stops
/// there, without dividing each part again — which is what separates this
/// from assigning to `beatCount`.
fn checked_divisions(
    meter: &mut RsTimeSignature,
    divisions: Option<&Bound<'_, PyAny>>,
) -> PyResult<()> {
    let Some(divisions) = divisions.filter(|divisions| !divisions.is_none()) else {
        return Ok(());
    };
    let asked: UnsignedIntegerType = divisions.extract()?;
    meter.divide_beats(asked).map_err(meter_error)
}

/// music21's `TimeSignature`: a numerator over a denominator, and everything
/// that follows from the pair.
#[pyclass(
    name = "TimeSignature",
    module = "music21.meter",
    subclass,
    skip_from_py_object
)]
pub struct TimeSignature {
    pub(crate) inner: RsTimeSignature,
    /// The display, beam, beat and accent sequences as the objects handed
    /// out, in music21's order, each standing for the one `inner` carries:
    /// music21 shares one set between every meter read from the same value.
    sequences: [Option<Py<PyAny>>; 4],
    /// The duration a caller wrote over the bar's own, which is handed back
    /// as given.
    bar_duration: Option<Py<PyAny>>,
}

/// The order music21 keeps a meter's sequences in, in its
/// `_defaultSequenceCache`.
const DISPLAY: usize = 0;
const BEAM: usize = 1;
const BEAT: usize = 2;
const ACCENT: usize = 3;

impl TimeSignature {
    pub(crate) fn wrap(inner: RsTimeSignature) -> Self {
        Self {
            inner,
            sequences: Default::default(),
            bar_duration: None,
        }
    }

    /// The same meter, sharing its sequences, as music21's copy shares them.
    fn duplicate(&self, py: Python<'_>) -> Self {
        Self {
            inner: self.inner.clone(),
            sequences: std::array::from_fn(|index| {
                self.sequences[index]
                    .as_ref()
                    .map(|sequence| sequence.clone_ref(py))
            }),
            bar_duration: None,
        }
    }

    /// One of the four sequences as the crate carries it.
    fn sequence_span(&self, index: usize) -> &RsMeterTerminal {
        match index {
            DISPLAY => self.inner.display_sequence(),
            BEAM => self.inner.beam_sequence(),
            BEAT => self.inner.beat_sequence(),
            _ => self.inner.accent_sequence(),
        }
    }

    /// Writes one of the four sequences into the crate's meter.
    fn set_sequence_span(&mut self, index: usize, span: RsMeterTerminal) {
        match index {
            DISPLAY => self.inner.set_display_sequence(span),
            BEAM => self.inner.set_beam_sequence(span),
            BEAT => self.inner.set_beat_sequence(span),
            _ => self.inner.set_accent_sequence(span),
        }
    }

    /// One of the four sequences as an object: the one held where it still
    /// stands for what the meter carries, and otherwise the object music21's
    /// `makeSequence` would make for it, which is then held.
    fn sequence_object(&mut self, py: Python<'_>, index: usize) -> PyResult<Py<PyAny>> {
        let span = as_sequence(self.sequence_span(index).clone())?;
        if let Some(held) = &self.sequences[index]
            && held
                .bind(py)
                .extract::<PyRef<'_, MeterSequence>>()
                .is_ok_and(|held| held.inner == span)
        {
            return Ok(held.clone_ref(py));
        }
        let made = sequence_from_span(py, span, false)?;
        self.sequences[index] = Some(made.clone_ref(py));
        Ok(made)
    }

    /// Assigns one of the four sequences, keeping the object given.
    fn assign_sequence(&mut self, index: usize, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.set_sequence_span(index, as_sequence(span_of(value)?)?);
        self.sequences[index] = value
            .extract::<PyRef<'_, MeterSequence>>()
            .is_ok()
            .then(|| value.clone().unbind());
        Ok(())
    }

    /// music21's `load` without what it keeps of the meter before: the meter
    /// read from a value, with its sequences taken from music21's
    /// `_defaultSequenceCache` where the same value was read before, and put
    /// there where it was not.
    fn read(py: Python<'_>, value: &str, divisions: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let mut inner = read_meter(value)?;
        checked_divisions(&mut inner, divisions)?;
        let mut meter = Self::wrap(inner);
        let Some(base) = installed_core(py).and_then(|_| {
            py.import("sys")
                .ok()?
                .getattr("modules")
                .ok()?
                .get_item("music21.meter.base")
                .ok()
        }) else {
            return Ok(meter);
        };
        let Ok(cache) = base.getattr("_defaultSequenceCache") else {
            return Ok(meter);
        };
        // A list of divisions is a tuple in the key, as music21 makes it.
        let divisions = match divisions {
            Some(divisions) if divisions.is_instance_of::<PyList>() => {
                PyTuple::new(py, divisions.try_iter()?.collect::<PyResult<Vec<_>>>()?)?.into_any()
            }
            Some(divisions) => divisions.clone(),
            None => py.None().into_bound(py),
        };
        let key = PyTuple::new(
            py,
            [named_ratio(value).into_pyobject(py)?.into_any(), divisions],
        )?;
        // A key that cannot be hashed is not cached, as music21 skips it.
        let Ok(cached) = cache.call_method1("get", (&key,)) else {
            return Ok(meter);
        };
        if !cached.is_none() {
            cache.call_method1("move_to_end", (&key,))?;
            for index in [DISPLAY, BEAM, BEAT, ACCENT] {
                meter.assign_sequence(index, &cached.get_item(index)?)?;
            }
            return Ok(meter);
        }
        let made = [DISPLAY, BEAM, BEAT, ACCENT]
            .into_iter()
            .map(|index| meter.sequence_object(py, index))
            .collect::<PyResult<Vec<_>>>()?;
        cache.set_item(&key, PyTuple::new(py, made)?)?;
        let size: usize = base.getattr("_DEFAULT_SEQUENCE_CACHE_SIZE")?.extract()?;
        while cache.len()? > size {
            let kwargs = PyDict::new(py);
            kwargs.set_item("last", false)?;
            cache.call_method("popitem", (), Some(&kwargs))?;
        }
        Ok(meter)
    }
}

#[pymethods]
impl TimeSignature {
    /// Refuses what music21 answers through a site, which the package
    /// keeps none of.
    fn __getattr__(slf: &Bound<'_, Self>, name: &str) -> PyResult<Py<PyAny>> {
        crate::stream::no_site(slf.as_any(), name)
    }

    /// What music21 says this is: its own class and every class it is a
    /// kind of, as music21's `classes` answers -- so a note is a
    /// `NotRest`, a `GeneralNote` and a `Music21Object` besides.
    #[getter]
    fn classes(slf: &Bound<'_, Self>) -> PyResult<Vec<String>> {
        crate::stream::lineage_of(slf.as_any())
    }
    /// music21's sort order for a meter among things at one offset: after
    /// the clef and the key signature.
    #[classattr]
    fn classSortOrder() -> i32 {
        4
    }

    /// A value of `None` is `4/4`, as music21 reads it.
    #[new]
    #[pyo3(signature = (value = None, divisions = None, **keywords))]
    fn new(
        py: Python<'_>,
        value: Option<String>,
        divisions: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let _ = keywords;
        let divisions = divisions.filter(|divisions| !divisions.is_none());
        Self::read(py, value.as_deref().unwrap_or("4/4"), divisions)
    }

    /// music21 builds in `__init__`, and a Python subclass of this one hands
    /// `__new__` its own arguments before calling up with music21's.
    #[pyo3(signature = (value = None, divisions = None, **keywords))]
    fn __init__(
        mut slf: PyRefMut<'_, Self>,
        value: Option<String>,
        divisions: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let py = slf.py();
        let built = Self::new(py, value, divisions, keywords)?;
        *slf = built;
        Ok(())
    }

    #[getter]
    fn get_symbol(&self) -> &str {
        self.inner.symbol().unwrap_or_default()
    }

    #[setter]
    fn set_symbol(&mut self, symbol: String) {
        self.inner.set_symbol(Some(symbol));
    }

    #[getter]
    fn get_symbolizeDenominator(&self) -> bool {
        self.inner.symbolize_denominator()
    }

    #[setter]
    fn set_symbolizeDenominator(&mut self, #[pyo3(from_py_with = crate::truthy)] symbolize: bool) {
        self.inner.set_symbolize_denominator(symbolize);
    }

    /// music21's `_reprInternal`, which its `__repr__` writes after the class
    /// name: `<music21.meter.TimeSignature 6/8>`.
    fn _reprInternal(&self) -> String {
        self.inner.ratio_string()
    }

    #[getter]
    fn get_numerator(&self) -> UnsignedIntegerType {
        self.inner.numerator()
    }

    /// music21's `numerator` setter, which reads the meter afresh over the
    /// same denominator.
    #[setter]
    fn set_numerator(&mut self, py: Python<'_>, numerator: u64) -> PyResult<()> {
        let ratio = format!("{numerator}/{}", self.inner.denominator());
        self.resetValues(py, Some(ratio), None)
    }

    #[getter]
    fn get_denominator(&self) -> UnsignedIntegerType {
        self.inner.denominator()
    }

    /// music21's `denominator` setter, which reads the meter afresh over the
    /// same numerator.
    #[setter]
    fn set_denominator(&mut self, py: Python<'_>, denominator: u64) -> PyResult<()> {
        let ratio = format!("{}/{denominator}", self.inner.numerator());
        self.resetValues(py, Some(ratio), None)
    }

    /// How music21 writes a meter: `<music21.meter.TimeSignature 3/4>`.
    ///
    /// Installed over music21's, a class answers through music21's own
    /// `Music21Object.__repr__`; this one is installed over nothing, so
    /// without it a meter read as the object it is at an address.
    fn __repr__(&self) -> String {
        format!(
            "<music21.meter.TimeSignature {}>",
            self.inner.ratio_string()
        )
    }

    #[getter]
    fn get_ratioString(&self) -> String {
        self.inner.ratio_string()
    }

    #[setter]
    fn set_ratioString(&mut self, py: Python<'_>, value: &str) -> PyResult<()> {
        self.resetValues(py, Some(value.to_string()), None)
    }

    /// music21's `load`: the meter read from a ratio, keeping how it was
    /// drawn and any bar length written over it, unless the ratio is `c`
    /// or `cut`, which names its symbol.
    #[pyo3(signature = (value, divisions = None))]
    fn load(
        &mut self,
        py: Python<'_>,
        value: &str,
        divisions: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let divisions = divisions.filter(|divisions| !divisions.is_none());
        let mut read = Self::read(py, value, divisions)?;
        if read.inner.symbol().is_none() {
            read.inner
                .set_symbol(self.inner.symbol().map(str::to_string));
        }
        read.inner
            .set_symbolize_denominator(self.inner.symbolize_denominator());
        read.inner
            .set_bar_duration(self.inner.written_bar_duration().cloned());
        read.bar_duration = self.bar_duration.take();
        *self = read;
        Ok(())
    }

    /// music21's `resetValues`: the meter read afresh from a ratio, its
    /// symbol, note-value denominator and written bar length cleared. The
    /// `numerator`, `denominator` and `ratioString` setters all go through
    /// it.
    #[pyo3(signature = (value = None, divisions = None))]
    fn resetValues(
        &mut self,
        py: Python<'_>,
        value: Option<String>,
        divisions: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let divisions = divisions.filter(|divisions| !divisions.is_none());
        *self = Self::read(py, value.as_deref().unwrap_or("4/4"), divisions)?;
        Ok(())
    }

    /// music21's `getMeasureOffsetOrMeterModulusOffset`: where an element
    /// falls in the bar this meter measures.
    ///
    /// The offset within the measure the element sits in, where there is
    /// one. Otherwise its offset in its stream, taken modulo the bar length,
    /// so a meter in a stream with no measures still says where the beat is.
    fn getMeasureOffsetOrMeterModulusOffset<'py>(
        slf: &Bound<'py, Self>,
        el: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let element_offset: f64 = el.call_method0("_getMeasureOffset")?.extract()?;
        let kwargs = PyDict::new(py);
        kwargs.set_item("includeMeasurePadding", false)?;
        let own_offset: f64 = slf
            .as_any()
            .call_method("_getMeasureOffset", (), Some(&kwargs))?
            .extract()?;
        let offset = slf.borrow().inner.offset_in_bar(element_offset, own_offset);
        crate::duration::op_frac(py, offset)
    }

    /// music21's `displaySequence`: how the bar is written.
    #[getter]
    fn displaySequence(&mut self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.sequence_object(py, DISPLAY)
    }

    /// music21's `beatSequence`: how the bar is counted.
    #[getter]
    fn beatSequence(&mut self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.sequence_object(py, BEAT)
    }

    /// music21's `beamSequence`: how the bar is beamed.
    #[getter]
    fn beamSequence(&mut self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.sequence_object(py, BEAM)
    }

    /// music21's `accentSequence`: how the bar is weighted.
    #[getter]
    fn accentSequence(&mut self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.sequence_object(py, ACCENT)
    }

    /// Each of the four is a plain attribute of music21's, and a sequence
    /// cannot change, so a meter is divided by assigning it a new one -- as
    /// music21's own `_setDefaultAccentWeights` does, which runs on one of
    /// these objects since the class replaced is a base of the class
    /// installed.
    #[setter]
    fn set_accentSequence(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.assign_sequence(ACCENT, value)
    }

    #[setter]
    fn set_beatSequence(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.assign_sequence(BEAT, value)
    }

    #[setter]
    fn set_beamSequence(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.assign_sequence(BEAM, value)
    }

    /// music21's `summedNumerator`, which it reads off how the bar is
    /// displayed.
    #[getter]
    fn get_summedNumerator(&self) -> bool {
        self.inner.summed_numerator()
    }

    #[setter]
    fn set_summedNumerator(&mut self, #[pyo3(from_py_with = crate::truthy)] value: bool) {
        self.inner.set_summed_numerator(value);
    }

    #[setter]
    fn set_displaySequence(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.assign_sequence(DISPLAY, value)
    }

    /// music21's `setDisplay`: write the bar a different way without
    /// changing what it counts.
    #[pyo3(signature = (value, partitionRequest = None))]
    fn setDisplay(
        &mut self,
        value: &Bound<'_, PyAny>,
        partitionRequest: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        if partitionRequest.is_some_and(|request| !request.is_none()) {
            return Err(MeterException::new_err(
                "setting a display and partitioning it at once is not modelled",
            ));
        }
        let written: String = value.extract().map_err(|_| {
            MeterException::new_err("a display is written as a ratio, such as 2/8+2/8+2/8")
        })?;
        self.inner.set_display(&written).map_err(meter_error)
    }

    // music21 lets a caller write this over the meter's own and hands that
    // very duration back; `None` clears it. The crate keeps its value, so a
    // copy and a pickle carry it.
    #[setter]
    fn set_barDuration(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let written = match value.is_none() {
            true => None,
            false => Some(duration_from_any(value)?),
        };
        self.bar_duration = written.as_ref().map(|_| value.clone().unbind());
        self.inner.set_bar_duration(written);
        Ok(())
    }

    /// The length of one bar, as music21's shared `FrozenDuration`, or the
    /// duration written over it.
    #[getter]
    fn barDuration(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        match self.inner.written_bar_duration() {
            Some(_) if self.bar_duration.is_some() => Ok(self
                .bar_duration
                .as_ref()
                .map(|written| written.clone_ref(py))
                .unwrap_or_else(|| py.None())),
            // The class installed over music21's where there is one, since a
            // duration is compared by its class as well as its length.
            Some(written) => Duration::object(py, written.clone()),
            None => frozen_duration(py, self.inner.bar_quarter_length()),
        }
    }

    #[getter]
    fn get_beatCount(&self) -> UnsignedIntegerType {
        self.inner.beat_count()
    }

    #[setter]
    fn set_beatCount(&mut self, count: &Bound<'_, PyAny>) -> PyResult<()> {
        // music21 hands this to `partition`, which reads a number or a list
        // of numerators alike.
        if let Ok(asked) = count.extract::<UnsignedIntegerType>() {
            return self.inner.set_beat_count(asked).map_err(|_| {
                TimeSignatureException::new_err(format!(
                    "cannot partition beat with provided value: {asked}"
                ))
            });
        }
        let numerators: Vec<UnsignedIntegerType> = count.extract().map_err(|_| {
            TimeSignatureException::new_err(
                "a bar is counted in a number of beats, or in a list of them",
            )
        })?;
        let mut beats = self
            .inner
            .beat_sequence()
            .partitioned_by_list(&numerators)
            .map_err(|_| {
                TimeSignatureException::new_err(format!(
                    "cannot partition beat with provided value: {numerators:?}"
                ))
            })?;
        if beats.len() > 1 {
            beats = beats
                .subdivided_partitions_equal(None)
                .map_err(meter_error)?;
        }
        self.inner.set_beat_sequence(beats);
        Ok(())
    }

    #[getter]
    fn beatCountName(&self) -> String {
        self.inner.beat_count_name()
    }

    /// music21's `beatDuration`: the first beat, as a `FrozenDuration`.
    #[getter]
    fn beatDuration(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        frozen_duration(py, self.inner.beat_quarter_length())
    }

    #[getter]
    fn beatDivisionCount(&self) -> UnsignedIntegerType {
        self.inner.beat_division_count()
    }

    #[getter]
    fn beatDivisionCountName(&self) -> &'static str {
        self.inner.beat_division_count_name()
    }

    #[getter]
    fn beatDivisionDurations(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        self.inner
            .beat_division_quarter_lengths()
            .map_err(time_signature_error)?
            .into_iter()
            .map(|quarter_length| frozen_duration(py, quarter_length))
            .collect()
    }

    #[getter]
    fn beatSubDivisionDurations(&self) -> PyResult<Vec<Duration>> {
        Ok(self
            .inner
            .beat_sub_division_durations()
            .map_err(time_signature_error)?
            .into_iter()
            .map(Duration::wrap)
            .collect())
    }

    #[getter]
    fn classification(&self) -> String {
        self.inner.classification()
    }

    #[getter]
    fn beatLengthToQuarterLengthRatio(&self) -> FloatType {
        self.inner.beat_length_to_quarter_length_ratio()
    }

    #[getter]
    fn quarterLengthToBeatLengthRatio(&self) -> FloatType {
        self.inner.quarter_length_to_beat_length_ratio()
    }

    /// music21's `getBeatDuration`: how long the beat at an offset is. A bar
    /// written additively has beats of different lengths, so this answers
    /// along the bar rather than the same everywhere in it.
    fn getBeatDuration(&self, py: Python<'_>, qLenPos: FloatType) -> PyResult<Py<PyAny>> {
        self.checked_offset(qLenPos)?;
        let beat = self.inner.beat_duration_at(qLenPos).map_err(meter_error)?;
        frozen_duration(py, beat.quarter_length())
    }

    fn getBeatOffsets(&self) -> Vec<FloatType> {
        self.inner.beat_offsets()
    }

    fn getBeat(&self, qLenPos: FloatType) -> PyResult<UnsignedIntegerType> {
        self.inner.beat_at_offset(qLenPos).map_err(meter_error)
    }

    fn getBeatProgress<'py>(
        &self,
        py: Python<'py>,
        qLenPos: FloatType,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let (beat, offset) = self.inner.beat_progress(qLenPos).map_err(meter_error)?;
        PyTuple::new(
            py,
            [
                beat.into_pyobject(py)?.into_any(),
                // A `Fraction` where the progress is not a binary fraction,
                // as music21's `opFrac` answers.
                crate::duration::op_frac(py, offset)?,
            ],
        )
    }

    /// Through `opFrac`, as music21's is: a beat a third of the way in is
    /// `Fraction(4, 3)`, not a float that only nearly says it.
    fn getBeatProportion<'py>(
        &self,
        py: Python<'py>,
        qLenPos: FloatType,
    ) -> PyResult<Bound<'py, PyAny>> {
        let proportion = self.inner.beat_proportion(qLenPos).map_err(meter_error)?;
        crate::duration::op_frac(py, proportion)
    }

    fn getBeatProportionStr(&self, qLenPos: FloatType) -> PyResult<String> {
        self.inner
            .beat_proportion_string(qLenPos)
            .map_err(meter_error)
    }

    fn getOffsetFromBeat<'py>(
        &self,
        py: Python<'py>,
        beat: FloatType,
    ) -> PyResult<Bound<'py, PyAny>> {
        let offset = self.inner.offset_from_beat(beat).map_err(meter_error)?;
        // music21 writes an offset through `opFrac`, so a third of a beat is
        // a Fraction rather than a float that nearly is one.
        crate::duration::op_frac(py, offset)
    }

    /// music21's `getBeams`: how a run of notes is beamed under this meter.
    ///
    /// Takes a stream or a list of objects, as music21 does, and answers one
    /// `Beams` or `None` for each.
    #[pyo3(signature = (srcList, measureStartOffset = 0.0))]
    fn getBeams<'py>(
        &self,
        py: Python<'py>,
        srcList: &Bound<'py, PyAny>,
        measureStartOffset: FloatType,
    ) -> PyResult<Bound<'py, PyList>> {
        // A run out of a Measure keeps its padding: a beam does not stop at
        // the end of a measure that is not full.
        let padding = srcList
            .getattr("paddingRight")
            .and_then(|padding| padding.call_method0("__float__"))
            .and_then(|padding| padding.extract::<FloatType>())
            .ok();
        // music21 lays a bare list end to end by appending it to a fresh
        // Measure; a stream's own offsets are read as they stand.
        let from_stream = srcList.hasattr("elements").unwrap_or(false);

        let mut notes = Vec::new();
        let mut laid_out = 0.0;
        for element in srcList.walk()? {
            let element = element?;
            let duration = element.getattr("duration")?;
            let quarter_length: FloatType = duration
                .getattr("quarterLength")?
                .call_method0("__float__")?
                .extract()?;
            let offset = if from_stream {
                element
                    .getattr("offset")
                    .and_then(|offset| offset.call_method0("__float__"))
                    .and_then(|offset| offset.extract::<FloatType>())
                    .unwrap_or(laid_out)
            } else {
                laid_out
            };
            let written: String = duration.getattr("type")?.extract()?;
            let sounds = element
                .getattr("classSet")
                .and_then(|classes| classes.contains("NotRest"))
                .unwrap_or(false);
            // A value this crate cannot name carries no beam, but still takes
            // its place, so its neighbours know they cannot beam to it.
            let duration_type =
                RsDurationType::from_music21_name(&written).unwrap_or(RsDurationType::Whole);
            notes.push(RsBeamedNote {
                offset,
                quarter_length,
                duration_type,
                sounds: sounds && RsDurationType::from_music21_name(&written).is_some(),
            });
            laid_out += quarter_length;
        }

        let beamed = self
            .inner
            .beams_for(&notes, measureStartOffset, padding)
            .map_err(meter_error)?;
        crate::notation::beams_list(py, beamed)
    }

    /// music21's `averageBeatStrength`: the mean accent weight of the
    /// offsets of everything in a stream, or of its notes alone, each read
    /// against the bar.
    #[pyo3(signature = (streamIn, notesOnly = true))]
    fn averageBeatStrength(
        &self,
        streamIn: &Bound<'_, PyAny>,
        #[pyo3(from_py_with = crate::truthy)] notesOnly: bool,
    ) -> PyResult<FloatType> {
        let elements = if notesOnly {
            streamIn.getattr("notes")?
        } else {
            streamIn.clone()
        };
        let bar = self.inner.bar_quarter_length();
        let mut total = 0.0;
        let mut count = 0_usize;
        for element in elements.walk()? {
            let offset: FloatType = element?
                .getattr("offset")?
                .call_method0("__float__")?
                .extract()?;
            total += self
                .inner
                .accent_weight_with(offset.rem_euclid(bar), true, false)
                .map_err(meter_error)?;
            count += 1;
        }
        if count == 0 {
            return Ok(0.0);
        }
        Ok(total / count as FloatType)
    }

    /// music21's `getAccent`: whether the offset starts one of the default
    /// accent partitions.
    fn getAccent(&self, qLenPos: FloatType) -> bool {
        self.inner.accent(qLenPos)
    }

    /// music21's `getAccentWeight`: the weight of the accent partition an
    /// offset falls in, at a level of the accent sequence.
    #[pyo3(signature = (qLenPos, level = 0, forcePositionMatch = false, permitMeterModulus = false))]
    fn getAccentWeight(
        &self,
        qLenPos: FloatType,
        level: u32,
        #[pyo3(from_py_with = crate::truthy)] forcePositionMatch: bool,
        #[pyo3(from_py_with = crate::truthy)] permitMeterModulus: bool,
    ) -> PyResult<FloatType> {
        self.inner
            .accent_weight_at_level(
                qLenPos,
                level as usize,
                forcePositionMatch,
                permitMeterModulus,
            )
            .map_err(meter_error)
    }

    /// music21's `setAccentWeight`: weigh the accent partitions of a level,
    /// looping the weights given over them.
    #[pyo3(signature = (weights, level = 0))]
    fn setAccentWeight(&mut self, weights: &Bound<'_, PyAny>, level: u32) -> PyResult<()> {
        let weights = match weights.extract::<Vec<FloatType>>() {
            Ok(weights) => weights,
            Err(_) => vec![weights.extract::<FloatType>()?],
        };
        self.inner
            .set_accent_weight(&weights, level as usize)
            .map_err(meter_error)
    }

    /// music21's `getBeatDepth`, quantized to the beat's division as its
    /// default alignment is.
    #[pyo3(signature = (qLenPos, align = "quantize"))]
    fn getBeatDepth(&self, qLenPos: FloatType, align: &str) -> PyResult<u8> {
        if align != "quantize" {
            return Err(MeterException::new_err(format!(
                "the beat depth is read with the quantize alignment here, not {align}"
            )));
        }
        self.inner.beat_depth(qLenPos).map_err(meter_error)
    }

    fn ratioEqual(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Self>>()
            .is_ok_and(|other| self.inner.ratio_equal(&other.inner))
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        for sequence in &self.sequences {
            visit.call(sequence)?;
        }
        visit.call(&self.bar_duration)?;
        Ok(())
    }

    fn __clear__(&mut self) {
        self.sequences = Default::default();
        self.bar_duration = None;
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.borrow().duplicate(slf.py());
        crate::copy_as_same_type(slf, copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.borrow().duplicate(slf.py());
        crate::copy_as_same_type(slf, copied)
    }

    /// music21 freezes a score by pickling it, and what this object is lives
    /// in Rust where a pickle cannot see it.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let (inner, extra) = crate::unpickled_extra::<_, RsTimeSignature>(slf, state)?;
        let Some(inner) = inner else {
            return Ok(());
        };
        let mut me = slf.borrow_mut();
        me.inner = inner;
        if let Some(extra) = extra
            && let Ok(extra) = extra.bind(py).cast::<pyo3::types::PyDict>()
        {
            // A pickle written before the crate carried how a meter is drawn
            // says it beside the value.
            if let Some(symbol) = extra.get_item("symbol")? {
                me.inner.set_symbol(Some(symbol.extract()?));
            }
            if let Some(symbolize) = extra.get_item("symbolizeDenominator")? {
                me.inner.set_symbolize_denominator(symbolize.extract()?);
            }
            // A pickle written before the crate carried a written bar
            // length says it beside the value.
            if let Some(bar) = extra.get_item("barDuration")?
                && !bar.is_none()
            {
                me.inner.set_bar_duration(Some(duration_from_any(&bar)?));
            }
        }
        Ok(())
    }
}

impl TimeSignature {
    /// music21 reads an offset from the start of the bar, and reports one
    /// past the end of it rather than answering for a bar it is not in.
    fn checked_offset(&self, offset: FloatType) -> PyResult<()> {
        if offset < 0.0 || offset >= self.inner.bar_quarter_length() {
            return Err(MeterException::new_err(format!(
                "cannot access from qLenPos {offset} where total duration is {}",
                self.inner.bar_quarter_length()
            )));
        }
        Ok(())
    }
}

/// The music21 duration module where music21 is loaded, whose
/// `FrozenDuration` a meter answers its lengths in.
fn music21_duration_module(py: Python<'_>) -> Option<Bound<'_, PyAny>> {
    let modules = py.import("sys").ok()?.getattr("modules").ok()?;
    modules.get_item("music21.duration").ok()
}

/// The `FrozenDuration`s handed out so far, one per length, so every `3/8`
/// answers the same object as music21's cached ones do.
static FROZEN_DURATIONS: PyOnceLock<Py<PyDict>> = PyOnceLock::new();

/// A length as music21's `FrozenDuration`, which is what a meter answers its
/// bar, its beats and its spans in. music21's own class is the one used where
/// music21 is loaded, since it is a value that cannot change and every
/// meter shares; a wheel standing alone answers a `Duration`.
pub(crate) fn frozen_duration(py: Python<'_>, quarter_length: FloatType) -> PyResult<Py<PyAny>> {
    let Some(module) = music21_duration_module(py) else {
        return Duration::object(py, RsDuration::new(quarter_length).map_err(meter_error)?);
    };
    let cache = FROZEN_DURATIONS.get_or_init(py, || PyDict::new(py).unbind());
    let cache = cache.bind(py);
    let key = quarter_length.to_bits();
    if let Some(found) = cache.get_item(key)? {
        return Ok(found.unbind());
    }
    let kwargs = PyDict::new(py);
    kwargs.set_item("quarterLength", quarter_length)?;
    let made = module
        .getattr("FrozenDuration")?
        .call((), Some(&kwargs))?
        .unbind();
    cache.set_item(key, &made)?;
    Ok(made)
}

/// music21's `meter.core`, where this package's classes are installed over
/// its own: its `getMeterTerminal` and `makeSequence` caches are then the
/// ones that decide which object a span is.
fn installed_core(py: Python<'_>) -> Option<Bound<'_, PyAny>> {
    crate::installed_class(py, "music21.meter.core", "MeterSequence")?;
    let modules = py.import("sys").ok()?.getattr("modules").ok()?;
    modules.get_item("music21.meter.core").ok()
}

/// The terminals handed out so far, one per ratio and weight, where music21
/// is not installed over: music21's `getMeterTerminal` keeps the same.
static TERMINALS: PyOnceLock<Py<PyDict>> = PyOnceLock::new();

/// How many terminals are kept, as music21 keeps its 1024 most recent.
const TERMINALS_KEPT: usize = 1024;

/// A terminal as the one object music21 would hand back for it: what its
/// `getMeterTerminal` answers, where it is installed over.
fn terminal_object(py: Python<'_>, span: RsMeterTerminal) -> PyResult<Py<PyAny>> {
    if let Some(core) = installed_core(py) {
        let kwargs = PyDict::new(py);
        kwargs.set_item("weight", span.weight())?;
        return Ok(core
            .getattr("getMeterTerminal")?
            .call((span.numerator(), span.denominator()), Some(&kwargs))?
            .unbind());
    }
    let cache = TERMINALS.get_or_init(py, || PyDict::new(py).unbind());
    let cache = cache.bind(py);
    let key = (
        span.numerator(),
        span.denominator(),
        span.weight().to_bits(),
    );
    if let Some(found) = cache.get_item(key)? {
        return Ok(found.unbind());
    }
    if cache.len() >= TERMINALS_KEPT {
        cache.clear();
    }
    let made = crate::installed_new(
        py,
        "music21.meter.core",
        "MeterTerminal",
        MeterTerminal::wrap(span),
    )?
    .into_any();
    cache.set_item(key, &made)?;
    Ok(made)
}

/// A span as music21 would hand it back: a `MeterTerminal` where nothing
/// divides it, a `MeterSequence` where something does.
fn node_object(py: Python<'_>, span: RsMeterTerminal) -> PyResult<Py<PyAny>> {
    if span.is_empty() {
        terminal_object(py, span)
    } else {
        sequence_from_span(py, span, false)
    }
}

/// A sequence of the parts given, as music21's `makeSequence` makes one:
/// the same object for the same parts, where music21 is installed over.
fn make_sequence(
    py: Python<'_>,
    inner: RsMeterTerminal,
    parts: Vec<Py<PyAny>>,
    parenthesis: bool,
) -> PyResult<Py<PyAny>> {
    if let Some(core) = installed_core(py)
        && !inner.is_empty()
    {
        let kwargs = PyDict::new(py);
        kwargs.set_item("parenthesis", parenthesis)?;
        kwargs.set_item("summedNumerator", inner.summed_numerator())?;
        return Ok(core
            .getattr("makeSequence")?
            .call(
                (
                    inner.numerator(),
                    inner.denominator(),
                    PyTuple::new(py, parts)?,
                ),
                Some(&kwargs),
            )?
            .unbind());
    }
    let mut sequence = MeterSequence::holding(inner, parenthesis);
    sequence.parts = Some(parts);
    sequence_object(py, sequence)
}

/// A span as a sequence object, its parts as the objects music21 would hand
/// back for them.
fn sequence_from_span(
    py: Python<'_>,
    span: RsMeterTerminal,
    parenthesis: bool,
) -> PyResult<Py<PyAny>> {
    let parts = span
        .parts()
        .iter()
        .map(|part| node_object(py, part.clone()))
        .collect::<PyResult<Vec<_>>>()?;
    make_sequence(py, span, parts, parenthesis)
}

/// A sequence as the class standing in for music21's.
fn sequence_object(py: Python<'_>, sequence: MeterSequence) -> PyResult<Py<PyAny>> {
    Ok(crate::installed_new(py, "music21.meter.core", "MeterSequence", sequence)?.into_any())
}

/// A whole sequence, which a span nothing divides is made into by holding
/// it as its one part: music21's `MeterSequence('4/4')` is `{4/4}`.
fn as_sequence(span: RsMeterTerminal) -> PyResult<RsMeterTerminal> {
    if !span.is_empty() {
        return Ok(span);
    }
    RsMeterTerminal::sequence(span.numerator(), span.denominator(), vec![span]).map_err(meter_error)
}

/// What a list of parts comes to, without reducing it: music21's
/// `fractionSum`, so `3/8+1/8` is `4/8`.
fn ratio_of(parts: &[RsMeterTerminal]) -> (UnsignedIntegerType, UnsignedIntegerType) {
    let denominator = parts
        .iter()
        .map(RsMeterTerminal::denominator)
        .reduce(num_integer_lcm)
        .unwrap_or(1);
    let numerator = parts
        .iter()
        .map(|part| part.numerator() * (denominator / part.denominator()))
        .sum();
    (numerator, denominator)
}

fn num_integer_lcm(left: UnsignedIntegerType, right: UnsignedIntegerType) -> UnsignedIntegerType {
    let (mut a, mut b) = (left, right);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    left / a * right
}

/// A sequence made of the parts given, written over what they come to.
fn joined(parts: Vec<RsMeterTerminal>) -> PyResult<RsMeterTerminal> {
    if parts.is_empty() {
        return RsMeterTerminal::new(1, 1).map_err(meter_error);
    }
    let (numerator, denominator) = ratio_of(&parts);
    RsMeterTerminal::sequence(numerator, denominator, parts).map_err(meter_error)
}

/// The error music21 raises for the methods that changed a sequence in
/// place, naming the method that hands back a new one.
fn removed_in_place(old: &str, new: &str) -> PyErr {
    MeterException::new_err(format!(
        "MeterSequence is immutable, so {old}() cannot change it in place. \
         Use the new sequence that {new}() returns: ms = ms.{new}(...), or for a \
         TimeSignature, ts.beatSequence = ts.beatSequence.{new}(...)"
    ))
}

/// The error music21 raises for setting anything on a value that cannot
/// change.
fn immutable(class: &str) -> PyErr {
    PyTypeError::new_err(format!("This {class} instance is immutable."))
}

/// The keywords a `modify` was handed that it does not take, as music21
/// names them: a sorted Python list.
fn unexpected_keywords(
    keywords: Option<&Bound<'_, PyDict>>,
    allowed: &[&str],
) -> PyResult<Option<String>> {
    let Some(keywords) = keywords else {
        return Ok(None);
    };
    let mut extra: Vec<String> = Vec::new();
    for key in keywords.keys() {
        let key: String = key.extract()?;
        if !allowed.contains(&key.as_str()) {
            extra.push(key);
        }
    }
    if extra.is_empty() {
        return Ok(None);
    }
    extra.sort();
    let listed: Vec<String> = extra.iter().map(|key| format!("'{key}'")).collect();
    Ok(Some(format!("[{}]", listed.join(", "))))
}

/// music21's `MeterTerminal`: a span of a bar that nothing divides. It
/// cannot change once made.
#[pyclass(
    name = "MeterTerminal",
    module = "music21.meter.core",
    subclass,
    skip_from_py_object
)]
pub struct MeterTerminal {
    inner: RsMeterTerminal,
    /// music21's `_frozenHash`: nothing until the hash is first asked for.
    frozen_hash: std::sync::OnceLock<isize>,
}

impl MeterTerminal {
    fn wrap(inner: RsMeterTerminal) -> Self {
        Self {
            inner,
            frozen_hash: std::sync::OnceLock::new(),
        }
    }
}

/// music21's `MeterSequence`: a span of a bar made of other spans. It
/// cannot change once made.
#[pyclass(
    name = "MeterSequence",
    module = "music21.meter.core",
    subclass,
    skip_from_py_object
)]
pub struct MeterSequence {
    /// The span, which holds its parts; one with none is the empty sequence.
    inner: RsMeterTerminal,
    /// music21's `parenthesis`.
    parenthesis: bool,
    /// The parts as the objects music21 hands back: the ones the sequence was
    /// made of where it was handed objects, built the first time one is asked
    /// for otherwise, and the same objects after that.
    parts: Option<Vec<Py<PyAny>>>,
    /// music21's `_frozenHash`: nothing until the hash is first asked for.
    frozen_hash: std::sync::OnceLock<isize>,
    /// Whether the slots can still be written. music21's `makeSequence`
    /// writes them into a blank sequence one at a time, `summedNumerator`
    /// last; after that, and for any sequence made whole, they cannot.
    sealed: bool,
}

impl MeterSequence {
    fn holding(inner: RsMeterTerminal, parenthesis: bool) -> Self {
        Self {
            inner,
            parenthesis,
            parts: None,
            frozen_hash: std::sync::OnceLock::new(),
            sealed: true,
        }
    }

    /// Refuses writing a slot of a sequence already made.
    fn writable(&self) -> PyResult<()> {
        if self.sealed {
            return Err(immutable("MeterSequence"));
        }
        Ok(())
    }

    /// The part objects, built once.
    fn parts_of(slf: &Bound<'_, Self>) -> PyResult<Vec<Py<PyAny>>> {
        let py = slf.py();
        if slf.borrow().parts.is_none() {
            let spans: Vec<RsMeterTerminal> = slf.borrow().inner.parts().to_vec();
            let built = spans
                .into_iter()
                .map(|span| node_object(py, span))
                .collect::<PyResult<Vec<_>>>()?;
            slf.borrow_mut().parts = Some(built);
        }
        Ok(slf
            .borrow()
            .parts
            .as_ref()
            .map(|parts| parts.iter().map(|part| part.clone_ref(py)).collect())
            .unwrap_or_default())
    }

    /// A new sequence like this one with different parts, kept as the
    /// objects given.
    fn rebuilt(
        &self,
        py: Python<'_>,
        inner: RsMeterTerminal,
        parts: Option<Vec<Py<PyAny>>>,
    ) -> PyResult<Py<PyAny>> {
        match parts {
            Some(parts) => make_sequence(py, inner, parts, self.parenthesis),
            None => sequence_from_span(py, inner, self.parenthesis),
        }
    }

    /// A new sequence like this one with a different span.
    fn with_inner(&self, py: Python<'_>, inner: RsMeterTerminal) -> PyResult<Py<PyAny>> {
        self.rebuilt(py, inner, None)
    }
}

/// The span a caller handed in, whichever of the two classes it is, or a
/// ratio written out.
fn span_of(value: &Bound<'_, PyAny>) -> PyResult<RsMeterTerminal> {
    if let Ok(sequence) = value.extract::<PyRef<'_, MeterSequence>>() {
        return Ok(sequence.inner.clone());
    }
    if let Ok(terminal) = value.extract::<PyRef<'_, MeterTerminal>>() {
        return Ok(terminal.inner.clone());
    }
    if let Ok(written) = value.extract::<String>() {
        return RsMeterTerminal::from_ratio_string(&written).map_err(meter_error);
    }
    if value.hasattr("numerator")? && value.hasattr("denominator")? {
        return read_span(value);
    }
    Err(MeterException::new_err(format!(
        "values in MeterSequences must be MeterTerminals or MeterSequences, not {}",
        value.repr()?
    )))
}

/// music21's `subdivide` dispatch over any span: a count, or a list of
/// numerators or of ratios written out.
fn subdivided(span: &RsMeterTerminal, value: &Bound<'_, PyAny>) -> PyResult<RsMeterTerminal> {
    if let Ok(count) = value.extract::<usize>() {
        return span.subdivide_by_count(count).map_err(meter_error);
    }
    if value.extract::<String>().is_err()
        && let Ok(items) = value.walk()
    {
        let items = items.collect::<PyResult<Vec<_>>>()?;
        return subdivided_by_list(span, &items);
    }
    Err(MeterException::new_err(format!(
        "cannot process partition argument {}",
        value.str()?
    )))
}

/// music21's `subdivideByList`: this span as a sequence of one part, then
/// partitioned by the list.
fn subdivided_by_list(
    span: &RsMeterTerminal,
    items: &[Bound<'_, PyAny>],
) -> PyResult<RsMeterTerminal> {
    let whole = RsMeterTerminal::new(span.numerator(), span.denominator())
        .map_err(meter_error)?
        .with_weight(span.weight());
    partitioned_by_list(&whole, items)
}

/// music21's `partitionedByList`: a list of numerators, or of ratios
/// written out, which have to come to the length of the span.
fn partitioned_by_list(
    span: &RsMeterTerminal,
    items: &[Bound<'_, PyAny>],
) -> PyResult<RsMeterTerminal> {
    let Some(first) = items.first() else {
        return Err(PyIndexError::new_err("list index out of range"));
    };
    if first.extract::<String>().is_ok()
        || first.extract::<PyRef<'_, MeterTerminal>>().is_ok()
        || first.extract::<PyRef<'_, MeterSequence>>().is_ok()
    {
        let mut written = Vec::with_capacity(items.len());
        let mut described = Vec::with_capacity(items.len());
        for item in items {
            let part = span_of(item)?;
            written.push(format!("{}/{}", part.numerator(), part.denominator()));
            described.push(item.repr()?.to_string());
        }
        let borrowed: Vec<&str> = written.iter().map(String::as_str).collect();
        return span.partitioned_by_parts(&borrowed).map_err(|_| {
            MeterException::new_err(format!(
                "Cannot set partition by [{}]",
                described.join(", ")
            ))
        });
    }
    let numerators = items
        .iter()
        .map(|item| item.extract::<UnsignedIntegerType>())
        .collect::<PyResult<Vec<_>>>()?;
    span.partitioned_by_list(&numerators).map_err(meter_error)
}

/// music21's `partitioned` dispatch: a count, a list, or another sequence.
fn partitioned(
    span: &RsMeterTerminal,
    value: &Bound<'_, PyAny>,
    load_default: bool,
) -> PyResult<RsMeterTerminal> {
    if let Ok(other) = value.extract::<PyRef<'_, MeterSequence>>() {
        return span.partitioned_by_other(&other.inner).map_err(meter_error);
    }
    if let Ok(count) = value.extract::<usize>() {
        return span
            .partitioned_by_count(count, load_default)
            .map_err(meter_error);
    }
    if value.extract::<String>().is_err()
        && let Ok(items) = value.walk()
    {
        let items = items.collect::<PyResult<Vec<_>>>()?;
        return partitioned_by_list(span, &items);
    }
    Err(MeterException::new_err(format!(
        "cannot process partition argument {}",
        value.str()?
    )))
}

/// Value equality and hashing for both classes, as music21's frozen
/// objects compare their slots.
fn same_value(left: &RsMeterTerminal, right: &RsMeterTerminal) -> bool {
    left == right
}

fn value_hash(span: &RsMeterTerminal, parenthesis: bool) -> isize {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    span.hash(&mut hasher);
    parenthesis.hash(&mut hasher);
    hasher.finish() as isize
}

#[pymethods]
impl MeterTerminal {
    #[new]
    #[pyo3(signature = (slashNotation = None, weight = 1.0, *, numerator = 0, denominator = 1))]
    fn new(
        slashNotation: Option<&str>,
        weight: FloatType,
        numerator: UnsignedIntegerType,
        denominator: UnsignedIntegerType,
    ) -> PyResult<Self> {
        let inner = match slashNotation {
            Some(written) => RsMeterTerminal::from_ratio_string(written).map_err(meter_error)?,
            None => RsMeterTerminal::new(numerator, denominator).map_err(meter_error)?,
        };
        Ok(Self::wrap(inner.with_weight(weight)))
    }

    /// music21's `_frozenHash`: `None` until the terminal is hashed.
    #[getter]
    fn _frozenHash(&self) -> Option<isize> {
        self.frozen_hash.get().copied()
    }

    /// music21's `_durationFromNumeratorDenominator`: 3/8 is a dotted
    /// quarter.
    #[staticmethod]
    fn _durationFromNumeratorDenominator(
        py: Python<'_>,
        numerator: UnsignedIntegerType,
        denominator: UnsignedIntegerType,
    ) -> PyResult<Py<PyAny>> {
        duration_of(py, numerator, denominator)
    }

    /// A terminal cannot change, so a copy of one is itself.
    fn __copy__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    #[pyo3(signature = (memo = None))]
    fn __deepcopy__(slf: Py<Self>, memo: Option<&Bound<'_, PyAny>>) -> Py<Self> {
        let _ = memo;
        slf
    }

    /// music21 freezes a score by pickling it, and what this object is lives
    /// in Rust where a pickle cannot see it.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        let span = slf.borrow().inner.clone();
        crate::pickled(slf, &span)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Some(span) = crate::unpickled::<_, RsMeterTerminal>(slf, state)? {
            slf.borrow_mut().inner = span;
        }
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!("<music21.meter.core.MeterTerminal {}>", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Self>>()
            .is_ok_and(|other| same_value(&self.inner, &other.inner))
    }

    fn __hash__(&self) -> isize {
        *self
            .frozen_hash
            .get_or_init(|| value_hash(&self.inner, false))
    }

    /// music21's `modify`: a terminal like this one with a different
    /// numerator, denominator or weight.
    #[pyo3(signature = (**keywords))]
    fn modify(&self, py: Python<'_>, keywords: Option<&Bound<'_, PyDict>>) -> PyResult<Py<PyAny>> {
        if let Some(extra) = unexpected_keywords(keywords, &["numerator", "denominator", "weight"])?
        {
            return Err(PyValueError::new_err(format!(
                "MeterTerminal.modify() does not accept {extra}; allowed keywords are \
                 numerator, denominator, weight."
            )));
        }
        let given = |name: &str| -> PyResult<Option<Bound<'_, PyAny>>> {
            match keywords {
                Some(keywords) => keywords.get_item(name),
                None => Ok(None),
            }
        };
        let numerator = match given("numerator")? {
            Some(value) => value.extract()?,
            None => self.inner.numerator(),
        };
        let denominator: UnsignedIntegerType = match given("denominator")? {
            Some(value) => value.extract()?,
            None => self.inner.denominator(),
        };
        if denominator == 0 || !denominator.is_power_of_two() {
            return Err(MeterException::new_err(format!(
                "bad denominator value: {denominator}"
            )));
        }
        let weight = match given("weight")? {
            Some(value) => value.extract()?,
            None => self.inner.weight(),
        };
        let span = RsMeterTerminal::new(numerator, denominator)
            .map_err(meter_error)?
            .with_weight(weight);
        terminal_object(py, span)
    }

    #[pyo3(signature = (**keywords))]
    fn __replace__(
        &self,
        py: Python<'_>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        self.modify(py, keywords)
    }

    #[getter]
    fn get_numerator(&self) -> UnsignedIntegerType {
        self.inner.numerator()
    }

    #[setter]
    fn set_numerator(&self, _value: &Bound<'_, PyAny>) -> PyResult<()> {
        Err(immutable("MeterTerminal"))
    }

    #[getter]
    fn get_denominator(&self) -> UnsignedIntegerType {
        self.inner.denominator()
    }

    #[setter]
    fn set_denominator(&self, _value: &Bound<'_, PyAny>) -> PyResult<()> {
        Err(immutable("MeterTerminal"))
    }

    #[getter]
    fn get_weight(&self) -> FloatType {
        self.inner.weight()
    }

    #[setter]
    fn set_weight(&self, _value: &Bound<'_, PyAny>) -> PyResult<()> {
        Err(immutable("MeterTerminal"))
    }

    /// music21's `depth`: how many levels deep this part is, which for a
    /// span nothing divides is always one.
    #[getter]
    fn depth(&self) -> usize {
        1
    }

    /// The length, as music21's shared `FrozenDuration`.
    #[getter]
    fn get_duration(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        duration_of(py, self.inner.numerator(), self.inner.denominator())
    }

    #[setter]
    fn set_duration(&self, _value: &Bound<'_, PyAny>) -> PyResult<()> {
        Err(immutable("MeterTerminal"))
    }

    /// music21's `ratioEqual`: whether two spans are written the same, which
    /// is not whether they last the same time — `3/4` and `6/8` are not.
    fn ratioEqual(&self, other: &Bound<'_, PyAny>) -> bool {
        node_ratio(other)
            .is_some_and(|ratio| ratio == (self.inner.numerator(), self.inner.denominator()))
    }

    /// music21's `subdivideByCount`.
    #[pyo3(signature = (countRequest = None))]
    fn subdivideByCount(
        &self,
        py: Python<'_>,
        countRequest: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        subdivide_by_count(py, &self.inner, countRequest)
    }

    /// music21's `subdivideByList`.
    fn subdivideByList(
        &self,
        py: Python<'_>,
        numeratorList: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let items = numeratorList.walk()?.collect::<PyResult<Vec<_>>>()?;
        sequence_from_span(py, subdivided_by_list(&self.inner, &items)?, false)
    }

    /// music21's `subdivide`, which hands back a new sequence.
    fn subdivide(&self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        sequence_from_span(py, subdivided(&self.inner, value)?, false)
    }
}

/// The ratio of anything with a numerator and a denominator.
fn node_ratio(other: &Bound<'_, PyAny>) -> Option<(UnsignedIntegerType, UnsignedIntegerType)> {
    if other.is_none() {
        return None;
    }
    let numerator = other.getattr("numerator").ok()?.extract().ok()?;
    let denominator = other.getattr("denominator").ok()?.extract().ok()?;
    Some((numerator, denominator))
}

/// The length of a ratio, as music21's shared `FrozenDuration`. The empty
/// sequence has none.
fn duration_of(
    py: Python<'_>,
    numerator: UnsignedIntegerType,
    denominator: UnsignedIntegerType,
) -> PyResult<Py<PyAny>> {
    let quarter_length = if denominator == 0 {
        0.0
    } else {
        4.0 * FloatType::from(numerator) / FloatType::from(denominator)
    };
    frozen_duration(py, quarter_length)
}

/// music21's `subdivideByCount` over any span: the span as a sequence of
/// one part, then partitioned, keeping its weight.
fn subdivide_by_count(
    py: Python<'_>,
    span: &RsMeterTerminal,
    count: Option<&Bound<'_, PyAny>>,
) -> PyResult<Py<PyAny>> {
    let whole = RsMeterTerminal::new(span.numerator(), span.denominator())
        .map_err(meter_error)?
        .with_weight(span.weight());
    let divided = match count.filter(|count| !count.is_none()) {
        Some(count) => partitioned(&whole, count, false)?,
        None => as_sequence(whole)?,
    };
    sequence_from_span(py, divided, false)
}

#[pymethods]
impl MeterSequence {
    /// music21's `MeterSequence(value, partitionRequest)`. A ratio written
    /// whole is a sequence of that one part, `{4/4}`; written in parts it is
    /// those parts; a span or a list of spans are the parts themselves. Given
    /// nothing it holds nothing and reads `{}`.
    #[new]
    #[pyo3(signature = (value = None, partitionRequest = None))]
    fn new(
        py: Python<'_>,
        value: Option<&Bound<'_, PyAny>>,
        partitionRequest: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let _ = py;
        let Some(value) = value.filter(|value| !value.is_none()) else {
            // A blank sequence, which music21's `makeSequence` writes the
            // slots of.
            let mut blank = Self::holding(RsMeterTerminal::new(1, 1).map_err(meter_error)?, false);
            blank.sealed = partitionRequest.is_some_and(|request| !request.is_none());
            return Ok(blank);
        };
        let (whole, objects) = if let Ok(written) = value.extract::<String>() {
            let read = RsMeterTerminal::from_partition_string(&written).map_err(meter_error)?;
            (as_sequence(read)?, None)
        } else if value.extract::<PyRef<'_, MeterSequence>>().is_ok()
            || value.extract::<PyRef<'_, MeterTerminal>>().is_ok()
        {
            (
                joined(vec![span_of(value)?])?,
                Some(vec![value.clone().unbind()]),
            )
        } else {
            let items = value
                .walk()
                .map_err(|_| {
                    MeterException::new_err(format!(
                        "cannot create a MeterSequence with a {}",
                        value
                            .repr()
                            .map(|repr| repr.to_string())
                            .unwrap_or_default()
                    ))
                })?
                .collect::<PyResult<Vec<_>>>()?;
            let mut spans = Vec::with_capacity(items.len());
            let mut objects = Vec::with_capacity(items.len());
            let mut all_objects = true;
            for item in &items {
                spans.push(span_of(item)?);
                if item.extract::<String>().is_ok() {
                    all_objects = false;
                }
                objects.push(item.clone().unbind());
            }
            (joined(spans)?, all_objects.then_some(objects))
        };
        match partitionRequest.filter(|request| !request.is_none()) {
            Some(request) => Ok(Self::holding(partitioned(&whole, request, false)?, false)),
            None => {
                let mut sequence = Self::holding(whole, false);
                sequence.parts = objects;
                Ok(sequence)
            }
        }
    }

    /// music21's `_frozenHash`: `None` until the sequence is hashed.
    #[getter]
    fn _frozenHash(&self) -> Option<isize> {
        self.frozen_hash.get().copied()
    }

    /// A sequence cannot change, so a copy of one is itself.
    fn __copy__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    #[pyo3(signature = (memo = None))]
    fn __deepcopy__(slf: Py<Self>, memo: Option<&Bound<'_, PyAny>>) -> Py<Self> {
        let _ = memo;
        slf
    }

    /// As `MeterTerminal`'s: the span goes into the pickle, and the part
    /// objects are built again when something asks for one.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        let span = slf.borrow().inner.clone();
        crate::pickled(slf, &span)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Some(span) = crate::unpickled::<_, RsMeterTerminal>(slf, state)? {
            let mut me = slf.borrow_mut();
            me.inner = span;
            me.parts = None;
        }
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!(
            "<music21.meter.core.MeterSequence {{{}}}>",
            self.inner.partition_display()
        )
    }

    fn __str__(&self) -> String {
        format!("{{{}}}", self.inner.partition_display())
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other.extract::<PyRef<'_, Self>>().is_ok_and(|other| {
            self.parenthesis == other.parenthesis && same_value(&self.inner, &other.inner)
        })
    }

    fn __hash__(&self) -> isize {
        *self
            .frozen_hash
            .get_or_init(|| value_hash(&self.inner, self.parenthesis))
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __getitem__(slf: &Bound<'_, Self>, key: isize) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let parts = Self::parts_of(slf)?;
        let length = parts.len() as isize;
        let resolved = if key < 0 { key + length } else { key };
        usize::try_from(resolved)
            .ok()
            .and_then(|index| parts.get(index))
            .map(|part| part.clone_ref(py))
            .ok_or_else(|| {
                PyIndexError::new_err(format!("MeterSequence does not have entry {key}."))
            })
    }

    fn __iter__(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let parts = Self::parts_of(slf)?;
        Ok(PyList::new(py, parts)?
            .into_any()
            .try_iter()?
            .into_any()
            .unbind())
    }

    /// music21's: a sequence cannot change, so a part is replaced with
    /// `setIndex`.
    fn __setitem__(&self, key: isize, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let _ = value;
        Err(PyTypeError::new_err(format!(
            "MeterSequence is immutable: use ms = ms.setIndex({key}, value)"
        )))
    }

    /// music21's: a sequence cannot be loaded again.
    #[pyo3(signature = (*arguments, **keywords))]
    fn load(
        &self,
        arguments: &Bound<'_, PyTuple>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let _ = keywords;
        let value = match arguments.get_item(0) {
            Ok(value) => value.repr()?.to_string(),
            Err(_) => "...".to_string(),
        };
        Err(MeterException::new_err(format!(
            "MeterSequence is immutable, so load() cannot change it: use ms = meter.MeterSequence({value})"
        )))
    }

    /// music21's `setIndex`: a sequence like this one with one part
    /// replaced by another written over the same ratio.
    fn setIndex(
        slf: &Bound<'_, Self>,
        index: isize,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        if value.extract::<PyRef<'_, MeterSequence>>().is_err()
            && value.extract::<PyRef<'_, MeterTerminal>>().is_err()
        {
            return Err(MeterException::new_err(format!(
                "values in MeterSequences must be MeterTerminals or MeterSequences, not {}",
                value.str()?
            )));
        }
        let length = slf.borrow().inner.len() as isize;
        let resolved = if index < 0 { index + length } else { index };
        let Some(position) = usize::try_from(resolved)
            .ok()
            .filter(|position| (*position as isize) < length)
        else {
            return Err(PyIndexError::new_err(format!(
                "MeterSequence does not have entry {index}."
            )));
        };
        let span = span_of(value)?;
        let inner = slf
            .borrow()
            .inner
            .set_index(position, span)
            .map_err(meter_error)?;
        let mut parts = Self::parts_of(slf)?;
        parts[position] = value.clone().unbind();
        slf.borrow().rebuilt(py, inner, Some(parts))
    }

    /// music21's `_rebuild`: a sequence like this one with a different
    /// partition, which has to come to the same ratio.
    fn _rebuild(&self, py: Python<'_>, partition: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let items = partition.walk()?.collect::<PyResult<Vec<_>>>()?;
        let spans = items.iter().map(span_of).collect::<PyResult<Vec<_>>>()?;
        let inner =
            RsMeterTerminal::sequence(self.inner.numerator(), self.inner.denominator(), spans)
                .map_err(meter_error)?
                .with_summed_numerator(self.inner.summed_numerator());
        self.rebuilt(
            py,
            inner,
            Some(items.into_iter().map(Bound::unbind).collect()),
        )
    }

    /// music21's `modify`: a sequence like this one with different parts, a
    /// parenthesis or summed numerators.
    #[pyo3(signature = (**keywords))]
    fn modify(slf: &Bound<'_, Self>, keywords: Option<&Bound<'_, PyDict>>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        if let Some(extra) =
            unexpected_keywords(keywords, &["partition", "parenthesis", "summedNumerator"])?
        {
            return Err(PyValueError::new_err(format!(
                "MeterSequence.modify() does not accept {extra}; allowed keywords are \
                 partition, parenthesis, summedNumerator."
            )));
        }
        let given = |name: &str| -> PyResult<Option<Bound<'_, PyAny>>> {
            match keywords {
                Some(keywords) => keywords.get_item(name),
                None => Ok(None),
            }
        };
        let me = slf.borrow();
        let summed = match given("summedNumerator")? {
            Some(value) => value.is_truthy()?,
            None => me.inner.summed_numerator(),
        };
        let parenthesis = match given("parenthesis")? {
            Some(value) => value.is_truthy()?,
            None => me.parenthesis,
        };
        let (inner, parts) = match given("partition")? {
            Some(partition) => {
                let items = partition.walk()?.collect::<PyResult<Vec<_>>>()?;
                let spans = items.iter().map(span_of).collect::<PyResult<Vec<_>>>()?;
                let objects = items.into_iter().map(Bound::unbind).collect();
                (joined(spans)?, Some(objects))
            }
            None => (
                me.inner.clone(),
                me.parts
                    .as_ref()
                    .map(|parts| parts.iter().map(|part| part.clone_ref(py)).collect()),
            ),
        };
        drop(me);
        let inner = inner.with_summed_numerator(summed);
        match parts {
            Some(parts) => make_sequence(py, inner, parts, parenthesis),
            None => sequence_from_span(py, inner, parenthesis),
        }
    }

    #[pyo3(signature = (**keywords))]
    fn __replace__(
        slf: &Bound<'_, Self>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        Self::modify(slf, keywords)
    }

    /// music21's `numerator`: what the sequence adds up to, over its
    /// denominator.
    #[getter]
    fn get_numerator(&self) -> UnsignedIntegerType {
        self.inner.numerator()
    }

    #[setter]
    fn set_numerator(&self, _value: &Bound<'_, PyAny>) -> PyResult<()> {
        Err(immutable("MeterSequence"))
    }

    #[getter]
    fn get_denominator(&self) -> UnsignedIntegerType {
        self.inner.denominator()
    }

    #[setter]
    fn set_denominator(&self, _value: &Bound<'_, PyAny>) -> PyResult<()> {
        Err(immutable("MeterSequence"))
    }

    // music21's `makeSequence` builds a sequence by writing these slots one
    // at a time into a blank one, so they can be written.
    #[getter]
    fn get__numerator(&self) -> UnsignedIntegerType {
        self.inner.numerator()
    }

    #[setter]
    fn set__numerator(&mut self, numerator: UnsignedIntegerType) -> PyResult<()> {
        self.writable()?;
        self.inner = RsMeterTerminal::sequence(
            numerator,
            self.inner.denominator(),
            self.inner.parts().to_vec(),
        )
        .map_err(meter_error)?
        .with_summed_numerator(self.inner.summed_numerator());
        Ok(())
    }

    #[getter]
    fn get__denominator(&self) -> UnsignedIntegerType {
        self.inner.denominator()
    }

    #[setter]
    fn set__denominator(&mut self, denominator: UnsignedIntegerType) -> PyResult<()> {
        self.writable()?;
        // The empty sequence is written over nought, which a span here is
        // never written over; it stays the empty sequence it already is.
        if denominator == 0 {
            return Ok(());
        }
        self.inner = RsMeterTerminal::sequence(
            self.inner.numerator(),
            denominator,
            self.inner.parts().to_vec(),
        )
        .map_err(meter_error)?
        .with_summed_numerator(self.inner.summed_numerator());
        Ok(())
    }

    #[getter]
    fn get__partition<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(slf.py(), Self::parts_of(slf)?)
    }

    #[setter]
    fn set__partition(&mut self, partition: &Bound<'_, PyAny>) -> PyResult<()> {
        self.writable()?;
        let items = partition.walk()?.collect::<PyResult<Vec<_>>>()?;
        let spans = items.iter().map(span_of).collect::<PyResult<Vec<_>>>()?;
        self.inner =
            RsMeterTerminal::sequence(self.inner.numerator(), self.inner.denominator(), spans)
                .map_err(meter_error)?
                .with_summed_numerator(self.inner.summed_numerator());
        self.parts = Some(items.into_iter().map(Bound::unbind).collect());
        Ok(())
    }

    #[getter]
    fn get_parenthesis(&self) -> bool {
        self.parenthesis
    }

    #[setter]
    fn set_parenthesis(
        &mut self,
        #[pyo3(from_py_with = crate::truthy)] value: bool,
    ) -> PyResult<()> {
        self.writable()?;
        self.parenthesis = value;
        Ok(())
    }

    /// music21's `summedNumerator`: whether this was written `3+2/8` rather
    /// than `3/8+2/8`.
    #[getter]
    fn get_summedNumerator(&self) -> bool {
        self.inner.summed_numerator()
    }

    /// The last slot music21's `makeSequence` writes, after which the
    /// sequence is made.
    #[setter]
    fn set_summedNumerator(
        &mut self,
        #[pyo3(from_py_with = crate::truthy)] value: bool,
    ) -> PyResult<()> {
        self.writable()?;
        self.inner = self.inner.with_summed_numerator(value);
        self.sealed = true;
        Ok(())
    }

    /// music21's `weight`: what the parts weigh together.
    #[getter]
    fn get_weight(&self) -> FloatType {
        self.inner.parts().iter().map(RsMeterTerminal::weight).sum()
    }

    #[setter]
    fn set_weight(&self, _value: &Bound<'_, PyAny>) -> PyResult<()> {
        Err(immutable("MeterSequence"))
    }

    /// music21's `withWeight`: a sequence whose weights add up to the one
    /// given, longer parts getting more of it.
    fn withWeight(&self, py: Python<'_>, targetWeight: FloatType) -> PyResult<Py<PyAny>> {
        self.with_inner(py, self.inner.with_weight(targetWeight))
    }

    /// The length, as music21's shared `FrozenDuration`.
    #[getter]
    fn get_duration(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        if self.inner.is_empty() {
            return duration_of(py, 0, 0);
        }
        duration_of(py, self.inner.numerator(), self.inner.denominator())
    }

    #[setter]
    fn set_duration(&self, _value: &Bound<'_, PyAny>) -> PyResult<()> {
        Err(immutable("MeterSequence"))
    }

    #[staticmethod]
    fn _durationFromNumeratorDenominator(
        py: Python<'_>,
        numerator: UnsignedIntegerType,
        denominator: UnsignedIntegerType,
    ) -> PyResult<Py<PyAny>> {
        duration_of(py, numerator, denominator)
    }

    fn ratioEqual(&self, other: &Bound<'_, PyAny>) -> bool {
        node_ratio(other)
            .is_some_and(|ratio| ratio == (self.inner.numerator(), self.inner.denominator()))
    }

    /// music21's `subdivideByCount`, over the whole sequence.
    #[pyo3(signature = (countRequest = None))]
    fn subdivideByCount(
        &self,
        py: Python<'_>,
        countRequest: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        subdivide_by_count(py, &self.inner, countRequest)
    }

    fn subdivideByList(
        &self,
        py: Python<'_>,
        numeratorList: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let items = numeratorList.walk()?.collect::<PyResult<Vec<_>>>()?;
        sequence_from_span(py, subdivided_by_list(&self.inner, &items)?, false)
    }

    fn subdivide(&self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        sequence_from_span(py, subdivided(&self.inner, value)?, false)
    }

    /// music21's `partitionDisplay`: the parts written without the braces.
    #[getter]
    fn partitionDisplay(&self) -> String {
        self.inner.partition_display()
    }

    /// music21's `partitionStr`: what a partition of this many parts is
    /// called, `Duple` for two and `Triple` for three.
    #[getter]
    fn partitionStr(&self) -> String {
        match self.inner.len() {
            0 => "Empty".to_string(),
            count => music21::meter::partition_name(count),
        }
    }

    /// music21's `depth`: how many distinct levels this sequence has, its
    /// own level counted as the first.
    #[getter]
    fn depth(&self) -> usize {
        self.inner.depth().max(1)
    }

    #[pyo3(signature = (*, depth = 0))]
    fn isUniformPartition(&self, depth: usize) -> bool {
        self.inner.is_uniform_partition(depth)
    }

    fn getPartitionOptions<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let options = self
            .inner
            .partition_options()
            .into_iter()
            .map(|option| PyTuple::new(py, option))
            .collect::<PyResult<Vec<_>>>()?;
        PyTuple::new(py, options)
    }

    #[pyo3(signature = (countRequest, loadDefault = true))]
    fn partitionedByCount(
        &self,
        py: Python<'_>,
        countRequest: usize,
        #[pyo3(from_py_with = crate::truthy)] loadDefault: bool,
    ) -> PyResult<Py<PyAny>> {
        let divided = self
            .inner
            .partitioned_by_count(countRequest, loadDefault)
            .map_err(meter_error)?;
        self.with_inner(py, divided)
    }

    fn partitionedByList(
        &self,
        py: Python<'_>,
        numeratorList: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let items = numeratorList.walk()?.collect::<PyResult<Vec<_>>>()?;
        self.with_inner(py, partitioned_by_list(&self.inner, &items)?)
    }

    fn partitionedByOtherMeterSequence(
        &self,
        py: Python<'_>,
        other: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let divided = self
            .inner
            .partitioned_by_other(&as_sequence(span_of(other)?)?)
            .map_err(meter_error)?;
        self.with_inner(py, divided)
    }

    #[pyo3(signature = (value, loadDefault = false))]
    fn partitioned(
        &self,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
        #[pyo3(from_py_with = crate::truthy)] loadDefault: bool,
    ) -> PyResult<Py<PyAny>> {
        self.with_inner(py, partitioned(&self.inner, value, loadDefault)?)
    }

    #[pyo3(signature = (divisions = None))]
    fn subdividedPartitionsEqual(
        &self,
        py: Python<'_>,
        divisions: Option<usize>,
    ) -> PyResult<Py<PyAny>> {
        let divided = self
            .inner
            .subdivided_partitions_equal(divisions)
            .map_err(meter_error)?;
        self.with_inner(py, divided)
    }

    #[pyo3(signature = (depth, firstPartitionForm = None, normalizeDenominators = true))]
    fn subdividedNestedHierarchy(
        &self,
        py: Python<'_>,
        depth: usize,
        firstPartitionForm: Option<&Bound<'_, PyAny>>,
        #[pyo3(from_py_with = crate::truthy)] normalizeDenominators: bool,
    ) -> PyResult<Py<PyAny>> {
        let span = &self.inner;
        let nested = match firstPartitionForm.filter(|form| !form.is_none()) {
            Some(form) => match form.extract::<UnsignedIntegerType>() {
                Ok(count) => {
                    span.subdivided_nested_hierarchy(depth, Some(count), normalizeDenominators)
                }
                Err(_) => {
                    let form = span_of(form)?;
                    let level = form.level(0, true).map_err(meter_error)?;
                    span.subdivided_nested_hierarchy_by(depth, &level, normalizeDenominators)
                }
            },
            None => span.subdivided_nested_hierarchy(depth, None, normalizeDenominators),
        }
        .map_err(meter_error)?;
        self.with_inner(py, nested)
    }

    fn _getFlatList(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        self.inner
            .flattened()
            .into_iter()
            .map(|part| terminal_object(py, part))
            .collect()
    }

    fn flatten(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        sequence_object(py, Self::holding(joined(self.inner.flattened())?, false))
    }

    #[getter]
    fn flatWeight(&self) -> Vec<FloatType> {
        self.inner.flat_weights()
    }

    #[pyo3(signature = (levelCount, flat = true))]
    fn getLevelList(
        &self,
        py: Python<'_>,
        levelCount: usize,
        #[pyo3(from_py_with = crate::truthy)] flat: bool,
    ) -> PyResult<Vec<Py<PyAny>>> {
        self.inner
            .level_list(levelCount, flat)
            .into_iter()
            .map(|part| node_object(py, part))
            .collect()
    }

    #[pyo3(signature = (level = 0, flat = true))]
    fn getLevel(
        &self,
        py: Python<'_>,
        level: usize,
        #[pyo3(from_py_with = crate::truthy)] flat: bool,
    ) -> PyResult<Py<PyAny>> {
        sequence_object(
            py,
            Self::holding(joined(self.inner.level_list(level, flat))?, false),
        )
    }

    /// music21's `getLevelSpan`: where each part of a level starts and ends,
    /// through `opFrac`.
    #[pyo3(signature = (level = 0))]
    fn getLevelSpan<'py>(
        &self,
        py: Python<'py>,
        level: usize,
    ) -> PyResult<Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>)>> {
        self.inner
            .level_span(level)
            .into_iter()
            .map(|(start, end)| {
                Ok((
                    crate::duration::op_frac(py, start)?,
                    crate::duration::op_frac(py, end)?,
                ))
            })
            .collect()
    }

    #[pyo3(signature = (level = 0))]
    fn getLevelWeight(&self, level: usize) -> Vec<FloatType> {
        self.inner.level_weight(level)
    }

    /// music21's `withLevelWeight`: a sequence with one level weighed,
    /// looping the weights given over it.
    #[pyo3(signature = (weightList, level = 0))]
    fn withLevelWeight(
        &self,
        py: Python<'_>,
        weightList: Vec<FloatType>,
        level: usize,
    ) -> PyResult<Py<PyAny>> {
        if weightList.is_empty() {
            return Err(PyZeroDivisionError::new_err("integer modulo by zero"));
        }
        let weighed = self
            .inner
            .with_level_weight(level, &weightList)
            .map_err(meter_error)?;
        self.with_inner(py, weighed)
    }

    #[pyo3(signature = (qLenPos, includeCoincidentBoundaries = false))]
    fn offsetToIndex(
        &self,
        qLenPos: FloatType,
        #[pyo3(from_py_with = crate::truthy)] includeCoincidentBoundaries: bool,
    ) -> PyResult<usize> {
        let index = self.inner.offset_to_index(qLenPos).map_err(meter_error)?;
        if includeCoincidentBoundaries && index > 0 {
            // The part before claims an offset on the boundary it shares.
            let (start, _) = self.inner.level_span(0)[index];
            if (start - qLenPos).abs() < 1e-9 {
                return Ok(index - 1);
            }
        }
        Ok(index)
    }

    #[pyo3(signature = (qLenPos, includeCoincidentBoundaries = false))]
    fn offsetToAddress(
        &self,
        qLenPos: FloatType,
        #[pyo3(from_py_with = crate::truthy)] includeCoincidentBoundaries: bool,
    ) -> PyResult<Vec<usize>> {
        let _ = includeCoincidentBoundaries;
        self.inner.offset_to_address(qLenPos).map_err(meter_error)
    }

    #[pyo3(signature = (qLenPos, permitMeterModulus = false))]
    fn offsetToSpan(
        &self,
        py: Python<'_>,
        qLenPos: FloatType,
        #[pyo3(from_py_with = crate::truthy)] permitMeterModulus: bool,
    ) -> PyResult<(Py<PyAny>, Py<PyAny>)> {
        let (start, end) = self
            .inner
            .offset_to_span(qLenPos, permitMeterModulus)
            .map_err(meter_error)?;
        let start = if start == 0.0 {
            0i64.into_pyobject(py)?.into_any().unbind()
        } else {
            crate::duration::op_frac(py, start)?.unbind()
        };
        Ok((start, crate::duration::op_frac(py, end)?.unbind()))
    }

    fn offsetToWeight(&self, py: Python<'_>, qLenPos: FloatType) -> PyResult<Py<PyAny>> {
        let weight = self.inner.offset_to_weight(qLenPos).map_err(meter_error)?;
        Ok(crate::duration::op_frac(py, weight)?.unbind())
    }

    /// music21's `offsetToDepth`. `index`, where given, names the part of
    /// the finest level the offset falls in, which quantizing would
    /// otherwise look up.
    #[pyo3(signature = (qLenPos, align = "quantize", index = None))]
    fn offsetToDepth(
        &self,
        qLenPos: FloatType,
        align: &str,
        index: Option<usize>,
    ) -> PyResult<usize> {
        let align = match align {
            "quantize" => RsOffsetAlign::Quantize,
            "start" => RsOffsetAlign::Start,
            "end" => RsOffsetAlign::End,
            other => {
                return Err(MeterException::new_err(format!(
                    "cannot align to {other:?}"
                )));
            }
        };
        let span = &self.inner;
        if let Some(index) = index.filter(|_| align == RsOffsetAlign::Quantize) {
            span.offset_to_depth(qLenPos, align).map_err(meter_error)?;
            let finest = span.level_span(span.depth().saturating_sub(1));
            let (start, _) = finest.get(index).copied().ok_or_else(|| {
                MeterException::new_err(format!(
                    "this meter has no part {index} to read a depth at"
                ))
            })?;
            return span
                .offset_to_depth(start, RsOffsetAlign::Start)
                .map_err(meter_error);
        }
        span.offset_to_depth(qLenPos, align).map_err(meter_error)
    }

    #[pyo3(signature = (*arguments, **keywords))]
    fn partition(
        &self,
        arguments: &Bound<'_, PyTuple>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let _ = (arguments, keywords);
        Err(removed_in_place("partition", "partitioned"))
    }

    #[pyo3(signature = (*arguments, **keywords))]
    fn partitionByCount(
        &self,
        arguments: &Bound<'_, PyTuple>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let _ = (arguments, keywords);
        Err(removed_in_place("partitionByCount", "partitionedByCount"))
    }

    #[pyo3(signature = (*arguments, **keywords))]
    fn partitionByList(
        &self,
        arguments: &Bound<'_, PyTuple>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let _ = (arguments, keywords);
        Err(removed_in_place("partitionByList", "partitionedByList"))
    }

    #[pyo3(signature = (*arguments, **keywords))]
    fn partitionByOtherMeterSequence(
        &self,
        arguments: &Bound<'_, PyTuple>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let _ = (arguments, keywords);
        Err(removed_in_place(
            "partitionByOtherMeterSequence",
            "partitionedByOtherMeterSequence",
        ))
    }

    #[pyo3(signature = (*arguments, **keywords))]
    fn subdividePartitionsEqual(
        &self,
        arguments: &Bound<'_, PyTuple>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let _ = (arguments, keywords);
        Err(removed_in_place(
            "subdividePartitionsEqual",
            "subdividedPartitionsEqual",
        ))
    }

    #[pyo3(signature = (*arguments, **keywords))]
    fn subdivideNestedHierarchy(
        &self,
        arguments: &Bound<'_, PyTuple>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let _ = (arguments, keywords);
        Err(removed_in_place(
            "subdivideNestedHierarchy",
            "subdividedNestedHierarchy",
        ))
    }

    #[pyo3(signature = (*arguments, **keywords))]
    fn setLevelWeight(
        &self,
        arguments: &Bound<'_, PyTuple>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let _ = (arguments, keywords);
        Err(removed_in_place("setLevelWeight", "withLevelWeight"))
    }
}

/// Reads a span built by music21's own class rather than this one, through
/// the Python API: the ratio, the weight of each leaf, and the parts under it.
fn read_span(value: &Bound<'_, PyAny>) -> PyResult<RsMeterTerminal> {
    let numerator: UnsignedIntegerType = value.getattr("numerator")?.extract()?;
    let denominator: UnsignedIntegerType = value.getattr("denominator")?.extract()?;
    let parts: Vec<Bound<'_, PyAny>> = match value.walk() {
        Ok(items) => items.collect::<PyResult<Vec<_>>>()?,
        // A terminal is not a sequence and holds nothing.
        Err(_) => Vec::new(),
    };
    if parts.is_empty() {
        let weight: FloatType = value.getattr("weight")?.extract()?;
        return Ok(RsMeterTerminal::new(numerator, denominator)
            .map_err(meter_error)?
            .with_weight(weight));
    }
    let parts = parts.iter().map(span_of).collect::<PyResult<Vec<_>>>()?;
    RsMeterTerminal::sequence(numerator, denominator, parts).map_err(meter_error)
}

/// A crate stream holding the notes, chords and rests of a music21 stream
/// at their offsets, flattened, which is all the meter questions read.
fn sounding_stream(measure: &Bound<'_, PyAny>) -> PyResult<RsStream> {
    let mut stream = RsStream::new();
    let flat = measure.call_method0("flatten")?.getattr("notesAndRests")?;
    for element in flat.walk()? {
        let element = element?;
        let offset: FloatType = element
            .getattr("offset")?
            .call_method0("__float__")?
            .extract()?;
        if element.getattr("isRest")?.extract::<bool>()? {
            let quarter_length: FloatType = element
                .getattr("duration")?
                .getattr("quarterLength")?
                .call_method0("__float__")?
                .extract()?;
            stream.insert(
                offset,
                RsRest::from_quarter_length(quarter_length).map_err(meter_error)?,
            );
        } else {
            let quarter_length: FloatType = element
                .getattr("duration")?
                .getattr("quarterLength")?
                .call_method0("__float__")?
                .extract()?;
            let duration = RsDuration::new(quarter_length).map_err(meter_error)?;
            if element.getattr("isChord")?.extract::<bool>()? {
                let mut chord = chord_from_any(Some(&element))?;
                chord.set_duration(duration);
                stream.insert(offset, chord);
            } else {
                let mut note = note_from_any(&element)?;
                note.set_duration(duration);
                stream.insert(offset, note);
            }
        }
    }
    Ok(stream)
}

/// music21's `bestTimeSignature`: the time signature a measure's notes and
/// rests fill most naturally.
#[pyfunction]
#[pyo3(name = "bestTimeSignature")]
fn bestTimeSignature(meas: &Bound<'_, PyAny>) -> PyResult<Py<TimeSignature>> {
    let inner =
        music21::meter::best_time_signature(&sounding_stream(meas)?).map_err(meter_error)?;
    crate::installed_new(
        meas.py(),
        "music21.meter.base",
        "TimeSignature",
        TimeSignature::wrap(inner),
    )
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(bestTimeSignature, m)?)?;
    m.add_class::<TimeSignature>()?;
    m.add_class::<MeterTerminal>()?;
    m.add_class::<MeterSequence>()?;
    Ok(())
}
