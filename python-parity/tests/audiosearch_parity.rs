//! The crate's `audiosearch` against music21's `audioSearch`.
//!
//! The recordings are music21's own `test_audio.wav` and WAV files made
//! here: a C major scale and a tune of held notes, with silences, in mono,
//! in stereo, and too short for a whole chunk. Every stage is given what
//! music21 gave its stage before, so each is compared on its own, bit for
//! bit -- frequencies snapped to a scale, smoothed, made pitches, joined
//! into notes and rests, and quantized into a stream -- except the
//! frequencies found by autocorrelation, which music21 finds by FFT and so
//! rounds otherwise; those must agree to within a millionth. Whole
//! transcriptions are compared note for note. The histogram, quantizing,
//! quarter length estimate and score follower's decision are compared on
//! numbers drawn here from a fixed seed.

use music21_rs::Pitch;
use music21_rs::audiosearch;
use music21_rs::musicxml::from_musicxml;
use music21_rs::scale::{Scale, ScaleType};
use music21_rs::stream::{Stream, StreamElement};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

const MUSIC21: &str = r#"
import io
import math
import os
import random
import struct
import tempfile
import wave
from music21 import audioSearch, corpus, scale
from music21.audioSearch import transcriber

def spelled(p):
    # The crate writes a flat as b where music21 writes -.
    return p.name.replace('-', 'b') + str(p.octave)

def bits(value):
    return str(struct.unpack('<Q', struct.pack('<d', float(value)))[0])

def tone(frequency, seconds, rate=44100, amplitude=8000, channels=1):
    frames = []
    for i in range(int(seconds * rate)):
        value = int(amplitude * math.sin(2 * math.pi * frequency * i / rate)) if frequency else 0
        frames.append(struct.pack('<h', value) * channels)
    return b''.join(frames)

def wav(frames, channels=1, rate=44100):
    out = io.BytesIO()
    with wave.open(out, 'wb') as w:
        w.setnchannels(channels)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(frames)
    return out.getvalue()

C_MAJOR = [261.63, 293.66, 329.63, 349.23, 392.0, 440.0, 493.88, 523.25]

def recordings():
    path = os.path.join(os.path.dirname(audioSearch.__file__), 'test_audio.wav')
    with open(path, 'rb') as f:
        own = f.read()
    scale_tones = b''.join(tone(f, 0.4) + tone(0, 0.1) for f in C_MAJOR)
    tune = b''.join(tone(f, s) for f, s in
                    [(0, 0.5), (392.0, 0.6), (440.0, 0.3), (392.0, 0.3), (329.63, 1.2), (0, 0.4),
                     (261.63, 0.6), (293.66, 0.6), (329.63, 0.9)])
    stereo = b''.join(tone(f, 0.5, channels=2) for f in [220.0, 246.94, 261.63, 293.66])
    return [
        ('test_audio', own),
        ('scale', wav(scale_tones)),
        ('tune', wav(tune)),
        ('stereo', wav(stereo, channels=2)),
        ('short', wav(tone(440.0, 0.01))),
    ]

def frequencies(data):
    with tempfile.NamedTemporaryFile(suffix='.wav', delete=False) as f:
        f.write(data)
        name = f.name
    try:
        return [float(x) for x in audioSearch.getFrequenciesFromAudioFile(waveFilename=name)]
    finally:
        os.remove(name)

def scales():
    return {'major': None, 'chromatic': scale.ChromaticScale('C4'), 'd-minor': scale.MinorScale('D4')}

