use crate::stepname::StepName;

pub mod hexatonicblues;
/// The named scales music21 exposes, realized from a tonic.
pub(crate) mod realized;
pub mod scaletype;
pub mod stepscale;

pub use hexatonicblues::{BluesForm, WeightedHexatonicBlues};
pub use realized::fix_default_octave_for_pitch_list;
pub use scaletype::{
    DegreeComparison, HUMDRUM_SOLFEG_SYLLABLES, SOLFEG_SYLLABLES, Scale, ScaleType, Simplification,
    SolfegVariant,
};
pub use stepscale::StepScale;

pub(crate) const FIFTHS_ORDER_SHARP: [StepName; 7] = [
    StepName::F,
    StepName::C,
    StepName::G,
    StepName::D,
    StepName::A,
    StepName::E,
    StepName::B,
];
