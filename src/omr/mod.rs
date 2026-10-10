//! Correcting scores read by optical music recognition: music21's `omr`
//! package. [`correctors`] finds the measures whose rhythm was misread and
//! puts the most likely rhythm in their place; [`evaluators`] measures how
//! close a score comes to its ground truth before and after.

pub mod correctors;
pub mod evaluators;
