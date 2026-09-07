//! The wheel and the crate are one release, so they carry one version.
//!
//! `python/Cargo.toml` is outside the workspace and cannot inherit the root
//! crate's version, and `pyproject.toml` takes the wheel's version from it.
//! Without this test a release would ship `music21-rs 0.4.0` on crates.io and
//! `music21_rs 0.3.0` on PyPI.

use std::path::Path;

fn version_of(manifest: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("python-parity has a parent directory");
    let text = std::fs::read_to_string(root.join(manifest))
        .unwrap_or_else(|error| panic!("reading {manifest}: {error}"));
    let table: toml::Table = text
        .parse()
        .unwrap_or_else(|error| panic!("parsing {manifest}: {error}"));
    table["package"]["version"]
        .as_str()
        .unwrap_or_else(|| panic!("{manifest} has no package version"))
        .to_string()
}

#[test]
fn wheel_version_matches_the_crate() {
    let crate_version = version_of("Cargo.toml");
    let wheel_version = version_of("python/Cargo.toml");
    assert_eq!(
        wheel_version, crate_version,
        "python/Cargo.toml says {wheel_version} where the crate says {crate_version}; \
         the wheel and the crate ship as one version"
    );
}
