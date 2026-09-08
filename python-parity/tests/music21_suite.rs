//! music21's own unit tests against the linked crate. See `src/suite.rs`.
//!
//! The *gate* lives in `xtask music21-suite`, which runs the whole of music21's
//! suite against the installed wheel twice — once plain, once with the crate
//! installed over it — and diffs the two. That comparison is what says whether
//! the crate changed music21's behaviour, and it is the right shape for a gate
//! because music21's suite fails a number of its own tests in any environment.
//!
//! This runs the modules the crate replaces against the crate *linked into this
//! binary*, so what music21's tests drive is instrumented and reaches the
//! coverage figure — which the wheel, being a `.pyd` in site-packages with no
//! object to map profiles onto, cannot manage.
//!
//! Having no baseline to diff against, this cannot tell a failure the crate
//! caused from one the environment caused. So it takes the shape the doctest
//! expectations take: what is known to fail is listed, and anything else fails
//! the run. The list is empty today — all 258 of these tests pass against the
//! crate — and an entry added to it should carry the reason, the way
//! `EXPECTED_DIVERGENCES` does in `xtask/src/music21_suite.rs`.

use music21_rs_python_parity::{music21_rs_facade, suite};

/// music21 tests that fail here for reasons that are not the crate's doing,
/// each with why. Empty today; keep it that way where you can.
const EXPECTED_FAILURES: &[(&str, &str)] = &[];

#[test]
fn music21s_own_tests_run_against_the_linked_crate() {
    pyo3::append_to_inittab!(music21_rs_facade);
    let outcome = suite::run(&suite::COVERED_MODULES);

    assert!(
        outcome.installed > 0,
        "install_into_music21 replaced nothing, so nothing here is testing the crate"
    );
    // Guards against the suite quietly collecting nothing — a module rename
    // upstream would otherwise leave this passing on an empty run.
    assert!(
        outcome.run > 200,
        "only {} of music21's tests ran; this is no longer exercising the crate",
        outcome.run
    );

    let unexpected: Vec<&String> = outcome
        .bad
        .iter()
        .filter(|name| !EXPECTED_FAILURES.iter().any(|(known, _)| *known == name.as_str()))
        .collect();
    assert!(
        unexpected.is_empty(),
        "{} of music21's own tests fail against the linked crate:\n  {}",
        unexpected.len(),
        unexpected
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );

    // A listed failure that has stopped failing should stop being excused, or
    // the list goes stale and starts hiding regressions.
    let stale: Vec<&str> = EXPECTED_FAILURES
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| !outcome.bad.contains(*name))
        .collect();
    assert!(
        stale.is_empty(),
        "these no longer fail; drop them from EXPECTED_FAILURES: {stale:?}"
    );
}
