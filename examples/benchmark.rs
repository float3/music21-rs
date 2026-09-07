//! Times the crate itself, with no Python in the way.
//!
//! `python/benchmarks/bench.py` compares the wheel against music21 through
//! the same Python API; this is the other half of that measurement, and what
//! says whether a cost lives in the crate or in the binding. Run it with
//! `cargo run --release --example benchmark` — a debug build measures
//! nothing useful.

use std::hint::black_box;
use std::time::Instant;

fn time<T>(label: &str, iterations: u32, mut f: impl FnMut() -> T) {
    let start = Instant::now();
    for _ in 0..iterations {
        black_box(f());
    }
    let nanos = start.elapsed().as_nanos() as f64 / f64::from(iterations);
    println!("{label:<44}{nanos:10.0} ns");
}

fn main() {
    use music21_rs::{Chord, ChromaticInterval, Interval, Pitch};
    let n = 200_000;
    time("Pitch::from_name(\"C#4\")", n, || {
        Pitch::from_name("C#4").unwrap()
    });
    time("Interval::from_name(\"P5\")", n, || {
        Interval::from_name("P5").unwrap()
    });
    time("Interval::from_semitones(7)", n, || {
        Interval::from_semitones(7).unwrap()
    });
    time("ChromaticInterval::from_int(7).get_diatonic()", n, || {
        ChromaticInterval::from_int(7).get_diatonic()
    });
    time("Chord::new(\"C4 E4 G4\")", n / 10, || {
        Chord::new("C4 E4 G4").unwrap()
    });
    let chord = Chord::new("C4 E4 G4").unwrap();
    time("Chord::common_name()", n / 10, || chord.common_name());
}
