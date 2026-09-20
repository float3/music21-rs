//! music21's `meter.base`, over the crate's `TimeSignature`.
//!
//! Only `TimeSignature` is replaced. music21 derives everything about a meter
//! from a `MeterSequence` partition tree — one for beats, one for beaming, one
//! for display, one for accent weights — and the crate carries all four, so
//! the beat, accent and depth questions are answered from them here.
//!
//! What is not ported yet is the tree itself: a caller cannot reach
//! `beatSequence` and its three siblings through this class, nor partition one
//! by hand. `getBeams` — which beams a run of notes, and so needs the notes —
//! goes with them.
//!
//! `SenzaMisuraTimeSignature` and `bestTimeSignature` stay music21's. The
//! first is a meter with no numbers in it at all, which this crate cannot
//! express; the second builds a `TimeSignature` out of a stream, and leaving
//! it alone means music21's own function is run over *our* meters.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::PyIndexError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::types::PyList;
use pyo3::types::PyTuple;

use music21_rs_crate::duration::DurationType as RsDurationType;
use music21_rs_crate::meter::BeamedNote as RsBeamedNote;
use music21_rs_crate::meter::MeterTerminal as RsMeterTerminal;
use music21_rs_crate::meter::OffsetAlign as RsOffsetAlign;
use music21_rs_crate::meter::TimeSignature as RsTimeSignature;
use music21_rs_crate::{
    Duration as RsDuration, FloatType, Rest as RsRest, Stream as RsStream, UnsignedIntegerType,
};

use crate::chord::chord_from_any;
use crate::note::note_from_any;

use crate::duration::Duration;

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
    /// music21's `_overriddenBarDuration`: the bar length a caller wrote
    /// over this meter's own, kept as the object it was given, since that is
    /// what music21 hands back.
    overridden_bar_duration: Option<Py<PyAny>>,
    /// music21's `symbol`: the name the meter was written with, where it has
    /// one. Empty for a meter written as a ratio.
    symbol: String,
}

