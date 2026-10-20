//! music21's `braille.translate`: a score, a part or a measure written in
//! braille.
//!
//! The writing is the crate's `braille::translate`, which `braille_parity`
//! holds to music21 on built parts and corpus scores; the facade reads the
//! stream it is given, turns music21's keywords into the crate's options
//! and hands the braille back.
//!
//! music21's `segmentBreaks` is refused rather than passed over: it says
//! where a segment is to be broken by hand, which the crate does not take,
//! and a caller asking for one would otherwise be given braille broken
//! somewhere else.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;

use music21_rs_crate::braille::segment::{Hand, SegmentOptions};
use music21_rs_crate::braille::translate;
use music21_rs_crate::stream::Stream as RsStream;

/// The names this module hands out.
pub const NAMES: &[&str] = &[
    "objectToBraille",
    "streamToBraille",
    "scoreToBraille",
    "metadataToString",
    "opusToBraille",
    "measureToBraille",
    "partToBraille",
    "keyboardPartsToBraille",
];

pyo3::create_exception!(
    music21_rs_facade,
    BrailleTranslateException,
    crate::Music21Exception
);

/// music21's error for music it cannot write in braille, out of the
/// crate's.
fn translate_error(error: &music21_rs_crate::Error) -> PyErr {
    BrailleTranslateException::new_err(crate::pitch::message(error))
}

/// Which hand's sign the lines start with, as music21 names it.
fn hand_of(showHand: Option<&str>) -> PyResult<Option<Hand>> {
    Ok(match showHand {
        None => None,
        Some("right") => Some(Hand::Right),
        Some("left") => Some(Hand::Left),
        Some(other) => {
            return Err(BrailleTranslateException::new_err(format!(
                "{other} is not a hand braille knows"
            )));
        }
    })
}

/// Every keyword music21's translations take, as the crate's options.
///
/// The arguments are music21's own, in music21's order, so that a reader
/// can set the two side by side.
#[allow(clippy::too_many_arguments)]
fn options_of(
    inPlace: bool,
    debug: bool,
    cancelOutgoingKeySig: bool,
    descendingChords: Option<bool>,
    dummyRestLength: Option<usize>,
    maxLineLength: usize,
    segmentBreaks: Option<&Bound<'_, PyAny>>,
    showClefSigns: bool,
    showFirstMeasureNumber: bool,
    showHand: Option<&str>,
    showHeading: bool,
    showLongSlursAndTiesTogether: Option<bool>,
    showShortSlursAndTiesTogether: bool,
    slurLongPhraseWithBrackets: bool,
    suppressOctaveMarks: bool,
    upperFirstInNoteFingering: bool,
) -> PyResult<translate::BrailleOptions> {
    if segmentBreaks.is_some_and(|given| !given.is_none()) {
        return Err(BrailleTranslateException::new_err(
            "segmentBreaks says where to break a segment by hand, which this package does not take",
        ));
    }
    Ok(translate::BrailleOptions {
        in_place: inPlace,
        debug,
        segment: SegmentOptions {
            cancel_outgoing_key_sig: cancelOutgoingKeySig,
            descending_chords: descendingChords,
            dummy_rest_length: dummyRestLength,
            max_line_length: maxLineLength,
            show_clef_signs: showClefSigns,
            show_first_measure_number: showFirstMeasureNumber,
            show_hand: hand_of(showHand)?,
            show_heading: showHeading,
            show_long_slurs_and_ties_together: showLongSlursAndTiesTogether,
            show_short_slurs_and_ties_together: showShortSlursAndTiesTogether,
            slur_long_phrase_with_brackets: slurLongPhraseWithBrackets,
            suppress_octave_marks: suppressOctaveMarks,
            upper_first_in_note_fingering: upperFirstInNoteFingering,
        },
    })
}

/// The stream as the crate reads one, with every element as the value it
/// is: braille writes the music itself, slurs and all.
fn stream_of(object: &Bound<'_, PyAny>) -> PyResult<RsStream> {
    crate::stream::crate_stream(object)
}

