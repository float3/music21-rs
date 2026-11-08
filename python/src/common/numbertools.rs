//! music21's `common.numberTools`, over the crate's
//! `common::numbertools`.

use num_bigint::BigInt;
use pyo3::exceptions::{
    PyIndexError, PyOverflowError, PyTypeError, PyValueError, PyZeroDivisionError,
};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyFloat, PyFrozenSet, PyInt, PyList, PyString, PyTuple};
use pyo3::wrap_pyfunction;

use music21::common::numbertools as tools;
use music21::common::numbertools::{IntOrFloat, OffsetQL};

use super::value_error;

/// music21's `numberTools.__all__`.
const ALL: [&str; 21] = [
    "addFloatPrecision",
    "approximateGCD",
    "contiguousList",
    "decimalToTuplet",
    "dotMultiplier",
    "fromRoman",
    "groupContiguousIntegers",
    "mixedNumeral",
    "musicOrdinals",
    "nearestMultiple",
    "numToIntOrFloat",
    "opFrac",
    "ordinalAbbreviation",
    "ordinals",
    "ordinalsToNumbers",
    "roundToHalfInteger",
    "strTrimFloat",
    "toRoman",
    "unitBoundaryProportion",
    "unitNormalizeProportion",
    "weightedSelection",
];

static FRACTION: PyOnceLock<Py<PyAny>> = PyOnceLock::new();
static KNOWN_PASSES: PyOnceLock<Py<PyFrozenSet>> = PyOnceLock::new();

