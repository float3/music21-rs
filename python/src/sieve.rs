//! music21's `sieve` module, over the crate's Xenakis sieves.
//!
//! Only `Sieve` itself is replaced. music21's `Residual`, `CompressionSegment`,
//! `PrimeSegment` and `PitchSieve` stay music21's, and the module's number
//! helpers with them — the crate carries none of the compression machinery,
//! and a facade for a class this crate does not model would take away more
//! than it gave. What that leaves is a harder test than a fuller facade would
//! be: music21's own `PitchSieve` realizes its pitches through *our* sieve.
//!
//! The one thing a caller loses is the compressed state. music21 keeps two
//! readings of a sieve, the expression as written (`exp`) and a compressed
//! form (`cmp`); the crate has only the first, so asking for `cmp` raises
//! rather than answering the wrong list.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use music21_rs_crate::{IntegerType, Sieve as RsSieve};

/// The names the `sieve` facade replaces in `music21.sieve`.
///
/// One class. See the module comment for why the rest of `music21.sieve` is
/// left where it is.
pub const NAMES: &[&str] = &["Sieve", "SieveException"];

pyo3::create_exception!(music21_rs_facade, SieveException, crate::Music21Exception);

error_into!(sieve_error, SieveException);

/// The integers a segment is read over: music21's `z`, which defaults to
/// `range(100)`.
///
/// music21 takes any list here, and every caller inside music21 hands it a
/// contiguous range — `PitchSieve` builds one from the pitch space it spans,
/// `collect` walks upward a hundred at a time. The crate reads a segment
/// between two bounds, so a gappy `z` is refused rather than answered as
/// though the gaps were not there.
fn bounds_of(z: Option<&Bound<'_, PyAny>>) -> PyResult<(IntegerType, IntegerType)> {
    let Some(z) = z else {
        return Ok((0, 99));
    };
    let mut values: Vec<IntegerType> = Vec::new();
    for item in z.try_iter()? {
        values.push(item?.extract()?);
    }
    let (Some(low), Some(high)) = (values.iter().min().copied(), values.iter().max().copied())
    else {
        return Err(SieveException::new_err("z is empty"));
    };
    let contiguous = usize::try_from(high - low + 1).is_ok_and(|span| span == values.len());
    if !contiguous {
        return Err(SieveException::new_err(
            "this sieve reads a segment between two bounds, so z has to be a contiguous range",
        ));
    }
    Ok((low, high))
}

/// music21 spells the formats both short and long, and reports an unknown
/// one rather than quietly answering integers.
#[derive(Clone, Copy)]
enum SegmentFormat {
    Integer,
    Binary,
    Unit,
    Width,
}

impl SegmentFormat {
    fn parse(name: Option<&str>) -> PyResult<Self> {
        match name {
            None | Some("int" | "integer") => Ok(Self::Integer),
            Some("bin" | "binary") => Ok(Self::Binary),
            Some("unit") => Ok(Self::Unit),
            Some("wid" | "width") => Ok(Self::Width),
            Some(other) => Err(SieveException::new_err(format!(
                "{other} not a valid sieve segmentFormat string."
            ))),
        }
    }
}

/// music21's `Sieve`: a logical expression over residual classes.
#[pyclass(
    name = "Sieve",
    module = "music21.sieve",
    subclass,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct Sieve {
    pub(crate) inner: RsSieve,
    /// music21 keeps the range and the format on the object, and a caller
    /// may set either; both are read whenever a segment is asked for without
    /// one of its own.
    z: (IntegerType, IntegerType),
    segment_format: Option<String>,
}

/// The shift a segment is read at: music21's `n`.
///
/// It is not always an integer. `PitchSieve` hands over the pitch space of
/// its origin, which is a float, and music21 then compares a float remainder
/// against an integer one — so a whole number matches and a fractional one
/// matches nothing at all. That is reproduced rather than raised: it is what
/// a caller asking for a quarter-tone origin already gets upstream.
fn shift_of(n: &Bound<'_, PyAny>) -> PyResult<Option<IntegerType>> {
    if let Ok(whole) = n.extract::<IntegerType>() {
        return Ok(Some(whole));
    }
    let value = n.extract::<f64>()?;
    if value.fract() == 0.0
        && value >= f64::from(IntegerType::MIN)
        && value <= f64::from(IntegerType::MAX)
    {
        return Ok(Some(value as IntegerType));
    }
    Ok(None)
}

impl Sieve {
    fn segment_in(
        &self,
        n: Option<IntegerType>,
        low: IntegerType,
        high: IntegerType,
        format: SegmentFormat,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let Some(n) = n else {
            // A fractional shift is congruent to nothing, so the sieve holds
            // no member of the range at all.
            return Ok(match format {
                SegmentFormat::Binary => vec![0; (high - low + 1).max(0) as usize]
                    .into_pyobject(py)?
                    .unbind(),
                SegmentFormat::Unit => Vec::<f64>::new().into_pyobject(py)?.unbind(),
                SegmentFormat::Integer | SegmentFormat::Width => {
                    Vec::<IntegerType>::new().into_pyobject(py)?.unbind()
                }
            });
        };
        let shifted = self.inner.shifted(n);
        Ok(match format {
            SegmentFormat::Integer => shifted.segment(low, high).into_pyobject(py)?.unbind(),
            SegmentFormat::Binary => shifted
                .segment_binary(low, high)
                .into_pyobject(py)?
                .unbind(),
            SegmentFormat::Width => shifted
                .segment_widths(low, high)
                .into_pyobject(py)?
                .unbind(),
            SegmentFormat::Unit => shifted.segment_unit(low, high).into_pyobject(py)?.unbind(),
        })
    }

