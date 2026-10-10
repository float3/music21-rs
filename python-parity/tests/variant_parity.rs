//! The crate's variants against music21's: `variant` and the `Stream`
//! methods that make variants real.
//!
//! For each corpus score music21 makes a second version -- in each part of
//! eight measures or more, the second measure's notes a tone higher, the
//! fourth taken out and the sixth played twice -- and writes it as
//! MusicXML. Both sides read the two texts and run every variant function
//! on them: the score merged with its second version, what each variant
//! replaces, each part's variants made real (matched by span and not),
//! made replacements, made into blocks and refined, each part merged with
//! the second version's as an ossia four ways, and the score shown with an
//! ossia-like part. Every measure, note, rest, chord and variant they leave
//! must be the same, offsets and lengths bit for bit, or both must fail.
//! `VARIANT_PARITY_SCORES`, a `;`-separated list of corpus names, runs those
//! instead of the writer test's scores, which is how the corpus is swept.

use music21_rs::musicxml::from_musicxml;
use music21_rs::variant::{
    Variant, make_all_variants_replacements, make_variant_blocks, merge_part_as_ossia,
    merge_variants, refine_variant, replaced_elements,
};
use music21_rs::{Stream, StreamElement, StreamKind};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::SCORES;

const MUSIC21: &str = r#"
import copy
import struct
import zipfile
from music21 import chord, clef, converter, corpus, key, meter, note, stream, variant
from music21.musicxml import m21ToXml

def bits(value):
    return str(struct.unpack('<Q', struct.pack('<d', float(value)))[0])

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

def parse(text):
    return converter.parse(text, format='musicxml', forceSource=True)

def second_version(text):
    y = parse(text)
    for p in y.parts:
        ms = list(p.getElementsByClass(stream.Measure))
        if len(ms) < 8:
            continue
        for n in ms[1].recurse().notes:
            if isinstance(n, note.Note):
                n.transpose('M2', inPlace=True)
        p.insertAndShift(ms[5].offset + ms[5].duration.quarterLength, copy.deepcopy(ms[5]))
        p.remove(ms[3], shiftOffsets=True)
    try:
        exporter = m21ToXml.GeneralObjectExporter(y)
        exporter.makeNotation = False
        return exporter.parse().decode('utf-8')
    except Exception:
        return None

def pitch_text(p):
    alter = '' if p.accidental is None else bits(p.accidental.alter)
    return f'{p.step}{p.octave}:{alter}'

def dump(e, offset):
    if isinstance(e, variant.Variant):
        inner = sorted(t for t in (dump(c, c.getOffsetBySite(e.containedSite))
                                   for c in e.containedSite) if t)
        groups = ','.join(str(g) for g in e.groups)
        return (f'V@{bits(offset)}:{groups}:{bits(e.replacementQuarterLength)}:{e.lengthType}['
                + ' '.join(inner) + ']')
    if isinstance(e, stream.Measure):
        inner = []
        for c in e.recurse():
            if isinstance(c, stream.Stream):
                continue
            text = dump(c, c.getOffsetInHierarchy(e))
            if text:
                inner.append(text)
        return f'M@{bits(offset)}#{e.number}:{bits(e.duration.quarterLength)}[' + ' '.join(sorted(inner)) + ']'
    if isinstance(e, chord.Chord):
        return f'C@{bits(offset)}:{bits(e.quarterLength)}[' + ' '.join(pitch_text(p) for p in e.pitches) + ']'
    if isinstance(e, note.Note):
        return f'N@{bits(offset)}:{bits(e.quarterLength)}:{pitch_text(e.pitch)}'
    if isinstance(e, note.Rest):
        hidden = 'h' if e.style.hideObjectOnPrint else ''
        return f'R@{bits(offset)}:{bits(e.quarterLength)}{hidden}'
    return None

def dump_stream(s):
    items = []
    for e in s:
        text = dump(e, s.elementOffset(e))
        if text:
            items.append(text)
    return ' '.join(sorted(items))

