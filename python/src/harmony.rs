//! music21's `harmony` module, over the crate's chord symbols.
//!
//! What is replaced is the pair of functions that *read* a chord and name it —
//! `chordSymbolFigureFromChord` and `chordSymbolFromChord` — which is where
//! the crate's own chord-symbol work lives.
//!
//! `Harmony`, `ChordSymbol`, `NoChord` and `ChordStepModification` stay
//! music21's, and that is deliberate rather than unfinished. music21's
//! `ChordSymbol` is a `Chord` that also carries a figure, a key, a roman
//! numeral, chord-step modifications and the MusicXML round trip; the crate
//! carries the figure and the pitches it stands for. Leaving the classes
//! alone means music21's own `ChordSymbol` builds its chords out of *our*
//! `Chord`, `Pitch`, `Note` and `Interval` — a harder test than a facade for
//! it would be, and the same choice `tempo` makes for `MetricModulation`.
//!
//! The four table functions (`getAbbreviationListGivenChordType` and its
//! neighbours) are left alone for a different reason: music21's
//! `changeAbbreviationFor` and `addNewChordSymbol` *edit* the table those read,
//! and the crate's is a static one. Replacing the readers without the writers
//! would make the pair disagree, and the table itself is already checked
//! against music21's by `chord_type_parity`. The figure is written with the
//! abbreviation music21's live table gives the kind, so an abbreviation a
//! caller has changed is the one written.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyTuple;

use music21_rs::chordsymbol::ChordSymbolFigure;

use crate::chord::Chord;

/// The names the `harmony` facade replaces in `music21.harmony`.
pub const NAMES: &[&str] = &[
    "chordSymbolFigureFromChord",
    "chordSymbolFromChord",
    "HarmonyException",
];

pyo3::create_exception!(music21_rs_facade, HarmonyException, crate::Music21Exception);

/// music21's sentence for a chord no kind in its table fits.
const UNIDENTIFIED: &str = "Chord Symbol Cannot Be Identified";

/// The crate's chord behind whatever was handed over.
///
/// music21 passes its own `Chord` here, which under the harness is one of
/// ours. A `ChordSymbol` is a chord of music21's own class, built before the
/// swap, so it is read by its pitches and the root its figure fixed.
fn chord_of(py: Python<'_>, inChord: &Bound<'_, PyAny>) -> PyResult<music21_rs::Chord> {
    if let Ok(chord) = inChord.extract::<PyRef<'_, Chord>>() {
        return Ok(chord.synced_inner(py));
    }
    let mut names = Vec::new();
    for pitch in inChord.getattr("pitches")?.try_iter()? {
        names.push(pitch?.getattr("nameWithOctave")?.extract::<String>()?);
    }
    let mut chord = music21_rs::Chord::new(names.as_slice())
        .map_err(|error| HarmonyException::new_err(crate::pitch::message(&error)))?;
    if let Ok(overrides) = inChord.getattr("_overrides")
        && let Ok(root) = overrides.call_method1("get", ("root",))
        && !root.is_none()
        && let Ok(name) = root.getattr("nameWithOctave")?.extract::<String>()
        && let Ok(root) = music21_rs::Pitch::from_name(name)
    {
        chord.set_root(Some(root));
    }
    Ok(chord)
}

/// The figure of a chord and its kind, written as music21 writes them.
///
/// A suspended second in inversion is read as a suspended fourth on its bass,
/// and music21 fixes the chord's root there as it says so; the chord handed
/// in is given the same root. The abbreviation is the one music21's live
/// table gives the kind, since `changeAbbreviationFor` edits that table.
fn figure_and_kind(py: Python<'_>, inChord: &Bound<'_, PyAny>) -> PyResult<(String, &'static str)> {
    let chord = chord_of(py, inChord)?;
    if chord.notes().is_empty() {
        return Ok((String::new(), ""));
    }
    let Some(figure) = ChordSymbolFigure::from_chord(&chord) else {
        return Ok((UNIDENTIFIED.to_string(), ""));
    };
    if chord.root().is_some_and(|root| root.name() != figure.root) {
        let bass = inChord.call_method0("bass")?;
        inChord.call_method1("root", (bass,))?;
    }
    let abbreviation = py
        .import("music21.harmony")?
        .getattr("getAbbreviationListGivenChordType")?
        .call1((figure.kind,))?
        .get_item(0)?
        .extract::<String>()
        .unwrap_or_else(|_| figure.abbreviation.to_string());
    Ok((figure.written_with(&abbreviation), figure.kind))
}

/// music21's `chordSymbolFigureFromChord`: the lead-sheet symbol a chord
/// would be written with.
///
/// With `includeChordType` it answers the kind beside the figure, as music21
/// does — except for an empty chord, which has no kind and which music21
/// answers with a bare empty string either way.
#[pyfunction]
#[pyo3(signature = (inChord, includeChordType = false))]
pub fn chordSymbolFigureFromChord<'py>(
    py: Python<'py>,
    inChord: &Bound<'py, PyAny>,
    includeChordType: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let (figure, kind) = figure_and_kind(py, inChord)?;
    if !includeChordType || figure.is_empty() {
        return Ok(figure.into_pyobject(py)?.into_any());
    }
    Ok(PyTuple::new(py, [figure.as_str(), kind])?.into_any())
}

/// music21's `chordSymbolFromChord`: the same figure, read back as a chord
/// symbol.
///
/// The object handed back is music21's own `ChordSymbol`, since that class is
/// not replaced — which is exactly what music21's own function does with the
/// figure it works out.
#[pyfunction]
pub fn chordSymbolFromChord<'py>(
    py: Python<'py>,
    inChord: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let (figure, _) = figure_and_kind(py, inChord)?;
    let symbol = py
        .import("music21")?
        .getattr("harmony")?
        .getattr("ChordSymbol")?
        .call1((figure,))?;
    // The symbol sounds the chord's own pitches, in the octaves the chord
    // has them, rather than the ones its figure would realize.
    symbol.setattr("pitches", inChord.getattr("pitches")?)?;
    Ok(symbol)
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add_function(wrap_pyfunction!(chordSymbolFigureFromChord, m)?)?;
    m.add_function(wrap_pyfunction!(chordSymbolFromChord, m)?)?;
    m.add("HarmonyException", py.get_type::<HarmonyException>())?;
    Ok(())
}
