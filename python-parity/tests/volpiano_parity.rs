//! The crate's Volpiano reader and writer against music21's `volpiano`.
//!
//! 1. Reading. Each string is read by music21's `toPart` and by
//!    `from_volpiano`, and the two parts must be the same: every measure,
//!    its number, offset and closing barline, every element in it with its
//!    offset, and every neume by the notes it joins. Each part is then
//!    written back by its own side's writer, music21's `fromStream` and
//!    `to_volpiano`, and the two strings must be the same; and where
//!    music21's MusicXML exporter will write the part, `to_musicxml` must
//!    write the same document. The strings are music21's own examples, a few
//!    written for the test, and a few hundred generated from every token.
//! 2. Writing. Corpus scores are read by music21 and by the crate's MusicXML
//!    reader, which `musicxml_read_parity` holds to music21's, and written
//!    by `fromStream` and `to_volpiano`; the two strings must be the same.
//!
//! music21's `fromStream` finds a note's neumes through the note, and
//! `toPart` makes a neume of every two notes before dropping all but the last
//! of a run, so what music21 writes back depends on whether Python has yet
//! collected the neumes it dropped. The test collects them first, which
//! leaves the neumes the part holds.
//!
//! music21 is music21 here, with nothing of the crate installed over it.
//! `VOLPIANO_PARITY_SCORES`, a `;`-separated list of corpus names, runs
//! those scores in the second half instead of the reader test's.

use music21_rs::musicxml::{ExportOptions, from_musicxml, to_musicxml};
use music21_rs::volpiano::{from_volpiano, to_volpiano};
use music21_rs::{SpannerKind, Stream, StreamElement, StreamKind};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::{SCORES, STRIP_LAYOUT, first_difference, normalize_ids};

/// Strings written for the test, beside music21's own examples.
const STRINGS: &[&str] = &[
    // music21's examples.
    "1---c--d---f--d---ed--c--d---f---g--h--j---hgf--g--h---",
    "1---c--2---c",
    "1---e--we--e--We--e",
    "1---e-7-e-77-e-777-e-3-e-4",
    "1---e-E-",
    "1--c--d---f--d---ed--c--d---f---g--h--j---hgf--g--h---",
    "1---e-E--",
    // Breaks and neumes after the first barline, which music21 puts in the
    // first measure.
    "1---e-3-ef-g-7-e-4-gh--j-77",
    // Neumes of three, four and five notes.
    "1---abc-defg-hjklm--",
    // Every flat and natural, in both clefs.
    "1---y-b-w-e-i-j-x-l-z-n-Y-b-W-e-I-j-X-l-Z-n-2---y-b-w-e-i-j-x-l-z-n-",
    // The lowest and highest notes, liquescent too.
    "1---9-s-)-S-2---9-s-)-S-",
    // A clef and a barline inside a neume, tokens Volpiano has no meaning
    // for, an empty measure and a final double barline.
    "1---ab2cd-ef3gh-]-(-k-33-4",
    // Nothing at all, and only a clef.
    "",
    "1",
    // A break at the very end, and breaks with nothing after them.
    "1---c-7",
    "1---c-777",
];

/// Characters the generated strings are made of: every token Volpiano has,
/// hyphens weighted to be common.
const ALPHABET: &str = "1234-----7abcdefghjklmnopqrs9ABCDEFGHJKLMNS)wxiyzWXIYZ";

/// A few hundred strings from `ALPHABET`, the same on every run. None holds
/// four `7`s in a row, which music21 cannot read.
fn generated() -> Vec<String> {
    let alphabet: Vec<char> = ALPHABET.chars().collect();
    let mut state: u64 = 0x5eed;
    let mut next = |below: usize| {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) as usize) % below
    };
    let mut out = Vec::new();
    while out.len() < 300 {
        let length = 4 + next(60);
        let mut text: String = (0..length)
            .map(|_| alphabet[next(alphabet.len())])
            .collect();
        if !text.starts_with(['1', '2']) {
            text.insert_str(0, "1---");
        }
        if !text.contains("7777") {
            out.push(text);
        }
    }
    out
}

/// What music21 makes of a string, in the outline's terms.
const MUSIC21: &str = r#"
import contextlib
import gc
import io
from music21 import volpiano
from music21.musicxml import m21ToXml

def number(value):
    return f'{float(value):.5f}'

