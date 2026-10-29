//! music21's instrument classes, one per kind, each extending its family's
//! as in music21. Written out of music21's `instrument` module alongside
//! `src/instrument/tables.rs`, whose kinds they are.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::instrument::Instrument;

macro_rules! kinds {
    ($(($class:ident, $name:literal, $parent:ident, [$($chain:ident),*])),* $(,)?) => {
        $(
            #[doc = concat!("music21's `instrument.", $name, "`.")]
            #[pyclass(
                name = $name,
                module = "music21.instrument",
                extends = $parent,
                subclass
            )]
            pub struct $class;

            #[pymethods]
            impl $class {
                #[new]
                #[pyo3(signature = (instrumentName = None, **_keywords))]
                fn new(
                    instrumentName: Option<String>,
                    _keywords: Option<&Bound<'_, PyDict>>,
                ) -> PyResult<PyClassInitializer<Self>> {
                    Ok(Instrument::initializer($name, instrumentName)?
                        $(.add_subclass($chain))*
                        .add_subclass($class))
                }
            }
        )*

        pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
            $(m.add_class::<$class>()?;)*
            Ok(())
        }

        /// The names this facade replaces in `music21.instrument`: the
        /// base class, its exception, the four functions and every kind.
        pub const NAMES: &[&str] = &[
            "Instrument",
            "InstrumentException",
            "fromString",
            "instrumentFromMidiProgram",
            "getAllNamesForInstrument",
            "ensembleNameBySize",
            $($name),*
        ];
    };
}

