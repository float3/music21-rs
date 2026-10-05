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

/// Writes a score as ABC and hands back the tune's text.
///
/// The score may be one of this wheel's streams or one of music21's, which
/// has no ABC writer of its own. What is written reads back, with music21's
/// reader and this package's alike, as the score it was written from; what
/// ABC cannot say, such as a microtone or an unpitched stroke, raises
/// `StreamException` rather than being left out. `unit_length` is the `L:`
/// field as a fraction of a whole note, `(1, 8)` for an eighth, the one the
/// tune is shortest to write against where it is not given;
/// `measures_per_line` is how many measures go on a line.
#[pyfunction]
#[pyo3(signature = (score, *, unit_length = None, measures_per_line = 4))]
fn to_abc(
    score: &Bound<'_, PyAny>,
    unit_length: Option<(u32, u32)>,
    measures_per_line: usize,
) -> PyResult<String> {
    let stream = crate::stream::crate_stream(score)?;
    let options = music21_rs_crate::abc::ExportOptions {
        unit_length,
        measures_per_line,
    };
    music21_rs_crate::abc::to_abc(&stream, &options).map_err(format_error)
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

/// Writes a score's roman numerals as a RomanText analysis, as music21's
/// `romanText.writeRoman.RnWriter` writes one, and hands back the text.
///
/// The score may be one of this wheel's streams or one of music21's; its
/// `RomanNumeral`s are written measure by measure at their beats, with the
/// key where it changes, under a header of the composer, title, analyst and
/// proofreader.
#[pyfunction]
fn to_roman_text(score: &Bound<'_, PyAny>) -> PyResult<String> {
    let stream = crate::stream::crate_stream(score)?;
    music21_rs_crate::romantext::to_roman_text(&stream).map_err(format_error)
}

/// Reads a Capella score as music21's `converter.parse` reads a `.capx`, as
/// a `Score` of a part for each staff.
///
/// `data` is a `.capx` file's `bytes`, which are unpacked to the
/// `score.xml` they hold, or that document's text. Barlines are read and
/// left out, as music21's MusicXML writer leaves out a barline standing in
/// a measure.
#[pyfunction]
fn from_capella<'py>(py: Python<'py>, data: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let text: String = if let Ok(text) = data.extract::<String>() {
        text
    } else {
        let bytes: Vec<u8> = data.extract()?;
        if bytes.starts_with(b"PK") {
            let archive = py.import("zipfile")?.getattr("ZipFile")?.call1((py
                .import("io")?
                .getattr("BytesIO")?
                .call1((pyo3::types::PyBytes::new(py, &bytes),))?,))?;
            let inner: Vec<u8> = archive.call_method1("read", ("score.xml",))?.extract()?;
            String::from_utf8_lossy(&inner).into_owned()
        } else {
            String::from_utf8_lossy(&bytes).into_owned()
        }
    };
    handed_back(py, music21_rs_crate::capella::from_capella(&text))
}

/// Reads a MuseData work as music21's `converter.parse` reads one, as a
/// `Score` of a part for each part the files hold.
///
/// `texts` is the text of one file, or a list of the files' texts in the
/// order the work's parts go in. Barlines, articulations, ornaments and
/// fermatas, and the work's title are read and left out.
#[pyfunction]
fn from_musedata<'py>(py: Python<'py>, texts: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    use crate::Walkable;
    let texts: Vec<String> = if let Ok(text) = texts.extract::<String>() {
        vec![text]
    } else {
        texts
            .walk()?
            .map(|text| text.and_then(|text| text.extract::<String>()))
            .collect::<PyResult<_>>()?
    };
    let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
    handed_back(py, music21_rs_crate::musedata::from_musedata(&texts))
}

/// Reads the text of a NoteWorthy Composer `.nwctxt` file as music21's
/// `converter.parse` reads one, as a `Score` of a part per staff.
///
/// Words, barlines, repeat marks, slurs, endings, unpitched strokes and the
/// title are read and left out.
#[pyfunction]
fn from_noteworthy<'py>(py: Python<'py>, text: &str) -> PyResult<Bound<'py, PyAny>> {
    handed_back(py, music21_rs_crate::noteworthy::from_noteworthy(text))
}

