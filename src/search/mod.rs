//! Finding music in streams: music21's `search` package. A
//! [`LyricSearcher`] finds text in a stream's lyrics, and the notes it is
//! sung to.

mod lyrics;

pub use lyrics::{IndexedLyric, LINE_BREAK, LyricIdentifier, LyricMatch, LyricSearcher};
