//! music21's `scale.intervalNetwork` enums: `Direction`, the way a scale
//! is walked, and `Terminus`, either end of its network.
//!
//! They are Python `enum.Enum`s, as music21's are, so code comparing
//! against `Direction.ASCENDING` or iterating the members works unchanged.
//! pyo3 cannot build a class on `enum.Enum`, so each is made with the enum
//! module's functional form, with the `repr` written here.

use pyo3::prelude::*;
use pyo3::types::PyModule;

/// The names this module hands out.
pub const NAMES: &[&str] = &["Direction", "Terminus"];

/// `Direction.ASCENDING` and the like: the class's name and the member's.
#[pyfunction]
fn member_name(member: &Bound<'_, PyAny>) -> PyResult<String> {
    let class = member.get_type().name()?.to_string();
    let name: String = member.getattr("name")?.extract()?;
    Ok(format!("{class}.{name}"))
}

/// An enum of music21's, its members and their values in music21's order.
fn made_enum<'py>(
    m: &Bound<'py, PyModule>,
    name: &str,
    members: &[(&str, &str)],
    doc: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py = m.py();
    let keywords = pyo3::types::PyDict::new(py);
    keywords.set_item("module", "music21.scale.intervalNetwork")?;
    keywords.set_item("qualname", name)?;
    let class = py
        .import("enum")?
        .getattr("Enum")?
        .call((name, members.to_vec()), Some(&keywords))?;
    let shown = py
        .import("functools")?
        .getattr("partialmethod")?
        .call1((wrap_pyfunction!(member_name, m)?,))?;
    class.setattr("__repr__", &shown)?;
    class.setattr("__str__", &shown)?;
    class.setattr("__doc__", doc)?;
    Ok(class)
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let direction = made_enum(
        m,
        "Direction",
        &[
            ("BI", "bi"),
            ("ASCENDING", "ascending"),
            ("DESCENDING", "descending"),
        ],
        "The way a scale is walked: Direction.ASCENDING, Direction.DESCENDING, \
         or Direction.BI for both.",
    )?;
    m.add("Direction", direction)?;
    let terminus = made_enum(
        m,
        "Terminus",
        &[("LOW", "terminusLow"), ("HIGH", "terminusHigh")],
        "Either end of a scale's network: Terminus.LOW or Terminus.HIGH.",
    )?;
    m.add("Terminus", terminus)?;
    Ok(())
}
