//! The crate's reading of a stream in seconds against music21's
//! `Stream.seconds` and `metronomeMarkBoundaries`.
//!
//! Each corpus score is read by music21 and by `from_musicxml` (which
//! `musicxml_read_parity` holds to music21's reader). The score and each of
//! its parts are asked which tempo holds over which stretch; the score and
//! each part flattened are asked how long they last. Every number is
//! compared bit for bit, `NaN` included.
//!
//! `seconds` is asked only of streams that hold their own tempi -- a score,
//! and a part flattened. music21 searches a stream's context for a tempo
//! when the stream states none from its start, and what that finds depends
//! on sort order: a measure standing *after* a tempo in its part finds it,
//! one standing at the same offset does not, because a stream sorts before
//! the marks at its offset. A crate stream knows nothing outside itself and
//! is given the tempo in force instead, so there is nothing to compare
//! there; the crate's own tests cover it.
//!
//! `STREAM_SECONDS_PARITY_SCORES`, a `;`-separated list of corpus names,
//! runs those instead of the writer test's scores, which is how the corpus
//! is swept.

use music21_rs::musicxml::from_musicxml;
use music21_rs::{Stream, StreamKind};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::SCORES;

const MUSIC21: &str = r#"
import struct
import zipfile
from music21 import stream

def bits(value):
    return str(struct.unpack('<Q', struct.pack('<d', float(value)))[0])

def source_text(name):
    from music21 import corpus
    path = corpus.getWork(name)
    if isinstance(path, (list, tuple)):
        path = path[0]
    path = str(path)
    lower = path.lower()
    if lower.endswith('.mxl'):
        with zipfile.ZipFile(path) as archive:
            names = [n for n in archive.namelist()
                     if not n.startswith('META-INF') and n.lower().endswith(('.xml', '.musicxml'))]
            data = archive.read(names[0])
    elif lower.endswith(('.xml', '.musicxml')):
        with open(path, 'rb') as handle:
            data = handle.read()
    else:
        return None
    for encoding in ('utf-8-sig', 'utf-16'):
        try:
            return data.decode(encoding)
        except UnicodeDecodeError:
            pass
    return data.decode('latin-1')

def score_of(text):
    from music21 import converter
    return converter.parse(text, format='musicxml', forceSource=True)

def holders(score):
    """The score, then each of its parts."""
    return [score] + list(score.parts)

def bpm(mm):
    """The quarter BPM a mark sounds, or `none` where it says none."""
    sounding = mm.getQuarterBPM()
    return 'none' if sounding is None else bits(sounding)

def boundaries(s):
    return [f'{bits(start)}-{bits(end)}@{bpm(mm)}'
            for start, end, mm in s.metronomeMarkBoundaries()]

def seconds_of(s):
    """The seconds, or `error` where music21 refuses to work them out."""
    try:
        return bits(s.seconds)
    except Exception:
        return 'error'

def flat_seconds(part):
    return seconds_of(part.flatten())
"#;

/// A number's bits, as the Python side spells them.
fn bits(value: f64) -> String {
    value.to_bits().to_string()
}

/// How the crate spells one stretch a tempo holds.
fn boundaries_of(stream: &Stream) -> Result<Vec<String>, String> {
    stream
        .metronome_mark_boundaries()
        .map_err(|error| error.to_string())
        .map(|boundaries| {
            boundaries
                .into_iter()
                .map(|(start, end, mark)| {
                    format!(
                        "{}-{}@{}",
                        bits(start),
                        bits(end),
                        mark.sounding_quarter_bpm()
                            .map_or_else(|| "none".to_string(), bits)
                    )
                })
                .collect()
        })
}

