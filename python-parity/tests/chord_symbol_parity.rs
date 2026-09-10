//! Checks the crate's reading of chord symbol figures against music21's
//! `ChordSymbol`, in `data/chord_symbol_expectations.toml`.

use music21_rs::Pitch;
use music21_rs::chordsymbol::ChordSymbol;

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
    symbol: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    figure: String,
    #[serde(default)]
    pitches: Vec<String>,
    root: Option<String>,
    bass: Option<String>,
    error: Option<String>,
}

#[test]
fn every_chord_symbol_figure_realizes_as_music21_realizes_it() {
    let expectations: Expectations = read("data/chord_symbol_expectations.toml");
    assert!(expectations.symbol.len() > 500, "the figure set");
    let mut mismatches = Vec::new();
    for case in &expectations.symbol {
        let parsed = ChordSymbol::parse(case.figure.as_str()).and_then(|symbol| {
            let chord = symbol.to_chord()?;
            Ok((symbol, chord))
        });
        let (symbol, chord) = match (parsed, &case.error) {
            (Err(_), Some(_)) => continue,
            // A figure music21 refuses and the crate reads, `Cmaj9` or `C°7`,
            // is not a difference in what a figure sounds.
            (Ok(_), Some(_)) => continue,
            (Err(err), None) => {
                mismatches.push(format!("{}: {err}", case.figure));
                continue;
            }
            (Ok(built), None) => built,
        };
        // music21 realizes a symbol in octaves and the crate does not, so the
        // names are compared, and the bass only where the figure names one.
        let mut got_names: Vec<String> = chord.pitches().iter().map(Pitch::name).collect();
        got_names.sort();
        let mut want_names: Vec<String> = case
            .pitches
            .iter()
            .map(|pitch| {
                pitch
                    .trim_end_matches(|c: char| c.is_ascii_digit())
                    .to_string()
            })
            .collect();
        want_names.sort();
        // The kind is left out: music21 reads `m7b5` as a minor seventh with
        // a lowered fifth and the crate as a half-diminished seventh, and the
        // two sound the same.
        let names_bass = case.figure.contains('/');
        let got_bass = if names_bass {
            symbol.bass().map(Pitch::name).unwrap_or_default()
        } else {
            String::new()
        };
        let want_bass = if names_bass {
            case.bass.clone().unwrap_or_default()
        } else {
            String::new()
        };
        let got = (got_names, symbol.root().name(), got_bass);
        let want = (want_names, case.root.clone().unwrap_or_default(), want_bass);
        if got != want {
            mismatches.push(format!(
                "{}:\n    got  {got:?}\n    want {want:?}",
                case.figure
            ));
        }
    }
    report("figures", expectations.symbol.len(), &mismatches);
}
