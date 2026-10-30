//! Every corpus score the suites before this one cached, read back with
//! nothing of this package installed.
//!
//! music21 caches a parsed corpus score as a pickle in one directory for
//! the whole machine, and a suite that has swapped a class writes that
//! class into the pickle. A later process reading it calls the wheel's
//! `_thawed`, which installs the modules `MUSIC21_MODULES` carries and
//! then, for a module it does not carry, hands back the **bare** facade
//! class -- which is not a `Music21Object`, so music21's own streams refuse
//! it and the score cannot be thawed at all.
//!
//! That is not hypothetical: swapping `music21.bar` in `doctest_windowed`
//! did it, and `music21_suite` failed three of music21's own tests on it
//! (`serial.testLabelingA`, `volume.testRealizeVolumeB`,
//! `key.testTonalAmbiguityA`), each dying on `'music21.bar.Barline' object
//! has no attribute 'activeSite'`. Nothing local said why: each test passes
//! on its own, and only the two run in order fail.
//!
//! **The name is what makes this work.** `cargo test` runs the test
//! binaries in order, so a name sorting after every `doctest_*` and after
//! `music21_suite` sees the cache they left -- which is the cache a real
//! run has, not the one a suite would write if it ran first. Twenty of the
//! eighty-five suites *would* poison it if each were first; in the order
//! they actually run, the first to parse a score writes the pickle and the
//! rest read it, and nothing is poisoned. This is what says so, and what
//! will say otherwise if a reordering, a new suite or a new swap changes
//! which suite gets there first.
//!
//! Nothing is installed here, on purpose: that is the condition being
//! tested. It reads the cache rather than parsing anything, so it costs
//! seconds.
//!
//! **It reads whatever is on disk, which locally may be older than this
//! run.** The cache outlives a run and is one directory for the machine, so
//! a pickle a *previous* run poisoned fails this until the directory is
//! cleared -- `rm ~/AppData/Local/Temp/music21/*.p.gz` on Windows, under
//! `getRootTempDir` elsewhere. On a CI runner the directory starts empty,
//! so what it reads is always this run's.
//!
//! **Under nextest it has nothing to read.** There every test process gets
//! an empty directory of its own (`utils::init_py`), so no suite reads
//! another's pickles, and this checks an empty directory. What it guards
//! against is a `cargo test` run.

use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

/// Reads every cached score the way any later process would.
const THAW: &str = r#"
import glob
import os

from music21 import environment, freezeThaw

def unreadable():
    """Each cached score that cannot be read, with why."""
    directory = environment.Environment().getRootTempDir()
    trouble = []
    read = 0
    for path in sorted(glob.glob(os.path.join(str(directory), '*.p.gz'))):
        thawer = freezeThaw.StreamThawer()
        try:
            # music21 writes these with zlib and names them `.p.gz`, so the
            # compression has to be named rather than guessed from the name.
            thawer.open(path, zipType='zlib')
            _ = thawer.stream
            read += 1
        except Exception as problem:  # noqa: BLE001 -- whatever it is, it is news
            trouble.append(f'{os.path.basename(path)}: {type(problem).__name__}: {problem}')
    return read, trouble
"#;

#[test]
fn every_cached_corpus_score_reads_back_in_a_plain_music21() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (read, trouble) = Python::attach(|py| -> PyResult<(usize, Vec<String>)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let helper = PyModule::from_code(
            py,
            &std::ffi::CString::new(THAW).expect("no nul in the helper"),
            c"corpus_cache_helper.py",
            c"corpus_cache_helper",
        )?;
        helper
            .getattr("unreadable")?
            .call0()?
            .extract::<(usize, Vec<String>)>()
    })
    .unwrap_or_else(|error| panic!("reading music21's corpus cache: {error}"));

    assert!(
        trouble.is_empty(),
        "{} of {} cached corpus scores cannot be read back with nothing installed:\n  {}\n\n\
         A suite before this one swapped a class `install_into_music21` does not install \
         and was the first to parse that score, so its pickle carries the facade. Either \
         stop that suite swapping the module, or answer music21's own class where music21 \
         can be asked -- `stream::barline_object` is the precedent.",
        trouble.len(),
        read + trouble.len(),
        trouble.join("\n  "),
    );
}