#[test]
fn the_crate_reads_a_stream_in_seconds_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"stream_seconds_parity_music21.py",
            c"stream_seconds_parity_music21",
        )?;

        let chosen: Vec<String> = match std::env::var("STREAM_SECONDS_PARITY_SCORES") {
            Ok(list) => list
                .split(';')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect(),
            Err(_) => SCORES
                .iter()
                .map(|(name, _)| name.to_string())
                .filter(|name| !name.starts_with("built:"))
                .collect(),
        };

        let mut failures = Vec::new();
        let mut compared = 0;
        for name in &chosen {
            let Some(text): Option<String> =
                music21.getattr("source_text")?.call1((name,))?.extract()?
            else {
                continue;
            };
            let ours = match from_musicxml(&text) {
                Ok(score) => score,
                Err(error) => {
                    failures.push(format!("{name}: the crate could not read it: {error}"));
                    continue;
                }
            };
            if ours.kind() != StreamKind::Score {
                continue;
            }
            let theirs = music21.getattr("score_of")?.call1((&text,))?;
            let their_holders: Vec<Bound<'_, PyAny>> =
                music21.getattr("holders")?.call1((&theirs,))?.extract()?;
            let our_parts = ours.parts();
            if their_holders.len() != our_parts.len() + 1 {
                failures.push(format!(
                    "{name}: {} parts to music21, {} to the crate",
                    their_holders.len() - 1,
                    our_parts.len()
                ));
                continue;
            }

            let mut our_holders: Vec<&Stream> = vec![&ours];
            our_holders.extend(our_parts.iter().copied());
            for (at, (our_holder, their_holder)) in
                our_holders.iter().zip(&their_holders).enumerate()
            {
                let where_ = if at == 0 {
                    "the score".to_string()
                } else {
                    format!("part {}", at - 1)
                };
                let theirs: Result<Vec<String>, String> =
                    match music21.getattr("boundaries")?.call1((their_holder,)) {
                        Ok(value) => value
                            .extract::<Vec<String>>()
                            .map_err(|error| error.to_string()),
                        Err(error) => Err(error.to_string()),
                    };
                let ours = boundaries_of(our_holder);
                compared += 1;
                match (theirs, ours) {
                    (Ok(theirs), Ok(ours)) if theirs == ours => {}
                    (Err(_), Err(_)) => {}
                    (theirs, ours) => failures.push(format!(
                        "{name} {where_}: the stretches each tempo holds differ\n  music21: {}\n  crate  : {}",
                        summarize(&theirs),
                        summarize(&ours)
                    )),
                }
            }

            // How long it lasts, asked of the streams that hold their own
            // tempi: the score, and each part flattened.
            let their_seconds: String = music21
                .getattr("seconds_of")?
                .call1((&theirs,))?
                .extract()?;
            let our_seconds = ours
                .seconds(None)
                .map_or_else(|_| "error".to_string(), bits);
            compared += 1;
            if their_seconds != our_seconds {
                failures.push(format!(
                    "{name} the score: {their_seconds} seconds to music21, {our_seconds} to the crate"
                ));
            }
            for (at, (our_part, their_part)) in
                our_parts.iter().zip(their_holders.iter().skip(1)).enumerate()
            {
                let theirs: String = music21
                    .getattr("flat_seconds")?
                    .call1((their_part,))?
                    .extract()?;
                let ours = our_part
                    .flatten()
                    .seconds(None)
                    .map_or_else(|_| "error".to_string(), bits);
                compared += 1;
                if theirs != ours {
                    failures.push(format!(
                        "{name} part {at} flattened: {theirs} seconds to music21, {ours} to the crate"
                    ));
                }
            }
        }
        Ok((failures, compared))
    })
    .unwrap_or_else(|error| panic!("asking music21 for a stream in seconds: {error}"));

    let shown: Vec<&str> = failures.iter().take(10).map(String::as_str).collect();
    assert!(
        failures.is_empty(),
        "{} of {compared} answers differ, first {}:\n{}",
        failures.len(),
        shown.len(),
        shown.join("\n")
    );
    assert!(compared > 0, "nothing was compared");
    println!("{compared} answers compared");
}

/// A list of numbers, or the error in its place, short enough to read.
fn summarize(answer: &Result<Vec<String>, String>) -> String {
    match answer {
        Ok(items) if items.len() > 6 => {
            format!("{} items starting {:?}", items.len(), &items[..6])
        }
        Ok(items) => format!("{items:?}"),
        Err(error) => format!("refused: {error}"),
    }
}
