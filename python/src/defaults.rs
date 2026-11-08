//! music21's `defaults` and `_version`: the constants music21 falls back
//! on, and the version, which here is music21-rs's own.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyModule, PyTuple};

static DEFAULTS: pyo3::sync::PyOnceLock<Py<PyModule>> = pyo3::sync::PyOnceLock::new();
static VERSION: pyo3::sync::PyOnceLock<Py<PyModule>> = pyo3::sync::PyOnceLock::new();

/// The version this package presents as music21's `__version__`.
pub(crate) const VERSION_TEXT: &str = env!("CARGO_PKG_VERSION");

/// A version's parts, as music21's `get_version_tuple` splits them: the
/// numbers as numbers, and anything after the last one's digits (a beta's
/// `b20`) as a part of its own.
pub(crate) fn version_parts(version: &str) -> Vec<VersionPart> {
    let mut parts: Vec<VersionPart> = Vec::new();
    let pieces: Vec<&str> = version.split('.').collect();
    let Some((last, leading)) = pieces.split_last() else {
        return parts;
    };
    for piece in leading {
        parts.push(VersionPart::of(piece));
    }
    let digits: String = last.chars().take_while(char::is_ascii_digit).collect();
    let rest = &last[digits.len()..];
    parts.push(VersionPart::of(&digits));
    if !rest.is_empty() {
        parts.push(VersionPart::Text(rest.to_owned()));
    }
    parts
}

/// One part of a version: a number, or text that is not one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum VersionPart {
    Number(u64),
    Text(String),
}

impl VersionPart {
    fn of(piece: &str) -> Self {
        let numeric = !piece.is_empty() && piece.chars().all(|c| c.is_ascii_digit());
        match piece.parse() {
            Ok(number) if numeric => Self::Number(number),
            _ => Self::Text(piece.to_owned()),
        }
    }
}

impl<'py> IntoPyObject<'py> for VersionPart {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        match self {
            Self::Number(number) => Ok(number.into_pyobject(py)?.into_any()),
            Self::Text(text) => Ok(text.into_pyobject(py)?.into_any()),
        }
    }
}

/// music21's `_version`: `__version__` and `__version_info__`.
pub(crate) fn version_module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = VERSION.get_or_try_init(py, || {
        let m = PyModule::new(py, "music21._version")?;
        m.add("__version__", VERSION_TEXT)?;
        m.add(
            "__version_info__",
            PyTuple::new(py, version_parts(VERSION_TEXT))?,
        )?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

/// music21's `defaults`: what a score, a note or a file is given where
/// nothing says otherwise.
pub(crate) fn defaults_module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = DEFAULTS.get_or_try_init(py, || {
        let m = PyModule::new(py, "music21.defaults")?;
        m.add("title", "Music21 Fragment")?;
        m.add("author", "Music21")?;
        m.add("software", format!("music21 v.{VERSION_TEXT}"))?;
        m.add("musicxmlVersion", "4.0")?;
        m.add("meterNumerator", 4)?;
        m.add("meterDenominator", "quarter")?;
        m.add("meterDenominatorBeatType", 4)?;
        m.add("maxMeterNumerator", 10_000)?;
        m.add("limitOffsetDenominator", 65535)?;
        m.add("partGroup", "Part Group")?;
        m.add("partGroupAbbreviation", "PG")?;
        m.add("durationType", "quarter")?;
        m.add("instrumentName", "")?;
        m.add("partName", "")?;
        m.add("keyFifths", 0)?;
        m.add("keyMode", "major")?;
        m.add("clefSign", "G")?;
        m.add("clefLine", 2)?;
        m.add("noteheadUnpitched", "square")?;
        // Every note's length a whole number of these: 128ths, triplets
        // within triplets, quintuplets and septuplets.
        let divisions = 32 * 3 * 3 * 5 * 7;
        m.add("divisionsPerQuarter", divisions)?;
        m.add("ticksPerQuarter", divisions)?;
        m.add("ticksAtStart", divisions)?;
        m.add("quantizationQuarterLengthDivisors", (4, 3))?;
        m.add("scalingMillimeters", 7)?;
        m.add("scalingTenths", 40)?;
        m.add("jupyterImageDpi", 200)?;
        m.add("multiMeasureRestUseSymbols", true)?;
        m.add("multiMeasureRestMaxSymbols", 11)?;
        m.add("maxMeasures", 100_000)?;
        m.add("minIdNumberToConsiderMemoryLocation", 100_000_001_i64)?;
        m.add("abcVersionDefault", (1, 3, 0))?;
        m.add("_version", version_module(py)?)?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

#[cfg(test)]
mod tests {
    use super::{VersionPart, version_parts};

    #[test]
    fn a_beta_suffix_is_a_part_of_its_own() {
        assert_eq!(
            version_parts("11.0.0b20"),
            [
                VersionPart::Number(11),
                VersionPart::Number(0),
                VersionPart::Number(0),
                VersionPart::Text("b20".to_owned()),
            ]
        );
        assert_eq!(
            version_parts("0.19.0"),
            [
                VersionPart::Number(0),
                VersionPart::Number(19),
                VersionPart::Number(0)
            ]
        );
    }
}