/// Reads Volpiano, the chant font's notation, as music21's
/// `volpiano.toPart` reads it, as a `Part`.
///
/// Each note is a quarter note with no stem, a liquescent one with an `x`
/// notehead, in measures closed by the barlines the string writes. Breaks
/// and neumes are read and left out.
#[pyfunction]
fn from_volpiano<'py>(py: Python<'py>, text: &str) -> PyResult<Bound<'py, PyAny>> {
    handed_back(py, music21_rs_crate::volpiano::from_volpiano(text))
}

/// Writes a score as Volpiano, as music21's `volpiano.fromStream` writes
/// it.
///
/// The score may be one of this wheel's streams or one of music21's. Treble
/// and bass clefs, barlines, notes, flats and naturals on B and E, music21's
/// volpiano breaks and its neumes are written; what Volpiano cannot say is
/// left out, as music21 leaves it out.
#[pyfunction]
fn to_volpiano(score: &Bound<'_, PyAny>) -> PyResult<String> {
    let stream = crate::stream::crate_stream(score)?;
    music21_rs_crate::volpiano::to_volpiano(&stream).map_err(format_error)
}

/// Reads a NoteWorthy Composer `.nwc` file's `bytes` as music21's
/// `converter.parse` reads one, as a `Score` of a part per staff: the
/// objects written out as `.nwctxt` lines and those read as
/// `from_noteworthy` reads them. A compressed file is inflated first, as
/// music21 inflates one.
#[pyfunction]
fn from_nwc<'py>(py: Python<'py>, data: &[u8]) -> PyResult<Bound<'py, PyAny>> {
    let inflated: Vec<u8>;
    let data = if data.starts_with(b"[NWZ]\x00") {
        inflated = py
            .import("zlib")?
            .call_method1("decompress", (pyo3::types::PyBytes::new(py, &data[6..]),))?
            .extract()?;
        &inflated[..]
    } else {
        data
    };
    handed_back(py, music21_rs_crate::noteworthy::from_nwc(data))
}

/// Reads RomanText as music21's `converter.parse` reads it, as a `Score`.
///
/// The score holds one part of measures, and in each the chords the
/// numerals stand for, in the key they were written in, each a
/// `RomanNumeral` carrying its figure as its lyric -- the key too where it
/// has just changed, as `G: V7`. `NC` is a rest under a no-chord symbol. A
/// file of several `Movement:` lines comes back as an `Opus` of scores. The
/// title, composer and other metadata are read and left out.
#[pyfunction]
fn from_roman_text<'py>(py: Python<'py>, text: &str) -> PyResult<Bound<'py, PyAny>> {
    handed_back(py, music21_rs_crate::romantext::from_roman_text(text))
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(to_musicxml, m)?)?;
    m.add_function(wrap_pyfunction!(to_midi, m)?)?;
    m.add_function(wrap_pyfunction!(to_abc, m)?)?;
    m.add_function(wrap_pyfunction!(from_musicxml, m)?)?;
    m.add_function(wrap_pyfunction!(from_abc, m)?)?;
    m.add_function(wrap_pyfunction!(from_abc_number, m)?)?;
    m.add_function(wrap_pyfunction!(from_midi, m)?)?;
    m.add_function(wrap_pyfunction!(from_tiny_notation, m)?)?;
    m.add_function(wrap_pyfunction!(from_humdrum, m)?)?;
    m.add_function(wrap_pyfunction!(from_mei, m)?)?;
    m.add_function(wrap_pyfunction!(from_roman_text, m)?)?;
    m.add_function(wrap_pyfunction!(from_noteworthy, m)?)?;
    m.add_function(wrap_pyfunction!(from_musedata, m)?)?;
    m.add_function(wrap_pyfunction!(from_capella, m)?)?;
    m.add_function(wrap_pyfunction!(to_roman_text, m)?)?;
    m.add_function(wrap_pyfunction!(from_nwc, m)?)?;
    m.add_function(wrap_pyfunction!(from_volpiano, m)?)?;
    m.add_function(wrap_pyfunction!(to_volpiano, m)?)?;
    Ok(())
}