impl Clone for TimeSignature {
    /// A copy keeps the bar length written over this meter, since that is
    /// part of what the meter says rather than a name for one object.
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            inner: self.inner.clone(),
            symbol: self.symbol.clone(),
            overridden_bar_duration: self
                .overridden_bar_duration
                .as_ref()
                .map(|written| written.clone_ref(py)),
        })
    }
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
        let (mut inner, symbol) = read_meter(&value)?;
        checked_divisions(&mut inner, divisions)?;
        Ok(Self {
            inner,
            symbol,
            overridden_bar_duration: None,
        })
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
        let (mut inner, symbol) = read_meter(value)?;
        checked_divisions(&mut inner, divisions)?;
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

    /// music21's `displaySequence`: how the bar is written.
    #[getter]
    fn displaySequence(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        Self::sequence_view(slf, Which::Display)
    }

    /// music21's `beatSequence`: how the bar is counted.
    #[getter]
    fn beatSequence(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        Self::sequence_view(slf, Which::Beat)
    }

    /// music21's `beamSequence`: how the bar is beamed.
    #[getter]
    fn beamSequence(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        Self::sequence_view(slf, Which::Beam)
    }

    /// music21's `accentSequence`: how the bar is weighted.
    #[getter]
    fn accentSequence(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        Self::sequence_view(slf, Which::Accent)
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

    /// music21 lets a caller write this, and keeps what it was given.
    #[setter]
    fn set_barDuration(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.overridden_bar_duration = Some(value.clone().unbind());
        Ok(())
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.overridden_bar_duration)?;
        Ok(())
    }

    fn __clear__(&mut self) {
        self.overridden_bar_duration = None;
    }

    #[getter]
    fn barDuration(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        if let Some(written) = &self.overridden_bar_duration {
            return Ok(written.clone_ref(py));
        }
        Ok(Py::new(py, Duration::wrap(self.inner.bar_duration()))?.into_any())
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
        let beats = self.inner.beat_sequence_mut();
        beats.partition_by_list(&numerators).map_err(|_| {
            TimeSignatureException::new_err(format!(
                "cannot partition beat with provided value: {numerators:?}"
            ))
        })?;
        let _ = beats.subdivide_partitions_equal(None);
        Ok(())
    }

    #[getter]
    fn beatCountName(&self) -> String {
        self.inner.beat_count_name()
    }

    #[getter]
    fn beatDuration(&self) -> PyResult<Duration> {
        Ok(Duration::wrap(
            self.inner.beat_duration().map_err(time_signature_error)?,
        ))
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
    fn beatDivisionDurations(&self) -> PyResult<Vec<Duration>> {
        Ok(self
            .inner
            .beat_division_durations()
            .map_err(meter_error)?
            .into_iter()
            .map(Duration::wrap)
            .collect())
    }

    #[getter]
    fn beatSubDivisionDurations(&self) -> PyResult<Vec<Duration>> {
        Ok(self
            .inner
            .beat_sub_division_durations()
            .map_err(meter_error)?
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
    fn getBeatDuration(&self, qLenPos: FloatType) -> PyResult<Duration> {
        self.checked_offset(qLenPos)?;
        Ok(Duration::wrap(
            self.inner.beat_duration_at(qLenPos).map_err(meter_error)?,
        ))
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
        for element in srcList.try_iter()? {
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

    /// music21's `getAccentWeight`: the weight of the accent partition an
    /// offset falls in, at a level of the accent sequence.
    #[pyo3(signature = (qLenPos, level = 0, forcePositionMatch = false, permitMeterModulus = false))]
    fn getAccentWeight(
        &self,
        qLenPos: FloatType,
        level: u32,
        forcePositionMatch: bool,
        permitMeterModulus: bool,
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
    /// One of the four sequences, to read.
    pub(crate) fn sequence(&self, which: Which) -> &RsMeterTerminal {
        match which {
            Which::Display => self.inner.display_sequence(),
            Which::Beat => self.inner.beat_sequence(),
            Which::Beam => self.inner.beam_sequence(),
            Which::Accent => self.inner.accent_sequence(),
        }
    }

    /// One of the four sequences, to divide.
    pub(crate) fn sequence_mut(&mut self, which: Which) -> &mut RsMeterTerminal {
        match which {
            Which::Display => self.inner.display_sequence_mut(),
            Which::Beat => self.inner.beat_sequence_mut(),
            Which::Beam => self.inner.beam_sequence_mut(),
            Which::Accent => self.inner.accent_sequence_mut(),
        }
    }

    /// A view of one of them, standing for the meter it came from.
    fn sequence_view(slf: &Bound<'_, Self>, which: Which) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        wrap_span(
            py,
            Held::View {
                owner: slf.clone().unbind(),
                which,
                path: Vec::new(),
            },
        )
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

/// Which of the four sequences a view looks at.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Which {
    /// How the bar is written.
    Display,
    /// How it is counted.
    Beat,
    /// How it is beamed.
    Beam,
    /// How it is weighted.
    Accent,
}

/// What a wrapper stands for: a span inside a meter, or one of its own.
enum Held {
    /// A span of a meter, reached by walking `path` down one of its
    /// sequences. Reads and writes go to the meter itself, so dividing one
    /// divides the bar.
    View {
        owner: Py<TimeSignature>,
        which: Which,
        path: Vec<usize>,
    },
    /// A part of a sequence that belongs to nobody else, reached by walking
    /// `path` down it. Reads and writes go to that sequence, so dividing a
    /// part of it divides it -- music21 hands back the very parts it holds,
    /// and its own docstrings divide one and read the whole back.
    Part {
        owner: Py<MeterSequence>,
        path: Vec<usize>,
    },
    /// A span belonging to nobody: what `subdivide` hands back, which
    /// music21 builds without touching the meter it came from.
    Loose(RsMeterTerminal),
}

/// The part a path names inside a span.
fn part_at(span: &RsMeterTerminal, path: &[usize]) -> PyResult<RsMeterTerminal> {
    let mut found = span;
    for step in path {
        found = found.parts().get(*step).ok_or_else(|| {
            MeterException::new_err("this part of the sequence is no longer there")
        })?;
    }
    Ok(found.clone())
}

/// Writes a span back into the part a path names.
fn write_part_at(
    span: &mut RsMeterTerminal,
    path: &[usize],
    value: RsMeterTerminal,
) -> PyResult<()> {
    let mut found = span;
    for step in path {
        found = found.parts_mut().get_mut(*step).ok_or_else(|| {
            MeterException::new_err("this part of the sequence is no longer there")
        })?;
    }
    *found = value;
    Ok(())
}

impl Held {
    /// The span this stands for, as a value.
    fn read(&self, py: Python<'_>) -> PyResult<RsMeterTerminal> {
        match self {
            Self::Loose(terminal) => Ok(terminal.clone()),
            Self::Part { owner, path } => {
                let held = owner.borrow(py).held.read(py)?;
                part_at(&held, path)
            }
            Self::View { owner, which, path } => {
                let owner = owner.borrow(py);
                let mut span = owner.sequence(*which);
                for step in path {
                    span = span.parts().get(*step).ok_or_else(|| {
                        MeterException::new_err("this part of the meter is no longer there")
                    })?;
                }
                Ok(span.clone())
            }
        }
    }

    /// Writes a span back where this one came from.
    fn write(&mut self, py: Python<'_>, value: RsMeterTerminal) -> PyResult<()> {
        match self {
            Self::Loose(terminal) => {
                *terminal = value;
                Ok(())
            }
            Self::Part { owner, path } => {
                // Read the sequence, write the part into it, and put it back
                // the way it came -- the sequence may itself be a part, or a
                // view of a meter.
                let mut held = owner.borrow(py).held.clone_for(py);
                let mut span = held.read(py)?;
                write_part_at(&mut span, path, value)?;
                held.write(py, span)?;
                owner.borrow_mut(py).held = held;
                Ok(())
            }
            Self::View { owner, which, path } => {
                let mut owner = owner.borrow_mut(py);
                let mut span = owner.sequence_mut(*which);
                for step in path {
                    span = span.parts_mut().get_mut(*step).ok_or_else(|| {
                        MeterException::new_err("this part of the meter is no longer there")
                    })?;
                }
                *span = value;
                Ok(())
            }
        }
    }

    /// The same view, one step further down.
    /// A copy of this handle, which a part needs so it can write its owner
    /// back afterwards.
    fn clone_for(&self, py: Python<'_>) -> Self {
        match self {
            Self::Loose(terminal) => Self::Loose(terminal.clone()),
            Self::Part { owner, path } => Self::Part {
                owner: owner.clone_ref(py),
                path: path.clone(),
            },
            Self::View { owner, which, path } => Self::View {
                owner: owner.clone_ref(py),
                which: *which,
                path: path.clone(),
            },
        }
    }

    fn descend(&self, py: Python<'_>, index: usize) -> PyResult<Self> {
        match self {
            Self::Loose(terminal) => terminal
                .parts()
                .get(index)
                .map(|part| Self::Loose(part.clone()))
                .ok_or_else(|| PyIndexError::new_err("list index out of range")),
            Self::Part { owner, path } => {
                let mut deeper = path.clone();
                deeper.push(index);
                Ok(Self::Part {
                    owner: owner.clone_ref(py),
                    path: deeper,
                })
            }
            Self::View { owner, which, path } => {
                let mut deeper = path.clone();
                deeper.push(index);
                Ok(Self::View {
                    owner: owner.clone_ref(py),
                    which: *which,
                    path: deeper,
                })
            }
        }
    }
}

/// music21's `MeterTerminal`: a span of a bar that nothing divides.
#[pyclass(
    name = "MeterTerminal",
    module = "music21.meter.core",
    subclass,
    skip_from_py_object
)]
pub struct MeterTerminal {
    held: Held,
}

/// music21's `MeterSequence`: a span of a bar made of other spans.
#[pyclass(
    name = "MeterSequence",
    module = "music21.meter.core",
    subclass,
    skip_from_py_object
)]
pub struct MeterSequence {
    held: Held,
    /// The parts as the objects music21 hands back, built the first time one
    /// is asked for and dropped whenever the partition changes. music21 holds
    /// its parts, so `s[0] is s[0]`, editing a part edits the sequence, and a
    /// part handed to the constructor goes on being the part it was given.
    parts: Vec<Py<PyAny>>,
    /// music21's `_levelListCache`, which its own docstrings look into. The
    /// crate caches nothing; this is the facade caching what music21 caches,
    /// as the chord facade does.
    level_list_cache: Option<Py<PyDict>>,
}

impl MeterSequence {
    /// A sequence standing for a span, holding no parts of its own yet.
    fn holding(held: Held) -> Self {
        Self {
            held,
            parts: Vec::new(),
            level_list_cache: None,
        }
    }

    /// Forgets the part objects and the cache, which anything that changes
    /// the partition has to do: the parts it had are no longer its parts, as
    /// music21 builds new ones and empties the same cache.
    fn partition_changed(&mut self) {
        self.parts.clear();
        self.level_list_cache = None;
    }

    /// The part objects, built as views of this sequence the first time and
    /// the same objects after that.
    fn parts_of(slf: &Bound<'_, Self>) -> PyResult<Vec<Py<PyAny>>> {
        let py = slf.py();
        let held = slf.borrow().held.clone_for(py);
        let length = held.read(py)?.len();
        if slf.borrow().parts.len() != length {
            let mut parts = Vec::with_capacity(length);
            for index in 0..length {
                let part = match &held {
                    // A part of a sequence nobody else holds is a view of it,
                    // so dividing the part divides the sequence.
                    Held::Loose(_) => Held::Part {
                        owner: slf.clone().unbind(),
                        path: vec![index],
                    },
                    held => held.descend(py, index)?,
                };
                parts.push(wrap_span(py, part)?);
            }
            slf.borrow_mut().parts = parts;
        }
        Ok(slf
            .borrow()
            .parts
            .iter()
            .map(|part| part.clone_ref(py))
            .collect())
    }

    /// Makes the objects handed in this sequence's own parts, each a view of
    /// it, so that editing one edits the sequence: music21 keeps the very
    /// spans it was given.
    fn adopt(slf: &Bound<'_, Self>, given: &Bound<'_, PyAny>) -> PyResult<()> {
        let mut parts = Vec::new();
        for (index, item) in given.try_iter()?.enumerate() {
            let item = item?;
            let anchored = Held::Part {
                owner: slf.clone().unbind(),
                path: vec![index],
            };
            if let Ok(mut sequence) = item.extract::<PyRefMut<'_, Self>>() {
                sequence.held = anchored;
            } else if let Ok(mut terminal) = item.extract::<PyRefMut<'_, MeterTerminal>>() {
                terminal.held = anchored;
            } else {
                // Something that was only written as a ratio has no object to
                // adopt, so the sequence keeps the span it read.
                return Ok(());
            }
            parts.push(item.unbind());
        }
        slf.borrow_mut().parts = parts;
        Ok(())
    }
}

