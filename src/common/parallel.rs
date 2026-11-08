//! music21's `common.parallel`, as far as it is not about Python objects.

use std::ops::Range;

/// How many processes music21 runs at once on a machine with this many
/// processors: one fewer than there are, leaving one free, from three up.
///
/// ```
/// use music21::common::parallel::cpus_for;
///
/// assert_eq!(cpus_for(8), 7);
/// assert_eq!(cpus_for(2), 2);
/// ```
pub fn cpus_for(count: usize) -> usize {
    if count >= 3 {
        return count - 1;
    }
    count
}

/// [`cpus_for`] this machine, as far as the standard library can tell how
/// many processors it has.
pub fn cpus() -> usize {
    cpus_for(std::thread::available_parallelism().map_or(1, std::num::NonZero::get))
}

/// The positions an update reports on once `done` tasks are done and it
/// reports every `window` of them: the last `window` positions, none before
/// the first.
///
/// ```
/// use music21::common::parallel::update_positions;
///
/// assert_eq!(update_positions(3, 3), 0..3);
/// assert_eq!(update_positions(2, 3), 0..2);
/// ```
pub fn update_positions(done: i64, window: i64) -> Range<i64> {
    done.saturating_sub(window).max(0)..done
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nothing is reported before any task is done, nor for a window that
    /// is not positive.
    #[test]
    fn updates_report_only_done_positions() {
        assert!(update_positions(0, 3).is_empty());
        assert!(update_positions(5, 0).is_empty());
        assert!(update_positions(5, -2).is_empty());
        assert_eq!(update_positions(9, 3), 6..9);
        assert_eq!(cpus_for(1), 1);
        assert_eq!(cpus_for(3), 2);
        assert!(cpus() >= 1);
    }
}