/// Python's `fractions.Fraction`.
pub(crate) fn fraction_class(py: Python<'_>) -> PyResult<&Bound<'_, PyAny>> {
    FRACTION.import(py, "fractions", "Fraction")
}

/// music21's `_KNOWN_PASSES`: the plain note lengths `opFrac` hands back at
/// once, as floats.
fn known_passes(py: Python<'_>) -> PyResult<&Bound<'_, PyFrozenSet>> {
    KNOWN_PASSES
        .get_or_try_init(py, || {
            let lengths = [
                0.0625, 0.09375, 0.125, 0.1875, 0.25, 0.375, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0,
                6.0,
            ];
            Ok(PyFrozenSet::new(py, lengths)?.unbind())
        })
        .map(|set| set.bind(py))
}

/// A quarter length as music21 keeps one: a float or a `Fraction`.
pub(crate) fn offset_ql(py: Python<'_>, value: OffsetQL) -> PyResult<Bound<'_, PyAny>> {
    match value {
        OffsetQL::Float(value) => Ok(PyFloat::new(py, value).into_any()),
        OffsetQL::Fraction {
            numerator,
            denominator,
        } => fraction_class(py)?.call1((numerator, denominator)),
    }
}

fn int_or_float(py: Python<'_>, value: IntOrFloat) -> PyResult<Bound<'_, PyAny>> {
    match value {
        IntOrFloat::Int(value) => Ok(value.into_pyobject(py)?.into_any()),
        IntOrFloat::Float(value) => Ok(PyFloat::new(py, value).into_any()),
    }
}

/// A number as a float the way Python's `float()` reads one, a string
/// included.
fn as_float(value: &Bound<'_, PyAny>) -> PyResult<f64> {
    if let Ok(value) = value.extract::<f64>() {
        return Ok(value);
    }
    let builtins = value.py().import("builtins")?;
    builtins.getattr("float")?.call1((value,))?.extract()
}

/// A real number as a float; for anything else, the error Python raises
/// for the operation music21 tries on it first.
fn real<T>(value: &Bound<'_, PyAny>, first: impl FnOnce() -> PyResult<T>) -> PyResult<f64> {
    if let Ok(value) = value.extract::<f64>() {
        return Ok(value);
    }
    first()?;
    Err(PyTypeError::new_err(format!(
        "must be real number, not {}",
        value.get_type().name()?
    )))
}

/// The values of an iterable, each read as a float.
fn floats(values: &Bound<'_, PyAny>) -> PyResult<Vec<f64>> {
    values
        .try_iter()?
        .map(|value| value.and_then(|value| as_float(&value)))
        .collect()
}

fn op_frac_float(py: Python<'_>, value: f64) -> PyResult<Bound<'_, PyAny>> {
    if value.is_infinite() {
        return Err(PyOverflowError::new_err(
            "cannot convert Infinity to integer ratio",
        ));
    }
    let snapped = tools::op_frac(value).map_err(|error| value_error(&error))?;
    offset_ql(py, snapped)
}

/// Whether a Python int is a power of two, `d & (d - 1) == 0` as music21
/// tests it.
fn binary(denominator: &Bound<'_, PyAny>) -> PyResult<bool> {
    let below = denominator.sub(1)?;
    denominator.bitand(below)?.eq(0)
}

/// opFrac -> optionally convert a number to a fraction or back.
///
/// A quarter length or offset as a float where a float says it exactly,
/// and as a `Fraction` with a denominator up to 65535 where none does.
#[pyfunction]
#[pyo3(name = "opFrac")]
fn op_frac<'py>(num: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = num.py();
    if num.is_exact_instance_of::<PyFloat>() {
        return op_frac_float(py, num.extract()?);
    }
    if num.is_exact_instance_of::<PyInt>() {
        return num.add(0.0);
    }
    if num.is_none() {
        return Ok(PyFloat::new(py, 0.0).into_any());
    }
    if known_passes(py)?.contains(num)? {
        return num.add(0.0);
    }
    if num.is_instance(fraction_class(py)?)? {
        let denominator = num.getattr("denominator")?;
        if binary(&denominator)? {
            return num.getattr("numerator")?.div(denominator.add(0.0)?);
        }
        return Ok(num.clone());
    }
    if num.is_instance_of::<PyInt>() {
        return num.add(0.0);
    }
    if num.is_instance_of::<PyFloat>() {
        return op_frac_float(py, num.extract()?);
    }
    Err(PyTypeError::new_err(format!(
        "Cannot convert num: {}",
        num.str()?
    )))
}

/// music21's `_preFracLimitDenominator`: the fraction nearest `n / d`
/// with a denominator up to 65535, as a numerator and a denominator.
#[pyfunction]
#[pyo3(name = "_preFracLimitDenominator")]
fn pre_frac_limit_denominator(n: BigInt, d: BigInt) -> PyResult<(BigInt, BigInt)> {
    tools::limit_denominator(&n, &d, &BigInt::from(tools::DENOM_LIMIT))
        .map_err(|error| value_error(&error))
}

/// A number very close to a whole number as that whole number, and any
/// other as a float. A string is read as a number first.
#[pyfunction]
#[pyo3(name = "numToIntOrFloat")]
fn num_to_int_or_float<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = value.py();
    if value.is_exact_instance_of::<PyFloat>() {
        return float_to_int_or_float(py, value.extract()?);
    }
    // Python's own rounding where the value has one, which is exact for a
    // `Fraction` of any size; anything else is read as a float first.
    let builtins = py.import("builtins")?;
    let rounded = match builtins.call_method1("round", (value,)) {
        Ok(rounded) => rounded,
        Err(error)
            if error.is_instance_of::<PyValueError>(py)
                || error.is_instance_of::<PyTypeError>(py) =>
        {
            return float_to_int_or_float(py, as_float(value)?);
        }
        Err(error) => return Err(error),
    };
    if tools::is_close(rounded.extract()?, value.extract()?, 1e-6) {
        return Ok(rounded);
    }
    value.add(0.0)
}

fn float_to_int_or_float(py: Python<'_>, value: f64) -> PyResult<Bound<'_, PyAny>> {
    if value.is_infinite() {
        return Err(PyOverflowError::new_err(
            "cannot convert float infinity to integer",
        ));
    }
    let found = tools::num_to_int_or_float(value).map_err(|error| value_error(&error))?;
    int_or_float(py, found)
}

/// A number as a mixed numeral, `1 1/3`, its fraction's denominator
/// limited to `limitDenominator` unless it is a `Fraction` already.
#[pyfunction]
#[pyo3(name = "mixedNumeral", signature = (expr, limitDenominator = 65535))]
#[allow(non_snake_case)]
fn mixed_numeral(expr: &Bound<'_, PyAny>, limitDenominator: i64) -> PyResult<String> {
    let py = expr.py();
    if expr.is_instance(fraction_class(py)?)? {
        let numerator: BigInt = expr.getattr("numerator")?.extract()?;
        let denominator: BigInt = expr.getattr("denominator")?.extract()?;
        return tools::mixed_numeral_fraction(&numerator, &denominator)
            .map_err(|error| PyOverflowError::new_err(crate::pitch::message(&error)));
    }
    let value = as_float(expr)?;
    if limitDenominator < 1 {
        return Err(PyValueError::new_err(
            "max_denominator should be at least 1",
        ));
    }
    tools::mixed_numeral(value, limitDenominator as u64).map_err(|error| value_error(&error))
}

/// A number rounded to the nearest half, a quarter rounding up: a whole
/// number on a whole number, a float on a half.
#[pyfunction]
#[pyo3(name = "roundToHalfInteger")]
fn round_to_half_integer<'py>(num: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let builtins = num.py().import("builtins")?;
    let value = real(num, || builtins.call_method1("divmod", (num, 1.0)))?;
    let rounded = tools::round_to_half_integer(value).map_err(|error| value_error(&error))?;
    int_or_float(num.py(), rounded)
}

/// A value near a third or a sixth as that `Fraction`, and any other as it
/// was given; a string is read as a float first.
#[pyfunction]
#[pyo3(name = "addFloatPrecision", signature = (x, grain = 1e-2))]
fn add_float_precision<'py>(x: &Bound<'py, PyAny>, grain: f64) -> PyResult<Bound<'py, PyAny>> {
    let py = x.py();
    let x = if x.is_instance_of::<PyString>() {
        PyFloat::new(py, as_float(x)?).into_any()
    } else {
        x.clone()
    };
    match tools::add_float_precision(x.extract()?, grain) {
        Some(found) => offset_ql(py, found),
        None => Ok(x),
    }
}

/// A float written with at most `maxNum` decimals, but never fewer than
/// one.
#[pyfunction]
#[pyo3(name = "strTrimFloat", signature = (floatNum, maxNum = 4))]
#[allow(non_snake_case)]
fn str_trim_float(floatNum: f64, maxNum: i64) -> PyResult<String> {
    let Ok(digits) = usize::try_from(maxNum) else {
        return Err(PyValueError::new_err(
            "unsupported format character '-' (0x2d) at index 2",
        ));
    };
    tools::str_trim_float(floatNum, digits).map_err(|error| value_error(&error))
}

/// The multiple of `unit` nearest a value that is not negative, the error
/// to seven digits and the signed difference.
#[pyfunction]
#[pyo3(name = "nearestMultiple")]
fn nearest_multiple<'py>(
    n: &Bound<'py, PyAny>,
    unit: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py = n.py();
    let value = real(n, || n.lt(0))?;
    let step = real(unit, || n.div(unit))?;
    if value < 0.0 {
        return Err(PyValueError::new_err(format!(
            "n ({}) is less than zero. Thus, cannot find the nearest multiple for a value less \
             than the unit, {}",
            n.str()?,
            unit.str()?
        )));
    }
    if step == 0.0 {
        return Err(PyZeroDivisionError::new_err("float division by zero"));
    }
    let quotient = value / step;
    if quotient.is_nan() {
        return Err(PyValueError::new_err("cannot convert float NaN to integer"));
    }
    if quotient.is_infinite() {
        return Err(PyOverflowError::new_err(
            "cannot convert float infinity to integer",
        ));
    }
    if n.is_exact_instance_of::<PyFloat>() && unit.is_exact_instance_of::<PyFloat>() {
        let found = tools::nearest_multiple(value, step).map_err(|error| value_error(&error))?;
        return PyTuple::new(py, [found.multiple, found.error, found.signed_error]);
    }
    exact_nearest_multiple(n, unit)
}

