//! The parity harness's doctest suites, read off the test files that
//! declare them. Both the fixture generator and the report need the list,
//! and the report is built without the `python` feature.

use std::fs;
use std::path::Path;

/// One `python-parity/tests/doctest_*.rs` suite: the music21 module it runs
/// and the name its expectation file and summary carry.
#[derive(Debug, PartialEq)]
pub(crate) struct HarnessSuite {
    pub(crate) module: String,
    pub(crate) name: String,
}

/// Every doctest suite the parity harness has, read off the suites.
pub(crate) fn harness_suites(workspace_root: &Path) -> Vec<HarnessSuite> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(workspace_root.join("python-parity/tests")) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("doctest_") && name.ends_with(".rs"))
        {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        if let Some(suite) = suite_in(&text) {
            found.push(suite);
        }
    }
    found
}

/// The suite a test file declares, from its
/// `doctest_suite!(name, "music21.x", "expectations", [...])`.
fn suite_in(text: &str) -> Option<HarnessSuite> {
    let at = text.find("doctest_suite!(")?;
    // The quoted strings alternate with what is between them, so the module
    // and the name are the first and second quoted.
    let mut quoted = text[at..].split('"').skip(1).step_by(2);
    let module = quoted.next()?;
    let name = quoted.next()?;
    if !module.starts_with("music21.") {
        return None;
    }
    Some(HarnessSuite {
        module: module.to_string(),
        name: name.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suite_names_module_and_name() {
        let text = "music21_rs_python_parity::doctest_suite!(\n    reduce,\n    \"music21.analysis.reduceChords\",\n    \"reducechords\",\n    [(\"music21.pitch\", doctest::PITCH_NAMES)]\n);";
        assert_eq!(
            suite_in(text),
            Some(HarnessSuite {
                module: "music21.analysis.reduceChords".to_string(),
                name: "reducechords".to_string(),
            })
        );
        assert_eq!(suite_in("fn main() {}"), None);
    }
}
