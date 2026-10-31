//! A sweep for the crate's flat `b` leaking into what the wheel hands
//! Python.
//!
//! The crate writes a flat `b` (`Bb4`) and music21 writes it `-` (`B-4`);
//! every name-like getter of the wheel respells on the way out. This asks
//! a few hundred pitches, chords, keys, chord symbols, Roman numerals and
//! scales for their names on both sides, and fails where ours has a step
//! letter followed by `b` and music21's does not say the same. music21
//! writes some itself -- `pitchedCommonName` is `Bb-minor triad` -- so its
//! own answer is the allowlist, asked rather than kept by hand.

use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use pyo3::types::PyModule;
use utils::{init_py, prepare};

const SWEEP: &str = r#"
from types import SimpleNamespace

import music21_rs
from music21 import chord, harmony, key, pitch, roman, scale

THEIRS = SimpleNamespace(
    Pitch=pitch.Pitch,
    Chord=chord.Chord,
    Key=key.Key,
    ChordSymbol=harmony.ChordSymbol,
    chordSymbolFromChord=harmony.chordSymbolFromChord,
    RomanNumeral=roman.RomanNumeral,
    MajorScale=scale.MajorScale,
)
OURS = SimpleNamespace(
    Pitch=music21_rs.Pitch,
    Chord=music21_rs.Chord,
    Key=music21_rs.Key,
    ChordSymbol=music21_rs.ChordSymbol,
    chordSymbolFromChord=music21_rs.chordSymbolFromChord,
    RomanNumeral=music21_rs.RomanNumeral,
    MajorScale=music21_rs.MajorScale,
)

ROOTS = [step + alter for step in 'CDEFGAB' for alter in ('', '#', '-')]
ODD = ['B--', 'E--', 'A--', 'B-`', 'E`', 'F##']
CHORDS = {
    'major': ['M3', 'P5'],
    'minor': ['m3', 'P5'],
    'diminished': ['m3', 'd5'],
    'augmented': ['M3', 'A5'],
    'dominant': ['M3', 'P5', 'm7'],
    'half-diminished': ['m3', 'd5', 'm7'],
    'diminished seventh': ['m3', 'd5', 'd7'],
}
KINDS = ['major', 'minor-seventh', 'dominant-ninth', 'half-diminished', 'major-sixth']
ROMANS = ['I', 'ii', 'bIII', 'bVI', 'bVII', 'N6', 'V7', '#ivo7', 'viio7', 'It6']


def spelled(root, intervals):
    """The pitches of a chord on `root`, as music21 spells them."""
    bottom = pitch.Pitch(root + '3')
    return [bottom.nameWithOctave] + [
        bottom.transpose(step).nameWithOctave for step in intervals
    ]


def questions():
    """Every (label, question) pair; a question takes one side's classes."""
    for name in ROOTS + ODD:
        for octave in (2, 4, 6):
            written = f'{name}{octave}'
            yield f'Pitch({written!r}).name', lambda m, w=written: m.Pitch(w).name
            yield (
                f'Pitch({written!r}).nameWithOctave',
                lambda m, w=written: m.Pitch(w).nameWithOctave,
            )
    for root in ROOTS:
        for kind, intervals in CHORDS.items():
            notes = spelled(root, intervals)
            for getter in ('commonName', 'pitchedCommonName'):
                yield (
                    f'Chord({notes}).{getter}',
                    lambda m, n=notes, g=getter: getattr(m.Chord(n), g),
                )
            yield (
                f'chordSymbolFromChord({notes}).figure',
                lambda m, n=notes: m.chordSymbolFromChord(m.Chord(n)).figure,
            )
        for kind in KINDS:
            yield (
                f'ChordSymbol(root={root!r}, kind={kind!r}).figure',
                lambda m, r=root, k=kind: m.ChordSymbol(root=r, kind=k).figure,
            )
        yield (
            f'ChordSymbol(root={root!r}, kind=minor-seventh, bass=A-).figure',
            lambda m, r=root: m.ChordSymbol(root=r, kind='minor-seventh', bass='A-').figure,
        )
        for tonic in (root, root.lower()):
            yield f'Key({tonic!r}).name', lambda m, t=tonic: m.Key(t).name
            yield (
                f'Key({tonic!r}).tonicPitchNameWithCase',
                lambda m, t=tonic: m.Key(t).tonicPitchNameWithCase,
            )
            yield f'Key({tonic!r}).tonic.name', lambda m, t=tonic: m.Key(t).tonic.name
        yield f'MajorScale({root!r}).name', lambda m, r=root: m.MajorScale(r).name
    for tonic in ('E-', 'A-', 'D-', 'b-', 'f', 'c'):
        for figure in ROMANS:
            for getter in ('figure', 'pitchedCommonName'):
                yield (
                    f'RomanNumeral({figure!r}, {tonic!r}).{getter}',
                    lambda m, f=figure, t=tonic, g=getter: getattr(
                        m.RomanNumeral(f, m.Key(t)), g
                    ),
                )


