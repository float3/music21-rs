//! The crate's `Metadata::search` against music21's.
//!
//! One metadata object is built the same way on both sides -- music21's
//! builds it, and the crate's is filled from what music21 then holds, so
//! that the software line music21 writes into every metadata is in both --
//! and the same queries are asked of each. A query and a field must find
//! the same text under the same field name, or find nothing on both sides.
//!
//! The one deviation is where the pattern is compiled. music21 reads a
//! regular expression out of any query holding `*.|+?{}`; the crate carries
//! no engine, so `MetadataQuery::Matcher` takes one the caller built, and
//! this test builds it with `regex` for exactly the queries music21 would
//! read as a pattern. It also holds `OLD_MUSIC21_NAMES`, the grandfathered
//! field names a search takes, to music21's own two tables.

use music21_rs::metadata::{Metadata, MetadataQuery, OLD_MUSIC21_NAMES};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use regex::Regex;
use utils::{init_py, prepare};

const MUSIC21: &str = r#"
from music21 import metadata

def build():
    md = metadata.Metadata()
    md.addContributor(metadata.Contributor(role='composer', name='Joplin, Scott'))
    md['title'] = metadata.Text('Maple Leaf Rag')
    md['dedicatedTo'] = metadata.Text('A Dedicatee')
    md['commissionedBy'] = metadata.Text('A Patron')
    md['movementName'] = metadata.Text('Grave')
    md['number'] = metadata.Text('3')
    md.addContributor(metadata.Contributor(role='tuba player', name='Somebody'))
    return md

def contents(md):
    """Each name, its value as text, and the role a contributor carries."""
    return [(name, str(value), getattr(value, 'role', None))
            for name, value in md.all(returnPrimitives=True, returnSorted=False)]

def search(md, query, field):
    found, where = md.search(query, field=field)
    return (found, where)

def grandfathered():
    from music21.metadata import properties
    rows = []
    for description in properties.STANDARD_PROPERTY_DESCRIPTIONS:
        if not description.oldMusic21Abbrev and not description.oldMusic21WorkId:
            continue
        unique = description.uniqueName or description.name
        rows.append((unique, description.oldMusic21Abbrev, description.oldMusic21WorkId))
    return rows
"#;

/// Each query with the field it is asked of, or none for every field.
const SEARCHES: &[(&str, Option<&str>)] = &[
    ("joplin", Some("composer")),
    ("joplin", Some("compos")),
    ("joplin", None),
    ("Wagner", Some("composer")),
    ("Wagner", None),
    ("leaf", None),
    ("leaf", Some("composer")),
    ("leaf", Some("title")),
    ("LEAF", Some("title")),
    ("rag", Some("otl")),
    ("rag", Some("OTL")),
    ("dedicatee", Some("dedication")),
    ("dedicatee", Some("ode")),
    ("dedicatee", Some("dedicatedTo")),
    ("patron", Some("commission")),
    ("patron", Some("oco")),
    ("somebody", Some("tuba")),
    ("somebody", Some("contributor")),
    ("somebody", None),
    ("grave", Some("movement")),
    ("grave", Some("omd")),
    ("3", Some("number")),
    ("3", Some("onm")),
    ("a", Some("number")),
    ("a", None),
    ("a", Some("nosuchfield")),
    ("", None),
    ("music21", Some("software")),
    ("leaf|entertainer", None),
    ("opl(.*)cott", Some("composer")),
    (".*Rag", Some("title")),
    ("gra.e", None),
    ("ded.*tee", Some("ode")),
    ("no+such+thing", None),
];

/// Whether music21 reads a plain query as a regular expression: it does
/// where the query holds any of these.
fn reads_a_pattern(query: &str) -> bool {
    query.chars().any(|character| "*.|+?{}".contains(character))
}

#[test]
fn the_crate_searches_metadata_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (contents, theirs, grandfathered) = Python::attach(|py| {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"metadata_search_parity_music21.py",
            c"metadata_search_parity_music21",
        )?;
        let md = music21.getattr("build")?.call0()?;
        let contents: Vec<(String, String, Option<String>)> =
            music21.getattr("contents")?.call1((&md,))?.extract()?;
        let search = music21.getattr("search")?;
        let mut theirs = Vec::new();
        for (query, field) in SEARCHES {
            let found: (bool, Option<String>) = search.call1((&md, *query, *field))?.extract()?;
            theirs.push(found);
        }
        let grandfathered: Vec<(String, Option<String>, Option<String>)> =
            music21.getattr("grandfathered")?.call0()?.extract()?;
        PyResult::Ok((contents, theirs, grandfathered))
    })
    .unwrap_or_else(|error| panic!("asking music21 to search its metadata: {error}"));

    let ours_grandfathered: Vec<(String, Option<String>, Option<String>)> = OLD_MUSIC21_NAMES
        .iter()
        .map(|(name, abbreviation, work_id)| {
            (
                (*name).to_string(),
                abbreviation.map(str::to_string),
                work_id.map(str::to_string),
            )
        })
        .collect();
    assert_eq!(ours_grandfathered, grandfathered);

    let mut md = Metadata::new();
    for (name, text, role) in &contents {
        match role {
            Some(role) => md.add_contributor(role.clone(), text.clone()),
            None => md.add_text(name.clone(), text.clone()),
        }
    }

    let mut failures = Vec::new();
    for ((query, field), theirs) in SEARCHES.iter().zip(&theirs) {
        let pattern = reads_a_pattern(query)
            .then(|| Regex::new(&format!("(?i){query}")).expect("a pattern music21 would read"));
        let found = match &pattern {
            Some(pattern) => {
                let matches = |text: &str| pattern.is_match(text);
                md.search(&MetadataQuery::Matcher(&matches), *field)
            }
            None => md.search(&MetadataQuery::Text(query), *field),
        };
        let ours = (
            found.is_some(),
            found
                .as_ref()
                .and_then(|found| found.field().map(str::to_string)),
        );
        if ours != *theirs {
            failures.push(format!(
                "{query:?} in {field:?}: music21 {theirs:?}, the crate {ours:?}"
            ));
        }
        // What matched is text music21 would hand back if it could: its own
        // search answers only whether and where.
        if let Some(found) = &found
            && let Some(field) = found.field()
        {
            assert!(
                !field.is_empty(),
                "{query:?} matched under an empty field name"
            );
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} searches differ:\n{}",
        failures.len(),
        SEARCHES.len(),
        failures.join("\n")
    );
}
