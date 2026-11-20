//! Graphs of a score's data: music21's `graph`, the data side.
//!
//! What a plot shows is worked out here: the ticks and labels of an axis
//! ([`axis`]), the plot a format and a list of quantities ask for
//! ([`find`]), the layout numbers a graph primitive draws with
//! ([`primitives`]), and colours and accidentals as matplotlib takes them
//! ([`utilities`]). Drawing is matplotlib's, and is the wheel's to drive.

pub mod axis;
pub mod find;
pub mod primitives;
pub mod utilities;

/// Python's `range(start, stop, step)`, which the tick rules count with.
///
/// ```
/// use music21::graph::py_range;
///
/// assert_eq!(py_range(0, 10, 4), vec![0, 4, 8]);
/// assert_eq!(py_range(5, 0, -2), vec![5, 3, 1]);
/// assert!(py_range(0, 10, -1).is_empty());
/// ```
///
/// # Panics
///
/// Never; a step of nought gives nothing, where Python raises.
pub fn py_range(start: i64, stop: i64, step: i64) -> Vec<i64> {
    let mut out = Vec::new();
    if step == 0 {
        return out;
    }
    let mut value = start;
    while (step > 0 && value < stop) || (step < 0 && value > stop) {
        out.push(value);
        value += step;
    }
    out
}