def outline(part):
    out = []
    notes = list(part.recurse().notes)
    for measure in part.getElementsByClass('Measure'):
        closing = measure.rightBarline
        out.append(f'Measure {measure.number} {number(measure.offset)} '
                   f'{closing.type if closing is not None else None}')
        for e in measure:
            classes = e.classes
            if 'Barline' in classes or 'Neume' in classes:
                continue
            at = number(e.getOffsetBySite(measure))
            if 'Note' in classes:
                out.append(f'  {at} Note {e.nameWithOctave} {e.notehead} '
                           f'{e.stemDirection} {number(e.quarterLength)}')
            elif 'Clef' in classes:
                out.append(f'  {at} Clef {e.sign}{e.line}')
            else:
                out.append(f'  {at} {classes[0]}')
    for neume in part.recurse().getElementsByClass('Neume'):
        places = [next(i for i, n in enumerate(notes) if n is spanned)
                  for spanned in neume.getSpannedElements()]
        out.append('Neume ' + ' '.join(str(place) for place in places))
    return '\n'.join(out)

def read(text):
    part = volpiano.toPart(text)
    # toPart makes a neume of every two notes and keeps the last of each
    # run; fromStream finds a note's neumes through the note, so one toPart
    # dropped counts until Python collects it. Collected, it is gone.
    gc.collect()
    # fromStream warns of every accidental it cannot write.
    with contextlib.redirect_stderr(io.StringIO()):
        return outline(part), volpiano.fromStream(part)

def musicxml(text, strip_layout):
    from music21 import stream
    score = stream.Score()
    score.insert(0, volpiano.toPart(text))
    exporter = m21ToXml.GeneralObjectExporter(strip_layout(score))
    exporter.makeNotation = False
    return exporter.parse().decode('utf-8')

def written(name):
    from music21 import corpus
    score = corpus.parse(name, forceSource=True)
    # fromStream warns of every note it cannot write.
    with contextlib.redirect_stderr(io.StringIO()):
        return volpiano.fromStream(score)
"#;

/// The text of a corpus score's MusicXML file, a compressed one unpacked;
/// nothing for another format.
const SOURCE_TEXT: &str = r#"
import zipfile
from music21 import corpus

def source_text(name):
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
"#;

fn number(value: f64) -> String {
    format!("{value:.5}")
}

/// A pitch's name and octave as music21 writes them, a flat as `-`.
fn music21_name(pitch: &music21_rs::Pitch) -> String {
    let name = pitch.name_with_octave();
    let mut letters = name.chars();
    let Some(step) = letters.next() else {
        return String::new();
    };
    format!("{step}{}", letters.as_str().replace('b', "-"))
}

