//! Fuzzes the crate's readers on stable Rust: each test reads its seeds and
//! then a fixed number of mutations of them -- bytes flipped, cut short,
//! spliced, lines and tags deleted or doubled, numbers swapped for
//! awkward ones -- and fails if a reader panics instead of returning `Ok`
//! or `Err`.
//!
//! The mutations come from a fixed seed, so a run is repeatable. Three
//! variables widen a run for a deep local search:
//!
//! - `MUSIC21_FUZZ_ITERATIONS`: mutations per reader (default 300).
//! - `MUSIC21_FUZZ_RNG`: the seed of the generator (default 0x5eed).
//! - `MUSIC21_FUZZ_CORPUS`: a directory whose files, matched by extension,
//!   are added to the seeds, such as music21's own `music21/music21`.
//!
//! ```text
//! MUSIC21_FUZZ_ITERATIONS=200000 MUSIC21_FUZZ_CORPUS=music21/music21 \
//!     cargo test --release --all-features --test fuzz_readers -- --nocapture
//! ```
//!
//! Each input that panics is written under `target/tmp/fuzz-crashes` and
//! named in the failure. The seeds are in `tests/fuzz_seeds` and in
//! `python-parity/tests`; the `.nwc`, `.nwctxt`, MuseData, Capella, MEI and
//! RomanText seeds are music21's own test files, under its BSD licence.

use std::cell::RefCell;
use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::{Duration, Instant};

/// Files above this size are left out of the seeds, to keep a run quick.
const LARGEST_SEED: u64 = 128 * 1024;

/// The most files a corpus directory adds to one reader's seeds.
const MOST_CORPUS_SEEDS: usize = 400;

/// An input slower than this is reported, as a reader that loops.
const SLOWEST_INPUT: Duration = Duration::from_secs(5);

/// A xorshift generator: small, and the same on every platform.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// A number below `bound`, or nought for an empty range.
    fn below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            return 0;
        }
        (self.next() % bound as u64) as usize
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// Bytes the readers give meaning to.
const BYTES: &[u8] = b"\0\x7f\x80\xff0123456789-+./:|<>=\"'&;!%*[](){}^_~,\n\t ";

/// Numbers that tend to break arithmetic.
const NUMBERS: &[&str] = &[
    "0",
    "-1",
    "-0",
    "1",
    "2",
    "3",
    "7",
    "127",
    "128",
    "255",
    "256",
    "65535",
    "65536",
    "2147483647",
    "2147483648",
    "-2147483649",
    "4294967296",
    "9223372036854775807",
    "18446744073709551616",
    "99999999999999999999999",
    "1e308",
    "1e309",
    "NaN",
    "inf",
    "-inf",
    "0.0",
    "0.5",
    "1/0",
    "0/0",
    "",
    "00",
    "\u{e9}",
    "\u{4e2d}",
];

/// A mutation of `seed`, one to four changes deep.
fn mutate(rng: &mut Rng, seed: &[u8], seeds: &[Vec<u8>]) -> Vec<u8> {
    let mut data = seed.to_vec();
    for _ in 0..=rng.below(4) {
        change(rng, &mut data, seeds);
    }
    data
}

fn change(rng: &mut Rng, data: &mut Vec<u8>, seeds: &[Vec<u8>]) {
    let len = data.len();
    match rng.below(12) {
        0 if len > 0 => {
            let at = rng.below(len);
            data[at] ^= 1 << rng.below(8);
        }
        1 if len > 0 => {
            let at = rng.below(len);
            data[at] = *rng.pick(BYTES);
        }
        2 => {
            let at = rng.below(len + 1);
            data.insert(at, *rng.pick(BYTES));
        }
        3 => data.truncate(rng.below(len + 1)),
        4 if len > 0 => {
            let start = rng.below(len);
            let end = (start + 1 + rng.below(32)).min(len);
            data.drain(start..end);
        }
        5 if len > 0 => {
            let start = rng.below(len);
            let end = (start + 1 + rng.below(64)).min(len);
            let copy = data[start..end].to_vec();
            let at = rng.below(len + 1);
            data.splice(at..at, copy);
        }
        6 => {
            let other = rng.pick(seeds);
            if !other.is_empty() {
                let start = rng.below(other.len());
                let end = (start + 1 + rng.below(256)).min(other.len());
                let at = rng.below(len + 1);
                data.splice(at..at, other[start..end].iter().copied());
            }
        }
        7 => change_line(rng, data),
        8 => change_tag(rng, data),
        9 => change_number(rng, data),
        10 if len > 0 => {
            // A run of one byte, for lengths and counts read from the data.
            let at = rng.below(len);
            let run = vec![*rng.pick(BYTES); 1 + rng.below(16)];
            data.splice(at..at, run);
        }
        _ => {
            let number = rng.pick(NUMBERS).as_bytes().to_vec();
            let at = rng.below(len + 1);
            data.splice(at..at, number);
        }
    }
}

