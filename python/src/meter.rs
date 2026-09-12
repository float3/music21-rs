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
use pyo3::types::PyTuple;

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
pub const NAMES: &[&str] = &["TimeSignature", "MeterException"];

/// The names this facade replaces in `music21.meter.core`.
pub const CORE_NAMES: &[&str] = &["MeterTerminal", "MeterSequence"];

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
        let (mut inner, symbol) = read_meter(&value)?;
        checked_divisions(&mut inner, divisions)?;
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
        let quarter_length: FloatType = value
            .getattr("quarterLength")
            .and_then(|length| length.call_method0("__float__"))
            .and_then(|length| length.extract())
            .or_else(|_| value.extract())?;
        let denominator = self.inner.denominator();
        let beat = 4.0 / FloatType::from(denominator);
        let numerator = (quarter_length / beat).round();
        if !(numerator.is_finite() && numerator >= 1.0) {
            return Err(MeterException::new_err(format!(
                "a bar cannot last {quarter_length} quarter lengths"
            )));
        }
        let rebuilt = RsTimeSignature::new(numerator as UnsignedIntegerType, denominator)
            .map_err(meter_error)?;
        self.inner = rebuilt;
        Ok(())
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
    fn set_beatCount(&mut self, count: UnsignedIntegerType) -> PyResult<()> {
        self.inner.set_beat_count(count).map_err(meter_error)
    }

    #[getter]
    fn beatCountName(&self) -> String {
        self.inner.beat_count_name()
    }

    #[getter]
    fn beatDuration(&self) -> PyResult<Duration> {
        Ok(Duration::wrap(
            self.inner.beat_duration().map_err(meter_error)?,
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
    /// A span belonging to nobody: what `subdivide` hands back, which
    /// music21 builds without touching the meter it came from.
    Loose(RsMeterTerminal),
}

impl Held {
    /// The span this stands for, as a value.
    fn read(&self, py: Python<'_>) -> PyResult<RsMeterTerminal> {
        match self {
            Self::Loose(terminal) => Ok(terminal.clone()),
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
    fn descend(&self, py: Python<'_>, index: usize) -> PyResult<Self> {
        match self {
            Self::Loose(terminal) => terminal
                .parts()
                .get(index)
                .map(|part| Self::Loose(part.clone()))
                .ok_or_else(|| PyIndexError::new_err("list index out of range")),
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
}

/// Hands back whichever of the two classes a span is: music21 calls a span
/// with parts a sequence and one without a terminal, and its docstrings
/// compare both reprs exactly.
fn wrap_span(py: Python<'_>, held: Held) -> PyResult<Py<PyAny>> {
    let span = held.read(py)?;
    if span.is_empty() {
        Ok(Py::new(py, MeterTerminal { held })?.into_any())
    } else {
        Ok(Py::new(py, MeterSequence { held })?.into_any())
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

    /// music21's `subdivide`, which hands back a new span rather than
    /// dividing this one.
    fn subdivide(&self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let span = self.held.read(py)?;
        let divided = subdivided(&span, value)?;
        wrap_span(py, Held::Loose(divided))
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
    /// A copy of a sequence stands alone: music21 deep-copies an accent
    /// sequence while working out the weights of a bar.
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

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let span = self.held.read(py)?;
        Ok(format!("<music21.meter.core.MeterSequence {span}>"))
    }

    fn __str__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(self.held.read(py)?.to_string())
    }

    fn __len__(&self, py: Python<'_>) -> PyResult<usize> {
        Ok(self.held.read(py)?.len())
    }

    fn __getitem__(&self, py: Python<'_>, index: isize) -> PyResult<Py<PyAny>> {
        let length = self.held.read(py)?.len() as isize;
        let resolved = if index < 0 { index + length } else { index };
        if resolved < 0 || resolved >= length {
            return Err(PyIndexError::new_err("list index out of range"));
        }
        wrap_span(py, self.held.descend(py, resolved as usize)?)
    }

    fn __setitem__(
        &mut self,
        py: Python<'_>,
        index: isize,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let mut span = self.held.read(py)?;
        let length = span.len() as isize;
        let resolved = if index < 0 { index + length } else { index };
        if resolved < 0 || resolved >= length {
            return Err(PyIndexError::new_err("list index out of range"));
        }
        let replacement = span_of(py, value)?;
        span.parts_mut()[resolved as usize] = replacement;
        self.held.write(py, span)
    }

    fn __iter__(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let held = &slf.borrow().held;
        let length = held.read(py)?.len();
        let mut parts = Vec::with_capacity(length);
        for index in 0..length {
            parts.push(wrap_span(py, held.descend(py, index)?)?);
        }
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
    fn partition(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let span = self.held.read(py)?;
        let divided = subdivided(&span, value)?;
        self.held.write(py, divided)
    }

    /// music21's `subdivide`, which hands back a new span.
    fn subdivide(&self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let span = self.held.read(py)?;
        let divided = subdivided(&span, value)?;
        wrap_span(py, Held::Loose(divided))
    }

    /// music21's `subdivideByCount`.
    #[pyo3(signature = (countRequest = None))]
    fn subdivideByCount(&self, py: Python<'_>, countRequest: Option<usize>) -> PyResult<Py<PyAny>> {
        let span = self.held.read(py)?;
        let count = countRequest.unwrap_or(1);
        let divided = span.subdivide_by_count(count).map_err(meter_error)?;
        wrap_span(py, Held::Loose(divided))
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

    /// music21's `flat`: every terminal of this sequence, however deep.
    #[getter]
    fn flat(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let span = self.held.read(py)?;
        let mut flattened =
            RsMeterTerminal::new(span.numerator(), span.denominator()).map_err(meter_error)?;
        *flattened.parts_mut() = span.flattened();
        wrap_span(py, Held::Loose(flattened))
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
    })
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(bestTimeSignature, m)?)?;
    m.add_class::<TimeSignature>()?;
    m.add_class::<MeterTerminal>()?;
    m.add_class::<MeterSequence>()?;
    Ok(())
}
