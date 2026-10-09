//! The crate's `find_consecutive_scale` against music21's
//! `alpha.analysis.search.findConsecutiveScale`.
//!
//! Melodies are walked through a key a seeded generator chooses: mostly a
//! step at a time, now and then a repeat, a leap, a jump of an octave or a
//! note off the key. Each is searched in a major, a harmonic minor and a
//! melodic minor scale, under every comparison, with runs of three and
//! five degrees, repeats allowed and not, and steps of one degree and two.
//! The runs found -- which notes, and which way -- must be music21's.

use music21_rs::alpha::analysis::search::{
    PitchAttribute, RunDirection, ScaleRunOptions, find_consecutive_scale,
};
use music21_rs::scale::{Scale, ScaleType};
use music21_rs::{Note, Pitch, Stream};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

const ASKED: &str = r#"
from music21 import note, scale, stream
from music21.alpha.analysis import search

def ask(names, scaleName, tonic, required, step, attribute, repeats):
    s = stream.Stream()
    for name in names:
        s.append(note.Note(name))
    sc = getattr(scale, scaleName)(tonic)
    try:
        found = search.findConsecutiveScale(
            s, sc, degreesRequired=required, stepSize=step,
            comparisonAttribute=attribute, repeatsAllowed=repeats)
    except Exception as error:
        return [f'error: {error}']
    out = []
    for run in found:
        places = ' '.join(str(s.index(e)) for e in run['stream'])
        way = run['direction'].name if run['direction'] is not None else 'None'
        out.append(f'{places} {way}')
    return out
"#;

/// The keys melodies are walked in: each degree's name, low to high.
const KEYS: [[&str; 7]; 4] = [
    ["A", "B", "C#", "D", "E", "F#", "G#"],
    ["C", "D", "E", "F", "G", "A", "B"],
    ["E", "F#", "G", "A", "B", "C", "D#"],
    ["D", "E", "F", "G", "A", "B-", "C#"],
];

/// The scales searched, as music21 names them and as the crate does.
const SCALES: [(&str, ScaleType, &str); 4] = [
    ("MajorScale", ScaleType::Major, "A4"),
    ("MajorScale", ScaleType::Major, "C4"),
    ("HarmonicMinorScale", ScaleType::HarmonicMinor, "E4"),
    ("MelodicMinorScale", ScaleType::MelodicMinor, "D4"),
];

/// A small linear congruential generator, so every run walks the same
/// melodies.
struct Walk(u64);

impl Walk {
    fn next(&mut self, below: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % below
    }
}

fn melodies() -> Vec<Vec<String>> {
    let mut walk = Walk(2026);
    let mut out = Vec::new();
    for count in 0..80 {
        let key = &KEYS[count % KEYS.len()];
        // Degrees counted from the key's tonic in octave 3.
        let mut degree: i64 = 7 + walk.next(7) as i64;
        let mut names = Vec::new();
        for _ in 0..24 {
            let roll = walk.next(100);
            degree += match roll {
                0..30 => 1,
                30..60 => -1,
                60..70 => 0,
                70..78 => 2,
                78..86 => -2,
                86..90 => 7,
                90..94 => -7,
                _ => 0,
            };
            degree = degree.clamp(0, 20);
            let letter = key[(degree % 7) as usize];
            // A letter below the tonic's, C to B, stands in the octave above.
            let rank = |name: &str| "CDEFGAB".find(&name[..1]).expect("a letter");
            let octave = (3 + degree / 7 + i64::from(rank(letter) < rank(key[0]))).min(6);
            let mut name = letter.to_string();
            if roll >= 94 {
                // A note off the key.
                name = match name.chars().nth(1) {
                    Some('#') => name[..1].to_string(),
                    Some('-') => name[..1].to_string(),
                    _ => format!("{name}#"),
                };
            }
            names.push(format!("{name}{octave}"));
        }
        out.push(names);
    }
    out
}

fn ours(names: &[String], scale: &Scale, options: &ScaleRunOptions) -> Vec<String> {
    let mut source = Stream::new();
    for name in names {
        source.push(Note::from_name(name).expect("a note"));
    }
    match find_consecutive_scale(&source, scale, options) {
        Ok(runs) => runs
            .into_iter()
            .map(|run| {
                let places: Vec<String> = run.elements.iter().map(ToString::to_string).collect();
                let way = match run.direction {
                    Some(RunDirection::Ascending) => "ASCENDING",
                    Some(RunDirection::Descending) => "DESCENDING",
                    None => "None",
                };
                format!("{} {way}", places.join(" "))
            })
            .collect(),
        Err(error) => vec![format!(
            "error: {}",
            error.to_string().trim_start_matches("Scale error: ")
        )],
    }
}

#[test]
fn scale_runs_are_found_as_music21_finds_them() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");
    let melodies = melodies();
    let attributes = [
        (PitchAttribute::Name, "name"),
        (PitchAttribute::NameWithOctave, "nameWithOctave"),
        (PitchAttribute::PitchClass, "pitchClass"),
        (PitchAttribute::Step, "step"),
    ];
    let mut asked = Vec::new();
    for (comparison, attribute) in attributes {
        for required in [3, 5] {
            for repeats in [true, false] {
                asked.push((comparison, attribute, required, 1, repeats));
            }
        }
        asked.push((comparison, attribute, 3, 2, true));
    }

    let (failures, compared, found) = Python::attach(|py| -> PyResult<(Vec<String>, usize, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let module = PyModule::from_code(
            py,
            &std::ffi::CString::new(ASKED).expect("no nul in the helper"),
            c"scale_search.py",
            c"scale_search",
        )?;
        let ask = module.getattr("ask")?;
        let mut failures = Vec::new();
        let mut compared = 0;
        let mut found = 0;
        for names in &melodies {
            for (class, kind, tonic) in SCALES {
                let scale = Scale::new(kind, Pitch::from_name(tonic).expect("a tonic"));
                for &(comparison, attribute, required, step, repeats) in &asked {
                    let options = ScaleRunOptions {
                        degrees_required: required,
                        step_size: step,
                        comparison,
                        repeats_allowed: repeats,
                    };
                    let theirs: Vec<String> = ask
                        .call1((names.clone(), class, tonic, required, step, attribute, repeats))?
                        .extract()?;
                    let ours = ours(names, &scale, &options);
                    compared += 1;
                    found += theirs.len();
                    if ours != theirs {
                        failures.push(format!(
                            "{} in {class}({tonic}), {attribute} {required} step {step} repeats {repeats}:\n  music21    {theirs:?}\n  music21-rs {ours:?}",
                            names.join(" ")
                        ));
                    }
                }
            }
        }
        Ok((failures, compared, found))
    })
    .unwrap_or_else(|error| panic!("running music21: {error}"));

    println!("{compared} searches compared, {found} runs found by music21");
    assert!(
        failures.is_empty(),
        "{} of {compared} searches differ:\n\n{}",
        failures.len(),
        failures
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n")
    );
}
