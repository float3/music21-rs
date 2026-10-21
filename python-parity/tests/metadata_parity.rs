//! The metadata vocabulary against music21's own.
//!
//! `music21_rs::metadata::STANDARD_PROPERTIES` is music21's
//! `metadata.properties.STANDARD_PROPERTY_DESCRIPTIONS` -- each property's
//! unique name, its namespaced name and whether it names a contributor --
//! and a MusicXML writer names every `<miscellaneous-field>` out of it. This
//! reads music21's table and asks for the same rows in the same order.

use music21_rs::metadata::primitives::{Date, DatePart, DatePrimitive, Relevance};
use music21_rs::metadata::{STANDARD_PROPERTIES, ValueKind, value_kind_of};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

#[test]
fn the_metadata_vocabulary_is_music21s() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");
    let theirs = Python::attach(|py| -> PyResult<Vec<(String, String, bool)>> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let properties = py.import("music21.metadata.properties")?;
        let mut rows = Vec::new();
        for description in properties
            .getattr("STANDARD_PROPERTY_DESCRIPTIONS")?
            .try_iter()?
        {
            let description = description?;
            let name: String = description.getattr("name")?.extract()?;
            let unique: Option<String> = description.getattr("uniqueName")?.extract()?;
            let namespace: String = description.getattr("namespace")?.extract()?;
            let contributor: bool = description.getattr("isContributor")?.extract()?;
            rows.push((
                unique.unwrap_or_else(|| name.clone()),
                format!("{namespace}:{name}"),
                contributor,
            ));
        }
        Ok(rows)
    })
    .unwrap_or_else(|error| panic!("reading music21's metadata properties: {error}"));

    let ours: Vec<(String, String, bool)> = STANDARD_PROPERTIES
        .iter()
        .map(|(unique, namespaced, contributor)| {
            (unique.to_string(), namespaced.to_string(), *contributor)
        })
        .collect();
    assert_eq!(ours, theirs);
}

/// Every vocabulary name's value kind against music21's
/// `UNIQUE_NAME_TO_VALUE_TYPE`, which is what its `convertValue` reads to
/// decide what to make of whatever it was handed.
#[test]
fn the_value_kinds_are_music21s() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");
    let theirs = Python::attach(|py| -> PyResult<Vec<(String, String)>> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let properties = py.import("music21.metadata.properties")?;
        let mut rows = Vec::new();
        let table = properties.getattr("UNIQUE_NAME_TO_VALUE_TYPE")?;
        for name in table.call_method0("keys")?.try_iter()? {
            let name = name?;
            let unique: String = name.extract()?;
            let kind: String = table.get_item(&name)?.getattr("__name__")?.extract()?;
            rows.push((unique, kind));
        }
        Ok(rows)
    })
    .unwrap_or_else(|error| panic!("reading music21's value types: {error}"));

    assert_eq!(theirs.len(), 131, "music21's vocabulary has 131 names");
    for (unique, kind) in &theirs {
        let ours = value_kind_of(unique)
            .unwrap_or_else(|| panic!("{unique} is not read as a vocabulary name here"));
        let named = match ours {
            ValueKind::Text => "Text",
            ValueKind::Copyright => "Copyright",
            ValueKind::Contributor => "Contributor",
            ValueKind::Date => "DatePrimitive",
            ValueKind::Number => "int",
        };
        assert_eq!(named, kind, "the kind of value {unique} takes");
    }
}