def stages(freqs):
    lines = []
    for label, sc in scales().items():
        try:
            detected = audioSearch.detectPitchFrequencies(freqs, sc)
        except Exception:
            lines.append(f'{label} detect error')
            continue
        lines.append(f'{label} detect ' + ' '.join(bits(f) for f in detected))
        for levels in (1, 3, 7):
            try:
                smoothed = audioSearch.smoothFrequencies(detected, smoothLevels=levels)
            except Exception:
                lines.append(f'{label} smooth {levels} error')
                continue
            lines.append(f'{label} smooth {levels} ' + ' '.join(str(f) for f in smoothed))
        try:
            smoothed = audioSearch.smoothFrequencies(detected)
            objects, plotted = audioSearch.pitchFrequenciesToObjects(smoothed, sc)
        except Exception:
            lines.append(f'{label} objects error')
            continue
        lines.append(f'{label} objects ' + ' '.join(spelled(p) for p in objects))
        lines.append(f'{label} plotted ' + ' '.join(bits(f) for f in plotted))
        try:
            notes, lengths = audioSearch.joinConsecutiveIdenticalPitches(objects)
        except Exception:
            lines.append(f'{label} join error')
            continue
        lines.append(f'{label} join ' + ' '.join(
            ('rest' if n.isRest else spelled(n.pitch)) + f'/{d}' for n, d in zip(notes, lengths)))
        try:
            score, unused = audioSearch.notesAndDurationsToStream(notes, lengths)
            part = score.parts.first()
            lines.append(f'{label} stream ' + ' '.join(
                ('rest' if n.isRest else spelled(n.pitch)) + f'@{bits(n.offset)}/{bits(n.quarterLength)}'
                for n in part.notesAndRests))
        except Exception:
            lines.append(f'{label} stream error')
    return lines

def transcribe(data):
    with tempfile.NamedTemporaryFile(suffix='.wav', delete=False) as f:
        f.write(data)
        name = f.name
    try:
        part = transcriber.monophonicStreamFromFile(name)
    except Exception:
        return ['transcribe error']
    finally:
        os.remove(name)
    return ['transcribe ' + ' '.join(
        ('rest' if n.isRest else spelled(n.pitch)) + f'@{bits(n.offset)}/{bits(n.quarterLength)}'
        for n in part.notesAndRests)]

def numbers():
    rng = random.Random(21)
    lines = []
    for _ in range(40):
        data = [rng.choice([rng.randint(0, 60), rng.uniform(0, 50)]) for _ in range(rng.randint(1, 25))]
        bins = rng.randint(1, 9)
        try:
            counts, edges = audioSearch.histogram(data, bins)
            outcome = ' '.join(str(c) for c in counts) + ' | ' + ' '.join(bits(e) for e in edges)
        except Exception:
            outcome = 'error'
        lines.append(f'histogram {bins} ' + ' '.join(bits(d) for d in data) + f' -> {outcome}')
        lengths = [rng.randint(1, 80) for _ in range(rng.randint(1, 20))]
        most = rng.choice([0, 0.25, 0.5, 1.0, 1.5, 2.0, 3.0])
        lines.append(f'estimate {bits(most)} ' + ' '.join(str(n) for n in lengths)
                     + f' -> {bits(audioSearch.quarterLengthEstimation(lengths, most))}')
    for i in range(0, 600, 7):
        lines.append(f'quantize {bits(i / 100)} -> {bits(audioSearch.quantizeDuration(i / 100))}')
    for _ in range(60):
        count = rng.randint(1, 5)
        beginning = [rng.uniform(0, 120) for _ in range(count + 2)]
        ids = rng.sample(range(count + 2), count)
        probabilities = sorted((rng.uniform(0.3, 1.0) for _ in range(count)), reverse=True)
        parts = []
        for i, p in zip(ids, probabilities):
            class Found:
                pass
            found = Found()
            found.id = str(i)
            found.matchProbability = p
            parts.append(found)
        prediction = rng.uniform(0, 120)
        last = rng.uniform(0, 100)
        countdown = rng.randint(0, 2)
        page = rng.choice([None, (rng.uniform(0, 50), rng.uniform(50, 120))])
        args = [parts, prediction, beginning, last, countdown]
        if page is not None:
            args += list(page)
        try:
            position, newCount = audioSearch.decisionProcess(*args)
            outcome = f'{position} {newCount}'
        except Exception:
            outcome = 'error'
        lines.append('decide ' + ' '.join(f'{i}:{bits(p)}' for i, p in zip(ids, probabilities))
                     + f' | {bits(prediction)} ' + ' '.join(bits(b) for b in beginning)
                     + f' | {bits(last)} {countdown} '
                     + ('-' if page is None else f'{bits(page[0])} {bits(page[1])}')
                     + f' -> {outcome}')
    return lines

