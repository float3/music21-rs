//! music21's `meter.base`, over the crate's `TimeSignature`.
//!
//! Only `TimeSignature` is replaced. music21 derives everything about a meter
//! from a `MeterSequence` partition tree — one for beats, one for beaming, one
//! for display, one for accent weights — and the crate derives the numbers it
//! answers from the numerator and the denominator alone. What the tree is for
//! beyond those numbers has no counterpart here, so the members that read one
//! are not ported and raise: beaming, accents, display partitions, beat depth
//! and the stream-walking lookups.
//!
//! `SenzaMisuraTimeSignature` and `bestTimeSignature` stay music21's. The
//! first is a meter with no numbers in it at all, which this crate cannot
//! express; the second builds a `TimeSignature` out of a stream, and leaving
//! it alone means music21's own function is run over *our* meters.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::types::PyTuple;

use music21_rs_crate::meter::TimeSignature as RsTimeSignature;
use music21_rs_crate::{
    Duration as RsDuration, FloatType, Rest as RsRest, Stream as RsStream, UnsignedIntegerType,
};

use crate::chord::chord_from_any;
use crate::note::note_from_any;

use crate::duration::Duration;

/// The names the `meter` facade replaces in `music21.meter.base`.
pub const NAMES: &[&str] = &["TimeSignature", "MeterException"];

pyo3::create_exception!(music21_rs_facade, MeterException, crate::Music21Exception);

error_into!(meter_error, MeterException);

/// music21 writes `common` and `cut` for the two meters that have names, and
/// takes a bare ratio otherwise. The name is kept beside the meter, since
/// music21 reports it as `symbol` and writes it to a score.
fn read_meter(value: &str) -> PyResult<(RsTimeSignature, String)> {
    // music21 reads the two names whatever case they are written in.
    let value = value.trim();
    match value.to_ascii_lowercase().as_str() {
        "common" | "c" => Ok((RsTimeSignature::common(), "common".to_string())),
        "cut" => Ok((RsTimeSignature::cut(), "cut".to_string())),
        _ => Ok((
            RsTimeSignature::from_ratio_string(value).map_err(meter_error)?,
            String::new(),
        )),
    }
}

/// music21's second argument partitions the bar into that many parts.
///
/// A partition is exactly what this crate does not model — but asking for as
/// many parts as the meter already has beats asks for the partition it
/// already has, and that is not a partition at all. Anything else is refused
/// rather than answered as though the bar had been left alone.
fn checked_divisions(meter: RsTimeSignature, divisions: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
    let Some(divisions) = divisions.filter(|divisions| !divisions.is_none()) else {
        return Ok(());
    };
    let asked: UnsignedIntegerType = divisions.extract()?;
    if asked == meter.beat_count() {
        return Ok(());
    }
    Err(MeterException::new_err(format!(
        "{} is partitioned into {} beats here, and a partition into {asked} is not modelled",
        meter.ratio_string(),
        meter.beat_count()
    )))
}

/// music21's `TimeSignature`: a numerator over a denominator, and everything
/// that follows from the pair.
#[pyclass(
    name = "TimeSignature",
    module = "music21.meter",
    subclass,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct TimeSignature {
    pub(crate) inner: RsTimeSignature,
    /// music21's `symbol`: the name the meter was written with, where it has
    /// one. Empty for a meter written as a ratio.
    symbol: String,
}

#[pymethods]
impl TimeSignature {
    #[new]
    #[pyo3(signature = (value = "4/4".to_string(), divisions = None, **keywords))]
    fn new(
        value: String,
        divisions: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let _ = keywords;
        let (inner, symbol) = read_meter(&value)?;
        checked_divisions(inner, divisions)?;
        Ok(Self { inner, symbol })
    }

    /// music21 builds in `__init__`, and a Python subclass of this one hands
    /// `__new__` its own arguments before calling up with music21's.
    #[pyo3(signature = (value = "4/4".to_string(), divisions = None, **keywords))]
    fn __init__(
        mut slf: PyRefMut<'_, Self>,
        value: String,
        divisions: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let built = Self::new(value, divisions, keywords)?;
        slf.inner = built.inner;
        slf.symbol = built.symbol;
        Ok(())
    }

    #[getter]
    fn get_symbol(&self) -> &str {
        &self.symbol
    }

    #[setter]
    fn set_symbol(&mut self, symbol: String) {
        self.symbol = symbol;
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

    #[setter]
    fn set_numerator(&mut self, numerator: UnsignedIntegerType) -> PyResult<()> {
        self.inner =
            RsTimeSignature::new(numerator, self.inner.denominator()).map_err(meter_error)?;
        Ok(())
    }

    #[getter]
    fn get_denominator(&self) -> UnsignedIntegerType {
        self.inner.denominator()
    }

    #[setter]
    fn set_denominator(&mut self, denominator: UnsignedIntegerType) -> PyResult<()> {
        self.inner =
            RsTimeSignature::new(self.inner.numerator(), denominator).map_err(meter_error)?;
        Ok(())
    }

    #[getter]
    fn get_ratioString(&self) -> String {
        self.inner.ratio_string()
    }

    #[setter]
    fn set_ratioString(&mut self, value: &str) -> PyResult<()> {
        (self.inner, self.symbol) = read_meter(value)?;
        Ok(())
    }

    /// music21's `load`, which is the ratio-string setter under another name.
    #[pyo3(signature = (value, divisions = None))]
    fn load(&mut self, value: &str, divisions: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let (inner, symbol) = read_meter(value)?;
        checked_divisions(inner, divisions)?;
        self.inner = inner;
        self.symbol = symbol;
        Ok(())
    }

    /// music21's `resetValues`, which reloads the meter from a ratio.
    #[pyo3(signature = (value = "4/4".to_string(), divisions = None))]
    fn resetValues(&mut self, value: String, divisions: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.load(&value, divisions)
    }

    /// music21's `getMeasureOffsetOrMeterModulusOffset`: where an element
    /// falls in the bar this meter measures.
    ///
    /// Within the measure the element sits in, where there is one; past the
    /// end of the bar, the same offset read modulo the bar, which is how a
    /// meter in a stream of no measures still says where the beat is. Both
    /// offsets come from music21 -- the element's place in its stream is the
    /// stream's to know, not this crate's.
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
        let bar = slf.borrow().inner.bar_quarter_length();
        // The sum says whether the element is still inside the bar; the
        // difference is what is left of it once the meter's own place in the
        // stream is taken off. music21 adds in one and subtracts in the
        // other, and the answers differ where a meter does not start the bar.
        if element_offset + own_offset < bar {
            return crate::duration::op_frac(py, element_offset);
        }
        crate::duration::op_frac(py, (element_offset - own_offset).rem_euclid(bar))
    }