/// music21's translations, each taking the same keywords: the crate's
/// function for each, over the stream read in.
macro_rules! translations {
    ($(($function:ident, $run:expr, $what:literal)),* $(,)?) => {
        $(
            #[doc = concat!("music21's `", stringify!($function), "`: ", $what, ".")]
            #[pyfunction]
            #[pyo3(signature = (
                music21Stream,
                *,
                inPlace = false,
                debug = false,
                cancelOutgoingKeySig = true,
                descendingChords = None,
                dummyRestLength = None,
                maxLineLength = 40,
                segmentBreaks = None,
                showClefSigns = false,
                showFirstMeasureNumber = true,
                showHand = None,
                showHeading = true,
                showLongSlursAndTiesTogether = None,
                showShortSlursAndTiesTogether = false,
                slurLongPhraseWithBrackets = true,
                suppressOctaveMarks = false,
                upperFirstInNoteFingering = true
            ))]
            #[allow(clippy::too_many_arguments)]
            fn $function(
                music21Stream: &Bound<'_, PyAny>,
                inPlace: bool,
                debug: bool,
                cancelOutgoingKeySig: bool,
                descendingChords: Option<bool>,
                dummyRestLength: Option<usize>,
                maxLineLength: usize,
                segmentBreaks: Option<&Bound<'_, PyAny>>,
                showClefSigns: bool,
                showFirstMeasureNumber: bool,
                showHand: Option<&str>,
                showHeading: bool,
                showLongSlursAndTiesTogether: Option<bool>,
                showShortSlursAndTiesTogether: bool,
                slurLongPhraseWithBrackets: bool,
                suppressOctaveMarks: bool,
                upperFirstInNoteFingering: bool,
            ) -> PyResult<String> {
                let options = options_of(
                    inPlace,
                    debug,
                    cancelOutgoingKeySig,
                    descendingChords,
                    dummyRestLength,
                    maxLineLength,
                    segmentBreaks,
                    showClefSigns,
                    showFirstMeasureNumber,
                    showHand,
                    showHeading,
                    showLongSlursAndTiesTogether,
                    showShortSlursAndTiesTogether,
                    slurLongPhraseWithBrackets,
                    suppressOctaveMarks,
                    upperFirstInNoteFingering,
                )?;
                let run: fn(&RsStream, &translate::BrailleOptions)
                    -> music21_rs_crate::error::Result<String> = $run;
                let stream = stream_of(music21Stream)?;
                run(&stream, &options).map_err(|error| translate_error(&error))
            }
        )*
    };
}

translations![
    (
        streamToBraille,
        translate::stream_to_braille,
        "a measure, a part, a score or an opus in braille"
    ),
    (
        scoreToBraille,
        translate::score_to_braille,
        "a score in braille, part by part"
    ),
    (
        opusToBraille,
        translate::stream_to_braille,
        "an opus in braille, score by score"
    ),
    (
        partToBraille,
        translate::part_to_braille,
        "a part in braille, segment by segment"
    ),
];

