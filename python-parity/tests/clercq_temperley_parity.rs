//! The crate's reader of the rock corpus's analyses against music21's,
//! song by song.
//!
//! Each song is read twice, by music21's `romanText.clercqTemperley` and by
//! `music21_rs::romantext::clercq_temperley::from_clercq_temperley`, and
//! the two parts are compared as an outline -- every measure and every
//! chord in it, with its offset, its length, its pitches, its ties and what
//! is written under it -- and then as the MusicXML each side's writer makes
//! of its own. The crate's writer is held to music21's byte for byte by
//! `musicxml_parity`, so the two documents are the same text only if the
//! two readers made the same part.
//!
//! The songs are the six music21 carries in the module itself, which are
//! what its own tests and docstrings are written on; eleven written for
//! this test, each for something those six do not say -- rests, a measure
//! repeated, a meter and a key changing part-way, a dot repeating the chord
//! before it, the lab's own spellings of a diminished, half-diminished and
//! augmented chord, a rule expanding a rule, chords with no barline after
//! them, and measures whose chords do not divide the bar evenly; and 200
//! made up from a seed out of the whole grammar. A song neither side can
//! read -- a dot with no chord before it in the rule keeping that state,
//! which is what the made-up songs find -- counts as an agreement; a song
//! only one side refuses is a difference.
//!
//! music21 is music21 here, with nothing of the crate installed over it.

use music21_rs::musicxml::{ExportOptions, to_musicxml};
use music21_rs::romantext::clercq_temperley::from_clercq_temperley;
use music21_rs::stream::{Stream, StreamKind};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::{OUTLINE, STRIP_LAYOUT, first_difference, normalize_ids, outline};

/// The songs music21 carries in the module, which is where its own tests
/// take theirs from.
const CARRIED: &[&str] = &[
    "BlitzkriegBopCT",
    "RockClockCT",
    "textString",
    "changeIsGonnaCome",
    "exampleClercqTemperley",
    "RingFireCT",
];

/// Songs written for the test, each for something the six do not say.
const WRITTEN: &[(&str, &str)] = &[
    (
        "a dot, an empty measure and a meter part-way",
        "% Dots\nVr: [4/4] bVII | IV | | [2/4] |\nS: [C] $Vr\n",
    ),
    (
        "the lab's own spellings",
        "% Spellings\nVr: iix | viih7 | iia | I |\nS: [F] $Vr\n",
    ),
    (
        "chords that do not divide the bar evenly",
        "% Thirds\nVr: I IV V | I V | [3/4] I IV | I |\nS: [D] $Vr\n",
    ),
    (
        "a rest, a measure repeated and a rule expanding a rule",
        "% Rests\nBP: I | R |*2 IV |\nVr: $BP*2 V |*3\nS: [A] $Vr $BP\n",
    ),
    (
        "a key changing part-way and the same chord across a barline",
        "% Keys\nVr: I | I | [G] I | I vi | [e] i |\nS: [C] $Vr $Vr\n",
    ),
    (
        "an irregular meter and a song with no key or meter of its own",
        "% Irregular\nVr: [0] I IV | V | I |\nS: $Vr\n",
    ),
    (
        "a chord struck twice in a measure and a rest beside chords",
        "% Twice\nVr: I I | I R | R I | I . V |\nS: [C] $Vr\n",
    ),
    (
        "a meter changing inside a rule another expands",
        "% Inner\nBP: [3/4] I | IV |\nVr: $BP I | [4/4] V |\nS: [F] $Vr $BP $Vr\n",
    ),
    (
        "chords with no barline after them, which music21 warns about",
        "% Loose\nVr: I | IV\nS: [C] $Vr I |\n",
    ),
    (
        "a song saying its meter before its key",
        "% Ordered\nVr: I | V | I IV V |\nS: [4/4] [G] $Vr\n",
    ),
    (
        "three chords to a compound bar",
        "% Compound\nVr: [12/8] I IV V | I | [6/8] I V |\nS: [E] $Vr\n",
    ),
];