    /// music21 reads a segment in one of two states. Only the expression as
    /// written is modelled here.
    fn checked_state(state: Option<&str>) -> PyResult<()> {
        match state {
            None | Some("exp") => Ok(()),
            Some("cmp") => Err(SieveException::new_err(
                "the compressed reading of a sieve is not modelled; only 'exp' is",
            )),
            Some(other) => Err(PyValueError::new_err(format!(
                "{other} is not a sieve state"
            ))),
        }
    }
}

#[pymethods]
impl Sieve {
    #[new]
    #[pyo3(signature = (usrStr, z = None))]
    fn new(usrStr: &Bound<'_, PyAny>, z: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        // music21 takes a list of expressions as well as one string, and
        // joins them with `|`.
        let expression = if let Ok(text) = usrStr.extract::<String>() {
            text
        } else {
            let mut parts: Vec<String> = Vec::new();
            for item in usrStr.try_iter()? {
                parts.push(item?.extract()?);
            }
            parts.join("|")
        };
        Ok(Self {
            inner: RsSieve::parse(&expression).map_err(sieve_error)?,
            z: bounds_of(z)?,
            segment_format: None,
        })
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    /// music21's `represent`, which `__str__` is. The `style` argument picks
    /// the mathematical spelling of a residual, which the crate does not
    /// write.
    #[pyo3(signature = (state = None, style = None))]
    fn represent(&self, state: Option<&str>, style: Option<&str>) -> PyResult<String> {
        Self::checked_state(state)?;
        if let Some(style) = style {
            return Err(SieveException::new_err(format!(
                "the {style} spelling of a residual is not written here"
            )));
        }
        Ok(self.inner.to_string())
    }

    fn period(&self) -> u32 {
        self.inner.period()
    }

    #[pyo3(signature = (state = None, n = None, z = None, segmentFormat = None))]
    fn segment(
        &self,
        py: Python<'_>,
        state: Option<&str>,
        n: Option<&Bound<'_, PyAny>>,
        z: Option<&Bound<'_, PyAny>>,
        segmentFormat: Option<&str>,
    ) -> PyResult<Py<PyAny>> {
        Self::checked_state(state)?;
        let (low, high) = match z {
            Some(z) => bounds_of(Some(z))?,
            None => self.z,
        };
        let format = SegmentFormat::parse(segmentFormat.or(self.segment_format.as_deref()))?;
        let shift = match n {
            Some(n) => shift_of(n)?,
            None => Some(0),
        };
        self.segment_in(shift, low, high, format, py)
    }

    /// music21's `__call__`, which is `segment` in the object's own state.
    /// `PitchSieve` realizes its pitches through this.
    #[pyo3(signature = (n = None, z = None, segmentFormat = None))]
    fn __call__(
        &self,
        py: Python<'_>,
        n: Option<&Bound<'_, PyAny>>,
        z: Option<&Bound<'_, PyAny>>,
        segmentFormat: Option<&str>,
    ) -> PyResult<Py<PyAny>> {
        self.segment(py, None, n, z, segmentFormat)
    }

    #[pyo3(signature = (n, zMinimum, length, segmentFormat, zStep = 100))]
    fn collect(
        &self,
        py: Python<'_>,
        n: &Bound<'_, PyAny>,
        zMinimum: IntegerType,
        length: usize,
        segmentFormat: Option<&str>,
        zStep: IntegerType,
    ) -> PyResult<Py<PyAny>> {
        // The crate walks the sieve itself; how far it steps between reads is
        // music21's own bookkeeping and changes no answer.
        let _ = zStep;
        let Some(n) = shift_of(n)? else {
            return Err(SieveException::new_err(
                "desired length of sieve segment cannot be found",
            ));
        };
        let members = self
            .inner
            .collect(n, zMinimum, length)
            .map_err(sieve_error)?;
        let format = SegmentFormat::parse(segmentFormat)?;
        let (Some(low), Some(high)) = (members.first().copied(), members.last().copied()) else {
            return Ok(Vec::<IntegerType>::new().into_pyobject(py)?.unbind());
        };
        // music21 gathers integers and only then reads them in the format
        // asked for, over a range running from the first member to the last.
        match format {
            SegmentFormat::Integer => Ok(members.into_pyobject(py)?.unbind()),
            _ => self.segment_in(Some(n), low, high, format, py),
        }
    }

    fn setSegmentFormat(&mut self, segmentFormat: &str) -> PyResult<()> {
        SegmentFormat::parse(Some(segmentFormat))?;
        self.segment_format = Some(segmentFormat.to_string());
        Ok(())
    }

    fn setZ(&mut self, z: &Bound<'_, PyAny>) -> PyResult<()> {
        self.z = bounds_of(Some(z))?;
        Ok(())
    }

    fn setZRange(&mut self, minInt: IntegerType, maxInt: IntegerType) -> PyResult<()> {
        if maxInt < minInt {
            return Err(SieveException::new_err("z range must be increasing"));
        }
        self.z = (minInt, maxInt);
        Ok(())
    }

    /// music21 puts the right operand first and braces both sides, so
    /// `a & b` reads `{b}&{a}`.
    fn __and__(&self, other: &Sieve) -> Self {
        Self {
            inner: other.inner.intersection(&self.inner),
            z: widest(self.z, other.z),
            segment_format: None,
        }
    }

    fn __or__(&self, other: &Sieve) -> Self {
        Self {
            inner: other.inner.union(&self.inner),
            z: widest(self.z, other.z),
            segment_format: None,
        }
    }

    fn __xor__(&self, other: &Sieve) -> Self {
        Self {
            inner: other.inner.symmetric_difference(&self.inner),
            z: widest(self.z, other.z),
            segment_format: None,
        }
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
}

/// music21 takes the union of the two ranges when it combines two sieves.
fn widest(
    left: (IntegerType, IntegerType),
    right: (IntegerType, IntegerType),
) -> (IntegerType, IntegerType) {
    (left.0.min(right.0), left.1.max(right.1))
}

/// The primes music21's `eratosthenes` yields, one at a time and without
/// end.
#[pyclass(module = "music21.sieve")]
pub struct Primes {
    inner: Box<dyn Iterator<Item = u64> + Send + Sync>,
}

impl std::fmt::Debug for Primes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Primes")
    }
}