/// music21's `measureToBraille`: one measure in braille.
///
/// Its defaults are not the others': music21 writes no heading and no
/// measure number for a lone measure, and says so in its own source
/// ("observe False!").
#[pyfunction]
#[pyo3(signature = (
    music21Measure,
    *,
    inPlace = false,
    debug = false,
    cancelOutgoingKeySig = true,
    descendingChords = None,
    dummyRestLength = None,
    maxLineLength = 40,
    segmentBreaks = None,
    showClefSigns = false,
    showFirstMeasureNumber = false,
    showHand = None,
    showHeading = false,
    showLongSlursAndTiesTogether = None,
    showShortSlursAndTiesTogether = false,
    slurLongPhraseWithBrackets = true,
    suppressOctaveMarks = false,
    upperFirstInNoteFingering = true
))]
#[allow(clippy::too_many_arguments)]
fn measureToBraille(
    music21Measure: &Bound<'_, PyAny>,
    inPlace: bool,
    debug: bool,
    cancelOutgoingKeySig: bool,
    descendingChords: Option<bool>,
    dummyRestLength: Option<usize>,
    maxLineLength: usize,
    segmentBreaks: Option<&Bound<'_, PyAny>>,
    showClefSigns: bool,
    showFirstMeasureNumber: bool,
    showHand: Option<&str>,
    showHeading: bool,
    showLongSlursAndTiesTogether: Option<bool>,
    showShortSlursAndTiesTogether: bool,
    slurLongPhraseWithBrackets: bool,
    suppressOctaveMarks: bool,
    upperFirstInNoteFingering: bool,
) -> PyResult<String> {
    let options = options_of(
        inPlace,
        debug,
        cancelOutgoingKeySig,
        descendingChords,
        dummyRestLength,
        maxLineLength,
        segmentBreaks,
        showClefSigns,
        showFirstMeasureNumber,
        showHand,
        showHeading,
        showLongSlursAndTiesTogether,
        showShortSlursAndTiesTogether,
        slurLongPhraseWithBrackets,
        suppressOctaveMarks,
        upperFirstInNoteFingering,
    )?;
    let measure = stream_of(music21Measure)?;
    translate::measure_to_braille(&measure, &options).map_err(|error| translate_error(&error))
}

/// music21's `keyboardPartsToBraille`: the two staves of a keyboard score
/// in braille, bar over bar.
///
/// music21 takes the score and finds its two staves; the crate takes the
/// staves, so they are found here -- and a score that has not exactly two
/// is refused in music21's words.
#[pyfunction]
#[pyo3(signature = (
    keyboardScore,
    *,
    inPlace = false,
    debug = false,
    cancelOutgoingKeySig = true,
    descendingChords = None,
    dummyRestLength = None,
    maxLineLength = 40,
    segmentBreaks = None,
    showClefSigns = false,
    showFirstMeasureNumber = true,
    showHand = None,
    showHeading = true,
    showLongSlursAndTiesTogether = None,
    showShortSlursAndTiesTogether = false,
    slurLongPhraseWithBrackets = true,
    suppressOctaveMarks = false,
    upperFirstInNoteFingering = true
))]
#[allow(clippy::too_many_arguments)]
fn keyboardPartsToBraille(
    keyboardScore: &Bound<'_, PyAny>,
    inPlace: bool,
    debug: bool,
    cancelOutgoingKeySig: bool,
    descendingChords: Option<bool>,
    dummyRestLength: Option<usize>,
    maxLineLength: usize,
    segmentBreaks: Option<&Bound<'_, PyAny>>,
    showClefSigns: bool,
    showFirstMeasureNumber: bool,
    showHand: Option<&str>,
    showHeading: bool,
    showLongSlursAndTiesTogether: Option<bool>,
    showShortSlursAndTiesTogether: bool,
    slurLongPhraseWithBrackets: bool,
    suppressOctaveMarks: bool,
    upperFirstInNoteFingering: bool,
) -> PyResult<String> {
    let options = options_of(
        inPlace,
        debug,
        cancelOutgoingKeySig,
        descendingChords,
        dummyRestLength,
        maxLineLength,
        segmentBreaks,
        showClefSigns,
        showFirstMeasureNumber,
        showHand,
        showHeading,
        showLongSlursAndTiesTogether,
        showShortSlursAndTiesTogether,
        slurLongPhraseWithBrackets,
        suppressOctaveMarks,
        upperFirstInNoteFingering,
    )?;
    let score = stream_of(keyboardScore)?;
    let staves = score.parts();
    let [upper, lower] = staves.as_slice() else {
        return Err(BrailleTranslateException::new_err(
            "Can only translate two keyboard parts at a time",
        ));
    };
    translate::keyboard_parts_to_braille(upper, lower, &options)
        .map_err(|error| translate_error(&error))
}

