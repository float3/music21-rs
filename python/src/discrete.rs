//! music21's `analysis.discrete`, which asks a stream one question and
//! answers it.
//!
//! What a method name asks for is read by the crate, and a stream answers
//! through [`crate::stream::Stream::analyze`]. The classes music21 keeps
//! here -- `Ambitus`, the five key weightings, `MelodicIntervalDiversity`
//! -- are not among the names yet; the exception they raise is, so that a
//! method name nobody answers to, or a stream with nothing to analyse,
//! raises the class music21 raises.

use pyo3::prelude::*;

/// The names this module hands out.
pub const NAMES: &[&str] = &["DiscreteAnalysisException"];

pyo3::create_exception!(
    music21_rs_facade,
    DiscreteAnalysisException,
    crate::Music21Exception
);

/// Adds the discrete-analysis names to a module.
///
/// The exception is not among them: building one builds music21's own
/// `Music21Exception` and so imports music21, which this wheel is not to
/// do for a name nobody has asked for. `lib.rs`'s table hands it out on
/// first asking.
pub(crate) fn register(_m: &Bound<'_, PyModule>) -> PyResult<()> {
    Ok(())
}
