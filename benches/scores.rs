//! Times whole-score work: reading MusicXML and MEI, and `chordify`,
//! `make_notation` and `flatten` on a score read from MusicXML.
//!
//! `cargo bench --features musicxml --bench scores`. The scores are read
//! from disk when the bench starts: music21's corpus when the submodule is
//! checked out, else the smaller copies the parity suite keeps. A bench
//! whose files are all missing is skipped with a note.

use std::hint::black_box;
use std::path::{Path, PathBuf};

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use music21::makenotation::make_notation;
use music21::mei::from_mei;
use music21::musicxml::from_musicxml;
use music21::{Stream, StreamKind};

/// A large MusicXML score: an 880 KB Clara Schumann piano trio movement,
/// or a 220 KB Mozart quartet movement without the submodule.
const MUSICXML: &[&str] = &[
    "music21/music21/corpus/schumann_clara/opus17/movement3.xml",
    "python-parity/tests/mscx/mozart-k80-movement3.musicxml",
];

/// An MEI score: music21's own test document, or the parity suite's.
const MEI: &[&str] = &[
    "music21/music21/mei/test/test_file.mei",
    "python-parity/tests/mei/spans.mei",
];

/// The first of `candidates` that can be read, named by its folder and
/// file, with its text.
fn first_readable(candidates: &[&str]) -> Option<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    candidates.iter().find_map(|candidate| {
        let path: PathBuf = root.join(candidate);
        let text = std::fs::read_to_string(&path).ok()?;
        let folder = path.parent()?.file_name()?.to_string_lossy();
        let name = format!("{folder}/{}", path.file_stem()?.to_string_lossy());
        Some((name, text))
    })
}

/// The score with each part flattened, leaving only what sounds and the
/// marks in force: what `make_notation` is for.
fn unnotated(score: &Stream) -> Stream {
    let mut flat = Stream::with_kind(StreamKind::Score);
    for part in score.parts() {
        let mut line = part.flatten();
        line.set_kind(StreamKind::Part);
        flat.insert(0.0, line);
    }
    flat
}

fn parsing(c: &mut Criterion) {
    match first_readable(MUSICXML) {
        Some((name, text)) => {
            c.bench_function(&format!("from_musicxml {name}"), |b| {
                b.iter(|| from_musicxml(black_box(&text)).unwrap())
            });
        }
        None => eprintln!("skipping from_musicxml: no score found"),
    }
    match first_readable(MEI) {
        Some((name, text)) => {
            c.bench_function(&format!("from_mei {name}"), |b| {
                b.iter(|| from_mei(black_box(&text)).unwrap())
            });
        }
        None => eprintln!("skipping from_mei: no score found"),
    }
}

fn streams(c: &mut Criterion) {
    let Some((name, text)) = first_readable(MUSICXML) else {
        eprintln!("skipping the stream benches: no score found");
        return;
    };
    let score = from_musicxml(&text).unwrap();

    c.bench_function(&format!("chordify {name}"), |b| {
        b.iter(|| black_box(&score).chordify().unwrap())
    });
    c.bench_function(&format!("flatten {name}"), |b| {
        b.iter(|| black_box(&score).flatten())
    });

    let flat = unnotated(&score);
    c.bench_function(&format!("make_notation {name}"), |b| {
        b.iter_batched(
            || flat.clone(),
            |mut score| {
                make_notation(&mut score).unwrap();
                score
            },
            BatchSize::LargeInput,
        )
    });
}

criterion_group!(benches, parsing, streams);
criterion_main!(benches);
