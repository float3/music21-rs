//! music21's `braille.basic`: one element of music written in braille.
//!
//! Every transcription here is the crate's `braille::basic`, which
//! `braille_basic_parity` holds to music21 sign for sign; the facade reads
//! the object it is given as the value it is and hands the braille back.
//!
//! **music21 writes what each sign says onto the object it transcribed**,
//! as `editorial.brailleEnglish`, and its own examples read it back. The
//! crate answers that beside the braille (`Transcription::english`), so
//! the facade writes it onto the object music21 would have written it
//! onto -- the score's own, as music21 does.
//!
//! `handlePitchWithAccidental`, `handleArticulations` and
//! `handleExpressions` stay music21's: they are steps inside its own
//! `noteToBraille`, writing into the lists it is building, and the crate
//! takes the note whole.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyList;

use music21_rs_crate::braille::basic;
use music21_rs_crate::stream::StreamElement;
use music21_rs_crate::{IntegerType, Pitch};

/// The names this module hands out.
pub const NAMES: &[&str] = &[
    "barlineToBraille",
    "chordToBraille",
    "pitchToOctave",
    "clefToBraille",
    "dynamicToBraille",
    "instrumentToBraille",
    "keySigToBraille",
    "metronomeMarkToBraille",
    "yieldBrailleArticulations",
    "noteToBraille",
    "restToBraille",
    "tempoTextToBraille",
    "textExpressionToBraille",
    "timeSigToBraille",
    "showOctaveWithNote",
    "transcribeHeading",
    "transcribeNoteFingering",
    "transcribeSignatures",
    "brailleUnicodeToBrailleAscii",
    "brailleAsciiToBrailleUnicode",
    "brailleUnicodeToSymbols",
    "yieldDots",
    "wordToBraille",
    "numberToBraille",
];

pyo3::create_exception!(
    music21_rs_facade,
    BrailleBasicException,
    crate::Music21Exception
);

/// music21's error for what cannot be written in braille, out of the
/// crate's.
fn braille_error(error: &music21_rs_crate::Error) -> PyErr {
    BrailleBasicException::new_err(crate::pitch::message(error))
}

/// One object as the value it is, refused in this module's words where the
/// crate has no value for it.
fn value_of(object: &Bound<'_, PyAny>) -> PyResult<StreamElement> {
    crate::stream::element_read(object)?.ok_or_else(|| {
        BrailleBasicException::new_err(format!(
            "{} cannot be transcribed to braille.",
            object
                .repr()
                .map(|repr| repr.to_string())
                .unwrap_or_default()
        ))
    })
}

/// What a kind of element is called, for a refusal that names it.
fn refuse<T>(object: &Bound<'_, PyAny>, wanted: &str) -> PyResult<T> {
    Err(BrailleBasicException::new_err(format!(
        "{} is not a {wanted} this package can transcribe.",
        object
            .repr()
            .map(|repr| repr.to_string())
            .unwrap_or_default()
    )))
}

/// What each sign says, written onto the object music21 writes it onto:
/// `editorial.brailleEnglish`.
fn write_english(object: &Bound<'_, PyAny>, english: &[String]) -> PyResult<()> {
    let py = object.py();
    let listed = PyList::empty(py);
    for line in english {
        listed.append(line)?;
    }
    // An object with no editorial of its own is left alone, as nothing
    // music21 transcribes is without one.
    if let Ok(editorial) = object.getattr("editorial") {
        editorial.setattr("brailleEnglish", listed)?;
    }
    Ok(())
}

/// A transcription handed back: the braille, with its English written onto
/// the object it came from.
fn transcribed(
    object: &Bound<'_, PyAny>,
    transcription: &basic::Transcription,
) -> PyResult<String> {
    write_english(object, &transcription.english)?;
    Ok(transcription.braille.clone())
}