/// A weight as music21 prints it: a whole one as an `int`, anything else as a
/// `float`. music21 keeps the very number a caller handed it, so a weight set
/// to one reads back `1` and not `1.0`. Only the weights of a level go
/// through this -- music21's own arithmetic answers floats everywhere else.
fn as_number(py: Python<'_>, value: FloatType) -> PyResult<Py<PyAny>> {
    if value.fract() == 0.0 && value.abs() < 9.007_199_254_740_992e15 {
        return Ok((value as i64).into_pyobject(py)?.into_any().unbind());
    }
    Ok(value.into_pyobject(py)?.into_any().unbind())
}

/// A span as music21's `MeterSequence`, whatever it holds: what a caller
/// built, and what every method handing back a divided span answers with.
fn sequence_of(py: Python<'_>, span: RsMeterTerminal) -> PyResult<Py<PyAny>> {
    Ok(Py::new(py, MeterSequence::holding(Held::Loose(span)))?.into_any())
}

/// Hands back whichever of the two classes a span is: music21 calls a span
/// with parts a sequence and one without a terminal, and its docstrings
/// compare both reprs exactly.
fn wrap_span(py: Python<'_>, held: Held) -> PyResult<Py<PyAny>> {
    let span = held.read(py)?;
    if span.is_empty() {
        Ok(Py::new(py, MeterTerminal { held })?.into_any())
    } else {
        Ok(Py::new(py, MeterSequence::holding(held))?.into_any())
    }
}

#[pymethods]
impl MeterTerminal {
    /// A copy of a span stands alone, since what it copied may have been a
    /// view of a meter.
    fn __copy__(&self, py: Python<'_>) -> PyResult<Self> {
        Ok(Self {
            held: Held::Loose(self.held.read(py)?),
        })
    }

    #[pyo3(signature = (memo = None))]
    fn __deepcopy__(&self, py: Python<'_>, memo: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let _ = memo;
        self.__copy__(py)
    }

    #[new]
    #[pyo3(signature = (slashNotation = None, weight = 1.0))]
    fn new(slashNotation: Option<&str>, weight: FloatType) -> PyResult<Self> {
        let mut inner = match slashNotation {
            Some(written) => RsMeterTerminal::from_ratio_string(written).map_err(meter_error)?,
            None => RsMeterTerminal::new(1, 4).map_err(meter_error)?,
        };
        inner.set_weight(weight);
        Ok(Self {
            held: Held::Loose(inner),
        })
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let span = self.held.read(py)?;
        Ok(format!("<music21.meter.core.MeterTerminal {span}>"))
    }

