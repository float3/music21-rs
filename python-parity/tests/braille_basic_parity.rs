//! The crate's braille for single elements against music21's:
//! `braille.basic`.
//!
//! The text of each corpus score is read by music21 and by `from_musicxml`
//! (which `musicxml_read_parity` holds to music21's reader). Every note --
//! with its octave and without, fingered upper first and lower -- chord --
//! descending with octaves and ascending without -- rest, clef, key
//! signature (cancelling the one before), time signature, barline, dynamic,
//! text expression, tempo text (in lines of 40 and 20), metronome mark and
//! instrument is written in braille by both, which must give the same
//! braille and the same English, kind by kind in the order music21 walks
//! the score, or both refuse. So must whether each note shows its octave
//! after the note before, each part's heading, every lyric syllable as a
//! word and as text, and the notes' braille written as braille ASCII, read
//! back, and drawn as dots. `BRAILLE_PARITY_SCORES`, a `;`-separated list of
//! corpus names, runs those instead of the writer test's scores, which is
//! how the corpus is swept.

use music21_rs::IntegerType;
use music21_rs::braille::basic::{
    self, NoteContext, Transcription, barline_to_braille, chord_to_braille, clef_to_braille,
    dynamic_to_braille, instrument_to_braille, key_sig_to_braille, metronome_mark_to_braille,
    note_to_braille, rest_to_braille, tempo_to_braille, text_expression_to_braille,
    time_sig_to_braille,
};
use music21_rs::musicxml::from_musicxml;
use music21_rs::stream::{Stream, StreamElement};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use std::collections::BTreeMap;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::SCORES;

const MUSIC21: &str = r#"
import zipfile
from music21 import (converter, corpus, note, chord, harmony, percussion, clef, key, meter, bar,
                     dynamics, expressions, tempo, instrument, stream)
from music21.braille import basic

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

def run(f, el):
    try:
        b = f()
        english = el.editorial.get('brailleEnglish', None) if el is not None else None
        return (b if isinstance(b, str) else repr(b), list(english or []), '')
    except Exception as e:
        return ('', [], type(e).__name__)

def plain(f):
    try:
        return (f(), '')
    except Exception as e:
        return ('', type(e).__name__)

def report(text):
    score = converter.parse(text, format='musicxml', forceSource=True)
    out = {}
    def add(kind, value):
        out.setdefault(kind, []).append(value)
    previous = None
    outgoing = None
    for el in score.recurse():
        if isinstance(el, note.Note):
            add('note', run(lambda: basic.noteToBraille(el), el))
            add('note bare', run(lambda: basic.noteToBraille(
                el, showOctave=False, upperFirstInFingering=False), el))
            add('octave', (basic.showOctaveWithNote(previous, el), ''))
            previous = el
        elif isinstance(el, chord.Chord) and not isinstance(el, (harmony.ChordSymbol,
                                                                  percussion.PercussionChord)):
            add('chord', run(lambda: basic.chordToBraille(el), el))
            add('chord up', run(lambda: basic.chordToBraille(
                el, descending=False, showOctave=False), el))
        elif isinstance(el, note.Rest):
            add('rest', run(lambda: basic.restToBraille(el), el))
        elif isinstance(el, clef.Clef):
            add('clef', run(lambda: basic.clefToBraille(el), el))
            add('clef switched', run(lambda: basic.clefToBraille(el, keyboardHandSwitched=True), el))
        elif isinstance(el, key.KeySignature):
            add('key', run(lambda: basic.keySigToBraille(el), el))
            add('key cancelling', run(lambda: basic.keySigToBraille(el, outgoingKeySig=outgoing), el))
            outgoing = el
        elif isinstance(el, meter.TimeSignature):
            add('time', run(lambda: basic.timeSigToBraille(el), el))
        elif isinstance(el, bar.Barline):
            add('barline', run(lambda: basic.barlineToBraille(el), el))
        elif isinstance(el, dynamics.Dynamic):
            add('dynamic', run(lambda: basic.dynamicToBraille(el), el))
            add('dynamic bare', run(lambda: basic.dynamicToBraille(el, precedeByWordSign=False), el))
        elif isinstance(el, expressions.TextExpression):
            add('text', run(lambda: basic.textExpressionToBraille(el), el))
            add('text bare', run(lambda: basic.textExpressionToBraille(
                el, precedeByWordSign=False), el))
        elif isinstance(el, tempo.MetronomeMark):
            add('metronome', run(lambda: basic.metronomeMarkToBraille(el), el))
        elif isinstance(el, tempo.TempoText):
            add('tempo', run(lambda: basic.tempoTextToBraille(el), el))
            add('tempo narrow', run(lambda: basic.tempoTextToBraille(el, maxLineLength=20), el))
        elif isinstance(el, instrument.Instrument):
            add('instrument', run(lambda: basic.instrumentToBraille(el), el))
        if isinstance(el, (note.Note, chord.Chord)) and not isinstance(el, harmony.ChordSymbol):
            for ly in el.lyrics:
                word = ly.text or ''
                add('word', plain(lambda: basic.wordToBraille(word)))
                add('word as text', plain(lambda: basic.wordToBraille(word, isTextExpression=True)))
    for p in score.parts:
        ks = p[key.KeySignature].first()
        ts = p[meter.TimeSignature].first()
        tt = p[tempo.TempoText].first()
        mm = p[tempo.MetronomeMark].first()
        for width in (40, 20):
            add('heading', plain(lambda: basic.transcribeHeading(ks, ts, tt, mm, maxLineLength=width)))
    notes = ''.join(b for b, _, e in out.get('note', []) if not e)[:200]
    add('ascii', plain(lambda: basic.brailleUnicodeToBrailleAscii(notes)))
    add('unicode', plain(lambda: basic.brailleAsciiToBrailleUnicode(
        basic.brailleUnicodeToBrailleAscii(notes))))
    add('symbols', plain(lambda: basic.brailleUnicodeToSymbols(notes[:20])))
    return out