/// Spans of `data` between `open` and the next `close`, both included.
fn spans(data: &[u8], open: u8, close: u8) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let mut start = None;
    for (index, byte) in data.iter().enumerate() {
        if *byte == open {
            start = Some(index);
        } else if *byte == close
            && let Some(begin) = start.take()
        {
            found.push((begin, index + 1));
        }
    }
    found
}

/// Deletes, doubles or moves a line.
fn change_line(rng: &mut Rng, data: &mut Vec<u8>) {
    let mut lines: Vec<Vec<u8>> = data
        .split(|byte| *byte == b'\n')
        .map(<[u8]>::to_vec)
        .collect();
    let at = rng.below(lines.len());
    match rng.below(3) {
        0 => {
            lines.remove(at);
        }
        1 => {
            let line = lines[at].clone();
            lines.insert(at, line);
        }
        _ => {
            let other = rng.below(lines.len());
            lines.swap(at, other);
        }
    }
    *data = lines.join(&b'\n');
}

/// Deletes, doubles or moves an XML tag.
fn change_tag(rng: &mut Rng, data: &mut Vec<u8>) {
    let tags = spans(data, b'<', b'>');
    if tags.is_empty() {
        return;
    }
    let (start, end) = *rng.pick(&tags);
    let tag = data[start..end].to_vec();
    match rng.below(3) {
        0 => {
            data.drain(start..end);
        }
        1 => {
            data.splice(start..start, tag);
        }
        _ => {
            let (other, _) = *rng.pick(&tags);
            data.drain(start..end);
            let at = other.min(data.len());
            data.splice(at..at, tag);
        }
    }
}

/// Swaps a run of digits for an awkward number.
fn change_number(rng: &mut Rng, data: &mut Vec<u8>) {
    let mut runs = Vec::new();
    let mut start = None;
    for (index, byte) in data.iter().enumerate() {
        match (byte.is_ascii_digit(), start) {
            (true, None) => start = Some(index),
            (false, Some(begin)) => {
                runs.push((begin, index));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(begin) = start {
        runs.push((begin, data.len()));
    }
    if runs.is_empty() {
        return;
    }
    let (start, end) = *rng.pick(&runs);
    let number = rng.pick(NUMBERS).as_bytes().to_vec();
    data.splice(start..end, number);
}

thread_local! {
    /// While set, a panic on this thread is recorded here and not printed.
    static CAUGHT: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Installs a panic hook that records a fuzzed reader's panic, with where it
/// happened, and leaves every other panic to the default hook.
fn quiet_panics() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        let default = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            let recorded = CAUGHT.with(|caught| {
                let mut caught = caught.borrow_mut();
                if caught.is_none() {
                    return false;
                }
                let place = info
                    .location()
                    .map(|place| format!("{}:{}", place.file(), place.line()))
                    .unwrap_or_default();
                let payload = info.payload();
                let message = payload
                    .downcast_ref::<&str>()
                    .map(ToString::to_string)
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_default();
                *caught = Some(format!("{place}: {message}"));
                true
            });
            if !recorded {
                default(info);
            }
        }));
    });
}

/// Runs `reader` on `input`, giving the panic's place and message if it
/// panics.
fn run(reader: &dyn Fn(&[u8]), input: &[u8]) -> Option<String> {
    CAUGHT.with(|caught| *caught.borrow_mut() = Some(String::new()));
    let result = panic::catch_unwind(AssertUnwindSafe(|| reader(input)));
    let caught = CAUGHT.with(|caught| caught.borrow_mut().take());
    result.err().map(|_| caught.unwrap_or_default())
}

fn variable(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The files of `directory`, and of the directories in it, sorted.
fn files_in(directory: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(directory) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(files_in(&path));
        } else {
            found.push(path);
        }
    }
    found.sort();
    found
}

