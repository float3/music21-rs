//! What music21's `stream.iterator` decides that is not about Python
//! objects: how `OffsetIterator` groups what it walks.

use std::ops::Range;

/// The runs of neighbours standing at one offset, as music21's
/// `OffsetIterator` hands them out: each run the positions of elements
/// next to one another in walking order whose offsets are equal. Two
/// elements at one offset with another between them are two runs.
///
/// ```
/// use music21::stream::iterator::offset_runs;
///
/// assert_eq!(offset_runs(&[0.0, 0.0, 1.0, 2.0, 2.0]), vec![0..2, 2..3, 3..5]);
/// assert!(offset_runs(&[]).is_empty());
/// ```
pub fn offset_runs(offsets: &[f64]) -> Vec<Range<usize>> {
    let mut runs = Vec::new();
    let mut start = 0;
    for at in 1..=offsets.len() {
        if at == offsets.len() || offsets[at] != offsets[start] {
            runs.push(start..at);
            start = at;
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::offset_runs;

    /// An offset met again after another starts a run of its own.
    #[test]
    fn offset_met_again_is_a_new_run() {
        assert_eq!(offset_runs(&[1.0, 2.0, 1.0]), vec![0..1, 1..2, 2..3]);
    }

    /// One element is one run.
    #[test]
    fn one_element_is_one_run() {
        assert_eq!(offset_runs(&[3.5]), vec![0..1]);
    }
}