/// The sharps a key signature is written with, or nothing where it is one
/// of music21's non-traditional signatures, which has no count.
fn sharps_of(object: &Bound<'_, PyAny>) -> PyResult<Option<IntegerType>> {
    if object
        .getattr("isNonTraditional")
        .and_then(|flag| flag.is_truthy())
        .unwrap_or(false)
    {
        return Ok(None);
    }
    match value_of(object)? {
        StreamElement::KeySignature(key) => Ok(key.sharps()),
        StreamElement::Key(key) => Ok(key.key_signature().sharps()),
        _ => refuse(object, "key signature"),
    }
}

/// The pitch of a note, or the pitch itself.
fn pitch_of(object: &Bound<'_, PyAny>) -> PyResult<Pitch> {
    if let Ok(pitch) = object.extract::<PyRef<'_, crate::pitch::Pitch>>() {
        return Ok(pitch.inner.clone());
    }
    if let Ok(held) = object.getattr("pitch")
        && let Ok(pitch) = held.extract::<PyRef<'_, crate::pitch::Pitch>>()
    {
        return Ok(pitch.inner.clone());
    }
    match value_of(object)? {
        StreamElement::Note(note) => Ok(note.pitch().clone()),
        _ => refuse(object, "note"),
    }
}

/// music21's two generators here are generators, and one of its own
/// examples prints one (`<generator object yieldDots at 0x...>`), so what
/// is handed back must be a generator of that name rather than an iterator
/// over a list. These two yield what the crate answered.
const GENERATORS: &str = r#"
def yieldDots(rows):
    """Each row of a cell whose left dot is filled."""
    yield from rows


def yieldBrailleArticulations(signs):
    """The signs written before a note."""
    yield from signs
"#;

/// The two built once.
static GENERATOR_MODULE: pyo3::sync::PyOnceLock<Py<PyModule>> = pyo3::sync::PyOnceLock::new();

/// What the crate answered, handed back as the generator music21 hands
/// back.
fn yielded<'py>(py: Python<'py>, named: &str, rows: Vec<String>) -> PyResult<Bound<'py, PyAny>> {
    let module = GENERATOR_MODULE.get_or_try_init(py, || -> PyResult<Py<PyModule>> {
        Ok(PyModule::from_code(
            py,
            &std::ffi::CString::new(GENERATORS)?,
            c"music21_rs_braille_generators.py",
            c"music21_rs_braille_generators",
        )?
        .unbind())
    })?;
    let listed = PyList::empty(py);
    for row in rows {
        listed.append(row)?;
    }
    module.bind(py).getattr(named)?.call1((listed,))
}

/// music21's `barlineToBraille`.
#[pyfunction]
fn barlineToBraille(music21Barline: &Bound<'_, PyAny>) -> PyResult<String> {
    match value_of(music21Barline)? {
        StreamElement::Barline(barline) => {
            transcribed(music21Barline, &basic::barline_to_braille(&barline))
        }
        _ => refuse(music21Barline, "barline"),
    }
}

/// music21's `chordToBraille`: one pitch of the chord written out, the
/// rest as the intervals from it.
#[pyfunction]
#[pyo3(signature = (music21Chord, descending = true, showOctave = true))]
fn chordToBraille(
    music21Chord: &Bound<'_, PyAny>,
    descending: bool,
    showOctave: bool,
) -> PyResult<String> {
    match value_of(music21Chord)? {
        StreamElement::Chord(chord) => transcribed(
            music21Chord,
            &basic::chord_to_braille(&chord, descending, showOctave),
        ),
        _ => refuse(music21Chord, "chord"),
    }
}

/// music21's `pitchToOctave`, which takes a pitch or the octave itself.
#[pyfunction]
fn pitchToOctave(music21PitchOrInt: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Ok(octave) = music21PitchOrInt.extract::<IntegerType>() {
        return Ok(basic::pitch_to_octave(octave));
    }
    let pitch = pitch_of(music21PitchOrInt)?;
    Ok(basic::pitch_to_octave(pitch.implicit_octave()))
}

