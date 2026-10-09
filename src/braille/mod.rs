//! Music in braille: music21's `braille` package.
//!
//! [`lookup`] holds the signs, [`basic`] writes one element at a time,
//! [`text`] lays braille out in lines, [`segment`] cuts a part into
//! segments of groupings, and [`translate`] writes scores, parts and
//! measures.

pub mod basic;
pub(crate) mod equality;
pub mod lookup;
pub mod segment;
pub mod text;
pub mod translate;
