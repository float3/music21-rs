//! music21's feature extractors, one class per row of the crate's table:
//! its `features.jSymbolic` and `features.native`.
//!
//! Every one of these is a row of `features::jsymbolic::JSYMBOLIC` or
//! `features::native::NATIVE`, which `features_parity` holds to music21
//! vector for vector; what the class adds is music21's own
//! `FeatureExtractor` interface over it -- `extract`, the labels, the blank
//! feature, and the seven things a class says about the feature it reads.
//!
//! **music21's `FeatureExtractor` itself cannot be swapped**, every one of
//! these having been built from it before any of this runs -- a class
//! already made keeps the base it was made from, which `rebind` cannot
//! change. So the base here is a class of this package's own that the
//! ninety-one kinds extend, and it is what carries the interface each kind
//! would otherwise be blocked for.
//!
//! **`LanguageFeature` stays music21's.** It reads which language a text is
//! in, which the crate answers only behind its `language-detection`
//! feature and which the wheel does not turn on, so there is no row to
//! stand behind it.
//!
//! **A class says what it reads before it has read anything.** `fe.id` and
//! `fe.name` are read off the *class* by music21's own `extractorsById`
//! examples, so each is a `#[classattr]` answering out of the crate's row
//! -- which asks music21 nothing, as a class attribute must not.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyDict;

use music21_rs_crate::features::{Extractor as RsExtractor, Library, extractors_by_id};

use crate::features::{FeatureException, feature_object, prepared};

pyo3::create_exception!(
    music21_rs_facade,
    JSymbolicFeatureException,
    FeatureException
);
pyo3::create_exception!(music21_rs_facade, NativeFeatureException, FeatureException);

/// A crate error as the exception the library this row belongs to raises:
/// music21 keeps one for each, and its own doctests read the module each
/// lives in.
fn library_error(library: Library, error: music21_rs_crate::Error) -> PyErr {
    let said = crate::pitch::message(&error);
    match library {
        Library::JSymbolic => JSymbolicFeatureException::new_err(said),
        Library::Native => NativeFeatureException::new_err(said),
    }
}

/// The crate's row for an id in a library, which is what every one of
/// these classes stands for.
///
/// A miss is a table out of step with the music21 beside it rather than
/// anything a caller did, so it is a panic at class-building time and not
/// an error a member answers with.
fn row_of(id: &str, library: Library) -> &'static RsExtractor {
    extractors_by_id(&[id], &[library])
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("this package has no feature extractor with the id {id}"))
}

/// What music21's `FeatureExtractor` answers, over one row of the crate's
/// table.
#[pyclass(
    name = "FeatureExtractor",
    module = "music21.features.base",
    subclass,
    skip_from_py_object,
    dict
)]
pub struct FeatureExtractor {
    /// The crate's reading this class stands for.
    row: &'static RsExtractor,
    /// Which of music21's two libraries it belongs to, which says which
    /// exception a refusal raises.
    library: Library,
}

impl FeatureExtractor {
    /// The base of a kind standing for a row.
    pub(crate) fn of(row: &'static RsExtractor, library: Library) -> Self {
        Self { row, library }
    }
}

#[pymethods]
impl FeatureExtractor {
    /// music21's `FeatureExtractor(dataOrStream=None)`.
    ///
    /// The piece is taken in `__init__` rather than in `__new__`, since a
    /// subclass of music21's hands `__new__` its own arguments and calls
    /// `super().__init__` with music21's.
    #[pyo3(signature = (dataOrStream = None, **_keywords))]
    fn __init__(
        slf: &Bound<'_, Self>,
        dataOrStream: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let py = slf.py();
        slf.setattr("feature", py.None())?;
        slf.call_method1(
            "setData",
            (dataOrStream.map_or_else(|| py.None().into_bound(py), pyo3::Bound::clone),),
        )?;
        Ok(())
    }