#[test]
fn the_crate_reads_the_rock_corpus_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared, outline_only, refused) =
        Python::attach(|py| -> PyResult<(Vec<String>, usize, usize, usize)> {
            init_py(py)?;
            add_dependency_venv(py, &root)?;
            let _ = py.import("music21")?;
            let helpers = PyModule::from_code(
                py,
                &std::ffi::CString::new(STRIP_LAYOUT).expect("no nul in the helper"),
                c"musicxml_parity_helpers.py",
                c"musicxml_parity_helpers",
            )?;
            let outlines = PyModule::from_code(
                py,
                &std::ffi::CString::new(OUTLINE).expect("no nul in the helper"),
                c"outline_helpers.py",
                c"outline_helpers",
            )?;
            let music21 = PyModule::from_code(
                py,
                &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
                c"clercq_temperley_parity_music21.py",
                c"clercq_temperley_parity_music21",
            )?;
            let exporter = py.import("music21.musicxml.m21ToXml")?;
            let today = musicxml_common::pin_encoding_date(py)?;
            let version: String = py.import("music21")?.getattr("__version__")?.extract()?;
            let software = format!("music21 v.{version}");

            let mut subjects: Vec<(String, String)> = Vec::new();
            for name in CARRIED {
                let text: String = music21.getattr("source")?.call1((name,))?.extract()?;
                subjects.push(((*name).to_string(), text));
            }
            for (name, text) in WRITTEN {
                subjects.push(((*name).to_string(), (*text).to_string()));
            }
            for number in 0..GENERATED {
                subjects.push((
                    format!("generated {number}"),
                    generated_song(number as u64 + 1),
                ));
            }

            let mut failures = Vec::new();
            let mut compared = 0;
            let mut outline_only = 0;
            let mut refused = 0;
            for (name, text) in &subjects {
                let read = music21.getattr("read")?.call1((text,));
                let ours = from_clercq_temperley(text).map(|part| {
                    let mut score = Stream::with_kind(StreamKind::Score);
                    score.insert(0.0, part);
                    score
                });
                let (read, ours) = match (read, ours) {
                    (Ok(read), Ok(ours)) => (read, ours),
                    // A song neither side can read -- a dot with no chord
                    // before it is the one the made-up songs find -- is an
                    // agreement rather than a difference.
                    (Err(_), Err(_)) => {
                        refused += 1;
                        continue;
                    }
                    (Ok(_), Err(error)) => {
                        failures.push(format!("{name}: the crate refused it: {error}"));
                        continue;
                    }
                    (Err(error), Ok(_)) => {
                        failures.push(format!("{name}: music21 could not read it: {error}"));
                        continue;
                    }
                };
                compared += 1;

                let their_outline: String =
                    outlines.getattr("outline")?.call1((&read,))?.extract()?;
                let our_outline = outline(&ours);
                if let Some(difference) = first_difference(&our_outline, &their_outline) {
                    write_both(&root, name, "txt", &their_outline, &our_outline);
                    failures.push(format!("{name}: outline {difference}"));
                    continue;
                }

                let theirs = helpers
                    .getattr("strip_layout")?
                    .call1((read,))
                    .and_then(|score| {
                        let general = exporter
                            .getattr("GeneralObjectExporter")?
                            .call1((&score,))?;
                        general.setattr("makeNotation", false)?;
                        general
                            .call_method0("parse")?
                            .call_method1("decode", ("utf-8",))?
                            .extract::<String>()
                    });
                // What music21 cannot write as it stands has been compared
                // by its outline.
                let Ok(theirs) = theirs else {
                    outline_only += 1;
                    continue;
                };
                let options = ExportOptions {
                    encoding_date: Some(today.clone()),
                    software: software.clone(),
                    ..ExportOptions::default()
                };
                let ours = match to_musicxml(&ours, &options) {
                    Ok(text) => text,
                    Err(error) => {
                        failures.push(format!("{name}: the crate could not write it: {error}"));
                        continue;
                    }
                };
                let (theirs, ours) = (normalize_ids(theirs.trim_end()), normalize_ids(&ours));
                if let Some(difference) = first_difference(&ours, &theirs) {
                    write_both(&root, name, "xml", &theirs, &ours);
                    failures.push(format!("{name}: {difference}"));
                }
            }
            Ok((failures, compared, outline_only, refused))
        })
        .expect("the comparison runs");

    println!(
        "{compared} compared, {outline_only} of them by outline alone; {refused} refused by both"
    );
    assert!(
        failures.is_empty(),
        "the two readers differ:\n  {}",
        failures.join("\n  ")
    );
}