def sweep():
    """(label, ours, theirs) for every question music21 can answer."""
    rows = []
    for label, question in questions():
        try:
            theirs = question(THEIRS)
        except Exception:
            continue
        try:
            ours = question(OURS)
        except Exception as error:
            ours = f'raised {type(error).__name__}: {error}'
        rows.append((label, str(ours), str(theirs)))
    return rows
"#;

/// Whether `text` has a step letter that opens a word, or follows `add` or
/// `omit`, followed by `b`: the crate's flat, where music21 writes `-`.
fn has_flat_b(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.windows(2).enumerate().any(|(at, pair)| {
        let before = &bytes[..at];
        let opens = before.last().is_none_or(|byte| !byte.is_ascii_alphabetic())
            || before.ends_with(b"add")
            || before.ends_with(b"omit");
        opens && matches!(pair[0], b'A'..=b'G' | b'a'..=b'g') && pair[1] == b'b'
    })
}

#[test]
fn flat_b_is_found_where_it_is() {
    assert!(has_flat_b("Bb4"));
    assert!(has_flat_b("bb minor"));
    assert!(has_flat_b("Bbm7/Ab"));
    assert!(has_flat_b("CaddDb"));
    assert!(has_flat_b("Db-major triad"));
    assert!(!has_flat_b("B-m7b5"));
    assert!(!has_flat_b("bVII7"));
    assert!(!has_flat_b("b- minor"));
    assert!(!has_flat_b("diminished seventh chord"));
}

#[test]
fn no_name_leaks_the_crates_flat() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let rows = Python::attach(|py| -> PyResult<Vec<(String, String, String)>> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let sweep = PyModule::from_code(
            py,
            &std::ffi::CString::new(SWEEP).expect("no nul in the helper"),
            c"flat_spelling_parity.py",
            c"flat_spelling_parity",
        )?;
        sweep.getattr("sweep")?.call0()?.extract()
    })
    .expect("the sweep runs");

    // Ours with a `b` flat is a leak unless music21 answers the same.
    let leaks: Vec<String> = rows
        .iter()
        .filter(|(_, ours, theirs)| has_flat_b(ours) && ours != theirs)
        .map(|(label, ours, theirs)| format!("{label}: {ours:?}, music21 {theirs:?}"))
        .collect();
    let allowed = rows
        .iter()
        .filter(|(_, ours, theirs)| has_flat_b(ours) && ours == theirs)
        .count();

    // A sweep that asked little, or found none of music21's own `b`s,
    // proves nothing.
    assert!(
        rows.len() > 500,
        "only {} answers were compared",
        rows.len()
    );
    assert!(allowed > 0, "no answer of music21's own spells a flat `b`");

    assert!(
        leaks.is_empty(),
        "{} names spell a flat `b` where music21 does not:\n  {}",
        leaks.len(),
        leaks.join("\n  ")
    );
}