/// `nearestMultiple` in Python's own arithmetic, for an int or a
/// `Fraction`, which stay exact where a float would not: a whole unit
/// gives whole multiples, and a fraction exact errors.
fn exact_nearest_multiple<'py>(
    n: &Bound<'py, PyAny>,
    unit: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py = n.py();
    let builtins = py.import("builtins")?;
    let multiple = py.import("math")?.call_method1("floor", (n.div(unit)?,))?;
    let low = unit.mul(&multiple)?;
    let high = unit.mul(multiple.add(1)?)?;
    if low.ge(n)? && n.ge(&high)? {
        return Err(PyValueError::new_err(format!(
            "cannot place n between multiples: {}, {}",
            low.str()?,
            high.str()?
        )));
    }
    let round = |value: Bound<'py, PyAny>| builtins.call_method1("round", (value, 7));
    if low.le(n)? && n.le(low.add(unit.div(2.0)?)?)? {
        let error = round(n.sub(&low)?)?;
        return PyTuple::new(py, [low, error.clone(), error]);
    }
    let error = round(high.sub(n)?)?;
    let signed = round(n.sub(&high)?)?;
    PyTuple::new(py, [high, error, signed])
}

/// What a note's length is multiplied by for `dots` dots.
#[pyfunction]
#[pyo3(name = "dotMultiplier")]
fn dot_multiplier(dots: i64) -> PyResult<f64> {
    if dots < -9 {
        return Err(PyIndexError::new_err("tuple index out of range"));
    }
    if dots < 0 {
        // A negative index counts from the end of music21's table.
        return Ok(tools::dot_multiplier((9 + dots) as u32));
    }
    if dots >= 1023 {
        return Err(PyOverflowError::new_err(
            "(34, 'Numerical result out of range')",
        ));
    }
    Ok(tools::dot_multiplier(dots as u32))
}

