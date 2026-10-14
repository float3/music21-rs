//! The crate's `Stream::lyrics` against music21's `Stream.lyrics`.
//!
//! Each corpus score is read by music21 and by `from_musicxml` (which
//! `musicxml_read_parity` holds to music21's reader). The score and each of
//! its parts are asked for their lyrics under all eight settings of
//! `ignoreBarlines`, `recurse` and `skipTies`, and every line is compared
//! place by place: the syllable sung there, the note that sings nothing
//! under that number, or the list a nested stream answered.
//!
//! A lyric nobody has sung anything to and one sung to an empty syllable
//! are spelled alike here -- music21 leaves the first's text `None` and the
//! crate's `text` answers the empty string for both -- which no corpus
//! score tells apart.
//!
//! `STREAM_LYRICS_PARITY_SCORES`, a `;`-separated list of corpus names,
//! runs those instead of the writer test's scores, which is how the corpus
//! is swept.

use music21_rs::musicxml::from_musicxml;
use music21_rs::stream::{LyricsOptions, SungLyric};
use music21_rs::{Stream, StreamKind};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::SCORES;

const MUSIC21: &str = r#"
import zipfile

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

def holders(text):
    from music21 import converter
    score = converter.parse(text, format='musicxml', forceSource=True)
    return [score] + list(score.parts)

def spell(line):
    out = []
    for place in line:
        if place is None:
            out.append('-')
        elif isinstance(place, list):
            out.append('[' + ' '.join(spell(place)) + ']')
        else:
            out.append('' if place.text is None else place.text)
    return out

def lyrics(s, ignoreBarlines, recurse, skipTies):
    found = s.lyrics(ignoreBarlines=ignoreBarlines, recurse=recurse, skipTies=skipTies)
    return [f'{number}: {" ".join(spell(line))}' for number, line in sorted(found.items())]
"#;

/// How the crate spells one place in a line, as the Python side spells it.
fn spell(line: &[SungLyric]) -> Vec<String> {
    line.iter()
        .map(|place| match place {
            SungLyric::Unsung => "-".to_string(),
            SungLyric::Nested(inner) => format!("[{}]", spell(inner).join(" ")),
            SungLyric::Lyric(lyric) => lyric.text(),
        })
        .collect()
}

/// The crate's answer for one stream under one setting.
fn lyrics_of(stream: &Stream, options: &LyricsOptions) -> Vec<String> {
    stream
        .lyrics(options)
        .into_iter()
        .map(|(number, line)| format!("{number}: {}", spell(&line).join(" ")))
        .collect()
}

#[test]
fn the_crate_gathers_lyrics_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"stream_lyrics_parity_music21.py",
            c"stream_lyrics_parity_music21",
        )?;

        let chosen: Vec<String> = match std::env::var("STREAM_LYRICS_PARITY_SCORES") {
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
            let their_holders: Vec<Bound<'_, PyAny>> =
                music21.getattr("holders")?.call1((&text,))?.extract()?;
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
                for flags in 0..8u8 {
                    let options = LyricsOptions {
                        ignore_barlines: flags & 1 != 0,
                        recurse: flags & 2 != 0,
                        skip_ties: flags & 4 != 0,
                    };
                    let theirs: Vec<String> = music21
                        .getattr("lyrics")?
                        .call1((
                            their_holder,
                            options.ignore_barlines,
                            options.recurse,
                            options.skip_ties,
                        ))?
                        .extract()?;
                    let ours = lyrics_of(our_holder, &options);
                    compared += 1;
                    if ours != theirs {
                        failures.push(format!(
                            "{name} {where_} {options:?}:\n  music21: {theirs:?}\n  crate  : {ours:?}"
                        ));
                    }
                }
            }
        }
        Ok((failures, compared))
    })
    .unwrap_or_else(|error| panic!("asking music21 for a stream's lyrics: {error}"));

    let shown: Vec<&str> = failures.iter().take(6).map(String::as_str).collect();
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
