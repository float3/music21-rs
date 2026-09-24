//! Articulations: music21's `articulations` module.
//!
//! An articulation is a mark on a note that says how it is played: how much
//! louder, how much shorter, and, when a note is split across a tie, which
//! part keeps the mark. music21 has a class for each; they differ in those
//! numbers, and a few carry a field or two more -- a fingering's finger, a
//! string or fret number, a bend -- so here an [`Articulation`] is those
//! values and an [`ArticulationKind`] names the class they started as.
//!
//! music21's hammer-on and pull-off are spanners joining two notes rather
//! than marks on one, and are not articulations here.

use crate::common::stringtools::camel_case_to_hyphen;
use crate::defaults::{FloatType, IntegerType};
use crate::interval::ChromaticInterval;

/// What an articulation of one kind starts out as.
struct KindRow {
    kind: ArticulationKind,
    class: &'static str,
    /// Its ancestors up to `Articulation`, in music21's method resolution
    /// order.
    parents: &'static [&'static str],
    volume_shift: FloatType,
    length_shift: FloatType,
    tie_attach: &'static str,
    /// The fields past the base ones music21's class carries, by music21's
    /// name for them.
    fields: &'static [&'static str],
}

/// One of music21's articulation classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ArticulationKind {
    /// music21's `articulations.Articulation`.
    Articulation,
    /// music21's `articulations.Caesura`.
    Caesura,
    /// music21's `articulations.DynamicArticulation`.
    DynamicArticulation,
    /// music21's `articulations.LengthArticulation`.
    LengthArticulation,
    /// music21's `articulations.PitchArticulation`.
    PitchArticulation,
    /// music21's `articulations.TechnicalIndication`.
    TechnicalIndication,
    /// music21's `articulations.TimbreArticulation`.
    TimbreArticulation,
    /// music21's `articulations.Accent`.
    Accent,
    /// music21's `articulations.Bowing`.
    Bowing,
    /// music21's `articulations.BreathMark`.
    BreathMark,
    /// music21's `articulations.DetachedLegato`.
    DetachedLegato,
    /// music21's `articulations.Fingering`.
    Fingering,
    /// music21's `articulations.FretIndication`.
    FretIndication,
    /// music21's `articulations.HandbellIndication`.
    HandbellIndication,
    /// music21's `articulations.Harmonic`.
    Harmonic,
    /// music21's `articulations.HarpIndication`.
    HarpIndication,
    /// music21's `articulations.IndeterminateSlide`.
    IndeterminateSlide,
    /// music21's `articulations.OrganIndication`.
    OrganIndication,
    /// music21's `articulations.Staccato`.
    Staccato,
    /// music21's `articulations.Tenuto`.
    Tenuto,
    /// music21's `articulations.Unstress`.
    Unstress,
    /// music21's `articulations.WindIndication`.
    WindIndication,
    /// music21's `articulations.BrassIndication`.
    BrassIndication,
    /// music21's `articulations.Doit`.
    Doit,
    /// music21's `articulations.DownBow`.
    DownBow,
    /// music21's `articulations.Falloff`.
    Falloff,
    /// music21's `articulations.FretBend`.
    FretBend,
    /// music21's `articulations.FretTap`.
    FretTap,
    /// music21's `articulations.HarpFingerNails`.
    HarpFingerNails,
    /// music21's `articulations.OpenString`.
    OpenString,
    /// music21's `articulations.OrganHeel`.
    OrganHeel,
    /// music21's `articulations.OrganToe`.
    OrganToe,
    /// music21's `articulations.Pizzicato`.
    Pizzicato,
    /// music21's `articulations.Plop`.
    Plop,
    /// music21's `articulations.Scoop`.
    Scoop,
    /// music21's `articulations.Staccatissimo`.
    Staccatissimo,
    /// music21's `articulations.Stopped`.
    Stopped,
    /// music21's `articulations.Stress`.
    Stress,
    /// music21's `articulations.StringIndication`.
    StringIndication,
    /// music21's `articulations.StringThumbPosition`.
    StringThumbPosition,
    /// music21's `articulations.StrongAccent`.
    StrongAccent,
    /// music21's `articulations.TonguingIndication`.
    TonguingIndication,
    /// music21's `articulations.UpBow`.
    UpBow,
    /// music21's `articulations.WoodwindIndication`.
    WoodwindIndication,
    /// music21's `articulations.DoubleTongue`.
    DoubleTongue,
    /// music21's `articulations.FrettedPluck`.
    FrettedPluck,
    /// music21's `articulations.NailPizzicato`.
    NailPizzicato,
    /// music21's `articulations.SnapPizzicato`.
    SnapPizzicato,
    /// music21's `articulations.StringHarmonic`.
    StringHarmonic,
    /// music21's `articulations.TripleTongue`.
    TripleTongue,
    /// music21's `articulations.Spiccato`.
    Spiccato,
    /// music21's `articulations.StringFingering`.
    StringFingering,
}