"#;

type Answer = (String, Vec<String>, String);

fn answer(transcription: Transcription) -> Answer {
    (transcription.braille, transcription.english, String::new())
}

fn plain(result: music21_rs::Result<String>) -> (String, String) {
    match result {
        Ok(text) => (text, String::new()),
        Err(error) => (String::new(), error.to_string()),
    }
}

/// The first element of each kind in a part, as music21's
/// `part[Class].first()` finds it.
fn first_in(part: &Stream, wanted: fn(&StreamElement) -> bool) -> Option<&StreamElement> {
    part.recurse()
        .into_iter()
        .map(|(_, element)| element)
        .find(|element| wanted(element))
}

fn sharps_of(element: &StreamElement) -> Option<Option<IntegerType>> {
    match element {
        StreamElement::Key(key) => Some(Some(key.sharps())),
        StreamElement::KeySignature(signature) => Some(signature.sharps()),
        _ => None,
    }
}

/// Every element of a score in the order music21's `recurse` walks it: a
/// measure's left barline before what it holds and its right barline after.
fn walk(stream: &Stream) -> Vec<StreamElement> {
    fn into(stream: &Stream, out: &mut Vec<StreamElement>) {
        if let Some(barline) = stream.left_barline() {
            out.push(StreamElement::Barline(barline.clone()));
        }
        for event in stream.events() {
            out.push(event.element().clone());
            if let StreamElement::Stream(inner) = event.element() {
                into(inner, out);
            }
        }
        if let Some(barline) = stream.right_barline() {
            out.push(StreamElement::Barline(barline.clone()));
        }
    }
    let mut out = Vec::new();
    into(stream, &mut out);
    out
}

fn add(out: &mut BTreeMap<String, Vec<Answer>>, kind: &str, value: Answer) {
    out.entry(kind.to_string()).or_default().push(value);
}