/// music21's `clefToBraille`.
#[pyfunction]
#[pyo3(signature = (music21Clef, keyboardHandSwitched = false))]
fn clefToBraille(music21Clef: &Bound<'_, PyAny>, keyboardHandSwitched: bool) -> PyResult<String> {
    match value_of(music21Clef)? {
        StreamElement::Clef(clef) => transcribed(
            music21Clef,
            &basic::clef_to_braille(&clef, keyboardHandSwitched),
        ),
        _ => refuse(music21Clef, "clef"),
    }
}

/// music21's `dynamicToBraille`.
#[pyfunction]
#[pyo3(signature = (music21Dynamic, precedeByWordSign = true))]
fn dynamicToBraille(
    music21Dynamic: &Bound<'_, PyAny>,
    precedeByWordSign: bool,
) -> PyResult<String> {
    match value_of(music21Dynamic)? {
        StreamElement::Dynamic(dynamic) => transcribed(
            music21Dynamic,
            &basic::dynamic_to_braille(&dynamic, precedeByWordSign),
        ),
        _ => refuse(music21Dynamic, "dynamic"),
    }
}

/// music21's `instrumentToBraille`.
#[pyfunction]
fn instrumentToBraille(music21Instrument: &Bound<'_, PyAny>) -> PyResult<String> {
    match value_of(music21Instrument)? {
        StreamElement::Instrument(instrument) => {
            let written =
                basic::instrument_to_braille(&instrument).map_err(|error| braille_error(&error))?;
            transcribed(music21Instrument, &written)
        }
        _ => refuse(music21Instrument, "instrument"),
    }
}

/// music21's `keySigToBraille`, which writes the naturals cancelling an
/// outgoing signature before the new one.
#[pyfunction]
#[pyo3(signature = (music21KeySignature, outgoingKeySig = None))]
fn keySigToBraille(
    music21KeySignature: &Bound<'_, PyAny>,
    outgoingKeySig: Option<&Bound<'_, PyAny>>,
) -> PyResult<String> {
    let sharps = sharps_of(music21KeySignature)?;
    let outgoing = match outgoingKeySig.filter(|given| !given.is_none()) {
        Some(given) => Some(sharps_of(given)?),
        None => None,
    };
    transcribed(
        music21KeySignature,
        &basic::key_sig_to_braille(sharps, outgoing),
    )
}

/// music21's `metronomeMarkToBraille`.
#[pyfunction]
fn metronomeMarkToBraille(music21MetronomeMark: &Bound<'_, PyAny>) -> PyResult<String> {
    match value_of(music21MetronomeMark)? {
        // A mark that says no number is written as nothing at all, as
        // music21 writes it -- a mark that only *sounds* a tempo is one.
        StreamElement::MetronomeMark(mark) => match basic::metronome_mark_to_braille(&mark) {
            Some(written) => transcribed(music21MetronomeMark, &written),
            None => Ok(String::new()),
        },
        _ => refuse(music21MetronomeMark, "metronome mark"),
    }
}

/// music21's `yieldBrailleArticulations`: the signs written before a note,
/// bowings first, then staccatos, then the rest by name.
#[pyfunction]
fn yieldBrailleArticulations<'py>(
    py: Python<'py>,
    noteEl: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let note = match value_of(noteEl)? {
        StreamElement::Note(note) => note,
        _ => return refuse(noteEl, "note"),
    };
    let signs = basic::braille_articulations(&note)
        .into_iter()
        .map(|(_, sign)| sign)
        .collect();
    yielded(py, "yieldBrailleArticulations", signs)
}