/// music21's `objectToBraille`: a stream written as a stream, and anything
/// else as the one element it is.
#[pyfunction]
#[pyo3(signature = (
    music21Obj,
    *,
    inPlace = false,
    debug = false,
    cancelOutgoingKeySig = true,
    descendingChords = None,
    dummyRestLength = None,
    maxLineLength = 40,
    segmentBreaks = None,
    showClefSigns = false,
    showFirstMeasureNumber = true,
    showHand = None,
    showHeading = true,
    showLongSlursAndTiesTogether = None,
    showShortSlursAndTiesTogether = false,
    slurLongPhraseWithBrackets = true,
    suppressOctaveMarks = false,
    upperFirstInNoteFingering = true
))]
#[allow(clippy::too_many_arguments)]
fn objectToBraille(
    music21Obj: &Bound<'_, PyAny>,
    inPlace: bool,
    debug: bool,
    cancelOutgoingKeySig: bool,
    descendingChords: Option<bool>,
    dummyRestLength: Option<usize>,
    maxLineLength: usize,
    segmentBreaks: Option<&Bound<'_, PyAny>>,
    showClefSigns: bool,
    showFirstMeasureNumber: bool,
    showHand: Option<&str>,
    showHeading: bool,
    showLongSlursAndTiesTogether: Option<bool>,
    showShortSlursAndTiesTogether: bool,
    slurLongPhraseWithBrackets: bool,
    suppressOctaveMarks: bool,
    upperFirstInNoteFingering: bool,
) -> PyResult<String> {
    let options = options_of(
        inPlace,
        debug,
        cancelOutgoingKeySig,
        descendingChords,
        dummyRestLength,
        maxLineLength,
        segmentBreaks,
        showClefSigns,
        showFirstMeasureNumber,
        showHand,
        showHeading,
        showLongSlursAndTiesTogether,
        showShortSlursAndTiesTogether,
        slurLongPhraseWithBrackets,
        suppressOctaveMarks,
        upperFirstInNoteFingering,
    )?;
    let is_stream = music21Obj
        .getattr("isStream")
        .and_then(|flag| flag.is_truthy())
        .unwrap_or(false)
        || music21Obj
            .extract::<PyRef<'_, crate::stream::Stream>>()
            .is_ok();
    if is_stream {
        let stream = stream_of(music21Obj)?;
        return translate::stream_to_braille(&stream, &options)
            .map_err(|error| translate_error(&error));
    }
    let Some(element) = crate::stream::element_read(music21Obj)? else {
        return Err(BrailleTranslateException::new_err(format!(
            "{} cannot be transcribed to braille.",
            music21Obj
                .repr()
                .map(|repr| repr.to_string())
                .unwrap_or_default()
        )));
    };
    translate::element_to_braille(element, &options).map_err(|error| translate_error(&error))
}

/// music21's `metadataToString`: a score's title, composer and the rest,
/// written as words or as braille.
#[pyfunction]
#[pyo3(signature = (music21Metadata, returnBrailleUnicode = false))]
fn metadataToString(
    music21Metadata: &Bound<'_, PyAny>,
    returnBrailleUnicode: bool,
) -> PyResult<String> {
    let metadata = crate::stream::metadata_value(music21Metadata)?;
    translate::metadata_to_string(&metadata, returnBrailleUnicode)
        .map_err(|error| translate_error(&error))
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(objectToBraille, m)?)?;
    m.add_function(wrap_pyfunction!(streamToBraille, m)?)?;
    m.add_function(wrap_pyfunction!(scoreToBraille, m)?)?;
    m.add_function(wrap_pyfunction!(metadataToString, m)?)?;
    m.add_function(wrap_pyfunction!(opusToBraille, m)?)?;
    m.add_function(wrap_pyfunction!(measureToBraille, m)?)?;
    m.add_function(wrap_pyfunction!(partToBraille, m)?)?;
    m.add_function(wrap_pyfunction!(keyboardPartsToBraille, m)?)?;
    Ok(())
}