/// The tuplet ratio, in lowest terms, a simple decimal stands for.
#[pyfunction]
#[pyo3(name = "decimalToTuplet")]
#[allow(non_snake_case)]
fn decimal_to_tuplet(decNum: &Bound<'_, PyAny>) -> PyResult<(i128, i128)> {
    let decNum = real(decNum, || decNum.le(0))?;
    if decNum <= 0.0 {
        return Err(PyZeroDivisionError::new_err(
            "number must be greater than zero",
        ));
    }
    tools::decimal_to_tuplet(decNum).map_err(|error| value_error(&error))
}

/// The values the iterable holds, refused where they sum to nought.
fn proportions(values: &Bound<'_, PyAny>) -> PyResult<Vec<f64>> {
    let values = floats(values)?;
    let sum: f64 = values.iter().sum();
    if !values.is_empty() && sum == 0.0 && values.iter().all(|value| *value >= 0.0) {
        return Err(PyZeroDivisionError::new_err("float division by zero"));
    }
    Ok(values)
}

/// Each value as its share of their sum.
#[pyfunction]
#[pyo3(name = "unitNormalizeProportion")]
fn unit_normalize_proportion(values: &Bound<'_, PyAny>) -> PyResult<Vec<f64>> {
    tools::unit_normalize_proportion(&proportions(values)?).map_err(|error| value_error(&error))
}

