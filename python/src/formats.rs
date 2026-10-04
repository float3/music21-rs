//! The score formats the crate reads, and MusicXML, which it also writes:
//! `from_musicxml`, `from_abc`, `from_abc_number`, `from_midi`,
//! `from_tiny_notation`, `from_humdrum`, `from_mei`, `from_roman_text` and
//! `to_musicxml`.
//!
//! These are the wheel's own rather than names music21 has: music21 reaches
//! its readers through `converter.parse` and its writer through an exporter
//! object, neither of which this wheel stands in for. Each reader hands back
//! the wheel's own streams, built by `stream::from_crate`.

use pyo3::prelude::*;

use music21_rs_crate::IntegerType;
use music21_rs_crate::musicxml::ExportOptions;
use music21_rs_crate::stream::Stream as RsStream;

use crate::stream::StreamException;

fn format_error(error: music21_rs_crate::Error) -> PyErr {
    StreamException::new_err(crate::pitch::message(&error))
}

/// The wheel's streams for what a reader read, or its error as a
/// `StreamException`.
fn handed_back<'py>(
    py: Python<'py>,
    read: music21_rs_crate::Result<RsStream>,
) -> PyResult<Bound<'py, PyAny>> {
    crate::stream::from_crate(py, &read.map_err(format_error)?)
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
    music21_rs_crate::musicxml::to_musicxml(&stream, &options).map_err(format_error)
}

/// Writes a score as a standard MIDI file and hands back its bytes, as
/// music21's `midi.translate.streamToMidiFile(score).writestr()` does.
///
/// The score may be one of this wheel's streams or one of music21's. A
/// score is a conductor track holding its tempos, meters and keys, then a
/// track per part; tied notes sound as one, each note at the velocity its
/// dynamic and articulations give it. Repeats are played out first, as
/// music21 plays them.
#[pyfunction]
#[pyo3(signature = (score, *, add_start_delay = false, add_end_delay = true, acceptable_channels = None))]
fn to_midi<'py>(
    py: Python<'py>,
    score: &Bound<'py, PyAny>,
    add_start_delay: bool,
    add_end_delay: bool,
    acceptable_channels: Option<Vec<u8>>,
) -> PyResult<Bound<'py, pyo3::types::PyBytes>> {
    let stream = crate::stream::crate_stream(score)?;
    let options = music21_rs_crate::midi::ExportOptions {
        add_start_delay,
        add_end_delay,
        acceptable_channels,
    };
    let bytes = music21_rs_crate::midi::to_midi(&stream, &options).map_err(format_error)?;
    Ok(pyo3::types::PyBytes::new(py, &bytes))
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
    handed_back(py, music21_rs_crate::musicxml::from_musicxml(text))
}

/// Reads ABC text as music21's `converter.parse` reads it.
///
/// A file holding one tune comes back as a `Score`, with a part for each of
/// its voices, holding measures where the tune has barlines. A file holding
/// several tunes, each under its own `X:` reference number, comes back as an
/// `Opus` of scores in the order of their numbers. Words, repeat marks,
/// barlines, slurs and the other spanners, and each tune's title and other
/// metadata are read and left out.
#[pyfunction]
fn from_abc<'py>(py: Python<'py>, text: &str) -> PyResult<Bound<'py, PyAny>> {
    handed_back(py, music21_rs_crate::abc::from_abc(text))
}

/// Reads the tune numbered `number` out of ABC text holding several, as
/// music21's `converter.parse(..., number=number)` does, as a `Score`.
///
/// What is left out is what `from_abc` leaves out.
#[pyfunction]
fn from_abc_number<'py>(
    py: Python<'py>,
    text: &str,
    number: IntegerType,
) -> PyResult<Bound<'py, PyAny>> {
    handed_back(py, music21_rs_crate::abc::from_abc_number(text, number))
}