    /// The piece this reads from: music21's `setData`, which takes a
    /// stream or one of its own `DataInstance`s.
    fn setData(slf: &Bound<'_, Self>, dataOrStream: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        if dataOrStream.is_none() {
            return Ok(());
        }
        let stream = dataOrStream
            .getattr("isStream")
            .and_then(|flag| flag.is_truthy())
            .unwrap_or(false);
        // music21 keeps the stream it was handed beside the data instance
        // it made of it, and nothing beside a data instance it was handed.
        slf.setattr(
            "stream",
            if stream {
                dataOrStream.clone()
            } else {
                py.None().into_bound(py)
            },
        )?;
        slf.setattr("data", dataOrStream)?;
        Ok(())
    }

    /// What each value of the feature is called in a data set: music21's
    /// `getAttributeLabels`, which is the crate's `attribute_labels`.
    fn getAttributeLabels(&self) -> Vec<String> {
        self.row.attribute_labels()
    }

    /// The feature as it stands before anything is read: music21's
    /// `getBlankFeature`, which is the crate's `blank`.
    fn getBlankFeature<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        feature_object(py, self.row, &self.row.blank())
    }

    /// A blank feature of this kind, kept on the object: music21's
    /// `prepareFeature`.
    fn prepareFeature(slf: &Bound<'_, Self>) -> PyResult<()> {
        let made = slf.borrow().getBlankFeature(slf.py())?;
        slf.setattr("feature", made)?;
        Ok(())
    }

    /// Writes what this class says of the feature onto a feature object:
    /// music21's `fillFeatureAttributes`.
    #[pyo3(signature = (feature = None))]
    fn fillFeatureAttributes<'py>(
        slf: &Bound<'py, Self>,
        feature: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let held = match feature.filter(|given| !given.is_none()) {
            Some(given) => given.clone(),
            None => slf.getattr("feature")?,
        };
        if held.is_none() {
            return Err(FeatureException::new_err(
                "cannot fill attributes without a feature",
            ));
        }
        let row = slf.borrow().row;
        held.setattr("name", row.name())?;
        held.setattr("description", row.description())?;
        held.setattr("isSequential", slf.getattr("isSequential")?)?;
        held.setattr("dimensions", row.dimensions())?;
        held.setattr("discrete", row.discrete())?;
        Ok(held)
    }

    /// Reads the feature and writes it onto the feature this holds:
    /// music21's `process`, which its own subclasses each write for
    /// themselves.
    ///
    /// The crate's `Extractor::extract` reads and scales in one, so what
    /// this writes is already scaled where the row asks for it -- and
    /// `extract` therefore does not scale it again.
    fn process(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let row = slf.borrow().row;
        let held = slf.getattr("data")?;
        if held.is_none() {
            return Err(FeatureException::new_err(
                "this extractor has been given no stream to read",
            ));
        }
        let library = slf.borrow().library;
        let data = prepared(&held)?;
        let found = row
            .extract(&data)
            .map_err(|error| library_error(library, error))?;
        if slf.getattr("feature")?.is_none() {
            Self::prepareFeature(slf)?;
        }
        let feature = slf.getattr("feature")?;
        feature.setattr("vector", crate::features::vector_object(py, &found)?)?;
        Ok(())
    }

    /// The feature this reads of the piece it was given: music21's
    /// `extract`.
    #[pyo3(signature = (source = None))]
    fn extract<'py>(
        slf: &Bound<'py, Self>,
        source: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        if let Some(source) = source.filter(|given| !given.is_none()) {
            slf.setattr("stream", source)?;
        }
        Self::prepareFeature(slf)?;
        Self::process(slf)?;
        slf.getattr("feature")
    }
}