/// The span of the unit interval each value's share of their sum takes.
#[pyfunction]
#[pyo3(name = "unitBoundaryProportion")]
fn unit_boundary_proportion(series: &Bound<'_, PyAny>) -> PyResult<Vec<(f64, f64)>> {
    tools::unit_boundary_proportion(&proportions(series)?).map_err(|error| value_error(&error))
}

/// One of `values`, chosen at random by `weights`; the draw is
/// `randomGenerator()`, or Python's `random.random()`.
#[pyfunction]
#[pyo3(name = "weightedSelection", signature = (values, weights, randomGenerator = None))]
#[allow(non_snake_case)]
fn weighted_selection<'py>(
    values: &Bound<'py, PyAny>,
    weights: &Bound<'py, PyAny>,
    randomGenerator: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let draw = match randomGenerator {
        Some(generator) => generator.call0()?,
        None => values.py().import("random")?.call_method0("random")?,
    };
    let index = tools::weighted_index(&proportions(weights)?, draw.extract()?)
        .map_err(|error| value_error(&error))?;
    values.get_item(index)
}

/// The greatest length that goes a whole number of times, to within
/// `grain`, into every one of the values.
#[pyfunction]
#[pyo3(name = "approximateGCD", signature = (values, grain = 1e-4))]
fn approximate_gcd(values: &Bound<'_, PyAny>, grain: f64) -> PyResult<f64> {
    let values = floats(values)?;
    if values.iter().copied().reduce(f64::min) == Some(0.0) {
        return Err(PyZeroDivisionError::new_err("float division by zero"));
    }
    tools::approximate_gcd(&values, grain).map_err(|error| value_error(&error))
}

/// The values of a sequence as whole numbers, or as floats where they are
/// not all whole.
enum Numbers {
    Whole(Vec<i64>),
    Float(Vec<f64>),
}

fn numbers(values: &Bound<'_, PyAny>) -> PyResult<Numbers> {
    if let Ok(whole) = values.extract::<Vec<i64>>() {
        return Ok(Numbers::Whole(whole));
    }
    Ok(Numbers::Float(floats(values)?))
}

/// Whether a sorted sequence holds only values each one more than the
/// last.
#[pyfunction]
#[pyo3(name = "contiguousList")]
#[allow(non_snake_case)]
fn contiguous_list(inputListOrTuple: &Bound<'_, PyAny>) -> PyResult<bool> {
    // The first value is read whatever happens, and an empty sequence
    // raises Python's own `IndexError` for it.
    inputListOrTuple.get_item(0)?;
    Ok(match numbers(inputListOrTuple)? {
        Numbers::Whole(values) => tools::contiguous_list(&values),
        Numbers::Float(values) => tools::contiguous_list(&values),
    })
}

/// A list of integers, sorted where it stands, in groups of contiguous
/// values.
#[pyfunction]
#[pyo3(name = "groupContiguousIntegers")]
fn group_contiguous_integers<'py>(src: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
    let py = src.py();
    if src.len()? <= 1 {
        return PyList::new(py, [src]);
    }
    src.call_method0("sort")?;
    let runs = match numbers(src)? {
        Numbers::Whole(values) => tools::contiguous_runs(&values),
        Numbers::Float(values) => tools::contiguous_runs(&values),
    };
    let groups = PyList::empty(py);
    for run in runs {
        let group = PyList::empty(py);
        for index in run {
            group.append(src.get_item(index)?)?;
        }
        groups.append(group)?;
    }
    Ok(groups)
}

/// A Roman numeral, upper or lower case, as an int; `strictModern`
/// refuses the medieval subtractions like `IC`.
#[pyfunction]
#[pyo3(name = "fromRoman", signature = (num, *, strictModern = false))]
#[allow(non_snake_case)]
fn from_roman(num: &Bound<'_, PyAny>, strictModern: bool) -> PyResult<i64> {
    if !num.is_instance_of::<PyString>() {
        // Python's own error for a value that has no `upper`.
        num.call_method0("upper")?;
    }
    tools::from_roman(&num.extract::<String>()?, strictModern).map_err(|error| value_error(&error))
}

