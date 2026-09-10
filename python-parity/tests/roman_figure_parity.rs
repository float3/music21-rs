//! Checks the crate's realization of roman numeral figures against
//! music21's `RomanNumeral`, in `data/roman_figure_expectations.toml`.

use music21_rs::{Key, Pitch, RomanNumeral};

use std::path::Path;

use serde::Deserialize;

fn read<T: serde::de::DeserializeOwned>(fixture: &str) -> T {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|root| root.join(fixture))
        .unwrap_or_default();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("{} is unreadable: {err}", path.display()));
    toml::from_str(&text).unwrap_or_else(|err| panic!("{fixture} does not parse: {err}"))
}

fn report(what: &str, total: usize, mismatches: &[String]) {
    assert!(
        mismatches.is_empty(),
        "{} of {total} {what} differ from music21:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}

#[derive(Debug, Deserialize)]
struct Expectations {
    numeral: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    figure: String,
    key: String,
    #[serde(default)]
    pitches: Vec<String>,
    roman_numeral: Option<String>,
    degree: Option<u8>,
    inversion: Option<u8>,
    error: Option<String>,
}

#[test]
fn every_roman_figure_realizes_as_music21_realizes_it() {
    let expectations: Expectations = read("data/roman_figure_expectations.toml");
    assert!(expectations.numeral.len() > 600, "the figure set");
    let mut mismatches = Vec::new();
    for case in &expectations.numeral {
        let label = format!("{} in {}", case.figure, case.key);
        let key = Key::from_tonic(&case.key).expect("a key");
        let built = RomanNumeral::new(case.figure.as_str(), key).and_then(|numeral| {
            let chord = numeral.to_chord()?;
            Ok((numeral, chord))
        });
        let (numeral, chord) = match (built, &case.error) {
            (Err(_), Some(_)) => continue,
            (Ok(_), Some(error)) => {
                mismatches.push(format!("{label}: built, but music21 raises {error}"));
                continue;
            }
            (Err(err), None) => {
                mismatches.push(format!("{label}: {err}"));
                continue;
            }
            (Ok(built), None) => built,
        };
        let got = (
            chord
                .pitches()
                .iter()
                .map(Pitch::name_with_octave)
                .collect::<Vec<_>>(),
            numeral.roman_numeral(),
            numeral.scale_degree_with_alteration().0,
            Some(numeral.inversion()),
        );
        let want = (
            case.pitches.clone(),
            case.roman_numeral.clone().unwrap_or_default(),
            case.degree.unwrap_or_default(),
            case.inversion,
        );
        if got != want {
            mismatches.push(format!("{label}:\n    got  {got:?}\n    want {want:?}"));
        }
    }
    report("numerals", expectations.numeral.len(), &mismatches);
}
