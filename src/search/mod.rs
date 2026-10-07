//! Finding music in streams: music21's `search` package. A
//! [`StreamSearcher`] and the searches beside it find runs of notes by
//! name, rhythm or anything else; the translations turn notes into text,
//! which the approximate searches compare as Python's `difflib` does; and a
//! [`LyricSearcher`] finds text in a stream's lyrics, and the notes it is
//! sung to. [`index_score_parts`] cuts each part of a score into
//! overlapping stretches of that text, and [`score_similarity`] compares
//! every stretch of some scores with every other. A [`SegmentMatcher`]
//! finds rows and sets of pitch classes in the notes of a stream, and
//! labels them there.

mod base;
mod difflib;
mod lyrics;
mod segment;
mod serial;

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
pub use segment::{
    SegmentAt, SegmentSimilarity, Segments, index_score_parts, index_score_parts_with,
    score_similarity, translate_monophonic_part_to_segments,
};
pub use serial::{
    ContiguousSegment, ContiguousSegmentSearcher, Matching, Repetitions, SegmentMatcher,
    SegmentNote,
};