fn small(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|meta| meta.len() <= LARGEST_SEED)
}

fn has_extension(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extensions.contains(&extension.to_ascii_lowercase().as_str()))
}

/// A reader's seeds: its directory under `tests/fuzz_seeds`, the files in
/// `python-parity/tests` and in `MUSIC21_FUZZ_CORPUS` with one of its
/// extensions, and `extra`.
fn seeds(name: &str, extensions: &[&str], extra: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    let mut paths = files_in(&manifest().join("tests/fuzz_seeds").join(name));
    let parity = files_in(&manifest().join("python-parity/tests"));
    paths.extend(
        parity
            .into_iter()
            .filter(|path| has_extension(path, extensions) && small(path)),
    );
    if let Some(corpus) = variable("MUSIC21_FUZZ_CORPUS") {
        let mut corpus: Vec<PathBuf> = files_in(Path::new(&corpus))
            .into_iter()
            .filter(|path| has_extension(path, extensions) && small(path))
            .collect();
        // An even spread of a large corpus, not its first directory.
        let step = corpus.len().div_ceil(MOST_CORPUS_SEEDS).max(1);
        corpus = corpus.into_iter().step_by(step).collect();
        paths.extend(corpus);
    }
    let mut found: Vec<Vec<u8>> = paths
        .iter()
        .filter_map(|path| fs::read(path).ok())
        .collect();
    found.extend(extra);
    assert!(!found.is_empty(), "no seeds for {name}");
    found
}

/// Reads every seed and `MUSIC21_FUZZ_ITERATIONS` mutations of them with
/// `reader`, failing with the inputs that panicked.
fn fuzz(name: &str, seeds: &[Vec<u8>], reader: &dyn Fn(&[u8])) {
    quiet_panics();
    let iterations: usize = variable("MUSIC21_FUZZ_ITERATIONS")
        .and_then(|value| value.parse().ok())
        .unwrap_or(300);
    let rng_seed: u64 = variable("MUSIC21_FUZZ_RNG")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0x5eed);
    let mut rng = Rng(rng_seed.max(1));
    let crashes = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("fuzz-crashes");
    let mut failures: Vec<String> = Vec::new();
    let mut places: Vec<String> = Vec::new();
    let started = Instant::now();

    let mutations = (0..iterations).map(|_| {
        let seed = rng.pick(seeds).clone();
        mutate(&mut rng, &seed, seeds)
    });
    // Reading lazily keeps the generator's draws in one order either way.
    let inputs = seeds.iter().cloned().chain(mutations);
    for (index, input) in inputs.enumerate() {
        let began = Instant::now();
        let panicked = run(reader, &input);
        let slow = began.elapsed() > SLOWEST_INPUT;
        let problem = match (panicked, slow) {
            (Some(message), _) => message,
            (None, true) => format!("took {:?}", began.elapsed()),
            (None, false) => continue,
        };
        // One report per place a reader panics, not one per input.
        let place = problem.split(": ").next().unwrap_or_default().to_string();
        if places.contains(&place) {
            continue;
        }
        places.push(place);
        let _ = fs::create_dir_all(&crashes);
        let path = crashes.join(format!("{name}-{index}"));
        let _ = fs::write(&path, &input);
        failures.push(format!("{problem}\n    input: {}", path.display()));
    }
    eprintln!(
        "fuzz {name}: {} seeds, {iterations} mutations in {:.1?}",
        seeds.len(),
        started.elapsed()
    );
    assert!(
        failures.is_empty(),
        "{name} panicked on {} input(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn text(bytes: &[u8]) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(bytes)
}

#[cfg(feature = "musicxml")]
#[test]
fn musicxml() {
    let seeds = seeds("musicxml", &["musicxml", "xml"], Vec::new());
    fuzz("musicxml", &seeds, &|bytes| {
        let _ = music21::musicxml::from_musicxml(&text(bytes));
    });
}

#[test]
fn mei() {
    let seeds = seeds("mei", &["mei"], Vec::new());
    fuzz("mei", &seeds, &|bytes| {
        let _ = music21::mei::from_mei(&text(bytes));
    });
}

#[test]
fn abc() {
    let seeds = seeds("abc", &["abc"], Vec::new());
    fuzz("abc", &seeds, &|bytes| {
        let text = text(bytes);
        let _ = music21::abc::from_abc(&text);
        let _ = music21::abc::from_abc_number(&text, 2);
    });
}

