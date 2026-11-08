//! music21's `braille.objects`: elements a stream holds to help braille
//! transcription along, which are never transcribed themselves.
//!
//! They carry nothing but what they are. The transcriber knows each by
//! its class's name, as it knew music21's own, so a division put into a
//! stream here divides the segment just as music21's does.

use pyo3::PyClass;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};

use crate::base::{Footing, Music21Object};

/// The names this module hands out.
pub const NAMES: &[&str] = &[
    "BrailleTranscriptionHelper",
    "BrailleSegmentDivision",
    "BrailleOptionalSegmentDivision",
    "BrailleOptionalNoteDivision",
    "BrailleMusicComma",
    "BrailleExplicitNoteLength",
    "BrailleExplicitNoteLarger",
    "BrailleExplicitNoteSmaller",
    "BrailleExplicitNoteExtraSmaller",
];

/// music21's `BrailleTranscriptionHelper`: something in a stream that is
/// not transcribed but helps with the transcription. It sorts before
/// everything else at its offset.
#[pyclass(
    name = "BrailleTranscriptionHelper",
    module = "music21.braille.objects",
    extends = Music21Object,
    subclass,
    dict,
    weakref
)]
pub struct BrailleTranscriptionHelper;

#[pymethods]
impl BrailleTranscriptionHelper {
    #[new]
    #[pyo3(signature = (*_arguments, **_keywords))]
    fn new(
        _arguments: &Bound<'_, PyTuple>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        crate::base::built(Self)
    }

    #[classattr]
    #[pyo3(name = "classSortOrder")]
    fn class_sort_order() -> i32 {
        -100
    }
}

impl Footing for BrailleTranscriptionHelper {
    fn under<T: PyClass<BaseType = Self>>(value: T) -> PyClassInitializer<T> {
        crate::base::built(BrailleTranscriptionHelper).add_subclass(value)
    }
}

/// A class of this module's on another of them, with nothing of its own.
macro_rules! helper_class {
    ($rust:ident, $name:literal, $base:ident, $doc:literal) => {
        #[doc = $doc]
        #[pyclass(name = $name, module = "music21.braille.objects", extends = $base, subclass)]
        pub struct $rust;

        #[pymethods]
        impl $rust {
            #[new]
            #[pyo3(signature = (*_arguments, **_keywords))]
            fn new(
                _arguments: &Bound<'_, PyTuple>,
                _keywords: Option<&Bound<'_, PyDict>>,
            ) -> PyClassInitializer<Self> {
                crate::base::built($rust)
            }
        }

        impl Footing for $rust {
            fn under<T: PyClass<BaseType = Self>>(value: T) -> PyClassInitializer<T> {
                crate::base::built($rust).add_subclass(value)
            }
        }
    };
}

helper_class!(
    BrailleSegmentDivision,
    "BrailleSegmentDivision",
    BrailleTranscriptionHelper,
    "music21's `BrailleSegmentDivision`: a segment must divide here."
);
helper_class!(
    BrailleOptionalSegmentDivision,
    "BrailleOptionalSegmentDivision",
    BrailleSegmentDivision,
    "music21's `BrailleOptionalSegmentDivision`: a segment may divide here."
);
helper_class!(
    BrailleOptionalNoteDivision,
    "BrailleOptionalNoteDivision",
    BrailleTranscriptionHelper,
    "music21's `BrailleOptionalNoteDivision`: the best place to divide a \
     measure's notes, should the measure need dividing."
);
helper_class!(
    BrailleMusicComma,
    "BrailleMusicComma",
    BrailleTranscriptionHelper,
    "music21's `BrailleMusicComma`: a music comma goes here."
);
helper_class!(
    BrailleExplicitNoteLength,
    "BrailleExplicitNoteLength",
    BrailleTranscriptionHelper,
    "music21's `BrailleExplicitNoteLength`: the next notes' values are said \
     outright."
);
helper_class!(
    BrailleExplicitNoteLarger,
    "BrailleExplicitNoteLarger",
    BrailleExplicitNoteLength,
    "music21's `BrailleExplicitNoteLarger`: the larger of a note's two values."
);
helper_class!(
    BrailleExplicitNoteSmaller,
    "BrailleExplicitNoteSmaller",
    BrailleExplicitNoteLength,
    "music21's `BrailleExplicitNoteSmaller`: the smaller of a note's two values."
);
helper_class!(
    BrailleExplicitNoteExtraSmaller,
    "BrailleExplicitNoteExtraSmaller",
    BrailleExplicitNoteLength,
    "music21's `BrailleExplicitNoteExtraSmaller`: the smallest values."
);

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<BrailleTranscriptionHelper>()?;
    m.add_class::<BrailleSegmentDivision>()?;
    m.add_class::<BrailleOptionalSegmentDivision>()?;
    m.add_class::<BrailleOptionalNoteDivision>()?;
    m.add_class::<BrailleMusicComma>()?;
    m.add_class::<BrailleExplicitNoteLength>()?;
    m.add_class::<BrailleExplicitNoteLarger>()?;
    m.add_class::<BrailleExplicitNoteSmaller>()?;
    m.add_class::<BrailleExplicitNoteExtraSmaller>()?;
    Ok(())
}