    fn __str__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(self.held.read(py)?.to_string())
    }

    #[getter]
    fn get_numerator(&self, py: Python<'_>) -> PyResult<UnsignedIntegerType> {
        Ok(self.held.read(py)?.numerator())
    }

    /// music21's `numerator` setter: the span is rewritten over the same
    /// denominator, so a `1/4` told three is `3/4`.
    #[setter]
    fn set_numerator(&mut self, py: Python<'_>, numerator: UnsignedIntegerType) -> PyResult<()> {
        let span = self.held.read(py)?;
        let rewritten = RsMeterTerminal::new(numerator, span.denominator()).map_err(meter_error)?;
        self.held.write(py, rewritten)
    }

    #[getter]
    fn get_denominator(&self, py: Python<'_>) -> PyResult<UnsignedIntegerType> {
        Ok(self.held.read(py)?.denominator())
    }

    /// music21's `denominator` setter, over the same numerator. music21 only
    /// writes a span over the denominators it has note values for, so a
    /// seventh is refused here even though `MeterTerminal('4/3')` is read.
    #[setter]
    fn set_denominator(
        &mut self,
        py: Python<'_>,
        denominator: UnsignedIntegerType,
    ) -> PyResult<()> {
        if !matches!(denominator, 1 | 2 | 4 | 8 | 16 | 32 | 64 | 128) {
            return Err(MeterException::new_err(format!(
                "bad denominator value: {denominator}"
            )));
        }
        let span = self.held.read(py)?;
        let rewritten = RsMeterTerminal::new(span.numerator(), denominator).map_err(meter_error)?;
        self.held.write(py, rewritten)
    }

    #[getter]
    fn get_weight(&self, py: Python<'_>) -> PyResult<FloatType> {
        Ok(self.held.read(py)?.weight())
    }

    #[setter]
    fn set_weight(&mut self, py: Python<'_>, weight: FloatType) -> PyResult<()> {
        let mut span = self.held.read(py)?;
        span.set_weight(weight);
        self.held.write(py, span)
    }

    #[getter]
    fn duration(&self, py: Python<'_>) -> PyResult<Duration> {
        let span = self.held.read(py)?;
        Ok(Duration::wrap(
            RsDuration::new(span.quarter_length()).map_err(meter_error)?,
        ))
    }

    /// music21's `ratioEqual`: whether two spans are written the same, which
    /// is not whether they last the same time — `3/4` and `6/8` are not.
    fn ratioEqual(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let span = self.held.read(py)?;
        let Ok(other) = span_of(py, other) else {
            return Ok(false);
        };
        Ok(span.numerator() == other.numerator() && span.denominator() == other.denominator())
    }

    /// music21's `subdivideByList`, the parts given as their numerators.
    fn subdivideByList(
        &self,
        py: Python<'_>,
        numeratorList: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let span = self.held.read(py)?;
        let divided = if let Ok(numerators) = numeratorList.extract::<Vec<UnsignedIntegerType>>() {
            span.subdivide_by_list(&numerators).map_err(meter_error)?
        } else {
            // The parts written out, as `['2/4', '1/4']`.
            let written: Vec<String> = numeratorList.extract()?;
            let borrowed: Vec<&str> = written.iter().map(String::as_str).collect();
            let mut divided = span.clone();
            divided.partition_by_parts(&borrowed).map_err(meter_error)?;
            divided
        };
        sequence_of(py, divided)
    }

    /// music21's `subdivideByOther`: this span divided into the one span
    /// another sequence is, so the other becomes its only part.
    fn subdivideByOther(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let span = self.held.read(py)?;
        let other = span_of(py, other)?;
        if (other.quarter_length() - span.quarter_length()).abs() > 1e-9 {
            return Err(MeterException::new_err(format!(
                "cannot insert {other} into space of {span}"
            )));
        }
        let mut divided =
            RsMeterTerminal::new(span.numerator(), span.denominator()).map_err(meter_error)?;
        divided.set_weight(span.weight());
        divided.parts_mut().push(other);
        sequence_of(py, divided)
    }

    /// music21's `subdivide`, which hands back a new span rather than
    /// dividing this one.
    fn subdivide(&self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let span = self.held.read(py)?;
        let divided = divided_by(py, &span, value)?;
        sequence_of(py, divided)
    }

    /// music21's `subdivideByCount`.
    #[pyo3(signature = (countRequest = None))]
    fn subdivideByCount(&self, py: Python<'_>, countRequest: Option<usize>) -> PyResult<Py<PyAny>> {
        let span = self.held.read(py)?;
        let count = countRequest.unwrap_or(1);
        let divided = span.subdivide_by_count(count).map_err(meter_error)?;
        wrap_span(py, Held::Loose(divided))
    }
}

#[pymethods]
impl MeterSequence {
    /// music21's `MeterSequence(value, partitionRequest)`. Unpartitioned, a
    /// sequence holds the whole span as its one part, which is what makes
    /// `MeterSequence('4/4')` read `{4/4}` and answer a length of one; given
    /// nothing at all it holds nothing and reads `{}`.
    #[new]
    #[pyo3(signature = (value = None, partitionRequest = None))]
    fn new(
        py: Python<'_>,
        value: Option<&Bound<'_, PyAny>>,
        partitionRequest: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let Some(value) = value.filter(|value| !value.is_none()) else {
            // music21's empty sequence reports a denominator of nought and no
            // length; a span here is always written over a real denominator,
            // so an empty one is a bare 1/1 holding nothing.
            return Ok(Self::holding(Held::Loose(
                RsMeterTerminal::new(1, 1).map_err(meter_error)?,
            )));
        };
        let whole = match value.extract::<String>() {
            Ok(written) => RsMeterTerminal::from_partition_string(&written).map_err(meter_error)?,
            Err(_) => match value.try_iter() {
                // A list of spans, which become the parts: music21 builds an
                // accent sequence out of the two spans a downbeat and an
                // upbeat are.
                Ok(items) => {
                    let mut parts = Vec::new();
                    for item in items {
                        parts.push(span_of(py, &item?)?);
                    }
                    RsMeterTerminal::from_parts_given(parts).map_err(meter_error)?
                }
                Err(_) => span_of(py, value)?,
            },
        };
        // A string that was already a partition is one: `MeterSequence('2/4+2/4')`
        // holds the two halves it names, and nothing wraps it again.
        if !whole.is_empty() && partitionRequest.is_none_or(Bound::is_none) {
            return Ok(Self::holding(Held::Loose(whole)));
        }
        let divided = match partitionRequest {
            Some(request) if !request.is_none() => subdivided(&whole, request)?,
            _ => {
                let mut sequence = RsMeterTerminal::new(whole.numerator(), whole.denominator())
                    .map_err(meter_error)?;
                sequence.parts_mut().push(whole);
                sequence
            }
        };
        Ok(Self::holding(Held::Loose(divided)))
    }