def followed(text):
    from music21 import converter
    notes = converter.parse(text, format='musicxml', forceSource=True).flatten().notes.stream()
    out = []
    for lengths in ([12, 12, 24, 6, 6, 48], [30, 31, 29, 60, 15, 90, 31]):
        from music21 import note as m21note
        objects = [m21note.Note('C4') for _ in lengths]
        score, estimate = audioSearch.notesAndDurationsToStream(objects, lengths, scNotes=notes)
        out.append(f'followed {bits(estimate)} ' + ' '.join(
            bits(n.quarterLength) for n in score.parts.first().notesAndRests))
    return out

def source(name):
    import zipfile
    path = str(corpus.getWork(name))
    with zipfile.ZipFile(path) as archive:
        names = [n for n in archive.namelist()
                 if not n.startswith('META-INF') and n.lower().endswith(('.xml', '.musicxml'))]
        return archive.read(names[0]).decode('utf-8')
"#;

fn bits(value: f64) -> String {
    value.to_bits().to_string()
}

fn scales() -> music21_rs::Result<Vec<(&'static str, Option<Scale>)>> {
    Ok(vec![
        ("major", None),
        (
            "chromatic",
            Some(Scale::new(ScaleType::Chromatic, Pitch::from_name("C4")?)),
        ),
        (
            "d-minor",
            Some(Scale::new(ScaleType::Minor, Pitch::from_name("D4")?)),
        ),
    ])
}

fn element_name(element: &StreamElement) -> String {
    match element {
        StreamElement::Note(note) => note.pitch().name_with_octave(),
        _ => "rest".to_string(),
    }
}