/// Every written date music21 reads, read here and written back, against
/// music21's own `Date`: the parts, how sure each is, and the string.
#[test]
fn a_dates_reading_is_music21s() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    // Every shape of written date music21's own tests and docstrings use,
    // with a grid of symbols and part counts over them.
    let mut written: Vec<String> = Vec::new();
    for year in ["1030", "1601", "1750", "1843", "1999", "2009"] {
        for month in ["", "/1", "/03", "/12"] {
            for day in ["", "/4", "/28", "/31"] {
                if month.is_empty() && !day.is_empty() {
                    continue;
                }
                written.push(format!("{year}{month}{day}"));
            }
        }
        for symbol in ["~", "?", "x", "z"] {
            written.push(format!("{year}{symbol}/03/04"));
            written.push(format!("{year}/03{symbol}/04"));
            written.push(format!("{year}/03/{symbol}4"));
        }
        written.push(format!("{year}/12/4/4:50:32"));
        written.push(format!("{year}/--/04"));
        written.push(format!("{year}/03/--"));
    }
    written.push("1600/02/29".to_string());
    written.push("1601/02/28".to_string());
    written.push("4.43".to_string());

    type Read = Option<(String, Vec<Option<i32>>, Vec<Option<String>>)>;
    let theirs = Python::attach(|py| -> PyResult<Vec<Read>> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let primitives = py.import("music21.metadata.primitives")?;
        let mut rows: Vec<Read> = Vec::new();
        for one in &written {
            let date = primitives.getattr("Date")?.call0()?;
            match date.call_method1("loadStr", (one.as_str(),)) {
                Err(_) => rows.push(None),
                Ok(_) => {
                    let mut values = Vec::new();
                    let mut errors = Vec::new();
                    for part in ["year", "month", "day", "hour", "minute", "second"] {
                        values.push(date.getattr(part)?.extract::<Option<i32>>()?);
                        errors.push(
                            date.getattr(format!("{part}Error"))?
                                .extract::<Option<String>>()?,
                        );
                    }
                    rows.push(Some((date.str()?.to_string(), values, errors)));
                }
            }
        }
        Ok(rows)
    })
    .unwrap_or_else(|error| panic!("reading music21's dates: {error}"));

    let mut compared = 0;
    for (one, theirs) in written.iter().zip(&theirs) {
        let ours = Date::from_written(one);
        match (theirs, &ours) {
            (None, Err(_)) => continue,
            (None, Ok(date)) => panic!("music21 refuses {one}; here it reads as {date}"),
            (Some(_), Err(error)) => panic!("music21 reads {one}; here it is refused: {error}"),
            (Some((text, values, errors)), Ok(date)) => {
                assert_eq!(&date.to_string(), text, "{one} written back");
                for (at, part) in DatePart::ALL.iter().enumerate() {
                    assert_eq!(date.value(*part), values[at], "{one}'s {}", part.name());
                    assert_eq!(
                        date.certainty(*part).map(|found| found.name().to_string()),
                        errors[at],
                        "how sure {one}'s {} is",
                        part.name()
                    );
                }
                compared += 1;
            }
        }
    }
    assert!(compared > 150, "only {compared} dates compared");
}

/// The four date classes written as music21 writes them, over every
/// relevance each takes.
#[test]
fn the_date_classes_are_written_as_music21_writes_them() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let one = "1843/03/03";
    let other = "2009/12/31";
    let singles = ["certain", "approximate", "uncertain"];
    let relatives = ["prior", "after", "onorbefore", "onorafter", "before"];
    let selections = ["or", "and"];

    let theirs = Python::attach(|py| -> PyResult<Vec<String>> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let primitives = py.import("music21.metadata.primitives")?;
        let mut written = Vec::new();
        for relevance in singles {
            written.push(
                primitives
                    .getattr("DateSingle")?
                    .call1((one, relevance))?
                    .str()?
                    .to_string(),
            );
        }
        for relevance in relatives {
            written.push(
                primitives
                    .getattr("DateRelative")?
                    .call1((one, relevance))?
                    .str()?
                    .to_string(),
            );
        }
        written.push(
            primitives
                .getattr("DateBetween")?
                .call1((vec![one, other],))?
                .str()?
                .to_string(),
        );
        for relevance in selections {
            written.push(
                primitives
                    .getattr("DateSelection")?
                    .call1((vec![one, other], relevance))?
                    .str()?
                    .to_string(),
            );
        }
        Ok(written)
    })
    .unwrap_or_else(|error| panic!("reading music21's date classes: {error}"));

    let mut ours = Vec::new();
    for relevance in singles {
        ours.push(
            DatePrimitive::single(one, Relevance::named(relevance).expect("a relevance"))
                .expect("a date")
                .to_string(),
        );
    }
    for relevance in relatives {
        ours.push(
            DatePrimitive::relative(one, Relevance::named(relevance).expect("a relevance"))
                .expect("a date")
                .to_string(),
        );
    }
    ours.push(
        DatePrimitive::between([one, other])
            .expect("a span")
            .to_string(),
    );
    for relevance in selections {
        ours.push(
            DatePrimitive::selection(
                [one, other],
                Relevance::named(relevance).expect("a relevance"),
            )
            .expect("a choice")
            .to_string(),
        );
    }
    assert_eq!(ours, theirs);
}