/// A number from 1 to 3999 as a Roman numeral.
#[pyfunction]
#[pyo3(name = "toRoman")]
fn to_roman(num: &Bound<'_, PyAny>) -> PyResult<String> {
    if !num.is_instance_of::<PyInt>() {
        return Err(PyTypeError::new_err(format!(
            "expected integer, got {}",
            num.get_type().str()?
        )));
    }
    let number = num.extract::<i64>().unwrap_or(0);
    tools::to_roman(number).map_err(|error| value_error(&error))
}

/// The ordinal abbreviation of a number, `st`, `nd`, `rd` or `th`, with
/// an `s` for `plural`.
#[pyfunction]
#[pyo3(name = "ordinalAbbreviation", signature = (value, plural = false))]
fn ordinal_abbreviation(value: &Bound<'_, PyAny>, plural: bool) -> PyResult<&'static str> {
    if let Ok(whole) = value.extract::<i64>() {
        return Ok(tools::ordinal_abbreviation(whole, plural));
    }
    // Python's own `%`, which a value that is no number refuses, and which
    // leaves something over for one that is not whole.
    let hundredths = value.rem(100)?;
    if !value.rem(1)?.eq(0)? {
        return Err(PyValueError::new_err("Something really weird"));
    }
    let whole: i64 = hundredths.call_method0("__int__")?.extract()?;
    Ok(tools::ordinal_abbreviation(whole, plural))
}

/// `music21.common.numberTools`, built here.
pub(super) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let module = PyModule::new(py, "music21.common.numberTools")?;
    module.add_function(wrap_pyfunction!(op_frac, &module)?)?;
    module.add_function(wrap_pyfunction!(pre_frac_limit_denominator, &module)?)?;
    module.add_function(wrap_pyfunction!(num_to_int_or_float, &module)?)?;
    module.add_function(wrap_pyfunction!(mixed_numeral, &module)?)?;
    module.add_function(wrap_pyfunction!(round_to_half_integer, &module)?)?;
    module.add_function(wrap_pyfunction!(add_float_precision, &module)?)?;
    module.add_function(wrap_pyfunction!(str_trim_float, &module)?)?;
    module.add_function(wrap_pyfunction!(nearest_multiple, &module)?)?;
    module.add_function(wrap_pyfunction!(dot_multiplier, &module)?)?;
    module.add_function(wrap_pyfunction!(decimal_to_tuplet, &module)?)?;
    module.add_function(wrap_pyfunction!(unit_normalize_proportion, &module)?)?;
    module.add_function(wrap_pyfunction!(unit_boundary_proportion, &module)?)?;
    module.add_function(wrap_pyfunction!(weighted_selection, &module)?)?;
    module.add_function(wrap_pyfunction!(approximate_gcd, &module)?)?;
    module.add_function(wrap_pyfunction!(contiguous_list, &module)?)?;
    module.add_function(wrap_pyfunction!(group_contiguous_integers, &module)?)?;
    module.add_function(wrap_pyfunction!(from_roman, &module)?)?;
    module.add_function(wrap_pyfunction!(to_roman, &module)?)?;
    module.add_function(wrap_pyfunction!(ordinal_abbreviation, &module)?)?;
    module.add("DENOM_LIMIT", tools::DENOM_LIMIT)?;
    module.add("ordinals", PyList::new(py, tools::ORDINAL_NAMES)?)?;
    module.add(
        "musicOrdinals",
        PyList::new(py, tools::MUSIC_ORDINAL_NAMES)?,
    )?;
    let numbers = PyDict::new(py);
    for (name, number) in tools::ordinals_to_numbers() {
        numbers.set_item(name, number)?;
    }
    module.add("ordinalsToNumbers", numbers)?;
    module.add("__all__", ALL.to_vec())?;
    Ok(module)
}