    /// The spans a caller handed the constructor become this sequence's own
    /// parts here rather than in `__new__`, since a sequence cannot hold
    /// itself until it exists.
    #[pyo3(signature = (value = None, partitionRequest = None))]
    fn __init__(
        slf: &Bound<'_, Self>,
        value: Option<&Bound<'_, PyAny>>,
        partitionRequest: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let Some(value) = value.filter(|value| !value.is_none()) else {
            return Ok(());
        };
        if partitionRequest.is_some_and(|request| !request.is_none()) {
            return Ok(());
        }
        // Only a list of spans is adopted; a ratio names no object.
        if value.extract::<String>().is_ok() || value.try_iter().is_err() {
            return Ok(());
        }
        Self::adopt(slf, value)
    }

    /// A copy of a sequence stands alone: music21 deep-copies an accent
    /// sequence while working out the weights of a bar.
    fn __copy__(&self, py: Python<'_>) -> PyResult<Self> {
        Ok(Self::holding(Held::Loose(self.held.read(py)?)))
    }

    #[pyo3(signature = (memo = None))]
    fn __deepcopy__(&self, py: Python<'_>, memo: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let _ = memo;
        self.__copy__(py)
    }

    /// A sequence always writes its parts inside braces, one part or none,
    /// where a terminal writes the bare ratio.
    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!(
            "<music21.meter.core.MeterSequence {{{}}}>",
            self.held.read(py)?.partition_display()
        ))
    }

    fn __str__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!("{{{}}}", self.held.read(py)?.partition_display()))
    }

    fn __len__(&self, py: Python<'_>) -> PyResult<usize> {
        Ok(self.held.read(py)?.len())
    }

    fn __getitem__(slf: &Bound<'_, Self>, index: isize) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let parts = Self::parts_of(slf)?;
        let length = parts.len() as isize;
        let resolved = if index < 0 { index + length } else { index };
        let part = usize::try_from(resolved)
            .ok()
            .and_then(|index| parts.get(index))
            .ok_or_else(|| PyIndexError::new_err("list index out of range"))?;
        Ok(part.clone_ref(py))
    }

    fn __setitem__(slf: &Bound<'_, Self>, index: isize, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        // Read both sides first: music21's own docstring assigns one part of a
        // sequence into another, and that part is a view of this very object.
        let replacement = span_of(py, value)?;
        let mut held = slf.borrow().held.clone_for(py);
        let mut span = held.read(py)?;
        let length = span.len() as isize;
        let resolved = if index < 0 { index + length } else { index };
        if resolved < 0 || resolved >= length {
            return Err(PyIndexError::new_err("list index out of range"));
        }
        let room = span.parts()[resolved as usize].quarter_length();
        if (replacement.quarter_length() - room).abs() > 1e-9 {
            return Err(MeterException::new_err(format!(
                "cannot insert {replacement} into space of {}",
                span.parts()[resolved as usize]
            )));
        }
        span.parts_mut()[resolved as usize] = replacement;
        held.write(py, span)?;
        {
            let mut me = slf.borrow_mut();
            me.held = held;
            me.partition_changed();
        }
        // The object assigned is this sequence's part from here on, as
        // music21's is.
        let parts = Self::parts_of(slf)?;
        let _ = parts;
        if let Ok(mut sequence) = value.extract::<PyRefMut<'_, Self>>() {
            sequence.held = Held::Part {
                owner: slf.clone().unbind(),
                path: vec![resolved as usize],
            };
        } else if let Ok(mut terminal) = value.extract::<PyRefMut<'_, MeterTerminal>>() {
            terminal.held = Held::Part {
                owner: slf.clone().unbind(),
                path: vec![resolved as usize],
            };
        }
        slf.borrow_mut().parts[resolved as usize] = value.clone().unbind();
        Ok(())
    }

    fn __iter__(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let parts = Self::parts_of(slf)?;
        Ok(pyo3::types::PyList::new(py, parts)?
            .into_any()
            .try_iter()?
            .into_any()
            .unbind())
    }

    #[getter]
    fn numerator(&self, py: Python<'_>) -> PyResult<UnsignedIntegerType> {
        Ok(self.held.read(py)?.numerator())
    }

    #[getter]
    fn denominator(&self, py: Python<'_>) -> PyResult<UnsignedIntegerType> {
        Ok(self.held.read(py)?.denominator())
    }

    #[getter]
    fn get_weight(&self, py: Python<'_>) -> PyResult<FloatType> {
        Ok(self.held.read(py)?.weight())
    }

    #[setter]
    fn set_weight(&mut self, py: Python<'_>, weight: FloatType) -> PyResult<()> {
        let mut span = self.held.read(py)?;
        span.set_weight(weight);
        self.held.write(py, span)
    }

    #[getter]
    fn duration(&self, py: Python<'_>) -> PyResult<Duration> {
        let span = self.held.read(py)?;
        Ok(Duration::wrap(
            RsDuration::new(span.quarter_length()).map_err(meter_error)?,
        ))
    }

    /// music21's `partitionDisplay`: the parts written without the braces.
    #[getter]
    fn partitionDisplay(&self, py: Python<'_>) -> PyResult<String> {
        Ok(self.held.read(py)?.partition_display())
    }

    /// music21's `isUniformPartition`, which its own code asks of a meter
    /// before it works out the accent weights.
    #[pyo3(signature = (*, depth = 0))]
    fn isUniformPartition(&self, py: Python<'_>, depth: usize) -> PyResult<bool> {
        Ok(self.held.read(py)?.is_uniform_partition(depth))
    }

    /// music21's `partition`, which divides this span in place — so dividing
    /// the beats of a meter divides that meter.
    #[pyo3(signature = (value, loadDefault = false))]
    fn partition(
        &mut self,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
        loadDefault: bool,
    ) -> PyResult<()> {
        let mut span = self.held.read(py)?;
        if let Ok(count) = value.extract::<usize>() {
            span.partition_by_count(count, loadDefault)
                .map_err(meter_error)?;
        } else {
            span = divided_by(py, &span, value)?;
        }
        self.partition_changed();
        self.held.write(py, span)
    }

    /// music21's `subdivide`, which hands back a new span.
    fn subdivide(&self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let span = self.held.read(py)?;
        let divided = divided_by(py, &span, value)?;
        sequence_of(py, divided)
    }

    /// music21's `subdivideByCount`.
    #[pyo3(signature = (countRequest = None))]
    fn subdivideByCount(&self, py: Python<'_>, countRequest: Option<usize>) -> PyResult<Py<PyAny>> {
        let span = self.held.read(py)?;
        let count = countRequest.unwrap_or(1);
        let divided = span.subdivide_by_count(count).map_err(meter_error)?;
        wrap_span(py, Held::Loose(divided))
    }

    /// music21's `subdivideByOther`: this span divided into the one span
    /// another sequence is, so the other becomes its only part.
    fn subdivideByOther(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let span = self.held.read(py)?;
        let other = span_of(py, other)?;
        if (other.quarter_length() - span.quarter_length()).abs() > 1e-9 {
            return Err(MeterException::new_err(format!(
                "cannot insert {other} into space of {span}"
            )));
        }
        let mut divided =
            RsMeterTerminal::new(span.numerator(), span.denominator()).map_err(meter_error)?;
        divided.set_weight(span.weight());
        divided.parts_mut().push(other);
        sequence_of(py, divided)
    }

    /// music21's `offsetToIndex`: which part an offset falls in.
    fn offsetToIndex(&self, py: Python<'_>, qLenPos: FloatType) -> PyResult<usize> {
        self.held
            .read(py)?
            .offset_to_index(qLenPos)
            .map_err(meter_error)
    }

    /// music21's `offsetToDepth`: how many levels start at an offset.
    #[pyo3(signature = (qLenPos, align = "quantize"))]
    fn offsetToDepth(&self, py: Python<'_>, qLenPos: FloatType, align: &str) -> PyResult<usize> {
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
        self.held
            .read(py)?
            .offset_to_depth(qLenPos, align)
            .map_err(meter_error)
    }

    /// music21's `getLevel`: one level of this sequence, as a sequence.
    #[pyo3(signature = (level = 0, flat = true))]
    fn getLevel(&self, py: Python<'_>, level: usize, flat: bool) -> PyResult<Py<PyAny>> {
        let span = self
            .held
            .read(py)?
            .level(level, flat)
            .map_err(meter_error)?;
        wrap_span(py, Held::Loose(span))
    }

    /// music21's `flatten`: every terminal of this sequence, however deep,
    /// as one sequence of them.
    fn flatten(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let span = self.held.read(py)?;
        let mut flattened =
            RsMeterTerminal::new(span.numerator(), span.denominator()).map_err(meter_error)?;
        *flattened.parts_mut() = span.flattened();
        sequence_of(py, flattened)
    }

    /// music21's `load`, which is its constructor over again on a sequence
    /// that already exists.
    #[pyo3(signature = (value, partitionRequest = None, autoWeight = false, targetWeight = None))]
    fn load(
        &mut self,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
        partitionRequest: Option<&Bound<'_, PyAny>>,
        autoWeight: bool,
        targetWeight: Option<FloatType>,
    ) -> PyResult<()> {
        let mut loaded = Self::new(py, Some(value), partitionRequest)?
            .held
            .read(py)?;
        // music21 weighs a loaded sequence by the target it is given, or
        // leaves the weight it had.
        if autoWeight {
            loaded.set_weight(targetWeight.unwrap_or(1.0));
        } else if let Some(weight) = targetWeight {
            loaded.set_weight(weight);
        }
        self.partition_changed();
        self.held.write(py, loaded)
    }

    /// music21's `partitionStr`: what a partition of this many parts is
    /// called, `Duple` for two and `Triple` for three.
    #[getter]
    fn partitionStr(&self, py: Python<'_>) -> PyResult<String> {
        Ok(music21_rs_crate::meter::partition_name(self.held.read(py)?.len()).to_string())
    }

    /// music21's `partitionByCount`, which divides this span in place.
    #[pyo3(signature = (countRequest, loadDefault = true))]
    fn partitionByCount(
        &mut self,
        py: Python<'_>,
        countRequest: usize,
        loadDefault: bool,
    ) -> PyResult<()> {
        let mut span = self.held.read(py)?;
        span.partition_by_count(countRequest, loadDefault)
            .map_err(meter_error)?;
        self.partition_changed();
        self.held.write(py, span)
    }

    /// music21's `partitionByList`, the parts given as their numerators or
    /// written out as ratios.
    fn partitionByList(
        &mut self,
        py: Python<'_>,
        numeratorList: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let mut span = self.held.read(py)?;
        if let Ok(numerators) = numeratorList.extract::<Vec<UnsignedIntegerType>>() {
            span.partition_by_list(&numerators).map_err(meter_error)?;
        } else {
            let written: Vec<String> = numeratorList.extract()?;
            let borrowed: Vec<&str> = written.iter().map(String::as_str).collect();
            span.partition_by_parts(&borrowed).map_err(meter_error)?;
        }
        self.partition_changed();
        self.held.write(py, span)
    }

    /// music21's `partitionByOtherMeterSequence`: this span divided the way
    /// another one is.
    fn partitionByOtherMeterSequence(
        &mut self,
        py: Python<'_>,
        other: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let mut span = self.held.read(py)?;
        let other = span_of(py, other)?;
        let numerators: Vec<UnsignedIntegerType> = other
            .parts()
            .iter()
            .map(RsMeterTerminal::numerator)
            .collect();
        span.partition_by_list(&numerators).map_err(meter_error)?;
        self.partition_changed();
        self.held.write(py, span)
    }

    /// music21's `subdividePartitionsEqual`: every part divided again, into
    /// as many pieces as it takes to make them all equal.
    #[pyo3(signature = (divisions = None))]
    fn subdividePartitionsEqual(
        &mut self,
        py: Python<'_>,
        divisions: Option<usize>,
    ) -> PyResult<()> {
        let mut span = self.held.read(py)?;
        span.subdivide_partitions_equal(divisions)
            .map_err(meter_error)?;
        self.partition_changed();
        self.held.write(py, span)
    }

    /// music21's `_levelListCache`: the level lists this sequence has been
    /// asked for, keyed by the level and whether it was asked flat. The crate
    /// caches nothing; a sequence standing in for music21's own class caches
    /// what music21 caches, as the chord facade does, and its own docstrings
    /// look in here.
    #[getter]
    fn _levelListCache<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let cache = match &self.level_list_cache {
            Some(cache) => cache.clone_ref(py),
            None => {
                let cache = PyDict::new(py).unbind();
                self.level_list_cache = Some(cache.clone_ref(py));
                cache
            }
        };
        Ok(cache.into_bound(py))
    }

    /// music21's `getLevelList`: one level of this sequence as its terminals.
    ///
    /// The list is kept, so asking twice answers the same objects, which is
    /// what music21 does and what its own docstrings check.
    #[pyo3(signature = (levelCount, flat = true))]
    fn getLevelList(slf: &Bound<'_, Self>, levelCount: usize, flat: bool) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let key = (levelCount, flat).into_pyobject(py)?;
        let cache = slf.borrow_mut()._levelListCache(py)?;
        if let Some(found) = cache.get_item(&key)? {
            // music21 hands back a new list of the same parts, so a caller
            // appending to one answer does not change the next.
            return Ok(
                PyList::new(py, found.try_iter()?.collect::<PyResult<Vec<_>>>()?)?
                    .into_any()
                    .unbind(),
            );
        }
        // A sequence's own parts are its parts, so level nought hands back
        // the objects it holds rather than copies of them.
        let level: Vec<Py<PyAny>> = if levelCount == 0 && !flat {
            Self::parts_of(slf)?
        } else {
            let held = slf.borrow().held.clone_for(py);
            held.read(py)?
                .level_list(levelCount, flat)
                .into_iter()
                .map(|part| wrap_span(py, Held::Loose(part)))
                .collect::<PyResult<Vec<_>>>()?
        };
        let level = PyList::new(py, level)?;
        cache.set_item(key, &level)?;
        Ok(PyList::new(py, level.iter())?.into_any().unbind())
    }

    /// music21's `getLevelSpan`: where each part of a level starts and ends.
    #[pyo3(signature = (level = 0))]
    fn getLevelSpan(&self, py: Python<'_>, level: usize) -> PyResult<Vec<(FloatType, FloatType)>> {
        Ok(self.held.read(py)?.level_span(level))
    }

    /// music21's `getLevelWeight`: what each part of a level weighs.
    #[pyo3(signature = (level = 0))]
    fn getLevelWeight(&self, py: Python<'_>, level: usize) -> PyResult<Vec<Py<PyAny>>> {
        self.held
            .read(py)?
            .level_list(level, true)
            .iter()
            .map(|part| as_number(py, part.weight()))
            .collect()
    }

    /// music21's `setLevelWeight`, which weighs a level part by part.
    #[pyo3(signature = (weightList, level = 0))]
    fn setLevelWeight(
        &mut self,
        py: Python<'_>,
        weightList: Vec<FloatType>,
        level: usize,
    ) -> PyResult<()> {
        let mut span = self.held.read(py)?;
        span.set_weights_at_level(level, &weightList)
            .map_err(meter_error)?;
        // The weights of a level are what a level list carries, so what was
        // worked out before is no longer what this sequence says.
        self.level_list_cache = None;
        self.held.write(py, span)
    }

    /// music21's `offsetToSpan`: where the part sounding at an offset starts
    /// and ends.
    #[pyo3(signature = (qLenPos, permitMeterModulus = false))]
    fn offsetToSpan(
        &self,
        py: Python<'_>,
        qLenPos: FloatType,
        permitMeterModulus: bool,
    ) -> PyResult<(Py<PyAny>, FloatType)> {
        let (start, end) = self
            .held
            .read(py)?
            .offset_to_span(qLenPos, permitMeterModulus)
            .map_err(meter_error)?;
        let start = if start == 0.0 {
            0i64.into_pyobject(py)?.into_any().unbind()
        } else {
            start.into_pyobject(py)?.into_any().unbind()
        };
        Ok((start, end))
    }

    /// music21's `subdivideNestedHierarchy`: this span nested down to a
    /// depth, whatever partitions it had.
    #[pyo3(signature = (depth, firstPartitionForm = None, normalizeDenominators = true))]
    fn subdivideNestedHierarchy(
        &mut self,
        py: Python<'_>,
        depth: usize,
        firstPartitionForm: Option<&Bound<'_, PyAny>>,
        normalizeDenominators: bool,
    ) -> PyResult<()> {
        // music21 takes either a number or a sequence to divide by first, and
        // reads the sequence's own first level as that number of parts.
        let first = match firstPartitionForm.filter(|form| !form.is_none()) {
            Some(form) => match form.extract::<UnsignedIntegerType>() {
                Ok(count) => Some(count),
                Err(_) => Some(span_of(py, form)?.len() as UnsignedIntegerType),
            },
            None => None,
        };
        let mut span = self.held.read(py)?;
        span.subdivide_nested_hierarchy(depth, first, normalizeDenominators)
            .map_err(meter_error)?;
        self.partition_changed();
        self.held.write(py, span)
    }

    /// music21's `_subdivideNested`: one level of the nesting above, over the
    /// spans handed in rather than over this one's own parts. Each is divided
    /// where it sits, and what comes back is their parts.
    ///
    /// The receiver is taken as an object rather than borrowed, since
    /// music21's own docstring hands a sequence to itself.
    #[pyo3(signature = (processObjList, divisions = None))]
    fn _subdivideNested(
        slf: Py<Self>,
        py: Python<'_>,
        processObjList: &Bound<'_, PyAny>,
        divisions: Option<usize>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        // music21 uses `self` only to empty a cache the crate does not keep.
        let _ = slf;
        let mut deeper = Vec::new();
        for item in processObjList.try_iter()? {
            let item = item?;
            let mut sequence = item.extract::<PyRefMut<'_, MeterSequence>>().map_err(|_| {
                MeterException::new_err("a level is divided over sequences, which these are not")
            })?;
            let mut span = sequence.held.read(py)?;
            span.subdivide_partitions_equal(divisions)
                .map_err(meter_error)?;
            sequence.held.write(py, span)?;
            sequence.partition_changed();
            drop(sequence);
            // The parts handed back are the sequence's own, so a caller can
            // divide one and read the whole back -- which is what the nesting
            // above does.
            let bound = item.cast::<MeterSequence>()?;
            deeper.extend(Self::parts_of(bound)?);
        }
        Ok(deeper)
    }

    /// music21's `getPartitionOptions`: the ways it conventionally divides a
    /// span of this length, in the order it prefers them.
    fn getPartitionOptions<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        // music21 hands back a tuple of tuples, and its docstrings print it.
        let options = self
            .held
            .read(py)?
            .division_options()
            .into_iter()
            .map(|option| PyTuple::new(py, option))
            .collect::<PyResult<Vec<_>>>()?;
        PyTuple::new(py, options)
    }

    /// music21's `_getFlatList`: every terminal of this sequence, however
    /// deep, as a list rather than as a sequence of them.
    fn _getFlatList(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        self.held
            .read(py)?
            .flattened()
            .into_iter()
            .map(|part| wrap_span(py, Held::Loose(part)))
            .collect()
    }

    /// music21's `offsetToAddress`: every index it takes to reach the
    /// terminal sounding at an offset, so the length of it is how deep that
    /// terminal lies.
    #[pyo3(signature = (qLenPos, includeCoincidentBoundaries = false))]
    fn offsetToAddress(
        &self,
        py: Python<'_>,
        qLenPos: FloatType,
        includeCoincidentBoundaries: bool,
    ) -> PyResult<Vec<usize>> {
        // music21 takes the flag and passes it to `offsetToIndex`, which does
        // nothing with it either.
        let _ = includeCoincidentBoundaries;
        self.held
            .read(py)?
            .address_of_offset(qLenPos)
            .map_err(meter_error)
    }

    /// music21's `offsetToWeight`: what the part sounding at an offset weighs.
    fn offsetToWeight(&self, py: Python<'_>, qLenPos: FloatType) -> PyResult<Py<PyAny>> {
        let weight = self
            .held
            .read(py)?
            .offset_to_weight(qLenPos)
            .map_err(meter_error)?;
        Ok(crate::duration::op_frac(py, weight)?.unbind())
    }
}