fn notes_line(part: &Stream) -> String {
    part.events()
        .iter()
        .map(|event| {
            format!(
                "{}@{}/{}",
                element_name(event.element()),
                bits(event.offset()),
                bits(event.element().quarter_length())
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn stages(frequencies: &[f64]) -> music21_rs::Result<Vec<String>> {
    let mut lines = Vec::new();
    for (label, scale) in scales()? {
        let scale = scale.as_ref();
        let Ok(detected) = audiosearch::detect_pitch_frequencies(frequencies, scale) else {
            lines.push(format!("{label} detect error"));
            continue;
        };
        lines.push(format!(
            "{label} detect {}",
            detected
                .iter()
                .map(|f| bits(*f))
                .collect::<Vec<_>>()
                .join(" ")
        ));
        for levels in [1, 3, 7] {
            match audiosearch::smooth_frequencies(&detected, levels) {
                Ok(smoothed) => lines.push(format!(
                    "{label} smooth {levels} {}",
                    smoothed
                        .iter()
                        .map(i64::to_string)
                        .collect::<Vec<_>>()
                        .join(" ")
                )),
                Err(_) => lines.push(format!("{label} smooth {levels} error")),
            }
        }
        let objects = audiosearch::smooth_frequencies(&detected, 7).and_then(|smoothed| {
            let smoothed: Vec<f64> = smoothed.into_iter().map(|f| f as f64).collect();
            audiosearch::pitch_frequencies_to_objects(&smoothed, scale)
        });
        let Ok((objects, plotted)) = objects else {
            lines.push(format!("{label} objects error"));
            continue;
        };
        lines.push(format!(
            "{label} objects {}",
            objects
                .iter()
                .map(Pitch::name_with_octave)
                .collect::<Vec<_>>()
                .join(" ")
        ));
        lines.push(format!(
            "{label} plotted {}",
            plotted
                .iter()
                .map(|f| bits(*f))
                .collect::<Vec<_>>()
                .join(" ")
        ));
        let Ok((notes, lengths)) = audiosearch::join_consecutive_identical_pitches(objects) else {
            lines.push(format!("{label} join error"));
            continue;
        };
        lines.push(format!(
            "{label} join {}",
            notes
                .iter()
                .zip(&lengths)
                .map(|(note, length)| format!("{}/{length}", element_name(note)))
                .collect::<Vec<_>>()
                .join(" ")
        ));
        match audiosearch::notes_and_durations_to_stream(&notes, &lengths, None, true) {
            Ok((score, _)) => {
                lines.push(format!("{label} stream {}", notes_line(score.parts()[0])))
            }
            Err(_) => lines.push(format!("{label} stream error")),
        }
    }
    Ok(lines)
}

fn transcribe(bytes: &[u8]) -> Vec<String> {
    match audiosearch::monophonic_stream_from_wav(bytes, None) {
        Ok(part) => vec![format!("transcribe {}", notes_line(&part))],
        Err(_) => vec!["transcribe error".to_string()],
    }
}

/// The crate's answer to each line `numbers` printed, from the inputs the
/// line gives, in the same form.
fn numbers(theirs: &[String]) -> Vec<String> {
    let float = |text: &str| f64::from_bits(text.parse::<u64>().expect("bits"));
    theirs
        .iter()
        .map(|line| {
            let (head, _) = line.split_once(" -> ").expect("outcome");
            let (kind, inputs) = head.split_once(' ').expect("kind");
            let ours = match kind {
                "histogram" => {
                    let mut words = inputs.split(' ');
                    let bins: usize = words.next().expect("bins").parse().expect("bins");
                    let data: Vec<f64> = words.map(float).collect();
                    match audiosearch::histogram(&data, bins) {
                        Ok((counts, edges)) => format!(
                            "{} | {}",
                            counts
                                .iter()
                                .map(usize::to_string)
                                .collect::<Vec<_>>()
                                .join(" "),
                            edges
                                .iter()
                                .map(|edge| bits(*edge))
                                .collect::<Vec<_>>()
                                .join(" ")
                        ),
                        Err(_) => "error".to_string(),
                    }
                }
                "estimate" => {
                    let mut words = inputs.split(' ');
                    let most = float(words.next().expect("most"));
                    let lengths: Vec<usize> =
                        words.map(|word| word.parse().expect("length")).collect();
                    audiosearch::quarter_length_estimation(&lengths, most)
                        .map_or("error".to_string(), bits)
                }
                "quantize" => bits(audiosearch::quantize_duration(float(inputs))),
                _ => {
                    let sections: Vec<&str> = inputs.split(" | ").collect();
                    let parts: Vec<(usize, f64)> = sections[0]
                        .split(' ')
                        .map(|pair| {
                            let (id, probability) = pair.split_once(':').expect("pair");
                            (id.parse().expect("id"), float(probability))
                        })
                        .collect();
                    let mut middle = sections[1].split(' ');
                    let prediction = float(middle.next().expect("prediction"));
                    let beginning: Vec<f64> = middle.map(float).collect();
                    let tail: Vec<&str> = sections[2].split(' ').collect();
                    let page = (tail[2] != "-").then(|| (float(tail[2]), float(tail[3])));
                    match audiosearch::decision_process(
                        &parts,
                        prediction,
                        &beginning,
                        float(tail[0]),
                        tail[1].parse().expect("countdown"),
                        page,
                    ) {
                        Ok((position, count)) => format!("{position} {count}"),
                        Err(_) => "error".to_string(),
                    }
                }
            };
            format!("{head} -> {ours}")
        })
        .collect()
}

/// Where two reports first differ, if they do.
fn difference(what: &str, ours: &[String], theirs: &[String]) -> Option<String> {
    if let Some((index, (a, b))) = ours
        .iter()
        .zip(theirs)
        .enumerate()
        .find(|(_, (a, b))| a != b)
    {
        return Some(format!(
            "{what}, line {index}:
  music21    {}
  music21-rs {}",
            b.chars().take(600).collect::<String>(),
            a.chars().take(600).collect::<String>()
        ));
    }
    (ours.len() != theirs.len()).then(|| {
        format!(
            "{what}: {} lines and {} in music21",
            ours.len(),
            theirs.len()
        )
    })
}

#[test]
fn the_crate_finds_notes_in_recordings_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let failures = Python::attach(|py| -> PyResult<Vec<String>> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"audiosearch_parity_music21.py",
            c"audiosearch_parity_music21",
        )?;
        let mut failures = Vec::new();

        let recordings: Vec<(String, Vec<u8>)> =
            music21.getattr("recordings")?.call0()?.extract()?;
        for (name, bytes) in &recordings {
            let theirs: Vec<f64> = match music21.getattr("frequencies")?.call1((bytes.as_slice(),))
            {
                Ok(found) => found.extract()?,
                Err(_) => Vec::new(),
            };
            match audiosearch::frequencies_from_wav(bytes) {
                Ok(ours) => {
                    if ours.len() != theirs.len() {
                        failures.push(format!(
                            "{name}: {} frequencies and {} in music21",
                            ours.len(),
                            theirs.len()
                        ));
                    } else if let Some((index, (a, b))) = ours
                        .iter()
                        .zip(&theirs)
                        .enumerate()
                        .find(|(_, (a, b))| ((*a - *b) / *b).abs() > 1e-6)
                    {
                        failures.push(format!("{name}: frequency {index} is {a}, music21's {b}"));
                    }
                }
                Err(error) => {
                    failures.push(format!("{name}: the crate could not read it: {error}"))
                }
            }
            let staged: Vec<String> = music21
                .getattr("stages")?
                .call1((theirs.clone(),))?
                .extract()?;
            let ours = stages(&theirs).unwrap_or_else(|error| vec![format!("error {error}")]);
            failures.extend(difference(&format!("{name} stages"), &ours, &staged));
            let whole: Vec<String> = music21
                .getattr("transcribe")?
                .call1((bytes.as_slice(),))?
                .extract()?;
            failures.extend(difference(
                &format!("{name} transcription"),
                &transcribe(bytes),
                &whole,
            ));
        }

        let theirs: Vec<String> = music21.getattr("numbers")?.call0()?.extract()?;
        failures.extend(difference("numbers", &numbers(&theirs), &theirs));

        let text: String = music21
            .getattr("source")?
            .call1(("bach/bwv66.6",))?
            .extract()?;
        let score = from_musicxml(&text).expect("the crate reads bwv66.6");
        let notes = score.flatten();
        let mut followed = Vec::new();
        for lengths in [vec![12, 12, 24, 6, 6, 48], vec![30, 31, 29, 60, 15, 90, 31]] {
            let objects: Vec<StreamElement> = lengths
                .iter()
                .map(|_| music21_rs::Note::from_name("C4").expect("C4").into())
                .collect();
            let (made, estimate) =
                audiosearch::notes_and_durations_to_stream(&objects, &lengths, Some(&notes), true)
                    .expect("stream");
            followed.push(format!(
                "followed {} {}",
                bits(estimate),
                made.parts()[0]
                    .events()
                    .iter()
                    .map(|event| bits(event.element().quarter_length()))
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
        let theirs_followed: Vec<String> =
            music21.getattr("followed")?.call1((&text,))?.extract()?;
        failures.extend(difference("followed", &followed, &theirs_followed));
        Ok(failures)
    })
    .expect("the Python side runs");

    assert!(
        failures.is_empty(),
        "{} differences:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}