macro_rules! extractors {
    ($(($class:ident, $name:literal, $module:literal, $id:literal, $library:ident, $sequential:literal)),* $(,)?) => {
        $(
            #[doc = concat!("music21's `", $module, ".", $name, "`.")]
            #[pyclass(
                name = $name,
                module = $module,
                extends = FeatureExtractor,
                subclass
            )]
            pub struct $class;

            #[pymethods]
            impl $class {
                /// The piece given is taken by the `__init__` this
                /// inherits, as music21's own subclasses take it.
                #[new]
                #[pyo3(signature = (*_arguments, **_keywords))]
                fn new(
                    _arguments: &Bound<'_, pyo3::types::PyTuple>,
                    _keywords: Option<&Bound<'_, PyDict>>,
                ) -> PyResult<PyClassInitializer<Self>> {
                    Ok(PyClassInitializer::from(FeatureExtractor::of(
                        row_of($id, Library::$library),
                        Library::$library,
                    ))
                    .add_subclass($class))
                }

                /// jSymbolic's code for the feature.
                #[classattr]
                fn id() -> &'static str {
                    row_of($id, Library::$library).id()
                }

                /// What the feature is called.
                #[classattr]
                fn name() -> &'static str {
                    row_of($id, Library::$library).name()
                }

                /// What the feature measures.
                #[classattr]
                fn description() -> &'static str {
                    row_of($id, Library::$library).description()
                }

                /// How many values it has.
                #[classattr]
                fn dimensions() -> usize {
                    row_of($id, Library::$library).dimensions()
                }

                /// Whether the values are counts rather than measurements.
                #[classattr]
                fn discrete() -> bool {
                    row_of($id, Library::$library).discrete()
                }

                /// Whether the values are scaled to sum to one.
                #[classattr]
                fn normalize() -> bool {
                    row_of($id, Library::$library).normalize()
                }

                /// Whether the feature is read of the piece in order,
                /// which every one of these is but the duration.
                #[classattr]
                fn isSequential() -> bool {
                    $sequential
                }
            }
        )*

        pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
            m.add_class::<FeatureExtractor>()?;
            $(m.add_class::<$class>()?;)*
            Ok(())
        }
    };
}