/// The part as `MUSIC21`'s `outline` writes of music21's.
fn outline(part: &Stream) -> String {
    let mut out = Vec::new();
    for event in part.events() {
        let Some(measure) = event
            .element()
            .as_stream()
            .filter(|inner| inner.kind() == StreamKind::Measure)
        else {
            continue;
        };
        out.push(format!(
            "Measure {} {} {}",
            measure.number(),
            number(event.offset()),
            measure
                .right_barline()
                .map_or("None", |barline| barline.bar_type().as_str())
        ));
        for held in measure.events() {
            let at = number(held.offset());
            out.push(match held.element() {
                StreamElement::Note(note) => format!(
                    "  {at} Note {} {} {} {}",
                    music21_name(note.pitch()),
                    note.notehead().as_str(),
                    note.stem_direction().as_str(),
                    number(held.element().quarter_length())
                ),
                StreamElement::Clef(clef) => format!(
                    "  {at} Clef {}{}",
                    clef.sign().unwrap_or("None"),
                    clef.line()
                        .map_or("None".to_string(), |line| line.to_string())
                ),
                StreamElement::Break(kind) => format!("  {at} {}", kind.class_name()),
                other => format!("  {at} {other:?}"),
            });
        }
    }
    // Each note's ordinal among the part's notes, by its place in the leaves.
    let leaves = part.leaves();
    let ordinal = |place: usize| {
        leaves[..place]
            .iter()
            .filter(|(_, element)| matches!(element, StreamElement::Note(_)))
            .count()
    };
    for spanner in part.spanners() {
        if spanner.kind() == SpannerKind::Neume {
            out.push(format!(
                "Neume {}",
                spanner
                    .spanned()
                    .iter()
                    .flatten()
                    .map(|place| ordinal(*place).to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
    }
    out.join("\n")
}

#[test]
fn the_crate_reads_and_writes_volpiano_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, read, as_musicxml, written) =
        Python::attach(|py| -> PyResult<(Vec<String>, usize, usize, usize)> {
            init_py(py)?;
            add_dependency_venv(py, &root)?;
            let _ = py.import("music21")?;
            let module = |code: &str, name: &str| -> PyResult<Bound<'_, PyModule>> {
                PyModule::from_code(
                    py,
                    &std::ffi::CString::new(code).expect("no nul in the helper"),
                    &std::ffi::CString::new(format!("{name}.py")).expect("no nul in the name"),
                    &std::ffi::CString::new(name).expect("no nul in the name"),
                )
            };
            let music21 = module(MUSIC21, "volpiano_parity_music21")?;
            let helpers = module(STRIP_LAYOUT, "volpiano_parity_helpers")?;
            let sources = module(SOURCE_TEXT, "volpiano_parity_sources")?;
            let today = musicxml_common::pin_encoding_date(py)?;
            let version: String = py.import("music21")?.getattr("__version__")?.extract()?;
            let options = ExportOptions {
                encoding_date: Some(today),
                software: format!("music21 v.{version}"),
                ..ExportOptions::default()
            };

            let mut failures = Vec::new();
            let mut read = 0;
            let mut as_musicxml = 0;
            let strings: Vec<String> = STRINGS
                .iter()
                .map(|text| text.to_string())
                .chain(generated())
                .collect();
            for text in &strings {
                let (their_outline, their_written): (String, String) =
                    match music21.getattr("read")?.call1((text,)) {
                        Ok(answer) => answer.extract()?,
                        Err(error) => {
                            failures.push(format!("{text:?}: music21 could not read it: {error}"));
                            continue;
                        }
                    };
                let part = match from_volpiano(text) {
                    Ok(part) => part,
                    Err(error) => {
                        failures.push(format!("{text:?}: the crate could not read it: {error}"));
                        continue;
                    }
                };
                read += 1;
                if let Some(difference) = first_difference(&outline(&part), &their_outline) {
                    failures.push(format!("{text:?}: read differently: {difference}"));
                    continue;
                }
                match to_volpiano(&part) {
                    Ok(ours) if ours == their_written => {}
                    Ok(ours) => failures.push(format!(
                        "{text:?}: written back differently:\n  music21    {their_written:?}\n  music21-rs {ours:?}"
                    )),
                    Err(error) => {
                        failures.push(format!("{text:?}: the crate could not write it: {error}"));
                    }
                }
                let theirs: String = match music21.getattr("musicxml")?.call1((
                    text,
                    helpers.getattr("strip_layout")?,
                )) {
                    Ok(document) => document.extract()?,
                    // A part music21's exporter refuses is compared by its
                    // outline alone.
                    Err(_) => continue,
                };
                as_musicxml += 1;
                let mut score = Stream::with_kind(StreamKind::Score);
                score.insert(0.0, part);
                match to_musicxml(&score, &options) {
                    Ok(ours) => {
                        if let Some(difference) = first_difference(
                            &normalize_ids(&ours),
                            &normalize_ids(theirs.trim_end()),
                        ) {
                            failures.push(format!("{text:?}: as MusicXML: {difference}"));
                        }
                    }
                    Err(error) => failures.push(format!(
                        "{text:?}: the crate could not write it as MusicXML: {error}"
                    )),
                }
            }

            let chosen: Vec<String> = match std::env::var("VOLPIANO_PARITY_SCORES") {
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
            let mut written = 0;
            for name in &chosen {
                let Some(source): Option<String> =
                    sources.getattr("source_text")?.call1((name,))?.extract()?
                else {
                    continue;
                };
                let theirs: String = match music21.getattr("written")?.call1((name,)) {
                    Ok(text) => text.extract()?,
                    Err(error) => {
                        failures.push(format!("{name}: music21 could not write it: {error}"));
                        continue;
                    }
                };
                let ours = match from_musicxml(&source).and_then(|score| to_volpiano(&score)) {
                    Ok(ours) => ours,
                    Err(error) => {
                        failures.push(format!("{name}: the crate could not write it: {error}"));
                        continue;
                    }
                };
                written += 1;
                if let Some(difference) = first_difference(&ours, &theirs) {
                    failures.push(format!("{name}: written differently: {difference}"));
                }
            }
            Ok((failures, read, as_musicxml, written))
        })
        .expect("the Python side runs");

    println!(
        "{read} strings read, {as_musicxml} of them compared as MusicXML; {written} scores written"
    );
    assert!(
        failures.is_empty(),
        "{} differences:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}