def report(x_text, y_text):
    lines = []
    def attempt(label, run):
        try:
            lines.extend(run())
        except Exception as e:
            lines.append(f'{label} error')
            errors.append(f'{label}: {type(e).__name__}: {e}')
    errors = []
    x, y = parse(x_text), parse(y_text)
    merged = None
    try:
        merged = variant.mergeVariants(x, y, 'v')
    except Exception as e:
        lines.append('merge error')
        errors.append(f'merge: {type(e).__name__}: {e}')
        return lines, errors
    parts = list(merged.parts)
    for i, part in enumerate(parts):
        lines.append(f'merged {i}: ' + dump_stream(part))
        for k, v in enumerate(part.getElementsByClass(variant.Variant)):
            attempt(f'replaced {i} {k}', lambda: [f'replaced {i} {k}: ' + dump_stream(v.replacedElements(part))])
        for span in (True, False):
            attempt(f'activated {i} {span}',
                    lambda: [f'activated {i} {span}: ' + dump_stream(part.activateVariants('v', matchBySpan=span))])
        attempt(f'replacements {i}',
                lambda: [f'replacements {i}: ' + dump_stream(variant.makeAllVariantsReplacements(part))])
        def blocks():
            copied = copy.deepcopy(part)
            try:
                variant.makeVariantBlocks(copied)
                failed = ''
            except Exception:
                failed = ' failed'
            return [f'blocks {i}{failed}: ' + dump_stream(copied)]
        attempt(f'blocks {i}', blocks)
        def refined():
            copied = copy.deepcopy(part)
            found = list(copied.getElementsByClass(variant.Variant))
            if not found:
                return [f'refined {i}: none']
            variant.refineVariant(copied, found[0], inPlace=True)
            return [f'refined {i}: ' + dump_stream(copied)]
        attempt(f'refined {i}', refined)
    x_parts, y_parts = list(x.parts), list(y.parts)
    for i, (xp, yp) in enumerate(zip(x_parts, y_parts)):
        for by_number in (False, True):
            for recurse in (False, True):
                attempt(f'ossia {i} {by_number} {recurse}', lambda: [
                    f'ossia {i} {by_number} {recurse}: ' + dump_stream(
                        variant.mergePartAsOssia(xp, yp, 'ossia', compareByMeasureNumber=by_number,
                                                 recurseInMeasures=recurse))])
    if parts:
        def ossialike():
            shown = merged.showVariantAsOssialikePart(parts[0], ['v'])
            added = list(shown.parts)[len(parts):]
            return [f'ossialike {len(added)}'] + ['ossialike: ' + dump_stream(p) for p in added]
        attempt('ossialike', ossialike)
    return lines, errors
"#;

fn bits(value: f64) -> String {
    value.to_bits().to_string()
}

fn pitch_text(pitch: &music21_rs::Pitch) -> String {
    let alter = pitch
        .written_accidental()
        .map_or(String::new(), |accidental| bits(accidental.alter()));
    let octave = pitch
        .octave()
        .map_or("None".to_string(), |octave| octave.to_string());
    let step = pitch.name().chars().next().unwrap_or('?');
    format!("{step}{octave}:{alter}")
}

fn length_name(variant: &Variant) -> &'static str {
    variant.length_type().as_str()
}

