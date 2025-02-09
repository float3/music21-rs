[![CI](https://github.com/float3/music21-rs/actions/workflows/CI.yaml/badge.svg)](https://github.com/float3/music21-rs/actions/workflows/CI.yaml)

This is a work-in-progress (WIP) **Rust** port of [music21](https://github.com/cuthbertLab/music21/).

### Motivation

The project began when I tried to develop a primitive chord-naming algorithm in HLSL and later C# for [AudioLink](https://github.com/llealloo/audiolink/edit/master/README.md). During my search for existing solutions, I discovered that the music21 library offered one of the most comprehensive implementations available.

Later, while working on [Tuning Playground](https://hilll.dev/tools/tuningplayground), I built a large chord-name lookup table (LUT) using music21 to generate it. However, this approach did not satisfy me because implementing the original algorithm is inherently more correct than relying on a LUT.

I wanted a more adaptable solution—an implementation of the music21 chord-naming algorithm that could run on the web, in WebAssembly, or be compiled to JavaScript, as well as on any other platform where running Python might not be ideal or possible. I considered writing it in Rust, TypeScript, C#/F#, Haskell, and Elm, but ultimately decided to go with Rust.

For now, I am adhering closely to the original design and APIs of music21; I plan to adopt a more idiomatic Rust style after implementing the core functionality and expanding the test suite.

I am also taking into account [this post](https://www.music21.org/music21docs/developerReference/startingOver.html). Thanks to the music21 team for providing this

### pyo3 Dependency

- **Purpose:**  
  `pyo3` is used only in the test suite and `build.rs`.

- **Build Script:**  
  When the `python` feature is enabled, `build.rs` generates Rust code from Python. Although normal usage doesn’t require running the build script (since the generated code is checked in), it’s preferable to keep the generator alongside the generated code.

- **Testing:**
  Some Tests are done by running python code and inspecting it to see if the Rust counterpart matches, for example that's how I test the auto-generated code.  

- **Limitations:**
  Running the test suite always requires Python to be installed (Cargo dev-dependencies cannot yet be optional)

- **Library Dependencies:**  
  The main library doesn’t—and will never—depend on `pyo3` or Python.

- **Shared Code:**  
  The `utils` crate contains code shared between `build.rs` and the test suite.

### Usage

```rust
use music21_rs::chord::Chord;

let chord = Chord::new(Some("C E G"));
// This currently fails because the library is still under development.
assert!(chord.is_ok());
assert_eq!(chord.unwrap().pitched_common_name(), "C-major triad");
```

thanks to Michael Scott Asato Cuthbert for his work in computational musicology

thanks to Michael and the music21 contributors and Community for the music21 library

thanks to Valentin for answering my Rust questions