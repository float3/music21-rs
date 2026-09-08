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
//! against music21's by `chord_type_parity`.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyTuple;

use music21_rs::chordsymbol::{chord_symbol_figure_from_chord, chord_symbol_kind_from_chord};

use crate::chord::Chord;
use crate::pitch::message;

/// The names the `harmony` facade replaces in `music21.harmony`.
pub const NAMES: &[&str] = &[
    "chordSymbolFigureFromChord",
    "chordSymbolFromChord",
    "HarmonyException",
];

pyo3::create_exception!(music21_rs_facade, HarmonyException, crate::Music21Exception);

fn harmony_error(error: music21_rs::Error) -> PyErr {
    HarmonyException::new_err(message(&error))
}

/// music21's sentence for a chord no kind in its table fits.
const UNIDENTIFIED: &str = "Chord Symbol Cannot Be Identified";

/// The crate's chord behind whatever was handed over.
///
/// music21 passes its own `Chord` here, which under the harness is one of
/// ours; anything else is the caller's mistake and is reported as one.
fn chord_of(py: Python<'_>, inChord: &Bound<'_, PyAny>) -> PyResult<music21_rs::Chord> {
    let chord = inChord.extract::<PyRef<'_, Chord>>()?;
    Ok(chord.synced_inner(py))
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
    let chord = chord_of(py, inChord)?;
    let figure = chord_symbol_figure_from_chord(&chord)
        .map_err(harmony_error)?
        .unwrap_or_else(|| UNIDENTIFIED.to_string());
    if !includeChordType || figure.is_empty() {
        return Ok(figure.into_pyobject(py)?.into_any());
    }
    let kind = chord_symbol_kind_from_chord(&chord).unwrap_or_default();
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
    let chord = chord_of(py, inChord)?;
    let figure = chord_symbol_figure_from_chord(&chord)
        .map_err(harmony_error)?
        .unwrap_or_else(|| UNIDENTIFIED.to_string());
    py.import("music21")?
        .getattr("harmony")?
        .getattr("ChordSymbol")?
        .call1((figure,))
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add_function(wrap_pyfunction!(chordSymbolFigureFromChord, m)?)?;
    m.add_function(wrap_pyfunction!(chordSymbolFromChord, m)?)?;
    m.add("HarmonyException", py.get_type::<HarmonyException>())?;
    Ok(())
}