fn dump(element: &StreamElement, offset: f64) -> Option<String> {
    Some(match element {
        StreamElement::Variant(variant) => {
            let mut inner: Vec<String> = variant
                .contents()
                .events()
                .iter()
                .filter_map(|event| dump(event.element(), event.offset()))
                .collect();
            inner.sort();
            format!(
                "V@{}:{}:{}:{}[{}]",
                bits(offset),
                variant.groups().join(","),
                bits(variant.replacement_quarter_length()),
                length_name(variant),
                inner.join(" ")
            )
        }
        StreamElement::Stream(measure) if measure.kind() == StreamKind::Measure => {
            let mut inner: Vec<String> = measure
                .recurse()
                .into_iter()
                .filter(|(_, element)| !matches!(element, StreamElement::Stream(_)))
                .filter_map(|(at, element)| dump(element, at))
                .collect();
            inner.sort();
            format!(
                "M@{}#{}:{}[{}]",
                bits(offset),
                measure.number(),
                bits(music21_rs::variant::highest_time(measure)),
                inner.join(" ")
            )
        }
        StreamElement::Chord(chord) => {
            let pitches: Vec<String> = chord
                .notes()
                .iter()
                .map(|note| pitch_text(note.pitch()))
                .collect();
            format!(
                "C@{}:{}[{}]",
                bits(offset),
                bits(element.quarter_length()),
                pitches.join(" ")
            )
        }
        StreamElement::ChordSymbol(symbol) => {
            // music21's chord symbol is a chord, of its pitches.
            let pitches: Vec<String> = symbol
                .pitches()
                .unwrap_or_default()
                .iter()
                .map(pitch_text)
                .collect();
            format!(
                "C@{}:{}[{}]",
                bits(offset),
                bits(element.quarter_length()),
                pitches.join(" ")
            )
        }
        StreamElement::Note(note) => format!(
            "N@{}:{}:{}",
            bits(offset),
            bits(element.quarter_length()),
            pitch_text(note.pitch())
        ),
        StreamElement::Rest(rest) => format!(
            "R@{}:{}{}",
            bits(offset),
            bits(element.quarter_length()),
            if rest.hidden() { "h" } else { "" }
        ),
        _ => return None,
    })
}

fn dump_stream(stream: &Stream) -> String {
    let mut items: Vec<String> = stream
        .events()
        .iter()
        .filter_map(|event| dump(event.element(), event.offset()))
        .collect();
    items.sort();
    items.join(" ")
}

fn variant_events(stream: &Stream) -> Vec<usize> {
    stream
        .events()
        .iter()
        .enumerate()
        .filter(|(_, event)| matches!(event.element(), StreamElement::Variant(_)))
        .map(|(index, _)| index)
        .collect()
}

fn our_report(x: &Stream, y: &Stream) -> (Vec<String>, Vec<String>) {
    let mut lines = Vec::new();
    let mut errors = Vec::new();
    let mut attempt =
        |lines: &mut Vec<String>,
         label: String,
         run: &mut dyn FnMut() -> music21_rs::Result<Vec<String>>| {
            match run() {
                Ok(more) => lines.extend(more),
                Err(error) => {
                    lines.push(format!("{label} error"));
                    errors.push(format!("{label}: {error}"));
                }
            }
        };
    let merged = match merge_variants(x, y, "v") {
        Ok(merged) => merged,
        Err(error) => return (vec!["merge error".to_string()], vec![error.to_string()]),
    };
    let parts: Vec<Stream> = merged.parts().into_iter().cloned().collect();
    for (i, part) in parts.iter().enumerate() {
        lines.push(format!("merged {i}: {}", dump_stream(part)));
        for (k, index) in variant_events(part).into_iter().enumerate() {
            attempt(&mut lines, format!("replaced {i} {k}"), &mut || {
                Ok(vec![format!(
                    "replaced {i} {k}: {}",
                    dump_stream(&replaced_elements(part, index, &[], false, false)?)
                )])
            });
        }
        for span in [true, false] {
            let label = if span { "True" } else { "False" };
            attempt(&mut lines, format!("activated {i} {label}"), &mut || {
                Ok(vec![format!(
                    "activated {i} {label}: {}",
                    dump_stream(&part.activate_variants(Some("v"), span)?)
                )])
            });
        }
        attempt(&mut lines, format!("replacements {i}"), &mut || {
            Ok(vec![format!(
                "replacements {i}: {}",
                dump_stream(&make_all_variants_replacements(part, false)?)
            )])
        });
        attempt(&mut lines, format!("blocks {i}"), &mut || {
            let mut copied = part.clone();
            let failed = if make_variant_blocks(&mut copied).is_err() {
                " failed"
            } else {
                ""
            };
            Ok(vec![format!(
                "blocks {i}{failed}: {}",
                dump_stream(&copied)
            )])
        });
        attempt(&mut lines, format!("refined {i}"), &mut || {
            let mut copied = part.clone();
            let Some(&first) = variant_events(&copied).first() else {
                return Ok(vec![format!("refined {i}: none")]);
            };
            refine_variant(&mut copied, first)?;
            Ok(vec![format!("refined {i}: {}", dump_stream(&copied))])
        });
    }
    let (x_parts, y_parts) = (x.parts(), y.parts());
    for (i, (xp, yp)) in x_parts.iter().zip(&y_parts).enumerate() {
        for by_number in [false, true] {
            for recurse in [false, true] {
                let label = format!(
                    "ossia {i} {} {}",
                    if by_number { "True" } else { "False" },
                    if recurse { "True" } else { "False" }
                );
                attempt(&mut lines, label.clone(), &mut || {
                    Ok(vec![format!(
                        "{label}: {}",
                        dump_stream(&merge_part_as_ossia(xp, yp, "ossia", by_number, recurse)?)
                    )])
                });
            }
        }
    }
    if !parts.is_empty() {
        attempt(&mut lines, "ossialike".to_string(), &mut || {
            let shown = merged.show_variant_as_ossialike_part(0, &["v"])?;
            let added: Vec<&Stream> = shown.parts().into_iter().skip(parts.len()).collect();
            let mut out = vec![format!("ossialike {}", added.len())];
            out.extend(
                added
                    .iter()
                    .map(|part| format!("ossialike: {}", dump_stream(part))),
            );
            Ok(out)
        });
    }
    (lines, errors)
}