kinds!(
    (BrassInstrument, "BrassInstrument", Instrument, []),
    (Conductor, "Conductor", Instrument, []),
    (Harmonica, "Harmonica", Instrument, []),
    (KeyboardInstrument, "KeyboardInstrument", Instrument, []),
    (Organ, "Organ", Instrument, []),
    (Percussion, "Percussion", Instrument, []),
    (StringInstrument, "StringInstrument", Instrument, []),
    (Vocalist, "Vocalist", Instrument, []),
    (WoodwindInstrument, "WoodwindInstrument", Instrument, []),
    (Accordion, "Accordion", Organ, [Organ]),
    (Alto, "Alto", Vocalist, [Vocalist]),
    (
        Bagpipes,
        "Bagpipes",
        WoodwindInstrument,
        [WoodwindInstrument]
    ),
    (Banjo, "Banjo", StringInstrument, [StringInstrument]),
    (Baritone, "Baritone", Vocalist, [Vocalist]),
    (Bass, "Bass", Vocalist, [Vocalist]),
    (Bassoon, "Bassoon", WoodwindInstrument, [WoodwindInstrument]),
    (Celesta, "Celesta", KeyboardInstrument, [KeyboardInstrument]),
    (Choir, "Choir", Vocalist, [Vocalist]),
    (
        Clarinet,
        "Clarinet",
        WoodwindInstrument,
        [WoodwindInstrument]
    ),
    (
        Clavichord,
        "Clavichord",
        KeyboardInstrument,
        [KeyboardInstrument]
    ),
    (
        Contrabass,
        "Contrabass",
        StringInstrument,
        [StringInstrument]
    ),
    (ElectricOrgan, "ElectricOrgan", Organ, [Organ]),
    (
        EnglishHorn,
        "EnglishHorn",
        WoodwindInstrument,
        [WoodwindInstrument]
    ),
    (Flute, "Flute", WoodwindInstrument, [WoodwindInstrument]),
    (Guitar, "Guitar", StringInstrument, [StringInstrument]),
    (Harp, "Harp", StringInstrument, [StringInstrument]),
    (
        Harpsichord,
        "Harpsichord",
        KeyboardInstrument,
        [KeyboardInstrument]
    ),
    (Horn, "Horn", BrassInstrument, [BrassInstrument]),
    (Koto, "Koto", StringInstrument, [StringInstrument]),
    (Lute, "Lute", StringInstrument, [StringInstrument]),
    (Mandolin, "Mandolin", StringInstrument, [StringInstrument]),
    (Oboe, "Oboe", WoodwindInstrument, [WoodwindInstrument]),
    (Piano, "Piano", KeyboardInstrument, [KeyboardInstrument]),
    (PipeOrgan, "PipeOrgan", Organ, [Organ]),
    (
        PitchedPercussion,
        "PitchedPercussion",
        Percussion,
        [Percussion]
    ),
    (ReedOrgan, "ReedOrgan", Organ, [Organ]),
    (Sampler, "Sampler", KeyboardInstrument, [KeyboardInstrument]),
    (
        Saxophone,
        "Saxophone",
        WoodwindInstrument,
        [WoodwindInstrument]
    ),
    (Shamisen, "Shamisen", StringInstrument, [StringInstrument]),
    (Shehnai, "Shehnai", WoodwindInstrument, [WoodwindInstrument]),
    (Sitar, "Sitar", StringInstrument, [StringInstrument]),
    (Soprano, "Soprano", Vocalist, [Vocalist]),
    (Tenor, "Tenor", Vocalist, [Vocalist]),
    (Trombone, "Trombone", BrassInstrument, [BrassInstrument]),
    (Trumpet, "Trumpet", BrassInstrument, [BrassInstrument]),
    (Tuba, "Tuba", BrassInstrument, [BrassInstrument]),
    (Ukulele, "Ukulele", StringInstrument, [StringInstrument]),
    (Viola, "Viola", StringInstrument, [StringInstrument]),
    (Violin, "Violin", StringInstrument, [StringInstrument]),
    (
        Violoncello,
        "Violoncello",
        StringInstrument,
        [StringInstrument]
    ),
    (
        AcousticBass,
        "AcousticBass",
        Guitar,
        [StringInstrument, Guitar]
    ),
    (
        AcousticGuitar,
        "AcousticGuitar",
        Guitar,
        [StringInstrument, Guitar]
    ),
    (
        AltoSaxophone,
        "AltoSaxophone",
        Saxophone,
        [WoodwindInstrument, Saxophone]
    ),
    (
        BaritoneSaxophone,
        "BaritoneSaxophone",
        Saxophone,
        [WoodwindInstrument, Saxophone]
    ),
    (
        BassClarinet,
        "BassClarinet",
        Clarinet,
        [WoodwindInstrument, Clarinet]
    ),
    (
        BassTrombone,
        "BassTrombone",
        Trombone,
        [BrassInstrument, Trombone]
    ),
    (
        ChurchBells,
        "ChurchBells",
        PitchedPercussion,
        [Percussion, PitchedPercussion]
    ),
    (
        Contrabassoon,
        "Contrabassoon",
        Bassoon,
        [WoodwindInstrument, Bassoon]
    ),
    (
        Dulcimer,
        "Dulcimer",
        PitchedPercussion,
        [Percussion, PitchedPercussion]
    ),
    (
        ElectricBass,
        "ElectricBass",
        Guitar,
        [StringInstrument, Guitar]
    ),
    (
        ElectricGuitar,
        "ElectricGuitar",
        Guitar,
        [StringInstrument, Guitar]
    ),
    (
        ElectricPiano,
        "ElectricPiano",
        Piano,
        [KeyboardInstrument, Piano]
    ),
    (
        FretlessBass,
        "FretlessBass",
        Guitar,
        [StringInstrument, Guitar]
    ),
    (
        Glockenspiel,
        "Glockenspiel",
        PitchedPercussion,
        [Percussion, PitchedPercussion]
    ),
    (
        Gong,
        "Gong",
        PitchedPercussion,
        [Percussion, PitchedPercussion]
    ),
    (
        Handbells,
        "Handbells",
        PitchedPercussion,
        [Percussion, PitchedPercussion]
    ),
    (
        Kalimba,
        "Kalimba",
        PitchedPercussion,
        [Percussion, PitchedPercussion]
    ),
    (
        Marimba,
        "Marimba",
        PitchedPercussion,
        [Percussion, PitchedPercussion]
    ),
    (MezzoSoprano, "MezzoSoprano", Soprano, [Vocalist, Soprano]),
    (Ocarina, "Ocarina", Flute, [WoodwindInstrument, Flute]),
    (PanFlute, "PanFlute", Flute, [WoodwindInstrument, Flute]),
    (Piccolo, "Piccolo", Flute, [WoodwindInstrument, Flute]),
    (Recorder, "Recorder", Flute, [WoodwindInstrument, Flute]),
    (Shakuhachi, "Shakuhachi", Flute, [WoodwindInstrument, Flute]),
    (
        SopranoSaxophone,
        "SopranoSaxophone",
        Saxophone,
        [WoodwindInstrument, Saxophone]
    ),
    (
        SteelDrum,
        "SteelDrum",
        PitchedPercussion,
        [Percussion, PitchedPercussion]
    ),
    (
        TenorSaxophone,
        "TenorSaxophone",
        Saxophone,
        [WoodwindInstrument, Saxophone]
    ),
    (
        Timpani,
        "Timpani",
        PitchedPercussion,
        [Percussion, PitchedPercussion]
    ),
    (
        TubularBells,
        "TubularBells",
        PitchedPercussion,
        [Percussion, PitchedPercussion]
    ),
    (
        UnpitchedPercussion,
        "UnpitchedPercussion",
        Percussion,
        [Percussion]
    ),
    (
        Vibraphone,
        "Vibraphone",
        PitchedPercussion,
        [Percussion, PitchedPercussion]
    ),
    (
        Xylophone,
        "Xylophone",
        PitchedPercussion,
        [Percussion, PitchedPercussion]
    ),
    (
        Agogo,
        "Agogo",
        UnpitchedPercussion,
        [Percussion, UnpitchedPercussion]
    ),
    (
        BellTree,
        "BellTree",
        UnpitchedPercussion,
        [Percussion, UnpitchedPercussion]
    ),
    (
        Cabasa,
        "Cabasa",
        UnpitchedPercussion,
        [Percussion, UnpitchedPercussion]
    ),
    (
        Castanets,
        "Castanets",
        UnpitchedPercussion,
        [Percussion, UnpitchedPercussion]
    ),
    (
        Claves,
        "Claves",
        UnpitchedPercussion,
        [Percussion, UnpitchedPercussion]
    ),
    (
        Cowbell,
        "Cowbell",
        UnpitchedPercussion,
        [Percussion, UnpitchedPercussion]
    ),
    (
        Cymbals,
        "Cymbals",
        UnpitchedPercussion,
        [Percussion, UnpitchedPercussion]
    ),
    (
        Guiro,
        "Guiro",
        UnpitchedPercussion,
        [Percussion, UnpitchedPercussion]
    ),
    (
        Ratchet,
        "Ratchet",
        UnpitchedPercussion,
        [Percussion, UnpitchedPercussion]
    ),
    (
        SandpaperBlocks,
        "SandpaperBlocks",
        UnpitchedPercussion,
        [Percussion, UnpitchedPercussion]
    ),
    (
        Shaker,
        "Shaker",
        UnpitchedPercussion,
        [Percussion, UnpitchedPercussion]
    ),
    (
        Siren,
        "Siren",
        UnpitchedPercussion,
        [Percussion, UnpitchedPercussion]
    ),
    (
        SleighBells,
        "SleighBells",
        UnpitchedPercussion,
        [Percussion, UnpitchedPercussion]
    ),
    (
        TamTam,
        "TamTam",
        UnpitchedPercussion,
        [Percussion, UnpitchedPercussion]
    ),
    (
        TempleBlock,
        "TempleBlock",
        UnpitchedPercussion,
        [Percussion, UnpitchedPercussion]
    ),
    (
        Triangle,
        "Triangle",
        UnpitchedPercussion,
        [Percussion, UnpitchedPercussion]
    ),
    (
        UnpitchedDrum,
        "UnpitchedDrum",
        UnpitchedPercussion,
        [Percussion, UnpitchedPercussion]
    ),
    (
        Vibraslap,
        "Vibraslap",
        UnpitchedPercussion,
        [Percussion, UnpitchedPercussion]
    ),
    (
        Whip,
        "Whip",
        UnpitchedPercussion,
        [Percussion, UnpitchedPercussion]
    ),
    (Whistle, "Whistle", Flute, [WoodwindInstrument, Flute]),
    (
        WindMachine,
        "WindMachine",
        UnpitchedPercussion,
        [Percussion, UnpitchedPercussion]
    ),
    (
        Woodblock,
        "Woodblock",
        UnpitchedPercussion,
        [Percussion, UnpitchedPercussion]
    ),
    (
        BassDrum,
        "BassDrum",
        UnpitchedDrum,
        [Percussion, UnpitchedPercussion, UnpitchedDrum]
    ),
    (
        BongoDrums,
        "BongoDrums",
        UnpitchedDrum,
        [Percussion, UnpitchedPercussion, UnpitchedDrum]
    ),
    (
        Caxixi,
        "Caxixi",
        Shaker,
        [Percussion, UnpitchedPercussion, Shaker]
    ),
    (
        ChineseCymbal,
        "ChineseCymbal",
        Cymbals,
        [Percussion, UnpitchedPercussion, Cymbals]
    ),
    (
        CongaDrum,
        "CongaDrum",
        UnpitchedDrum,
        [Percussion, UnpitchedPercussion, UnpitchedDrum]
    ),
    (
        CrashCymbals,
        "CrashCymbals",
        Cymbals,
        [Percussion, UnpitchedPercussion, Cymbals]
    ),
    (
        Cuica,
        "Cuica",
        UnpitchedDrum,
        [Percussion, UnpitchedPercussion, UnpitchedDrum]
    ),
    (
        FingerCymbals,
        "FingerCymbals",
        Cymbals,
        [Percussion, UnpitchedPercussion, Cymbals]
    ),
    (
        HiHat,
        "HiHat",
        Cymbals,
        [Percussion, UnpitchedPercussion, Cymbals]
    ),
    (
        Maracas,
        "Maracas",
        Shaker,
        [Percussion, UnpitchedPercussion, Shaker]
    ),
    (
        RideCymbal,
        "RideCymbal",
        Cymbals,
        [Percussion, UnpitchedPercussion, Cymbals]
    ),
    (
        SizzleCymbal,
        "SizzleCymbal",
        Cymbals,
        [Percussion, UnpitchedPercussion, Cymbals]
    ),
    (
        SnareDrum,
        "SnareDrum",
        UnpitchedDrum,
        [Percussion, UnpitchedPercussion, UnpitchedDrum]
    ),
    (
        SplashCymbal,
        "SplashCymbal",
        Cymbals,
        [Percussion, UnpitchedPercussion, Cymbals]
    ),
    (
        Surdo,
        "Surdo",
        UnpitchedDrum,
        [Percussion, UnpitchedPercussion, UnpitchedDrum]
    ),
    (
        SuspendedCymbal,
        "SuspendedCymbal",
        Cymbals,
        [Percussion, UnpitchedPercussion, Cymbals]
    ),
    (
        Taiko,
        "Taiko",
        UnpitchedDrum,
        [Percussion, UnpitchedPercussion, UnpitchedDrum]
    ),
    (
        Tambourine,
        "Tambourine",
        UnpitchedDrum,
        [Percussion, UnpitchedPercussion, UnpitchedDrum]
    ),
    (
        TenorDrum,
        "TenorDrum",
        UnpitchedDrum,
        [Percussion, UnpitchedPercussion, UnpitchedDrum]
    ),
    (
        Timbales,
        "Timbales",
        UnpitchedDrum,
        [Percussion, UnpitchedPercussion, UnpitchedDrum]
    ),
    (
        TomTom,
        "TomTom",
        UnpitchedDrum,
        [Percussion, UnpitchedPercussion, UnpitchedDrum]
    ),
);
