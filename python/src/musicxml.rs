//! MusicXML, read and written by the crate: `from_musicxml` and
//! `to_musicxml`.
//!
//! These two are the wheel's own rather than names music21 has: music21
//! reaches its reader through `converter.parse` and its writer through an
//! exporter object, neither of which this wheel stands in for.

use pyo3::prelude::*;

use music21_rs_crate::musicxml::ExportOptions;

use crate::stream::StreamException;

fn musicxml_error(error: music21_rs_crate::Error) -> PyErr {
    StreamException::new_err(crate::pitch::message(&error))
}

/// Writes a score as MusicXML and hands back the document's text.
///
/// The score may be one of this wheel's streams or one of music21's, and is
/// written as it stands, as music21's own exporter writes one with
/// `makeNotation=False`: what it holds has to be in measures already.
///
/// With `make_notation=True` the notation the score leaves unsaid is worked
/// out first, as music21's exporter does by default, and what is written
/// need not be a score in measures: a part of loose notes is cut into
/// measures, gaps are filled with rests that are not printed, notes running
/// past a barline are cut and tied, accidentals are decided, notes are
/// beamed and tuplets bracketed where the part has none yet, and lengths no
/// single note value writes are cut into tied values. The score handed in
/// is not changed.
///
/// `encoding_date` is the date the document says it was written on, as
/// `YYYY-MM-DD`, left out where none is given; `software` is what a score
/// with no metadata is signed with.
#[pyfunction]
#[pyo3(signature = (score, *, encoding_date = None, software = None, make_notation = false))]
fn to_musicxml(
    score: &Bound<'_, PyAny>,
    encoding_date: Option<String>,
    software: Option<String>,
    make_notation: bool,
) -> PyResult<String> {
    let stream = crate::stream::crate_stream(score)?;
    let mut options = ExportOptions {
        encoding_date,
        make_notation,
        ..ExportOptions::default()
    };
    if let Some(software) = software {
        options.software = software;
    }
    music21_rs_crate::musicxml::to_musicxml(&stream, &options).map_err(musicxml_error)
}

/// Reads the text of a MusicXML document into a `Score` of this wheel's
/// streams: parts, or a part for each staff of one written on several,
/// holding measures, holding the notes, chords, rests, clefs, keys, meters,
/// tempo marks, dynamics, chord symbols and instruments read, a voice
/// apiece where a measure has more than one.
///
/// It reads what music21's reader reads, and hands back what this wheel has
/// classes for: words, repeat marks, unpitched strokes, barlines, slurs and
/// the other spanners, and the score's metadata are read and left out. A
/// compressed `.mxl` is a zip holding the document; unpack it first.
#[pyfunction]
fn from_musicxml<'py>(py: Python<'py>, text: &str) -> PyResult<Bound<'py, PyAny>> {
    let score = music21_rs_crate::musicxml::from_musicxml(text).map_err(musicxml_error)?;
    crate::stream::from_crate(py, &score)
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(to_musicxml, m)?)?;
    m.add_function(wrap_pyfunction!(from_musicxml, m)?)?;
    Ok(())
}