/// The first line two reports differ on, and what each has there.
fn first_difference(ours: &[String], theirs: &[String]) -> String {
    let at = ours
        .iter()
        .zip(theirs)
        .position(|(a, b)| a != b)
        .unwrap_or(ours.len().min(theirs.len()));
    // Where in the line they part, with some of what comes before.
    let from = match (ours.get(at), theirs.get(at)) {
        (Some(a), Some(b)) => a
            .chars()
            .zip(b.chars())
            .position(|(x, y)| x != y)
            .unwrap_or(a.len().min(b.len()))
            .saturating_sub(150),
        _ => 0,
    };
    let cut = |line: Option<&String>| {
        line.map(|line| line.chars().skip(from).take(450).collect::<String>())
    };
    format!(
        "{} lines and {} in music21; first difference at line {at}:\n  music21    {:?}\n  music21-rs {:?}",
        ours.len(),
        theirs.len(),
        cut(theirs.get(at)),
        cut(ours.get(at))
    )
}

#[test]
fn the_crate_merges_and_activates_variants_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"variant_parity_music21.py",
            c"variant_parity_music21",
        )?;
        let mut failures = Vec::new();
        let chosen: Vec<String> = match std::env::var("VARIANT_PARITY_SCORES") {
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
        let mut compared = 0;
        for name in &chosen {
            let Some(x_text): Option<String> =
                music21.getattr("source_text")?.call1((name,))?.extract()?
            else {
                continue;
            };
            let Some(y_text): Option<String> =
                music21.getattr("second_version")?.call1((&x_text,))?.extract()?
            else {
                continue;
            };
            let (x, y) = match (from_musicxml(&x_text), from_musicxml(&y_text)) {
                (Ok(x), Ok(y)) => (x, y),
                (Err(error), _) | (_, Err(error)) => {
                    failures.push(format!("{name}: the crate could not read it: {error}"));
                    continue;
                }
            };
            compared += 1;
            let (theirs, their_errors): (Vec<String>, Vec<String>) = music21
                .getattr("report")?
                .call1((&x_text, &y_text))?
                .extract()?;
            let (ours, our_errors) = our_report(&x, &y);
            if ours != theirs {
                failures.push(format!(
                    "{name}: {}\n  music21 errors {their_errors:?}\n  music21-rs errors {our_errors:?}",
                    first_difference(&ours, &theirs)
                ));
            }
        }
        Ok((failures, compared))
    })
    .expect("the Python side runs");

    println!("{compared} scores compared");
    assert!(
        failures.is_empty(),
        "{} differences:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}