/// Reads the span a caller handed in, whichever of the two classes it is.
fn span_of(py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<RsMeterTerminal> {
    if let Ok(sequence) = value.extract::<PyRef<'_, MeterSequence>>() {
        return sequence.held.read(py);
    }
    if let Ok(terminal) = value.extract::<PyRef<'_, MeterTerminal>>() {
        return terminal.held.read(py);
    }
    let written: String = value.extract().map_err(|_| {
        MeterException::new_err("a meter span is written as a ratio, or is one of ours")
    })?;
    RsMeterTerminal::from_ratio_string(&written).map_err(meter_error)
}

/// music21's `subdivide` dispatch: a count, a list of numerators, a list of
/// ratios written out, or another sequence to be divided the way it is.
fn divided_by(
    py: Python<'_>,
    span: &RsMeterTerminal,
    value: &Bound<'_, PyAny>,
) -> PyResult<RsMeterTerminal> {
    if let Ok(other) = span_of(py, value)
        && !other.is_empty()
    {
        let numerators: Vec<UnsignedIntegerType> = other
            .parts()
            .iter()
            .map(RsMeterTerminal::numerator)
            .collect();
        return span.subdivide_by_list(&numerators).map_err(meter_error);
    }
    subdivided(span, value)
}

/// music21's `subdivide` dispatch: a count, a list of numerators, or a list
/// of ratios written out.
fn subdivided(span: &RsMeterTerminal, value: &Bound<'_, PyAny>) -> PyResult<RsMeterTerminal> {
    if let Ok(count) = value.extract::<usize>() {
        return span.subdivide_by_count(count).map_err(meter_error);
    }
    if let Ok(numerators) = value.extract::<Vec<UnsignedIntegerType>>() {
        return span.subdivide_by_list(&numerators).map_err(meter_error);
    }
    if let Ok(written) = value.extract::<Vec<String>>() {
        let mut divided = span.clone();
        let borrowed: Vec<&str> = written.iter().map(String::as_str).collect();
        divided.partition_by_parts(&borrowed).map_err(meter_error)?;
        return Ok(divided);
    }
    Err(MeterException::new_err(
        "a meter is divided by a count, by a list of numerators, or by the parts written out",
    ))
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
        overridden_bar_duration: None,
    })
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(bestTimeSignature, m)?)?;
    m.add_class::<TimeSignature>()?;
    m.add_class::<MeterTerminal>()?;
    m.add_class::<MeterSequence>()?;
    Ok(())
}
