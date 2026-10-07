//! Finding music in streams: music21's `search` package. A
//! [`StreamSearcher`] and the searches beside it find runs of notes by
//! name, rhythm or anything else; the translations turn notes into text,
//! which the approximate searches compare as Python's `difflib` does; and a
//! [`LyricSearcher`] finds text in a stream's lyrics, and the notes it is
//! sung to.

mod base;
mod difflib;
mod lyrics;

pub use base::{
    Algorithm, Filter, MeasureRhythm, SearchMatch, SearchTerm, Searched, StreamSearcher,
    Translation, approximate_note_search, approximate_note_search_no_rhythm,
    approximate_note_search_only_rhythm, approximate_note_search_weighted, iterated,
    most_common_measure_rhythms, note_name_rhythmic_search, note_name_search, notes_and_rests,
    recursed, rhythmic_search, similarity, translate_diatonic_stream_to_string,
    translate_duration_to_bytes, translate_intervals_and_speed, translate_note_tie_to_byte,
    translate_note_to_byte, translate_note_with_duration_to_bytes, translate_stream_to_string,
    translate_stream_to_string_no_rhythm, translate_stream_to_string_only_rhythm,
};
pub use lyrics::{IndexedLyric, LINE_BREAK, LyricIdentifier, LyricMatch, LyricSearcher};