/// How a note stands among its neighbours, which a segment works out and
/// writes onto the note for the time it is transcribed.
fn note_context(object: &Bound<'_, PyAny>) -> basic::NoteContext {
    let said = |name: &str| {
        object
            .getattr(name)
            .and_then(|flag| flag.is_truthy())
            .unwrap_or(false)
    };
    basic::NoteContext {
        begin_long_bracket_slur: said("beginLongBracketSlur"),
        end_long_bracket_slur: said("endLongBracketSlur"),
        begin_long_double_slur: said("beginLongDoubleSlur"),
        end_long_double_slur: said("endLongDoubleSlur"),
        short_slur: said("shortSlur"),
        beam_start: said("beamStart"),
        beam_continue: said("beamContinue"),
    }
}

/// music21's `noteToBraille`.
#[pyfunction]
#[pyo3(signature = (music21Note, *, showOctave = true, upperFirstInFingering = true))]
fn noteToBraille(
    music21Note: &Bound<'_, PyAny>,
    showOctave: bool,
    upperFirstInFingering: bool,
) -> PyResult<String> {
    match value_of(music21Note)? {
        StreamElement::Note(note) => transcribed(
            music21Note,
            &basic::note_to_braille(
                &note,
                showOctave,
                upperFirstInFingering,
                note_context(music21Note),
            ),
        ),
        _ => refuse(music21Note, "note"),
    }
}

/// music21's `restToBraille`.
#[pyfunction]
fn restToBraille(music21Rest: &Bound<'_, PyAny>) -> PyResult<String> {
    match value_of(music21Rest)? {
        StreamElement::Rest(rest) => transcribed(music21Rest, &basic::rest_to_braille(&rest)),
        _ => refuse(music21Rest, "rest"),
    }
}

/// music21's `tempoTextToBraille`, which splits the words at each comma
/// and ends them with the literary period braille asks for.
#[pyfunction]
#[pyo3(signature = (music21TempoText, *, maxLineLength = 40))]
fn tempoTextToBraille(
    music21TempoText: &Bound<'_, PyAny>,
    maxLineLength: usize,
) -> PyResult<String> {
    match value_of(music21TempoText)? {
        StreamElement::TempoText(text) => transcribed(
            music21TempoText,
            &basic::tempo_to_braille(&text, maxLineLength),
        ),
        _ => refuse(music21TempoText, "tempo text"),
    }
}

/// music21's `textExpressionToBraille`.
#[pyfunction]
#[pyo3(signature = (music21TextExpression, precedeByWordSign = true))]
fn textExpressionToBraille(
    music21TextExpression: &Bound<'_, PyAny>,
    precedeByWordSign: bool,
) -> PyResult<String> {
    match value_of(music21TextExpression)? {
        StreamElement::TextExpression(text) => transcribed(
            music21TextExpression,
            &basic::text_expression_to_braille(&text, precedeByWordSign),
        ),
        _ => refuse(music21TextExpression, "text expression"),
    }
}

/// music21's `timeSigToBraille`.
#[pyfunction]
fn timeSigToBraille(m21TimeSignature: &Bound<'_, PyAny>) -> PyResult<String> {
    match value_of(m21TimeSignature)? {
        StreamElement::TimeSignature(meter) => {
            transcribed(m21TimeSignature, &basic::time_sig_to_braille(&meter))
        }
        _ => refuse(m21TimeSignature, "time signature"),
    }
}

/// music21's `showOctaveWithNote`: whether a note carries an octave mark
/// after the one before it.
#[pyfunction]
fn showOctaveWithNote(
    previousNote: Option<&Bound<'_, PyAny>>,
    currentNote: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    let previous = match previousNote.filter(|given| !given.is_none()) {
        Some(given) => Some(pitch_of(given)?),
        None => None,
    };
    let current = pitch_of(currentNote)?;
    Ok(basic::show_octave_with_note(previous.as_ref(), &current))
}