extractors!(
    (
        MelodicIntervalHistogramFeature,
        "MelodicIntervalHistogramFeature",
        "music21.features.jSymbolic",
        "M1",
        JSymbolic,
        true
    ),
    (
        AverageMelodicIntervalFeature,
        "AverageMelodicIntervalFeature",
        "music21.features.jSymbolic",
        "M2",
        JSymbolic,
        true
    ),
    (
        MostCommonMelodicIntervalFeature,
        "MostCommonMelodicIntervalFeature",
        "music21.features.jSymbolic",
        "M3",
        JSymbolic,
        true
    ),
    (
        DistanceBetweenMostCommonMelodicIntervalsFeature,
        "DistanceBetweenMostCommonMelodicIntervalsFeature",
        "music21.features.jSymbolic",
        "M4",
        JSymbolic,
        true
    ),
    (
        MostCommonMelodicIntervalPrevalenceFeature,
        "MostCommonMelodicIntervalPrevalenceFeature",
        "music21.features.jSymbolic",
        "M5",
        JSymbolic,
        true
    ),
    (
        RelativeStrengthOfMostCommonIntervalsFeature,
        "RelativeStrengthOfMostCommonIntervalsFeature",
        "music21.features.jSymbolic",
        "M6",
        JSymbolic,
        true
    ),
    (
        NumberOfCommonMelodicIntervalsFeature,
        "NumberOfCommonMelodicIntervalsFeature",
        "music21.features.jSymbolic",
        "M7",
        JSymbolic,
        true
    ),
    (
        AmountOfArpeggiationFeature,
        "AmountOfArpeggiationFeature",
        "music21.features.jSymbolic",
        "M8",
        JSymbolic,
        true
    ),
    (
        RepeatedNotesFeature,
        "RepeatedNotesFeature",
        "music21.features.jSymbolic",
        "M9",
        JSymbolic,
        true
    ),
    (
        ChromaticMotionFeature,
        "ChromaticMotionFeature",
        "music21.features.jSymbolic",
        "m10",
        JSymbolic,
        true
    ),
    (
        StepwiseMotionFeature,
        "StepwiseMotionFeature",
        "music21.features.jSymbolic",
        "M11",
        JSymbolic,
        true
    ),
    (
        MelodicThirdsFeature,
        "MelodicThirdsFeature",
        "music21.features.jSymbolic",
        "M12",
        JSymbolic,
        true
    ),
    (
        MelodicFifthsFeature,
        "MelodicFifthsFeature",
        "music21.features.jSymbolic",
        "M13",
        JSymbolic,
        true
    ),
    (
        MelodicTritonesFeature,
        "MelodicTritonesFeature",
        "music21.features.jSymbolic",
        "M14",
        JSymbolic,
        true
    ),
    (
        MelodicOctavesFeature,
        "MelodicOctavesFeature",
        "music21.features.jSymbolic",
        "M15",
        JSymbolic,
        true
    ),
    (
        DirectionOfMotionFeature,
        "DirectionOfMotionFeature",
        "music21.features.jSymbolic",
        "m17",
        JSymbolic,
        true
    ),
    (
        DurationOfMelodicArcsFeature,
        "DurationOfMelodicArcsFeature",
        "music21.features.jSymbolic",
        "M18",
        JSymbolic,
        true
    ),
    (
        SizeOfMelodicArcsFeature,
        "SizeOfMelodicArcsFeature",
        "music21.features.jSymbolic",
        "M19",
        JSymbolic,
        true
    ),
    (
        PitchedInstrumentsPresentFeature,
        "PitchedInstrumentsPresentFeature",
        "music21.features.jSymbolic",
        "I1",
        JSymbolic,
        true
    ),
    (
        NotePrevalenceOfPitchedInstrumentsFeature,
        "NotePrevalenceOfPitchedInstrumentsFeature",
        "music21.features.jSymbolic",
        "I3",
        JSymbolic,
        true
    ),
    (
        VariabilityOfNotePrevalenceOfPitchedInstrumentsFeature,
        "VariabilityOfNotePrevalenceOfPitchedInstrumentsFeature",
        "music21.features.jSymbolic",
        "I6",
        JSymbolic,
        true
    ),
    (
        NumberOfPitchedInstrumentsFeature,
        "NumberOfPitchedInstrumentsFeature",
        "music21.features.jSymbolic",
        "I8",
        JSymbolic,
        true
    ),
    (
        StringKeyboardFractionFeature,
        "StringKeyboardFractionFeature",
        "music21.features.jSymbolic",
        "I11",
        JSymbolic,
        true
    ),
    (
        AcousticGuitarFractionFeature,
        "AcousticGuitarFractionFeature",
        "music21.features.jSymbolic",
        "I12",
        JSymbolic,
        true
    ),
    (
        ElectricGuitarFractionFeature,
        "ElectricGuitarFractionFeature",
        "music21.features.jSymbolic",
        "I13",
        JSymbolic,
        true
    ),
    (
        ViolinFractionFeature,
        "ViolinFractionFeature",
        "music21.features.jSymbolic",
        "I14",
        JSymbolic,
        true
    ),
    (
        SaxophoneFractionFeature,
        "SaxophoneFractionFeature",
        "music21.features.jSymbolic",
        "I15",
        JSymbolic,
        true
    ),
    (
        BrassFractionFeature,
        "BrassFractionFeature",
        "music21.features.jSymbolic",
        "I16",
        JSymbolic,
        true
    ),
    (
        WoodwindsFractionFeature,
        "WoodwindsFractionFeature",
        "music21.features.jSymbolic",
        "I17",
        JSymbolic,
        true
    ),
    (
        OrchestralStringsFractionFeature,
        "OrchestralStringsFractionFeature",
        "music21.features.jSymbolic",
        "I18",
        JSymbolic,
        true
    ),
    (
        StringEnsembleFractionFeature,
        "StringEnsembleFractionFeature",
        "music21.features.jSymbolic",
        "I19",
        JSymbolic,
        true
    ),
    (
        ElectricInstrumentFractionFeature,
        "ElectricInstrumentFractionFeature",
        "music21.features.jSymbolic",
        "I20",
        JSymbolic,
        true
    ),
    (
        NoteDensityFeature,
        "NoteDensityFeature",
        "music21.features.jSymbolic",
        "R15",
        JSymbolic,
        true
    ),
    (
        AverageNoteDurationFeature,
        "AverageNoteDurationFeature",
        "music21.features.jSymbolic",
        "R17",
        JSymbolic,
        true
    ),
    (
        VariabilityOfNoteDurationFeature,
        "VariabilityOfNoteDurationFeature",
        "music21.features.jSymbolic",
        "R18",
        JSymbolic,
        true
    ),
    (
        MaximumNoteDurationFeature,
        "MaximumNoteDurationFeature",
        "music21.features.jSymbolic",
        "R19",
        JSymbolic,
        true
    ),
    (
        MinimumNoteDurationFeature,
        "MinimumNoteDurationFeature",
        "music21.features.jSymbolic",
        "R20",
        JSymbolic,
        true
    ),
    (
        StaccatoIncidenceFeature,
        "StaccatoIncidenceFeature",
        "music21.features.jSymbolic",
        "R21",
        JSymbolic,
        true
    ),
    (
        AverageTimeBetweenAttacksFeature,
        "AverageTimeBetweenAttacksFeature",
        "music21.features.jSymbolic",
        "R22",
        JSymbolic,
        true
    ),
    (
        VariabilityOfTimeBetweenAttacksFeature,
        "VariabilityOfTimeBetweenAttacksFeature",
        "music21.features.jSymbolic",
        "R23",
        JSymbolic,
        true
    ),
    (
        AverageTimeBetweenAttacksForEachVoiceFeature,
        "AverageTimeBetweenAttacksForEachVoiceFeature",
        "music21.features.jSymbolic",
        "R24",
        JSymbolic,
        true
    ),
    (
        AverageVariabilityOfTimeBetweenAttacksForEachVoiceFeature,
        "AverageVariabilityOfTimeBetweenAttacksForEachVoiceFeature",
        "music21.features.jSymbolic",
        "R25",
        JSymbolic,
        true
    ),
    (
        InitialTempoFeature,
        "InitialTempoFeature",
        "music21.features.jSymbolic",
        "R30",
        JSymbolic,
        true
    ),
    (
        InitialTimeSignatureFeature,
        "InitialTimeSignatureFeature",
        "music21.features.jSymbolic",
        "R31",
        JSymbolic,
        true
    ),
    (
        CompoundOrSimpleMeterFeature,
        "CompoundOrSimpleMeterFeature",
        "music21.features.jSymbolic",
        "R32",
        JSymbolic,
        true
    ),
    (
        TripleMeterFeature,
        "TripleMeterFeature",
        "music21.features.jSymbolic",
        "R33",
        JSymbolic,
        true
    ),
    (
        QuintupleMeterFeature,
        "QuintupleMeterFeature",
        "music21.features.jSymbolic",
        "R34",
        JSymbolic,
        true
    ),
    (
        ChangesOfMeterFeature,
        "ChangesOfMeterFeature",
        "music21.features.jSymbolic",
        "R35",
        JSymbolic,
        true
    ),
    (
        DurationFeature,
        "DurationFeature",
        "music21.features.jSymbolic",
        "R36",
        JSymbolic,
        false
    ),
    (
        MaximumNumberOfIndependentVoicesFeature,
        "MaximumNumberOfIndependentVoicesFeature",
        "music21.features.jSymbolic",
        "T1",
        JSymbolic,
        true
    ),
    (
        AverageNumberOfIndependentVoicesFeature,
        "AverageNumberOfIndependentVoicesFeature",
        "music21.features.jSymbolic",
        "T2",
        JSymbolic,
        true
    ),
    (
        VariabilityOfNumberOfIndependentVoicesFeature,
        "VariabilityOfNumberOfIndependentVoicesFeature",
        "music21.features.jSymbolic",
        "T3",
        JSymbolic,
        true
    ),
    (
        MostCommonPitchPrevalenceFeature,
        "MostCommonPitchPrevalenceFeature",
        "music21.features.jSymbolic",
        "P1",
        JSymbolic,
        true
    ),
    (
        MostCommonPitchClassPrevalenceFeature,
        "MostCommonPitchClassPrevalenceFeature",
        "music21.features.jSymbolic",
        "P2",
        JSymbolic,
        true
    ),
    (
        RelativeStrengthOfTopPitchesFeature,
        "RelativeStrengthOfTopPitchesFeature",
        "music21.features.jSymbolic",
        "P3",
        JSymbolic,
        true
    ),
    (
        RelativeStrengthOfTopPitchClassesFeature,
        "RelativeStrengthOfTopPitchClassesFeature",
        "music21.features.jSymbolic",
        "P4",
        JSymbolic,
        true
    ),
    (
        IntervalBetweenStrongestPitchesFeature,
        "IntervalBetweenStrongestPitchesFeature",
        "music21.features.jSymbolic",
        "P5",
        JSymbolic,
        true
    ),
    (
        IntervalBetweenStrongestPitchClassesFeature,
        "IntervalBetweenStrongestPitchClassesFeature",
        "music21.features.jSymbolic",
        "P6",
        JSymbolic,
        true
    ),
    (
        NumberOfCommonPitchesFeature,
        "NumberOfCommonPitchesFeature",
        "music21.features.jSymbolic",
        "P7",
        JSymbolic,
        true
    ),
    (
        PitchVarietyFeature,
        "PitchVarietyFeature",
        "music21.features.jSymbolic",
        "P8",
        JSymbolic,
        true
    ),
    (
        PitchClassVarietyFeature,
        "PitchClassVarietyFeature",
        "music21.features.jSymbolic",
        "P9",
        JSymbolic,
        true
    ),
    (
        RangeFeature,
        "RangeFeature",
        "music21.features.jSymbolic",
        "P10",
        JSymbolic,
        true
    ),
    (
        MostCommonPitchFeature,
        "MostCommonPitchFeature",
        "music21.features.jSymbolic",
        "P11",
        JSymbolic,
        true
    ),
    (
        PrimaryRegisterFeature,
        "PrimaryRegisterFeature",
        "music21.features.jSymbolic",
        "P12",
        JSymbolic,
        true
    ),
    (
        ImportanceOfBassRegisterFeature,
        "ImportanceOfBassRegisterFeature",
        "music21.features.jSymbolic",
        "P13",
        JSymbolic,
        true
    ),
    (
        ImportanceOfMiddleRegisterFeature,
        "ImportanceOfMiddleRegisterFeature",
        "music21.features.jSymbolic",
        "P14",
        JSymbolic,
        true
    ),
    (
        ImportanceOfHighRegisterFeature,
        "ImportanceOfHighRegisterFeature",
        "music21.features.jSymbolic",
        "P15",
        JSymbolic,
        true
    ),
    (
        MostCommonPitchClassFeature,
        "MostCommonPitchClassFeature",
        "music21.features.jSymbolic",
        "P16",
        JSymbolic,
        true
    ),
    (
        BasicPitchHistogramFeature,
        "BasicPitchHistogramFeature",
        "music21.features.jSymbolic",
        "P19",
        JSymbolic,
        true
    ),
    (
        PitchClassDistributionFeature,
        "PitchClassDistributionFeature",
        "music21.features.jSymbolic",
        "P20",
        JSymbolic,
        true
    ),
    (
        FifthsPitchHistogramFeature,
        "FifthsPitchHistogramFeature",
        "music21.features.jSymbolic",
        "P21",
        JSymbolic,
        true
    ),
    (
        QualityFeature,
        "QualityFeature",
        "music21.features.jSymbolic",
        "P22",
        JSymbolic,
        true
    ),
    (
        NativeQualityFeature,
        "QualityFeature",
        "music21.features.native",
        "P22",
        Native,
        true
    ),
    (
        TonalCertainty,
        "TonalCertainty",
        "music21.features.native",
        "K1",
        Native,
        true
    ),
    (
        UniqueNoteQuarterLengths,
        "UniqueNoteQuarterLengths",
        "music21.features.native",
        "QL1",
        Native,
        true
    ),
    (
        MostCommonNoteQuarterLength,
        "MostCommonNoteQuarterLength",
        "music21.features.native",
        "QL2",
        Native,
        true
    ),
    (
        MostCommonNoteQuarterLengthPrevalence,
        "MostCommonNoteQuarterLengthPrevalence",
        "music21.features.native",
        "QL3",
        Native,
        true
    ),
    (
        RangeOfNoteQuarterLengths,
        "RangeOfNoteQuarterLengths",
        "music21.features.native",
        "QL4",
        Native,
        true
    ),
    (
        UniquePitchClassSetSimultaneities,
        "UniquePitchClassSetSimultaneities",
        "music21.features.native",
        "CS1",
        Native,
        true
    ),
    (
        UniqueSetClassSimultaneities,
        "UniqueSetClassSimultaneities",
        "music21.features.native",
        "CS2",
        Native,
        true
    ),
    (
        MostCommonPitchClassSetSimultaneityPrevalence,
        "MostCommonPitchClassSetSimultaneityPrevalence",
        "music21.features.native",
        "CS3",
        Native,
        true
    ),
    (
        MostCommonSetClassSimultaneityPrevalence,
        "MostCommonSetClassSimultaneityPrevalence",
        "music21.features.native",
        "CS4",
        Native,
        true
    ),
    (
        MajorTriadSimultaneityPrevalence,
        "MajorTriadSimultaneityPrevalence",
        "music21.features.native",
        "CS5",
        Native,
        true
    ),
    (
        MinorTriadSimultaneityPrevalence,
        "MinorTriadSimultaneityPrevalence",
        "music21.features.native",
        "CS6",
        Native,
        true
    ),
    (
        DominantSeventhSimultaneityPrevalence,
        "DominantSeventhSimultaneityPrevalence",
        "music21.features.native",
        "CS7",
        Native,
        true
    ),
    (
        DiminishedTriadSimultaneityPrevalence,
        "DiminishedTriadSimultaneityPrevalence",
        "music21.features.native",
        "CS8",
        Native,
        true
    ),
    (
        TriadSimultaneityPrevalence,
        "TriadSimultaneityPrevalence",
        "music21.features.native",
        "CS9",
        Native,
        true
    ),
    (
        DiminishedSeventhSimultaneityPrevalence,
        "DiminishedSeventhSimultaneityPrevalence",
        "music21.features.native",
        "CS10",
        Native,
        true
    ),
    (
        IncorrectlySpelledTriadPrevalence,
        "IncorrectlySpelledTriadPrevalence",
        "music21.features.native",
        "CS11",
        Native,
        true
    ),
    (
        ChordBassMotionFeature,
        "ChordBassMotionFeature",
        "music21.features.native",
        "CS12",
        Native,
        true
    ),
    (
        LandiniCadence,
        "LandiniCadence",
        "music21.features.native",
        "MC1",
        Native,
        true
    ),
);

