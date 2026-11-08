//! music21's `common`: the plumbing under the rest of music21, which knows
//! nothing of music. Each module is the music21 module of the same name.

pub mod classtools;
pub mod decorators;
pub mod enums;
pub mod filetools;
pub mod formats;
pub mod misc;
pub mod numbertools;
pub mod objects;
pub mod parallel;
pub mod pathtools;
pub mod stringtools;

/// music21's `common.DEBUG_OFF`: no debugging output.
pub const DEBUG_OFF: i64 = 0;
/// music21's `common.DEBUG_USER`: what a user would want to see.
pub const DEBUG_USER: i64 = 1;
/// music21's `common.DEBUG_DEVEL`: what a developer would.
pub const DEBUG_DEVEL: i64 = 63;
/// music21's `common.DEBUG_ALL`: everything.
pub const DEBUG_ALL: i64 = 255;