#[test]
fn humdrum() {
    let seeds = seeds("humdrum", &["krn"], Vec::new());
    fuzz("humdrum", &seeds, &|bytes| {
        let _ = music21::humdrum::from_humdrum(&text(bytes));
    });
}

/// MIDI files written by the crate from the ABC and Humdrum seeds, beside
/// the files in the seed directories.
fn written_midi() -> Vec<Vec<u8>> {
    let abc = seeds("abc", &["abc"], Vec::new());
    let humdrum = seeds("humdrum", &["krn"], Vec::new());
    let scores = abc
        .iter()
        .filter_map(|bytes| music21::abc::from_abc(&text(bytes)).ok())
        .chain(
            humdrum
                .iter()
                .filter_map(|bytes| music21::humdrum::from_humdrum(&text(bytes)).ok()),
        );
    scores
        .filter_map(|score| music21::to_midi(&score, &music21::midi::ExportOptions::default()).ok())
        .collect()
}

#[test]
fn midi() {
    let seeds = seeds("midi", &["mid", "midi"], written_midi());
    fuzz("midi", &seeds, &|bytes| {
        let _ = music21::from_midi(bytes);
        let _ = music21::midi::read_midi_bytes_with_tempo(bytes);
    });
}

#[test]
fn capella() {
    let seeds = seeds("capella", &["capx"], Vec::new());
    fuzz("capella", &seeds, &|bytes| {
        let _ = music21::capella::from_capella(&text(bytes));
    });
}

#[test]
fn noteworthy() {
    let seeds = seeds("noteworthy", &["nwctxt"], Vec::new());
    fuzz("noteworthy", &seeds, &|bytes| {
        let _ = music21::noteworthy::from_noteworthy(&text(bytes));
    });
}

#[test]
fn nwc() {
    let seeds = seeds("nwc", &["nwc"], Vec::new());
    fuzz("nwc", &seeds, &|bytes| {
        let _ = music21::noteworthy::from_nwc(bytes);
    });
}

#[test]
fn musedata() {
    let seeds = seeds("musedata", &["md"], Vec::new());
    fuzz("musedata", &seeds, &|bytes| {
        let text = text(bytes);
        let _ = music21::musedata::from_musedata(&[&text]);
    });
}

#[test]
fn romantext() {
    let seeds = seeds("romantext", &["rntxt"], Vec::new());
    fuzz("romantext", &seeds, &|bytes| {
        let _ = music21::romantext::from_roman_text(&text(bytes));
    });
}

#[test]
fn tsv() {
    use music21::romantext::tsv::{Version, from_tsv};

    let seeds = seeds("tsv", &["tsv"], Vec::new());
    fuzz("tsv", &seeds, &|bytes| {
        let text = text(bytes);
        let _ = from_tsv(&text, Version::One);
        let _ = from_tsv(&text, Version::Two);
    });
}

#[test]
fn clercq_temperley() {
    let seeds = seeds("clercq_temperley", &["cttxt"], Vec::new());
    fuzz("clercq_temperley", &seeds, &|bytes| {
        let _ = music21::romantext::clercq_temperley::from_clercq_temperley(&text(bytes));
    });
}

#[test]
fn tinynotation() {
    let seeds = seeds("tinynotation", &[], Vec::new());
    fuzz("tinynotation", &seeds, &|bytes| {
        let _ = music21::tinynotation::from_tiny_notation(&text(bytes));
    });
}

#[test]
fn volpiano() {
    let seeds = seeds("volpiano", &[], Vec::new());
    fuzz("volpiano", &seeds, &|bytes| {
        let _ = music21::volpiano::from_volpiano(&text(bytes));
    });
}

#[test]
fn musescore() {
    let seeds = seeds("musescore", &["mscx"], Vec::new());
    fuzz("musescore", &seeds, &|bytes| {
        let _ = music21::musescore::from_mscx(&text(bytes));
    });
}

#[test]
fn scala() {
    let seeds = seeds("scala", &["scl"], Vec::new());
    fuzz("scala", &seeds, &|bytes| {
        let _ = music21::tuningsystem::scala::ScalaScale::parse_bytes(bytes);
    });
}

#[test]
fn sieve() {
    let seeds = seeds("sieve", &[], Vec::new());
    fuzz("sieve", &seeds, &|bytes| {
        let _ = music21::Sieve::parse(&text(bytes));
    });
}