    #[getter]
    fn barDuration(&self) -> Duration {
        Duration::wrap(self.inner.bar_duration())
    }

    #[getter]
    fn get_beatCount(&self) -> UnsignedIntegerType {
        self.inner.beat_count()
    }

    #[setter]
    fn set_beatCount(&mut self, _count: UnsignedIntegerType) -> PyResult<()> {
        Err(MeterException::new_err(
            "the beat count follows from the meter here; it is not a partition to be set",
        ))
    }

    #[getter]
    fn beatCountName(&self) -> String {
        self.inner.beat_count_name()
    }

    #[getter]
    fn beatDuration(&self) -> Duration {
        Duration::wrap(self.inner.beat_duration())
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
    fn beatDivisionDurations(&self) -> Vec<Duration> {
        self.inner
            .beat_division_durations()
            .into_iter()
            .map(Duration::wrap)
            .collect()
    }

    #[getter]
    fn beatSubDivisionDurations(&self) -> Vec<Duration> {
        self.inner
            .beat_sub_division_durations()
            .into_iter()
            .map(Duration::wrap)
            .collect()
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

    /// music21's `getBeatDuration`, which takes an offset because a
    /// hand-partitioned meter can have beats of different lengths. Every
    /// meter this type can express has one uniform beat, so the offset only
    /// has to be inside the bar.
    fn getBeatDuration(&self, qLenPos: FloatType) -> PyResult<Duration> {
        self.checked_offset(qLenPos)?;
        Ok(Duration::wrap(self.inner.beat_duration()))
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
                offset.into_pyobject(py)?.into_any(),
            ],
        )
    }

    fn getBeatProportion(&self, qLenPos: FloatType) -> PyResult<FloatType> {
        self.inner.beat_proportion(qLenPos).map_err(meter_error)
    }

    fn getBeatProportionStr(&self, qLenPos: FloatType) -> PyResult<String> {
        self.inner
            .beat_proportion_string(qLenPos)
            .map_err(meter_error)
    }

    fn getOffsetFromBeat(&self, beat: FloatType) -> PyResult<FloatType> {
        self.inner.offset_from_beat(beat).map_err(meter_error)
    }

    /// music21's `averageBeatStrength`: the mean accent weight of the
    /// offsets of everything in a stream, or of its notes alone, each read
    /// against the bar.
    #[pyo3(signature = (streamIn, notesOnly = true))]
    fn averageBeatStrength(
        &self,
        streamIn: &Bound<'_, PyAny>,
        notesOnly: bool,
    ) -> PyResult<FloatType> {
        let elements = if notesOnly {
            streamIn.getattr("notes")?
        } else {
            streamIn.clone()
        };
        let bar = self.inner.bar_quarter_length();
        let mut total = 0.0;
        let mut count = 0_usize;
        for element in elements.try_iter()? {
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

    /// music21's `getAccentWeight` over the default accent hierarchy, which
    /// has one level, so `level` picks nothing.
    #[pyo3(signature = (qLenPos, level = 0, forcePositionMatch = false, permitMeterModulus = false))]
    fn getAccentWeight(
        &self,
        qLenPos: FloatType,
        level: u32,
        forcePositionMatch: bool,
        permitMeterModulus: bool,
    ) -> PyResult<FloatType> {
        let _ = level;
        self.inner
            .accent_weight_with(qLenPos, forcePositionMatch, permitMeterModulus)
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
            .is_ok_and(|other| self.inner.ratio_equal(other.inner))
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.borrow().clone();
        crate::copy_as_same_type(slf, copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.borrow().clone();
        crate::copy_as_same_type(slf, copied)
    }

    /// music21 freezes a score by pickling it, and what this object is lives
    /// in Rust where a pickle cannot see it.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let Some(inner) = crate::unpickled::<_, RsTimeSignature>(slf, state)? else {
            return Ok(());
        };
        slf.borrow_mut().inner = inner;
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

/// A crate stream holding the notes, chords and rests of a music21 stream
/// at their offsets, flattened, which is all the meter questions read.
fn sounding_stream(measure: &Bound<'_, PyAny>) -> PyResult<RsStream> {
    let mut stream = RsStream::new();
    let flat = measure.call_method0("flatten")?.getattr("notesAndRests")?;
    for element in flat.try_iter()? {
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
fn bestTimeSignature(meas: &Bound<'_, PyAny>) -> PyResult<TimeSignature> {
    let inner = music21_rs_crate::meter::best_time_signature(&sounding_stream(meas)?)
        .map_err(meter_error)?;
    Ok(TimeSignature {
        inner,
        symbol: String::new(),
    })
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(bestTimeSignature, m)?)?;
    m.add_class::<TimeSignature>()?;
    Ok(())
}