/// The names this facade replaces in `music21.features.jSymbolic`.
pub const JSYMBOLIC_NAMES: &[&str] = &[
    "MelodicIntervalHistogramFeature",
    "AverageMelodicIntervalFeature",
    "MostCommonMelodicIntervalFeature",
    "DistanceBetweenMostCommonMelodicIntervalsFeature",
    "MostCommonMelodicIntervalPrevalenceFeature",
    "RelativeStrengthOfMostCommonIntervalsFeature",
    "NumberOfCommonMelodicIntervalsFeature",
    "AmountOfArpeggiationFeature",
    "RepeatedNotesFeature",
    "ChromaticMotionFeature",
    "StepwiseMotionFeature",
    "MelodicThirdsFeature",
    "MelodicFifthsFeature",
    "MelodicTritonesFeature",
    "MelodicOctavesFeature",
    "DirectionOfMotionFeature",
    "DurationOfMelodicArcsFeature",
    "SizeOfMelodicArcsFeature",
    "PitchedInstrumentsPresentFeature",
    "NotePrevalenceOfPitchedInstrumentsFeature",
    "VariabilityOfNotePrevalenceOfPitchedInstrumentsFeature",
    "NumberOfPitchedInstrumentsFeature",
    "StringKeyboardFractionFeature",
    "AcousticGuitarFractionFeature",
    "ElectricGuitarFractionFeature",
    "ViolinFractionFeature",
    "SaxophoneFractionFeature",
    "BrassFractionFeature",
    "WoodwindsFractionFeature",
    "OrchestralStringsFractionFeature",
    "StringEnsembleFractionFeature",
    "ElectricInstrumentFractionFeature",
    "NoteDensityFeature",
    "AverageNoteDurationFeature",
    "VariabilityOfNoteDurationFeature",
    "MaximumNoteDurationFeature",
    "MinimumNoteDurationFeature",
    "StaccatoIncidenceFeature",
    "AverageTimeBetweenAttacksFeature",
    "VariabilityOfTimeBetweenAttacksFeature",
    "AverageTimeBetweenAttacksForEachVoiceFeature",
    "AverageVariabilityOfTimeBetweenAttacksForEachVoiceFeature",
    "InitialTempoFeature",
    "InitialTimeSignatureFeature",
    "CompoundOrSimpleMeterFeature",
    "TripleMeterFeature",
    "QuintupleMeterFeature",
    "ChangesOfMeterFeature",
    "DurationFeature",
    "MaximumNumberOfIndependentVoicesFeature",
    "AverageNumberOfIndependentVoicesFeature",
    "VariabilityOfNumberOfIndependentVoicesFeature",
    "MostCommonPitchPrevalenceFeature",
    "MostCommonPitchClassPrevalenceFeature",
    "RelativeStrengthOfTopPitchesFeature",
    "RelativeStrengthOfTopPitchClassesFeature",
    "IntervalBetweenStrongestPitchesFeature",
    "IntervalBetweenStrongestPitchClassesFeature",
    "NumberOfCommonPitchesFeature",
    "PitchVarietyFeature",
    "PitchClassVarietyFeature",
    "RangeFeature",
    "MostCommonPitchFeature",
    "PrimaryRegisterFeature",
    "ImportanceOfBassRegisterFeature",
    "ImportanceOfMiddleRegisterFeature",
    "ImportanceOfHighRegisterFeature",
    "MostCommonPitchClassFeature",
    "BasicPitchHistogramFeature",
    "PitchClassDistributionFeature",
    "FifthsPitchHistogramFeature",
    "QualityFeature",
];

/// The names it replaces in `music21.features.native`.
///
/// `LanguageFeature` is not among them: it reads which language a text is
/// in, which the crate answers only behind its `language-detection` feature
/// and which the wheel does not turn on.
pub const NATIVE_NAMES: &[&str] = &[
    "QualityFeature",
    "TonalCertainty",
    "UniqueNoteQuarterLengths",
    "MostCommonNoteQuarterLength",
    "MostCommonNoteQuarterLengthPrevalence",
    "RangeOfNoteQuarterLengths",
    "UniquePitchClassSetSimultaneities",
    "UniqueSetClassSimultaneities",
    "MostCommonPitchClassSetSimultaneityPrevalence",
    "MostCommonSetClassSimultaneityPrevalence",
    "MajorTriadSimultaneityPrevalence",
    "MinorTriadSimultaneityPrevalence",
    "DominantSeventhSimultaneityPrevalence",
    "DiminishedTriadSimultaneityPrevalence",
    "TriadSimultaneityPrevalence",
    "DiminishedSeventhSimultaneityPrevalence",
    "IncorrectlySpelledTriadPrevalence",
    "ChordBassMotionFeature",
    "LandiniCadence",
];