#[pymethods]
impl Primes {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>) -> Option<u64> {
        slf.inner.next()
    }
}

/// music21's `eratosthenes`: every prime from `firstCandidate` upward.
#[pyfunction]
#[pyo3(name = "eratosthenes", signature = (firstCandidate = 2))]
fn eratosthenes(firstCandidate: u64) -> Primes {
    Primes {
        inner: Box::new(music21_rs_crate::sieve::eratosthenes(firstCandidate)),
    }
}

/// music21's `rabinMiller`: whether a number is prime.
#[pyfunction]
#[pyo3(name = "rabinMiller")]
fn rabinMiller(n: i64) -> bool {
    music21_rs_crate::sieve::rabin_miller(n)
}

/// music21's `discreteBinaryPad`: a series of integers as one flag per
/// integer across the range they span, or across `fixRange`.
#[pyfunction]
#[pyo3(name = "discreteBinaryPad", signature = (series, fixRange = None))]
fn discreteBinaryPad(
    series: Vec<IntegerType>,
    fixRange: Option<Vec<IntegerType>>,
) -> PyResult<Vec<IntegerType>> {
    Ok(
        music21_rs_crate::sieve::discrete_binary_pad(&series, fixRange.as_deref())
            .map_err(sieve_error)?
            .into_iter()
            .map(IntegerType::from)
            .collect(),
    )
}

/// music21's `unitNormRange`: a series scaled onto the unit interval, over
/// its own range or over `fixRange`.
#[pyfunction]
#[pyo3(name = "unitNormRange", signature = (series, fixRange = None))]
fn unitNormRange(series: Vec<f64>, fixRange: Option<Vec<f64>>) -> Vec<f64> {
    music21_rs_crate::sieve::unit_norm_range(&series, fixRange.as_deref())
}

/// music21's `unitNormEqual`: `parts` points spread evenly over the unit
/// interval.
#[pyfunction]
#[pyo3(name = "unitNormEqual")]
fn unitNormEqual(parts: usize) -> Vec<f64> {
    music21_rs_crate::sieve::unit_norm_equal(parts)
}

/// music21's `unitNormStep`: the interval from `a` to `b` walked in steps
/// of `step`, scaled onto the unit interval unless `normalized` is off.
#[pyfunction]
#[pyo3(name = "unitNormStep", signature = (step, a = 0.0, b = 1.0, normalized = true))]
fn unitNormStep(step: f64, a: f64, b: f64, normalized: bool) -> PyResult<Vec<f64>> {
    music21_rs_crate::sieve::unit_norm_step(step, a, b, normalized).map_err(sieve_error)
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Primes>()?;
    m.add_function(wrap_pyfunction!(eratosthenes, m)?)?;
    m.add_function(wrap_pyfunction!(rabinMiller, m)?)?;
    m.add_function(wrap_pyfunction!(discreteBinaryPad, m)?)?;
    m.add_function(wrap_pyfunction!(unitNormRange, m)?)?;
    m.add_function(wrap_pyfunction!(unitNormEqual, m)?)?;
    m.add_function(wrap_pyfunction!(unitNormStep, m)?)?;
    m.add_class::<Sieve>()?;
    Ok(())
}