/// What an articulation of each kind starts out as, and which of the
/// fields beside the base ones its class carries.
const KINDS: [KindRow; 52] = [
    KindRow {
        kind: ArticulationKind::Articulation,
        class: "Articulation",
        parents: &[],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::Caesura,
        class: "Caesura",
        parents: &["Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::DynamicArticulation,
        class: "DynamicArticulation",
        parents: &["Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::LengthArticulation,
        class: "LengthArticulation",
        parents: &["Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "last",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::PitchArticulation,
        class: "PitchArticulation",
        parents: &["Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::TechnicalIndication,
        class: "TechnicalIndication",
        parents: &["Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::TimbreArticulation,
        class: "TimbreArticulation",
        parents: &["Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::Accent,
        class: "Accent",
        parents: &["DynamicArticulation", "Articulation"],
        volume_shift: 0.1,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::Bowing,
        class: "Bowing",
        parents: &["TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::BreathMark,
        class: "BreathMark",
        parents: &["LengthArticulation", "Articulation"],
        volume_shift: 0.0,
        length_shift: 0.7,
        tie_attach: "last",
        fields: &["symbol"],
    },
    KindRow {
        kind: ArticulationKind::DetachedLegato,
        class: "DetachedLegato",
        parents: &["LengthArticulation", "Articulation"],
        volume_shift: 0.0,
        length_shift: 0.9,
        tie_attach: "last",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::Fingering,
        class: "Fingering",
        parents: &["TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &["alternate", "fingerNumber", "substitution"],
    },
    KindRow {
        kind: ArticulationKind::FretIndication,
        class: "FretIndication",
        parents: &["TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &["number"],
    },
    KindRow {
        kind: ArticulationKind::HandbellIndication,
        class: "HandbellIndication",
        parents: &["TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::Harmonic,
        class: "Harmonic",
        parents: &["TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::HarpIndication,
        class: "HarpIndication",
        parents: &["TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::IndeterminateSlide,
        class: "IndeterminateSlide",
        parents: &["PitchArticulation", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::OrganIndication,
        class: "OrganIndication",
        parents: &["TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &["substitution"],
    },
    KindRow {
        kind: ArticulationKind::Staccato,
        class: "Staccato",
        parents: &["LengthArticulation", "Articulation"],
        volume_shift: 0.05,
        length_shift: 0.7,
        tie_attach: "last",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::Tenuto,
        class: "Tenuto",
        parents: &["LengthArticulation", "Articulation"],
        volume_shift: -0.05,
        length_shift: 1.1,
        tie_attach: "last",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::Unstress,
        class: "Unstress",
        parents: &["DynamicArticulation", "Articulation"],
        volume_shift: -0.05,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::WindIndication,
        class: "WindIndication",
        parents: &["TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::BrassIndication,
        class: "BrassIndication",
        parents: &["WindIndication", "TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::Doit,
        class: "Doit",
        parents: &["IndeterminateSlide", "PitchArticulation", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "last",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::DownBow,
        class: "DownBow",
        parents: &["Bowing", "TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::Falloff,
        class: "Falloff",
        parents: &["IndeterminateSlide", "PitchArticulation", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "last",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::FretBend,
        class: "FretBend",
        parents: &["FretIndication", "TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &["bendAlter", "number", "preBend", "release", "withBar"],
    },
    KindRow {
        kind: ArticulationKind::FretTap,
        class: "FretTap",
        parents: &["FretIndication", "TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &["number"],
    },
    KindRow {
        kind: ArticulationKind::HarpFingerNails,
        class: "HarpFingerNails",
        parents: &["HarpIndication", "TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::OpenString,
        class: "OpenString",
        parents: &["Bowing", "TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::OrganHeel,
        class: "OrganHeel",
        parents: &["OrganIndication", "TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &["substitution"],
    },
    KindRow {
        kind: ArticulationKind::OrganToe,
        class: "OrganToe",
        parents: &["OrganIndication", "TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &["substitution"],
    },
    KindRow {
        kind: ArticulationKind::Pizzicato,
        class: "Pizzicato",
        parents: &["Bowing", "TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::Plop,
        class: "Plop",
        parents: &["IndeterminateSlide", "PitchArticulation", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::Scoop,
        class: "Scoop",
        parents: &["IndeterminateSlide", "PitchArticulation", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::Staccatissimo,
        class: "Staccatissimo",
        parents: &["Staccato", "LengthArticulation", "Articulation"],
        volume_shift: 0.05,
        length_shift: 0.5,
        tie_attach: "last",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::Stopped,
        class: "Stopped",
        parents: &["WindIndication", "TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::Stress,
        class: "Stress",
        parents: &["DynamicArticulation", "LengthArticulation", "Articulation"],
        volume_shift: 0.05,
        length_shift: 1.1,
        tie_attach: "last",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::StringIndication,
        class: "StringIndication",
        parents: &["Bowing", "TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &["number"],
    },
    KindRow {
        kind: ArticulationKind::StringThumbPosition,
        class: "StringThumbPosition",
        parents: &["Bowing", "TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::StrongAccent,
        class: "StrongAccent",
        parents: &["Accent", "DynamicArticulation", "Articulation"],
        volume_shift: 0.15,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &["pointDirection"],
    },
    KindRow {
        kind: ArticulationKind::TonguingIndication,
        class: "TonguingIndication",
        parents: &["WindIndication", "TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::UpBow,
        class: "UpBow",
        parents: &["Bowing", "TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::WoodwindIndication,
        class: "WoodwindIndication",
        parents: &["WindIndication", "TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::DoubleTongue,
        class: "DoubleTongue",
        parents: &[
            "TonguingIndication",
            "WindIndication",
            "TechnicalIndication",
            "Articulation",
        ],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::FrettedPluck,
        class: "FrettedPluck",
        parents: &[
            "FretIndication",
            "Fingering",
            "TechnicalIndication",
            "Articulation",
        ],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &["alternate", "fingerNumber", "number", "substitution"],
    },
    KindRow {
        kind: ArticulationKind::NailPizzicato,
        class: "NailPizzicato",
        parents: &["Pizzicato", "Bowing", "TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::SnapPizzicato,
        class: "SnapPizzicato",
        parents: &["Pizzicato", "Bowing", "TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::StringHarmonic,
        class: "StringHarmonic",
        parents: &["Bowing", "Harmonic", "TechnicalIndication", "Articulation"],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &["harmonicType", "pitchType"],
    },
    KindRow {
        kind: ArticulationKind::TripleTongue,
        class: "TripleTongue",
        parents: &[
            "TonguingIndication",
            "WindIndication",
            "TechnicalIndication",
            "Articulation",
        ],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::Spiccato,
        class: "Spiccato",
        parents: &[
            "Staccato",
            "LengthArticulation",
            "Accent",
            "DynamicArticulation",
            "Articulation",
        ],
        volume_shift: 0.1,
        length_shift: 0.7,
        tie_attach: "first",
        fields: &[],
    },
    KindRow {
        kind: ArticulationKind::StringFingering,
        class: "StringFingering",
        parents: &[
            "StringIndication",
            "Bowing",
            "Fingering",
            "TechnicalIndication",
            "Articulation",
        ],
        volume_shift: 0.0,
        length_shift: 1.0,
        tie_attach: "first",
        fields: &["alternate", "fingerNumber", "number", "substitution"],
    },
];

impl ArticulationKind {
    /// Every kind, parents before children.
    pub const ALL: [ArticulationKind; 52] = [
        ArticulationKind::Articulation,
        ArticulationKind::Caesura,
        ArticulationKind::DynamicArticulation,
        ArticulationKind::LengthArticulation,
        ArticulationKind::PitchArticulation,
        ArticulationKind::TechnicalIndication,
        ArticulationKind::TimbreArticulation,
        ArticulationKind::Accent,
        ArticulationKind::Bowing,
        ArticulationKind::BreathMark,
        ArticulationKind::DetachedLegato,
        ArticulationKind::Fingering,
        ArticulationKind::FretIndication,
        ArticulationKind::HandbellIndication,
        ArticulationKind::Harmonic,
        ArticulationKind::HarpIndication,
        ArticulationKind::IndeterminateSlide,
        ArticulationKind::OrganIndication,
        ArticulationKind::Staccato,
        ArticulationKind::Tenuto,
        ArticulationKind::Unstress,
        ArticulationKind::WindIndication,
        ArticulationKind::BrassIndication,
        ArticulationKind::Doit,
        ArticulationKind::DownBow,
        ArticulationKind::Falloff,
        ArticulationKind::FretBend,
        ArticulationKind::FretTap,
        ArticulationKind::HarpFingerNails,
        ArticulationKind::OpenString,
        ArticulationKind::OrganHeel,
        ArticulationKind::OrganToe,
        ArticulationKind::Pizzicato,
        ArticulationKind::Plop,
        ArticulationKind::Scoop,
        ArticulationKind::Staccatissimo,
        ArticulationKind::Stopped,
        ArticulationKind::Stress,
        ArticulationKind::StringIndication,
        ArticulationKind::StringThumbPosition,
        ArticulationKind::StrongAccent,
        ArticulationKind::TonguingIndication,
        ArticulationKind::UpBow,
        ArticulationKind::WoodwindIndication,
        ArticulationKind::DoubleTongue,
        ArticulationKind::FrettedPluck,
        ArticulationKind::NailPizzicato,
        ArticulationKind::SnapPizzicato,
        ArticulationKind::StringHarmonic,
        ArticulationKind::TripleTongue,
        ArticulationKind::Spiccato,
        ArticulationKind::StringFingering,
    ];
}

impl ArticulationKind {
    fn row(self) -> &'static KindRow {
        KINDS
            .iter()
            .find(|row| row.kind == self)
            .expect("every kind has a row")
    }

    /// music21's class name for the kind: `"StrongAccent"`.
    pub fn class_name(self) -> &'static str {
        self.row().class
    }

    /// The kind music21 names by that class.
    pub fn from_class_name(class: &str) -> Option<Self> {
        KINDS
            .iter()
            .find(|row| row.class == class)
            .map(|row| row.kind)
    }

    /// The classes above this one, in music21's method resolution order, up
    /// to `Articulation`. A spiccato is a staccato and an accent both.
    pub fn parents(self) -> &'static [&'static str] {
        self.row().parents
    }

    /// Whether music21's class for the kind carries the field music21 names
    /// `field`: `"fingerNumber"`, `"number"`, `"bendAlter"` and the rest.
    pub fn has_field(self, field: &str) -> bool {
        self.row().fields.contains(&field)
    }
}

/// An articulation: how a note is played, and the numbers that say it.
///
/// Two articulations are equal when they are of one kind, as music21's are;
/// where they sit and what else they say does not count.
///
/// ```
/// use music21_rs::articulations::{Articulation, ArticulationKind};
///
/// let accent = Articulation::of_kind(ArticulationKind::StrongAccent);
/// assert_eq!(accent.name(), "strong accent");
/// assert!(accent.volume_shift() > 0.1);
/// assert!(accent.is_a("Accent"));
/// ```
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Articulation {
    kind: ArticulationKind,
    placement: Option<String>,
    volume_shift: FloatType,
    length_shift: FloatType,
    tie_attach: String,
    display_text: Option<String>,
    finger_number: Option<IntegerType>,
    substitution: bool,
    alternate: bool,
    number: IntegerType,
    point_direction: Option<String>,
    symbol: Option<String>,
    harmonic_type: Option<String>,
    pitch_type: Option<String>,
    bend_alter: Option<ChromaticInterval>,
    pre_bend: bool,
    release: Option<FloatType>,
    with_bar: Option<String>,
}

impl PartialEq for Articulation {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}

impl Default for Articulation {
    /// music21's bare `Articulation`.
    fn default() -> Self {
        Self::of_kind(ArticulationKind::Articulation)
    }
}

impl Articulation {
    /// An articulation as music21's class for `kind` starts out.
    pub fn of_kind(kind: ArticulationKind) -> Self {
        let row = kind.row();
        Self {
            kind,
            placement: None,
            volume_shift: row.volume_shift,
            length_shift: row.length_shift,
            tie_attach: row.tie_attach.to_string(),
            display_text: None,
            finger_number: None,
            substitution: false,
            alternate: false,
            number: 0,
            point_direction: kind.has_field("pointDirection").then(|| "up".to_string()),
            symbol: None,
            harmonic_type: kind
                .has_field("harmonicType")
                .then(|| "natural".to_string()),
            pitch_type: None,
            bend_alter: None,
            pre_bend: false,
            release: None,
            with_bar: None,
        }
    }

    /// The kind of articulation this started as.
    pub fn kind(&self) -> ArticulationKind {
        self.kind
    }

    /// Whether this is an articulation of that music21 class or one below
    /// it: `is_a("Staccato")` holds for a spiccato.
    pub fn is_a(&self, class: &str) -> bool {
        self.kind.class_name() == class || self.kind.parents().contains(&class)
    }

    /// music21's `name`: the class name as lower-case words, so a snap
    /// pizzicato is `"snap pizzicato"`.
    pub fn name(&self) -> String {
        camel_case_to_hyphen(self.kind.class_name(), ' ')
    }

    /// Where the mark sits: `"above"`, `"below"`, or nothing said.
    pub fn placement(&self) -> Option<&str> {
        self.placement.as_deref()
    }

    /// Says where the mark sits.
    pub fn set_placement(&mut self, placement: Option<String>) {
        self.placement = placement;
    }

    /// How much the mark shifts the note's volume, between -1 and 1:
    /// music21's `volumeShift`, which [`crate::Volume::realized_with`]
    /// adds.
    pub fn volume_shift(&self) -> FloatType {
        self.volume_shift
    }

    /// Changes the volume shift, held between -1 and 1.
    pub fn set_volume_shift(&mut self, shift: FloatType) {
        self.volume_shift = shift.clamp(-1.0, 1.0);
    }

    /// What the note's sounding length is multiplied by: music21's
    /// `lengthShift`, 0.7 for a staccato.
    pub fn length_shift(&self) -> FloatType {
        self.length_shift
    }

    /// Changes the length shift.
    pub fn set_length_shift(&mut self, shift: FloatType) {
        self.length_shift = shift;
    }

    /// Which part of a note split across a tie keeps the mark: music21's
    /// `tieAttach`, `"first"`, `"last"` or `"all"`.
    pub fn tie_attach(&self) -> &str {
        &self.tie_attach
    }

    /// Changes which part keeps the mark.
    pub fn set_tie_attach(&mut self, attach: impl Into<String>) {
        self.tie_attach = attach.into();
    }

    /// Text written for the mark instead of its usual sign.
    pub fn display_text(&self) -> Option<&str> {
        self.display_text.as_deref()
    }

    /// Changes the text written for the mark.
    pub fn set_display_text(&mut self, text: Option<String>) {
        self.display_text = text;
    }

    /// A fingering's finger: music21's `fingerNumber`.
    pub fn finger_number(&self) -> Option<IntegerType> {
        self.finger_number
    }

    /// Changes the finger.
    pub fn set_finger_number(&mut self, finger: Option<IntegerType>) {
        self.finger_number = finger;
    }

    /// Whether a fingering or organ mark is a substitution.
    pub fn substitution(&self) -> bool {
        self.substitution
    }

    /// Says whether it is a substitution.
    pub fn set_substitution(&mut self, substitution: bool) {
        self.substitution = substitution;
    }

    /// Whether a fingering is an alternate one.
    pub fn alternate(&self) -> bool {
        self.alternate
    }

    /// Says whether it is an alternate.
    pub fn set_alternate(&mut self, alternate: bool) {
        self.alternate = alternate;
    }

    /// A string or fret indication's number: music21's `number`, nought when
    /// none is given.
    pub fn number(&self) -> IntegerType {
        self.number
    }

    /// Changes the string or fret number.
    pub fn set_number(&mut self, number: IntegerType) {
        self.number = number;
    }

    /// Which way a strong accent points: music21's `pointDirection`.
    pub fn point_direction(&self) -> Option<&str> {
        self.point_direction.as_deref()
    }

    /// Changes which way it points.
    pub fn set_point_direction(&mut self, direction: Option<String>) {
        self.point_direction = direction;
    }

    /// A breath mark's symbol.
    pub fn symbol(&self) -> Option<&str> {
        self.symbol.as_deref()
    }

    /// Changes the breath mark's symbol.
    pub fn set_symbol(&mut self, symbol: Option<String>) {
        self.symbol = symbol;
    }

    /// A string harmonic's type: `"natural"` unless said otherwise.
    pub fn harmonic_type(&self) -> Option<&str> {
        self.harmonic_type.as_deref()
    }

    /// Changes the harmonic's type.
    pub fn set_harmonic_type(&mut self, harmonic_type: Option<String>) {
        self.harmonic_type = harmonic_type;
    }

    /// Which pitch a string harmonic's written note stands for.
    pub fn pitch_type(&self) -> Option<&str> {
        self.pitch_type.as_deref()
    }

    /// Changes which pitch the written note stands for.
    pub fn set_pitch_type(&mut self, pitch_type: Option<String>) {
        self.pitch_type = pitch_type;
    }

    /// How far a fret bend bends the string: music21's `bendAlter`.
    pub fn bend_alter(&self) -> Option<&ChromaticInterval> {
        self.bend_alter.as_ref()
    }

    /// Changes how far the string is bent.
    pub fn set_bend_alter(&mut self, alter: Option<ChromaticInterval>) {
        self.bend_alter = alter;
    }

    /// Whether the string is bent before the note sounds.
    pub fn pre_bend(&self) -> bool {
        self.pre_bend
    }

    /// Says whether the string is bent beforehand.
    pub fn set_pre_bend(&mut self, pre_bend: bool) {
        self.pre_bend = pre_bend;
    }

    /// When the bend is released, in quarter lengths from the start of the
    /// note.
    pub fn release(&self) -> Option<FloatType> {
        self.release
    }

    /// Changes when the bend is released.
    pub fn set_release(&mut self, release: Option<FloatType>) {
        self.release = release;
    }

    /// The whammy-bar movement a bend uses: `"scoop"` or `"dip"`.
    pub fn with_bar(&self) -> Option<&str> {
        self.with_bar.as_deref()
    }

    /// Changes the whammy-bar movement.
    pub fn set_with_bar(&mut self, with_bar: Option<String>) {
        self.with_bar = with_bar;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every answer here is music21's own, from its articulation docstrings.
    #[test]
    fn an_articulation_answers_what_music21_s_answers() {
        let staccato = Articulation::of_kind(ArticulationKind::Staccato);
        assert_eq!(staccato.name(), "staccato");
        assert_eq!(staccato.length_shift(), 0.7);
        assert_eq!(staccato.tie_attach(), "last");
        assert_eq!(
            Articulation::of_kind(ArticulationKind::SnapPizzicato).name(),
            "snap pizzicato"
        );

        let mut first = Articulation::of_kind(ArticulationKind::StrongAccent);
        let mut second = Articulation::of_kind(ArticulationKind::StrongAccent);
        first.set_placement(Some("above".to_string()));
        second.set_placement(Some("below".to_string()));
        assert_eq!(first, second);
        assert_ne!(first, Articulation::of_kind(ArticulationKind::Accent));
        assert_eq!(first.point_direction(), Some("up"));

        let spiccato = Articulation::of_kind(ArticulationKind::Spiccato);
        assert!(spiccato.is_a("Staccato") && spiccato.is_a("Accent"));
        assert_eq!(
            (spiccato.volume_shift(), spiccato.length_shift()),
            (0.1, 0.7)
        );

        let mut accent = Articulation::of_kind(ArticulationKind::Accent);
        accent.set_volume_shift(3.0);
        assert_eq!(accent.volume_shift(), 1.0);
        accent.set_volume_shift(-3.0);
        assert_eq!(accent.volume_shift(), -1.0);

        assert!(ArticulationKind::FrettedPluck.has_field("fingerNumber"));
        assert!(ArticulationKind::FrettedPluck.has_field("number"));
        assert!(!ArticulationKind::Accent.has_field("number"));
        assert_eq!(
            Articulation::of_kind(ArticulationKind::StringHarmonic).harmonic_type(),
            Some("natural")
        );
    }
}