/// Reads the bytes of a standard MIDI file as music21's `converter.parse`
/// reads them, as a `Score`.
///
/// Every track with notes is a part. Notes are moved onto the nearest
/// sixteenth or triplet eighth, gathered into chords and voices, cut into
/// measures by the meters the file states (`4/4` where it states none),
/// tied across barlines and filled out with rests. Channel 10 is read as
/// unpitched percussion, which this wheel has no class for: those strokes
/// are left out, and a drum track comes back holding its instrument,
/// measures, meters and rests alone. A file timed in frames a second, and a
/// format 2 file, are refused.
#[pyfunction]
fn from_midi<'py>(py: Python<'py>, data: &[u8]) -> PyResult<Bound<'py, PyAny>> {
    handed_back(py, music21_rs_crate::midi::from_midi(data))
}

/// Reads a line of TinyNotation, as music21's
/// `converter.parse('tinyNotation: ...')` does, as a `Part` of measures.
///
/// The text may start with `tinyNotation:`. A line with no meter is in
/// `4/4`, and a note running past a barline is left whole in the measure it
/// starts in.
#[pyfunction]
fn from_tiny_notation<'py>(py: Python<'py>, text: &str) -> PyResult<Bound<'py, PyAny>> {
    handed_back(py, music21_rs_crate::tinynotation::from_tiny_notation(text))
}

/// Reads a Humdrum file as music21's `converter.parse` reads one, as a
/// `Score`.
///
/// Every `**kern` spine is a part, the last in the file first, holding
/// measures, and a spine split in two is two voices in the measures where it
/// is split. The dynamics of a `**dynam` spine and the lyrics of a `**text`
/// spine are kept. A file holding several tables comes back as an `Opus` of
/// scores. Words, barlines and metadata are read and left out.
#[pyfunction]
fn from_humdrum<'py>(py: Python<'py>, text: &str) -> PyResult<Bound<'py, PyAny>> {
    handed_back(py, music21_rs_crate::humdrum::from_humdrum(text))
}

/// Reads the text of an MEI document as music21's `converter.parse` reads
/// one, as a `Score` of a part for each staff.
///
/// Words, barlines, slurs and the other spanners, and metadata are read and
/// left out.
#[pyfunction]
fn from_mei<'py>(py: Python<'py>, text: &str) -> PyResult<Bound<'py, PyAny>> {
    handed_back(py, music21_rs_crate::mei::from_mei(text))
}

/// Reads RomanText as music21's `converter.parse` reads it, as a `Score`.
///
/// The score holds one part of measures, and in each the chords the
/// numerals stand for, in the key they were written in, each carrying its
/// figure as its lyric -- the key too where it has just changed, as
/// `G: V7`. `NC` is a rest under a no-chord symbol. A file of several
/// `Movement:` lines comes back as an `Opus` of scores. The chords are
/// `Chord`s rather than music21's `RomanNumeral`s. The title, composer and
/// other metadata are read and left out.
#[pyfunction]
fn from_roman_text<'py>(py: Python<'py>, text: &str) -> PyResult<Bound<'py, PyAny>> {
    handed_back(py, music21_rs_crate::romantext::from_roman_text(text))
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(to_musicxml, m)?)?;
    m.add_function(wrap_pyfunction!(to_midi, m)?)?;
    m.add_function(wrap_pyfunction!(from_musicxml, m)?)?;
    m.add_function(wrap_pyfunction!(from_abc, m)?)?;
    m.add_function(wrap_pyfunction!(from_abc_number, m)?)?;
    m.add_function(wrap_pyfunction!(from_midi, m)?)?;
    m.add_function(wrap_pyfunction!(from_tiny_notation, m)?)?;
    m.add_function(wrap_pyfunction!(from_humdrum, m)?)?;
    m.add_function(wrap_pyfunction!(from_mei, m)?)?;
    m.add_function(wrap_pyfunction!(from_roman_text, m)?)?;
    Ok(())
}
