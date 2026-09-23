//! Checks the instruments transcribed from music21's `instrument` module.
//!
//! Every class's starting values, the General MIDI program table and the name
//! tables are typed into `src/instrument/tables.rs`; this reads them back
//! through the crate's own API and compares each with what music21 answered,
//! recorded in `data/instrument_expectations.toml` by
//! `cargo run --release -p xtask --features python -- regenerate-fixtures`.
//! `fromString` is checked behaviourally, on every name in every table and on
//! the strings its own documentation reads.

use music21_rs::instrument::{Instrument, SearchLanguage};
use serde::Deserialize;

use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct Expectations {
    instrument: Vec<Kind>,
    midi_program: BTreeMap<String, String>,
    ensemble_names: Vec<String>,
    lookup: Vec<Lookup>,
}

#[derive(Debug, Deserialize)]
struct Kind {
    class: String,
    parents: Vec<String>,
    name: Option<String>,
    abbreviation: Option<String>,
    sound: Option<String>,
    best_name: Option<String>,
    midi_program: Option<u8>,
    midi_channel: Option<u8>,
    percussion_pitch: Option<u8>,
    lowest: Option<String>,
    highest: Option<String>,
    transposition: Option<String>,
    percussion_map: bool,
    all_names: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct Lookup {
    text: String,
    language: String,
    class: Option<String>,
    transposition: Option<String>,
    error: Option<String>,
}

fn expectations() -> Expectations {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("python-parity has a parent directory")
        .join("data/instrument_expectations.toml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("{} is unreadable: {err}", path.display()));
    toml::from_str(&text).expect("instrument expectations parse")
}

#[test]
fn every_kind_starts_out_as_music21_s_class_does() {
    let expected = expectations();
    let ours: Vec<&str> = Instrument::kinds().collect();
    let theirs: Vec<&str> = expected
        .instrument
        .iter()
        .map(|kind| kind.class.as_str())
        .collect();
    assert_eq!(ours, theirs, "the kinds, in order");

    for kind in &expected.instrument {
        let made = Instrument::of_kind(&kind.class).expect("a kind the crate has");
        let label = &kind.class;
        let families: Vec<&str> = made.families().to_vec();
        assert_eq!(families, kind.parents, "{label}: families");
        assert_eq!(made.name(), kind.name.as_deref(), "{label}: name");
        assert_eq!(
            made.abbreviation(),
            kind.abbreviation.as_deref(),
            "{label}: abbreviation"
        );
        assert_eq!(made.sound(), kind.sound.as_deref(), "{label}: sound");
        assert_eq!(
            made.best_name(),
            kind.best_name.as_deref(),
            "{label}: best name"
        );
        assert_eq!(
            made.midi_program(),
            kind.midi_program,
            "{label}: MIDI program"
        );
        assert_eq!(
            made.midi_channel(),
            kind.midi_channel,
            "{label}: MIDI channel"
        );
        assert_eq!(
            made.percussion_pitch(),
            kind.percussion_pitch,
            "{label}: drum"
        );
        assert_eq!(
            made.in_percussion_map(),
            kind.percussion_map,
            "{label}: percussion map"
        );
        assert_eq!(
            made.lowest().map(|pitch| pitch.name_with_octave()),
            kind.lowest,
            "{label}: lowest"
        );
        assert_eq!(
            made.highest().map(|pitch| pitch.name_with_octave()),
            kind.highest,
            "{label}: highest"
        );
        assert_eq!(
            made.transposition()
                .map(|interval| interval.directed_name()),
            kind.transposition,
            "{label}: transposition"
        );
    }
}

#[test]
fn every_midi_program_makes_music21_s_instrument() {
    let expected = expectations();
    assert_eq!(expected.midi_program.len(), 128);
    for (program, class) in &expected.midi_program {
        let program: u8 = program.parse().expect("a program number");
        let made = Instrument::from_midi_program(program).expect("a General MIDI program");
        assert_eq!(made.kind(), class, "program {program}");
        assert_eq!(made.midi_program(), Some(program), "program {program}");
    }
    assert!(Instrument::from_midi_program(128).is_err());
}

#[test]
fn every_name_is_found_as_fromstring_finds_it() {
    let expected = expectations();
    assert!(
        expected.lookup.len() > 1000,
        "every name in every table is asked"
    );
    let mut wrong = Vec::new();
    for lookup in &expected.lookup {
        let language = SearchLanguage::from_name(&lookup.language).expect("a language");
        let ours = Instrument::from_name(&lookup.text, language);
        let (class, transposition) = match &ours {
            Ok(made) => (
                Some(made.kind().to_string()),
                made.transposition()
                    .map(|interval| interval.directed_name()),
            ),
            Err(_) => (None, None),
        };
        let agrees = match &lookup.error {
            Some(_) => ours.is_err(),
            None => class == lookup.class && transposition == lookup.transposition,
        };
        if !agrees {
            wrong.push(format!(
                "{:?} in {}: music21 {:?} {:?} {:?}, crate {:?} {:?}",
                lookup.text,
                lookup.language,
                lookup.class,
                lookup.transposition,
                lookup.error,
                class,
                transposition
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} lookups differ:\n  {}",
        wrong.len(),
        wrong.join("\n  ")
    );
}

#[test]
fn every_kind_s_names_are_the_ones_music21_gives_it() {
    let expected = expectations();
    for kind in &expected.instrument {
        let made = Instrument::of_kind(&kind.class).expect("a kind the crate has");
        for (language, names) in made.all_names(SearchLanguage::All) {
            let mut ours = names.clone();
            ours.sort_unstable();
            let mut music21: Vec<&str> = kind.all_names[language.as_str()]
                .iter()
                .map(String::as_str)
                .collect();
            music21.sort_unstable();
            assert_eq!(ours, music21, "{} in {}", kind.class, language.as_str());
        }
    }
}

#[test]
fn ensembles_are_named_as_music21_names_them() {
    let expected = expectations();
    for (players, name) in expected.ensemble_names.iter().enumerate() {
        assert_eq!(
            music21_rs::instrument::ensemble_name_by_size(players),
            Some(name.as_str()),
            "{players} players"
        );
    }
    assert_eq!(
        music21_rs::instrument::ensemble_name_by_size(expected.ensemble_names.len()),
        None
    );
}