fn ours(score: &Stream) -> BTreeMap<String, Vec<Answer>> {
    let mut out: BTreeMap<String, Vec<Answer>> = BTreeMap::new();
    let mut previous: Option<music21_rs::pitch::Pitch> = None;
    let mut outgoing: Option<Option<IntegerType>> = None;
    for element in &walk(score) {
        match element {
            StreamElement::Note(note) => {
                add(
                    &mut out,
                    "note",
                    answer(note_to_braille(note, true, true, NoteContext::default())),
                );
                add(
                    &mut out,
                    "note bare",
                    answer(note_to_braille(note, false, false, NoteContext::default())),
                );
                let shown = basic::show_octave_with_note(previous.as_ref(), note.pitch());
                add(
                    &mut out,
                    "octave",
                    (
                        if shown { "True" } else { "False" }.to_string(),
                        Vec::new(),
                        String::new(),
                    ),
                );
                previous = Some(note.pitch().clone());
            }
            StreamElement::Chord(chord) => {
                add(
                    &mut out,
                    "chord",
                    answer(chord_to_braille(chord, true, true)),
                );
                add(
                    &mut out,
                    "chord up",
                    answer(chord_to_braille(chord, false, false)),
                );
            }
            StreamElement::Rest(rest) => add(&mut out, "rest", answer(rest_to_braille(rest))),
            StreamElement::Clef(clef) => {
                add(&mut out, "clef", answer(clef_to_braille(clef, false)));
                add(
                    &mut out,
                    "clef switched",
                    answer(clef_to_braille(clef, true)),
                );
            }
            StreamElement::Key(_) | StreamElement::KeySignature(_) => {
                let sharps = sharps_of(element).expect("a key");
                add(&mut out, "key", answer(key_sig_to_braille(sharps, None)));
                add(
                    &mut out,
                    "key cancelling",
                    answer(key_sig_to_braille(sharps, outgoing)),
                );
                outgoing = Some(sharps);
            }
            StreamElement::TimeSignature(meter) => {
                add(&mut out, "time", answer(time_sig_to_braille(meter)))
            }
            StreamElement::Barline(barline) => {
                add(&mut out, "barline", answer(barline_to_braille(barline)))
            }
            StreamElement::Dynamic(dynamic) => {
                add(
                    &mut out,
                    "dynamic",
                    answer(dynamic_to_braille(dynamic, true)),
                );
                add(
                    &mut out,
                    "dynamic bare",
                    answer(dynamic_to_braille(dynamic, false)),
                );
            }
            StreamElement::TextExpression(text) => {
                add(
                    &mut out,
                    "text",
                    answer(text_expression_to_braille(text, true)),
                );
                add(
                    &mut out,
                    "text bare",
                    answer(text_expression_to_braille(text, false)),
                );
            }
            StreamElement::MetronomeMark(mark) => add(
                &mut out,
                "metronome",
                metronome_mark_to_braille(mark)
                    .map_or_else(|| (String::new(), Vec::new(), String::new()), answer),
            ),
            StreamElement::TempoText(tempo) => {
                add(&mut out, "tempo", answer(tempo_to_braille(tempo, 40)));
                add(
                    &mut out,
                    "tempo narrow",
                    answer(tempo_to_braille(tempo, 20)),
                );
            }
            StreamElement::Instrument(instrument) => add(
                &mut out,
                "instrument",
                match instrument_to_braille(instrument) {
                    Ok(transcription) => answer(transcription),
                    Err(error) => (String::new(), Vec::new(), error.to_string()),
                },
            ),
            _ => {}
        }
        let lyrics: Vec<music21_rs::notation::Lyric> = match element {
            StreamElement::Note(note) => note.lyrics().to_vec(),
            StreamElement::Chord(chord) => chord.lyrics().to_vec(),
            _ => Vec::new(),
        };
        for lyric in lyrics {
            let word = lyric.explicit_text().unwrap_or_default();
            let (text, error) = plain(basic::word_to_braille(&word, false));
            add(&mut out, "word", (text, Vec::new(), error));
            let (text, error) = plain(basic::word_to_braille(&word, true));
            add(&mut out, "word as text", (text, Vec::new(), error));
        }
    }
    for part in score.parts() {
        let sharps = first_in(part, |element| sharps_of(element).is_some()).and_then(sharps_of);
        let meter = first_in(part, |element| {
            matches!(element, StreamElement::TimeSignature(_))
        })
        .and_then(|element| match element {
            StreamElement::TimeSignature(meter) => Some(meter),
            _ => None,
        });
        let tempo = first_in(part, |element| {
            matches!(element, StreamElement::TempoText(_))
        })
        .and_then(|element| match element {
            StreamElement::TempoText(tempo) => Some(tempo),
            _ => None,
        });
        let mark = first_in(part, |element| {
            matches!(element, StreamElement::MetronomeMark(_))
        })
        .and_then(|element| match element {
            StreamElement::MetronomeMark(mark) => Some(mark),
            _ => None,
        });
        for width in [40, 20] {
            let (text, error) = plain(basic::transcribe_heading(sharps, meter, tempo, mark, width));
            add(&mut out, "heading", (text, Vec::new(), error));
        }
    }
    let notes: String = out
        .get("note")
        .map(|notes| {
            notes
                .iter()
                .filter(|(_, _, error)| error.is_empty())
                .map(|(braille, _, _)| braille.as_str())
                .collect::<String>()
        })
        .unwrap_or_default()
        .chars()
        .take(200)
        .collect();
    let ascii = basic::braille_unicode_to_braille_ascii(&notes);
    let back = ascii
        .as_ref()
        .map_err(|error| music21_rs::Error::Notation(error.to_string()))
        .and_then(|ascii| basic::braille_ascii_to_braille_unicode(ascii));
    let first: String = notes.chars().take(20).collect();
    for (kind, result) in [
        ("ascii", ascii),
        ("unicode", back),
        (
            "symbols",
            basic::braille_unicode_to_symbols(&first, "o", "\u{b7}"),
        ),
    ] {
        let (text, error) = plain(result);
        add(&mut out, kind, (text, Vec::new(), error));
    }
    out
}