/// music21's side: the songs it carries, and a song read into a score of
/// one part, which is what its exporter will write.
const MUSIC21: &str = r#"
from music21 import stream
from music21.romanText import clercqTemperley as ct

def source(name):
    return getattr(ct, name)

def read(text):
    part = ct.CTSong(text).toPart()
    score = stream.Score()
    score.insert(0, part)
    return score
"#;

/// How many songs are made up for the test besides the ones written out.
const GENERATED: usize = 200;

/// A song made up from a seed: two to four rules of chords, rests, dots,
/// brackets and repeats, and an `S:` rule playing them through. A rule only
/// ever expands one written before it, so no song can expand itself for
/// ever, and no rule opens with a dot, which both sides refuse.
fn generated_song(seed: u64) -> String {
    let mut state = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    let mut next = move |bound: usize| -> usize {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) as usize) % bound
    };
    const CHORDS: &[&str] = &[
        "I", "ii", "iii", "IV", "V", "vi", "viio", "V7", "IVd7", "bVII", "V/vi", "ii65", "I64",
        "iix", "viih7", "iia", "bVId7", "V43/ii",
    ];
    const METERS: &[&str] = &["[4/4]", "[3/4]", "[2/4]", "[6/8]", "[12/8]", "[0]"];
    const KEYS: &[&str] = &["[C]", "[G]", "[e]", "[Bb]", "[f#]", "[A]"];
    let names = ["BP", "In", "Vr", "Br", "Ch", "Co"];
    let rules = 2 + next(3);
    let mut text = format!("% Generated {seed}\n");
    for rule in 0..rules {
        let mut line = format!("{}:", names[rule]);
        for measure in 0..1 + next(4) {
            // A rule may open by saying a meter or a key.
            if measure == 0 && next(4) == 0 {
                line.push_str(&format!(" {}", METERS[next(METERS.len())]));
            }
            if measure == 0 && next(5) == 0 {
                line.push_str(&format!(" {}", KEYS[next(KEYS.len())]));
            }
            // An earlier rule, played through once or twice.
            if rule > 0 && next(4) == 0 {
                let named = names[next(rule)];
                let repeats = if next(3) == 0 { "*2" } else { "" };
                line.push_str(&format!(" ${named}{repeats}"));
                continue;
            }
            for chord in 0..1 + next(3) {
                let atom = match next(10) {
                    0 => "R",
                    1 if measure > 0 || chord > 0 => ".",
                    _ => CHORDS[next(CHORDS.len())],
                };
                line.push_str(&format!(" {atom}"));
            }
            line.push_str(match next(6) {
                0 => " |*2",
                1 => " |*3",
                _ => " |",
            });
        }
        text.push_str(&line);
        text.push('\n');
    }
    let mut song = format!("S: {}", KEYS[next(KEYS.len())]);
    for name in names.iter().take(rules) {
        song.push_str(&format!(" ${name}"));
    }
    text.push_str(&song);
    text.push('\n');
    text
}

/// Both sides of a difference, left where a reader can look at them.
fn write_both(root: &std::path::Path, name: &str, kind: &str, theirs: &str, ours: &str) {
    let directory = root.join("target").join("clercq-temperley-parity");
    if std::fs::create_dir_all(&directory).is_ok() {
        let index: String = name
            .chars()
            .map(|letter| {
                if letter.is_alphanumeric() {
                    letter
                } else {
                    '_'
                }
            })
            .collect();
        let _ = std::fs::write(directory.join(format!("{index}.music21.{kind}")), theirs);
        let _ = std::fs::write(directory.join(format!("{index}.music21-rs.{kind}")), ours);
    }
}