/// music21's `transcribeHeading`: the tempo words, the metronome mark and
/// the signatures, each line centred.
#[pyfunction]
#[pyo3(signature = (
    music21KeySignature = None,
    music21TimeSignature = None,
    music21TempoText = None,
    music21MetronomeMark = None,
    maxLineLength = 40
))]
fn transcribeHeading(
    music21KeySignature: Option<&Bound<'_, PyAny>>,
    music21TimeSignature: Option<&Bound<'_, PyAny>>,
    music21TempoText: Option<&Bound<'_, PyAny>>,
    music21MetronomeMark: Option<&Bound<'_, PyAny>>,
    maxLineLength: usize,
) -> PyResult<String> {
    let sharps = match music21KeySignature.filter(|given| !given.is_none()) {
        Some(given) => Some(sharps_of(given)?),
        None => None,
    };
    let meter = match music21TimeSignature.filter(|given| !given.is_none()) {
        Some(given) => match value_of(given)? {
            StreamElement::TimeSignature(meter) => Some(meter),
            _ => return refuse(given, "time signature"),
        },
        None => None,
    };
    let tempo = match music21TempoText.filter(|given| !given.is_none()) {
        Some(given) => match value_of(given)? {
            StreamElement::TempoText(text) => Some(text),
            _ => return refuse(given, "tempo text"),
        },
        None => None,
    };
    let metronome = match music21MetronomeMark.filter(|given| !given.is_none()) {
        Some(given) => match value_of(given)? {
            StreamElement::MetronomeMark(mark) => Some(mark),
            _ => return refuse(given, "metronome mark"),
        },
        None => None,
    };
    basic::transcribe_heading(
        sharps,
        meter.as_ref(),
        tempo.as_ref(),
        metronome.as_ref(),
        maxLineLength,
    )
    .map_err(|error| braille_error(&error))
}

/// music21's `transcribeNoteFingering`.
#[pyfunction]
#[pyo3(signature = (sampleNoteFingering = "1".to_string(), upperFirstInFingering = true))]
fn transcribeNoteFingering(
    sampleNoteFingering: String,
    upperFirstInFingering: bool,
) -> PyResult<String> {
    basic::transcribe_note_fingering(&sampleNoteFingering, upperFirstInFingering)
        .map_err(|error| braille_error(&error))
}

/// music21's `transcribeSignatures`.
#[pyfunction]
#[pyo3(signature = (music21KeySignature, music21TimeSignature, outgoingKeySig = None))]
fn transcribeSignatures(
    music21KeySignature: Option<&Bound<'_, PyAny>>,
    music21TimeSignature: Option<&Bound<'_, PyAny>>,
    outgoingKeySig: Option<&Bound<'_, PyAny>>,
) -> PyResult<String> {
    let sharps = match music21KeySignature.filter(|given| !given.is_none()) {
        Some(given) => Some(sharps_of(given)?),
        None => None,
    };
    let meter = match music21TimeSignature.filter(|given| !given.is_none()) {
        Some(given) => match value_of(given)? {
            StreamElement::TimeSignature(meter) => Some(meter),
            _ => return refuse(given, "time signature"),
        },
        None => None,
    };
    let outgoing = match outgoingKeySig.filter(|given| !given.is_none()) {
        Some(given) => Some(sharps_of(given)?),
        None => None,
    };
    Ok(basic::transcribe_signatures(sharps, meter.as_ref(), outgoing).braille)
}

/// music21's `brailleUnicodeToBrailleAscii`.
#[pyfunction]
fn brailleUnicodeToBrailleAscii(brailleUnicode: &str) -> PyResult<String> {
    basic::braille_unicode_to_braille_ascii(brailleUnicode).map_err(|error| braille_error(&error))
}

/// music21's `brailleAsciiToBrailleUnicode`.
#[pyfunction]
fn brailleAsciiToBrailleUnicode(brailleAscii: &str) -> PyResult<String> {
    basic::braille_ascii_to_braille_unicode(brailleAscii).map_err(|error| braille_error(&error))
}