#[test]
fn the_crate_writes_elements_in_braille_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"braille_basic_parity_music21.py",
            c"braille_basic_parity_music21",
        )?;
        let mut failures = Vec::new();
        let chosen: Vec<String> = match std::env::var("BRAILLE_PARITY_SCORES") {
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
            let Some(text): Option<String> =
                music21.getattr("source_text")?.call1((name,))?.extract()?
            else {
                continue;
            };
            let score = match from_musicxml(&text) {
                Ok(score) => score,
                Err(error) => {
                    failures.push(format!("{name}: the crate could not read it: {error}"));
                    continue;
                }
            };
            let theirs: BTreeMap<String, Vec<(String, Vec<String>, String)>> =
                match music21.getattr("report")?.call1((&text,)) {
                    Ok(report) => {
                        let raw: BTreeMap<String, Vec<Bound<'_, PyAny>>> = report.extract()?;
                        raw.into_iter()
                            .map(|(kind, values)| {
                                let values = values
                                    .into_iter()
                                    .map(|value| {
                                        if let Ok(full) = value.extract::<(String, Vec<String>, String)>() {
                                            full
                                        } else if let Ok((flag, error)) = value.extract::<(bool, String)>() {
                                            (if flag { "True" } else { "False" }.to_string(), Vec::new(), error)
                                        } else {
                                            let (text, error): (String, String) =
                                                value.extract().unwrap_or_default();
                                            (text, Vec::new(), error)
                                        }
                                    })
                                    .collect();
                                (kind, values)
                            })
                            .collect()
                    }
                    Err(error) => {
                        failures.push(format!("{name}: music21 could not write it: {error}"));
                        continue;
                    }
                };
            compared += 1;
            let ours = ours(&score);
            let kinds: std::collections::BTreeSet<&String> = ours.keys().chain(theirs.keys()).collect();
            for kind in kinds {
                let mine = ours.get(kind).cloned().unwrap_or_default();
                let music21 = theirs.get(kind).cloned().unwrap_or_default();
                let agree = |a: &Answer, b: &Answer| {
                    if b.2.is_empty() != a.2.is_empty() {
                        return false;
                    }
                    !b.2.is_empty() || (a.0 == b.0 && a.1 == b.1)
                };
                let differs = mine.len() != music21.len()
                    || mine.iter().zip(&music21).any(|(a, b)| !agree(a, b));
                if differs {
                    let at = mine
                        .iter()
                        .zip(&music21)
                        .position(|(a, b)| !agree(a, b))
                        .unwrap_or(mine.len().min(music21.len()));
                    failures.push(format!(
                        "{name}: {kind}: {} and {} in music21; first difference at {at}:\n  music21    {:?}\n  music21-rs {:?}",
                        mine.len(),
                        music21.len(),
                        music21.get(at),
                        mine.get(at)
                    ));
                }
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
        failures.join("\n")
    );
}