/// music21's `brailleUnicodeToSymbols`: each cell drawn as its dots.
#[pyfunction]
#[pyo3(signature = (brailleUnicode, filledSymbol = "o".to_string(), emptySymbol = "\u{00B7}".to_string()))]
fn brailleUnicodeToSymbols(
    brailleUnicode: &str,
    filledSymbol: String,
    emptySymbol: String,
) -> PyResult<String> {
    basic::braille_unicode_to_symbols(brailleUnicode, &filledSymbol, &emptySymbol)
        .map_err(|error| braille_error(&error))
}

/// music21's `yieldDots`: the rows of one cell, drawn.
#[pyfunction]
fn yieldDots<'py>(py: Python<'py>, brailleCharacter: &str) -> PyResult<Bound<'py, PyAny>> {
    let cell = brailleCharacter
        .chars()
        .next()
        .ok_or_else(|| BrailleBasicException::new_err("a cell is one character"))?;
    yielded(py, "yieldDots", basic::yield_dots(cell))
}

/// music21's `wordToBraille`.
#[pyfunction]
#[pyo3(signature = (sampleWord, isTextExpression = false))]
fn wordToBraille(sampleWord: &str, isTextExpression: bool) -> PyResult<String> {
    basic::word_to_braille(sampleWord, isTextExpression).map_err(|error| braille_error(&error))
}

/// music21's `numberToBraille`.
#[pyfunction]
#[pyo3(signature = (sampleNumber, withNumberSign = true, lower = false))]
fn numberToBraille(
    sampleNumber: &Bound<'_, PyAny>,
    withNumberSign: bool,
    lower: bool,
) -> PyResult<String> {
    // music21 takes the number itself, and its own code hands this one
    // strings as readily as integers.
    let written = sampleNumber.str()?.to_string();
    basic::number_to_braille(&written, withNumberSign, lower).map_err(|error| braille_error(&error))
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(barlineToBraille, m)?)?;
    m.add_function(wrap_pyfunction!(chordToBraille, m)?)?;
    m.add_function(wrap_pyfunction!(pitchToOctave, m)?)?;
    m.add_function(wrap_pyfunction!(clefToBraille, m)?)?;
    m.add_function(wrap_pyfunction!(dynamicToBraille, m)?)?;
    m.add_function(wrap_pyfunction!(instrumentToBraille, m)?)?;
    m.add_function(wrap_pyfunction!(keySigToBraille, m)?)?;
    m.add_function(wrap_pyfunction!(metronomeMarkToBraille, m)?)?;
    m.add_function(wrap_pyfunction!(yieldBrailleArticulations, m)?)?;
    m.add_function(wrap_pyfunction!(noteToBraille, m)?)?;
    m.add_function(wrap_pyfunction!(restToBraille, m)?)?;
    m.add_function(wrap_pyfunction!(tempoTextToBraille, m)?)?;
    m.add_function(wrap_pyfunction!(textExpressionToBraille, m)?)?;
    m.add_function(wrap_pyfunction!(timeSigToBraille, m)?)?;
    m.add_function(wrap_pyfunction!(showOctaveWithNote, m)?)?;
    m.add_function(wrap_pyfunction!(transcribeHeading, m)?)?;
    m.add_function(wrap_pyfunction!(transcribeNoteFingering, m)?)?;
    m.add_function(wrap_pyfunction!(transcribeSignatures, m)?)?;
    m.add_function(wrap_pyfunction!(brailleUnicodeToBrailleAscii, m)?)?;
    m.add_function(wrap_pyfunction!(brailleAsciiToBrailleUnicode, m)?)?;
    m.add_function(wrap_pyfunction!(brailleUnicodeToSymbols, m)?)?;
    m.add_function(wrap_pyfunction!(yieldDots, m)?)?;
    m.add_function(wrap_pyfunction!(wordToBraille, m)?)?;
    m.add_function(wrap_pyfunction!(numberToBraille, m)?)?;
    Ok(())
}
